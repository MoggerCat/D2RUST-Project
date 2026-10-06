// Spec: specs/world/quests-act2.md §7 (A2Q5 The Summoner, chain 12, slot 13)
//! A2Q5 callback by callback. STUB: filled by the A2Q5 task.

use crate::units::{RoomId, UnitId};
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Extra data (§7.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00 killed.
    pub killed: bool,
    /// +0x01 seen.
    pub seen: bool,
    /// +0x04 the kill room.
    pub kill_room: Option<RoomId>,
    /// +0x08 the timer exists.
    pub timer: bool,
    /// +0x09 the timer phase.
    pub phase: u8,
}

pub(super) fn callback<W: QuestWorld>(
    _ctl: &mut QuestControl,
    _w: &mut W,
    _i: usize,
    _args: EventArgs,
    _list: Option<&mut TextList>,
) -> bool {
    false
}

pub(super) fn active<W: QuestWorld>(
    _ctl: &QuestControl,
    _w: &mut W,
    _i: usize,
    _player: UnitId,
    _npc: u16,
) -> bool {
    false
}

/// Timer `0x0059BFD0` (§7.2); true = remove.
pub(super) fn timer<W: QuestWorld>(_ctl: &mut QuestControl, _w: &mut W, _i: usize) -> bool {
    true
}
