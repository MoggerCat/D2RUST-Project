//! Client↔server messages (`specs/sim/intents-events.md`) through
//! `conformance::packets::replay_packets`:
//!
//! * `fixtures/packets-dispatch.jsonl` through `d2-server`'s dispatcher
//!   on a game with one player (the synthetic world of `d2-server`'s
//!   adapter tests): two point messages, accepted (result 0) and refused
//!   (result 1, `intents-events.md` §2.4 rule 3), one from a client in no
//!   game (dropped, §2.2 rule 1), a system message and a tick;
//! * `fixtures/packets-scripted.jsonl` through a scripted server, to
//!   prove the byte comparison reports exactly the changed, missing or
//!   extra message (METHODS M08);
//! * the `packets-raw-1` recordings in `traces/raw/` through
//!   `d2-server` (ignored: local).
//!
//! Both fixtures pass `check_packets.py` (rules R1–R7). No `Covers:`
//! claims: a claim here counts as verified against 1.14d
//! (`docs/COVERAGE.md` §2), which only a passing recording earns.

use std::path::PathBuf;

use conformance::packets::{replay_packets, DispatchServer, PacketServer, PacketStats};
use conformance::raw::{raw_files, Mismatch, RawRecording, PACKETS_RAW};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::seams::{ClientId, MessageSink, PlayerGate, Pos, SessionHandler};
use d2_sim::game::Game;
use d2_sim::units::lists::client_state;
use d2_sim::units::UnitType;

fn fixture(name: &str) -> RawRecording {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"));
    RawRecording::load(&path, PACKETS_RAW).unwrap_or_else(|e| panic!("{e}"))
}

/// The session code is Phase 5 (`d2-server::seams::SessionHandler`):
/// system messages queue nothing here.
#[derive(Default)]
struct NoSession(Vec<(ClientId, Vec<u8>)>);

impl SessionHandler for NoSession {
    fn system_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        _size: usize,
        _out: &mut dyn MessageSink,
    ) {
        self.0.push((client, msg.to_vec()));
    }
}

/// Transport client 0 plays a live player at (100, 100) in act 0.
fn server() -> DispatchServer<SimGame, ProtoSizes, NoSession> {
    let mut game = Game::new();
    game.lists.ensure_act(0).unwrap();
    let room = game.lists.create_room(0).unwrap();
    game.lists.activate_room(room).unwrap();
    let player = game.spawn_unit(UnitType::Player, Some(room), true).unwrap();
    let mut sim = SimGame::new(game);
    sim.join(0, Some(player), Some(room), client_state::IN_GAME)
        .unwrap();
    sim.set_player(
        player,
        PlayerFields {
            gate: PlayerGate {
                mode: 1,
                uninterruptable: false,
            },
            data: Some(PlayerData { last_accept: 0 }),
        },
    );
    sim.set_unit(
        player,
        UnitFacts {
            act: 0,
            pos: Pos { x: 100, y: 100 },
            owner: None,
        },
    );
    DispatchServer::new(sim, ProtoSizes, NoSession::default())
}

fn mismatch_of(rec: &RawRecording, server: &mut dyn PacketServer) -> Mismatch {
    match replay_packets(rec, server) {
        Err(e) => e
            .mismatch()
            .cloned()
            .unwrap_or_else(|| panic!("not a mismatch: {e}")),
        Ok(s) => panic!("perturbed recording replayed: {s:?}"),
    }
}

/// The record with `seq`.
fn by_seq(rec: &mut RawRecording, seq: u64) -> &mut serde_json::Value {
    rec.records
        .iter_mut()
        .find(|r| r["seq"] == seq)
        .unwrap_or_else(|| panic!("no seq {seq}"))
}

#[test]
fn d2_server_dispatch_replays_exactly() {
    let mut s = server();
    let stats = replay_packets(&fixture("packets-dispatch.jsonl"), &mut s)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stats,
        PacketStats {
            game_messages: 3,
            system_messages: 1,
            results: 2,
            ticks: 1,
            server_messages: 0,
        }
    );
    assert_eq!(s.session.0, vec![(0, vec![0x6B])]);
    assert_eq!(s.game.game.frame, 1);
}

#[test]
fn a_changed_result_or_dispatch_is_reported() {
    // The refused point message's result recorded as 0 (seq 6).
    let mut rec = fixture("packets-dispatch.jsonl");
    by_seq(&mut rec, 6)["code"] = 0.into();
    let m = mismatch_of(&rec, &mut server());
    assert_eq!((m.at, m.field.as_str()), (6, "code"), "{m}");
    // The accepted message shown as dropped (its dispatch removed): the
    // server dispatched it (reported at the message, seq 1).
    let mut rec = fixture("packets-dispatch.jsonl");
    rec.records.retain(|r| r["seq"] != 2 && r["seq"] != 3);
    let m = mismatch_of(&rec, &mut server());
    assert_eq!((m.at, m.field.as_str()), (1, "dispatch"), "{m}");
    // The message of client 7 (no game) shown as dispatched (seq 7).
    let mut rec = fixture("packets-dispatch.jsonl");
    let mut d = by_seq(&mut rec, 2).clone();
    d["seq"] = 70.into();
    let at = rec.records.iter().position(|r| r["seq"] == 8).unwrap();
    rec.records.insert(at, d);
    let m = mismatch_of(&rec, &mut server());
    assert_eq!((m.at, m.field.as_str()), (7, "dispatch"), "{m}");
}

