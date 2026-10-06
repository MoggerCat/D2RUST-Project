// Spec: specs/items/inventory.md §3
//! The belt: belt type and boxes (§3.1), slots (§3.2), beltable (§3.3),
//! similar items (§3.4), the free slot for an item (§3.5), the auto-belt
//! gate (§3.6), placing in a slot (§3.7) and compaction (§3.8).

use super::{body, grid::place_in_grid, grid_id, iflag, InvTables, InvWorld, Inventory, BELT_GRID};
use crate::units::UnitId;

/// Belt record used without a belt (§3.1): `belts` record 2, 4 boxes.
pub const DEFAULT_BELT: usize = 2;
/// Slot count of the belt grid (§1.2).
pub const BELT_SLOTS: u8 = 16;

/// Similar-potion groups (`0x00744684`, `0x0074466C`, `0x00744660`; §3.4).
pub const POTION_GROUPS: [&[[u8; 4]]; 3] = [
    &[*b"hp1 ", *b"hp2 ", *b"hp3 ", *b"hp4 ", *b"hp5 "],
    &[*b"mp1 ", *b"mp2 ", *b"mp3 ", *b"mp4 ", *b"mp5 "],
    &[*b"rvl ", *b"rvs "],
];

/// Belt type (`0x00621ED0`, §3.1 rule 1): items `belt` of the item at body
/// location 8; no belt → record 2.
pub fn belt_type<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, t: &InvTables) -> usize {
    inv.body_item(body::BELT)
        .and_then(|b| w.item(b))
        .and_then(|d| t.item(d.record))
        .map_or(DEFAULT_BELT, |r| usize::from(r.belt))
}

/// `numboxes` of the belt in use (§3.1); none when the record is missing.
pub fn belt_numboxes<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, t: &InvTables) -> Option<u8> {
    t.numboxes(belt_type(inv, w, t))
}

/// Beltable (`0x0062BAD0`, §3.3): itemtypes `beltable` of the item's type.
pub fn beltable(t: &InvTables, record: usize) -> bool {
    t.itype_of(record).is_some_and(|r| r.beltable != 0)
}

/// Similar (`0x00628A40`, §3.4): same item class, or both codes in one
/// potion group.
pub fn similar(t: &InvTables, a: usize, b: usize) -> bool {
    if a == b {
        return true;
    }
    let (Some(ra), Some(rb)) = (t.item(a), t.item(b)) else {
        return false;
    };
    POTION_GROUPS
        .iter()
        .any(|g| g.contains(&ra.code) && g.contains(&rb.code))
}

fn is_1x1(t: &InvTables, record: usize) -> bool {
    t.size(record) == Some((1, 1))
}

/// Free slot for an item (`0x0063C600`, §3.5 rule 5).
pub fn free_belt_slot<W: InvWorld + ?Sized>(
    inv: &Inventory,
    w: &W,
    t: &InvTables,
    item: UnitId,
) -> Option<u8> {
    let rec = w.item(item)?.record;
    if !beltable(t, rec) || !is_1x1(t, rec) {
        return None;
    }
    let n = belt_numboxes(inv, w, t)?;
    for c in 0..4u8 {
        let held = inv.belt_item(c).and_then(|i| w.item(i)).map(|d| d.record);
        if c < n && held.is_some_and(|h| similar(t, h, rec)) {
            let slot = (c..n).step_by(4).find(|&s| inv.belt_item(s).is_none());
            // TODO(spec: §3.5 does not say what follows a similar column with no empty slot; the next column is tried)
            if slot.is_some() {
                return slot;
            }
        }
    }
    if t.item(rec).is_some_and(|r| r.autobelt != 0) {
        return (0..4u8).find(|&s| inv.belt_item(s).is_none());
    }
    None
}

/// Auto-belt gate (`0x00628BA0`, §3.6): true for every item. Its call to
/// the bottom-row test `0x0063C560` can only reject on −1, which that test
/// never returns (original bug, edge case 2; reproduced).
pub fn auto_belt_gate<W: InvWorld + ?Sized>(
    _inv: &Inventory,
    _w: &W,
    _t: &InvTables,
    _item: UnitId,
) -> bool {
    true
}

/// Place in a slot (`0x0063C4F0`, §3.7): beltable, 1 × 1, slot < 16 →
/// §2.2 on grid 1 at (slot, 0). No `numboxes` check (edge case 6).
pub fn place_in_belt_slot<W: InvWorld + ?Sized>(
    inv: &mut Inventory,
    w: &mut W,
    t: &InvTables,
    item: UnitId,
    slot: u8,
) -> bool {
    let Some(rec) = w.item(item).map(|d| d.record) else {
        return false;
    };
    if !beltable(t, rec) || !is_1x1(t, rec) || slot >= BELT_SLOTS {
        return false;
    }
    place_in_grid(
        inv,
        w,
        item,
        grid_id::BELT,
        i32::from(slot),
        0,
        (1, 1),
        BELT_GRID,
    )
}

/// Compaction after slot `s` is emptied (`0x0055EDC0`, §3.8): column
/// c = min(s & 3, 3); walking rows 0..3, each item found moves down to the
/// lowest free row of the column below it (§3.7), gets item flags 0x400
/// and 0x1, loses 0x4000, refreshes its owner and joins the update list.
/// Returns the moves (from slot, to slot) in order.
pub fn compact_belt<W: InvWorld + ?Sized>(
    inv: &mut Inventory,
    w: &mut W,
    t: &InvTables,
    s: u8,
) -> Vec<(u8, u8)> {
    let c = (s & 3).min(3);
    let mut moves = Vec::new();
    for row in 0..4u8 {
        let from = c + 4 * row;
        let Some(item) = inv.belt_item(from) else {
            continue;
        };
        let Some(to_row) = (0..row).find(|&r| inv.belt_item(c + 4 * r).is_none()) else {
            // TODO(spec: whether an item that does not move is still flagged and listed)
            continue;
        };
        let to = c + 4 * to_row;
        if !place_in_belt_slot(inv, w, t, item, to) {
            continue;
        }
        moves.push((from, to));
        let mut guid = None;
        if let Some(d) = w.item_mut(item) {
            d.flags |= iflag::F400 | iflag::CHANGED;
            d.flags &= !iflag::F4000;
            guid = Some(d.guid);
        }
        w.owner_refresh(inv.owner);
        if let Some(g) = guid {
            inv.push_update(g);
        }
    }
    moves
}
