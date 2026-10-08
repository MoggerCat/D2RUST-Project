//! A save entry made an item of the owner's inventory (`load.rs`,
//! `formats/d2s.md` §8.2): written with `save_view`, the original taken
//! out and freed, the entry read back and placed where it was saved.

use super::*;
use crate::items::bitstream::{self, read, BitWriter, Isc};
use crate::items::{flag, stat};
use crate::wiring::inventory::load::LoadFault;

/// Save columns for the stats the tests write (as `copy.rs`).
fn isc() -> Vec<Isc> {
    let mut t = vec![Isc::default(); N_STATS];
    let bits = |b: u8, add: u32| Isc {
        valshift: 0,
        save_bits: b,
        save_add: add,
        save_param_bits: 0,
    };
    t[19] = bits(10, 0);
    t[usize::from(stat::ARMORCLASS)] = bits(11, 10);
    t[usize::from(stat::DURABILITY)] = bits(9, 0);
    t[usize::from(stat::MAXDURABILITY)] = bits(8, 0);
    t[usize::from(stat::NUMSOCKETS)] = bits(4, 0);
    t
}

/// The save entry of `u`, as the d2s writer makes it.
fn entry_of(w: &mut World, u: UnitId) -> read::ReadEntry {
    let bytes = w.desk(|d| {
        let v = d.save_view(u).expect("view");
        let mut bw = BitWriter::new(bitstream::SAVE_BUFFER);
        bitstream::write_save_into(&mut bw, &v, &d.econ.tables.isc);
        bw.finish().expect("fits")
    });
    read::read_save_entry(&bytes, &w.tables).expect("reads back")
}

fn world() -> World {
    let mut w = World::new();
    w.tables.isc = isc();
    w
}

/// A stored item comes back on its page at its cell, in mode 0, with the
/// record's fields and the load's flags (0x80000 set, 0x2000 clear).
// Covers: specs/formats/d2s.md §8.2 r3, §8.2 r7
#[test]
fn a_stored_item_returns_to_its_cell() {
    let mut w = world();
    let p = w.player;
    let k = w.cursor_item(CAP);
    let u = w.unit(k).unwrap();
    w.items.get_mut(u).unwrap().inv_page = 0;
    w.items.get_mut(u).unwrap().ilvl = 23;
    assert!(w.desk(|d| d.place(p, u, (4, 2), false, true)));
    w.drain();
    let e = entry_of(&mut w, u);
    assert!(w.desk(|d| d.remove(p, u)));
    w.desk(|d| d.free(u));
    assert!(w.state.items_of(p).is_empty());

    let n = w.desk(|d| d.load_entry(p, &e)).expect("loaded");
    let g = w.units.get(n).unwrap().guid;
    let d = w.data(g);
    assert_eq!((d.page, d.x, d.y), (0, 4, 2));
    assert_eq!(w.mode(g), 0);
    assert_eq!(w.state.items_of(p), [n]);
    assert_eq!(w.inventory().item_at(2, 4, 2), Some(n));
    let it = w.items.get(n).unwrap();
    assert_eq!((it.record, it.ilvl), (CAP, 23));
    assert_ne!(it.flags & flag::INIT, 0);
    assert_eq!(it.flags & flag::INSTORE, 0);
    assert!(w.state.errors.is_empty());
}

/// The saved cell is taken: the item takes a free position instead of
/// being lost (d2rs-own, PROVISIONAL).
#[test]
fn a_taken_cell_falls_back_to_a_free_position() {
    let mut w = world();
    let p = w.player;
    let k = w.cursor_item(CAP);
    let u = w.unit(k).unwrap();
    w.items.get_mut(u).unwrap().inv_page = 0;
    assert!(w.desk(|d| d.place(p, u, (0, 0), false, true)));
    let e = entry_of(&mut w, u);
    // The original stays at (0, 0): the copy cannot take its cell.
    let n = w.desk(|d| d.load_entry(p, &e)).expect("loaded");
    assert_eq!(w.state.items_of(p).len(), 2);
    let g = w.units.get(n).unwrap().guid;
    assert_ne!((w.data(g).x, w.data(g).y), (0, 0));
}

/// An owner without an inventory in the model loads nothing.
#[test]
fn no_inventory_no_item() {
    let mut w = world();
    let p = w.player;
    let k = w.cursor_item(CAP);
    let u = w.unit(k).unwrap();
    w.items.get_mut(u).unwrap().inv_page = 0;
    assert!(w.desk(|d| d.place(p, u, (0, 0), false, true)));
    let e = entry_of(&mut w, u);
    assert_eq!(
        w.desk(|d| d.load_entry(UnitId(9999), &e)),
        Err(LoadFault::NoInventory)
    );
}

/// An equipped item comes back at its body location, in mode 1.
// Covers: specs/formats/d2s.md §8.2 r3
#[test]
fn an_equipped_item_returns_to_its_body_location() {
    let mut w = world();
    let p = w.player;
    let k = w.cursor_item(CAP);
    assert_eq!(w.handle(&body(0x1A, k, 1)), Ok(0));
    w.drain();
    let u = w.unit(k).unwrap();
    assert_eq!(w.state.body_items(p), [u]);
    let e = entry_of(&mut w, u);
    assert_eq!(e.item.item.mode, 1);
    assert_eq!(w.handle(&unequip(1)), Ok(0));
    w.drain();
    // The helm is on the cursor: put it down and free it.
    let u = w.unit(k).unwrap();
    assert!(w.desk(|d| d.place(p, u, (0, 0), true, false)));
    assert!(w.desk(|d| d.remove(p, u)));
    w.desk(|d| d.free(u));

    let n = w.desk(|d| d.load_entry(p, &e)).expect("loaded");
    let g = w.units.get(n).unwrap().guid;
    assert_eq!(w.mode(g), 1);
    assert_eq!(w.state.body_items(p), [n]);
    assert_eq!(w.data(g).body_loc, 1);
}

/// Rule 5 (`d2s-load.md` §6): a stored item with the runeword flag whose
/// sockets match no runeword row is freed on load.
// Covers: specs/formats/d2s.md §8.2 r5
#[test]
fn a_stale_runeword_item_is_freed() {
    let mut w = world();
    let p = w.player;
    let k = w.cursor_item(CAP);
    let u = w.unit(k).unwrap();
    w.items.get_mut(u).unwrap().inv_page = 0;
    w.items.get_mut(u).unwrap().flags |= bitstream::hflag::RUNEWORD;
    assert!(w.desk(|d| d.place(p, u, (4, 2), false, true)));
    w.drain();
    let e = entry_of(&mut w, u);
    assert!(w.desk(|d| d.remove(p, u)));
    w.desk(|d| d.free(u));
    assert_eq!(
        w.desk(|d| d.load_entry(p, &e)),
        Err(LoadFault::StaleRuneword)
    );
    assert!(w.state.items_of(p).is_empty());
}

/// Without the runeword flag the same entry loads.
#[test]
fn an_item_without_the_runeword_flag_is_not_refreshed() {
    let mut w = world();
    let p = w.player;
    let k = w.cursor_item(CAP);
    let u = w.unit(k).unwrap();
    w.items.get_mut(u).unwrap().inv_page = 0;
    assert!(w.desk(|d| d.place(p, u, (4, 2), false, true)));
    let e = entry_of(&mut w, u);
    assert!(w.desk(|d| d.load_entry(p, &e)).is_ok());
}
