// Spec: specs/skills/use.md, specs/skills/levels.md, specs/combat/vitals.md
//! [`World`]: the skill and vitals seams of `d2-sim` on one game of the
//! wired sim (`d2_sim::wiring::action::ActionSim`) for one message.
//!
//! | Seam part | Provider here |
//! |---|---|
//! | frame, unit lookup, timers | [`Game`] (`tick.md` §2, §5) |
//! | stats, states, state lists, seeds, unit type / class / mode / flags | the action wiring's [`CombatView`] / [`View`] (`stats.md`, `stat-lists.md`, `units.md` §2) |
//! | items (`SkillUnits` item reads) | the action wiring's `Pending` (as for combat) |
//! | mode starts (`use.md` §4) | `d2_sim::units::modes` (`units.md` §4.1, §4.5) |
//! | player data +0x168, positions, acts, owned items of the message | the server's staged fields ([`super::Staged`]) |
//! | server messages | collected for the caller ([`World::sends`]) |
//! | everything else | [`SkillSeams`] |

use d2_sim::combat::vitals::VitalsUnits;
use d2_sim::combat::RoomKind;
use d2_sim::game::Game;
use d2_sim::rng::Seed;
use d2_sim::skills::use_::{
    MissileAim, ModeTarget, ServerMsg, SkillFunctions, UseMissiles, UseState, UseWorld,
};
use d2_sim::skills::{LearnUnits, ManaUnits, SkillEntry, SkillUnits};
use d2_sim::stats::lists::RemoveCallback;
use d2_sim::units::hooks::Sim;
use d2_sim::units::record::flags;
use d2_sim::units::{modes, UnitId, UnitType};
use d2_sim::wiring::action::combat::CombatView;
use d2_sim::wiring::action::{Pending, View, WiringError};

use super::seams::SkillSeams;
use super::Staged;
use crate::dispatch::in_range;
use crate::seams::Pos;

/// Stat-list allocation flags of the cooldown list (`use.md` §6).
pub const DELAY_LIST_FLAGS: u32 = 2;
/// Remove callback of the cooldown list (`use.md` §6).
pub const DELAY_REMOVE_CALLBACK: u32 = 0x0056_E900;

/// One message's view of the wired sim.
pub struct World<'a, X, S> {
    pub cv: CombatView<'a, X>,
    pub s: &'a mut S,
    pub staged: Staged,
    /// Server messages the pipeline sent, in order.
    pub sends: Vec<ServerMsg>,
    /// Player data +0x168 written by the point validator.
    pub point_accept: Option<i32>,
}

impl<'a, X: Pending, S: SkillSeams> World<'a, X, S> {
    pub fn new(game: &'a mut Game, v: View<'a, X>, s: &'a mut S, staged: Staged) -> Self {
        Self {
            cv: CombatView { game, v },
            s,
            staged,
            sends: Vec::new(),
            point_accept: None,
        }
    }

    fn pos(&self, u: UnitId) -> Pos {
        match self.staged.positions.iter().find(|p| p.0 == u) {
            Some(&(_, p)) => p,
            None => {
                let (x, y) = self.s.position(u);
                Pos { x, y }
            }
        }
    }

    /// Runs `f` on the [`Sim`] of the wired unit system; a unit error is
    /// recorded in the wiring's error list.
    fn with_sim<R>(
        &mut self,
        f: impl FnOnce(
            &mut Sim<'_>,
            &mut d2_sim::wiring::action::ActionHooks<X>,
        ) -> Result<R, d2_sim::units::modes::UnitError>,
    ) -> Option<R> {
        let v = &mut self.cv.v;
        let r = {
            let mut sim = Sim {
                game: &mut *self.cv.game,
                units: &mut *v.units,
                stats: &mut *v.stats,
                data: v.data,
            };
            f(&mut sim, &mut *v.h)
        };
        match r {
            Ok(r) => Some(r),
            Err(e) => {
                v.h.errors.push(WiringError::Unit(e));
                None
            }
        }
    }
}

