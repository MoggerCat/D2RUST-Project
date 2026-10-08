//! The belt (§3) through auto pickup (§8.1 step 6), 0x23 ItemToBelt
//! (§7.14), 0x24 ItemFromBelt (§7.15, compaction §3.8), 0x25
//! SwitchBeltItem (§7.16) and 0x26 UseBeltItem (§7.17).

use super::*;

/// A potion auto-picked into the belt; drained.
fn belted(w: &mut World, record: usize) -> Guid {
    let g = w.ground_item(record, 12, 11);
    assert_eq!(w.handle(&pick(g, 0)), Ok(0));
    w.drain();
    g
}

fn slot_of(w: &World, g: Guid) -> i32 {
    let d = w.data(g);
    assert_eq!((d.node_grid, d.node_kind), (2, 2), "in the belt grid");
    d.x
}

/// §8.1 step 6 with no belt equipped (default record 2, 4 boxes): `hp1`
/// goes to slot 0 (B4), a second `hp2` to slot 1 (B1, autobelt ≠ 0):
/// mode 2, page 0xFF, command flag 0x2000 → 0x9C action 0xE (row 11).
#[test]
fn auto_pickup_fills_the_belt() {
    let mut w = World::new();
    let a = w.ground_item(HP1, 12, 11);
    assert_eq!(w.handle(&pick(a, 0)), Ok(0));
    assert_eq!(w.mode(a), 2);
    assert_eq!(slot_of(&w, a), 0);
    let d = w.data(a);
    assert_eq!((d.page, d.cmd_flags), (0xFF, 0x2000));
    assert!(!w.in_room(a));
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x0E, a)]);
    let b = belted(&mut w, HP2);
    assert_eq!(slot_of(&w, b), 1);
}

/// §7.14: the cursor potion to slot 4 (row 1 of column 0; no `numboxes`
/// check, edge case 6): mode 2, command flag 0x400 → 0x9C action 0xE.
/// §7.15: taking slot 0 back: cursor, command flag 0x800 → 0x9C action
/// 0xF; compaction (§3.8) moves slot 4 down to slot 0 with item flags
/// 0x400 | 0x1 → 0x9D action 0x15 (row 20, mode 2).
#[test]
fn belt_in_out_and_compaction() {
    let mut w = World::new();
    let a = belted(&mut w, HP1);
    let b = w.cursor_item(HP1);
    assert_eq!(w.handle(&msg(0x23, &[b, 4])), Ok(0));
    assert_eq!(w.mode(b), 2);
    assert_eq!(slot_of(&w, b), 4);
    assert_eq!(w.data(b).cmd_flags, 0x400);
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x0E, b)]);

    assert_eq!(w.handle(&msg(0x24, &[a])), Ok(0));
    assert_eq!(w.mode(a), 4);
    assert_eq!(w.inventory().cursor(), w.unit(a));
    assert_eq!(w.data(a).cmd_flags, 0x800);
    assert_eq!(slot_of(&w, b), 0, "compacted 4 → 0");
    assert_eq!(w.data(b).flags & 0x401, 0x401);
    assert_eq!(w.inventory().belt_item(4), None);
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x0F, a), (0x9D, 0x15, b)]);
    assert_eq!(w.handle(&msg(0x24, &[b])), Ok(2), "a cursor item → 2");
}

/// §7.16: the cursor potion C and the belt potion B change places: B to
/// the cursor, C to B's slot; both command flag 0x1000 → 0x9C action
/// 0x10 in update-list order.
#[test]
fn switch_belt_item() {
    let mut w = World::new();
    let _a = belted(&mut w, HP1);
    let b = belted(&mut w, HP1);
    assert_eq!(slot_of(&w, b), 1);
    let c = w.cursor_item(HP2);
    assert_eq!(w.handle(&msg(0x25, &[c, b])), Ok(0));
    assert_eq!(w.inventory().cursor(), w.unit(b));
    assert_eq!((w.mode(b), w.mode(c)), (4, 2));
    assert_eq!(slot_of(&w, c), 1);
    assert_eq!(item_msgs(&w.drain()), [(0x9C, 0x10, b), (0x9C, 0x10, c)]);
}

/// §7.17 with the provisional potion effect (REC-BELT-POTION): `hp1` on
/// the player attaches a `healthpot` list with stat 74 per tick that
/// expires; the potion leaves the belt with a removal message (0x9D
/// action 5, flag 0x20) and the next potion moves to slot 0. A cursor
/// item → 0 before the use.
#[test]
fn use_belt_item() {
    use crate::wiring::inventory::potion::{POTION_FRAMES, STATE_HEALTHPOT};
    let mut w = World::new();
    let a = belted(&mut w, HP1);
    let b = belted(&mut w, HP2);
    let p = w.pguid();
    let pu = w.player;
    let use_msg = |g: Guid| msg(0x26, &[g, 0, 0]);
    assert_eq!(w.stats.unit_total(pu, 74, 0), 0);
    assert_eq!(w.handle(&use_msg(a)), Ok(0));
    assert_eq!(w.stats.unit_total(pu, 74, 0), 45 * 256 / POTION_FRAMES);
    let l = w.stats.state_list_owner(pu, STATE_HEALTHPOT);
    assert_eq!(l, Some((0, p)));
    assert!(w.unit(a).is_none(), "the potion is freed");
    assert_eq!(slot_of(&w, b), 1, "another column stays");
    let sent: Vec<Vec<u8>> = w.rest.sent.iter().map(|(_, m)| m.clone()).collect();
    assert_eq!(item_msgs(&sent), [(0x9D, 0x05, a)]);
    // The list expires at its frame.
    let f = w.game.frame + POTION_FRAMES;
    w.stats.expire_lists(&mut w.hooks, pu, f).unwrap();
    assert_eq!(w.stats.unit_total(pu, 74, 0), 0);
    let _c = w.cursor_item(KEY);
    assert_eq!(w.handle(&use_msg(b)), Ok(0));
    assert!(w.unit(b).is_some(), "a cursor item blocks the use");
}
