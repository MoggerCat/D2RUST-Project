// Spec: specs/monsters/ai.md §1–§9 (seams `AiUnits`, `AiModes`, `AiWorld`, `AiTargets`, `AiSkills`)
//! Monster AI ↔ units, modes, timer events and the DRLG: [`View`]
//! implements [`crate::monsters::ai::AiHost`]. Real providers: seeds,
//! class, mode, states (`stat-lists.md` §9), the state-54 clear of
//! `0x005544B0`, the dead test, act, level of the room (DRLG), life and
//! life writes (`stats.md`), mode changes (`units.md` §4.6), the attack
//! flag 0x40, the town test and collision grids (`rooms.md` §10). Path,
//! targets, skills, monster data and sounds go to [`Pending`].

use crate::game::Game;
use crate::monsters::ai::{AiModes, AiSkills, AiTargets, AiUnits, AiWorld, ModeTarget};
use crate::rng::Seed;
use crate::stats::stat;
use crate::units::record::flags;
use crate::units::{RoomId, UnitId};

use super::units::clear_uninterruptable;
use super::{Pending, View};

/// Monster mode 3, get-hit (`ai.md` §1.2).
const MODE_GETHIT: u32 = 3;

impl<X: Pending> AiUnits for View<'_, X> {
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        View::seed(self, unit)
    }
    fn class(&self, unit: UnitId) -> i32 {
        self.units.get(unit).map_or(-1, |r| r.class as i32)
    }
    fn anim_mode(&self, unit: UnitId) -> u8 {
        self.units.get(unit).map_or(0, |r| r.mode as u8)
    }
    fn has_state(&self, unit: UnitId, s: u16) -> bool {
        self.stats.has_state(unit, u32::from(s))
    }
    fn clear_uninterruptable(&mut self, game: &mut Game, unit: UnitId) {
        clear_uninterruptable(self, game, unit);
    }
    fn is_dead(&self, unit: UnitId) -> bool {
        self.units.is_dead(unit)
    }
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.h.x.position(unit)
    }
    fn size(&self, unit: UnitId) -> i32 {
        self.h.x.size(unit)
    }
    fn act(&self, unit: UnitId) -> u8 {
        self.units.get(unit).map_or(0, |r| r.act)
    }
    /// The level id of the unit's room (DRLG); 0 without a room.
    fn level_id(&self, game: &Game, unit: UnitId) -> i32 {
        game.lists
            .unit(unit)
            .and_then(|e| e.room())
            .and_then(|r| self.h.drlg.level_id(game, r))
            .map_or(0, |l| l as i32)
    }
    fn monster_level(&self, unit: UnitId) -> i32 {
        self.h.x.monster_level(unit)
    }
    /// Life in percent of max life.
    ///
    /// TODO(ai.md §9.2, §2.4): the rounding of "life %" is not stated;
    /// `100 · life / max` truncating (0 with max 0).
    fn life_percent(&self, unit: UnitId) -> i32 {
        let life = i64::from(self.stats.unit_total(unit, stat::HITPOINTS, 0));
        let max = i64::from(self.stats.max_life(unit));
        if max == 0 {
            0
        } else {
            (life * 100 / max) as i32
        }
    }
    /// Life += amount (8.8), capped at max life (`ai.md` §2.4 step 2).
    fn add_life(&mut self, unit: UnitId, amount: i32) {
        let life = self
            .stats
            .unit_total(unit, stat::HITPOINTS, 0)
            .wrapping_add(amount)
            .min(self.stats.max_life(unit));
        self.set_base(unit, stat::HITPOINTS, life);
    }
    fn ai_state(&self, unit: UnitId) -> u32 {
        self.h.x.ai_state(unit)
    }
    fn alignment(&self, unit: UnitId) -> u8 {
        self.h.x.alignment(unit)
    }
    fn is_unique(&self, unit: UnitId) -> bool {
        self.h.x.monster_flag(unit, 8)
    }
    fn is_champion(&self, unit: UnitId) -> bool {
        self.h.x.monster_flag(unit, 4)
    }
    fn is_boss(&self, unit: UnitId) -> bool {
        self.h.x.is_boss(unit)
    }
    fn vision_seen(&self, unit: UnitId) -> Option<u32> {
        self.h.x.vision_seen(unit)
    }
    fn mark_seen(&mut self, unit: UnitId) {
        self.h.x.mark_seen(unit);
    }
    fn ai_reset(&mut self, unit: UnitId) {
        self.h.x.ai_reset(unit);
    }
    fn interacting(&self, unit: UnitId) -> bool {
        self.h.x.interacting(unit)
    }
    fn busy(&self, unit: UnitId) -> bool {
        self.h.x.busy(unit)
    }
}

