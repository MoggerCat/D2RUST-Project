// Spec: specs/world/quests-act5-2.md §9
//! The Act V intro record (chain 40, slot 42). Not yet implemented: every callback is reported through
//! `QuestWorld::unhandled` (the address in `quests.tsv`).

use super::super::{EventArgs, QuestControl, QuestFlags, QuestWorld, TextList};
use crate::units::UnitId;

/// Quest extra data (record +0x18).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {}

/// Timers this quest makes (`quests.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {}

/// Init beyond `quests.tsv` (§2 table of the act spec).
pub fn init(r: &mut super::super::QuestRecord) {
    let _ = r;
}

/// One callback of record `i`; false = not handled (reported).
pub fn callback<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    i: usize,
    args: EventArgs,
    list: Option<&mut TextList>,
) -> bool {
    let _ = (ctl, w, i, args, list);
    false
}

/// The active function; `f` is its address.
pub fn active<W: QuestWorld>(
    ctl: &QuestControl,
    w: &mut W,
    i: usize,
    player: UnitId,
    npc_class: u16,
    f: u32,
) -> bool {
    let _ = (i, player, npc_class);
    w.unhandled(ctl.records[i].chain, f);
    false
}

/// The status function; `f` is its address.
pub fn status<W: QuestWorld>(
    ctl: &QuestControl,
    w: &mut W,
    i: usize,
    player: UnitId,
    pf: &QuestFlags,
    f: u32,
) -> Option<u8> {
    let _ = (player, pf);
    w.unhandled(ctl.records[i].chain, f);
    None
}

/// Runs a timer; true = remove it.
pub fn run_timer<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, t: Timer, chain: u8) -> bool {
    let _ = (ctl, w, chain);
    match t {}
}
