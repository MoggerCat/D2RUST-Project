// Spec: specs/monsters/ai.md §1.4, §7.1, §7.5 (mode request record, movement set-up); specs/sim/units.md §4.6 (start and event functions, rules 5–14); specs/sim/pathing.md §8.1, §9.1, §9.3, §9.10, §13.1 (monster motion on the path provider)
//! Monster walk / run on the path provider and the monster mode
//! functions of `units.md` §4.6: the path part of the monster mode set
//! `0x005A7C20` (target, re-path budget, the movement set-up `0x005A63F0`
//! / `0x005A6290` with the AI's velocity request, `ai.md` §7.5), the
//! velocity half of `0x00623F50` for a monster's mode, the start
//! functions (rules 5–12), the event-0 functions of the moving modes and
//! S3 (rules 11, 13), the knockback end (rule 14) and the mode end
//! `0x005A8030` (the inline think of `ai.md` §1.4).
//!
//! Each entry is called from one [`crate::units::hooks::UnitHooks`]
//! method of [`ActionHooks`]. The path parts do nothing while the
//! provider is off; there a unit has no path, so its point count reads 0
//! (a walk / run start then fails into neutral).

use crate::monsters::ai::{self, ModeTarget, VelocityRequest};
use crate::path::record::path_types;
use crate::path::walk::find::{compute, reset_type};
use crate::path::walk::seams::{PathWorld, WalkUnits};
use crate::path::walk::velocity::{mode_velocity_as, set_velocity};
use crate::path::walk::Step;
use crate::path::DynamicPath;
use crate::tick::events::event;
use crate::units::hooks::Sim;
use crate::units::modes::{monster_mode, monster_moves, MONSTER_MODES};
use crate::units::record::flags as unit_flags;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::unit_update::anim_complete;
use crate::wiring::action::{ActionHooks, Pending, View, WiringError};

use super::missiles::set_step_counts;
use super::PathCtx;

/// The re-path budget the monster mode set gives (`0x006490E0(path, 20)`,
/// `pathing.md` §9.10).
pub const MONSTER_REPATH_BUDGET: u32 = 20;
/// Request path-type byte of a moving mode (`ai.md` §7.1): path type 13.
pub const REQUEST_MOVING: u8 = 101;
/// Request path-type byte of any other mode: no path.
pub const REQUEST_NO_PATH: u8 = 100;
/// Default step count of the movement set-up (`ai.md` §7.5 rule 4.3).
pub const SETUP_STEPS: i32 = 5;
/// The mode end `0x005A8030` (event 1 of modes 3–9 and 14, `units.md`
/// §4.6; walk's stop, `pathing.md` §9.1).
pub const MODE_END: u32 = 0x005A_8030;
/// Monster walk / run / knockback / sequence / S3 event 0 (`units.md`
/// §4.6 rules 11, 13).
pub const WALK_EVENT0: u32 = MONSTER_MODES[2].event0;
pub const RUN_EVENT0: u32 = MONSTER_MODES[15].event0;
pub const KB_EVENT0: u32 = MONSTER_MODES[13].event0;
pub const SQ_EVENT0: u32 = MONSTER_MODES[14].event0;
/// The attack-family event 0 `0x005A7670` (modes 4, 5, 7, 8, 9).
pub const ATTACK_EVENT0: u32 = MONSTER_MODES[4].event0;
pub const S3_EVENT0: u32 = MONSTER_MODES[10].event0;
/// S3 event 1 `0x005A74D0`: nothing.
pub const S3_EVENT1: u32 = MONSTER_MODES[10].event1;
/// Knockback event 1 `0x005A8520` (rule 14).
pub const KB_EVENT1: u32 = MONSTER_MODES[13].event1;
/// The start functions (`units.md` §4.6 rules 5–12).
pub const WALK_START: u32 = MONSTER_MODES[2].start;
pub const RUN_START: u32 = MONSTER_MODES[15].start;
pub const GH_START: u32 = MONSTER_MODES[3].start;
pub const ATTACK_START: u32 = MONSTER_MODES[4].start;
pub const BL_START: u32 = MONSTER_MODES[6].start;
pub const KB_START: u32 = MONSTER_MODES[13].start;
pub const SQ_START: u32 = MONSTER_MODES[14].start;
pub const S3_START: u32 = MONSTER_MODES[10].start;
pub const S4_START: u32 = MONSTER_MODES[11].start;
/// State 13 and state 22, the walk event's skill calls (`pathing.md`
/// §9.1: `0x005C9D90`, `0x005CE4F0`).
const STATE_13: u16 = 13;
const STATE_22: u16 = 22;
/// `BaseId` 78 (`sandleaper1`, rules 9 and 14) and 110 (`vulture1`, rule 7).
const BASE_SANDLEAPER: u16 = 78;
const BASE_VULTURE: u16 = 110;
/// The knockback distance budget (`0x00648E40`, rule 9).
const KB_BUDGET_SANDLEAPER: u8 = 10;
const KB_BUDGET: u8 = 5;
/// The S4 start's think delay (rule 12).
const S4_THINK_DELAY: i32 = 15;
/// Monster modes used here (`units.md` §4.6).
const MODE_WL: u32 = 2;
const MODE_GH: u32 = 3;
const MODE_BL: u32 = 6;
const MODE_S3: u32 = 10;
const MODE_S4: u32 = 11;
const MODE_KB: u32 = 13;
const MODE_SQ: u32 = 14;
const MODE_RN: u32 = 15;