impl<X: Pending, S: SkillSeams> SkillUnits for World<'_, X, S> {
    type Unit = UnitId;
    type Item = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        self.cv.unit_type(u)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        self.cv.class_id(u)
    }
    fn stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        SkillUnits::stat(&self.cv, u, stat, layer)
    }
    fn item_stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        self.cv.item_stat(u, stat, layer)
    }
    fn base_stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        SkillUnits::base_stat(&self.cv, u, stat, layer)
    }
    fn formula_stat(&self, u: UnitId, stat: u16, mode: i32) -> i32 {
        self.cv.formula_stat(u, stat, mode)
    }
    fn stat_entries(&self, u: UnitId, stat: u16, max: usize) -> Vec<(u16, i32)> {
        self.cv.stat_entries(u, stat, max)
    }
    fn has_state(&self, u: UnitId, state: u16) -> bool {
        SkillUnits::has_state(&self.cv, u, state)
    }
    fn state_stat(&self, u: UnitId, state: u16, stat: u16) -> Option<i32> {
        self.cv.state_stat(u, state, stat)
    }
    fn seed(&mut self, u: UnitId) -> &mut Seed {
        SkillUnits::seed(&mut self.cv, u)
    }
    fn skill_list(&self, u: UnitId) -> Vec<SkillEntry> {
        self.s.skill_list(u)
    }
    fn used_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.s.used_skill(u)
    }
    fn current_weapon(&self, u: UnitId) -> Option<UnitId> {
        self.cv.current_weapon(u)
    }
    fn weapon(&self, u: UnitId) -> Option<UnitId> {
        self.cv.weapon(u)
    }
    fn item_at(&self, u: UnitId, loc: u8) -> Option<UnitId> {
        self.cv.item_at(u, loc)
    }
    fn item_is(&self, item: UnitId, itype: i32) -> bool {
        self.cv.item_is(item, itype)
    }
    fn itype_is(&self, itype: i32, parent: i32) -> bool {
        self.cv.itype_is(itype, parent)
    }
    fn wield_type(&self, item: UnitId) -> i32 {
        self.cv.wield_type(item)
    }
    fn item_damage(&self, item: UnitId, max: bool) -> i32 {
        self.cv.item_damage(item, max)
    }
    fn str_dex_bonus(&self, item: UnitId) -> (i32, i32) {
        self.cv.str_dex_bonus(item)
    }
    fn item_flag_throw(&self, item: UnitId) -> bool {
        self.cv.item_flag_throw(item)
    }
    fn missile_level(&self, u: UnitId) -> i32 {
        self.cv.missile_level(u)
    }
}

impl<X: Pending, S: SkillSeams> ManaUnits for World<'_, X, S> {
    fn shapeshifted(&self, u: UnitId) -> bool {
        self.s.shapeshifted(u)
    }
    fn consume_charges(&mut self, u: UnitId, entry: &SkillEntry) -> bool {
        self.s.consume_charges(u, entry)
    }
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool {
        self.s.pay_life(u, cost)
    }
    /// Base set `0x00627260` on the real stat list.
    fn set_stat(&mut self, u: UnitId, stat: u16, value: i32) {
        self.cv.v.set_base(u, stat, value);
    }
}

impl<X: Pending, S: SkillSeams> SkillFunctions for World<'_, X, S> {
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32 {
        self.s.srvst(index, u, skill, lvl)
    }
    fn srvdo(
        &mut self,
        index: u16,
        u: UnitId,
        skill: i32,
        lvl: i32,
        charge: bool,
        item: bool,
        aim: bool,
    ) -> i32 {
        self.s.srvdo(index, u, skill, lvl, charge, item, aim)
    }
}

impl<X: Pending, S: SkillSeams> UseMissiles for World<'_, X, S> {
    fn create_skill_missile(
        &mut self,
        u: UnitId,
        skill: i32,
        lvl: i32,
        missile: u16,
        lob: bool,
        aim: MissileAim,
    ) {
        self.s
            .create_skill_missile(u, skill, lvl, missile, lob, aim);
    }
}

