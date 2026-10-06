// Spec: specs/world/quests-act2.md §4 (A2Q2 The Horadric Staff, chain 9, slot 10)
//! A2Q2 callback by callback. STUB: filled by the A2Q2 task.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};

/// Extra data (§4.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x14 drop count.
    pub drops: i32,
    /// +0x18 Staff of Kings held in the game.
    pub staff_count: i32,
    /// +0x1C Horadric Staffs held in the game.
    pub hstaff_count: i32,
    /// +0x20 cubes held in the game.
    pub cube_count: i32,
    /// +0x24 amulets held in the game.
    pub amulet_count: i32,
    /// +0x28 Staff of Kings dropped.
    pub staff_dropped: bool,
    /// +0x29 cube dropped.
    pub cube_dropped: bool,
    /// +0x2A staff assembled.
    pub assembled: bool,
    /// +0x2B "missing" already reported.
    pub missing_reported: bool,
    /// +0x2C GUID of the assembling player (`None`: −1).
    pub assembler: Option<u32>,
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

/// Status function `0x0059E630` (§4.6).
pub(super) fn status<W: QuestWorld>(
    _ctl: &QuestControl,
    _w: &mut W,
    _i: usize,
    _player: UnitId,
    _pf: &QuestFlags,
) -> u8 {
    0
}
