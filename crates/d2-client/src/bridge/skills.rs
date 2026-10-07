// Spec: specs/client/msg-skills.md (§1, §2)
//! The client skill list (unit +0xA8, §1) and the shared list operations
//! the skill messages run (§2): add, assign, remove, select. Entry fields
//! and the level formula are `skills/levels.md` §1 (single owner).
//!
//! The passive-state parts of add, remove and refresh (§2 rules 1, 2.2,
//! 4) act on the unit's state bits and state lists, not on the list: the
//! operations record them in [`SkillList::fx`] in call order and the
//! message handler applies them ([`super::passive::apply`]).

use super::world::{SkillRow, MONSTER, PLAYER};

/// Owner GUID of a native entry (−1, §1 rule 3).
pub const NATIVE: u32 = u32::MAX;

/// One entry (0x40 bytes, zeroed at creation; §1 rule 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillEntry {
    /// The skill id (via the record pointer +0x00).
    pub skill: u16,
    /// +0x08.
    pub mode: u32,
    /// +0x28 base level.
    pub base: i32,
    /// +0x2C level bonus (writers: open question 2).
    pub level_bonus: i32,
    /// +0x30 quantity.
    pub quantity: i32,
    /// +0x34 owner GUID; [`NATIVE`] = the unit's own skill.
    pub owner: u32,
    /// +0x38 charges.
    pub charges: i32,
    /// +0x3C has-charges flag.
    pub has_charges: bool,
}

/// The list (§1 rule 2): entries in list order (new ones appended at the
/// tail) and the left (+0x08), right (+0x0C) and current (+0x10) entry
/// references, as indices into `entries`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillList {
    pub entries: Vec<SkillEntry>,
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub current: Option<usize>,
    /// What the list operations owe the unit (passive state bits and
    /// refreshes, §2 rules 1, 2.2, 4), in call order; the handler applies
    /// and drains them ([`super::passive::apply`]).
    pub fx: Vec<SkillFx>,
}

/// An effect of a list operation on its unit (§2 rules 1, 2.2, 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillFx {
    /// The passive state on (`0x00643690`, `0x00639DB0(unit, state, 1)`).
    StateOn(u8),
    /// The passive state off (`0x00639DB0(unit, state, 0)`).
    StateOff(u8),
    /// The refresh `0x00646D60(unit, skill)`.
    Refresh(u16),
}

/// A skill-list operation the original does not complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SkillError {
    /// Select with a skill outside the table (fatal 0x668, §2 rule 3).
    #[error("fatal assert 0x668 (select: skill {0} outside the skills table)")]
    BadSkill(u16),
    /// Remove would leave a hand pointing at the freed entry (no entry of
    /// skill 0, or skill 0's own entry removed): a dangling pointer in
    /// 1.14d.
    #[error("skill list: a hand references the removed entry (dangling in 1.14d)")]
    Dangling,
}

/// The unit facts the operations read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub unit_type: u8,
    pub class: u32,
}

impl SkillList {
    /// "The entry of (skill, owner)" (`0x006439B0`, §1 rule 4): the first
    /// in list order.
    pub fn find(&self, skill: u16, owner: u32) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.skill == skill && e.owner == owner)
    }

    /// The native entry of `skill`.
    pub fn native(&self, skill: u16) -> Option<usize> {
        self.find(skill, NATIVE)
    }

    pub fn left_entry(&self) -> Option<&SkillEntry> {
        self.entries.get(self.left?)
    }

    pub fn right_entry(&self) -> Option<&SkillEntry> {
        self.entries.get(self.right?)
    }
}

/// The skills record of `skill` when it is inside the table (count =
/// the rows).
fn row(rows: &[SkillRow], skill: u16) -> Option<&SkillRow> {
    rows.get(usize::from(skill))
}

/// `max_level(skill)` (`skills/levels.md` §6 rule 1): `maxlvl` if > 0,
/// else 20.
pub fn max_level(r: &SkillRow) -> i32 {
    let m = i32::from(r.maxlvl as i16);
    if m > 0 {
        m
    } else {
        20
    }
}

/// The skill's passive state when it has one (`passivestate` > 0).
fn passive_state(rows: &[SkillRow], skill: u16) -> Option<u16> {
    row(rows, skill)
        .map(|r| r.passivestate)
        .filter(|&p| p as i16 > 0)
}

/// Refresh `0x00646D60` (§2 rule 4): owed to the unit when the skill has
/// a passive state (the state list lives on the unit,
/// [`super::passive::refresh`]).
pub fn refresh(list: &mut SkillList, rows: &[SkillRow], skill: u16) -> Result<(), SkillError> {
    if passive_state(rows, skill).is_some() {
        list.fx.push(SkillFx::Refresh(skill));
    }
    Ok(())
}

