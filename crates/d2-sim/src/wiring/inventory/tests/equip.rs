//! 0x1A EquipItem (§7.5, §4.6), 0x1B Swap2HandedItem (§7.6), 0x1C
//! RemoveBodyItem (§7.7) and 0x1D SwapCursorWithBody (§7.8) on the real
//! body grid.

use super::*;

/// Equips `record` from the cursor at `loc` through 0x1A; drained.
fn equipped(w: &mut World, record: usize, loc: u32) -> Guid {
    let g = w.cursor_item(record);
    assert_eq!(w.handle(&body(0x1A, g, loc)), Ok(0));
    w.drain();
    g
}

/// §7.5 → §4.6: a cap to the head: grid 0 cell 1, mode 1, body location
/// 1, page 0xFF, command flag 0x8 and item flag 0x1 → 0x9D action 6 to
/// all (row 5). §7.7: off again: cursor, mode 4, command flag 0x10 → 0x9D
/// action 8 (row 7).
#[test]
fn equip_and_unequip_a_helm() {
    let mut w = World::new();
    let c = w.cursor_item(CAP);
    let u = w.unit(c).unwrap();
    assert_eq!(w.handle(&body(0x1A, c, 11)), Ok(2), "location ∉ 1..10");
    assert_eq!(w.handle(&body(0x1A, c, 1)), Ok(0));
    assert_eq!(w.mode(c), 1);
    assert_eq!(w.inventory().body_item(1), Some(u));
    assert_eq!(w.inventory().cursor(), None);
    let d = w.data(c);
    assert_eq!(
        (d.body_loc, d.page, d.cmd_flags, d.node_kind),
        (1, 0xFF, 0x8, 3)
    );
    assert_eq!(d.flags & 0x1, 0x1);
    assert_eq!(item_msgs(&w.drain()), [(0x9D, 0x06, c)]);

    assert_eq!(w.handle(&unequip(1)), Ok(0));
    assert_eq!(w.mode(c), 4);
    assert_eq!(w.inventory().body_item(1), None);
    assert_eq!(w.inventory().cursor(), Some(u));
    assert_eq!(w.data(c).cmd_flags, 0x10);
    assert_eq!(item_msgs(&w.drain()), [(0x9D, 0x08, c)]);
    assert_eq!(w.handle(&unequip(1)), Ok(0), "cursor present: nothing");
    assert!(w.state.errors.is_empty());
}

/// §4.3 with requirements (§4.2): strength 10 < reqstr 50 → the check
/// gives 0, §4.6 refuses without out → 0; the item stays on the cursor.
#[test]
fn equip_refused_by_strength() {
    let mut w = World::new();
    let h = w.cursor_item(HEAVY_CAP);
    assert_eq!(w.handle(&body(0x1A, h, 1)), Ok(0));
    assert_eq!(w.mode(h), 4);
    assert_eq!(w.inventory().body_item(1), None);
    let p = w.player;
    w.set_stat(p, 0, 50);
    assert_eq!(w.handle(&body(0x1A, h, 1)), Ok(0));
    assert_eq!(w.mode(h), 1, "strength 50 meets reqstr 50");
}

/// §7.6: a two-handed sword onto the right hand while the left holds a
/// shield: §4.3 gives 2; the shield leaves the body (mode 4, not
/// listed), the sword goes to location 4 with command flag 0x10000 → 0x9D
/// action 7 (row 6). §7.7 takes it off again (0x9D action 8).
#[test]
fn two_handed_swap_and_removal_from_the_other_hand() {
    let mut w = World::new();
    let s = equipped(&mut w, SHIELD, 5);
    let t = w.cursor_item(TWO_HANDER);
    w.inv.items[TWO_HANDER].twohanded = 1;
    let (su, tu) = (w.unit(s).unwrap(), w.unit(t).unwrap());

    assert_eq!(w.handle(&body(0x1A, t, 4)), Ok(0), "§4.3 gives 2 ≠ 1");
    assert_eq!(w.mode(t), 4);
    assert_eq!(w.handle(&body(0x1B, t, 3)), Ok(3), "location ∉ {{4, 5}}");
    assert_eq!(w.handle(&body(0x1B, t, 4)), Ok(0));
    assert_eq!(w.inventory().body_item(4), Some(tu));
    assert_eq!(w.inventory().body_item(5), None);
    // X stays the cursor item: `0x00563D20` does not clear the cursor
    // (§7.6, WN2).
    assert_eq!(w.inventory().cursor(), Some(su));
    assert_eq!(w.mode(s), 4);
    assert!(!w.inventory().contains(su));
    assert_eq!(w.mode(t), 1);
    assert_eq!(w.data(t).cmd_flags, 0x10000);
    assert_eq!(w.data(s).node_grid, 0, "unlinked");
    assert_eq!(item_msgs(&w.drain()), [(0x9D, 0x07, t)]);

    // §4.3 without N on the empty left hand: 4, and `0x0063E490` picks
    // the two-handed sword of the other hand.
    let me = w.me();
    use crate::items::moves::InventoryOps;
    // 0x1C needs an empty cursor: the shield leaves it (§1.4 rule 3).
    w.desk(|d| d.set_cursor(me, None));
    assert_eq!(w.inventory().cursor(), None);
    assert_eq!(w.desk(|d| d.equip_check(me, 5, None, false)), 4);
    assert_eq!(w.desk(|d| d.item_to_remove(me, 5)), Some(t));
    // §7.7: the empty left hand is refused before §4.3 ("empty location
    // → 0"), so its result 4 is not reached from 0x1C; the right hand
    // gives 3.
    assert_eq!(w.handle(&unequip(5)), Ok(0));
    assert_eq!(w.inventory().body_item(4), Some(tu));
    assert_eq!(w.handle(&unequip(4)), Ok(0));
    assert_eq!(w.inventory().body_item(4), None);
    assert_eq!(w.inventory().cursor(), Some(tu));
    assert_eq!(item_msgs(&w.drain()), [(0x9D, 0x08, t)]);
    assert!(w.state.errors.is_empty());
}

