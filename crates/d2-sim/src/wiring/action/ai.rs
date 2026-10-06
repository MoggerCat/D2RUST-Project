// Spec: specs/monsters/ai.md §1–§9; specs/monsters/init.md §7 (seams `AiUnits`, `AiModes`, `AiWorld`, `AiTargets`, `AiSkills`, `AiQuests`)
//! Monster AI ↔ units, modes, timer events and the DRLG: [`View`]
//! implements [`crate::monsters::ai::AiHost`]. Real providers: seeds,
//! class, mode, states (`stat-lists.md` §9), the state-54 clear of
//! `0x005544B0`, the dead test, act, level of the room (DRLG), life and
//! life writes (`stats.md`), mode changes (`units.md` §4.6), the attack
//! flag 0x40, the town test and collision grids (`rooms.md` §10), the
//! door operate and `MonsterOK` on the object state (`objects.md` §7.1). Path,
//! targets, skills, sounds and the monster data the lent monster world
//! ([`super::monsters`]) does not answer go to [`Pending`].

use crate::game::Game;
use crate::monsters::ai::{
    AiModes, AiQuests, AiSkills, AiTargets, AiUnits, AiWorld, ModeTarget, PortalNpc,
};
use crate::rng::Seed;
use crate::stats::stat;
use crate::units::record::flags;
use crate::units::{RoomId, UnitId};