impl<X: Pending, S: SkillSeams> UseWorld for World<'_, X, S> {
    fn frame(&self) -> i32 {
        self.cv.game.frame
    }
    fn send(&mut self, _u: UnitId, msg: ServerMsg) {
        self.sends.push(msg);
    }

    // ---- staged player data and positions (`intents-events.md` §2.4)
    fn has_player_data(&self, u: UnitId) -> bool {
        u == self.staged.player && self.staged.last_accept.is_some()
    }
    fn last_point_frame(&self, u: UnitId) -> i32 {
        match self.point_accept {
            Some(f) if u == self.staged.player => f,
            _ => self
                .staged
                .last_accept
                .filter(|_| u == self.staged.player)
                .unwrap_or(0),
        }
    }
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32) {
        if u == self.staged.player {
            self.point_accept = Some(frame);
        }
    }
    fn position(&self, u: UnitId) -> (i32, i32) {
        let p = self.pos(u);
        (p.x, p.y)
    }
    fn find_unit(&self, ty: u32, guid: u32) -> Option<UnitId> {
        let ty = *UnitType::ALL.get(ty as usize)?;
        self.cv.game.lists.find_unit(ty, guid)
    }
    fn in_own_inventory(&self, u: UnitId, item: UnitId) -> bool {
        u == self.staged.player && self.staged.owned_item == Some(item)
    }
    /// The staged acts of the message's units (§2.4 rule 4); other pairs:
    /// the unit records' acts (unit +0x18).
    fn same_act(&self, a: UnitId, b: UnitId) -> bool {
        if self.staged.other_act.is_some_and(|t| t == a || t == b) {
            return false;
        }
        let staged = |u| self.staged.positions.iter().any(|p| p.0 == u);
        if staged(a) && staged(b) {
            return true;
        }
        let act = |u| self.cv.v.units.get(u).map(|r| r.act);
        act(a).is_some() && act(a) == act(b)
    }
    /// `0x00548EF0` on the positions.
    fn within_reach(&self, a: UnitId, b: UnitId) -> bool {
        in_range(self.pos(a), self.pos(b))
    }
    fn owner(&self, u: UnitId) -> Option<UnitId> {
        self.s.owner(u)
    }

    // ---- skills
    fn left_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.s.left_skill(u)
    }
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.s.right_skill(u)
    }
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.s.set_left_skill(u, e);
    }
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.s.set_right_skill(u, e);
    }
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        self.s.find_entry(u, skill)
    }
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry> {
        self.s.find_entry_owned(u, skill, owner)
    }
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        self.s.owns_skill(u, skill)
    }
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>) {
        self.s.set_used_skill(u, e);
    }
    fn used_skill_flags(&self, u: UnitId) -> u32 {
        self.s.used_skill_flags(u)
    }
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32) {
        self.s.set_used_skill_flags(u, f);
    }
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32 {
        self.s.entry_mode(u, e)
    }
    fn attack_param4(&self, u: UnitId) -> i32 {
        self.s.attack_param4(u)
    }
    fn set_attack_param4(&mut self, u: UnitId, v: i32) {
        self.s.set_attack_param4(u, v);
    }
    fn use_state(&mut self, u: UnitId, e: &SkillEntry) -> UseState {
        self.s.use_state(u, e)
    }
    fn dec_quantity(&mut self, u: UnitId, skill: i32) {
        self.s.dec_quantity(u, skill);
    }

    // ---- equipment
    fn can_dual_wield(&self, u: UnitId) -> bool {
        self.s.can_dual_wield(u)
    }
    fn equippable(&self, item: UnitId) -> bool {
        self.s.equippable(item)
    }
    fn bow_equipped(&self, u: UnitId) -> bool {
        self.s.bow_equipped(u)
    }
    fn state_mask(&self, u: UnitId, mask: u32) -> bool {
        self.s.state_mask(u, mask)
    }
    fn in_melee_range(&self, u: UnitId, target: UnitId) -> bool {
        self.s.in_melee_range(u, target)
    }

    // ---- modes
    /// Unit +0x10 of the unit record.
    fn mode(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.mode)
    }
    fn cursor_item(&self, u: UnitId) -> bool {
        self.s.cursor_item(u)
    }
    /// `0x005415A0` on the timer queue: smallest positive expire of the
    /// unit's type-1 timers, 0 if none.
    fn endanim_expire(&self, u: UnitId) -> i32 {
        let q = &self.cv.game.timers;
        q.unit_timers(u)
            .into_iter()
            .filter(|&t| q.event(t).is_some_and(|e| e.0 == 1))
            .filter_map(|t| q.expire(t))
            .filter(|&e| e > 0)
            .min()
            .unwrap_or(0)
    }
    /// A mode set with `reenter = 1` (no gates): the player mode start of
    /// `units.md` §4.5.
    // TODO(units.md §4.5): `player_start` asks the request-check hook,
    // which `reenter = 1` skips in 1.14d; the wiring's hook accepts.
    fn set_mode(&mut self, u: UnitId, mode: u32) {
        self.with_sim(|sim, h| modes::player_start(sim, h, u, mode));
    }
    /// `use.md` §4 last paragraph, in its order: set mode `0x00553570`,
    /// clear target, `0x005533D0`, delete type-0/1 timers, schedule the
    /// frame events (`units.md` §4.1 `animate`), clear flag 0x40.
    fn start_mode(&mut self, u: UnitId, mode: u32, target: ModeTarget<UnitId>) {
        // TODO(use.md §4): where `0x0057FE90` / `0x0057FEF0` store the
        // point or unit target is not stated; the target is not kept.
        let _ = target;
        if self
            .with_sim(|sim, h| modes::set_mode(sim, h, u, mode))
            .is_none()
        {
            return;
        }
        self.s.clear_target(u);
        self.with_sim(|sim, h| modes::animate(sim, h, u));
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.flags &= !flags::ATTACK_PENDING;
        }
    }
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry) {
        self.s.run_to(u, target, e);
    }
    fn target(&self, u: UnitId) -> Option<UnitId> {
        self.s.target(u)
    }
    fn clear_target(&mut self, u: UnitId) {
        self.s.clear_target(u);
    }
    /// Unit +0xC4.
    fn unit_flags(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.flags)
    }
    fn set_unit_flags(&mut self, u: UnitId, f: u32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.flags = f;
        }
    }
    fn event_arg(&self, u: UnitId) -> i32 {
        self.s.event_arg(u)
    }
    fn set_event_arg(&mut self, u: UnitId, a: i32) {
        self.s.set_event_arg(u, a);
    }
    fn step_path(&mut self, u: UnitId) -> i32 {
        self.s.step_path(u)
    }
    /// Not dead (`units.md` §2, `0x005541B0`).
    fn is_alive(&self, u: UnitId) -> bool {
        self.cv.v.units.get(u).is_some_and(|r| !r.is_dead())
    }

    // ---- relations, rooms, line of sight
    fn is_hostile(&self, a: UnitId, b: UnitId) -> bool {
        self.s.is_hostile(a, b)
    }
    fn is_pet(&self, a: UnitId, b: UnitId) -> bool {
        self.s.is_pet(a, b)
    }
    fn is_ally(&self, a: UnitId, b: UnitId) -> bool {
        self.s.is_ally(a, b)
    }
    fn room(&self, u: UnitId) -> RoomKind {
        self.s.room(u)
    }
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)> {
        self.s.target_position(u)
    }
    fn line_clear(&self, u: UnitId, to: (i32, i32), mask: u32) -> bool {
        self.s.line_clear(u, to, mask)
    }

    // ---- timers (`tick.md` §5)
    fn schedule(&mut self, u: UnitId, kind: u8, frame: i32, arg1: i32, arg2: i32) {
        let r =
            self.cv
                .game
                .schedule_event(u, u32::from(kind), frame, None, arg1 as u32, arg2 as u32);
        if let Err(e) = r {
            self.cv
                .v
                .h
                .errors
                .push(WiringError::Unit(modes::UnitError::Game(e)));
        }
    }
    fn delete_timers(&mut self, u: UnitId, kind: u8, arg1: i32) {
        self.cv
            .game
            .timers
            .cancel_unit_events(u, kind, Some(arg1 as u32));
    }

    // ---- stat lists (`stat-lists.md`)
    fn has_state_list(&self, u: UnitId, state: u16) -> bool {
        self.cv.v.state_list(u, state).is_some()
    }
    /// `use.md` §6: list (flags 2, expire, owner unit), state 121, remove
    /// callback `0x0056E900`, attach, state 121 on.
    // TODO(stat-lists.md §8.1): the attach `reset` argument is not stated
    // for this list; reset = 1 as the action wiring's state lists.
    fn create_delay_list(&mut self, u: UnitId, expire: i32) {
        let v = &mut self.cv.v;
        let Some(r) = v.units.get(u) else {
            return;
        };
        let (ty, guid) = (r.ty, r.guid);
        let state = d2_sim::skills::use_::state::SKILL_DELAY;
        let l = v
            .stats
            .alloc(DELAY_LIST_FLAGS, expire, ty.index() as u32, guid);
        v.stats.set_expire(l, expire);
        v.stats.set_state(l, u32::from(state));
        v.stats
            .set_remove_callback(l, Some(RemoveCallback(DELAY_REMOVE_CALLBACK)));
        v.stats.attach(&mut *v.h, u, l, true);
        v.set_state(u, state, true);
    }
    fn set_state_list_expiry(&mut self, u: UnitId, state: u16, expire: i32) {
        if let Some(l) = self.cv.v.state_list(u, state) {
            self.cv.v.stats.set_expire(l, expire);
        }
    }
    fn free_aura_state(&mut self, u: UnitId, state: u16) {
        self.s.free_aura_state(u, state);
    }
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32) {
        self.s.set_aura_state(u, state, skill, lvl);
    }
}