/// Add `0x00647110` (§2 rule 1). Returns the entry, or none for a skill
/// outside the table. The refresh error (a passive skill) comes after
/// the list is updated.
pub fn add(
    list: &mut SkillList,
    rows: &[SkillRow],
    unit: Owner,
    skill: u16,
) -> Result<Option<usize>, SkillError> {
    let Some(r) = row(rows, skill) else {
        return Ok(None);
    };
    let i = match list.native(skill) {
        Some(i) => {
            let e = &mut list.entries[i];
            if e.base < max_level(r) || unit.unit_type == MONSTER {
                e.base += 1;
            }
            i
        }
        None => {
            let mode = if unit.unit_type == MONSTER {
                u32::from(r.monanim)
            } else if unit.unit_type == PLAYER && unit.class == 6 && skill == 5 {
                0x10
            } else {
                u32::from(r.anim)
            };
            list.entries.push(SkillEntry {
                skill,
                mode,
                base: 1,
                owner: NATIVE,
                ..SkillEntry::default()
            });
            list.entries.len() - 1
        }
    };
    // The passive state on (`0x00643690`, `0x00639DB0(unit, state, 1)`),
    // then the refresh.
    if let Some(p) = passive_state(rows, skill) {
        list.fx.push(SkillFx::StateOn(p as u8));
    }
    refresh(list, rows, skill)?;
    Ok(Some(i))
}

/// Select `0x00643BC0` (left) / `0x00643C50` (right) (§2 rule 3).
pub fn select(
    list: &mut SkillList,
    rows: &[SkillRow],
    left: bool,
    skill: u16,
    owner: u32,
) -> Result<(), SkillError> {
    if row(rows, skill).is_none() {
        return Err(SkillError::BadSkill(skill));
    }
    if let Some(i) = list.find(skill, owner) {
        if left {
            list.left = Some(i);
        } else {
            list.right = Some(i);
        }
    }
    Ok(())
}

/// Remove `0x00646FD0` (§2 rule 2.2): the native entry's passive state
/// off, a left / right reference to it reset to (skill 0, native), a
/// current reference cleared, then the entry is unlinked.
pub fn remove(list: &mut SkillList, rows: &[SkillRow], skill: u16) -> Result<(), SkillError> {
    let Some(i) = list.native(skill) else {
        return Ok(());
    };
    if let Some(p) = passive_state(rows, skill) {
        list.fx.push(SkillFx::StateOff(p as u8));
    }
    if list.left == Some(i) {
        select(list, rows, true, 0, NATIVE)?;
    }
    if list.right == Some(i) {
        select(list, rows, false, 0, NATIVE)?;
    }
    if list.current == Some(i) {
        list.current = None;
    }
    if list.left == Some(i) || list.right == Some(i) {
        return Err(SkillError::Dangling);
    }
    list.entries.remove(i);
    for j in [&mut list.left, &mut list.right, &mut list.current]
        .into_iter()
        .flatten()
    {
        if *j > i {
            *j -= 1;
        }
    }
    Ok(())
}