use super::objects::ObjectRoute;
use super::units::clear_uninterruptable;
use super::{Pending, View};
use crate::world::objects::Dispatch;

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
        self.h.path_position(unit)
    }
    fn size(&self, unit: UnitId) -> i32 {
        self.path_size(unit)
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
    /// The monster level: stat 12 (`level`, `init.md` §7 rule 4) of a
    /// monster with monster data in the lent monster world; else
    /// [`Pending::monster_level`].
    ///
    /// TODO(ai.md §2.4 step 2): the getter of "level" is not named; the
    /// unit total (`0x00625480`, layer 0) is read.
    fn monster_level(&self, unit: UnitId) -> i32 {
        match self.h.monster_data(unit) {
            Some(_) => self.stats.unit_total(unit, stat::LEVEL, 0),
            None => self.h.x.monster_level(unit),
        }
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
    /// `0x005A0180(unit, 8)` ([`super::ActionHooks::monster_flag`]).
    fn is_unique(&self, unit: UnitId) -> bool {
        self.h.monster_flag(unit, 8)
    }
    /// `0x005A0180(unit, 4)`.
    fn is_champion(&self, unit: UnitId) -> bool {
        self.h.monster_flag(unit, 4)
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
    fn has_interaction_block(&self, unit: UnitId) -> bool {
        self.h.x.has_interaction_block(unit)
    }
    fn in_interaction_list(&self, npc: UnitId, player: UnitId) -> bool {
        self.h.x.in_interaction_list(npc, player)
    }
    /// `0x00627260(unit, 6, value)`: life (stat 6) base := value.
    fn set_life(&mut self, unit: UnitId, value: i32) {
        self.set_base(unit, stat::HITPOINTS, value);
    }
    fn stat(&self, unit: UnitId, s: u16) -> i32 {
        View::stat(self, unit, s)
    }
    fn set_stat(&mut self, unit: UnitId, s: u16, value: i32) {
        self.set_base(unit, s, value);
    }
    /// Unit flags (unit +0xC4, `units.md` §2).
    fn set_unit_flag(&mut self, unit: UnitId, mask: u32) {
        if let Some(r) = self.units.get_mut(unit) {
            r.flags |= mask;
        }
    }
    /// The state toggle `0x00625A70` (`stat-lists.md` §9.2).
    ///
    /// TODO(spec: ai.md §9.26): SandRaider calls `0x00639DB0`; read as the
    /// state toggle of `stat-lists.md` §9.2.
    fn set_state(&mut self, unit: UnitId, s: u16, on: bool) {
        View::set_state(self, unit, s, on);
    }
    fn path_target(&self, unit: UnitId) -> Option<UnitId> {
        self.h.x.path_target(unit)
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
    /// The operate entry `0x00584540` (`objects.md` §7.1) with the monster
    /// as operator on the object state ([`super::objects`]); a quest,
    /// waypoint or `todo` route goes to [`Pending::object_route`]. A game
    /// without an object state: [`Pending::operate_door`].
    fn operate_door(&mut self, game: &mut Game, unit: UnitId, door: UnitId) {
        if self.h.objects.is_none() {
            self.h.x.operate_door(game, unit, door);
            return;
        }
        let Some(guid) = game.lists.unit(door).map(|e| e.guid) else {
            return;
        };
        if let Some((_, Some(d))) = self.operate_object(game, Some(unit), guid) {
            if !matches!(d, Dispatch::Done(_)) {
                let room = game.lists.unit(door).and_then(|e| e.room());
                let at = self.h.path_position(door);
                self.object_route(game, ObjectRoute::Operate(d), room, at);
            }
        }
    }
    /// Overlay `0x00621E40` ([`Pending::overlay`]).
    fn start_overlay(&mut self, unit: UnitId, overlay: i32) {
        self.h.x.overlay(unit, overlay);
    }
    fn set_facing(&mut self, unit: UnitId, dir: i32) {
        self.h.x.set_facing(unit, dir);
    }
}

impl<X: Pending> AiWorld for View<'_, X> {
    fn in_town(&self, game: &Game, room: RoomId) -> bool {
        self.h.drlg.in_town(game, room)
    }
    fn los_draw(&self, game: &Game, room: RoomId) -> bool {
        self.h.x.los_draw(game, room)
    }
    /// `0x0064D910`: the grid at the unit's position has a `mask` bit
    /// (with the path provider: the pattern test of
    /// `path-placement.md` §4 rule 5 with the path's pattern and room).
    fn collides(&self, game: &Game, unit: UnitId, mask: u16) -> bool {
        if let Some(d) = self.h.paths.as_ref().and_then(|p| p.dynamic(unit)) {
            return crate::path::collision::pattern_collides(
                &self.h.drlg,
                d.room,
                d.x(),
                d.y(),
                d.pattern,
                mask,
            );
        }
        let (x, y) = self.h.path_position(unit);
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
    fn footprint_ok(&self, game: &Game, class: i32, room: Option<RoomId>, x: i32, y: i32) -> bool {
        self.h.x.footprint_ok(game, class, room, x, y)
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
    /// objects.txt `MonsterOK` from the object state; without one (or
    /// without object data for `door`) [`Pending::door_monster_ok`].
    fn door_monster_ok(&self, door: UnitId) -> bool {
        self.object_monster_ok(door)
            .unwrap_or_else(|| self.h.x.door_monster_ok(door))
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
    fn nearest_evil_monster(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId> {
        self.h.x.nearest_evil_monster(game, unit)
    }
}

impl<X: Pending> AiSkills for View<'_, X> {
    fn skill_usable(&mut self, game: &mut Game, unit: UnitId, skill: i32, target: UnitId) -> bool {
        self.h.x.skill_usable(game, unit, skill, target)
    }
}

impl<X: Pending> AiQuests for View<'_, X> {
    fn portal_setup(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) -> bool {
        self.h.x.portal_setup(game, unit, npc)
    }
    fn spawn_town_portal(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) {
        self.h.x.spawn_town_portal(game, unit, npc);
    }
    fn spawn_outside_portal(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) -> bool {
        self.h.x.spawn_outside_portal(game, unit, npc)
    }
    fn portal_coords(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        npc: PortalNpc,
    ) -> Option<(i32, i32)> {
        self.h.x.portal_coords(game, unit, npc)
    }
    fn drehya_update(&mut self, game: &mut Game) {
        self.h.x.drehya_update(game);
    }
    fn drehya_wait(&mut self, game: &mut Game) -> bool {
        self.h.x.drehya_wait(game)
    }
    fn jerhyn_palace_active(&mut self, game: &mut Game) -> bool {
        self.h.x.jerhyn_palace_active(game)
    }
    fn jerhyn_npc_state(&mut self, game: &mut Game, unit: UnitId) -> (i32, i32) {
        self.h.x.jerhyn_npc_state(game, unit)
    }
    fn guard_moving(&mut self, game: &mut Game, unit: UnitId) -> bool {
        self.h.x.guard_moving(game, unit)
    }
    fn alkor_bird(&mut self, game: &mut Game) -> bool {
        self.h.x.alkor_bird(game)
    }
    fn alkor_reset(&mut self, game: &mut Game) {
        self.h.x.alkor_reset(game);
    }
    fn ormus_altar(&mut self, game: &mut Game) -> Option<(i32, i32)> {
        self.h.x.ormus_altar(game)
    }
    fn ormus_set_altar_mode(&mut self, game: &mut Game) {
        self.h.x.ormus_set_altar_mode(game);
    }
    fn cain_town_coords(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32)> {
        self.h.x.cain_town_coords(game, unit)
    }
    fn cain_in_town_activated(&mut self, game: &mut Game, unit: UnitId) {
        self.h.x.cain_in_town_activated(game, unit);
    }
    fn anya_open_portal(&mut self, game: &mut Game, unit: UnitId) {
        self.h.x.anya_open_portal(game, unit);
    }
}
