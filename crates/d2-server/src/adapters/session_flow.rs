// Spec: specs/sim/intents-events.md §2.5, §8.1, §8.2; specs/sim/path-placement.md §13 rule 2
//! The single-player session sequence on the host's drain
//! (`intents-events.md` §8): C→S 0x67 (game creation, `0x0052C330` →
//! `0x00530BF0`) and C→S 0x6B (join, `0x0052C550` → `0x00530190`), both
//! queue-0 system messages (§2.5) that reach the game through
//! [`crate::seams::Intents::session_message`]. A [`SimGame`] runs them
//! only when it has a [`SessionFlow`] ([`SimGame::set_session`]); the
//! other system ids, and every id of a game without a flow, go to the
//! host's `SessionHandler`.
//!
//! **0x67** ([`SessionFlow::create`]): the request checks of `0x0052C330`
//! that the spec states (§2.5: u8@0x2D > 14, u32@0x27 with neither bit 1
//! nor bit 2, class u8@0x12 ≥ 7 → refused, nothing happens); then §8.1:
//! the client record is allocated and prepended (state 0,
//! [`SimGame::join`]), S→C 0x01 (difficulty u8@0x14, the arena flags,
//! expansion = flags bit 20, ladder = bit 21), S→C 0x00, client state 1,
//! S→C 0x02. The messages go straight to the client's buffers (there is
//! no player yet), so they leave with the next flush.
//!
//! **0x6B** ([`SessionFlow::join`]): §8.2 rule 1 (no client record →
//! stop); rule 2's load: a classic game with an expansion class (≥ 5) is
//! result 0x18, else the caller's [`CharacterLoader`] creates the player
//! (unplaced, at (0, 0)) and gives the join's [`Entry`] or a result code;
//! a non-zero result removes the client; then rules 3–6 through
//! [`super::session::enter_game`] (its messages go through the action
//! wiring's transport seam and are queued at the next tick, ahead of the
//! tick's own messages).
//!
//! Not done, because no spec gives it (named, not guessed):
//! - the name checks of `0x0052C330` (`0x0053EFC0(name, 16)` on both
//!   names, `0x00538B70`, `0x00538C60`);
//! - §8.1 rules 1–2's game record, acts, arena record and seeds: the
//!   caller builds the game before the flow runs (`rng.md` §5.2), and the
//!   0x01 arena flags come from [`SessionFlow::arena_flags`] (the arena
//!   record `0x0053F4B0` / `0x0053FD40` is not specified);
//! - the refusal's S→C 0xB4 (§8.2 rule 2, direct, `0x0053B260`): its
//!   5-byte layout is not in `server-messages.tsv`; the refusal is
//!   recorded in [`SessionFlow::faults`] and the client is removed;
//! - §8.2 rule 4's act build at the join (`0x0052C210`): d2rs builds the
//!   acts with the game, so a join into an act without a DRLG fails
//!   ([`JoinError::NoAct`]);
//! - the other system ids (0x69 leave `0x005303D0`, 0x6A, 0x6C, 0x6E,
//!   0x70) and 0x6D (ping, out of scope, §4 rule 4).

use std::collections::BTreeMap;

pub use d2_proto::client::CreateGame;
use d2_proto::FixedMessage;
use d2_sim::units::lists::client_state;
use d2_sim::units::messages as msg;
use d2_sim::units::UnitId;

use super::handlers::world::ActionEvents;
use super::session::{enter_game, Entry, GameSetup, JoinError};
use super::SimGame;
use crate::buffers::QueueError;
use crate::seams::{ClientId, MessageSink};

/// The u32@0x27 bits of 0x67 (§2.5).
pub mod create_flags {
    /// Bit 1 or bit 2 must be set.
    pub const REQUIRED: u32 = 0x2 | 0x4;
    /// Bit 20 → game +0x70 (expansion).
    pub const EXPANSION: u32 = 1 << 20;
    /// Bit 21 → game +0x74 (ladder).
    pub const LADDER: u32 = 1 << 21;
}

