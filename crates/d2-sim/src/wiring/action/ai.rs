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
    AiActs, AiModes, AiQuests, AiSkills, AiSummons, AiTargets, AiUnits, AiWorld, HireRow,
    ModeTarget, PortalNpc, QuestCall,
};
use crate::rng::Seed;
use crate::stats::stat;
use crate::units::hooks::Sim;
use crate::units::record::flags;
use crate::units::{RoomId, UnitId, UnitType};

use super::objects::ObjectRoute;
use super::units::clear_uninterruptable;
use super::{Pending, View, WiringError};
use crate::world::objects::Dispatch;
use crate::world::quests::act2::q4::JerhynStep;

/// Monster mode 3, get-hit (`ai.md` §1.2).
const MODE_GETHIT: u32 = 3;

impl<X: Pending> View<'_, X> {
    /// `0x00588E10` (`quests-act5.md` §4.10): a dead (mode 12) prison
    /// door (class 434) among the units of the rooms adjacent to `u`'s
    /// room (the room itself included).
    fn dead_prison_door_near(&self, game: &Game, u: UnitId) -> bool {
        use crate::path::collision::CollisionRooms;
        let Some(room) = game.lists.unit(u).and_then(|e| e.room()) else {
            return false;
        };
        let d = &self.h.drlg;
        (0..d.adjacent_count(room))
            .filter_map(|i| d.adjacent(room, i))
            .flat_map(|r| game.lists.room_units(r))
            .any(|m| {
                self.units.get(m).is_some_and(|r| {
                    r.ty == crate::units::UnitType::Monster && r.class == 434 && r.mode == 12
                })
            })
    }
    /// A monster's class and its monstats `interact` flag (flags byte
    /// +0xD bit 1, `ai.md` §5.3); `None` for a non-monster.
    fn npc_interact(&self, unit: UnitId) -> Option<(u16, bool)> {
        let r = self
            .units
            .get(unit)
            .filter(|r| r.ty == crate::units::UnitType::Monster)?;
        let class = u16::try_from(r.class).ok()?;
        let interact = self
            .h
            .tables
            .combat
            .monstats
            .get(usize::from(class))
            .is_some_and(|m| m.interact);
        Some((class, interact))
    }
}

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
        match self.h.monster_data(unit) {
            Some(m) => m.ai_state,
            None => self.h.x.ai_state(unit),
        }
    }
    fn source_unit(&self, unit: UnitId) -> Option<UnitId> {
        if self.units.get(unit)?.flags2 & 0x400 == 0 {
            return None;
        }
        self.h.unit_source.get(&unit).copied()
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
    /// `0x0063E9F0(0, unit)`: a monster whose monstats byte +0x0C has
    /// the `boss` bit (0x40; the global at `0x006CE280`).
    fn is_boss(&self, unit: UnitId) -> bool {
        let Some(r) = self.units.get(unit) else {
            return false;
        };
        r.ty == UnitType::Monster
            && usize::try_from(r.class)
                .ok()
                .and_then(|c| self.h.tables.combat.monstats.get(c))
                .is_some_and(|m| m.boss)
    }
    /// Monster data +0x50 (the coordinate record of `population.md`
    /// §9.6 step 3) and its +0x24 word: a monster of the lent monster
    /// world reads [`super::ActionHooks::vision_seen`]; other units ask
    /// [`Pending`].
    fn vision_seen(&self, unit: UnitId) -> Option<u32> {
        match self.h.monster_data(unit) {
            Some(m) => m
                .vision
                .map(|r| self.h.vision_seen.get(&r).copied().unwrap_or(0)),
            None => self.h.x.vision_seen(unit),
        }
    }
    /// §5.2 step 7 on the record: +0x24 := `value` (S == 0), written
    /// only when step 2 loaded the record (`0x005DDBE6`).
    fn mark_seen(&mut self, unit: UnitId, value: u32) {
        match self.h.monster_data(unit).map(|m| m.vision) {
            Some(Some(r)) => {
                self.h.vision_seen.insert(r, value);
            }
            Some(None) => {}
            None => self.h.x.mark_seen(unit, value),
        }
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
    /// Monster data +0x30: every monster gets its block at init
    /// (`0x00572BA0`, `world/npc.md` §2 r2, `monsters/init.md` §5), so any
    /// monster has one; other units ask [`Pending`].
    fn has_interaction_block(&self, unit: UnitId) -> bool {
        self.units
            .get(unit)
            .is_some_and(|r| r.ty == crate::units::UnitType::Monster)
            || self.h.x.has_interaction_block(unit)
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
    /// `0x00639DB0(unit, s, on)` (`ai-bodies.md` §9.26, `stat-lists.md`
    /// §9.2): s outside 0 … states count − 1 → nothing; else the toggle
    /// `0x00625A70`, then the update-queue insert `0x0064C040`
    /// (`unit-order.md` §6.2) whether or not the bit changed.
    fn set_state(&mut self, game: &mut Game, unit: UnitId, s: u16, on: bool) {
        if usize::from(s) >= self.stats.data().states.count() {
            return;
        }
        View::set_state(self, unit, s, on);
        if let Err(e) = game.lists.queue_update(unit) {
            self.h
                .errors
                .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                    e.into(),
                )));
        }
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
        crate::wiring::path::monsters::stage_request(self.h, unit, target, None);
        self.monster_set_mode(game, unit, u32::from(mode))
    }
    fn change_mode_path_byte(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        mode: u8,
        target: ModeTarget,
        path_byte: u8,
    ) -> bool {
        self.h.x.set_mode_target(unit, target);
        crate::wiring::path::monsters::stage_request(self.h, unit, target, Some(path_byte));
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
        path_byte: Option<u8>,
        velocity: &mut crate::monsters::ai::VelocityRequest,
    ) -> bool {
        if u32::from(mode) != MODE_GETHIT {
            let v = std::mem::take(velocity);
            crate::wiring::path::monsters::stage_velocity(self.h, unit, v);
        }
        match path_byte {
            Some(b) => self.change_mode_path_byte(game, unit, mode, target, b),
            None => self.change_mode(game, unit, mode, target),
        }
    }
    /// `0x00624690(unit, mode)` (`units.md` §4.1): no mode start.
    fn set_anim_mode(&mut self, game: &mut Game, unit: UnitId, mode: u8) {
        let r = {
            let mut sim = crate::units::hooks::Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::modes::write_mode(&mut sim, &mut *self.h, unit, u32::from(mode))
        };
        if let Err(e) = r {
            self.unit_error(e);
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
    /// `0x00553380(unit, sound, to)` ([`crate::units::sound::queue_sound`]:
    /// S→C 0x2C in the tick's client pass), then [`Pending::play_sound`].
    /// Recorded: `a2-npc-warriv-talk` frame 41, Jerhyn's greeting (§9.9
    /// interaction step 6) `2c 01 01000000 1200`.
    fn play_sound(&mut self, game: &mut Game, unit: UnitId, sound: u32, to: Option<UnitId>) {
        if let Ok(event) = u16::try_from(sound) {
            if let Err(e) = crate::units::sound::queue_sound(game, unit, event, to) {
                self.unit_error(crate::game::GameError::from(e).into());
            }
        }
        self.h.x.play_sound(game, unit, sound, to);
    }
    /// `0x005A8520` get-hit branch: mode change to get-hit (3).
    ///
    /// TODO(ai.md §1.2): the last-hit class 160 goes to a unit field the
    /// units spec does not describe; not stored.
    fn knockback_to_gethit(&mut self, game: &mut Game, unit: UnitId) {
        self.monster_set_mode(game, unit, MODE_GETHIT);
    }
    /// `0x005DE4E0`: mode 2 (walk) to [`crate::monsters::ai::radius_point`]
    /// with path step count 1, as the walks to coordinates (`ai.md` §7.2).
    /// A point on the unit's own cell (k = 0 or t on the unit) is still
    /// requested (§7.5 rule 8, REC-665): no path, neutral, and the think
    /// at f + `aidel`.
    fn walk_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        a: i32,
        b: i32,
        velocity: &mut crate::monsters::ai::VelocityRequest,
    ) -> bool {
        let at = self.h.path_position(unit);
        let to = self.h.path_position(target);
        let size = self.path_size(unit);
        let (x, y) = crate::monsters::ai::radius_point(at, size, to, a, b);
        AiModes::set_path_steps(self, unit, 1);
        self.change_mode_with(game, unit, 2, ModeTarget::Point(x, y), None, velocity)
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

/// The AI's line-of-sight mask (`draw-order-2.md` §15.1 caller table).
const LINE_MASK_AI: u16 = 4;

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
    /// `0x00622AA0(a, b, 4)` (`draw-order-2.md` §15.1) with the path
    /// provider ([`View::units_line_blocked`]); else [`Pending`].
    fn line_blocked(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        self.units_line_blocked(game, a, b, LINE_MASK_AI)
            .unwrap_or_else(|| self.h.x.line_blocked(game, a, b))
    }
    fn in_melee_range(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        match self.monster_in_melee_range(game, a, b) {
            Some(r) => r,
            None => self.h.x.in_melee_range(a, b, 0),
        }
    }
    /// `0x005DC640` (`ai.md` §6) with the path provider
    /// ([`crate::path::line::can_reach_directly`] on the path positions,
    /// `unit`'s size and room); else [`Pending`].
    fn can_reach_directly(&self, game: &Game, unit: UnitId, target: UnitId) -> bool {
        if self.h.paths.is_none() {
            return self.h.x.can_reach_directly(game, unit, target);
        }
        let (ax, ay) = self.h.path_position(unit);
        let size = self.path_size(unit);
        let b = self.h.path_position(target);
        let d = crate::monsters::ai::distance_full_size((ax, ay), size, b);
        let a = crate::path::line::LineUnit {
            room: game.lists.unit(unit).and_then(|e| e.room()),
            x: ax,
            y: ay,
            size,
        };
        crate::path::line::can_reach_directly(&self.h.drlg, &a, b, d)
    }
    fn find_spot(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32, RoomId)> {
        self.h.x.find_spot(game, unit)
    }
    /// Room +0x38..+0x44 (`0x0061AFA0`'s ring, [`super::monster_death`]):
    /// each slot's GUID as a monster.
    fn last_dead(&self, game: &Game, room: RoomId) -> [Option<UnitId>; 4] {
        game.lists.room(room).map_or([None; 4], |r| {
            r.dead_guids
                .map(|g| game.lists.find_unit(crate::units::UnitType::Monster, g))
        })
    }
    /// `0x005FD350(class, room, x, y, 0)` (`ai-bodies-4.md` §2 birth; the
    /// population's flag-1 form is `population.md` §9.3): by the class's
    /// `BaseId` the point tested and its mask; the room holding it
    /// (`0x00463740` from the unit's room) must exist and have no
    /// collision with a size-2 box (`0x0064D9B0`). Other classes pass.
    /// Without the path provider the host's answer.
    fn footprint_ok(&self, game: &Game, class: i32, room: Option<RoomId>, x: i32, y: i32) -> bool {
        if self.h.paths.is_none() {
            return self.h.x.footprint_ok(game, class, room, x, y);
        }
        let base = usize::try_from(class)
            .ok()
            .and_then(|c| self.h.tables.combat.monstats.get(c))
            .map(|m| i32::from(m.baseid as i16));
        let (px, py, mask) = match base {
            Some(206) => (x, y + 3, 0x3C01),
            Some(228) => (x, y + 2, 0x3C01),
            Some(298) => (x, y, 0x3C01),
            Some(334) => (x - 2, y - 2, 0x1C0),
            Some(528) => (x + 2, y + 4, 0x3C01),
            _ => return true,
        };
        let Some(r) = room.and_then(|r| self.h.drlg.find_room(game, r, px, py)) else {
            return false;
        };
        crate::path::collision::size_value(&self.h.drlg, Some(r), px, py, 2, mask) == 0
    }
}

impl<X: Pending> AiTargets for View<'_, X> {
    /// The host's lists (slot heads) followed by the nodes inserted
    /// through `0x005B1990` / `0x005B1900` ([`Game::target_nodes`]).
    fn target_nodes(&self, game: &Game) -> [Vec<UnitId>; 10] {
        let mut nodes = self.h.x.target_nodes(game);
        for (slot, list) in nodes.iter_mut().enumerate() {
            list.extend(game.target_nodes.slot(slot).iter().copied());
        }
        nodes
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
    /// `0x005DDC30` on the wired units ([`super::ai_scan`]).
    fn secondary_target(&mut self, game: &mut Game, unit: UnitId) -> (Option<UnitId>, i32, bool) {
        self.secondary_search(game, unit)
    }
    /// `0x005DDF20` (`ai.md` §5.3): scan 2 (mode 1, §5.4: the client
    /// players of the unit's room's near-room list, own room included, in
    /// list order) with the callback `0x005DDE80`: d := the full-size
    /// distance `0x005DC380` (the NPC's size subtracted per axis, clamped
    /// at 0); d > 15 → skip. An NPC without the monstats `interact` flag
    /// takes the first such player; with it, the quest active test
    /// (`world/quests.md` §6.4, [`AiSummons::npc_wants_interact`], which
    /// sends 0x8A on true) runs for each such player in scan order and
    /// the first true is taken. Taking stops the scan. "Close" when d < 4;
    /// the unit itself when none.
    fn nearest_player(&mut self, game: &mut Game, unit: UnitId) -> (UnitId, bool) {
        use crate::monsters::ai::AiSummons as _;
        use crate::path::collision::CollisionRooms;
        let Some(room) = game.lists.unit(unit).and_then(|e| e.room()) else {
            return (unit, false);
        };
        let d = &self.h.drlg;
        let mut rooms: Vec<RoomId> = (0..d.adjacent_count(room))
            .filter_map(|i| d.adjacent(room, i))
            .collect();
        if !rooms.contains(&room) {
            rooms.insert(0, room);
        }
        let players = super::dying::client_players(game);
        let at = self.h.path_position(unit);
        let size = self.path_size(unit);
        let interact = self.npc_interact(unit).is_some_and(|(_, i)| i);
        let mut scan = Vec::new();
        for r in rooms {
            scan.extend(
                game.lists
                    .room_units(r)
                    .into_iter()
                    .filter(|p| players.contains(p)),
            );
        }
        for p in scan {
            let dist = crate::monsters::ai::distance_full_size(at, size, self.h.path_position(p));
            if dist > 15 {
                continue;
            }
            if !interact || self.npc_wants_interact(game, p, unit) {
                return (p, dist < 4);
            }
        }
        (unit, false)
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
    /// The lent quest control's A2Q4 hooks (`world/quests-act2.md` §10);
    /// without one (a host without quests) [`Pending`]'s.
    fn jerhyn_palace_active(&mut self, game: &mut Game) -> bool {
        match self.h.quest_host.as_mut() {
            Some(q) => q.jerhyn_palace_active(),
            None => self.h.x.jerhyn_palace_active(game),
        }
    }
    fn jerhyn_npc_state(&mut self, game: &mut Game, unit: UnitId) -> JerhynStep {
        let Some(mut host) = self.h.quest_host.take() else {
            let (a, b) = self.h.x.jerhyn_npc_state(game, unit);
            return JerhynStep::Out(a, b);
        };
        let at = self.h.path_position(unit);
        let r = host.jerhyn_npc_state(game, self, at);
        self.h.quest_host = Some(host);
        r
    }
    fn jerhyn_placed(&mut self, _game: &mut Game) {
        if let Some(q) = self.h.quest_host.as_mut() {
            q.jerhyn_placed();
        }
    }
    fn guard_moving(&mut self, game: &mut Game, unit: UnitId) -> bool {
        match self.h.quest_host.as_mut() {
            Some(q) => q.guard_moving(),
            None => self.h.x.guard_moving(game, unit),
        }
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
        self.stats.max_mana(unit)
    }
    fn has_state_group(&self, unit: UnitId, g: u8) -> bool {
        self.stats.has_group(unit, usize::from(g))
    }
    fn states_count(&self) -> i32 {
        self.stats.data().states.count() as i32
    }
    fn has_list_flag(&self, unit: UnitId, flags: u32) -> bool {
        // `0x00625760`: the unit's list must be extended; then the first
        // list of the active (or parked) chain sharing a flag (`0x006256E0`).
        self.stats
            .unit_list(unit)
            .filter(|&r| self.stats.is_extended(r))
            .is_some_and(|r| self.stats.list_by_flags(r, flags).is_some())
    }
    fn hostile(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        self.h.x.ai_hostile(game, a, b)
    }
    /// `0x00552FD0`: the source-unit link (+0xC8 bit 0x400: the unit of
    /// type +0x94 and GUID +0x98), else the host's.
    fn owner(&self, game: &Game, unit: UnitId) -> Option<UnitId> {
        if let Some((ty, guid)) = self
            .units
            .get(unit)
            .filter(|r| r.flags2 & 0x400 != 0)
            .map(|r| r.source)
        {
            let ty = *crate::units::UnitType::ALL.get(ty as usize)?;
            return game.lists.find_unit(ty, guid);
        }
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
    /// A summoned monster's entry ([`ActionHooks::monster_skills`]): its
    /// base level (no bonus source on a monster entry); else the seam.
    fn skill_level(&self, unit: UnitId, skill: i32, highest: bool) -> Option<i32> {
        match self.h.monster_skills.get(&unit) {
            Some(m) => m.get(&skill).copied(),
            None => match self.h.natural_skills.get(&unit) {
                Some(m) => m.get(&skill).copied(),
                None => self.h.x.ai_skill_level(unit, skill, highest),
            },
        }
    }
    /// A summoned monster's entry: the id and its mode, fixed when the
    /// entry is created (`skills/use.md` §5.1: a monster's `monanim`).
    fn skill_entry(&self, unit: UnitId, skill: i32) -> Option<(i32, u8)> {
        match self.h.monster_skills.get(&unit) {
            Some(m) => {
                m.get(&skill)?;
                let mode = self.h.tables.skills.skill(skill)?.monanim;
                Some((skill, mode))
            }
            None => self.h.x.ai_skill_entry(unit, skill),
        }
    }
    /// A monster with a skill list (an assigned aura,
    /// [`Pending::monster_right_aura`]): its hand's entry, id and base
    /// level; else the seam.
    fn hand_skill(&self, unit: UnitId, right: bool) -> Option<(i32, i32)> {
        match self.h.skill_lists.get(&unit) {
            Some(l) => {
                let i = if right { l.right } else { l.left };
                i.and_then(|i| l.view().get(i).map(|e| (e.skill, e.base)))
            }
            None => self.h.x.ai_hand_skill(unit, right),
        }
    }
    /// `0x0056DEB0` + `0x005701B0`: an aura goes to
    /// [`Pending::monster_right_aura`] (`ai-bodies-2.md` §14 Duriel), any
    /// other skill to the seam.
    fn add_right_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32) {
        if self.h.tables.skills.skill(skill).is_some_and(|r| r.aura) {
            let mut sim = Sim {
                game,
                units: &mut *self.units,
                stats: &mut *self.stats,
                data: self.data,
            };
            X::monster_right_aura(&mut *self.h, &mut sim, unit, skill, level);
        } else {
            self.h.x.ai_add_right_skill(game, unit, skill, level);
        }
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
        if self.h.paths.is_none() {
            return self.h.x.ai_skill_check(game, unit, skill, target, x, y);
        }
        let world = SkillRooms { v: self, game };
        crate::monsters::ai::skill_check::skill_check(
            &world,
            &self.h.tables.skills.skills,
            unit,
            skill,
            target,
            x,
            y,
        )
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
    /// `0x00621DC0(unit, target x, target y)` → `0x0064FDC0` through the
    /// path provider ([`View::path_dir64`]); else the host's.
    fn direction64(&self, unit: UnitId, target: UnitId) -> i32 {
        let at = self.h.path_position(target);
        self.path_dir64(unit, at)
            .unwrap_or_else(|| self.h.x.ai_direction64(unit, target))
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
    /// The unit leaves its room and is removed (`0x00555600`, `units.md`
    /// §3.2: the caged barbarians at their portal, Baal at the stairs);
    /// every player is told (S→C 0x0A), as the quest host's removal.
    fn remove_unit(&mut self, game: &mut Game, unit: UnitId) {
        let Some((ty, guid)) = game.lists.unit(unit).map(|u| (u.ty as u8, u.guid)) else {
            return self.h.x.ai_remove_unit(game, unit);
        };
        let msg = crate::units::messages::remove_unit(ty, guid);
        for p in game.lists.units_of_type(crate::units::UnitType::Player) {
            self.h.x.send(p, &msg);
        }
        self.remove(game, unit);
    }
    fn link_clone(&mut self, game: &mut Game, unit: UnitId, clone: UnitId) {
        self.h.x.ai_link_clone(game, unit, clone);
    }
    /// `0x00574370` on the lent monster world (`init.md` §27); without
    /// one, the host's answer.
    fn reinit_class(&mut self, game: &mut Game, unit: UnitId, class: i32, mode: u8) {
        let mut sim = Sim {
            game,
            units: self.units,
            stats: self.stats,
            data: self.data,
        };
        let done = self
            .h
            .with_monster_world(|w, h| w.reinit(&mut sim, h, unit, class, u32::from(mode)));
        if done.is_none() {
            self.h.x.ai_reinit_class(sim.game, unit, class, mode);
        }
    }
    fn change_class_list(&mut self, game: &mut Game, unit: UnitId, class: i32) {
        self.h.x.ai_change_class_list(game, unit, class);
    }
    fn wisp_buff(&mut self, game: &mut Game, target: UnitId, value: i32, expire: i32) {
        self.h.x.ai_wisp_buff(game, target, value, expire);
    }
    /// `0x00571C00`: an 0xA4 record (class u16) on the unit, the unit
    /// queued for update (`intents-events.md` §7.9 rule 2).
    fn preload_class(&mut self, game: &mut Game, unit: UnitId, class: i32) {
        let r = super::event_records::EventRecord::Preload {
            class: class as u16,
        };
        self.h.event_records.push(unit, r);
        let _ = game.lists.queue_update(unit);
    }
    fn wisp_find(&mut self, game: &mut Game, unit: UnitId) -> Vec<UnitId> {
        self.h.x.ai_wisp_find(game, unit)
    }
    /// Wave `w` (0..=4) is superunique 61 + w (Baal Subject 1..5, the
    /// table `0x006E3528`; hcIdx map `0x00586B30` = identity, as
    /// `quests-act4.md` §5.4): its class from the drop tables'
    /// `superuniques` row, the mapped id `0x00659B80(2, ·)` = row +
    /// the `monstats` count (`quests-helpers.md` §2 r1). No drop tables
    /// or row: the host's answer (`Pending::ai_wave`).
    fn wave(&self, w: i32) -> Option<(i32, i32)> {
        let row = 61 + w;
        let found = (0..=4).contains(&w).then_some(()).and_then(|_| {
            let d = self.h.object_drops.as_ref()?;
            let su = d.tables.superuniques.get(row as usize)?;
            let count = self.h.tables.combat.monstats.len() as i32;
            Some((row + count, su.class as i32))
        });
        found.or_else(|| self.h.x.ai_wave(w))
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
    /// Path +0x18 / +0x1A of the unit's dynamic path (`0x00648A20` /
    /// `0x00648A30`); no dynamic path: (0, 0).
    fn path_final_point(&self, unit: UnitId) -> (i32, i32) {
        let p = self.h.paths.as_ref().and_then(|p| p.dynamic(unit));
        p.map_or((0, 0), |d| {
            let f = d.final_target();
            (f.x, f.y)
        })
    }
    /// Path +0x10 / +0x12 of the unit's dynamic path; none: (0, 0).
    fn path_target_point(&self, unit: UnitId) -> (i32, i32) {
        let p = self.h.paths.as_ref().and_then(|p| p.dynamic(unit));
        p.map_or((0, 0), |d| {
            let t = d.target();
            (t.x, t.y)
        })
    }
    /// `0x0061B130` → `0x0066CE30` (`drlg/levels.md` §11.4), as the
    /// population view's: the room holding the point among `room` and its
    /// adjacency array; none (or no room) → 0; that room's record at the
    /// point → its index; no record → −1.
    fn coord_index(&self, game: &Game, room: Option<RoomId>, x: i32, y: i32) -> i32 {
        let Some(at) = room.and_then(|r| self.h.drlg.find_room(game, r, x, y)) else {
            return 0;
        };
        self.h
            .drlg
            .drlg_room(game, at)
            .and_then(|(d, r)| d.coord_at(r, x, y))
            .map_or(-1, |c| c.index as i32)
    }
    /// The palace guard's door hooks from the lent quest control. The Act
    /// V prisoner AI's hooks (`quests-act5.md` §4.10): the reads from the
    /// quest control's published states
    /// ([`Pending::quest_rescue`]); the calls with an effect queued for
    /// it ([`Pending::queue_quest_event`]); `0x00588E10` read here. Other
    /// hooks keep the default.
    fn quest_hook(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        player: Option<UnitId>,
        hook: crate::monsters::ai::QuestHook,
    ) -> bool {
        use super::QuestEvent;
        use crate::monsters::ai::QuestHook;
        let guid = game.lists.unit(unit).map_or(0, |e| e.guid);
        match hook {
            // The palace guard's A2Q4 hooks (`world/quests-act2.md` §10)
            // on the lent quest control; none: false. PROVISIONAL
            // (REC-1633): `0x0059B8F0` ([`AiSummons::palace_guard_point`])
            // keeps its default, its return value is not stated.
            QuestHook::PalaceDoorOpen => self
                .h
                .quest_host
                .as_mut()
                .is_some_and(|q| q.palace_door_open()),
            QuestHook::PalaceGuardAside => self
                .h
                .quest_host
                .as_mut()
                .is_some_and(|q| q.palace_guard_aside()),
            QuestHook::WussieLeaving => self.h.x.quest_rescue(guid).0,
            QuestHook::WussieLeave => {
                self.h.x.queue_quest_event(QuestEvent::WussieLeft { guid });
                false
            }
            QuestHook::WussieCanRescue => self.dead_prison_door_near(game, player.unwrap_or(unit)),
            QuestHook::WussieRescue => {
                if let Some(player) = player {
                    self.h
                        .x
                        .queue_quest_event(QuestEvent::WussieRescue { player, unit });
                }
                false
            }
            QuestHook::WussieWait => {
                self.h.x.queue_quest_event(QuestEvent::WussieWait);
                false
            }
            _ => false,
        }
    }
    /// `0x00588D60`: the group's portal when spawned and existing.
    fn rescue_portal(&mut self, game: &mut Game, unit: UnitId) -> Option<Option<UnitId>> {
        let guid = game.lists.unit(unit)?.guid;
        let portal = self.h.x.quest_rescue(guid).1?;
        let o = game
            .lists
            .find_unit(crate::units::UnitType::Object, portal)?;
        Some(Some(o))
    }
    /// The quest active test `0x00544590(game, player, npc)`
    /// (`world/quests.md` §6.4) on the game's quest control, lent to the
    /// hooks while the tick runs ([`super::ActionHooks::quest_host`]):
    /// true when an active function wants the player to talk to `unit`
    /// (and 8A 01 <GUID> was sent). No lent control (a host without
    /// quests): false, no send.
    fn npc_wants_interact(&mut self, game: &mut Game, player: UnitId, unit: UnitId) -> bool {
        let Some((class, interact)) = self.npc_interact(unit) else {
            return false;
        };
        let Some(mut host) = self.h.quest_host.take() else {
            return false;
        };
        let r = host.npc_wants_interact(game, self, player, unit, class, interact);
        self.h.quest_host = Some(host);
        r
    }
    /// `0x00646CA0(unit, calc, skill, level)`: the calc column on the
    /// unit (`data/calc-expressions.md`, `skills/levels.md`).
    fn skill_calc(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        skill: i32,
        calc: u32,
        level: i32,
    ) -> i32 {
        let t = self.h.tables.clone();
        let mut cv = self.combat(game);
        crate::skills::eval_skill(&mut cv, &t.skills, Some(unit), calc, skill, level)
    }
    /// `0x00622AA0(a, b, mask)` with the path provider; else the seam's
    /// default (clear).
    fn line_blocked_mask(&self, game: &Game, a: UnitId, b: UnitId, mask: u16) -> bool {
        self.units_line_blocked(game, a, b, mask).unwrap_or(false)
    }
    fn set_unit_flags2(&mut self, unit: UnitId, mask: u32) {
        if let Some(r) = self.units.get_mut(unit) {
            r.flags2 |= mask;
        }
    }
    fn target_slot(&self, unit: UnitId) -> i32 {
        self.units.get(unit).map_or(11, |r| r.node_index as i32)
    }
    /// `0x005B1990(game, unit, 0, slot)`: the node at the head of list
    /// `slot`; unit +0xD0 := slot (`ai.md` §5.2).
    fn register_target_node(&mut self, game: &mut Game, unit: UnitId, slot: i32) {
        let Some(r) = self.units.get_mut(unit) else {
            return;
        };
        let Ok(index) = u8::try_from(slot) else {
            return;
        };
        r.node_index = index.into();
        game.target_nodes.push_front(slot, unit);
    }
    /// `0x00574BD0` from the published hireling facts: the `Id` of the
    /// unit's node when `owner` holds it.
    fn hireling_id(&self, _game: &Game, owner: UnitId, unit: UnitId) -> Option<i32> {
        match self.h.hireling_ai.ids.get(&unit) {
            Some(&(o, id)) if o == owner => i32::try_from(id).ok(),
            _ => None,
        }
    }
    /// `0x006562F0(expansion, id, level)` (`hirelings.md` §1.2 rule 2)
    /// on the published rows.
    fn hireling_row(&self, _game: &Game, id: i32, level: i32) -> Option<HireRow> {
        let rows = self.h.hireling_ai.rows.as_ref()?;
        let i = rows.row_at(self.data.expansion, u32::try_from(id).ok()?, level)?;
        rows.rows.get(i).map(|r| r.ai_row())
    }
}

impl<X: Pending> View<'_, X> {
    /// `0x00622C40(a, b, 0)` (`combat/hit.md` §7.2) for a monster `a`
    /// with the path provider and a monstats2 row: reach `MeleeRng` + 1
    /// against the unit distance `0x00641530` (`pathing.md` §9.5), then
    /// the collision line (mask 0x804). `None`: not answerable here (the
    /// host answers).
    ///
    /// PROVISIONAL (hit.md §7.3 step 3, REC-1110): `MeleeRng` 255 reads
    /// the unit's weapon class in its current mode; monsters carry no
    /// weapon here, so it is reach 0.
    fn monster_in_melee_range(&self, game: &Game, a: UnitId, b: UnitId) -> Option<bool> {
        let paths = self.h.paths.as_ref()?;
        let t = &self.h.tables.combat;
        let class = self
            .units
            .get(a)
            .filter(|r| r.ty == UnitType::Monster)?
            .class;
        let ex = t.monstats.get(usize::try_from(class).ok()?)?.monstatsex;
        let rng = t.monstats2.get(usize::from(ex))?.meleerng;
        let reach = if rng == 255 { 0 } else { i32::from(rng) };
        let dist = |a: UnitId, b: UnitId| {
            let pt = |u: UnitId| {
                let (x, y) = self.h.path_position(u);
                crate::path::Point { x, y }
            };
            crate::path::walk::geom::unit_distance(
                &paths.tables,
                pt(a),
                self.path_size(a),
                pt(b),
                self.path_size(b),
            )
        };
        // Step 2: the tentacle classes.
        let tentacle = self
            .units
            .get(b)
            .filter(|r| r.ty == UnitType::Monster)
            .and_then(|r| {
                t.monstats
                    .get(usize::try_from(r.class).ok()?)
                    .map(|m| m.baseid)
            });
        if matches!(tentacle, Some(258 | 261)) && reach + 8 > dist(a, b) {
            return Some(true);
        }
        // Step 3.
        let d = dist(a, b);
        if d <= 0 {
            return Some(true);
        }
        if reach + 1 < d {
            return Some(false);
        }
        Some(!self.units_line_blocked(game, a, b, 0x804).unwrap_or(false))
    }
}

/// `0x005FD470`'s world (`ai.md` §7.4) on the DRLG rooms and the path
/// records; used once the host has turned the path provider on.
struct SkillRooms<'a, 'v, X: Pending> {
    v: &'a View<'v, X>,
    game: &'a Game,
}

impl<X: Pending> crate::monsters::ai::skill_check::SkillCheckWorld for SkillRooms<'_, '_, X> {
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.v.h.path_position(unit)
    }
    fn room(&self, unit: UnitId) -> Option<RoomId> {
        self.game.lists.unit(unit).and_then(|e| e.room())
    }
    fn has_unit_flag(&self, unit: UnitId, bit: u32) -> bool {
        self.v.units.get(unit).is_some_and(|r| r.flags & bit != 0)
    }
    fn dead_or_dying(&self, unit: UnitId) -> bool {
        self.v
            .units
            .get(unit)
            .is_some_and(|r| r.mode == 0 || r.mode == 12)
    }
    fn in_town(&self, room: RoomId) -> bool {
        self.v.h.drlg.in_town(self.game, room)
    }
    fn pattern_free(&self, room: Option<RoomId>, unit: UnitId, x: i32, y: i32, mask: u16) -> bool {
        let pattern = self
            .v
            .h
            .paths
            .as_ref()
            .and_then(|p| p.dynamic(unit))
            .map_or(0, |d| d.pattern);
        !crate::path::collision::pattern_collides(&self.v.h.drlg, room, x, y, pattern, mask)
    }
    fn free_point_room(
        &self,
        room: Option<RoomId>,
        unit: UnitId,
        p: (i32, i32),
        mask: u16,
    ) -> Option<RoomId> {
        let mut pt = crate::path::Point::new(p.0, p.1);
        crate::path::search::free_point(
            &crate::wiring::path::place::Rooms(&self.v.h.drlg),
            room,
            &mut pt,
            self.v.path_size(unit),
            u32::from(mask),
            false,
        )
        .ok()
        .flatten()
    }
    fn line_clear(&self, from: (i32, i32), to: (i32, i32), room: RoomId, mask: u16) -> bool {
        !crate::path::line::line_test(
            &self.v.h.drlg,
            Some(room),
            crate::path::Point::new(from.0, from.1),
            crate::path::Point::new(to.0, to.1),
            mask,
        )
        .blocked()
    }
    fn room_at(&self, unit: UnitId, x: i32, y: i32) -> Option<RoomId> {
        let from = self.room(unit)?;
        self.v.h.drlg.find_room(self.game, from, x, y)
    }
    fn diab_prison_ok(&self, _: Option<RoomId>, _: Option<UnitId>) -> bool {
        // Skill 199 (DiabPrison): the placement test is not wired.
        false
    }
}
