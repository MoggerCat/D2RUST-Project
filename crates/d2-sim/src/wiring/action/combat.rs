// Spec: specs/combat/hit.md, specs/combat/damage.md, specs/skills/levels.md (seams `CombatWorld`, `SkillUnits`)
//! Combat ↔ stats and unit fields: [`CombatView`] implements
//! [`CombatWorld`] and [`SkillUnits`] on the unit records
//! ([`crate::units::record`]), the stat lists ([`crate::stats`]), the
//! unit lists and timers ([`Game`]) and the DRLG town test. Unit handles
//! are [`UnitId`]s; items are unit ids too (no item provider yet: every
//! item query goes to [`Pending`]).

use crate::combat::{CombatEntry, CombatWorld, DamageRecord, RoomKind};
use crate::game::Game;
use crate::rng::Seed;
use crate::skills::{SkillEntry, SkillUnits};
use crate::stats::key_layer;
use crate::units::{UnitId, UnitType};

use super::{Pending, View};

/// Hireling monster classes (`0x0063EE90`, `monsters/init.md` §6 step 4).
pub const HIRELING_CLASSES: [u32; 5] = [271, 338, 359, 560, 561];

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
        self.v.h.x.skill_list(u)
    }
    fn used_skill(&self, u: UnitId) -> Option<SkillEntry> {
        self.v.h.x.used_skill(u)
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
    fn moving_mode(&self, u: UnitId) -> bool {
        self.v.h.x.moving_mode(u)
    }
    fn monster_flag(&self, u: UnitId, mask: u32) -> bool {
        self.v.h.x.monster_flag(u, mask)
    }
    fn is_boss(&self, u: UnitId) -> bool {
        self.v.h.x.is_boss(u)
    }
    fn is_hireling(&self, u: UnitId) -> bool {
        self.v.is_hireling(self.game, u)
    }
    fn is_demon(&self, u: UnitId) -> bool {
        self.v.h.x.is_demon(u)
    }
    fn is_undead(&self, u: UnitId) -> bool {
        self.v.h.x.is_undead(u)
    }
    fn is_prime_evil(&self, u: UnitId) -> bool {
        self.v.h.x.is_prime_evil(u)
    }
    fn is_revived(&self, u: UnitId) -> bool {
        self.v.h.x.is_revived(u)
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
    fn in_melee_range(&self, a: UnitId, d: UnitId, range: i32) -> bool {
        self.v.h.x.in_melee_range(a, d, range)
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
    fn weapon_hit_class(&self, u: UnitId) -> u32 {
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
        self.v
            .h
            .x
            .unit_event(event, Some(unit), Some(other), Some(record));
    }
    fn set_state(&mut self, u: UnitId, state: u16, on: bool) {
        self.v.set_state(u, state, on);
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
        self.v.create_state_list(u, state, owner, expiry);
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
    fn refresh_anim_rate(&mut self, u: UnitId) {
        self.v.h.x.refresh_anim_rate(u);
    }
    fn set_last_attacker(&mut self, d: UnitId, a: UnitId) {
        self.v.h.x.set_last_attacker(d, a);
    }
    fn monster_hit_hook(&mut self, a: UnitId) {
        self.v.h.x.monster_hit_hook(a);
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
    fn reaction(&mut self, a: UnitId, d: UnitId, record: &mut DamageRecord) {
        self.v.h.x.reaction(a, d, record);
    }
}
