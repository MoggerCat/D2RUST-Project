// Spec: specs/items/bitstream.md
//! Reader of the item bit stream carried by S→C 0x9C (from byte 8) and
//! 0x9D (from byte 13): the client's and the conformance harness's side of
//! `d2_sim::items::bitstream` (the server's writer).
//!
//! The stream does not say which type tests an item code passes or how
//! wide a stat is: the reader asks the tables through [`ItemLookup`]
//! (items / itemtypes facts by code, itemstatcost save columns by stat).
//! Values are returned as written, `Save Add` removed ([`Stat::value`]);
//! `ValShift`'s low bits are not on the wire.

/// Header flag bits the reader branches on (§2).
pub mod hflag {
    pub const IDENTIFIED: u32 = 0x10;
    pub const SOCKETED: u32 = 0x800;
    pub const EAR: u32 = 0x10000;
    pub const COMPACT: u32 = 0x200000;
    pub const PERSONALIZED: u32 = 0x1000000;
    pub const ALT_CODE: u32 = 0x2000000;
    pub const RUNEWORD: u32 = 0x4000000;
}

/// List terminator (§4.6 rule 5).
pub const TERMINATOR: u32 = 0x1FF;

/// The itemstatcost save columns of one stat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IscSave {
    pub save_bits: u8,
    pub save_add: u32,
    pub save_param_bits: u32,
}

/// What the tables say about an item code (`generation.md` §1.3 type tests
/// with equivalence; items and itemtypes columns).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeFacts {
    /// Type 50 `armo`.
    pub armor: bool,
    /// Type 45 `weap`.
    pub weapon: bool,
    /// Type 4 `gold`.
    pub gold: bool,
    /// Type 13 `char`.
    pub charm: bool,
    /// Type 40 `body` and not type 7 `play`.
    pub body_part: bool,
    /// Type 22 `scro` or 18 `book`.
    pub scroll_or_book: bool,
    /// Items `stackable` ≠ 0.
    pub stackable: bool,
    /// The primary type's `varinvgfx` ≠ 0.
    pub varinvgfx: bool,
    /// Items `quest` ≠ 0 and `questdiffcheck` ≠ 0.
    pub quest_diff: bool,
}

/// The tables the reader needs.
pub trait ItemLookup {
    /// Facts of an item code; `None`: unknown code (a decode error).
    fn code(&self, code: [u8; 4]) -> Option<CodeFacts>;
    /// Save columns of a stat; `None`: no row (a decode error).
    fn isc(&self, stat: u16) -> Option<IscSave>;
}

/// Why a stream did not decode.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ItemBitsError {
    #[error("stream ends at bit {0}")]
    Short(usize),
    #[error("unknown item code {0:?}")]
    UnknownCode([u8; 4]),
    #[error("stat {0} has no itemstatcost row")]
    UnknownStat(u16),
    #[error("stat {0} has no save bits but is on the wire")]
    Unsaved(u16),
    #[error("{0} bits left after the record (more than the padding)")]
    Trailing(usize),
    #[error("padding bit {0} is set")]
    Padding(usize),
}

/// LSB-first bit reader (§1 rule 1).
#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Bits 0..n−1 of the next n bits (n ≤ 32).
    pub fn read(&mut self, n: u32) -> Result<u32, ItemBitsError> {
        let n = n as usize;
        if self.pos + n > self.buf.len() * 8 {
            return Err(ItemBitsError::Short(self.pos));
        }
        let mut v = 0u32;
        for i in 0..n {
            let at = self.pos + i;
            v |= u32::from((self.buf[at / 8] >> (at % 8)) & 1) << i;
        }
        self.pos += n;
        Ok(v)
    }

    pub fn pos(&self) -> usize {
        self.pos
    }
}

/// Where the item is (§4.1 rules 2–3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// Modes 3 and 5: sub-tile position.
    Ground { x: u16, y: u16 },
    /// Other modes: body location, grid / slot x and y, page + 1.
    Slot { body: u8, x: u8, y: u8, page1: u8 },
}

/// Ear fields (§3 rule 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ear {
    pub class: u8,
    pub level: u8,
    pub name: Vec<u8>,
}

