// Spec: specs/world/npc.md, specs/world/vendors.md, specs/world/waypoints.md, specs/world/quests.md, specs/world/objects.md §7.1, specs/sim/intents-events.md §2.4, §3.2
//! The world intent handlers: NPC interaction (`world/npc.md` §2–§4, §6,
//! §7), vendors (`world/vendors.md` §5.5, §7, §8), waypoints
//! (`world/waypoints.md` §6), quests (`world/quests.md` §1.7, §6.2,
//! §7.3) and the C→S 0x13 object case (`world/waypoints.md` §5.2,
//! `world/objects.md` §7.1).
//!
//! The dispatcher has run the gate and the exact-size check
//! (`intents-events.md` §2.3, §2.4 rule 1) when [`handle`] is called.
//! Every field check, refusal code and S→C message is the `d2-sim`
//! module's: a handler here finds the client's player unit, hands the
//! message to the module through the game's [`WorldHost`], queues what
//! the module sent to the clients of the receiving players, and maps
//! the module's result to a [`ResultCode`].
//!
//! [`WORLD_IDS`] lists every world-related C→S id with its owner spec.
//! An id with no written owner, or whose system the host does not
//! provide, stays a stub ([`handle`] returns `None`).
//!
//! [`WorldHost`] is the one host trait of a game: besides the world
//! systems it carries the cube ([`WorldHost::cube`], `handlers::items`)
//! and the skill handlers ([`WorldHost::skill`], `handlers::skills`).
//! Hosts: [`NoWorld`] (nothing), [`ActionWorld`] (the systems on the
//! action wiring alone: waypoints, skills), [`WiredWorld`] (the wired
//! single-player host: those, plus the economy, NPCs, vendors, quests
//! and the cube on the same unit world).

mod action;
mod gap_items;
mod hireling_drive;
mod hireling_host;
mod item_approach;
mod item_save;
mod npc_approach;
mod wired;

#[cfg(test)]
pub(crate) mod tests;

pub use action::{ActionEvents, ActionWorld, Outbox, ProcessState};
pub use gap_items::HirelingBlock;
pub use item_save::LoadedItems;
pub use wired::{Parts, TradeRest, WiredWorld};

use d2_sim::game::Game;
use d2_sim::tick::EventDispatch;
use d2_sim::units::{ClientId as SimClient, UnitId};
use d2_sim::wiring::action::ObjectCase;
use d2_sim::world::npc::{NpcControl, NpcError, NpcVendors, NpcWorld};
use d2_sim::world::quests::{QuestControl, QuestError, QuestWorld};
use d2_sim::world::vendors::gamble::identify_gamble;
use d2_sim::world::vendors::price::PriceFatal;
use d2_sim::world::vendors::trade::{buy, repair, sell, BuyMsg, RepairMsg, SellMsg};
use d2_sim::world::vendors::{VendorRecord, VendorTables, VendorWorld};
use d2_sim::world::waypoints::{ArrivalList, WaypointData, WaypointError, WaypointWorld};

use super::super::character::{self, StartItemWorld, StartPlace};
use super::super::SimGame;
use super::items::moves::MoveCall;
use super::items::moves::{take_sent as inv_take_sent, InvParts, MoveRest, StagedPlace};
use super::items::CubeCall;
use super::player::{Outcome as PlayerOutcome, Run as PlayerRun};
use super::skills::{Call as SkillCall, Handled as SkillHandled};
use super::walk::{WalkCall, WalkResult};
use crate::buffers::QueueError;
use crate::seams::{ClientId, MessageSink, ResultCode};
use d2_sim::items::inventory::{InvTables, UnitKind};
use d2_sim::items::moves::{InventoryOps, MovePending, MoveUnits, Owner, Spot};
use d2_sim::items::ItemRequest;
use d2_sim::items::{flag, q, stat as istat, ItemStats, ListKey};
use d2_sim::units::lifecycle::LifecycleHooks;
use d2_sim::units::RoomId;
use d2_sim::wiring::economy::quest_reward::create_reward;
use d2_sim::wiring::economy::ItemSpawn;
use d2_sim::wiring::economy::{find_list, Economy, StatCtx, UnitStats};
use d2_sim::wiring::inventory::InvRest;
use std::collections::BTreeMap;

/// Where a world-related C→S id's behaviour is specified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Handled here, by the named `d2-sim` system.
    Implemented(System),
    /// Owned by a spec another handler module implements.
    OtherModule(&'static str),
    /// No written spec owns the handler (or the part named): stub.
    NoOwner,
}

/// The `d2-sim` world system an id goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Npc,
    Vendors,
    Waypoints,
    Quests,
    /// The object case of 0x13 (unit type 2; [`route`]).
    Objects,
    /// The tile case of 0x13 (unit type 5, a level warp; [`route`]).
    /// PROVISIONAL (REC-99).
    Warps,
}

