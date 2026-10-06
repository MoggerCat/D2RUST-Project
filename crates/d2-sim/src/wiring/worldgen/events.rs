// Spec: specs/sim/tick.md §5.5, §5.6; specs/sim/units.md §3.2, §5, §6.2; specs/monsters/init.md §22
//! The unit hooks of [`super::WorldSim`]'s timer events: the action
//! hooks ([`ActionHooks`]) for everything they route, plus the world
//! state for what needs it.
//!
//! - Monster event 7 (`0x005A4370`) → [`init::handle_event7`]: the umod
//!   dispatcher in mode 2 on the monster's data (`init.md` §22). The
//!   unit dispatch keeps its checks (handler table, the frozen-monster
//!   drop of `tick.md` §5.6, event 7 included) before the hook runs.
//! - A unit removed while an event runs or through
//!   [`WorldSim::remove_unit`] (`units.md` §3.2, the kind free) also
//!   leaves the world state: monster data (unit +0x14), its minion
//!   list, its owner link, a superunique's pending init tail.
//!
//! Every other hook is the action hooks' own (a hook added to
//! [`UnitHooks`] must be delegated here too). The action adapters build
//! their own views over [`ActionHooks`] (missiles, AI): a unit those
//! remove goes through the action hooks' kind free only (see the
//! session notes, `docs/handoff/wire-open-seams.md`).

use crate::game::Game;
use crate::monsters::init;
use crate::stats::lists::{CallbackEvent, RemoveCallback};
use crate::stats::{ListId, StatHost, StatLists};
use crate::tick::timer::TimerRun;
use crate::units::dispatch;
use crate::units::hooks::{Sim, UnitHooks};
use crate::units::lifecycle::{self, AllocRequest, LifecycleHooks};
use crate::units::modes::MonsterModeRecord;
use crate::units::record::{AnimRecord, Sequence};
use crate::units::UnitId;

use super::super::action::{ActionHooks, View, WiringError};
use super::{WorldHost, WorldPending, WorldSim, WorldState};

/// The action hooks and the world state, as the unit hooks of one timer
/// event.
pub struct WorldHooks<'a, X> {
    pub action: &'a mut ActionHooks<X>,
    pub world: &'a mut WorldState,
}

impl WorldState {
    /// A removed unit leaves the world state: its monster data, its
    /// minion list, its owner link and a pending superunique tail.
    pub fn forget(&mut self, unit: UnitId) {
        self.monsters.remove(unit);
        self.minions.remove(&unit);
        self.owners.remove(&unit);
        self.superunique_tail.remove(&unit);
    }
}

impl<X: WorldPending> WorldSim<X> {
    /// One timer event: the unit dispatch (`units.md` §5) with
    /// [`WorldHooks`]; an error is recorded as the unit system records it.
    pub(super) fn run_world_event(&mut self, game: &mut Game, run: &TimerRun) {
        let s = &mut self.action.sys;
        let r = {
            let mut sim = Sim {
                game,
                units: &mut s.units,
                stats: &mut s.stats,
                data: &s.data,
            };
            let mut h = WorldHooks {
                action: &mut s.hooks,
                world: &mut self.world,
            };
            dispatch::dispatch(&mut sim, &mut h, run)
        };
        if let Err(e) = r {
            s.errors.push((*run, e));
        }
    }

    /// Unit removal `0x00555600` (`units.md` §3.2) with [`WorldHooks`]: the
    /// entry for a host (or the corpse and death code once specified) that
    /// removes a unit outside the action adapters.
    pub fn remove_unit(&mut self, game: &mut Game, unit: UnitId) {
        let s = &mut self.action.sys;
        let r = {
            let mut sim = Sim {
                game,
                units: &mut s.units,
                stats: &mut s.stats,
                data: &s.data,
            };
            let mut h = WorldHooks {
                action: &mut s.hooks,
                world: &mut self.world,
            };
            lifecycle::remove(&mut sim, &mut h, unit)
        };
        if let Err(e) = r {
            s.hooks.errors.push(WiringError::Unit(e));
        }
    }
}

impl<X: WorldPending> StatHost for WorldHooks<'_, X> {
    fn act_time(&self, unit: UnitId) -> Option<i32> {
        self.action.act_time(unit)
    }
    fn on_callback(&mut self, lists: &StatLists, ev: &CallbackEvent) {
        self.action.on_callback(lists, ev)
    }
    fn item_event(&mut self, lists: &mut StatLists, owner: UnitId, stat: u16, new: i32) {
        self.action.item_event(lists, owner, stat, new)
    }
    fn skill_stat_changed(
        &mut self,
        lists: &mut StatLists,
        owner: UnitId,
        key: i32,
        old: i32,
        new: i32,
    ) {
        self.action.skill_stat_changed(lists, owner, key, old, new)
    }
    fn list_removed(
        &mut self,
        lists: &mut StatLists,
        unit: UnitId,
        state: u32,
        list: ListId,
        callback: RemoveCallback,
    ) {
        self.action.list_removed(lists, unit, state, list, callback)
    }
    fn stays_on_death(&self, lists: &StatLists, unit: UnitId, state: u32) -> bool {
        self.action.stays_on_death(lists, unit, state)
    }
}

