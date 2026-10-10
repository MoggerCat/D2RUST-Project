// Spec: specs/tools/facts-render.md (§5)
//! The d2rs side of a rendering fact set: one built frame's draw list as
//! `draws.tsv`, `frame.tsv` and `sprites.tsv`. Reads the world view's
//! output only; nothing here feeds back into the game or the frame.

use std::collections::BTreeMap;
use std::path::Path;

use super::{
    sha256_hex, write, FactsError, Header, DRAWS_FILE, DRAW_COLUMNS, FRAME_COLUMNS, FRAME_FILE,
    FRAME_KEYS, NA, SPRITES_FILE, SPRITE_COLUMNS, UNKNOWN,
};
use crate::frames::{FrameAnchor, FramePart, FrameSetKey, FrameStore, IndexFrame};
use crate::rules::camera::{Camera, CELL_HALF_WIDTH};
use crate::rules::draw_order::weather::SkyDraw;
use crate::rules::placement;
use crate::scene::order::pass;
use crate::scene::{BlendOp, DrawItem, DrawKey, FrameCycle, ItemTag, MapId};
use crate::world_view::weather_view::is_sky_call_path;
use crate::world_view::UnitCall;

/// What the exporter needs besides the items.
pub struct ExportContext<'a> {
    pub frames: &'a FrameStore,
    /// The frame's camera `view.left` (`None`: no camera, tile positions
    /// are `?`).
    pub view_left: Option<i32>,
    /// The unit type of a drawn unit's GUID; `None` when unknown.
    pub unit_type: &'a dyn Fn(u32) -> Option<u8>,
    /// Pass 9's calls (`WorldFrame::sky`), written in place of their
    /// pixel and flash items (§5 r10).
    pub sky: &'a [SkyDraw],
    /// Each drawn unit's cel context direction by GUID
    /// (`WorldFrame::unit_dirs`, §5 r14).
    pub unit_dirs: &'a BTreeMap<u64, u8>,
    /// Cel calls without pixels (`WorldFrame::unit_calls`), sorted by key;
    /// one row each, merged with the items by key (§5 r15).
    pub unit_calls: &'a [UnitCall],
    /// The first of the 256 colour rows of the UI rectangles
    /// (`ViewAssets::color_rows`): a rectangle's colour is its shade map
    /// minus this (§5 r16).
    pub color_rows: Option<MapId>,
    /// `WorldFrame::ui_calls`: a UI cel's op (§5 r18).
    pub ui_calls: &'a [crate::ui::draw::UiCelInfo],
    /// The act's text-colour maps (`ViewAssets::text_colors`): a glyph's
    /// colour index `k` is the position + 1 of its shade map.
    pub text_maps: &'a [MapId],
}

/// The frame-set path prefix of the UI rectangles
/// (`world_view::ui_bind::rect_key`).
const UI_RECT_PREFIX: &str = "d2rs/ui/rect/";

/// `draws.tsv` and `sprites.tsv` rows (without the header and column row).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rows {
    pub draws: Vec<Vec<String>>,
    pub sprites: Vec<Vec<String>>,
}

/// §4 r1: `yoff` as 1.14d's cel holds it (bottom row for a bottom-up cel).
fn cel_yoff(f: &IndexFrame) -> i32 {
    match f.anchor {
        FrameAnchor::Top => f.y_off + f.height as i32 - 1,
        FrameAnchor::Bottom | FrameAnchor::TopDown => f.y_off,
    }
}

/// §5 r4: the call's (X, Y) of a cel item, checked by placing it again.
fn cel_xy(f: &IndexFrame, item: &DrawItem) -> Option<(i32, i32)> {
    let x = item.x - f.x_off;
    let y = match f.anchor {
        FrameAnchor::Top | FrameAnchor::TopDown => item.y - f.y_off,
        FrameAnchor::Bottom => item.y - f.y_off + f.height as i32 - 1,
    };
    let p = placement::place(f, x, y, item.clip);
    (p.x == item.x && p.y == item.y).then_some((x, y))
}

