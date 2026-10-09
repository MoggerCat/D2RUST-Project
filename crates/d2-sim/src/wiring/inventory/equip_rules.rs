// Spec: specs/items/inventory.md §5.5, §5.7, §5.8; specs/items/properties.md §9, §13
//! The equipment bookkeeping (`items::inventory::bookkeeping`) and the
//! set-item state update (`items::set_state`) on [`InvDesk`]: the
//! inventories, the item data copies, the item store, the item tables
//! and the stat lists (links of the set lists, park / unpark,
//! `sim/stat-lists.md` §8.5). The skill list, the books table's skill
//! columns, the player data mouse slots, the stat link of an item and the
//! messages have no d2-sim owner: [`InvRest`]'s equipment seams.
//!
//! The rules run in place of the [`InvRest`] calls of the same addresses
//! (`MovePending::charm_relink`, `charm_unlink`, `inventory_pass`,
//! `weapon_bookkeeping`) when [`InvState::equip_rules`] is on.

use super::{InvDesk, InvError, InvRest};
use crate::items::inventory::bookkeeping::{self as bk, EquipWorld, SkillRef};
use crate::items::inventory::checks::{active_inventory_item, usable};
use crate::items::inventory::{body, iflag, node, InvWorld};
use crate::items::set_state::{self, SetItemRow, SetWorld, QUALITY_SET, STAT_SET_ID};
use crate::items::ty;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};

/// An equipment-rule call asked by an inventory function while the
/// owner's inventory was lent out (`InvState::equip_queue`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipCall {
    /// §5.5 item-skill link (owner, item).
    SkillLink(UnitId, UnitId),
    /// §5.7 inventory pass with send 0.
    Pass(UnitId),
    /// §5.8 weapon bookkeeping.
    Weapons(UnitId),
    /// The set-item update after an item was linked (owner, item;
    /// [`item_link`](super::item_link)).
    SetLink(UnitId, UnitId),
    /// The set-item update after an item left the body.
    SetUnlink(UnitId, UnitId),
    /// The charm links of the inventory pass (§5.7 step 2).
    Charms(UnitId),
}

