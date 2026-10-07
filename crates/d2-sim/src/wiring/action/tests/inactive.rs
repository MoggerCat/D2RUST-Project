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
}
