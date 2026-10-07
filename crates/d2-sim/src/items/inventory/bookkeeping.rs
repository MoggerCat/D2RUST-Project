// Spec: specs/items/inventory.md §5.5, §5.7, §5.8
//! The equipment bookkeeping of the inventory spec: the item-skill link
//! of scrolls and tomes (§5.5, `0x0055C110`) with the cube recount
//! (`0x0055FA40`), the inventory pass (§5.7, `0x0055DBC0`) and the weapon
//! bookkeeping of the mouse skills (§5.8, `0x0055C5C0`).
//!
//! The rules run on [`EquipWorld`]: the unit's inventory and items, its
//! stat-list links and its skill list (whose owner is the skills spec,
//! reached through the wiring's seams).

use super::{body, iflag, mode, node};

/// A skill of a unit's skill list: (skill id, owner GUID; −1 native).
pub type SkillRef = (i32, i32);

/// Itemtypes rows (§5.5 step 1, §5.8).
pub const TYPE_BOOK: i16 = 18;
pub const TYPE_SCROLL: i16 = 22;
pub const TYPE_WEAP: i16 = 45;
pub const TYPE_MELE: i16 = 46;
pub const TYPE_THRO: i16 = 48;
/// Stats (§5.5).
pub const STAT_NEWSKILLS: u16 = 5;
pub const STAT_QUANTITY: u16 = 70;
/// Skills selected by §5.8 (Throw, Left Hand Throw).
pub const SKILL_THROW: i32 = 2;
pub const SKILL_LEFT_HAND_THROW: i32 = 4;
/// `use_state` results that make a mouse skill unusable (no quantity, no
/// level; `skills/use.md` §2).
pub const USE_NO_QUANTITY: u8 = 2;
pub const USE_NO_LEVEL: u8 = 7;
/// Quality 5 (set; §5.7 step 5).
pub const QUALITY_SET: u8 = 5;
/// Unit flag 0x200 (+0xC4 bit 9; hirelings, §5.7).
pub const UNIT_FLAG_200: u32 = 0x200;
/// Unit type 0 (player).
pub const TYPE_PLAYER: u8 = 0;

/// The original's fatal asserts on these paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EquipFatal {
    /// §5.5 step 3: the skill is still absent after it was learned.
    #[error("item-skill link: skill {0} absent after learning")]
    LearnedSkillMissing(i32),
    /// §5.5 step 3 (add = 0) and the recount: the skill is absent.
    #[error("item-skill unlink: skill {0} absent")]
    UnlinkSkillMissing(i32),
    /// §5.8 step 5: a missing mouse skill reaches `0x00647960`.
    #[error("weapon bookkeeping: no mouse skill")]
    NoMouseSkill,
}

/// What the rules read and write. Items and units are unit ids; skills
/// are [`SkillRef`]s.
pub trait EquipWorld {
    // ---- units
    /// Unit type (0 player … 5 tile); `None`: no unit.
    fn unit_type(&self, u: crate::units::UnitId) -> Option<u8>;
    /// Unit flags (+0xC4).
    fn unit_flags(&self, u: crate::units::UnitId) -> u32;
    /// The unit has an inventory (+0x60).
    fn has_inventory(&self, u: crate::units::UnitId) -> bool;
    /// The inventory's item list, in list order.
    fn item_list(&self, u: crate::units::UnitId) -> Vec<crate::units::UnitId>;
    /// The item at body location `loc` (`0x0063BDE0`).
    fn body_item(&self, u: crate::units::UnitId, loc: u8) -> Option<crate::units::UnitId>;
    /// The weapon in use (inventory +0x1C, `0x0063BEF0`).
    fn weapon_in_use(&self, u: crate::units::UnitId) -> Option<crate::units::UnitId>;
    /// Adds `d` to a unit stat (`0x006272B0`).
    fn add_unit_stat(&mut self, u: crate::units::UnitId, stat: u16, d: i32);

