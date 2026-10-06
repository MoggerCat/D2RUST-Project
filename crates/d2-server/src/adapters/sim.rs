// Spec: specs/sim/intents-events.md
//! [`Intents`] and [`Tick`] on `d2_sim::game::Game` (§2.2–§2.4, §4;
//! `tick.md` §3; client list order `unit-order.md` §7).
//!
//! What the seams read that `d2-sim` does not hold yet (unit mode, state
//! 54, player data, positions, owners, acts of units outside rooms) is
//! owned by unit, path and item specs not written yet. Until they are,
//! the caller stages those fields here ([`PlayerFields`], [`UnitFacts`]);
//! nothing here derives or defaults them. Intent handlers are stubs
//! (see [`SimGame`]'s `handle`).

use std::collections::BTreeMap;

use d2_sim::game::Game;
use d2_sim::tick::timer::TimerRun;
use d2_sim::tick::{self, EventDispatch, TickHooks};
use d2_sim::units::{ClientId as SimClient, RoomId, UnitId, UnitType};

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

/// The tick hooks: timer events go to `D`; every step body owned by an
/// unwritten spec keeps its `TickHooks` default (does nothing).
struct Steps<'a, D>(&'a mut D);

impl<D: EventDispatch> EventDispatch for Steps<'_, D> {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        self.0.run_event(game, run);
    }
}

impl<D: EventDispatch> TickHooks for Steps<'_, D> {}

/// One `d2-sim` game behind the `d2-server` seams.
///
/// Transport client ids (`d2-server`) and client records (`d2-sim` slots)
/// are mapped by [`SimGame::join`]. Timer events go to `events`.
pub struct SimGame<D = Unspecified> {
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
    /// Skill / combat handlers (`handlers::skills`); `None`: stubs.
    pub skills: Option<Box<dyn super::handlers::skills::SkillHost<D> + Send + Sync>>,
}

impl SimGame<Unspecified> {
    pub fn new(game: Game) -> Self {
        Self::with_events(game, Unspecified)
    }
}

impl<D: EventDispatch> SimGame<D> {
    pub fn with_events(game: Game, events: D) -> Self {
        Self {
            game,
            events,
            clients: BTreeMap::new(),
            transport_ids: BTreeMap::new(),
            players: BTreeMap::new(),
            units: BTreeMap::new(),
            resyncs: Vec::new(),
            unhandled: Vec::new(),
            skills: None,
        }
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
        self.game.lists.remove_client(id)?;
        Ok(())
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
}

impl<D: EventDispatch> Intents for SimGame<D> {
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

    /// Per-intent behaviour belongs to the system specs (movement,
    /// skills, items, NPCs, quests; §4 rule 1), none written yet. Every
    /// handler is a stub: it records the intent and returns 0, the "does
    /// nothing" result of 1.14d's stubs (§2.4 rule 2). TODO(stats spec):
    /// skill messages (0x05–0x11 except 0x0B) owe `pierce_idx` += 1
    /// (§2.4 rule 5).
    fn handle(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    ) -> ResultCode {
        if let Some(code) = super::handlers::skills::handle(self, client, msg, out) {
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
}

impl<D: EventDispatch> Tick for SimGame<D> {
    /// `d2_sim::tick::tick`. No step sends a message yet: every sender
    /// belongs to an unwritten spec, so `out` is unused.
    fn tick(&mut self, _out: &mut dyn MessageSink) {
        tick::tick(&mut self.game, &mut Steps(&mut self.events));
    }
}
