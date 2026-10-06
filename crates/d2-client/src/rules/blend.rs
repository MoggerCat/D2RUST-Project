// Spec: specs/render/blend-modes.md
//! Blend modes: the draw mode → blend table of a cel draw (§1) and its
//! pixel ops through `scene::PixelTables` (§2, `composition.md` §5), the
//! draw mode of a composite component (§3) and of single-cel units and
//! overlays (§4), unit shadows and shadow tiles (§5: pixel, shape,
//! position and skips), translucent walls and roofs (§6), GDI lines and
//! rectangles (§8). Integer math only.

use crate::scene::{
    BlendOp, DrawItem, FrameId, FrameImage, FrameView, GradientKind, MapId, PixelTables, Rect,
    ShadeChain,
};

use super::camera::{Camera, ClientPos, FrameSize};

use super::shading::{BlockLight, ShadeTables};

/// Opaque draw mode (§1).
pub const MODE_OPAQUE: u8 = 5;
/// The hover highlight (§1, `shading.md` §5).
pub const MODE_HIGHLIGHT: u8 = 7;

/// The six blend tables of the palette-table block (§1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendTable {
    /// Alpha level 0 (PL2 `0x3500`, block `+0x00`).
    A0,
    /// Alpha level 1 (`0x13500`, `+0x04`).
    A1,
    /// Alpha level 2 (`0x23500`, `+0x08`).
    A2,
    /// Additive (`0x33500`, `+0x104`).
    Add,
    /// Multiplicative (`0x43500`, `+0x108`).
    Mul,
    /// Max-component (`0x5B500`, `+0x10C`).
    Max,
}

impl BlendTable {
    /// The table's first row in `tables`.
    pub fn base(self, tables: &ShadeTables) -> MapId {
        match self {
            BlendTable::A0 => tables.alpha[0],
            BlendTable::A1 => tables.alpha[1],
            BlendTable::A2 => tables.alpha[2],
            BlendTable::Add => tables.additive,
            BlendTable::Mul => tables.multiplicative,
            BlendTable::Max => tables.max_component,
        }
    }
}

/// `T` of a draw mode (§1, the getter `0x006C8250`): 0 → `A2`, 1 → `A1`,
/// 2 → `A0`, 3 → `ADD`, 4 → `MUL`, 6 → `MAX`; 5, 7 and any other value
/// (data can give modes outside 0–7) → none.
pub fn mode_table(mode: u8) -> Option<BlendTable> {
    match mode {
        0 => Some(BlendTable::A2),
        1 => Some(BlendTable::A1),
        2 => Some(BlendTable::A0),
        3 => Some(BlendTable::Add),
        4 => Some(BlendTable::Mul),
        6 => Some(BlendTable::Max),
        _ => None,
    }
}

/// The `P`, `L`, `T` a cel draw passes the row drawer (§1, §2;
/// `shading.md` §3): `L` from the light byte `v`, or `H` in mode 7; `T`
/// from the mode.
pub fn cel_tables(tables: &ShadeTables, mode: u8, remap: Option<MapId>, v: u8) -> PixelTables {
    let light = if mode == MODE_HIGHLIGHT {
        Some(tables.highlight)
    } else {
        tables.cel_light(v)
    };
    PixelTables {
        remap,
        light,
        blend: mode_table(mode).map(|t| t.base(tables)),
    }
}

/// The shade chain and blend op of a cel draw (§2 cel drawers, row =
/// destination): `d' = T[256·d + P[s]]`, `T[256·d + L[s]]` (lit: `P`
/// dropped) or `L[P[s]]` without `T`.
pub fn cel_ops(
    tables: &ShadeTables,
    mode: u8,
    remap: Option<MapId>,
    v: u8,
) -> (ShadeChain, BlendOp) {
    cel_tables(tables, mode, remap, v).ops()
}

/// What kind of unit a composite belongs to, for the unit override (§3 r).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitKind {
    Player,
    Monster,
    Other,
}

