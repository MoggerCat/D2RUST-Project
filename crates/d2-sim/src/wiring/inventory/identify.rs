// Spec: specs/items/inventory-moves.md §7.18; specs/items/inventory.md §5.5
//! Identify from a scroll or tome (C→S 0x27 `UseItemAction`, the
//! `use_item` seam).
//!
//! The item-use spec (`0x005BF240`) is unwritten, so the effect is
//! PROVISIONAL (`docs/HANDOFF.md` §7, REC-113): a scroll of identify
//! (`isc`) or a tome of identify (`ibk`) used on a stored or equipped
//! item that is not identified sets the identified flag (`generation.md`
//! §1.1, item flag 0x10) and marks the item changed (item flag 0x1), so
//! the next update pass sends it again (row `UpdateStats`, 0x9D action
//! 0x15, `inventory-moves.md` §6.2) with its properties shown
//! (`bitstream.md` §4.3). The scroll's consumption and the tome's charge
//! are the 0x27 handler's (§7.18 steps 7–9).
//!
//! d2rs-own, unverified: the two codes and the "already identified is
//! not used" answer are preview fills.

use super::{InvDesk, InvRest};
use crate::items::inventory::mode;
use crate::items::moves::{deferred, iflag, Guid, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;
use crate::world::npc::{InvEntry, Place};

/// Items codes of the identify scroll and tome (d2rs-own, unverified).
pub const IDENTIFY_CODES: [[u8; 4]; 2] = [*b"isc ", *b"ibk "];

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// `use_item` for an identify scroll or tome `item` on `target`:
    /// true = used. Any other item, a target that is not an item or is
    /// already identified → false (the 0x27 handler then does nothing
    /// more, §7.18 step 5).
    pub fn use_identify(&mut self, player: Owner, target: Owner, item: Guid) -> bool {
        if !IDENTIFY_CODES.contains(&self.code(item)) || target.ty != Owner::ITEM {
            return false;
        }
        let t = target.guid;
        if self.item_unit(t).is_none() {
            return false;
        }
        self.mark_identified(player, t)
    }

    /// Item flags 0x10 and 0x1 on `item` and the owner update queued;
    /// false when it is already identified.
    fn mark_identified(&mut self, player: Owner, item: Guid) -> bool {
        let flags = self.item_flags(item);
        if flags & iflag::IDENTIFIED != 0 {
            return false;
        }
        self.set_item_flags(item, flags | iflag::IDENTIFIED | iflag::CHANGED);
        deferred::mark(self, player, item, 0);
        true
    }

    /// The player's items in link order with their places and item flags
    /// (`npc.md` §6 step 3: what Cain's identify walks).
    pub fn npc_entries(&self, player: UnitId) -> Vec<InvEntry> {
        self.state
            .items_of(player)
            .into_iter()
            .filter_map(|u| {
                let d = self.state.items.get(&u)?;
                let place = match d.mode {
                    mode::STORED => Place::Grid(d.page),
                    mode::EQUIPPED => Place::Equipped,
                    mode::BELT => Place::Belt,
                    _ => Place::Other,
                };
                Some(InvEntry {
                    item: u,
                    place,
                    flags: d.flags,
                })
            })
            .collect()
    }

    /// Cain's identify (C→S 0x34, `AppRest::identify`'s call) of the
    /// item unit `item` held by `player` (d2rs-own, unverified, REC-113's
    /// effect): flags set and the owner update queued.
    pub fn identify_unit(&mut self, player: UnitId, item: UnitId) -> bool {
        let (Some(o), Some(_)) = (self.owner_of(player), self.state.items.get(&item)) else {
            return false;
        };
        let g = self.guid_of(item);
        self.mark_identified(o, g)
    }
}