#[test]
fn a_server_message_d2_server_does_not_send_is_reported() {
    // An S→C message in the tick: no d2-sim step sends one yet.
    let mut rec = fixture("packets-dispatch.jsonl");
    let at = rec
        .records
        .iter()
        .position(|r| r["type"] == "tick_end")
        .unwrap();
    rec.records.insert(
        at,
        serde_json::json!({"type": "s2c", "client": 0, "size": 2, "bytes": "1a05", "seq": 99}),
    );
    let m = mismatch_of(&rec, &mut server());
    assert_eq!((m.at, m.field.as_str()), (99, "s2c"), "{m}");
}

/// Answers each game message with S→C 0x1D (id, 0) and queues 0x1A
/// with the tick number on each tick; result 0.
#[derive(Default)]
struct Scripted {
    ticks: u8,
    sent: Vec<(ClientId, Vec<u8>)>,
}

impl PacketServer for Scripted {
    fn game_message(
        &mut self,
        client: ClientId,
        msg: &[u8],
        _size: usize,
        _now: u32,
    ) -> Result<Option<u8>, String> {
        self.sent.push((client, vec![0x1D, msg[0], 0]));
        Ok(Some(0))
    }
    fn system_message(&mut self, _client: ClientId, _msg: &[u8], _size: usize) {}
    fn tick(&mut self) {
        self.ticks += 1;
        self.sent.push((0, vec![0x1A, self.ticks]));
    }
    fn take_sent(&mut self) -> Vec<(ClientId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

#[test]
fn scripted_server_replays_exactly() {
    let rec = fixture("packets-scripted.jsonl");
    let stats = replay_packets(&rec, &mut Scripted::default()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        (stats.server_messages, stats.results, stats.ticks),
        (6, 4, 2)
    );
}

#[test]
fn every_flipped_byte_is_reported_at_its_message() {
    // As `check_packets.py --perturb N`: the last byte of message N ^ 0x5A.
    let base = fixture("packets-scripted.jsonl");
    let seqs: Vec<u64> = base
        .records
        .iter()
        .filter(|r| r["type"] == "s2c")
        .map(|r| r["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs.len(), 6);
    for seq in seqs {
        let mut rec = base.clone();
        let r = by_seq(&mut rec, seq);
        let mut b = r["bytes"].as_str().unwrap().to_owned();
        let last = u8::from_str_radix(&b[b.len() - 2..], 16).unwrap() ^ 0x5A;
        b.replace_range(b.len() - 2.., &format!("{last:02x}"));
        r["bytes"] = b.into();
        let n = r["size"].as_u64().unwrap() - 1;
        let m = mismatch_of(&rec, &mut Scripted::default());
        assert_eq!((m.at, m.field.clone()), (seq, format!("bytes[{n}]")), "{m}");
    }
}

#[test]
fn a_missing_or_extra_message_is_reported() {
    let base = fixture("packets-scripted.jsonl");
    // Missing from the recording: the server's extra message is reported
    // at the next input (the tick, seq 9).
    let mut rec = base.clone();
    rec.records.retain(|r| r["seq"] != 7);
    let m = mismatch_of(&rec, &mut Scripted::default());
    assert_eq!((m.at, m.field.as_str()), (9, "s2c"), "{m}");
    // Recorded but not queued by the server (and another client).
    let mut rec = base.clone();
    let mut extra = by_seq(&mut rec, 7).clone();
    extra["seq"] = 70.into();
    let at = rec.records.iter().position(|r| r["seq"] == 8).unwrap();
    rec.records.insert(at, extra);
    let m = mismatch_of(&rec, &mut Scripted::default());
    assert_eq!((m.at, m.field.as_str()), (70, "s2c"), "{m}");
    let mut rec = base.clone();
    by_seq(&mut rec, 7)["client"] = 1.into();
    let m = mismatch_of(&rec, &mut Scripted::default());
    assert_eq!((m.at, m.field.as_str()), (7, "client"), "{m}");
}

/// Every `packets-raw-1` recording in `traces/raw/` through `d2-server`
/// with an empty game. Expected to fail today at the first message the
/// original dispatched: no session code builds the recorded game yet
/// (`docs/handoff/conformance-harness.md`).
#[test]
#[ignore = "needs traces/raw/*-packets.jsonl (local)"]
fn recorded_messages_replay_exactly() {
    let files = raw_files("-packets.jsonl");
    assert!(!files.is_empty(), "no traces/raw/*-packets.jsonl");
    for path in &files {
        let rec = RawRecording::load(path, PACKETS_RAW).unwrap_or_else(|e| panic!("{e}"));
        let mut s =
            DispatchServer::new(SimGame::new(Game::new()), ProtoSizes, NoSession::default());
        let stats = replay_packets(&rec, &mut s).unwrap_or_else(|e| panic!("{e}"));
        println!("{}: {stats:?}", path.display());
    }
}