/// §7.6: with a one-handed sword in the left hand nothing blocks (§4.3
/// gives 1, not 2) → 3; with an empty other hand → 3.
#[test]
fn two_handed_swap_needs_a_blocking_other_hand() {
    let mut w = World::new();
    let t = w.cursor_item(TWO_HANDER);
    w.inv.items[TWO_HANDER].twohanded = 1;
    assert_eq!(w.handle(&body(0x1B, t, 4)), Ok(3), "other hand empty");
    let mut w = World::new();
    let _s = equipped(&mut w, SWORD, 5);
    let n = w.cursor_item(SWORD);
    assert_eq!(w.handle(&body(0x1B, n, 4)), Ok(3), "barbarian: compatible");
    assert_eq!(w.mode(n), 4);
}

/// §7.8: a cap on the cursor over an equipped cap: §4.3 gives 5; E to the
/// cursor (item flags 0x80 | 0x1, command flag 0x20), N to the head (item
/// flags 0x40 | 0x1, command flag 0x20); both 0x9D action 9 in update-list
/// order. Empty location → 1.
#[test]
fn swap_cursor_with_body() {
    let mut w = World::new();
    let e = equipped(&mut w, CAP, 1);
    let n = w.cursor_item(CAP);
    assert_eq!(w.handle(&body(0x1D, n, 9)), Ok(1), "feet empty");
    assert_eq!(w.handle(&body(0x1D, n, 1)), Ok(0));
    assert_eq!(w.inventory().body_item(1), w.unit(n));
    assert_eq!(w.inventory().cursor(), w.unit(e));
    assert_eq!((w.mode(e), w.mode(n)), (4, 1));
    assert_eq!(w.data(e).flags & 0x81, 0x81);
    assert_eq!(w.data(n).flags & 0x41, 0x41);
    assert_eq!((w.data(e).cmd_flags, w.data(n).cmd_flags), (0x20, 0x20));
    assert_eq!(item_msgs(&w.drain()), [(0x9D, 0x09, e), (0x9D, 0x09, n)]);
}

/// REC-161 (d2rs-own, unverified): a worn item's stat list is attached to
/// the wearer, so its base damage reaches the wearer's stats (21 / 22),
/// and goes with the item when it comes off.
#[test]
fn a_worn_weapon_gives_its_damage_to_the_wearer() {
    const MIN: u16 = 21;
    const MAX: u16 = 22;
    let mut w = World::new();
    w.state.link_item_stats = true;
    let p = w.player;
    let c = w.cursor_item(SWORD);
    let u = w.unit(c).unwrap();
    w.set_stat(u, MIN, 3);
    w.set_stat(u, MAX, 9);
    assert_eq!(w.stats.unit_total(p, MAX, 0), 0);
    assert_eq!(w.handle(&body(0x1A, c, 4)), Ok(0));
    assert_eq!(w.mode(c), 1);
    assert_eq!(w.stats.unit_total(p, MIN, 0), 3);
    assert_eq!(w.stats.unit_total(p, MAX, 0), 9);
    assert_eq!(w.handle(&unequip(4)), Ok(0));
    assert_eq!(w.stats.unit_total(p, MIN, 0), 0);
    assert_eq!(w.stats.unit_total(p, MAX, 0), 0);
}

/// A gem socketed into a worn item reaches the wearer (the filler's list
/// hangs on the item's list, which hangs on the wearer's).
#[test]
fn a_socketed_gem_reaches_the_wearer() {
    use crate::items::tables::{GemRec, PropRec, PropSlot, PropertyRec};
    const STAT: u16 = 31;
    let mut w = World::new();
    w.state.link_item_stats = true;
    let p = w.player;
    let sword = equipped(&mut w, SWORD, 4);
    let su = w.unit(sword).unwrap();
    w.items.get_mut(su).unwrap().flags |= 0x800;
    w.set_stat(su, super::super::inv_world::STAT_SOCKETS, 1);
    let mut pr = PropertyRec::default();
    pr.slots[0] = PropSlot {
        func: 1,
        stat: STAT,
        set: 0,
        val: 0,
    };
    w.tables.properties = vec![pr];
    let block = [
        PropRec {
            code: 0,
            param: 0,
            min: 5,
            max: 5,
        },
        PropRec::NONE,
        PropRec::NONE,
    ];
    w.tables.gems = vec![GemRec { mods: [block; 3] }];
    let gem = w.cursor_item(GEM);
    let before = w.stats.unit_total(p, STAT, 0);
    assert_eq!(w.handle(&msg(0x28, &[gem, sword])), Ok(0));
    assert_eq!(w.mode(gem), 6);
    assert_eq!(w.stats.unit_total(p, STAT, 0), before + 5);
    assert_eq!(w.handle(&unequip(4)), Ok(0));
    assert_eq!(w.stats.unit_total(p, STAT, 0), before);
}