/// The suffix of a derived unit-shadow frame set
/// (`world_view::unit_shadow::shadow_key`).
const SHADOW_SUFFIX: &str = "#shadow";

/// The pass of the weather calls (`draw-order.md` §10, `draw-order-2.md` §11.7).
const SKY_PASS: u32 = pass::UNIDENTIFIED_9;

/// §5 r3 (PROVISIONAL, REC-295): the wrapper name of an item.
fn op(tile: bool, item: &DrawItem) -> &'static str {
    let p = item.key.pass();
    match (tile, p) {
        (true, pass::FLOORS | pass::ROOFS) => "FloorTileDraw",
        (true, pass::SHADOWS) => "ShadowTileDraw",
        (true, _) if matches!(item.blend, BlendOp::IndexTableSrcRow(_)) => "TileDrawTrans",
        (true, _) => "TileDrawLit",
        (false, pass::SHADOWS) => "CelDrawShadow",
        (false, _) => "CelDraw",
    }
}

/// §5 r5 (PROVISIONAL, REC-296): the handed (X, Y) of a tile item.
fn tile_xy(f: &IndexFrame, item: &DrawItem, view_left: Option<i32>) -> Option<(i32, i32)> {
    let origin = (item.x - f.x_off, item.y - f.y_off);
    match item.key.pass() {
        pass::FLOORS | pass::ROOFS => view_left.map(|l| (origin.0 + CELL_HALF_WIDTH - l, origin.1)),
        _ => Some(origin),
    }
}

/// §5 r10: one row per pass-9 call. `DrawLine` (x0, y0, x1, y1, color,
/// alpha) and the flash's `DrawBox` (x0, y0, x1, y1, color, mode),
/// `blend-modes.md` §8 r1–r2: `x`, `y` = x0, y0, `mode` = the color,
/// `at` = [`PASS9_TAG`](super::compare::PASS9_TAG) (§6 r5).
fn sky_rows(sky: &[SkyDraw]) -> Vec<Vec<String>> {
    sky.iter()
        .map(|d| {
            let (op, x, y, color) = match *d {
                SkyDraw::Line { x0, y0, color, .. } => ("DrawLine", x0, y0, color),
                SkyDraw::Flash { x0, y0, .. } => ("DrawBox", x0, y0, SkyDraw::FLASH_COLOR),
            };
            let mut row = vec![NA.to_owned(); DRAW_COLUMNS.len()];
            row[1] = op.into();
            row[6] = x.to_string();
            row[7] = y.to_string();
            row[12] = color.to_string();
            row[16] = super::compare::PASS9_TAG.into();
            row
        })
        .collect()
}

fn num(v: Option<i32>) -> String {
    v.map_or_else(|| UNKNOWN.to_owned(), |v| v.to_string())
}

/// §5 r1: the unit draw `0x00471EC0` starts a unit's run outside the
/// shadow pass; 1.14d's shadow pass calls `CelDrawShadow` with no unit
/// draw (`a1-town-arrival-ama` rows 97–115). A run after the unit's own
/// shadow still starts with it. One unit draw is one draw-order slot: a new
/// slot starts a run even under the same tag (a GUID two unit types share,
/// `gen-ui-hud` monster 1:3 then object 2:3). `last_run` is the previous
/// entry's (tag, shadow, slot) of any kind.
fn unit_row(
    tag: ItemTag,
    shadow: bool,
    slot: u64,
    cx: &ExportContext<'_>,
    last_run: &mut Option<(ItemTag, bool, u64)>,
    draws: &mut Vec<Vec<String>>,
) {
    if let (ItemTag::Unit(guid), false) = (tag, shadow) {
        if *last_run != Some((tag, false, slot)) {
            let unit = match (cx.unit_type)(guid) {
                Some(t) => format!("{t}:{guid}"),
                None => UNKNOWN.to_owned(),
            };
            let mut row = vec![NA.to_owned(); DRAW_COLUMNS.len()];
            row[1] = "unit".into();
            for c in [6, 7, 13] {
                row[c] = UNKNOWN.into();
            }
            row[15] = unit;
            draws.push(row);
        }
    }
    *last_run = Some((tag, shadow, slot));
}