/// Every world-related C→S id (`client-messages.tsv`): (id, owner spec
/// section, status). The order is the id order.
pub const WORLD_IDS: &[(u8, &str, Status)] = &[
    // Unit type 1 (`npc.md` §2); type 2 is the object case
    // (`waypoints.md` §5.2 → `objects.md` §7.1, `System::Objects`,
    // chosen by `route`); other types to no written spec: stubs.
    (0x13, "world/npc.md §2", Status::Implemented(System::Npc)),
    (
        0x2A,
        "world/cube.md (handlers/items.rs)",
        Status::OtherModule("items"),
    ),
    (0x2F, "world/npc.md §3", Status::Implemented(System::Npc)),
    (0x30, "world/npc.md §3", Status::Implemented(System::Npc)),
    (
        0x31,
        "world/quests.md §7.3",
        Status::Implemented(System::Quests),
    ),
    (
        0x32,
        "world/vendors.md §7.1",
        Status::Implemented(System::Vendors),
    ),
    (
        0x33,
        "world/vendors.md §7.2",
        Status::Implemented(System::Vendors),
    ),
    (0x34, "world/npc.md §6", Status::Implemented(System::Npc)),
    (
        0x35,
        "world/vendors.md §8.1",
        Status::Implemented(System::Vendors),
    ),
    (0x36, "world/npc.md §7.3", Status::Implemented(System::Npc)),
    (
        0x37,
        "world/vendors.md §5.5",
        Status::Implemented(System::Vendors),
    ),
    (0x38, "world/npc.md §4", Status::Implemented(System::Npc)),
    // `quests.md` §9.4 names the item checks but not their result codes,
    // and `read_clue` has no item seam: stub.
    (0x3E, "world/quests.md §9.4 (partial)", Status::NoOwner),
    (
        0x3F,
        "sim/intents-events.md §9 r5 (handlers/player.rs)",
        Status::OtherModule("player"),
    ),
    (
        0x40,
        "world/quests.md §6.2",
        Status::Implemented(System::Quests),
    ),
    (
        0x44,
        "world/quests-act2.md §8.6 (entry: sim/intents-events.md §9 r7, handlers/player.rs)",
        Status::OtherModule("player"),
    ),
    (
        0x46,
        "sim/intents-events.md §9 r8 (handlers/player.rs)",
        Status::OtherModule("player"),
    ),
    (
        0x47,
        "sim/intents-events.md §9 r8 (handlers/player.rs)",
        Status::OtherModule("player"),
    ),
    (
        0x49,
        "world/waypoints.md §6",
        Status::Implemented(System::Waypoints),
    ),
    (
        0x4C,
        "world/cube.md §10: Transmogrify `0x0056C6A0`, owner the item-use spec (not written; which item the always-sent 0x3F names is not stated); handlers/items.rs takes only 0x2A and 0x4F",
        Status::NoOwner,
    ),
    (
        0x4D,
        "sim/intents-events.md §9 r11 (handlers/player.rs)",
        Status::OtherModule("player"),
    ),
    (
        0x4F,
        "world/cube.md (handlers/items.rs)",
        Status::OtherModule("items"),
    ),
    (
        0x58,
        "world/quests.md §1.7",
        Status::Implemented(System::Quests),
    ),
    // §9 rule 1 names `monsters/ai.md` §9.9; the AI params are
    // `monsters/ai-bodies.md` §9.9's, the handler's entry is not written.
    (
        0x59,
        "monsters/ai-bodies.md §9.9 (entry not written)",
        Status::NoOwner,
    ),
    (0x62, "world/npc.md §7.4", Status::Implemented(System::Npc)),
];

/// The system that handles `id` here, if any.
pub fn system(id: u8) -> Option<System> {
    WORLD_IDS.iter().find_map(|&(i, _, s)| match s {
        Status::Implemented(sys) if i == id => Some(sys),
        _ => None,
    })
}

/// The system of one message: [`system`] of its id, except 0x13 of size
/// 9 with unit type 2 (u32 @1), the object case ([`System::Objects`],
/// `waypoints.md` §5.2).
pub fn route(msg: &[u8]) -> Option<System> {
    let id = *msg.first()?;
    if id == 0x13 && msg.len() == 9 && msg[1..5] == 2u32.to_le_bytes() {
        return Some(System::Objects);
    }
    if id == 0x13 && msg.len() == 9 && msg[1..5] == 5u32.to_le_bytes() {
        return Some(System::Warps);
    }
    system(id)
}

