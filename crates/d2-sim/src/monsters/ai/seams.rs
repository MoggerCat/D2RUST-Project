// Spec: specs/monsters/ai.md (Related specs; the seams to other systems)
// Spec: specs/monsters/ai-bodies.md (§9, split out of `ai.md`)
// Spec: specs/monsters/ai-bodies-6.md; specs/monsters/ai-bodies-7.md (the `AiSummons` seams)
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
    /// §9.2; SandRaider's states 90, 91): a state outside 0 … count − 1
    /// does nothing; else the toggle, then the unit's update-queue insert
    /// whether or not the bit changed.
    fn set_state(&mut self, game: &mut Game, unit: UnitId, state: u16, on: bool);
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
    /// [`Self::change_mode`] with the request record's path-type byte
    /// (+0x15, §7.1) overwritten with `path_byte` after the builder set
    /// it (100 = no path, 101 = type 13, else the path type). Default:
    /// the plain request.
    fn change_mode_path_byte(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        mode: u8,
        target: ModeTarget,
        path_byte: u8,
    ) -> bool {
        let _ = path_byte;
        self.change_mode(game, unit, mode, target)
    }
    /// [`AiModes::change_mode`] (or, with a path byte,
    /// [`AiModes::change_mode_path_byte`]) with the monster's pending
    /// velocity request (`ai.md` §7.3), which the mode set's movement
    /// set-up consumes for every mode but GH (§7.5 rule 4.1): a provider
    /// that runs the set-up takes it and leaves `velocity` zeroed.
    /// Default: the plain change, the request kept.
    fn change_mode_with(
        &mut self,
        game: &mut Game,
        unit: UnitId,
        mode: u8,
        target: ModeTarget,
        path_byte: Option<u8>,
        velocity: &mut super::VelocityRequest,
    ) -> bool {
        let _ = velocity;
        match path_byte {
            Some(b) => self.change_mode_path_byte(game, unit, mode, target, b),
            None => self.change_mode(game, unit, mode, target),
        }
    }
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
    /// `0x0054DC40`: a free spot for a teleport; its own draws on the room
    /// seed (`monsters/population.md` §8). Returns the spot and its room.
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

/// One hireling.txt row as the hireling skill pick reads it
/// (`ai-bodies-6.md` §7 step 7, `world/hirelings.md` §1.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HireRow {
    /// `Level`.
    pub level: i32,
    /// `DefaultChance` (+0x64).
    pub default_chance: i32,
    /// `Skill1`..`Skill6` (+0x78 + 4i).
    pub skill: [i32; 6],
    /// `Chance1`..`Chance6` (+0x90 + 4i).
    pub chance: [i32; 6],
    /// `ChancePerLvl1`..`ChancePerLvl6` (+0xA8 + 4i).
    pub chance_per_lvl: [i32; 6],
    /// `Mode1`..`Mode6` (+0xC0 + i).
    pub mode: [u8; 6],
}

/// Quest seams of the one-row AIs (`ai-bodies-6.md` §11,
/// `ai-bodies-7.md` §7, §12, §24; `world/quests-act2.md` …
/// `-act5.md`). The result is the call's return value where it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestHook {
    /// `0x0059DF50(game, unit)`: Tyrael (class 251) may leave.
    TyraelLeave,
    /// `0x0059C750(game)`: Tyrael left.
    TyraelGone,
    /// `0x005B43F0(game, unit)`: Izual's ghost (class 406) may leave.
    IzualGhostLeave,
    /// `0x005B4440(game)`: Izual's ghost left.
    IzualGhostGone,
    /// `0x0059B8B0(game)`: the palace door is open (JarJar).
    PalaceDoorOpen,
    /// `0x0059AEC0(game)`: the palace guard steps aside (JarJar §7 step 3.2).
    PalaceGuardAside,
    /// `0x005BD4A0(game, unit)`: the Dark Wanderer's minion hook.
    DarkWandererGone,
    /// `0x00588830(game, unit)`: a caged barbarian leaves (Wussie).
    WussieLeaving,
    /// `0x00588880(game, 0, unit)`: the barbarian left.
    WussieLeave,
    /// `0x00588E10(game, P, unit)`: P can rescue the barbarian.
    WussieCanRescue,
    /// `0x005888D0(game, P, unit)`: rescue.
    WussieRescue,
    /// `0x00588DD0(game, P, unit)`: the barbarian waits for P.
    WussieWait,
}