/// §5 r18 / `facts-render.md` §2 r3: the `mode light pal` cells (12–14)
/// of a UI cel by its wrapper's arguments: `CelDraw` and `CelDrawColor`
/// (X, Y, light 0xFF, mode, palette / colour index), `CelDrawEx` (X, Y,
/// skip, lines, mode) and `CelDrawClipped` (X, Y, clip, mode) carry no
/// light and no palette (`-`).
fn ui_cells(info: &crate::ui::draw::UiCelInfo, item: &DrawItem, text: &[MapId], row: &mut [String]) {
    use crate::ui::draw::CelCall;
    let k = if info.text {
        // The glyph's colour: its text map's position + 1, else 0.
        Some(
            item.shade
                .maps()
                .iter()
                .find_map(|m| text.iter().position(|t| t == m))
                .map_or(0, |p| p as i32 + 1),
        )
    } else {
        info.pal
    };
    let pal = k.map_or_else(|| UNKNOWN.to_owned(), |k| k.to_string());
    match info.call {
        CelCall::Draw | CelCall::Color => {
            row[12] = info.mode.to_string();
            row[13] = "0xffffffff".into();
            row[14] = pal;
        }
        CelCall::Ex | CelCall::Clipped => {
            row[12] = info.mode.to_string();
            row[13] = NA.into();
            row[14] = NA.into();
        }
    }
}

/// §5 r15: a cel call without pixels (a component file in no archive):
/// `CelDrawShadow` / `CelDraw`, the file, the context's `dir64` and
/// frame; nothing measured beside (no cel: no size, no `sprites.tsv`
/// row).
fn call_rows(
    c: &UnitCall,
    cx: &ExportContext<'_>,
    last_run: &mut Option<(ItemTag, bool, u64)>,
    draws: &mut Vec<Vec<String>>,
) {
    unit_row(c.tag, c.shadow, c.key.slot(), cx, last_run, draws);
    // §5 r17: the unit draw alone (the body failed the pre-test).
    let Some(path) = &c.path else {
        return;
    };
    let mut row = vec![NA.to_owned(); DRAW_COLUMNS.len()];
    row[1] = if c.shadow { "CelDrawShadow" } else { "CelDraw" }.into();
    row[2] = path.as_str().to_owned();
    row[3] = c.dir64.to_string();
    row[4] = c.frame.to_string();
    for cell in &mut row[6..15] {
        *cell = UNKNOWN.into();
    }
    draws.push(row);
}

