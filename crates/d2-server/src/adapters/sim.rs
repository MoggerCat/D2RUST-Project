// Spec: specs/sim/intents-events.md; specs/combat/vitals.md §5.1; specs/world/quests-helpers.md §6
//! [`Intents`] and [`Tick`] on `d2_sim::game::Game` (§2.2–§2.4, §4;
//! `tick.md` §3; client list order `unit-order.md` §7).
//!
//! What the seams read that `d2-sim` does not hold yet (unit mode, state
//! 54, player data, positions, owners, acts of units outside rooms) is
//! owned by unit, path and item specs not written yet. Until they are,
//! the caller stages those fields here ([`PlayerFields`], [`UnitFacts`]);
//! nothing here derives or defaults them.
//!
//! The game's systems beyond the event dispatch (world, items, skills)
//! are one host value `W` ([`WorldHost`]): the intent handlers
//! ([`handlers`]) reach their `d2-sim` providers through it, and the
//! messages its seams send, from a handler or from a tick, go to the
//! receivers' clients.

use std::collections::BTreeMap;

use d2_sim::game::Game;
use d2_sim::tick::timer::TimerRun;
use d2_sim::tick::{EventDispatch, TickHooks};
use d2_sim::units::{ClientId as SimClient, RoomId, UnitId, UnitType};
use d2_sim::world::quests::HostRequest;

use super::handlers;
use super::handlers::player::{HotKey, HOTKEY_SLOTS};
use super::handlers::world::ActionEvents;
use super::handlers::world::{self as world_handlers, NoWorld, WorldError, WorldHost};
use super::session_flow::{SessionFlow, SessionRunner};
use crate::seams::{
    ClientId, Intents, MessageSink, PlayerGate, PlayerLookup, PointState, Pos, ResultCode, Tick,
    UnitTarget,
};

/// Player data (`0x006221A0`) as far as the point parser reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerData {
    /// Player data +0x168: frame of the last accepted point target.
    pub last_accept: i32,
}

/// The player fields of a player unit the dispatch gate and the point
/// parser read (§2.3 rule 3, §2.4 rule 3). Staged by the caller until the
/// unit specs move them into `d2-sim`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerFields {
    pub gate: PlayerGate,
    /// `None`: the player has no player data (point messages → 2).
    pub data: Option<PlayerData>,
}

/// Unit fields the unit-target lookup `0x00548F80` reads (§2.4 rule 4).
/// Staged by the caller until the path and item specs move them into
/// `d2-sim`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitFacts {
    /// The unit's act.
    pub act: u8,
    /// Position (dynamic path, or static path for unit types 2, 4, 5).
    pub pos: Pos,
    /// For items: the player unit that owns it.
    pub owner: Option<UnitId>,
}

/// Misuse of the adapter's client bookkeeping.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdapterError {
    #[error("client {0} already joined")]
    AlreadyJoined(ClientId),
    #[error("client {0} is not in the game")]
    NotJoined(ClientId),
    #[error(transparent)]
    List(#[from] d2_sim::units::ListError),
}

/// Timer-event dispatch until the unit specs exist (`tick.md` open
/// question 3): runs nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unspecified;

impl EventDispatch for Unspecified {
    fn run_event(&mut self, _game: &mut Game, _run: &TimerRun) {}
}

/// Every step body keeps its `TickHooks` default (does nothing).
impl TickHooks for Unspecified {}

