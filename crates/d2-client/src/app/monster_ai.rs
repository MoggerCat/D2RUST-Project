// Spec: specs/monsters/ai.md §7.1, §7.5 (mode request, target); specs/skills/use.md §5.2–§5.3 (monster start and per-frame)
//! The monster side of the play preview's skill seams (`q-monster-ai`):
//! the target a mode request hands the unit, and the skill a monster's
//! attack mode uses. `LocalSeams` keeps one [`MonsterAi`] and answers the
//! monster calls of `Pending` / `UseRest` from it, so a monster's skill
//! mode runs the player's own skill pipeline (`use.md` §5.3 start) and
//! the hit takes the server's damage path. A plain A1 / A2 (`ai.md` §7.1
//! `0x005DDF90`) has no used skill entry.
//!
//! The open point is PROVISIONAL (REC-111 part 3 in `docs/HANDOFF.md`
//! §7).

use std::collections::BTreeMap;

use d2_data::tables::{Monstats, Monstats2};
use d2_sim::monsters::ai::ModeTarget;
use d2_sim::skills::SkillEntry;
use d2_sim::units::UnitId;

/// Per-unit monster fields of the skill seams.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonsterAi {
    /// The unit a mode request targets (`Pending::set_mode_target`).
    targets: BTreeMap<UnitId, UnitId>,
    /// The current skill (`Pending::set_current_skill`).
    current: BTreeMap<UnitId, i32>,
    /// A summoned monster's skill entries' base levels (`0x0056DEB0`,
    /// `skills/bodies.md` §6.5 step 6), mirrored from the sim each frame
    /// (`sync_seams`); a monster without one uses level 1.
    levels: BTreeMap<UnitId, BTreeMap<i32, i32>>,
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

    /// Replace the mirrored summon skill levels.
    pub fn set_levels(&mut self, levels: BTreeMap<UnitId, BTreeMap<i32, i32>>) {
        self.levels = levels;
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

    /// Current skill := none (the used entry cleared).
    pub fn clear_current(&mut self, unit: UnitId) {
        self.current.remove(&unit);
    }

    /// The used skill (`0x00620250`) of a unit: the one the request set
    /// ([`Self::set_current`], `0x005DEAD0` / `0x005DE000`), else none. A
    /// mode from the plain request `0x005DDF90` has **no** used skill
    /// entry (`monsters/ai.md` §7.1 row `0x005DDF90`: the builder clears
    /// list +0x10; Attack is not substituted), so its readers take their
    /// "no used skill entry" branch.
    pub fn used_skill(&self, unit: UnitId) -> Option<SkillEntry> {
        let &skill = self.current.get(&unit)?;
        Some(SkillEntry {
            skill,
            base: self
                .levels
                .get(&unit)
                .and_then(|m| m.get(&skill))
                .copied()
                .unwrap_or(1),
            level_bonus: 0,
            owner_guid: -1,
            charges: 0,
            has_charges: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/monsters/ai.md §7.1
    #[test]
    fn a_plain_request_leaves_no_used_skill() {
        let (m, p) = (UnitId(5), UnitId(1));
        let mut ai = MonsterAi::default();
        // `0x005DDF90(A1, target)`: a target, no skill.
        ai.set_target(m, ModeTarget::Unit(p));
        assert_eq!(ai.target(m), Some(p));
        assert_eq!(ai.used_skill(m), None);
        // `0x005DEAD0(mode, k, target)`: the skill k is the used entry.
        assert!(ai.set_current(m, 47));
        let e = ai.used_skill(m).unwrap();
        assert_eq!((e.skill, e.owner_guid), (47, -1));
        // A skill id below 0 sets nothing.
        assert!(!ai.set_current(UnitId(6), -1));
        assert_eq!(ai.used_skill(UnitId(6)), None);
    }
}
