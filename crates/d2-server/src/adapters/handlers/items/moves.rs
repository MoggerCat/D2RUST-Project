// Spec: specs/items/inventory-moves.md §6–§11; specs/sim/intents-events.md §2.3, §2.4, §3.2; specs/sim/tick.md §6; specs/audio/triggers-2.md §14; specs/world/cube.md §8
// Spec: specs/items/inventory.md (the sections other than §6–§11)
//! The item-move intents (C→S 0x16–0x29, 0x50, 0x61, 0x63; `inventory-moves.md`
//! §7) and the deferred item messages (§6, §11) on the wired host.
//!
//! A handler finds the client's player unit and hands the message to
//! `d2_sim::items::moves::handle` on the inventory wiring
//! (`d2_sim::wiring::inventory::InvDesk`: the inventory model, the unit
//! records and lists, the stat lists and the item store of the host's
//! one economy) through [`WorldHost::moves`]. Every check, result code
//! and state change is the module's; the handler maps the result to a
//! [`ResultCode`] and queues what the module sent "now" (§6.4: 0x63's
//! 0x9D / 0x9C, the 0x42 of a stack merge, the 0x3F of a targeting
//! reset) to the receivers' clients, in send order.
//!
//! The 0x9C / 0x9D / 0x7D messages are not the handler's: they leave in
//! the player unit update of the next client pass (§6.1).
//! [`update_pass`] runs it after `d2_sim::tick::tick` (see its reading).
//!
//! What no d2-sim module provides (positions and the free-spot search,
//! player data, item use, sockets, hirelings, sounds, the item bit
//! stream) is the host's [`MoveRest`] (`d2_sim::wiring::inventory::InvRest`
//! with its `MovePending` part).

use d2_sim::game::Game;
use d2_sim::items::inventory::InvTables;
use d2_sim::items::moves::{self as sim_moves, MoveFatal, MoveUnits, Owner, HANDLED};
use d2_sim::tick::EventDispatch;
use d2_sim::units::lifecycle::LifecycleHooks;
use d2_sim::units::sound::sound_message;
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::Economy;
use d2_sim::wiring::inventory::{InvDesk, InvRest, InvState};

use super::super::super::SimGame;
use super::super::world::{WorldError, WorldFault, WorldHost};
use super::result_code;
use crate::seams::{ClientId, Intents, MessageSink, ResultCode};

/// Every item-move C→S id: (id, `client-messages.tsv` name, owner spec
/// section). Exactly `d2_sim::items::moves::HANDLED`, in id order.
pub const MOVE_IDS: &[(u8, &str, &str)] = &[
    (0x16, "PickItem", "specs/items/inventory-moves.md §7.1"),
    (0x17, "DropItem", "specs/items/inventory-moves.md §7.2"),
    (
        0x18,
        "InsertItemInBuffer",
        "specs/items/inventory-moves.md §7.3",
    ),
    (
        0x19,
        "RemoveItemFromBuffer",
        "specs/items/inventory-moves.md §7.4",
    ),
    (0x1A, "EquipItem", "specs/items/inventory-moves.md §7.5"),
    (
        0x1B,
        "Swap2HandedItem",
        "specs/items/inventory-moves.md §7.6",
    ),
    (
        0x1C,
        "RemoveBodyItem",
        "specs/items/inventory-moves.md §7.7",
    ),
    (
        0x1D,
        "SwapCursorWithBody",
        "specs/items/inventory-moves.md §7.8",
    ),
    (0x1E, "Swap1HWith2H", "specs/items/inventory-moves.md §7.9"),
    (
        0x1F,
        "SwapCursorBufferItem",
        "specs/items/inventory-moves.md §7.10",
    ),
    (0x20, "UseGridItem", "specs/items/inventory-moves.md §7.11"),
    (0x21, "StackItems", "specs/items/inventory-moves.md §7.12"),
    (0x22, "UnstackItems", "specs/items/inventory-moves.md §7.13"),
    (0x23, "ItemToBelt", "specs/items/inventory-moves.md §7.14"),
    (0x24, "ItemFromBelt", "specs/items/inventory-moves.md §7.15"),
    (
        0x25,
        "SwitchBeltItem",
        "specs/items/inventory-moves.md §7.16",
    ),
    (0x26, "UseBeltItem", "specs/items/inventory-moves.md §7.17"),
    (
        0x27,
        "UseItemAction",
        "specs/items/inventory-moves.md §7.18",
    ),
    (0x28, "SocketItem", "specs/items/inventory-moves.md §7.19"),
    (0x29, "ScrollToBook", "specs/items/inventory-moves.md §7.20"),
    (0x50, "DropGold", "specs/items/inventory-moves.md §7.22"),
    (0x61, "MercItem", "specs/items/inventory-moves.md §7.23"),
    (
        0x63,
        "ItemToBeltShift",
        "specs/items/inventory-moves.md §7.24",
    ),
];

