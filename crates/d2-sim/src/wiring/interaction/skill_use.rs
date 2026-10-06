// Spec: specs/skills/use.md §1–§7; specs/skills/bodies.md (BodyWorld); specs/missiles/missiles.md §R2; specs/sim/units.md §4.1; specs/sim/tick.md §5.2–§5.4; specs/sim/stat-lists.md §4, §8.1, §9.2
//! Skill use → missiles, combat and the timers: the seams of
//! [`crate::skills::use_`] on the action wiring's providers.
//!
//! [`UseView`] wraps the action wiring's
//! [`crate::wiring::action::combat::CombatView`] (game, unit records,
//! stat lists, shared action state), so [`SkillUnits`] is combat's.
//! Real here: the frame, GUID lookups, the act test, unit modes (the
//! plain mode set `0x00553570`), unit flags, the alive test, the ENDANIM
//! expire, hostility and melee range (the action wiring's seams for the
//! same addresses), the room kind, timer scheduling and deletion, the
//! skill-delay and aura state lists, stat writes, and missile creation
//! through [`crate::missiles::create_missile`] on the real missile store.
//! The skill bodies of `functions.tsv` status `spec'd-here` run on this
//! view ([`BodyWorld`], below): real stat lists, states, timers, rooms,
//! the combat code and the handler lists of [`ActionHooks`]; the parts
//! of other unwritten systems through [`Pending`]'s skill-body seams.
//! The skill list, player data, paths, items and the other per-skill
//! bodies have no provider: [`UseRest`], implemented by the same
//! [`Pending`] value as the action wiring's other open seams (one owner
//! for the skill list, which combat reads through
//! [`Pending::skill_list`]).

use crate::combat::{CombatWorld, RoomKind};
use crate::game::Game;
use crate::missiles::{self, MissileParams};
use crate::rng::Seed;
use crate::skills::use_::bodies::{self, BodyWorld};
use crate::skills::use_::{
    MissileAim, ModeTarget, ServerMsg, SkillFunctions, UseMissiles, UseState, UseWorld,
};
use crate::skills::{ManaUnits, SkillEntry, SkillUnits};
use crate::stats::lists::{ListId, RemoveCallback};
use crate::tick::events::event;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::combat::CombatView;
use crate::wiring::action::{ActionSim, Pending, View, WiringError};

/// `skilldelay` (state 121, `use.md` §6).
const STATE_SKILL_DELAY: u16 = 121;
/// List flags of the delay list (`use.md` §6: "flags 2").
const DELAY_LIST_FLAGS: u32 = 2;
/// The delay list's remove callback `0x0056E900` (an opaque id; the
/// stat host gives it its meaning, `stat-lists.md` §4).
pub const DELAY_REMOVE_CALLBACK: RemoveCallback = RemoveCallback(0x0056_E900);

