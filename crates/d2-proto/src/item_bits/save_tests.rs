// Spec: specs/items/bitstream.md (§5 save format)
// Spec: specs/formats/d2s.md (§8.1 rule 2, §8.2 rule 4)
use super::tests::Fixture;
use super::*;

/// LSB-first bit writer (§1 rule 1) for hand-built save streams.
#[derive(Default)]
struct W {
    buf: Vec<u8>,
    pos: usize,
}

impl W {
    fn put(&mut self, n: u32, v: u32) -> &mut Self {
        for i in 0..n {
            if self.pos.is_multiple_of(8) {
                self.buf.push(0);
            }
            if (v >> i) & 1 != 0 {
                *self.buf.last_mut().unwrap() |= 1 << (self.pos % 8);
            }
            self.pos += 1;
        }
        self
    }

    fn code(&mut self, c: &[u8; 4]) -> &mut Self {
        self.put(32, u32::from_le_bytes(*c))
    }

    /// JM, flags, version 101, mode 0, slot location (body 0, x, y, page1 1).
    fn head(&mut self, flags: u32, x: u32) -> &mut Self {
        self.put(16, SAVE_MARKER)
            .put(32, flags)
            .put(10, 101)
            .put(3, 0)
            .put(4, 0)
            .put(4, x)
            .put(4, 0)
            .put(3, 1)
    }

    /// Pads to a whole byte (zero bits) and returns the bytes.
    fn done(&mut self) -> Vec<u8> {
        self.pos = self.buf.len() * 8;
        self.buf.clone()
    }
}

const COMPACT: u32 = 0x800000 | hflag::COMPACT;
const FULL: u32 = 0x800000 | hflag::IDENTIFIED;

fn compact(trailer: Option<(u32, u32, u32)>) -> Vec<u8> {
    let mut w = W::default();
    w.head(COMPACT, 2).code(b"hp1 ");
    match trailer {
        None => w.put(1, 0),
        Some((a, b, z)) => w.put(1, 1).put(32, a).put(32, b).put(32, z),
    };
    w.done()
}

/// A full normal `cap ` with `filled` children announced and unit +0x28 =
/// 0xDEADBEEF; socketed (194 = 2 sockets) when `filled` > 0.
fn full_cap(flags: u32, filled: u32) -> W {
    let flags = if filled > 0 {
        flags | hflag::SOCKETED
    } else {
        flags
    };
    let mut w = W::default();
    w.head(flags, 0)
        .code(b"cap ")
        .put(3, filled)
        .put(32, 0xDEAD_BEEF)
        .put(7, 30) // ilvl
        .put(4, 2) // quality normal
        .put(1, 0) // no gfx
        .put(1, 0) // no auto affix
        .put(1, 0) // trailer bit 0 (after §4.4)
        .put(11, 13) // defense raw (3 + Save Add 10)
        .put(8, 12) // max durability
        .put(9, 12); // durability
    if filled > 0 {
        w.put(4, 2);
    }
    w.put(9, TERMINATOR);
    w
}

// Covers: specs/items/bitstream.md §2 r2, §3 r6, §5 r1, §5 r2
// Covers: specs/formats/d2s.md §8.1 r2
#[test]
fn compact_trailer_zero() {
    let b = compact(None);
    let e = save_entry_len(&b, &Fixture).unwrap();
    // 16 + 32 + 10 + 3 + 15 + 32 + 1 = 109 bits → 14 bytes.
    assert_eq!(e.item.bits, 109);
    assert_eq!(e.len, 14);
    assert_eq!(e.len, b.len());
    assert_eq!(e.item.code, *b"hp1 ");
    assert_eq!(e.item.save_trailer, None);
    assert_eq!(e.item.save_unit28, None);
    assert!(e.item.save && e.item.shown());
    assert!(e.children.is_empty());
}

