// Spec: specs/world/quests-act3.md §9.2 (A3Q7 Dark Wanderer, chain 28)
//! A3Q7: event 13, the wanderer object (init 43), its walk target and
//! the minion hook and timer.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Chain 28's extra data (§9.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the wanderer object was seen.
    pub seen: bool,
    /// +0x01: the wanderer is still to spawn (1 at init).
    pub to_spawn: bool,
    /// +0x02: the walk target is fixed; +0x04 / +0x08 the target.
    pub target_fixed: bool,
    pub target_x: i32,
    pub target_y: i32,
    /// +0x0C: minions spawned.
    pub minions: bool,
    /// +0x0D: the minion timer exists.
    pub timer: bool,
    /// +0x10: the wanderer's GUID.
    pub wanderer_guid: u32,
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

/// Init 43 `0x005BD1F0` (object 368).
pub fn wanderer_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// `0x005BD0D0` (the wanderer's AI): the walk target.
pub fn wanderer_target<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    wanderer: UnitId,
) -> Option<(i32, i32)> {
    let _ = (ctl, w, wanderer);
    None
}

/// `0x005BD4A0` (the wanderer's AI): the minion hook.
pub fn wanderer_minions<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, wanderer: UnitId) {
    let _ = (ctl, w, wanderer);
}

/// Timer `0x005BD390`; returns 1.
pub(super) fn minion_timer<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize) -> bool {
    let _ = (ctl, w, i);
    true
}
