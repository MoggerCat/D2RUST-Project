// Spec: specs/monsters/ai.md (Related specs; the seams to other systems)
//! What AI code needs from systems owned by other specs. Each trait names
//! its expected provider; tests use small fakes. The decisions (draws,
//! tests, delays) stay in the AI modules.

use crate::game::Game;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};

/// Where a mode request points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeTarget {
    Unit(UnitId),
    Point(i32, i32),
}

/// Monster and unit fields. Provider: the units/stats session
/// (`sim/units.md`, `sim/stats.md`, `sim/stat-lists.md`,
/// `monsters/init.md` for monster data).
pub trait AiUnits {
    /// The unit seed (unit +0x20).
    fn seed(&mut self, unit: UnitId) -> &mut Seed;
    /// Unit class (monstats row for monsters).
    fn class(&self, unit: UnitId) -> i32;
    /// Animation mode (unit +0x10).
    fn anim_mode(&self, unit: UnitId) -> u8;
    fn has_state(&self, unit: UnitId, state: u16) -> bool;
    /// `0x005544B0(unit, 0)` minus the timer part: clears state 54 (and
    /// state 92 for players). The AI code cancels the type-2 events.
    fn clear_uninterruptable(&mut self, game: &mut Game, unit: UnitId);
    /// `0x005541B0`: the unit is dead.
    fn is_dead(&self, unit: UnitId) -> bool;
    /// Position in subtiles.
    fn position(&self, unit: UnitId) -> (i32, i32);
    /// Unit size (`0x00620510`).
    fn size(&self, unit: UnitId) -> i32;
    /// Act (unit +0x18).
    fn act(&self, unit: UnitId) -> u8;
    /// `levels.txt` row of the unit's level (the level of its room in
    /// the game's lists).
    fn level_id(&self, game: &Game, unit: UnitId) -> i32;
    /// Monster level (teleport heal, §2.4).
    fn monster_level(&self, unit: UnitId) -> i32;
    /// Life in percent of max life.
    fn life_percent(&self, unit: UnitId) -> i32;
    /// Adds life (fixed point), capped at max life.
    fn add_life(&mut self, unit: UnitId, amount: i32);
    /// Monster data `dwAiState` (read by `0x005734E0`).
    fn ai_state(&self, unit: UnitId) -> u32;
    /// Alignment (`0x006259B0`): 0 evil, 1 neutral, 2 good.
    fn alignment(&self, unit: UnitId) -> u8;
    /// Monster type flag 8 (`0x005A0180`).
    fn is_unique(&self, unit: UnitId) -> bool;
    /// Champion type flag.
    fn is_champion(&self, unit: UnitId) -> bool;
    /// `0x0063E9F0`.
    fn is_boss(&self, unit: UnitId) -> bool;
    /// Monster vision record (monster data +0x50): its +0x24 field, or
    /// `None` without a record.
    fn vision_seen(&self, unit: UnitId) -> Option<u32>;
    /// §5.2 step 7: the target-seen update of the vision record.
    fn mark_seen(&mut self, unit: UnitId);
    /// Type-10 handler `0x00573120`: clears monster data +0x34, +0x38.
    fn ai_reset(&mut self, unit: UnitId);
    /// Interacting with a player (`0x00572DC0`).
    fn interacting(&self, unit: UnitId) -> bool;
    /// Player busy (`0x00535060`).
    fn busy(&self, unit: UnitId) -> bool;
    /// Monster data +0x30: the NPC interaction block exists
    /// (`world/npc.md` §2; read by the interaction handler `0x005E68F0`).
    fn has_interaction_block(&self, unit: UnitId) -> bool;
    /// `0x00572DE0`: `player` is in the NPC's interaction list.
    fn in_interaction_list(&self, npc: UnitId, player: UnitId) -> bool;
    /// `0x00627260(unit, 6, value)`: sets stat 6 (life).
    fn set_life(&mut self, unit: UnitId, value: i32);
    /// Unit getter `0x00625480(unit, stat, 0)` (Baboon's stat 74).
    fn stat(&self, unit: UnitId, stat: u16) -> i32;
    /// Unit set `0x00627260(unit, stat, value, 0)`.
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32);
    /// Sets bits of the unit flags (unit +0xC4; FoulCrowNest's 0x20000).
    fn set_unit_flag(&mut self, unit: UnitId, mask: u32);
    /// State on / off (`0x00639DB0(unit, state, on)`, `stat-lists.md`
    /// §9.2; SandRaider's states 90, 91).
    fn set_state(&mut self, unit: UnitId, state: u16, on: bool);
    /// `0x00553540`: the unit's path target unit (`None` when there is
    /// none).
    fn path_target(&self, unit: UnitId) -> Option<UnitId>;
}

