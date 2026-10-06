// Spec: specs/world/quests-act3.md §8 (A3Q6 The Guardian, chain 20)
//! A3Q6: events 0, 2, 3, 8, 10, 11, 13, the active function, the status
//! timer, the Hellgate (init 44), Mephisto's bridge (init 45, event 7),
//! Natalya (init 52) and the Durance warp `0x005BCFD0`.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Chain 20's extra data (§8.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the status timer exists.
    pub timer: bool,
    /// +0x01: the Hellgate is initialised; +0x04 its GUID.
    pub gate_known: bool,
    pub gate_guid: u32,
    /// +0x02: the bridge is initialised; +0x08 its GUID.
    pub bridge_known: bool,
    pub bridge_guid: u32,
    /// +0x03: Ormus started the quest (chat end pending).
    pub ormus_started: bool,
    /// +0x0C: the Hellgate's mode.
    pub gate_mode: i32,
    /// +0x10: the bridge's mode.
    pub bridge_mode: i32,
    /// +0x14: soulstones dropped.
    pub stones_dropped: i32,
    /// +0x18: a soulstone was dropped.
    pub stone_dropped: bool,
    /// +0x1C: soulstones to drop.
    pub stones_to_drop: i32,
    /// +0x20: Natalya was spawned; +0x2C her GUID.
    pub natalya_spawned: bool,
    pub natalya_guid: u32,
    /// +0x24: her map AI is stored (`0x005BD040`, no caller in 1.14d).
    pub map_ai: bool,
    /// +0x30: the map AI was applied.
    pub ai_applied: bool,
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

/// Timer `0x005BC720`; returns 1.
pub(super) fn status_timer<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize) -> bool {
    let _ = (ctl, w, i);
    true
}

/// Hellgate init 44 `0x005BCBF0` (object 342).
pub fn hellgate_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Bridge init 45 `0x005BCB90` (object 341).
pub fn bridge_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Bridge object event 7 `0x005BCAC0` (class 341).
pub fn bridge_event<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Natalya init 52 `0x005BCE80` (object 382).
pub fn natalya_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// The Durance warp's Act III part `0x005BCFD0` (`quests.md` §8.1).
pub fn durance_warp<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W) {
    let _ = (ctl, w);
}
