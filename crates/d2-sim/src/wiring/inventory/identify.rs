// Spec: specs/items/inventory-moves.md §7.18; specs/items/inventory.md §5.5
//! Identify from a scroll or tome (C→S 0x27 `UseItemAction`, the
//! `use_item` seam).
//!
//! The item-use spec (`0x005BF240`) is unwritten, so the effect is
//! PROVISIONAL (`docs/HANDOFF.md` §7, REC-110): a scroll of identify
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
use crate::items::moves::{deferred, iflag, Guid, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;

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
        let flags = self.item_flags(t);
        if flags & iflag::IDENTIFIED != 0 {
            return false;
        }
        self.set_item_flags(t, flags | iflag::IDENTIFIED | iflag::CHANGED);
        deferred::mark(self, player, t, 0);
        true
    }
}
