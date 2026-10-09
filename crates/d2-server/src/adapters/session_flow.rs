// Spec: specs/sim/intents-events.md §2.5, §8.1, §8.2; specs/sim/path-placement.md §13 rule 2; specs/flows/save-exit.md §2; specs/formats/d2s.md §2.4 r5
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
//! in their order (§2.5 rule 1, [`check_create`]; a refusal sends
//! nothing); then §8.1: the arena record's flags (u32@0x27 & 0x3179C7,
//! §8.1 rule 1), the client record allocated and prepended (state 0,
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
//! **0x69** leave, **0x6A** game list, **0x6C** save upload, **0x6E**,
//! **0x70**: §2.5 rules 2–6 ([`SessionFlow::leave`],
//! [`SessionFlow::game_list`], [`SessionFlow::upload`]).
//!
//! Not done, because no spec or provider gives it (named, not guessed):
//! - §8.1 rules 1–2's game record, acts and seeds: the caller builds the
//!   game before the flow runs (`rng.md` §5.2);
//! - the refusal's S→C 0xB4 (§8.2 rule 2, direct, `0x0053B260`): the
//!   refusal is recorded in [`SessionFlow::faults`] and the client is
//!   removed;
//! - §8.2 rule 4's act build at the join (`0x0052C210`): d2rs builds the
//!   acts with the game, so a join into an act without a DRLG fails
//!   ([`JoinError::NoAct`]);
//! - the leave's character saves (`0x0052CA10` → `0x00532400`): d2rs has
//!   no character-save writer; recorded as [`SessionFault::NotSaved`];
//! - 0x6D (ping, out of scope, §4 rule 4).

use std::collections::{BTreeMap, BTreeSet};

pub use d2_proto::client::CreateGame;
use d2_proto::FixedMessage;
use d2_sim::units::lists::client_state;
use d2_sim::units::messages as msg;
use d2_sim::units::UnitId;

use super::handlers::player::HotKey;
use super::handlers::world::ActionEvents;
use super::session::{enter_game, Entry, GameSetup, JoinError};
use super::storage::SaveFault;
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
    /// The bits the arena record keeps (+0x08, §8.1 rule 1).
    pub const ARENA: u32 = 0x31_79C7;
}

/// Load result of an expansion class in a classic game (§8.2 rule 2).
pub const LOAD_CLASSIC_EXPANSION_CLASS: u32 = 0x18;
/// The first expansion class (client +8 ≥ 5, §8.2 rule 2).
const FIRST_EXPANSION_CLASS: u8 = 5;
/// Class u8@0x12 must be below this (§2.5).
const CLASS_LIMIT: u8 = 7;
/// u8@0x2D above this is refused (§2.5).
const LOCALE_MAX: u8 = 14;
/// 0x6C: a total of this or more is a fatal assert (§2.5 table).
pub const UPLOAD_LIMIT: u32 = 0x2000;
/// S→C 0x5A code 3, a player left (§2.5 rule 2).
const EVENT_LEFT: u8 = 3;

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

/// Why 0x67's checks refused a request (§2.5 rule 1; nothing happens).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateRefusal {
    /// The character name (cstr16@0x15) has no NUL in its 16 bytes
    /// (`0x0053EFC0`).
    CharName,
    /// u8@0x2D > 14.
    Locale(u8),
    /// The game name (cstr16@1) has no NUL in its 16 bytes.
    GameName,
    /// u32@0x27 has neither bit 1 nor bit 2.
    Flags(u32),
    /// `0x00538B70`: the client already has a record (a game); the
    /// character name of that record.
    HasGame([u8; 16]),
    /// `0x00538C60`: another record has the same character name
    /// (`_strnicmp`, 16); that record's name.
    NameTaken([u8; 16]),
    /// Class u8@0x12 ≥ 7.
    Class(u8),
}

