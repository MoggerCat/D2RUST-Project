// Spec: specs/items/bitstream.md (save format, §2–§5, read as the inverse of the writer); specs/formats/d2s.md §8.1 rule 2, §8.2 rules 4, 7, 8; specs/world/vendors.md §7.3 step 3
//! The save-format record reader on the server side: the inverse of
//! [`super::write_save`] into the writer's own view ([`StreamItem`]), so a
//! record read back writes the same bytes. `world/vendors.md` §7.3 step
//! 3 reads the decoder `0x0062E430` (`0x0062CBE0` full, `0x0062A970`
//! compact) as "the inverse of `items/bitstream.md`" (its Open question
//! 8); this module is that reading. The client's reader of the network
//! stream is `d2_proto::item_bits`.
//!
//! What the stream does not carry, the reader takes from the tables (the
//! class from the code, the type tests); what the writer derives (the
//! runeword name id, the filled count) is returned as read.

use super::{
    hflag, Isc, IscTable, Kind, StatEntry, StreamItem, AUTO_OFFSET, PREFIX_OFFSET, SAVE_MARKER,
    TERMINATOR,
};
use crate::items::{stat, ty, ItemTables};

/// Why a record did not read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("record ends at bit {0}")]
    Short(usize),
    #[error("save marker is {0:#06x}, not 0x4D4A (\"JM\")")]
    BadMarker(u32),
    /// The class lookup of the header peek (`0x0062E410`): a code outside
    /// the items table (`vendors.md` §7.3 step 3: none).
    #[error("unknown item code {0:?}")]
    UnknownCode([u8; 4]),
    #[error("stat {0} has no save bits but is in a list")]
    Unsaved(u16),
    #[error("save trailer ends with {0:#x}, not 0")]
    TrailerTail(u32),
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

    /// The next `n` bits (n ≤ 32), bit 0 first.
    pub fn read(&mut self, n: u32) -> Result<u32, ReadError> {
        let n = n as usize;
        if self.pos + n > self.buf.len() * 8 {
            return Err(ReadError::Short(self.pos));
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

/// One record read back: the items class its code names and the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadItem {
    /// Combined items index of the code (`ear ` for an ear record).
    pub record: usize,
    pub item: StreamItem,
}

/// One save item entry (`d2s.md` §8.1 rule 2): the record, its byte
/// length (padded), and its children (`filled` of them, §8.2 rules 4, 8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadEntry {
    pub len: usize,
    pub item: ReadItem,
    pub children: Vec<ReadEntry>,
}

/// The combined items index of an item code (`generation.md` §10.1: the
/// first row with that code).
pub fn record_of(t: &ItemTables, code: [u8; 4]) -> Option<usize> {
    t.items.iter().position(|r| r.code == code)
}

/// The writer's type tests of a record (`generation.md` §1.3).
pub fn kind_of(t: &ItemTables, record: usize) -> Kind {
    let is = |k: u16| t.is_type(record, k as i16);
    Kind {
        armor: is(ty::ARMO),
        weapon: is(ty::WEAP),
        gold: is(ty::GOLD),
        charm: is(ty::CHAR),
        body_part: is(ty::BODY) && !is(ty::PLAY),
        scroll_or_book: is(ty::SCRO) || is(ty::BOOK),
    }
}

/// "ISC(s)" read back: the written value minus `Save Add` (§1 rule 4).
fn isc_get(r: &mut BitReader<'_>, c: Isc) -> Result<i32, ReadError> {
    Ok(r.read(u32::from(c.save_bits))?.wrapping_sub(c.save_add) as i32)
}

fn read_name(r: &mut BitReader<'_>) -> Result<[u8; 16], ReadError> {
    let mut name = [0u8; 16];
    for i in 0.. {
        let c = r.read(7)? as u8;
        if c == 0 {
            break;
        }
        // TODO(spec: bitstream.md §3 rule 3, edge case 7): a name of 16 or
        // more characters is read to its 0 and kept to 16 bytes.
        if let Some(b) = name.get_mut(i) {
            *b = c;
        }
    }
    Ok(name)
}