/// A fatal assert of the original (or a sink failure) met by a world
/// handler. 1.14d ends the game; d2rs records it with the host
/// ([`WorldHost::fault`]) and the handler returns [`ResultCode::Malformed`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorldError {
    #[error(transparent)]
    Npc(#[from] NpcError),
    #[error(transparent)]
    Waypoint(#[from] WaypointError),
    #[error(transparent)]
    Quest(#[from] QuestError),
    #[error(transparent)]
    Price(#[from] PriceFatal),
    #[error(transparent)]
    Move(#[from] d2_sim::items::moves::MoveFatal),
    #[error("sink: {0}")]
    Sink(String),
}

impl From<QueueError> for WorldError {
    fn from(e: QueueError) -> Self {
        WorldError::Sink(e.to_string())
    }
}

/// A [`WorldError`] with the message that met it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldFault {
    pub client: ClientId,
    pub id: u8,
    pub error: WorldError,
}

/// One call into the NPC module with its seams (`NpcWorld + NpcVendors`).
pub trait NpcCall {
    type Out;
    fn call<W: NpcWorld + NpcVendors>(self, ctl: &mut NpcControl, w: &mut W) -> Self::Out;
}

/// One call into the vendors module. `records`: the vendor part of every
/// NPC record (`vendors.md` design point 1: one [`VendorRecord`] per
/// `npc.md` §1 record, found by class).
pub trait VendorCall {
    type Out;
    fn call<W: VendorWorld>(
        self,
        tables: &VendorTables,
        records: &mut [VendorRecord],
        w: &mut W,
    ) -> Self::Out;
}

/// One call into the waypoint module (with the object control's arrival
/// list, `waypoints.md` §7.1).
pub trait WaypointCall {
    type Out;
    fn call<W: WaypointWorld>(
        self,
        data: &WaypointData,
        arrivals: &mut ArrivalList,
        w: &mut W,
    ) -> Self::Out;
}

/// One call into the quest module.
pub trait QuestCall {
    type Out;
    fn call<W: QuestWorld>(self, ctl: &mut QuestControl, w: &mut W) -> Self::Out;
}

/// The systems of a game beyond its event dispatch and the providers of
/// their seams, as the handlers reach them: the world systems, the cube
/// and the skill handlers. `D` is the game's event dispatch (it owns the
/// unit side, e.g. `d2_sim::wiring::action::ActionSim`).
///
/// A method returning `None` means the game has no provider for that
/// system: the ids stay stubs. Every seam `send` must be kept in order
/// and handed back by [`WorldHost::take_sent`] (after a handler, and
/// after each tick).
#[allow(unused_variables)]
pub trait WorldHost<D> {
    fn npc<C: NpcCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        None
    }
    fn vendors<C: VendorCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        None
    }
    fn waypoints<C: WaypointCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        None
    }
    fn quests<C: QuestCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        None
    }
    /// The C→S 0x13 object case (`waypoints.md` §5.2,
    /// `objects.md` §7.1) by `player` on the object with `guid`, on the
    /// host's object state.
    fn objects(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        guid: u32,
    ) -> Option<ObjectCase> {
        None
    }
    /// The C→S 0x13 tile case (a level warp, `path-placement.md` §12.2;
    /// PROVISIONAL, REC-99) by `player` on the tile with `guid`: the
    /// result code. `None`: no provider.
    fn warp_tile(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        guid: u32,
    ) -> Option<u32> {
        None
    }
    /// The Town Portal scroll or tome of `player` was used (REC-117,
    /// `wiring::action::town_portal`): make the portal pair. `false`:
    /// nothing was created.
    fn town_portal(&mut self, game: &mut Game, events: &mut D, player: UnitId) -> bool {
        false
    }
    /// The run of `walk.0` to the ground item `walk.1` a pick-up asked
    /// for (`inventory-moves.md` §7.1 step 2, `0x00548A50`; cursor flag
    /// `walk.2`), with the pick-up on arrival. Default: nothing.
    fn item_walk(&mut self, game: &mut Game, events: &mut D, walk: (UnitId, UnitId, bool)) {}
    /// The cube (`handlers::items`) on the host's economy.
    fn cube<C: CubeCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        None
    }
    /// The item moves and the deferred item messages
    /// (`handlers::items::moves`) on the host's economy and inventories.
    fn moves<C: MoveCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        None
    }
    /// A weapon switch of `player` went through (C→S 0x60): the host's
    /// per-player switch state follows. Default: none.
    fn weapon_switched(&mut self, events: &mut D, player: UnitId) {}
    /// The skill handlers (`handlers::skills`).
    fn skill(&mut self, call: SkillCall<'_, D>) -> Option<SkillHandled> {
        None
    }
    /// The small client-intent handlers of `intents-events.md` §9
    /// (`handlers::player`) on the host's game state.
    fn player(
        &mut self,
        game: &mut Game,
        events: &mut D,
        run: PlayerRun<'_>,
    ) -> Option<PlayerOutcome> {
        None
    }
    /// The path positions (sub-tiles) of the listed units that have a
    /// path, read at the end of each tick to move the staged
    /// [`UnitFacts`](super::UnitFacts) the point-message parser reads
    /// (`intents-events.md` §2.4 rule 3). Default: none (the caller
    /// stages positions itself).
    fn unit_positions(&mut self, events: &mut D, units: &[UnitId]) -> Vec<(UnitId, (i32, i32))> {
        Vec::new()
    }
    /// The walk / run handlers (`handlers::walk`) on the path provider.
    fn walk(&mut self, game: &mut Game, events: &mut D, call: WalkCall) -> Option<WalkResult> {
        None
    }
    /// The live gate fields of a player (`intents-events.md` §2.3 rule 3:
    /// unit mode, state 0x36), read after each tick to refresh the staged
    /// [`super::super::PlayerFields`] (a dead player's gate opens 0x41).
    /// `None`: the host has none; the staged fields stay.
    fn player_gate(
        &mut self,
        game: &Game,
        events: &mut D,
        unit: UnitId,
    ) -> Option<crate::seams::PlayerGate> {
        None
    }
    /// The live act and position of `unit` the point / unit parser reads
    /// (`intents-events.md` §2.4 rules 3–4), from the game's own unit
    /// (its room's act, its path position). `None`: the host has none
    /// (the caller's staged [`super::super::UnitFacts`] are used).
    fn live_facts(
        &mut self,
        game: &Game,
        events: &mut D,
        unit: UnitId,
    ) -> Option<super::super::UnitFacts> {
        None
    }
    /// The client vitals sync (`combat/vitals.md` §5) for one client at
    /// the end of a flush: the messages to send it, in order. `None`:
    /// the host has no sync (or it is off), nothing runs. `staged`: the
    /// host's position of a player without a path record; `queued`: the
    /// client has a queued buffer (§5.1 rule 2).
    fn vitals_sync(
        &mut self,
        game: &mut Game,
        events: &mut D,
        client: SimClient,
        staged: (u16, u16),
        queued: bool,
    ) -> Option<Vec<Vec<u8>>> {
        None
    }
    /// The messages the seams sent since the last take, in send order:
    /// (receiving player unit, bytes).
    fn take_sent(&mut self, events: &mut D) -> Vec<(UnitId, Vec<u8>)> {
        Vec::new()
    }
    /// The tick's steps (`d2_sim::tick::tick` with `events` as the step
    /// hooks); a host may lend its own parts to the hooks around them
    /// (`WiredWorld`: the quest control, so quest object inits run inside
    /// their allocation).
    fn run_tick(&mut self, game: &mut Game, events: &mut D)
    where
        D: EventDispatch + d2_sim::tick::TickHooks,
    {
        d2_sim::tick::tick(game, events);
    }
    /// Runs after the tick's steps, before its sends are taken (the
    /// quest routes the tick queued, `WiredWorld`).
    fn after_tick(&mut self, game: &mut Game, events: &mut D) {}
    /// The host's millisecond clock (`Intents::set_host_tick`): the
    /// object code's `GetTickCount` input (`objects.md` edge case 9).
    fn host_tick(&mut self, events: &mut D, ms: u32) {}
    /// The host calls the game's quest rules raised since the last take
    /// (`quests-helpers.md` §6: `QuestControl::take_host_requests`), in
    /// call order. A host without quests raises none.
    fn take_host_requests(&mut self) -> Vec<d2_sim::world::quests::HostRequest> {
        Vec::new()
    }
    /// Records a fatal path (see [`WorldError`]).
    fn fault(&mut self, fault: WorldFault);
}