impl<X: Pending> AiModes for View<'_, X> {
    /// A monster mode change `0x005A7C20` (`units.md` §4.6) toward
    /// `target` (path spec). False when the mode set failed.
    ///
    /// TODO(ai.md §7.1, units.md §4.6): `0x005A7C20` falls into the
    /// neutral start itself when a start function fails and reports
    /// nothing; "failed" is read as the mode set returning an error
    /// (state 54, bad mode).
    fn change_mode(&mut self, game: &mut Game, unit: UnitId, mode: u8, target: ModeTarget) -> bool {
        self.h.x.set_mode_target(unit, target);
        self.monster_set_mode(game, unit, u32::from(mode))
    }
    /// The anim mode (unit +0x10) without a mode change.
    fn set_anim_mode(&mut self, unit: UnitId, mode: u8) {
        if let Some(r) = self.units.get_mut(unit) {
            r.mode = u32::from(mode);
        }
    }
    fn set_path_steps(&mut self, unit: UnitId, steps: i32) {
        self.h.x.set_path_steps(unit, steps);
    }
    fn path_blocked(&self, unit: UnitId) -> bool {
        self.h.x.path_blocked(unit)
    }
    fn stop_path(&mut self, unit: UnitId) {
        self.h.x.stop_path(unit);
    }
    fn set_current_skill(&mut self, unit: UnitId, skill: i32) -> bool {
        self.h.x.set_current_skill(unit, skill)
    }
    /// Unit flag 0x40 (`units.md` §2).
    fn set_skill_flag(&mut self, unit: UnitId) {
        if let Some(r) = self.units.get_mut(unit) {
            r.flags |= flags::ATTACK_PENDING;
        }
    }
    fn class_has_mode(&self, class: i32, mode: u8) -> bool {
        self.h.x.class_has_mode(class, mode)
    }
    fn play_sound(&mut self, game: &mut Game, unit: UnitId, sound: u32, to: Option<UnitId>) {
        self.h.x.play_sound(game, unit, sound, to);
    }
    /// `0x005A8520` get-hit branch: mode change to get-hit (3).
    ///
    /// TODO(ai.md §1.2): the last-hit class 160 goes to a unit field the
    /// units spec does not describe; not stored.
    fn knockback_to_gethit(&mut self, game: &mut Game, unit: UnitId) {
        self.monster_set_mode(game, unit, MODE_GETHIT);
    }
    fn walk_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        a: i32,
        b: i32,
    ) -> bool {
        self.h.x.walk_in_radius(game, unit, target, a, b)
    }
    fn operate_door(&mut self, game: &mut Game, unit: UnitId, door: UnitId) {
        self.h.x.operate_door(game, unit, door);
    }
}

impl<X: Pending> AiWorld for View<'_, X> {
    fn in_town(&self, game: &Game, room: RoomId) -> bool {
        self.h.drlg.in_town(game, room)
    }
    fn los_draw(&self, game: &Game, room: RoomId) -> bool {
        self.h.x.los_draw(game, room)
    }
    /// `0x0064D910`: the grid at the unit's position has a `mask` bit.
    fn collides(&self, game: &Game, unit: UnitId, mask: u16) -> bool {
        let (x, y) = self.h.x.position(unit);
        game.lists
            .unit(unit)
            .and_then(|e| e.room())
            .and_then(|r| self.h.drlg.collision(game, r, x, y))
            .is_some_and(|m| m & mask != 0)
    }
    fn line_blocked(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        self.h.x.line_blocked(game, a, b)
    }
    fn in_melee_range(&self, _: &Game, a: UnitId, b: UnitId) -> bool {
        self.h.x.in_melee_range(a, b, 0)
    }
    fn can_reach_directly(&self, game: &Game, unit: UnitId, target: UnitId) -> bool {
        self.h.x.can_reach_directly(game, unit, target)
    }
    fn find_spot(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32, RoomId)> {
        self.h.x.find_spot(game, unit)
    }
    fn last_dead(&self, game: &Game, room: RoomId) -> [Option<UnitId>; 4] {
        self.h.x.last_dead(game, room)
    }
}

impl<X: Pending> AiTargets for View<'_, X> {
    fn target_nodes(&self, game: &Game) -> [Vec<UnitId>; 10] {
        self.h.x.target_nodes(game)
    }
    fn forced_target(&mut self, game: &mut Game, unit: UnitId) -> Option<(UnitId, i32)> {
        self.h.x.forced_target(game, unit)
    }
    fn good_target_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        los: bool,
    ) -> Option<(UnitId, i32)> {
        self.h.x.good_target_search(game, unit, los)
    }
    fn choose_alternative(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        main: Option<UnitId>,
        alt: UnitId,
    ) -> bool {
        self.h.x.choose_alternative(game, unit, main, alt)
    }
    fn secondary_target(&mut self, game: &mut Game, unit: UnitId) -> (Option<UnitId>, i32, bool) {
        self.h.x.secondary_target(game, unit)
    }
    fn nearest_player(&mut self, game: &mut Game, unit: UnitId) -> (UnitId, bool) {
        self.h.x.nearest_player(game, unit)
    }
    fn find_door(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId> {
        self.h.x.find_door(game, unit)
    }
    fn door_monster_ok(&self, door: UnitId) -> bool {
        self.h.x.door_monster_ok(door)
    }
    fn special_walk_target(&mut self, game: &mut Game, unit: UnitId) -> Option<(UnitId, i32)> {
        self.h.x.special_walk_target(game, unit)
    }
    fn shaman_corpses(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        max_sq: i32,
        own_minions: bool,
    ) -> (Option<UnitId>, u32) {
        self.h.x.shaman_corpses(game, unit, max_sq, own_minions)
    }
}

impl<X: Pending> AiSkills for View<'_, X> {
    fn skill_usable(&mut self, game: &mut Game, unit: UnitId, skill: i32, target: UnitId) -> bool {
        self.h.x.skill_usable(game, unit, skill, target)
    }
}
