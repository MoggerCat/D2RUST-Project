// Spec: specs/items/inventory.md §5
//! Shared checks: the item checks (§5.1), busy and trading (§5.2), the
//! targeting reset (§5.3) and the item-move gate (§5.4).

use super::{iflag, mode, InteractionTarget, InvWorld, Inventory};
use crate::units::UnitId;

/// Ground range of the "ground or owned" check (§5.1; Constants: 10 per
/// axis).
pub const GROUND_RANGE: i32 = 10;
/// `0x00567620`: allowed while player data +0x50 is below this (§5.4).
pub const GATE_LIMIT: u32 = 5;

/// A player's interaction, as §5 reads it.
pub type Interaction = InteractionTarget;

fn lookup<W: InvWorld + ?Sized>(w: &W, guid: u32) -> Option<(UnitId, u8, Option<UnitId>)> {
    let id = w.item_by_guid(guid)?;
    let d = w.item(id)?;
    Some((id, d.mode, d.inv))
}

/// Cursor item check (`0x005490E0`): 0 when the item exists, is in mode 4
/// and is the player's cursor item; else 1.
pub fn cursor_item_check<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, guid: u32) -> u8 {
    match lookup(w, guid) {
        Some((id, mode::CURSOR, _)) if inv.cursor() == Some(id) => 0,
        _ => 1,
    }
}

/// Stored item check (`0x00549150`): 0 when the item exists, is in mode 0
/// and is in the player's inventory; else 1.
pub fn stored_item_check<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, guid: u32) -> u8 {
    match lookup(w, guid) {
        Some((_, mode::STORED, Some(o))) if o == inv.owner => 0,
        _ => 1,
    }
}

/// Stored or equipped (`0x005491B0`): 1 only for an item in mode 0 / 1
/// that is not in the player's inventory; a missing item passes (edge
/// case 5).
pub fn stored_or_equipped_check<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, guid: u32) -> u8 {
    match lookup(w, guid) {
        Some((_, mode::STORED | mode::EQUIPPED, o)) if o != Some(inv.owner) => 1,
        _ => 0,
    }
}

/// Owned item (`0x00549220`): 0 when the item exists and is in the
/// player's inventory or is the cursor item; else 1.
pub fn owned_item_check<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, guid: u32) -> u8 {
    match lookup(w, guid) {
        Some((id, _, o)) if o == Some(inv.owner) || inv.cursor() == Some(id) => 0,
        _ => 1,
    }
}

/// Belt item (`0x005492F0`): 1 only for an item in mode 2 that is not in
/// the player's inventory; a missing item passes (edge case 5).
pub fn belt_item_check<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, guid: u32) -> u8 {
    match lookup(w, guid) {
        Some((_, mode::BELT, o)) if o != Some(inv.owner) => 1,
        _ => 0,
    }
}

/// Ground or owned (`0x00549350`): modes 0, 1, 2, 4 → the owned item
/// check; mode 3 → another act 2, out of range (10 subtiles per axis,
/// `0x00548EF0`) 1, else 0; missing or mode > 4 → 1.
pub fn ground_or_owned_check<W: InvWorld + ?Sized>(inv: &Inventory, w: &W, guid: u32) -> u8 {
    match lookup(w, guid) {
        None => 1,
        Some((id, mode::GROUND, _)) => {
            if !w.same_act(inv.owner, id) {
                2
            } else if !w.within_range(inv.owner, id, GROUND_RANGE) {
                1
            } else {
                0
            }
        }
        Some((_, m, _)) if m > mode::CURSOR => 1,
        Some(_) => owned_item_check(inv, w, guid),
    }
}

/// Busy (`0x00535060`, §5.2): an interaction, a cursor item, or player
/// data +0x4C ≠ 0.
pub fn busy<W: InvWorld + ?Sized>(inv: &Inventory, w: &W) -> bool {
    w.interaction(inv.owner) != InteractionTarget::None
        || inv.cursor().is_some()
        || w.player_data_4c(inv.owner) != 0
}

/// Trading (`0x005678A0`, §5.2): the interaction is with a player unit
/// that exists.
pub fn trading<W: InvWorld + ?Sized>(inv: &Inventory, w: &W) -> bool {
    matches!(
        w.interaction(inv.owner),
        InteractionTarget::Unit { ty: 0, .. }
    )
}

/// Targeting reset (`0x0055BF50`, §5.3): every item of the player's item
/// list with item flag 0x4 loses it; when `0x0044BE50` returns 0, S→C 0x3F
/// is queued for it. Items are visited in list order.
pub fn targeting_reset<W: InvWorld + ?Sized>(inv: &Inventory, w: &mut W) {
    for &item in inv.items() {
        let Some(d) = w.item_mut(item) else { continue };
        if d.flags & iflag::TARGETING == 0 {
            continue;
        }
        d.flags &= !iflag::TARGETING;
        let guid = d.guid;
        if w.targeting_probe(item) == 0 {
            w.queue_untarget(inv.owner, guid);
        }
    }
}

/// `0x00567620` without the player-trade part (§5.4).
fn gate_trade<W: InvWorld + ?Sized>(w: &W, player: UnitId) -> bool {
    w.player_trade_gate(player)
        .unwrap_or_else(|| w.player_data_50(player) < GATE_LIMIT)
}

/// Item-move gate (`0x00535610`, §5.4): whether the player may move items
/// now. Clears an interaction whose unit is missing.
pub fn item_move_gate<W: InvWorld + ?Sized>(inv: &Inventory, w: &mut W) -> bool {
    let player = inv.owner;
    match w.interaction(player) {
        InteractionTarget::None => w.player_data_4c(player) == 0 && gate_trade(w, player),
        InteractionTarget::Missing => {
            w.clear_interaction(player);
            false
        }
        InteractionTarget::Unit { ty: 1, unit } => !w.npc_talking(unit, player),
        InteractionTarget::Unit { .. } => gate_trade(w, player),
    }
}
