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
    /// `levels.txt` row of the unit's level.
    fn level_id(&self, unit: UnitId) -> i32;
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
}

/// Skill code. Provider: the combat/skills session.
pub trait AiSkills {
    /// `0x005FD470(skill, target)`: the skill can be used on the target.
    fn skill_usable(&mut self, game: &mut Game, unit: UnitId, skill: i32, target: UnitId) -> bool;
}

/// Everything AI code needs.
pub trait AiHost: AiUnits + AiModes + AiWorld + AiTargets + AiSkills {}

impl<T: AiUnits + AiModes + AiWorld + AiTargets + AiSkills + ?Sized> AiHost for T {}