/// The movement set-up's fields of the AI param record (monster data
/// +0x2C, `ai.md` §7.5 rules 4–5). The velocity request (+0x18, +0x1C,
/// +0x20) is the AI store's ([`VelocityRequest`]).
///
/// The counters of rule 4.5 (game +0x1D70 + 4·c, +0x1DB4) have no reader
/// in the specs and are not kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveSetup {
    /// Bytes +0x00, +0x01 (zeroed by rule 5).
    pub byte0: u8,
    pub byte1: u8,
    /// +0x04, +0x08, +0x0C: the target cache (path target unit, target x,
    /// target y).
    pub cache_unit: Option<UnitId>,
    pub cache_x: u16,
    pub cache_y: u16,
    /// +0x10: the request's speed (rule 4.4).
    pub speed: i32,
    /// +0x14: 10 or −1 (rule 4.4).
    pub wait: i32,
}

/// The path types whose failed compute retries with type 15 (`ai.md`
/// §7.5 rule 5, `0x005A6290`).
fn retries(ty: u32) -> bool {
    matches!(ty, 2 | 7 | 9 | path_types::TOWARD_FINISH)
}

/// The request record's path-type byte (`0x005A7E60`, `ai.md` §7.1):
/// 101 for a moving mode (`0x005A6B10`), else 100.
pub fn request_path_byte(moves: bool) -> u8 {
    if moves {
        REQUEST_MOVING
    } else {
        REQUEST_NO_PATH
    }
}

/// What the movement set-up's compute (rule 5) left to do on the unit.
struct Computed {
    /// c of rule 4.
    c: u32,
    /// U queued for update and flags |= 1.
    queue: bool,
}

/// Compute `0x005A6290(t)` (`ai.md` §7.5 rule 5) on a loaded path.
fn setup_compute<C: PathWorld + WalkUnits + ?Sized>(
    tables: &crate::path::PathTables,
    c: &mut C,
    d: &mut DynamicPath,
    unit: UnitId,
    p: &mut MoveSetup,
    t: u32,
    n: i32,
) -> Result<Computed, WiringError> {
    set_step_counts(d, n);
    let cache = (d.target_unit.map(|u| u.unit), d.target_x, d.target_y);
    if cache != (p.cache_unit, p.cache_x, p.cache_y) {
        (p.cache_unit, p.cache_x, p.cache_y) = cache;
        p.byte0 = 0;
        p.byte1 = 0;
    }
    d.set_path_type(tables, false, t)
        .map_err(WiringError::Path)?;
    compute(tables, c, d, unit, false).map_err(WiringError::Walk)?;
    if d.point_count != 0 {
        p.byte0 = 0;
        p.byte1 = 0;
        return Ok(Computed { c: t, queue: true });
    }
    if retries(t) {
        d.set_path_type(tables, false, path_types::WALL_FOLLOW)
            .map_err(WiringError::Path)?;
        compute(tables, c, d, unit, false).map_err(WiringError::Walk)?;
        return Ok(Computed {
            c: path_types::WALL_FOLLOW,
            queue: true,
        });
    }
    Ok(Computed { c: t, queue: false })
}

