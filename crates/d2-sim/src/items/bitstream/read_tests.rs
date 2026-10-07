// Spec: specs/items/bitstream.md (the save-format reader as the inverse of the writer); specs/formats/d2s.md §8.1 rule 2, §8.2 rules 4, 7, 8
//! Round trips: every save record the writer produces reads back into a
//! view that writes the same bytes (`read::read_save_entry`), with the
//! fields the spec names restored; the decoder's flag rule (§8.2 rule 7:
//! 0x80000 and the alt-code bit dropped) and its errors.

use super::read::{read_save_entry, BitReader, ReadError};
use super::tests::isc_114d;
use super::*;
use crate::items::tests::{item_rec, push_item, tables, AXE, RING};
use crate::items::{ty, ItemTables};

struct Codes {
    t: ItemTables,
}

fn codes() -> Codes {
    let mut t = tables();
    t.isc = isc_114d();
    for (ty_, c) in [
        (ty::GOLD, b"gld "),
        (ty::HELM, b"cap "),
        (AXE, b"axe "),
        (ty::CHAR, b"cm1 "),
        (ty::BOOK, b"tbk "),
        (ty::SCRO, b"tsc "),
        (RING, b"rin "),
        (ty::BODY, b"ear "),
        (ty::BODY, b"hrt "),
        (ty::MISC, b"hp1 "),
    ] {
        push_item(&mut t, item_rec(ty_, c));
    }
    let hp1 = t.items.iter().position(|r| &r.code == b"hp1 ").unwrap();
    t.items[hp1].stackable = 0;
    let rin = t.items.iter().position(|r| &r.code == b"rin ").unwrap();
    t.items[rin].stackable = 1;
    Codes { t }
}

fn kind(c: &Codes, code: &[u8; 4]) -> Kind {
    read::kind_of(&c.t, read::record_of(&c.t, *code).unwrap())
}

/// Writes `item`, reads it back, writes the read view again.
fn round_trip(c: &Codes, item: &StreamItem) -> StreamItem {
    let (bytes, _) = write_save(item, &c.t.isc).unwrap();
    let e = read_save_entry(&bytes, &c.t).unwrap();
    assert_eq!(e.len, bytes.len(), "the entry is the whole stream");
    let (again, _) = write_save(&e.item.item, &c.t.isc).unwrap();
    assert_eq!(again, bytes, "re-written bytes differ for {:?}", item.code);
    e.item.item
}

fn full(c: &Codes, code: &[u8; 4]) -> StreamItem {
    StreamItem {
        flags: 0x10,
        version: 101,
        code: *code,
        kind: kind(c, code),
        ilvl: 12,
        quality: 2,
        unit28: 0xDEAD_BEEF,
        runeword: 0xFFFF,
        main: Some(Vec::new()),
        ..StreamItem::default()
    }
}

fn e(stat: u16, param: u16, value: i32) -> StatEntry {
    StatEntry { stat, param, value }
}

/// Compact records (§3): position, page + 1, gold, the trailer; the
/// ear's class, level and name replace the code (edge case 8).
// Covers: specs/items/bitstream.md §3 r1, §3 r2, §3 r3, §3 r4, §4.1 r3, §5 r2
#[test]
fn compact_records_read_back() {
    let c = codes();
    let hp1 = StreamItem {
        flags: 0x10,
        compact: true,
        version: 101,
        code: *b"hp1 ",
        x: 3,
        y: 1,
        page: 0xFF,
        save_trailer: Some((7, 9)),
        ..StreamItem::default()
    };
    let r = round_trip(&c, &hp1);
    assert_eq!(
        (r.x, r.y, r.page, r.save_trailer),
        (3, 1, 0xFF, Some((7, 9)))
    );
    let gold = StreamItem {
        code: *b"gld ",
        kind: kind(&c, b"gld "),
        total_gold: 5000,
        mode: 3,
        x: 600,
        y: 700,
        ..hp1.clone()
    };
    let r = round_trip(&c, &gold);
    assert_eq!((r.total_gold, r.mode, r.x, r.y), (5000, 3, 600, 700));
    let mut ear = StreamItem {
        flags: 0x10 | hflag::EAR,
        ear_class: 3,
        ear_level: 44,
        code: *b"ear ",
        ..hp1
    };
    ear.name[..4].copy_from_slice(b"Bomb");
    let r = round_trip(&c, &ear);
    assert_eq!(
        (r.ear_class, r.ear_level, &r.name[..5]),
        (3, 44, &b"Bomb\0"[..])
    );
}

