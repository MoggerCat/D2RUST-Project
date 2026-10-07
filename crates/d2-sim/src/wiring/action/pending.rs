// Spec: specs/sim/units.md (path, unit fields), specs/combat/damage.md, specs/combat/hit.md, specs/missiles/missiles.md, specs/monsters/ai.md, specs/world/waypoints.md (seams with no provider yet)
//! The calls of the action seams whose owner spec is not written or not
//! implemented yet: path and position (`units.md` path, not written),
//! monster data (`monsters/init.md`, not implemented), items, skill use
//! (`skills/use.md`, not implemented), objects, sounds, messages, the
//! AI target sources (ai.md open question 7).
//!
//! The adapters of [`super`] call these where the real provider is
//! missing. Every default is the narrowest reading: nothing happens, or
//! the value that makes the caller do nothing. A host (or a test)
//! overrides what it can provide; [`NoPending`] takes every default.
//! When a provider lands, its methods move out of this trait into the
//! adapter that wires it.

use crate::game::Game;
use crate::monsters::ai::{ModeTarget, PortalNpc, QuestCall};
use crate::units::hooks::Sim;
use crate::units::{RoomId, UnitId, UnitType};

use super::ActionHooks;

/// A skill timer event the unit dispatch hands to the skills
/// (`stat-lists.md` §10.2, §10.3; `use.md` §7), with its arguments as the
/// dispatch passes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillEvent {
    /// Event 5 `0x0056D790`: `srvactivefunc` `f` of the skill's aura
    /// state (already checked < 191), the skill (a1) and a2.
    ActiveState {
        unit: UnitId,
        f: u16,
        skill: u32,
        arg2: u32,
    },
    /// Event 8 `0x0056FCB0` with its two arguments.
    Periodic { unit: UnitId, arg1: u32, arg2: u32 },
    /// Event 9 `0x0056FE40` after its checks: a1, the skill (a2) and the
    /// level `l` = total(151, layer = skill).
    ItemAura {
        unit: UnitId,
        arg1: u32,
        skill: u32,
        level: i32,
    },
}

/// A step of the kill `0x0057CCB0` (`damage.md` §7.2) whose callee has
/// no body in d2-sim, in the kill's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillStep {
    /// Pet death bookkeeping `0x005751A0` (step 1).
    PetCredit,
    /// The arena kill event `0x0053F720` (step 2).
    AttackerBookkeeping,
    /// The death mode faces the attacker (path direction, path spec).
    FaceAttacker,
    /// Quest kill parse (not run for a revived monster).
    QuestKill,
    /// The act 5 barricade doors (`objCol` monsters open object 571 /
    /// 572 at the same spot).
    BarricadeDoors,
}

/// Seams without a provider (see the module doc). Grouped by the spec
/// that will own them.
#[allow(unused_variables)]
pub trait Pending {
    // ---- animation (`formats/animdata.md` OQ2, `units.md` §4.3) -------

    /// The COF name the composer `0x0064F5B0` builds for a unit in a
    /// mode (token + mode + weapon class, NUL-padded to 8 bytes) for
    /// the AnimData lookup `0x0066A9B0` (`animdata.md` §5). The composer
    /// for players, objects and units with an inventory is
    /// `animdata.md` Open question 2. `None`: no name (no record).
    fn anim_name(&self, unit: UnitId, ty: UnitType, class: u32, mode: u32) -> Option<[u8; 8]> {
        None
    }
    /// The animation rate `0x00623F50` (unit +0x4C) from the AnimData
    /// speed (`None` when the unit has no record) and the rate stats and
    /// states (`units.md` §4.3; animation-rate spec, not written).
    fn anim_rate(&self, unit: UnitId, speed: Option<u32>) -> i16 {
        0
    }
    /// The frame bonus `0x00623B10` (table `0x006E8E60` by class and
    /// weapon type, `units.md` §4.3; animation-rate spec, not written).
    fn frame_bonus(&self, unit: UnitId) -> i32 {
        0
    }

    // ---- path and position (`units.md` path; not written) -------------

