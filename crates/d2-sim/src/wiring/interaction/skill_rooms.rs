// Spec: specs/skills/bodies-2b.md §6.5, specs/skills/bodies.md §6.12, specs/sim/path-placement.md §7, §10
//! The skill bodies' room seams on the DRLG rooms: the level's `Teleport`
//! flag, the box collision, the free point and the placement of a unit.
//! Once the host has turned the path provider on
//! ([`crate::wiring::action::ActionHooks::enable_paths`]) they are
//! answered here, as [`UseView::line_blocked`] already is; without it
//! they stay [`Pending`]'s (tests, hosts without rooms).

use super::skill_use::UseView;
use crate::path::search::free_point;
use crate::path::Point;
use crate::units::{RoomId, UnitId};
use crate::wiring::action::Pending;
use crate::wiring::path::place::{place_unit, Rooms};
use crate::wiring::path::PathCtx;

use super::UseRest;

impl<X: Pending + UseRest> UseView<'_, X> {
    fn on_rooms(&self) -> bool {
        self.cv.v.h.paths.is_some()
    }

    /// The `Teleport` column of the room's level (`levels.txt`); `None`
    /// without the provider, a level or a row.
    pub(super) fn level_teleport(&self, r: RoomId) -> Option<i32> {
        if !self.on_rooms() {
            return None;
        }
        let level = self.cv.v.h.drlg.level_id(&*self.cv.game, r)?;
        let row = self
            .cv
            .v
            .h
            .tables
            .levels
            .get(usize::try_from(level).ok()?)?;
        Some(i32::from(row.teleport))
    }

    /// `0x0064D800(room, x, y, size, mask)` ≠ 0 on the rooms' grids.
    pub(super) fn rooms_box_collides(
        &self,
        r: RoomId,
        at: (i32, i32),
        size: i32,
        mask: u32,
    ) -> Option<bool> {
        use crate::path::place_seams::CollisionView;
        if !self.on_rooms() {
            return None;
        }
        let rooms = Rooms(&self.cv.v.h.drlg);
        // `path-placement.md` §4 rules 3–4: a Size ≤ 1 is a point query.
        let v = if size <= 1 {
            rooms.point_query(r, at.0, at.1, mask)
        } else {
            let s = size as u32;
            rooms.box_query(r, at.0, at.1, s, s, mask)
        };
        Some(v != 0)
    }

    /// `0x0064D910(room, x, y, pattern, mask)` ≠ 0 on the rooms' grids,
    /// with the unit's own collision pattern (`path-placement.md` §4
    /// rule 5; the unit's path record, pattern 0 without one).
    // d2rs-own, unverified: that the leap's check reads the unit's path
    // pattern is read from the call shape, not confirmed on 1.14d.
    pub(super) fn rooms_pattern_collides(
        &self,
        r: RoomId,
        at: (i32, i32),
        u: UnitId,
        mask: u32,
    ) -> Option<bool> {
        if !self.on_rooms() {
            return None;
        }
        let pattern = self
            .cv
            .v
            .h
            .paths
            .as_ref()
            .and_then(|p| p.dynamic(u))
            .map_or(0, |d| d.pattern);
        Some(crate::path::collision::pattern_collides(
            &self.cv.v.h.drlg,
            Some(r),
            at.0,
            at.1,
            pattern,
            mask as u16,
        ))
    }

    /// `0x0064E7B0(room, &pt, size, mask, fallback)` on the rooms.
    pub(super) fn rooms_free_point(
        &self,
        r: RoomId,
        at: (i32, i32),
        size: i32,
        mask: u32,
        fallback: bool,
    ) -> Option<Option<(RoomId, (i32, i32))>> {
        if !self.on_rooms() {
            return None;
        }
        let mut p = Point::new(at.0, at.1);
        let found = free_point(
            &Rooms(&self.cv.v.h.drlg),
            Some(r),
            &mut p,
            size,
            mask,
            fallback,
        );
        Some(found.ok().flatten().map(|room| (room, (p.x, p.y))))
    }

    /// `0x00554EA0(game, unit, room, x, y, 0, 0)` (`path-placement.md`
    /// §10) through the path provider.
    pub(super) fn rooms_place_unit(
        &mut self,
        u: UnitId,
        r: Option<RoomId>,
        at: (i32, i32),
    ) -> Option<bool> {
        if !self.on_rooms() {
            return None;
        }
        let c = PathCtx::of(&mut self.cv.v, &mut *self.cv.game);
        Some(place_unit(c, u, r, at.0, at.1, false, false))
    }
}

impl<X: Pending + UseRest> UseView<'_, X> {
    /// The path's target point (`bodies-2b.md` §6.2 step 3 "the path's
    /// target point without T").
    // PROVISIONAL (skills/bodies-2b.md §6.2 step 3): a cast at a point
    // does not walk, so the path record holds no target; the point kept
    // at the mode start (`UseRest::target_position`) stands in when
    // [`Pending::path_target_point`] has none (0, 0). Settled by REC-154.
    pub(super) fn cast_target_point(&self, u: UnitId) -> (i32, i32) {
        match self.x().path_target_point(u) {
            (0, 0) => self.x().target_position(u).unwrap_or((0, 0)),
            p => p,
        }
    }
}
