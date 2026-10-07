// Spec: specs/world/quests.md §9.1; specs/items/generation.md §10.1, §10.2; specs/items/inventory.md §2.4, §5.5; specs/items/inventory-moves.md §9.1; specs/world/quests-act5.md open question 2; specs/world/vendors.md "Stat readers"
//! The quest reward `0x005466B0` (`quests.md` §9.1) on the host's
//! inventory model: the item created from its code (`0x00633640`,
//! `0x00559CE0`), its durability filled, placed in the player's inventory
//! (`0x00560200`) or, when it does not fit, dropped next to the player
//! (`0x00545340` + the ground placement) or freed.
//!
//! The inventory model is the host's ([`QuestInventory`]): a quest call
//! reaches it only when the host lends it ([`super::HostQuests::inventory`]);
//! [`QuestInv`] is the inventory wiring's provider of it
//! (`crate::wiring::inventory`). The free-spot search is the quest
//! helper's ([`crate::world::quests::helpers::free_spot`]), run by
//! [`super::HostQuests`] between the two halves here.
//!
//! Status: wired, unverified (no recording of a quest reward).

use super::{Economy, EconomyError, ItemSpawn, UnitStats};
use crate::items::bitstream::read::record_of;
use crate::items::inventory::{InvTables, InvWorld};
use crate::items::moves::{ground, Spot};
use crate::items::{flag, stat, CreateError, ItemGame, ItemRequest, ItemStats};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};
use crate::wiring::inventory::{InvDesk, InvRest, InvState};

/// Unit mode of an item created into an inventory (`generation.md` §10.2
/// spawn mode; 4 = inventory / cursor).
const MODE_INVENTORY: u32 = 4;
/// §9.1's drop search: size 1, mask 0x3E01, limit 100 (`0x00545340`).
pub const DROP_SIZE: u32 = 1;
pub const DROP_MASK: u32 = 0x3E01;
pub const DROP_LIMIT: u32 = 100;

