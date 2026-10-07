// Spec: specs/render/shading.md
//! Shading: the act's palette-table block as map-table rows (§1, with the
//! computed highlight `H` and red `R` maps of §5, §8), the light map of a
//! cel draw (§3), the light of DT1 wall and floor blocks (§4), the hover
//! light byte (§5) and the remap tables `P` (§6). Integer math only; the
//! maps are the act PL2's own bytes (§2), never a color formula.
//!
//! How `P`, `L` and `T` combine is `scene::PixelTables`
//! (`composition.md` §5); which draw mode a draw uses is
//! [`super::blend`].

use d2_formats::palette::{Palette, Pl2};

use crate::scene::{GradientKind, LightGradient, MapId, MapTable, ShadeChain};

/// Light maps of the block (`+0x0C + 4k`, k = 0…31).
pub const LIGHT_MAPS: u32 = 32;
/// Hue, tone and unknown variation maps at PL2 `0x53500` (§1): 111 hue
/// variations, the red, green and blue tones, 14 unknown variations.
pub const REMAP_MAPS: u32 = 128;
/// Highlight factor of `H` (§5): `⌊c · 170 / 100⌋`.
const HIGHLIGHT_NUM: u32 = 170;
const HIGHLIGHT_DEN: u32 = 100;

/// Errors of the shading rules. A question the spec leaves open is an
/// error naming it, never a default (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShadingError {
    #[error(
        "unit palette index {0} is past the 128 remap maps (render/shading.md §6 r1: p = 1…128 → map p − 1)"
    )]
    RemapIndex(u8),
    #[error("floor light grid of {len} cells has no cell {need} for block ({gx}, {gy})")]
    FloorGrid {
        len: usize,
        need: usize,
        gx: u8,
        gy: u8,
    },
}

/// The palette-table block of one act PL2 pushed into a [`MapTable`] (§1):
/// the rows the cel and tile draws read, plus `Z` (all zero), the shadow
/// chain of `blend-modes.md` §7. Every table is pushed as stored, never
/// transposed (blend tables `[destination][source]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShadeTables {
    /// Light map 0; map `k` is row `light0 + k` (§3).
    pub light0: MapId,
    /// Highlight `H` (block `+0x118`, computed, §5).
    pub highlight: MapId,
    /// Red `R` (block `+0x11C`, computed, §8; no GDI reader).
    pub red: MapId,
    /// `Z[i] = 0` for all `i` (unit shadows, `blend-modes.md` §5, §7).
    pub zero: MapId,
    /// First of the 128 remap maps at PL2 `0x53500` (§6 r1).
    pub remap0: MapId,
    /// Alpha blend levels 0, 1, 2 (block `+0x00`, `+0x04`, `+0x08`).
    pub alpha: [MapId; 3],
    /// Additive (block `+0x104`).
    pub additive: MapId,
    /// Multiplicative (block `+0x108`).
    pub multiplicative: MapId,
    /// Max-component (block `+0x10C`).
    pub max_component: MapId,
}

impl ShadeTables {
    /// Pushes the block of `pl2` (the act palette of `composition.md` §4)
    /// into `maps`. `H` and `R` are computed from `pl2`'s base palette.
    pub fn push(maps: &mut MapTable, pl2: &Pl2) -> ShadeTables {
        let light0 = push_rows(maps, &pl2.light_levels);
        let highlight = maps.push(highlight_map(&pl2.base_palette));
        let red = maps.push(red_map(&pl2.base_palette));
        let zero = maps.push([0; 256]);
        let remap0 = push_rows(maps, &pl2.hue_variations);
        maps.push(pl2.red_tones);
        maps.push(pl2.green_tones);
        maps.push(pl2.blue_tones);
        push_rows(maps, &pl2.unknown_variations);
        let alpha = [
            push_rows(maps, &pl2.alpha_blend[0]),
            push_rows(maps, &pl2.alpha_blend[1]),
            push_rows(maps, &pl2.alpha_blend[2]),
        ];
        let additive = push_rows(maps, &pl2.additive_blend);
        let multiplicative = push_rows(maps, &pl2.multiplicative_blend);
        let max_component = push_rows(maps, &pl2.max_component_blend);
        ShadeTables {
            light0,
            highlight,
            red,
            zero,
            remap0,
            alpha,
            additive,
            multiplicative,
            max_component,
        }
    }

    /// Light map `k` (0…31; larger `k` is clamped by the callers' `>> 3`).
    pub fn light_map(&self, k: u8) -> MapId {
        debug_assert!(u32::from(k) < LIGHT_MAPS);
        MapId(self.light0.0 + u32::from(k))
    }

