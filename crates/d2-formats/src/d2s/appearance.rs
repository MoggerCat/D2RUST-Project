// Spec: specs/formats/d2s-appearance.md
//! The 32 appearance bytes of a character save (header +0x88..+0xA7,
//! `formats/d2s.md` §2.8): the graphics token table (§1) and its lookup
//! (§2), and the fill from the equipped items (§3–§6: helm and hand
//! items, body armour parts, the colour byte).
//!
//! Pure functions over plain inputs: the item table columns
//! ([`ItemGfx`], one per weapons / armor / misc row in record order), the
//! `armtype` tokens, the colour columns of the affix, set, unique, gem
//! and state tables ([`ColourTables`]) and the itemtypes is-a matrix
//! ([`IsA`]) are projected from the data tables by the caller (d2-server
//! world data); the equipped items and the player's state inputs
//! ([`Equipment`]) are filled by the character writer from the game.
//!
//! Status: implemented from a draft spec; unverified (the token rule
//! reproduces the 8 measured component values in the spec, colour bytes
//! and body armour were never measured: spec Open question 2, IT-6).
//!
//! Randomness: none.

#[cfg(test)]
mod tests;

/// Body parts: byte i of the components and of the colours (Rules
/// preamble; the composit token order).
pub mod part {
    pub const HD: usize = 0;
    pub const TR: usize = 1;
    pub const LG: usize = 2;
    pub const RA: usize = 3;
    pub const LA: usize = 4;
    pub const RH: usize = 5;
    pub const LH: usize = 6;
    pub const SH: usize = 7;
    pub const S1: usize = 8;
    pub const S2: usize = 9;
    /// Number of parts (16 component and 16 colour bytes).
    pub const COUNT: usize = 16;
}

/// Item types the rules test (runtime itemtypes indices, §1 r2, §3, §6).
pub mod ty {
    pub const TORS: i32 = 3;
    pub const GEM: i32 = 20;
    pub const HELM: i32 = 37;
    pub const WEAP: i32 = 45;
    pub const ARMO: i32 = 50;
    pub const SHLD: i32 = 51;
    pub const CIRC: i32 = 75;
}

/// `wclass` indices (table `0x007446A0`, Constants).
pub mod wclass {
    pub const BOW: i32 = 1;
    pub const ONE_HS: i32 = 2;
    pub const ONE_HT: i32 = 3;
    pub const XBW: i32 = 7;
    pub const HT1: i32 = 12;
}

/// Item qualities (item data +0x00, §6 r2).
pub mod quality {
    pub const MAGIC: u8 = 4;
    pub const SET: u8 = 5;
    pub const RARE: u8 = 6;
    pub const UNIQUE: u8 = 7;
}

/// Body locations (item data +0x44, §3 r1).
pub mod body {
    pub const HEAD: u8 = 1;
    pub const TORSO: u8 = 3;
    pub const RIGHT_HAND: u8 = 4;
    pub const LEFT_HAND: u8 = 5;
}

/// Unit mode of an equipped item (unit +0x10, §3 r1).
pub const MODE_EQUIPPED: u8 = 1;
/// Item flag "socketed" (item data +0x18, §6 r2.4).
pub const FLAG_SOCKETED: u32 = 0x800;
/// Entries of the token table (§1 r1; entry 0 unused).
pub const TOKEN_ENTRIES: usize = 255;
/// Entries of the 1.00 reference table at `0x00744CA8` (Constants).
pub const REFERENCE_ENTRIES: usize = 256;
/// Body grid slots the hand-owner search reads (grid 0, slots 0–10, §4 r1).
pub const BODY_SLOTS: usize = 11;

/// The three `armtype` tokens the table starts with (§1 r1).
pub const ARMTYPE_CODES: [[u8; 4]; 3] = [*b"lit ", *b"med ", *b"hvy "];

/// The `wclass` index of a `wclass` code (`0x00629FE0`, table
/// `0x007446A0`): `bow` 1, `1hs` 2, `1ht` 3, `stf` 4, `2hs` 5, `2ht` 6,
/// `xbw` 7, `ht1` 12; any other code 0 (no table entry).
pub fn wclass_index(code: [u8; 4]) -> i32 {
    match &code {
        b"bow " => 1,
        b"1hs " => 2,
        b"1ht " => 3,
        b"stf " => 4,
        b"2hs " => 5,
        b"2ht " => 6,
        b"xbw " => 7,
        b"ht1 " => 12,
        _ => 0,
    }
}