    /// Position in subtiles (path +0x2C).
    fn position(&self, unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// Places a unit at allocation (allocation arguments x, y).
    fn place(&mut self, unit: UnitId, x: i32, y: i32) {}
    /// Unit size (`0x00620510`).
    fn size(&self, unit: UnitId) -> i32 {
        0
    }
    /// The unit has a path (unit +0x2C).
    fn has_path(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x00648690`.
    fn set_velocity(&mut self, unit: UnitId, v: i32) {}
    fn velocity(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00648B90`.
    fn set_target_unit(&mut self, unit: UnitId, target: UnitId) {}
    /// `0x00648AD0`.
    fn set_target_point(&mut self, unit: UnitId, x: i32, y: i32) {}
    /// `0x00648C30`.
    fn set_footprint_mask(&mut self, unit: UnitId, mask: u16) {}
    /// `0x00648CE0`.
    fn set_move_mask(&mut self, unit: UnitId, mask: u16) {}
    /// `0x00649970`.
    fn build_path(&mut self, game: &mut Game, unit: UnitId) {}
    /// Path +0x88, +0x84.
    fn set_acceleration(&mut self, unit: UnitId, accel: i32, max_velocity: i32) {}
    /// `0x006417F0`.
    fn target_distance(&self, unit: UnitId) -> i32 {
        0
    }
    /// Unit step `0x00554CA0`: false when it returns 2 (no movement).
    fn step(&mut self, game: &mut Game, unit: UnitId) -> bool {
        false
    }
    /// The collision word cached in the path by the last step (path
    /// +0x54, `missiles.md` §R4 step 6). `None`: not modelled; the adapter
    /// then reads the room's grid at the current position.
    fn cached_collision_word(&self, unit: UnitId) -> Option<u16> {
        None
    }
    /// `0x00648F40`: subtiles crossed by the last step, in path order.
    fn crossed_subtiles(&self, unit: UnitId) -> Vec<(i32, i32)> {
        Vec::new()
    }
    /// The target point or unit handed to a mode start (AI mode requests).
    fn set_mode_target(&mut self, unit: UnitId, target: ModeTarget) {}
    /// The path step count (AI): the stop distance `0x00649070`
    /// (`ai.md` §7.5 rule 7) without the path provider. Default: nothing.
    fn set_path_steps(&mut self, unit: UnitId, steps: i32) {}
    /// Path flag 0x800 (blocked step).
    fn path_blocked(&self, unit: UnitId) -> bool {
        false
    }
    /// Stops the unit's path (`0x00648730`, `pathing.md` §13.1 rule 3)
    /// without the path provider. Default: nothing.
    fn stop_path(&mut self, unit: UnitId) {}
    /// `0x005DE6D0` → `0x005DE4E0` walk in radius; false = failed.
    fn walk_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        a: i32,
        b: i32,
    ) -> bool {
        false
    }

    // ---- unit queries without a spec rule ------------------------------

    /// Hostility `0x00554200` (not specified; `skills/use.md` names it).
    fn may_attack(&self, attacker: UnitId, defender: UnitId) -> bool {
        false
    }
    /// Alignment `0x006259B0`: 0 evil, 1 neutral, 2 good.
    fn alignment(&self, unit: UnitId) -> u8 {
        0
    }
    /// `0x005A0180(unit, mask)`: monster type flags (2 superunique, 4
    /// champion, 8 unique). Monster data (`monsters/init.md`).
    fn monster_flag(&self, unit: UnitId, mask: u32) -> bool {
        false
    }
    /// `0x0063E9F0`.
    fn is_boss(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0063E940`.
    fn is_demon(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0063E990`.
    fn is_undead(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0063EDC0`.
    fn is_prime_evil(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x00451F30`: unit flag 0x80000000.
    fn is_revived(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x00622D00`.
    fn moving_mode(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x00622870`.
    fn melee_range(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00622C40(a, d, range)`.
    fn in_melee_range(&self, a: UnitId, d: UnitId, range: i32) -> bool {
        false
    }
    /// `0x00622AA0(a, b, 4)`: blocked line.
    fn line_blocked(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        false
    }
    /// `0x005DC640`.
    fn can_reach_directly(&self, game: &Game, unit: UnitId, target: UnitId) -> bool {
        false
    }
    /// `0x0054DC40`: a free teleport spot and its room.
    fn find_spot(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32, RoomId)> {
        None
    }
    /// `0x0061AA40`: the room's "LOS draw" test.
    fn los_draw(&self, game: &Game, room: RoomId) -> bool {
        false
    }
    /// Room +0x38..+0x44: the four last-dead units.
    fn last_dead(&self, game: &Game, room: RoomId) -> [Option<UnitId>; 4] {
        [None; 4]
    }
    /// `0x0046C140(class, mode)`: the monster class has the mode.
    fn class_has_mode(&self, class: i32, mode: u8) -> bool {
        false
    }
    /// `0x00645270`: the unit type the unit's mode converts to; `None` =
    /// the unit's own type.
    fn converted_type(&self, unit: UnitId) -> Option<i32> {
        None
    }
    /// `0x0057A830`.
    fn montype_matches(&self, layer: u16, montype: i32) -> bool {
        false
    }
    /// `0x005738F0(n)` (`monsters/init.md` §9; not implemented).
    fn player_count_bonus(&self, players: i32) -> i32 {
        0
    }

    // ---- monster data (`monsters/init.md`; not implemented) ------------

    /// Monster level.
    fn monster_level(&self, unit: UnitId) -> i32 {
        0
    }
    /// Monster data `dwAiState`.
    fn ai_state(&self, unit: UnitId) -> u32 {
        0
    }
    /// Monster data +0x50 vision record's +0x24.
    fn vision_seen(&self, unit: UnitId) -> Option<u32> {
        None
    }
    fn mark_seen(&mut self, unit: UnitId) {}
    /// Type-10 handler `0x00573120`: monster data +0x34, +0x38 := 0.
    fn ai_reset(&mut self, unit: UnitId) {}
    /// `0x00572DC0`.
    fn interacting(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x00535060` without its interaction part (a cursor item, player
    /// data +0x4C ≠ 0): the interact info is the unit record's
    /// ([`crate::units::record::InteractInfo`]), which the callers test
    /// first.
    fn busy(&self, unit: UnitId) -> bool {
        false
    }
    /// Monster data +0x30, the NPC interaction block (`world/npc.md` §2).
    fn has_interaction_block(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x00572DE0`: `player` is in the NPC's interaction list.
    fn in_interaction_list(&self, npc: UnitId, player: UnitId) -> bool {
        false
    }
    /// `0x00553540`: the unit's path target unit.
    fn path_target(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// Path facing `0x00648820(path, dir)`.
    fn set_facing(&mut self, unit: UnitId, dir: i32) {}
    /// `0x005FD350(class, room, x, y, 0)`, the monster footprint test
    /// (`population.md` §9); `false` (no room) by default.
    fn footprint_ok(&self, game: &Game, class: i32, room: Option<RoomId>, x: i32, y: i32) -> bool {
        false
    }
    /// SandRaider's help search (scan 1, `ai-bodies.md` §9.26 step 4).
    fn nearest_evil_monster(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId> {
        None
    }

    // ---- the Act II–V AI bodies (`monsters/ai-bodies-2.md`..`-5.md`;
    // [`crate::monsters::ai::AiActs`]) -------------------------------------

    /// Max mana `0x00625D60`.
    fn ai_max_mana(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00625760(unit, flags)`: an active stat list with the flags.
    fn ai_has_list_flag(&self, unit: UnitId, flags: u32) -> bool {
        false
    }
    /// `0x00554200(game, a, b)`: b hostile to a (`combat/hit.md`).
    fn ai_hostile(&self, game: &Game, a: UnitId, b: UnitId) -> bool {
        false
    }
    /// `0x00552FD0`: the unit's owner.
    fn ai_owner(&self, game: &Game, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x0058F090`: a monster's owner record (type, GUID).
    fn ai_owner_record(&self, unit: UnitId) -> Option<(i32, u32)> {
        None
    }
    /// `0x0065C310` on the player's quest record of the difficulty.
    fn ai_quest_flag(&self, player: UnitId, difficulty: u8, quest: i32, flag: i32) -> bool {
        false
    }
    /// `0x005353F0`: the player's portal GUID.
    fn ai_portal_guid(&self, player: UnitId) -> Option<u32> {
        None
    }
    /// Monster data `nComponent[i]`.
    fn ai_component(&self, unit: UnitId, i: usize) -> u8 {
        0
    }
    /// `0x00553010`: the unit's target unit.
    fn ai_target_unit(&self, game: &Game, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x00573090`: target override.
    fn ai_set_target_override(&mut self, unit: UnitId, kind: i32, guid: u32) {}
    /// `0x006510C0(class, 0, 0)`: the monstats chain byte +0x4B.
    fn ai_chain_index(&self, class: i32) -> i32 {
        0
    }
    /// `0x0063EC70(room, class)`; `class` itself by default.
    fn ai_class_for_level(&self, game: &Game, room: Option<RoomId>, class: i32) -> i32 {
        class
    }
    /// Skill level with bonus of the unit's (highest) entry.
    fn ai_skill_level(&self, unit: UnitId, skill: i32, highest: bool) -> Option<i32> {
        None
    }
    /// The unit's highest skill entry: id and mode.
    fn ai_skill_entry(&self, unit: UnitId, skill: i32) -> Option<(i32, u8)> {
        None
    }
    /// Left / right skill: id and level.
    fn ai_hand_skill(&self, unit: UnitId, right: bool) -> Option<(i32, i32)> {
        None
    }
    /// `0x0056DEB0` + `0x005701B0`.
    fn ai_add_right_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32) {}
    /// `0x00647280`.
    fn ai_assign_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32) {}
    /// `0x00644560`; false (no entry) by default.
    fn ai_set_skill_param(&mut self, unit: UnitId, skill: i32, value: i32) -> bool {
        false
    }
    /// `0x005FD470(skill, target, x, y)`.
    fn ai_skill_check(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        skill: i32,
        target: Option<UnitId>,
        x: i32,
        y: i32,
    ) -> bool {
        false
    }
    /// `0x0056E390`.
    fn ai_corpse_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: Option<UnitId>,
        skill: i32,
        level: i32,
    ) -> Option<UnitId> {
        None
    }
    /// Path pattern (path +0x48).
    fn ai_path_pattern(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00649190`.
    fn ai_set_path_pattern(&mut self, unit: UnitId, pattern: i32) {}
    /// `0x00554EA0`; false by default.
    fn ai_place_unit(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        room: Option<RoomId>,
        x: i32,
        y: i32,
    ) -> bool {
        false
    }
    /// `0x0064EA90`.
    fn ai_stamp_pattern(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        pattern: i32,
        mask: u16,
    ) {
    }
    /// `0x0064CBE0`.
    fn ai_clear_cell(&mut self, game: &mut Game, room: Option<RoomId>, x: i32, y: i32, bits: u16) {}
    /// `0x0064CB30`.
    fn ai_point_collides(
        &self,
        game: &Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        mask: u16,
    ) -> bool {
        false
    }
    /// `0x0064D910` with a given pattern at the unit's position.
    fn ai_pattern_collides(&self, game: &Game, unit: UnitId, pattern: i32, mask: u16) -> bool {
        false
    }
    /// `0x0064E7B0`.
    fn ai_free_point(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        size: i32,
    ) -> Option<(i32, i32)> {
        None
    }
    /// `0x0054DC40` with a class and a start point.
    fn ai_free_spot_for(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        class: i32,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32)> {
        None
    }
    /// `0x00463740`.
    fn ai_room_at(&self, game: &Game, unit: UnitId, x: i32, y: i32) -> Option<RoomId> {
        None
    }
    /// `0x005DE4E0(target, mode, a, b)`.
    fn ai_move_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        mode: u8,
        a: i32,
        b: i32,
    ) -> bool {
        false
    }
    /// `0x00620C10`.
    fn ai_set_path_target(&mut self, unit: UnitId, target: UnitId) {}
    /// Path type 2 and compute toward the target; whether it has points.
    fn ai_path_has_points(&mut self, game: &mut Game, unit: UnitId, target: UnitId) -> bool {
        false
    }
    /// `0x00621DC0`.
    fn ai_direction64(&self, unit: UnitId, target: UnitId) -> i32 {
        0
    }
    /// `0x005B2F20`; nothing spawned by default.
    #[allow(clippy::too_many_arguments)]
    fn ai_spawn_monster(
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
        None
    }
    /// SandMaggotQueen's spawn class (`ai-bodies-2.md` OQ4); −1.
    fn ai_queen_spawn_class(&self, unit: UnitId) -> i32 {
        -1
    }
    /// `0x0057CCB0(game, unit, killer, 1)`.
    fn ai_kill(&mut self, game: &mut Game, unit: UnitId, killer: Option<UnitId>) {}
    /// The unit leaves its room and is removed.
    fn ai_remove_unit(&mut self, game: &mut Game, unit: UnitId) {}
    /// Baal clone owner links.
    fn ai_link_clone(&mut self, game: &mut Game, unit: UnitId, clone: UnitId) {}
    /// `0x00574370`.
    fn ai_reinit_class(&mut self, game: &mut Game, unit: UnitId, class: i32, mode: u8) {}
    /// BaalThrone's change-class stat list.
    fn ai_change_class_list(&mut self, game: &mut Game, unit: UnitId, class: i32) {}
    /// WillOWisp's magic-find stat list.
    fn ai_wisp_buff(&mut self, game: &mut Game, target: UnitId, value: i32, expire: i32) {}
    /// S→C 0xA4.
    fn ai_preload_class(&mut self, game: &mut Game, unit: UnitId, class: i32) {}
    /// WillOWisp's unit find; nothing found by default.
    fn ai_wisp_find(&mut self, game: &mut Game, unit: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    /// BaalThrone wave record: (mapped superunique, class).
    fn ai_wave(&self, w: i32) -> Option<(i32, i32)> {
        None
    }
    /// `0x0061AED0(room, 0)`.
    fn ai_clear_room_portal_flag(&mut self, game: &mut Game, room: Option<RoomId>) {}
    /// A quest call of the Act II–V bodies; false by default.
    fn ai_quest_call(&mut self, game: &mut Game, unit: UnitId, call: QuestCall) -> bool {
        false
    }
    /// The unit's last attacker (`0x00621D50`; unit field not described).
    fn set_last_attacker(&mut self, defender: UnitId, attacker: UnitId) {}
    /// `0x005A4390(game, attacker)` after a monster's hit.
    fn monster_hit_hook(&mut self, attacker: UnitId) {}
    /// `0x005D6410(defender)` after a non-lethal hit on a monster.
    fn monster_damaged_hook(&mut self, defender: UnitId) {}
    /// Unique-mod hook `0x005A43B0` for a missile's monster owner.
    fn unique_mod_missile(&mut self, game: &mut Game, owner: UnitId, missile: UnitId) {}

    // ---- AI targets and skills (ai.md OQ6, OQ7; skills/use.md) ---------

    /// Game +0x10F8 target-node lists.
    fn target_nodes(&self, game: &Game) -> [Vec<UnitId>; 10] {
        Default::default()
    }
    /// `0x005DD610`.
    fn forced_target(&mut self, game: &mut Game, unit: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    /// ai.md §5.2 step 4.
    fn good_target_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        los: bool,
    ) -> Option<(UnitId, i32)> {
        None
    }
    /// `0x005DD510`.
    fn choose_alternative(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        main: Option<UnitId>,
        alt: UnitId,
    ) -> bool {
        false
    }
    /// `0x005DDC30`.
    fn secondary_target(&mut self, game: &mut Game, unit: UnitId) -> (Option<UnitId>, i32, bool) {
        (None, 0x7FFF_FFFF, false)
    }
    /// `0x005DDF20`: the NPC itself when none.
    fn nearest_player(&mut self, game: &mut Game, unit: UnitId) -> (UnitId, bool) {
        (unit, false)
    }
    /// Scan 8: a door.
    fn find_door(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// objects.txt `MonsterOK`.
    fn door_monster_ok(&self, door: UnitId) -> bool {
        false
    }
    /// Scan 11.
    fn special_walk_target(&mut self, game: &mut Game, unit: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    /// FallenShaman corpse scan.
    fn shaman_corpses(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        max_sq: i32,
        own_minions: bool,
    ) -> (Option<UnitId>, u32) {
        (None, 0)
    }
    // ---- AI quest calls (ai-bodies.md §9.32; world/quests.md) -----------------

    /// Portal coordinates set up; `true` (nothing to report) by default.
    fn portal_setup(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) -> bool {
        true
    }
    fn spawn_town_portal(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) {}
    /// The out-of-town portal; `false` (none spawned) by default.
    fn spawn_outside_portal(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) -> bool {
        false
    }
    fn portal_coords(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        npc: PortalNpc,
    ) -> Option<(i32, i32)> {
        None
    }
    fn drehya_update(&mut self, game: &mut Game) {}
    fn drehya_wait(&mut self, game: &mut Game) -> bool {
        false
    }
    /// The Npc class cases (`ai-bodies.md` §9.9 step 2); defaults: no quest
    /// state (jerhyn's palace inactive, nothing brought or found).
    fn jerhyn_palace_active(&mut self, game: &mut Game) -> bool {
        false
    }
    fn jerhyn_npc_state(&mut self, game: &mut Game, unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    fn guard_moving(&mut self, game: &mut Game, unit: UnitId) -> bool {
        false
    }
    fn alkor_bird(&mut self, game: &mut Game) -> bool {
        false
    }
    fn alkor_reset(&mut self, game: &mut Game) {}
    fn ormus_altar(&mut self, game: &mut Game) -> Option<(i32, i32)> {
        None
    }
    fn ormus_set_altar_mode(&mut self, game: &mut Game) {}
    fn cain_town_coords(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32)> {
        None
    }
    fn cain_in_town_activated(&mut self, game: &mut Game, unit: UnitId) {}
    fn anya_open_portal(&mut self, game: &mut Game, unit: UnitId) {}

    /// `0x005FD470(skill, target)`.
    fn skill_usable(&mut self, game: &mut Game, unit: UnitId, skill: i32, target: UnitId) -> bool {
        false
    }
    /// Current skill := `skill` (unit skill list); false when out of range.
    fn set_current_skill(&mut self, unit: UnitId, skill: i32) -> bool {
        false
    }
    /// Missile damage setup `0x0059F900` (skills spec).
    fn missile_damage_setup(
        &mut self,
        game: &mut Game,
        owner: UnitId,
        origin: Option<UnitId>,
        missile: UnitId,
        level: i32,
    ) {
    }
    /// A missile parameter record's init callback with an id no spec
    /// names (the specified ones run in `missiles::init_cb`, §R2.3 step
    /// 21).
    fn missile_init_callback(&mut self, game: &mut Game, missile: UnitId, callback: u32, arg: u32) {
    }
    /// The curse helper `0x0056E970` (skills spec).
    #[allow(clippy::too_many_arguments)]
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
    }
    /// Thorns `0x005D10C0` (skills spec).
    fn thorns(&mut self, a: UnitId, d: UnitId, record: &mut crate::combat::DamageRecord) {}

    // ---- unit events, reaction, overlays (units.md, damage.md §7, §8) --

    /// `0x005C0C30(game, event, unit, other, record)` for a host without
    /// the event registry: with [`ActionHooks::unit_events`] set
    /// (`ActionHooks::enable_unit_events`, a host with
    /// [`crate::wiring::interaction::UseRest`]) the iteration runs
    /// [`crate::combat::events::run`] on [`ActionHooks::handlers`] and this
    /// seam is not called. Event 0 (hit by missile) comes with no record
    /// and possibly no unit.
    fn unit_event(
        &mut self,
        event: u8,
        unit: Option<UnitId>,
        other: Option<UnitId>,
        record: Option<&mut crate::combat::DamageRecord>,
    ) {
    }
    /// The global layer split of the item event registrations (data
    /// +0xC6C shift, +0xC70 mask; `items/properties.md` §5 rule 9,
    /// `data/runtime-maps.md` §3), used when the hooks have no body
    /// tables. Default: the 1.14d values (6, 0x3F).
    fn event_layer_split(&self) -> (u32, u32) {
        (6, 0x3F)
    }
    /// Terror install `0x005DDD00(game, source, unit, skill, a, b)`
    /// (`monsters/ai.md`; `combat/events.md` §2.8).
    fn event_terror(
        &mut self,
        game: &mut Game,
        source: UnitId,
        unit: UnitId,
        skill: i32,
        a: i32,
        b: i32,
    ) {
    }
    /// `0x0064D870(U's room, x, y, U's pattern, U's collision mask)` = 0
    /// (`combat/events.md` §2.7; collision).
    fn event_point_free(&self, unit: UnitId, at: (i32, i32)) -> bool {
        false
    }
    /// The corpse find of item target 3 (`0x005FD9C0`,
    /// `combat/events.md` §3): the first unit of the unit find around
    /// `t0`, radius 10, flags 0x1002, callback `0x00645680`.
    fn event_corpse_near(&mut self, game: &mut Game, t0: UnitId) -> Option<UnitId> {
        None
    }
    /// S→C 0x99 / 0x9A on the unit's message list and the update queue
    /// (`0x005717C0` / `0x00571840`, `0x0064C040`).
    fn queue_item_cast(&mut self, unit: UnitId, msg: crate::combat::events::ItemCastMsg) {}
    /// The raise test `0x00645510(V, 0)` (`combat/events.md` §2.21).
    fn raise_test(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0064EC10(V's room, V x, V y, V pattern, 0x8000)`.
    fn clear_pattern(&mut self, unit: UnitId) {}
    /// A step of the Reanimate raise on the new monster
    /// (`combat/events.md` §2.21) with no body in d2-sim.
    fn raise_step(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        step: crate::combat::events::RaiseStep<UnitId>,
    ) {
    }
    // ---- player death (`combat/vitals.md` §4.6–§4.7, [`super::death`]) --

    /// The stash limit `0x00623460` (§4.6 rule 1; not written).
    fn stash_cap(&self, unit: UnitId) -> i32 {
        0
    }
    /// The death's gold drop `0x00535510(game, P, P's GUID, amount)`
    /// (`items/inventory.md` §7.22).
    fn death_drop_gold(&mut self, game: &mut Game, unit: UnitId, amount: i32) {}
    /// Corpse creation `0x0057F700` without its experience (§4.7 rule
    /// 1): a new player-type unit of P's class in mode 17 holding P's
    /// items (`items/inventory.md`). Default: none.
    fn create_corpse(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// The corpse's owner GUID from its inventory (`0x0063D450`).
    fn corpse_owner_guid(&self, corpse: UnitId) -> Option<u32> {
        None
    }
    /// `0x0055B300(owner, P, 1)` for the corpse's player owner
    /// (`0x0057FAF0`'s second test).
    fn corpse_loot_allowed(&self, corpse: UnitId, unit: UnitId) -> bool {
        false
    }

    /// Reaction `0x0057CEE0` (`damage.md` §7.1, call level only; its mode
    /// changes and the kill `0x0057CCB0` are not specified in full).
    fn reaction(&mut self, a: UnitId, d: UnitId, record: &mut crate::combat::DamageRecord) {}
    /// Overlay `0x00621E40`.
    fn overlay(&mut self, unit: UnitId, id: i32) {}
    /// `0x00623F50` animation rate refresh.
    fn refresh_anim_rate(&mut self, unit: UnitId) {}
    /// Monster sound.
    fn play_sound(&mut self, game: &mut Game, unit: UnitId, sound: u32, to: Option<UnitId>) {}
    /// Operate a door (`0x00584540`, objects spec) in a game without an
    /// object state (with one: [`super::objects`]).
    fn operate_door(&mut self, game: &mut Game, unit: UnitId, door: UnitId) {}

    // ---- items (items group) -------------------------------------------

    /// Item/skill getter `0x00625500`.
    fn item_stat(&self, unit: UnitId, stat: u16, layer: u16) -> i32 {
        0
    }
    /// Unit skill list (unit +0xA8).
    fn skill_list(&self, unit: UnitId) -> Vec<crate::skills::SkillEntry> {
        Vec::new()
    }
    /// `0x00620250`.
    fn used_skill(&self, unit: UnitId) -> Option<crate::skills::SkillEntry> {
        None
    }
    /// `0x00535BC0`.
    fn current_weapon(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x00623990(unit, 0)`.
    fn weapon(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    fn item_at(&self, unit: UnitId, loc: u8) -> Option<UnitId> {
        None
    }
    fn item_is(&self, item: UnitId, itype: i32) -> bool {
        false
    }
    fn itype_is(&self, itype: i32, parent: i32) -> bool {
        false
    }
    fn wield_type(&self, item: UnitId) -> i32 {
        0
    }
    fn item_damage(&self, item: UnitId, max: bool) -> i32 {
        0
    }
    fn str_dex_bonus(&self, item: UnitId) -> (i32, i32) {
        (0, 0)
    }
    fn item_flag_throw(&self, item: UnitId) -> bool {
        false
    }
    fn item_has_durability(&self, item: UnitId) -> bool {
        false
    }
    fn durability_loss(&mut self, owner: UnitId, item: UnitId) {}
    /// `0x0063C8F0`.
    fn has_shield(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x006225F0` composit shield.
    fn composit_shield(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0064F380`.
    fn weapon_class(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00623C20`.
    fn weapon_hit_class(&self, unit: UnitId) -> u32 {
        0
    }
    /// `0x00535D10` / `0x00535E20`.
    fn dual_wield_switch(&mut self, a: UnitId, offhand: bool, on: bool) {}

    // ---- objects, interaction, messages (waypoints seam) ---------------

    /// `0x00624690` object mode change of an object without object data
    /// (with data: [`super::objects`], `objects.md` §4).
    fn set_object_mode(&mut self, game: &mut Game, object: UnitId, mode: u8) {}
    /// waypoints.md §6.1 host clock (never true in single player).
    fn hostile_delay(&self, player: UnitId) -> bool {
        false
    }
    /// `0x00553380`.
    fn attach_sound(&mut self, unit: UnitId, event: u8) {}
    /// A message to the player's client (server transport).
    fn send(&mut self, player: UnitId, msg: &[u8]) {}
    /// `0x0053AEC0` warp (level change / act change and placement).
    fn warp(&mut self, game: &mut Game, player: UnitId, level: u32, tile_code: u8) {}
    /// `0x005809D0(game, player, no skill, 2, x, y, 0)` (player path modes).
    fn set_player_mode_arrival(&mut self, game: &mut Game, player: UnitId) {}

    // ---- objects (`world/objects.md`; `ObjectWorld` seams) ------------

    /// `0x00620A70`: stamp an object's footprint. The path provider has
    /// no objects.txt shape for objects (`wiring::path` `path_shape`) and
    /// the function is not in `path-placement.md`.
    fn object_stamp_footprint(&mut self, game: &mut Game, object: UnitId) {}
    /// `0x00623830`: free an object's footprint (as above).
    fn object_free_footprint(&mut self, game: &mut Game, object: UnitId) {}
    /// Attach object sound `id` (`objects.md` §14; `0x00571740` when
    /// `now`). Sounds spec, not written.
    fn object_sound(&mut self, unit: UnitId, id: u8, to: Option<UnitId>, now: bool) {}
    /// `0x0055F140`: the key test and use (inventory). Default: no key.
    fn object_key_test(&mut self, player: UnitId) -> bool {
        false
    }
    /// `0x00623660`: the operator is in interact range of the object
    /// (path; not in `path-placement.md`). Default: out of range (the
    /// operate entry `objects.md` §7.1 rule 3 returns before the
    /// dispatch).
    fn object_in_range(&self, game: &Game, operator: UnitId, object: UnitId) -> bool {
        false
    }
    /// Player data +0x4C ≠ 0 (`objects.md` §7.2 rule 2).
    fn object_player_busy(&self, player: UnitId) -> bool {
        false
    }
    /// An item is on the player's cursor (inventory).
    fn object_cursor_item(&self, player: UnitId) -> bool {
        false
    }
    /// `0x0061AEB0`: the act II staff-tomb level (quest spec). Default: a
    /// level id no level has.
    fn object_staff_tomb(&self) -> u32 {
        u32::MAX
    }
    /// The portal travel's host facts and steps (`objects.md` §12 rules
    /// 4–13; [`crate::world::objects::MiscWorld`]): the party id
    /// (`0x00554630`, default 0xFFFF none).
    fn object_party_id(&self, unit: UnitId) -> u16 {
        0xFFFF
    }
    /// `0x00553720`: the partner portal. Default: none.
    fn object_portal_partner(&mut self, game: &mut Game, object: UnitId) -> Option<UnitId> {
        None
    }
    /// The player's quest record for the difficulty exists. Default: no.
    fn object_quest_record(&self, player: UnitId) -> bool {
        false
    }
    /// `0x0065C310(Q, q, bit)`. Default: clear.
    fn object_quest_bit(&self, player: UnitId, quest: u32, bit: u8) -> bool {
        false
    }
    /// `0x005353F0`: player data +0x48. Default 0.
    fn object_portal_guid(&self, player: UnitId) -> u32 {
        0
    }
    /// `0x0061B060(act, level, 0, &x, &y, 3)`. Default: none.
    fn object_level_spawn(&mut self, game: &mut Game, level: u32) -> Option<(RoomId, i32, i32)> {
        None
    }
    /// `0x00543B90(game, from, to, P)`. Default: nothing.
    fn object_quest_level_change(&mut self, player: UnitId, from: u32, to: u32) {}
    /// A portal's removal (§12 rule 12). Default: nothing.
    fn object_remove_portal(&mut self, game: &mut Game, object: UnitId) {}
    /// `0x0058CF50(game, L)`. Default: nothing.
    fn object_portal_act5(&mut self, partner: UnitId) {}
    /// State 102 on P until `expire` (§12 rule 13). Default: nothing.
    fn object_just_portaled(&mut self, game: &mut Game, player: UnitId, expire: i32) {}
    /// What the object module handed back without running it: quest,
    /// waypoint and `todo` inits, operates and events, uncovered presets
    /// ([`super::objects::ObjectRoute`]).
    fn object_route(&mut self, game: &mut Game, route: super::objects::ObjectRoute) {}
    /// A class-59 portal's owner for S→C 0x82 (`intents-events.md`
    /// §7.2): (owner GUID, owner name, the paired portal's GUID or
    /// 0xFFFF_FFFF). Default: unknown (nothing is sent).
    fn portal_owner(&self, object: UnitId) -> Option<(u32, Vec<u8>, u32)> {
        None
    }
    /// The skill messages 0x21 of a monster's add (`intents-events.md`
    /// §7.2): for each `monstats` skill slot i = 0..7 whose bit i of row
    /// +0x16C is set, whose skill id (row +0x170 + 2i) is valid and which
    /// the unit has: (skill, base level, bonus level). Default: none.
    fn monster_add_skills(&self, unit: UnitId) -> Vec<(u16, u8, u8)> {
        Vec::new()
    }
    /// The inventory messages `0x00534F80(unit, client)`
    /// (`intents-events.md` §7.9 rule 4) to `receiver`'s client.
    /// Default: nothing.
    fn inventory_messages(&mut self, receiver: UnitId, unit: UnitId) {}
    /// The GUID of the unit's minion owner (`0x0058F0D0`) when it is a
    /// player whose pet type of the unit is 7 (hireable, `sim/pets.md`
    /// §9; `monsters/init.md` §24 rule 4). Default: none.
    fn hireling_owner_guid(&self, unit: UnitId) -> Option<u32> {
        None
    }
    /// The unit's owner fields (type +0x94, id +0x98). Default: none.
    fn unit_owner(&self, unit: UnitId) -> Option<(u32, u32)> {
        None
    }
    /// The C→S 0x13 object case's reach step (`waypoints.md` §5.2,
    /// `0x00548B00`: distance > 50 → refuse; in range and unobstructed →
    /// stop the player and operate; else walk and operate on arrival;
    /// owner: the object-interaction spec, not written). Default:
    /// operate (the operate entry's own range test,
    /// [`Pending::object_in_range`], still applies).
    fn object_approach(
        &mut self,
        game: &mut Game,
        player: UnitId,
        object: UnitId,
    ) -> super::objects::ObjectReach {
        super::objects::ObjectReach::Operate
    }

    // ---- the kill and the death (`damage.md` §7.2, `treasure.md` §3) ---

    /// A step of the kill with no written body ([`KillStep`]).
    fn kill_step(&mut self, game: &mut Game, step: KillStep, defender: UnitId, attacker: UnitId) {}
    /// `0x0057E7B0` for a monster attacker (`vitals.md` §4.4 step 2):
    /// its owner, or the owner of a flag-0x800 stat list on A, then D.
    /// Default: none (no experience).
    fn kill_credited_player(&self, attacker: UnitId, defender: UnitId) -> Option<UnitId> {
        None
    }
    /// `vitals.md` §4.4 step 3: player `p`'s hireling share
    /// (`world/hirelings.md` §7.1 rule 2). Default: nothing.
    fn kill_hireling_share(&mut self, p: UnitId, attacker: UnitId, defender: UnitId, e: i32) {}
    /// `0x00554630(P)` ≠ 0xFFFF. Default: not in a party (single player).
    fn kill_in_party(&self, p: UnitId) -> bool {
        false
    }
    /// The party share's kept members (`vitals.md` §4.4 rule 6).
    /// Default: P only.
    fn kill_party_members(&self, p: UnitId, defender: UnitId) -> Vec<UnitId> {
        vec![p]
    }
    /// `0x005A03A0`: the monster's superunique index (hcIdx ≠ −1).
    /// Monster data (`monsters/init.md`).
    fn superunique(&self, unit: UnitId) -> Option<u16> {
        None
    }
    /// `0x0058F0D0`: the unit's minion owner (units spec, not written).
    fn minion_owner(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x005408E0`: the party count the drop walk reads for `unit`
    /// (party, not written).
    fn party_size(&self, unit: UnitId) -> Option<i32> {
        None
    }
    /// `treasure.md` §3.3 for recipient `r`: the quest owner `P` is a
    /// player whose quest flags for the difficulty have none of 15, 1 and
    /// `cp` (quests and owner resolution `0x0058F0D0`, `0x0063A690`,
    /// `0x00552F60`). False: no quest TC.
    fn quest_tc_open(&self, r: UnitId, cp: u8) -> bool {
        false
    }

    // ---- vitals (`combat/vitals.md` §3; the rest of `VitalsRest`) ------

    /// `0x0064C040` after a strength / dexterity change (not specified).
    fn stats_refresh(&mut self, unit: UnitId) {}
    /// `vitals.md` §3 step 7 (party roster, sound, broadcast, callbacks).
    fn level_up_notify(&mut self, unit: UnitId) {}
    /// Unit event 12 `levelup` (`0x005C0C30`, event registry).
    fn level_up_event(&mut self, unit: UnitId) {}

    /// The monster death start `0x005A6FF0` (mode table, `units.md`
    /// §4.6) run by the monster mode set with the mode change's
    /// `target` (`ActionHooks::mode_target`). Its body is not written
    /// beyond two callees: the drop gate and drop (`treasure.md` §3.1,
    /// `crate::wiring::economy::monster_death_drop`) and the evil-killed
    /// count `0x00547E50` (`population.md` §13 item 3). A host that holds the
    /// economy state overrides this. Returns whether the mode started;
    /// default: started, nothing done (as every other start function).
    fn monster_death_start(
        h: &mut ActionHooks<Self>,
        sim: &mut Sim<'_>,
        unit: UnitId,
        target: Option<UnitId>,
    ) -> bool
    where
        Self: Sized,
    {
        true
    }

    // ---- the monster mode message and the death end (`intents-events.md` §7.4, §7.7)

    /// Unit +0xB0, read as e of a mode-0 and mode-3 message and f of a
    /// mode-13 message (§7.4 rule 5). Its writers are not specified
    /// (`stat-lists.md` §10: the regeneration kill sets it to 0;
    /// `audio/triggers.md` OQ3 reads the client copy as the hit class of
    /// the last hit). Default: 0.
    fn unit_b0(&self, unit: UnitId) -> u8 {
        0
    }
    /// `0x005A0180(unit, 0x100)`, which sets bit 0x80 of a mode-3
    /// message's d (§7.4 rule 5; not specified). Default: false.
    fn monster_flag_100(&self, unit: UnitId) -> bool {
        false
    }
    /// The animation refresh `0x00623E00` of the stepping death (§7.7
    /// rule 3, base id 78; not specified).
    fn refresh_animation(&mut self, game: &mut Game, unit: UnitId) {}
    /// The `monstats` `SplEndDeath` action of the death end (§7.7 rule
    /// 3): 1 → `0x00574370(game, unit, minion, 1)` then `0x00573780`; 2 →
    /// the kill `0x0057CCB0` of `0x00552FD0(unit)`. Neither callee is
    /// specified. Default: nothing.
    fn death_end_action(&mut self, game: &mut Game, unit: UnitId, action: u8, minion: u16) {}

    // ---- skill bodies (`skills/bodies.md`; their other systems) ---------

    /// `0x00554DE0`: allies (same unit after the monster owner
    /// resolution, or two players in one party; units / party, not
    /// written). Default: the same unit.
    fn allied(&self, a: UnitId, b: UnitId) -> bool {
        a == b
    }
    /// `0x00574A20(unit, pet GUID)` and the pettype `unsummon` bit: the
    /// player's pet may be unsummoned (pets, not written).
    fn pet_unsummonable(&self, unit: UnitId, pet: UnitId) -> bool {
        false
    }
    /// Used skill entry param `i` (1: +0x18 `0x00644560`, 2:
    /// `0x006445A0`; the skill list's owner).
    fn set_entry_param(&mut self, unit: UnitId, i: u8, v: i32) {}
    /// `0x0064F060` through the disguise remap `0x00645270`: the composit
    /// weapon class (items / composits).
    fn composit_weapon_class(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x00623C60`: a player's hand class (items).
    fn hand_class(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x0062E6F0`: the item's type has a `shoots` value (items).
    fn item_shoots(&self, item: UnitId) -> bool {
        false
    }
    /// `0x006289F0`: items `stackable` (items).
    fn item_stackable(&self, item: UnitId) -> bool {
        false
    }
    /// `0x006295B0`: the maximum stack (items).
    fn item_max_stack(&self, item: UnitId) -> i32 {
        0
    }
    /// `0x00625E00`: the maximum durability (items). `None`: unknown, the
    /// durability is left as it is.
    fn item_max_durability(&self, item: UnitId) -> Option<i32> {
        None
    }
    /// `0x00558580(game, item)`: the quantity-replenish timer
    /// (`items/generation.md`).
    fn quantity_timer(&mut self, game: &mut Game, item: UnitId) {}
    /// Message 0x3E (item stat) to the player's client.
    fn send_item_stat(&mut self, unit: UnitId, item: UnitId, stat: u16, value: i32) {}
    /// `0x00580310(game, unit)`: attack-mode cleanup (`bodies.md` OQ4).
    fn attack_cleanup(&mut self, unit: UnitId) {}
    /// `0x00580380(game, unit)` (`bodies.md` OQ4).
    fn weapon_cleanup(&mut self, unit: UnitId) {}
    /// `0x00646F20(unit)`: passive refresh.
    fn passive_refresh(&mut self, unit: UnitId) {}
    /// `0x0056DE40(unit)`: the buff callback's refresh.
    fn buff_refresh(&mut self, unit: UnitId) {}
    /// `0x00575900(game, unit)`: a player's skill resync (`bodies.md` OQ5).
    fn skill_resync(&mut self, unit: UnitId) {}
    /// `0x00646D60(unit, entry)` after a passive state is switched on.
    fn passive_state_apply(&mut self, unit: UnitId, entry: &crate::skills::SkillEntry) {}
    /// `0x005B0E00(game, unit, AI control or none, k)` (`monsters/ai.md`
    /// §3.3) from the curse bodies.
    fn set_ai_state(&mut self, unit: UnitId, k: i32) {}
    /// `0x005D2B60`: aura mana under blood mana (`levels.md` OQ8).
    fn blood_mana(&mut self, unit: UnitId, cost: i32) {}
    /// `0x00571AA0`: message 0xA3 queued on the unit.
    fn queue_progressive(
        &mut self,
        unit: UnitId,
        msg: crate::skills::use_::bodies::ProgressiveMsg<UnitId>,
    ) {
    }

    // ---- skill bodies, batch 2 and 3 (`bodies.md` §6–§8, `bodies-2.md`, `bodies-2b.md`) --

    /// A call of the bodies into a system with no provider here
    /// ([`crate::skills::use_::bodies::BodyEffect`]). Default: nothing.
    fn body_effect(&mut self, e: crate::skills::use_::bodies::BodyEffect<UnitId, UnitId, RoomId>) {}
    /// A path operation ([`crate::skills::use_::bodies::PathOp`]; paths of
    /// skill moves are not provided). Default: nothing, compute 0.
    fn body_path_op(
        &mut self,
        unit: UnitId,
        op: crate::skills::use_::bodies::PathOp<UnitId>,
    ) -> i32 {
        0
    }
    /// The L-flag (game +0x6A or +0x74 ≠ 0, `monsters/init.md` §8.1).
    /// Default: false.
    fn l_flag(&self) -> bool {
        false
    }
    /// Frame event index (unit +0x38 bits 8+). Default: 0.
    fn frame_event_index(&self, unit: UnitId) -> i32 {
        0
    }
    /// Frame event index set (`0x006212C0`).
    fn set_frame_event_index(&mut self, unit: UnitId, i: i32) {}
    /// `0x0058F710`: a monster AI control's spawn class. Default: none.
    fn minion_spawn_class(&self, unit: UnitId) -> Option<i32> {
        None
    }
    /// The owner's linked unit (`0x00554070`). Default: none.
    fn linked_unit(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x00553010`: the death delay's killer. Default: none.
    fn killer_of(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x0058F090`: the minion owner record (GUID, type). Default: none.
    fn minion_owner_ident(&self, unit: UnitId) -> Option<(u32, u32)> {
        None
    }
    /// `0x005C0BE0`: a handler with this key and skill field. Default:
    /// none.
    fn has_handler(&self, unit: UnitId, key_type: i32, key: i32, skill: i32) -> bool {
        false
    }
    /// Param `i` (1…4) of the unit's skill entry. Default: 0.
    fn entry_param(&self, unit: UnitId, e: &crate::skills::SkillEntry, i: u8) -> i32 {
        0
    }
    /// Param `i` of the unit's skill entry set.
    fn set_entry_param_of(&mut self, unit: UnitId, e: &crate::skills::SkillEntry, i: u8, v: i32) {}
    /// Entry flags +0x0C. Default: 0.
    fn entry_flags(&self, unit: UnitId, e: &crate::skills::SkillEntry) -> u32 {
        0
    }
    /// Entry flags set.
    fn set_entry_flags(&mut self, unit: UnitId, e: &crate::skills::SkillEntry, f: u32) {}
    /// Entry mode set (`0x00644340`).
    fn set_entry_mode(&mut self, unit: UnitId, e: &crate::skills::SkillEntry, m: u32) {}
    /// `0x00645270`: the disguise remap. Default: the mode unchanged.
    fn disguise_mode(&self, unit: UnitId, m: u32) -> u32 {
        m
    }
    /// The used skill's sequence records. Default: none.
    fn skill_sequence(&self, unit: UnitId) -> Option<Vec<[u8; 6]>> {
        None
    }
    /// `0x0056E210` → `0x00553B10(game, unit, p)`.
    fn anim_rewind(&mut self, unit: UnitId, p: i32) {}
    /// `0x00553C70(game, unit, v)`.
    fn anim_restart(&mut self, unit: UnitId, v: i32) {}
    /// `0x00553DC0(game, unit, f)`.
    fn anim_from(&mut self, unit: UnitId, f: i32) {}
    /// The room level's act (`0x0061A1B0`, `0x006427F0`). Default: 0.
    fn room_act(&self, room: RoomId) -> i32 {
        0
    }
    /// The room level's `Teleport`. Default: no level record.
    fn room_teleport(&self, room: RoomId) -> Option<i32> {
        None
    }
    /// `0x0064E7B0`: the free point. Default: none.
    fn free_point(
        &mut self,
        room: RoomId,
        at: (i32, i32),
        size: i32,
        mask: u32,
        fallback: bool,
    ) -> Option<(RoomId, (i32, i32))> {
        None
    }
    /// `0x0064D910` ≠ 0. Default: collides.
    fn pattern_collides(&self, room: RoomId, at: (i32, i32), unit: UnitId, mask: u32) -> bool {
        true
    }
    /// `0x0064D800` ≠ 0. Default: collides.
    fn box_collides(&self, room: RoomId, at: (i32, i32), size: i32, mask: u32) -> bool {
        true
    }
    /// `0x0064E260` ≠ 0. Default: blocked.
    fn body_line_blocked(&self, room: RoomId, from: (i32, i32), to: (i32, i32), mask: u32) -> bool {
        true
    }
    /// `0x00554EA0(game, unit, room, x, y, 0, 0)`. Default: not placed.
    fn place_unit(&mut self, unit: UnitId, room: Option<RoomId>, at: (i32, i32)) -> bool {
        false
    }
    /// `0x006487D0`. Default: 0.
    fn path_point_count(&self, unit: UnitId) -> i32 {
        0
    }
    /// The path's last point. Default: (0, 0).
    fn path_last_point(&self, unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// The path's target point. Default: (0, 0).
    fn path_target_point(&self, unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// `0x005B2F20(game, room, x, y, class, mode, spread, 0x42)`.
    /// Default: none.
    fn create_monster(
        &mut self,
        room: RoomId,
        at: (i32, i32),
        class: i32,
        mode: i32,
        spread: i32,
    ) -> Option<UnitId> {
        None
    }
    /// Monster mode request (`0x005A7E60`, `0x005A7C20`). Default: 0.
    fn mode_request(&mut self, unit: UnitId, mode: i32, target: Option<UnitId>) -> i32 {
        0
    }
    /// `0x00535060`: busy. Default: false.
    fn inventory_busy(&self, unit: UnitId) -> bool {
        false
    }
    /// The unit has an inventory. Default: none.
    fn has_inventory(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0063BEF0`. Default: none.
    fn weapon_in_use(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x00627D40`. Default: 0.
    fn body_loc(&self, item: UnitId) -> i32 {
        0
    }
    /// `0x0062A4E0`. Default: false.
    fn item_usable(&self, item: UnitId) -> bool {
        false
    }
    /// `0x00625820(item, 0)`. Default: false.
    fn item_active(&self, item: UnitId) -> bool {
        false
    }
    /// `0x00629930`. Default: false.
    fn item_breakable(&self, item: UnitId) -> bool {
        false
    }
    /// `0x0063C8F0`: the shield. Default: none.
    fn shield(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// armor `mindam` / `maxdam`. Default: no record.
    fn shield_damage(&self, item: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// weapons `missiletype`. Default: 0.
    fn item_missile_type(&self, item: UnitId) -> i32 {
        0
    }
    /// The Iron Golem item test (`bodies-2b.md` §7.11). Default: false.
    fn golem_item(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0062EA80`. Default: 0.
    fn item_first_loc(&self, unit: UnitId) -> i32 {
        0
    }
    /// `0x0063C050(inventory, 6 / 5)` both `mele`. Default: false.
    fn two_melee_weapons(&self, unit: UnitId) -> bool {
        false
    }
    /// `0x0062A710`: attack frames. Default: none (fatal).
    fn attack_frames(&self, unit: UnitId, item: UnitId) -> Option<i32> {
        None
    }

    // ---- skill bodies, batch 4 (`bodies-3.md`, `bodies-4.md`) --

    /// `0x00621DC0`: the 64-step direction from the unit to (x, y)
    /// (`sim/pathing.md` §8.3). Default: 0.
    fn body_dir64(&self, unit: UnitId, at: (i32, i32)) -> i32 {
        0
    }
    /// A missile's total frames. Default: 0.
    fn body_missile_frames(&self, missile: UnitId) -> i32 {
        0
    }
    /// A missile's total frames and frames left (`0x0064A2B0`,
    /// `0x0064A330`). Default: nothing.
    fn body_set_missile_frames(&mut self, missile: UnitId, total: i32, left: i32) {}
    /// `0x00621920`: an action event in the frames (a, b]. Default: none.
    fn body_action_event_between(&self, unit: UnitId, a: i32, b: i32) -> bool {
        false
    }
    /// The books row of an item's spell index: (`scrollskill`,
    /// `bookskill`). Default: none.
    fn body_book_skills(&self, item: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// The unit's inventory nodes (item, node kind) in list order.
    /// Default: none.
    fn body_inventory_nodes(&self, unit: UnitId) -> Vec<(UnitId, i32)> {
        Vec::new()
    }
    /// The unit find of `missiles/bodies-2.md` §44. Default: nothing found.
    fn body_unit_find(&self, room: RoomId, at: (i32, i32), r: i32, f: u32) -> Vec<UnitId> {
        Vec::new()
    }
    /// `0x0064CB30(room, x, y, mask)`. Default: collides.
    fn body_point_collides(&self, room: RoomId, at: (i32, i32), mask: u32) -> bool {
        true
    }
    /// A monster creation entry point (`monsters/init.md` §1). Default:
    /// none.
    /// `0x00554EA0(game, unit, room, x, y, a, 0)` with a ≠ 0. Default: not
    /// placed.
    fn body_place_unit_flag(
        &mut self,
        unit: UnitId,
        room: Option<RoomId>,
        at: (i32, i32),
        a: i32,
    ) -> bool {
        false
    }
    fn body_spawn_monster(
        &mut self,
        q: crate::skills::use_::bodies::MonsterSpawn<UnitId, RoomId>,
    ) -> Option<UnitId> {
        None
    }
    /// `0x00627910` for the kick damage: weapon lists off / back on
    /// (`levels.md` §3.5).
    fn toggle_weapon_lists(&mut self, unit: UnitId, on: bool) {}
    /// Items record bytes +0xFE / +0xFF of the boots. Default: (0, 0).
    fn boots_damage(&self, item: UnitId) -> (i32, i32) {
        (0, 0)
    }

    /// The units `scan_unit(game, owner, x, y, r, f, …, noaura 0)`
    /// (`skills/bodies.md` §2.12) accepts, in order, for the missile
    /// area bodies (`missiles.md` §R9.6). The scan runs on the skill use
    /// view (`UseView`, which needs `UseRest`); the action view has no
    /// provider. Default: none.
    fn missile_area_units(
        &mut self,
        game: &Game,
        owner: UnitId,
        at: (i32, i32),
        r: i32,
        f: u32,
    ) -> Vec<UnitId> {
        Vec::new()
    }

    // ---- skill timer events (`stat-lists.md` §10.2, §10.3; `use.md` §7) --

    /// Routes timer events 5, 8 and 9 to the skill use pipeline. The
    /// pipeline's own seams ([`crate::wiring::interaction::UseRest`]) are a
    /// wider bound than `Pending`, so the route is chosen by the seam
    /// value: a value that also implements `UseRest` sets this to
    /// [`crate::wiring::interaction::skill_events::route`]. Default: nothing.
    fn skill_event(h: &mut ActionHooks<Self>, sim: &mut Sim<'_>, ev: SkillEvent)
    where
        Self: Sized,
    {
    }

    /// Player event 0 in an attack, cast or skill mode: the action frame
    /// `0x00580460` (`units.md` §4.5) with the event's (a1, a2); returns
    /// the action result (2 runs the ENDANIM handler at once). A seam
    /// value that also implements
    /// [`crate::wiring::interaction::UseRest`] routes it to
    /// [`crate::wiring::interaction::skill_events::action_frame`]
    /// (`use.md` §5.2). Default: 1, nothing done.
    fn action_frame(
        h: &mut ActionHooks<Self>,
        sim: &mut Sim<'_>,
        unit: UnitId,
        a1: u32,
        a2: u32,
    ) -> u32
    where
        Self: Sized,
    {
        1
    }
    /// The skill start `0x0056FAF0` of a monster's attack / skill start
    /// and sequence start (`units.md` §4.6 rules 7, 10); its result. A
    /// [`crate::wiring::interaction::UseRest`] value routes it to
    /// [`crate::wiring::interaction::skill_events::monster_skill_start`]
    /// (`use.md` §5.3). Default: 0 (no skill pipeline: no used skill).
    fn monster_skill_start(h: &mut ActionHooks<Self>, sim: &mut Sim<'_>, unit: UnitId) -> i32
    where
        Self: Sized,
    {
        0
    }
    /// The skill part of the monster sequence event 0 `0x005A8670`
    /// (`units.md` §4.6 rule 13, before the animation refresh): E flags,
    /// the moving skill's step and the do `0x0056FC50` by frame code. A
    /// [`crate::wiring::interaction::UseRest`] value routes it to
    /// [`crate::wiring::interaction::skill_events::monster_sequence_frame`].
    /// Default: nothing.
    fn monster_sequence_frame(h: &mut ActionHooks<Self>, sim: &mut Sim<'_>, unit: UnitId)
    where
        Self: Sized,
    {
    }

    // ---- client intents (`sim/intents-events.md` §9; d2-server's
    // `handlers::player`) -------------------------------------------------

    /// `0x00413490(text, −1)` (C→S 0x14, §9 rule 3): non-zero ends the
    /// handler with nothing done. What it tests is not stated. Default:
    /// 0 (the text passes).
    fn overhead_text_test(&self, text: &[u8]) -> bool {
        false
    }
    /// The player's overhead record (unit +0xA4) replaced (`0x006611A0`
    /// free, `0x00661110(game +0x1C, text, frame)` new, byte +8 :=
    /// `byte8`, `0x00661230`; §9 rule 3). Its timeout frame is
    /// `UnitRecord::hover`, which the caller sets; the record's other
    /// contents (hover/chat spec, S→C 0x26 §7.9) live here. Default:
    /// nothing kept.
    fn replace_overhead(&mut self, player: UnitId, text: &[u8], byte8: u8, end: i32) {}
    /// The items refresh `0x0055FDE0` of tick step 1 (`sim/tick.md` §3:
    /// an act's environment report, before its 0x53). Default: nothing.
    fn environment_refresh_items(&mut self, player: UnitId) {}
    /// The room clean-up's client part for a player (`intents-events.md`
    /// §7.5 step 7, `0x0053FA90`: the client record's +0x34 → +4 := 0).
    /// Default: nothing (no client record model).
    fn client_cleanup(&mut self, player: UnitId) {}
    /// The text (`0x006611E0`) and byte +8 of the unit's overhead record
    /// (unit +0xA4), for the overhead 0x26 of `intents-events.md` §7.9
    /// rule 3. Default: none kept (nothing is sent).
    fn overhead_record(&self, unit: UnitId) -> Option<(Vec<u8>, u8)> {
        None
    }
    /// `0x0055B300(a, b, flag)`: a's relation to b has `flag` (party /
    /// hostility flags). Default: no relation.
    fn player_relation(&self, a: UnitId, b: UnitId, flag: u32) -> bool {
        false
    }
    /// `0x005845D0(game, player, GUID)`: the door highlight of C→S 0x3D
    /// (§9 rule 4, open question 15). Default: nothing.
    fn highlight_door(&mut self, game: &mut Game, player: UnitId, guid: u32) {}
    /// Client flag 4 (`0x00538670`, hardcore) of the player's client (§9
    /// rule 6). Default: softcore.
    fn client_hardcore(&self, player: UnitId) -> bool {
        false
    }
    /// `0x0052CAF0(game, client, reason)`: drop the player's client
    /// (`tools/original-hooks.md` §6.1 rule 3; session code). Default:
    /// nothing.
    fn drop_client(&mut self, player: UnitId, reason: u32) {}
    /// The SetStat message part of `0x00548520` (S→C 0x1D–0x1F, after the
    /// stat is set). Default: nothing sent.
    fn stat_sent(&mut self, player: UnitId, stat: u16, value: u32) {}
    /// C→S 0x41's last step (§9 rule 6): the left skill (`0x00620190`)
    /// re-selected with EDX = 1, then the right skill (`0x006201D0`) with
    /// EDX = 0 (`0x005701B0`; owner: the skill list, `UseRest`).
    /// Default: nothing.
    fn reselect_hand_skills(&mut self, game: &mut Game, player: UnitId) {}
    /// `0x005678A0(…, 1)`: the player is trading (C→S 0x44, §9 rule 7).
    /// Default: not trading.
    fn player_trading(&self, player: UnitId) -> bool {
        false
    }
    /// `0x00549520(game, player, object, item, action)` and its result
    /// (C→S 0x44, `world/quests-act2.md` §8.6, `quests-act2-2.md` §3.2).
    /// `None` (the default): no provider, the id stays a stub.
    fn staff_in_orifice(
        &mut self,
        game: &mut Game,
        player: UnitId,
        object: u32,
        item: u32,
        action: u16,
    ) -> Option<u32> {
        None
    }
    /// `0x00574EC0(game, player, 7, 0)`: the player's hireling (§9 rule
    /// 8; `hirelings.md` §5 rule 4; the wired host answers it from the
    /// hireling list). Default: none.
    fn player_hireling(&self, player: UnitId) -> Option<UnitId> {
        None
    }
    /// Player data +0x4C := 0 (`0x005350B0`, C→S 0x48, §9 rule 9; read
    /// by [`Pending::object_player_busy`]). Default: nothing.
    fn clear_player_busy(&mut self, player: UnitId) {}
    /// `0x005724C0`: clear NPC `class`'s intro bit in the player's record
    /// for `difficulty` (C→S 0x4D, §9 rule 11; which of the two bit fields
    /// of `quests.md` §6.7 is not stated). Default: nothing.
    fn clear_npc_intro(&mut self, player: UnitId, difficulty: u8, class: u16) {}
    /// `0x005616A0(game, player, &fail)`: the weapon switch of C→S 0x60
    /// (§9 rule 14, open question 16): (result, fail). `None` (the
    /// default): no provider, the id stays a stub.
    fn weapon_switch(&mut self, game: &mut Game, player: UnitId) -> Option<(u32, bool)> {
        None
    }
}

/// Every default.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPending;

impl Pending for NoPending {}