impl<X: Pending> ActionHooks<X> {
    /// The path part of `0x005A7C20` before the start function (every
    /// mode but GH, `ai.md` §7.5 rules 1–6): the path target from the
    /// request (unit `0x00648B90` when given, else the point `0x00648AD0`;
    /// a caller without a record target gives (0, 0), rule 6), the
    /// re-path budget 20, then the movement set-up `0x005A63F0` (rule 4)
    /// with the AI's velocity request, which it consumes.
    pub(crate) fn monster_path_setup(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {
        let Some(p) = self.paths.as_mut() else {
            return;
        };
        if mode == monster_mode::GH {
            return;
        }
        // Rule 1: a monster only.
        if sim.units.get(unit).map(|r| r.ty) != Some(UnitType::Monster) {
            return;
        }
        let staged = match p.mode_request {
            Some((u, t)) if u == unit => {
                p.mode_request = None;
                Some(t)
            }
            _ => None,
        };
        let byte_override = match p.mode_request_byte {
            Some((u, b)) if u == unit => {
                p.mode_request_byte = None;
                Some(b)
            }
            _ => None,
        };
        let staged_velocity = match p.mode_velocity {
            Some((u, v)) if u == unit => {
                p.mode_velocity = None;
                Some(v)
            }
            _ => None,
        };
        // Rule 4.1: the request (staged by an AI mode request while the
        // AI store is lent out, else the store's own) is consumed.
        let velocity = staged_velocity.unwrap_or_else(|| {
            self.ai
                .as_mut()
                .filter(|s| s.get(unit).is_some())
                .map(|s| std::mem::take(&mut s.entry(unit).velocity))
                .unwrap_or_default()
        });
        // Rule 2 and rule 6: no record target → the point (0, 0).
        let target = staged
            .or(self.mode_target.map(ModeTarget::Unit))
            .unwrap_or(ModeTarget::Point(0, 0));
        let moves = monster_moves(sim, unit, mode);
        let Some(p) = self.paths.as_mut() else {
            return;
        };
        let tables = p.tables.clone();
        let mut setup = p.setup.get(&unit).copied().unwrap_or_default();
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        match target {
            ModeTarget::Unit(t) => v.path_set_target_unit(unit, t),
            ModeTarget::Point(x, y) => v.h.path_set_target_point(unit, x, y),
        }
        let mut c = PathCtx::of(&mut v, sim.game);
        let Some(mut d) = c.load_path(unit) else {
            return;
        };
        let r = (|| -> Result<bool, WiringError> {
            // Rule 3.
            d.set_repath_budget(MONSTER_REPATH_BUDGET)
                .map_err(WiringError::Path)?;
            // Rule 4.1.
            let VelocityRequest {
                method,
                speed,
                steps,
            } = velocity;
            // The record's byte +0x15: the builder's, or the AI's
            // override (`ai.md` §7.1).
            let mut t = u32::from(byte_override.unwrap_or_else(|| request_path_byte(moves)));
            let mut v = 0i32;
            let mut n = 0i32;
            if method != 0 {
                t = method as u32;
            }
            if speed != 0 {
                v = speed;
            }
            if steps != 0 {
                n = i32::from(steps as u8);
            }
            // Rules 4.2–4.3.
            let (c_res, queue) = if t == u32::from(REQUEST_NO_PATH) {
                v = 0;
                (0, false)
            } else {
                if n == 0 {
                    n = SETUP_STEPS;
                }
                if t == u32::from(REQUEST_MOVING) {
                    t = path_types::TOWARD_FINISH;
                }
                let k = setup_compute(&tables, &mut c, &mut d, unit, &mut setup, t, n)?;
                (k.c, k.queue)
            };
            // Rule 4.4.
            setup.wait = if t != u32::from(REQUEST_NO_PATH) && c_res != 1 && d.target_unit.is_some()
            {
                10
            } else {
                -1
            };
            setup.speed = v;
            Ok(queue)
        })();
        c.store_path(unit, &d);
        if let Some(p) = self.paths.as_mut() {
            p.setup.insert(unit, setup);
        }
        match r {
            Ok(true) => {
                if let Some(rec) = sim.units.get_mut(unit) {
                    rec.flags |= unit_flags::CHANGED;
                }
                if let Err(e) = sim.game.lists.queue_update(unit) {
                    self.errors
                        .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                            e.into(),
                        )));
                }
            }
            Ok(false) => {}
            Err(e) => self.errors.push(e),
        }
    }

    /// The velocity half of `0x00623F50` (`pathing.md` §8.1) for a
    /// monster in its current mode, run by the animation prepare of every
    /// monster mode set: a mode with the velocity modifier (or knockback)
    /// sets the path velocity, any other mode leaves it.
    pub(crate) fn monster_mode_velocity(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(p) = self.paths.as_ref() else {
            return;
        };
        let Some(r) = sim.units.get(unit) else {
            return;
        };
        if r.ty != UnitType::Monster {
            return;
        }
        let own = (r.ty, r.class, r.mode);
        let tables = p.tables.clone();
        // `0x00623F50` runs on the draw identity `0x00645270`: a summoned
        // Valkyrie (state 93, `gfxtype` 2, class 0) reads the Amazon's
        // `WalkVelocity` and the player's mode rows.
        let (ty, class, mode) = self.draw_identity(sim, unit).unwrap_or(own);
        let base = if ty == UnitType::Player {
            self.tables
                .combat
                .charstats
                .get(class as usize)
                .map(|c| i32::from(c.walkvelocity))
        } else {
            self.tables
                .combat
                .monstats
                .get(class as usize)
                .map(|m| i32::from(m.velocity))
        }
        .unwrap_or(0)
            * 256;
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        let c = PathCtx::of(&mut v, sim.game);
        let ident = ((ty, class) != (own.0, own.1)).then_some((ty, class));
        let Some(vel) = mode_velocity_as(&tables, &c, unit, ident, mode, base) else {
            return;
        };
        if let Some(d) = self.paths.as_mut().and_then(|p| p.dynamic_mut(unit)) {
            set_velocity(d, vel);
        }
    }

    /// The monster mode functions of this module: `Some(started)` when
    /// `address` is one of them, `None` otherwise (the caller keeps its
    /// own routes).
    pub(crate) fn monster_motion_function(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        address: u32,
    ) -> Option<bool> {
        let started = match address {
            WALK_START => self.monster_move_start(sim, unit, MODE_WL),
            RUN_START => self.monster_move_start(sim, unit, MODE_RN),
            GH_START => self.monster_gh_start(sim, unit),
            ATTACK_START => self.monster_attack_start(sim, unit),
            BL_START => self.plain_mode(sim, unit, MODE_BL),
            KB_START => self.monster_kb_start(sim, unit),
            SQ_START => self.monster_sq_start(sim, unit),
            S3_START => self.plain_mode(sim, unit, MODE_S3),
            S4_START => self.monster_s4_start(sim, unit),
            MODE_END => {
                let ended = sim.units.get(unit).map_or(0, |r| r.mode);
                self.ai_mode_end(sim, unit, ended as u8);
                true
            }
            WALK_EVENT0 => {
                self.monster_moving_event0(sim, unit, true);
                true
            }
            RUN_EVENT0 => {
                self.monster_moving_event0(sim, unit, false);
                true
            }
            KB_EVENT0 => {
                self.monster_kb_event0(sim, unit);
                true
            }
            KB_EVENT1 => {
                self.monster_kb_event1(sim, unit);
                true
            }
            SQ_EVENT0 => {
                self.monster_sq_event0(sim, unit);
                true
            }
            ATTACK_EVENT0 => {
                self.monster_attack_event0(sim, unit);
                true
            }
            S3_EVENT0 => {
                self.monster_s3_event0(sim, unit);
                true
            }
            S3_EVENT1 => true,
            _ => return None,
        };
        Some(started)
    }

    /// "Set mode m": the plain mode set `0x00553570` (`units.md` §4.1);
    /// returns 1.
    fn plain_mode(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) -> bool {
        if let Err(e) = crate::units::modes::set_mode(sim, self, unit, mode) {
            self.errors.push(WiringError::Unit(e));
        }
        true
    }

    /// Path point count (+0x28, `0x00648780`); 0 without a path.
    fn point_count(&self, unit: UnitId) -> u32 {
        self.paths
            .as_ref()
            .and_then(|p| p.dynamic(unit))
            .map_or(0, |d| d.point_count)
    }

    /// The unit's monstats `BaseId` (row +0x02).
    fn base_id(&self, sim: &Sim<'_>, unit: UnitId) -> Option<u16> {
        let class = sim.units.get(unit)?.class;
        self.tables
            .combat
            .monstats
            .get(class as usize)
            .map(|m| m.baseid)
    }

    /// `0x0046C140(class, mode)`.
    fn class_has(&self, sim: &Sim<'_>, unit: UnitId, mode: u8) -> bool {
        let class = sim.units.get(unit).map_or(-1, |r| r.class as i32);
        self.x.class_has_mode(class, mode)
    }

    /// The path compute `0x00649970(path, U, 0)` with the provider.
    fn monster_compute(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        if self.paths.is_none() {
            return;
        }
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        crate::wiring::path::walk::build_path(&mut v, sim.game, unit);
    }

    /// The path step `0x00554CA0` (`pathing.md` §9.3): `true` when it
    /// returned 2 (stopped). Without the provider, the pending step.
    fn monster_step(&mut self, sim: &mut Sim<'_>, unit: UnitId) -> bool {
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        if v.h.paths.is_some() {
            PathCtx::of(&mut v, sim.game).step(unit) == Some(Step::Stopped)
        } else {
            !v.h.x.step(sim.game, unit)
        }
    }

    /// The animation is complete (`0x006217C0`).
    fn monster_anim_complete(&self, sim: &Sim<'_>, unit: UnitId) -> bool {
        sim.units.get(unit).is_some_and(|r| anim_complete(&r.anim))
    }

    /// WL start `0x005A7520` / RN start `0x005A7550` (`units.md` §4.6
    /// rule 5): point count 0 → 0 (neutral); else set the mode, 1.
    fn monster_move_start(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) -> bool {
        if self.point_count(unit) == 0 {
            return false;
        }
        let started = self.plain_mode(sim, unit, mode);
        self.apply_speed_bonus(sim, unit);
        started
    }

    /// The set-up's speed bonus (P +0x10, `ai.md` §7.5 rule 4.4; the AI's
    /// velocity request speed, e.g. the Fallen's escape 50) as a
    /// temporary stat-67 list like the player's run list (`pathing.md`
    /// §8.2), read by the velocity rule (§8.1 rule 2) and the rate
    /// (`units.md` §4.7 step 7).
    // PROVISIONAL (ai.md §7.5 rule 4.4, REC-1111): the spec stores P
    // +0x10 := v and names no reader. 1.14d's Fallen flees at velocity
    // and speed ×(75 + 50) / 75 of its walk (speed 192 → 320, check
    // combat-melee-fallen, unit 20 from f48), which is stat 67 raised by
    // v for the mode. Settled by a recording of another speed bonus.
    fn apply_speed_bonus(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(speed) = self
            .paths
            .as_ref()
            .and_then(|p| p.setup.get(&unit))
            .map(|s| s.speed)
            .filter(|&v| v != 0)
        else {
            return;
        };
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut c = PathCtx::of(&mut v, sim.game);
            WalkUnits::attach_run_stats(&mut c, unit, speed);
        }
        self.monster_mode_velocity(sim, unit);
    }

    /// GH start `0x005A7580` (rule 6).
    fn monster_gh_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) -> bool {
        let Some(m) = sim.units.get(unit).map(|r| r.mode) else {
            return true;
        };
        if matches!(m, 0 | 12) {
            return true;
        }
        if !self.class_has(sim, unit, MODE_GH as u8) {
            return false;
        }
        self.plain_mode(sim, unit, MODE_GH)
    }

    /// Attack / skill start `0x005A75C0` (modes 4, 5, 7, 8, 9; rule 7)
    /// with m = the record's mode ([`ActionHooks::monster_request`]).
    fn monster_attack_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) -> bool {
        let m = self.monster_request;
        if monster_moves(sim, unit, m) {
            if self.point_count(unit) != 0 {
                self.monster_compute(sim, unit);
            } else if self.base_id(sim, unit) == Some(BASE_VULTURE) {
                self.monster_compute(sim, unit);
                return self.plain_mode(sim, unit, m);
            } else {
                return false;
            }
        }
        self.plain_mode(sim, unit, m);
        // PROVISIONAL (units.md §4.6 rule 7 gives no flag step; REC-143):
        // unit flag 0x40 off before the skill start, as the player's mode
        // starts (rule table row 7..18) and the SQ start (rule 10) do. The
        // attack's do sets it (`skills/bodies.md` melee), the per-frame
        // event runs the do only while it is clear (`skills/use.md` §5.2
        // rule 3), so without this a monster whose first attack did not
        // kill never attacked again.
        if let Some(r) = sim.units.get_mut(unit) {
            r.flags &= !unit_flags::ATTACK_PENDING;
        }
        if self.used_skill_of(unit).is_some() {
            let _ = X::monster_skill_start(self, sim, unit);
        }
        true
    }

    /// KB start `0x005A77D0` (rule 9).
    fn monster_kb_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) -> bool {
        if !self.class_has(sim, unit, MODE_GH as u8) || !self.class_has(sim, unit, MODE_KB as u8) {
            return false;
        }
        match sim.units.get(unit).map(|r| r.mode) {
            None | Some(0) => return true,
            Some(_) => {}
        }
        let budget = if self.base_id(sim, unit) == Some(BASE_SANDLEAPER) {
            KB_BUDGET_SANDLEAPER
        } else {
            KB_BUDGET
        };
        if let Some(p) = self.paths.as_mut() {
            let tables = p.tables.clone();
            if let Some(d) = p.dynamic_mut(unit) {
                if let Err(e) = d.set_path_type(&tables, false, path_types::KNOCKBACK_SERVER) {
                    self.errors.push(WiringError::Path(e));
                }
                d.dist_budget = budget;
            }
            self.monster_compute(sim, unit);
        }
        self.plain_mode(sim, unit, MODE_KB)
    }

    /// SQ start `0x005A7870` (rule 10): mode 14, unit flag 0x40 off, the
    /// skill start's result (0 → neutral).
    fn monster_sq_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) -> bool {
        self.plain_mode(sim, unit, MODE_SQ);
        if let Some(r) = sim.units.get_mut(unit) {
            r.flags &= !unit_flags::ATTACK_PENDING;
        }
        X::monster_skill_start(self, sim, unit) != 0
    }

    /// S4 start `0x005A74E0` (rule 12): mode 11, think at f + 15.
    fn monster_s4_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) -> bool {
        self.plain_mode(sim, unit, MODE_S4);
        let at = sim.game.frame.wrapping_add(S4_THINK_DELAY);
        if let Err(e) = sim
            .game
            .schedule_event(unit, u32::from(event::AI_THINK), at, None, 0, 0)
        {
            self.errors
                .push(WiringError::Unit(crate::units::modes::UnitError::Game(e)));
        }
        true
    }

    /// WL event 0 `0x005A8490` / RN event 0 `0x005A84F0` (rule 13,
    /// `pathing.md` §9.1): WL only: state 13 → `0x005C9D90`, state 22 →
    /// `0x005CE4F0` (skills spec: seams), neither result tested; then the
    /// step (§9.3); 2 → the mode end `0x005A8030`. Without the provider
    /// nothing moves (the walk start failed).
    fn monster_moving_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId, walk: bool) {
        if self.paths.is_none() {
            return;
        }
        let stopped = {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut c = PathCtx::of(&mut v, sim.game);
            if walk {
                if c.has_state(unit, STATE_13) {
                    c.state13_step(unit);
                }
                if c.has_state(unit, STATE_22) {
                    c.state22_step(unit);
                }
            }
            c.step(unit) == Some(Step::Stopped)
        };
        if stopped {
            let ended = sim.units.get(unit).map_or(0, |r| r.mode);
            self.ai_mode_end(sim, unit, ended as u8);
        }
    }

    /// KB event 0 `0x005A8630` (rule 13): step (result ignored),
    /// animation refresh, animation complete → KB event 1.
    fn monster_kb_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let _ = self.monster_step(sim, unit);
        self.x.refresh_animation(sim.game, unit);
        if self.monster_anim_complete(sim, unit) {
            self.monster_kb_event1(sim, unit);
        }
    }

    /// KB event 1 `0x005A8520` (rule 14): the path type reset `0x00648DC0`
    /// (`pathing.md` §1.5 rule 1), then the knockback end of the AI
    /// (`ai.md` §1.2, [`ai::knockback_end`]).
    fn monster_kb_event1(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        if let Some(p) = self.paths.as_mut() {
            let tables = p.tables.clone();
            if let Some(d) = p.dynamic_mut(unit) {
                if let Err(e) = reset_type(&tables, d, UnitType::Monster) {
                    self.errors.push(WiringError::Walk(e));
                }
            }
        }
        self.with_ai(sim, |g, cx| ai::knockback_end(g, cx, unit));
    }

    /// SQ event 0 `0x005A8670` (rule 13): animation complete → the mode
    /// end; else the skill part ([`Pending::monster_sequence_frame`]),
    /// then the animation refresh.
    fn monster_sq_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        if self.monster_anim_complete(sim, unit) {
            let ended = sim.units.get(unit).map_or(0, |r| r.mode);
            self.ai_mode_end(sim, unit, ended as u8);
            return;
        }
        X::monster_sequence_frame(self, sim, unit);
        let advanced = sim
            .units
            .get_mut(unit)
            .is_some_and(|r| crate::units::anim::advance_sequence(&mut r.anim));
        if !advanced {
            self.x.refresh_animation(sim.game, unit);
        }
    }

    /// Attack-family event 0 `0x005A7670` (modes 4, 5, 7, 8, 9;
    /// `skills/use.md` §5.2 "Monsters"). With a used skill: its branch
    /// ([`Pending::monster_attack_skill`]: a moving skill's step, the do
    /// on every event with no frame-code test), then the animation
    /// refresh. Without one: r := the mode moves (`0x005A6B10`); moving →
    /// step, refresh, animation complete → done (2), trigger(U) false →
    /// done; then (r = 0, or a moving mode at its trigger frame) the
    /// strike ([`Pending::monster_attack_strike`]: the mode missile, else
    /// the melee on the path target), with no refresh.
    fn monster_attack_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        if self.used_skill_of(unit).is_some() {
            X::monster_attack_skill(self, sim, unit);
            self.x.refresh_animation(sim.game, unit);
            return;
        }
        let Some(mode) = sim.units.get(unit).map(|r| r.mode) else {
            return;
        };
        let moving = monster_moves(sim, unit, mode);
        if moving {
            let _ = self.monster_step(sim, unit);
            self.x.refresh_animation(sim.game, unit);
            if self.monster_anim_complete(sim, unit) {
                return;
            }
            if !crate::wiring::interaction::skill_events::monster_trigger(sim.units, unit) {
                return;
            }
        }
        X::monster_attack_strike(self, sim, unit, moving);
    }

    /// S3 event 0 `0x005A74A0` (rule 11): step (result ignored),
    /// animation refresh, animation complete → set mode 11.
    fn monster_s3_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let _ = self.monster_step(sim, unit);
        self.x.refresh_animation(sim.game, unit);
        if self.monster_anim_complete(sim, unit) {
            self.plain_mode(sim, unit, MODE_S4);
        }
    }

    /// The mode end `0x005A8030` (`ai.md` §1.4) on the AI store.
    fn ai_mode_end(&mut self, sim: &mut Sim<'_>, unit: UnitId, ended: u8) {
        self.with_ai(sim, |g, cx| ai::mode_end(g, cx, unit, ended));
    }

    /// Runs `f` on the AI store and the wired AI world.
    fn with_ai(
        &mut self,
        sim: &mut Sim<'_>,
        f: impl FnOnce(&mut crate::game::Game, &mut ai::Ctx<'_, View<'_, X>>),
    ) {
        let Some(mut store) = self.ai.take() else {
            self.errors.push(WiringError::Reentrant("ai"));
            return;
        };
        let t = self.tables.clone();
        let info = self.ai_info;
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                    skills: &t.skills.skills,
                    missiles: &t.skills.missiles,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            f(sim.game, &mut cx);
        }
        self.ai = Some(store);
    }
}

