// Spec: specs/world/npc.md, specs/world/vendors.md, specs/world/waypoints.md, specs/world/quests.md, specs/sim/intents-events.md §2.4, §3.2
//! The world intent handlers: NPC interaction (`world/npc.md` §2–§4, §6,
//! §7), vendors (`world/vendors.md` §5.5, §7, §8), waypoints
//! (`world/waypoints.md` §6) and quests (`world/quests.md` §1.7, §6.2,
//! §7.3).
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
mod wired;

#[cfg(test)]
pub(crate) mod tests;

pub use action::{ActionEvents, ActionWorld, Outbox};
pub use wired::{Parts, TradeRest, WiredWorld};

use d2_sim::game::Game;
use d2_sim::tick::EventDispatch;
use d2_sim::units::UnitId;
use d2_sim::world::npc::{NpcControl, NpcError, NpcVendors, NpcWorld};
use d2_sim::world::quests::{QuestControl, QuestError, QuestWorld};
use d2_sim::world::vendors::gamble::identify_gamble;
use d2_sim::world::vendors::price::PriceFatal;
use d2_sim::world::vendors::trade::{buy, repair, sell, BuyMsg, RepairMsg, SellMsg};
use d2_sim::world::vendors::{VendorRecord, VendorTables, VendorWorld};
use d2_sim::world::waypoints::{ArrivalList, WaypointData, WaypointError, WaypointWorld};

use super::super::SimGame;
use super::items::CubeCall;
use super::skills::{Call as SkillCall, Handled as SkillHandled};
use crate::buffers::QueueError;
use crate::seams::{ClientId, MessageSink, ResultCode};

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
}

/// Every world-related C→S id (`client-messages.tsv`): (id, owner spec
/// section, status). The order is the id order.
pub const WORLD_IDS: &[(u8, &str, Status)] = &[
    // Unit type 1 only (`npc.md` §2); type 2 goes to the
    // object-interaction spec (not written, `waypoints.md` §5.2), other
    // types to no written spec: those stay stubs.
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
    (0x3F, "-", Status::NoOwner),
    (
        0x40,
        "world/quests.md §6.2",
        Status::Implemented(System::Quests),
    ),
    (0x44, "-", Status::NoOwner),
    (0x46, "-", Status::NoOwner),
    (0x47, "-", Status::NoOwner),
    (
        0x49,
        "world/waypoints.md §6",
        Status::Implemented(System::Waypoints),
    ),
    (
        0x4C,
        "world/cube.md (handlers/items.rs)",
        Status::OtherModule("items"),
    ),
    (0x4D, "-", Status::NoOwner),
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
    (0x59, "-", Status::NoOwner),
    (0x62, "world/npc.md §7.4", Status::Implemented(System::Npc)),
];

/// The system that handles `id` here, if any.
pub fn system(id: u8) -> Option<System> {
    WORLD_IDS.iter().find_map(|&(i, _, s)| match s {
        Status::Implemented(sys) if i == id => Some(sys),
        _ => None,
    })
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
    /// The cube (`handlers::items`) on the host's economy.
    fn cube<C: CubeCall>(&mut self, game: &mut Game, events: &mut D, call: C) -> Option<C::Out> {
        None
    }
    /// The skill handlers (`handlers::skills`).
    fn skill(&mut self, call: SkillCall<'_, D>) -> Option<SkillHandled> {
        None
    }
    /// The messages the seams sent since the last take, in send order:
    /// (receiving player unit, bytes).
    fn take_sent(&mut self, events: &mut D) -> Vec<(UnitId, Vec<u8>)> {
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
    let sys = system(id)?;
    let player = sim.player_of(client)?;
    // Every handled id has a fixed size ≤ 17 (`client-messages.tsv`), so
    // the drained copy is the whole message.
    let msg = &msg[..size.min(msg.len())];
    let (game, events) = (&mut sim.game, &mut sim.events);
    let run = match sys {
        System::Npc => sim.world.npc(game, events, NpcRun { player, msg }),
        System::Vendors => sim.world.vendors(game, events, VendorRun { player, msg }),
        System::Waypoints => sim
            .world
            .waypoints(game, events, WaypointRun { player, msg }),
        System::Quests => sim.world.quests(game, events, QuestRun { player, msg }),
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
            0x35 => match RepairMsg::parse(m) {
                Some(r) => Some(repair(tables, w, p, &r)?),
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