/// A session step that did not complete, in drain order (d2rs
/// diagnostics; the original logs or does nothing).
#[derive(Debug, PartialEq, Eq)]
pub enum SessionFault {
    /// 0x67 refused by its checks.
    CreateRefused(CreateRefusal),
    /// 0x6B without a client record (§8.2 rule 1: log, stop).
    NoClientRecord,
    /// §8.2 rule 2: the load gave a non-zero result; the client was
    /// removed (its S→C 0xB4 is not sent, module docs).
    LoadRefused(u32),
    /// The join failed after the load.
    Join(JoinError),
    /// A message could not be queued.
    Queue(QueueError),
    /// §2.5 rule 2: the leave's character save (`0x00532400`) of this
    /// client's player ran with no storage installed
    /// ([`SimGame::set_storage`]).
    NotSaved,
    /// §2.5 rule 2: the character storage refused the save.
    SaveFailed(String),
    /// §2.5 table: 0x6C with total ≥ 0x2000 (fatal assert).
    UploadTotal(u32),
    /// §2.5 rule 4: count + len > total (fatal 0xB2F).
    UploadOverflow { count: u32, len: u32, total: u32 },
}

/// A save upload in progress (client +0x17C buffer, +0x180 count; §2.5
/// rule 4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Upload {
    /// The `total` bytes, as received so far (+0x17C).
    pub buffer: Vec<u8>,
    pub total: u32,
    /// Received count (+0x180).
    pub count: u32,
    /// Client +0x3D4 bit 3: the upload is complete.
    pub complete: bool,
    /// +0x18C: the save checksum of the complete buffer (`0x00531E30`,
    /// `formats/d2s.md` §3).
    pub checksum: u32,
}

/// The session sequence of one game (module docs).
pub struct SessionFlow<D, W> {
    loader: CharacterLoader<D, W>,
    /// The 0x67 request of each client with a record, read by its 0x6B;
    /// its character name is the record's (+0x0D), the name table of
    /// `0x00538C60`.
    pub requests: BTreeMap<ClientId, CreateGame>,
    /// The game's name (game +0x2A, from the creating 0x67).
    pub game_name: Option<[u8; 16]>,
    /// Game +0x28, the game's id in the game table (u16@0x33 of 0xB2).
    ///
    /// PROVISIONAL (intents-events.md §2.5 r3; no REC): d2rs has no game
    /// table; 0 unless the caller sets it.
    pub game_id: u16,
    /// Save uploads by client (§2.5 rule 4).
    pub uploads: BTreeMap<ClientId, Upload>,
    /// Clients whose record +0x504 is set by 0x70 (§2.5 rule 6; read only
    /// by the host heartbeat).
    pub heartbeat_flag: BTreeSet<ClientId>,
    pub faults: Vec<(ClientId, SessionFault)>,
}

impl<D, W> SessionFlow<D, W> {
    pub fn new(loader: CharacterLoader<D, W>) -> Self {
        Self {
            loader,
            requests: BTreeMap::new(),
            game_name: None,
            game_id: 0,
            uploads: BTreeMap::new(),
            heartbeat_flag: BTreeSet::new(),
            faults: Vec::new(),
        }
    }
}

/// `_strnicmp(a, b, 16) == 0` (`0x00413590`): ASCII case-insensitive, up
/// to 16 bytes or the first NUL.
pub fn same_name(a: &[u8; 16], b: &[u8; 16]) -> bool {
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (x.to_ascii_lowercase(), y.to_ascii_lowercase());
        if x != y {
            return false;
        }
        if x == 0 {
            break;
        }
    }
    true
}

/// `0x0052C330`'s checks of §2.5 rule 1 in their order (single player:
/// no host callbacks, so u8@0x11 is not tested). `has_game`: the client's
/// record name when it has one (`0x00538B70`); `taken`: the name of a
/// record with the same character name (`0x00538C60`).
pub fn check_create(
    r: &CreateGame,
    has_game: Option<[u8; 16]>,
    taken: Option<[u8; 16]>,
) -> Result<(), CreateRefusal> {
    if !r.char_name.contains(&0) {
        return Err(CreateRefusal::CharName);
    }
    if r.locale > LOCALE_MAX {
        return Err(CreateRefusal::Locale(r.locale));
    }
    if !r.game_name.contains(&0) {
        return Err(CreateRefusal::GameName);
    }
    if r.flags & create_flags::REQUIRED == 0 {
        return Err(CreateRefusal::Flags(r.flags));
    }
    if let Some(n) = has_game {
        return Err(CreateRefusal::HasGame(n));
    }
    if let Some(n) = taken {
        return Err(CreateRefusal::NameTaken(n));
    }
    if r.class >= CLASS_LIMIT {
        return Err(CreateRefusal::Class(r.class));
    }
    Ok(())
}

