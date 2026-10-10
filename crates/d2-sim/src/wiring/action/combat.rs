// Spec: specs/combat/hit.md, specs/combat/damage.md, specs/skills/levels.md, specs/monsters/init.md §22 (seams `CombatWorld`, `SkillUnits`)
//! Combat ↔ stats and unit fields: [`CombatView`] implements
//! [`CombatWorld`] and [`SkillUnits`] on the unit records
//! ([`crate::units::record`]), the stat lists ([`crate::stats`]), the
//! unit lists and timers ([`Game`]) and the DRLG town test. Unit handles
//! are [`UnitId`]s; items are unit ids too (no item provider yet: every
//! item query goes to [`Pending`]).

use crate::combat::{CombatEntry, CombatWorld, DamageRecord, RoomKind};
use crate::game::Game;
use crate::missiles::damage::SetupWorld;
use crate::rng::Seed;
use crate::skills::{SkillEntry, SkillUnits};
use crate::stats::key_layer;
use crate::units::hooks::Sim;
use crate::units::{UnitId, UnitType};

use super::monsters::umod_mode;
use super::{Pending, View};

/// Hireling monster classes (`0x0063EE90`, `monsters/init.md` §6 step 4).
pub const HIRELING_CLASSES: [u32; 5] = [271, 338, 359, 560, 561];

/// The unit event iteration `0x005C0C30(game, event, unit, other,
/// record)` (`skills/bodies.md` §2.18) installed in
/// [`super::ActionHooks::unit_events`]: the last matching function's
/// result, 0 when none matched.
pub type UnitEventFn<X> = fn(
    &mut CombatView<'_, X>,
    u8,
    Option<UnitId>,
    Option<UnitId>,
    Option<&mut DamageRecord>,
) -> i32;

/// Combat's view of a game: the game (frame, lists, timers) and a
/// [`View`] of the unit side.
pub struct CombatView<'a, X> {
    pub game: &'a mut Game,
    pub v: View<'a, X>,
}

impl<X: Pending> CombatView<'_, X> {
    fn ty(&self, u: UnitId) -> UnitType {
        self.game
            .lists
            .unit(u)
            .map(|e| e.ty)
            .or_else(|| self.v.units.get(u).map(|r| r.ty))
            .unwrap_or(UnitType::Tile)
    }
}

impl<X: Pending> CombatView<'_, X> {
    /// `0x005C0C30(game, event, unit, other, record)`: the registry
    /// ([`super::ActionHooks::unit_events`]) when installed, else
    /// [`Pending::unit_event`].
    pub fn fire_unit_event(
        &mut self,
        event: u8,
        unit: Option<UnitId>,
        other: Option<UnitId>,
        record: Option<&mut DamageRecord>,
    ) {
        match self.v.h.unit_events {
            Some(run) => {
                run(self, event, unit, other, record);
            }
            None => self.v.h.x.unit_event(event, unit, other, record),
        }
    }
}