// Covers: specs/items/bitstream.md §3 r6, §5 r2
#[test]
fn compact_trailer_one() {
    let b = compact(Some((0x1234_5678, 0x9ABC_DEF0, 0)));
    let e = save_entry_len(&b, &Fixture).unwrap();
    assert_eq!(e.item.bits, 109 + 96);
    assert_eq!(e.len, (109 + 96usize).div_ceil(8));
    assert_eq!(e.item.save_trailer, Some((0x1234_5678, 0x9ABC_DEF0)));
}

// The third word is read and dropped whatever it holds (§5 rule 2;
// `bitstream-legacy.md` §2 rule 2 at the current version).
// Covers: specs/items/bitstream.md §5 r2; specs/items/bitstream-legacy.md §2 row7
#[test]
fn trailer_tail_is_read_and_dropped() {
    let b = compact(Some((1, 2, 3)));
    let e = save_entry_len(&b, &Fixture).unwrap();
    assert_eq!(e.item.save_trailer, Some((1, 2)));
    assert_eq!(e.item.bits, 109 + 96);
    assert!(!e.item.failed);
}

// Covers: specs/items/bitstream.md §4.1 r6, §4.1 r7, §4.4 r3, §5 r1
#[test]
fn full_record_unit28() {
    // Not identified: still "shown" in the save format (§4.3), so the
    // main list's terminator follows (§4.5 rule 6 does not end it).
    let b = full_cap(0x800000, 0).done();
    let e = save_entry_len(&b, &Fixture).unwrap();
    let it = &e.item;
    assert_eq!(it.save_unit28, Some(0xDEAD_BEEF));
    assert_eq!(it.filled, 0);
    assert_eq!(it.ilvl, 30);
    assert_eq!(it.quality, 2);
    assert_eq!(it.save_trailer, None);
    assert_eq!(it.defense.map(|s| s.value()), Some(3));
    assert_eq!(it.durability.map(|s| s.value()), Some(12));
    assert_eq!(it.lists, vec![Some(vec![])]);
    assert!(it.shown());
    assert_eq!(e.len, b.len());
}

// Covers: specs/items/bitstream.md §2 r2
#[test]
fn bad_marker_is_error() {
    let mut b = compact(None);
    b[0] = 0x4D; // "MM"
    assert_eq!(
        save_entry_len(&b, &Fixture).unwrap_err(),
        ItemBitsError::BadMarker(0x4D4D)
    );
    let mut r = BitReader::new(&b);
    assert_eq!(
        decode_save_record(&mut r, &Fixture).unwrap_err(),
        ItemBitsError::BadMarker(0x4D4D)
    );
}

// Covers: specs/items/bitstream.md §2 r5, §4.1 r6
// Covers: specs/formats/d2s.md §8.1 r2, §8.2 r4
#[test]
fn filled_one_reads_child() {
    let parent = full_cap(FULL, 1).done();
    let child = compact(Some((7, 8, 0)));
    let mut b = parent.clone();
    b.extend_from_slice(&child);
    b.extend_from_slice(&[0xAA, 0x55]); // the next entry's bytes: not read
    let e = save_entry_len(&b, &Fixture).unwrap();
    assert_eq!(e.item.filled, 1);
    assert_eq!(e.item.sockets, Some(2));
    assert_eq!(e.children.len(), 1);
    assert_eq!(e.children[0].len, child.len());
    assert_eq!(e.children[0].item.save_trailer, Some((7, 8)));
    assert_eq!(e.len, parent.len() + child.len());
}

// Covers: specs/formats/d2s.md §8.1 r2
#[test]
fn padding_bit_set_is_error() {
    let mut b = compact(None);
    // 109 bits used: bit 109 (byte 13, bit 5) is padding.
    b[13] |= 1 << 5;
    assert_eq!(
        save_entry_len(&b, &Fixture).unwrap_err(),
        ItemBitsError::Padding(109)
    );
}

/// [`Fixture`] with a synthetic row for stat 0 and 80 `setitems` rows.
struct Rows;