/// The rows of one frame's sorted draw list (§5 r1–r7).
pub fn draw_rows(items: &[DrawItem], cx: &ExportContext<'_>) -> Result<Rows, FactsError> {
    let mut draws: Vec<Vec<String>> = Vec::new();
    let mut sprites = BTreeMap::new();
    let mut last: Option<&DrawItem> = None;
    let mut last_tile: Option<(ItemTag, DrawKey, Vec<String>)> = None;
    let mut sky = Some(sky_rows(cx.sky));
    // The last unit-pass entry (item or call): its tag and whether it
    // was a shadow (§5 r1 unit rows).
    let mut last_run: Option<(ItemTag, bool, u64)> = None;
    let mut calls = cx.unit_calls.iter().peekable();
    for item in items {
        // §5 r15: the calls without pixels before this item's key.
        while let Some(c) = calls.next_if(|c| c.key < item.key) {
            call_rows(c, cx, &mut last_run, &mut draws);
            last_tile = None;
        }
        // §5 r10: the pass-9 calls stand where their first drawing is, or
        // before the first later pass when every line is off-screen.
        let call_item = cx
            .frames
            .owner(item.frame)
            .is_some_and(|(k, _)| is_sky_call_path(k.path()));
        if call_item || item.key.pass() > SKY_PASS {
            if let Some(rows) = sky.take() {
                draws.extend(rows);
            }
        }
        if call_item {
            continue;
        }
        // §5 r1: the per-block draws of one tile are one row; two tile
        // records (two draw keys) are two calls.
        if last.is_some_and(|l| {
            l.tag == item.tag
                && l.frame == item.frame
                && l.x == item.x
                && l.y == item.y
                && l.key == item.key
        }) {
            continue;
        }
        // §5 r1: the unit draw `0x00471EC0` starts a unit's run outside
        // the shadow pass; 1.14d's shadow pass calls `CelDrawShadow` with
        // no unit draw (`a1-town-arrival-ama` rows 97–115).
        unit_row(
            item.tag,
            item.key.pass() == pass::SHADOWS,
            item.key.slot(),
            cx,
            &mut last_run,
            &mut draws,
        );
        last = Some(item);
        let (key, index) = cx
            .frames
            .owner(item.frame)
            .ok_or_else(|| FactsError::Export(format!("frame {:?} has no owner", item.frame)))?;
        let f = cx
            .frames
            .frame(item.frame)
            .ok_or_else(|| FactsError::Export(format!("frame {:?} not resident", item.frame)))?;
        let mut row = vec![NA.to_owned(); DRAW_COLUMNS.len()];
        // §5 r16: a UI rectangle (`0x0046EFD0` → `DrawRectangle`) is the
        // call's `DrawBox` row: its left, top and colour.
        if key.path().starts_with(UI_RECT_PREFIX) {
            row[1] = "DrawBox".into();
            row[6] = item.x.to_string();
            row[7] = item.y.to_string();
            row[12] = match (cx.color_rows, item.shade.maps().first()) {
                (Some(base), Some(m)) if m.0 >= base.0 && m.0 - base.0 < 256 => {
                    (m.0 - base.0).to_string()
                }
                _ => UNKNOWN.into(),
            };
            last_tile = None;
            draws.push(row);
            continue;
        }
        row[2] = key.path().to_owned();
        // §5 r10: a pass-4 pool cel is tagged for `--skip-weather`.
        if item.key.pass() == pass::UNIDENTIFIED_4 {
            row[16] = super::compare::POOLS_TAG.into();
        }
        match key.part() {
            FramePart::Tile(t) => {
                row[1] = op(true, item).into();
                row[4] = t.to_string();
                row[5] = UNKNOWN.into();
                let xy = tile_xy(f, item, cx.view_left);
                row[6] = num(xy.map(|p| p.0));
                row[7] = num(xy.map(|p| p.1));
                if row[1] == "TileDrawTrans" {
                    row[12] = UNKNOWN.into();
                }
                if row[1] != "ShadowTileDraw" {
                    row[13] = UNKNOWN.into();
                }
            }
            FramePart::Dir(d) => {
                row[1] = match item.tag {
                    // §5 r18: a UI cel's wrapper.
                    ItemTag::Ui(i) => cx
                        .ui_calls
                        .get(i as usize)
                        .map_or(op(false, item), |c| c.call.op()),
                    _ => op(false, item),
                }
                .into();
                // §5 r14: a unit cel's `dir` is the context's `dir64`, not
                // the file direction the frame set is keyed by.
                let d = match item.tag {
                    ItemTag::Unit(_) => cx.unit_dirs.get(&item.key.slot()).copied().unwrap_or(d),
                    _ => d,
                };
                row[3] = d.to_string();
                row[4] = index.to_string();
                // §5 r6: a unit shadow is drawn from the unit's own cel
                // (`blend-modes.md` §5); d2rs composes it from a derived
                // `#shadow` set of sheared frames. The row names the cel's
                // file and size; the call's X, Y (the sheared image's
                // inverse is not specified) are not measured.
                let (f, xy) = match key.path().strip_suffix(SHADOW_SUFFIX) {
                    Some(base) => {
                        row[2] = base.to_owned();
                        let base_key = FrameSetKey::new(base, key.part())
                            .map_err(|e| FactsError::Export(e.to_string()))?;
                        let id = cx
                            .frames
                            .id(&base_key, index)
                            .map_err(|e| FactsError::Export(format!("{base}: {e}")))?;
                        let base_frame = cx.frames.frame(id).ok_or_else(|| {
                            FactsError::Export(format!("{base} frame {index} not resident"))
                        })?;
                        (base_frame, None)
                    }
                    None => (f, cel_xy(f, item)),
                };
                row[6] = num(xy.map(|p| p.0));
                row[7] = num(xy.map(|p| p.1));
                let size = [
                    f.width.to_string(),
                    f.height.to_string(),
                    f.x_off.to_string(),
                    cel_yoff(f).to_string(),
                ];
                row[8..12].clone_from_slice(&size);
                for cell in &mut row[12..15] {
                    *cell = UNKNOWN.into();
                }
                if let ItemTag::Ui(i) = item.tag {
                    if let Some(info) = cx.ui_calls.get(i as usize) {
                        ui_cells(info, item, cx.text_maps, &mut row);
                    }
                }
                sprites.insert((row[2].clone(), u32::from(d), index), size);
            }
        }
        // §5 r1: a tile's block draws are separate frames (each block with
        // its own offsets) that give the same row: one row per tile.
        let tile_row = matches!(key.part(), FramePart::Tile(_));
        if tile_row
            && last_tile
                .as_ref()
                .is_some_and(|(tag, key, r)| *tag == item.tag && *key == item.key && *r == row)
        {
            continue;
        }
        last_tile = tile_row.then(|| (item.tag, item.key, row.clone()));
        draws.push(row);
    }
    for c in calls {
        call_rows(c, cx, &mut last_run, &mut draws);
    }
    if let Some(rows) = sky.take() {
        draws.extend(rows);
    }
    for (i, r) in draws.iter_mut().enumerate() {
        r[0] = i.to_string();
    }
    let sprites = sprites
        .into_iter()
        .map(|((file, dir, frame), size)| {
            let mut r = vec![file, dir.to_string(), frame.to_string()];
            r.extend(size);
            r
        })
        .collect();
    Ok(Rows { draws, sprites })
}

