// Spec: specs/formats/d2s.md §8.2 rules 2–4, 7 (item list reading); specs/items/bitstream-legacy.md §3 rule 12
//! A save item entry made an item unit of an owner's inventory:
//! [`InvDesk::load_entry`]. The record is decoded into a unit
//! (`Economy::item_from_record`, `0x00558CB0`), its socketed children are
//! inserted ([`InvDesk::insert_filler`], rule 4), then the item is placed
//! by the mode and position it was saved with.
//!
//! PROVISIONAL (REC-115): the placement routine `0x00531210` has no spec
//! (`d2s.md` §8.2 rule 3 is blocked). d2rs-own, unverified: the unit is
//! set to cursor mode and goes through the same inventory calls the start
//! items use (`equip_from_cursor` at its body location; the belt at slot x;
//! the page its record names at (x, y)); a stored item that does not fit
//! at its saved cell takes a free position; an item that cannot be
//! placed at all is freed (rule 3).

use super::{InvDesk, InvError, InvRest};
use crate::items::bitstream::hflag;
use crate::items::bitstream::read::ReadEntry;
use crate::items::inventory::page;
use crate::items::inventory::UnitKind;
use crate::items::moves::{mode, InventoryOps, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

/// Why a loaded item was not placed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadFault {
    /// The unit could not be made from the record.
    NotCreated,
    /// The record was read but failed (`items/bitstream-legacy.md` §3
    /// rule 12): no item, the entry is skipped (rule 2); its children are
    /// read and freed (rule 4).
    RecordFailed,
    /// The owner has no inventory in the model.
    NoInventory,
    /// Neither the saved place nor a free position took it; the unit is freed.
    NoRoom,
    /// Rule 5: the item carries the runeword flag but matches no runeword
    /// row any more; stored and cursor items are freed.
    StaleRuneword,
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// Makes the item of `entry` (and its socketed children) and places it
    /// in `owner`'s inventory. Placements queue the owner's item messages
    /// (take them with the host's sent queue).
    pub fn load_entry(&mut self, owner: UnitId, entry: &ReadEntry) -> Result<UnitId, LoadFault> {
        self.load_entry_in(owner, entry, false)
    }

    /// [`Self::load_entry`] for a corpse's list (`d2s.md` §8.2 rule 3: the
    /// corpse placement `0x00531390`, `items/bitstream-legacy.md` rule 2):
    /// a worn item is put straight at its saved body location (mode 1,
    /// page none; no equip from the cursor, no requirement test); a
    /// stored one at its saved cell, sending nothing; no free-position
    /// fallback (a failure frees the unit).
    pub fn load_corpse_entry(
        &mut self,
        corpse: UnitId,
        entry: &ReadEntry,
    ) -> Result<UnitId, LoadFault> {
        self.load_entry_in(corpse, entry, true)
    }

    fn load_entry_in(
        &mut self,
        owner: UnitId,
        entry: &ReadEntry,
        corpse: bool,
    ) -> Result<UnitId, LoadFault> {
        if !self.state.inventories.contains_key(&owner) {
            return Err(LoadFault::NoInventory);
        }
        let rec = &entry.item;
        if rec.failed {
            return Err(LoadFault::RecordFailed);
        }
        let unit = match self.econ.item_from_record(rec, None) {
            Ok(u) => u,
            Err(e) => {
                self.state.errors.push(InvError::Economy(e));
                return Err(LoadFault::NotCreated);
            }
        };
        self.sync_in();
        let it = &rec.item;
        if !entry.children.is_empty() {
            let g = self.guid_of(unit);
            self.state.add_inventory(unit, UnitKind::Item, g);
            for child in &entry.children {
                let _ = self.insert_filler(unit, &child.item);
            }
        }
        // Rule 5 (`0x00563470`, `d2s-load.md` §6): a runeword flag the
        // sockets no longer back. Stored / cursor: the unit is freed. An
        // equipped one is taken off and placed like a stored item (where
        // the original leaves it is untraced, Open question 13;
        // PROVISIONAL, REC-241).
        let stale = self
            .econ
            .items
            .get(unit)
            .is_some_and(|i| i.flags & hflag::RUNEWORD != 0)
            && !self.runeword_matches(unit);
        let mut saved_mode = it.mode as u8;
        if stale {
            if saved_mode != mode::EQUIPPED {
                self.free(unit);
                return Err(LoadFault::StaleRuneword);
            }
            saved_mode = mode::STORED;
        }
        let page = if it.page == 0xFF { 0 } else { it.page };
        if let Some(d) = self.state.items.get_mut(&unit) {
            d.x = it.x;
            d.y = it.y;
            d.body_loc = it.body_loc;
            d.page = page;
            d.mode = mode::CURSOR;
        }
        self.sync_out();
        let (o, g) = (
            self.owner_of(owner).unwrap_or(Owner::player(0)),
            self.guid_of(unit),
        );
        if corpse {
            let placed = match saved_mode {
                mode::EQUIPPED if self.place_body(o, g, it.body_loc) => {
                    self.set_body_loc(g, it.body_loc);
                    self.set_mode(g, mode::EQUIPPED);
                    self.set_page(g, page::NONE);
                    true
                }
                mode::EQUIPPED => false,
                _ => self.place(owner, unit, (it.x, it.y), false, false),
            };
            if placed {
                return Ok(unit);
            }
            self.free(unit);
            return Err(LoadFault::NoRoom);
        }
        let at_saved_place = match saved_mode {
            mode::EQUIPPED => self.equip_from_cursor(o, g, it.body_loc, true).0,
            mode::BELT => self.belt_place(o, g, it.x as u32),
            _ if stale => false,
            _ => self.place(owner, unit, (it.x, it.y), false, true),
        };
        if at_saved_place || self.place(owner, unit, (0, 0), true, true) {
            return Ok(unit);
        }
        self.free(unit);
        Err(LoadFault::NoRoom)
    }
}