impl<X: Pending> SkillUnits for CombatView<'_, X> {
    type Unit = UnitId;
    type Item = UnitId;

    fn unit_type(&self, u: UnitId) -> UnitType {
        self.ty(u)
    }
    fn class_id(&self, u: UnitId) -> i32 {
        self.v.units.get(u).map_or(0, |r| r.class as i32)
    }
    fn stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        self.v.stats.unit_total(u, stat, layer)
    }
    fn item_stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        self.v.h.x.item_stat(u, stat, layer)
    }
    fn base_stat(&self, u: UnitId, stat: u16, layer: u16) -> i32 {
        self.v.stats.unit_base(u, stat, layer)
    }
    fn formula_stat(&self, u: UnitId, stat: u16, mode: i32) -> i32 {
        // TODO(calc-expressions.md §3.5, OQ5): the getters of modes 2
        // (`mod`) and 0 (`accr`) are the stats spec's; mode 1 reads the
        // base, every other mode the total.
        if mode == 1 {
            self.v.stats.unit_base(u, stat, 0)
        } else {
            self.v.stats.unit_total(u, stat, 0)
        }
    }
    fn stat_entries(&self, u: UnitId, stat: u16, max: usize) -> Vec<(u16, i32)> {
        // `0x006261D0`: the unit list's full entries of `stat`, in key
        // (list) order.
        let Some(l) = self.v.stats.unit_list(u) else {
            return Vec::new();
        };
        self.v
            .stats
            .full_entries(l)
            .into_iter()
            .filter(|&(k, _)| crate::stats::key_stat(k) == stat)
            .map(|(k, v)| (key_layer(k), v))
            .take(max)
            .collect()
    }
    fn has_state(&self, u: UnitId, state: u16) -> bool {
        self.v.stats.has_state(u, u32::from(state))
    }
    fn state_stat(&self, u: UnitId, state: u16, stat: u16) -> Option<i32> {
        self.v.state_stat(u, state, stat)
    }
    fn seed(&mut self, u: UnitId) -> &mut Seed {
        self.v.seed(u)
    }
    fn skill_list(&self, u: UnitId) -> Vec<SkillEntry> {
        self.v.h.skill_list_of(u)
    }
    fn used_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.v.h.used_skill_of(u)
    }
    fn current_weapon(&self, u: UnitId) -> Option<UnitId> {
        self.v.h.x.current_weapon(u)
    }
    fn weapon(&self, u: UnitId) -> Option<UnitId> {
        self.v.h.x.weapon(u)
    }
    fn item_at(&self, u: UnitId, loc: u8) -> Option<UnitId> {
        self.v.h.x.item_at(u, loc)
    }
    fn item_is(&self, item: UnitId, itype: i32) -> bool {
        self.v.h.x.item_is(item, itype)
    }
    fn itype_is(&self, itype: i32, parent: i32) -> bool {
        self.v.h.x.itype_is(itype, parent)
    }
    fn wield_type(&self, item: UnitId) -> i32 {
        self.v.h.x.wield_type(item)
    }
    fn item_damage(&self, item: UnitId, max: bool) -> i32 {
        self.v.h.x.item_damage(item, max)
    }
    fn str_dex_bonus(&self, item: UnitId) -> (i32, i32) {
        self.v.h.x.str_dex_bonus(item)
    }
    fn item_flag_throw(&self, item: UnitId) -> bool {
        self.v.h.x.item_flag_throw(item)
    }
    fn missile_level(&self, u: UnitId) -> i32 {
        // Missile data +0x0C (u16).
        self.v
            .h
            .missiles
            .as_ref()
            .and_then(|s| s.get(u))
            .map_or(0, |d| i32::from(d.level as u16))
    }
}