/// §5 r9: the frame cycle's own draw calls around the items: first
/// `StartDraw(bClear, 0, 0, 0)` with `bClear` the BlankScreen flag of the
/// player's level (`render/composition.md` §3 step 2), last `ClearScreen(0)`
/// when the frame's plan clears after drawing (§3 step 4). Renumbers `i`.
pub fn add_cycle_rows(rows: &mut Rows, blank_screen: bool, clear_after: bool) {
    let mut start = vec![NA.to_owned(); DRAW_COLUMNS.len()];
    start[1] = "StartDraw".into();
    start[6] = u8::from(blank_screen).to_string();
    start[7] = "0".into();
    rows.draws.insert(0, start);
    if clear_after {
        let mut clear = vec![NA.to_owned(); DRAW_COLUMNS.len()];
        clear[1] = "ClearScreen".into();
        clear[6] = "0".into();
        rows.draws.push(clear);
    }
    for (i, r) in rows.draws.iter_mut().enumerate() {
        r[0] = i.to_string();
    }
}

/// The `frame.tsv` values d2rs knows (§5 r7–r8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameState {
    pub seq: u64,
    pub tick: u64,
    pub width: u32,
    pub height: u32,
    pub act: Option<u8>,
    pub level: Option<u16>,
    pub camera: Option<Camera>,
    pub open_mode: Option<u8>,
    pub draws: usize,
    pub index_sha256: Option<String>,
    pub palette_sha256: String,
}

