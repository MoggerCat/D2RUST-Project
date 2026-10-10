// Spec: specs/sim/intents-events.md
//! The narrow interfaces `d2-server` needs from other crates, defined here
//! so the transport and host loop can be built and tested before those
//! crates exist. Each trait names its provider; the adapters in
//! [`crate::adapters`] implement them on the real crates.
//!
//! | Trait | Provider |
//! |---|---|
//! | [`MessageSizes`] | `d2-proto` (size rules from `sim/*-messages.tsv`): [`crate::adapters::ProtoSizes`] |
//! | [`Intents`] | `d2-sim` (player gate state, intent handlers, client list): [`crate::adapters::SimGame`] |
//! | [`Tick`] | `d2-sim::tick` (one game tick): [`crate::adapters::SimGame`] |
//! | [`SessionHandler`] | `d2-server` session code (Phase 5) |
//! | [`Clock`] | host: [`crate::host::SystemClock`]; tests: a manual clock |

/// Client id as the transport carries it (spec Inputs: u32; 0 for the
/// local client).
pub type ClientId = u32;

/// Why a size rule gives no size (the original returns 0 for both).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeError {
    /// Table entry 0, or an id outside the table: never a valid message.
    Invalid,
    /// Fewer bytes than the rule needs, or a string without its NUL.
    Incomplete,
    /// The C→S chat rule (§2.1 rule 5) came out negative (signed byte).
    /// What the classifier `0x0052B100` does with it is not in the spec
    /// (`docs/HANDOFF.md` §7 question 6); callers must not treat it as
    /// either of the other results.
    Negative(i32),
}

/// Per-direction message size rules (spec §2.1 rule 5, §3.1 rule 1).
///
/// Provider: `d2-proto`, generated from the `transport_size` column of
/// `specs/sim/client-messages.tsv` and the `size` column of
/// `specs/sim/server-messages.tsv`.
pub trait MessageSizes {
    /// C→S size rule `0x0052BC20` for ids 0x00..=0x70 (`msg[0]` is the id,
    /// `msg` is non-empty). Ids ≥ 0x71 never reach it: the classifier
    /// handles them (0x71..=0xFE invalid, 0xFF fixed at 16 bytes).
    fn client_size(&self, msg: &[u8]) -> Result<usize, SizeError>;

    /// S→C size rule `0x0052B920` (`msg` non-empty); ids ≥ 0xB5 are
    /// [`SizeError::Invalid`].
    fn server_size(&self, msg: &[u8]) -> Result<usize, SizeError>;
}

/// Dispatch result code (spec §2.3). 1.14d ignores it (§2.2 rule 5);
/// d2rs keeps it as a diagnostic only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ResultCode {
    /// 0: done (also "gate refused" and the stub0 handlers).
    Done = 0,
    /// 1: refused (target missing, out of range, wrong state).
    Refused = 1,
    /// 2: invalid field (bad unit type, wrong act, out of range index).
    Invalid = 2,
    /// 3: malformed (wrong size, stub, bad id).
    Malformed = 3,
}

/// The player of a client as the gate sees it (spec §2.2 rule 4, §2.3
/// rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerLookup {
    /// The client is in no game (`0x0052FEE0` fails): dropped and logged.
    NotInGame,
    /// In a game, but no player unit (or the unit is not unit type 0):
    /// dropped.
    NoPlayer,
    /// A player unit.
    Player(PlayerGate),
}

/// The player fields the dispatch gate reads (spec §2.3 rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerGate {
    /// Unit mode (unit +0x10): 0 = death, 0x11 = dead.
    pub mode: u32,
    /// Has state 0x36 (54, uninterruptable).
    pub uninterruptable: bool,
}

/// Subtile position (spec §2.4 rules 3–4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

/// What the point-message parser reads from the player (spec §2.4 rule 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointState {
    /// Player position (dynamic path, or static path for unit types 2, 4,
    /// 5).
    pub player: Pos,
    /// Player data +0x168: frame of the last accepted point target.
    pub last_accept: i32,
}

/// Result of the unit-target lookup `0x00548F80` before the range test
/// (spec §2.4 rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitTarget {
    /// No such unit → 1.
    Missing,
    /// An item the player owns → accept without a range test.
    OwnedItem,
    /// Target in another act than the player → 2.
    OtherAct,
    /// Both positions, for the 50-subtile test (→ 1 when out of range).
    At { player: Pos, target: Pos },
}

/// Receives the server's messages to clients (spec §3.2 rule 1, D2MOO
/// `D2GAME_PACKETS_SendPacket`). Implemented by
/// [`crate::buffers::ClientBuffers`].
pub trait MessageSink {
    /// Queue `msg` for `client`. An unknown client is ignored (spec §3.2
    /// rule 1: "client null → nothing").
    fn queue(&mut self, client: ClientId, msg: &[u8]) -> Result<(), crate::buffers::QueueError>;

    /// The client has a queued buffer (head, client +0x1B8 ≠ 0; read by
    /// the vitals sync's force rule, `combat/vitals.md` §5.1 rule 2).
    /// Default: no.
    fn has_queued(&self, _client: ClientId) -> bool {
        false
    }

