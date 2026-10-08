// Spec: specs/world/cube.md §1 (open cube)
//! Opening the Horadric Cube from the inventory (C→S 0x20 on a `box `
//! item): the use function `0x005BF0C0` (`cube.md` §1).
//!
//! With the stash open (interaction type 2, object class 0x10B) the
//! interaction is cleared and S→C 0x77 0x11 is queued first. Then, if no
//! interaction is active, the interaction becomes (type 4, the cube's
//! GUID) and S→C 0x77 0x15 is queued; the inventory pass `0x0055FA40`
//! follows.
//!
//! d2rs-own, unverified (REC-116): the item-use table entry is read
//! through the code `box `, not through `pSpell` 7 of `misc.txt`
//! (the item-use spec `0x005BF240` is unwritten, `cube.md` OQ 7).

use super::{InvDesk, InvRest};
use crate::items::moves::{Guid, MovePending, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitType;
use crate::world::cube::trade_action;

/// Object class of the stash (`cube.md` §1).
const STASH_CLASS: u32 = 0x10B;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The cube use function for `cube` by `player`: true = opened.
    pub fn open_cube_desk(&mut self, player: Owner, cube: Guid) -> bool {
        let Some(p) = self.unit_of(player) else {
            return false;
        };
        let stash_open = self
            .econ
            .units
            .get(p)
            .and_then(|r| r.interact.get())
            .is_some_and(|(ty, guid)| {
                ty == UnitType::Object as u8
                    && self
                        .econ
                        .game
                        .lists
                        .find_unit(UnitType::Object, guid)
                        .and_then(|u| self.econ.units.get(u))
                        .is_some_and(|r| r.class == STASH_CLASS)
            });
        if stash_open {
            if let Some(r) = self.econ.units.get_mut(p) {
                r.interact.reset();
            }
            self.inventory_pass(player);
            self.rest.send(player, trade_action(0x11).to_vec());
        }
        if let Some(r) = self.econ.units.get_mut(p) {
            if r.interact.get().is_none() {
                r.interact.set(UnitType::Item as u8, cube);
            }
        }
        self.rest.send(player, trade_action(0x15).to_vec());
        self.inventory_pass(player);
        true
    }
}