/// Full records (§4): head, by-quality fields with the prefix and
/// automagic offsets undone, type values, sockets, the lists with
/// `ValShift` and the grouped partners, set lists by the mask, the
/// runeword list; all re-written byte for byte.
// Covers: specs/items/bitstream.md §4.1, §4.2, §4.3, §4.4 r2, §4.5, §4.6
#[test]
fn full_records_read_back() {
    let c = codes();
    // Armor: defense, durability, sockets, a list with ValShift 8.
    let mut cap = full(&c, b"cap ");
    cap.flags |= hflag::SOCKETED;
    cap.base_defense = 7;
    cap.base_max_dur = 12;
    cap.total_dur = 9;
    cap.base_sockets = 2;
    cap.filled = 0;
    cap.main = Some(vec![e(9, 0, 5 << 8), e(19, 0, 3), e(107, 36, 2)]);
    let r = round_trip(&c, &cap);
    assert_eq!(
        (r.base_defense, r.base_max_dur, r.total_dur, r.base_sockets),
        (7, 12, 9, 2)
    );
    assert_eq!(r.main, cap.main);
    assert_eq!(r.unit28, 0xDEAD_BEEF);
    // Magic weapon with an automagic affix and a grouped stat (17 → 18).
    let mut axe = full(&c, b"axe ");
    axe.quality = 4;
    axe.prefix[0] = PREFIX_OFFSET + 9;
    axe.suffix[0] = 33;
    axe.auto_affix = AUTO_OFFSET + 2;
    axe.varinvgfx = true;
    axe.gfx = 5;
    axe.main = Some(vec![e(17, 0, 40), e(18, 0, 40)]);
    let r = round_trip(&c, &axe);
    assert_eq!(
        (r.prefix[0], r.suffix[0], r.auto_affix),
        (axe.prefix[0], 33, axe.auto_affix)
    );
    assert_eq!((r.varinvgfx, r.gfx), (true, 5));
    // The partner equal to the recorded value is not written twice.
    assert_eq!(r.main, Some(vec![e(17, 0, 40), e(18, 0, 40)]));
    // Rare with slots, then crafted.
    let mut rin = full(&c, b"rin ");
    rin.quality = 6;
    rin.rare_prefix = 12;
    rin.rare_suffix = 40;
    rin.prefix = [PREFIX_OFFSET + 1, 0, PREFIX_OFFSET + 3];
    rin.suffix = [0, 8, 0];
    rin.total_quantity = 0;
    rin.stackable = true;
    let r = round_trip(&c, &rin);
    assert_eq!((r.prefix, r.suffix), (rin.prefix, rin.suffix));
    rin.quality = 8;
    round_trip(&c, &rin);
    // Set with lists 0 and 2; unique with a negative file index.
    let mut set = full(&c, b"cap ");
    set.quality = 5;
    set.file_index = 77;
    set.sets[0] = Some(vec![e(19, 0, 1)]);
    set.sets[2] = Some(vec![e(19, 0, 2)]);
    let r = round_trip(&c, &set);
    assert_eq!((r.file_index, &r.sets), (77, &set.sets));
    set.quality = 7;
    set.file_index = -1;
    set.sets = Default::default();
    assert_eq!(round_trip(&c, &set).file_index, -1);
    // Runeword: name id and its own list; personalized name.
    let mut rw = full(&c, b"axe ");
    rw.flags |= hflag::RUNEWORD | hflag::PERSONALIZED;
    rw.runeword = 0x1234;
    rw.runeword_list = Some(vec![e(19, 0, 4)]);
    rw.name[..2].copy_from_slice(b"Al");
    let r = round_trip(&c, &rw);
    assert_eq!((r.runeword, &r.runeword_list), (0x1234, &rw.runeword_list));
    assert_eq!(&r.name[..3], b"Al\0");
    // Other qualities: charm prefix / suffix, body part, scroll spell.
    let mut cm = full(&c, b"cm1 ");
    cm.prefix[0] = PREFIX_OFFSET + 4;
    assert_eq!(round_trip(&c, &cm).prefix[0], PREFIX_OFFSET + 4);
    cm.prefix[0] = 0;
    cm.suffix[0] = 6;
    assert_eq!(round_trip(&c, &cm).suffix[0], 6);
    let mut hrt = full(&c, b"hrt ");
    hrt.file_index = 300;
    assert_eq!(round_trip(&c, &hrt).file_index, 300);
    let mut tsc = full(&c, b"tsc ");
    tsc.suffix[0] = 17;
    assert_eq!(round_trip(&c, &tsc).suffix[0], 17);
}

