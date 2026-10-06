// Spec: specs/items/inventory.md (wiring of the inventory and item-move seams); specs/sim/units.md §2; specs/sim/unit-order.md §5–§6; specs/items/generation.md §1.3
//! The inventory seams on their real providers: the item-move code
//! (`items::moves`, `inventory.md` §6–§11) runs on the inventory model
//! (`items::inventory`, §1–§5), the unit records and lists, the stat lists
//! and the economy's item store.
//!
//! - [`InvDesk`] implements [`crate::items::inventory::InvWorld`]
//!   ([`inv_world`]), [`crate::items::moves::InventoryOps`] ([`ops`]),
//!   [`crate::items::moves::MoveUnits`] ([`units`]) and
//!   [`crate::items::moves::MovePending`] ([`pending`]): every move
//!   handler (`items::moves::handle`) and the per-client update pass
//!   (`items::moves::player_update`) run on it.
//! - [`host`]: the model for the other item systems (vendors, the cube):
//!   reads on [`InvState`], placement / removal / checks on [`InvDesk`],
//!   keyed by unit, so one inventory per owner serves every system.
//! - [`InvRest`]: the calls without a d2-sim provider (positions and the
//!   free-spot search of `sim/path-placement.md`, player data, NPC
//!   interaction, item use, sockets, hirelings, sounds, transport, and
//!   the open questions of `inventory.md`).
//!
//! Ownership, one owner per field: the unit record holds the mode
//! (+0x10), the unit flags (+0xC4) and the update bits (+0xC8); the
//! economy's [`ItemStore`] holds the item flags (+0x18), the page
//! (+0x45), quality and file index; the stat lists hold the stats;
//! [`InvState`] holds the inventories and the item data fields only this
//! spec uses (command flags, body location, stored page, grid position,
//! item owner, owning inventory, node fields, ground expiry). The
//! [`InvItem`] the inventory functions read is a copy of the first three
//! owners' fields plus [`InvState`]'s: [`InvDesk::new`] fills it, the
//! [`crate::items::moves::MoveUnits`] setters write through, and every
//! mutating inventory call writes it back.
//!
//! Status: wired, unverified (`inventory.md` is a draft; no recording
//! R1–R6 exists). Nothing here decides behaviour: rules stay in the
//! modules; an adapter maps a seam call to a provider call.

pub mod bits;
pub mod host;
pub mod inv_world;
pub mod ops;
pub mod pending;
pub mod units;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use super::economy::{Economy, EconomyError, ItemSpawn};
use crate::items::inventory::{InteractionTarget, InvItem, InvTables, Inventory, UnitKind};
use crate::items::moves::{deferred, Guid, MovePending, Owner};
use crate::items::ItemRequest;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::lists::ListError;
use crate::units::{UnitId, UnitType};

/// What went wrong in an adapter (a provider's error), in order. The
/// handlers' own fatal asserts are `items::moves::MoveFatal`.
#[derive(Debug, PartialEq, Eq)]
pub enum InvError {
    Economy(EconomyError),
    List(ListError),
    /// An item was freed (`0x00557FD0`) while still linked in an
    /// inventory; the spec writes no unlink there.
    FreedWhileLinked(UnitId),
    /// Ground placement found the item in another room than the spot's
    /// (`0x00558AA0` "room added" is written for an item in no room).
    OtherRoom(UnitId),
    /// `0x0063BE30` found the body slot still holding an item after the
    /// unlink (the model clears cells only through §1.4 unlink).
    BodySlotHeld(UnitId, u8),
}

