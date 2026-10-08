//! Town Portal scroll and tome use (REC-117, `inventory/town_portal.rs`):
//! the use records a request for the host and, for a tome, spends one
//! charge. Any other item is not taken.

use super::*;
use crate::items::moves::{MovePending, MoveUnits};

const QUANTITY: u16 = 70;

/// A scroll used from the inventory: the request names the player, the
/// item is not touched here (the move handler consumes it).
#[test]
fn scroll_use_records_a_portal_request() {
    let mut w = World::new();
    let s = w.cursor_item(TSC);
    let (me, pu) = (w.me(), w.player);
    assert!(w.desk(|d| d.use_item_at(me, s, 0, 0)));
    assert_eq!(w.desk(|d| d.take_portal_requests()), [pu]);
    assert!(w.desk(|d| d.take_portal_requests()).is_empty());
}

/// A tome loses a charge per use; an empty tome is refused.
#[test]
fn tome_use_spends_a_charge() {
    let mut w = World::new();
    let t = w.cursor_item(TBK);
    let tu = w.unit(t).unwrap();
    w.set_stat(tu, QUANTITY, 2);
    let me = w.me();
    let it = crate::items::moves::Owner::item(t);
    assert!(w.desk(|d| d.use_item_at(me, t, 0, 0)));
    assert_eq!(w.desk(|d| d.stat(it, QUANTITY)), 1);
    assert!(w.desk(|d| d.use_item_at(me, t, 0, 0)));
    assert_eq!(w.desk(|d| d.stat(it, QUANTITY)), 0);
    assert!(!w.desk(|d| d.use_item_at(me, t, 0, 0)), "empty");
    assert_eq!(w.desk(|d| d.take_portal_requests()).len(), 2);
}

/// Another item is not a portal item.
#[test]
fn other_items_are_left_alone() {
    let mut w = World::new();
    let k = w.cursor_item(KEY);
    let me = w.me();
    assert!(!w.desk(|d| d.use_item_at(me, k, 0, 0)));
    assert!(w.desk(|d| d.take_portal_requests()).is_empty());
}
