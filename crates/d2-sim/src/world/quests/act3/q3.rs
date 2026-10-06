// Spec: specs/world/quests-act3.md §5 (A3Q3 Blade of the Old Religion, chain 17)
//! A3Q3: events 0, 2, 3, 4, 8, 9, 11, 13, 14, the status and active
//! functions, the decoy (operate 31, init 25), its timer, the boss, the
//! altar (init 39) and Ormus' map-AI hooks.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};

/// Chain 17's extra data (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the Gidbinn was dropped.
    pub gidbinn_dropped: bool,
    /// +0x01: Hratli started the quest (chat end pending).
    pub hratli_started: bool,
    /// +0x02: the boss was spawned.
    pub boss_spawned: bool,
    /// +0x03: the decoy was activated.
    pub decoy_active: bool,
    /// +0x04: the boss is being spawned.
    pub boss_spawning: bool,
    /// +0x05: the spawn timer exists.
    pub timer: bool,
    /// +0x06: the altar may activate.
    pub altar_ready: bool,
    /// +0x07: the Gidbinn was brought (chat end pending).
    pub brought: bool,
    /// +0x08 / +0x0C: the decoy's position.
    pub decoy_x: i32,
    pub decoy_y: i32,
    /// +0x10 / +0x14: the altar's position.
    pub altar_x: i32,
    pub altar_y: i32,
    /// +0x18: the decoy is initialised.
    pub decoy_known: bool,
    /// +0x1C: Gidbinns held in the game.
    pub held: i32,
    /// +0x20: the last holder left.
    pub holder_left: bool,
    /// +0x24: the boss's GUID.
    pub boss_guid: u32,
    /// +0x28: the altar's GUID.
    pub altar_guid: u32,
    /// +0x2C: the altar's mode.
    pub altar_mode: i32,
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

/// Decoy operate 31 `0x005B9B40` (returns 0).
pub fn decoy_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
) {
    let _ = (ctl, w, object, player);
}

/// Decoy init 25 `0x005B9AE0`.
pub fn decoy_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Timer `0x005B9A30`; returns 1 (remove).
pub(super) fn boss_timer<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, i: usize) -> bool {
    let _ = (ctl, w, i);
    true
}

/// Altar init 39 `0x005B9D40` (object 251).
pub fn altar_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// `0x005B9CA0` (Ormus' map AI): the altar position, only while +0x06.
pub fn altar_position(ctl: &QuestControl) -> Option<(i32, i32)> {
    let _ = ctl;
    None
}

/// `0x005B9CD0` (Ormus' map AI): activate the altar.
pub fn activate_altar<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W) {
    let _ = (ctl, w);
}
