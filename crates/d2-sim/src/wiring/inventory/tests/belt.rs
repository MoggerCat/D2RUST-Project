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

/// §7.17 with the provisional potion effect (REC-102): `hp1` on
/// the player attaches a `healthpot` list with stat 74 per tick that
/// expires; the potion leaves the belt with the belt removal
/// `0x00561E70` (0x9C action 0xF, flag 0x20; recorded 2026-10-09,
/// `facts/items/a1-town-potions-low.tsv` n 11) and the next potion moves
/// to slot 0. A cursor item → 0 before the use.
#[test]
fn use_belt_item() {
    use crate::wiring::inventory::potion::{POTION_FRAMES, STATE_HEALTHPOT};
    let mut w = World::new();
    let a = belted(&mut w, HP1);
    let b = belted(&mut w, HP2);
    let p = w.pguid();
    let pu = w.player;
    // Below full life (at full life the state ends at once,
    // [`belt_potion_state_follows_life`]).
    w.set_stat(pu, 7, 100 << 8);
    w.set_stat(pu, 6, 10 << 8);
    let use_msg = |g: Guid| msg(0x26, &[g, 0, 0]);
    assert_eq!(w.stats.unit_total(pu, 74, 0), 0);
    assert_eq!(w.handle(&use_msg(a)), Ok(0));
    assert_eq!(w.stats.unit_total(pu, 74, 0), 45 * 256 / POTION_FRAMES);
    let l = w.stats.state_list_owner(pu, STATE_HEALTHPOT);
    assert_eq!(l, Some((0, p)));
    assert!(w.unit(a).is_none(), "the potion is freed");
    assert_eq!(slot_of(&w, b), 1, "another column stays");
    let sent: Vec<Vec<u8>> = w.rest.sent.iter().map(|(_, m)| m.clone()).collect();
    assert_eq!(item_msgs(&sent), [(0x9C, 0x0F, a)]);
    assert_eq!(sent.last().map(|m| m[8] & 0x20), Some(0x20));
    // The list expires at its frame.
    let f = w.game.frame + POTION_FRAMES;
    w.stats.expire_lists(&mut w.hooks, pu, f).unwrap();
    assert_eq!(w.stats.unit_total(pu, 74, 0), 0);
    let _c = w.cursor_item(KEY);
    assert_eq!(w.handle(&use_msg(b)), Ok(0));
    assert!(w.unit(b).is_some(), "a cursor item blocks the use");
}

/// Whether `state` is on for `u`, and whether its changed bit is set.
fn state_of(w: &World, u: UnitId, state: u32) -> (bool, bool) {
    let (bits, changed) = w.stats.state_bits(u).expect("an extended list");
    let (i, b) = (state as usize / 32, 1u32 << (state % 32));
    (bits[i] & b != 0, changed[i] & b != 0)
}

/// §7.17 with the potion state (REC-102 provisional effect; recorded
/// 2026-10-09): below full life the `healthpot` state 100 goes on with
/// its changed bit and the player is queued for update, so the client
/// pass sends 0xA8 (`facts/items/a1-town-potions-low.tsv` n 12); at full
/// life it ends in the same frame, the changed bit set, so the pass sends
/// 0xA9 (`facts/items/a1-town-item-moves.tsv` n 43).
#[test]
fn belt_potion_state_follows_life() {
    use crate::wiring::inventory::potion::STATE_HEALTHPOT;
    let mut w = World::new();
    let pu = w.player;
    w.set_stat(pu, 7, 100 << 8);
    w.set_stat(pu, 6, 10 << 8);
    let a = belted(&mut w, HP1);
    assert_eq!(state_of(&w, pu, STATE_HEALTHPOT), (false, false));
    assert_eq!(w.handle(&msg(0x26, &[a, 0, 0])), Ok(0));
    assert_eq!(state_of(&w, pu, STATE_HEALTHPOT), (true, true));
    assert!(w.game.lists.unit(pu).is_some_and(|e| e.is_queued()));
    // Full life: the state is off again, its change still to be sent.
    w.stats.clear_states_changed(pu);
    w.set_stat(pu, 6, 100 << 8);
    let b = belted(&mut w, HP2);
    assert_eq!(w.handle(&msg(0x26, &[b, 0, 0])), Ok(0));
    assert!(w.unit(b).is_none(), "used at full life too");
    assert_eq!(state_of(&w, pu, STATE_HEALTHPOT), (false, true));
    assert_eq!(w.stats.state_list_owner(pu, STATE_HEALTHPOT), None);
    assert_eq!(w.state.errors, Vec::new());
}