    // ---- items
    /// Primary type (items +0x11E).
    fn item_type(&self, i: crate::units::UnitId) -> i16;
    /// Is-a `t` (`0x00629BB0`: equivalence, with the second type).
    fn item_is_type(&self, i: crate::units::UnitId, t: i16) -> bool;
    fn item_flags(&self, i: crate::units::UnitId) -> u32;
    fn set_item_flag(&mut self, i: crate::units::UnitId, bits: u32, on: bool);
    fn item_mode(&self, i: crate::units::UnitId) -> u8;
    /// Node kind (+0x69).
    fn item_node(&self, i: crate::units::UnitId) -> u8;
    /// Page (+0x45).
    fn item_page(&self, i: crate::units::UnitId) -> u8;
    fn item_quality(&self, i: crate::units::UnitId) -> u8;
    fn item_stat(&self, i: crate::units::UnitId, stat: u16) -> i32;
    /// The books row of the item's spell index (item suffix 0, item data
    /// +0x3E): `scrollskill` (scroll) or `bookskill`; `None`: no row.
    fn book_skill(&self, i: crate::units::UnitId, scroll: bool) -> Option<i32>;
    /// §5.6 active inventory item (`0x0062FF70`).
    fn active_inventory_item(&self, u: crate::units::UnitId, i: crate::units::UnitId) -> bool;
    /// §5.6 usable (`0x0055DB00`).
    fn usable(&self, u: crate::units::UnitId, i: crate::units::UnitId) -> bool;
    /// The item's stat list is linked to the unit (`0x00625820`).
    fn stat_linked(&self, u: crate::units::UnitId, i: crate::units::UnitId) -> bool;
    /// Stat link `0x0063D1D0`.
    fn stat_link(&mut self, u: crate::units::UnitId, i: crate::units::UnitId);
    /// Stat unlink `0x0063D2B0`.
    fn stat_unlink(&mut self, u: crate::units::UnitId, i: crate::units::UnitId);
    /// Deactivation `0x0055C730(X, U, 1, 1)`.
    fn deactivate(&mut self, i: crate::units::UnitId, u: crate::units::UnitId);
    /// Stat refresh `0x0055C2C0(X, U, 1)` (`items/properties.md` §9).
    fn stat_refresh(&mut self, i: crate::units::UnitId, u: crate::units::UnitId);
    /// Owner refresh `0x00621000(U, 1)` (`inventory-moves.md` §6.1).
    fn owner_refresh(&mut self, u: crate::units::UnitId);
    /// §5.7 step 8: S→C 0x48 (type U, arg 0, U's GUID; `0x0053D3C0`) to
    /// U's client, or for a non-player to its player owner's client.
    fn send_unit_refresh(&mut self, u: crate::units::UnitId);

    // ---- skills (the skill list; `skills/use.md` §2)
    /// The quantity (skill +0x30) of U's skill `skill` with owner −1
    /// (`0x006439B0`); `None`: absent.
    fn skill_quantity(&self, u: crate::units::UnitId, skill: i32) -> Option<i32>;
    /// Skill quantity := q (`0x00645120`).
    fn set_skill_quantity(&mut self, u: crate::units::UnitId, skill: i32, q: i32);
    /// Learn `skill` (`0x00570080`, skills owner).
    fn learn_skill(&mut self, u: crate::units::UnitId, skill: i32);
    /// S→C 0x22 (`0x0053C520`; `inventory-moves.md` §11).
    fn send_skill_quantity(&mut self, u: crate::units::UnitId, skill: i32, q: i32);
    /// The left (`0x00620190`) or right (`0x006201D0`) mouse skill.
    fn mouse_skill(&self, u: crate::units::UnitId, left: bool) -> Option<SkillRef>;
    /// Select a skill on a side (`0x005701B0`, EDX = 1 left / 0 right).
    fn select_skill(&mut self, u: crate::units::UnitId, left: bool, s: SkillRef);
    /// U has the skill (id, owner) (`0x006439B0`).
    fn has_skill(&self, u: crate::units::UnitId, s: SkillRef) -> bool;
    /// `0x00647960` (`skills/use.md` §2).
    fn use_state(&mut self, u: crate::units::UnitId, s: SkillRef) -> u8;
    /// The skills.txt part of the ranged-throw test `0x0055C560`:
    /// `itypea1` > 0 and is-a `thro` (`0x00629B50`), `range` = 2 (`rng`).
    fn throw_skill_row(&self, skill: i32) -> bool;

