// Spec: specs/world/quests-act3.md §3 (A3Q1 Lam Esen's Tome, chain 15)
//! A3Q1: events 0, 2, 3, 4, 5, 9, 10, 11, 13, 14, the status and active
//! functions and the tome object (operate 28, init 23).

use crate::units::UnitId;
use crate::world::quests::GuidList;
use crate::world::quests::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};

/// Chain 15's extra data (§3.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the reward is not yet handed out in this game (1 at init).
    pub reward_open: bool,
    /// +0x01: the tome was dropped.
    pub tome_dropped: bool,
    /// +0x02: the tome is active.
    pub tome_active: bool,
    /// +0x04: party scratch (§3.8).
    pub party_scratch: bool,
    /// +0x05: the last tome holder left.
    pub holder_left: bool,
    /// +0x08: tomes in the game.
    pub tomes: i32,
    /// +0x0C: the tome was brought to Alkor (chat end pending).
    pub brought: bool,
    /// +0x10: GUID of the player who brought it.
    pub brought_by: u32,
    /// +0x14: the tome object's GUID.
    pub tome_guid: u32,
    /// +0x18: the tome object's mode.
    pub tome_mode: i32,
    /// +0x1C: tome holders.
    pub holders: GuidList,
}

/// Dispatches the chain's callbacks; false = no body (unhandled).
pub(super) fn callback<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    args: EventArgs,
    list: Option<&mut TextList>,
) -> bool {
    let _ = (ctl, w, i, args, list);
    false
}

/// Active function ("wants to talk").
pub(super) fn active<W: QuestWorld>(
    ctl: &QuestControl,
    w: &mut W,
    i: usize,
    player: UnitId,
    npc_class: u16,
) -> bool {
    let _ = (ctl, w, i, player, npc_class);
    false
}

/// Status function (always reports).
pub(super) fn status<W: QuestWorld>(
    ctl: &QuestControl,
    w: &mut W,
    i: usize,
    player: UnitId,
    r: &QuestFlags,
) -> u8 {
    let _ = (ctl, w, i, player, r);
    0
}

/// Init 23 (`0x00544E30` → `0x005B7310`).
pub fn tome_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Operate 28 `0x005B7A60` (returns 0).
pub fn tome_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
) {
    let _ = (ctl, w, object, player);
}
