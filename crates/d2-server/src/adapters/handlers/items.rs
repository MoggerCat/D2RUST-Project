// Spec: specs/world/cube.md §1, §2; specs/sim/intents-events.md §2.4
//! Item, inventory and cube intents (`docs/HANDOFF.md` §2 step 3).
//!
//! Only ids whose behaviour a written spec owns get a handler; every
//! other item id stays the [`super::super::SimGame`] stub. The table
//! [`ITEM_IDS`] lists each item-related C→S id with its owner spec
//! (`docs/handoff/server-items.md` §2).
//!
//! The handlers run the `d2-sim` modules on their real providers: the
//! cube (`d2_sim::world::cube`) through the economy wiring
//! (`d2_sim::wiring::economy::EconomyCube`: items, stats, unit records,
//! item creation). What the cube asks for that no written spec provides
//! is either staged here ([`Staged`]: interaction state, inventory lists,
//! the local date, sound events; the caller fills them, as with
//! [`super::super::UnitFacts`]) or goes to [`ItemPending`], whose
//! provider is the unwritten owner spec (placement, removal, sockets, the
//! item routines no items spec writes, quest hooks).

mod cube_world;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_sim::game::Game;
use d2_sim::items::ItemTables;
use d2_sim::stats::{StatHost, StatLists};
use d2_sim::units::hooks::{UnitData, UnitHooks};
use d2_sim::units::lifecycle::LifecycleHooks;
use d2_sim::units::record::Units;
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::{Economy, EconomyCube, EconomyError, GameFields, ItemStore};
use d2_sim::world::cube::CubeData;

use super::super::UnitFacts;
use crate::buffers::QueueError;
use crate::seams::{ClientId, MessageSink, ResultCode};

pub use cube_world::CreationInfo;

/// C→S 0x2A ItemToCube (`cube.md` §2).
pub const ITEM_TO_CUBE: u8 = 0x2A;
/// C→S 0x4F ClickButton (`cube.md` §1: the cube's buttons).
pub const CLICK_BUTTON: u8 = 0x4F;

/// Every item-related C→S id (`client-messages.tsv`) and the spec that
/// owns its behaviour; `None`: no written spec does, the id stays a stub.
/// Vendor ids (0x32–0x38) belong to `world/vendors.md` and the world
/// handlers, not here.
pub const ITEM_IDS: &[(u8, Option<&str>)] = &[
    (0x16, None), // PickItem
    (0x17, None), // DropItem
    (0x18, None), // InsertItemInBuffer
    (0x19, None), // RemoveItemFromBuffer
    (0x1A, None), // EquipItem
    (0x1B, None), // Swap2HandedItem
    (0x1C, None), // RemoveBodyItem
    (0x1D, None), // SwapCursorWithBody
    (0x1E, None), // Swap1HWith2H
    (0x1F, None), // SwapCursorBufferItem
    (0x20, None), // UseGridItem
    (0x21, None), // StackItems
    (0x22, None), // UnstackItems
    (0x23, None), // ItemToBelt
    (0x24, None), // ItemFromBelt
    (0x25, None), // SwitchBeltItem
    (0x26, None), // UseBeltItem
    (0x27, None), // UseItemAction
    (0x28, None), // SocketItem
    (0x29, None), // ScrollToBook
    (ITEM_TO_CUBE, Some("specs/world/cube.md §2")),
    // `cube.md` §10 routes 0x4C to the item-use spec (not written).
    (0x4C, None), // Transmogrify
    (CLICK_BUTTON, Some("specs/world/cube.md §1")),
    (0x50, None), // DropGold
    (0x61, None), // MercItem
    (0x63, None), // ItemToBeltShift
];

/// Errors the handler result code cannot carry, in order.
#[derive(Debug, PartialEq, Eq)]
pub enum ItemError {
    /// A provider's error inside a seam call (`EconomyCube::errors`).
    Economy(EconomyError),
    /// Queueing a message for the acting client failed.
    Sink(QueueError),
    /// A module asked to send to a player other than the acting one.
    OtherPlayer(UnitId),
}

