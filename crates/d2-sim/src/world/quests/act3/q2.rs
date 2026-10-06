// Spec: specs/world/quests-act3.md §4 (A3Q2 Khalim's Will, chain 16)
//! A3Q2: events 0, 2, 3, 4, 10, 11, 13, the status and active functions,
//! Khalim's chests (operate 57 / 59 / 58), the sewer lever and stairs and
//! the cube hook `0x005B86E0`.

use crate::units::UnitId;
use crate::world::quests::GuidList;
use crate::world::quests::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};

/// Chain 16's extra data (§4.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the sewer stairs are initialised.
    pub stairs_known: bool,
    /// +0x01: Cain started the quest (chat end pending).
    pub cain_started: bool,
    /// +0x04: the stairs' GUID.
    pub stairs_guid: u32,
    /// +0x08: the stairs' mode.
    pub stairs_mode: i32,
    /// +0x0C: drop count scratch.
    pub drop_count: i32,
    /// +0x10 / +0x14 / +0x18 / +0x1C: eyes, brains, hearts, flails
    /// dropped (live counts).
    pub eyes: i32,
    pub brains: i32,
    pub hearts: i32,
    pub flails: i32,
    /// +0x20: Wills cubed.
    pub wills: i32,
    /// +0x24 / +0x25 / +0x26 / +0x27: eye, brain, heart, flail dropped
    /// once.
    pub eye_dropped: bool,
    pub brain_dropped: bool,
    pub heart_dropped: bool,
    pub flail_dropped: bool,
    /// +0x2C: player list (reset only).
    pub players: GuidList,
}

/// Khalim's chests (§4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KhalimChest {
    /// Object 405, operate 57 `0x005B8860`, `qhr `.
    Heart,
    /// Object 406, operate 59 `0x005B8A20`, `qbr `.
    Brain,
    /// Object 407, operate 58 `0x005B8940`, `qey `.
    Eye,
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

/// Khalim's chest operate (§4.6; returns 0).
pub fn chest_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
    chest: KhalimChest,
) {
    let _ = (ctl, w, object, player, chest);
}

/// Stairs init 41 `0x005B8660`.
pub fn stairs_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Stairs operate 44 `0x005B84E0`.
pub fn stairs_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
) {
    let _ = (ctl, w, object, player);
}

/// Lever init 42 `0x005B86B0`.
pub fn lever_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Lever operate 45 `0x005B8530` (returns 0).
pub fn lever_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
) {
    let _ = (ctl, w, object, player);
}

/// Lever object event 7 `0x005B85E0` (class 367).
pub fn lever_event<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Cubing the Will `0x005B86E0` (`world/cube.md` §8).
pub fn will_cubed<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, player: UnitId) {
    let _ = (ctl, w, player);
}
