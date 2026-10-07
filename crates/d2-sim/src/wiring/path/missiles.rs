// Spec: specs/sim/path-placement.md §2.3, §6 r4; specs/sim/pathing.md §2, §11; specs/skills/bodies.md §2.4; specs/skills/bodies-2.md §2.3; specs/missiles/bodies.md §19; specs/missiles/bodies-2.md §46 (missile-body path seams on the path provider)
//! The path seams of the missile server-do / server-hit bodies
//! ([`crate::missiles::MissileBodies`]) answered by the path provider:
//! the target point (`0x00648A00` / `0x00648A10`), the target position
//! `0x0056D2C0`, set type `0x00648CF0`, the step counts `0x00648E70`
//! and the teleport `0x00650BE0`. `None` from each helper: the provider
//! is off, the caller keeps its default answer.

use crate::game::Game;
use crate::path::record::PATH_POINTS;
use crate::path::walk::seams::PathWorld;
use crate::units::{RoomId, UnitId};
use crate::wiring::action::{Pending, View, WiringError};

use super::PathCtx;

/// The step-count cap of `0x00648E70` (`skills/bodies-2.md` §2.3: 77).
pub const STEP_COUNT_CAP: i32 = PATH_POINTS as i32 - 1;

impl<X: Pending> View<'_, X> {
    /// The path target point (path +0x10, +0x12).
    pub(crate) fn path_target_xy(&self, unit: UnitId) -> Option<(i32, i32)> {
        let p = self.h.paths.as_ref()?;
        Some(
            p.dynamic(unit)
                .map_or((0, 0), |d| (i32::from(d.target_x), i32::from(d.target_y))),
        )
    }

    /// `0x0056D2C0` (`skills/bodies.md` §2.4): the path's target unit's
    /// position, else the path target point; `Some(None)` when either
    /// coordinate is 0.
    ///
    /// TODO(spec: skills/bodies.md §2.4): a stale target unit (GUID no
    /// longer found) is not described; the stored unit's position is read
    /// only while it resolves (as `path_target`), else the point.
    pub(crate) fn path_target_position(
        &self,
        game: &Game,
        unit: UnitId,
    ) -> Option<Option<(i32, i32)>> {
        let p = self.h.paths.as_ref()?;
        let Some(d) = p.dynamic(unit) else {
            return Some(None);
        };
        let live = d
            .target_unit
            .filter(|t| game.lists.find_unit(t.ty, t.guid) == Some(t.unit));
        let (x, y) = match live {
            Some(t) => self.h.path_position(t.unit),
            None => (i32::from(d.target_x), i32::from(d.target_y)),
        };
        Some((x != 0 && y != 0).then_some((x, y)))
    }

    /// Set type `0x00648CF0` (`pathing.md` §2) on a missile's path.
    pub(crate) fn path_set_type(&mut self, unit: UnitId, ty: i32) -> Option<()> {
        let p = self.h.paths.as_mut()?;
        let tables = p.tables.clone();
        if let Some(d) = p.dynamic_mut(unit) {
            if let Err(e) = d.set_path_type(&tables, false, ty as u32) {
                self.h.errors.push(WiringError::Path(e));
            }
        }
        Some(())
    }

    /// Step counts `0x00648E70`: distance budget (+0x90) and max path
    /// distance (+0x91) := n, capped at 77 (`skills/bodies-2.md` §2.3).
    ///
    /// TODO(spec: skills/bodies-2.md §2.3): a negative n is not described
    /// (callers pass frame counts); it is stored as its low byte.
    pub(crate) fn path_set_step_counts(&mut self, unit: UnitId, n: i32) -> Option<()> {
        let p = self.h.paths.as_mut()?;
        if let Some(d) = p.dynamic_mut(unit) {
            let v = n.min(STEP_COUNT_CAP) as u8;
            d.dist_budget = v;
            d.max_distance = v;
        }
        Some(())
    }

    /// `0x00650BE0(path, unit, room, x, y)` (`path-placement.md` §6 rule
    /// 4): the teleport, then point count := 0.
    pub(crate) fn path_teleport_to(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        room: Option<RoomId>,
        x: i32,
        y: i32,
    ) -> Option<()> {
        self.h.paths.as_ref()?;
        let mut c = PathCtx::of(self, game);
        c.teleport(unit, room, x, y);
        if let Some(mut d) = c.load_path(unit) {
            d.point_count = 0;
            c.store_path(unit, &d);
        }
        Some(())
    }
}