/// Unit type 4 (item), the owner type of a new set list (§13 step 6).
const OWNER_TYPE_ITEM: u32 = 4;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// Logs a fatal assert of the rules.
    fn note_equip<T>(&mut self, r: Result<T, bk::EquipFatal>) {
        if let Err(e) = r {
            self.state.errors.push(InvError::Equip(e));
        }
    }

    /// Queues a call for the end of the inventory call (the owner's
    /// inventory is lent to the inventory function now).
    pub(super) fn queue_equip(&mut self, c: EquipCall) {
        self.state.equip_queue.push(c);
    }

    /// Runs the queued calls, in order. d2rs order: 1.14d runs each one
    /// inline at its call site (`inventory.md` §2.4 steps 6 and 9, the
    /// equip of §4.6); the later steps of those functions read no skill
    /// quantity, mouse skill, stat link or 0x4000 flag they change.
    pub(super) fn run_equip_queue(&mut self) {
        for c in std::mem::take(&mut self.state.equip_queue) {
            match c {
                EquipCall::SkillLink(o, i) => self.run_item_skill_link(o, i, true),
                EquipCall::Pass(o) => self.run_inventory_pass(o, false),
                EquipCall::Weapons(o) => self.run_weapon_bookkeeping(o),
                EquipCall::SetLink(o, i) => self.run_set_link(o, i),
                EquipCall::SetUnlink(o, i) => self.run_set_unlink(o, i),
                EquipCall::Charms(o) => self.run_link_charms(o),
            }
        }
    }

    /// §5.5 item-skill link (`add`) on the rules.
    pub fn run_item_skill_link(&mut self, owner: UnitId, item: UnitId, add: bool) {
        let r = bk::item_skill_link(self, owner, item, add);
        self.note_equip(r);
    }

    /// §5.5 cube recount `0x0055FA40` on the rules.
    pub fn run_cube_recount(&mut self, owner: UnitId) {
        let r = bk::cube_recount(self, owner);
        self.note_equip(r);
    }

    /// §5.7 inventory pass `0x0055DBC0(send)` on the rules.
    pub fn run_inventory_pass(&mut self, owner: UnitId, send: bool) {
        bk::inventory_pass(self, owner, send);
        self.sync_out();
    }

    /// §5.8 weapon bookkeeping `0x0055C5C0` on the rules.
    pub fn run_weapon_bookkeeping(&mut self, owner: UnitId) {
        let r = bk::weapon_bookkeeping(self, owner);
        self.note_equip(r);
    }

    /// `0x00663CC0(O, I, r, p)` (`properties.md` §13).
    pub fn run_set_item_update(&mut self, owner: UnitId, item: UnitId, r: i32, p: i32) -> bool {
        set_state::set_item_update(self, Some(owner), Some(item), r, p)
    }

    fn inv_items(&self, u: UnitId) -> Vec<UnitId> {
        self.state
            .inventories
            .get(&u)
            .map(|i| i.items().to_vec())
            .unwrap_or_default()
    }

    fn record_type(&self, i: UnitId) -> i16 {
        self.state
            .items
            .get(&i)
            .and_then(|d| self.econ.tables.item(d.record))
            .map_or(-1, |r| r.type_)
    }

    fn set_row_of(&self, i: UnitId) -> Option<SetItemRow> {
        let it = self.econ.items.get(i)?;
        let si = usize::try_from(it.file_index)
            .ok()
            .and_then(|f| self.econ.tables.setitems.get(f))?;
        Some(SetItemRow {
            add_func: si.add_func,
            slot: si.slot as i16,
            set: si.set,
        })
    }

    fn owner_set_list(&self, o: UnitId, state: u32) -> Option<crate::stats::lists::ListId> {
        let r = self.econ.stats.unit_list(o)?;
        self.econ.stats.list_of_state(r, state)
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> EquipWorld for InvDesk<'_, '_, H, R> {
    fn unit_type(&self, u: UnitId) -> Option<u8> {
        self.econ.units.get(u).map(|r| r.ty as u8)
    }
    fn unit_flags(&self, u: UnitId) -> u32 {
        self.econ.units.get(u).map_or(0, |r| r.flags)
    }
    fn has_inventory(&self, u: UnitId) -> bool {
        self.state.inventories.contains_key(&u)
    }
    fn item_list(&self, u: UnitId) -> Vec<UnitId> {
        self.inv_items(u)
    }
    fn body_item(&self, u: UnitId, loc: u8) -> Option<UnitId> {
        self.state.inventories.get(&u)?.body_item(loc)
    }
    fn weapon_in_use(&self, u: UnitId) -> Option<UnitId> {
        // +0x1C, written by the body link / unlink (`world/quests-act3-2.md`
        // §11.5 rules 1–2, `items::inventory::weapon`).
        let inv = self.state.inventories.get(&u)?;
        self.item_unit(inv.weapon_guid)
    }
    fn add_unit_stat(&mut self, u: UnitId, stat: u16, d: i32) {
        self.econ
            .stats
            .unit_add(&mut *self.econ.hooks, u, stat, d, 0);
    }
    fn item_type(&self, i: UnitId) -> i16 {
        self.record_type(i)
    }
    fn item_is_type(&self, i: UnitId, t: i16) -> bool {
        self.state
            .items
            .get(&i)
            .is_some_and(|d| self.tables.is_type(d.record, t))
    }
    fn item_flags(&self, i: UnitId) -> u32 {
        self.state.items.get(&i).map_or(0, |d| d.flags)
    }
    fn set_item_flag(&mut self, i: UnitId, bits: u32, on: bool) {
        if let Some(d) = self.state.items.get_mut(&i) {
            if on {
                d.flags |= bits;
            } else {
                d.flags &= !bits;
            }
        }
    }
    fn item_mode(&self, i: UnitId) -> u8 {
        self.state.items.get(&i).map_or(0xFF, |d| d.mode)
    }
    fn item_node(&self, i: UnitId) -> u8 {
        self.state.items.get(&i).map_or(node::NONE, |d| d.node_kind)
    }
    fn item_page(&self, i: UnitId) -> u8 {
        self.state.items.get(&i).map_or(0xFF, |d| d.page)
    }
    fn item_quality(&self, i: UnitId) -> u8 {
        InvWorld::quality(self, i)
    }
    fn item_stat(&self, i: UnitId, stat: u16) -> i32 {
        InvWorld::item_stat(self, i, stat)
    }
    /// The books row of the spell index (item data +0x3E,
    /// [`InvDesk::spell_of`]).
    fn book_skill(&self, i: UnitId, scroll: bool) -> Option<i32> {
        let spell = self.spell_of(i);
        self.rest.book_skill(spell, scroll)
    }
    fn active_inventory_item(&self, u: UnitId, i: UnitId) -> bool {
        active_inventory_item(self, self.tables, i, u)
    }
    fn usable(&self, u: UnitId, i: UnitId) -> bool {
        self.state
            .inventories
            .get(&u)
            .is_some_and(|inv| usable(inv, self, self.tables, i))
    }
    fn stat_linked(&self, u: UnitId, i: UnitId) -> bool {
        InvWorld::item_active_on(self, i, u)
    }
    /// `0x0055D970`: the weapon-in-use link `0x0063D1D0`, then the stat
    /// link.
    fn stat_link(&mut self, u: UnitId, i: UnitId) {
        self.weapon_link_on(u, i, true);
        InvWorld::stat_link(self, u, i)
    }
    /// The body unlink `0x0063D2B0` (+0x1C, §11.5 rule 2), then the stat
    /// unlink.
    fn stat_unlink(&mut self, u: UnitId, i: UnitId) {
        self.weapon_link_on(u, i, false);
        if self.unlink_item_stats(u, i) {
            return;
        }
        let (o, g) = (self.owner_or_none(u), self.guid_of(i));
        self.rest.stat_unlink(o, g)
    }
    /// `0x0055C730(X, U, 1, 1)`: the set update with r = 1, p = 0
    /// (`properties.md` §13 callers), then the rest's part.
    fn deactivate(&mut self, i: UnitId, u: UnitId) {
        if InvWorld::quality(self, i) == QUALITY_SET {
            set_state::set_item_update(self, Some(u), Some(i), 1, 0);
        }
        let o = self.owner_or_none(u);
        self.rest.stat_refresh_unlink(o, 1);
    }
    /// `0x0055C2C0(X, U, 1)`: the set branch (`properties.md` §9 rule 3)
    /// on the rules; gems and runes (an item owner only) stay the rest's.
    fn stat_refresh(&mut self, i: UnitId, u: UnitId) {
        let gem_or_rune =
            self.item_is_type(i, ty::GEM as i16) || self.item_is_type(i, ty::RUNE as i16);
        if !set_state::stat_refresh_set(self, u, i, gem_or_rune) {
            let o = self.owner_or_none(u);
            self.rest.stat_refresh(o);
        }
    }
    fn owner_refresh(&mut self, u: UnitId) {
        InvWorld::owner_refresh(self, u)
    }
    fn send_unit_refresh(&mut self, u: UnitId) {
        let o = self.owner_or_none(u);
        self.rest.send_unit_refresh(o)
    }
    fn skill_quantity(&self, u: UnitId, skill: i32) -> Option<i32> {
        self.rest.skill_quantity(self.owner_or_none(u), skill)
    }
    fn set_skill_quantity(&mut self, u: UnitId, skill: i32, q: i32) {
        let o = self.owner_or_none(u);
        self.rest.set_skill_quantity(o, skill, q)
    }
    fn learn_skill(&mut self, u: UnitId, skill: i32) {
        let o = self.owner_or_none(u);
        self.rest.learn_skill(o, skill)
    }
    /// S→C 0x22 (`0x0053C520`, `client/msg-skills.md` §5 r1) to U's
    /// client through the rest's transport: U's type and GUID, the skill,
    /// the quantity's low byte, flag 1 when U has state 7 at send. A unit
    /// without a record sends nothing.
    fn send_skill_quantity(&mut self, u: UnitId, skill: i32, q: i32) {
        let Some(o) = self.owner_of(u) else {
            return;
        };
        let state7 = self.econ.stats.has_state(u, 7);
        let msg =
            crate::units::messages::update_item_skill(o.ty, o.guid, skill as u16, q as u8, state7);
        self.rest.send(o, msg.to_vec())
    }
    fn mouse_skill(&self, u: UnitId, left: bool) -> Option<SkillRef> {
        self.rest.mouse_skill(self.owner_or_none(u), left)
    }
    fn select_skill(&mut self, u: UnitId, left: bool, s: SkillRef) {
        let o = self.owner_or_none(u);
        self.rest.select_skill(o, left, s)
    }
    fn has_skill(&self, u: UnitId, s: SkillRef) -> bool {
        self.rest.has_skill_owned(self.owner_or_none(u), s)
    }
    fn use_state(&mut self, u: UnitId, s: SkillRef) -> u8 {
        let o = self.owner_or_none(u);
        self.rest.skill_use_state(o, s)
    }
    fn throw_skill_row(&self, skill: i32) -> bool {
        self.rest.throw_skill_row(skill)
    }
    fn saved_mouse_skill(&self, u: UnitId, left: bool) -> SkillRef {
        self.rest.saved_mouse_skill(self.owner_or_none(u), left)
    }
    fn set_saved_mouse_skill(&mut self, u: UnitId, left: bool, s: SkillRef) {
        let o = self.owner_or_none(u);
        self.rest.set_saved_mouse_skill(o, left, s)
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> SetWorld for InvDesk<'_, '_, H, R> {
    fn is_item(&self, u: UnitId) -> bool {
        self.econ
            .units
            .get(u)
            .is_some_and(|r| r.ty == UnitType::Item)
    }
    fn has_inventory(&self, u: UnitId) -> bool {
        self.state.inventories.contains_key(&u)
    }
    fn in_body_of(&self, o: UnitId, i: UnitId) -> bool {
        self.state
            .items
            .get(&i)
            .is_some_and(|d| d.inv == Some(o) && d.node_kind == node::BODY)
    }
    /// `0x0062A370(O, I, 1)` (`properties.md` §11): the set slots of O's
    /// equipped (node 3) set items of I's set, I included, without the
    /// no-equip (0x4000) or broken (0x100) ones.
    fn set_mask(&self, o: UnitId, i: UnitId) -> u32 {
        let Some(set) = self.set_row_of(i).map(|r| r.set) else {
            return 0;
        };
        let mut mask = 0u32;
        let mut add = |x: UnitId| {
            if let Some(r) = self.set_row_of(x).filter(|r| r.set == set) {
                if let Some(bit) = 1u32.checked_shl(r.slot as u32) {
                    mask |= bit;
                }
            }
        };
        add(i);
        for x in self.inv_items(o) {
            if x == i {
                continue;
            }
            let Some(d) = self.state.items.get(&x) else {
                continue;
            };
            if d.node_kind == node::BODY
                && d.flags & (iflag::F4000 | iflag::BROKEN) == 0
                && InvWorld::quality(self, x) == QUALITY_SET
                && (body::HEAD..=body::GLOVES).contains(&d.body_loc)
            {
                add(x);
            }
        }
        mask
    }
    fn quality(&self, i: UnitId) -> u8 {
        InvWorld::quality(self, i)
    }
    fn setitem_row(&self, i: UnitId) -> Option<SetItemRow> {
        self.set_row_of(i)
    }
    fn sets_count(&self) -> usize {
        self.econ.tables.sets.len()
    }
    fn park(&mut self, i: UnitId, state: u32, park: bool) {
        self.econ.stats.park(&mut *self.econ.hooks, i, state, park);
    }
    fn owner_list_tag(&self, o: UnitId, state: u32) -> Option<i32> {
        let l = self.owner_set_list(o, state)?;
        Some(self.econ.stats.base(l, STAT_SET_ID, 0))
    }
    fn reset_owner_list(&mut self, o: UnitId, state: u32, set: i32) {
        let Some(l) = self.owner_set_list(o, state) else {
            return;
        };
        let st = &mut *self.econ.stats;
        st.remove_all(&mut *self.econ.hooks, l);
        st.set(&mut *self.econ.hooks, l, STAT_SET_ID, set, 0, None);
    }
    fn free_owner_list(&mut self, o: UnitId, state: u32) {
        let Some(l) = self.owner_set_list(o, state) else {
            return;
        };
        let st = &mut *self.econ.stats;
        st.unit_detach(&mut *self.econ.hooks, l);
        st.free_plain(&mut *self.econ.hooks, l);
    }
    fn new_owner_list(&mut self, o: UnitId, i: UnitId, state: u32) {
        let guid = self.econ.units.get(i).map_or(0, |r| r.guid);
        let st = &mut *self.econ.stats;
        let l = st.alloc(0, 0, OWNER_TYPE_ITEM, guid);
        st.attach(&mut *self.econ.hooks, o, l, true);
        st.set_state(l, state);
    }
    fn tag_owner_list(&mut self, o: UnitId, state: u32, set: i32) {
        if let Some(l) = self.owner_set_list(o, state) {
            self.econ
                .stats
                .set(&mut *self.econ.hooks, l, STAT_SET_ID, set, 0, None);
        }
    }
    /// `0x00660120` (`properties.md` §11): the rest's.
    fn set_bonuses(&mut self, o: UnitId, i: UnitId, state: u32) {
        if self.state.link_item_stats {
            return self.apply_set_bonuses(o, i, state);
        }
        let (ow, g) = (self.owner_or_none(o), self.guid_of(i));
        self.rest.set_bonuses(ow, g, state)
    }
}