/// A game without world systems: every world id stays a stub.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoWorld;

impl<D> WorldHost<D> for NoWorld {
    /// Never reached: no system runs, so nothing can fail.
    fn fault(&mut self, _fault: WorldFault) {}
}

/// The handler result of a module call: the result code, or `None` when
/// the message is not this module's (0x13 for a unit type other than 1).
type Run = Result<Option<u32>, WorldError>;

fn code(r: u32) -> ResultCode {
    match r {
        0 => ResultCode::Done,
        1 => ResultCode::Refused,
        2 => ResultCode::Invalid,
        // The modules return 0–3 only (`intents-events.md` §2.3).
        _ => ResultCode::Malformed,
    }
}

/// The world handler for one dispatched message (`intents-events.md`
/// §2.4, §4 rule 1). `None`: not a world id handled here, no player, the
/// host has no provider for its system, or another module's unit type
/// (0x13): the caller keeps its stub.
pub fn handle<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    client: ClientId,
    msg: &[u8],
    size: usize,
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let id = *msg.first()?;
    // Every handled id has a fixed size ≤ 17 (`client-messages.tsv`), so
    // the drained copy is the whole message.
    let msg = &msg[..size.min(msg.len())];
    let sys = route(msg)?;
    let player = sim.player_of(client)?;
    let (game, events) = (&mut sim.game, &mut sim.events);
    let run = match sys {
        System::Npc => sim.world.npc(game, events, NpcRun { player, msg }),
        System::Vendors => sim.world.vendors(game, events, VendorRun { player, msg }),
        System::Waypoints => sim
            .world
            .waypoints(game, events, WaypointRun { player, msg }),
        System::Quests => sim.world.quests(game, events, QuestRun { player, msg }),
        System::Warps => {
            let guid = u32::from_le_bytes([msg[5], msg[6], msg[7], msg[8]]);
            sim.world
                .warp_tile(game, events, player, guid)
                .map(|c| Ok(Some(c)))
        }
        System::Objects => {
            let guid = u32::from_le_bytes([msg[5], msg[6], msg[7], msg[8]]);
            match sim.world.objects(game, events, player, guid)? {
                ObjectCase::Code(c) => Some(Ok(Some(c))),
                // Operate 23 (`waypoints.md` §5.2) on the host's
                // waypoints; without them the id stays a stub.
                ObjectCase::Waypoint(_) => {
                    sim.world
                        .waypoints(game, events, WaypointOperate { player, guid })
                }
            }
        }
    }?;
    let sent = sim.world.take_sent(&mut sim.events);
    let mut faults = Vec::new();
    for (unit, bytes) in sent {
        // §3.2 rule 1: a player without a client receives nothing.
        if let Some(c) = sim.client_of(unit) {
            if let Err(e) = out.queue(c, &bytes) {
                faults.push(WorldError::from(e));
            }
        }
    }
    let result = match run {
        Ok(None) => None,
        Ok(Some(r)) => Some(code(r)),
        Err(e) => {
            faults.push(e);
            Some(ResultCode::Malformed)
        }
    };
    let failed = !faults.is_empty();
    for error in faults {
        sim.world.fault(WorldFault { client, id, error });
    }
    if failed {
        return Some(ResultCode::Malformed);
    }
    result
}

struct NpcRun<'m> {
    player: UnitId,
    msg: &'m [u8],
}

impl NpcCall for NpcRun<'_> {
    type Out = Run;
    fn call<W: NpcWorld + NpcVendors>(self, ctl: &mut NpcControl, w: &mut W) -> Run {
        let (p, m) = (self.player, self.msg);
        Ok(match m[0] {
            0x13 => ctl.interact(w, p, m)?,
            0x2F => Some(ctl.chat_open(w, p, m)),
            0x30 => Some(ctl.chat_close(w, p, m)),
            0x34 => Some(ctl.identify(w, p, m)),
            0x36 => Some(ctl.hire(w, p, m)?),
            0x38 => Some(ctl.menu_action(w, p, m)?),
            0x62 => Some(ctl.resurrect(w, p, m)),
            _ => None,
        })
    }
}

struct VendorRun<'m> {
    player: UnitId,
    msg: &'m [u8],
}

/// The vendor record of the NPC with GUID `npc`. When the NPC is missing
/// the trade functions refuse before reading the record (`vendors.md`
/// §7.1 text, §7.2 rules 1–2), so an empty scratch record stands in.
/// TODO(vendors.md design point 1): a class without a record is read as
/// an empty record, like a non-trader's (§1 rule 5).
fn record_of<'r, W: VendorWorld>(
    w: &W,
    records: &'r mut [VendorRecord],
    scratch: &'r mut VendorRecord,
    npc: u32,
) -> &'r mut VendorRecord {
    let class = w.npc_by_guid(npc).map(|n| w.npc_class(n));
    match class.and_then(|c| records.iter().position(|r| r.class == c)) {
        Some(i) => &mut records[i],
        None => scratch,
    }
}

impl VendorCall for VendorRun<'_> {
    type Out = Run;
    fn call<W: VendorWorld>(
        self,
        tables: &VendorTables,
        records: &mut [VendorRecord],
        w: &mut W,
    ) -> Run {
        let (p, m) = (self.player, self.msg);
        let mut scratch = VendorRecord::default();
        Ok(match m[0] {
            0x32 => match BuyMsg::parse(m) {
                Some(b) => {
                    let rec = record_of(w, records, &mut scratch, b.npc);
                    Some(buy(tables, rec, w, p, &b)?)
                }
                None => Some(3),
            },
            0x33 => match SellMsg::parse(m) {
                Some(s) => {
                    let rec = record_of(w, records, &mut scratch, s.npc);
                    Some(sell(tables, rec, w, p, &s)?)
                }
                None => Some(3),
            },
            // The handler `0x0054BB60` drops the routine's result: 0 for
            // every 17-byte message (`vendors.md` §8.1 rule 7).
            0x35 => match RepairMsg::parse(m) {
                Some(r) => {
                    repair(tables, w, p, &r)?;
                    Some(0)
                }
                None => Some(3),
            },
            0x37 => Some(identify_gamble(w, p, m)),
            _ => None,
        })
    }
}

