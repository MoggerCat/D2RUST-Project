// Spec: specs/skills/bodies-2.md (Charge, §5.4), specs/skills/bodies-3.md (Blessed Hammer, §6.9), specs/sim/pathing.md (§2, §3, §8.1)
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

/// A skill whose mode is the run (3) used at a point (Charge): the
/// path's target point `0x00648AD0` (`pathing.md` §8.1), so the start's
/// compute has a goal. Other modes and unit targets are the mode
/// start's.
// d2rs-own, unverified: where a skill mode's point target reaches the
// path is not specified (`use.md` §4); the run is the only mode taken.
pub fn run_to_point<X: Pending>(
    v: &mut View<'_, X>,
    unit: UnitId,
    mode: u32,
    target: crate::skills::use_::ModeTarget<UnitId>,
) {
    use crate::skills::use_::ModeTarget;
    if let (3, ModeTarget::Point(x, y)) = (mode, target) {
        v.h.path_set_target_point(unit, x, y);
    }
}