/// The skill use calls no written spec provides yet. Implemented by the
/// action wiring's [`Pending`] provider (the skill list's one owner).
pub trait UseRest {
    // ---- messages (d2-server)
    fn send(&mut self, u: UnitId, msg: ServerMsg);
    // ---- player data (player spec)
    fn has_player_data(&self, u: UnitId) -> bool;
    fn last_point_frame(&self, u: UnitId) -> i32;
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32);
    fn cursor_item(&self, u: UnitId) -> bool;
    // ---- relations, reach (not specified)
    fn in_own_inventory(&self, u: UnitId, item: UnitId) -> bool;
    fn within_reach(&self, a: UnitId, b: UnitId) -> bool;
    fn owner(&self, u: UnitId) -> Option<UnitId>;
    fn is_pet(&self, a: UnitId, b: UnitId) -> bool;
    fn is_ally(&self, a: UnitId, b: UnitId) -> bool;
    // ---- the unit's skill list (units / skills; `use.md` §2)
    fn left_skill(&self, u: UnitId) -> Option<SkillEntry>;
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry>;
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry);
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry);
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry>;
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry>;
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool;
    /// The entry [`Pending::used_skill`] then returns.
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>);
    fn used_skill_flags(&self, u: UnitId) -> u32;
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32);
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32;
    fn attack_param4(&self, u: UnitId) -> i32;
    fn set_attack_param4(&mut self, u: UnitId, v: i32);
    /// `0x00647960` (`use.md` §2: parts listed, order not).
    fn use_state(&mut self, u: UnitId, e: &SkillEntry) -> UseState;
    // ---- mana (`skills/levels.md` §4)
    fn shapeshifted(&self, u: UnitId) -> bool;
    fn consume_charges(&mut self, u: UnitId, e: &SkillEntry) -> bool;
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool;
    // ---- equipment (items, inventory)
    fn can_dual_wield(&self, u: UnitId) -> bool;
    fn equippable(&self, item: UnitId) -> bool;
    fn bow_equipped(&self, u: UnitId) -> bool;
    fn state_mask(&self, u: UnitId, mask: u32) -> bool;
    // ---- modes and paths (units.md player modes, path spec)
    fn start_mode(&mut self, game: &mut Game, u: UnitId, mode: u32, target: ModeTarget<UnitId>);
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry);
    fn target(&self, u: UnitId) -> Option<UnitId>;
    fn clear_target(&mut self, u: UnitId);
    fn event_arg(&self, u: UnitId) -> i32;
    fn set_event_arg(&mut self, u: UnitId, a: i32);
    fn step_path(&mut self, u: UnitId) -> i32;
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)>;
    fn line_clear(&self, u: UnitId, to: (i32, i32), mask: u32) -> bool;
    // ---- aura state (`use.md` §7: list contents not written)
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32);
    // ---- skill code (the slots `bodies` does not specify)
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32;
    #[allow(clippy::too_many_arguments)]
    fn srvdo(
        &mut self,
        index: u16,
        u: UnitId,
        skill: i32,
        lvl: i32,
        charge: bool,
        item: bool,
        aim: bool,
    ) -> i32;
}

/// The skill use pipeline's world: combat's view plus the skill use
/// seams.
pub struct UseView<'a, X> {
    pub cv: CombatView<'a, X>,
}

impl<X: Pending + UseRest> ActionSim<X> {
    /// Runs `f` with the skill use pipeline's view (message handlers,
    /// the do / periodic event handlers, tests).
    pub fn skill_use<R>(&mut self, game: &mut Game, f: impl FnOnce(&mut UseView<'_, X>) -> R) -> R {
        let s = &mut self.sys;
        let mut w = UseView {
            cv: CombatView {
                game,
                v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
            },
        };
        f(&mut w)
    }
}

impl<X: Pending + UseRest> UseView<'_, X> {
    fn x(&self) -> &X {
        &self.cv.v.h.x
    }
    fn xm(&mut self) -> &mut X {
        &mut self.cv.v.h.x
    }
    fn error(&mut self, e: WiringError) {
        self.cv.v.h.errors.push(e);
    }
}

// ---- SkillUnits: combat's ------------------------------------------------

impl<X: Pending + UseRest> SkillUnits for UseView<'_, X> {
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
        SkillUnits::item_stat(&self.cv, u, stat, layer)
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
        SkillUnits::state_stat(&self.cv, u, state, stat)
    }
    fn seed(&mut self, u: UnitId) -> &mut Seed {
        SkillUnits::seed(&mut self.cv, u)
    }
    fn skill_list(&self, u: UnitId) -> Vec<SkillEntry> {
        self.cv.skill_list(u)
    }
    fn used_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.cv.used_skill(u)
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

impl<X: Pending + UseRest> ManaUnits for UseView<'_, X> {
    fn shapeshifted(&self, u: UnitId) -> bool {
        self.x().shapeshifted(u)
    }
    fn consume_charges(&mut self, u: UnitId, entry: &SkillEntry) -> bool {
        self.xm().consume_charges(u, entry)
    }
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool {
        self.xm().pay_life(u, cost)
    }
    /// Unit set `0x00627260(unit, stat, value, 0)`.
    fn set_stat(&mut self, u: UnitId, stat: u16, value: i32) {
        self.cv.v.set_base(u, stat, value);
    }
}