impl<X: Pending, S: SkillSeams> LearnUnits for World<'_, X, S> {
    fn is_class_skill(&self, u: UnitId, skill: i32) -> bool {
        self.s.is_class_skill(u, skill)
    }
    fn add_skill_level(&mut self, u: UnitId, skill: i32, cost: i32) {
        self.s.add_skill_level(u, skill, cost);
    }
}

impl<X: Pending, S: SkillSeams> VitalsUnits for World<'_, X, S> {
    type Unit = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        self.cv.unit_type(u)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        self.cv.class_id(u)
    }
    fn base_stat(&self, u: UnitId, stat: u16) -> i32 {
        self.cv.v.stats.unit_base(u, stat, 0)
    }
    fn stat(&self, u: UnitId, stat: u16) -> i32 {
        self.cv.v.stats.unit_total(u, stat, 0)
    }
    fn set_base_stat(&mut self, u: UnitId, stat: u16, v: i32) {
        self.cv.v.set_base(u, stat, v);
    }
    /// Unit add `0x006272B0`, layer 0.
    fn add_base_stat(&mut self, u: UnitId, stat: u16, d: i32) {
        let v = &mut self.cv.v;
        v.stats.unit_add(&mut *v.h, u, stat, d, 0);
    }
    fn max_life(&self, u: UnitId) -> i32 {
        self.cv.v.stats.max_life(u)
    }
    fn max_mana(&self, u: UnitId) -> i32 {
        self.cv.v.stats.max_mana(u)
    }
    fn max_stamina(&self, u: UnitId) -> i32 {
        self.cv.v.stats.max_stamina(u)
    }
    fn refresh(&mut self, u: UnitId) {
        self.s.refresh(u);
    }
    fn level_up_notify(&mut self, u: UnitId) {
        self.s.level_up_notify(u);
    }
    fn level_up_event(&mut self, u: UnitId) {
        self.s.level_up_event(u);
    }
}
