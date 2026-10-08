// Spec: specs/items/inventory.md (§1.2 body locations 11 / 12, §4 stat link), specs/sim/intents-events.md (§9 r14, open question 16)
//! The weapon switch (C→S 0x60): the items on the weapon-swap body
//! locations 11 / 12 trade places with the ones in the hands (4 / 5).
//!
//! PROVISIONAL (M22, REC-231): the body of `0x005616A0` is not written
//! (`intents-events.md` open question 16). This is d2rs-own, unverified:
//! each hand item leaves its slot (its stat list detaches), the swap-set
//! items take the hands (their stat lists attach; the stat link is not
//! for 11 / 12, `inventory.md` §4), every moved item joins the update
//! list so the client is told where it went, and S→C 0x97 flips the
//! client's weapon set. Requirements are not rechecked and nothing is
//! refused when both sets are empty.

use super::{InvDesk, InvRest};
use crate::items::inventory::body;
use crate::items::moves::{InventoryOps, MovePending, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;

/// S→C 0x97 WeaponSwitch (`msg-items`: one byte).
const WEAPON_SWITCH: u8 = 0x97;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// Trades the hands with the swap set. False when the owner has no
    /// inventory or a placement failed (the model is then unchanged
    /// apart from the failed slot, which stays empty).
    pub fn swap_weapon_sets(&mut self, owner: Owner) -> bool {
        if !self.has_inventory(owner) {
            return false;
        }
        let pairs = [
            (body::RIGHT_HAND, body::SWAP_RIGHT),
            (body::LEFT_HAND, body::SWAP_LEFT),
        ];
        let held: Vec<_> = pairs
            .iter()
            .map(|&(h, s)| (self.body_item(owner, h), self.body_item(owner, s)))
            .collect();
        // Out of the slots: the hand items lose their effects.
        for &(hand, swap) in &held {
            if let Some(i) = hand {
                self.body_leave_effects(owner, i);
                self.unlink(owner, i);
            }
            if let Some(i) = swap {
                self.unlink(owner, i);
            }
        }
        let mut ok = true;
        for (&(hand_loc, swap_loc), &(hand, swap)) in pairs.iter().zip(&held) {
            if let Some(i) = hand {
                ok &= self.place_body(owner, i, swap_loc);
                self.set_body_loc(i, swap_loc);
                self.update_list_add(owner, i);
            }
            if let Some(i) = swap {
                ok &= self.place_body(owner, i, hand_loc);
                self.set_body_loc(i, hand_loc);
                self.stat_link(owner, i);
                self.update_list_add(owner, i);
            }
        }
        self.weapon_bookkeeping(owner);
        self.send(owner, vec![WEAPON_SWITCH]);
        ok
    }
}