/// Stages the AI's mode request (the record's target fields, `ai.md`
/// §7.1) for the monster mode set that follows.
pub(crate) fn stage_request<X: Pending>(
    h: &mut ActionHooks<X>,
    unit: UnitId,
    target: ModeTarget,
    path_byte: Option<u8>,
) {
    if let Some(p) = h.paths.as_mut() {
        p.mode_request = Some((unit, target));
        p.mode_request_byte = path_byte.map(|b| (unit, b));
    }
}

/// Stages the AI's velocity request (`ai.md` §7.5 rule 4.1) for the
/// monster mode set that follows (the AI store is lent out meanwhile).
pub(crate) fn stage_velocity<X: Pending>(
    h: &mut ActionHooks<X>,
    unit: UnitId,
    velocity: VelocityRequest,
) {
    if let Some(p) = h.paths.as_mut() {
        p.mode_velocity = Some((unit, velocity));
    }
}

/// `0x00553540`: the path target unit; `None` without one.
pub(crate) fn path_target<X: Pending>(h: &ActionHooks<X>, unit: UnitId) -> Option<UnitId> {
    h.paths.as_ref()?.dynamic(unit)?.target_unit.map(|t| t.unit)
}

/// Path flag 0x800 (`ai.md` §2.2 rule 2: a blocked step).
pub(crate) fn path_blocked<X: Pending>(h: &ActionHooks<X>, unit: UnitId) -> Option<bool> {
    let p = h.paths.as_ref()?;
    Some(p.dynamic(unit).is_some_and(|d| d.flags & 0x800 != 0))
}

