// Spec: specs/sim/units.md §3, §5, §6; specs/sim/stat-lists.md §4, §8, §9; specs/monsters/ai.md §1; specs/missiles/missiles.md §R3
//! The unit side of the wiring: the unit hooks of [`ActionHooks`] (the
//! missile class handler for missile events, the AI think and reset for
//! monster events 2 and 10, the state-54 rule before a think is
//! scheduled, the town test, the combat list drop, the kind frees, the
//! skill events 5 / 8 / 9 through [`Pending::skill_event`]), and
//! the unit-field helpers of [`View`] the other adapters share (stats,
//! states, state lists, seeds).

use crate::game::Game;
use crate::missiles;
use crate::monsters::ai;
use crate::rng::Seed;
use crate::stats::states::state;
use crate::stats::{ListId, StatHost};
use crate::tick::events::event;
use crate::units::hooks::{Sim, UnitHooks};
use crate::units::lifecycle::{AllocRequest, LifecycleHooks};
use crate::units::modes::UnitError;
use crate::units::record::flags2;
use crate::units::{UnitId, UnitType};

use super::combat::HIRELING_CLASSES;
use super::{ActionHooks, Pending, SkillEvent, View, WiringError};

/// Stat-list state of `justhit` (`missiles.md` §R5 step 6.1).
pub const STATE_JUSTHIT: u16 = 86;
/// State 92 (`death_delay`), cleared for players by `0x005544B0`.
pub const STATE_DEATH_DELAY: u16 = 92;

impl<X: Pending> StatHost for ActionHooks<X> {}

impl<X: Pending> UnitHooks for ActionHooks<X> {
    fn has_path(&mut self, _: &Sim<'_>, unit: UnitId) -> bool {
        self.x.has_path(unit)
    }

    /// `0x0057C980`: the unit's own entries leave its combat list
    /// (`damage.md` §3 step 3: entries are (attacker, defender) records).
    fn drop_combat_entries(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(e) = sim.game.lists.unit(unit) else {
            return;
        };
        let id = (e.ty, e.guid);
        if let Some(list) = self.combat_lists.get_mut(&unit) {
            list.retain(|c| c.attacker != id);
        }
    }

    /// `0x0061AB00` on the unit's room.
    fn room_flag(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        sim.game
            .lists
            .unit(unit)
            .and_then(|e| e.room())
            .is_some_and(|r| self.drlg.in_town(sim.game, r))
    }

    /// `0x005544B0(unit, 0)` before a think is scheduled on a monster with
    /// state 54 (`tick.md` §5.2 rule 4, `ai.md` §1.1): state 54 off, the
    /// monster's type-2 events cancelled.
    fn uninterruptable_check(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        v.set_state(unit, state::UNINTERRUPTABLE as u16, false);
        sim.game
            .timers
            .cancel_unit_events(unit, event::AI_THINK, None);
    }

    /// Event 2 `0x005B1740` (`ai.md` §2). The freeze drop of `tick.md`
    /// §5.6 already ran in the unit dispatch.
    fn ai_think(&mut self, sim: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
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
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            ai::think(sim.game, &mut cx, unit);
        }
        self.ai = Some(store);
    }

    /// Event 10 `0x005A7F70` → `0x00573120` (`ai.md` §1; monster data).
    fn ai_reset(&mut self, _: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        self.x.ai_reset(unit);
    }

    /// Event 5 `0x0056D790` (`stat-lists.md` §10.2): the skills'
    /// active-state function, through [`Pending::skill_event`].
    fn active_state(&mut self, sim: &mut Sim<'_>, unit: UnitId, f: u16, skill: u32, a2: u32) {
        X::skill_event(
            self,
            sim,
            SkillEvent::ActiveState {
                unit,
                f,
                skill,
                arg2: a2,
            },
        );
    }

    /// Event 8 `0x0056FCB0` (`use.md` §7), through [`Pending::skill_event`].
    fn periodic_skills(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        X::skill_event(
            self,
            sim,
            SkillEvent::Periodic {
                unit,
                arg1: a1,
                arg2: a2,
            },
        );
    }

    /// Event 9 `0x0056FE40` after its checks (`stat-lists.md` §10.3), through
    /// [`Pending::skill_event`].
    fn apply_item_aura(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        a1: u32,
        skill: u32,
        level: i32,
    ) {
        X::skill_event(
            self,
            sim,
            SkillEvent::ItemAura {
                unit,
                arg1: a1,
                skill,
                level,
            },
        );
    }

    /// Missile events (`0x005ADBB0`, `missiles.md` §R3).
    fn missile_do(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(mut store) = self.missiles.take() else {
            self.errors.push(WiringError::Reentrant("missiles"));
            return;
        };
        let t = self.tables.clone();
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut v,
            };
            missiles::class_handler(sim.game, &mut cx, unit);
        }
        self.missiles = Some(store);
    }
}

impl<X: Pending> LifecycleHooks for ActionHooks<X> {
    /// The per-kind state of the action modules leaves with the unit:
    /// AI control (`AiStore::remove`), missile data, combat list.
    fn free_kind(&mut self, _: &mut Sim<'_>, unit: UnitId) {
        if let Some(ai) = self.ai.as_mut() {
            ai.remove(unit);
        }
        if let Some(m) = self.missiles.as_mut() {
            m.remove(unit);
        }
        self.combat_lists.remove(&unit);
    }
}

