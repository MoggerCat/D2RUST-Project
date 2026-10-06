// Spec: specs/sim/intents-events.md §1, §2.2, §2.3, §3.1
//! Client↔server messages of a packets recording (`packets-raw-1`,
//! `record_packets.py`; the format `check_packets.py` reads) replayed
//! through a [`PacketServer`]: the recorded client messages go in as the
//! server's drain read them, the recorded ticks run, and every
//! server→client message the server queues is compared byte for byte
//! with the recorded `s2c` records (METHODS M01).
//!
//! Records used (`tools/trace-recorder/README.md`, record_packets):
//!
//! * `c2s` / `c2s_sys`: a message taken from the game / system queue
//!   (client, size, bytes; at most 0x1FC bytes are logged, the drain
//!   buffer's size) → [`PacketServer::game_message`] /
//!   [`PacketServer::system_message`];
//! * `dispatch` + `result` (`dispatched: true`): the message reached the
//!   dispatcher and its handler returned `code` (`intents-events.md`
//!   §2.3) → compared with what `game_message` returned (`None`: dropped
//!   before dispatch, §2.2);
//! * `tick` → [`PacketServer::tick`];
//! * `s2c`: a message queued for a client (client, size, bytes).
//!
//! The recorded `s2c` records between two inputs (a client message or a
//! tick) are the window of the earlier input; at every input and at the
//! end, [`PacketServer::take_sent`] must return exactly that window:
//! same clients, same bytes, same order. The first difference is
//! reported by the recorded `seq` (the `s2c` that differs or is missing;
//! for a message the server queued and the recording has not, the `seq`
//! of the input after the window). Flush and delivery (`net` records,
//! rules R6–R7) are `check_packets.py`'s and `d2-server`'s own tests'.
//!
//! [`DispatchServer`] is the provider on `d2-server`: its
//! `process_game_message` (the dispatch gate, size checks, parsers and
//! intent handlers of `adapters::SimGame`) and the game's `Tick`, with
//! every message queued through `MessageSink` captured in order.

use std::collections::BTreeMap;

use d2_server::dispatch::{process_game_message, ClientRecord, Outcome};
use d2_server::seams::{ClientId, Intents, MessageSink, MessageSizes, SessionHandler, Tick};

use crate::raw::{bad, encode_hex, Fields, HarnessError, Mismatch, RawRecording, PACKETS_RAW};

/// The system under test.
pub trait PacketServer {
    /// A game-queue message as the drain read it (`size` is the
    /// message's size; `msg` may be shorter when the recorder truncated
    /// it). Returns the handler's result code, or `None` when the
    /// message was dropped before dispatch (§2.2). `now_ms` is the
    /// recording's clock at the read.
    fn game_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        now_ms: u32,
    ) -> Result<Option<u8>, String>;
    /// A system-queue message (ids 0x67..=0x70, §2.5).
    fn system_message(&mut self, client: ClientId, msg: &[u8], size: usize);
    /// One game tick (`tick.md` §3).
    fn tick(&mut self);
    /// The server→client messages queued since the last call, in order.
    fn take_sent(&mut self) -> Vec<(ClientId, Vec<u8>)>;
}

/// What a replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacketStats {
    pub game_messages: usize,
    pub system_messages: usize,
    /// Result codes compared.
    pub results: usize,
    pub ticks: usize,
    /// Server→client messages compared.
    pub server_messages: usize,
}

/// One recorded `s2c`.
struct Sent {
    seq: u64,
    client: ClientId,
    size: usize,
    bytes: Vec<u8>,
}

/// The dispatch the recording shows for the last client message.
enum Pending {
    /// No client message yet, or a system message.
    None,
    /// A game message, the server's result, the recording's so far.
    Game {
        seq: u64,
        ours: Option<u8>,
        recorded_dispatch: bool,
    },
}