/// The bodies of `functions.tsv` status `spec'd-here` run here
/// ([`bodies::run_start`] / [`bodies::run_do`] on this view); every other
/// slot goes to [`UseRest::srvst`] / [`UseRest::srvdo`].
impl<X: Pending + UseRest> SkillFunctions for UseView<'_, X> {
    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32 {
        let t = self.cv.v.h.tables.clone();
        match bodies::run_start(self, &t.skills, &t.combat, index, u, skill, lvl) {
            Some(v) => v,
            None => self.xm().srvst(index, u, skill, lvl),
        }
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
        let t = self.cv.v.h.tables.clone();
        match bodies::run_do(self, &t.skills, &t.combat, index, u, skill, lvl) {
            Some(v) => v,
            None => self.xm().srvdo(index, u, skill, lvl, charge, item, aim),
        }
    }
}

impl<X: Pending + UseRest> UseMissiles for UseView<'_, X> {
    /// The `srvmissile` path (`bodies.md` §5): `skill_missile`
    /// (`0x0056EE90` for `lob`, else `0x0056ECB0`, §2.4) with `quant` = 0,
    /// on the real missile store ([`BodyWorld::spawn_missile`]).
    fn create_skill_missile(
        &mut self,
        u: UnitId,
        skill: i32,
        lvl: i32,
        missile: u16,
        lob: bool,
        aim: MissileAim,
    ) {
        let (d, at) = match aim {
            MissileAim::None => ((0, 0), (0, 0)),
            MissileAim::At { offset, aim } => (offset, aim),
        };
        bodies::skill_missile(self, i32::from(missile), u, skill, lvl, d, at, false, lob);
    }
}