/// The stop distance `0x00649070(path, n)` (`pathing.md` §13.1 rule 2,
/// §9.5 rule 3): +0x93 := n − 1 for n in 1…19, else 0. `None`: the
/// provider is off.
pub fn stop_distance(n: i32) -> u8 {
    if (1..=19).contains(&n) {
        (n - 1) as u8
    } else {
        0
    }
}

/// [`stop_distance`] written to the unit's path; `None` without the
/// provider.
pub(crate) fn set_stop_distance<X: Pending>(
    h: &mut ActionHooks<X>,
    unit: UnitId,
    n: i32,
) -> Option<()> {
    let p = h.paths.as_mut()?;
    if let Some(d) = p.dynamic_mut(unit) {
        d.stop_distance = stop_distance(n);
    }
    Some(())
}

/// Stop the path `0x00648730` (`pathing.md` §13.1 rule 3): flags &=
/// ~0x20, point count := 0, nothing else; `None` without the provider.
pub(crate) fn stop_path<X: Pending>(h: &mut ActionHooks<X>, unit: UnitId) -> Option<()> {
    let p = h.paths.as_mut()?;
    if let Some(d) = p.dynamic_mut(unit) {
        d.flags &= !crate::path::record::flags::ACTIVE;
        d.point_count = 0;
    }
    Some(())
}
