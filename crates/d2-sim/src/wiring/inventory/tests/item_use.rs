//! The item-use dispatcher on the desk (`items/use.md` §1–§4,
//! `wiring/inventory/item_use.rs`): the Town Portal entry runs the cast
//! of the hooks, and 0x20 / 0x27 charge the scroll or tome only when the
//! cast made the pair.

use super::*;
use crate::items::inventory::tables::InvBookRec;
use crate::items::moves::{iflag, MovePending, MoveUnits};

const QUANTITY: u16 = 70;
/// Book of Townportal / Scroll of Townportal skills (any ids for the test).
const BOOK_SKILL: i32 = 220;
const SCROLL_SKILL: i32 = 219;

/// `record` stored at the cell (x, y) of page 0; drained.
fn stored(w: &mut World, record: usize, x: u32, y: u32) -> Guid {
    let g = w.cursor_item(record);
    assert_eq!(w.handle(&insert(g, x, y, 0)), Ok(0));
    w.drain();
    g
}

/// The 0x7C / 0x3F messages the rest was sent, in order.
fn use_msgs(w: &mut World) -> Vec<Vec<u8>> {
    std::mem::take(&mut w.rest.sent)
        .into_iter()
        .map(|(_, m)| m)
        .filter(|m| m[0] == 0x7C || m[0] == 0x3F)
        .collect()
}

fn used(g: Guid) -> Vec<u8> {
    let mut m = vec![0x7C, 4];
    m.extend_from_slice(&g.to_le_bytes());
    m
}

fn reset(g: Guid) -> Vec<u8> {
    let mut m = vec![0x3F, 0xFF];
    m.extend_from_slice(&g.to_le_bytes());
    m.extend_from_slice(&[0xFF, 0xFF]);
    m
}

/// A world whose books row 0 is "of Town Portal" (`pSpell` 2).
fn with_books() -> World {
    let mut t = inv_tables();
    t.books = vec![InvBookRec {
        pspell: 2,
        scrollskill: SCROLL_SKILL,
        bookskill: BOOK_SKILL,
    }];
    World::with_tables(t)
}

/// §1 rule 2: `tsc` / `tbk` index 2 from `misc.txt` without a books row;
/// with one, from its `pSpell` and e = `BookSkill`; a key has none.
// Covers: specs/items/use.md §1 r2, §3
#[test]
fn use_index_from_the_books_row_then_the_items_record() {
    let mut w = World::new();
    let s = stored(&mut w, TSC, 0, 0);
    let k = stored(&mut w, KEY, 2, 0);
    assert_eq!(w.desk(|d| d.use_index(s)), Some((2, -1)));
    assert_eq!(w.desk(|d| d.use_index(k)), Some((0, -1)));
    let mut w = with_books();
    let b = stored(&mut w, TBK, 0, 0);
    assert_eq!(w.desk(|d| d.use_index(b)), Some((2, BOOK_SKILL)));
    // `0x0055E050`: the tome's skill is `bookskill`, the scroll's
    // `scrollskill`.
    let s = stored(&mut w, TSC, 2, 0);
    assert_eq!(w.desk(|d| MovePending::item_skill(d, b)), BOOK_SKILL);
    assert_eq!(w.desk(|d| MovePending::item_skill(d, s)), SCROLL_SKILL);
}

/// 0x20 with a scroll, the cast made the pair: the cast was asked for the
/// player, its own 0x7C went out, then §7.11 step 3's targeting reset
/// clears the scroll's flag 0x4 (S→C 0x3F) and the scroll is consumed.
// Covers: specs/items/use.md §1 r5, §4; specs/items/inventory-moves.md §7.11 r3
#[test]
fn a_made_cast_uses_the_scroll() {
    let mut w = World::new();
    w.hooks.cast = Some((1, true));
    let s = stored(&mut w, TSC, 0, 0);
    w.rest.sent.clear();
    assert_eq!(w.handle(&msg(0x20, &[s, 10, 10])), Ok(0));
    assert_eq!(w.hooks.casts, [w.player]);
    assert!(w.unit(s).is_none(), "the scroll is used up");
    assert_eq!(use_msgs(&mut w), [used(s), reset(s)]);
}

/// The town refusal (result 0, no 0x7C from the cast): the scroll stays,
/// its flag 0x4 is cleared with one S→C 0x3F, and the dispatcher's 0x7C
/// is the only one (edge case 2).
// Covers: specs/items/use.md §1 r5, §2, §4, edge case 2
#[test]
fn a_refused_cast_keeps_the_scroll() {
    let mut w = World::new();
    w.hooks.cast = Some((0, false));
    let s = stored(&mut w, TSC, 0, 0);
    w.rest.sent.clear();
    assert_eq!(w.handle(&msg(0x20, &[s, 10, 10])), Ok(0));
    assert!(w.unit(s).is_some(), "the scroll stays");
    assert_eq!(w.data(s).flags & iflag::TARGETING, 0);
    assert_eq!(use_msgs(&mut w), [reset(s), used(s)]);
}