fn read_gold(r: &mut BitReader<'_>) -> Result<i32, ReadError> {
    Ok(if r.read(1)? == 1 {
        r.read(32)? as i32
    } else {
        r.read(12)? as i32
    })
}

fn read_trailer(r: &mut BitReader<'_>, it: &mut StreamItem) -> Result<(), ReadError> {
    if r.read(1)? == 1 {
        let a = r.read(32)?;
        let b = r.read(32)?;
        let z = r.read(32)?;
        if z != 0 {
            return Err(ReadError::TrailerTail(z));
        }
        it.save_trailer = Some((a, b));
    }
    Ok(())
}

/// The inverse of a prefix id on the wire (§4.2): an id ≠ 0 is p − P.
// TODO(spec: bitstream.md §4.2): the writer sends a prefix p ≤ P
// unchanged, so its inverse is not unique; a prefix id is always > P in
// the combined array, read as p′ + P.
fn prefix_from(p: u32) -> u16 {
    if p == 0 {
        0
    } else {
        p as u16 + PREFIX_OFFSET
    }
}

/// One list's stats up to its terminator (§4.6 rule 4 inverted): values
/// are shifted back by `ValShift` (the low bits are not on the wire);
/// grouped partners are written without the shift and read the same way.
fn read_list(r: &mut BitReader<'_>, t: &dyn IscTable) -> Result<Vec<StatEntry>, ReadError> {
    let mut out = Vec::new();
    loop {
        let s = r.read(9)?;
        if s == TERMINATOR {
            return Ok(out);
        }
        let s = s as u16;
        let c = t.isc(s);
        if c.save_bits == 0 {
            return Err(ReadError::Unsaved(s));
        }
        match s {
            17 | 48 | 50 | 52 | 54 | 57 => {
                let v = isc_get(r, c)?;
                out.push(StatEntry {
                    stat: s,
                    param: 0,
                    value: v << c.valshift,
                });
                for &p in super::partners(s) {
                    let v = isc_get(r, t.isc(p))?;
                    out.push(StatEntry {
                        stat: p,
                        param: 0,
                        value: v,
                    });
                }
            }
            // §4.6 rule 4.4: the id only (no value on the wire).
            326 => {}
            _ => {
                let param = if c.save_param_bits > 0 {
                    r.read(c.save_param_bits)? as u16
                } else {
                    0
                };
                let v = isc_get(r, c)?;
                out.push(StatEntry {
                    stat: s,
                    param,
                    value: v << c.valshift,
                });
            }
        }
    }
}

fn read_location(r: &mut BitReader<'_>, it: &mut StreamItem) -> Result<(), ReadError> {
    it.mode = r.read(3)?;
    if it.mode == 3 || it.mode == 5 {
        it.x = r.read(16)? as i32;
        it.y = r.read(16)? as i32;
    } else {
        it.body_loc = r.read(4)? as u8;
        it.x = r.read(4)? as i32;
        it.y = r.read(4)? as i32;
        // Page + 1 in 3 bits; page 0xFF is written as 0 (§4.1 rule 3).
        it.page = match r.read(3)? {
            0 => 0xFF,
            v => v as u8 - 1,
        };
    }
    Ok(())
}