fn code_u32(c: [u8; 4]) -> u32 {
    u32::from_le_bytes(c)
}

/// The itemtypes is-a matrix (`data/runtime-maps.md` §2, `0x00629B50`):
/// bit (i, j) = "type i is a type j". Out of range → false.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IsA {
    n: usize,
    bits: Vec<bool>,
}

impl IsA {
    /// From a predicate over the `n` × `n` type pairs.
    pub fn from_fn(n: usize, f: impl Fn(usize, usize) -> bool) -> Self {
        let mut bits = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                bits.push(f(i, j));
            }
        }
        Self { n, bits }
    }

    /// Whether type `ty` is a type `of`.
    pub fn is_a(&self, ty: i32, of: i32) -> bool {
        let (Ok(i), Ok(j)) = (usize::try_from(ty), usize::try_from(of)) else {
            return false;
        };
        i < self.n && j < self.n && self.bits[i * self.n + j]
    }
}

/// The items record columns the appearance reads (Inputs: `code` +0x80,
/// `alternategfx` +0x90, `component` +0x115, the six armour bytes
/// +0x116..+0x11B, `type` +0x11E, `hasinv` +0x137, `Transform` +0x141,
/// `gemoffset` +0xF0, `wclass` +0xC0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemGfx {
    pub code: [u8; 4],
    /// All zero when empty.
    pub alternategfx: [u8; 4],
    /// `type`, runtime itemtypes index.
    pub item_type: i32,
    pub component: u8,
    /// `Torso`, `Legs`, `rArm`, `lArm`, `rSPad`, `lSPad`: the `armtype`
    /// rows of body parts TR, LG, RA, LA, S1, S2 (§5 r1).
    pub arm: [u8; 6],
    pub hasinv: u8,
    pub transform: u8,
    pub gemoffset: i32,
    pub wclass: [u8; 4],
}

/// One slot of the 1.00 reference table (`0x00744CA8`): whether its type
/// is-a `weap` and is-a `armo` (the only test §1 r2.4 makes of it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RefSlot {
    pub weap: bool,
    pub armo: bool,
}

/// The 1.00 reference table (`0x00744CA8`, 256 entries).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceSlots(pub Vec<RefSlot>);

impl ReferenceSlots {
    /// From the table's types (the u32 at +4 of each 8-byte entry, read
    /// from the image) and the is-a matrix.
    pub fn from_types(types: &[i32], m: &IsA) -> Self {
        Self(
            types
                .iter()
                .map(|&t| RefSlot {
                    weap: m.is_a(t, ty::WEAP),
                    armo: m.is_a(t, ty::ARMO),
                })
                .collect(),
        )
    }

    /// A table without reserved slots (every slot neither `weap` nor
    /// `armo`).
    pub fn none() -> Self {
        Self(vec![RefSlot::default(); REFERENCE_ENTRIES])
    }

    /// The 1.14d reference table (`0x00744CA8`): [`REFERENCE_TYPES`]
    /// under the game's itemtypes is-a matrix `m` (§1 r2 step 4).
    pub fn game(m: &IsA) -> Self {
        Self::from_types(&reference_types(), m)
    }

    /// The 1.14d slots as §1 r3 describes their effect: the weapon
    /// tokens fill 4–56, the throwing potions skip to 125–134 while the
    /// helms start at 57, so slots 57–124 are `weap` slots; no other slot
    /// is reserved.
    // PROVISIONAL (formats/d2s-appearance.md §1 r2, §1 r3, Open
    // question 3; IT-6; REC-91): the
    // table's bytes are not in the spec; this reconstruction gives the
    // anchors of §1 r3 (`hax` 4 … `ktr` 45, `cap` 57, `buc` 79, potions
    // 125–134) but not the second `ktr` at 243 (edge case 4). Settled by
    // reading `0x00744CA8` from the 1.14d image, or by the IT-6 saves.
    pub fn provisional_1_14d() -> Self {
        let mut s = Self::none();
        for slot in &mut s.0[57..=124] {
            slot.weap = true;
        }
        s
    }

    fn get(&self, i: usize) -> RefSlot {
        self.0.get(i).copied().unwrap_or_default()
    }
}