/// A stat on the wire: id, param, the written value and its `Save Add`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stat {
    pub stat: u16,
    pub param: u32,
    /// As written (clamped, `Save Add` included).
    pub raw: u32,
    pub save_add: u32,
}

impl Stat {
    /// The value without `Save Add` (the list value >> `ValShift`).
    pub fn value(&self) -> i64 {
        i64::from(self.raw) - i64::from(self.save_add)
    }
}

/// The quality-dependent fields (§4.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QualityFields {
    /// Qualities 1, 3 (3 bits), 5, 7 (12 bits); type `body` (10 bits).
    pub file_index: Option<u32>,
    /// Magic: prefix and suffix slot 0 as sent.
    pub magic: Option<(u16, u16)>,
    /// Rare, crafted, tempered names.
    pub rare_names: Option<(u8, u8)>,
    /// Rare / crafted slots: (prefix i, suffix i) as sent, 0 = none.
    pub rare_slots: Option<[(u16, u16); 3]>,
    /// Charm affix (1 bit: is a prefix; 11 bits id).
    pub charm: Option<(bool, u16)>,
    /// Scroll / book: suffix slot 0 (5 bits).
    pub spell: Option<u8>,
}

/// One decoded stream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemBits {
    /// Header flags F (§2).
    pub flags: u32,
    pub version: u16,
    pub mode: u8,
    pub location: Option<Location>,
    /// Alt-code record: the base code, and nothing after it.
    pub base_code: Option<[u8; 4]>,
    pub code: [u8; 4],
    pub ear: Option<Ear>,
    /// Personalized name (characters before the 0).
    pub name: Option<Vec<u8>>,
    /// Gold (compact gold, or type `gold`).
    pub gold: Option<u32>,
    /// `questitemdifficulty` (compact).
    pub quest_diff: Option<Stat>,
    // ---- full record
    pub filled: u8,
    pub ilvl: u8,
    pub quality: u8,
    pub gfx: Option<u8>,
    pub auto_affix: Option<u16>,
    pub quality_fields: QualityFields,
    pub runeword: Option<u16>,
    pub defense: Option<Stat>,
    pub max_durability: Option<Stat>,
    pub durability: Option<Stat>,
    pub quantity: Option<u16>,
    pub sockets: Option<u8>,
    /// Set mask (quality 5, shown).
    pub set_mask: Option<u8>,
    /// Lists c = −1, 0, …, L − 1 in order (§4.6 rule 3): `None` for a slot
    /// without its terminator (no list, no runeword).
    pub lists: Vec<Option<Vec<Stat>>>,
    /// Bits used (the stream is these, padded to whole bytes).
    pub bits: usize,
}

impl ItemBits {
    /// Identified ("shown", §4.3 network case).
    pub fn shown(&self) -> bool {
        self.flags & hflag::IDENTIFIED != 0
    }
}

fn read_isc(
    r: &mut BitReader<'_>,
    t: &dyn ItemLookup,
    s: u16,
    param: u32,
) -> Result<Stat, ItemBitsError> {
    let c = t.isc(s).ok_or(ItemBitsError::UnknownStat(s))?;
    Ok(Stat {
        stat: s,
        param,
        raw: r.read(u32::from(c.save_bits))?,
        save_add: c.save_add,
    })
}

fn read_name(r: &mut BitReader<'_>) -> Result<Vec<u8>, ItemBitsError> {
    let mut out = Vec::new();
    loop {
        let c = r.read(7)? as u8;
        if c == 0 {
            return Ok(out);
        }
        out.push(c);
        // TODO(spec: bitstream.md §3 rule 3): the writer's +0x4A is 16
        // bytes; a longer name is read until its 0.
    }
}

fn read_gold(r: &mut BitReader<'_>) -> Result<u32, ItemBitsError> {
    if r.read(1)? == 1 {
        r.read(32)
    } else {
        r.read(12)
    }
}