/// Reads one save-format record (§2 rule 2 marker, §3 / §4, the trailer
/// of §5 rule 2): no padding, no children. The flags are kept as stored
/// except 0x80000 and the alt-code bit, which the decoder drops
/// (`d2s.md` §8.2 rule 7); `compact` and `alt` come from the header.
pub fn read_save_record(r: &mut BitReader<'_>, t: &ItemTables) -> Result<ReadItem, ReadError> {
    let isc: &dyn IscTable = &t.isc;
    let m = r.read(16)?;
    if m != SAVE_MARKER {
        return Err(ReadError::BadMarker(m));
    }
    let f = r.read(32)?;
    let mut it = StreamItem {
        flags: f & !(hflag::INIT | hflag::ALT_CODE),
        compact: f & hflag::COMPACT != 0,
        alt: f & hflag::ALT_CODE != 0,
        version: r.read(10)? as u16,
        ..StreamItem::default()
    };
    read_location(r, &mut it)?;
    let facts = |it: &mut StreamItem, record: usize| {
        it.kind = kind_of(t, record);
        if let Some(rec) = t.item(record) {
            it.stackable = rec.stackable != 0;
            it.quest_diff = rec.quest != 0 && rec.questdiffcheck != 0;
        }
    };
    if it.compact {
        let record = if f & hflag::EAR != 0 {
            it.ear_class = r.read(3)? as i32;
            it.ear_level = r.read(7)? as i32;
            it.name = read_name(r)?;
            it.code = *b"ear ";
            record_of(t, it.code).ok_or(ReadError::UnknownCode(it.code))?
        } else {
            it.code = r.read(32)?.to_le_bytes();
            let record = record_of(t, it.code).ok_or(ReadError::UnknownCode(it.code))?;
            facts(&mut it, record);
            if it.kind.gold {
                it.total_gold = read_gold(r)?;
            }
            record
        };
        facts(&mut it, record);
        if it.quest_diff {
            let c = isc.isc(stat::QUESTITEMDIFFICULTY);
            it.total_quest_diff = isc_get(r, c)? << c.valshift;
        }
        read_trailer(r, &mut it)?;
        return Ok(ReadItem { record, item: it });
    }
    let code = r.read(32)?.to_le_bytes();
    if it.alt {
        // §4.1 rule 4: the record ends after the base code.
        it.code = code;
        it.base_code = code;
        let record = record_of(t, code).ok_or(ReadError::UnknownCode(code))?;
        facts(&mut it, record);
        return Ok(ReadItem { record, item: it });
    }
    it.code = code;
    let record = record_of(t, code).ok_or(ReadError::UnknownCode(code))?;
    facts(&mut it, record);
    it.filled = r.read(3)?;
    it.unit28 = r.read(32)?;
    it.ilvl = r.read(7)? as i32;
    it.quality = r.read(4)? as u8;
    it.varinvgfx = r.read(1)? == 1;
    if it.varinvgfx {
        it.gfx = r.read(3)? as i32;
    }
    if r.read(1)? == 1 {
        // TODO(spec: bitstream.md §4.1 rule 11): as for prefixes, an auto
        // affix a ≤ A is sent unchanged; read as a′ + A.
        it.auto_affix = r.read(11)? as u16 + AUTO_OFFSET;
    }
    // §4.3, "shown" always in the save format.
    match it.quality {
        1 | 3 => it.file_index = r.read(3)? as i32,
        4 => {
            it.prefix[0] = prefix_from(r.read(11)?);
            it.suffix[0] = r.read(11)? as u16;
        }
        5 | 7 => {
            let v = r.read(12)?;
            // A negative file index is written as 0xFFF (§4.3 rule 4).
            it.file_index = if v == 0xFFF { -1 } else { v as i32 };
        }
        6 | 8 => {
            it.rare_prefix = r.read(8)? as u16;
            it.rare_suffix = r.read(8)? as u16;
            for i in 0..3 {
                if r.read(1)? == 1 {
                    it.prefix[i] = prefix_from(r.read(11)?);
                }
                if r.read(1)? == 1 {
                    it.suffix[i] = r.read(11)? as u16;
                }
            }
        }
        9 => {
            it.rare_prefix = r.read(8)? as u16;
            it.rare_suffix = r.read(8)? as u16;
        }
        _ => {
            if it.kind.charm {
                let is_prefix = r.read(1)? == 1;
                let v = r.read(11)?;
                if is_prefix {
                    it.prefix[0] = prefix_from(v);
                } else {
                    it.suffix[0] = v as u16;
                }
            }
            if it.kind.body_part {
                it.file_index = r.read(10)? as i32;
            }
            if it.kind.scroll_or_book {
                it.suffix[0] = r.read(5)? as u16;
            }
        }
    }
    it.runeword = if f & hflag::RUNEWORD != 0 {
        r.read(16)? as u16
    } else {
        0xFFFF
    };
    if f & hflag::EAR != 0 {
        it.ear_class = r.read(3)? as i32;
        it.ear_level = r.read(7)? as i32;
        it.name = read_name(r)?;
    } else if f & hflag::PERSONALIZED != 0 {
        it.name = read_name(r)?;
    }
    read_trailer(r, &mut it)?;
    let durability = |r: &mut BitReader<'_>, it: &mut StreamItem| -> Result<(), ReadError> {
        it.base_max_dur = isc_get(r, isc.isc(stat::MAXDURABILITY))?;
        if it.base_max_dur != 0 {
            it.total_dur = isc_get(r, isc.isc(stat::DURABILITY))?;
        }
        Ok(())
    };
    if it.kind.armor {
        it.base_defense = isc_get(r, isc.isc(stat::ARMORCLASS))?;
        durability(r, &mut it)?;
    } else if it.kind.weapon {
        durability(r, &mut it)?;
    } else if it.kind.gold {
        it.total_gold = read_gold(r)?;
    }
    if it.stackable {
        it.total_quantity = r.read(9)? as i32;
    }
    if f & hflag::SOCKETED != 0 {
        it.base_sockets = r.read(u32::from(isc.isc(stat::NUMSOCKETS).save_bits))? as i32;
    }
    // §4.6: the set mask, then the lists c = −1 … L − 1.
    let mut mask = 0u32;
    let mut l = 0usize;
    if it.quality == 5 {
        mask = r.read(5)?;
        l = (32 - mask.leading_zeros()) as usize;
    }
    let runeword = f & hflag::RUNEWORD != 0;
    if runeword {
        l += 1;
    }
    it.main = Some(read_list(r, isc)?);
    for c in 0..l {
        if runeword && c == l - 1 {
            it.runeword_list = Some(read_list(r, isc)?);
        } else if mask & (1 << c) != 0 || runeword {
            let list = read_list(r, isc)?;
            if mask & (1 << c) != 0 {
                it.sets[c] = Some(list);
            }
        }
    }
    Ok(ReadItem { record, item: it })
}