/// The inventory state of a game: one [`Inventory`] per unit that owns
/// one (players, NPCs, hirelings, items with fillers) and the item data
/// fields of [`InvItem`] that this spec owns.
#[derive(Debug, Default)]
pub struct InvState {
    pub inventories: BTreeMap<UnitId, Inventory>,
    /// Item data per item unit (§1.1). Mode, item flags, page, GUID and
    /// record are copies of the unit record's and the item store's.
    pub items: BTreeMap<UnitId, InvItem>,
    /// Ground expiry (item data +0x24, §9.2).
    pub expiry: BTreeMap<UnitId, i32>,
    pub errors: Vec<InvError>,
    /// Owner refreshes asked by the inventory functions during a call;
    /// run (`items::moves::owner_refresh`) when the call returns.
    refresh: Vec<UnitId>,
}

impl InvState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Gives `unit` an inventory (`0x0063ABD0`; the owner kind decides
    /// the grid records, §1.3).
    pub fn add_inventory(&mut self, unit: UnitId, kind: UnitKind, guid: u32) {
        self.inventories
            .entry(unit)
            .or_insert_with(|| Inventory::new(unit, kind, guid));
    }
}

/// The inventory calls without a d2-sim provider. [`MovePending`] carries
/// the item-move ones (with their narrowest defaults); the methods here
/// are the inventory model's (`items::inventory::InvWorld`) and the unit
/// fields that no d2-sim module holds yet. Methods without a default are
/// open points of `inventory.md` the host must answer.
#[allow(unused_variables)]
pub trait InvRest: MovePending {
    // ---- positions (`sim/path-placement.md`, being implemented) ---------

    /// Position of a non-item unit (subtiles). Items keep theirs in the
    /// item data (§2.2 "item x, y"; ground placement §9.1 step 3).
    fn pos(&self, u: Owner) -> (i32, i32) {
        (0, 0)
    }
    fn set_pos(&mut self, u: Owner, x: i32, y: i32) {}

    // ---- item creation (`items/generation.md`; economy WE9) -------------

    /// The creation request of a gold pile (`0x00559CE0`, code `gld` at
    /// combined index `gld`): its layout is not written. Default: none
    /// (no pile).
    fn gold_request(&self, unit: Owner, gld: usize) -> Option<(ItemRequest, ItemSpawn)> {
        None
    }

    // ---- sockets (`items/properties.md` §9–§10) ---------------------------

    /// `0x0063B210` on an item's inventory (it sockets the item). Default:
    /// fails.
    fn socket_link(&mut self, target: Guid, item: Guid, kind: u8) -> bool {
        false
    }
    /// §7.19 step 3: the filler linked into the target's inventory
    /// (created first, `0x0063ABD0`). Default: fails.
    fn link_into_item(&mut self, target: Guid, filler: Guid) -> bool {
        false
    }
    /// Socketed with fillers (`0x0055F590`). Default: no.
    fn socket_filled(&self, item: Guid) -> bool {
        false
    }
    /// Socket-filler test (`0x0062BEB0`). Default: no.
    fn socket_filler(&self, item: Guid) -> bool {
        false
    }
    /// Book / scroll spell (`0x00627F80`). Default: 0.
    fn spell(&self, item: Guid) -> i32 {
        0
    }

    // ---- inventory model seams (`inventory.md` §2–§5) ---------------------

    /// Trade hook `0x00568770` (page 2; multiplayer trade).
    fn trade_hook(&mut self, owner: Owner, item: Guid) {}
    /// `0x0044BE50` of an item in the targeting reset (§5.3; 0 → S→C
    /// 0x3F). Default: 1 (nothing queued).
    fn targeting_probe(&self, item: Guid) -> u32 {
        1
    }
    /// `value × p / 100` (`0x00483360`; rounding: open question 4).
    fn percent_of(&self, value: i32, p: i32) -> i32;
    /// The item is active on the unit (`0x00625820`).
    fn item_active_on(&self, item: Guid, unit: Owner) -> bool;
    /// The item's own contribution to a unit stat (`0x0062B450`).
    fn own_contribution(&self, item: Guid, unit: Owner, stat: u16) -> i32;
    /// Level requirement (`0x0062B5B0`, −1 none; open question 5).
    fn level_requirement(&self, item: Guid, unit: Owner) -> i32;
    /// Two-handed (`0x006289C0`).
    fn two_handed(&self, item: Guid) -> bool;
    /// One-or-two-handed for the unit (`0x0062A1E0`).
    fn one_or_two_handed(&self, unit: Owner, item: Guid) -> bool;
    /// Ammo type (`0x0062E6F0`): an itemtypes row.
    fn ammo_type(&self, item: Guid) -> Option<i16>;
    /// `0x0062A2F0` in the stack test (open question 7).
    fn stack_quality_ok(&self, item: Guid) -> bool;
    /// Allowed location (`0x0062FDF0`).
    fn has_allowed_location(&self, item: Guid) -> bool;
    /// Quiver-type item (`0x00628480`).
    fn quiver_kind(&self, item: Guid) -> bool;
    /// Weapon / shield comparison of auto-equip (open question 8).
    fn auto_equip_allows(&self, unit: Owner, item: Guid, loc: u8) -> bool;