/// The inputs of the unit override `r` (§3, `0x004DB360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverrideInput {
    pub kind: UnitKind,
    /// Stat 181 `fade`.
    pub fade: i32,
    /// Monster only: some item of its inventory list has item flag
    /// 0x400000 (ethereal). Open question 4: which list.
    pub monster_has_ethereal: bool,
    /// The component's item record: `transparent` (items `+300`) and
    /// `transtbl` (`+301`); `None` without an item.
    pub item_trans: Option<(u8, u8)>,
    /// COF component id of the draw (5 RH, 6 LH, 7 SH).
    pub component: u8,
    /// The item on this component is ethereal.
    pub item_ethereal: bool,
}

/// The unit override mode `r` (§3, first match wins), `None` for no
/// override.
pub fn unit_override(i: &OverrideInput) -> Option<u8> {
    match i.kind {
        UnitKind::Player if i.fade != 0 => return Some(1),
        UnitKind::Monster if (1..=15).contains(&i.fade) => return Some(1),
        UnitKind::Monster if i.fade > 15 => {
            // No override and stop: the item's `transparent` is ignored
            // (Edge case 2).
            return i.monster_has_ethereal.then_some(1);
        }
        _ => {}
    }
    if let Some((transparent, transtbl)) = i.item_trans {
        if transparent != 0 {
            return Some(transtbl);
        }
    }
    if i.kind == UnitKind::Player && i.item_ethereal {
        match i.component {
            5 | 6 => return Some(1),
            7 => return Some(2),
            _ => {}
        }
    }
    None
}

/// Whether the hover target is drawn highlighted (§3 `h`, `0x00464370`):
/// every hover target except an object whose `objects.SubClass` has bit
/// 0x80. `object_subclass` is `None` for non-objects.
pub fn hover_highlighted(is_hover_target: bool, object_subclass: Option<u8>) -> bool {
    is_hover_target && object_subclass.is_none_or(|s| s & 0x80 == 0)
}

/// The draw mode of a composite component (§3 decision). `layer_override`
/// is the COF layer's new level `lv` when its override byte is set.
pub fn component_mode(
    ghostly: bool,
    unit_override: Option<u8>,
    hovered: bool,
    layer_override: Option<u8>,
) -> u8 {
    if ghostly {
        return 1;
    }
    if let Some(r) = unit_override {
        return r;
    }
    match layer_override {
        // An override layer is never highlighted (Edge case 4).
        Some(lv) => lv,
        None if hovered => MODE_HIGHLIGHT,
        None => MODE_OPAQUE,
    }
}

/// The draw mode of a missile (§4): `missiles.Trans` 1 → 3, 2 → 4, else
/// 5; 7 when it is the hover target.
pub fn missile_mode(trans: u8, hovered: bool) -> u8 {
    if hovered {
        return MODE_HIGHLIGHT;
    }
    match trans {
        1 => 3,
        2 => 4,
        _ => MODE_OPAQUE,
    }
}

/// The draw mode of an item on the ground (§4): 5, 7 when hovered.
pub fn item_mode(hovered: bool) -> u8 {
    if hovered {
        MODE_HIGHLIGHT
    } else {
        MODE_OPAQUE
    }
}

/// The draw mode of an overlay (§4): `overlay.Trans` as is. Overlays draw
/// without a colormap (Edge case 5: the blood map is dropped).
pub fn overlay_mode(trans: u8) -> u8 {
    trans
}

/// The ops of a unit shadow pixel (§5 r1, §7): chain `[Z]`, then
/// `d' = A0[256·d + 0]` with Blended Shadows on, else `d' = 0`.
pub fn unit_shadow_ops(tables: &ShadeTables, blended: bool) -> (ShadeChain, BlendOp) {
    let chain = ShadeChain::new(&[tables.zero]).expect("one map");
    if blended {
        (chain, BlendOp::IndexTable(tables.alpha[0]))
    } else {
        (chain, BlendOp::Opaque)
    }
}

/// The ops of a shadow tile (§5): blended `d' = A0[256·d + s]`, else an
/// opaque copy.
pub fn shadow_tile_ops(tables: &ShadeTables, blended: bool) -> (ShadeChain, BlendOp) {
    if blended {
        (ShadeChain::EMPTY, BlendOp::IndexTable(tables.alpha[0]))
    } else {
        (ShadeChain::EMPTY, BlendOp::Opaque)
    }
}