impl ItemLookup for Rows {
    fn code(&self, code: [u8; 4]) -> Option<CodeFacts> {
        Fixture.code(code)
    }
    fn isc(&self, stat: u16) -> Option<IscSave> {
        if stat == 0 {
            return Fixture.isc(19);
        }
        Fixture.isc(stat)
    }
    fn set_item_rows(&self) -> Option<usize> {
        Some(80)
    }
}

/// A full `cap ` of `quality` (no quality fields written unless given by
/// `fields`), then its main list's `stats` (id, 10-bit value) and the
/// terminator unless `open`.
fn cap_of(quality: u32, fields: &[(u32, u32)], stats: &[(u32, u32)], open: bool) -> Vec<u8> {
    let mut w = W::default();
    w.head(FULL, 0)
        .code(b"cap ")
        .put(3, 0)
        .put(32, 0xDEAD_BEEF)
        .put(7, 30)
        .put(4, quality)
        .put(1, 0)
        .put(1, 0);
    for &(n, v) in fields {
        w.put(n, v);
    }
    w.put(1, 0).put(11, 13).put(8, 12).put(9, 12);
    if quality == 5 {
        w.put(5, 0);
    }
    for &(s, v) in stats {
        w.put(9, s);
        if v != u32::MAX {
            w.put(10, v);
        }
    }
    if !open {
        w.put(9, TERMINATOR);
    }
    w.done()
}

// `bitstream-legacy.md` §3 rule 6.7, edge case 6: a quality outside 1–9
// reads nothing in the quality step, is read on to its end and fails.
// Covers: specs/items/bitstream-legacy.md §3 r6, §3 r12, §edge-cases-original-bugs r6
#[test]
fn a_quality_outside_1_to_9_fails_after_reading_on() {
    for q in [0, 10, 15] {
        let b = cap_of(q, &[], &[], false);
        let e = save_entry_len(&b, &Rows).unwrap();
        assert!(e.item.failed, "quality {q}");
        assert_eq!(e.len, b.len(), "quality {q}: read to the end");
        assert_eq!(e.item.defense.map(|s| s.value()), Some(3));
    }
    assert!(
        !save_entry_len(&cap_of(2, &[], &[], false), &Rows)
            .unwrap()
            .item
            .failed
    );
}

// Covers: specs/items/bitstream-legacy.md §3 r8
#[test]
fn a_set_index_without_a_setitems_row_fails() {
    let ok = save_entry_len(&cap_of(5, &[(12, 79)], &[], false), &Rows).unwrap();
    assert_eq!(
        (ok.item.failed, ok.item.quality_fields.file_index),
        (false, Some(79))
    );
    let bad = save_entry_len(&cap_of(5, &[(12, 80)], &[], false), &Rows).unwrap();
    assert!(bad.item.failed);
    // Without the row count the reader has no test.
    assert!(
        !save_entry_len(&cap_of(5, &[(12, 80)], &[], false), &Fixture)
            .unwrap()
            .item
            .failed
    );
}

// `bitstream-legacy.md` §4 rule 1, edge cases 1, 2: id 0 alone is a stat;
// 0 after 0 fails; an id with no row ends the list with no failure.
// Covers: specs/items/bitstream-legacy.md §4 r1, §edge-cases-original-bugs r1, §edge-cases-original-bugs r2
#[test]
fn list_ids_0_0_fail_and_an_id_without_a_row_ends_the_list() {
    let one = save_entry_len(&cap_of(2, &[], &[(0, 5), (19, 1)], false), &Rows).unwrap();
    assert!(!one.item.failed);
    assert_eq!(one.item.lists[0].as_ref().map(Vec::len), Some(2));
    let two = save_entry_len(&cap_of(2, &[], &[(0, 5), (0, 6)], true), &Rows).unwrap();
    assert!(two.item.failed);
    assert_eq!(two.item.lists[0].as_ref().map(Vec::len), Some(1));
    // Stat 5 has no row: the list ends at its id.
    let b = cap_of(2, &[], &[(5, u32::MAX)], true);
    let e = save_entry_len(&b, &Rows).unwrap();
    assert!(!e.item.failed);
    assert_eq!(e.item.lists[0], Some(Vec::new()));
}
