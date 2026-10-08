// Spec: specs/sim/units.md §3.3, §3.4
//! The inactive store on the action wiring: tick step 9's compress and
//! the restore of the room's next population.

use super::*;
use crate::tick::TickHooks;
use crate::units::inactive::{AreaNode, InactiveStore};

fn node(fx: &Fx) -> Vec<AreaNode> {
    fx.sim.sys.hooks.inactive.as_ref().unwrap().acts[0].clone()
}

/// §3.3 with the store off: nothing happens (the old behaviour).
// Covers: specs/sim/units.md §3.3 text
#[test]
fn without_the_store_compress_does_nothing() {
    let mut fx = Fx::new();
    let a = fx.a;
    let m = fx.spawn(UnitType::Missile, 0, a, 5, 5);
    let mut g = std::mem::take(&mut fx.game);
    fx.sim.compress_unit(&mut g, m);
    fx.game = g;
    assert!(fx.game.lists.unit(m).is_some());
}

/// §3.3: a missile is freed without a record; a monster with
/// `SaveMonsters` 0 is freed without a record; an object with S off is
/// freed; the node of the room holds nothing.
// Covers: specs/sim/units.md §3.3 r9, §3.3 text
#[test]
fn missiles_and_unsaved_monsters_are_freed() {
    let mut fx = Fx::new();
    fx.sim.hooks().enable_inactive_store();
    let a = fx.a;
    let m = fx.spawn(UnitType::Missile, 0, a, 5, 5);
    let mon = fx.spawn(UnitType::Monster, 0, a, 6, 6);
    let mut g = std::mem::take(&mut fx.game);
    fx.sim.compress_unit(&mut g, m);
    fx.sim.compress_unit(&mut g, mon);
    fx.game = g;
    assert!(fx.game.lists.unit(m).is_none());
    assert!(fx.game.lists.unit(mon).is_none());
    assert!(node(&fx).is_empty());
}

/// §3.3 rule 3 / §3.4 rule 1: a living monster with node index < 8 is
/// stored (K := 1) and freed; the record carries its position, class,
/// GUID and frame; the restore of its room hands it to the host's
/// re-spawn, after the node is unlinked.
// Covers: specs/sim/units.md §3.3 r3, §3.4 r1, §3.4 r4
#[test]
fn a_stored_monster_is_restored_with_its_guid() {
    let mut fx = Fx::new();
    fx.sim.hooks().enable_inactive_store();
    let a = fx.a;
    // monstats2 `restore` 1 (rule 8 leaves K; 574 of the 1.14d rows).
    std::sync::Arc::make_mut(&mut fx.sim.sys.hooks.tables)
        .combat
        .monstats2[0]
        .restore = 1;
    let mon = fx.spawn(UnitType::Monster, 0, a, 6, 6);
    fx.sim.sys.units.get_mut(mon).unwrap().node_index = 3;
    let guid = fx.sim.sys.units.get(mon).unwrap().guid;
    fx.game.frame = 77;
    let mut g = std::mem::take(&mut fx.game);
    fx.sim.compress_unit(&mut g, mon);
    fx.game = g;
    assert!(fx.game.lists.unit(mon).is_none());
    let n = node(&fx);
    assert_eq!(n.len(), 1);
    let rec = &n[0].monsters[0];
    assert_eq!((rec.class, rec.guid, rec.frame), (0, guid, 77));
    let mut g = std::mem::take(&mut fx.game);
    assert!(fx.sim.restore(&mut g, a));
    fx.game = g;
    assert!(node(&fx).is_empty());
    assert_eq!(fx.sim.sys.hooks.inactive, Some(InactiveStore::default()));
    // Spawned again with the stored GUID, in mode 1, at its place.
    let back = fx
        .game
        .lists
        .find_unit(UnitType::Monster, guid)
        .expect("the monster is back");
    assert_eq!(fx.game.lists.unit(back).and_then(|e| e.room()), Some(a));
    let r = fx.sim.sys.units.get(back).unwrap();
    assert_eq!((r.class, r.mode), (0, 1));
    assert_eq!(fx.sim.sys.hooks.path_position(back), (6, 6));
}

/// §3.3 / §3.4 rule 4.3: an object with unit flag 0x2000000 (S) is
/// stored and freed; the restore creates a new object (new GUID) of its
/// class and mode at its place, with unit flags 0x3000000 (`0x005557D0`).
// Covers: specs/sim/units.md §3.3 text, §3.4 r3, §3.4 r4
#[test]
fn a_stored_object_comes_back_as_a_new_unit() {
    let mut fx = Fx::new();
    fx.sim.hooks().enable_inactive_store();
    let a = fx.a;
    let o = fx.spawn(UnitType::Object, 3, a, 7, 4);
    fx.sim.sys.units.get_mut(o).unwrap().flags |= 0x200_0000;
    let guid = fx.sim.sys.units.get(o).unwrap().guid;
    let mut g = std::mem::take(&mut fx.game);
    fx.sim.compress_unit(&mut g, o);
    fx.game = g;
    assert!(fx.game.lists.unit(o).is_none());
    let n = node(&fx);
    assert_eq!((n[0].others[0].ty, n[0].others[0].class), (2, 3));
    let mut g = std::mem::take(&mut fx.game);
    assert!(fx.sim.restore(&mut g, a));
    fx.game = g;
    let back: Vec<_> = fx
        .game
        .lists
        .room_units(a)
        .into_iter()
        .filter(|&u| {
            fx.game
                .lists
                .unit(u)
                .is_some_and(|e| e.ty == UnitType::Object)
        })
        .collect();
    assert_eq!(back.len(), 1);
    let r = fx.sim.sys.units.get(back[0]).unwrap();
    assert_eq!((r.class, r.mode), (3, 1));
    assert_ne!(r.guid, guid);
    assert_eq!(r.flags & 0x300_0000, 0x300_0000);
    assert_eq!(fx.sim.sys.hooks.path_position(back[0]), (7, 4));
}

/// §3.3 / §3.4 rule 4.3: a portal (class 59) is kept: detached, not freed,
/// flags 2 |= 0x100; the restore places the same unit again with unit
/// flag 0x10.
// Covers: specs/sim/units.md §3.3 text, §3.4 r4
#[test]
fn a_kept_portal_is_placed_again() {
    let mut fx = Fx::new();
    fx.sim.hooks().enable_inactive_store();
    let a = fx.a;
    let p = fx.spawn(UnitType::Object, 59, a, 5, 3);
    let mut g = std::mem::take(&mut fx.game);
    fx.sim.compress_unit(&mut g, p);
    fx.game = g;
    assert!(fx.game.lists.unit(p).is_some_and(|e| e.room().is_none()));
    assert_ne!(fx.sim.sys.units.get(p).unwrap().flags2 & 0x100, 0);
    let mut g = std::mem::take(&mut fx.game);
    assert!(fx.sim.restore(&mut g, a));
    fx.game = g;
    assert_eq!(fx.game.lists.unit(p).and_then(|e| e.room()), Some(a));
    assert_ne!(fx.sim.sys.units.get(p).unwrap().flags & 0x10, 0);
}
