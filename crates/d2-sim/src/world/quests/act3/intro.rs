// Spec: specs/world/quests-act3.md §9.3 (Act III intro, chain 39)
//! The Act III intro record: events 0, 11 and the active function.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

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

/// Active `0x005B6E30`.
pub(super) fn active<W: QuestWorld>(w: &mut W, player: UnitId, npc_class: u16) -> bool {
    let _ = (w, player, npc_class);
    false
}