/// The mode machinery, paths and sounds. Provider: the units session
/// (`sim/units.md` modes and movement).
pub trait AiModes {
    /// Requests a mode change; false when the mode start failed (which
    /// then falls into the neutral start, [`super::neutral_mode_start`]).
    fn change_mode(&mut self, game: &mut Game, unit: UnitId, mode: u8, target: ModeTarget) -> bool;
    /// Sets the anim mode without a mode change (inline thinks, §1.4).
    fn set_anim_mode(&mut self, unit: UnitId, mode: u8);
    /// The path step count.
    fn set_path_steps(&mut self, unit: UnitId, steps: i32);
    /// Path flag 0x800 (blocked step).
    fn path_blocked(&self, unit: UnitId) -> bool;
    /// Stops the unit's path (NPC interaction).
    fn stop_path(&mut self, unit: UnitId);
    /// Current skill := `skill`; false when the id is out of range.
    fn set_current_skill(&mut self, unit: UnitId, skill: i32) -> bool;
    /// Unit flag 0x40 (set by `0x005DEAD0`).
    fn set_skill_flag(&mut self, unit: UnitId);
    /// `0x0046C140(class, mode)`: the class has the mode.
    fn class_has_mode(&self, class: i32, mode: u8) -> bool;
    /// Monster sound (`sim/units.md`), optionally to one player.
    fn play_sound(&mut self, game: &mut Game, unit: UnitId, sound: u32, to: Option<UnitId>);
    /// `0x005A8520` gethit branch: mode change to gethit (3) with last-hit
    /// class 160.
    fn knockback_to_gethit(&mut self, game: &mut Game, unit: UnitId);
    /// `0x005DE6D0` → `0x005DE4E0` walk in radius: the point geometry is
    /// D2MOO's (§7.2); the provider computes it and walks.
    fn walk_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        a: i32,
        b: i32,
    ) -> bool;
    /// Operates a door (`0x00584540`, object spec).
    fn operate_door(&mut self, game: &mut Game, unit: UnitId, door: UnitId);
    /// Starts an overlay (`0x00621E40(unit, overlay, 0)`, stat 178
    /// `unit_dooverlay`).
    fn start_overlay(&mut self, unit: UnitId, overlay: i32);
    /// Path facing `0x00648820(path, dir)` (path spec).
    fn set_facing(&mut self, unit: UnitId, dir: i32);
}