/// Load result of an expansion class in a classic game (§8.2 rule 2).
pub const LOAD_CLASSIC_EXPANSION_CLASS: u32 = 0x18;
/// The first expansion class (client +8 ≥ 5, §8.2 rule 2).
const FIRST_EXPANSION_CLASS: u8 = 5;
/// Class u8@0x12 must be below this (§2.5).
const CLASS_LIMIT: u8 = 7;
/// u8@0x2D above this is refused (§2.5).
const LOCALE_MAX: u8 = 14;

/// What a [`CharacterLoader`] gives the join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loaded {
    /// The player unit the load created: no room, at (0, 0).
    pub player: UnitId,
    /// The character's join values (act, name, hot keys, record).
    pub entry: Entry,
}

/// The character load of §8.2 rule 2 (`0x005345A0`: a save, or a new
/// character): creates the player of `client` from the 0x67 request and
/// returns it, or a non-zero load result (0x13, 0x14, 0x15, 0x17, 0x18).
pub type CharacterLoader<D, W> =
    Box<dyn FnMut(&mut SimGame<D, W>, ClientId, &CreateGame) -> Result<Loaded, u32>>;

/// Why 0x67's checks refused a request (§2.5; nothing happens).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateRefusal {
    /// u8@0x2D > 14.
    Locale(u8),
    /// u32@0x27 has neither bit 1 nor bit 2.
    Flags(u32),
    /// Class u8@0x12 ≥ 7.
    Class(u8),
}

/// A session step that did not complete, in drain order (d2rs
/// diagnostics; the original logs or does nothing).
#[derive(Debug, PartialEq, Eq)]
pub enum SessionFault {
    /// 0x67 refused by its checks.
    CreateRefused(CreateRefusal),
    /// 0x67 for a client that already has a record (d2rs runs one game
    /// per [`SimGame`]).
    AlreadyCreated,
    /// 0x6B without a client record (§8.2 rule 1: log, stop).
    NoClientRecord,
    /// §8.2 rule 2: the load gave a non-zero result; the client was
    /// removed (its S→C 0xB4 is not sent, module docs).
    LoadRefused(u32),
    /// The join failed after the load.
    Join(JoinError),
    /// A message could not be queued.
    Queue(QueueError),
}

/// The session sequence of one game (module docs).
pub struct SessionFlow<D, W> {
    /// The arena record's flags sent in 0x01's u32@2 (recorded 0x00100004
    /// in every join of both recordings).
    pub arena_flags: u32,
    loader: CharacterLoader<D, W>,
    /// The 0x67 request of each created client, read by its 0x6B.
    pub requests: BTreeMap<ClientId, CreateGame>,
    pub faults: Vec<(ClientId, SessionFault)>,
}

impl<D, W> SessionFlow<D, W> {
    pub fn new(arena_flags: u32, loader: CharacterLoader<D, W>) -> Self {
        Self {
            arena_flags,
            loader,
            requests: BTreeMap::new(),
            faults: Vec::new(),
        }
    }
}

/// §2.5's stated checks of `0x0052C330` (module docs for the rest).
pub fn check_create(r: &CreateGame) -> Result<(), CreateRefusal> {
    if r.locale > LOCALE_MAX {
        return Err(CreateRefusal::Locale(r.locale));
    }
    if r.flags & create_flags::REQUIRED == 0 {
        return Err(CreateRefusal::Flags(r.flags));
    }
    if r.class >= CLASS_LIMIT {
        return Err(CreateRefusal::Class(r.class));
    }
    Ok(())
}

/// S→C 0x01's game fields from a request (§8.1 rule 3, §2.5).
pub fn game_setup(r: &CreateGame, arena_flags: u32) -> GameSetup {
    GameSetup {
        difficulty: r.difficulty,
        arena_flags,
        expansion: r.flags & create_flags::EXPANSION != 0,
        ladder: r.flags & create_flags::LADDER != 0,
    }
}

