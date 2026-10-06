// Spec: specs/world/hirelings.md §4, §7, §10 r5, §10 r6, §13 r4, §13 r5, §13 r6
//! Level stats (§4), experience and level-up (§7), the restore level
//! (§10 rules 5–6) and the stat / experience / speech messages (§13
//! rules 4–6).

use super::{HirelingError, HirelingState, HirelingTables, HirelingWorld};
use crate::units::UnitId;

/// §4 (`0x00572840(game, player, merc, level)`): `merc` `None` = the
/// player's living hireling; `level` 0 = merc level + 1.
pub fn apply_level<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &HirelingState,
    player: UnitId,
    merc: Option<UnitId>,
    level: i32,
) {
    let _ = (w, t, st, player, merc, level);
    todo!("hirelings §4")
}

/// §13 rule 4 (`0x005726C0(game, player, flag)`): the living hireling's
/// stats to the player's client.
pub fn send_stats<W: HirelingWorld>(
    w: &mut W,
    st: &HirelingState,
    player: UnitId,
) -> Result<(), HirelingError> {
    let _ = (w, st, player);
    todo!("hirelings §13 rule 4")
}