impl<X: WorldPending> UnitHooks for WorldHooks<'_, X> {
    /// Event 7 `0x005A4370` → the umod dispatcher, mode 2 (`init.md` §22).
    fn monster_umod(&mut self, sim: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        let t = self.world.tables.clone();
        let mut h = WorldHost {
            game: &mut *sim.game,
            v: View::of(
                &mut *sim.units,
                &mut *sim.stats,
                sim.data,
                &mut *self.action,
            ),
            w: &mut *self.world,
        };
        init::handle_event7(&t.init(), &mut h, unit);
    }

    fn anim_record(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<AnimRecord> {
        self.action.anim_record(sim, unit)
    }
    fn anim_rate(&mut self, sim: &Sim<'_>, unit: UnitId) -> i16 {
        self.action.anim_rate(sim, unit)
    }
    fn frame_bonus(&mut self, sim: &Sim<'_>, unit: UnitId) -> i32 {
        self.action.frame_bonus(sim, unit)
    }
    fn load_sequence(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<Sequence> {
        self.action.load_sequence(sim, unit)
    }
    fn has_path(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        self.action.has_path(sim, unit)
    }
    fn reinit_anim(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.reinit_anim(sim, unit)
    }
    fn drop_combat_entries(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.drop_combat_entries(sim, unit)
    }
    fn room_flag(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        self.action.room_flag(sim, unit)
    }
    fn player_request_check(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) -> bool {
        self.action.player_request_check(sim, unit, mode)
    }
    fn player_death(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.player_death(sim, unit)
    }
    fn player_corpse(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.player_corpse(sim, unit)
    }
    fn player_knockback_path(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.player_knockback_path(sim, unit)
    }
    fn player_skill_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.player_skill_start(sim, unit)
    }
    fn player_movement_step(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        self.action.player_movement_step(sim, unit, a1, a2)
    }
    fn player_action_frame(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        self.action.player_action_frame(sim, unit, a1, a2)
    }
    fn player_item_row_flagged(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        self.action.player_item_row_flagged(sim, unit)
    }
    fn player_attack_cleanup(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.player_attack_cleanup(sim, unit)
    }
    fn player_refresh(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.player_refresh(sim, unit)
    }
    fn update_trade(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        self.action.update_trade(sim, unit, a1, a2)
    }
    fn monster_mode_bookkeeping(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {
        self.action.monster_mode_bookkeeping(sim, unit, mode)
    }
    fn monster_class_record(
        &mut self,
        sim: &Sim<'_>,
        unit: UnitId,
        mode: u32,
    ) -> Option<MonsterModeRecord> {
        self.action.monster_class_record(sim, unit, mode)
    }
    fn monster_mode_function(&mut self, sim: &mut Sim<'_>, unit: UnitId, address: u32) -> bool {
        self.action.monster_mode_function(sim, unit, address)
    }
    fn uninterruptable_check(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.uninterruptable_check(sim, unit)
    }
    fn ai_think(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        self.action.ai_think(sim, unit, a1, a2)
    }
    fn ai_reset(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        self.action.ai_reset(sim, unit, a1, a2)
    }
    fn monster_death(&mut self, sim: &mut Sim<'_>, unit: UnitId, killer: Option<UnitId>) {
        self.action.monster_death(sim, unit, killer)
    }
    fn active_state(&mut self, sim: &mut Sim<'_>, unit: UnitId, f: u16, skill: u32, a2: u32) {
        self.action.active_state(sim, unit, f, skill, a2)
    }
    fn periodic_skills(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        self.action.periodic_skills(sim, unit, a1, a2)
    }
    fn apply_item_aura(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        a1: u32,
        skill: u32,
        level: i32,
    ) {
        self.action.apply_item_aura(sim, unit, a1, skill, level)
    }
    fn cooldown_end(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        self.action.cooldown_end(sim, unit, a1, a2)
    }
    fn missile_do(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.missile_do(sim, unit)
    }
    fn object_event(&mut self, sim: &mut Sim<'_>, unit: UnitId, event: u8) {
        self.action.object_event(sim, unit, event)
    }
    fn item_replenish(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.item_replenish(sim, unit)
    }
    fn send_life_fraction(&mut self, sim: &mut Sim<'_>, unit: UnitId, fraction: i32) {
        self.action.send_life_fraction(sim, unit, fraction)
    }
    fn free_hover(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.free_hover(sim, unit)
    }
}

impl<X: WorldPending> LifecycleHooks for WorldHooks<'_, X> {
    fn init_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId, req: &AllocRequest) {
        self.action.init_kind(sim, unit, req)
    }
    /// The action state leaves first (AI control, missile data, combat
    /// list), then the world state ([`WorldState::forget`]).
    fn free_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.action.free_kind(sim, unit);
        self.world.forget(unit);
    }
}
