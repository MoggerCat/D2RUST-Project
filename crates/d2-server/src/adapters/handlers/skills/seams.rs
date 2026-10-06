// Spec: specs/skills/use.md, specs/skills/levels.md §6.4, specs/combat/vitals.md
//! [`SkillSeams`]: the parts of the skill and vitals seams
//! (`UseWorld`, `ManaUnits`, `UseMissiles`, `SkillFunctions`,
//! `LearnUnits`, `VitalsUnits`) that no written-and-implemented spec
//! provides yet. Everything else is answered by the wired sim
//! ([`super::world`]).
//!
//! Defaults follow the action wiring's `Pending` convention
//! (`docs/handoff/wire-action.md` §4): the narrowest answer (nothing,
//! `None`, `false`, 0). The three methods with no neutral answer
//! ([`SkillSeams::use_state`], [`SkillSeams::srvst`],
//! [`SkillSeams::srvdo`]) have no default.

use d2_sim::combat::RoomKind;
use d2_sim::skills::use_::{MissileAim, UseState};
use d2_sim::skills::SkillEntry;
use d2_sim::units::UnitId;

/// Seam calls without a provider; each names its expected owner.
#[allow(unused_variables)]
pub trait SkillSeams {
    // ---- skill list of a unit (units/stats: unit +0xA8; use.md §2, §7)

    /// The unit's skill list (`levels.md` §1).
    fn skill_list(&self, u: UnitId) -> Vec<SkillEntry> {
        Vec::new()
    }
    /// The used skill and its flags (`0x00620210`).
    fn used_skill(&self, u: UnitId) -> Option<SkillEntry> {
        None
    }
    fn set_used_skill(&mut self, u: UnitId, e: Option<SkillEntry>) {}
    fn used_skill_flags(&self, u: UnitId) -> u32 {
        0
    }
    fn set_used_skill_flags(&mut self, u: UnitId, f: u32) {}
    /// Left / right skill (`0x00622F10` / `0x00622EA0`).
    fn left_skill(&self, u: UnitId) -> Option<SkillEntry> {
        None
    }
    fn right_skill(&self, u: UnitId) -> Option<SkillEntry> {
        None
    }
    fn set_left_skill(&mut self, u: UnitId, e: SkillEntry) {}
    fn set_right_skill(&mut self, u: UnitId, e: SkillEntry) {}
    /// The entry lookups (use.md §2 step 3: unnamed; 0x3C).
    fn find_entry(&self, u: UnitId, skill: i32) -> Option<SkillEntry> {
        None
    }
    fn find_entry_owned(&self, u: UnitId, skill: i32, owner: i32) -> Option<SkillEntry> {
        None
    }
    /// `0x006439F0`.
    fn owns_skill(&self, u: UnitId, skill: i32) -> bool {
        false
    }
    /// Skill entry +8, fixed at creation (use.md §5.1).
    fn entry_mode(&self, u: UnitId, e: &SkillEntry) -> u32 {
        0
    }
    /// Param4 of the Attack entry (use.md §2 step 2).
    fn attack_param4(&self, u: UnitId) -> i32 {
        0
    }
    fn set_attack_param4(&mut self, u: UnitId, v: i32) {}
    /// `use_state` `0x00647960` (use.md §2 step 3; the order of its parts
    /// is open).
    fn use_state(&mut self, u: UnitId, e: &SkillEntry) -> UseState;
    /// `0x0056C3F0` (`decquant`).
    fn dec_quantity(&mut self, u: UnitId, skill: i32) {}
    /// Aura state list free / switch-on with stats 350/351 (0x3C, use.md
    /// §7; the list's allocation is not stated).
    fn free_aura_state(&mut self, u: UnitId, state: u16) {}
    fn set_aura_state(&mut self, u: UnitId, state: u16, skill: i32, lvl: i32) {}

    // ---- mana (levels.md §4)