/// The partners of a grouped stat (§4.6 rule 4.3).
pub fn partners(s: u16) -> &'static [u16] {
    match s {
        17 => &[18],
        48 => &[49],
        50 => &[51],
        52 => &[53],
        54 => &[55, 56],
        57 => &[58, 59],
        _ => &[],
    }
}

fn read_list(r: &mut BitReader<'_>, t: &dyn ItemLookup) -> Result<Vec<Stat>, ItemBitsError> {
    let mut out = Vec::new();
    loop {
        let s = r.read(9)?;
        if s == TERMINATOR {
            return Ok(out);
        }
        let s = s as u16;
        let c = t.isc(s).ok_or(ItemBitsError::UnknownStat(s))?;
        if c.save_bits == 0 {
            return Err(ItemBitsError::Unsaved(s));
        }
        match s {
            17 | 48 | 50 | 52 | 54 | 57 => {
                out.push(read_isc(r, t, s, 0)?);
                for &p in partners(s) {
                    out.push(read_isc(r, t, p, 0)?);
                }
            }
            326 => out.push(Stat {
                stat: s,
                param: 0,
                raw: 0,
                save_add: 0,
            }),
            _ => {
                let param = if c.save_param_bits > 0 {
                    r.read(c.save_param_bits)?
                } else {
                    0
                };
                out.push(read_isc(r, t, s, param)?);
            }
        }
    }
}

fn read_location(r: &mut BitReader<'_>, it: &mut ItemBits) -> Result<(), ItemBitsError> {
    it.mode = r.read(3)? as u8;
    it.location = Some(if it.mode == 3 || it.mode == 5 {
        Location::Ground {
            x: r.read(16)? as u16,
            y: r.read(16)? as u16,
        }
    } else {
        Location::Slot {
            body: r.read(4)? as u8,
            x: r.read(4)? as u8,
            y: r.read(4)? as u8,
            page1: r.read(3)? as u8,
        }
    });
    Ok(())
}

fn read_code(r: &mut BitReader<'_>) -> Result<[u8; 4], ItemBitsError> {
    Ok(r.read(32)?.to_le_bytes())
}

/// Decodes one stream. The stream must end in its last byte with zero
/// padding (§1 rule 1); anything else is an error.
pub fn decode(stream: &[u8], t: &dyn ItemLookup) -> Result<ItemBits, ItemBitsError> {
    let mut r = BitReader::new(stream);
    let it = decode_record(&mut r, t)?;
    let used = r.pos();
    let total = stream.len() * 8;
    if total - used >= 8 {
        return Err(ItemBitsError::Trailing(total - used));
    }
    for at in used..total {
        if (stream[at / 8] >> (at % 8)) & 1 != 0 {
            return Err(ItemBitsError::Padding(at));
        }
    }
    Ok(it)
}

