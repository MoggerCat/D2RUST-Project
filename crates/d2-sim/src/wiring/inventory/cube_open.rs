// Spec: specs/world/cube.md §1 (open cube)
//! Opening the Horadric Cube from the inventory (C→S 0x20 on a `box `
//! item): the use function `0x005BF0C0` (`cube.md` §1).
//!
//! With the stash open (interaction type 2, object class 0x10B) the
//! interaction is cleared and S→C 0x77 0x11 is queued first. Then, if no
//! interaction is active, the interaction becomes (type 4, the cube's
//! GUID) and S→C 0x77 0x15 is queued; the cube recount `0x0055FA40`
//! (`items/inventory.md` §5.5) follows.
//!
//! The use function returns 0, so the dispatcher `0x005BF240` takes its
//! tail for a returned 0 (`items/use.md` §1 step 5): the item was armed
//! (flag 0x4) before the call, the reset `0x005BE1C0` then sends S→C 0x3F
//! for it (and any other armed item) and S→C 0x7C names the cube; the
//! 0x20 handler (`0x0055E170` step 4, code `box `) ends with 0.
//!
//! d2rs-own, unverified (REC-119): the item-use table entry is read
//! through the code `box `, not through `pSpell` 7 of `misc.txt`
//! (the item-use spec `0x005BF240` is unwritten, `cube.md` OQ 7).

use super::{InvDesk, InvRest};
use crate::items::moves::{iflag, layouts, Guid, MovePending, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitType;
use crate::world::cube::trade_action;

/// Object class of the stash (`cube.md` §1).
const STASH_CLASS: u32 = 0x10B;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The cube use function for `cube` by `player`: true = opened.
    pub fn open_cube_desk(&mut self, player: Owner, cube: Guid) -> bool {
        // Dispatcher step 5: arm the item before its use function.
        let f = self.item_flags(cube);
        self.set_item_flags(cube, f | iflag::TARGETING);
        self.open_cube_body(player, cube);
        // The function returned 0: `0x005BE1C0`, then S→C 0x7C.
        self.use_failure_reset(player);
        self.send(player, layouts::item_used(Owner::ITEM, cube));
        false
    }

    /// The use function `0x005BF0C0`.
    fn open_cube_body(&mut self, player: Owner, cube: Guid) -> bool {
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
            self.cube_recount(player);
            self.rest.send(player, trade_action(0x11).to_vec());
        }
        if let Some(r) = self.econ.units.get_mut(p) {
            if r.interact.get().is_none() {
                r.interact.set(UnitType::Item as u8, cube);
            }
        }
        self.rest.send(player, trade_action(0x15).to_vec());
        self.cube_recount(player);
        true
    }

    /// `0x0055FA40` on the rules (`items/inventory.md` §5.5); without the
    /// equipment rules, the rest's pass.
    fn cube_recount(&mut self, player: Owner) {
        match (self.state.equip_rules, self.unit_of(player)) {
            (true, Some(o)) => self.run_cube_recount(o),
            _ => self.rest.inventory_pass(player),
        }
    }
}
