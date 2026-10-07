//! The item copy (`copy.rs`, `world/vendors.md` §7.3) on the real item:
//! save stream with children, the first record read back into a new
//! item unit, the flags, the source's mark, the per-item reset.

use super::*;
use crate::items::bitstream::Isc;
use crate::items::{flag, stat, ListKey};
use crate::wiring::inventory::copy::COPIED;
use crate::wiring::inventory::InvError;

/// Save columns for the stats the tests write (`Save Bits` > 0).
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

fn world() -> World {
    let mut w = World::new();
    w.tables.isc = isc();
    w
}

/// §7.3 steps 2–8 on a held cap (the cursor item) with defense,
/// durability and a main list: a new unit (the next GUID) of the same
/// class in no room, in the source's mode and position, the record's
/// fields and stats, unit +0x28,
/// item flag 0x80000 set and 0x2000 cleared, the source marked
/// 0x8000000, the per-item reset (item flags 0x1 … cleared).
// Covers: specs/world/vendors-2.md §7.3 r1, §7.3 r2, §7.3 r3, §7.3 r4, §7.3 r6, §7.3 r8
#[test]
fn copy_of_a_held_item() {
    let mut w = world();
    let s = w.cursor_item(CAP);
    let su = w.unit(s).unwrap();
    w.set_stat(su, stat::ARMORCLASS, 7);
    w.set_stat(su, stat::MAXDURABILITY, 12);
    w.set_stat(su, stat::DURABILITY, 9);
    {
        let it = w.items.get_mut(su).unwrap();
        it.ilvl = 17;
        it.flags |= flag::INSTORE | 0x1;
    }
    w.econ().with_stats(|ctx| {
        let mut st = crate::wiring::economy::UnitStats::new(ctx, su);
        crate::items::ItemStats::list_set(&mut st, ListKey::ITEM, 19, 0, 33);
    });
    let init = w.units.get(su).unwrap().init_seed;
    let c = w.desk(|d| d.copy_of(su, true)).expect("copied");
    assert_ne!(c, su);
    let cg = w.units.get(c).unwrap().guid;
    assert_eq!(cg, s + 1, "the next item GUID");
    // The save's 32 bits (unit +0x28) become the copy's item seed
    // (`rng.md` §5.3).
    let seed = w.units.get(c).unwrap().item_seed.unwrap().0;
    assert_eq!(seed, crate::rng::Seed::init_low(init));
    assert_eq!(w.units.get(c).unwrap().mode, w.units.get(su).unwrap().mode);
    assert_eq!(w.game.lists.unit(c).unwrap().room(), None);
    assert_eq!((w.data(cg).x, w.data(cg).y), (w.data(s).x, w.data(s).y));
    let (a, b) = (w.items.get(su).unwrap(), w.items.get(c).unwrap());
    assert_eq!((b.record, b.ilvl, b.quality), (a.record, 17, a.quality));
    assert_ne!(b.flags & flag::INIT, 0);
    assert_eq!(b.flags & (flag::INSTORE | 0x1), 0, "0x2000 and the reset");
    assert_ne!(a.flags & COPIED, 0);
    for s_ in [stat::ARMORCLASS, stat::MAXDURABILITY, stat::DURABILITY] {
        assert_eq!(
            w.stats.unit_total(c, s_, 0),
            w.stats.unit_total(su, s_, 0),
            "stat {s_}"
        );
    }
    let list = w.econ().with_stats(|ctx| {
        let st = crate::wiring::economy::UnitStats::new(ctx, c);
        crate::items::ItemStats::list_get(&st, ListKey::ITEM, 19, 0)
    });
    assert_eq!(list, 33);
    assert!(w.state.errors.is_empty());
}

/// Step 1.1: a source on the ground (in a room) is a caller error: no
/// copy, no new unit, the source unmarked, the error recorded.
// Covers: specs/world/vendors-2.md §7.3 r1
#[test]
fn a_ground_source_is_a_caller_error() {
    let mut w = world();
    let s = w.ground_item(CAP, 20, 21);
    let su = w.unit(s).unwrap();
    let before = w.items.len();
    assert_eq!(w.desk(|d| d.copy_of(su, true)), None);
    assert_eq!(w.items.len(), before);
    assert_eq!(w.items.get(su).unwrap().flags & COPIED, 0);
    assert_eq!(w.state.errors, [InvError::GroundCopySource(su)]);
}

/// Step 2: a stream that does not fit 1,024 bytes gives length 0 and
/// the read fails: no copy.
// Covers: specs/world/vendors-2.md §7.3 r2
#[test]
fn a_stream_over_the_buffer_copies_nothing() {
    let mut w = world();
    let s = w.cursor_item(CAP);
    let su = w.unit(s).unwrap();
    w.econ().with_stats(|ctx| {
        let mut st = crate::wiring::economy::UnitStats::new(ctx, su);
        for p in 0..500u16 {
            crate::items::ItemStats::list_set(&mut st, ListKey::ITEM, 19, p, 1);
        }
    });
    let before = w.items.len();
    assert_eq!(w.desk(|d| d.copy_of(su, true)), None);
    assert_eq!(w.items.len(), before);
}

/// Step 5: with fillers 0 the children are not read: the copy keeps the
/// socketed flag and its socket count, without fillers; with fillers 1
/// each child is read, allocated (two more game-seed steps) and linked
/// into the copy in mode 6 (provisional reading of `0x00562660(…, 0, 1,
/// 0, 0)`).
// Covers: specs/world/vendors-2.md §7.3 r5
#[test]
fn children_and_the_fillers_argument() {
    let mut w = world();
    let s = w.cursor_item(SWORD);
    let su = w.unit(s).unwrap();
    w.items.get_mut(su).unwrap().flags |= 0x800;
    w.set_stat(su, stat::NUMSOCKETS, 2);
    w.state.add_inventory(su, UnitKind::Item, s);
    let k = w.ground_item(KEY, 22, 21);
    let ku = w.unit(k).unwrap();
    w.desk(|d| {
        let mut inv = d.state.inventories.remove(&su).unwrap();
        inv.link(d, ku, None);
        d.state.inventories.insert(su, inv);
    });
    // The stream's filled count is the inventory's only with `hasinv`.
    w.tables.items[SWORD].hasinv = 1;
    let c = w.desk(|d| d.copy_of(su, false)).expect("copied");
    assert_ne!(w.items.get(c).unwrap().flags & 0x800, 0);
    assert_eq!(w.stats.unit_total(c, stat::NUMSOCKETS, 0), 2);
    assert!(!w.state.inventories.contains_key(&c), "no fillers");
    let seed = w.desk(|d| d.econ.fields.seed);
    let c2 = w
        .desk(|d| d.copy_of(su, true))
        .expect("copied with fillers");
    let mut steps = seed;
    for _ in 0..4 {
        steps.step();
    }
    assert_eq!(w.desk(|d| d.econ.fields.seed), steps, "2 · (1 + 1) steps");
    let kids = w
        .state
        .inventories
        .get(&c2)
        .expect("fillers")
        .items()
        .to_vec();
    assert_eq!(kids.len(), 1);
    assert_ne!(kids[0], ku);
    assert_eq!(w.state.items.get(&kids[0]).unwrap().mode, 6);
    assert_eq!(w.state.errors, Vec::new());
}
