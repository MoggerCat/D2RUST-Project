// Spec: specs/world/quests-act2.md §8 (A2Q6 The Seven Tombs, chain 13, slot 14)
//! A2Q6 callback by callback. STUB: filled by the A2Q6 task.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};

/// Extra data (§8.2). The staff tomb level (+0x34) is the shared
/// `act1::Extra::tomb_level`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x0B the lair entrance is open.
    pub lair_open: bool,
    /// +0x0E the staff items were removed.
    pub staff_removed: bool,
    /// +0x10 the staff is missing.
    pub missing: bool,
    /// +0x14 the missing status (8 or 9, §4.10).
    pub missing_status: u8,
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

/// Status function `0x0059CA50` (§8.5).
pub(super) fn status<W: QuestWorld>(
    _ctl: &QuestControl,
    _w: &mut W,
    _i: usize,
    _player: UnitId,
    _pf: &QuestFlags,
) -> u8 {
    0
}

/// Timer `0x0059CEE0` (§8.11); true = remove.
pub(super) fn duriel_timer<W: QuestWorld>(_ctl: &mut QuestControl, _w: &mut W, _i: usize) -> bool {
    true
}

/// Timer `0x0059D870` (§8.8); true = remove.
pub(super) fn lair_timer<W: QuestWorld>(_ctl: &mut QuestControl, _w: &mut W, _i: usize) -> bool {
    true
}
