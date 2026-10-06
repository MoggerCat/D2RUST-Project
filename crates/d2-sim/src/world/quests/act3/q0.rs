// Spec: specs/world/quests-act3.md §9.1 (A3Q0 Hratli gossip, chain 14)
//! A3Q0: events 0, 11, 13, the active function and Hratli's start / end
//! dummies (init 49, 50).

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Chain 14's extra data (§9.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the start Hratli was spawned.
    pub start_spawned: bool,
    /// +0x01: the end Hratli was spawned.
    pub end_spawned: bool,
    /// +0x02: the end object was seen.
    pub end_seen: bool,
    /// +0x03: the start Hratli is present.
    pub start_present: bool,
    /// +0x04 / +0x08: the end position.
    pub end_x: i32,
    pub end_y: i32,
    /// +0x0C: Hratli's GUID.
    pub hratli_guid: u32,
    /// +0x10: the map AI was applied.
    pub ai_applied: bool,
    /// +0x18: a map AI is stored (`0x005B7230`, no caller in 1.14d).
    pub map_ai: bool,
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

/// Active `0x005B6FA0`.
pub(super) fn active<W: QuestWorld>(w: &mut W, player: UnitId, npc_class: u16) -> bool {
    let _ = (w, player, npc_class);
    false
}

/// Init 49 `0x005B70B0` (start dummy 378).
pub fn hratli_start_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Init 50 `0x005B7160` (end dummy 379).
pub fn hratli_end_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}