/// The item type of each slot of the 1.00 reference table `0x00744CA8`
/// (256 × 8 bytes, the type at +4; `formats/d2s-appearance.md`
/// Constants, read from the 1.14d image at file offset 0x344CA8), as
/// (first slot, last slot, type) runs covering 0..=255.
pub const REFERENCE_TYPES: [(u8, u8, i32); 91] = [
    (0, 3, 1),
    (4, 10, 37),
    (11, 26, 3),
    (27, 30, 2),
    (31, 33, 16),
    (34, 36, 15),
    (37, 39, 19),
    (40, 40, 37),
    (41, 42, 2),
    (43, 45, 25),
    (46, 46, 29),
    (47, 47, 43),
    (48, 49, 30),
    (50, 50, 36),
    (51, 51, 28),
    (52, 53, 30),
    (54, 54, 36),
    (55, 56, 31),
    (57, 58, 30),
    (59, 62, 38),
    (63, 66, 32),
    (67, 69, 33),
    (70, 71, 30),
    (72, 75, 33),
    (76, 79, 27),
    (80, 83, 26),
    (84, 84, 28),
    (85, 85, 34),
    (86, 87, 28),
    (88, 88, 34),
    (89, 89, 31),
    (90, 91, 34),
    (92, 93, 35),
    (94, 94, 43),
    (95, 95, 29),
    (96, 96, 30),
    (97, 97, 36),
    (98, 98, 24),
    (99, 100, 38),
    (101, 101, 42),
    (102, 102, 32),
    (103, 105, 30),
    (106, 106, 33),
    (107, 110, 27),
    (111, 111, 26),
    (112, 112, 28),
    (113, 113, 34),
    (114, 114, 28),
    (115, 116, 35),
    (117, 117, 1),
    (118, 119, 3),
    (120, 120, 2),
    (121, 121, 3),
    (122, 123, 40),
    (124, 125, 19),
    (126, 127, 16),
    (128, 129, 15),
    (130, 130, 30),
    (131, 131, 28),
    (132, 132, 43),
    (133, 133, 29),
    (134, 134, 3),
    (135, 136, 28),
    (137, 138, 36),
    (139, 140, 30),
    (141, 142, 32),
    (143, 145, 33),
    (146, 147, 34),
    (148, 148, 32),
    (149, 149, 33),
    (150, 151, 24),
    (152, 153, 26),
    (154, 154, 36),
    (155, 164, 28),
    (165, 168, 25),
    (169, 169, 36),
    (170, 172, 24),
    (173, 179, 36),
    (180, 193, 30),
    (194, 197, 32),
    (198, 198, 42),
    (199, 199, 43),
    (200, 200, 42),
    (201, 201, 43),
    (202, 211, 33),
    (212, 217, 34),
    (218, 222, 26),
    (223, 230, 27),
    (231, 234, 35),
    (235, 241, 37),
    (242, 255, 3),
];

/// [`REFERENCE_TYPES`] by slot.
pub fn reference_types() -> [i32; 256] {
    let mut out = [0; 256];
    for &(a, b, t) in &REFERENCE_TYPES {
        for slot in &mut out[usize::from(a)..=usize::from(b)] {
            *slot = t;
        }
    }
    out
}

/// The token table (`0x0096CC68`, built by `0x0063D710`): 255 entries of
/// (code, item type), entry 0 unused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTable {
    entries: Vec<(u32, i32)>,
    n: usize,
}

impl TokenTable {
    /// §1 r1–r2: entries 1–3 `lit`, `med`, `hvy` (type 3), then every
    /// eligible items record in record order.
    pub fn build(rows: &[ItemGfx], m: &IsA, reference: &ReferenceSlots) -> Self {
        let mut entries = vec![(0u32, 0i32); TOKEN_ENTRIES];
        for (k, c) in ARMTYPE_CODES.iter().enumerate() {
            entries[k + 1] = (code_u32(*c), 3);
        }
        let mut n = 4usize;
        for r in rows {
            // r2.1.
            let g = if r.alternategfx != [0; 4] {
                code_u32(r.alternategfx)
            } else {
                code_u32(r.code)
            };
            // r2.2: only entries below n are compared.
            if entries[..n].iter().any(|&(c, _)| c == g) {
                continue;
            }
            // r2.3.
            let t = r.item_type;
            let eligible = (m.is_a(t, ty::WEAP)
                || m.is_a(t, ty::TORS)
                || m.is_a(t, ty::SHLD)
                || m.is_a(t, ty::HELM))
                && !m.is_a(t, ty::CIRC);
            if !eligible {
                continue;
            }
            // r2.4.
            let weap = m.is_a(t, ty::WEAP);
            let armo = m.is_a(t, ty::ARMO);
            let mut i = n;
            while i < TOKEN_ENTRIES {
                let s = reference.get(i);
                if (s.weap && weap) || (s.armo && armo) || entries[i].0 != 0 {
                    i += 1;
                } else {
                    break;
                }
            }
            if i >= TOKEN_ENTRIES {
                i = n;
            }
            if i >= TOKEN_ENTRIES {
                // The original writes past its 255 entries here; d2rs
                // stops placing (no table this size occurs in 1.14d).
                continue;
            }
            // r2.5.
            entries[i] = (g, t);
            if i == n {
                n += 1;
            }
        }
        Self { entries, n }
    }

