// Spec: specs/monsters/ai.md §1–§8; specs/monsters/ai-bodies.md §9; specs/monsters/init.md §7; specs/monsters/ai-bodies-2.md..ai-bodies-7.md (seams `AiUnits`, `AiModes`, `AiWorld`, `AiTargets`, `AiSkills`, `AiQuests`, `AiActs`, `AiSummons`)
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
    AiActs, AiModes, AiQuests, AiSkills, AiSummons, AiTargets, AiUnits, AiWorld, ModeTarget,
    PortalNpc, QuestCall,
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
    /// TODO(ai-bodies.md §9.2, ai.md §2.4): the rounding of "life %" is not stated;
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
    /// `0x00535060`: the interact info on the unit record (active),
    /// then [`Pending::busy`] (cursor, player data +0x4C).
    fn busy(&self, unit: UnitId) -> bool {
        self.units.get(unit).is_some_and(|r| r.interact.active) || self.h.x.busy(unit)
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
    /// TODO(spec: ai-bodies.md §9.26): SandRaider calls `0x00639DB0`; read as the
    /// state toggle of `stat-lists.md` §9.2.
    fn set_state(&mut self, unit: UnitId, s: u16, on: bool) {
        View::set_state(self, unit, s, on);
    }
    fn path_target(&self, unit: UnitId) -> Option<UnitId> {
        if self.h.paths.is_some() {
            return crate::wiring::path::monsters::path_target(self.h, unit);
        }
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
        crate::wiring::path::monsters::stage_request(self.h, unit, target);
        self.monster_set_mode(game, unit, u32::from(mode))
    }
    /// The mode change with the velocity request (`ai.md` §7.5 rule
    /// 4.1): every mode but GH consumes it (staged for the movement
    /// set-up of [`crate::wiring::path::monsters`]; dropped without the
    /// provider, which has no path to give it to).
    fn change_mode_with(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        mode: u8,
        target: ModeTarget,
        velocity: &mut crate::monsters::ai::VelocityRequest,
    ) -> bool {
        if u32::from(mode) != MODE_GETHIT {
            let v = std::mem::take(velocity);
            crate::wiring::path::monsters::stage_velocity(self.h, unit, v);
        }
        self.change_mode(game, unit, mode, target)
    }
    /// The anim mode (unit +0x10) without a mode change.
    fn set_anim_mode(&mut self, unit: UnitId, mode: u8) {
        if let Some(r) = self.units.get_mut(unit) {
            r.mode = u32::from(mode);
        }
    }
    /// The path step count: the stop distance `0x00649070` (`ai.md`
    /// §7.5 rule 7, `pathing.md` §13.1 rule 2) with the path provider.
    fn set_path_steps(&mut self, unit: UnitId, steps: i32) {
        if crate::wiring::path::monsters::set_stop_distance(self.h, unit, steps).is_none() {
            self.h.x.set_path_steps(unit, steps);
        }
    }
    fn path_blocked(&self, unit: UnitId) -> bool {
        crate::wiring::path::monsters::path_blocked(self.h, unit)
            .unwrap_or_else(|| self.h.x.path_blocked(unit))
    }
    /// Stop the path `0x00648730` (`ai.md` §7.5 rule 7, `pathing.md`
    /// §13.1 rule 3) with the path provider.
    fn stop_path(&mut self, unit: UnitId) {
        if crate::wiring::path::monsters::stop_path(self.h, unit).is_none() {
            self.h.x.stop_path(unit);
        }
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

/// The Act II–V seams: unit flags, max life, state groups and the states
/// count are real (`units.md` §2, `stats.md`, `stat-lists.md` §9.3); the
/// move mask and the path stop go to the path seams of [`Pending`];
/// everything else to the `ai_*` calls of [`Pending`].
impl<X: Pending> AiActs for View<'_, X> {
    fn unit_flags(&self, unit: UnitId) -> u32 {
        self.units.get(unit).map_or(0, |r| r.flags)
    }
    fn clear_unit_flag(&mut self, unit: UnitId, mask: u32) {
        if let Some(r) = self.units.get_mut(unit) {
            r.flags &= !mask;
        }
    }
    fn max_life(&self, unit: UnitId) -> i32 {
        self.stats.max_life(unit)
    }
    fn max_mana(&self, unit: UnitId) -> i32 {
        self.h.x.ai_max_mana(unit)
    }
    fn has_state_group(&self, unit: UnitId, g: u8) -> bool {
        self.stats.has_group(unit, usize::from(g))
    }
    fn states_count(&self) -> i32 {
        self.stats.data().states.count() as i32
    }
    fn has_list_flag(&self, unit: UnitId, flags: u32) -> bool {
        self.h.x.ai_has_list_flag(unit, flags)
    }
    fn hostile(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        self.h.x.ai_hostile(game, a, b)
    }
    fn owner(&self, game: &Game, unit: UnitId) -> Option<UnitId> {
        self.h.x.ai_owner(game, unit)
    }
    fn owner_record(&self, unit: UnitId) -> Option<(i32, u32)> {
        self.h.x.ai_owner_record(unit)
    }
    fn quest_flag(&self, player: UnitId, difficulty: u8, quest: i32, flag: i32) -> bool {
        self.h.x.ai_quest_flag(player, difficulty, quest, flag)
    }
    fn portal_guid(&self, player: UnitId) -> Option<u32> {
        self.h.x.ai_portal_guid(player)
    }
    fn component(&self, unit: UnitId, i: usize) -> u8 {
        self.h.x.ai_component(unit, i)
    }
    fn target_unit(&self, game: &Game, unit: UnitId) -> Option<UnitId> {
        self.h.x.ai_target_unit(game, unit)
    }
    fn set_target_override(&mut self, unit: UnitId, kind: i32, guid: u32) {
        self.h.x.ai_set_target_override(unit, kind, guid);
    }
    fn chain_index(&self, class: i32) -> i32 {
        self.h.x.ai_chain_index(class)
    }
    fn class_for_level(&self, game: &Game, room: Option<RoomId>, class: i32) -> i32 {
        self.h.x.ai_class_for_level(game, room, class)
    }
    fn skill_level(&self, unit: UnitId, skill: i32, highest: bool) -> Option<i32> {
        self.h.x.ai_skill_level(unit, skill, highest)
    }
    fn skill_entry(&self, unit: UnitId, skill: i32) -> Option<(i32, u8)> {
        self.h.x.ai_skill_entry(unit, skill)
    }
    fn hand_skill(&self, unit: UnitId, right: bool) -> Option<(i32, i32)> {
        self.h.x.ai_hand_skill(unit, right)
    }
    fn add_right_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32) {
        self.h.x.ai_add_right_skill(game, unit, skill, level);
    }
    fn assign_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32) {
        self.h.x.ai_assign_skill(game, unit, skill, level);
    }
    fn set_skill_param(&mut self, unit: UnitId, skill: i32, value: i32) -> bool {
        self.h.x.ai_set_skill_param(unit, skill, value)
    }
    fn skill_check(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        skill: i32,
        target: Option<UnitId>,
        x: i32,
        y: i32,
    ) -> bool {
        self.h.x.ai_skill_check(game, unit, skill, target, x, y)
    }
    fn corpse_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: Option<UnitId>,
        skill: i32,
        level: i32,
    ) -> Option<UnitId> {
        self.h.x.ai_corpse_search(game, unit, target, skill, level)
    }
    fn path_pattern(&self, unit: UnitId) -> i32 {
        self.h.x.ai_path_pattern(unit)
    }
    fn set_path_pattern(&mut self, unit: UnitId, pattern: i32) {
        self.h.x.ai_set_path_pattern(unit, pattern);
    }
    fn set_move_mask(&mut self, unit: UnitId, mask: u16) {
        self.h.x.set_move_mask(unit, mask);
    }
    fn place_unit(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        room: Option<RoomId>,
        x: i32,
        y: i32,
    ) -> bool {
        self.h.x.ai_place_unit(game, unit, room, x, y)
    }
    fn stamp_pattern(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        pattern: i32,
        mask: u16,
    ) {
        self.h.x.ai_stamp_pattern(game, room, x, y, pattern, mask);
    }
    fn clear_cell(&mut self, game: &mut Game, room: Option<RoomId>, x: i32, y: i32, bits: u16) {
        self.h.x.ai_clear_cell(game, room, x, y, bits);
    }
    fn point_collides(&self, game: &Game, room: Option<RoomId>, x: i32, y: i32, mask: u16) -> bool {
        self.h.x.ai_point_collides(game, room, x, y, mask)
    }
    fn pattern_collides(&self, game: &Game, unit: UnitId, pattern: i32, mask: u16) -> bool {
        self.h.x.ai_pattern_collides(game, unit, pattern, mask)
    }
    fn free_point(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        size: i32,
    ) -> Option<(i32, i32)> {
        self.h.x.ai_free_point(game, room, x, y, size)
    }
    fn free_spot_for(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        class: i32,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32)> {
        self.h.x.ai_free_spot_for(game, unit, class, x, y)
    }
    fn room_at(&self, game: &Game, unit: UnitId, x: i32, y: i32) -> Option<RoomId> {
        self.h.x.ai_room_at(game, unit, x, y)
    }
    fn move_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        mode: u8,
        a: i32,
        b: i32,
    ) -> bool {
        self.h.x.ai_move_in_radius(game, unit, target, mode, a, b)
    }
    fn set_path_target(&mut self, unit: UnitId, target: UnitId) {
        self.h.x.ai_set_path_target(unit, target);
    }
    fn path_has_points(&mut self, game: &mut Game, unit: UnitId, target: UnitId) -> bool {
        self.h.x.ai_path_has_points(game, unit, target)
    }
    fn direction64(&self, unit: UnitId, target: UnitId) -> i32 {
        self.h.x.ai_direction64(unit, target)
    }
    fn stop_unit_path(&mut self, unit: UnitId) {
        AiModes::stop_path(self, unit);
    }
    fn spawn_monster(
        &mut self,
        game: &mut Game,
        room: RoomId,
        x: i32,
        y: i32,
        class: i32,
        mode: u8,
        spread: i32,
        flags: u32,
    ) -> Option<UnitId> {
        self.h
            .x
            .ai_spawn_monster(game, room, x, y, class, mode, spread, flags)
    }
    fn kill(&mut self, game: &mut Game, unit: UnitId, killer: Option<UnitId>) {
        self.h.x.ai_kill(game, unit, killer);
    }
    fn remove_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.h.x.ai_remove_unit(game, unit);
    }
    fn link_clone(&mut self, game: &mut Game, unit: UnitId, clone: UnitId) {
        self.h.x.ai_link_clone(game, unit, clone);
    }
    fn reinit_class(&mut self, game: &mut Game, unit: UnitId, class: i32, mode: u8) {
        self.h.x.ai_reinit_class(game, unit, class, mode);
    }
    fn change_class_list(&mut self, game: &mut Game, unit: UnitId, class: i32) {
        self.h.x.ai_change_class_list(game, unit, class);
    }
    fn wisp_buff(&mut self, game: &mut Game, target: UnitId, value: i32, expire: i32) {
        self.h.x.ai_wisp_buff(game, target, value, expire);
    }
    fn preload_class(&mut self, game: &mut Game, unit: UnitId, class: i32) {
        self.h.x.ai_preload_class(game, unit, class);
    }
    fn wisp_find(&mut self, game: &mut Game, unit: UnitId) -> Vec<UnitId> {
        self.h.x.ai_wisp_find(game, unit)
    }
    fn wave(&self, w: i32) -> Option<(i32, i32)> {
        self.h.x.ai_wave(w)
    }
    fn clear_room_portal_flag(&mut self, game: &mut Game, room: Option<RoomId>) {
        self.h.x.ai_clear_room_portal_flag(game, room);
    }
    fn quest_call(&mut self, game: &mut Game, unit: UnitId, call: QuestCall) -> bool {
        self.h.x.ai_quest_call(game, unit, call)
    }
}

/// The seams of `ai-bodies-6.md` / `ai-bodies-7.md`: unit flags 2 (+0xC8)
/// and the target-node slot (+0xD0) are real (`units.md` §2); everything
/// else keeps the narrow default of [`AiSummons`] until its owner wires it.
impl<X: Pending> AiSummons for View<'_, X> {
    fn set_unit_flags2(&mut self, unit: UnitId, mask: u32) {
        if let Some(r) = self.units.get_mut(unit) {
            r.flags2 |= mask;
        }
    }
    fn target_slot(&self, unit: UnitId) -> i32 {
        self.units.get(unit).map_or(11, |r| r.node_index as i32)
    }
}
