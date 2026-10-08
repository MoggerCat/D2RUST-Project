// Spec: specs/items/use.md §1–§4; specs/world/objects-2.md §27.1; specs/items/inventory-moves.md §7.11 r3, §7.17, §7.18 r6
//! The item-use dispatcher `0x005BF240` on the desk, for the entries
//! d2-sim runs: entry 2 (Town Portal, word 1 `0x005BE290`, the cast of
//! the host that owns the objects, [`LifecycleHooks::town_portal_cast`]).
//! The index comes from the item's `books.txt` row, else its items
//! record (`pSpell`); the item gets flag 0x4; a zero result resets every
//! flagged item (S→C 0x3F each) and sends S→C 0x7C. The caller (0x20,
//! 0x26, 0x27) charges the item only on a nonzero result.
//!
//! The other entries (identify, potions, the cube) keep their callers'
//! stand-ins ([`InvDesk::item_use`] answers `None` for them).

use super::{InvDesk, InvRest};
use crate::items::moves::{iflag, layouts, ty, Guid, MovePending, MoveUnits, Owner};
use crate::units::hooks::Sim;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};

/// The use-table entry of `tsc` / `tbk` (`items/use.md` §3).
pub const ENTRY_TOWN_PORTAL: i32 = 2;
/// The table's entry count `[0x0074178C]` (`items/use.md` §1 rule 3).
const TABLE_COUNT: i32 = 31;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The walks to ground items since the last call (player, item,
    /// cursor flag; `inventory-moves.md` §7.1 step 2, REC-281).
    pub fn take_item_walks(&mut self) -> Vec<(UnitId, UnitId, bool)> {
        std::mem::take(&mut self.state.item_walks)
    }

    /// The use-table index n and the extra e of an item (`items/use.md` §1
    /// rule 2): a `book` (18) or `scro` (22) with a books row (item data
    /// +0x3E): e := `BookSkill`, n := its `pSpell`, when > 0; else the
    /// items record's `pSpell`. `None`: no item.
    pub fn use_index(&self, item: Guid) -> Option<(i32, i32)> {
        let u = self.item_unit(item)?;
        let mut e = -1;
        if self.is_type(item, ty::BOOK) || self.is_type(item, ty::SCRO) {
            let row = usize::try_from(self.spell_of(u))
                .ok()
                .and_then(|s| self.tables.books.get(s));
            if let Some(b) = row {
                e = b.bookskill;
                if b.pspell > 0 {
                    return Some((b.pspell, e));
                }
            }
        }
        let n = self.item_rec(item).map_or(0, |r| r.pspell as i32);
        Some((n, e))
    }

    /// `0x005BF240(game, U; I, T, x, y)` (`items/use.md` §1) for the
    /// entries run here: `Some(result)`; `None`: the item's entry is not
    /// one of them (the caller keeps its own answer). Entry 2 has no
    /// first word: I's flag 0x4 is set, then the cast (§4).
    pub fn item_use(&mut self, user: Owner, item: Guid) -> Option<u32> {
        let (n, _e) = self.use_index(item)?;
        if !(1..TABLE_COUNT).contains(&n) || n != ENTRY_TOWN_PORTAL {
            return None;
        }
        // Rule 1.
        let Some(u) = self.unit_of(user) else {
            return Some(0);
        };
        let kind = self.econ.units.get(u).map(|r| r.ty);
        if !matches!(kind, Some(UnitType::Player | UnitType::Monster)) {
            return Some(0);
        }
        // Rule 5.
        let f = self.item_flags(item);
        self.set_item_flags(item, f | iflag::TARGETING);
        let (r, item_message) = self.town_portal_cast(u);
        if item_message {
            MovePending::send(self, user, layouts::item_used(Owner::ITEM, item));
        }
        if r == 0 {
            self.failure_reset(user);
            MovePending::send(self, user, layouts::item_used(Owner::ITEM, item));
        }
        Some(r)
    }

    /// The cast on the hooks with the call's units and game seed
    /// ([`LifecycleHooks::town_portal_cast`]); no host: 0, no message.
    fn town_portal_cast(&mut self, player: UnitId) -> (u32, bool) {
        let e = &mut *self.econ;
        let mut sim = Sim {
            game: &mut *e.game,
            units: &mut *e.units,
            stats: &mut *e.stats,
            data: e.data,
        };
        e.hooks
            .town_portal_cast(&mut sim, &mut e.fields.seed, player)
            .unwrap_or((0, false))
    }

    /// Failure reset `0x005BE1C0` (`items/use.md` §2): every item of U's
    /// inventory list with flag 0x4, in list order: flag cleared, S→C
    /// 0x3F (`3F FF <GUID> FF FF`) to U's client. No inventory: nothing.
    pub fn failure_reset(&mut self, user: Owner) {
        let Some(u) = self.unit_of(user) else {
            return;
        };
        let Some(items) = self.state.inventories.get(&u).map(|i| i.items().to_vec()) else {
            return;
        };
        for i in items {
            let g = self.guid_of(i);
            let f = self.item_flags(g);
            if f & iflag::TARGETING == 0 {
                continue;
            }
            self.set_item_flags(g, f & !iflag::TARGETING);
            MovePending::send(self, user, layouts::use_stackable(0xFF, g, 0xFFFF));
        }
    }

    /// Item skill `0x0055E050` (`inventory-moves.md` §7.18 step 6): a book
    /// → its books row's `bookskill`, a scroll → `scrollskill`. `None`:
    /// not a book or scroll, or no books row in the tables (the caller
    /// asks the rest, whose default is −1).
    pub fn books_item_skill(&self, item: Guid) -> Option<i32> {
        let u = self.item_unit(item)?;
        let book = self.is_type(item, ty::BOOK);
        if !book && !self.is_type(item, ty::SCRO) {
            return None;
        }
        let b = usize::try_from(self.spell_of(u))
            .ok()
            .and_then(|s| self.tables.books.get(s))?;
        Some(if book { b.bookskill } else { b.scrollskill })
    }
}