/// Reads one save item entry at `buf[0..]` (`d2s.md` §8.1 rule 2): the
/// record, zero padding to a whole byte, then its children (the record's
/// `filled` count, 0 for compact and alt-code records: §8.2 rule 8),
/// each an entry of its own.
pub fn read_save_entry(buf: &[u8], t: &ItemTables) -> Result<ReadEntry, ReadError> {
    let mut r = BitReader::new(buf);
    let item = read_save_record(&mut r, t)?;
    let used = r.pos();
    let len = used.div_ceil(8);
    for at in used..len * 8 {
        if (buf[at / 8] >> (at % 8)) & 1 != 0 {
            return Err(ReadError::Padding(at));
        }
    }
    let n = if item.item.compact || item.item.alt {
        0
    } else {
        item.item.filled
    };
    let mut entry = ReadEntry {
        len,
        item,
        children: Vec::new(),
    };
    for _ in 0..n {
        let rest = buf.get(entry.len..).unwrap_or_default();
        let child = read_save_entry(rest, t).map_err(|e| match e {
            ReadError::Short(b) => ReadError::Short(b + entry.len * 8),
            ReadError::Padding(b) => ReadError::Padding(b + entry.len * 8),
            e => e,
        })?;
        entry.len += child.len;
        entry.children.push(child);
    }
    Ok(entry)
}