    /// `L` of a cel draw with light byte `v` (§3 r1, r2): none for 0xFF,
    /// else light map `v >> 3`. Draw mode 7 replaces it with
    /// [`ShadeTables::highlight`] (§3 r3, [`super::blend::cel_ops`]).
    pub fn cel_light(&self, v: u8) -> Option<MapId> {
        cel_light_level(v).map(|k| self.light_map(k))
    }

    /// `P` of a unit palette index `p` (unit `+0x6C`, §6 r1): none for
    /// `p = 0`, else remap map `p − 1` (1…111 hue, 112–114 red, green,
    /// blue tones, 115–128 unknown variations).
    pub fn unit_remap(&self, p: u8) -> Result<Option<MapId>, ShadingError> {
        match p {
            0 => Ok(None),
            p if u32::from(p) <= REMAP_MAPS => Ok(Some(MapId(self.remap0.0 + u32::from(p) - 1))),
            p => Err(ShadingError::RemapIndex(p)),
        }
    }

    /// The light gradient of a block at screen `(x, y)` with `corners`.
    pub fn gradient(&self, kind: GradientKind, x: i32, y: i32, corners: [u8; 4]) -> LightGradient {
        LightGradient {
            kind,
            x,
            y,
            corners,
            light0: self.light0,
        }
    }

    /// The shade chain of a lit DT1 block (`shading.md` §10: `[L]`, per
    /// pixel for a gradient): an unlit block has none, a flat one map, a
    /// gradient one the per-pixel map of `kind` at the block's screen
    /// top-left `(x, y)`.
    pub fn block_chain(&self, light: BlockLight, kind: GradientKind, x: i32, y: i32) -> ShadeChain {
        match light {
            BlockLight::Unlit => ShadeChain::EMPTY,
            BlockLight::Flat(k) => ShadeChain::new(&[self.light_map(k)]).expect("one map"),
            BlockLight::Gradient(c) => {
                ShadeChain::EMPTY.with_gradient(self.gradient(kind, x, y, c))
            }
        }
    }
}

fn push_rows(maps: &mut MapTable, rows: &[[u8; 256]]) -> MapId {
    let first = MapId(maps.len() as u32);
    for row in rows {
        maps.push(*row);
    }
    first
}

/// Light map number of a cel light byte `v` (§3): `None` for 0xFF
/// (unlit), else `v >> 3`.
pub fn cel_light_level(v: u8) -> Option<u8> {
    (v != 0xFF).then_some(v >> 3)
}

/// The light byte of a hovered unit, missile or item (§5,
/// `blend-modes.md` §3, §4): doubled and clamped to 0x40…0xFF. The GDI cel
/// draw ignores it in mode 7 (§3 r3); kept for the light record.
pub fn hover_light(v: u8) -> u8 {
    (u32::from(v) * 2).clamp(0x40, 0xFF) as u8
}

/// The palette index nearest to `(r, g, b)` (`0x00605210`, §5): smallest
/// `(R_j − r)² + (G_j − g)² + (B_j − b)²`, the lowest `j` on ties, index 0
/// included.
pub fn nearest(palette: &Palette, r: u32, g: u32, b: u32) -> u8 {
    let dist = |c: &d2_formats::palette::Rgb| {
        let d = |p: u8, q: u32| (i64::from(p) - i64::from(q)).pow(2);
        d(c.r, r) + d(c.g, g) + d(c.b, b)
    };
    let mut best = 0usize;
    for (j, c) in palette.colors.iter().enumerate().skip(1) {
        if dist(c) < dist(&palette.colors[best]) {
            best = j;
        }
    }
    best as u8
}

/// The highlight map `H` (§5): `H[i] = nearest(min(255, ⌊R·170/100⌋), …)`.
pub fn highlight_map(palette: &Palette) -> [u8; 256] {
    let up = |c: u8| (u32::from(c) * HIGHLIGHT_NUM / HIGHLIGHT_DEN).min(255);
    let mut out = [0u8; 256];
    for (o, c) in out.iter_mut().zip(&palette.colors) {
        *o = nearest(palette, up(c.r), up(c.g), up(c.b));
    }
    out
}

/// The red map `R` (§8): `R[i] = nearest(R_i, 0, 0)`.
pub fn red_map(palette: &Palette) -> [u8; 256] {
    let mut out = [0u8; 256];
    for (o, c) in out.iter_mut().zip(&palette.colors) {
        *o = nearest(palette, u32::from(c.r), 0, 0);
    }
    out
}

