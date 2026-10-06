// Spec: specs/skills/functions.tsv (notes), specs/skills/use.md §8
//! The per-skill start / do bodies the spec states completely. The start
//! core (§5.3 step 6.6) and the do core (§5.4 step 6) run a slot from here
//! when it has a body, and through [`super::SkillFunctions`] otherwise
//! (every other filled slot: `functions.tsv` status `mapped`, body not
//! specified, Open question 10).
//!
//! Status: implemented, unverified (no recording of these skills).

/// `srvst` slots with a body here.
pub const START_BODIES: &[u16] = &[18];
/// `srvdo` slots with a body here (none is specified yet).
pub const DO_BODIES: &[u16] = &[];

/// `srvst[index](game, unit, skill, level)` when the slot has a body
/// here; `None` sends the call to the seam.
pub fn start(index: u16) -> Option<i32> {
    match index {
        // SrvSt18 Attract `0x005C3260`: `mov eax, 1; ret 8`, arguments
        // unread, nothing changed.
        18 => Some(1),
        _ => None,
    }
}

/// `srvdo[index](…)` when the slot has a body here (none yet).
pub fn do_(index: u16) -> Option<i32> {
    let _ = index;
    None
}