impl<D: ActionEvents, W> SessionFlow<D, W> {
    /// One queue-0 message; false for the ids the flow does not run.
    pub fn message(
        &mut self,
        s: &mut SimGame<D, W>,
        client: ClientId,
        m: &[u8],
        out: &mut dyn MessageSink,
    ) -> bool {
        match m.first() {
            Some(&CreateGame::ID) => {
                // The transport checked the 46-byte size (§2.1).
                if let Ok(r) = CreateGame::decode(m) {
                    self.create(s, client, &r, out);
                }
                true
            }
            Some(&0x6B) => {
                self.join(s, client);
                true
            }
            _ => false,
        }
    }

    /// C→S 0x67 (module docs).
    pub fn create(
        &mut self,
        s: &mut SimGame<D, W>,
        client: ClientId,
        r: &CreateGame,
        out: &mut dyn MessageSink,
    ) {
        if let Err(e) = check_create(r) {
            self.faults.push((client, SessionFault::CreateRefused(e)));
            return;
        }
        // Rule 1: the client record, prepended, state 0.
        let Ok(id) = s.join(client, None, None, 0) else {
            self.faults.push((client, SessionFault::AlreadyCreated));
            return;
        };
        self.requests.insert(client, *r);
        let g = game_setup(r, self.arena_flags);
        let flags = msg::game_flags(g.difficulty, g.arena_flags, g.expansion, g.ladder);
        // Rules 3–6.
        let mut sent = out.queue(client, &flags);
        sent = sent.and_then(|_| out.queue(client, &msg::GAME_LOADING));
        if let Some(e) = s.game.lists.client_mut(id) {
            e.state = client_state::LOADING;
        }
        sent = sent.and_then(|_| out.queue(client, &msg::LOAD_SUCCESSFUL));
        if let Err(e) = sent {
            self.faults.push((client, SessionFault::Queue(e)));
        }
    }

    /// C→S 0x6B (module docs). Returns the placed player.
    pub fn join(&mut self, s: &mut SimGame<D, W>, client: ClientId) -> Option<UnitId> {
        let (Some(id), Some(r)) = (s.sim_client(client), self.requests.get(&client).copied())
        else {
            self.faults.push((client, SessionFault::NoClientRecord));
            return None;
        };
        let classic = r.flags & create_flags::EXPANSION == 0;
        let loaded = if classic && r.class >= FIRST_EXPANSION_CLASS {
            Err(LOAD_CLASSIC_EXPANSION_CLASS)
        } else {
            (self.loader)(s, client, &r)
        };
        let loaded = match loaded {
            Ok(l) => l,
            Err(code) => {
                // Cannot fail: the client is joined.
                let _ = s.leave(client);
                self.requests.remove(&client);
                self.faults.push((client, SessionFault::LoadRefused(code)));
                return None;
            }
        };
        if let Some(e) = s.game.lists.client_mut(id) {
            e.player = Some(loaded.player);
        }
        match enter_game(s, client, &loaded.entry) {
            Ok(p) => Some(p),
            Err(e) => {
                self.faults.push((client, SessionFault::Join(e)));
                None
            }
        }
    }
}

/// A [`SessionFlow`] behind [`SimGame`]'s `session` field, so the
/// game's `Intents` impl (no `ActionEvents` bound) can run it.
pub trait SessionRunner<D, W> {
    fn run(
        &mut self,
        s: &mut SimGame<D, W>,
        client: ClientId,
        m: &[u8],
        out: &mut dyn MessageSink,
    ) -> bool;
    /// The flow itself, for its requests and faults.
    fn flow(&self) -> &SessionFlow<D, W>;
}

impl<D: ActionEvents, W> SessionRunner<D, W> for SessionFlow<D, W> {
    fn run(
        &mut self,
        s: &mut SimGame<D, W>,
        client: ClientId,
        m: &[u8],
        out: &mut dyn MessageSink,
    ) -> bool {
        self.message(s, client, m, out)
    }
    fn flow(&self) -> &SessionFlow<D, W> {
        self
    }
}