/// One `d2-sim` game behind the `d2-server` seams.
///
/// Transport client ids (`d2-server`) and client records (`d2-sim` slots)
/// are mapped by [`SimGame::join`]. Timer events go to `events`; the
/// game's world, item and skill systems are the host `world`.
pub struct SimGame<D = Unspecified, W = NoWorld> {
    pub game: Game,
    pub events: D,
    clients: BTreeMap<ClientId, SimClient>,
    transport_ids: BTreeMap<SimClient, ClientId>,
    players: BTreeMap<UnitId, PlayerFields>,
    units: BTreeMap<UnitId, UnitFacts>,
    /// Clients the point parser asked to resync with S→C 0x15, in order.
    /// Not queued: the 11-byte layout of 0x15 is not in
    /// `server-messages.tsv` (`docs/HANDOFF.md` §7).
    pub resyncs: Vec<ClientId>,
    /// Intents that passed the gate, size check and parse, in order:
    /// (client, id, size). Their handlers are not written (see `handle`).
    pub unhandled: Vec<(ClientId, u8, usize)>,
    /// The game's systems for the intent handlers ([`handlers`]: world,
    /// cube, skills) and the outbox of their seams.
    pub world: W,
    /// Messages sent during a tick that could not be queued, in order.
    pub tick_faults: Vec<(ClientId, WorldError)>,
    /// The host calls the quest rules raised (`quests-helpers.md` §6:
    /// game end `0x00530590`, save pass `0x0052E2A0`), drained from the
    /// world after each tick's steps, in call order. Running them is the
    /// session layer's (client removal `0x005303D0`, the save): it takes
    /// them with [`SimGame::take_host_requests`].
    pub host_requests: Vec<HostRequest>,
    /// The hot-key slots of each client (client +0x3DC, 16 × 8 bytes;
    /// written by C→S 0x51, `intents-events.md` §9 rule 12; read by the
    /// join's S→C 0x7B, §8.2 rule 3.6).
    hotkeys: BTreeMap<ClientId, [HotKey; HOTKEY_SLOTS]>,
    /// The session sequence of C→S 0x67 / 0x6B (`intents-events.md` §8,
    /// [`super::session_flow`]); `None`: every system message goes to the
    /// host's `SessionHandler`.
    session: Option<Box<dyn SessionRunner<D, W>>>,
}

/// [`SimGame`]'s fields borrowed apart (for a handler).
pub struct SimParts<'s, D, W> {
    pub game: &'s mut Game,
    pub events: &'s mut D,
    pub world: &'s mut W,
    /// The staged unit facts (act, position, owner).
    pub facts: &'s BTreeMap<UnitId, UnitFacts>,
}

impl SimGame<Unspecified> {
    pub fn new(game: Game) -> Self {
        Self::with_events(game, Unspecified)
    }
}

impl<D: EventDispatch, W> SimGame<D, W> {
    pub fn with_events(game: Game, events: D) -> Self
    where
        W: Default,
    {
        Self::with_world(game, events, W::default())
    }

    /// A game with the given world host (one without a `Default`, e.g.
    /// `handlers::world::WiredWorld`, built from the game's tables).
    pub fn with_world(game: Game, events: D, world: W) -> Self {
        Self {
            game,
            events,
            clients: BTreeMap::new(),
            transport_ids: BTreeMap::new(),
            players: BTreeMap::new(),
            units: BTreeMap::new(),
            resyncs: Vec::new(),
            unhandled: Vec::new(),
            world,
            tick_faults: Vec::new(),
            host_requests: Vec::new(),
            hotkeys: BTreeMap::new(),
            session: None,
        }
    }