struct WaypointRun<'m> {
    player: UnitId,
    msg: &'m [u8],
}

impl WaypointCall for WaypointRun<'_> {
    type Out = Run;
    fn call<W: WaypointWorld>(
        self,
        data: &WaypointData,
        arrivals: &mut ArrivalList,
        w: &mut W,
    ) -> Run {
        Ok(match self.msg[0] {
            0x49 => Some(data.take_or_close(w, arrivals, self.player, self.msg)?),
            _ => None,
        })
    }
}

/// Operate function 23 (`waypoints.md` §5.2) of the 0x13 object case.
///
/// TODO(waypoints.md §5.2): the 0x13 result after the operate is not
/// stated; read as 0.
struct WaypointOperate {
    player: UnitId,
    guid: u32,
}

impl WaypointCall for WaypointOperate {
    type Out = Run;
    fn call<W: WaypointWorld>(self, data: &WaypointData, _: &mut ArrivalList, w: &mut W) -> Run {
        let Some((object, facts)) = w.object(self.guid) else {
            return Ok(Some(1));
        };
        data.operate(w, object, &facts, self.player)?;
        Ok(Some(0))
    }
}

struct QuestRun<'m> {
    player: UnitId,
    msg: &'m [u8],
}

impl QuestCall for QuestRun<'_> {
    type Out = Run;
    fn call<W: QuestWorld>(self, ctl: &mut QuestControl, w: &mut W) -> Run {
        let (p, m) = (self.player, self.msg);
        Ok(match m[0] {
            0x31 => Some(ctl.quest_message(w, p, m)),
            // `0x0054C0C0`: size 1 (checked by the dispatcher), then
            // `0x00546040`. The spec names no result; read as 0.
            0x40 => {
                ctl.request_quest_data(w, p)?;
                Some(0)
            }
            0x58 => Some(ctl.quest_completed(w, p, m)),
            _ => None,
        })
    }
}

/// What the start items (`items/generation.md` §10.3) did on the wired
/// host ([`WiredWorld::start_items`]).
#[derive(Debug, Default)]
pub struct StartItems {
    /// Each created item and where it went, in creation order.
    pub items: Vec<(UnitId, StartPlace)>,
    /// What the inventory placements queued (receiving unit, bytes), in
    /// send order. Not sent by the join (`intents-events.md` §8.2 rule
    /// 3.5 is not wired); the caller decides.
    pub sent: Vec<(UnitId, Vec<u8>)>,
    /// Why no item was made: no charstats row, no inventory model, or an
    /// item creation error.
    pub faults: Vec<String>,
}

impl<R, S> WiredWorld<R, S> {
    /// What the inventory model queued outside a vendor or quest call
    /// (the quest objects' item removals, run on the loan): receiving
    /// unit, bytes.
    pub fn take_inventory_sent<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
    ) -> Vec<(UnitId, Vec<u8>)> {
        let Some(mut inv) = self.inventory.take() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        self.with_economy(game, events, |econ, _| {
            let mut d = inv.desk(econ);
            out = inv_take_sent(&mut d)
                .into_iter()
                .filter_map(|(u, b)| Some((u?, b)))
                .collect();
        });
        self.inventory = Some(inv);
        out
    }

    /// The start items `0x00534F10` (`items/generation.md` §10.3) of
    /// `player` on this host's economy and inventory model
    /// ([`character::start_items`] over [`WiredStart`]). The player's
    /// inventory is added to the model when it has none (the 1.14d unit
    /// allocation makes it; here the first item user does).
    pub fn start_items<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
    ) -> StartItems {
        let mut r = StartItems::default();
        let Some(mut inv) = self.inventory.take() else {
            r.faults
                .push("no inventory model (WiredWorld::inventory is None)".into());
            return r;
        };
        let extra = self.start_extra.clone();
        self.with_economy(game, events, |econ, _| {
            let Some((class, guid)) = econ.units.get(player).map(|u| (u.class, u.guid)) else {
                r.faults.push(format!("no player unit {player:?}"));
                return;
            };
            // d2rs-own, unverified: the play host never adds the player's
            // inventory (unit allocation `0x0063ABD0` has no caller there);
            // the quest items (a reward, a delete) need it without start
            // items (q-a4-quest-items, REC-235).
            if !inv.state.inventories.contains_key(&player) {
                inv.state
                    .add_inventory(player, UnitKind::Player { class: class as u8 }, guid);
            }
            let Some(vitals) = econ.hooks.vitals.clone() else {
                r.faults
                    .push("no vitals tables (charstats) on the action wiring".into());
                return;
            };
            let Some(cs) = vitals.charstats(class as i32) else {
                // §10.3: no row → nothing.
                return;
            };
            let mut slots = character::start_slots(cs).to_vec();
            // d2rs-own, unverified (REC-244): after the charstats slots,
            // one inventory copy of each extra code (loc 0, count 1).
            slots.extend(extra.iter().map(|&code| character::StartSlot {
                code,
                loc: 0,
                count: 1,
            }));
            let skills = econ.hooks.tables.skills.skills.len();
            let start_skill = Some(cs.startskill).filter(|&k| usize::from(k) < skills);
            let mut w = WiredStart {
                econ,
                inv: &mut inv,
                player,
                owner: Owner::player(guid),
                faults: Vec::new(),
            };
            r.items = character::start_items(&mut w, &slots, start_skill);
            r.faults.append(&mut w.faults);
            let mut d = inv.desk(econ);
            // The placements put the items on the player's update list
            // (`inventory-moves.md` §6.1 rule 1). The join sends that list
            // as the player's item messages (`intents-events.md` §8.2 rule
            // 3.5, `0x00597890`) and resets it (rule 3.10, `0x00597B00`),
            // so no tick's unit update sends them again. Here both run at
            // the start items' end: the load's later steps (start skill,
            // mouse skills) never touch the list. d2rs-own placement of
            // the two calls, unverified (no new-character join recorded;
            // PROVISIONAL REC-278).
            let owner = Owner::player(guid);
            let mut listed = Vec::new();
            if d.update_bits(owner) & 1 != 0 {
                match d2_sim::items::moves::update_list_pass(&mut d, guid, guid) {
                    Ok(m) => listed = m,
                    Err(e) => r.faults.push(format!("start items: update list: {e:?}")),
                }
                d.update_done(owner);
            }
            d.sync_out();
            r.sent = inv_take_sent(&mut d)
                .into_iter()
                .filter_map(|(u, b)| Some((u?, b)))
                .collect();
            r.sent.extend(listed.into_iter().map(|m| (player, m)));
        });
        self.inventory = Some(inv);
        r
    }
}