    // ---- player data (+0x70..+0x7C)
    /// The saved mouse skill of a side (left +0x74 / +0x7C, right +0x70 /
    /// +0x78).
    fn saved_mouse_skill(&self, u: crate::units::UnitId, left: bool) -> SkillRef;
    fn set_saved_mouse_skill(&mut self, u: crate::units::UnitId, left: bool, s: SkillRef);
}

use crate::units::UnitId;

/// §5.5 step 1 (`0x0055BFF0`): the skill of a scroll or tome.
fn item_skill<W: EquipWorld + ?Sized>(w: &W, i: UnitId) -> Option<(i32, bool)> {
    let t = w.item_type(i);
    let scroll = match t {
        TYPE_SCROLL => true,
        TYPE_BOOK => false,
        _ => return None,
    };
    w.book_skill(i, scroll).map(|s| (s, scroll))
}

/// Item-skill link `0x0055C110` (§5.5; `add` = 1 link `0x0055C270`, 0
/// unlink `0x0055C6E0`). Returns the routine's value (0 or 1). Only a
/// player owner acts.
pub fn item_skill_link<W: EquipWorld + ?Sized>(
    w: &mut W,
    u: UnitId,
    i: UnitId,
    add: bool,
) -> Result<bool, EquipFatal> {
    if w.unit_type(u) != Some(TYPE_PLAYER) {
        return Ok(false);
    }
    // Step 1.
    let Some((s, scroll)) = item_skill(w, i) else {
        return Ok(false);
    };
    // Step 2.
    let mut q = w.item_stat(i, STAT_QUANTITY);
    if q == 0 {
        if !scroll {
            return Ok(false);
        }
        q = 1;
    }
    // Step 3.
    let new = match (add, w.skill_quantity(u, s)) {
        (true, Some(cur)) => cur.wrapping_add(q),
        (true, None) => {
            w.add_unit_stat(u, STAT_NEWSKILLS, q);
            w.learn_skill(u, s);
            if w.skill_quantity(u, s).is_none() {
                return Err(EquipFatal::LearnedSkillMissing(s));
            }
            q
        }
        (false, Some(cur)) => (cur - q).max(0),
        (false, None) => return Err(EquipFatal::UnlinkSkillMissing(s)),
    };
    // Step 4.
    w.set_skill_quantity(u, s, new);
    w.send_skill_quantity(u, s, new);
    // Step 5.
    if new == 0 {
        for left in [true, false] {
            if w.mouse_skill(u, left).is_some_and(|m| m.0 == s) {
                w.select_skill(u, left, (0, -1));
            }
        }
    }
    Ok(true)
}

/// Recount on cube open / close (`0x0055FA40`, §5.5).
pub fn cube_recount<W: EquipWorld + ?Sized>(w: &mut W, u: UnitId) -> Result<(), EquipFatal> {
    let mut count = 0usize;
    for i in w.item_list(u) {
        if w.item_mode(i) != mode::STORED {
            continue;
        }
        let Some((s, _)) = item_skill(w, i) else {
            continue;
        };
        if w.skill_quantity(u, s).is_none() {
            return Err(EquipFatal::UnlinkSkillMissing(s));
        }
        w.set_skill_quantity(u, s, 0);
        w.send_skill_quantity(u, s, 0);
        if w.item_page(i) == super::page::INVENTORY {
            count += 1;
        }
    }
    for i in w.item_list(u) {
        if count == 0 {
            break;
        }
        if w.item_mode(i) == mode::STORED && w.item_page(i) == super::page::INVENTORY {
            item_skill_link(w, u, i, true)?;
            count -= 1;
        }
    }
    Ok(())
}