    /// The quests' host requests drained so far ([`SimGame::host_requests`]),
    /// in call order, for the session layer to run.
    pub fn take_host_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.host_requests)
    }

    /// Runs C→S 0x67 / 0x6B through `flow` from now on.
    pub fn set_session(&mut self, flow: SessionFlow<D, W>)
    where
        D: ActionEvents + 'static,
        W: 'static,
    {
        self.session = Some(Box::new(flow));
    }

    /// The session flow, if one is set.
    pub fn session(&self) -> Option<&SessionFlow<D, W>> {
        self.session.as_deref().map(|r| r.flow())
    }

    /// Adds a client record for transport client `client`
    /// (`UnitLists::add_client`, `unit-order.md` §7.2: prepended).
    pub fn join(
        &mut self,
        client: ClientId,
        player: Option<UnitId>,
        room: Option<RoomId>,
        state: u32,
    ) -> Result<SimClient, AdapterError> {
        if self.clients.contains_key(&client) {
            return Err(AdapterError::AlreadyJoined(client));
        }
        let id = self.game.lists.add_client(player, room, state);
        self.clients.insert(client, id);
        self.transport_ids.insert(id, client);
        Ok(id)
    }

    /// Removes the client's record (`UnitLists::remove_client`).
    pub fn leave(&mut self, client: ClientId) -> Result<(), AdapterError> {
        let id = self
            .clients
            .remove(&client)
            .ok_or(AdapterError::NotJoined(client))?;
        self.transport_ids.remove(&id);
        self.hotkeys.remove(&client);
        self.game.lists.remove_client(id)?;
        Ok(())
    }

    /// The transport clients with a record, in client-list order
    /// (`unit-order.md` §7).
    pub fn client_list(&self) -> Vec<ClientId> {
        self.game
            .lists
            .clients()
            .into_iter()
            .filter_map(|id| self.transport_ids.get(&id).copied())
            .collect()
    }

    /// The `d2-sim` client record of a transport client.
    pub fn sim_client(&self, client: ClientId) -> Option<SimClient> {
        self.clients.get(&client).copied()
    }

    /// Stages a player unit's gate fields and player data.
    pub fn set_player(&mut self, unit: UnitId, fields: PlayerFields) {
        self.players.insert(unit, fields);
    }

    /// The staged player fields of a unit.
    pub fn player_fields(&self, unit: UnitId) -> Option<&PlayerFields> {
        self.players.get(&unit)
    }

    /// Stages a unit's act, position and owner.
    pub fn set_unit(&mut self, unit: UnitId, facts: UnitFacts) {
        self.units.insert(unit, facts);
    }

    /// The client's player unit, if it is a player (unit type 0).
    fn player_unit(&self, client: ClientId) -> Option<UnitId> {
        let id = self.clients.get(&client)?;
        let unit = self.game.lists.client(*id)?.player?;
        (self.game.lists.unit(unit)?.ty == UnitType::Player).then_some(unit)
    }

    /// The client's hot-key slots (client +0x3DC; a slot never bound is
    /// [`HotKey::UNBOUND`]).
    pub fn hotkeys(&self, client: ClientId) -> [HotKey; HOTKEY_SLOTS] {
        self.hotkeys
            .get(&client)
            .copied()
            .unwrap_or([HotKey::UNBOUND; HOTKEY_SLOTS])
    }

    /// Stores one hot-key slot (`0x005390A0`); `slot` < 16.
    pub fn set_hotkey(&mut self, client: ClientId, slot: usize, key: HotKey) {
        if slot < HOTKEY_SLOTS {
            self.hotkeys
                .entry(client)
                .or_insert([HotKey::UNBOUND; HOTKEY_SLOTS])[slot] = key;
        }
    }

    /// The staged position of the client's player ([`UnitFacts`]).
    pub fn player_pos(&self, client: ClientId) -> Option<Pos> {
        let unit = self.player_unit(client)?;
        Some(self.units.get(&unit)?.pos)
    }

    /// The client's player unit (unit type 0), for the handlers.
    pub fn player_of(&self, client: ClientId) -> Option<UnitId> {
        self.player_unit(client)
    }

    /// The fields a handler works on, borrowed apart.
    pub fn parts(&mut self) -> SimParts<'_, D, W> {
        SimParts {
            game: &mut self.game,
            events: &mut self.events,
            world: &mut self.world,
            facts: &self.units,
        }
    }

    /// The transport client whose player is `unit` (`None`: a player
    /// without a client, `intents-events.md` §3.2 rule 1).
    pub fn client_of(&self, unit: UnitId) -> Option<ClientId> {
        self.game
            .lists
            .clients()
            .into_iter()
            .find(|&id| {
                self.game
                    .lists
                    .client(id)
                    .is_some_and(|r| r.player == Some(unit))
            })
            .and_then(|id| self.transport_ids.get(&id).copied())
    }
}

impl<D: EventDispatch, W: WorldHost<D>> Intents for SimGame<D, W> {
    /// Not joined (or record gone) → not in game; no player unit, not a
    /// player, or no staged [`PlayerFields`] → no player.
    fn player(&self, client: ClientId) -> PlayerLookup {
        let in_game = self
            .clients
            .get(&client)
            .is_some_and(|id| self.game.lists.client(*id).is_some());
        if !in_game {
            return PlayerLookup::NotInGame;
        }
        match self.player_unit(client).and_then(|u| self.players.get(&u)) {
            Some(p) => PlayerLookup::Player(p.gate),
            None => PlayerLookup::NoPlayer,
        }
    }

    fn frame(&self) -> i32 {
        self.game.frame
    }