/// The light of one DT1 block (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockLight {
    /// Copied without a light map (walls only, `c0` in 0xF8…0xFF).
    Unlit,
    /// Every pixel uses light map `k`.
    Flat(u8),
    /// Per-pixel gradient from the corners `c0` (top-left), `c1`
    /// (top-right), `c2` (bottom-right), `c3` (bottom-left).
    Gradient([u8; 4]),
}

/// Light of a wall or roof block from its corner values (§4 walls r1–r4).
/// `low_quality` is the settings `+0x08` word (default off).
pub fn wall_block_light(corners: [u8; 4], low_quality: bool) -> BlockLight {
    let [c0, c1, c2, c3] = corners.map(i32::from);
    let delta = (c1 - c0).abs() + (c3 - c0).abs() + (c2 - c1).abs();
    if !low_quality && delta > 9 {
        BlockLight::Gradient(corners)
    } else if corners[0] >= 0xF8 {
        BlockLight::Unlit
    } else {
        BlockLight::Flat(corners[0] >> 3)
    }
}

/// Light of the floor block at grid `(gx, gy)` (block record bytes 6, 7)
/// from the floor light grid `cells` (byte 0 of each 12-byte cell, 8 cells
/// per row; §4 floors r1, r2). A flat floor block is never unlit.
pub fn floor_block_light(
    cells: &[u8],
    gx: u8,
    gy: u8,
    low_quality: bool,
) -> Result<BlockLight, ShadingError> {
    let g = usize::from(gx) + 8 * usize::from(gy);
    let need = g + 26;
    if need >= cells.len() {
        return Err(ShadingError::FloorGrid {
            len: cells.len(),
            need,
            gx,
            gy,
        });
    }
    let e = |n: usize| u32::from(cells[g + n]);
    let delta = e(10).abs_diff(e(9)) + e(17).abs_diff(e(9)) + e(18).abs_diff(e(10));
    if low_quality || delta < 10 {
        return Ok(BlockLight::Flat(cells[g + 9] >> 3));
    }
    // c2 averages e[g+17], not e[g+18] (Edge case 1: reproduce).
    let avg = |a, b, c, d| ((e(a) + e(b) + e(c) + e(d)) >> 2) as u8;
    Ok(BlockLight::Gradient([
        avg(8, 9, 16, 17),
        avg(1, 2, 9, 10),
        avg(10, 11, 17, 19),
        avg(17, 18, 25, 26),
    ]))
}

/// The shade chain of a floor block at screen `(x, y)` (§4 floors r3,
/// r4): RLE (block flag bit 2) and isometric blocks alike use the 15-row
/// gradient `a_r = (16·c0 + r·(c3 − c0)) >> 7` with `x` the column inside
/// the 32-wide block; an isometric block's diamond rows only cover their
/// pixels of that block (its transparent image corners draw nothing).
pub fn floor_block_chain(tables: &ShadeTables, light: BlockLight, x: i32, y: i32) -> ShadeChain {
    tables.block_chain(light, GradientKind::RleFloor, x, y)
}

/// The item palette files of §6 r4 by `t` (1…8), loaded as 21 maps each
/// from `Data\Global\Items\Palette\<name>.dat`.
pub const ITEM_PALETTE_FILES: [&str; 8] = [
    "grey",
    "grey2",
    "gold",
    "brown",
    "greybrown",
    "invgrey",
    "invgrey2",
    "invgreybrown",
];
/// Maps per item palette file.
pub const ITEM_PALETTE_MAPS: u8 = 21;

/// Which item palette map an item color `(t, c)` selects (§6 r4,
/// `0x00600C20`): `(t, c)` for a map of file `ITEM_PALETTE_FILES[t − 1]`,
/// or `None` (no map) for `t` = 0, 3, 4 or ≥ 9, or `c` ≥ 21. Files 3 and 4
/// are loaded but never selected (Edge case 2).
pub fn item_color(t: u8, c: u8) -> Option<(u8, u8)> {
    match t {
        1 | 2 | 5..=8 if c < ITEM_PALETTE_MAPS => Some((t, c)),
        _ => None,
    }
}

// ---------------------------------------------------------------------
// Monster palette shift and blood map (§6 r2, r3, r6, r7).
// ---------------------------------------------------------------------