    /// `0x0063A400`.
    fn shapeshifted(&self, u: UnitId) -> bool {
        false
    }
    /// `0x0056BEC0`.
    fn consume_charges(&mut self, u: UnitId, e: &SkillEntry) -> bool {
        false
    }
    /// `0x005D2B60` (levels.md OQ8).
    fn pay_life(&mut self, u: UnitId, cost: i32) -> bool {
        false
    }

    // ---- per-skill bodies and missiles (functions.tsv `mapped`, OQ10)

    fn srvst(&mut self, index: u16, u: UnitId, skill: i32, lvl: i32) -> i32;
    #[allow(clippy::too_many_arguments)]
    fn srvdo(
        &mut self,
        index: u16,
        u: UnitId,
        skill: i32,
        lvl: i32,
        charge: bool,
        item: bool,
        aim: bool,
    ) -> i32;
    /// `0x0056EE90` / `0x0056ECB0`.
    fn create_skill_missile(
        &mut self,
        u: UnitId,
        skill: i32,
        lvl: i32,
        missile: u16,
        lob: bool,
        aim: MissileAim,
    ) {
    }

    // ---- learning (levels.md §6.4)

    /// `0x0056C700`.
    fn is_class_skill(&self, u: UnitId, skill: i32) -> bool {
        false
    }
    /// `0x00570080` after the cost check: spend, add a level, refresh,
    /// passive state, `0x00646D60`; refund on failure.
    fn add_skill_level(&mut self, u: UnitId, skill: i32, cost: i32) {}
    /// §6.4 step 5: `0x0055F4F0(…, 1)`, then `0x0056DE40(unit)`.
    fn after_skill_point(&mut self, u: UnitId) {}

    // ---- items and equipment (items specs)

    /// `0x006235A0`.
    fn can_dual_wield(&self, u: UnitId) -> bool {
        false
    }
    fn equippable(&self, item: UnitId) -> bool {
        false
    }
    fn bow_equipped(&self, u: UnitId) -> bool {
        false
    }
    fn cursor_item(&self, u: UnitId) -> bool {
        false
    }

    // ---- units, path, relations (units / path specs)

    /// The state mask test of `range` (mask 0x26).
    fn state_mask(&self, u: UnitId, mask: u32) -> bool {
        false
    }
    /// Melee range `0x00622C40`.
    fn in_melee_range(&self, u: UnitId, target: UnitId) -> bool {
        false
    }
    /// Position of a unit the server does not stage (path).
    fn position(&self, u: UnitId) -> (i32, i32) {
        (0, 0)
    }
    fn owner(&self, u: UnitId) -> Option<UnitId> {
        None
    }
    /// `0x00548A50`.
    fn run_to(&mut self, u: UnitId, target: UnitId, e: SkillEntry) {}
    fn target(&self, u: UnitId) -> Option<UnitId> {
        None
    }
    fn clear_target(&mut self, u: UnitId) {}
    /// Unit +0x38 bits 8+ (`0x006212C0`).
    fn event_arg(&self, u: UnitId) -> i32 {
        0
    }
    fn set_event_arg(&mut self, u: UnitId, a: i32) {}
    /// `0x00553490`, `0x00554CA0`.
    fn step_path(&mut self, u: UnitId) -> i32 {
        0
    }
    /// `0x00554200`, `0x005542C0`, `0x00554D20`.
    fn is_hostile(&self, a: UnitId, b: UnitId) -> bool {
        false
    }
    fn is_pet(&self, a: UnitId, b: UnitId) -> bool {
        false
    }
    fn is_ally(&self, a: UnitId, b: UnitId) -> bool {
        false
    }
    fn room(&self, u: UnitId) -> RoomKind {
        RoomKind::None
    }
    /// `0x0056D2C0`.
    fn target_position(&self, u: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// `0x00645950`.
    fn line_clear(&self, u: UnitId, to: (i32, i32), mask: u32) -> bool {
        false
    }

    // ---- vitals (vitals.md §2, §3)

    /// `0x0064C040`.
    fn refresh(&mut self, u: UnitId) {}
    /// vitals.md §3 step 7.
    fn level_up_notify(&mut self, u: UnitId) {}
    /// Unit event 12.
    fn level_up_event(&mut self, u: UnitId) {}
}