/// Rooms, collision and line tests. Provider: the DRLG session (and
/// `sim/units.md` for the unit-to-unit tests).
pub trait AiWorld {
    /// `0x0061AB00`.
    fn in_town(&self, game: &Game, room: RoomId) -> bool;
    /// `0x0061AA40`: the room's "LOS draw" test.
    fn los_draw(&self, game: &Game, room: RoomId) -> bool;
    /// `0x0064D910`: the unit's position collides with `mask`.
    fn collides(&self, game: &Game, unit: UnitId, mask: u16) -> bool;
    /// `0x00622AA0(a, b, 4)`: blocked line between a and b.
    fn line_blocked(&self, game: &Game, a: UnitId, b: UnitId) -> bool;
    /// `0x00622C40(a, b, 0)`: melee range.
    fn in_melee_range(&self, game: &Game, a: UnitId, b: UnitId) -> bool;
    /// `0x005DC640`: "can reach directly" (geometry D2MOO's, §6).
    fn can_reach_directly(&self, game: &Game, unit: UnitId, target: UnitId) -> bool;
    /// `0x0054DC40`: a free spot for a teleport; its own draws (ai.md open
    /// question 4). Returns the spot and its room.
    fn find_spot(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32, RoomId)>;
    /// The four last-dead GUIDs of a room (room +0x38..+0x44), as units.
    fn last_dead(&self, game: &Game, room: RoomId) -> [Option<UnitId>; 4];
    /// `0x005FD350(class, room, x, y, 0)`: the monster footprint test
    /// (`monsters/population.md` §9).
    fn footprint_ok(&self, game: &Game, class: i32, room: Option<RoomId>, x: i32, y: i32) -> bool;
}

/// Target sources: the game's target-node lists, room scans and forced
/// targets. Provider: the units session (target nodes, `sim/units.md` /
/// clients) and the skills session (forced targets).
pub trait AiTargets {
    /// Game +0x10F8: the 10 target-node lists in list order (slots 0–7:
    /// a player head and its attached units; ai.md open question 7).
    fn target_nodes(&self, game: &Game) -> [Vec<UnitId>; 10];
    /// `0x005DD610` (ai.md open question 6): forced target and its
    /// full-size distance.
    fn forced_target(&mut self, game: &mut Game, unit: UnitId) -> Option<(UnitId, i32)>;
    /// §5.2 step 4: scan 5 within 35 with LOS flag `los`, then
    /// `0x005DD510`; target and distance.
    fn good_target_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        los: bool,
    ) -> Option<(UnitId, i32)>;
    /// `0x005DD510`: whether the slot-9 alternative replaces `main`.
    fn choose_alternative(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        main: Option<UnitId>,
        alt: UnitId,
    ) -> bool;
    /// `0x005DDC30`: secondary target, distance (0x7FFFFFFF if none) and
    /// melee flag.
    fn secondary_target(&mut self, game: &mut Game, unit: UnitId) -> (Option<UnitId>, i32, bool);
    /// `0x005DDF20`: nearest interacting player within 15 and the "close"
    /// flag (distance < 4); the NPC itself when none.
    fn nearest_player(&mut self, game: &mut Game, unit: UnitId) -> (UnitId, bool);
    /// Scan 8 (§5.4, start distance 9): a door.
    fn find_door(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId>;
    /// objects.txt `MonsterOK` of the door.
    fn door_monster_ok(&self, door: UnitId) -> bool;
    /// Scan 11: alternative target for the special walk, with distance.
    fn special_walk_target(&mut self, game: &mut Game, unit: UnitId) -> Option<(UnitId, i32)>;
    /// FallenShaman corpse scan (§9.6 step 2): scan 9 over its own
    /// minions (`own_minions`) or the adjacent rooms with `0x005F1380`;
    /// the last match and the count.
    fn shaman_corpses(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        max_sq: i32,
        own_minions: bool,
    ) -> (Option<UnitId>, u32);
    /// SandRaider's help search (§9.26 step 4): scan 1 for the nearest
    /// other monster with alignment 0 not in mode 0 or 12, by squared
    /// distance (`0x005B0BD0`, strictly smaller wins).
    fn nearest_evil_monster(&mut self, game: &mut Game, unit: UnitId) -> Option<UnitId>;
}

/// Skill code. Provider: the combat/skills session.
pub trait AiSkills {
    /// `0x005FD470(skill, target)`: the skill can be used on the target.
    fn skill_usable(&mut self, game: &mut Game, unit: UnitId, skill: i32, target: UnitId) -> bool;
}

/// The quest NPC whose portal functions NpcOutOfTown calls (§9.32).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalNpc {
    /// cain1: Act 1 quest 4 functions.
    Cain,
    /// drehyaiced: Act 5 quest 3 functions.
    Drehya,
}