/// A cast that fails outside a town (result 0 after its own 0x7C): two
/// 0x7C (edge case 2); without a cast host the use fails the same way
/// with one.
// Covers: specs/items/use.md §1 r5, edge case 2
#[test]
fn a_failed_cast_sends_two_item_messages() {
    let mut w = World::new();
    w.hooks.cast = Some((0, true));
    let s = stored(&mut w, TSC, 0, 0);
    w.rest.sent.clear();
    assert_eq!(w.handle(&msg(0x20, &[s, 10, 10])), Ok(0));
    assert!(w.unit(s).is_some());
    assert_eq!(use_msgs(&mut w), [used(s), reset(s), used(s)]);
    w.hooks.cast = None;
    assert_eq!(w.handle(&msg(0x20, &[s, 10, 10])), Ok(0));
    assert!(w.unit(s).is_some());
    assert_eq!(use_msgs(&mut w), [reset(s), used(s)]);
}

/// The failure reset clears flag 0x4 on every flagged item of the
/// inventory, in list order, with `3F FF <GUID> FF FF` each (test
/// vector of `items/use.md`).
// Covers: specs/items/use.md §2, test vector 4
#[test]
fn the_failure_reset_clears_every_flagged_item() {
    let mut w = World::new();
    w.hooks.cast = Some((0, false));
    let k = stored(&mut w, KEY, 4, 0);
    let s = stored(&mut w, TSC, 0, 0);
    w.desk(|d| {
        let f = d.item_flags(k);
        d.set_item_flags(k, f | iflag::TARGETING);
    });
    w.rest.sent.clear();
    let me = w.me();
    assert_eq!(w.desk(|d| d.item_use(me, s)), Some(0));
    let order: Vec<Guid> = w
        .inventory()
        .items()
        .iter()
        .map(|&u| w.units.get(u).unwrap().guid)
        .filter(|g| *g == k || *g == s)
        .collect();
    let mut want: Vec<Vec<u8>> = order.into_iter().map(reset).collect();
    want.push(used(s));
    assert_eq!(use_msgs(&mut w), want);
    assert_eq!(w.data(k).flags & iflag::TARGETING, 0);
}

/// A tome loses its charge (stat 70, S→C 0x3E) only when the cast made
/// the pair; a failed cast keeps the charge.
// Covers: specs/items/inventory-moves.md §7.11 r3; specs/items/use.md §4
#[test]
fn a_tome_is_charged_only_on_a_made_cast() {
    let mut w = with_books();
    let t = stored(&mut w, TBK, 0, 0);
    let tu = w.unit(t).unwrap();
    w.set_stat(tu, QUANTITY, 2);
    let it = Owner::item(t);
    w.hooks.cast = Some((0, true));
    assert_eq!(w.handle(&msg(0x20, &[t, 10, 10])), Ok(0));
    assert_eq!(w.desk(|d| d.stat(it, QUANTITY)), 2, "a failed cast is free");
    w.hooks.cast = Some((1, true));
    w.rest.log.clear();
    assert_eq!(w.handle(&msg(0x20, &[t, 10, 10])), Ok(0));
    assert_eq!(w.desk(|d| d.stat(it, QUANTITY)), 1);
    assert_eq!(w.rest.called("send_item_stat"), 1);
    assert!(w.unit(t).is_some(), "a tome stays");
}

/// Items of another entry are not the dispatcher's here: a key's use
/// goes to the rest.
// Covers: specs/items/use.md §3
#[test]
fn other_entries_keep_the_rest_answer() {
    let mut w = World::new();
    w.hooks.cast = Some((1, true));
    let k = stored(&mut w, KEY, 0, 0);
    let me = w.me();
    assert_eq!(w.desk(|d| d.item_use(me, k)), None);
    assert!(w.hooks.casts.is_empty());
}

/// 0x27 with a Town Portal scroll on an item: the use runs the cast;
/// made → the scroll is consumed (§7.18 step 9), not made → result 1 and
/// the scroll stays (step 5).
// Covers: specs/items/inventory-moves.md §7.18 r5, r9; specs/items/use.md §4
#[test]
fn use_item_action_runs_the_cast() {
    let mut w = World::new();
    let target = stored(&mut w, CAP, 0, 0);
    let s = stored(&mut w, TSC, 4, 0);
    w.hooks.cast = Some((0, true));
    let _ = w.handle(&msg(0x27, &[target, s]));
    assert!(w.unit(s).is_some());
    w.hooks.cast = Some((1, true));
    assert_eq!(w.handle(&msg(0x27, &[target, s])), Ok(0));
    assert!(w.unit(s).is_none());
    assert_eq!(w.hooks.casts.len(), 2);
}
