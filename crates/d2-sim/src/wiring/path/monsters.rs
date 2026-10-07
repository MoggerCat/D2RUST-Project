// Spec: specs/monsters/ai.md §1.4, §7.1 (mode request record, movement set-up); specs/sim/units.md §4.6; specs/sim/pathing.md §8.1, §9.1, §9.3, §9.10 (monster motion on the path provider)
//! Monster walk / run on the path provider: the path part of the monster
//! mode set `0x005A7C20` (target, re-path budget, path type and compute
//! of `0x005A63F0` / `0x005A6290`), the velocity half of `0x00623F50`
//! for a monster's mode, the walk event 0 `0x005A8490` (step, then the
//! mode end `0x005A8030` on a stop) and the mode end itself (the inline
//! think of `ai.md` §1.4) for the event-1 functions of modes 3–9 and 14.
//!
//! Each entry is called from one [`crate::units::hooks::UnitHooks`]
//! method of [`ActionHooks`] and does nothing while the provider is off
//! (the mode end excepted: it is not a path function).
//!
//! Gaps left as seams (`docs/handoff/impl-path-motion.md`):
//! - the walk / run start bodies (`0x005A7520`, `0x005A7550`) are read as
//!   "set the mode" (`umod-callbacks.md` §2 rule 2);
//! - the run event 0 `0x005A84F0`, knockback `0x005A8630` and sequence
//!   `0x005A8670` bodies are not described ([`Pending::monster_run_event0`]);
//! - the AI velocity request (`ai.md` §7.3) is not mapped onto the
//!   request record's fields (§7.1 "replaces them"): not consumed;
//! - the path step count `0x00649070` writes a field `path-placement.md`
//!   §2.3 does not name: stays [`Pending::set_path_steps`].

use crate::monsters::ai::{self, ModeTarget};
use crate::path::record::path_types;
use crate::path::walk::find::compute;
use crate::path::walk::seams::{PathWorld, WalkUnits};
use crate::path::walk::velocity::{mode_velocity, set_velocity};
use crate::path::walk::Step;
use crate::units::hooks::Sim;
use crate::units::modes::{monster_mode, monster_moves, MONSTER_MODES};
use crate::units::{UnitId, UnitType};
use crate::wiring::action::{ActionHooks, Pending, View, WiringError};

use super::PathCtx;

/// The re-path budget the monster mode set gives (`0x006490E0(path, 20)`,
/// `pathing.md` §9.10).
pub const MONSTER_REPATH_BUDGET: u32 = 20;
/// Request path-type byte of a moving mode (`ai.md` §7.1): path type 13.
pub const REQUEST_MOVING: u8 = 101;
/// Request path-type byte of any other mode: no path (path type 0).
pub const REQUEST_NO_PATH: u8 = 100;
/// The mode end `0x005A8030` (event 1 of modes 3–9 and 14, `units.md`
/// §4.6; walk's stop, `pathing.md` §9.1).
pub const MODE_END: u32 = 0x005A_8030;
/// Monster walk event 0 (`pathing.md` §9.1).
pub const WALK_EVENT0: u32 = MONSTER_MODES[2].event0;
/// Monster run event 0 (`units.md` §4.6; body not described).
pub const RUN_EVENT0: u32 = MONSTER_MODES[15].event0;
/// Monster walk and run starts (`units.md` §4.6).
pub const WALK_START: u32 = MONSTER_MODES[2].start;
pub const RUN_START: u32 = MONSTER_MODES[15].start;
/// State 13 and state 22, the walk event's skill calls (`pathing.md`
/// §9.1: `0x005C9D90`, `0x005CE4F0`).
const STATE_13: u16 = 13;
const STATE_22: u16 = 22;

/// The path types whose failed compute retries with type 15 (`ai.md`
/// §7.1, `0x005A6290`).
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

/// The path type the movement set-up gives a request byte (`ai.md`
/// §7.1): 100 → `None` (type 0, nothing computed), 101 → 13, any other
/// value → itself.
pub fn path_type_of(byte: u8) -> Option<u32> {
    match byte {
        REQUEST_NO_PATH => None,
        REQUEST_MOVING => Some(path_types::TOWARD_FINISH),
        b => Some(u32::from(b)),
    }
}

