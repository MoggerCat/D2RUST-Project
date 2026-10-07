// Spec: specs/client/msg-skills.md (§3–§6)
//! The skill messages: 0x94 BaseSkillLevels (the native skills at
//! join), 0x21 UpdateItemOSkill (one skill's hard-point level), 0x22
//! UpdateItemSkill (a tome / scroll quantity) and 0x23 SetSkill (left /
//! right skill), over the client skill list of [`super::super::skills`].

use super::super::dispatch::{HandlerError, Message};
use super::super::skills::{self, Owner, SkillError, SkillList};
use super::super::world::{ClientWorld, SkillRow, UnitKey, PLAYER};
use super::Bytes;

impl From<SkillError> for HandlerError {
    fn from(e: SkillError) -> Self {
        match e {
            SkillError::BadSkill(_) => HandlerError::Fatal(0x668),
            SkillError::PassiveState { .. } => HandlerError::Unspecified(
                "client/msg-skills.md §2 r4, client/stat-lists.md §1 r2: a passive state needs \
                 the client state bits and stat list",
            ),
            SkillError::Dangling => {
                HandlerError::Invalid("skill list: a hand references the removed entry")
            }
        }
    }
}

/// The unit `key` in set S with its skill list; `None` when the unit is
/// not in S. A unit without a list gives `Some((_, None))`: every
/// operation then does nothing (§1 rule 1).
fn unit_list(w: &mut ClientWorld, key: UnitKey) -> Option<(Owner, Option<&mut SkillList>)> {
    let u = w.units.get_mut(&key)?;
    let owner = Owner {
        unit_type: u.key.unit_type,
        class: u.class,
    };
    Some((owner, u.skills.as_mut()))
}

fn rows<'a>(msg: &Message<'a>) -> &'a [SkillRow] {
    &msg.inputs.tables.skills
}

/// 0x94 BaseSkillLevels (§3): count n u8@1, player GUID u32@2, n entries
/// (skill u16, level u8) from @6; each assigned with remove 0.
pub fn base_skill_levels(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    let n = usize::from(b.u8(1)?);
    if msg.bytes.len() != 6 + 3 * n {
        return Err(HandlerError::Invalid("0x94 is 6 + 3 n bytes"));
    }
    let key = UnitKey::new(PLAYER, b.u32(2)?);
    let Some((owner, list)) = unit_list(w, key) else {
        return Ok(());
    };
    let Some(list) = list else {
        return Ok(());
    };
    // Every entry is assigned; the first pending part is reported after
    // the loop, as the original runs them all.
    let mut first = Ok(());
    for i in 0..n {
        let at = 6 + 3 * i;
        let (skill, level) = (b.u16(at)?, b.u8(at + 2)?);
        let r = skills::assign(list, rows(msg), owner, skill, i32::from(level), false);
        if first.is_ok() {
            first = r;
        }
    }
    Ok(first?)
}

/// 0x21 UpdateItemOSkill (§4): unit type u8@1, remove u8@2, GUID u32@3,
/// skill u16@7, base level u8@9; then the skill-tree flag
/// `[0x007C0C3C]` := 0. Bytes @10 and @11 are not read.
pub fn update_item_oskill(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 12 {
        return Err(HandlerError::Invalid("0x21 is 12 bytes"));
    }
    let key = UnitKey::new(b.u8(1)?, b.u32(3)?);
    let (remove, skill, level) = (b.u8(2)? != 0, b.u16(7)?, b.u8(9)?);
    let Some((owner, list)) = unit_list(w, key) else {
        return Ok(());
    };
    let result = match list {
        Some(list) => skills::assign(list, rows(msg), owner, skill, i32::from(level), remove),
        None => Ok(()),
    };
    // `0x004AA8F0` runs after the assign returns.
    w.skill_tree_flag = Some(0);
    Ok(result?)
}

/// 0x22 UpdateItemSkill (§5): flag u8@11 ≠ 0 → nothing; else the player
/// (0, GUID u32@3)'s native entry of skill u16@7 gets quantity u8@9;
/// no such entry is fatal 0xAD6.
pub fn update_item_skill(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 12 {
        return Err(HandlerError::Invalid("0x22 is 12 bytes"));
    }
    if b.u8(11)? != 0 {
        return Ok(());
    }
    let key = UnitKey::new(PLAYER, b.u32(3)?);
    let (skill, quantity) = (b.u16(7)?, b.u8(9)?);
    let Some((_, list)) = unit_list(w, key) else {
        return Ok(());
    };
    let Some(list) = list else {
        return Ok(());
    };
    let i = list.native(skill).ok_or(HandlerError::Fatal(0xAD6))?;
    list.entries[i].quantity = i32::from(quantity);
    Ok(())
}

/// 0x23 SetSkill (§6): the player (0, GUID u32@2) selects (skill u16@7,
/// owner u32@9) as its left skill when hand u8@6 ≠ 0, else as its right.
pub fn set_skill(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    let b = Bytes(msg.bytes);
    if msg.bytes.len() != 13 {
        return Err(HandlerError::Invalid("0x23 is 13 bytes"));
    }
    let key = UnitKey::new(PLAYER, b.u32(2)?);
    let (left, skill, owner) = (b.u8(6)? != 0, b.u16(7)?, b.u32(9)?);
    let Some((_, list)) = unit_list(w, key) else {
        return Ok(());
    };
    let Some(list) = list else {
        return Ok(());
    };
    skills::select(list, rows(msg), left, skill, owner)?;
    Ok(())
}