/// A unit shadow of one cel as an image (§5 r2): drawn row `k` is source
/// row `2k` from the bottom, on screen row `y0 − k` from column `x0 − k`,
/// `⌊h / 2⌋` rows, `y0 = Y + trunc(yoff / 2)`, `x0 = X + xoff +
/// trunc(yoff / 2)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowImage {
    /// Opaque where the source pixel is (any non-zero index; the shadow
    /// ops ignore it).
    pub image: FrameImage,
    /// Screen top-left of `image` (the `DrawItem` position).
    pub x: i32,
    pub y: i32,
}

/// Builds the sheared shadow of `cel` (rows top first, the bottom row last)
/// drawn at `(x, y)` with offsets `(xoff, yoff)` (§5 r2). Rows and columns
/// outside the frame and the cel clip are the draw item's clip.
pub fn shadow_image(cel: &FrameView<'_>, x: i32, y: i32, xoff: i32, yoff: i32) -> ShadowImage {
    let (w, h) = (cel.width(), cel.height());
    let rows = h / 2;
    let half = yoff / 2; // truncation toward zero
    let x0 = x + xoff + half;
    let y0 = y + half;
    let width = if rows == 0 { 0 } else { w + rows - 1 };
    let mut pixels = vec![0u8; width as usize * rows as usize];
    let src = cel.pixels();
    for j in 0..rows {
        // Image row j (top first) is drawn row k = rows − 1 − j, source row
        // 2k from the bottom, shifted right by j inside the image.
        let k = rows - 1 - j;
        let sy = (h - 1 - 2 * k) as usize;
        let line = &src[sy * w as usize..(sy + 1) * w as usize];
        let out = &mut pixels[(j * width + j) as usize..(j * width + j + w) as usize];
        out.copy_from_slice(line);
    }
    let lift = rows.saturating_sub(1) as i32;
    ShadowImage {
        image: FrameImage {
            width,
            height: rows,
            pixels,
        },
        x: x0 - lift,
        y: y0 - lift,
    }
}

/// How a wall or roof block with alpha byte `a` is drawn (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WallDraw {
    /// `a` = 0xFF: the lit wall drawer (`shading.md` §4 walls).
    Lit,
    /// `a` < 0xFF: the translucent wall drawer with this table.
    Translucent(BlendTable),
    /// `a` < 0x40: the block is not drawn.
    Hidden,
}

/// The wall drawer of alpha byte `a` (§6): 0xC0…0xFE `A0`, 0x80…0xBF
/// `A1`, 0x40…0x7F `A2`, below 0x40 not drawn.
pub fn wall_draw(a: u8) -> WallDraw {
    match a {
        0xFF => WallDraw::Lit,
        0xC0..=0xFE => WallDraw::Translucent(BlendTable::A0),
        0x80..=0xBF => WallDraw::Translucent(BlendTable::A1),
        0x40..=0x7F => WallDraw::Translucent(BlendTable::A2),
        _ => WallDraw::Hidden,
    }
}

/// The ops of a wall or roof block at screen `(x, y)` with alpha `a` and
/// light corners `c0…c3`, or `None` when the block is not drawn (§6;
/// `shading.md` §4 walls). Lit: flat, unlit or gradient light, opaque.
/// Translucent: always the gradient light map (no flat or unlit branch)
/// and the transposed read `d' = T[256·L[s] + d]`.
pub fn wall_block_ops(
    tables: &ShadeTables,
    a: u8,
    corners: [u8; 4],
    low_quality: bool,
    x: i32,
    y: i32,
) -> Option<(ShadeChain, BlendOp)> {
    match wall_draw(a) {
        WallDraw::Hidden => None,
        WallDraw::Lit => {
            let light = super::shading::wall_block_light(corners, low_quality);
            Some((
                tables.block_chain(light, GradientKind::Wall, x, y),
                BlendOp::Opaque,
            ))
        }
        WallDraw::Translucent(t) => Some((
            tables.block_chain(BlockLight::Gradient(corners), GradientKind::Wall, x, y),
            BlendOp::IndexTableSrcRow(t.base(tables)),
        )),
    }
}