/// The host's inventory model as a quest call sees it, on the call's
/// economy (one inventory per unit: the item moves', the vendors' and the
/// cube's).
pub trait QuestInventory<H> {
    /// `0x00560200(game, player, item, 0, 0, find free 1, send 1, 0)`
    /// (`items/inventory.md` §2.4) into the page the item data names;
    /// true when placed.
    fn place(&mut self, econ: &mut Economy<'_, H>, player: UnitId, item: UnitId) -> bool;
    /// The ground placement `0x00558AA0` (`inventory-moves.md` §9.1 step
    /// 3) of `item` at `spot`.
    fn drop_at(&mut self, econ: &mut Economy<'_, H>, item: UnitId, spot: Spot);
    /// `0x0055FA40` (`items/inventory.md` §5.5) on the player.
    fn inventory_pass(&mut self, econ: &mut Economy<'_, H>, player: UnitId);
    /// An economy error of a quest call's item creation (the host keeps
    /// it with its other provider errors).
    fn fault(&mut self, error: EconomyError);
}

/// [`QuestInventory`] on the inventory wiring: an [`InvDesk`] over the
/// call's economy for each call, with the host's inventory tables, state
/// and rest. Creation errors are kept in [`QuestInv::errors`].
pub struct QuestInv<'d, R: ?Sized> {
    pub tables: &'d InvTables,
    pub state: &'d mut InvState,
    pub rest: &'d mut R,
    pub errors: Vec<EconomyError>,
}

impl<'d, R: ?Sized> QuestInv<'d, R> {
    pub fn new(tables: &'d InvTables, state: &'d mut InvState, rest: &'d mut R) -> Self {
        Self {
            tables,
            state,
            rest,
            errors: Vec::new(),
        }
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> QuestInventory<H> for QuestInv<'_, R> {
    fn place(&mut self, econ: &mut Economy<'_, H>, player: UnitId, item: UnitId) -> bool {
        InvDesk::new(econ, self.tables, self.state, self.rest).place(
            player,
            item,
            (0, 0),
            true,
            true,
        )
    }
    fn drop_at(&mut self, econ: &mut Economy<'_, H>, item: UnitId, spot: Spot) {
        let mut d = InvDesk::new(econ, self.tables, self.state, self.rest);
        let g = d.guid_of(item);
        ground::ground_place(&mut d, g, spot);
        d.sync_out();
    }
    fn inventory_pass(&mut self, econ: &mut Economy<'_, H>, player: UnitId) {
        let mut d = InvDesk::new(econ, self.tables, self.state, self.rest);
        InvWorld::inventory_pass(&mut d, player);
    }
    fn fault(&mut self, error: EconomyError) {
        self.errors.push(error);
    }
}

/// `0x00558200(unit, 0)` (`quests-act5.md` open question 2): a player's
/// base stat 12, a monster's total stat 12, at least 1. `None`: another
/// unit type (its room level's `MonLvl`, not read here).
pub fn item_level<H>(econ: &Economy<'_, H>, unit: UnitId) -> Option<i32> {
    let v = match econ.units.get(unit)?.ty {
        UnitType::Player => econ.stats.unit_base(unit, stat::LEVEL, 0),
        UnitType::Monster => econ.stats.unit_total(unit, stat::LEVEL, 0),
        _ => return None,
    };
    Some(v.max(1))
}

/// `0x00625E00` (`vendors.md` "Stat readers"): 0 when the item has no
/// base stat 73, else its total stat 73.
///
/// PROVISIONAL (vendors.md "Stat readers"; REC-none): "no base-array
/// entry" is read as base stat 73 = 0 (the stat lists keep no empty
/// entries apart).
fn max_durability<H>(econ: &Economy<'_, H>, item: UnitId) -> i32 {
    if econ.stats.unit_base(item, stat::MAXDURABILITY, 0) == 0 {
        return 0;
    }
    econ.stats.unit_total(item, stat::MAXDURABILITY, 0)
}

/// §9.1's first half: the code lookup (`0x00633640`, `generation.md`
/// §10.1; absent → none), the level (`level` ≠ 0, else the player's
/// [`item_level`]), the creation `0x00559CE0` (§10.2) and the item made
/// ready for placement: durability := max durability when that is > 0,
/// inventory page 0. `Ok(None)`: no item (unknown code, or the creation
/// refused it).
///
/// The §10.2 arguments §9.1 does not name are chosen:
/// PROVISIONAL (quests.md §9.1; REC-none): source unit = the player
/// (class and level; no player data, which only ears read), spawn mode 4
/// (inventory), no-sockets 0, never-ethereal 0, no seeds ("use seed" 0).
pub fn create_reward<H: LifecycleHooks>(
    econ: &mut Economy<'_, H>,
    player: UnitId,
    code: [u8; 4],
    level: i32,
    quality: u8,
) -> Result<Option<UnitId>, EconomyError> {
    let Some(rec) = record_of(econ.tables, code) else {
        return Ok(None);
    };
    let level = if level != 0 {
        level
    } else {
        item_level(econ, player).unwrap_or(1)
    };
    let mut rq = ItemRequest {
        unit: econ.request_unit(player, None),
        // §10.2: ilvl ≤ 0 becomes 1.
        ilvl: level.max(1),
        item: rec as i32,
        format: ItemGame::item_format(&*econ.fields),
        quality,
        ..ItemRequest::default()
    };
    let spawn = ItemSpawn {
        room: None,
        mode: MODE_INVENTORY,
        init_flags: 1,
    };
    let item = match econ.create_item(&mut rq, false, spawn) {
        Ok(u) => u,
        Err(EconomyError::Create(CreateError::Fatal(f))) => return Err(f.into()),
        Err(EconomyError::Create(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    // §10.2: on success, item flag 0x10 (identified).
    if let Some(i) = econ.items.get_mut(item) {
        i.flags |= flag::IDENTIFIED;
    }
    let max = max_durability(econ, item);
    if max > 0 {
        econ.with_stats(|ctx| UnitStats::new(ctx, item).set_base(stat::DURABILITY, 0, max));
    }
    if let Some(i) = econ.items.get_mut(item) {
        i.inv_page = 0;
    }
    Ok(Some(item))
}

/// What §9.1's placement did with a created reward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    /// In the player's inventory (identified).
    Stored(UnitId),
    /// The placement failed: the caller drops or frees it.
    Refused(UnitId),
}

/// §9.1's second half up to the drop: place the created item
/// (`0x00560200`); placed → identify it unless identified
/// (`0x006280D0(item, 0x10)`).
pub fn place_reward<H: LifecycleHooks>(
    econ: &mut Economy<'_, H>,
    inv: &mut dyn QuestInventory<H>,
    player: UnitId,
    item: UnitId,
) -> Placed {
    if !inv.place(econ, player, item) {
        return Placed::Refused(item);
    }
    if let Some(i) = econ.items.get_mut(item) {
        i.flags |= flag::IDENTIFIED;
    }
    Placed::Stored(item)
}

/// §9.1's end for an item the placement refused: droppable with a spot →
/// dropped there and returned; otherwise freed (`0x00555600`) and none.
pub fn drop_or_free<H: LifecycleHooks>(
    econ: &mut Economy<'_, H>,
    inv: &mut dyn QuestInventory<H>,
    item: UnitId,
    spot: Option<Spot>,
) -> Option<UnitId> {
    if let Some(s) = spot {
        inv.drop_at(econ, item, s);
        return Some(item);
    }
    if let Err(e) = econ.free_item(item) {
        inv.fault(e);
    }
    None
}
