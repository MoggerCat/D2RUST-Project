// Spec: specs/sim/pathing.md §9 (per-tick movement)
//! Per-tick movement: the player event-0 step, the step, movement, arrival
//! check, one step, cell walk, footprint move, set position, room recache,
//! reset, room-change messages, run drain and re-path. 16.16 fixed point
//! on u32 positions, as the original (wrapping where it wraps).

use super::find::{compute, path_type, set_type};
use super::geom::{centre, unit_distance};
use super::request::{neutral_start, start_movement};
use super::seams::{flag, PathWorld, Point, WalkError, WalkPath, WalkUnits, MAX_POINTS};
use super::tables::PathTables;
use super::velocity::aim;
use crate::game::Game;
use crate::units::{RoomId, UnitId, UnitType};

/// Step base, 100 % (`0x00554CA0`).
pub const STEP_BASE: i32 = 0x400;
/// Stamina stat (8.8 fixed point).
pub const STAT_STAMINA: u16 = 10;
/// `item_staminadrainpct`.
pub const STAT_STAMINADRAIN: u16 = 154;
/// Body location of the torso item.
pub const BODYLOC_TORSO: u8 = 3;
/// Bound on the cell walk's loop: no rule; a step of at most 0x10000 per
/// iteration reaches any reachable cell far sooner. Exceeding it is
/// reported as fatal instead of hanging.
const CELL_WALK_GUARD: u32 = 1 << 20;

/// Result of the step (§9.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// 1: still moving.
    Moving = 1,
    /// 2: stopped.
    Stopped = 2,
}

/// The walk context: tables and both seam sides.
pub struct Walk<'a, W: ?Sized, U: ?Sized> {
    pub t: &'a PathTables,
    pub w: &'a mut W,
    pub u: &'a mut U,
}

fn cell_of(p: u32) -> i32 {
    (p >> 16) as i32
}

fn centre_of(path: &WalkPath) -> (u32, u32) {
    let c = path.cell();
    (centre(c.x), centre(c.y))
}

