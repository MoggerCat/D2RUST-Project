// Spec: specs/monsters/ai.md §7.1, §7.5 (mode request, target); specs/skills/use.md §5.2–§5.3 (monster start and per-frame)
//! The monster side of the play preview's skill seams (`q-monster-ai`):
//! the target a mode request hands the unit, and the skill a monster's
//! attack mode uses. `LocalSeams` keeps one [`MonsterAi`] and answers the
//! monster calls of `Pending` / `UseRest` from it, so a monster's A1
//! runs the player's own skill pipeline (`use.md` §5.3 start, the Attack
//! do function `skills/bodies.md` §4.1) and the hit takes the server's
//! damage path.
//!
//! Everything here is a preview fill (decision D1), marked
//! `// d2rs-own, unverified`; the open points are PROVISIONAL (REC-111 in
//! `docs/HANDOFF.md` §7).

use std::collections::BTreeMap;

use d2_data::tables::{Monstats, Monstats2};
use d2_sim::monsters::ai::ModeTarget;
use d2_sim::skills::SkillEntry;
use d2_sim::units::UnitId;

/// The skill id of a monster's melee attack (skills.txt row 0, "Attack").
// PROVISIONAL (skills/use.md OQ6; REC-111): the monster's attack modes use
// skill 0 at level 1; monstats `Skill1`…`Skill8` / `Sk1mode`… (which skill
// an attack mode picks) are not specified for the preview. d2rs-own,
// unverified.
pub const ATTACK_SKILL: i32 = 0;

/// Per-unit monster fields of the skill seams.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonsterAi {
    /// The unit a mode request targets (`Pending::set_mode_target`).
    targets: BTreeMap<UnitId, UnitId>,
    /// The current skill (`Pending::set_current_skill`).
    current: BTreeMap<UnitId, i32>,
    /// The modes each class has (monstats2 `mDT`…`mRN`, bit = mode).
    pub modes: Vec<u16>,
}

impl MonsterAi {
    /// The per-class facts of the user's (or the test's) tables.
    pub fn from_tables(monstats: &[Monstats], monstats2: &[Monstats2]) -> Self {
        let modes = monstats
            .iter()
            .map(|m| {
                monstats2.get(usize::from(m.monstatsex)).map_or(0, |x| {
                    [
                        x.mdt, x.mnu, x.mwl, x.mgh, x.ma1, x.ma2, x.mbl, x.msc, x.ms1, x.ms2,
                        x.ms3, x.ms4, x.mdd, x.mkb, x.msq, x.mrn,
                    ]
                    .iter()
                    .enumerate()
                    .fold(0, |acc, (i, &on)| acc | u16::from(on) << i)
                })
            })
            .collect();
        Self {
            modes,
            ..Self::default()
        }
    }

    /// `0x0046C140(class, mode)`: the class has the mode.
    pub fn class_has_mode(&self, class: i32, mode: u8) -> bool {
        usize::try_from(class)
            .ok()
            .and_then(|c| self.modes.get(c))
            .is_some_and(|&m| mode < 16 && m >> mode & 1 != 0)
    }

    /// A mode request's target: a unit is kept, a point clears the unit.
    pub fn set_target(&mut self, unit: UnitId, target: ModeTarget) {
        match target {
            ModeTarget::Unit(t) => {
                self.targets.insert(unit, t);
            }
            ModeTarget::Point(..) => {
                self.targets.remove(&unit);
            }
        }
    }

    pub fn target(&self, unit: UnitId) -> Option<UnitId> {
        self.targets.get(&unit).copied()
    }

    /// Current skill := `skill`; false when out of the skills table.
    pub fn set_current(&mut self, unit: UnitId, skill: i32) -> bool {
        if skill < 0 {
            return false;
        }
        self.current.insert(unit, skill);
        true
    }

    /// The used skill (`0x00620250`) of a monster: the one the request set
    /// ([`Self::set_current`]), else Attack.
    // PROVISIONAL (monsters/ai.md §7.1, skills/use.md OQ6; REC-111): the AI's
    // plain attack request (`0x005DDF90`) sets no skill, and the spec does
    // not say which entry the unit then uses; the preview takes Attack
    // (skill 0, level 1). d2rs-own, unverified.
    pub fn used_skill(&self, unit: UnitId, monster: bool) -> Option<SkillEntry> {
        let skill = match self.current.get(&unit) {
            Some(&skill) => skill,
            None if monster && self.targets.contains_key(&unit) => ATTACK_SKILL,
            None => return None,
        };
        Some(SkillEntry {
            skill,
            base: 1,
            level_bonus: 0,
            owner_guid: -1,
            charges: 0,
            has_charges: false,
        })
    }
}