    /// §2 r1 (`0x0063D900`): the first entry 1..254 whose code is `a` or
    /// `b`; none → 0. An entry never filled has code 0, so a zero code
    /// matches the first unfilled entry (the original's comparison).
    // PROVISIONAL (formats/d2s-appearance.md §2 r1, Open question 4;
    // IT-6; REC-92): an empty `a` is compared like any code.
    pub fn lookup(&self, a: [u8; 4], b: [u8; 4]) -> u8 {
        let (a, b) = (code_u32(a), code_u32(b));
        (1..TOKEN_ENTRIES)
            .find(|&i| self.entries[i].0 == a || self.entries[i].0 == b)
            .map_or(0, |i| i as u8)
    }

    /// Entry `i`: (code, item type).
    pub fn entry(&self, i: usize) -> Option<([u8; 4], i32)> {
        self.entries.get(i).map(|&(c, t)| (c.to_le_bytes(), t))
    }

    /// The count n after the build (§1 r1–r2).
    pub fn count(&self) -> usize {
        self.n
    }
}

/// One `states` row of the state-colour list (Constants: every row whose
/// `itemtype` > 0, in row order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateColour {
    /// The state's row id.
    pub state: u32,
    /// `itemtype` (+0x2A).
    pub item_type: i32,
    /// `itemtrans` (+0x2C, link8 read signed).
    pub itemtrans: i32,
}

/// The colour columns (Inputs), link8 values read signed (miss −1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColourTables {
    /// `transformcolor` of the combined magic affix array (suffixes,
    /// prefixes, automagic), by combined index (affix id − 1).
    pub affix: Vec<i32>,
    /// `setitems` `chrtransform`, by row.
    pub set: Vec<i32>,
    /// `uniqueitems` `chrtransform`, by row.
    pub unique: Vec<i32>,
    /// `gems` `transform`, by row.
    pub gem: Vec<i32>,
    /// The state-colour list.
    pub states: Vec<StateColour>,
}

/// Everything the fill reads from the data tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppearanceTables {
    /// Items records in record order (weapons, armor, misc).
    pub items: Vec<ItemGfx>,
    /// `armtype` `token` (+0x20) by row.
    pub armtype: Vec<[u8; 4]>,
    pub types: IsA,
    pub colours: ColourTables,
    pub tokens: TokenTable,
}

impl AppearanceTables {
    /// The tables with the token table built from `items` (§1).
    pub fn new(
        items: Vec<ItemGfx>,
        armtype: Vec<[u8; 4]>,
        types: IsA,
        colours: ColourTables,
        reference: &ReferenceSlots,
    ) -> Self {
        let tokens = TokenTable::build(&items, &types, reference);
        Self {
            items,
            armtype,
            types,
            colours,
            tokens,
        }
    }
}

/// One item of the player's inventory item list (§3 r1), with the item
/// data the rules read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquippedItem {
    /// Items record (the unit's class id, unit +4).
    pub record: usize,
    /// Unit mode (unit +0x10).
    pub mode: u8,
    /// Body location (item data +0x44).
    pub body_loc: u8,
    /// Quality (item data +0x00).
    pub quality: u8,
    /// Magic prefix ids (+0x38, +0x3A, +0x3C; combined index + 1, 0 none).
    pub prefix: [u16; 3],
    /// Magic suffix ids (+0x3E, +0x40, +0x42).
    pub suffix: [u16; 3],
    /// Automagic affix id (+0x36).
    pub auto_affix: u16,
    /// Set / unique row (+0x28).
    pub file_index: i32,
    /// Item flags (+0x18).
    pub flags: u32,
    /// The type's socket limit for the item level (`0x0062BC20`).
    pub max_sockets: i32,
    /// Items record of the first item of its own inventory.
    pub first_child: Option<usize>,
}

