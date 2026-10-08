// Spec: specs/client/model.md (§6 rule 6, §13)
//! The play app's unit visibility predicate (`0x004DBF20`, `model.md`
//! §13) for the position check of S→C 0x95 / 0x96 (§6 rule 6). Without
//! one the bridge refuses every check whose point is off the unit's cell
//! on both axes (§13 rule 6), so the vitals sync's walk verify of the
//! local player was a rejected message in every play run.
//!
//! PROVISIONAL (client/model.md §13; REC-278): every unit is visible.
//! Rules 1–5 test the unit's COF box and current cel against the screen;
//! the checked unit is the local player, drawn at the screen centre, so
//! both of its points are on screen and the reading holds for it. Off
//! screen units would be corrected by the original (rule 7); settled by
//! the render side's predicate (the camera, COF and cel state of
//! `render/unit-composite.md` §4) checked against a 1.14d trace of 0x96
//! corrections.

use crate::bridge::world::ClientUnit;

/// `visible(U, a, b)`: true (module doc).
pub fn visible(_unit: &ClientUnit, _a: i32, _b: i32) -> bool {
    true
}