/// §7.11 step 3 for a potion (`items/use.md` entry 3, REC-102
/// provisional): a stored `hp1` right-clicked in the grid (0x20) is drunk
/// by the player and consumed (`0x0055E000`): S→C 0x3F of the targeting
/// reset, then 0x9D action 5 with the removal flag 0x20, and the state
/// goes on. Recorded 2026-10-09 for an `mp1`
/// (`facts/items/a1-town-potions-low.tsv` n 28–31); before, d2rs refused
/// it with 0x7C.
#[test]
fn grid_potion_is_drunk_and_consumed() {
    use crate::wiring::inventory::potion::STATE_HEALTHPOT;
    let mut w = World::new();
    let pu = w.player;
    w.set_stat(pu, 7, 100 << 8);
    w.set_stat(pu, 6, 10 << 8);
    let g = w.cursor_item(HP1);
    assert_eq!(w.handle(&msg(0x18, &[g, 0, 0, 0])), Ok(0));
    w.drain();
    assert_eq!(w.mode(g), 0);
    w.rest.sent.clear();
    assert_eq!(w.handle(&msg(0x20, &[g, 0, 0])), Ok(0));
    assert!(w.unit(g).is_none(), "the potion is freed");
    let sent: Vec<Vec<u8>> = w.rest.sent.iter().map(|(_, m)| m.clone()).collect();
    assert_eq!(sent.first().map(|m| m[0]), Some(0x3F));
    assert!(!sent.iter().any(|m| m[0] == 0x7C), "not refused");
    assert_eq!(item_msgs(&sent), [(0x9D, 0x05, g)]);
    let m = sent.iter().find(|m| m[0] == 0x9D).unwrap();
    assert_eq!(m[13] & 0x20, 0x20, "removal flag in the stream");
    assert_eq!(state_of(&w, pu, STATE_HEALTHPOT), (true, true));
    assert_eq!(w.state.errors, Vec::new());
}

/// §6.2 row 4 / §6.4: the 0x9D action 5 of a 0x19 lift (sender
/// `0x0053D010`) shows the stored page, not the cursor page 0xFF.
/// Recorded 2026-10-09: page 0 in the stream (page + 1 = 1,
/// `facts/items/a1-town-potions-low.tsv` n 1).
#[test]
fn lift_shows_the_stored_page() {
    let mut w = World::new();
    let g = w.cursor_item(KEY);
    assert_eq!(w.handle(&msg(0x18, &[g, 0, 0, 0])), Ok(0));
    w.drain();
    assert_eq!(w.handle(&msg(0x19, &[g])), Ok(0));
    let out = w.drain();
    assert_eq!(item_msgs(&out), [(0x9D, 0x05, g)]);
    assert_eq!(w.data(g).page, 0xFF);
    let (stored, cursor) = w.desk(|d| {
        use crate::items::moves::MovePending;
        (d.item_bits(g, 0, 0), d.item_bits(g, 0, 0xFF))
    });
    assert_ne!(stored, cursor);
    assert_eq!(out[0][13..], stored[..]);
}

/// §9.1 step 3: a drop from the cursor (0x17) records the item for the
/// host's update pass, which announces it with 0x9C action 2 after the
/// room clean-up cleared unit flag 0x1000 (`InvState::dropped`).
#[test]
fn a_drop_is_recorded_for_the_update_pass() {
    let mut w = World::new();
    let g = w.cursor_item(KEY);
    w.state.dropped.clear();
    w.rest.room_at = true;
    w.rest.spot = Some(Spot {
        room: w.room,
        x: 13,
        y: 12,
    });
    assert_eq!(w.handle(&msg(0x17, &[g])), Ok(0));
    assert_eq!(w.mode(g), 3);
    let u = w.unit(g).unwrap();
    assert_eq!(w.desk(|d| d.take_dropped()), [u].into_iter().collect());
    assert!(w.state.dropped.is_empty());
}
