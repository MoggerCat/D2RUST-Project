// Spec: specs/items/use.md §3.1; specs/items/inventory-moves.md §7.11, §7.17; specs/sim/stat-lists.md §9.2, §10.1
//! Potion use from the belt (C→S 0x26) and the grid (C→S 0x20): the
//! `use_item` / `remove_used` / `consume_item` seams.
//!
//! Healing and mana potions (`hp1`–`hp5`, `mp1`–`mp5`, use entry 3) run
//! the entry 3 body `0x005BE3F0` (`items/use.md` §3.1) from the item's
//! `misc.txt` use fields: a `healthpot` (100) / `manapot` (106) list
//! whose stat 74 / 26 is the amount over `len` frames (extended by what
//! is left of a running one), event 12 at its expiry, and the default
//! remove callback `0x0056E900` (state off). The regeneration tick
//! (`stat-lists.md` §10.1) frees the list when the bar fills.
//!
//! The stamina (`vps`, entry 9) and rejuvenation (`rvs`, `rvl`, entry 5)
//! potions keep their earlier answers: their entry bodies are unwritten
//! (`items/use.md` open question 1); d2rs-own, unverified (stamina:
//! REC-135).

use super::{InvDesk, InvError, InvRest};
use crate::items::moves::{mode, unequip_detached, Guid, MovePending, MoveUnits, Owner};
use crate::rng::Seed;
use crate::skills::calc::{self, CalcContext};
use crate::stats::lists::{flag, RemoveCallback, StatLists};
use crate::tick::events::event;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};

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
/// life (6), mana (8), energy (1), vitality (3).
const STAT_HPREGEN: u16 = 74;
const STAT_MANARECOVERY: u16 = 26;
const STAT_LIFE: u16 = 6;
const STAT_MANA: u16 = 8;
const STAT_ENERGY: u16 = 1;
const STAT_VITALITY: u16 = 3;
/// Flags of the potion list (`items/use.md` §3.1 step 2: "flags 2").
const POTION_LIST_FLAGS: u32 = flag::NEWLENGTH;
/// The default remove callback `0x0056E900`: the state off.
pub const POTION_REMOVE_CALLBACK: RemoveCallback = RemoveCallback(0x0056_E900);
/// Page byte shown in the removal message (`inventory-moves.md` §6.4).
const REMOVED_FLAG: u32 = 0x20;
/// 0x9C action of the belt removal `0x00561E70` (`0x0053EED0`).
const BELT_REMOVED_ACTION: u8 = 0x0F;

/// What a potion does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Potion {
    /// Healing or mana potion: use entry 3 (`items/use.md` §3.1).
    Entry3,
    /// Stamina recovery for a while (`vps`).
    Stamina,
    /// Share of life and mana at once, in percent.
    Rejuv(i32),
}

/// The potion of an item code: `hp1`–`hp5`, `mp1`–`mp5` (entry 3), and
/// the d2rs-own, unverified `vps`, `rvs`, `rvl` answers.
pub fn classify(code: [u8; 4]) -> Option<Potion> {
    match (&code[..2], code[2]) {
        (b"hp" | b"mp", b'1'..=b'5') => Some(Potion::Entry3),
        (b"vp", b's') => Some(Potion::Stamina),
        (b"rv", b's') => Some(Potion::Rejuv(35)),
        (b"rv", b'l') => Some(Potion::Rejuv(70)),
        _ => None,
    }
}

/// Life factor `0x0062A5D0` (`items/use.md` §3.1): a player of class
/// 0 / 3 / 6 ×1.5 (v + (v >> 1)), class 4 ×2, other classes ×1; a
/// non-player ×2.
pub fn life_factor(player: Option<u32>, v: i32) -> i32 {
    match player {
        Some(0 | 3 | 6) => v.wrapping_add(v >> 1),
        Some(4) | None => v.wrapping_mul(2),
        Some(_) => v,
    }
}

/// Mana factor `0x0062A620` (`items/use.md` §3.1): a player of class
/// 0 / 3 / 6 ×1.5, class 1 / 2 / 5 ×2, others and non-players ×1.
pub fn mana_factor(player: Option<u32>, v: i32) -> i32 {
    match player {
        Some(0 | 3 | 6) => v.wrapping_add(v >> 1),
        Some(1 | 2 | 5) => v.wrapping_mul(2),
        _ => v,
    }
}