/// Seams of the bodies of `ai-bodies-6.md` and `ai-bodies-7.md` (pets,
/// hirelings, summons, traps, spawners, quest NPCs). Every method has a
/// narrow default (nothing found, nothing done) until its owner wires
/// it; the owners are named per method.
pub trait AiSummons {
    // ---- paths (`sim/pathing.md`) ----

    /// The path's final point (path +0x18 / +0x1A, `0x00648A20` /
    /// `0x00648A30`): where the unit is heading. Default: (0, 0).
    fn path_final_point(&self, _unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// The path's target point (path +0x10 / +0x12). Default: (0, 0).
    fn path_target_point(&self, _unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// Boss pick slot 9 with no best (`ai-bodies-4.md` §7.1 step 3): path
    /// target unit := 0 (type and GUID kept), path type 2, the path
    /// computed toward its stored target point; whether it has points.
    /// Default: none.
    fn path_has_points_no_target(&mut self, _game: &mut Game, _unit: UnitId) -> bool {
        false
    }
    /// `0x00648AD0`: the path target point := (x, y).
    fn set_path_target_point(&mut self, _unit: UnitId, _x: i32, _y: i32) {}
    /// `0x00621DC0` → `0x0064FDC0`: the 64-step direction from the unit's
    /// position to (x, y). Default: 0.
    fn direction64_to(&self, _unit: UnitId, _x: i32, _y: i32) -> i32 {
        0
    }
    /// `0x006488A0(path, dir)`: snaps the path direction. Default: none.
    fn snap_direction(&mut self, _unit: UnitId, _dir: i32) {}
    /// `0x00622AA0(a, b, mask)`: the line a→b is blocked for `mask`
    /// (`render/draw-order-2.md` §15–16). Default: clear.
    fn line_blocked_mask(&self, _game: &Game, _a: UnitId, _b: UnitId, _mask: u16) -> bool {
        false
    }

    // ---- rooms and placement (DRLG, `sim/path-placement.md`,
    // `monsters/population.md`) ----

    /// `0x0061B130(room, x, y)`: the coordinate index at (x, y). Default: 0.
    fn coord_index(&self, _game: &Game, _room: Option<RoomId>, _x: i32, _y: i32) -> i32 {
        0
    }
    /// `0x0061AD30(room, x, y)`: the spot class argument of the free spot
    /// search. Default: 0.
    fn spot_class(&self, _game: &Game, _room: Option<RoomId>, _x: i32, _y: i32) -> i32 {
        0
    }
    /// `0x0054DC40(game, room, cl, class, &x, &y, 0)`
    /// (`monsters/population.md` §8, room-seed draws). Default: none.
    fn free_spot(
        &mut self,
        _game: &mut Game,
        _room: Option<RoomId>,
        _cl: i32,
        _class: i32,
    ) -> Option<(i32, i32)> {
        None
    }
    /// `0x0064E7E0(room, &pt, size, mask, n)`: a free point near (x, y).
    /// Default: none.
    #[allow(clippy::too_many_arguments)]
    fn free_point_masked(
        &mut self,
        _game: &mut Game,
        _room: Option<RoomId>,
        _x: i32,
        _y: i32,
        _size: i32,
        _mask: u16,
        _n: i32,
    ) -> Option<(i32, i32)> {
        None
    }
    /// `0x00619730`: the room's sub-tile box (x, y, w, h). Default: none.
    fn room_box(&self, _game: &Game, _room: RoomId) -> Option<(i32, i32, i32, i32)> {
        None
    }
    /// `0x005429B0(game, R, x, y, class)`: a dead monster of `class` in the
    /// room at (x, y) (or an inactive record with the dead bit). Default:
    /// false.
    fn class_dead_at(&self, _game: &Game, _room: RoomId, _x: i32, _y: i32, _class: i32) -> bool {
        false
    }
    /// The unit's target-node slot (unit +0xD0); 11 = none. Default: 11.
    fn target_slot(&self, _unit: UnitId) -> i32 {
        11
    }
    /// `0x005B1990(game, unit, 0, slot)`: a node pushed at the head of the
    /// game's target-node list `slot`; unit +0xD0 := slot.
    fn register_target_node(&mut self, _game: &mut Game, _unit: UnitId, _slot: i32) {}
    /// Unit flags 2 (unit +0xC8) |= `mask`.
    fn set_unit_flags2(&mut self, _unit: UnitId, _mask: u32) {}
    /// The trap kind cached in the unit's level's monster region (+0x2D4,
    /// `ai-bodies-7.md` §5); −1 = unset. Default: −1.
    fn trap_kind(&self, _game: &Game, _unit: UnitId) -> i32 {
        -1
    }
    /// Stores the trap kind in the unit's level's monster region.
    fn set_trap_kind(&mut self, _game: &mut Game, _unit: UnitId, _kind: i32) {}
    /// The monster classes of the unit's level's monster region (+0x14 +
    /// 0x34·i, count +0x10; `monsters/population.md` §2.2). Default: none.
    fn region_classes(&self, _game: &Game, _unit: UnitId) -> Vec<i32> {
        Vec::new()
    }

    // ---- players and pets (`sim/pets.md`, `world/hirelings.md`) ----

    /// Player data +0xA0 (next index) and +0xA8 (the 20 position history
    /// entries, `sim/path-placement.md` §10 rule 7). Default: empty.
    fn position_history(&self, _player: UnitId) -> (usize, [(i32, i32); 20]) {
        (0, [(0, 0); 20])
    }
    /// Player data +0x148 / +0x14C: the last placed point. Default: (0, 0).
    fn last_placed_point(&self, _player: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// `0x00574DE0`: the living units of the owner's pet lists (types 1 …
    /// count − 1, nodes without flag bit 0), in list order. Default: none.
    fn pets(&self, _game: &Game, _owner: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    /// `0x00574F40`: the owner's total pet count. Default: 0.
    fn pet_count(&self, _owner: UnitId) -> i32 {
        0
    }
    /// `0x00574A20`: the unit's pet type in the owner's lists; −1 = none.
    fn pet_type_of(&self, _game: &Game, _owner: UnitId, _unit: UnitId) -> i32 {
        -1
    }
    /// The `pettype.txt` row count (data tables +0xBF0;
    /// `ai-bodies-7.md` §18 Allowed). Default: 0.
    fn pettype_count(&self) -> i32 {
        0
    }
    /// `0x00574BD0`: the hireling `Id` (+8 of the record) of the unit's
    /// node in the owner's pet lists. Default: none.
    fn hireling_id(&self, _game: &Game, _owner: UnitId, _unit: UnitId) -> Option<i32> {
        None
    }
    /// `0x006562F0(game +0x70, id, level)`: the hireling row. Default: none.
    fn hireling_row(&self, _game: &Game, _id: i32, _level: i32) -> Option<HireRow> {
        None
    }
    /// `0x00622560`: the unit's attack rating. Default: 0.
    fn attack_rating(&self, _unit: UnitId) -> i32 {
        0
    }
    /// `0x00535BC0` + `0x00645830(unit, weapon, 0, 0)`: the weapon mastery
    /// to-hit of the unit's weapon; `None` without a weapon.
    fn mastery_tohit(&self, _unit: UnitId) -> Option<i32> {
        None
    }

    // ---- skills (`skills/bodies.md`, `skills/use.md`) ----

    /// `0x00646CA0(unit, calc, skill, level)`: a skills calc column
    /// (`data/calc-expressions.md`). Default: 0.
    fn skill_calc(&self, _unit: UnitId, _skill: i32, _calc: u32, _level: i32) -> i32 {
        0
    }
    /// The mode of the unit's entry of `skill` with owner −1
    /// (`0x006439B0`, entry +0x08). Default: none.
    fn entry_mode(&self, _unit: UnitId, _skill: i32) -> Option<u8> {
        None
    }
    /// `0x006442A0(unit, entry, 0)`: the base level of the unit's entry
    /// of `skill`. Default: none.
    fn skill_base_level(&self, _unit: UnitId, _skill: i32) -> Option<i32> {
        None
    }
    /// The unit's skill list (`0x00643910` / `0x006438F0`), in list order:
    /// skill id and level with bonus. Default: empty.
    fn unit_skills(&self, _unit: UnitId) -> Vec<(i32, i32)> {
        Vec::new()
    }
    /// The unit has a skill list (unit +0xA8). Default: false.
    fn has_skill_list(&self, _unit: UnitId) -> bool {
        false
    }
    /// The class skill list of a player class (data tables +0xBA4 /
    /// +0xBAC, `0x00646140` / `0x006460F0`). Default: empty.
    fn class_skills(&self, _class: i32) -> Vec<i32> {
        Vec::new()
    }
    /// `0x005701B0(unit, skill, −1)`: make `skill` the right skill.
    fn make_right_skill(&mut self, _game: &mut Game, _unit: UnitId, _skill: i32) {}
    /// `0x00643BC0` (left) / `0x00643C50` (right): the hand skill := the
    /// unit's entry of `skill`.
    fn set_hand_skill(&mut self, _unit: UnitId, _skill: i32, _right: bool) {}
    /// `0x0064F460(unit)`: the "both" range (3) reads as ranged (2) when
    /// set (`0x00645460`). Default: false.
    fn ranged_both(&self, _unit: UnitId) -> bool {
        false
    }
    /// `0x0056EDE0(game, owner, skill, level, missile, x, y)`: a skill
    /// missile from `owner` (`missiles/bodies.md`). Default: none.
    #[allow(clippy::too_many_arguments)]
    fn skill_missile(
        &mut self,
        _game: &mut Game,
        _owner: UnitId,
        _skill: i32,
        _level: i32,
        _missile: i32,
        _x: i32,
        _y: i32,
    ) -> Option<UnitId> {
        None
    }
    /// `0x00621CE0(a, b)`: a stores b as its owner.
    fn link_owner(&mut self, _game: &mut Game, _a: UnitId, _b: UnitId) {}
    /// `0x005D2F80(game, unit, ·, n)`: the corpse search (unit find flags
    /// 0x1002, size n around the unit; corpse test `0x00623600`, hostile
    /// `0x00554200`): the first match. Default: none.
    fn corpse_find(&mut self, _game: &mut Game, _unit: UnitId, _n: i32) -> Option<UnitId> {
        None
    }

    // ---- states (`sim/stat-lists.md`) ----

    /// `0x005EB7F0`: a state of the same `group` (states +0x1E) as
    /// `state` is active on the unit. Default: false.
    fn state_group_active(&self, _unit: UnitId, _state: i32) -> bool {
        false
    }
    /// `0x0063A2B0`: the unit has a `pgsv` state (state flag bit 4).
    fn has_pgsv_state(&self, _unit: UnitId) -> bool {
        false
    }
    /// The value of `stat` in the stat list of the unit's `state`
    /// (`0x006256B0`); `None` without the list or the stat.
    fn state_stat(&self, _unit: UnitId, _state: i32, _stat: i32) -> Option<i32> {
        None
    }
    /// `0x005A0180`: the monster type flags (8 unique, …). Default: 0.
    fn monster_type_flags(&self, _unit: UnitId) -> u32 {
        0
    }

    // ---- quests and messages (`world/quests.md` and its act files) ----

    /// A quest hook; the result where the call returns one. Default: 0.
    fn quest_hook(
        &mut self,
        _game: &mut Game,
        _unit: UnitId,
        _player: Option<UnitId>,
        _hook: QuestHook,
    ) -> bool {
        false
    }
    /// `0x0059B8F0(game, &(x, y))`: nonzero and the (possibly moved) point.
    /// Default: (false, x, y).
    fn palace_guard_point(&mut self, _game: &mut Game, x: i32, y: i32) -> (bool, i32, i32) {
        (false, x, y)
    }
    /// `0x005BD0D0(game, unit, &x, &y)`: the Dark Wanderer's walk target.
    fn dark_wanderer_target(&mut self, _game: &mut Game, _unit: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// `0x00588D60(game, unit, &P)`: `None` = no rescue portal; else the
    /// portal unit P (`None` = 0).
    fn rescue_portal(&mut self, _game: &mut Game, _unit: UnitId) -> Option<Option<UnitId>> {
        None
    }
    /// S→C 0x8A NpcWantsInteract {1, the unit's GUID} to the player's
    /// client (`0x005531C0`, `0x0053DFF0`).
    fn npc_wants_interact(&mut self, _game: &mut Game, _player: UnitId, _unit: UnitId) {}
}

/// Everything AI code needs.
pub trait AiHost:
    AiUnits + AiModes + AiWorld + AiTargets + AiSkills + AiQuests + AiActs + AiSummons
{
}

impl<
        T: AiUnits + AiModes + AiWorld + AiTargets + AiSkills + AiQuests + AiActs + AiSummons + ?Sized,
    > AiHost for T
{
}