/// The appearance provider: what the character writer reads from the
/// game for the fill.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Equipment {
    /// The inventory item list, in link order.
    pub items: Vec<EquippedItem>,
    /// The body grid (grid 0, slots 0–10): indices into `items`.
    pub body_grid: [Option<usize>; BODY_SLOTS],
    /// The weapon in use (inventory +0x1C): index into `items`.
    pub weapon_in_use: Option<usize>,
    /// The player's weapon class index (`0x0064F380`, mode −1).
    pub weapon_class: i32,
    /// The states the player has (only the state-colour rows are asked).
    pub states: Vec<u32>,
}

/// w(T, k) (`0x0062A250`, §6): (T × 32 + (k & 0x1F)) mod 256 for 1 ≤ T ≤
/// 8 and 0 ≤ k ≤ 20, else 0.
pub fn colour_byte(t: u8, k: i32) -> u8 {
    if (1..=8).contains(&t) && (0..=20).contains(&k) {
        (u32::from(t) * 32 + (k as u32 & 0x1F)) as u8
    } else {
        0
    }
}

/// ok(T, k) (`0x00600C20`, §6): a palette exists for 1 ≤ T ≤ 8, T ≠ 3,
/// T ≠ 4 and k < 21 (signed).
pub fn colour_ok(t: u8, k: i32) -> bool {
    (1..=8).contains(&t) && t != 3 && t != 4 && k < 21
}

/// A link8 column value as the colour tables hold it (signed, miss −1).
pub fn link8_signed(v: u8) -> i32 {
    i32::from(v as i8)
}

struct Fill<'a> {
    eq: &'a Equipment,
    t: &'a AppearanceTables,
    comps: &'a mut [u8; part::COUNT],
    cols: &'a mut [u8; part::COUNT],
}