/// Quest calls of the AI functions (§9.32). Provider: the quest session
/// (`world/quests.md`); the names are D2MOO's.
pub trait AiQuests {
    /// Set up the portal coordinates (`0x005944B0` /
    /// `0x0058A940`); false = failed.
    fn portal_setup(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) -> bool;
    /// Spawn the town portal (`0x005944F0` / `0x0058A980`).
    fn spawn_town_portal(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc);
    /// Spawn the portal out of town (`0x005943B0` / `0x0058A820`); false
    /// = failed.
    fn spawn_outside_portal(&mut self, game: &mut Game, unit: UnitId, npc: PortalNpc) -> bool;
    /// The portal coordinates (`0x00594450` / `0x0058A8D0`); `None` =
    /// none.
    fn portal_coords(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        npc: PortalNpc,
    ) -> Option<(i32, i32)>;
    /// drehyaiced's per-think call `0x0058AA10(game)`.
    fn drehya_update(&mut self, game: &mut Game);
    /// drehyaiced's walk gate `0x0058A9F0(game)` (nonzero → wait).
    fn drehya_wait(&mut self, game: &mut Game) -> bool;

    // ---- the Npc class cases (§9.9 step 2) ----

    /// jerhyn: `0x0059F570` `ACT2Q4_IsJerhynPalaceActivated`.
    fn jerhyn_palace_active(&mut self, game: &mut Game) -> bool;
    /// jerhyn: `0x0059F580` `ACT2Q4_GetAndUpdatePalaceNpcState`: (a, b).
    fn jerhyn_npc_state(&mut self, game: &mut Game, unit: UnitId) -> (i32, i32);
    /// jerhyn: `0x0059B6E0` `ACT2Q4_IsGuardMoving`.
    fn guard_moving(&mut self, game: &mut Game, unit: UnitId) -> bool;
    /// alkor: `0x005BAD20` `ACT3Q4_GoldenBirdBroughtToAlkor`.
    fn alkor_bird(&mut self, game: &mut Game) -> bool;
    /// alkor: `0x005BAD40` `ACT3Q4_ResetAlkor`.
    fn alkor_reset(&mut self, game: &mut Game);
    /// ormus: `0x005B9CA0` `ACT3Q3_GetAltarCoordinates`.
    fn ormus_altar(&mut self, game: &mut Game) -> Option<(i32, i32)>;
    /// ormus: `0x005B9CD0` `ACT3Q3_SetAltarMode`.
    fn ormus_set_altar_mode(&mut self, game: &mut Game);
    /// cain5: `0x00594360` `ACT1Q4_GetCainPortalInTownCoordinates`.
    fn cain_town_coords(&mut self, game: &mut Game, unit: UnitId) -> Option<(i32, i32)>;
    /// cain5: `0x005945F0` `ACT1Q4_OnCainInTownActivated`.
    fn cain_in_town_activated(&mut self, game: &mut Game, unit: UnitId);
    /// drehya: `0x0058BC80` `ACT5Q4_AnyaOpenPortal`.
    fn anya_open_portal(&mut self, game: &mut Game, unit: UnitId);
}

/// Quest calls of the Act II–V bodies (`world/quests.md`; D2MOO names).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestCall {
    /// `0x00599420(game, unit)` `ACT2Q1_OnRadamentActivated`
    /// (`ai-bodies-2.md` §3).
    RadamentActivated,
    /// `0x0059C330(game, unit)` `ACT2Q5_OnSummonerActivated` (§15).
    SummonerActivated,
    /// `0x005B4390(game)` `ACT4Q1_OnIzualActivated` (`ai-bodies-4.md` §8).
    IzualActivated,
    /// `0x00587900(game, unit)` (Shenk, `ai-bodies-5.md` §7).
    Shenk,
    /// `0x0058BC40(game, unit)` (Nihlathak, `ai-bodies-5.md` §23).
    Nihlathak,
    /// `0x0058CF90(game)` `ACT5Q5_IsNotActivatable` (§12): the result.
    AncientsNotActivatable,
    /// `0x0058CFE0(game)` `ACT5Q5_IsActivatable` (§13): the result.
    AncientsActivatable,
    /// `0x0058CFB0(game)` (§13): the Ancients' portal exists.
    AncientsPortal,
    /// `0x0058E600(game)` (§19): Baal enters the Worldstone Chamber.
    BaalToStairs,
}