    /// The host world's object host tick ([`WorldHost::host_tick`]).
    fn set_host_tick(&mut self, ms: u32) {
        self.world.host_tick(&mut self.events, ms);
    }

    /// `None` without player data, or without a staged position.
    fn point_state(&self, client: ClientId) -> Option<PointState> {
        let unit = self.player_unit(client)?;
        let data = self.players.get(&unit)?.data?;
        let player = self.units.get(&unit)?.pos;
        Some(PointState {
            player,
            last_accept: data.last_accept,
        })
    }

    fn set_point_accept(&mut self, client: ClientId, frame: i32) {
        let data = self
            .player_unit(client)
            .and_then(|u| self.players.get_mut(&u))
            .and_then(|p| p.data.as_mut());
        if let Some(d) = data {
            d.last_accept = frame;
        }
    }

    /// TODO(server-messages.tsv layout of 0x15): records the request; the
    /// message is queued once its layout is specified.
    fn queue_resync(&mut self, client: ClientId, _out: &mut dyn MessageSink) {
        self.resyncs.push(client);
    }

    /// §2.4 rule 4 in its order: missing → owned item → other act →
    /// positions. A target or player without staged [`UnitFacts`] counts
    /// as missing.
    fn unit_target(&self, client: ClientId, unit_type: u32, unit_id: u32) -> UnitTarget {
        let Some(&ty) = UnitType::ALL.get(unit_type as usize) else {
            return UnitTarget::Missing;
        };
        let Some(target) = self.game.lists.find_unit(ty, unit_id) else {
            return UnitTarget::Missing;
        };
        let Some(t) = self.units.get(&target) else {
            return UnitTarget::Missing;
        };
        let player = self.player_unit(client);
        if ty == UnitType::Item && player.is_some() && t.owner == player {
            return UnitTarget::OwnedItem;
        }
        let Some(p) = player.and_then(|u| self.units.get(&u)) else {
            return UnitTarget::Missing;
        };
        if p.act != t.act {
            return UnitTarget::OtherAct;
        }
        UnitTarget::At {
            player: p.pos,
            target: t.pos,
        }
    }

    /// The host's live position facts (`WorldHost::unit_position`: the
    /// path provider's position and the act of the unit's room) replace
    /// the staged [`UnitFacts`] of the client's player and, for a unit
    /// message, of the target, so the range tests of §2.4 rules 3–4 read
    /// the position the walk moved (the staged facts were never updated
    /// in the app's game, so every walk was refused, `docs/handoff`
    /// `wire-path-server.md` finding 3 = WS3). A host without the
    /// provider answers `None` and the staged facts stay. An item keeps
    /// its staged owner.
    fn refresh_positions(&mut self, client: ClientId, msg: &[u8]) {
        let mut units = Vec::new();
        if let Some(p) = self.player_unit(client) {
            units.push(p);
        }
        if msg.len() >= 9 && crate::dispatch::is_unit(msg[0]) {
            let ty = u32::from_le_bytes([msg[1], msg[2], msg[3], msg[4]]);
            let guid = u32::from_le_bytes([msg[5], msg[6], msg[7], msg[8]]);
            if let Some(&ty) = UnitType::ALL.get(ty as usize) {
                units.extend(self.game.lists.find_unit(ty, guid));
            }
        }
        for unit in units {
            let Some((act, (x, y))) = self.world.unit_position(&self.game, &mut self.events, unit)
            else {
                continue;
            };
            let owner = self.units.get(&unit).and_then(|f| f.owner);
            self.units.insert(
                unit,
                UnitFacts {
                    act,
                    pos: Pos { x, y },
                    owner,
                },
            );
        }
    }

    /// Per-intent behaviour belongs to the system specs (movement,
    /// skills, items, NPCs, quests; §4 rule 1). Ids a [`handlers`]
    /// module owns run there when the host provides their system (the
    /// skill messages' `pierce_idx` += 1 of §2.4 rule 5 is
    /// `use_::handle_message`'s); every other handler is a stub: it
    /// records the intent and returns 0, the "does nothing" result of
    /// 1.14d's stubs (§2.4 rule 2).
    fn handle(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    ) -> ResultCode {
        if let Some(r) = handlers::items::handle(self, client, msg, out) {
            return r;
        }
        if let Some(r) = handlers::items::moves::handle(self, client, msg, size, out) {
            return r;
        }
        if let Some(code) = world_handlers::handle(self, client, msg, size, out) {
            return code;
        }
        if let Some(code) = super::handlers::skills::handle(self, client, msg, out) {
            return code;
        }
        if let Some(code) = handlers::walk::handle(self, client, msg, out) {
            return code;
        }
        if let Some(code) = handlers::player::handle(self, client, msg, size, out) {
            return code;
        }
        self.unhandled.push((client, msg[0], size));
        ResultCode::Done
    }