impl<X: Pending + UseRest> UseWorld for UseView<'_, X> {
    fn frame(&self) -> i32 {
        self.cv.game.frame
    }
    fn send(&mut self, u: UnitId, msg: ServerMsg) {
        UseRest::send(self.xm(), u, msg);
    }
    fn has_player_data(&self, u: UnitId) -> bool {
        self.x().has_player_data(u)
    }
    fn last_point_frame(&self, u: UnitId) -> i32 {
        self.x().last_point_frame(u)
    }
    fn set_last_point_frame(&mut self, u: UnitId, frame: i32) {
        self.xm().set_last_point_frame(u, frame);
    }
    /// The path position (`ActionHooks::path_position`: the path
    /// provider's, else [`Pending::position`]).
    fn position(&self, u: UnitId) -> (i32, i32) {
        self.cv.v.h.path_position(u)
    }
    /// `0x00552F60` on the type's hash (`unit-order.md` §2.3).
    fn find_unit(&self, ty: u32, guid: u32) -> Option<UnitId> {
        let ty = *UnitType::ALL.get(usize::try_from(ty).ok()?)?;
        self.cv.game.lists.find_unit(ty, guid)
    }
    fn in_own_inventory(&self, u: UnitId, item: UnitId) -> bool {
        self.x().in_own_inventory(u, item)
    }
    /// The act fields (unit +0x18) are equal.
    fn same_act(&self, a: UnitId, b: UnitId) -> bool {
        let act = |u| self.cv.v.units.get(u).map(|r| r.act);
        act(a).is_some() && act(a) == act(b)
    }
    fn within_reach(&self, a: UnitId, b: UnitId) -> bool {
        self.x().within_reach(a, b)
    }
    fn owner(&self, u: UnitId) -> Option<UnitId> {
        self.x().owner(u)
    }

    fn left_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.x().left_skill(u)
    }
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.x().right_skill(u)
    }
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.xm().set_left_skill(u, e);
    }
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry) {
        self.xm().set_right_skill(u, e);
    }
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        self.x().find_entry(u, skill)
    }
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry> {
        self.x().find_entry_owned(u, skill, owner)
    }
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        self.x().owns_skill(u, skill)
    }
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>) {
        self.xm().set_used_skill(u, e);
    }
    fn used_skill_flags(&self, u: UnitId) -> u32 {
        self.x().used_skill_flags(u)
    }
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32) {
        self.xm().set_used_skill_flags(u, f);
    }
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32 {
        self.x().entry_mode(u, e)
    }
    fn attack_param4(&self, u: UnitId) -> i32 {
        self.x().attack_param4(u)
    }
    fn set_attack_param4(&mut self, u: UnitId, v: i32) {
        self.xm().set_attack_param4(u, v);
    }
    fn use_state(&mut self, u: UnitId, e: &SkillEntry) -> UseState {
        self.xm().use_state(u, e)
    }
    /// `0x0056C3F0` (`bodies.md` §2.5).
    fn dec_quantity(&mut self, u: UnitId, _skill: i32) {
        bodies::dec_quantity(self, u);
    }

    fn can_dual_wield(&self, u: UnitId) -> bool {
        self.x().can_dual_wield(u)
    }
    fn equippable(&self, item: UnitId) -> bool {
        self.x().equippable(item)
    }
    fn bow_equipped(&self, u: UnitId) -> bool {
        self.x().bow_equipped(u)
    }
    fn state_mask(&self, u: UnitId, mask: u32) -> bool {
        self.x().state_mask(u, mask)
    }
    /// `0x00622C40(a, d, 0x00622870(a))` (the action wiring's seams).
    fn in_melee_range(&self, u: UnitId, target: UnitId) -> bool {
        let x = self.x();
        x.in_melee_range(u, target, x.melee_range(u))
    }

    /// Unit +0x10.
    fn mode(&self, u: UnitId) -> u32 {
        self.cv.v.units.get(u).map_or(0, |r| r.mode)
    }
    fn cursor_item(&self, u: UnitId) -> bool {
        self.x().cursor_item(u)
    }
    /// `0x005415A0`: the smallest positive expire frame of the unit's
    /// type-1 timers (`tick.md` §5), 0 if none.
    fn endanim_expire(&self, u: UnitId) -> i32 {
        let t = &self.cv.game.timers;
        t.unit_timers(u)
            .into_iter()
            .filter(|&id| t.event(id).is_some_and(|e| e.0 == event::END_ANIM))
            .filter_map(|id| t.expire(id))
            .filter(|&e| e > 0)
            .min()
            .unwrap_or(0)
    }
    /// The plain mode set `0x00553570` (`units.md` §4.1).
    fn set_mode(&mut self, u: UnitId, mode: u32) {
        let r = {
            let v = &mut self.cv.v;
            let mut sim = crate::units::hooks::Sim {
                game: self.cv.game,
                units: v.units,
                stats: v.stats,
                data: v.data,
            };
            crate::units::modes::set_mode(&mut sim, &mut *v.h, u, mode)
        };
        if let Err(e) = r {
            self.error(WiringError::Unit(e));
        }
    }
    fn start_mode(&mut self, u: UnitId, mode: u32, target: ModeTarget<UnitId>) {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.start_mode(game, u, mode, target);
    }
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry) {
        self.xm().run_to(u, target, e);
    }
    fn target(&self, u: UnitId) -> Option<UnitId> {
        UseRest::target(self.x(), u)
    }
    fn clear_target(&mut self, u: UnitId) {
        self.xm().clear_target(u);
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
        self.x().event_arg(u)
    }
    fn set_event_arg(&mut self, u: UnitId, a: i32) {
        self.xm().set_event_arg(u, a);
    }
    fn step_path(&mut self, u: UnitId) -> i32 {
        self.xm().step_path(u)
    }
    /// `0x005541B0` on the unit record.
    fn is_alive(&self, u: UnitId) -> bool {
        !self.cv.v.units.is_dead(u)
    }

    /// `0x00554200` (the action wiring's [`Pending::may_attack`]).
    fn is_hostile(&self, a: UnitId, b: UnitId) -> bool {
        self.x().may_attack(a, b)
    }
    fn is_pet(&self, a: UnitId, b: UnitId) -> bool {
        self.x().is_pet(a, b)
    }
    fn is_ally(&self, a: UnitId, b: UnitId) -> bool {
        self.x().is_ally(a, b)
    }
    /// Combat's room kind (`0x00620BB0`, `0x0061AB00`).
    fn room(&self, u: UnitId) -> RoomKind {
        CombatWorld::room(&self.cv, u)
    }
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)> {
        self.x().target_position(u)
    }
    fn line_clear(&self, u: UnitId, to: (i32, i32), mask: u32) -> bool {
        self.x().line_clear(u, to, mask)
    }

    /// `0x005416B0` (`tick.md` §5.2).
    fn schedule(&mut self, u: UnitId, kind: u8, frame: i32, arg1: i32, arg2: i32) {
        let r =
            self.cv
                .game
                .schedule_event(u, u32::from(kind), frame, None, arg1 as u32, arg2 as u32);
        if let Err(e) = r {
            self.error(WiringError::Unit(e.into()));
        }
    }
    /// `tick.md` §5.4: the unit's timers of `kind` with that arg1.
    fn delete_timers(&mut self, u: UnitId, kind: u8, arg1: i32) {
        self.cv
            .game
            .timers
            .cancel_unit_events(u, kind, Some(arg1 as u32));
    }

    fn has_state_list(&self, u: UnitId, state: u16) -> bool {
        self.cv.v.state_list(u, state).is_some()
    }
    /// `use.md` §6: a list with flags 2, expire `e`, the unit as owner,
    /// state 121, remove callback `0x0056E900`; attached; state 121 on.
    ///
    /// TODO(use.md §6, stat-lists.md §8.1): the attach `reset` argument is
    /// not stated; reset = 1 as the action wiring's state lists.
    fn create_delay_list(&mut self, u: UnitId, expire: i32) {
        let Some((ty, guid)) = self.cv.v.units.get(u).map(|r| (r.ty, r.guid)) else {
            return;
        };
        let v = &mut self.cv.v;
        let l = v
            .stats
            .alloc(DELAY_LIST_FLAGS, expire, ty.index() as u32, guid);
        v.stats.set_expire(l, expire);
        v.stats.set_state(l, u32::from(STATE_SKILL_DELAY));
        v.stats.set_remove_callback(l, Some(DELAY_REMOVE_CALLBACK));
        v.stats.attach(&mut *v.h, u, l, true);
        v.set_state(u, STATE_SKILL_DELAY, true);
    }
    fn set_state_list_expiry(&mut self, u: UnitId, state: u16, expire: i32) {
        if let Some(l) = self.cv.v.state_list(u, state) {
            self.cv.v.stats.set_expire(l, expire);
        }
    }
    /// The aura state's list freed (`stat-lists.md` §8) and the state off
    /// (§9.2).
    fn free_aura_state(&mut self, u: UnitId, state: u16) {
        let v = &mut self.cv.v;
        v.stats.free_state_list(&mut *v.h, u, u32::from(state));
        v.set_state(u, state, false);
    }
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32) {
        self.xm().set_aura_state(u, state, skill, lvl);
    }
}