impl<W: PathWorld + ?Sized, U: WalkUnits + ?Sized> Walk<'_, W, U> {
    /// Player event 0 (`0x00580C20`, §9.2). Returns the step result.
    pub fn player_event0(&mut self, game: &mut Game, unit: UnitId) -> Result<Step, WalkError> {
        let Some(mut path) = self.w.load_path(unit) else {
            return Ok(Step::Stopped);
        };
        let r = self.player_event0_on(game, unit, &mut path);
        self.w.store_path(unit, &path);
        let s = r?;
        if s == Step::Stopped {
            // Step 6: no 1.14d server code stores a non-zero queued action
            // (player data +0x150), so a stop is a neutral start.
            neutral_start(self.w, self.u, game, unit, &path);
        }
        Ok(s)
    }

    fn player_event0_on(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        path: &mut WalkPath,
    ) -> Result<Step, WalkError> {
        // Step 1.
        self.target_check(path);
        // Step 2.
        if self.u.has_state(unit, 13) {
            // `0x005C9D90` (skills spec). TODO(spec: pathing.md §9.2 step 2,
            // whether the step continues after it): treated as a branch.
            self.u.state13_step(game, unit);
            return Ok(Step::Moving);
        }
        // Step 3.
        if self.u.mode(unit) == 3
            && self.run_drain(game, unit, path)
            && start_movement(self.t, self.w, self.u, game, unit, path, 3)? == 0
        {
            neutral_start(self.w, self.u, game, unit, path);
        }
        // Step 4.
        self.step(game, unit, path)
        // Step 5 (host-only position history) is not simulated.
    }

    /// Target check `0x00553490` (§9.2 step 1).
    pub fn target_check(&mut self, path: &mut WalkPath) {
        let Some(tu) = path.target_unit else { return };
        let found = self.u.find_unit(tu.ty, tu.guid);
        let drop = match found {
            Some(f) if f == tu.unit => {
                self.u.unit_type(f) == UnitType::Item && matches!(self.u.mode(f), 1 | 2)
            }
            _ => true,
        };
        if drop {
            path.target_unit = None;
        }
    }

    /// Step `0x00554CA0(game, unit)` (§9.3).
    pub fn step(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        path: &mut WalkPath,
    ) -> Result<Step, WalkError> {
        self.target_check(path);
        let m = self.movement(game, unit, path, STEP_BASE)?;
        if path.flags & flag::ROOM_CHANGED != 0 {
            self.room_change_messages(game, unit, path);
        }
        Ok(if m { Step::Moving } else { Step::Stopped })
    }

    /// Movement `0x00650840(unit, base)` (§9.4). True = result 1.
    pub fn movement(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        path: &mut WalkPath,
        base: i32,
    ) -> Result<bool, WalkError> {
        let ty = self.u.unit_type(unit);
        let missile = ty == UnitType::Missile;
        // Rule 1.
        path.flags &= !flag::CROSSED;
        if missile {
            path.collided = 0;
        }
        // Rule 2.
        let go = path.flags & flag::ACTIVE != 0
            && path.count > 0
            && path.velocity != 0
            && (missile || self.arrival(game, unit, path)?)
            && path.index < path.count;
        if go {
            // 2.1.
            let base = if base <= 0 { STEP_BASE } else { base };
            if path.acceleration != 0 {
                path.accel_counter += 1;
                if path.accel_counter > 4 {
                    path.velocity = (path.velocity + path.acceleration).clamp(0, path.max_velocity);
                    if path.velocity == path.max_velocity {
                        path.acceleration = 0;
                    }
                    path.accel_counter = 0;
                }
            }
            let m = base.wrapping_mul(path.velocity) >> 6;
            path.vel_vec = (
                m.wrapping_mul(path.dir_vec.0) >> 12,
                m.wrapping_mul(path.dir_vec.1) >> 12,
            );
            // 2.2.
            if path.vel_vec != (0, 0) {
                // 2.3, 2.4.
                let q = self.one_step(game, unit, path)?;
                self.set_position(unit, path, q, None);
                // 2.5.
                if !missile && path.index < path.count {
                    aim(self.t, path, ty);
                }
                // 2.6.
                if path.index < path.count {
                    return Ok(true);
                }
            }
        }
        // Rule 3.
        self.reset(unit, path);
        Ok(false)
    }

    /// Arrival check `0x006503F0` (§9.5). The re-path's result decides
    /// where the rule says "re-path".
    fn arrival(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        path: &mut WalkPath,
    ) -> Result<bool, WalkError> {
        let pos = path.cell();
        if matches!(path.path_type, 5 | 6) && path.index >= path.count {
            return Ok(true);
        }
        let Some(tu) = path.target_unit else {
            if path.index >= path.count && pos != path.final_target {
                return Ok(self.repath(game, unit, path, false)? != 0);
            }
            return Ok(true);
        };
        let tpos = self.u.position(tu.unit);
        let d = unit_distance(self.t, pos, path.size, tpos, self.u.unit_size(tu.unit));
        if d <= path.stop_distance as i32 {
            return Ok(false);
        }
        let refreshed = super::find::refresh_point(self.u, path, tu.unit);
        let tty = self.u.unit_type(tu.unit);
        if matches!(tty, UnitType::Player | UnitType::Monster)
            && ((refreshed.x - path.prev_target.x).abs() > 5
                || (refreshed.y - path.prev_target.y).abs() > 5)
        {
            return Ok(self.repath(game, unit, path, true)? != 0);
        }
        if path.index < path.count || pos == path.final_target {
            return Ok(true);
        }
        Ok(self.repath(game, unit, path, true)? != 0)
    }

    /// One step `0x00650660` (§9.6 rules 1–7): the new precise position Q.
    fn one_step(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        path: &mut WalkPath,
    ) -> Result<(u32, u32), WalkError> {
        let ty = self.u.unit_type(unit);
        let missile = ty == UnitType::Missile;
        // Rule 1.
        path.collided = 0;
        let monster_repath = ty == UnitType::Monster && path.flags & flag::KEEP_TARGET != 0;
        // Rule 2.
        if path.vel_vec == (0, 0) {
            path.count = 0;
            path.index = 0;
            return Ok(centre_of(path));
        }
        // Rule 3.
        let (mut dx, mut dy) = path.vel_vec;
        let mut reaches = false;
        if !missile {
            let i = path.index.clamp(0, MAX_POINTS as i32 - 1) as usize;
            let p = path.points[i];
            let rx = centre(p.x).wrapping_sub(path.precise_x) as i32;
            let ry = centre(p.y).wrapping_sub(path.precise_y) as i32;
            let m = dx.abs().max(dy.abs());
            if rx.abs() <= m && ry.abs() <= m {
                dx = rx;
                dy = ry;
                reaches = true;
            }
        }
        // Rule 4.
        let nx = path.precise_x.wrapping_add_signed(dx);
        let ny = path.precise_y.wrapping_add_signed(dy);
        let mut q = (nx, ny);
        if (cell_of(nx), cell_of(ny)) != (cell_of(path.precise_x), cell_of(path.precise_y)) {
            if path.distance_budget > 0
                && path.path_type != path_type::KNOCKBACK
                && path.path_type != path_type::KNOCKBACK_CLIENT
            {
                path.distance_budget -= 1;
            }
            match self.cell_walk(unit, path, (dx, dy))? {
                Ok(end) => q = end,
                Err(last_free) => {
                    if monster_repath {
                        let finish = path.target_unit.is_some();
                        self.repath(game, unit, path, finish)?;
                    } else {
                        path.index = path.count;
                    }
                    return Ok(last_free);
                }
            }
        }
        // Rule 7.
        if reaches {
            path.index += 1;
        }
        Ok(q)
    }

    /// Cell walk `0x00650150` (§9.6 rule 5). `Ok(Q)` or `Err(centre of the
    /// last free cell)`.
    #[allow(clippy::type_complexity)]
    pub(crate) fn cell_walk(
        &mut self,
        unit: UnitId,
        path: &mut WalkPath,
        d: (i32, i32),
    ) -> Result<Result<(u32, u32), (u32, u32)>, WalkError> {
        let mut k = 0usize;
        path.saved_count = 0;
        let (mut sx, mut sy) = d;
        while sx.abs() > 0x10000 || sy.abs() > 0x10000 {
            sx >>= 1;
            sy >>= 1;
        }
        let end = (
            path.precise_x.wrapping_add_signed(d.0),
            path.precise_y.wrapping_add_signed(d.1),
        );
        let end_cell = (cell_of(end.0), cell_of(end.1));
        let mut c = (path.precise_x, path.precise_y);
        let mut guard = 0u32;
        while (cell_of(c.0), cell_of(c.1)) != end_cell {
            guard += 1;
            if guard > CELL_WALK_GUARD {
                return Err(WalkError::Fatal("cell walk does not reach its cell"));
            }
            let n = (c.0.wrapping_add_signed(sx), c.1.wrapping_add_signed(sy));
            let (cc, nc) = (
                Point::new(cell_of(c.0), cell_of(c.1)),
                Point::new(cell_of(n.0), cell_of(n.1)),
            );
            if cc != nc {
                if !self.footprint_move(unit, path, cc, nc)? {
                    path.saved_count = k as i32;
                    if k > 0 {
                        path.flags |= flag::CROSSED;
                    }
                    return Ok(Err((centre(cc.x), centre(cc.y))));
                }
                if path.flags & flag::SAVE_STEPS != 0 {
                    path.saved_steps[k] = nc;
                    k += 1;
                    if k >= super::seams::MAX_SAVED_STEPS {
                        break;
                    }
                }
            }
            c = n;
        }
        path.saved_count = k as i32;
        if k > 0 {
            path.flags |= flag::CROSSED;
        }
        Ok(Ok(end))
    }

    /// Footprint move `0x0064FF90` (§9.6 rule 6). True = accepted.
    fn footprint_move(
        &mut self,
        unit: UnitId,
        path: &mut WalkPath,
        old: Point,
        new: Point,
    ) -> Result<bool, WalkError> {
        let ty = self.u.unit_type(unit);
        if path.flags & flag::FORCED != 0 {
            if !matches!(ty, UnitType::Player | UnitType::Monster) {
                return Err(WalkError::Fatal("forced footprint move of a non-walker"));
            }
            self.w
                .forced_move(path.room, old, new, path.pattern, path.footprint_mask);
            return Ok(true);
        }
        if ty == UnitType::Missile {
            let r = self.w.missile_move(
                path.room,
                old,
                new,
                path.size,
                path.footprint_mask,
                path.move_mask,
            );
            path.collided |= r;
            return Ok(path.collided & 0x5 == 0);
        }
        let test = if path.move_mask == 0x3401 {
            0x3C01
        } else {
            path.move_mask
        };
        let r = self
            .w
            .try_move(path.room, old, new, path.pattern, path.footprint_mask, test);
        path.collided |= r;
        Ok(path.collided == 0)
    }

    /// Set position `0x0064FB90(Q, hint)` (§9.6 rule 8).
    pub fn set_position(
        &mut self,
        unit: UnitId,
        path: &mut WalkPath,
        q: (u32, u32),
        hint: Option<RoomId>,
    ) {
        let missile = self.u.unit_type(unit) == UnitType::Missile;
        if missile
            && self
                .w
                .cell_room(path.room, cell_of(q.0), cell_of(q.1))
                .is_none()
        {
            path.count = 0;
            return;
        }
        path.precise_x = q.0;
        path.precise_y = q.1;
        let a = (q.0 as i32) >> 11;
        let b = (q.1 as i32) >> 11;
        path.client_x = (a - b) >> 1;
        path.client_y = (a + b) >> 2;
        if path.flags & flag::OUTSIDE_ROOM != 0 {
            self.room_recache(unit, path, hint, missile);
        }
    }

    /// Room recache `0x0064FAD0(hint)` (§9.6 rule 9).
    fn room_recache(
        &mut self,
        unit: UnitId,
        path: &mut WalkPath,
        hint: Option<RoomId>,
        missile: bool,
    ) {
        let c = path.cell();
        if let Some(r) = path.room {
            let (rx, ry, rw, rh) = self.w.room_rect(r);
            if c.x >= rx && c.x < rx + rw && c.y >= ry && c.y < ry + rh {
                return;
            }
        }
        let new = self
            .w
            .cell_room(path.room, c.x, c.y)
            .or_else(|| self.w.cell_room(hint, c.x, c.y));
        if new.is_none() && missile {
            path.count = 0;
            return;
        }
        path.prev_room = path.room;
        if let Some(old) = path.room {
            self.w.room_list_remove(unit, old);
        }
        path.flags |= flag::ROOM_CHANGED;
        path.room = new;
        if let Some(n) = new {
            self.w.room_list_insert(unit, n);
            self.w.queue_for_update(unit);
        }
    }

    /// Reset `0x006507B0` (§9.7).
    pub fn reset(&mut self, unit: UnitId, path: &mut WalkPath) {
        let q = centre_of(path);
        self.set_position(unit, path, q, None);
        path.flags &= !flag::ACTIVE;
        path.count = 0;
        path.index = 0;
        path.vel_vec = (0, 0);
    }

    /// Room-change messages `0x00554670(game, unit, 0)` (§9.8).
    pub fn room_change_messages(&mut self, game: &mut Game, unit: UnitId, path: &mut WalkPath) {
        if path.flags & flag::ROOM_CHANGED == 0 {
            return;
        }
        if self.u.unit_type(unit) == UnitType::Monster {
            self.u.clear_ai_room_memo(unit);
        }
        let old = path
            .prev_room
            .map(|r| self.w.room_clients(r))
            .unwrap_or_default();
        let new = path
            .room
            .map(|r| self.w.room_clients(r))
            .unwrap_or_default();
        let (mut i, mut j) = (0, 0);
        while i < old.len() || j < new.len() {
            let a = old.get(i).copied();
            let b = new.get(j).copied();
            match (a, b) {
                (Some(x), Some(y)) if x == y => {
                    i += 1;
                    j += 1;
                }
                (Some(x), y) if y.is_none_or(|y| x < y) => {
                    if self.u.client_player(x) != Some(unit) {
                        self.u.send_unit_removal(game, x, unit);
                    }
                    i += 1;
                }
                (_, Some(y)) => {
                    if self.u.client_player(y) != Some(unit) {
                        self.u.send_unit_add(game, y, unit);
                    }
                    j += 1;
                }
                _ => break,
            }
        }
        path.flags &= !flag::ROOM_CHANGED;
    }

    /// Run drain `0x0057F240` (§9.9). True = exhausted.
    pub fn run_drain(&mut self, game: &mut Game, unit: UnitId, path: &WalkPath) -> bool {
        if path.room.is_some_and(|r| self.w.room_in_town(r)) {
            return false;
        }
        let mut d = 2 * self.u.charstats_velocity(unit).2;
        if let Some(speed) = self.u.torso_speed(unit) {
            d *= speed / 10 + 1;
        }
        let s = self.u.item_stat(unit, STAT_STAMINADRAIN);
        d -= s.wrapping_mul(d) / 100;
        if d < 1 {
            d = 1;
        }
        self.u.add_base_stat(game, unit, STAT_STAMINA, -d);
        if self.u.stat(unit, STAT_STAMINA) > 0 {
            return false;
        }
        self.u.set_base_stat(game, unit, STAT_STAMINA, 0);
        true
    }

    /// Re-path `0x00650350(unit, finish)` (§9.10).
    pub fn repath(
        &mut self,
        _game: &mut Game,
        unit: UnitId,
        path: &mut WalkPath,
        finish: bool,
    ) -> Result<i32, WalkError> {
        let ty = self.u.unit_type(unit);
        if path.flags & flag::KEEP_TARGET == 0 {
            if matches!(ty, UnitType::Player | UnitType::Monster) && self.u.repath_budget(unit) == 0
            {
                return Ok(0);
            }
            self.w.queue_for_update(unit);
            self.u.set_unit_flag(unit, 1);
            path.distance_budget = path.distance_budget.wrapping_sub(path.index as u8);
        }
        // TODO(spec: pathing.md §9.10, the town-access argument of the
        // re-path's compute): 0, the walk request's value.
        let town = false;
        if matches!(
            path.path_type,
            path_type::TOWARD | path_type::TOWARD_FINISH | path_type::WALL_FOLLOW
        ) {
            if finish {
                set_type(self.t, path, ty, path_type::TOWARD_FINISH)?;
            } else {
                set_type(self.t, path, ty, path_type::TOWARD)?;
                path.target = path.final_target;
            }
            let r = compute(self.t, self.w, self.u, path, unit, town)?;
            if r != 0 {
                return Ok(r);
            }
            set_type(self.t, path, ty, path_type::WALL_FOLLOW)?;
            return compute(self.t, self.w, self.u, path, unit, town);
        }
        compute(self.t, self.w, self.u, path, unit, town)
    }
}
