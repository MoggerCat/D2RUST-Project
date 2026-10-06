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
use crate::monsters::ai::{ModeTarget, PortalNpc};
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

/// A step of the kill `0x0057CCB0` with no written body (`damage.md`
/// §7.2 lists them at call level only), in the order the spec lists
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillStep {
    /// Pet kill credit to a player owner.
    PetCredit,
    /// Attacker bookkeeping and the arena kill event.
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
    /// The path step count (AI).
    fn set_path_steps(&mut self, unit: UnitId, steps: i32) {}
    /// Path flag 0x800 (blocked step).
    fn path_blocked(&self, unit: UnitId) -> bool {
        false
    }
    /// Stops the unit's path.
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
    /// `0x00535060`.
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
    /// SandRaider's help search (scan 1, `ai.md` §9.26 step 4).
    fn nearest_evil_monster(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId> {
        None
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
    // ---- AI quest calls (ai.md §9.32; world/quests.md) -----------------

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
    /// The Npc class cases (`ai.md` §9.9 step 2); defaults: no quest
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
    /// A missile parameter record's init callback (skills spec).
    fn missile_init_callback(&mut self, game: &mut Game, missile: UnitId, callback: u32, arg: u32) {
    }
    /// Server-damage function `index` 1…14 (`0x0073C960`, skills spec):
    /// adjusts the missile's damage record.
    fn srv_dmg(
        &mut self,
        game: &mut Game,
        index: i16,
        missile: UnitId,
        unit: UnitId,
        damage: &mut crate::missiles::Damage,
    ) {
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

    /// `0x005C0C30(game, event, unit, other, record)`: the unit's event
    /// functions (registered by items; no registry yet). Event 0 (hit by
    /// missile) comes with no record and possibly no unit.
    fn unit_event(
        &mut self,
        event: u8,
        unit: Option<UnitId>,
        other: Option<UnitId>,
        record: Option<&mut crate::combat::DamageRecord>,
    ) {
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
    /// Operate a door (`0x00584540`, objects spec).
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

    /// `0x00624690` object mode change (objects spec).
    fn set_object_mode(&mut self, game: &mut Game, object: UnitId, mode: u8) {}
    /// `0x00554120`.
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {}
    /// `0x00554190`.
    fn reset_interact(&mut self, player: UnitId) {}
    /// `0x00554D00`.
    fn interact_guid(&self, player: UnitId) -> Option<u32> {
        None
    }
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

    // ---- the kill and the death (`damage.md` §7.2, `treasure.md` §3) ---

    /// A step of the kill with no written body ([`KillStep`]).
    fn kill_step(&mut self, game: &mut Game, step: KillStep, defender: UnitId, attacker: UnitId) {}
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

    // ---- skill bodies, batch 2 and 3 (`bodies.md` §6–§8, `bodies-2.md`) --

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
    /// The Iron Golem item test (`bodies-2.md` §7.11). Default: false.
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
}

/// Every default.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPending;

impl Pending for NoPending {}