/// An item-move id (one [`MOVE_IDS`] row).
pub fn is_move_id(id: u8) -> bool {
    HANDLED.iter().any(|&(i, _)| i == id)
}

/// The inventory seams without a d2-sim provider, as the host holds
/// them: [`InvRest`] (with `MovePending`), plus the take of what
/// `MovePending::send` queued.
pub trait MoveRest: InvRest {
    /// The messages `MovePending::send` queued since the last take, in
    /// send order: (receiving unit, bytes).
    fn take_sent(&mut self) -> Vec<(Owner, Vec<u8>)>;
}

/// The item-move part of a game's world host: the inventory tables
/// (`InvTables::from_fixed`), the inventory state (one inventory per
/// owner unit, the item data fields this spec owns, the wiring's
/// errors) and the seams without a provider. The units, stats and items
/// are the host's economy.
pub struct InvParts {
    pub tables: InvTables,
    /// `InvState::errors` collects the wiring's provider errors; the
    /// host reads them (they are not handler results).
    pub state: InvState,
    pub rest: Box<dyn MoveRest + Send + Sync>,
}

impl InvParts {
    pub fn new(tables: InvTables, rest: Box<dyn MoveRest + Send + Sync>) -> Self {
        Self {
            tables,
            state: InvState::new(),
            rest,
        }
    }

    /// The inventory desk of one call over the host's economy
    /// (`InvDesk::new` fills the item data copies): the item moves, the
    /// update pass, and the vendor and cube adapters all run on it.
    pub fn desk<'d, 'a, H: LifecycleHooks>(
        &'d mut self,
        econ: &'d mut Economy<'a, H>,
    ) -> InvDesk<'d, 'a, H, dyn MoveRest + Send + Sync> {
        let InvParts {
            tables,
            state,
            rest,
        } = self;
        InvDesk::new(econ, tables, state, rest.as_mut())
    }
}

/// One call on the inventory wiring over the host's economy.
pub trait MoveCall {
    type Out;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> Self::Out;
}

/// What the rest sent during a call, with the receiving units looked up
/// (`None`: a GUID without a unit; nothing is sent to it).
pub(crate) fn take_sent<H: LifecycleHooks>(
    d: &mut InvDesk<'_, '_, H, dyn MoveRest + Send + Sync>,
) -> Vec<(Option<UnitId>, Vec<u8>)> {
    let sent = d.rest.take_sent();
    sent.into_iter().map(|(o, b)| (d.unit_of(o), b)).collect()
}

/// One item-move message.
struct MoveRun<'m> {
    player: UnitId,
    msg: &'m [u8],
}

type MoveOut = (
    Option<Result<u32, MoveFatal>>,
    Vec<(Option<UnitId>, Vec<u8>)>,
);

impl MoveCall for MoveRun<'_> {
    type Out = MoveOut;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> MoveOut {
        let mut d = parts.desk(econ);
        let guid = d.guid_of(self.player);
        let r = sim_moves::handle(&mut d, guid, self.msg);
        (r, take_sent(&mut d))
    }
}