/// Decodes one record from `r` (no padding check).
pub fn decode_record(r: &mut BitReader<'_>, t: &dyn ItemLookup) -> Result<ItemBits, ItemBitsError> {
    let mut it = ItemBits {
        flags: r.read(32)?,
        ..ItemBits::default()
    };
    let f = it.flags;
    it.version = r.read(10)? as u16;
    read_location(r, &mut it)?;
    if f & hflag::COMPACT != 0 {
        if f & hflag::EAR != 0 {
            it.ear = Some(read_ear(r)?);
        } else {
            it.code = read_code(r)?;
            let facts = t.code(it.code).ok_or(ItemBitsError::UnknownCode(it.code))?;
            if facts.gold {
                it.gold = Some(read_gold(r)?);
            }
        }
        // §3 rule 5 needs the code's facts; an ear record has no code.
        if f & hflag::EAR == 0 && t.code(it.code).is_some_and(|c| c.quest_diff) {
            it.quest_diff = Some(read_isc(r, t, 356, 0)?);
        }
        it.bits = r.pos();
        return Ok(it);
    }
    if f & hflag::ALT_CODE != 0 {
        it.base_code = Some(read_code(r)?);
        it.bits = r.pos();
        return Ok(it);
    }
    it.code = read_code(r)?;
    let facts = t.code(it.code).ok_or(ItemBitsError::UnknownCode(it.code))?;
    it.filled = r.read(3)? as u8;
    it.ilvl = r.read(7)? as u8;
    it.quality = r.read(4)? as u8;
    if r.read(1)? == 1 {
        it.gfx = Some(r.read(3)? as u8);
    }
    if r.read(1)? == 1 {
        it.auto_affix = Some(r.read(11)? as u16);
    }
    let shown = f & hflag::IDENTIFIED != 0;
    let q = &mut it.quality_fields;
    match it.quality {
        1 | 3 => q.file_index = Some(r.read(3)?),
        4 => {
            if shown {
                q.magic = Some((r.read(11)? as u16, r.read(11)? as u16));
            }
        }
        5 | 7 => {
            if shown {
                q.file_index = Some(r.read(12)?);
            }
        }
        6 | 8 => {
            if shown {
                q.rare_names = Some((r.read(8)? as u8, r.read(8)? as u8));
            }
            let mut slots = [(0u16, 0u16); 3];
            for s in slots.iter_mut() {
                let mut one = || -> Result<u16, ItemBitsError> {
                    Ok(if r.read(1)? == 1 {
                        r.read(11)? as u16
                    } else {
                        0
                    })
                };
                *s = (one()?, one()?);
            }
            q.rare_slots = Some(slots);
        }
        9 => {
            if shown {
                q.rare_names = Some((r.read(8)? as u8, r.read(8)? as u8));
            }
        }
        _ => {
            if facts.charm && shown {
                let is_prefix = r.read(1)? == 1;
                q.charm = Some((is_prefix, r.read(11)? as u16));
            }
            if facts.body_part {
                q.file_index = Some(r.read(10)?);
            }
            if facts.scroll_or_book {
                q.spell = Some(r.read(5)? as u8);
            }
        }
    }
    // A quality outside 1–9 is rewritten to 2 by the writer and its
    // lists' stats skipped (§4.3 rule 7): the reader sees terminators.
    if f & hflag::RUNEWORD != 0 {
        it.runeword = Some(r.read(16)? as u16);
    }
    if f & hflag::EAR != 0 {
        it.ear = Some(read_ear(r)?);
    } else if f & hflag::PERSONALIZED != 0 {
        it.name = Some(read_name(r)?);
    }
    if facts.armor {
        it.defense = Some(read_isc(r, t, 31, 0)?);
        read_durability(r, t, &mut it)?;
    } else if facts.weapon {
        read_durability(r, t, &mut it)?;
    } else if facts.gold {
        it.gold = Some(read_gold(r)?);
    }
    if facts.stackable {
        it.quantity = Some(r.read(9)? as u16);
    }
    if f & hflag::SOCKETED != 0 {
        let c = t.isc(194).ok_or(ItemBitsError::UnknownStat(194))?;
        it.sockets = Some(r.read(u32::from(c.save_bits))? as u8);
    }
    if !shown {
        it.bits = r.pos();
        return Ok(it);
    }
    let mut l = 0usize;
    let mut mask = 0u32;
    if it.quality == 5 {
        mask = r.read(5)?;
        it.set_mask = Some(mask as u8);
        l = (32 - mask.leading_zeros()) as usize;
    }
    let runeword = f & hflag::RUNEWORD != 0;
    if runeword {
        l += 1;
    }
    it.lists.push(Some(read_list(r, t)?));
    for c in 0..l {
        let present = runeword || mask & (1 << c) != 0;
        it.lists.push(if present {
            Some(read_list(r, t)?)
        } else {
            None
        });
    }
    it.bits = r.pos();
    Ok(it)
}

fn read_ear(r: &mut BitReader<'_>) -> Result<Ear, ItemBitsError> {
    Ok(Ear {
        class: r.read(3)? as u8,
        level: r.read(7)? as u8,
        name: read_name(r)?,
    })
}

fn read_durability(
    r: &mut BitReader<'_>,
    t: &dyn ItemLookup,
    it: &mut ItemBits,
) -> Result<(), ItemBitsError> {
    let md = read_isc(r, t, 73, 0)?;
    it.max_durability = Some(md);
    if md.value() != 0 {
        it.durability = Some(read_isc(r, t, 72, 0)?);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