// ---- BodyWorld: the skill bodies on the wired units ----------------------

impl<X: Pending + UseRest> UseView<'_, X> {
    fn body_tables(&self) -> Option<&bodies::BodyTables> {
        self.cv.v.h.bodies.as_deref()
    }
}

impl<'a, X: Pending + UseRest> BodyWorld for UseView<'a, X> {
    type List = ListId;
    type Combat = CombatView<'a, X>;

    fn combat(&mut self) -> &mut CombatView<'a, X> {
        &mut self.cv
    }

    fn stat_info(&self, s: i32) -> Option<bodies::BodyStat> {
        self.body_tables()?.stat(s)
    }
    fn state_count(&self) -> i32 {
        i32::try_from(self.cv.v.stats.data().states.count()).unwrap_or(i32::MAX)
    }
    fn state_flag(&self, s: i32, g: usize) -> bool {
        u32::try_from(s).is_ok_and(|s| self.cv.v.stats.data().states.has_flag(s, g))
    }
    fn state_group(&self, s: i32) -> i32 {
        let i = usize::try_from(s).ok();
        self.body_tables()
            .and_then(|b| b.state_group.get(i?).copied())
            .unwrap_or(0)
    }
    fn state_is_aura(&self, s: i32) -> bool {
        let i = usize::try_from(s).ok();
        self.body_tables()
            .and_then(|b| b.state_aura.get(i?).copied())
            .unwrap_or(false)
    }
    fn overlay_count(&self) -> i32 {
        self.body_tables().map_or(0, |b| b.overlay_count)
    }

    fn has_group(&self, u: UnitId, g: usize) -> bool {
        self.cv.v.stats.has_group(u, g)
    }
    /// `0x00639DB0`: the toggle (`stat-lists.md` §9.2) and the unit
    /// queued for update.
    fn state_on(&mut self, u: UnitId, s: i32, on: bool) {
        let Ok(s) = u16::try_from(s) else { return };
        self.cv.v.set_state(u, s, on);
        BodyWorld::queue_update(self, u);
    }
    fn mark_state_changed(&mut self, u: UnitId, s: i32) {
        if let Ok(s) = u32::try_from(s) {
            self.cv.v.stats.set_state_changed(u, s, true);
        }
    }
    fn clear_group_states(&mut self, u: UnitId, g: usize) {
        for s in 0..self.state_count() {
            let s32 = s as u32;
            if self.cv.v.stats.data().states.has_flag(s32, g) && self.cv.v.stats.has_state(u, s32) {
                self.cv.v.stats.set_state_changed(u, s32, true);
                self.cv.v.set_state(u, s as u16, false);
            }
        }
        BodyWorld::queue_update(self, u);
    }
    /// `0x0064C040` (`unit-order.md` §6.2).
    fn queue_update(&mut self, u: UnitId) {
        if self.cv.game.lists.queue_update(u).is_err() {
            self.error(WiringError::Unit(
                crate::units::modes::UnitError::UnknownUnit(u),
            ));
        }
    }
    /// The stat host's `0x0063A4A0` (`stat-lists.md` §8.8).
    fn stays_on_death(&self, u: UnitId, s: i32) -> bool {
        use crate::stats::lists::StatHost;
        let v = &self.cv.v;
        v.h.stays_on_death(v.stats, u, s as u32)
    }

    fn state_list(&self, u: UnitId, s: i32) -> Option<ListId> {
        self.cv.v.state_list(u, u16::try_from(s).ok()?)
    }
    /// The unit's list by flags (`0x006256E0` on the unit's list).
    fn first_list_with_flags(&self, u: UnitId, flags: u32) -> Option<ListId> {
        let st = &self.cv.v.stats;
        st.list_by_flags(st.unit_list(u)?, flags)
    }
    fn alloc_list(&mut self, flags: u32, expire: i32, owner: Option<UnitId>) -> Option<ListId> {
        let (ty, guid) = match owner {
            Some(o) => {
                let r = self.cv.v.units.get(o)?;
                (r.ty.index() as u32, r.guid)
            }
            None => (6, u32::MAX),
        };
        Some(self.cv.v.stats.alloc(flags, expire, ty, guid))
    }
    fn list_state(&self, l: ListId) -> i32 {
        self.cv.v.stats.state(l) as i32
    }
    fn set_list_state(&mut self, l: ListId, s: i32) {
        self.cv.v.stats.set_state(l, s as u32);
    }
    fn list_skill(&self, l: ListId) -> (i32, i32) {
        let (s, v) = self.cv.v.stats.skill(l);
        (s as i32, v as i32)
    }
    fn set_list_skill(&mut self, l: ListId, skill: i32, lvl: i32) {
        self.cv.v.stats.set_skill(l, skill as u32, lvl as u32);
    }
    fn list_expire(&self, l: ListId) -> i32 {
        self.cv.v.stats.expire(l)
    }
    fn set_list_expire(&mut self, l: ListId, e: i32) {
        self.cv.v.stats.set_expire(l, e);
    }
    fn list_get(&self, l: ListId, s: i32) -> i32 {
        u16::try_from(s).map_or(0, |s| self.cv.v.stats.base(l, s, 0))
    }
    fn list_set(&mut self, l: ListId, s: i32, v: i32) {
        if let Ok(s) = u16::try_from(s) {
            self.cv.v.set_list_stat(l, s, v);
        }
    }
    fn attach(&mut self, u: UnitId, l: ListId) {
        let v = &mut self.cv.v;
        v.stats.attach(&mut *v.h, u, l, true);
    }
    fn set_remove_callback(&mut self, l: ListId, cb: u32) {
        self.cv
            .v
            .stats
            .set_remove_callback(l, Some(RemoveCallback(cb)));
    }
    /// Detach (`stat-lists.md` §8.2), the list's remove callback as the
    /// bodies give it ([`bodies::remove_callback`]; the stat host runs
    /// none), then free (§8.3).
    fn detach_free(&mut self, u: UnitId, l: ListId) {
        let st = self.cv.v.stats.state(l) as i32;
        let cb = self.cv.v.stats.remove_callback(l);
        {
            let v = &mut self.cv.v;
            v.stats.detach(&mut *v.h, l);
        }
        if let Some(cb) = cb {
            let t = self.cv.v.h.tables.clone();
            bodies::remove_callback(self, &t.skills, u, st, cb.0);
        }
        let v = &mut self.cv.v;
        v.stats.free_plain(&mut *v.h, l);
    }

    fn add_handler(&mut self, u: UnitId, h: bodies::Handler) {
        self.cv.v.h.handlers.entry(u).or_default().insert(0, h);
    }
    /// TODO(bodies.md OQ6): the deferred free of a running record (flags
    /// bit 0 → flags |= 2) needs the handler iteration, which has no
    /// provider; records are unlinked at once.
    fn remove_handlers(&mut self, u: UnitId, key_type: i32, key: i32) {
        if let Some(v) = self.cv.v.h.handlers.get_mut(&u) {
            v.retain(|h| !(h.key_type == key_type && h.key == key));
        }
    }

    /// The adjacency array (`drlg/rooms.md` §6) of the source's room, or
    /// of the room containing `at` (`0x00463740`); each room's town test
    /// and unit list.
    fn scan_rooms(
        &self,
        source: UnitId,
        at: Option<(i32, i32)>,
    ) -> Option<Vec<bodies::ScanRoom<UnitId>>> {
        let game = &*self.cv.game;
        let drlg = &self.cv.v.h.drlg;
        let mut room = game.lists.unit(source)?.room()?;
        if let Some((x, y)) = at {
            room = drlg.find_room(game, room, x, y)?;
        }
        let adjacent = game.lists.room(room)?.adjacent.clone();
        Some(
            adjacent
                .into_iter()
                .map(|r| bodies::ScanRoom {
                    town: drlg.in_town(game, r),
                    units: game.lists.room_units(r),
                })
                .collect(),
        )
    }
    fn allied(&self, a: UnitId, b: UnitId) -> bool {
        self.x().allied(a, b)
    }
    /// The missile store's owner (`0x00552FD0`).
    fn missile_owner(&self, u: UnitId) -> Option<UnitId> {
        let o = self.cv.v.h.missiles.as_ref()?.get(u)?.owner?;
        self.cv.game.lists.find_unit(o.ty, o.guid)
    }
    fn minion_owner(&self, u: UnitId) -> Option<UnitId> {
        self.x().minion_owner(u)
    }
    fn pet_unsummonable(&self, u: UnitId, pet: UnitId) -> bool {
        self.x().pet_unsummonable(u, pet)
    }

    fn frame_bonus(&self, u: UnitId) -> i32 {
        self.x().frame_bonus(u)
    }
    fn set_anim_frame(&mut self, u: UnitId, v: i32) {
        if let Some(r) = self.cv.v.units.get_mut(u) {
            r.anim.frame = v;
        }
    }
    fn set_entry_param(&mut self, u: UnitId, i: u8, v: i32) {
        self.xm().set_entry_param(u, i, v);
    }
    fn stat_max(&self, u: UnitId, s: u16) -> i32 {
        let st = &self.cv.v.stats;
        match s {
            6 => st.max_life(u),
            8 => st.max_mana(u),
            _ => st.max_stamina(u),
        }
    }

    fn composit_weapon_class(&self, u: UnitId) -> i32 {
        self.x().composit_weapon_class(u)
    }
    fn hand_class(&self, u: UnitId) -> i32 {
        self.x().hand_class(u)
    }
    fn item_shoots(&self, item: UnitId) -> bool {
        self.x().item_shoots(item)
    }
    fn item_stackable(&self, item: UnitId) -> bool {
        self.x().item_stackable(item)
    }
    fn item_stat_of(&self, item: UnitId, s: u16) -> i32 {
        self.cv.v.stats.unit_total(item, s, 0)
    }
    fn set_item_stat(&mut self, item: UnitId, s: u16, v: i32) {
        self.cv.v.set_base(item, s, v);
    }
    fn item_max_stack(&self, item: UnitId) -> i32 {
        self.x().item_max_stack(item)
    }
    /// [`Pending::item_max_durability`]; unknown → the current
    /// durability (nothing is changed).
    fn item_max_durability(&self, item: UnitId) -> i32 {
        self.x()
            .item_max_durability(item)
            .unwrap_or_else(|| self.cv.v.stats.unit_total(item, 72, 0))
    }
    fn quantity_timer(&mut self, item: UnitId) {
        let game = &mut *self.cv.game;
        self.cv.v.h.x.quantity_timer(game, item);
    }
    fn send_item_stat(&mut self, u: UnitId, item: UnitId, s: u16, v: i32) {
        self.xm().send_item_stat(u, item, s, v);
    }
    fn attack_cleanup(&mut self, u: UnitId) {
        self.xm().attack_cleanup(u);
    }
    fn weapon_cleanup(&mut self, u: UnitId) {
        self.xm().weapon_cleanup(u);
    }

    /// `0x0059FA30` (`missiles.md` §R2.3) on the real missile store.
    fn spawn_missile(&mut self, req: bodies::MissileRequest<UnitId>) -> bool {
        let p = MissileParams {
            flags: req.flags,
            owner: Some(req.owner),
            origin: req.origin,
            class: req.class,
            x: req.x,
            y: req.y,
            target_x: req.target_x,
            target_y: req.target_y,
            skill: req.skill,
            level: req.level,
            attack_bonus: req.attack_bonus,
            ..MissileParams::default()
        };
        let Some(mut store) = self.cv.v.h.missiles.take() else {
            self.error(WiringError::Reentrant("missiles"));
            return false;
        };
        let t = self.cv.v.h.tables.clone();
        let made = {
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut self.cv.v,
            };
            missiles::create_missile(self.cv.game, &mut cx, &p)
        };
        self.cv.v.h.missiles = Some(store);
        made.is_some()
    }
    fn passive_refresh(&mut self, u: UnitId) {
        self.xm().passive_refresh(u);
    }
    fn buff_refresh(&mut self, u: UnitId) {
        self.xm().buff_refresh(u);
    }
    fn skill_resync(&mut self, u: UnitId) {
        self.xm().skill_resync(u);
    }
    fn passive_state_apply(&mut self, u: UnitId, e: &SkillEntry) {
        self.xm().passive_state_apply(u, e);
    }
    fn set_ai_state(&mut self, u: UnitId, k: i32) {
        self.xm().set_ai_state(u, k);
    }
    fn blood_mana(&mut self, u: UnitId, cost: i32) {
        self.xm().blood_mana(u, cost);
    }
    fn queue_progressive(&mut self, u: UnitId, msg: bodies::ProgressiveMsg<UnitId>) {
        self.xm().queue_progressive(u, msg);
    }
}