impl<X: Pending> ActionHooks<X> {
    /// The path part of `0x005A7C20` before the start function (every
    /// mode but GH, `units.md` §4.6; `ai.md` §7.1): the path target from
    /// the request (unit `0x00648B90` when given, else the point
    /// `0x00648AD0`), the re-path budget 20 (`pathing.md` §9.10), then the
    /// movement set-up: no path for a mode that does not move, else path
    /// type 13 computed, and once more with type 15 when that finds no
    /// point.
    ///
    /// TODO(spec: ai.md §7.1): the request record reaches the mode set
    /// only from the AI's mode changes (staged in
    /// [`super::PathState::mode_request`]) and the kill's death change
    /// ([`ActionHooks::mode_target`]); other callers (quests, pets, skill
    /// bodies) do not pass theirs, so the target is left as it was.
    /// TODO(spec: ai.md §7.1): whether the movement set-up writes the
    /// type through set type `0x00648CF0` or directly is not stated; set
    /// type is used. The compute's town-access argument is not stated: 0
    /// (as the re-path, `pathing.md` §9.10).
    pub(crate) fn monster_path_setup(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {
        let Some(p) = self.paths.as_mut() else {
            return;
        };
        if mode == monster_mode::GH {
            return;
        }
        let staged = match p.mode_request {
            Some((u, t)) if u == unit => {
                p.mode_request = None;
                Some(t)
            }
            _ => None,
        };
        let target = staged.or(self.mode_target.map(ModeTarget::Unit));
        let moves = monster_moves(sim, unit, mode);
        let tables = p.tables.clone();
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        match target {
            Some(ModeTarget::Unit(t)) => v.path_set_target_unit(unit, t),
            Some(ModeTarget::Point(x, y)) => v.h.path_set_target_point(unit, x, y),
            None => {}
        }
        let mut c = PathCtx::of(&mut v, sim.game);
        let Some(mut d) = c.load_path(unit) else {
            return;
        };
        let r = (|| -> Result<(), WiringError> {
            d.set_repath_budget(MONSTER_REPATH_BUDGET)
                .map_err(WiringError::Path)?;
            let Some(ty) = path_type_of(request_path_byte(moves)) else {
                d.set_path_type(&tables, false, 0)
                    .map_err(WiringError::Path)?;
                return Ok(());
            };
            d.set_path_type(&tables, false, ty)
                .map_err(WiringError::Path)?;
            let n = compute(&tables, &mut c, &mut d, unit, false).map_err(WiringError::Walk)?;
            if n == 0 && retries(ty) {
                d.set_path_type(&tables, false, path_types::WALL_FOLLOW)
                    .map_err(WiringError::Path)?;
                compute(&tables, &mut c, &mut d, unit, false).map_err(WiringError::Walk)?;
            }
            Ok(())
        })();
        c.store_path(unit, &d);
        if let Err(e) = r {
            self.errors.push(e);
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
        let mode = r.mode;
        let tables = p.tables.clone();
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        let c = PathCtx::of(&mut v, sim.game);
        let Some(vel) = mode_velocity(&tables, &c, unit, mode) else {
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
        match address {
            WALK_START | RUN_START if self.paths.is_some() => {
                let mode = if address == WALK_START { 2 } else { 15 };
                Some(self.monster_move_start(sim, unit, mode))
            }
            MODE_END => {
                let ended = sim.units.get(unit).map_or(0, |r| r.mode);
                self.ai_mode_end(sim, unit, ended as u8);
                Some(true)
            }
            WALK_EVENT0 if self.paths.is_some() => {
                self.monster_walk_event0(sim, unit);
                Some(true)
            }
            RUN_EVENT0 if self.paths.is_some() => {
                self.x.monster_run_event0(unit);
                Some(true)
            }
            _ => None,
        }
    }

    /// Walk / run start `0x005A7520` / `0x005A7550`.
    ///
    /// TODO(spec: units.md §4.6): the start bodies are not described;
    /// `umod-callbacks.md` §2 rule 2 states that the mode field holds the
    /// new mode after the start function, read as the mode set
    /// `0x00553570` (§4.1) with the requested mode, started.
    fn monster_move_start(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) -> bool {
        match crate::units::modes::set_mode(sim, self, unit, mode) {
            Ok(()) => true,
            Err(e) => {
                self.errors.push(WiringError::Unit(e));
                false
            }
        }
    }

    /// Monster walk event 0 `0x005A8490` (`pathing.md` §9.1): state 13 →
    /// `0x005C9D90`, state 22 → `0x005CE4F0` (skills spec: seams), the
    /// step (§9.3); result 2 → the mode end `0x005A8030`.
    ///
    /// TODO(spec: pathing.md §9.1): whether the two state calls' results
    /// gate the step is not stated; the step runs after them (as the
    /// player's state 13, §9.2 step 2).
    fn monster_walk_event0(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let stopped = {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut c = PathCtx::of(&mut v, sim.game);
            if c.has_state(unit, STATE_13) {
                c.state13_step(unit);
            }
            if c.has_state(unit, STATE_22) {
                c.state22_step(unit);
            }
            c.step(unit) == Some(Step::Stopped)
        };
        if stopped {
            let ended = sim.units.get(unit).map_or(0, |r| r.mode);
            self.ai_mode_end(sim, unit, ended as u8);
        }
    }

    /// The mode end `0x005A8030` (`ai.md` §1.4) on the AI store.
    fn ai_mode_end(&mut self, sim: &mut Sim<'_>, unit: UnitId, ended: u8) {
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
            ai::mode_end(sim.game, &mut cx, unit, ended);
        }
        self.ai = Some(store);
    }
}

/// Stages the AI's mode request (the record's target fields, `ai.md`
/// §7.1) for the monster mode set that follows.
pub(crate) fn stage_request<X: Pending>(h: &mut ActionHooks<X>, unit: UnitId, target: ModeTarget) {
    if let Some(p) = h.paths.as_mut() {
        p.mode_request = Some((unit, target));
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