impl<X: Pending> CombatWorld for CombatView<'_, X> {
    fn frame(&self) -> i32 {
        self.game.frame
    }
    fn expansion(&self) -> bool {
        self.v.data.expansion
    }
    fn difficulty(&self) -> usize {
        usize::from(self.v.data.difficulty)
    }
    fn hit_class_counter(&mut self) -> &mut u8 {
        &mut self.v.h.hit_class
    }
    fn ident(&self, u: UnitId) -> (UnitType, u32) {
        self.game
            .lists
            .unit(u)
            .map_or((UnitType::Tile, u32::MAX), |e| (e.ty, e.guid))
    }
    fn mode(&self, u: UnitId) -> i32 {
        self.v.units.get(u).map_or(0, |r| r.mode as i32)
    }
    /// `0x00622D00` (`combat/hit.md` §6.1 step 3): a player in mode 2, 3 or 6
    /// (walk, run, town walk), a monster in mode 2 or 15, or one in mode 8 /
    /// 9 whose `BaseId` is 110 (vulture). A unit with a shapeshift record
    /// (unit +0x30, read through `0x00621190`) is not modelled: its mode is
    /// used. Another unit type: the host's.
    fn moving_mode(&self, u: UnitId) -> bool {
        let Some(r) = self.v.units.get(u) else {
            return self.v.h.x.moving_mode(u);
        };
        match r.ty {
            UnitType::Player => matches!(r.mode, 2 | 3 | 6),
            UnitType::Monster => match r.mode {
                2 | 15 => true,
                8 | 9 => usize::try_from(r.class)
                    .ok()
                    .and_then(|c| self.v.h.tables.combat.monstats.get(c))
                    .is_some_and(|m| m.baseid == 110),
                _ => false,
            },
            _ => false,
        }
    }
    /// `0x005A0180`: the lent monster world's type flags
    /// ([`super::ActionHooks::monster_flag`]).
    fn monster_flag(&self, u: UnitId, mask: u32) -> bool {
        self.v.h.monster_flag(u, mask)
    }
    fn is_boss(&self, u: UnitId) -> bool {
        self.v.h.is_boss(u)
    }
    fn is_hireling(&self, u: UnitId) -> bool {
        self.v.is_hireling(self.game, u)
    }
    fn is_demon(&self, u: UnitId) -> bool {
        self.v.h.is_demon(u)
    }
    fn is_undead(&self, u: UnitId) -> bool {
        self.v.h.is_undead(u)
    }
    fn is_prime_evil(&self, u: UnitId) -> bool {
        self.v.h.is_prime_evil(u)
    }
    /// `0x00451F30(u, 0x80000000)`: unit flag +0xC4 bit 31 (set by the
    /// summon finish, `skills/bodies.md` §6.2 step 4), else the host's.
    fn is_revived(&self, u: UnitId) -> bool {
        match self.v.units.get(u) {
            Some(r) => r.flags & 0x8000_0000 != 0,
            None => self.v.h.x.is_revived(u),
        }
    }
    fn alignment(&self, u: UnitId) -> i32 {
        i32::from(self.v.h.x.alignment(u))
    }
    fn hostile(&self, a: UnitId, d: UnitId) -> bool {
        self.v.h.x.may_attack(a, d)
    }
    fn melee_range(&self, u: UnitId) -> i32 {
        self.v.h.x.melee_range(u)
    }
    /// A monster attacker with the path provider takes the exact test
    /// (reach `MeleeRng` + `range` + 1 against the unit distance, then the
    /// collision line, [`View::monster_in_melee_range`]); else the host's.
    fn in_melee_range(&self, a: UnitId, d: UnitId, range: i32) -> bool {
        self.v
            .monster_in_melee_range(self.game, a, d, range)
            .unwrap_or_else(|| self.v.h.x.in_melee_range(a, d, range))
    }
    fn has_shield(&self, u: UnitId) -> bool {
        self.v.h.x.has_shield(u)
    }
    fn composit_shield(&self, u: UnitId) -> bool {
        self.v.h.x.composit_shield(u)
    }
    fn weapon_class(&self, u: UnitId) -> i32 {
        self.v.h.x.weapon_class(u)
    }
    /// `0x00623C20` (`damage.md` §5.1 step 4.4): a monster's is its
    /// monstats2 `HitClass` (+0x14, through monstats `MonStatsEx`; no
    /// row → 0); a player's (its weapon's item hit class, 1 without one)
    /// and other types: [`Pending::weapon_hit_class`].
    fn weapon_hit_class(&self, u: UnitId) -> u32 {
        if self.ty(u) == UnitType::Monster {
            let t = &self.v.h.tables.combat;
            return self
                .v
                .units
                .get(u)
                .and_then(|r| t.monstats.get(r.class as usize))
                .and_then(|m| t.monstats2.get(usize::from(m.monstatsex)))
                .map_or(0, |m2| u32::from(m2.hitclass));
        }
        self.v.h.x.weapon_hit_class(u)
    }
    fn montype_matches(&self, layer: u16, montype: i32) -> bool {
        self.v.h.x.montype_matches(layer, montype)
    }
    fn room(&self, u: UnitId) -> RoomKind {
        // `0x00620BB0` then `0x0061AB00`.
        match self.game.lists.unit(u).and_then(|e| e.room()) {
            None => RoomKind::None,
            Some(r) if self.v.h.drlg.in_town(self.game, r) => RoomKind::Town,
            Some(_) => RoomKind::Field,
        }
    }
    fn is_dead(&self, u: UnitId) -> bool {
        self.v.units.is_dead(u)
    }
    fn monster_has_mode(&self, u: UnitId, mode: i32) -> bool {
        u8::try_from(mode).is_ok_and(|m| self.v.h.x.class_has_mode(self.class_id(u), m))
    }
    fn converted_type(&self, u: UnitId) -> i32 {
        self.v
            .h
            .x
            .converted_type(u)
            .unwrap_or(self.ty(u).index() as i32)
    }
    fn item_has_durability(&self, item: UnitId) -> bool {
        self.v.h.x.item_has_durability(item)
    }
    fn player_count_bonus(&self, players: i32) -> i32 {
        self.v.h.x.player_count_bonus(players)
    }

    fn set_stat(&mut self, u: UnitId, stat: u16, value: i32) {
        self.v.set_base(u, stat, value);
    }
    fn unit_event(&mut self, event: u8, unit: UnitId, other: UnitId, record: &mut DamageRecord) {
        self.fire_unit_event(event, Some(unit), Some(other), Some(record));
    }
    /// Toggle `0x00625A70` then the update-queue insert `0x0064C040`
    /// (as `0x00639DB0`, `ai.rs`), so the state-change message
    /// ([`View::state_change_messages`]) goes out in the next client pass.
    fn set_state(&mut self, u: UnitId, state: u16, on: bool) {
        self.v.set_state(u, state, on);
        let _ = self.game.lists.queue_update(u);
    }
    fn curse(
        &mut self,
        target: UnitId,
        owner: UnitId,
        state: u16,
        stat: u16,
        value: i32,
        frames: i32,
        skill: i32,
        level: i32,
    ) {
        self.v
            .h
            .x
            .curse(target, owner, state, stat, value, frames, skill, level);
    }
    fn state_list_expiry(&self, u: UnitId, state: u16) -> Option<i32> {
        let l = self.v.state_list(u, state)?;
        Some(self.v.stats.expire(l))
    }
    fn set_state_list_expiry(&mut self, u: UnitId, state: u16, expiry: i32) {
        if let Some(l) = self.v.state_list(u, state) {
            self.v.stats.set_expire(l, expiry);
        }
    }
    fn create_state_list(&mut self, u: UnitId, state: u16, owner: UnitId, expiry: i32) {
        let owner = self.game.lists.unit(owner).map(|e| (e.ty, e.guid));
        let l = self.v.create_state_list(u, state, owner, expiry);
        // Cold's own remove callback (`0x0057AD80`, damage.md §5.6).
        if let (Some(l), 11) = (l, state) {
            self.v.stats.set_remove_callback(
                l,
                Some(crate::stats::lists::RemoveCallback(
                    crate::skills::use_::bodies::callback::COLD,
                )),
            );
        }
    }
    fn set_state_list_stat(&mut self, u: UnitId, state: u16, stat: u16, value: i32) {
        if let Some(l) = self.v.state_list(u, state) {
            self.v.set_list_stat(l, stat, value);
        }
    }
    fn schedule_timer(&mut self, u: UnitId, ty: u8, frame: i32) {
        // `tick.md` §5.2 (form `0x005417D0`, no callback, args 0).
        if let Err(e) = self
            .game
            .schedule_event(u, u32::from(ty), frame, None, 0, 0)
        {
            self.v.unit_error(e.into());
        }
    }
    fn cancel_timers(&mut self, u: UnitId, ty: u8) {
        self.game.timers.cancel_unit_events(u, ty, None);
    }
    fn overlay(&mut self, u: UnitId, id: i32) {
        self.v.h.x.overlay(u, id);
    }
    /// `0x00623F50(unit)` (`skills/bodies.md` §2.6 "anim refresh"):
    /// the speed (+0x4C) of the unit's current mode recomputed
    /// ([`ActionHooks::rate_refresh`]: a shape state changes the draw
    /// identity, a stat fill the rate stats), then the host's hook.
    fn refresh_anim_rate(&mut self, u: UnitId) {
        let mut sim = Sim {
            game: &mut *self.game,
            units: &mut *self.v.units,
            stats: &mut *self.v.stats,
            data: self.v.data,
        };
        let speed = self.v.h.rate_refresh(&sim, u);
        if let (Some(s), Some(r)) = (speed, sim.units.get_mut(u)) {
            r.anim.speed = s;
        }
        // The velocity half of the same routine (`pathing.md` §8.1).
        self.v.h.monster_mode_velocity(&mut sim, u);
        self.v.h.x.refresh_anim_rate(u);
    }
    fn set_last_attacker(&mut self, d: UnitId, a: UnitId) {
        self.v.h.x.set_last_attacker(d, a);
    }
    /// `0x005A4390(game, attacker)` (`damage.md` §5.2 step 9): the umod
    /// dispatcher in mode 3 (`init.md` §22) on the lent monster world;
    /// without one, [`Pending::monster_hit_hook`].
    fn monster_hit_hook(&mut self, a: UnitId) {
        let mut sim = Sim {
            game: &mut *self.game,
            units: &mut *self.v.units,
            stats: &mut *self.v.stats,
            data: self.v.data,
        };
        if !self.v.h.run_umods(&mut sim, a, None, umod_mode::HIT) {
            self.v.h.x.monster_hit_hook(a);
        }
    }
    fn monster_damaged_hook(&mut self, d: UnitId) {
        self.v.h.x.monster_damaged_hook(d);
    }
    fn dual_wield_switch(&mut self, a: UnitId, offhand: bool, on: bool) {
        self.v.h.x.dual_wield_switch(a, offhand, on);
    }
    fn combat_list(&mut self, u: UnitId) -> &mut Vec<CombatEntry> {
        self.v.h.combat_lists.entry(u).or_default()
    }
    fn durability_loss(&mut self, owner: UnitId, item: UnitId) {
        self.v.h.x.durability_loss(owner, item);
    }
    fn thorns(&mut self, a: UnitId, d: UnitId, record: &mut DamageRecord) {
        self.v.h.x.thorns(a, d, record);
    }
    /// `damage.md` §7.1 ([`super::reaction::reaction`]).
    fn reaction(&mut self, a: UnitId, d: UnitId, record: &mut DamageRecord) {
        super::reaction::reaction(self, a, d, record);
    }
}

/// `missiles/damage.md` §1 step 6 and §2 on the unit records; the item
/// queries go to [`Pending`].
impl<X: Pending> SetupWorld for CombatView<'_, X> {
    /// A player's attack weapon `0x00623990(owner, 1)`; a monster with an
    /// inventory: `0x00622830` (both [`Pending::attack_weapon`]).
    fn setup_weapon(&self, owner: UnitId) -> Option<UnitId> {
        match self.ty(owner) {
            UnitType::Player => self.v.h.x.attack_weapon(owner),
            UnitType::Monster if self.v.h.x.has_inventory(owner) => self.v.h.x.attack_weapon(owner),
            _ => None,
        }
    }
    fn has_inventory(&self, u: UnitId) -> bool {
        self.v.h.x.has_inventory(u)
    }
    fn two_handed(&self, item: UnitId) -> bool {
        self.v.h.x.item_two_handed(item)
    }
    fn set_layer_stat(&mut self, u: UnitId, stat: u16, layer: u16, value: i32) {
        self.v.stats.unit_set(&mut *self.v.h, u, stat, value, layer);
    }
    fn dual_wield_toggle(&mut self, owner: UnitId) {
        self.v.h.x.dual_wield_toggle(owner);
    }
}