/// Bytes of one remap map.
pub const MAP_BYTES: usize = 256;
/// Maps of `RandTransforms.dat` (§6 r2).
pub const RAND_TRANSFORMS_MAPS: u32 = 30;
/// The shift index every gfx starts with (`0x0046EBB0`, §6 r6).
pub const SHIFT_DEFAULT: u32 = 2;
/// Monster classes whose shift 0 / 1 picks a `palshift` map (jump table
/// `0x00477608`, §6 r6): 363 `necroskeleton`, 364 `necromage`.
pub const SHIFT_LOW_CLASSES: [u32; 2] = [363, 364];
/// Offset of `palshift` map 0 from the class's base (§6 r6:
/// `base + 4 + 256·s`).
pub const PALSHIFT_FIRST: usize = 4;
/// Offset of the second 8-map set from the class's base (§6 r6:
/// `base + 0x804 + 256·s`).
pub const PALSHIFT_SECOND: usize = 0x804;
/// Fatal error code of a RandTransforms index past 29 (§6 r6).
pub const FATAL_RAND_TRANSFORMS: u32 = 0x160;

/// Errors of the monster palette shift and blood map rules.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShiftError {
    /// `s − 8` > 29 with RandTransforms loaded: the original's fatal
    /// error 0x160 (§6 r6; r6.5 keeps `s` ≤ 29 for a 0xAC monster).
    #[error("RandTransforms map {0} > 29: fatal 0x160 (render/shading.md §6 r6)")]
    RandTransformsFatal(u32),
    /// A map read past the end of the caller's data (the original reads
    /// past the loaded maps, §6 r6 `s` ≥ 8 without RandTransforms).
    #[error("{what} map at byte {offset} is past the {len} bytes given")]
    PastData {
        what: &'static str,
        offset: usize,
        len: usize,
    },
}

/// What the shift index of a monster created by S→C 0xAC depends on (§6
/// r6, `0x00466360`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShiftInputs {
    /// Monster class (`monstats` row).
    pub class: u32,
    /// The 16-bit name seed of the message.
    pub name_seed: u16,
    /// `monstats` `TransLvl` (`+0x4D`).
    pub trans_lvl: u8,
    /// Monster type flag 0x8 (`0x004AE360`).
    pub unique: bool,
    /// `monstats2` `noUniqueShift` (flag bit 15, `0x004638A0(class, 15)`).
    pub no_unique_shift: bool,
    /// `monstats2` `Utrans` for the current difficulty (`+0x122 +
    /// [0x007A060C]`).
    pub utrans: u8,
    /// `0x004791B0(unit)` ≠ 0, the owner relation test of `Utrans` 255.
    /// Its inputs are Open question 7 of `render/shading.md`: the caller
    /// supplies the result.
    pub owner_relation: bool,
    /// Superunique (type flag 0x2): the `superuniques` row's `Utrans` for
    /// the difficulty (`+0x28 + difficulty`); `None` when not superunique.
    pub superunique_utrans: Option<u8>,
}

/// §6 r6 step 1: `TransLvl + 2` when `TransLvl` < 8, else 2.
pub fn shift_trans_lvl(trans_lvl: u8) -> u32 {
    if trans_lvl < 8 {
        u32::from(trans_lvl) + 2
    } else {
        SHIFT_DEFAULT
    }
}

/// §6 r6 step 2 (`0x00477620`): `9 + (n mod 30)`, `n` the new low word of
/// one D2 RNG step of the seed {low = class + name seed, high = 666}.
pub fn shift_unique(class: u32, name_seed: u16) -> u32 {
    let mut seed = d2_sim::rng::Seed::init_low(class.wrapping_add(u32::from(name_seed)));
    9 + seed.step() % 30
}

/// §6 r6 step 3: `Utrans` ≠ 0 replaces `s`; 255 gives 1 when the owner
/// relation `0x004791B0` is 0, else 0. `None` = no change.
pub fn shift_utrans(utrans: u8, owner_relation: bool) -> Option<u32> {
    match utrans {
        0 => None,
        255 => Some(if owner_relation { 0 } else { 1 }),
        u => Some(u32::from(u)),
    }
}

/// The shift index `s` of a monster created by S→C 0xAC (§6 r6 steps 1–5,
/// each overriding the previous one).
pub fn monster_shift_index(m: &ShiftInputs) -> u32 {
    let mut s = shift_trans_lvl(m.trans_lvl);
    if m.unique && !m.no_unique_shift {
        s = shift_unique(m.class, m.name_seed);
    }
    if let Some(u) = shift_utrans(m.utrans, m.owner_relation) {
        s = u;
    }
    if let Some(u) = m.superunique_utrans.filter(|&u| u != 0) {
        s = u32::from(u);
    }
    if s >= 30 {
        s = SHIFT_DEFAULT;
    }
    s
}

