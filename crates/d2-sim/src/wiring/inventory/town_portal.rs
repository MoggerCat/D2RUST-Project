// Spec: specs/skills/bodies-3.md §4.4; specs/items/inventory-moves.md §7.11
//! Town Portal scroll and tome use (C→S 0x20, the `use_item_at` seam).
//!
//! The scroll / book skill (`srvdo 113`, `bodies-3.md` §4.4) is consumed
//! through the item-use path; its effect, the portal pair, is made by the
//! action wiring (`ActionSim::open_town_portal`), which this desk cannot
//! reach. The use is therefore recorded as a request
//! ([`InvState::portal_requests`]) that the host takes after the call.
//!
//! PROVISIONAL (REC-117): the item-use table `0x00741790` entry of
//! `tsc` / `tbk` is unwritten; the codes below stand in for it. A tome
//! loses one charge here (the move handler leaves a book's quantity
//! alone when no books row is known).
// d2rs-own, unverified

use super::{InvDesk, InvRest};
use crate::items::moves::{stat, Guid, MovePending, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

/// Town Portal scroll and tome.
pub const SCROLL: [u8; 4] = *b"tsc ";
pub const TOME: [u8; 4] = *b"tbk ";

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// `use_item_at` for a Town Portal scroll or tome: true = used, the
    /// request recorded. Any other item: false.
    pub fn use_portal_item(&mut self, player: Owner, item: Guid) -> bool {
        let Some(p) = self.unit_of(player) else {
            return false;
        };
        let Some(code) = self.item_code(item) else {
            return false;
        };
        match code {
            SCROLL => {}
            TOME => {
                let it = Owner::item(item);
                let q = self.stat(it, stat::QUANTITY);
                if q < 1 {
                    return false;
                }
                self.set_stat(it, stat::QUANTITY, q - 1);
                self.send_item_stat(player, item, stat::QUANTITY);
            }
            _ => return false,
        }
        self.state.portal_requests.push(p);
        true
    }

    /// The item is a Town Portal scroll.
    pub fn is_portal_scroll(&self, item: Guid) -> bool {
        self.item_code(item) == Some(SCROLL)
    }

    fn item_code(&self, item: Guid) -> Option<[u8; 4]> {
        let i = self.item_unit(item)?;
        let record = self.state.items.get(&i).map_or(0, |d| d.record);
        self.tables.item(record).map(|r| r.code)
    }

    /// The Town Portal uses since the last call, by player unit.
    pub fn take_portal_requests(&mut self) -> Vec<UnitId> {
        std::mem::take(&mut self.state.portal_requests)
    }
}