/// Errors of the blend rules. A case the spec leaves open is an error
/// naming it, never a default (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlendError {
    #[error("perspective mode is not GDI (§5 r3: position from 0x004F6760, not specified)")]
    Perspective,
    #[error(
        "GDI line with |dx| = |dy| = {0} > 0: which axis is major (TODO(spec: render/blend-modes.md §8 r1))"
    )]
    LineMajorAxisTie(u32),
    #[error("GDI rectangle y1 {y1} < y0 {y0}: fatal error 0x32 (§8 r2)")]
    RectangleRowsReversed { y0: i32, y1: i32 },
    #[error(
        "GDI rectangle x1 {x1} < x0 {x0} (TODO(spec: render/blend-modes.md §8 r2))"
    )]
    RectangleColumnsReversed { x0: i32, x1: i32 },
}

/// Unit flags `+0xC4` bit 5: no shadow (§5 r3).
pub const UNIT_FLAG_NO_SHADOW: u32 = 1 << 5;
/// State 146 `invis`: no shadow (§5 r3, `0x00639DF0`).
pub const STATE_INVIS: u16 = 146;

/// Whether a unit has no shadow (§5 r3, both shadow draws): unit flags
/// `+0xC4` bit 5 set, or state 146 `invis`.
pub fn unit_shadow_skipped(unit_flags: u32, invis: bool) -> bool {
    unit_flags & UNIT_FLAG_NO_SHADOW != 0 || invis
}

/// The `objects` fields a shadow reads (§5 r3, r4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectShadow {
    /// `Xoffset` (`+0x148`), `Yoffset` (`+0x14C`).
    pub xoffset: i32,
    pub yoffset: i32,
    /// `Draw` (`+0x150`) ≠ 0.
    pub draw: bool,
    /// `BlocksLight` of the object's mode (`+0x118 + mode`) ≠ 0.
    pub blocks_light: bool,
}

/// The motion-record values a shadow position reads (§5 r3, r4;
/// `unit-composite.md` §8): the getters' results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShadowMotion {
    /// `ox`, `oy`, `oz` (`0x004DA0B0`, `0x004DA0D0`, `0x004DA0F0`).
    pub ox: i32,
    pub oy: i32,
    pub oz: i32,
    /// The raw x, y (`0x004DA110`, `0x004DA130`); 0 without a record.
    pub mx: i32,
    pub my: i32,
}

/// The composite unit shadow position (§5 r3, `0x00471620`, types 0–2):
/// `X = px + h + ox − (cx_u − shiftX) − 2`, `Y = py + h + oy − (cy_u −
/// 8)` with `h = oz / 2` (C division); an object (type 2) adds its
/// `Xoffset` / `Yoffset` with no `Draw` test. `perspective` (never in
/// GDI) is [`BlendError::Perspective`]. The skips are
/// [`unit_shadow_skipped`].
pub fn composite_shadow_position(
    camera: &Camera,
    perspective: bool,
    at: ClientPos,
    m: &ShadowMotion,
    object: Option<&ObjectShadow>,
) -> Result<(i32, i32), BlendError> {
    if perspective {
        return Err(BlendError::Perspective);
    }
    let h = m.oz / 2; // Rust `/` truncates toward zero, as C
    let mut x = at.x + h + m.ox - (camera.unit.x - camera.view.shift_x) - 2;
    let mut y = at.y + h + m.oy - (camera.unit.y - 8);
    if let Some(o) = object {
        x += o.xoffset;
        y += o.yoffset;
    }
    Ok((x, y))
}

/// The single-cel shadow position (§5 r4, `0x00471450`, types ≥ 3):
/// `X = px + (mx >> 11) − (cx_u − shiftX)`, `Y = py + (my >> 11) − (cy_u
/// − 8)` (arithmetic shifts; no −2, no `oz`); an object needs `Draw` and
/// `BlocksLight` (else `None`: no shadow), then adds `Xoffset` /
/// `Yoffset`. The skips are [`unit_shadow_skipped`].
pub fn single_cel_shadow_position(
    camera: &Camera,
    perspective: bool,
    at: ClientPos,
    m: &ShadowMotion,
    object: Option<&ObjectShadow>,
) -> Result<Option<(i32, i32)>, BlendError> {
    if perspective {
        return Err(BlendError::Perspective);
    }
    let mut x = at.x + (m.mx >> 11) - (camera.unit.x - camera.view.shift_x);
    let mut y = at.y + (m.my >> 11) - (camera.unit.y - 8);
    if let Some(o) = object {
        if !o.draw || !o.blocks_light {
            return Ok(None);
        }
        x += o.xoffset;
        y += o.yoffset;
    }
    Ok(Some((x, y)))
}