/// `frame.tsv` rows in §3's key order.
pub fn frame_rows(s: &FrameState) -> Vec<Vec<String>> {
    let opt = |v: Option<i64>| v.map_or_else(|| UNKNOWN.to_owned(), |v| v.to_string());
    let cam = s.camera.as_ref();
    let value = |key: &str| -> String {
        match key {
            "seq" => s.seq.to_string(),
            "tick" => s.tick.to_string(),
            "w" => s.width.to_string(),
            "h" => s.height.to_string(),
            "act" => opt(s.act.map(i64::from)),
            "level" => opt(s.level.map(i64::from)),
            "tile_origin_x" => opt(cam.map(|c| i64::from(c.tile.x))),
            "tile_origin_y" => opt(cam.map(|c| i64::from(c.tile.y))),
            "unit_origin_x" => opt(cam.map(|c| i64::from(c.unit.x))),
            "unit_origin_y" => opt(cam.map(|c| i64::from(c.unit.y))),
            "open_mode" => opt(s.open_mode.map(i64::from)),
            "shift_x" => opt(cam.map(|c| i64::from(c.view.shift_x))),
            "draws" => s.draws.to_string(),
            "index_sha256" => s.index_sha256.clone().unwrap_or_else(|| UNKNOWN.into()),
            "palette_sha256" => s.palette_sha256.clone(),
            // §5 r7: not measured by d2rs.
            _ => UNKNOWN.to_owned(),
        }
    };
    FRAME_KEYS
        .iter()
        .map(|(k, _)| vec![(*k).to_owned(), value(k)])
        .collect()
}

/// The three files' text.
pub fn files(header: &Header, rows: &Rows, frame: &FrameState) -> [(&'static str, String); 3] {
    [
        (DRAWS_FILE, write(header, &DRAW_COLUMNS, &rows.draws)),
        (
            FRAME_FILE,
            write(header, &FRAME_COLUMNS, &frame_rows(frame)),
        ),
        (SPRITES_FILE, write(header, &SPRITE_COLUMNS, &rows.sprites)),
    ]
}

/// The `--dump-draws DIR --at-tick N[,M...] [--dump-image]` request of
/// `play` (§5 r13, r19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpRequest {
    pub dir: std::path::PathBuf,
    /// Per dump, the first drawn frame whose server tick is at least this;
    /// strictly increasing, at least one.
    pub at_ticks: Vec<u64>,
    /// §5 r19: also write the composed frame as `frame.png`.
    pub image: bool,
    /// The header's `command` (the command line).
    pub command: String,
}

impl DumpRequest {
    /// The folder of dump `i` (§5 r19): `dir` itself for a single tick,
    /// else `dir/tick-<N>`.
    pub fn dir_for(&self, i: usize) -> std::path::PathBuf {
        match self.at_ticks.as_slice() {
            [_] => self.dir.clone(),
            ticks => self.dir.join(format!("tick-{}", ticks[i])),
        }
    }

    /// The d2rs header of §1 r1.
    pub fn header(&self) -> Header {
        let v = env!("CARGO_PKG_VERSION");
        Header {
            tool: format!("d2-client {v} play --dump-draws"),
            command: self.command.clone(),
            game: format!("d2rs {v}"),
        }
    }
}

/// Everything one dump reads from the play loop.
pub struct DumpFrame<'a> {
    pub world: &'a crate::bridge::world::ClientWorld,
    pub frame: &'a crate::world_view::WorldFrame,
    pub assets: &'a crate::world_view::ViewAssets,
    pub cycle: &'a FrameCycle,
    pub blank_screen: bool,
    pub open_mode: Option<u8>,
    pub seq: u64,
}