/// Replays a packets recording through `server`.
pub fn replay_packets(
    rec: &RawRecording,
    server: &mut dyn PacketServer,
) -> Result<PacketStats, HarnessError> {
    if rec.format != PACKETS_RAW {
        return Err(bad(&rec.name, 0, format!("not a {PACKETS_RAW} recording")));
    }
    let name = rec.name.as_str();
    let mismatch = |at: u64, field: &str, detail: String| {
        HarnessError::Mismatch(Mismatch {
            source: name.to_owned(),
            at,
            field: field.to_owned(),
            detail,
        })
    };
    let mut stats = PacketStats::default();
    let mut window: Vec<Sent> = Vec::new();
    let mut pending = Pending::None;

    // Closes the window before the input at `seq` (u64::MAX: the end).
    let close = |server: &mut dyn PacketServer,
                 window: &mut Vec<Sent>,
                 pending: &mut Pending,
                 stats: &mut PacketStats,
                 seq: u64|
     -> Result<(), HarnessError> {
        if let Pending::Game {
            seq: at,
            ours: Some(code),
            recorded_dispatch: false,
        } = *pending
        {
            return Err(mismatch(
                at,
                "dispatch",
                format!("the server dispatched the message (result {code}); the recording shows no dispatch"),
            ));
        }
        *pending = Pending::None;
        let ours = server.take_sent();
        for (k, w) in window.iter().enumerate() {
            let Some((client, bytes)) = ours.get(k) else {
                return Err(mismatch(
                    w.seq,
                    "s2c",
                    format!(
                        "recorded message {} to client {} (size {}) not queued by the server",
                        encode_hex(&w.bytes),
                        w.client,
                        w.size
                    ),
                ));
            };
            if *client != w.client {
                return Err(mismatch(
                    w.seq,
                    "client",
                    format!("recorded client {}, server {client}", w.client),
                ));
            }
            if bytes.len() != w.size {
                return Err(mismatch(
                    w.seq,
                    "size",
                    format!(
                        "recorded size {}, server {} ({} vs {})",
                        w.size,
                        bytes.len(),
                        encode_hex(&w.bytes),
                        encode_hex(bytes)
                    ),
                ));
            }
            if let Some(b) = (0..w.bytes.len()).find(|&b| w.bytes[b] != bytes[b]) {
                return Err(mismatch(
                    w.seq,
                    &format!("bytes[{b}]"),
                    format!(
                        "recorded {}, server {}",
                        encode_hex(&w.bytes),
                        encode_hex(bytes)
                    ),
                ));
            }
            stats.server_messages += 1;
        }
        if let Some((client, bytes)) = ours.get(window.len()) {
            return Err(mismatch(
                seq,
                "s2c",
                format!(
                    "server queued {} to client {client} beyond the {} recorded messages before this record",
                    encode_hex(bytes),
                    window.len()
                ),
            ));
        }
        window.clear();
        Ok(())
    };

    for (i, r) in rec.records.iter().enumerate() {
        let f = Fields { name, i, r };
        let kind = r["type"].as_str().unwrap_or("");
        if matches!(kind, "header" | "footer") {
            continue;
        }
        let seq = r["seq"].as_u64().ok_or_else(|| bad(name, i, "no seq"))?;
        match kind {
            "c2s" | "c2s_sys" | "tick" => {
                close(server, &mut window, &mut pending, &mut stats, seq)?;
                if kind == "tick" {
                    stats.ticks += 1;
                    server.tick();
                    continue;
                }
                let client = f.u32("client")?;
                let size = usize::try_from(f.u32("size")?).unwrap_or(usize::MAX);
                let bytes = f.bytes("bytes")?;
                if kind == "c2s_sys" {
                    stats.system_messages += 1;
                    server.system_message(client, &bytes, size);
                    continue;
                }
                stats.game_messages += 1;
                let now = r["ms"].as_f64().map_or(0, |ms| ms as u32);
                let ours = server
                    .game_message(client, &bytes, size, now)
                    .map_err(|e| mismatch(seq, "dispatch", format!("server failed: {e}")))?;
                pending = Pending::Game {
                    seq,
                    ours,
                    recorded_dispatch: false,
                };
            }
            "dispatch" => match &mut pending {
                Pending::Game {
                    ours: None,
                    seq: at,
                    ..
                } => {
                    return Err(mismatch(
                        *at,
                        "dispatch",
                        format!("recorded dispatch (seq {seq}); the server dropped the message"),
                    ))
                }
                Pending::Game {
                    recorded_dispatch, ..
                } => *recorded_dispatch = true,
                Pending::None => return Err(bad(name, i, "dispatch without a client message")),
            },
            "result" if f.bool("dispatched").unwrap_or(false) => {
                let code = f.i64("code")?;
                if let Pending::Game {
                    ours: Some(ours), ..
                } = pending
                {
                    if i64::from(ours) != code {
                        return Err(mismatch(
                            seq,
                            "code",
                            format!("recorded result {code}, server {ours}"),
                        ));
                    }
                    stats.results += 1;
                }
            }
            "s2c" => window.push(Sent {
                seq,
                client: f.u32("client")?,
                size: usize::try_from(f.u32("size")?).unwrap_or(usize::MAX),
                bytes: f.bytes("bytes")?,
            }),
            _ => {}
        }
    }
    close(server, &mut window, &mut pending, &mut stats, u64::MAX)?;
    Ok(stats)
}

/// Captures every queued server→client message, in order.
#[derive(Debug, Default)]
pub struct Capture(pub Vec<(ClientId, Vec<u8>)>);

impl MessageSink for Capture {
    fn queue(
        &mut self,
        client: ClientId,
        msg: &[u8],
    ) -> Result<(), d2_server::buffers::QueueError> {
        self.0.push((client, msg.to_vec()));
        Ok(())
    }
}

/// [`PacketServer`] on `d2-server`'s dispatcher and a game.
pub struct DispatchServer<G, S, H> {
    pub game: G,
    pub sizes: S,
    pub session: H,
    pub records: BTreeMap<ClientId, ClientRecord>,
    pub sent: Capture,
}

impl<G, S, H> DispatchServer<G, S, H> {
    pub fn new(game: G, sizes: S, session: H) -> Self {
        Self {
            game,
            sizes,
            session,
            records: BTreeMap::new(),
            sent: Capture::default(),
        }
    }
}

impl<G, S, H> PacketServer for DispatchServer<G, S, H>
where
    G: Intents + Tick,
    S: MessageSizes,
    H: SessionHandler,
{
    fn game_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        size: usize,
        now_ms: u32,
    ) -> Result<Option<u8>, String> {
        // The host registers a client's record when it connects
        // (`Host::connect`); the recording starts after that.
        self.records.entry(client).or_default();
        let outcome = process_game_message(
            &mut self.game,
            &self.sizes,
            &mut self.records,
            &mut self.sent,
            client,
            msg,
            size,
            now_ms,
        )
        .map_err(|e| e.to_string())?;
        Ok(match outcome {
            Outcome::Dispatched(code) => Some(code as u8),
            Outcome::NotInGame | Outcome::NoPlayer => None,
        })
    }

    fn system_message(&mut self, client: ClientId, msg: &[u8], size: usize) {
        self.session
            .system_message(client, msg, size, &mut self.sent);
    }

    fn tick(&mut self) {
        self.game.tick(&mut self.sent);
    }

    fn take_sent(&mut self) -> Vec<(ClientId, Vec<u8>)> {
        std::mem::take(&mut self.sent.0)
    }
}