/// The unit shadow position of a unit of `unit_type` (§5 r3, r4): `None`
/// when the unit casts no shadow (flags `+0xC4` bit 5, state `invis`, or
/// an object failing r4's tests); types 0–2 (composite) by
/// [`composite_shadow_position`], 3 and up by
/// [`single_cel_shadow_position`].
#[allow(clippy::too_many_arguments)]
pub fn unit_shadow_position(
    camera: &Camera,
    perspective: bool,
    unit_type: u8,
    unit_flags: u32,
    invis: bool,
    at: ClientPos,
    m: &ShadowMotion,
    object: Option<&ObjectShadow>,
) -> Result<Option<(i32, i32)>, BlendError> {
    if unit_shadow_skipped(unit_flags, invis) {
        return Ok(None);
    }
    if unit_type <= 2 {
        composite_shadow_position(camera, perspective, at, m, object).map(Some)
    } else {
        single_cel_shadow_position(camera, perspective, at, m, object)
    }
}

/// One GDI line or rectangle as a draw item's inputs (§8): `image` holds
/// index 1 on the touched pixels (0 elsewhere), the chain turns 1 into the
/// written source and the op does the write. The clip is the surface
/// `[0, W) × [0, H)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GdiDraw {
    pub image: FrameImage,
    pub x: i32,
    pub y: i32,
    pub clip: Rect,
    pub shade: ShadeChain,
    pub blend: BlendOp,
}

impl GdiDraw {
    /// The draw item of `image` stored as `frame`.
    pub fn item(&self, frame: FrameId) -> DrawItem {
        let mut it = DrawItem::new(frame, self.x, self.y);
        it.clip = self.clip;
        it.shade = self.shade;
        it.blend = self.blend;
        it
    }
}

/// The map row a GDI draw of `color` needs (every entry `color`): pushed
/// once per color, its id is the `color_map` of [`gdi_line`] and
/// [`gdi_rectangle`]. A mapped 0 draws opaque index 0 (`shading.md` §7).
pub fn color_row(color: u8) -> [u8; 256] {
    [color; 256]
}

/// The pixels of a GDI line from `(x0, y0)` to `(x1, y1)` in draw order,
/// unclipped (§8 r1): the first is `(x0, y0)`, then `n = max(|Δx|, |Δy|)`
/// steps along the major axis; the error starts at 0, gains the minor
/// distance each step, and when it **exceeds** the major distance the
/// minor axis advances one pixel toward the end and the error loses the
/// major distance. `|Δx| = |Δy| > 0` is [`BlendError::LineMajorAxisTie`].
pub fn gdi_line_pixels(x0: i32, y0: i32, x1: i32, y1: i32) -> Result<Vec<(i32, i32)>, BlendError> {
    let (dx, dy) = (i64::from(x1) - i64::from(x0), i64::from(y1) - i64::from(y0));
    let (adx, ady) = (dx.unsigned_abs(), dy.unsigned_abs());
    if adx == ady && adx != 0 {
        return Err(BlendError::LineMajorAxisTie(adx as u32));
    }
    let x_major = adx > ady;
    let (major, minor) = if x_major { (adx, ady) } else { (ady, adx) };
    let (sx, sy) = (dx.signum(), dy.signum());
    let (mut x, mut y) = (i64::from(x0), i64::from(y0));
    let mut out = Vec::with_capacity(major as usize + 1);
    out.push((x as i32, y as i32));
    let mut err = 0u64;
    for _ in 0..major {
        err += minor;
        let advance = err > major;
        if advance {
            err -= major;
        }
        if x_major {
            x += sx;
            if advance {
                y += sy;
            }
        } else {
            y += sy;
            if advance {
                x += sx;
            }
        }
        out.push((x as i32, y as i32));
    }
    Ok(out)
}