impl<X: Pending> View<'_, X> {
    /// Records an error of a unit operation.
    pub fn unit_error(&mut self, e: UnitError) {
        self.h.errors.push(WiringError::Unit(e));
    }

    /// The unit seed (unit +0x20). A unit without a record gets a scratch
    /// seed and an error (API misuse).
    pub fn seed(&mut self, u: UnitId) -> &mut Seed {
        if self.units.get(u).is_none() {
            self.h
                .errors
                .push(WiringError::Unit(UnitError::UnknownUnit(u)));
            self.h.orphan_seed = Seed::init();
            return &mut self.h.orphan_seed;
        }
        &mut self.units.get_mut(u).expect("checked").seed
    }

    /// Unit getter `0x00625480(unit, stat, 0)`.
    pub fn stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_total(u, s, 0)
    }

    /// Unit set `0x00627260(unit, stat, value, 0)`.
    pub fn set_base(&mut self, u: UnitId, s: u16, value: i32) {
        self.stats.unit_set(&mut *self.h, u, s, value, 0);
    }

    /// Set a stat of a list (`0x006270B0`, layer 0).
    pub fn set_list_stat(&mut self, l: ListId, s: u16, value: i32) {
        self.stats.set(&mut *self.h, l, s, value, 0, None);
    }

    /// State toggle `0x00625A70` (`stat-lists.md` §9.2) with the disguise
    /// bit of unit +0xC8.
    pub fn set_state(&mut self, u: UnitId, s: u16, on: bool) {
        let t = self.stats.toggle_state(u, u32::from(s), on);
        if let (Some(d), Some(r)) = (t.disguise, self.units.get_mut(u)) {
            if d {
                r.flags2 |= flags2::DISGUISE;
            } else {
                r.flags2 &= !flags2::DISGUISE;
            }
        }
    }

    /// The unit's stat list of `state` (`0x006256B0`).
    pub fn state_list(&self, u: UnitId, s: u16) -> Option<ListId> {
        let r = self.stats.unit_list(u)?;
        self.stats.list_of_state(r, u32::from(s))
    }

    /// `stat` of the unit's list of `state` (its own base value).
    pub fn state_stat(&self, u: UnitId, s: u16, st: u16) -> Option<i32> {
        let l = self.state_list(u, s)?;
        Some(self.stats.base(l, st, 0))
    }

    /// A plain stat list for `state` attached to the unit: allocation
    /// `0x006251F0` with the owner's type and GUID, expire `0x00627440`
    /// (sets NEWLENGTH when > 0), state field, attach `0x00626E10`.
    ///
    /// TODO(stat-lists.md §4, §8.1): the callers' allocation flags and the
    /// attach `reset` argument are not stated for state lists; flags 0
    /// and reset = 1 (no DYNAMIC) are used. Without an owner the unit's
    /// own type and GUID are used.
    pub fn create_state_list(
        &mut self,
        u: UnitId,
        s: u16,
        owner: Option<(UnitType, u32)>,
        expire: i32,
    ) -> Option<ListId> {
        let (ty, guid) = match owner {
            Some(o) => o,
            None => {
                let r = self.units.get(u)?;
                (r.ty, r.guid)
            }
        };
        let l = self.stats.alloc(0, 0, ty.index() as u32, guid);
        self.stats.set_expire(l, expire);
        self.stats.set_state(l, u32::from(s));
        self.stats.attach(&mut *self.h, u, l, true);
        Some(l)
    }

    /// Hireling test `0x0063EE90`: a monster of a hireling class
    /// (`monsters/init.md` §6 step 4).
    pub fn is_hireling(&self, game: &Game, u: UnitId) -> bool {
        game.lists
            .unit(u)
            .is_some_and(|e| e.ty == UnitType::Monster)
            && self
                .units
                .get(u)
                .is_some_and(|r| HIRELING_CLASSES.contains(&r.class))
    }

    /// Unit allocation `0x00555230` (`units.md` §3.1) on the game seed,
    /// then the position (path spec, [`Pending::place`]).
    pub fn allocate(
        &mut self,
        game: &mut Game,
        req: &AllocRequest,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        let mut seed = self.h.game_seed;
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::allocate(&mut sim, &mut *self.h, &mut seed, req)
        };
        self.h.game_seed = seed;
        match r {
            Ok(Some(u)) => {
                self.h.x.place(u, x, y);
                Some(u)
            }
            Ok(None) => None,
            Err(e) => {
                self.unit_error(e);
                None
            }
        }
    }

    /// Unit removal `0x00555600` (`units.md` §3.2).
    pub fn remove(&mut self, game: &mut Game, u: UnitId) {
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::remove(&mut sim, &mut *self.h, u)
        };
        if let Err(e) = r {
            self.unit_error(e);
        }
    }

    /// A monster mode change (`units.md` §4.6, `0x005A7C20`).
    pub fn monster_set_mode(&mut self, game: &mut Game, u: UnitId, mode: u32) -> bool {
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::modes::monster_set_mode(&mut sim, &mut *self.h, u, mode)
        };
        match r {
            Ok(()) => true,
            Err(e) => {
                self.unit_error(e);
                false
            }
        }
    }
}

/// Clears state 54 and, for players, state 92 (`0x005544B0` minus the
/// timer part, `ai.md` §1.1).
pub fn clear_uninterruptable<X: Pending>(v: &mut View<'_, X>, game: &Game, u: UnitId) {
    v.set_state(u, state::UNINTERRUPTABLE as u16, false);
    if game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Player) {
        v.set_state(u, STATE_DEATH_DELAY, false);
    }
}