/// Seams of the Act II–V bodies (`ai-bodies-2.md`..`ai-bodies-5.md`).
/// Providers: the units / stats session (unit fields, stat lists), the
/// skills session (skill entries and checks), the path session
/// (patterns, placement, collision), the population / init session
/// (spawns), the damage session (kill) and the quest session.
pub trait AiActs {
    // ---- unit fields ----

    /// Unit flags (unit +0xC4).
    fn unit_flags(&self, unit: UnitId) -> u32;
    /// Clears bits of the unit flags (Vulture take-off).
    fn clear_unit_flag(&mut self, unit: UnitId, mask: u32);
    /// `0x00625D10`: max life (stat 7, fixed point).
    fn max_life(&self, unit: UnitId) -> i32;
    /// `0x00625D60`: max mana.
    fn max_mana(&self, unit: UnitId) -> i32;
    /// A state of flag group `g` (`0x0063A7B0`; 2 `hide`, 33 `udead`).
    fn has_state_group(&self, unit: UnitId, g: u8) -> bool;
    /// States count (data tables +0xC4).
    fn states_count(&self) -> i32;
    /// `0x00625760(unit, flags)`: an active stat list with list flags
    /// (0x20: a curse).
    fn has_list_flag(&self, unit: UnitId, flags: u32) -> bool;
    /// `0x00554200(game, a, b)`: b is hostile to a (`combat/hit.md`).
    fn hostile(&self, game: &Game, a: UnitId, b: UnitId) -> bool;
    /// `0x00552FD0`: the unit's owner unit.
    fn owner(&self, game: &Game, unit: UnitId) -> Option<UnitId>;
    /// `0x0058F090`: a monster's owner record (type, GUID).
    fn owner_record(&self, unit: UnitId) -> Option<(i32, u32)>;
    /// `0x0065C310(record, quest, flag)` on the player's quest record of
    /// `difficulty` (player data +0x10 + 4 × difficulty).
    fn quest_flag(&self, player: UnitId, difficulty: u8, quest: i32, flag: i32) -> bool;
    /// `0x005353F0`: the player's portal GUID.
    fn portal_guid(&self, player: UnitId) -> Option<u32>;
    /// Monster data byte +0x04 + i (D2MOO `nComponent[i]`).
    fn component(&self, unit: UnitId, i: usize) -> u8;
    /// `0x00553010`: the unit's target unit.
    fn target_unit(&self, game: &Game, unit: UnitId) -> Option<UnitId>;
    /// `0x00573090(unit, kind, GUID)`: the target override (`ai.md` §5.1).
    fn set_target_override(&mut self, unit: UnitId, kind: i32, guid: u32);
    /// `0x006510C0(class, 0, 0)`: the monstats chain byte +0x4B.
    fn chain_index(&self, class: i32) -> i32;
    /// `0x0063EC70(room, class)`: the class of `class`'s chain for the
    /// room's level (`monsters/population.md` §11.6).
    fn class_for_level(&self, game: &Game, room: Option<RoomId>, class: i32) -> i32;

    // ---- skills ----