/// The unit and class state the map choice `0x00477530` reads (§6 r6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShiftMapInputs {
    /// Unit type (1 = monster).
    pub unit_type: u32,
    /// Monster class.
    pub class: u32,
    /// The class has `palshift` data (`[0x007B9578]` entry).
    pub has_palshift: bool,
    /// The green-blood switch (r7, [`green_blood_switch`]).
    pub green_blood: bool,
    /// `monstats2` `localBlood` (`+0x11C`).
    pub local_blood: u32,
    /// RandTransforms loaded (`[0x007BB380]` = 1, `0x00476EA0`).
    pub rand_transforms_loaded: bool,
}

/// The `P` a monster's shift index selects (§6 r6, `0x00477530`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShiftMap {
    /// No `P`.
    None,
    /// A `palshift` map: `offset` = bytes from the class's base
    /// (`base + 4 + 256·s` or `base + 0x804 + 256·s`).
    Palshift { offset: usize },
    /// RandTransforms map `m` (0…29).
    RandTransforms(u32),
}

/// The map choice of `0x00477530` for shift index `s` (§6 r6).
pub fn shift_map(s: u32, m: &ShiftMapInputs) -> Result<ShiftMap, ShiftError> {
    if m.unit_type != 1 || !m.has_palshift {
        return Ok(ShiftMap::None);
    }
    let palshift = |set: usize| ShiftMap::Palshift {
        offset: set + MAP_BYTES * s as usize,
    };
    match s {
        0 | 1 if SHIFT_LOW_CLASSES.contains(&m.class) => Ok(palshift(PALSHIFT_FIRST)),
        0 | 1 => Ok(ShiftMap::None),
        s if s >= 8 && m.rand_transforms_loaded => {
            let k = s - 8;
            if k >= RAND_TRANSFORMS_MAPS {
                Err(ShiftError::RandTransformsFatal(k))
            } else {
                Ok(ShiftMap::RandTransforms(k))
            }
        }
        // 2…7, and ≥ 8 without RandTransforms (reads past the 8 maps).
        _ if m.green_blood && m.local_blood == 2 => Ok(palshift(PALSHIFT_SECOND)),
        _ => Ok(palshift(PALSHIFT_FIRST)),
    }
}

fn map_at<'a>(
    data: &'a [u8],
    offset: usize,
    what: &'static str,
) -> Result<&'a [u8; MAP_BYTES], ShiftError> {
    data.get(offset..offset + MAP_BYTES)
        .map(|m| m.try_into().expect("256 bytes"))
        .ok_or(ShiftError::PastData {
            what,
            offset,
            len: data.len(),
        })
}

/// The 256 bytes of a [`ShiftMap::Palshift`] offset in the class's
/// `palshift` block `base` (the caller's bytes from the class's base, as
/// `0x00477530` addresses them; the spec does not say what `base + 0…3`
/// holds, so the caller supplies the block, not the bare file).
pub fn palshift_map(base: &[u8], offset: usize) -> Result<&[u8; MAP_BYTES], ShiftError> {
    map_at(base, offset, "palshift")
}

/// RandTransforms map `m` from the bytes of
/// `Data\Global\Monsters\RandTransforms.dat` (30 maps of 256 bytes, §6 r2).
pub fn rand_transforms_map(file: &[u8], m: u32) -> Result<&[u8; MAP_BYTES], ShiftError> {
    if m >= RAND_TRANSFORMS_MAPS {
        return Err(ShiftError::RandTransformsFatal(m));
    }
    map_at(file, MAP_BYTES * m as usize, "RandTransforms")
}

/// The blood map (§6 r3, `0x00477680`): the `GreenBlood.dat` map (its
/// first 256 bytes) for an S8 component or a missile with `LocalBlood`
/// (`uses_blood`) when the green-blood switch (r7) is on; else none.
pub fn blood_map(
    green_blood_file: &[u8],
    green_blood: bool,
    uses_blood: bool,
) -> Result<Option<&[u8; MAP_BYTES]>, ShiftError> {
    if !(green_blood && uses_blood) {
        return Ok(None);
    }
    map_at(green_blood_file, 0, "GreenBlood").map(Some)
}

/// The green-blood switch `[0x007A05FC]` (§6 r7, `0x0044DBE0`): on when
/// `Data\Local\Color.txt` opens (`color_txt` = its bytes) and its first
/// byte is `1` (0x31).
pub fn green_blood_switch(color_txt: Option<&[u8]>) -> bool {
    color_txt.and_then(|b| b.first()) == Some(&b'1')
}