/// [`StartItemWorld`] on the wired host's economy and inventory model:
/// creation through the quest reward's `create_reward` (§10.1, §10.2 with
/// §10.3's arguments), the stat writes through the item's stat lists,
/// placement through an inventory desk per call.
pub struct WiredStart<'e, 'a, H> {
    pub econ: &'e mut Economy<'a, H>,
    pub inv: &'e mut InvParts,
    pub player: UnitId,
    pub owner: Owner,
    pub faults: Vec<String>,
}

impl<H: LifecycleHooks> WiredStart<'_, '_, H> {
    fn guid(&self, item: UnitId) -> u32 {
        self.econ.units.get(item).map_or(u32::MAX, |u| u.guid)
    }
    fn set_stat(&mut self, item: UnitId, id: u16, v: i32) {
        self.econ
            .with_stats(|ctx| UnitStats::new(ctx, item).set_base(id, 0, v));
    }
}

impl<H: LifecycleHooks> StartItemWorld for WiredStart<'_, '_, H> {
    fn create(&mut self, code: [u8; 4]) -> Option<UnitId> {
        // `create_reward`: level 0 → the player's base level (≥ 1),
        // quality 2, spawn mode 4, no sockets, not ethereal, no seeds, then
        // durability := max and page 0 (step 2.6 sets 72 again after the
        // placement, the same value).
        match create_reward(self.econ, self.player, code, 0, q::NORMAL) {
            Ok(i) => i,
            Err(e) => {
                self.faults.push(format!("start item {code:?}: {e:?}"));
                None
            }
        }
    }
    fn drop_class_skill_list(&mut self, item: UnitId) {
        self.econ.with_stats(|ctx| {
            let mut c = ctx.borrow_mut();
            let StatCtx { lists, host } = &mut *c;
            if let Some(l) = find_list(lists, item, ListKey::ITEM) {
                lists.unit_detach(*host, l);
                lists.free_plain(*host, l);
            }
        });
    }
    fn set_single_skill(&mut self, item: UnitId, skill: u16) {
        self.econ.with_stats(|ctx| {
            UnitStats::new(ctx, item).list_set(ListKey::ITEM, istat::ITEM_SINGLESKILL, skill, 1)
        });
    }
    fn stackable(&mut self, item: UnitId) -> bool {
        let g = self.guid(item);
        self.inv.desk(self.econ).stackable(g)
    }
    fn fill_stack(&mut self, item: UnitId) {
        let g = self.guid(item);
        let n = self.inv.desk(self.econ).max_stack(g);
        self.set_stat(item, istat::QUANTITY, n);
    }
    fn mark_start(&mut self, item: UnitId, loc: u8) {
        if let Some(i) = self.econ.items.get_mut(item) {
            i.flags |= flag::STARTITEM;
        }
        let d = self.inv.desk(self.econ);
        if let Some(i) = d.state.items.get_mut(&item) {
            i.body_loc = loc;
        }
    }
    fn beltable(&mut self, item: UnitId) -> bool {
        let g = self.guid(item);
        InventoryOps::beltable(&self.inv.desk(self.econ), g)
    }
    fn place_belt(&mut self, item: UnitId) -> bool {
        // d2rs-own, unverified: `0x0055E9B0(item, slot = item x, find 1)`
        // (`inventory-moves.md` §7.14) read as the free-slot search (§3.5)
        // then the slot placement (§3.7); the item's x is not read.
        let (o, g) = (self.owner, self.guid(item));
        let mut d = self.inv.desk(self.econ);
        match d.belt_free_slot(o, g) {
            Some(s) => d.belt_place(o, g, u32::from(s)),
            None => false,
        }
    }
    fn place_inventory(&mut self, item: UnitId) -> bool {
        if let Some(i) = self.econ.items.get_mut(item) {
            i.inv_page = 0;
        }
        let p = self.player;
        self.inv.desk(self.econ).place(p, item, (0, 0), true, true)
    }
    fn equip(&mut self, item: UnitId, loc: u8) -> bool {
        let (o, g) = (self.owner, self.guid(item));
        self.inv
            .desk(self.econ)
            .equip_from_cursor(o, g, loc, true)
            .0
    }
    fn quiver(&mut self, item: UnitId) -> bool {
        let g = self.guid(item);
        self.inv.desk(self.econ).quiver(g)
    }
    fn set_quantity(&mut self, item: UnitId, n: i32) {
        self.set_stat(item, istat::QUANTITY, n);
    }
    fn fill_durability(&mut self, item: UnitId) {
        // `0x00625E00`: 0 without a base stat 73, else its total.
        let max = self.econ.with_stats(|ctx| {
            let s = UnitStats::new(ctx, item);
            if s.base(istat::MAXDURABILITY, 0) == 0 {
                0
            } else {
                s.stat(istat::MAXDURABILITY, 0)
            }
        });
        self.set_stat(item, istat::DURABILITY, max);
    }
}

