// Spec: specs/skills/bodies-2.md (Charge, §5.4), specs/skills/bodies-2b.md (Blessed Hammer, §6.9), specs/sim/pathing.md (§2, §3, §8.1, §10, §13.2), specs/skills/use.md (§4)
//! The path operations the skill bodies make on a unit's path
//! ([`PathOp`]), on the path provider of the action wiring: velocity,
//! target point and unit, path type, step counts, the two masks and the
//! compute toward the target. Charge and Blessed Hammer use these; the
//! other operations (`Clear14`, `Face`, `Reset`, ...) stay with the
//! [`Pending`] seam's default.

use crate::skills::use_::bodies::PathOp;
use crate::units::UnitId;
use crate::wiring::action::{Pending, View};
use crate::wiring::path::walk::build_path;

/// Runs `op` on `unit`'s path; `None` for an operation this module does
/// not carry (the caller falls back to the seam).
pub fn path_op<X: Pending>(
    v: &mut View<'_, X>,
    game: &mut crate::game::Game,
    unit: UnitId,
    op: PathOp<UnitId>,
) -> Option<i32> {
    match op {
        PathOp::Velocity(n) => v.h.path_set_velocity(unit, n),
        PathOp::TargetPoint(x, y) => v.h.path_set_target_point(unit, x, y),
        PathOp::TargetUnit(Some(t)) => v.path_set_target_unit(unit, t),
        PathOp::Type(t) => {
            v.path_set_type(unit, i32::from(t))?;
        }
        PathOp::Steps(n) => {
            v.path_set_step_counts(unit, n)?;
        }
        PathOp::MoveMask(m) => v.h.path_set_move_mask(unit, m as u16),
        PathOp::FootprintMask(m) => v.path_set_foot_mask(unit, m as u16),
        PathOp::Compute => {
            build_path(v, game, unit);
            return Some(1);
        }
        _ => return None,
    }
    Some(0)
}

/// A skill mode started at a point: the path's target point `0x00648AD0`
/// (target unit := none), the point every skill reads back through the
/// target position `0x0056D2C0` (`pathing.md` §13.2: the path's target
/// point when the target unit is none) and the skill message 0x4D sends
/// (`pathing.md` §10 rule 2: target x, y = the path target). Charge's run
/// (mode 3) computes its path to it. Unit targets are the mode start's.
// PROVISIONAL (use.md §4, REC-440): the point-form start `0x0057FE90` is
// read as writing the point with `0x00648AD0`, since `0x0056D2C0` (Leap,
// Whirlwind, the `lineofsight` test) and the 0x4D builder read only the
// path's +0x10 / +0x12; settled by the 0x4D bytes another client gets
// for a Leap at a point (R-SKPT-1).
pub fn point_target<X: Pending>(
    v: &mut View<'_, X>,
    unit: UnitId,
    target: crate::skills::use_::ModeTarget<UnitId>,
) {
    use crate::skills::use_::ModeTarget;
    if let ModeTarget::Point(x, y) = target {
        v.h.path_set_target_point(unit, x, y);
    }
}