    /// A direct send (spec §3.3 rule 5): straight to the client's receive
    /// lists, ahead of anything still buffered. Default: refused (a sink
    /// with no receive lists cannot do it; never a silent queue).
    fn send_direct(
        &mut self,
        _client: ClientId,
        msg: &[u8],
    ) -> Result<(), crate::buffers::QueueError> {
        Err(crate::buffers::QueueError::NoDirect(
            msg.first().copied().unwrap_or(0),
        ))
    }

    /// Flushes one client's buffers into its receive lists
    /// (`0x0052E320(game, 0)`, the leave of §2.5 rule 2). Default: refused.
    fn flush_client(&mut self, _client: ClientId) -> Result<(), crate::buffers::QueueError> {
        Err(crate::buffers::QueueError::NoDirect(0))
    }
}

/// Game state the dispatcher reads and the intent handlers.
///
/// Provider: `d2-sim` (player units, player data, the game's client list
/// of `sim/unit-order.md` §7, and one handler per C→S id).
pub trait Intents {
    /// §2.2 rules 1 and 4: the client's game and player.
    fn player(&self, client: ClientId) -> PlayerLookup;

    /// The game frame counter (`tick.md` §2, game +0xA8).
    fn frame(&self) -> i32;

    /// Before the point / unit parse (spec §2.4 rules 3–4) of message
    /// `msg`: the provider brings the facts the parse reads (the
    /// client's player and, for a unit message, the target) up to date.
    /// Default: nothing (the facts are staged by the caller).
    fn refresh_targets(&mut self, client: ClientId, msg: &[u8]) {
        let _ = (client, msg);
    }

    /// Point parser inputs (spec §2.4 rule 3); `None` when the player has
    /// no player data (→ 2).
    fn point_state(&self, client: ClientId) -> Option<PointState>;

    /// Records an accepted point target: player data +0x168 = `frame`.
    fn set_point_accept(&mut self, client: ClientId, frame: i32);

    /// Queues S→C 0x15 (reassign player) to resync `client` (spec §2.4
    /// rule 3).
    fn queue_resync(&mut self, client: ClientId, out: &mut dyn MessageSink);

    /// Unit-target lookup for unit messages (spec §2.4 rule 4), called
    /// with a unit type already checked < 6.
    fn unit_target(&self, client: ClientId, unit_type: u32, unit_id: u32) -> UnitTarget;

    /// The intent handler for an id with kind `handler` (the
    /// `client-messages.tsv` `kind` column), called after the gate, the
    /// size check and the server-side parse (spec §2.4, §4 rule 1).
    /// `msg` is the drained copy (at most 0x1FC bytes, §2.1 rule 7);
    /// `size` is the full size. Point and unit messages arrive already
    /// range-checked; skill messages still owe the `pierce_idx` increment
    /// (§2.4 rule 5).
    fn handle(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    ) -> ResultCode;

    /// The game's client list in list order (`sim/unit-order.md` §7):
    /// the flush order (spec §3.2 rule 3).
    fn clients(&self) -> Vec<ClientId>;

    /// The game's part of a drained queue-0 message (spec §2.5, §8: the
    /// single-player session sequence C→S 0x67 → 0x6B). True when the
    /// game handled it; false hands it to the host's [`SessionHandler`].
    /// Default: the game handles none.
    fn session_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    ) -> bool {
        let _ = (client, msg, size, out);
        false
    }

    /// The host's millisecond clock of this frame, read once per frame
    /// before the drain (`Host::frame`): the `GetTickCount` input of the
    /// object code (`world/objects.md` edge case 9; `d2-sim` never reads a
    /// clock). Default: the game has no reader.
    fn set_host_tick(&mut self, ms: u32) {
        let _ = ms;
    }
}

/// One game tick (`tick.md` §3: frame += 1, then the steps).
///
/// Provider: `d2-sim::tick`.
pub trait Tick {
    /// Runs one tick; events go to `out` in production order.
    fn tick(&mut self, out: &mut dyn MessageSink);

    /// The per-client work of the flush routine `0x0052E320` that runs
    /// before a client's buffers are sent (`combat/vitals.md` §5.1 rule
    /// 1: the vitals sync, from `0x0052D980`). Called once per flush that
    /// is not throttled away, after the flush is announced and before any
    /// buffer is sent. Default: nothing.
    fn flush_sync(&mut self, _out: &mut dyn MessageSink) {}
}

/// System messages 0x67..=0x70 (spec §2.5). Provider: `d2-server`'s
/// session code (game creation, join, leave, save upload; Phase 5).
pub trait SessionHandler {
    /// One drained queue-0 message (`msg` truncated as in §2.1 rule 7).
    fn system_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        out: &mut dyn MessageSink,
    );
}

/// Millisecond wall clock (`timeGetTime` / `GetTickCount`; host only,
/// `tick.md` §8). Injected so tests are deterministic.
pub trait Clock {
    /// Milliseconds since an arbitrary origin, wrapping at 2^32.
    fn now_ms(&mut self) -> u32;
}
