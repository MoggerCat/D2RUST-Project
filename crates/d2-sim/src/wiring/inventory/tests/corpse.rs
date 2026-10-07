//! The corpse pickup of C→S 0x16 type 0 (`inventory-moves.md` §7.1):
//! the desk hands `0x0057FB70` to the unit hooks
//! (`UnitHooks::player_corpse_pickup`, the action wiring's
//! `ActionHooks::corpse_pickup`), not to the rest.

use super::*;

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

/// A dead player's corpse C with a cap on its head and a key in its
/// backpack; the player U takes both back (§12.2): the cap to U's head
/// (phase 1), the key to U's page 0 (phase 2, second sweep); result 1, then
/// §12.1 step 4 (the rest's removal) and sound 93.
// Covers: specs/items/inventory-moves.md §12.1, §12.2
#[test]
fn corpse_take_back_moves_body_and_stored_items() {
    let mut w = World::new();
    w.hooks.corpse_allowed = true;
    let me = w.me();
    let c = w.alloc(UnitType::Player, 4);
    let cg = w.units.get(c).unwrap().guid;
    w.state.add_inventory(c, UnitKind::Player { class: 4 }, cg);
    let corpse = Owner::player(cg);
    let cap = w.cursor_item(CAP);
    w.desk(|d| {
        assert!(moves::InventoryOps::place_body(d, corpse, cap, 1));
        moves::MoveUnits::set_mode(d, cap, moves::mode::EQUIPPED);
    });
    let key = w.cursor_item(KEY);
    w.desk(|d| {
        assert!(moves::InventoryOps::place_at(d, corpse, key, 0, 0, 0));
        moves::MoveUnits::set_mode(d, key, moves::mode::STORED);
    });
    let mut out = None;
    w.desk(|d| {
        if MovePending::corpse_pickup(d, me, corpse) {
            out = Some(moves::ground::corpse_pickup_rest(d, me, corpse));
        }
    });
    assert_eq!(out, Some(Ok(())));
    assert_eq!(w.data(cap).mode, moves::mode::EQUIPPED);
    assert_eq!(w.inventory().body_item(1), w.unit(cap));
    assert_eq!(w.data(key).mode, moves::mode::STORED);
    assert!(w.state.items_of(c).is_empty());
    let log = &w.rest.log;
    assert!(
        log.contains(&format!("corpse_taken {} {cg}", me.guid)),
        "{log:?}"
    );
    assert_eq!(log.last(), Some(&format!("sound {} 0x5d", me.guid)));
}

/// The take permission refused (steps 1–2): no take-back at all.
// Covers: specs/items/inventory-moves.md §12.1
#[test]
fn corpse_take_back_needs_the_permission() {
    let mut w = World::new();
    let me = w.me();
    assert!(!w.desk(|d| MovePending::corpse_pickup(d, me, me)));
}

/// §12.3 rows: rings move to the free finger; two shields fail; a
/// barbarian takes two weapons.
// Covers: specs/items/inventory-moves.md §12.3
#[test]
fn corpse_slot_fit_rows() {
    let mut w = World::new();
    let me = w.me();
    let cap = w.ground_item(CAP, 11, 11);
    let s1 = w.ground_item(SHIELD, 11, 11);
    let s2 = w.ground_item(SHIELD, 11, 11);
    let sw1 = w.ground_item(SWORD, 11, 11);
    let sw2 = w.ground_item(SWORD, 11, 11);
    w.desk(|d| {
        let fit = |d: &InvDesk<'_, '_, Hooks, Rest>, x, dd, a, l| {
            MovePending::corpse_slot_fit(d, me, x, dd, a, l)
        };
        // Both slots empty: L unchanged.
        assert_eq!(fit(d, cap, None, None, 1), (true, 1));
        // D present, A none: the partner slot; not a hand → fit.
        assert_eq!(fit(d, cap, Some(s1), None, 6), (true, 7));
        // Both present.
        assert_eq!(fit(d, cap, Some(s1), Some(s2), 4), (false, 4));
        // Hands: two shields fail, two swords pass for a barbarian.
        assert_eq!(fit(d, s2, Some(s1), None, 4), (false, 5));
        assert_eq!(fit(d, sw2, Some(sw1), None, 4), (true, 5));
        assert_eq!(fit(d, sw2, None, Some(s1), 4), (true, 4));
    });
}