impl Fill<'_> {
    fn rec(&self, i: &EquippedItem) -> ItemGfx {
        self.t.items.get(i.record).copied().unwrap_or_default()
    }

    fn token_of(&self, r: &ItemGfx) -> u8 {
        self.t.tokens.lookup(r.alternategfx, r.code)
    }

    /// §4 r1: the hand owners R and L, by index into the item list.
    fn hand_owners(&self) -> (Option<usize>, Option<usize>) {
        let (mut r, mut l) = (None, None);
        for &slot in &self.eq.body_grid {
            let Some(ix) = slot else { continue };
            let Some(it) = self.eq.items.get(ix) else {
                continue;
            };
            let g = self.rec(it);
            let wc = wclass_index(g.wclass);
            let (is_r, is_l) = if matches!(wc, wclass::ONE_HS | wclass::ONE_HT | wclass::HT1) {
                let in_use = self.eq.weapon_in_use == Some(ix);
                (in_use, !in_use)
            } else {
                (g.component == 5, g.component == 6)
            };
            if is_r && r.is_none() {
                r = Some(ix);
            }
            if is_l && l.is_none() {
                l = Some(ix);
            }
        }
        (r, l)
    }

    /// §6 (`0x0062C100`): writes the byte (or not) and returns the result.
    fn colour(&self, it: &EquippedItem, byte: &mut u8) -> bool {
        let g = self.rec(it);
        let tr = g.transform;
        // r1: state colours.
        for s in &self.t.colours.states {
            if self.eq.states.contains(&s.state)
                && s.itemtrans < 21
                && self.t.types.is_a(g.item_type, s.item_type)
            {
                return colour_ok(tr, s.itemtrans);
            }
        }
        let affix_k = |id: u16| -> Option<i32> {
            let k = *self.t.colours.affix.get(usize::from(id).checked_sub(1)?)?;
            (k != -1).then_some(k)
        };
        let automagic = |byte: &mut u8| -> bool {
            match affix_k(it.auto_affix) {
                Some(k) => {
                    *byte = colour_byte(tr, k);
                    colour_ok(tr, k)
                }
                None => false,
            }
        };
        let row = |v: &Vec<i32>| -> Option<i32> {
            usize::try_from(it.file_index)
                .ok()
                .and_then(|r| v.get(r).copied())
        };
        match it.quality {
            quality::MAGIC | quality::RARE => {
                let first = it
                    .suffix
                    .iter()
                    .chain(it.prefix.iter())
                    .find_map(|&id| affix_k(id));
                match first {
                    Some(k) => {
                        *byte = colour_byte(tr, k);
                        colour_ok(tr, k)
                    }
                    None => automagic(byte),
                }
            }
            quality::SET | quality::UNIQUE => {
                let v = if it.quality == quality::SET {
                    &self.t.colours.set
                } else {
                    &self.t.colours.unique
                };
                match row(v) {
                    Some(k) if k >= 0 => {
                        *byte = colour_byte(tr, k);
                        colour_ok(tr, k)
                    }
                    _ => false,
                }
            }
            _ => {
                let gem = it.first_child.and_then(|c| {
                    let cg = self.t.items.get(c)?;
                    if !self.t.types.is_a(cg.item_type, ty::GEM) {
                        return None;
                    }
                    let r = usize::try_from(cg.gemoffset).ok()?;
                    self.t.colours.gem.get(r).copied()
                });
                match gem {
                    Some(k)
                        if g.hasinv != 0
                            && it.max_sockets != 0
                            && it.flags & FLAG_SOCKETED != 0 =>
                    {
                        *byte = colour_byte(tr, k);
                        colour_ok(tr, k)
                    }
                    _ => automagic(byte),
                }
            }
        }
    }

    /// §4 r5 (and §5 r1): colours[p] from §6.
    fn set_colour(&mut self, it: &EquippedItem, p: usize) {
        let mut b = self.cols[p];
        let ok = self.colour(it, &mut b);
        self.cols[p] = if ok { b.wrapping_add(1) } else { 0xFF };
    }

    /// §4 (`0x0063DA70`).
    fn hand_or_helm(&mut self, ix: usize, r_owner: Option<usize>, l_owner: Option<usize>) {
        let it = self.eq.items[ix];
        let g = self.rec(&it);
        // r2.
        let mut p = usize::from(g.component);
        let t = self.token_of(&g);
        if t == 0 {
            if p < part::COUNT {
                self.comps[p] = 0xFF;
                self.cols[p] = 0xFF;
            }
            return;
        }
        // r3.
        if r_owner == Some(ix) {
            p = part::RH;
        } else if l_owner == Some(ix) {
            p = part::LH;
        } else if p >= part::COUNT {
            return;
        }
        self.comps[p] = t;
        // r4: only for the hands.
        if matches!(it.body_loc, body::RIGHT_HAND | body::LEFT_HAND) {
            let w = self.eq.weapon_class;
            if w == wclass::XBW && matches!(g.component, 5 | 6) {
                if let Some(r) = r_owner {
                    let rg = self.rec(&self.eq.items[r]);
                    match self.token_of(&rg) {
                        0 => {
                            self.comps[part::LH] = 0xFF;
                            self.cols[part::LH] = 0xFF;
                        }
                        rt => self.comps[part::LH] = rt,
                    }
                }
            }
            // `bow` with component 6 looks up the items record `lit `
            // (`0x00633640`); 1.14d has none, and the spec states no
            // effect for a table that has one: nothing happens.
        }
        // r5.
        self.set_colour(&it, p);
    }

    /// §5: body armour.
    fn body_armour(&mut self, ix: usize) {
        const PARTS: [usize; 6] = [part::TR, part::LG, part::RA, part::LA, part::S1, part::S2];
        let it = self.eq.items[ix];
        let g = self.rec(&it);
        for (k, &c) in PARTS.iter().enumerate() {
            let v = usize::from(g.arm[k]);
            // A row past the `armtype` table: the original reads past
            // it; d2rs treats it as no token.
            let t = self
                .t
                .armtype
                .get(v)
                .map_or(0, |&tok| self.t.tokens.lookup(tok, tok));
            if it.record == 0 || t == 0 {
                self.comps[c] = 0xFF;
                self.cols[c] = 0xFF;
            } else {
                self.comps[c] = t;
                self.set_colour(&it, c);
            }
        }
    }
}

/// §3 (`0x0063E510`): writes the bytes of every equipped helm, body
/// armour and hand item into `components` / `colours` (the caller has
/// filled them with 0xFF, `formats/d2s.md` §2.8 r1).
pub fn fill(
    eq: &Equipment,
    t: &AppearanceTables,
    components: &mut [u8; part::COUNT],
    colours: &mut [u8; part::COUNT],
) {
    let mut f = Fill {
        eq,
        t,
        comps: components,
        cols: colours,
    };
    let (r, l) = f.hand_owners();
    for (ix, it) in eq.items.iter().enumerate() {
        if it.mode != MODE_EQUIPPED {
            continue;
        }
        match it.body_loc {
            body::HEAD | body::RIGHT_HAND | body::LEFT_HAND => {
                if t.types.is_a(f.rec(it).item_type, ty::CIRC) {
                    continue;
                }
                f.hand_or_helm(ix, r, l);
            }
            body::TORSO => f.body_armour(ix),
            _ => {}
        }
    }
}
