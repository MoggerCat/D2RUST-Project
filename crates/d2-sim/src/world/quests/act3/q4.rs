// Spec: specs/world/quests-act3.md §6 (A3Q4 The Golden Bird, chain 18)
//! A3Q4: events 0, 2, 3, 4, 8, 9, 10, 11, 13, 14, the status and active
//! functions, the boss choice and removal hooks, Alkor's map-AI hooks and
//! the Potion of Life.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};

/// Chain 18's extra data (§6.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the bird was brought to Alkor (Alkor's map AI).
    pub bird_brought: bool,
    /// +0x01: a boss may be chosen (1 at init).
    pub may_choose: bool,
    /// +0x02: a boss is chosen.
    pub chosen: bool,
    /// +0x04: its GUID.
    pub boss_guid: u32,
    /// +0x08 / +0x09 / +0x0A / +0x0B: chat end pending for Alkor, Cain's
    /// first talk, Cain's second talk, Meshif.
    pub pend_alkor: bool,
    pub pend_cain1: bool,
    pub pend_cain2: bool,
    pub pend_meshif: bool,
    /// +0x0C: the figurine is still to drop (1 at init).
    pub to_drop: bool,
    /// +0x10: figurines plus birds held in the game.
    pub held: i32,
    /// +0x14: the figurine was dropped.
    pub dropped: bool,
    /// +0x15: the last holder left.
    pub holder_left: bool,
    /// +0x18: the bit for the party iterate.
    pub party_bit: u8,
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

/// `0x00544E80` → `0x005BAC70` (§6.2): from special monster creation
/// for a monster in Act III; `flags_0d` is its monstats flags byte
/// +0x0D.
pub fn choose_bird_boss<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    unit: UnitId,
    class: u16,
    flags_0d: u8,
) {
    let _ = (ctl, w, unit, class, flags_0d);
}

/// `0x005BACF0` (§6.2): a monster linked to chain 18 is removed.
pub fn bird_boss_removed<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, unit: UnitId) {
    let _ = (ctl, w, unit);
}

/// `0x005BAD20` (Alkor's map AI): the +0x00 test.
pub fn alkor_bird_brought(ctl: &QuestControl) -> bool {
    let _ = ctl;
    false
}

/// `0x005BAD40` (Alkor's map AI): clear +0x00.
pub fn alkor_bird_clear(ctl: &mut QuestControl) {
    let _ = ctl;
}

/// Using `xyz ` (`0x0055E170`, §6.6): true when used (consume it).
pub fn potion_of_life<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, player: UnitId) -> bool {
    let _ = (ctl, w, player);
    false
}