/// `0x0055D970` and the refresh test of §5.7 steps 2 and 4: stat link
/// (not for body locations 11 / 12), then the stat refresh when the item
/// is in mode 1, or in mode 0 and an active inventory item.
fn switch_on<W: EquipWorld + ?Sized>(w: &mut W, u: UnitId, x: UnitId, loc: Option<u8>) {
    w.set_item_flag(x, iflag::F4000, false);
    if !matches!(loc, Some(body::SWAP_RIGHT | body::SWAP_LEFT)) {
        w.stat_link(u, x);
    }
    let m = w.item_mode(x);
    if m == mode::EQUIPPED || (m == mode::STORED && w.active_inventory_item(u, x)) {
        w.stat_refresh(x, u);
    }
}

/// Inventory pass `0x0055DBC0` (§5.7).
pub fn inventory_pass<W: EquipWorld + ?Sized>(w: &mut W, u: UnitId, send: bool) {
    let Some(ty) = w.unit_type(u) else {
        return;
    };
    if ty != TYPE_PLAYER && w.unit_flags(u) & UNIT_FLAG_200 == 0 {
        return;
    }
    // Step 1 (stats 6, 8, 10 are not read again).
    let saved = [
        w.mouse_skill(u, true).unwrap_or((0, -1)),
        w.mouse_skill(u, false).unwrap_or((0, -1)),
    ];
    if !w.has_inventory(u) {
        return;
    }
    // Step 2: charms.
    for x in w.item_list(u) {
        if w.item_node(x) == node::PAGE
            && w.active_inventory_item(u, x)
            && !w.stat_linked(u, x)
            && w.usable(u, x)
        {
            switch_on(w, u, x, None);
        }
    }
    // Step 3: switch off.
    for loc in body::HEAD..=body::GLOVES {
        let Some(x) = w.body_item(u, loc) else {
            continue;
        };
        let linked = w.stat_linked(u, x);
        let f = w.item_flags(x);
        if (f & iflag::BROKEN == 0 || linked)
            && (f & iflag::F4000 == 0 || linked)
            && !w.usable(u, x)
        {
            w.set_item_flag(x, iflag::F4000, true);
            w.stat_unlink(u, x);
            if w.item_mode(x) == mode::EQUIPPED {
                w.deactivate(x, u);
            }
        }
    }
    // Step 4: switch on, until a sweep changes nothing.
    loop {
        let mut changed = false;
        for loc in body::HEAD..=body::GLOVES {
            let Some(x) = w.body_item(u, loc) else {
                continue;
            };
            let f = w.item_flags(x);
            if f & iflag::BROKEN == 0
                && (f & iflag::F4000 != 0 || !w.stat_linked(u, x))
                && w.usable(u, x)
            {
                switch_on(w, u, x, Some(loc));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Step 5: set items.
    for loc in body::HEAD..=body::GLOVES {
        let Some(x) = w.body_item(u, loc) else {
            continue;
        };
        let f = w.item_flags(x);
        if w.item_quality(x) == QUALITY_SET && f & (iflag::BROKEN | iflag::F4000) == 0 {
            w.deactivate(x, u);
            w.stat_refresh(x, u);
        }
    }
    // Step 6: restore the step-1 skills, left then right.
    for (left, s) in [(true, saved[0]), (false, saved[1])] {
        if s.0 == 0 || !w.has_skill(u, s) || w.mouse_skill(u, left) == Some(s) {
            continue;
        }
        let st = w.use_state(u, s);
        if st != USE_NO_QUANTITY && st != USE_NO_LEVEL {
            w.select_skill(u, left, s);
        }
    }
    // Steps 7–8.
    w.owner_refresh(u);
    if send {
        w.send_unit_refresh(u);
    }
}

/// The hands of §5.8 step 2 (`0x0055C470`): (W, O).
fn hands<W: EquipWorld + ?Sized>(w: &W, u: UnitId) -> (Option<UnitId>, Option<UnitId>) {
    if !w.has_inventory(u) {
        return (None, None);
    }
    let wpn = w.weapon_in_use(u);
    let a = w.body_item(u, body::RIGHT_HAND);
    let b = w.body_item(u, body::LEFT_HAND);
    let o = if wpn == a { b } else { a };
    let o = o.filter(|&o| w.item_is_type(o, TYPE_WEAP));
    (wpn, o)
}

/// "Throw-only" (§5.8 step 4).
fn throw_only<W: EquipWorld + ?Sized>(w: &W, i: Option<UnitId>) -> bool {
    i.is_some_and(|i| w.item_is_type(i, TYPE_THRO) && !w.item_is_type(i, TYPE_MELE))
}

fn unusable(state: u8) -> bool {
    state == USE_NO_QUANTITY || state == USE_NO_LEVEL
}

/// Ranged throw skill `0x0055C560` (§5.8).
fn ranged_throw<W: EquipWorld + ?Sized>(w: &mut W, u: UnitId, s: Option<SkillRef>) -> bool {
    let Some(s) = s else {
        return false;
    };
    w.throw_skill_row(s.0) && !unusable(w.use_state(u, s))
}

/// Restore `0x0055C4F0` (§5.8): the saved slot of a side comes back when
/// it still exists, differs from the current skill and is usable.
fn restore<W: EquipWorld + ?Sized>(w: &mut W, u: UnitId, cur: Option<SkillRef>, left: bool) {
    let Some(c) = cur else {
        return;
    };
    let saved = w.saved_mouse_skill(u, left);
    if !w.has_skill(u, saved) || saved == c {
        return;
    }
    if !unusable(w.use_state(u, saved)) {
        w.select_skill(u, left, saved);
    }
}

/// One side of §5.8 (steps 4–5 for the left with W, step 6 for the right
/// with O).
fn side<W: EquipWorld + ?Sized>(
    w: &mut W,
    u: UnitId,
    item: Option<UnitId>,
    left: bool,
) -> Result<(), EquipFatal> {
    let t = throw_only(w, item);
    let mut s = w.mouse_skill(u, left);
    if t && !ranged_throw(w, u, s) {
        let cur = s.unwrap_or((0, -1));
        w.set_saved_mouse_skill(u, left, cur);
        let throw = if left {
            SKILL_THROW
        } else {
            SKILL_LEFT_HAND_THROW
        };
        w.select_skill(u, left, (throw, -1));
        s = w.mouse_skill(u, left);
    }
    // Step 5: `S` passed unchecked (a missing skill is the fatal assert).
    let Some(cur) = s else {
        return Err(EquipFatal::NoMouseSkill);
    };
    if unusable(w.use_state(u, cur)) {
        restore(w, u, Some(cur), left);
    }
    Ok(())
}

/// Weapon bookkeeping `0x0055C5C0` (§5.8).
pub fn weapon_bookkeeping<W: EquipWorld + ?Sized>(w: &mut W, u: UnitId) -> Result<(), EquipFatal> {
    // Step 1.
    if w.unit_type(u) != Some(TYPE_PLAYER) {
        return Ok(());
    }
    // Steps 2–3.
    let (wpn, o) = hands(w, u);
    if wpn.is_none() {
        return Ok(());
    }
    // Steps 4–5, then 6.
    side(w, u, wpn, true)?;
    side(w, u, o, false)
}

#[cfg(test)]
mod tests;