/// The item-move seams no d2-sim module provides, answered for the first
/// playable preview (`docs/PLAN.md` decisions, D1) so the play host can
/// have an inventory model ([`preview_inv_parts`]): nothing is active,
/// no own contribution, every location allowed, no quiver kind, player
/// data +0x4C / +0x50 zero, no NPC talk, no player trade; the sends are
/// collected for [`MoveRest::take_sent`].
///
/// d2rs-own, unverified: every answer here is a preview fill, not a
/// spec'd behaviour; each one names the open point of `inventory.md` it
/// stands in for (the `InvRest` method docs).
#[derive(Debug, Default)]
pub struct PreviewMoveRest {
    sent: Vec<(Owner, Vec<u8>)>,
    /// The places staged at the start of the call ([`MoveRest::stage`]).
    places: BTreeMap<Owner, StagedPlace>,
    item_format: u16,
    /// The players' quest flags staged at the start of the call, and the
    /// writes made since ([`MoveRest::stage_quest_flags`]).
    quest_flags: BTreeMap<Owner, d2_sim::world::quests::QuestFlags>,
    quest_writes: Vec<(Owner, u8, u8, bool)>,
    /// The skill seams over the players' lists (REC-266).
    skills: super::items::moves::preview_skills::PreviewSkills,
}

impl MovePending for PreviewMoveRest {
    fn send(&mut self, player: Owner, bytes: Vec<u8>) {
        self.sent.push((player, bytes));
    }
    /// d2rs-own, unverified (M22, REC-277 (d)): the room delete record
    /// `0x0061A270` reaches the room's clients as S→C 0x0A (type 4, GUID,
    /// `units::messages::remove_unit`) at once; when 1.14d sends the
    /// room's delete list is not written. The staged players stand for
    /// the room's clients (single player: the one). Without it a picked
    /// gold pile, freed on the server, stayed on the client's ground; an
    /// item that goes on to the inventory is re-added by its 0x9C.
    fn room_delete_notice(&mut self, item: d2_sim::items::moves::Guid) {
        let players: Vec<Owner> = self
            .places
            .values()
            .filter(|p| p.owner.is_player())
            .map(|p| p.owner)
            .collect();
        for p in players {
            let m = d2_sim::units::messages::remove_unit(4, item);
            self.sent.push((p, m.to_vec()));
        }
    }
    /// The staged quest record (`0x0065C310`).
    fn quest_flag(&self, player: Owner, quest: u8, flag: u8) -> bool {
        self.quest_flags
            .get(&player)
            .is_some_and(|f| f.get(quest, flag))
    }
    /// `0x0065C360` / `0x0065C3A0`: applied to the staged copy at once,
    /// and recorded for the host to write back.
    fn set_quest_flag(&mut self, player: Owner, quest: u8, flag: u8, on: bool) {
        if let Some(f) = self.quest_flags.get_mut(&player) {
            if on {
                f.set(quest, flag);
            } else {
                f.clear(quest, flag);
            }
        }
        self.quest_writes.push((player, quest, flag, on));
    }
    /// d2rs-own, unverified (D1): `0x00641530` is not specified; the
    /// larger of the two sub-tile axis distances of the staged places,
    /// out of range without both.
    fn distance(&self, a: Owner, b: Owner) -> i32 {
        match (self.places.get(&a), self.places.get(&b)) {
            (Some(a), Some(b)) => (a.pos.0 - b.pos.0).abs().max((a.pos.1 - b.pos.1).abs()),
            _ => i32::MAX,
        }
    }
    /// d2rs-own, unverified (D1): a room exists wherever the player's
    /// room does (no room lookup by position here).
    fn room_at(&self, _: i32, _: i32) -> bool {
        self.player_room().is_some()
    }
    /// d2rs-own, unverified (D1): `0x0064E810` (collision search) is not
    /// provided; the start point in the player's room is free.
    fn free_spot(
        &self,
        start: (i32, i32),
        _: (i32, i32),
        _: u32,
        _: u32,
        _: u32,
        _: u32,
    ) -> Option<Spot> {
        Some(Spot {
            room: self.player_room()?,
            x: start.0,
            y: start.1,
        })
    }
}

impl PreviewMoveRest {
    /// The room of the first staged player (the preview has one).
    fn player_room(&self) -> Option<RoomId> {
        self.places
            .values()
            .find(|p| p.owner.is_player())
            .and_then(|p| p.room)
    }
}

