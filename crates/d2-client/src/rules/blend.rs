// Spec: specs/render/blend-modes.md
//! Blend modes: the draw mode → blend table of a cel draw (§1) and its
//! pixel ops through `scene::PixelTables` (§2, `composition.md` §5), the
//! draw mode of a composite component (§3) and of single-cel units and
//! overlays (§4), unit shadows and shadow tiles (§5), translucent walls
//! and roofs (§6). Integer math only.

use crate::scene::{BlendOp, FrameImage, FrameView, GradientKind, MapId, PixelTables, ShadeChain};

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
