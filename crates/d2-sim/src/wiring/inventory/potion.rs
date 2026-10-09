// Spec: specs/items/inventory-moves.md §7.11, §7.17; specs/sim/stat-lists.md §9.2, §10.1
//! Potion use from the belt (C→S 0x26) and the grid (C→S 0x20): the
//! `use_item` / `remove_used` / `consume_item` seams.
//!
//! The item-use spec (`0x005BF240`, `pSpell` table) is unwritten, so the
//! effect is PROVISIONAL (`docs/HANDOFF.md` §7, REC-102): a
//! healing potion attaches a `healthpot` (state 100) list with stat 74
//! (life regeneration per tick) that expires after [`POTION_FRAMES`]; a
//! mana potion a `manapot` (state 106) list with stat 26 (mana recovery).
//! The regeneration tick (`stat-lists.md` §10.1) and the life / mana
//! predictions (`vitals.md` §5.2) are the existing ones. A rejuvenation
//! potion restores a share of both at once.
//!
//! d2rs-own, unverified: the codes, amounts, durations and the list
//! flags below are preview fills, not facts of the original.

use super::{InvDesk, InvRest};
use crate::items::moves::{mode, unequip_detached, Guid, MovePending, MoveUnits, Owner};
use crate::stats::lists::flag;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

/// States of the potion lists (`stat-lists.md` §10.1, `states.txt`).
pub const STATE_HEALTHPOT: u32 = 100;
pub const STATE_MANAPOT: u32 = 106;
/// The stamina state (the shrine's, `states.txt` 136).
pub const STATE_STAMINA: u32 = 136;
/// Stat 28 `staminarecoverybonus`, in percent (`units.md` §6.1 stamina).
const STAT_STAMINARECOVERYBONUS: u16 = 28;
/// Frames a stamina potion works, and its bonus (d2rs-own, unverified;
/// 1000 is the shrine's value and makes stamina regenerate while moving).
pub const STAMINA_POTION_FRAMES: i32 = 250;
const STAMINA_POTION_BONUS: i32 = 1000;
/// Stats: life regeneration per tick (74), mana recovery per tick (26),
/// life (6), mana (8).
const STAT_HPREGEN: u16 = 74;
const STAT_MANARECOVERY: u16 = 26;
const STAT_LIFE: u16 = 6;
const STAT_MANA: u16 = 8;
/// Frames a healing / mana potion works (d2rs-own, unverified).
pub const POTION_FRAMES: i32 = 100;
/// Page byte shown in the removal message (`inventory-moves.md` §6.4).
const REMOVED_FLAG: u32 = 0x20;
/// 0x9C action of the belt removal `0x00561E70` (`0x0053EED0`).
const BELT_REMOVED_ACTION: u8 = 0x0F;

/// What a potion does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Potion {
    /// Life over time, in whole points.
    Life(i32),
    /// Mana over time, in whole points.
    Mana(i32),
    /// Stamina recovery for a while (`vps`).
    Stamina,
    /// Share of life and mana at once, in percent.
    Rejuv(i32),
}