    /// `UnitLists::clients` (`unit-order.md` §7: newest first), as
    /// transport ids.
    fn clients(&self) -> Vec<ClientId> {
        self.game
            .lists
            .clients()
            .into_iter()
            .filter_map(|id| self.transport_ids.get(&id).copied())
            .collect()
    }

    /// The game's session flow, when set ([`SimGame::set_session`]).
    fn session_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        _size: usize,
        out: &mut dyn MessageSink,
    ) -> bool {
        let Some(mut flow) = self.session.take() else {
            return false;
        };
        let handled = flow.run(self, client, msg, out);
        self.session = Some(flow);
        handled
    }
}

impl<D: EventDispatch + TickHooks, W: WorldHost<D>> Tick for SimGame<D, W> {
    /// `d2_sim::tick::tick` with `D` as the step hooks, through
    /// [`WorldHost::run_tick`] (`tick.md` §3:
    /// the wired dispatch's room, DRLG and population steps run; a
    /// dispatch without them keeps the defaults), then the host's
    /// [`WorldHost::after_tick`]; the quests' host requests are drained
    /// into [`SimGame::host_requests`] (`quests-helpers.md` §6: after the
    /// frame's quest step, in call order). What the host's seams
    /// sent during the tick ([`WorldHost::take_sent`]) is queued to the
    /// receivers' clients in send order (§3.2 rule 1: a player without a
    /// client receives nothing); a queueing failure is recorded in
    /// [`SimGame::tick_faults`]. Then the deferred item messages
    /// (`handlers::items::moves::update_pass`, `inventory-moves.md` §6.1), then
    /// the client vitals sync ([`SimGame::vitals_sync`]).
    fn tick(&mut self, out: &mut dyn MessageSink) {
        self.world.run_tick(&mut self.game, &mut self.events);
        self.world.after_tick(&mut self.game, &mut self.events);
        let requests = self.world.take_host_requests();
        self.host_requests.extend(requests);
        for (unit, bytes) in self.world.take_sent(&mut self.events) {
            if let Some(c) = self.client_of(unit) {
                if let Err(e) = out.queue(c, &bytes) {
                    self.tick_faults.push((c, WorldError::from(e)));
                }
            }
        }
        handlers::items::moves::update_pass(self, out);
        self.vitals_sync(out);
    }
}

impl<D: EventDispatch + TickHooks, W: WorldHost<D>> SimGame<D, W> {
    /// The client vitals sync (`combat/vitals.md` §5.1 rule 1): every
    /// flush with argument 1 runs `0x0052D980` for each client in game,
    /// before its buffers are sent, so these messages end the tick's
    /// batch. Single player flushes once after each tick that ran, so it
    /// runs here, at the end of the tick, in client list order. A player
    /// without a path record is at its staged position ([`UnitFacts`];
    /// (0, 0) when none is staged). Off unless the host's world turns it
    /// on (`WorldHost::vitals_sync`).
    fn vitals_sync(&mut self, out: &mut dyn MessageSink) {
        for c in self.clients() {
            let Some(sc) = self.clients.get(&c).copied() else {
                continue;
            };
            let staged = self
                .game
                .lists
                .client(sc)
                .and_then(|r| r.player)
                .and_then(|p| self.units.get(&p))
                .map_or((0, 0), |f| (f.pos.x as u16, f.pos.y as u16));
            let queued = out.has_queued(c);
            let Some(msgs) =
                self.world
                    .vitals_sync(&mut self.game, &mut self.events, sc, staged, queued)
            else {
                continue;
            };
            for m in msgs {
                if let Err(e) = out.queue(c, &m) {
                    self.tick_faults.push((c, WorldError::from(e)));
                }
            }
        }
    }
}