/// The items calc context (`data/calc-expressions.md` §3.5): the unit the
/// item acts on, parameter 0, functions `min`, `max`, `rand` (the unit's
/// seed), `stat`.
struct ItemCalc<'x> {
    stats: &'x StatLists,
    seed: &'x mut Seed,
    unit: UnitId,
    /// `stat(19, …)` was asked: the attack-rating getter `0x00622560` is
    /// not reachable from the inventory desk.
    unwritten: bool,
}

impl CalcContext for ItemCalc<'_> {
    fn function_count(&self) -> u8 {
        4
    }
    fn arity(&self, _index: u8) -> u8 {
        2
    }
    fn param(&mut self, _c: i32) -> i32 {
        0
    }
    fn call(&mut self, index: u8, a: &[i32]) -> i32 {
        match index {
            0 => a[0].min(a[1]),
            1 => a[0].max(a[1]),
            2 => calc::rand(self.seed, a[0], a[1]),
            _ => {
                let n = self.stats.data().stats.len();
                let Some(s) = u16::try_from(a[0]).ok().filter(|&s| usize::from(s) < n) else {
                    return 0;
                };
                match (s, a[1]) {
                    (19, _) => {
                        self.unwritten = true;
                        0
                    }
                    (_, 1) => self.stats.unit_base(self.unit, s, 0),
                    _ => self.stats.unit_total(self.unit, s, 0),
                }
            }
        }
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
            Potion::Entry3 => return self.use_entry3(u, item),
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

    /// `eval(calc)` of the items family (`0x00627C20(U, I, calc)`).
    fn item_calc(&mut self, u: UnitId, offset: u32) -> i32 {
        let Some(rec) = self.econ.units.get_mut(u) else {
            return 0;
        };
        let mut ctx = ItemCalc {
            stats: &*self.econ.stats,
            seed: &mut rec.seed,
            unit: u,
            unwritten: false,
        };
        let v = calc::eval(&self.tables.item_use.code, offset, &mut ctx);
        if ctx.unwritten {
            self.state.errors.push(InvError::Unwritten(
                "items calc stat 19 (attack rating 0x00622560)",
            ));
        }
        v
    }

    /// The entry 3 body `0x005BE3F0` (`items/use.md` §3.1) for unit `u`
    /// (player or monster) and item `item`: true = used (result 1).
    pub fn use_entry3(&mut self, u: UnitId, item: Guid) -> bool {
        let Some(rec) = self
            .item_unit(item)
            .and_then(|i| self.state.items.get(&i))
            .and_then(|d| self.tables.item(d.record))
            .copied()
        else {
            return false;
        };
        let Some((ty, guid, class)) = self.econ.units.get(u).map(|r| (r.ty, r.guid, r.class))
        else {
            return false;
        };
        let player = (ty == UnitType::Player).then_some(class);
        let frame = self.econ.game.frame;
        // Step 1.
        let len = self.item_calc(u, rec.use_len);
        let state = i32::from(rec.use_state);
        let mut list = None;
        let mut rem = 0i32;
        if len > 0 {
            let count = self.econ.stats.data().states.count();
            if state < 1 || state as usize >= count {
                return false;
            }
            list = self.econ.stats.list_by_state_flags(u, state as u32, 0);
            if let Some(l) = list {
                rem = self.econ.stats.expire(l).wrapping_sub(frame);
            }
        }
        // Step 2.
        let n_stats = self.econ.stats.data().stats.len();
        let mut used = false;
        for k in 0..3 {
            let Some(stat) = u16::try_from(rec.use_stat[k])
                .ok()
                .filter(|&s| usize::from(s) < n_stats)
            else {
                break;
            };
            let mut v = self.item_calc(u, rec.use_calc[k]);
            let draw = match stat {
                STAT_HPREGEN | STAT_LIFE => {
                    if stat == STAT_HPREGEN {
                        v = v.wrapping_shl(8);
                    }
                    v = life_factor(player, v);
                    Some(STAT_VITALITY)
                }
                STAT_MANARECOVERY | STAT_MANA => {
                    if stat == STAT_MANARECOVERY {
                        v = v.wrapping_shl(8);
                    }
                    v = mana_factor(player, v);
                    Some(STAT_ENERGY)
                }
                _ => None,
            };
            if let Some(c) = draw {
                let a = self.econ.stats.unit_total(u, c, 0);
                if a > 0 {
                    if let Some(r) = self.econ.units.get_mut(u) {
                        let r1 = r.seed.roll(a) as i32;
                        let r2 = r.seed.roll(100) as i32;
                        if r2 < r1 >> 1 {
                            v = v.wrapping_mul(2);
                        }
                    }
                }
            }
            let shift = u32::from(self.econ.stats.data().stats.valshift(stat));
            v = v.wrapping_shl(shift);
            if v <= 0 {
                continue;
            }
            // PROVISIONAL (REC-680, `items/use.md` §3.1): the Blood Golem
            // share `0x005C6870` (stat 6 / 74, U a player) is unread past
            // its pet test; v is unchanged, as with no golem. The life
            // fraction update of stat 6 (§3.1 step 3) needs the regen
            // tick's sender: no 1.14d entry 3 row (`hp*`, `mp*`) uses
            // stat 6.
            if len <= 0 {
                // `rvs` / `rvl` style: add at once, capped at `maxstat`.
                let cur = self.econ.stats.unit_total(u, stat, 0);
                let max = self.tables.item_use.maxstat.get(usize::from(stat));
                if let Some(max) = max
                    .and_then(|&m| u16::try_from(m).ok())
                    .filter(|&m| usize::from(m) < n_stats)
                {
                    let m = self.econ.stats.unit_total(u, max, 0);
                    if cur.wrapping_add(v) > m {
                        v = m.wrapping_sub(cur);
                    }
                }
                let s = &mut *self.econ.stats;
                s.unit_add(&mut *self.econ.hooks, u, stat, v, 0);
            } else {
                let e = frame.wrapping_add(rem).wrapping_add(len);
                let l = match list {
                    Some(l) => l,
                    None => {
                        let s = &mut *self.econ.stats;
                        let l = s.alloc(POTION_LIST_FLAGS, e, ty.index() as u32, guid);
                        s.set_state(l, state as u32);
                        // `0x00639DB0`: the state on with its changed bit,
                        // the unit queued for update (`stat-lists.md`
                        // §9.2), so the client pass sends S→C 0xA8.
                        s.toggle_state(u, state as u32, true);
                        let r = self.econ.game.lists.queue_update(u);
                        self.note_list(r);
                        if ty != UnitType::Player {
                            let g = &mut *self.econ.game;
                            g.timers.cancel_unit_events(u, event::STAT_REGEN, None);
                            let r = g.schedule_event(
                                u,
                                u32::from(event::STAT_REGEN),
                                frame.wrapping_add(1),
                                None,
                                0,
                                0,
                            );
                            if let Err(e) = r {
                                self.state.errors.push(InvError::Economy(e.into()));
                            }
                        }
                        let s = &mut *self.econ.stats;
                        s.set_remove_callback(l, Some(POTION_REMOVE_CALLBACK));
                        s.attach(&mut *self.econ.hooks, u, l, true);
                        list = Some(l);
                        l
                    }
                };
                let s = &mut *self.econ.stats;
                s.set_expire(l, e);
                if k == 0 {
                    let r = self.econ.game.schedule_event(
                        u,
                        u32::from(event::REMOVE_STATE),
                        e,
                        None,
                        0,
                        0,
                    );
                    if let Err(e) = r {
                        self.state.errors.push(InvError::Economy(e.into()));
                    }
                }
                let s = &mut *self.econ.stats;
                let old = s.base(l, stat, 0);
                let d = len.wrapping_add(rem);
                if d > 0 {
                    let value = old.wrapping_mul(rem).wrapping_add(v).wrapping_div(d);
                    s.set(&mut *self.econ.hooks, l, stat, value, 0, Some(u));
                }
            }
            used = true;
        }
        used
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
    fn potion_codes_are_classified() {
        assert_eq!(classify(*b"vps "), Some(Potion::Stamina));
        assert_eq!(classify(*b"yps "), None);
        assert_eq!(classify(*b"hp3 "), Some(Potion::Entry3));
        assert_eq!(classify(*b"mp5 "), Some(Potion::Entry3));
        assert_eq!(classify(*b"hp6 "), None);
    }

    // Covers: specs/items/use.md §3.1
    /// The factors of §3.1: life ×1.5 for classes 0 / 3 / 6, ×2 for 4 and
    /// non-players; mana ×1.5 for 0 / 3 / 6, ×2 for 1 / 2 / 5.
    #[test]
    fn class_factors() {
        let v = 7680;
        let life: Vec<i32> = (0..7).map(|c| life_factor(Some(c), v)).collect();
        assert_eq!(life, [11520, 7680, 7680, 11520, 15360, 7680, 11520]);
        assert_eq!(life_factor(None, v), 15360);
        let mana: Vec<i32> = (0..7).map(|c| mana_factor(Some(c), v)).collect();
        assert_eq!(mana, [11520, 15360, 15360, 11520, 7680, 15360, 11520]);
        assert_eq!(mana_factor(None, v), 7680);
    }
}
