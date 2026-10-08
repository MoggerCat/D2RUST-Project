// Spec: specs/items/properties.md §9, §11, §13; specs/sim/stat-lists.md §8.2, §8.4
//! An equipped item's stats reach its wearer: the stat link `0x0063D1D0`
//! attaches the item's stat list to the owner (`stat-lists.md` §8.4,
//! reset 1), the leaving of a body slot detaches it (§8.2), and a set
//! item runs the set-item update (`properties.md` §13) and the set
//! bonuses (§11) on the owner.
//!
//! PROVISIONAL (M22; REC-161): the bodies of the stat link `0x0063D1D0`
//! and unlink `0x0063D2B0` are not written (`inventory.md` OQ6); the
//! item's whole unit list is attached and detached, and the remaining
//! set items are re-evaluated after one leaves (the set mask without
//! the leaving item). Settled by a trace of equipping a set piece.
//! Runs only when [`InvState::link_item_stats`] is on.
//!
//! [`InvState::link_item_stats`]: super::InvState::link_item_stats

use super::equip_rules::EquipCall;
use super::{InvDesk, InvError, InvRest};
use crate::items::inventory::{active_inventory_item, body, iflag, node, InvWorld};
use crate::items::set_state::{self, QUALITY_SET};
use crate::items::{props, ListKey};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The stat link of `item` onto `owner`; false when the switch is off.
    pub(super) fn link_item_stats(&mut self, owner: UnitId, item: UnitId) -> bool {
        if !self.state.link_item_stats {
            return false;
        }
        if let Some(l) = self.econ.stats.unit_list(item) {
            self.econ
                .stats
                .equip(&mut *self.econ.hooks, owner, Some(l), false, true);
        }
        if InvWorld::quality(self, item) == QUALITY_SET {
            self.set_update(EquipCall::SetLink(owner, item));
        }
        true
    }

    /// The unlink of `item` from `owner` (still on the body); false when
    /// the switch is off.
    pub(super) fn unlink_item_stats(&mut self, owner: UnitId, item: UnitId) -> bool {
        if !self.state.link_item_stats {
            return false;
        }
        if let Some(l) = self.econ.stats.unit_list(item) {
            if self.econ.stats.attached_unit(l) == Some(owner) {
                self.econ.stats.detach(&mut *self.econ.hooks, l);
            }
        }
        if InvWorld::quality(self, item) == QUALITY_SET {
            self.set_update(EquipCall::SetUnlink(owner, item));
        }
        true
    }

    /// Inventory pass step 2 (`inventory.md` §5.7): each active
    /// inventory item (a charm on page 0, §5.6) whose list is not linked
    /// to `owner` is linked. PROVISIONAL (REC-163): the rest of the pass
    /// is the rest's (or the equipment rules'); the unlink of a charm that
    /// leaves page 0 is `charm_unlink` (the bodies of `0x0063D1D0` /
    /// `0x0063D2B0` are unwritten, `inventory.md` OQ6).
    pub(super) fn link_charms(&mut self, owner: UnitId) {
        if !self.state.link_item_stats {
            return;
        }
        // The owner's inventory is lent out during an inventory call:
        // the sweep then runs at its end.
        if self.state.inventories.contains_key(&owner) {
            self.run_link_charms(owner);
        } else {
            self.queue_equip(EquipCall::Charms(owner));
        }
    }

    /// The sweep of [`Self::link_charms`].
    pub(super) fn run_link_charms(&mut self, owner: UnitId) {
        let items = self
            .state
            .inventories
            .get(&owner)
            .map(|inv| inv.items().to_vec())
            .unwrap_or_default();
        for i in items {
            let linked = self
                .econ
                .stats
                .unit_list(i)
                .is_some_and(|l| self.econ.stats.attached_unit(l) == Some(owner));
            if !linked && active_inventory_item(self, self.tables, i, owner) {
                self.link_item_stats(owner, i);
            }
        }
    }

    /// The set update needs the owner's inventory (§13 step 1), which is
    /// lent out during an inventory call: queued then, run at its end.
    fn set_update(&mut self, c: EquipCall) {
        let (EquipCall::SetLink(o, _) | EquipCall::SetUnlink(o, _)) = c else {
            return;
        };
        if self.state.inventories.contains_key(&o) {
            match c {
                EquipCall::SetLink(o, i) => self.run_set_link(o, i),
                EquipCall::SetUnlink(o, i) => self.run_set_unlink(o, i),
                _ => {}
            }
        } else {
            self.queue_equip(c);
        }
    }

    /// `0x00663CC0(owner, item, 0, 0)` after the link.
    pub(super) fn run_set_link(&mut self, owner: UnitId, item: UnitId) {
        set_state::set_item_update(self, Some(owner), Some(item), 0, 0);
    }

    /// After a set item left the body: its owner list is freed
    /// (`r` = 1), then the pieces still worn count again (the mask no
    /// longer holds the leaving one).
    pub(super) fn run_set_unlink(&mut self, owner: UnitId, item: UnitId) {
        set_state::set_item_update(self, Some(owner), Some(item), 1, 0);
        let others: Vec<UnitId> = self
            .state
            .inventories
            .get(&owner)
            .map(|inv| inv.items().to_vec())
            .unwrap_or_default()
            .into_iter()
            .filter(|&x| x != item)
            .filter(|x| {
                self.state.items.get(x).is_some_and(|d| {
                    d.node_kind == node::BODY && (body::HEAD..=body::GLOVES).contains(&d.body_loc)
                })
            })
            .filter(|&x| InvWorld::quality(self, x) == QUALITY_SET)
            .collect();
        // The leaving piece's data copy can still read as worn when the
        // queue runs: flag it no-equip (0x4000) for the set mask, then
        // restore it.
        let saved = self.state.items.get(&item).map(|d| d.flags);
        if let Some(d) = self.state.items.get_mut(&item) {
            d.flags |= iflag::F4000;
        }
        for x in others {
            set_state::set_item_update(self, Some(owner), Some(x), 0, 1);
        }
        if let (Some(f), Some(d)) = (saved, self.state.items.get_mut(&item)) {
            d.flags = f;
        }
    }

    /// The set bonuses `0x00660120` of `item` into the owner's list of
    /// `state` (`properties.md` §11), with the owner's set mask.
    pub(super) fn apply_set_bonuses(&mut self, owner: UnitId, item: UnitId, state: u32) {
        let mask = set_state::SetWorld::set_mask(self, owner, item);
        let key = ListKey {
            state: state as u16,
            flags: 0,
        };
        let r = self.econ.with_item(item, |s| {
            let mut who = s.unit_stats(owner);
            props::set_bonuses(s.tables, s.item, mask, &mut who, key);
        });
        if let Err(e) = r {
            self.state.errors.push(InvError::Economy(e));
        }
        self.sync_in();
    }
}
