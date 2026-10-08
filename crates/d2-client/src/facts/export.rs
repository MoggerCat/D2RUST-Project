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
use crate::frames::{FrameAnchor, FramePart, FrameStore, IndexFrame};
use crate::rules::camera::{Camera, CELL_HALF_WIDTH};
use crate::rules::placement;
use crate::scene::order::pass;
use crate::scene::{BlendOp, DrawItem, FrameCycle, ItemTag};

/// What the exporter needs besides the items.
pub struct ExportContext<'a> {
    pub frames: &'a FrameStore,
    /// The frame's camera `view.left` (`None`: no camera, tile positions
    /// are `?`).
    pub view_left: Option<i32>,
    /// The unit type of a drawn unit's GUID; `None` when unknown.
    pub unit_type: &'a dyn Fn(u32) -> Option<u8>,
}

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

fn num(v: Option<i32>) -> String {
    v.map_or_else(|| UNKNOWN.to_owned(), |v| v.to_string())
}

/// The rows of one frame's sorted draw list (§5 r1–r7).
pub fn draw_rows(items: &[DrawItem], cx: &ExportContext<'_>) -> Result<Rows, FactsError> {
    let mut draws: Vec<Vec<String>> = Vec::new();
    let mut sprites = BTreeMap::new();
    let mut last: Option<&DrawItem> = None;
    for item in items {
        // §5 r1: the per-block draws of one tile are one row.
        if last.is_some_and(|l| {
            l.tag == item.tag && l.frame == item.frame && l.x == item.x && l.y == item.y
        }) {
            continue;
        }
        if let ItemTag::Unit(guid) = item.tag {
            if last.map(|l| l.tag) != Some(item.tag) {
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
        row[2] = key.path().to_owned();
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
                row[1] = op(false, item).into();
                row[3] = d.to_string();
                row[4] = index.to_string();
                let xy = cel_xy(f, item);
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
                sprites.insert((key.path().to_owned(), u32::from(d), index), size);
            }
        }
        draws.push(row);
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

/// The `--dump-draws DIR --at-tick N` request of `play`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpRequest {
    pub dir: std::path::PathBuf,
    /// The first drawn frame whose server tick is at least this.
    pub at_tick: u64,
    /// The header's `command` (the command line).
    pub command: String,
}

impl DumpRequest {
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

/// Writes the three files of `d` into `req.dir` (§5).
pub fn dump(req: &DumpRequest, d: &DumpFrame<'_>) -> Result<(), FactsError> {
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
    };
    let rows = draw_rows(&d.frame.items, &cx)?;
    // §5 r8: the CPU reference composition onto a copy of the framebuffer.
    let mut cycle = d.cycle.clone();
    let index = cycle
        .compose(
            d.blank_screen,
            &d.frame.items,
            &d.assets.frames,
            &d.assets.maps,
        )
        .map(sha256_hex)
        .map_err(|e| FactsError::Export(format!("compose: {e}")))?;
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
    write_dir(&req.dir, &files(&req.header(), &rows, &state))
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