/// The unit hooks of the item world: every hook keeps its `d2-sim`
/// default (the unit, stat-list and lifecycle bodies their specs own).
#[derive(Clone, Copy, Debug, Default)]
pub struct ItemHooks;

impl StatHost for ItemHooks {}
impl UnitHooks for ItemHooks {}
impl LifecycleHooks for ItemHooks {}

/// A player's interaction state (`cube.md` Inputs: player unit +0x64
/// GUID, +0x68 unit type, +0x6C active byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interaction {
    pub guid: u32,
    pub unit_type: u8,
    pub active: bool,
}

impl Interaction {
    /// After `0x00554190` (`cube.md` §1 row 0x17: GUID −1, type 6,
    /// active 0).
    pub const RESET: Self = Self {
        guid: u32::MAX,
        unit_type: 6,
        active: false,
    };
}

/// A player's inventory as the cube reads it: the item list in list
/// order (`cube.md` OQ 5: the order is the inventory spec's) and the
/// cursor item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub items: Vec<UnitId>,
    pub cursor: Option<UnitId>,
}

/// State no `d2-sim` module holds yet, staged by the caller until its
/// owner spec moves it into `d2-sim`; and what the handlers record for
/// it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Staged {
    pub interactions: BTreeMap<UnitId, Interaction>,
    pub inventories: BTreeMap<UnitId, Inventory>,
    /// `GetLocalTime` (day of month, day of week + 1): host input.
    pub local_date: (u8, u8),
    /// Sound events attached to players (`0x00553380`, `cube.md`
    /// Outputs), in order. Which message carries them is open (`cube.md`
    /// OQ 2), so none is queued.
    pub sounds: Vec<(UnitId, u8)>,
    /// Targeting resets (`0x0055BF50`, `cube.md` §2 step 3.1), in order.
    /// TODO(server-messages.tsv layout of 0x3F; the argument of
    /// `0x0044BE50`): the reset's 0x3F is not queued.
    pub targeting_resets: Vec<UnitId>,
}

/// The cube's calls whose owner spec is not written; the provider is
/// that spec's code. Messages they queue go to `out` (the acting
/// client's, in call order).
pub trait ItemPending {
    /// `0x0055FA40` (inventory / UI owner, `cube.md` OQ 8).
    fn inventory_pass(&mut self, player: UnitId, out: &mut Vec<Vec<u8>>);
    /// `0x00560200(game, player, id, 0, 0, 1, 1, 0)` (inventory spec);
    /// true when placed.
    fn place(
        &mut self,
        inv: &mut Inventory,
        player: UnitId,
        item: UnitId,
        out: &mut Vec<Vec<u8>>,
    ) -> bool;
    /// `cube.md` §8 step 1 for one item before the free: queue 0x9D
    /// (action 5, flags 0x20, page 3; bytes open, `cube.md` OQ 1) and
    /// remove it from the inventory (`0x0055DF10`, inventory spec).
    fn remove_cube_item(
        &mut self,
        inv: &mut Inventory,
        player: UnitId,
        item: UnitId,
        out: &mut Vec<Vec<u8>>,
    );
    /// Items socketed into `item`, in order (inventory spec).
    fn socketed(&self, item: UnitId) -> Vec<UnitId>;
    /// `0x0055A2A0` (no items spec writes it).
    fn duplicate(&mut self, item: UnitId, fillers: bool) -> Option<UnitId>;
    /// `0x005C1BC0(item, prefix)` (open question WE6 of the economy
    /// wiring).
    fn tempered_affix(&mut self, item: UnitId, prefix: bool) -> u16;
    /// `0x00558C50`.
    fn drop_runeword_stats(&mut self, item: UnitId);
    /// `0x0055F900`.
    fn repair(&mut self, item: UnitId);
    /// `0x0055FE80`.
    fn recharge(&mut self, item: UnitId);
    /// `hst ` / `qf2 ` hooks (Act II / III quests, not specified).
    fn quest_item_hook(&mut self, player: UnitId, item: UnitId, code: [u8; 4]);
    /// Kind 1 (`world/quests.md` §8.4 through the game's quest control).
    fn cow_portal(&mut self, player: UnitId) -> bool;
}