/// The handler of an item-move id after the dispatcher's gate and size
/// check (`intents-events.md` §2.3–§2.4). `None`: not an item-move id,
/// no player, or a host without the inventory parts, so the caller
/// keeps its stub. A fatal assert of the original (`MoveFatal`) or a
/// queueing failure is recorded with the host ([`WorldHost::fault`]) and
/// the handler returns [`ResultCode::Malformed`], as the world handlers
/// do.
pub fn handle<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    client: ClientId,
    msg: &[u8],
    size: usize,
    out: &mut dyn MessageSink,
) -> Option<ResultCode> {
    let id = *msg.first()?;
    if !is_move_id(id) {
        return None;
    }
    let player = sim.player_of(client)?;
    // Every item-move id has a fixed size ≤ 17 (`client-messages.tsv`).
    let msg = &msg[..size.min(msg.len())];
    let (game, events) = (&mut sim.game, &mut sim.events);
    let (run, sent) = sim.world.moves(game, events, MoveRun { player, msg })?;
    let mut faults = Vec::new();
    for (unit, bytes) in sent {
        // §3.2 rule 1: a unit without a client receives nothing.
        if let Some(c) = unit.and_then(|u| sim.client_of(u)) {
            if let Err(e) = out.queue(c, &bytes) {
                faults.push(WorldError::from(e));
            }
        }
    }
    let result = match run? {
        Ok(r) => result_code(r),
        Err(e) => {
            faults.push(WorldError::Move(e));
            ResultCode::Malformed
        }
    };
    let failed = !faults.is_empty();
    for error in faults {
        sim.world.fault(WorldFault { client, id, error });
    }
    Some(if failed {
        ResultCode::Malformed
    } else {
        result
    })
}

/// One client's part of the update pass: the client's own player (the
/// owner test of §6.2) and the players whose unit update it processes.
#[derive(Clone)]
struct Receiver {
    client: ClientId,
    own: UnitId,
    players: Vec<UnitId>,
}

/// The update pass of one tick.
#[derive(Clone)]
struct UpdateRun {
    receivers: Vec<Receiver>,
    /// Every player of the pass, for the clean-up.
    players: Vec<UnitId>,
}

impl UpdateRun {
    /// The S→C 0x2C of each (receiver, player) of the pass, in pass order
    /// (`audio/triggers-2.md` §14 rule 2: target none or the receiver's
    /// player).
    fn sounds(&self, game: &Game) -> Vec<Option<[u8; 8]>> {
        self.receivers
            .iter()
            .flat_map(|r| {
                r.players
                    .iter()
                    .map(move |&p| sound_message(game, p, r.own))
            })
            .collect()
    }

    /// Unit flag 0x400 of every player of the pass := 0 (the room
    /// clean-up's step 3, `intents-events.md` §7.5, for the players; the
    /// tick wiring's clean-up leaves players' slots to this pass).
    fn clear_sounds(&self, game: &mut Game) {
        for &p in &self.players {
            game.sounds.clear(p);
        }
    }
}

type UpdateOut = (Vec<(ClientId, Vec<u8>)>, Vec<(ClientId, MoveFatal)>);

impl MoveCall for UpdateRun {
    type Out = UpdateOut;
    fn call<H: LifecycleHooks>(self, econ: &mut Economy<'_, H>, parts: &mut InvParts) -> UpdateOut {
        // The sound of each (receiver, player), read before the pass: the
        // item messages do not touch the sound slots.
        let sounds = self.sounds(econ.game);
        let mut d = parts.desk(econ);
        let (mut sent, mut fatal) = (Vec::new(), Vec::new());
        let mut sounds = sounds.into_iter();
        for r in &self.receivers {
            let own = d.guid_of(r.own);
            for &p in &r.players {
                let guid = d.guid_of(p);
                match sim_moves::player_update(&mut d, own, guid) {
                    Ok(msgs) => sent.extend(msgs.into_iter().map(|m| (r.client, m))),
                    Err(e) => fatal.push((r.client, e)),
                }
                // `cube.md` §8 rule 3: after the item messages and 0x47 /
                // 0x48, flag 0x400 → `0x00571740` (S→C 0x2C).
                if let Some(m) = sounds.next().flatten() {
                    sent.push((r.client, m.to_vec()));
                }
            }
        }
        // The update-list reset of the room clean-up (`tick.md` §3 step 6,
        // `0x00553220` → `0x00597B00`; §6.1 rule 4, `InvDesk::update_done`):
        // +0xC8 bit 0 cleared (bit 1, "save pending", stays: IS1), the
        // per-item resets, the update lists freed. The unit-flag part of
        // `0x00553220` runs in the tick wiring's step 6
        // (`d2_sim::wiring::action::View::room_cleanup`, `intents-events.md`
        // §7.5 step 3), before this pass.
        for &p in &self.players {
            let Some(o) = d.owner_of(p) else {
                continue;
            };
            if d.update_bits(o) & 1 == 0 {
                continue;
            }
            d.update_done(o);
        }
        // The desk's borrow of the economy ends here.
        self.clear_sounds(econ.game);
        (sent, fatal)
    }
}