/// S→C 0x01's game fields from a request (§8.1 rules 1, 3; §2.5): the
/// arena record's flags are u32@0x27 & 0x3179C7.
pub fn game_setup(r: &CreateGame) -> GameSetup {
    GameSetup {
        difficulty: r.difficulty,
        arena_flags: r.flags & create_flags::ARENA,
        expansion: r.flags & create_flags::EXPANSION != 0,
        ladder: r.flags & create_flags::LADDER != 0,
    }
}

/// The 40-byte S→C 0x5A of a join (code 2) or leave (code 3), shared
/// with the sim's join sequence (§2.5 rule 2, §8.3).
pub use d2_sim::units::messages::player_event;

/// The 53-byte S→C 0xB2 (`0x0053B1B0`, §2.5 rule 3): the name (16
/// bytes), u16@0x31, u16@0x33; bytes 0x11–0x30 are never written (d2rs:
/// zero).
pub fn game_list_entry(name: &[u8; 16], players: u16, id: u16) -> [u8; 53] {
    let mut m = [0u8; 53];
    m[0] = 0xB2;
    m[1..17].copy_from_slice(name);
    m[0x31..0x33].copy_from_slice(&players.to_le_bytes());
    m[0x33..0x35].copy_from_slice(&id.to_le_bytes());
    m
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
            Some(&0x69) => {
                self.leave(s, client, out);
                true
            }
            Some(&0x6A) => {
                self.game_list(s, client, out);
                true
            }
            Some(&0x6B) => {
                self.join(s, client);
                true
            }
            Some(&0x6C) => {
                self.upload(s, client, m);
                true
            }
            // Rule 5: no state change and no message in 1.14d.
            Some(&0x6E) => true,
            Some(&0x70) => {
                // Rule 6: the record found → +0x504 := 1.
                if s.sim_client(client).is_some() {
                    self.heartbeat_flag.insert(client);
                }
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
        let has_game = s
            .sim_client(client)
            .map(|_| self.requests.get(&client).map_or([0; 16], |q| q.char_name));
        let taken = self
            .requests
            .values()
            .map(|q| q.char_name)
            .find(|n| same_name(n, &r.char_name));
        if let Err(e) = check_create(r, has_game, taken) {
            self.faults.push((client, SessionFault::CreateRefused(e)));
            return;
        }
        // Rule 1: the client record, prepended, state 0 (checked absent
        // above, so this cannot fail).
        let Ok(id) = s.join(client, None, None, 0) else {
            return;
        };
        self.requests.insert(client, *r);
        self.game_name.get_or_insert(r.game_name);
        let g = game_setup(r);
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

    /// C→S 0x69 (§2.5 rule 2): only for a client in state 4. The
    /// characters of the game's clients with a player are saved
    /// (`0x0052CA10`; single player is game type 3, so always), then the
    /// leaving client gets S→C 0x05, 0x06, a direct 0xB0 and its buffers
    /// flushed; its record is removed; S→C 0x5C (its player GUID) then the 0x5A code 3 go to every
    /// remaining client in state 4 (client-list order) when the name has
    /// a NUL in its 16 bytes. Returns whether the client left.
    pub fn leave(
        &mut self,
        s: &mut SimGame<D, W>,
        client: ClientId,
        out: &mut dyn MessageSink,
    ) -> bool {
        let Some(id) = s.sim_client(client) else {
            return false;
        };
        if s.game.lists.client(id).map(|e| e.state) != Some(client_state::IN_GAME) {
            return false;
        }
        for (c, r) in s.save_characters() {
            match r {
                Ok(()) => {}
                Err(SaveFault::NoStorage) => self.faults.push((c, SessionFault::NotSaved)),
                Err(SaveFault::Failed(e)) => self.faults.push((c, SessionFault::SaveFailed(e))),
            }
        }
        let mut sent = out.queue(client, &[0x05]);
        sent = sent.and_then(|_| out.queue(client, &[0x06]));
        sent = sent.and_then(|_| out.send_direct(client, &[0xB0]));
        sent = sent.and_then(|_| out.flush_client(client));
        if let Err(e) = sent {
            self.faults.push((client, SessionFault::Queue(e)));
        }
        let name = self
            .requests
            .remove(&client)
            .map_or([0; 16], |r| r.char_name);
        self.uploads.remove(&client);
        self.heartbeat_flag.remove(&client);
        let guid = s
            .player_of(client)
            .and_then(|p| s.game.lists.unit(p))
            .map(|e| e.guid);
        // Cannot fail: the client is joined.
        let _ = s.leave(client);
        // The remaining clients in state 4, client-list order (the leaver
        // is already unlinked, so it gets neither message).
        let in_game: Vec<ClientId> = s
            .client_list()
            .into_iter()
            .filter(|&c| {
                s.sim_client(c)
                    .and_then(|i| s.game.lists.client(i))
                    .is_some_and(|e| e.state == client_state::IN_GAME)
            })
            .collect();
        // S→C 0x5C (`0x0052C500` → `0x0053CA90`) during the removal.
        if let Some(g) = guid {
            let m = msg::player_left(g);
            for &c in &in_game {
                if let Err(e) = out.queue(c, &m) {
                    self.faults.push((c, SessionFault::Queue(e)));
                }
            }
        }
        if name.contains(&0) {
            let m = player_event(EVENT_LEFT, &name);
            for &c in &in_game {
                if let Err(e) = out.queue(c, &m) {
                    self.faults.push((c, SessionFault::Queue(e)));
                }
            }
        }
        true
    }

    /// C→S 0x6A (§2.5 rule 3): a direct S→C 0xB2 for the game (d2rs runs
    /// one game per flow), then the terminator (empty name, 0, 0xFFFF).
    /// The client ignores 0xB2 (§3.4 rule 2).
    pub fn game_list(&mut self, s: &SimGame<D, W>, client: ClientId, out: &mut dyn MessageSink) {
        let mut sent = Ok(());
        if let Some(name) = self.game_name {
            // PROVISIONAL (intents-events.md §2.5 r3; no REC): game +0x8C
            // read as the game's client count (`monsters/ai.md` §5.2: "only
            // while game +0x8C ≤ 8" at a player join).
            let players = s.client_list().len() as u16;
            sent = out.send_direct(client, &game_list_entry(&name, players, self.game_id));
        }
        sent = sent.and_then(|_| out.send_direct(client, &game_list_entry(&[0; 16], 0, 0xFFFF)));
        if let Err(e) = sent {
            self.faults.push((client, SessionFault::Queue(e)));
        }
    }

    /// C→S 0x6C (§2.5 table and rule 4): one save chunk (len u8@1, total
    /// u32@2, data @6) appended to the client's upload; complete →
    /// client +0x3D4 bit 3 and the checksum. No client record → nothing.
    pub fn upload(&mut self, s: &SimGame<D, W>, client: ClientId, m: &[u8]) {
        let (Some(&len), Some(t)) = (m.get(1), m.get(2..6)) else {
            return;
        };
        let total = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
        if total >= UPLOAD_LIMIT {
            self.faults.push((client, SessionFault::UploadTotal(total)));
            return;
        }
        if s.sim_client(client).is_none() {
            return;
        }
        let u = self.uploads.entry(client).or_default();
        if u.count == 0 {
            *u = Upload {
                buffer: Vec::with_capacity(total as usize),
                total,
                ..Upload::default()
            };
        }
        let len = u32::from(len);
        if u.count + len > u.total {
            let f = SessionFault::UploadOverflow {
                count: u.count,
                len,
                total: u.total,
            };
            self.faults.push((client, f));
            return;
        }
        let data = m.get(6..6 + len as usize).unwrap_or(&[]);
        u.buffer.extend_from_slice(data);
        u.count += len;
        if u.count == u.total {
            u.complete = true;
            u.checksum = d2_formats::d2s::checksum(&u.buffer);
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
        // The load writes the client's hot-key slots (`formats/d2s.md`
        // §2.4 rules 5–6, `0x0056A283`): the slots the save reads back and
        // C→S 0x51 changes (`intents-events.md` §9 rule 12). Unbound slots
        // (skill −1) stay as a new record holds them.
        for (slot, k) in loaded.entry.hotkeys.iter().enumerate() {
            if k.skill >= 0 {
                let key = HotKey {
                    skill: k.skill,
                    left: k.flag,
                    item: k.item,
                };
                s.set_hotkey(client, slot, key);
            }
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