/// One game's item state behind the server: what the economy wiring
/// works on, the cube tables, the staged state and the pending provider.
///
/// TODO(wired single-player host, `docs/HANDOFF.md` §2 step 4): the
/// unit records and stat lists here are the item world's own; the
/// action wiring's `ActionSim` holds another pair. One game needs one.
pub struct ItemWorld {
    pub units: Units,
    pub stats: StatLists,
    pub data: UnitData,
    pub hooks: ItemHooks,
    pub fields: GameFields,
    pub tables: ItemTables,
    pub items: ItemStore,
    pub cube: CubeData,
    pub staged: Staged,
    /// Player data item creation reads, by player unit (`generation.md`
    /// §9 step 5; player data is not in `d2-sim`).
    pub creation: BTreeMap<UnitId, CreationInfo>,
    pub pending: Box<dyn ItemPending + Send + Sync>,
    pub errors: Vec<ItemError>,
}

impl ItemWorld {
    /// The economy over this world and `game`.
    pub fn economy<'a>(&'a mut self, game: &'a mut Game) -> Economy<'a, ItemHooks> {
        Economy {
            game,
            units: &mut self.units,
            stats: &mut self.stats,
            data: &self.data,
            hooks: &mut self.hooks,
            fields: &mut self.fields,
            tables: &self.tables,
            items: &mut self.items,
        }
    }
}

/// What a handler borrows from [`super::super::SimGame`].
pub struct ItemView<'a> {
    /// The acting client's player unit.
    pub player: UnitId,
    pub game: &'a mut Game,
    pub world: &'a mut ItemWorld,
    /// The staged unit facts (act, position, owner).
    pub facts: &'a BTreeMap<UnitId, UnitFacts>,
}

fn result_code(r: u32) -> ResultCode {
    match r {
        0 => ResultCode::Done,
        1 => ResultCode::Refused,
        2 => ResultCode::Invalid,
        _ => ResultCode::Malformed,
    }
}

/// The handler of an item id this module owns, after the dispatcher's
/// gate and size check (`intents-events.md` §2.3–§2.4). `None`: not an
/// id with a handler here (or a 0x4F button the cube does not own), so
/// the caller keeps its stub.
pub fn handle(
    view: Option<ItemView<'_>>,
    client: ClientId,
    msg: &[u8],
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let id = *msg.first()?;
    if id != ITEM_TO_CUBE && id != CLICK_BUTTON {
        return None;
    }
    let ItemView {
        player,
        game,
        world,
        facts,
    } = view?;
    let ItemWorld {
        units,
        stats,
        data,
        hooks,
        fields,
        tables,
        items,
        cube,
        staged,
        creation,
        pending,
        errors,
    } = world;
    let mut econ = Economy {
        game,
        units,
        stats,
        data,
        hooks,
        fields,
        tables,
        items,
    };
    let mut info = cube_world::InfoRest(creation);
    let mut w = cube_world::ServerCube::new(
        EconomyCube::new(&mut econ, &mut info),
        staged,
        pending.as_mut(),
        facts,
        player,
    );
    let code = if id == ITEM_TO_CUBE {
        Some(cube.put_in(&mut w, player, msg))
    } else {
        // 0x4F: button u16 at +1 (size 7 already checked).
        let button = u16::from_le_bytes([msg[1], msg[2]]);
        cube.click_button(&mut w, player, button)
    };
    let (sent, errs) = w.finish();
    errors.extend(errs);
    for m in sent {
        if let Err(e) = out.queue(client, &m) {
            errors.push(ItemError::Sink(e));
        }
    }
    code.map(result_code)
}