    // ---- player data and interaction (`world/npc.md` §2) -------------------

    /// The player's interaction (`0x00554100`).
    fn interaction(&self, player: Owner) -> InteractionTarget;
    /// `0x00554190`.
    fn clear_interaction(&mut self, player: Owner);
    /// Player data +0x4C.
    fn player_data_4c(&self, player: Owner) -> u32;
    /// Player data +0x50.
    fn player_data_50(&self, player: Owner) -> u32;
    /// The player talks to the NPC (interaction list state 0).
    fn npc_talking(&self, npc: Owner, player: Owner) -> bool;
    /// The player-trade part of `0x00567620` (multiplayer).
    fn player_trade_gate(&self, player: Owner) -> Option<bool>;
}

/// The inventory world of one call: the economy's parts (game, unit
/// records and lists, stat lists, hooks, game fields, item store), the
/// inventory tables, the inventory state and the seams without a provider
/// (`R`).
pub struct InvDesk<'d, 'a, H, R: ?Sized> {
    pub econ: &'d mut Economy<'a, H>,
    pub tables: &'d InvTables,
    pub state: &'d mut InvState,
    pub rest: &'d mut R,
}

impl<'d, 'a, H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'d, 'a, H, R> {
    /// A desk for one call; the item data copies are filled from their
    /// owners.
    pub fn new(
        econ: &'d mut Economy<'a, H>,
        tables: &'d InvTables,
        state: &'d mut InvState,
        rest: &'d mut R,
    ) -> Self {
        let mut d = Self {
            econ,
            tables,
            state,
            rest,
        };
        d.sync_in();
        d
    }

    /// Unit lookup by type and GUID (`unit-order.md` §2).
    pub fn unit_of(&self, o: Owner) -> Option<UnitId> {
        let ty = *UnitType::ALL.get(usize::from(o.ty))?;
        self.econ.game.lists.find_unit(ty, o.guid)
    }

    /// The item unit of a GUID (lookup type 4), when it has item data.
    pub fn item_unit(&self, guid: Guid) -> Option<UnitId> {
        self.econ
            .game
            .lists
            .find_unit(UnitType::Item, guid)
            .filter(|u| self.state.items.contains_key(u))
    }

    /// Type and GUID of a unit.
    pub fn owner_of(&self, u: UnitId) -> Option<Owner> {
        self.econ.units.get(u).map(|r| Owner {
            ty: r.ty.index() as u8,
            guid: r.guid,
        })
    }

    /// The GUID of a unit (−1 when it has no record).
    pub fn guid_of(&self, u: UnitId) -> Guid {
        self.econ.units.get(u).map_or(u32::MAX, |r| r.guid)
    }