/// `d2s.md` §8.2 rules 4 and 8: a full record's filled count of children
/// follows it, each its own padded entry; the decoder keeps the stored
/// flags except 0x80000 and the alt-code bit (§8.2 rule 7).
// Covers: specs/formats/d2s.md §8.1 r2, §8.2 r4, §8.2 r8
#[test]
fn children_follow_their_parent() {
    let c = codes();
    let mut cap = full(&c, b"cap ");
    cap.flags |= hflag::SOCKETED | hflag::INIT;
    cap.base_sockets = 2;
    cap.filled = 2;
    let child = StreamItem {
        flags: 0x10,
        compact: true,
        version: 101,
        code: *b"hp1 ",
        mode: 6,
        ..StreamItem::default()
    };
    cap.children = vec![child.clone(), child];
    let (bytes, _) = write_save(&cap, &c.t.isc).unwrap();
    let got = read_save_entry(&bytes, &c.t).unwrap();
    assert_eq!(got.len, bytes.len());
    assert_eq!(got.children.len(), 2);
    assert_eq!(got.children[0].item.item.mode, 6);
    assert_eq!(got.item.item.flags & hflag::INIT, 0, "0x80000 dropped");
    assert_ne!(got.item.item.flags & hflag::FORCED, 0, "kept as stored");
}

/// Errors: a missing marker, an unknown code, a short record, a set
/// padding bit.
#[test]
fn reader_errors() {
    let c = codes();
    let hp1 = StreamItem {
        flags: 0x10,
        compact: true,
        version: 101,
        code: *b"hp1 ",
        ..StreamItem::default()
    };
    let (mut bytes, _) = write_save(&hp1, &c.t.isc).unwrap();
    assert_eq!(
        read::read_save_record(&mut BitReader::new(&bytes[..4]), &c.t),
        Err(ReadError::Short(16)),
        "the flags do not fit after the marker"
    );
    let last = bytes.len() - 1;
    bytes[last] |= 0x80;
    assert!(matches!(
        read_save_entry(&bytes, &c.t),
        Err(ReadError::Padding(_))
    ));
    bytes[0] = 0;
    assert!(matches!(
        read_save_entry(&bytes, &c.t),
        Err(ReadError::BadMarker(_))
    ));
    let unknown = StreamItem {
        code: *b"zzz ",
        ..hp1
    };
    let (bytes, _) = write_save(&unknown, &c.t.isc).unwrap();
    assert_eq!(
        read_save_entry(&bytes, &c.t),
        Err(ReadError::UnknownCode(*b"zzz "))
    );
}