/// Assign `0x00647280(unit, skill, level, remove)` (§2 rule 2).
pub fn assign(
    list: &mut SkillList,
    rows: &[SkillRow],
    unit: Owner,
    skill: u16,
    level: i32,
    remove_flag: bool,
) -> Result<(), SkillError> {
    if level != 0 {
        let e = match list.native(skill) {
            Some(i) => Some(i),
            None => add(list, rows, unit, skill)?,
        };
        if let Some(i) = e {
            list.entries[i].base = level;
        }
        return refresh(list, rows, skill);
    }
    if remove_flag {
        remove(list, rows, skill)?;
        return refresh(list, rows, skill);
    }
    let e = match list.native(skill) {
        Some(i) => Some(i),
        None => {
            add(list, rows, unit, skill)?;
            list.native(skill)
        }
    };
    let Some(i) = e else {
        return Ok(());
    };
    list.entries[i].base = 0;
    refresh(list, rows, skill)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(n: usize) -> Vec<SkillRow> {
        vec![SkillRow::default(); n]
    }

    const PLAYER0: Owner = Owner {
        unit_type: PLAYER,
        class: 0,
    };

    // Covers: specs/client/msg-skills.md §2 r1
    #[test]
    fn add_appends_then_raises_up_to_max_level() {
        let mut t = rows(4);
        t[2].anim = 7;
        t[2].maxlvl = 2;
        let mut l = SkillList::default();
        assert_eq!(add(&mut l, &t, PLAYER0, 2), Ok(Some(0)));
        assert_eq!(
            l.entries[0],
            SkillEntry {
                skill: 2,
                mode: 7,
                base: 1,
                owner: NATIVE,
                ..SkillEntry::default()
            }
        );
        add(&mut l, &t, PLAYER0, 2).unwrap();
        add(&mut l, &t, PLAYER0, 2).unwrap();
        assert_eq!(l.entries[0].base, 2, "a player stops at max_level");
        let monster = Owner {
            unit_type: MONSTER,
            class: 0,
        };
        let mut m = SkillList::default();
        t[2].monanim = 3;
        for _ in 0..3 {
            add(&mut m, &t, monster, 2).unwrap();
        }
        assert_eq!((m.entries[0].mode, m.entries[0].base), (3, 3));
        assert_eq!(add(&mut l, &t, PLAYER0, 4), Ok(None), "outside the table");
    }

    // Covers: specs/client/msg-skills.md §2 r1
    #[test]
    fn assassin_skill_5_has_mode_0x10() {
        let t = rows(6);
        let mut l = SkillList::default();
        let sin = Owner {
            unit_type: PLAYER,
            class: 6,
        };
        add(&mut l, &t, sin, 5).unwrap();
        assert_eq!(l.entries[0].mode, 0x10);
    }

    // Covers: specs/skills/levels.md §6 r1
    #[test]
    fn max_level_defaults_to_20() {
        let mut r = SkillRow::default();
        assert_eq!(max_level(&r), 20);
        r.maxlvl = 0xFFFF;
        assert_eq!(max_level(&r), 20);
        r.maxlvl = 3;
        assert_eq!(max_level(&r), 3);
    }

    // Covers: specs/client/msg-skills.md §2 r2
    #[test]
    fn assign_level_zero_creates_at_base_zero_or_removes() {
        let t = rows(40);
        let mut l = SkillList::default();
        assign(&mut l, &t, PLAYER0, 36, 0, false).unwrap();
        assert_eq!((l.entries[0].skill, l.entries[0].base), (36, 0));
        assign(&mut l, &t, PLAYER0, 36, 1, false).unwrap();
        assert_eq!(l.entries[0].base, 1);
        assign(&mut l, &t, PLAYER0, 36, 0, true).unwrap();
        assert!(l.entries.is_empty());
        // Outside the table: nothing.
        assign(&mut l, &t, PLAYER0, 40, 0, false).unwrap();
        assign(&mut l, &t, PLAYER0, 40, 3, false).unwrap();
        assert!(l.entries.is_empty());
    }

    // Covers: specs/client/msg-skills.md §2 r2, §2 r3
    #[test]
    fn remove_resets_a_hand_to_skill_0() {
        let t = rows(40);
        let mut l = SkillList::default();
        assign(&mut l, &t, PLAYER0, 0, 1, false).unwrap();
        assign(&mut l, &t, PLAYER0, 36, 1, false).unwrap();
        assign(&mut l, &t, PLAYER0, 37, 1, false).unwrap();
        select(&mut l, &t, false, 36, NATIVE).unwrap();
        select(&mut l, &t, true, 37, NATIVE).unwrap();
        l.current = Some(1);
        assign(&mut l, &t, PLAYER0, 36, 0, true).unwrap();
        assert_eq!(
            l.entries.iter().map(|e| e.skill).collect::<Vec<_>>(),
            [0, 37]
        );
        assert_eq!(l.right_entry().map(|e| e.skill), Some(0));
        assert_eq!(l.left_entry().map(|e| e.skill), Some(37));
        assert_eq!(l.current, None);
        // A hand on the removed skill 0 itself would dangle.
        select(&mut l, &t, false, 0, NATIVE).unwrap();
        assert_eq!(
            assign(&mut l, &t, PLAYER0, 0, 0, true),
            Err(SkillError::Dangling)
        );
    }

    // Covers: specs/client/msg-skills.md §2 r3
    #[test]
    fn select_needs_the_skill_in_the_table_and_an_entry() {
        let t = rows(3);
        let mut l = SkillList::default();
        assert_eq!(
            select(&mut l, &t, true, 3, NATIVE),
            Err(SkillError::BadSkill(3))
        );
        select(&mut l, &t, true, 2, NATIVE).unwrap();
        assert_eq!(l.left, None, "no entry: unchanged");
    }

    // Covers: specs/client/msg-skills.md §2 r4
    #[test]
    fn a_passive_skill_owes_its_state_and_refreshes() {
        let mut t = rows(3);
        t[1].passivestate = 5;
        let mut l = SkillList::default();
        assert_eq!(assign(&mut l, &t, PLAYER0, 1, 1, false), Ok(()));
        assert_eq!((l.entries.len(), l.entries[0].base), (1, 1));
        // Add: state on, refresh; then assign's own refresh.
        assert_eq!(
            l.fx,
            [
                SkillFx::StateOn(5),
                SkillFx::Refresh(1),
                SkillFx::Refresh(1)
            ]
        );
        l.fx.clear();
        assert_eq!(refresh(&mut l, &t, 2), Ok(()));
        assert!(l.fx.is_empty(), "no passive state: nothing owed");
        // Remove: state off, then assign's refresh (no entry: the list of
        // the state is freed by the refresh).
        assign(&mut l, &t, PLAYER0, 1, 0, true).unwrap();
        assert_eq!(l.fx, [SkillFx::StateOff(5), SkillFx::Refresh(1)]);
    }
}
