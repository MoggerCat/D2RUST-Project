// Spec: specs/items/use.md §1–§4 (§3.1: wiring/inventory/potion.rs); specs/world/objects-2.md §27.1; specs/items/inventory-moves.md §7.11, §7.18
//! The item-use dispatcher `0x005BF240` on the inventory model (the
//! `use_item_at` / `use_item` seams) and the Town Portal entry.
//!
//! [`InvDesk::dispatch_use`] runs `items/use.md` §1: the entry n from the
//! books row (`pSpell`, extra = `BookSkill`) or the items row (`pSpell`),
//! the first / second use with item flag 0x4, and the failure reset (§2)
//! with S→C 0x7C on a refused use. Entry 2 (Town Portal, §4) is the cast
//! `0x005BE290` (`objects-2.md` §27.1) up to its town refusal; the pair
//! itself is made by the action wiring (`ActionSim::open_town_portal`),
//! which this desk cannot reach, so a cast that passes the refusal is
//! recorded as a request ([`InvState::portal_requests`]) the host takes
//! after the call. Its cost (the scroll's skill count and consumption,
//! a tome's charge) is the caller's (`inventory-moves.md` §7.11 step 3,
//! §7.18 step 9), paid only for a 1.
//!
//! Entry 3 (healing and mana potions) is the body of §3.1
//! ([`InvDesk::use_entry3`]). The other entries keep their earlier
//! answers: stamina (REC-135) and rejuvenation (d2rs-own) potions and
//! identify (REC-113) on `use_item`, everything else on the rest
//! (`items/use.md` open question 1: their bodies are unwritten).
//!
//! PROVISIONAL (REC-289): the cast's creation (§27.1 step 6) runs after
//! the call, so a creation that fails still costs the use.