/// A GDI line (§8 r1, `0x006C8C80`): every pixel of [`gdi_line_pixels`]
/// inside `[0, W) × [0, H)` set to `color` (opaque; the alpha argument is
/// never read). `color_map` is the pushed [`color_row`] of the color.
/// `None` when no pixel is on the surface.
pub fn gdi_line(
    size: FrameSize,
    color_map: MapId,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Result<Option<GdiDraw>, BlendError> {
    let clip = size.rect();
    let pixels: Vec<(i32, i32)> = gdi_line_pixels(x0, y0, x1, y1)?
        .into_iter()
        .filter(|&(x, y)| x >= 0 && y >= 0 && x < size.width && y < size.height)
        .collect();
    let (Some(left), Some(top)) = (
        pixels.iter().map(|p| p.0).min(),
        pixels.iter().map(|p| p.1).min(),
    ) else {
        return Ok(None);
    };
    let right = pixels.iter().map(|p| p.0).max().unwrap_or(left);
    let bottom = pixels.iter().map(|p| p.1).max().unwrap_or(top);
    let (w, h) = ((right - left + 1) as u32, (bottom - top + 1) as u32);
    let mut image = vec![0u8; w as usize * h as usize];
    for (x, y) in pixels {
        image[(y - top) as usize * w as usize + (x - left) as usize] = 1;
    }
    Ok(Some(GdiDraw {
        image: FrameImage {
            width: w,
            height: h,
            pixels: image,
        },
        x: left,
        y: top,
        clip,
        shade: ShadeChain::new(&[color_map]).expect("one map"),
        blend: BlendOp::Opaque,
    }))
}

/// The per-mode value `k` of the blend getter (§1, §8 r2; table
/// `0x0074C5A0`): modes 0–7 → 2, 2, 2, 1, 1, 0, 1, 0; other → 0.
pub fn gdi_mode_value(mode: u8) -> u8 {
    match mode {
        0..=2 => 2,
        3 | 4 | 6 => 1,
        _ => 0,
    }
}

/// A GDI rectangle (§8 r2, `0x006C8A60`). Each coordinate is clamped to
/// `[0, W − 1]` (x) or `[0, H − 1]` (y); nothing is drawn (`None`) when
/// `x0 = x1` or `y0 = y1`; `y1 < y0` is fatal 0x32 (checked after the
/// empty test, in the spec's order); `x1 < x0` is not specified. Pixels:
/// columns `x0 … x1 − 1`, rows `y0 … y1 − 1`, written by `k`
/// ([`gdi_mode_value`]): 0 → `d' = color`; 1 → `d' = T[d]` (row 0 of `T`,
/// column `d`: chain `[Z]` and the transposed read); 2 → `d' = T[256·d +
/// color]`. `color_map` is the pushed [`color_row`] of the color.
#[allow(clippy::too_many_arguments)]
pub fn gdi_rectangle(
    tables: &ShadeTables,
    size: FrameSize,
    color_map: MapId,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    mode: u8,
) -> Result<Option<GdiDraw>, BlendError> {
    let cx = |v: i32| v.clamp(0, (size.width - 1).max(0));
    let cy = |v: i32| v.clamp(0, (size.height - 1).max(0));
    let (x0, y0, x1, y1) = (cx(x0), cy(y0), cx(x1), cy(y1));
    if x0 == x1 || y0 == y1 {
        return Ok(None);
    }
    if y1 < y0 {
        return Err(BlendError::RectangleRowsReversed { y0, y1 });
    }
    if x1 < x0 {
        return Err(BlendError::RectangleColumnsReversed { x0, x1 });
    }
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let table = mode_table(mode).map(|t| t.base(tables));
    let (shade, blend) = match (gdi_mode_value(mode), table) {
        (1, Some(t)) => (
            ShadeChain::new(&[tables.zero]).expect("one map"),
            BlendOp::IndexTableSrcRow(t),
        ),
        (2, Some(t)) => (
            ShadeChain::new(&[color_map]).expect("one map"),
            BlendOp::IndexTable(t),
        ),
        _ => (
            ShadeChain::new(&[color_map]).expect("one map"),
            BlendOp::Opaque,
        ),
    };
    Ok(Some(GdiDraw {
        image: FrameImage {
            width: w,
            height: h,
            pixels: vec![1; w as usize * h as usize],
        },
        x: x0,
        y: y0,
        clip: size.rect(),
        shade,
        blend,
    }))
}
