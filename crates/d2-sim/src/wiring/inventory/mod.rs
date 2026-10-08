// Spec: specs/items/inventory.md, specs/items/inventory-moves.md (wiring of the inventory and item-move seams); specs/sim/units.md §2; specs/sim/unit-order.md §5–§6; specs/items/generation.md §1.3
//! The inventory seams on their real providers: the item-move code
//! (`items::moves`, `inventory-moves.md` §6–§11) runs on the inventory model
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
pub mod copy;
pub mod cube_open;
pub mod equip_rules;
pub mod host;
pub mod identify;
pub mod inv_world;
pub mod item_link;
pub mod load;
pub mod merc;
pub mod ops;
pub mod pending;
pub mod potion;
pub mod queries;
pub mod save_index;
pub mod swap;
pub mod town_portal;
pub mod units;

#[cfg(test)]
pub(crate) mod tests;

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::economy::{Economy, EconomyError, ItemSpawn};
use crate::items::bitstream::WriteBack;
use crate::items::inventory::{InvItem, InvTables, Inventory, UnitKind};
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
    /// A fatal assert of an item-move rule run outside a move handler
    /// (e.g. the sell's direct 0x9D, `vendors.md` §7.2 rule 9).
    Move(crate::items::moves::MoveFatal),
    /// Ground placement found the item in another room than the spot's
    /// (`0x00558AA0` "room added" is written for an item in no room).
    OtherRoom(UnitId),
    /// `0x0063BE30` found the body slot still holding an item after the
    /// unlink (the model clears cells only through §1.4 unlink).
    BodySlotHeld(UnitId, u8),
    /// A step whose rule the spec does not write (named); the call stops
    /// there.
    Unwritten(&'static str),
    /// The item copy (`0x0055A2A0`) was given a source on the ground
    /// (`world/vendors-2.md` §7.3 step 1.1): a caller error; no copy.
    GroundCopySource(UnitId),
    /// Placement into a page (`0x00560200`) was given an item that is
    /// still in a room (`items/inventory.md` §2.4 rule 2): a caller error;
    /// not placed.
    PlacedWithRoom(UnitId),
    /// A fatal assert of the equipment bookkeeping (`inventory.md` §5.5,
    /// §5.8; [`equip_rules`]).
    Equip(crate::items::inventory::bookkeeping::EquipFatal),
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
    /// The equipment rules run on the desk ([`equip_rules`]: item-skill
    /// link, inventory pass, weapon bookkeeping, the set-item update) in
    /// place of the [`InvRest`] calls of the same addresses. Off (the
    /// default): those calls go to the rest, as before; a host turns it
    /// on when its rest answers the equipment seams (the skill list,
    /// player data mouse slots, stat links).
    pub equip_rules: bool,
    /// The item-move effects run on the desk in place of the [`InvRest`]
    /// calls (q-fix-items-movepending-host-wiring): the gold rest pile
    /// (`inventory-moves.md` §10.1) and the quest-chain notice 0x5D
    /// (§7.11 step 4). Off (the default): those
    /// calls go to the rest, as before.
    pub move_effects: bool,
    /// An item moved onto a body slot attaches its stat list to the
    /// wearer, and its leaving detaches it, with the set-item update and
    /// the set bonuses ([`item_link`]). PROVISIONAL (REC-161): the stat
    /// link `0x0063D1D0` / unlink `0x0063D2B0` bodies are unwritten
    /// (`stat-lists.md` §8.4 gives the attach). Off (the default): those
    /// calls go to the rest, as before.
    pub link_item_stats: bool,
    /// The weapon in use (inventory +0x1C, `0x0063BEF0`) is the right-hand
    /// item when +0x1C holds none, for the weapon bookkeeping of §5.8.
    /// PROVISIONAL (REC-266, d2rs-own, unverified): nothing in the play
    /// host writes +0x1C (the setter `0x006233A0` is the skills code's), so
    /// without this the bookkeeping never sees a weapon. Off by default.
    pub weapon_hand_fallback: bool,
    /// Town Portal scroll / tome uses of the call, taken by the host
    /// ([`InvDesk::take_portal_requests`]; `items/use.md` §4, the cast of
    /// `world/objects-2.md` §27.1 past its town refusal).
    pub portal_requests: Vec<UnitId>,
    /// The walks to a ground item the pick-ups of the call asked for
    /// (§7.1 step 2, `0x00548A50`: player, item, cursor flag), taken by
    /// the host that runs them ([`InvDesk::take_item_walks`], REC-281).
    pub item_walks: Vec<(UnitId, UnitId, bool)>,
    /// Equipment-rule calls the inventory functions asked for while the
    /// owner's inventory was lent to them ([`InvDesk::with_inv`]); run
    /// when the call returns, before the owner refreshes.
    equip_queue: Vec<equip_rules::EquipCall>,
    /// Owner refreshes asked by the inventory functions during a call;
    /// run (`items::moves::owner_refresh`) when the call returns.
    refresh: Vec<UnitId>,
    /// The bit-stream writer's changes to items (`items/bitstream.md`
    /// §4.1 rule 8, §4.3 rule 7), queued by the read-only stream seam and
    /// written by [`InvDesk::apply_write_backs`].
    write_backs: RefCell<Vec<(UnitId, WriteBack)>>,
    /// The game's hireling lists (`world/hirelings.md` §5), lent by the
    /// host that holds them for a call (the 0x61 give's hireling,
    /// owner test and swap, [`merc`]); read only here. `None` (the
    /// default): those seams stay the rest's.
    pub hirelings: Option<crate::world::hirelings::HirelingState>,
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
    /// Book / scroll spell (`0x00627F80`). Not asked by [`InvDesk`]
    /// (`queries`: item data +0x3E). Default: 0.
    fn spell(&self, item: Guid) -> i32 {
        0
    }

    // ---- inventory model seams (`inventory.md` §2–§5) ---------------------

    /// Trade hook `0x00568770` (page 2; multiplayer trade).
    fn trade_hook(&mut self, owner: Owner, item: Guid) {}
    /// The item is active on the unit (`0x00625820`).
    fn item_active_on(&self, item: Guid, unit: Owner) -> bool;
    /// The item's own contribution to a unit stat (`0x0062B450`).
    fn own_contribution(&self, item: Guid, unit: Owner, stat: u16) -> i32;
    /// Level requirement (`0x0062B5B0`, §4.8). Not asked by [`InvDesk`]
    /// (`queries` gathers the values). Default: 0.
    fn level_requirement(&self, item: Guid, unit: Owner) -> i32 {
        0
    }
    /// Two-handed (`0x006289C0`). Not asked by [`InvDesk`] (`queries`:
    /// items `2handed`). Default: no.
    fn two_handed(&self, item: Guid) -> bool {
        false
    }
    /// One-or-two-handed for the unit (`0x0062A1E0`).
    fn one_or_two_handed(&self, unit: Owner, item: Guid) -> bool;
    /// Ammo type (`0x0062E6F0`): an itemtypes row. Not asked by
    /// [`InvDesk`] (`queries`: the primary type's `shoots`). Default: none.
    fn ammo_type(&self, item: Guid) -> Option<i16> {
        None
    }
    /// Allowed location (`0x0062FDF0`).
    fn has_allowed_location(&self, item: Guid) -> bool;
    /// Quiver-type item (`0x00628480`).
    fn quiver_kind(&self, item: Guid) -> bool;

    // ---- player data (`world/npc.md` §2; the interact info is the unit
    // record's, `units::record::InteractInfo`) ---------------------------

    /// Player data +0x4C.
    fn player_data_4c(&self, player: Owner) -> u32;
    /// Player data +0x50.
    fn player_data_50(&self, player: Owner) -> u32;
    /// The player talks to the NPC (interaction list state 0).
    fn npc_talking(&self, npc: Owner, player: Owner) -> bool;
    /// The player-trade part of `0x00567620` (multiplayer).
    fn player_trade_gate(&self, player: Owner) -> Option<bool>;

    // ---- equipment seams ([`equip_rules`]; `inventory.md` §5.5–§5.8,
    // `properties.md` §11, §13). The skill list is the skills spec's
    // (`skills/use.md` §2); defaults: no skill, no row, nothing sent.

    /// books `scrollskill` (`scroll`) or `bookskill` of the spell index
    /// `spell` (`0x006374B0`); `None`: no row.
    fn book_skill(&self, spell: i32, scroll: bool) -> Option<i32> {
        None
    }
    /// Stat unlink `0x0063D2B0`.
    fn stat_unlink(&mut self, owner: Owner, item: Guid) {}
    /// §5.7 step 8: S→C 0x48 for the unit (`0x0053D3C0`).
    fn send_unit_refresh(&mut self, unit: Owner) {}
    /// The quantity of the unit's skill `skill`, owner −1 (`0x006439B0`,
    /// skill +0x30); `None`: absent.
    fn skill_quantity(&self, unit: Owner, skill: i32) -> Option<i32> {
        None
    }
    /// `0x00645120`.
    fn set_skill_quantity(&mut self, unit: Owner, skill: i32, q: i32) {}
    /// `0x00570080`.
    fn learn_skill(&mut self, unit: Owner, skill: i32) {}
    /// The left / right mouse skill (`0x00620190` / `0x006201D0`).
    fn mouse_skill(&self, unit: Owner, left: bool) -> Option<(i32, i32)> {
        None
    }
    /// `0x005701B0`.
    fn select_skill(&mut self, unit: Owner, left: bool, skill: (i32, i32)) {}
    /// `0x006439B0` with (id, owner).
    fn has_skill_owned(&self, unit: Owner, skill: (i32, i32)) -> bool {
        false
    }
    /// `0x00647960` (`skills/use.md` §2). Default: 0 (usable).
    fn skill_use_state(&mut self, unit: Owner, skill: (i32, i32)) -> u8 {
        0
    }
    /// The skills.txt part of `0x0055C560` (`itypea1` > 0 is-a `thro`,
    /// `range` 2).
    fn throw_skill_row(&self, skill: i32) -> bool {
        false
    }
    /// Player data +0x70..+0x7C (§5.8). Default: (0, −1).
    fn saved_mouse_skill(&self, unit: Owner, left: bool) -> (i32, i32) {
        (0, -1)
    }
    fn set_saved_mouse_skill(&mut self, unit: Owner, left: bool, skill: (i32, i32)) {}
    /// The set bonuses `0x00660120` (`properties.md` §11).
    fn set_bonuses(&mut self, owner: Owner, item: Guid, state: u32) {}
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
        d.apply_write_backs();
        d.sync_in();
        d
    }

    /// The owner of a unit, or the "none" owner (type 6).
    pub fn owner_or_none(&self, u: UnitId) -> Owner {
        self.owner_of(u).unwrap_or(Owner {
            ty: Owner::NONE,
            guid: u32::MAX,
        })
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
            let d = self.state.items.entry(u).or_insert_with(|| {
                let mut d = InvItem::new(r.guid, it.record);
                // An item new to the model that lies on the ground (a
                // treasure drop, placed by the path code): its position
                // is the path's (`bitstream.md` §4.1 rule 2, static path
                // +0x0C / +0x10).
                if r.mode == u32::from(crate::items::moves::mode::GROUND) {
                    if let Some((x, y)) = self.econ.hooks.path_xy(u) {
                        (d.x, d.y) = (x, y);
                    }
                }
                d
            });
            d.guid = r.guid;
            d.record = it.record;
            d.mode = r.mode as u8;
            d.flags = it.flags;
            d.page = it.inv_page;
        }
        self.state.items.retain(|u, _| live.contains(u));
    }

    /// Writes the copied fields back to their owners (and the queued
    /// bit-stream write-backs).
    pub fn sync_out(&mut self) {
        self.apply_write_backs();
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
        self.flush_equip();
        Some(out)
    }

    /// Runs the queued equipment-rule calls, then the owner refreshes the
    /// inventory functions asked for. A host calls it when a call that
    /// ran the rules directly (not through [`InvDesk::with_inv`], e.g. a
    /// body remove's inventory pass) returns, so no refresh outlives its
    /// call.
    pub fn flush_equip(&mut self) {
        self.run_equip_queue();
        for r in std::mem::take(&mut self.state.refresh) {
            if let Some(o) = self.owner_of(r) {
                deferred::owner_refresh(self, o);
            }
        }
    }

    /// The update-list reset after the update pass (`0x00597B00`, §6.1
    /// rule 4: [`deferred::update_list_reset`]). The per-unit flags of the
    /// room clean-up `0x00553220` ([`deferred::room_cleanup`]) are the
    /// tick wiring's (`tick.md` §3 step 6).
    pub fn update_done(&mut self, owner: Owner) {
        self.apply_write_backs();
        deferred::update_list_reset(self, owner);
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