/// d2rs-own, unverified: the potion of an item code (`hp1`–`hp5`,
/// `mp1`–`mp5`, `rvs`, `rvl`).
pub fn classify(code: [u8; 4]) -> Option<Potion> {
    let n = code[2].checked_sub(b'1')? as usize;
    match (&code[..2], code[2]) {
        (b"hp", b'1'..=b'5') => Some(Potion::Life([45, 90, 150, 270, 480][n])),
        (b"mp", b'1'..=b'5') => Some(Potion::Mana([30, 60, 120, 225, 450][n])),
        (b"vp", b's') => Some(Potion::Stamina),
        (b"rv", b's') => Some(Potion::Rejuv(35)),
        (b"rv", b'l') => Some(Potion::Rejuv(70)),
        _ => None,
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// `use_item` for a potion: true = used. Not a potion → false.
    pub fn use_potion(&mut self, player: Owner, item: Guid) -> bool {
        let Some(u) = self.unit_of(player) else {
            return false;
        };
        let Some(i) = self.item_unit(item) else {
            return false;
        };
        let code = match self
            .tables
            .item(self.state.items.get(&i).map(|d| d.record).unwrap_or(0))
        {
            Some(r) => r.code,
            None => return false,
        };
        let Some(p) = classify(code) else {
            return false;
        };
        let frame = self.econ.game.frame;
        match p {
            Potion::Life(n) => self.attach_potion(u, STATE_HEALTHPOT, STAT_HPREGEN, n, frame),
            Potion::Mana(n) => self.attach_potion(u, STATE_MANAPOT, STAT_MANARECOVERY, n, frame),
            Potion::Stamina => {
                let s = &mut *self.econ.stats;
                let h = &mut *self.econ.hooks;
                let Some((ty, guid)) = self
                    .econ
                    .units
                    .get(u)
                    .map(|r| (r.ty.index() as u32, r.guid))
                else {
                    return false;
                };
                s.free_state_list(h, u, STATE_STAMINA);
                let l = s.alloc(flag::NEWLENGTH, frame + STAMINA_POTION_FRAMES, ty, guid);
                s.set_state(l, STATE_STAMINA);
                s.add(h, l, STAT_STAMINARECOVERYBONUS, STAMINA_POTION_BONUS, 0);
                s.attach(h, u, l, true);
            }
            Potion::Rejuv(pct) => {
                for (stat, max) in [
                    (STAT_LIFE, self.econ.stats.max_life(u)),
                    (STAT_MANA, self.econ.stats.max_mana(u)),
                ] {
                    let cur = self.econ.stats.unit_total(u, stat, 0);
                    let v = cur.saturating_add(max / 100 * pct).min(max);
                    if v > cur {
                        let s = &mut *self.econ.stats;
                        s.unit_set(&mut *self.econ.hooks, u, stat, v, 0);
                    }
                }
            }
        }
        true
    }

    /// A new `state` list on the unit giving `points` over
    /// [`POTION_FRAMES`] as stat `stat` per tick (8.8 fixed point);
    /// replaces a running list of the same state.
    fn attach_potion(&mut self, u: UnitId, state: u32, stat: u16, points: i32, frame: i32) {
        let Some((ty, guid)) = self
            .econ
            .units
            .get(u)
            .map(|r| (r.ty.index() as u32, r.guid))
        else {
            return;
        };
        let per_tick = points.saturating_mul(256) / POTION_FRAMES;
        let s = &mut *self.econ.stats;
        let h = &mut *self.econ.hooks;
        s.free_state_list(h, u, state);
        let l = s.alloc(flag::NEWLENGTH, frame + POTION_FRAMES, ty, guid);
        s.set_state(l, state);
        s.add(h, l, stat, per_tick, 0);
        s.attach(h, u, l, true);
        // The state goes on with its changed bit and the unit is queued
        // for update (`0x00639DB0`, `stat-lists.md` §9.2), so the client
        // pass sends S→C 0xA8. Recorded 2026-10-09: 0xA8 state 100 after
        // an hp1, 106 after an mp1, with an empty stat stream
        // (`facts/items/a1-town-potions-low.tsv` n 12, n 31).
        s.toggle_state(u, state, true);
        // At full life (mana) the state ends in the same frame: the
        // changed bit stays, so the client pass sends S→C 0xA9 instead.
        // Recorded 2026-10-09: an hp1 from the belt at full life and an
        // mp1 at full mana give 0xA9 100 / 106 and no 0xA8
        // (`facts/items/a1-town-item-moves.tsv` n 41–43, n 49–51).
        // PROVISIONAL (REC-730): where 1.14d ends it is unread (entry 3,
        // `items/use.md` open question 1).
        let (vital, max) = if state == STATE_HEALTHPOT {
            (STAT_LIFE, s.max_life(u))
        } else {
            (STAT_MANA, s.max_mana(u))
        };
        if s.unit_total(u, vital, 0) >= max {
            s.free_state_list(h, u, state);
            s.toggle_state(u, state, false);
        }
        let r = self.econ.game.lists.queue_update(u);
        self.note_list(r);
    }

    /// `remove_used` of a belt item, `0x00561E70` (`inventory-moves.md`
    /// §7.17, §7.18 step 3): S→C 0x9C action 0xF with the bit-stream flag
    /// 0x20, sent now, then the item leaves the belt and is freed
    /// (`0x0055ED30`). Recorded 2026-10-09 (`facts/items/a1-town-potions-low.tsv`
    /// n 11, `a1-town-item-moves.tsv` n 42): `3F`, then `9C 0F 14 …` with
    /// item flags 0x30, in the 0x26's own frame.
    pub fn remove_belt_item(&mut self, player: Owner, item: Guid) {
        let Some(u) = self.item_unit(item) else {
            return;
        };
        let Some(p) = self.unit_of(player) else {
            return;
        };
        let _ = self.send_item_world(p, u, BELT_REMOVED_ACTION, REMOVED_FLAG);
        self.free_item(item);
    }

    /// The consumption of a stored item `0x0055E000` (`inventory-moves.md`
    /// §7.11 step 3, §7.18 step 9): the removal message (0x9D action 5,
    /// flag 0x20) with the stored page, then the item leaves the
    /// inventory and is freed.
    pub fn remove_used_item(&mut self, player: Owner, item: Guid) {
        let Some(u) = self.item_unit(item) else {
            return;
        };
        let Some(p) = self.unit_of(player) else {
            return;
        };
        let page = self.page(item);
        let _ = self.send_item_page(p, u, REMOVED_FLAG, page);
        self.free_item(item);
    }

    /// `0x005440A0` (`quests.md` §9.2) by the item's mode: 0 stored: the
    /// removal message (flag 0x20) with the stored page, unlink, freed;
    /// 1 equipped: the detached take-off `0x00560CD0(.., 1)`
    /// ([`unequip_detached`]; not freed, the next unit update sends the
    /// removal); 4 cursor: the cursor consume `0x0055EEA0`; any other
    /// mode (belt, ground): nothing.
    pub fn delete_held_item(&mut self, player: UnitId, item: UnitId) {
        let g = self.guid_of(item);
        match self.mode(g) {
            mode::STORED => {
                let page = self.page(g);
                let _ = self.send_item_page(player, item, REMOVED_FLAG, page);
                self.free_item(g);
            }
            mode::EQUIPPED => {
                if let Some(o) = self.owner_of(player) {
                    let loc = self.body_loc(g);
                    let _ = unequip_detached(self, o, loc);
                }
            }
            mode::CURSOR => {
                self.take_cursor(player, item);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamina_potion_code_is_classified() {
        assert_eq!(classify(*b"vps "), Some(Potion::Stamina));
        assert_eq!(classify(*b"yps "), None);
        assert_eq!(classify(*b"hp3 "), Some(Potion::Life(150)));
    }
}