impl InvRest for PreviewMoveRest {
    fn mouse_skill(&self, u: Owner, left: bool) -> Option<(i32, i32)> {
        self.skills.mouse_skill(u, left)
    }
    fn select_skill(&mut self, u: Owner, left: bool, skill: (i32, i32)) {
        self.skills.select_skill(u, left, skill)
    }
    fn has_skill_owned(&self, u: Owner, skill: (i32, i32)) -> bool {
        self.skills.has_skill_owned(u, skill)
    }
    fn throw_skill_row(&self, skill: i32) -> bool {
        self.skills.throw_skill_row(skill)
    }
    fn saved_mouse_skill(&self, u: Owner, left: bool) -> (i32, i32) {
        self.skills.saved_mouse_skill(u, left)
    }
    fn set_saved_mouse_skill(&mut self, u: Owner, left: bool, skill: (i32, i32)) {
        self.skills.set_saved_mouse_skill(u, left, skill)
    }
    fn skill_quantity(&self, u: Owner, skill: i32) -> Option<i32> {
        self.skills.skill_quantity(u, skill)
    }
    fn set_skill_quantity(&mut self, u: Owner, skill: i32, q: i32) {
        self.skills.set_skill_quantity(u, skill, q)
    }
    fn learn_skill(&mut self, u: Owner, skill: i32) {
        self.skills.learn_skill(u, skill)
    }
    /// d2rs-own, unverified (D1): the staged place of a unit.
    fn pos(&self, u: Owner) -> (i32, i32) {
        self.places.get(&u).map_or((0, 0), |p| p.pos)
    }
    fn set_pos(&mut self, u: Owner, x: i32, y: i32) {
        if let Some(p) = self.places.get_mut(&u) {
            p.pos = (x, y);
        }
    }
    /// d2rs-own, unverified (D1): the request layout of a `gld` pile
    /// (`0x00559CE0`) is unwritten; a plain normal-quality item of the
    /// game's format, ground mode.
    fn gold_request(&self, _: Owner, gld: usize) -> Option<(ItemRequest, ItemSpawn)> {
        Some((
            ItemRequest {
                item: gld as i32,
                format: self.item_format,
                ilvl: 1,
                quality: q::NORMAL,
                flags2: 0x2,
                ..ItemRequest::default()
            },
            ItemSpawn {
                room: None,
                mode: 3,
                init_flags: 1,
            },
        ))
    }
    fn item_active_on(&self, _: u32, _: Owner) -> bool {
        false
    }
    fn own_contribution(&self, _: u32, _: Owner, _: u16) -> i32 {
        0
    }
    fn one_or_two_handed(&self, _: Owner, _: u32) -> bool {
        false
    }
    fn has_allowed_location(&self, _: u32) -> bool {
        true
    }
    fn quiver_kind(&self, _: u32) -> bool {
        false
    }
    fn player_data_4c(&self, _: Owner) -> u32 {
        0
    }
    fn player_data_50(&self, _: Owner) -> u32 {
        0
    }
    fn npc_talking(&self, _: Owner, _: Owner) -> bool {
        false
    }
    fn player_trade_gate(&self, _: Owner) -> Option<bool> {
        None
    }
}

impl MoveRest for PreviewMoveRest {
    fn stage_skills(
        &mut self,
        stage: super::items::moves::preview_skills::SkillStage,
        tables: &InvTables,
    ) {
        if self.skills.throw_rows.is_empty() {
            self.skills.throw_rows =
                super::items::moves::preview_skills::throw_rows(&stage.tables, &tables.equiv);
        }
        self.skills.stage = Some(stage);
    }
    fn take_skills(&mut self) -> Option<super::items::moves::preview_skills::SkillStage> {
        self.skills.stage.take()
    }
    fn queue_sent(&mut self, sent: Vec<(Owner, Vec<u8>)>) {
        self.sent.extend(sent);
    }
    fn take_sent(&mut self) -> Vec<(Owner, Vec<u8>)> {
        let mut sent = std::mem::take(&mut self.sent);
        sent.append(&mut self.skills.sent);
        sent
    }
    fn stage(&mut self, places: &[StagedPlace], item_format: u16) {
        self.places = places.iter().map(|p| (p.owner, *p)).collect();
        self.item_format = item_format;
    }
    fn stage_quest_flags(&mut self, flags: &[(Owner, d2_sim::world::quests::QuestFlags)]) {
        self.quest_flags = flags.iter().copied().collect();
        self.quest_writes.clear();
    }
    fn take_quest_flag_writes(&mut self) -> Vec<(Owner, u8, u8, bool)> {
        std::mem::take(&mut self.quest_writes)
    }
}

/// d2rs-own, unverified (REC-119): the cube's calls no written spec owns
/// ([`ItemPending`]) in the play host. The inventory pass queues nothing
/// (the transmuted items leave through the inventory model's own sends),
/// a copy, a tempered affix, the runeword and repair / recharge effects
/// and the quest hooks do nothing, and the Cow portal is refused.
#[derive(Debug, Default)]
pub struct PreviewCubePending {
    /// `hst ` hooks recorded for the quest control (REC-136).
    quest_items: Vec<(UnitId, [u8; 4])>,
}

impl super::items::ItemPending for PreviewCubePending {
    fn inventory_pass(&mut self, _: UnitId, _: &mut Vec<Vec<u8>>) {}
    fn duplicate(&mut self, _: UnitId, _: bool) -> Option<UnitId> {
        None
    }
    fn tempered_affix(&mut self, _: UnitId, _: bool) -> u16 {
        0
    }
    fn drop_runeword_stats(&mut self, _: UnitId) {}
    fn repair(&mut self, _: UnitId) {}
    fn recharge(&mut self, _: UnitId) {}
    fn quest_item_hook(&mut self, player: UnitId, _: UnitId, code: [u8; 4]) {
        self.quest_items.push((player, code));
    }
    fn take_quest_items(&mut self) -> Vec<(UnitId, [u8; 4])> {
        std::mem::take(&mut self.quest_items)
    }
    fn cow_portal(&mut self, _: UnitId) -> bool {
        false
    }
}

/// The cube parts of the play host ([`WiredWorld::cube`]) over `cube`
/// (`CubeData::from_fixed`) with [`PreviewCubePending`].
pub fn preview_cube_parts(cube: d2_sim::world::cube::CubeData) -> super::items::CubeParts {
    super::items::CubeParts::new(cube, Box::new(PreviewCubePending::default()))
}

/// An inventory model for the play host ([`WiredWorld::inventory`]) over
/// `tables` (`InvTables::from_fixed`) with [`PreviewMoveRest`].
pub fn preview_inv_parts(tables: InvTables) -> InvParts {
    let mut parts = InvParts::new(tables, Box::new(PreviewMoveRest::default()));
    // PROVISIONAL (REC-161, d2rs-own, unverified): worn items feed the
    // wearer's stats, set bonuses included (`wiring/inventory/item_link.rs`).
    parts.state.link_item_stats = true;
    parts
}