    /// The level with bonus (`0x006442A0(unit, entry, 1)`) of the unit's
    /// skill entry: `highest` = the highest entry `0x006439F0`, else the
    /// entry with owner −1 `0x006439B0`; `None` without an entry.
    fn skill_level(&self, unit: UnitId, skill: i32, highest: bool) -> Option<i32>;
    /// The unit's highest entry of `skill` (`0x006439F0`): its skill id
    /// (`0x00643CE0`) and mode (entry +0x08, `0x00644360`).
    fn skill_entry(&self, unit: UnitId, skill: i32) -> Option<(i32, u8)>;
    /// The left (`0x00620190`) or right (`0x006201D0`) skill: id and level
    /// with bonus.
    fn hand_skill(&self, unit: UnitId, right: bool) -> Option<(i32, i32)>;
    /// `0x0056DEB0(unit, skill, level, 1)` then `0x005701B0`: add the
    /// skill and make it the right skill (Duriel's aura).
    fn add_right_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32);
    /// `0x00647280`: assign the skill at `level`.
    fn assign_skill(&mut self, game: &mut Game, unit: UnitId, skill: i32, level: i32);
    /// `0x00644560`: the param of the unit's highest `skill` entry; false
    /// when there is no entry (fatal in 1.14d).
    fn set_skill_param(&mut self, unit: UnitId, skill: i32, value: i32) -> bool;
    /// `0x005FD470(skill, target, x, y)`: the skill check.
    fn skill_check(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        skill: i32,
        target: Option<UnitId>,
        x: i32,
        y: i32,
    ) -> bool;
    /// `0x0056E390(unit, target, skill, level)`: the corpse search of
    /// Corpse Explosion (skills spec).
    fn corpse_search(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: Option<UnitId>,
        skill: i32,
        level: i32,
    ) -> Option<UnitId>;

    // ---- path, placement, collision ----

    /// Path pattern (path +0x48, `0x00649180`).
    fn path_pattern(&self, unit: UnitId) -> i32;
    /// `0x00649190`.
    fn set_path_pattern(&mut self, unit: UnitId, pattern: i32);
    /// Move mask (path +0x50, `0x00648CE0`).
    fn set_move_mask(&mut self, unit: UnitId, mask: u16);
    /// `0x00554EA0(room, x, y, exact 0, alt 0)` (`path-placement.md`
    /// §10): place the unit; false = failed.
    fn place_unit(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        room: Option<RoomId>,
        x: i32,
        y: i32,
    ) -> bool;
    /// `0x0064EA90(room, (x, y), pattern, mask)`: pattern stamp.
    fn stamp_pattern(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        pattern: i32,
        mask: u16,
    );
    /// `0x0064CBE0`: clears `bits` of the one cell at (x, y).
    fn clear_cell(&mut self, game: &mut Game, room: Option<RoomId>, x: i32, y: i32, bits: u16);
    /// `0x0064CB30(room, x, y, mask)`: the point collides.
    fn point_collides(&self, game: &Game, room: Option<RoomId>, x: i32, y: i32, mask: u16) -> bool;
    /// `0x0064D910(room, unit's position, pattern, mask)`.
    fn pattern_collides(&self, game: &Game, unit: UnitId, pattern: i32, mask: u16) -> bool;
    /// `0x0064E7B0(room, &pt, size, 0xFFFF, 1)`: a free point near (x, y).
    fn free_point(
        &mut self,
        game: &mut Game,
        room: Option<RoomId>,
        x: i32,
        y: i32,
        size: i32,
    ) -> Option<(i32, i32)>;
    /// `0x0054DC40(…, room, class, &x, &y, 0)`: a free spot for the class
    /// near (x, y) (`ai.md` §2.4; its draws are open question 4).
    fn free_spot_for(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        class: i32,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32)>;
    /// `0x00463740`: the room at (x, y) from the unit's room.
    fn room_at(&self, game: &Game, unit: UnitId, x: i32, y: i32) -> Option<RoomId>;
    /// `0x005DE4E0(target, mode, a, b)`: the walk-in-radius core with a
    /// mode (`ai.md` §7.2; geometry D2MOO's).
    fn move_in_radius(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        target: UnitId,
        mode: u8,
        a: i32,
        b: i32,
    ) -> bool;
    /// `0x00620C10` → `0x00648B90`: the path target unit.
    fn set_path_target(&mut self, unit: UnitId, target: UnitId);
    /// Boss pick slot 9 (`ai-bodies-4.md` §7.1 step 3): path target :=
    /// target, path type 2, compute the path; whether it has points.
    fn path_has_points(&mut self, game: &mut Game, unit: UnitId, target: UnitId) -> bool;
    /// `0x00621DC0`: the 64-step direction from the unit to the target
    /// (`sim/pathing.md` §8.3).
    fn direction64(&self, unit: UnitId, target: UnitId) -> i32;
    /// `0x00648730`: stops the path (BaalToStairs).
    fn stop_unit_path(&mut self, unit: UnitId);

    // ---- spawns, removal, damage, stat lists ----

    /// `0x005B2F20(room, x, y, class, mode, spread, flags)`
    /// (`monsters/init.md` §1).
    #[allow(clippy::too_many_arguments)]
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
    ) -> Option<UnitId>;
    /// The class the spawn info `0x0063EFA0` gives SandMaggotQueen
    /// (`0x0054DA60(68, …)`, `ai-bodies-2.md` open question 4).
    fn queen_spawn_class(&self, unit: UnitId) -> i32;
    /// `0x0057CCB0(game, unit, killer, 1)` (`combat/damage.md` §7.2).
    fn kill(&mut self, game: &mut Game, unit: UnitId, killer: Option<UnitId>);
    /// The unit leaves its room and is removed (`0x0061A270`,
    /// `0x00623830`, `0x0064C370`, `0x00555600`).
    fn remove_unit(&mut self, game: &mut Game, unit: UnitId);
    /// Baal clone links (`0x0058F030`, `0x0058F100`, `0x00621CE0`): the
    /// clone joins the unit's minions and each stores the other as owner.
    fn link_clone(&mut self, game: &mut Game, unit: UnitId, clone: UnitId);
    /// `0x00574370(game, unit, class, mode)`: reinitialize as `class`.
    fn reinit_class(&mut self, game: &mut Game, unit: UnitId, class: i32, mode: u8);
    /// BaalThrone §20 step 6: a stat list (flags 0, no expiry, owned by
    /// the unit) attached, its state := 142, stat 355 := `class`.
    fn change_class_list(&mut self, game: &mut Game, unit: UnitId, class: i32);
    /// WillOWisp §10 step 2.4: the magic-find stat list on `target`
    /// (stat 80 := `value`, expiry and type-12 timer at `expire`).
    fn wisp_buff(&mut self, game: &mut Game, target: UnitId, value: i32, expire: i32);
    /// `0x00571C00`: S→C 0xA4 (client preload) with `class` on the unit.
    fn preload_class(&mut self, game: &mut Game, unit: UnitId, class: i32);
    /// The unit find of WillOWisp §10 step 1 (`0x0065A950`, size 32,
    /// flags 0x583; open question 3): the units it visits, in its order.
    fn wisp_find(&mut self, game: &mut Game, unit: UnitId) -> Vec<UnitId>;
    /// BaalThrone wave `w` (superunique 61 + w, `0x00586B30`,
    /// `0x00659B80(2, ·)`, `0x006556E0`): (mapped superunique id, class);
    /// `None` without a record.
    fn wave(&self, w: i32) -> Option<(i32, i32)>;
    /// `0x0061AED0(room, 0)`: clears the room's portal flag.
    fn clear_room_portal_flag(&mut self, game: &mut Game, room: Option<RoomId>);
    /// A quest call; the result where the call returns one.
    fn quest_call(&mut self, game: &mut Game, unit: UnitId, call: QuestCall) -> bool;
}

/// Everything AI code needs.
pub trait AiHost: AiUnits + AiModes + AiWorld + AiTargets + AiSkills + AiQuests + AiActs {}

impl<T: AiUnits + AiModes + AiWorld + AiTargets + AiSkills + AiQuests + AiActs + ?Sized> AiHost
    for T
{
}
