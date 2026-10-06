// Spec: specs/world/quests-act2.md §3 (A2Q1 Radament's Lair, chain 8, slot 9)
//! A2Q1 callback by callback. STUB: filled by the A2Q1 task.

use crate::units::{RoomId, UnitId};
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Extra data (§3.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00 Radament killed.
    pub killed: bool,
    /// +0x04 the room of the kill.
    pub kill_room: Option<RoomId>,
    /// +0x08 Atma started the quest (chat end pending).
    pub atma_started: bool,
    /// +0x09 "first entry status sent".
    pub entry_sent: bool,
    /// +0x0A the status timer exists.
    pub timer: bool,
    /// +0x0C Book of Skill drop count.
    pub books: i32,
    /// +0x10 reward pending from an earlier game.
    pub reward_pending: bool,
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

/// Timer `0x00598F70` (§3.7); true = remove.
pub(super) fn timer<W: QuestWorld>(_ctl: &mut QuestControl, _w: &mut W, _i: usize) -> bool {
    true
}
