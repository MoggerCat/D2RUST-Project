// Spec: specs/world/vendors-2.md §7.3 (item copy `0x0055A2A0`); specs/items/bitstream.md §5 (save format with children); specs/items/inventory-moves.md §6.1 rule 4.2 (per-item reset)
//! The item copy on the real item: the source written as a save-format
//! stream with children ([`InvDesk::save_view`], `items::bitstream`), the
//! first record read back (`items::bitstream::read`) and made an item
//! unit (`Economy::item_from_record`), then the steps of §7.3 after it.
//! Callers: the vendors' buy and sell, the cube outputs, the hireling
//! take and the NPC socketing hand their source here (fillers as their
//! rule says); the server's cube copies here too; the interaction
//! wiring's NPC socketing and the economy cube's own `CubeRest` still
//! ask their rests.

use super::{InvDesk, InvError, InvRest};
use crate::items::bitstream::{self, read, BitWriter, StreamItem};
use crate::items::inventory::UnitKind;
use crate::items::moves::{
    add_iflags, clear_iflags, clear_uflags, deferred, iflag, mode, uflag, InventoryOps,
    MovePending, Owner,
};
use crate::items::{flag, stat};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

/// The copy's stream buffer (§7.3 step 2: 1,024 bytes).
pub const COPY_BUFFER: usize = 0x400;
/// Item flag set on every copied source (§7.3 step 6).
pub const COPIED: u32 = 0x800_0000;
/// Command flag 0x1, cleared on the copy (§7.3 step 8).
const CMD_REMOVE: u32 = 0x1;
/// Item timer event 3: replenish (`generation.md` §9 step 6).
const EVENT_REPLENISH: u8 = 3;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The save-format view of `u` (`items/bitstream.md` §5): the stream
    /// view at its own page with unit +0x28 (the unit record's init seed,
    /// `units.md` §2) and the items of its own inventory as children, in
    /// list order (§2 rule 5). The trailer values are not named (Open
    /// question 3): none.
    pub fn save_view(&self, u: UnitId) -> Option<StreamItem> {
        let page = self.state.items.get(&u)?.page;
        let mut v = self.stream_item(self.guid_of(u), 0, page)?;
        v.unit28 = self.econ.units.get(u)?.init_seed;
        v.children = self
            .state
            .inventories
            .get(&u)
            .map(|inv| {
                inv.items()
                    .iter()
                    .filter_map(|&c| self.save_view(c))
                    .collect()
            })
            .unwrap_or_default();
        Some(v)
    }

    /// One child record of §7.3 step 5 / `d2s.md` §8.2 rule 4: read as
    /// step 3 with no room (failure → none; the copy and the children read
    /// so far stay), mode 4, socketed into `parent`, flags 0x80000 /
    /// 0x2000, command flag 0x1 cleared. `parent` has an inventory.
    pub(crate) fn insert_filler(&mut self, parent: UnitId, rec: &read::ReadItem) -> Option<UnitId> {
        let cg = self.guid_of(parent);
        let child = match self.econ.item_from_record(rec, None) {
            Ok(c) => c,
            Err(e) => {
                self.state.errors.push(InvError::Economy(e));
                return None;
            }
        };
        self.sync_in();
        // Mode 4, then `0x00562660(child, copy, &out, 0, 1, 0, 0)`
        // with EDX = the copy (`inventory-moves.md` §7.19 rule 4).
        // The rule 2 gates (f3 = 0: no target mode test) hold for
        // a stream written from a socketed source whose fillers
        // passed them, so result 0 (fatal, line 0xDD4) does not
        // arise; the link cannot fail here.
        if let Some(d) = self.state.items.get_mut(&child) {
            d.mode = mode::CURSOR;
        }
        let fg = self.guid_of(child);
        let mut inv = self.state.inventories.remove(&parent)?;
        inv.link(self, child, None);
        // f2 = 1: the copy's inventory cursor := none.
        inv.put_cursor(self, None);
        self.state.inventories.insert(parent, inv);
        clear_uflags(self, fg, uflag::TARGETABLE);
        // Filler properties `0x0055C2C0` and owner link `0x006276C0`.
        self.filler_linked(fg, cg);
        if let Some(d) = self.state.items.get_mut(&child) {
            d.mode = mode::SOCKETED;
        }
        // f4 = 0: a match runs the runeword stats and the timers
        // without the recharge `0x0055FE80`; f1 = 0: no match
        // returns here, before the tail.
        if self.activate_runeword_on(parent) {
            add_iflags(self, cg, iflag::CHANGED);
            clear_iflags(self, cg, iflag::NOEQUIP);
            deferred::owner_refresh(self, Owner::item(cg));
            self.update_list_add(Owner::item(cg), cg);
        }
        if let Some(d) = self.state.items.get_mut(&child) {
            d.flags = (d.flags | flag::INIT) & !flag::INSTORE;
            d.cmd_flags &= !CMD_REMOVE;
        }
        self.sync_out();
        Some(child)
    }

    /// `0x0055A2A0` (§7.3): a copy of `src`, or none. `fillers`: read
    /// and socket the source's children into the copy (step 5).
    pub fn copy_of(&mut self, src: UnitId, fillers: bool) -> Option<UnitId> {
        // 1. R: the source's room (none off the ground). 1.1: a ground
        // source would put the copy in R, on the ground, and no caller
        // takes it out; every 1.14d caller passes a held source, so a
        // ground one is a caller error (no copy).
        if self
            .econ
            .game
            .lists
            .unit(src)
            .and_then(|e| e.room())
            .is_some()
        {
            self.state.errors.push(InvError::GroundCopySource(src));
            return None;
        }
        let room = None;
        // 2. The save stream with children into 1,024 bytes; one that does
        // not fit has length 0 and step 3's read fails.
        let view = self.save_view(src)?;
        let mut w = BitWriter::new(COPY_BUFFER);
        bitstream::write_save_into(&mut w, &view, &self.econ.tables.isc);
        let bytes = w.finish().unwrap_or_default();
        // 3. The first record: class from the code, the unit allocated at
        // the record's mode and decoded; flags 0x80000 / 0x2000; timers.
        let mut r = read::BitReader::new(&bytes);
        let first = read::read_save_record(&mut r, self.econ.tables).ok()?;
        let copy = match self.econ.item_from_record(&first, room) {
            Ok(c) => c,
            Err(e) => {
                self.state.errors.push(InvError::Economy(e));
                return None;
            }
        };
        self.sync_in();
        if let Some(d) = self.state.items.get_mut(&copy) {
            d.x = first.item.x;
            d.y = first.item.y;
            d.body_loc = first.item.body_loc;
        }
        // 4. Flags again.
        if let Some(it) = self.econ.items.get_mut(copy) {
            it.flags = (it.flags | flag::INIT) & !flag::INSTORE;
        }
        // 5. The children.
        let n = if first.item.compact || first.item.alt {
            0
        } else {
            first.item.filled
        };
        if fillers && n != 0 {
            // Each child record in stream order (each starts at a byte,
            // `bitstream.md` §5): read as step 3 with no room (failure →
            // none; the copy and the children read so far stay), mode 4,
            // socketed into the copy, flags 0x80000 / 0x2000, command
            // flag 0x1 cleared.
            let mut at = r.pos().div_ceil(8);
            let cg = self.guid_of(copy);
            self.state.add_inventory(copy, UnitKind::Item, cg);
            for _ in 0..n {
                let entry = read::read_save_entry(bytes.get(at..)?, self.econ.tables).ok()?;
                at += entry.len;
                self.insert_filler(copy, &entry.item)?;
            }
        }
        // 6. The source's flag.
        if let Some(it) = self.econ.items.get_mut(src) {
            it.flags |= COPIED;
        }
        // 7. Replenish: stat 252, then 253, a total ≠ 0 and no event 3 on
        // the copy (step 3 scheduled one when a total is set).
        let scheduled = self
            .econ
            .game
            .timers
            .unit_timers(copy)
            .into_iter()
            .any(|id| {
                self.econ
                    .game
                    .timers
                    .event(id)
                    .is_some_and(|(e, _, _)| e == EVENT_REPLENISH)
            });
        if !scheduled {
            let frame = self.econ.game.frame as u32;
            let v = [stat::REPLENISH_DURABILITY, stat::REPLENISH_QUANTITY]
                .into_iter()
                .map(|s| self.econ.stats.unit_total(copy, s, 0))
                .find(|&v| v != 0);
            if let Some(v) = v {
                let at = frame.wrapping_add((2500 / v + 1) as u32);
                if let Err(e) = self.econ.game.schedule_event(
                    copy,
                    u32::from(EVENT_REPLENISH),
                    at as i32,
                    None,
                    0,
                    0,
                ) {
                    self.state.errors.push(InvError::Economy(e.into()));
                }
            }
        }
        self.sync_in();
        // 8. The per-item reset (`inventory-moves.md` §6.1 rule 4.2), then
        // command flag 0x1 cleared.
        let g = self.guid_of(copy);
        deferred::item_reset(self, g);
        if let Some(d) = self.state.items.get_mut(&copy) {
            d.cmd_flags &= !CMD_REMOVE;
        }
        self.sync_out();
        Some(copy)
    }
}
