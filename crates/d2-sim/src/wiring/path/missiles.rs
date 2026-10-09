// Spec: specs/sim/path-placement.md §2.3, §6 r4; specs/skills/bodies-3.md §3.8; specs/skills/bodies-4.md §2.4; specs/sim/pathing.md §2, §11, §13.1, §13.2, §13.3; specs/skills/bodies.md §2.4; specs/skills/bodies-2.md §2.3; specs/missiles/bodies.md §19; specs/missiles/bodies-2.md §46 (missile-body path seams on the path provider)
//! The path seams of the missile server-do / server-hit bodies
//! ([`crate::missiles::MissileBodies`]) answered by the path provider:
//! the target point (`0x00648A00` / `0x00648A10`), the target position
//! `0x0056D2C0`, set type `0x00648CF0`, the step counts `0x00648E70`
//! and the teleport `0x00650BE0`. `None` from each helper: the provider
//! is off, the caller keeps its default answer.

use crate::game::Game;
use crate::path::record::PATH_POINTS;
use crate::path::walk::seams::PathWorld;
use crate::path::DynamicPath;
use crate::units::{RoomId, UnitId, UnitType};
use crate::wiring::action::{Pending, View, WiringError};

use super::PathCtx;

/// The step-count cap of `0x00648E70` (`skills/bodies-2.md` §2.3: 77).
pub const STEP_COUNT_CAP: i32 = PATH_POINTS as i32 - 1;

/// Step counts `0x00648E70(path, n)` (`pathing.md` §13.1 rule 1): only
/// the low byte of n, unsigned, capped at 77. So −1 → 255 → 77, −256 →
/// 0, −200 → 56, 300 → 44.
pub fn step_count_byte(n: i32) -> u8 {
    (n as u8).min(STEP_COUNT_CAP as u8)
}

/// `0x00648E70` on a path: the distance budget (+0x90) and the max path
/// distance (+0x91) := [`step_count_byte`].
pub fn set_step_counts(d: &mut DynamicPath, n: i32) {
    let b = step_count_byte(n);
    d.dist_budget = b;
    d.max_distance = b;
}

impl<X: Pending> View<'_, X> {
    /// The path target point (path +0x10, +0x12).
    pub(crate) fn path_target_xy(&self, unit: UnitId) -> Option<(i32, i32)> {
        let p = self.h.paths.as_ref()?;
        Some(
            p.dynamic(unit)
                .map_or((0, 0), |d| (i32::from(d.target_x), i32::from(d.target_y))),
        )
    }

    /// `0x00553540(game, unit)` (`pathing.md` §13.2 rule 1): the target
    /// check `0x00553490` (a target unit whose stored type and GUID no
    /// longer resolve to the stored unit, or an item in mode 1 or 2, is
    /// cleared to none), then the path's target unit; the unit itself
    /// counts as none.
    pub(crate) fn path_target_checked(&mut self, game: &Game, unit: UnitId) -> Option<UnitId> {
        let d = self.h.paths.as_ref()?.dynamic(unit)?;
        let t = d.target_unit?;
        let stale = match game.lists.find_unit(t.ty, t.guid) {
            Some(f) if f == t.unit => self
                .units
                .get(f)
                .is_some_and(|r| r.ty == UnitType::Item && matches!(r.mode, 1 | 2)),
            _ => true,
        };
        if stale {
            if let Some(d) = self.h.paths.as_mut().and_then(|p| p.dynamic_mut(unit)) {
                d.target_unit = None;
            }
            return None;
        }
        (t.unit != unit).then_some(t.unit)
    }

    /// `0x0056D2C0` (`pathing.md` §13.2 rules 2–3): T := the checked path
    /// target ([`View::path_target_checked`], which clears a stale one);
    /// T present → its position, else the path's target point (+0x10,
    /// +0x12). `Some(None)` when either coordinate is 0.
    pub(crate) fn path_target_position(
        &mut self,
        game: &Game,
        unit: UnitId,
    ) -> Option<Option<(i32, i32)>> {
        self.h.paths.as_ref()?;
        let t = self.path_target_checked(game, unit);
        let Some(d) = self.h.paths.as_ref()?.dynamic(unit) else {
            return Some(None);
        };
        let (x, y) = match t {
            Some(t) => self.h.path_position(t),
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

    /// Step counts `0x00648E70` ([`set_step_counts`], `pathing.md` §13.1
    /// rule 1).
    pub(crate) fn path_set_step_counts(&mut self, unit: UnitId, n: i32) -> Option<()> {
        let p = self.h.paths.as_mut()?;
        if let Some(d) = p.dynamic_mut(unit) {
            set_step_counts(d, n);
        }
        Some(())
    }

    /// `0x00621DC0(unit, x, y)` → `0x0064FDC0` (`skills/bodies-3.md`
    /// §3.8): the direction vector's direction (`pathing.md` §8.3 rules
    /// 1–3) from the unit's position to (x, y).
    ///
    /// TODO(spec: skills/bodies-3.md §3.8): the coordinates `0x0064FDC0`
    /// feeds §8.3 (sub-tile position or the path's 16.16 one, and the
    /// target's fraction) are not stated; sub-tiles are used (equal to
    /// 16.16 whenever both fractions match). §8.3 rule 4 (`0x0064FED5`)
    /// is the §8.4 caller's and is not applied.
    pub(crate) fn path_dir64(&self, unit: UnitId, at: (i32, i32)) -> Option<i32> {
        let p = self.h.paths.as_ref()?;
        let (x, y) = self.h.path_position(unit);
        let (_, d) = crate::path::walk::geom::direction_vector(
            &p.tables,
            (x as u32, y as u32),
            (at.0 as u32, at.1 as u32),
        );
        Some(i32::from(d))
    }

    /// `0x006488A0(path, d)` (`sim/pathing.md` §8.5): the snap, direction
    /// := new direction := d & 63. `None` without the path provider or
    /// a dynamic record.
    pub(crate) fn path_snap_direction(&mut self, unit: UnitId, d: i32) -> Option<()> {
        let p = self.h.paths.as_mut()?;
        let r = p.dynamic_mut(unit)?;
        r.direction = (d & 63) as u8;
        r.new_direction = r.direction;
        Some(())
    }

    /// Path point i := (x, y) (+0x9C array, `skills/bodies-4.md` §2.4);
    /// an index outside the array is not written.
    pub(crate) fn path_set_point(&mut self, unit: UnitId, i: i32, at: (u16, u16)) -> Option<()> {
        let p = self.h.paths.as_mut()?;
        if let Some(d) = p.dynamic_mut(unit) {
            if let Some(pt) = usize::try_from(i).ok().and_then(|i| d.points.get_mut(i)) {
                pt.x = at.0;
                pt.y = at.1;
            }
        }
        Some(())
    }

    /// Point count (+0x28) := n (`0x00648790`).
    pub(crate) fn path_set_point_count(&mut self, unit: UnitId, n: i32) -> Option<()> {
        let p = self.h.paths.as_mut()?;
        if let Some(d) = p.dynamic_mut(unit) {
            d.point_count = n as u32;
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
