//! The bit stream on the real item (`bits.rs`): the runeword record's
//! name id from the item's own inventory (`items/bitstream.md` §4.4 rule
//! 1, `items/properties.md` §10.1) and the writer's changes to the item
//! (§4.1 rule 8, §4.3 rule 7), written back to the item store.

use super::*;
use crate::items::tables::RuneRec;

/// A ground sword with `sockets` (stat 194) and an inventory holding one
/// key per entry of `fillers`; returns (sword GUID, sword unit).
fn socketed(w: &mut World, sockets: i32, fillers: usize) -> (Guid, UnitId) {
    let s = w.ground_item(SWORD, 20, 20);
    let u = w.unit(s).unwrap();
    w.set_stat(u, super::super::inv_world::STAT_SOCKETS, sockets);
    w.state.add_inventory(u, UnitKind::Item, s);
    for i in 0..fillers {
        let k = w.ground_item(KEY, 30 + i as i32, 20);
        let ku = w.unit(k).unwrap();
        w.desk(|d| {
            let mut inv = d.state.inventories.remove(&u).unwrap();
            inv.link(d, ku, None);
            d.state.inventories.insert(u, inv);
        });
    }
    (s, u)
}

/// §4.4 rule 1: the record of §10.1 (fillers = the class ids of the
/// item's inventory in list order, socket count = stat 194) gives its
/// name id (+0x82); no record (count mismatch, empty inventory, a
/// magic item, a row whose runes differ) → 0xFFFF.
// Covers: specs/items/bitstream.md §4.4 r1; specs/items/properties.md §10.1 r1, §10.1 r2, §10.1 r3, §10.1 r4, §10.1 r5
#[test]
fn runeword_name_from_the_items_inventory() {
    let mut w = World::new();
    w.tables.runes = vec![
        RuneRec {
            complete: 1,
            itype: [T_WEAP as i16, 0, 0, 0, 0, 0],
            runes: [HP1 as i32, 0, 0, 0, 0, 0],
            name_id: 0x0BAD,
            ..RuneRec::default()
        },
        RuneRec {
            complete: 1,
            itype: [T_WEAP as i16, 0, 0, 0, 0, 0],
            runes: [KEY as i32, KEY as i32, 0, 0, 0, 0],
            name_id: 0x1234,
            ..RuneRec::default()
        },
    ];
    let (s, u) = socketed(&mut w, 2, 2);
    assert_eq!(w.desk(|d| d.runeword_name(u)), 0x1234);
    assert_eq!(
        w.desk(|d| d.stream_item(s, 0, 0).map(|v| v.runeword)),
        Some(0x1234)
    );
    // Socket count ≠ filler count.
    w.set_stat(u, super::super::inv_world::STAT_SOCKETS, 3);
    assert_eq!(w.desk(|d| d.runeword_name(u)), 0xFFFF);
    // Quality 4 (magic).
    w.set_stat(u, super::super::inv_world::STAT_SOCKETS, 2);
    w.items.get_mut(u).unwrap().quality = q::MAGIC;
    assert_eq!(w.desk(|d| d.runeword_name(u)), 0xFFFF);
    // An empty inventory with a row of no runes: none (step 1).
    let (_, e) = socketed(&mut w, 0, 0);
    w.tables.runes[0].runes = [0; 6];
    assert_eq!(w.desk(|d| d.runeword_name(e)), 0xFFFF);
}

/// §4.1 rule 8 and §4.3 rule 7 (edge case 2): item level 0 and quality
/// 12 are written as 1 and 2, and the item store holds them after the
/// call. The overwrite branch skips the property lists (only the main
/// list's 0x1FF), so the next stream, of a quality-2 item, carries them
/// and differs; it is stable from then on.
// Covers: specs/items/bitstream.md §edge-cases-original-bugs r2
#[test]
fn writer_changes_reach_the_item() {
    let mut w = World::new();
    let s = w.ground_item(CAP, 20, 20);
    let u = w.unit(s).unwrap();
    {
        let it = w.items.get_mut(u).unwrap();
        it.ilvl = 0;
        it.quality = 12;
    }
    let first = w.desk(|d| d.item_stream(s, 0, 0));
    assert!(!first.is_empty());
    let it = w.items.get(u).unwrap();
    assert_eq!((it.ilvl, it.quality), (0, 12), "queued, not yet written");
    w.desk(|_| ());
    let it = w.items.get(u).unwrap();
    assert_eq!((it.ilvl, it.quality), (1, q::NORMAL));
    let second = w.desk(|d| d.item_stream(s, 0, 0));
    assert_ne!(second, first);
    assert_eq!(w.desk(|d| d.item_stream(s, 0, 0)), second);
    assert!(w.state.write_backs.borrow().is_empty());
}
