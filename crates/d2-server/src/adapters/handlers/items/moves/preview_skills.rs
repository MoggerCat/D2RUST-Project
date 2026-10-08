// Spec: specs/items/inventory.md §5.5, §5.8; specs/skills/levels.md §7.1; specs/client/msg-skills.md §2, §4
//! The skill seams of the preview item-move rest (`InvRest`: mouse
//! skills, `has_skill_owned`, the ranged-throw test, the saved mouse
//! skills) over the players' server skill lists
//! (`ActionHooks::skill_lists`, `d2_sim::skills::list`: never a second
//! copy). The lists are lent to the rest for the length of one move call
//! ([`SkillStage`]) and returned after it, when [`sync_oskills`] has run
//! the stat 97 / 107 callback (`levels.md` §7.1) for the items worn.
//!
//! PROVISIONAL (REC-266, d2rs-own, unverified): the callback is run once
//! after each item-move call rather than at each stat change (the stat
//! lists have no skill-callback hook in the play host), `use_state`
//! stays "usable", and a scroll or tome's quantity send (0x22) is not
//! made.

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::fixup::maps::EquivMatrix;
use d2_sim::items::moves::Owner;
use d2_sim::skills::list::{ListOwner, SkillList, NATIVE};
use d2_sim::units::messages::{set_skill, update_oskill};
use d2_sim::units::UnitId;
use d2_sim::wiring::action::ActionTables;

/// `thro` (item type 48, `inventory.md` §5.8).
const TYPE_THRO: usize = 48;
/// Stats 97 `item_nonclassskill`, 107 `item_singleskill`.
const NONCLASSSKILL: u16 = 97;
const SINGLESKILL: u16 = 107;

/// One player's list, lent for a call.
#[derive(Debug)]
pub struct StagedList {
    pub list: SkillList,
    pub unit: UnitId,
    pub class: i32,
}

/// The players' skill lists lent to the rest for a move call.
#[derive(Debug)]
pub struct SkillStage {
    pub tables: Arc<ActionTables>,
    pub lists: BTreeMap<Owner, StagedList>,
}

/// The skills.txt part of `0x0055C560`: `itypea1` > 0 and is-a `thro`,
/// `range` 2, per skill row.
pub fn throw_rows(t: &ActionTables, equiv: &EquivMatrix) -> Vec<bool> {
    t.skills
        .skills
        .iter()
        .map(|r| r.itypea1 > 0 && equiv.get(usize::from(r.itypea1), TYPE_THRO) && r.range == 2)
        .collect()
}

/// The preview rest's skill state.
#[derive(Debug, Default)]
pub struct PreviewSkills {
    pub stage: Option<SkillStage>,
    pub throw_rows: Vec<bool>,
    /// Player data +0x70..+0x7C: the saved mouse skill per (player, left).
    saved: BTreeMap<(Owner, bool), (i32, i32)>,
    /// S→C messages the selections made.
    pub sent: Vec<(Owner, Vec<u8>)>,
}

impl PreviewSkills {
    fn list(&self, u: Owner) -> Option<&StagedList> {
        self.stage.as_ref()?.lists.get(&u)
    }

    /// `0x00620190` / `0x006201D0`.
    pub fn mouse_skill(&self, u: Owner, left: bool) -> Option<(i32, i32)> {
        let l = &self.list(u)?.list;
        let i = if left { l.left } else { l.right }?;
        l.entries.get(i).map(|e| (i32::from(e.skill), e.owner))
    }

    /// `0x005701B0`: select on a side; the client is told (S→C 0x23).
    pub fn select_skill(&mut self, u: Owner, left: bool, s: (i32, i32)) {
        let Some(st) = self.stage.as_mut() else {
            return;
        };
        let rows = &st.tables.skills.skills;
        let Some(sl) = st.lists.get_mut(&u) else {
            return;
        };
        let before = if left { sl.list.left } else { sl.list.right };
        if sl.list.select(rows, left, s.0, s.1).is_err() {
            return;
        }
        let after = if left { sl.list.left } else { sl.list.right };
        if before != after {
            let hand = u8::from(left);
            let owner = s.1 as u32;
            self.sent
                .push((u, set_skill(0, u.guid, hand, s.0 as u16, owner).to_vec()));
        }
    }

    /// `0x006439B0` with (id, owner).
    pub fn has_skill_owned(&self, u: Owner, s: (i32, i32)) -> bool {
        self.list(u)
            .is_some_and(|l| l.list.find(s.0, s.1).is_some())
    }

    pub fn throw_skill_row(&self, skill: i32) -> bool {
        usize::try_from(skill)
            .ok()
            .and_then(|i| self.throw_rows.get(i))
            .copied()
            .unwrap_or(false)
    }

    pub fn saved_mouse_skill(&self, u: Owner, left: bool) -> (i32, i32) {
        self.saved.get(&(u, left)).copied().unwrap_or((0, -1))
    }

    pub fn set_saved_mouse_skill(&mut self, u: Owner, left: bool, s: (i32, i32)) {
        self.saved.insert((u, left), s);
    }

    /// The quantity of the native entry of `skill` (`0x006439B0`).
    pub fn skill_quantity(&self, u: Owner, skill: i32) -> Option<i32> {
        let l = &self.list(u)?.list;
        l.native(skill).map(|i| l.entries[i].quantity)
    }

    pub fn set_skill_quantity(&mut self, u: Owner, skill: i32, q: i32) {
        if let Some(sl) = self.stage.as_mut().and_then(|s| s.lists.get_mut(&u)) {
            if let Some(i) = sl.list.native(skill) {
                sl.list.entries[i].quantity = q;
            }
        }
    }

    /// `0x00570080`: the skill is added to the list.
    pub fn learn_skill(&mut self, u: Owner, skill: i32) {
        let Some(st) = self.stage.as_mut() else {
            return;
        };
        let rows = &st.tables.skills.skills;
        if let Some(sl) = st.lists.get_mut(&u) {
            sl.list.add(rows, ListOwner::player(sl.class), skill);
        }
    }
}

/// Stats 97 / 107 for the worn items (`levels.md` §7.1): a skill with a
/// positive total and no native entry gets one (base 0) and the client
/// its 0x21; a base-0 entry whose total fell to 0 is removed (the hands
/// on it back to Attack; no message). `total(stat, skill)` reads the
/// player's stat list.
pub fn sync_oskills(
    st: &mut SkillStage,
    u: Owner,
    total: impl Fn(&StagedList, u16, i32) -> i32,
    sent: &mut Vec<(Owner, Vec<u8>)>,
) {
    let rows = &st.tables.skills.skills;
    let Some(sl) = st.lists.get_mut(&u) else {
        return;
    };
    let lo = ListOwner::player(sl.class);
    for (s, row) in rows.iter().enumerate().skip(1) {
        let single = i32::from(row.charclass) == sl.class;
        let single = if single {
            total(sl, SINGLESKILL, s as i32).max(0)
        } else {
            0
        };
        let n = single + total(sl, NONCLASSSKILL, s as i32).max(0);
        let native = sl.list.native(s as i32);
        match native {
            None if n > 0 => {
                sl.list.set_base(rows, lo, s as i32, 0);
                sent.push((u, update_oskill(0, false, u.guid, s as u16, 0, 0).to_vec()));
            }
            Some(i) if n <= 0 && sl.list.entries[i].base == 0 => {
                for left in [true, false] {
                    let h = if left { sl.list.left } else { sl.list.right };
                    if h == Some(i) {
                        let _ = sl.list.select(rows, left, 0, NATIVE);
                    }
                }
                sl.list.remove(i);
            }
            _ => {}
        }
    }
}
