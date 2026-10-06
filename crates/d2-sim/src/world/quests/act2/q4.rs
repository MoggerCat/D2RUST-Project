// Spec: specs/world/quests-act2.md §6 (A2Q4 Arcane Sanctuary, chain 11, slot 12)
//! A2Q4 callback by callback. STUB: filled by the A2Q4 task.

use crate::units::{RoomId, UnitId};
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Extra data (§6.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x04 Drognan started the quest.
    pub drognan_started: bool,
    /// +0x08 the tome's room.
    pub tome_room: Option<RoomId>,
    /// +0x0E the palace is open.
    pub palace_open: bool,
    /// +0x11 the harem blocker exists.
    pub blocker_made: bool,
    /// +0x38 the blocker's GUID.
    pub blocker_guid: u32,
    /// u16 +0x40 the blocker mode.
    pub blocker_mode: u16,
    /// +0x46 the blocker was neutral when opened.
    pub blocker_was_neutral: bool,
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

/// `0x0059B710` (§6.8), object event 7 of class 0x7A: record 11 (no
/// record → nothing).
pub fn harem_blocker<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, _object: UnitId) {
    if ctl.find(11).is_some() {
        w.unhandled(11, 0x0059_B710);
    }
}

/// `0x0059B660` (§4.9): the Arcane hook after the staff is assembled.
pub(crate) fn arcane_hook<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W) {
    if ctl.find(11).is_some() {
        w.unhandled(11, 0x0059_B660);
    }
}