/// Writes the three files of `d` (and with `req.image` the frame's
/// `frame.png`, §5 r19) into `dir` (§5).
pub fn dump(req: &DumpRequest, dir: &Path, d: &DumpFrame<'_>) -> Result<(), FactsError> {
    let unit_type = |guid: u32| {
        let mut types = d.world.units.keys().filter(|k| k.guid == guid);
        match (types.next(), types.next()) {
            (Some(k), None) => Some(k.unit_type),
            _ => None,
        }
    };
    let cx = ExportContext {
        frames: &d.assets.frames,
        view_left: d.frame.camera.map(|c| c.view.left),
        unit_type: &unit_type,
        sky: &d.frame.sky,
        unit_dirs: &d.frame.unit_dirs,
        unit_calls: &d.frame.unit_calls,
        color_rows: d.assets.color_rows,
        ui_calls: &d.frame.ui_calls,
        text_maps: d.assets.text_colors.as_ref().map_or(&[][..], |t| &t.maps[..]),
    };
    // §5 r12: the drawer calls without pixels join the items by key.
    let mut all = d.frame.items.clone();
    all.extend_from_slice(&d.frame.calls);
    crate::scene::order(&mut all);
    let mut rows = draw_rows(&all, &cx)?;
    add_cycle_rows(
        &mut rows,
        d.blank_screen,
        d.cycle.plan(d.blank_screen).clear_after,
    );
    // §5 r8: the CPU reference composition onto a copy of the framebuffer.
    let mut cycle = d.cycle.clone();
    let pixels = cycle
        .compose(
            d.blank_screen,
            &d.frame.items,
            &d.assets.frames,
            &d.assets.maps,
        )
        .map(<[u8]>::to_vec)
        .map_err(|e| FactsError::Export(format!("compose: {e}")))?;
    let index = sha256_hex(&pixels);
    let palette: Vec<u8> = d
        .assets
        .palette
        .colors
        .iter()
        .flat_map(|c| [c.r, c.g, c.b])
        .collect();
    let view = cycle.view();
    let state = FrameState {
        seq: d.seq,
        tick: d.world.server_ticks,
        width: view.width,
        height: view.height,
        act: d.world.act.as_ref().map(|a| a.act),
        level: d.world.local_room().map(|r| r.level),
        camera: d.frame.camera,
        open_mode: d.open_mode,
        draws: rows.draws.len(),
        index_sha256: Some(index),
        palette_sha256: sha256_hex(&palette),
    };
    write_dir(dir, &files(&req.header(), &rows, &state))?;
    if req.image {
        let png = indexed_png(view.width, view.height, &pixels, &palette)?;
        let path = dir.join(IMAGE_FILE);
        std::fs::write(&path, png).map_err(|source| FactsError::Io { path, source })?;
    }
    Ok(())
}

/// §5 r19: the composed frame of `--dump-image`.
pub const IMAGE_FILE: &str = "frame.png";

/// An 8-bit palettized PNG of `pixels` (the index bytes as they are, the
/// palette as `PLTE`), the form of `record_frames.py`'s captures.
pub fn indexed_png(
    width: u32,
    height: u32,
    pixels: &[u8],
    palette_rgb: &[u8],
) -> Result<Vec<u8>, FactsError> {
    let err = |e: png::EncodingError| FactsError::Export(format!("png: {e}"));
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width, height);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(palette_rgb.to_vec());
    let mut w = enc.write_header().map_err(err)?;
    w.write_image_data(pixels).map_err(err)?;
    w.finish().map_err(err)?;
    Ok(out)
}

fn write_dir(dir: &Path, files: &[(&'static str, String)]) -> Result<(), FactsError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| FactsError::Io { path, source }
    };
    std::fs::create_dir_all(dir).map_err(io(dir))?;
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::write(&path, text).map_err(io(&path))?;
    }
    Ok(())
}
