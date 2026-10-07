//! The corpse pickup of C→S 0x16 type 0 (`inventory-moves.md` §7.1):
//! the desk hands `0x0057FB70` to the unit hooks
//! (`UnitHooks::player_corpse_pickup`, the action wiring's
//! `ActionHooks::corpse_pickup`), not to the rest.

use super::*;
use crate::items::moves::MovePending;

// Covers: specs/items/inventory-moves.md §7.1
#[test]
fn corpse_pickup_goes_to_the_unit_hooks() {
    let mut w = World::new();
    let (me, player) = (w.me(), w.player);
    // A unit that is not in the game: nothing is handed over.
    w.desk(|d| MovePending::corpse_pickup(d, me, Owner::player(0xDEAD)));
    assert!(w.hooks.corpse_pickups.is_empty());
    w.desk(|d| MovePending::corpse_pickup(d, me, me));
    assert_eq!(w.hooks.corpse_pickups, [(player, player)]);
    assert!(w.rest.log.is_empty(), "{:?}", w.rest.log);
}