use super::{InvDesk, InvRest};
use crate::items::moves::{iflag, layouts, ty, Guid, MovePending, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

/// Town Portal scroll and tome.
pub const SCROLL: [u8; 4] = *b"tsc ";
pub const TOME: [u8; 4] = *b"tbk ";

/// Entries of the use table `0x00741790` (`items/use.md` §3).
pub mod entry {
    /// Identify (`isc`, `ibk`): the only entry with a first word.
    pub const IDENTIFY: u32 = 1;
    /// Town Portal (`tsc`, `tbk`).
    pub const TOWN_PORTAL: u32 = 2;
    /// Healing and mana potions (`hp1`–`hp5`, `mp1`–`mp5`).
    pub const POTION: u32 = 3;
    /// The table's size `[0x0074178C]`.
    pub const COUNT: u32 = 31;
}

/// Pandemonium Finale: the cast's other refusal (`objects-2.md` §27.1
/// step 4).
const LEVEL_FINALE: u32 = 136;
/// Sound 24 `notintown` (§27.1 step 4).
const SOUND_NOT_IN_TOWN: u16 = 24;
/// Item flag 0x4: armed for use (`items/use.md` §1 steps 4–5, §2).
const ARMED: u32 = iflag::TARGETING;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// §1 step 2: the entry n of item `item` and the extra e (−1 unless a
    /// books row gives `BookSkill`); `None` for n < 1 or n ≥ 31.
    pub fn use_entry(&self, item: Guid) -> Option<(u32, i32)> {
        let rec = self.tables.item(self.record_of(item)?)?;
        let book_or_scroll = self.is_type(item, ty::BOOK) || self.is_type(item, ty::SCRO);
        let books = usize::try_from(self.spell(item))
            .ok()
            .and_then(|i| self.tables.books.get(i));
        let (n, e) = match books {
            Some(b) if book_or_scroll && b.pspell > 0 => (b.pspell, b.bookskill),
            _ => (rec.pspell, -1),
        };
        (1..entry::COUNT).contains(&n).then_some((n, e))
    }

    /// The dispatcher `0x005BF240(U = player; I = item, T = target, x, y)`
    /// (`items/use.md` §1): true = used.
    pub fn dispatch_use(
        &mut self,
        player: Owner,
        item: Guid,
        target: Owner,
        x: i32,
        y: i32,
    ) -> bool {
        // Step 1.
        if player.ty > 1 || self.item_unit(item).is_none() {
            return false;
        }
        // Steps 2–3.
        let Some((n, _e)) = self.use_entry(item) else {
            return false;
        };
        let armed = self.item_flags(item) & ARMED != 0;
        // Step 4: only entry 1 has a first word (`0x005BE130`, unwritten:
        // open question 1); identify keeps its REC-113 answer below.
        if n == entry::IDENTIFY && !armed {
            return self.use_unwritten(player, item, target, x, y);
        }
        // Step 5: entries without a second word are 12–30 only.
        let f = self.item_flags(item);
        self.set_item_flags(item, f | ARMED);
        let r = match n {
            entry::TOWN_PORTAL => self.cast_town_portal(player),
            // §3.1: U drinks it, whatever the target.
            entry::POTION => self
                .unit_of(player)
                .is_some_and(|u| self.use_entry3(u, item)),
            _ => self.use_unwritten(player, item, target, x, y),
        };
        if !r {
            self.use_failure_reset(player);
            self.send(player, layouts::item_used(Owner::ITEM, item));
        }
        r
    }

    /// The entries whose bodies are unwritten (`items/use.md` open
    /// question 1): the earlier answers (REC-135 stamina and the d2rs-own
    /// rejuvenation potions, REC-113 identify on a target, the rest).
    fn use_unwritten(&mut self, player: Owner, item: Guid, target: Owner, x: i32, y: i32) -> bool {
        if target == Owner::item(item) {
            // A potion used from the grid (0x20) is drunk by U, the
            // player, as entry 3 is (§3.1 reads only U and I).
            if self.use_potion(player, item) {
                return true;
            }
            return self.rest.use_item_at(player, item, x, y);
        }
        if target == player && self.use_potion(player, item) {
            return true;
        }
        if self.use_identify(player, target, item) {
            return true;
        }
        self.rest.use_item(player, target, item)
    }

    /// The cast `0x005BE290` (`objects-2.md` §27.1) up to its town
    /// refusal (steps 1, 3, 4); the rest of it is requested (module doc).
    fn cast_town_portal(&mut self, player: Owner) -> bool {
        let Some(p) = self.unit_of(player).filter(|_| player.is_player()) else {
            return false;
        };
        let level = self
            .econ
            .game
            .lists
            .unit(p)
            .and_then(|e| e.room())
            .and_then(|r| self.econ.hooks.room_level(self.econ.game, r));
        if self.in_town(player) || level == Some(LEVEL_FINALE) {
            let _ = crate::units::sound::queue_sound(self.econ.game, p, SOUND_NOT_IN_TOWN, Some(p));
            return false;
        }
        self.state.portal_requests.push(p);
        true
    }

    /// Failure reset `0x005BE1C0` (`items/use.md` §2): every item of the
    /// player's inventory list with flag 0x4 loses it, each with S→C 0x3F
    /// (`3F FF`, GUID, `FF FF`).
    fn use_failure_reset(&mut self, player: Owner) {
        let Some(items) = self
            .unit_of(player)
            .and_then(|p| self.state.inventories.get(&p))
            .map(|i| i.items().to_vec())
        else {
            return;
        };
        for u in items {
            let g = self.guid_of(u);
            let f = self.item_flags(g);
            if f & ARMED != 0 {
                self.set_item_flags(g, f & !ARMED);
                self.send(player, layouts::use_stackable(0xFF, g, 0xFFFF));
            }
        }
    }

    /// The item is a Town Portal scroll.
    pub fn is_portal_scroll(&self, item: Guid) -> bool {
        self.item_code(item) == Some(SCROLL)
    }

    fn record_of(&self, item: Guid) -> Option<usize> {
        let i = self.item_unit(item)?;
        self.state.items.get(&i).map(|d| d.record)
    }

    fn item_code(&self, item: Guid) -> Option<[u8; 4]> {
        self.tables.item(self.record_of(item)?).map(|r| r.code)
    }

    /// Item skill `0x0055E050` (`inventory-moves.md` §7.18 step 6): a
    /// book's books `bookskill`, a scroll's `scrollskill`, else −1. No
    /// books row (the original's fatal) reads −1.
    pub fn book_item_skill(&self, item: Guid) -> i32 {
        let row = usize::try_from(self.spell(item))
            .ok()
            .and_then(|i| self.tables.books.get(i));
        match (self.primary_type(item), row) {
            (t, Some(b)) if t == ty::BOOK => b.bookskill,
            (t, Some(b)) if t == ty::SCRO => b.scrollskill,
            _ => -1,
        }
    }

    /// The Town Portal uses since the last call, by player unit.
    pub fn take_portal_requests(&mut self) -> Vec<UnitId> {
        std::mem::take(&mut self.state.portal_requests)
    }

    /// The walks to ground items since the last call (player, item,
    /// cursor flag).
    pub fn take_item_walks(&mut self) -> Vec<(UnitId, UnitId, bool)> {
        std::mem::take(&mut self.state.item_walks)
    }

    /// The items placed on the ground since the last call
    /// ([`InvState::dropped`](super::InvState::dropped)).
    pub fn take_dropped(&mut self) -> std::collections::BTreeSet<UnitId> {
        std::mem::take(&mut self.state.dropped)
    }
}