/// The deferred item messages of one tick (§6.1 rules 2–4): for each
/// client in client-list order (`unit-order.md` §7), for each player unit
/// in the client room's adjacent rooms (the rooms in array order, the
/// players of a room in client-list order), the player unit update
/// `0x00580860` (`items::moves::player_update`: with +0xC8 bit 0, the
/// update-list pass, then 0x47 and 0x48); the messages go to that
/// client. Then the clean-up of every player of the pass.
///
/// Reading: in 1.14d this runs inside the client pass (`tick.md` §6,
/// step 5) and the clean-up in step 6; here it runs after
/// `d2_sim::tick::tick`. The tick wiring's per-client unit update sends
/// no item message and its clean-up (`intents-events.md` §7.5, flags
/// only) clears no bit this pass reads (+0xC8 bits 0 and 1, the item and
/// command flags), so no step of the tick changes what this pass does. Every player
/// is queued for update by its own per-client update (`tick.md` §6 step
/// 5, last), so the queue membership test is the room test above. The
/// client's room is read after the tick's room switch (`0x00537B50`, in
/// the per-client update after the unit updates): in the tick of a
/// switch 1.14d walks the old room's adjacent rooms. The ground items' unit update (§6.3,
/// `d2_sim::items::moves::item_unit_update`) is not run: it belongs to the
/// per-unit update `0x0053A500` over the client's rooms, which the tick
/// wiring does not run for items; run here, after the tick, it would find
/// unit flags 0x1 and 0x10 already cleared by the room clean-up (IS2,
/// IS3).
pub fn update_pass<D: EventDispatch, W: WorldHost<D>>(
    sim: &mut SimGame<D, W>,
    out: &mut dyn MessageSink,
) {
    let clients = sim.clients();
    let players: Vec<(UnitId, Option<d2_sim::units::RoomId>)> = clients
        .iter()
        .filter_map(|&c| sim.player_of(c))
        .map(|p| (p, sim.game.lists.unit(p).and_then(|u| u.room())))
        .collect();
    let mut receivers = Vec::new();
    for &c in &clients {
        let Some(own) = sim.player_of(c) else {
            continue;
        };
        let adjacent = sim
            .sim_client(c)
            .and_then(|id| sim.game.lists.client(id))
            .and_then(|e| e.room)
            .and_then(|r| sim.game.lists.room(r))
            .map(|r| r.adjacent.clone())
            .unwrap_or_default();
        let seen: Vec<UnitId> = adjacent
            .iter()
            .flat_map(|&room| {
                players
                    .iter()
                    .filter(move |&&(_, r)| r == Some(room))
                    .map(|&(p, _)| p)
            })
            .collect();
        receivers.push(Receiver {
            client: c,
            own,
            players: seen,
        });
    }
    let run = UpdateRun {
        receivers,
        players: players.iter().map(|&(p, _)| p).collect(),
    };
    let (game, events) = (&mut sim.game, &mut sim.events);
    let sound_only = run.clone();
    let Some((sent, fatal)) = sim.world.moves(game, events, run) else {
        // A host without inventory parts: no item messages; the players'
        // sounds still leave (`cube.md` §8 rule 3) and are cleared.
        let mut sounds = sound_only.sounds(&sim.game).into_iter();
        for r in &sound_only.receivers {
            for _ in &r.players {
                let Some(m) = sounds.next().flatten() else {
                    continue;
                };
                if let Err(e) = out.queue(r.client, &m) {
                    sim.tick_faults.push((r.client, WorldError::from(e)));
                }
            }
        }
        sound_only.clear_sounds(&mut sim.game);
        return;
    };
    for (c, bytes) in sent {
        if let Err(e) = out.queue(c, &bytes) {
            sim.tick_faults.push((c, WorldError::from(e)));
        }
    }
    for (c, e) in fatal {
        sim.tick_faults.push((c, WorldError::Move(e)));
    }
}

#[cfg(test)]
pub(crate) mod tests;