    /// Fills the item data copies from the unit records and the item
    /// store: every item unit of the lists with item data; entries of
    /// units that are gone are dropped.
    pub fn sync_in(&mut self) {
        let live = self.econ.game.lists.units_of_type(UnitType::Item);
        for &u in &live {
            let (Some(r), Some(it)) = (self.econ.units.get(u), self.econ.items.get(u)) else {
                continue;
            };
            let d = self
                .state
                .items
                .entry(u)
                .or_insert_with(|| InvItem::new(r.guid, it.record));
            d.guid = r.guid;
            d.record = it.record;
            d.mode = r.mode as u8;
            d.flags = it.flags;
            d.page = it.inv_page;
        }
        self.state.items.retain(|u, _| live.contains(u));
    }

    /// Writes the copied fields back to their owners.
    pub fn sync_out(&mut self) {
        for (&u, d) in &self.state.items {
            if let Some(r) = self.econ.units.get_mut(u) {
                r.mode = u32::from(d.mode);
            }
            if let Some(it) = self.econ.items.get_mut(u) {
                it.flags = d.flags;
                it.inv_page = d.page;
            }
        }
    }

    /// Runs `f` on the owner's inventory, lent out of the state, then
    /// writes the item copies back and runs the owner refreshes the call
    /// asked for. `None` when the owner has no inventory.
    pub(crate) fn with_inv<T>(
        &mut self,
        owner: Owner,
        f: impl FnOnce(&mut Inventory, &mut Self) -> T,
    ) -> Option<T> {
        let u = self.unit_of(owner)?;
        let mut inv = self.state.inventories.remove(&u)?;
        let out = f(&mut inv, self);
        self.state.inventories.insert(u, inv);
        self.sync_out();
        for r in std::mem::take(&mut self.state.refresh) {
            if let Some(o) = self.owner_of(r) {
                deferred::owner_refresh(self, o);
            }
        }
        Some(out)
    }

    /// The clean-up after the update pass (§6.1 rule 4): each item of the
    /// owner's update list gets its command flags reset (0) and the list
    /// is freed. The per-unit flags (+0xC8) are the room clean-up's
    /// (`tick.md` §3 step 6), not this one's.
    // TODO(spec: inventory.md §6.1 r4, OQ9): the 1.14d address of this
    // step; "reset" is read as 0.
    pub fn update_done(&mut self, owner: Owner) {
        let Some(u) = self.unit_of(owner) else {
            return;
        };
        let Some(list) = self.state.inventories.get_mut(&u).map(|i| i.take_updates()) else {
            return;
        };
        for g in list {
            if let Some(d) = self.item_unit(g).and_then(|i| self.state.items.get_mut(&i)) {
                d.cmd_flags = 0;
            }
        }
    }

    /// The owner's inventory.
    pub fn inventory(&self, owner: Owner) -> Option<&Inventory> {
        self.state.inventories.get(&self.unit_of(owner)?)
    }

    /// The owner's inventory, or an empty one when it has none (the
    /// checks of §5 then see no item of the owner).
    pub(crate) fn inv_or_empty(&self, owner: Owner) -> std::borrow::Cow<'_, Inventory> {
        match self.inventory(owner) {
            Some(i) => std::borrow::Cow::Borrowed(i),
            None => {
                let u = self.unit_of(owner).unwrap_or(UnitId(u32::MAX));
                let kind = self.kind_of(u).unwrap_or(UnitKind::Other);
                std::borrow::Cow::Owned(Inventory::new(u, kind, owner.guid))
            }
        }
    }

    /// The inventory model's kind of a unit (§1.3, §4.2–§4.4).
    pub fn kind_of(&self, u: UnitId) -> Option<UnitKind> {
        let r = self.econ.units.get(u)?;
        Some(match r.ty {
            UnitType::Player => UnitKind::Player {
                class: r.class as u8,
            },
            UnitType::Monster => UnitKind::Monster { class: r.class },
            UnitType::Object => UnitKind::Object { class: r.class },
            UnitType::Item => UnitKind::Item,
            UnitType::Missile | UnitType::Tile => UnitKind::Other,
        })
    }

    pub(crate) fn note_list(&mut self, r: Result<(), ListError>) {
        if let Err(e) = r {
            self.state.errors.push(InvError::List(e));
        }
    }
}
