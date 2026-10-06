// Spec: specs/world/quests-act3.md §7 (A3Q5 The Blackened Temple, chain 19)
//! A3Q5: events 0, 2, 3, 8, 10, 11, 13, the active function, the council
//! registration, the Compelling Orb (init 60, operate 53), stairs R
//! (init 53) and the Durance warp check.

use crate::units::UnitId;
use crate::world::quests::{EventArgs, QuestControl, QuestWorld, TextList};

/// Chain 19's extra data (§7.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extra {
    /// +0x00: the last killed council member's GUID.
    pub last_council: u32,
    /// +0x04: Ormus started the quest (chat end pending).
    pub ormus_started: bool,
    /// +0x05: the council was seen.
    pub council_seen: bool,
    /// +0x08: the starting player had 17.0.
    pub had_lam: bool,
    /// +0x0C: the Compelling Orb is smashed.
    pub orb_smashed: bool,
    /// +0x0D: the flail was dropped.
    pub flail_dropped: bool,
    /// +0x0E: the cube was dropped.
    pub cube_dropped: bool,
    /// +0x10: the council GUIDs (up to 6).
    pub council: Vec<u32>,
    /// +0x28: the orb monster was spawned; +0x2C its GUID.
    pub orb_spawned: bool,
    pub orb_guid: u32,
    /// +0x30: council registered.
    pub registered: i32,
    /// +0x34: council left to kill.
    pub left: i32,
    /// +0x38: orb hits.
    pub hits: i32,
    /// +0x3C / +0x40: flails, cubes to drop.
    pub flails_to_drop: i32,
    pub cubes_to_drop: i32,
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

/// `0x00545B50` → `0x005BB550` (§7.5): a council member placed by the
/// preset path.
pub fn council_preset<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, unit: UnitId) {
    let _ = (ctl, w, unit);
}

/// Orb init 60 `0x005BBBA0`.
pub fn orb_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// Orb operate 53 `0x005BB980` (returns 0).
pub fn orb_operate<W: QuestWorld>(
    ctl: &mut QuestControl,
    w: &mut W,
    object: UnitId,
    player: UnitId,
) {
    let _ = (ctl, w, object, player);
}

/// Stairs R init 53 `0x005BBB70` (object 386).
pub fn stairs_r_init<W: QuestWorld>(ctl: &mut QuestControl, w: &mut W, object: UnitId) {
    let _ = (ctl, w, object);
}

/// The Durance warp check `0x005BBFA0` (`quests.md` §8.2): open?
pub fn durance_open(ctl: &QuestControl, from: u32) -> bool {
    let _ = (ctl, from);
    false
}
