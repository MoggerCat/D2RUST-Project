// Spec: specs/world/quests-act2.md §5 (A2Q3 Tainted Sun, chain 10, slot 11)
//! A2Q3 callback by callback. STUB: filled by the A2Q3 task.

use crate::units::{RoomId, UnitId};
use crate::world::quests::{EventArgs, GuidList, QuestControl, QuestWorld, TextList};

/// Extra data (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x01 the darken timer exists.
    pub darken_timer: bool,
    /// +0x02 darkness applied.
    pub dark: bool,
    /// +0x03 darkness pending (Act II not loaded).
    pub dark_pending: bool,
    /// +0x04 the altar is destroyed.
    pub altar_destroyed: bool,
    /// +0x05 the altar was seen.
    pub altar_seen: bool,
    /// +0x06 the status timer.
    pub status_timer: bool,
    /// +0x08 the altar mode (0 = neutral).
    pub altar_mode: i32,
    /// +0x0C the altar's GUID.
    pub altar_guid: u32,
    /// +0x10 the altar's room.
    pub altar_room: Option<RoomId>,
    /// +0x14 player list (only event 10 touches it).
    pub list: GuidList,
    /// +0x98 the altar's level.
    pub altar_level: u32,
    /// +0x9C amulet drop count.
    pub amulets: i32,
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

/// Timer `0x0059ED80` (§5.3); true = remove.
pub(super) fn darken_timer<W: QuestWorld>(_ctl: &mut QuestControl, _w: &mut W, _i: usize) -> bool {
    true
}

/// Timer `0x0059A700` (§5.7); true = remove.
pub(super) fn altar_timer<W: QuestWorld>(_ctl: &mut QuestControl, _w: &mut W, _i: usize) -> bool {
    true
}
