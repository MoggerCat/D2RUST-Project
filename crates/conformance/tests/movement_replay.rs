//! Walk / run replay (`conformance::movement`) on a synthetic packets
//! recording with a test mover, every perturbation reported at the changed
//! record (METHODS M08), and the file-driven test over `traces/raw/`.
//!
//! `fixtures/movement-walk.jsonl` is hand-built (our own numbers; passes
//! `check_packets.py`): game entry 0x15 at (100, 100), then walk to
//! (105, 100) (the `pathing.md` M1 track: 14 ticks), an idle tick, run to
//! (103, 104), walk and run to unit 1:7 at (110, 98); one 0x96 per tick
//! while moving, one other C→S input (0x13) and other S→C (0x07) between.
//! [`TestMover`] moves each axis by at most 0x6000 (walk) / 0x9000 (run)
//! per tick: a stand-in with M1's numbers, not the d2-sim rules.

use std::collections::BTreeMap;
use std::path::PathBuf;

use conformance::movement::{
    read_movement, replay_movement, MoveOptions, MoveRequest, MoveStats, MovementRecording, Mover,
    NotWired, Precise, NOT_WIRED,
};
use conformance::raw::{raw_files, Mismatch, RawRecording, PACKETS_RAW, TICK_RAW};
use d2_proto::server::WalkVerify;
use d2_proto::FixedMessage;

fn fixture() -> RawRecording {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/movement-walk.jsonl");
    RawRecording::load(&path, PACKETS_RAW).unwrap()
}

/// The test mover: one player per client, 0x96 to its client each tick it
/// moves (stamina 0x1234, dx = dy = 0, as the fixture).
#[derive(Default)]
struct TestMover {
    units: BTreeMap<(u8, u32), Precise>,
    players: BTreeMap<u32, u32>,
    target: Option<(u32, Precise, u32)>,
    sent: Vec<(u32, Vec<u8>)>,
    /// Do not queue messages (positions-only replays).
    silent: bool,
}

impl TestMover {
    fn new() -> Self {
        let mut m = Self::default();
        m.units.insert((1, 7), Precise::centre(110, 98));
        m
    }
}

impl Mover for TestMover {
    fn seed(&mut self, client: u32, ut: u8, guid: u32, x: u16, y: u16) -> Result<(), String> {
        self.units.insert((ut, guid), Precise::centre(x, y));
        if ut == 0 {
            self.players.insert(client, guid);
        }
        Ok(())
    }

    fn request(&mut self, client: u32, r: &MoveRequest) -> Result<(), String> {
        let t = match *r {
            MoveRequest::Walk { x, y } | MoveRequest::Run { x, y } => Precise::centre(x, y),
            MoveRequest::WalkToUnit { unit_type, guid }
            | MoveRequest::RunToUnit { unit_type, guid } => *self
                .units
                .get(&(unit_type as u8, guid))
                .ok_or_else(|| format!("no unit {unit_type}:{guid}"))?,
        };
        let v = if r.runs() { 0x9000 } else { 0x6000 };
        self.target = Some((client, t, v));
        Ok(())
    }

    fn tick(&mut self) -> Result<(), String> {
        let Some((client, t, v)) = self.target else {
            return Ok(());
        };
        let guid = self.players[&client];
        let p = self.units.get_mut(&(0, guid)).unwrap();
        let step = |from: u32, to: u32| -> u32 {
            let d = i64::from(to) - i64::from(from);
            (i64::from(from) + d.clamp(-i64::from(v), i64::from(v))) as u32
        };
        if t.x.abs_diff(p.x) <= v && t.y.abs_diff(p.y) <= v {
            *p = t;
            self.target = None;
        } else {
            *p = Precise {
                x: step(p.x, t.x),
                y: step(p.y, t.y),
            };
        }
        let (x, y) = p.sub_tile();
        if !self.silent {
            let m = WalkVerify {
                stamina: 0x1234,
                x,
                y,
                dx: 0,
                dy: 0,
            };
            self.sent.push((client, m.encode().to_vec()));
        }
        Ok(())
    }

    fn take_sent(&mut self) -> Vec<(u32, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }

    fn position(&self, ut: u8, guid: u32) -> Option<Precise> {
        self.units.get(&(ut, guid)).copied()
    }
}

fn replay(rec: &RawRecording, opts: MoveOptions) -> Result<MoveStats, Mismatch> {
    let m = read_movement(rec).unwrap();
    replay_movement(&m, &mut TestMover::new(), opts).map_err(|e| {
        e.mismatch()
            .cloned()
            .unwrap_or_else(|| panic!("not a mismatch: {e}"))
    })
}

fn seqs_of_96(rec: &RawRecording) -> Vec<u64> {
    rec.records
        .iter()
        .filter(|r| r["type"] == "s2c" && r["bytes"].as_str().is_some_and(|b| b.starts_with("96")))
        .map(|r| r["seq"].as_u64().unwrap())
        .collect()
}

fn by_seq(rec: &mut RawRecording, seq: u64) -> &mut serde_json::Value {
    rec.records.iter_mut().find(|r| r["seq"] == seq).unwrap()
}

/// Rewrites the 0x96 at `seq` with x + 1.
fn bump_x(rec: &mut RawRecording, seq: u64) {
    let r = by_seq(rec, seq);
    let b = r["bytes"].as_str().unwrap();
    let bytes: Vec<u8> = (0..b.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&b[i..i + 2], 16).unwrap())
        .collect();
    let mut m = WalkVerify::decode(&bytes).unwrap();
    m.x += 1;
    r["bytes"] = m
        .encode()
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect::<String>()
        .into();
}

#[test]
fn reader_keeps_the_movement_events() {
    let m: MovementRecording = read_movement(&fixture()).unwrap();
    assert_eq!(m.read.requests, 4);
    assert_eq!(m.read.ticks, 45);
    assert_eq!(m.read.seeds, 1);
    assert_eq!(m.read.other_inputs, 1);
    assert_eq!(m.read.sent.get(&0x96), Some(&42));
    assert_eq!(m.read.sent.len(), 1);
}

#[test]
fn walk_fixture_replays_exactly() {
    let s = replay(&fixture(), MoveOptions::default()).unwrap_or_else(|m| panic!("{m}"));
    assert_eq!(
        s,
        MoveStats {
            requests: 4,
            ticks: 45,
            seeds: 1,
            messages: 42,
            positions: 42
        }
    );
}

#[test]
fn walk_follows_the_m1_track() {
    // pathing.md M1: x per tick 0x648000 + k·0x6000 (k = 1..13), then
    // 0x698000; the fixture's first 14 0x96 carry those sub-tiles.
    let rec = fixture();
    let xs: Vec<u16> = seqs_of_96(&rec)[..14]
        .iter()
        .map(|&s| {
            let r = rec.records.iter().find(|r| r["seq"] == s).unwrap();
            let b = r["bytes"].as_str().unwrap();
            let bytes: Vec<u8> = (0..b.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&b[i..i + 2], 16).unwrap())
                .collect();
            WalkVerify::decode(&bytes).unwrap().x
        })
        .collect();
    let want: Vec<u16> = (1..=13u32)
        .map(|k| ((0x648000 + k * 0x6000) >> 16) as u16)
        .chain([0x69])
        .collect();
    assert_eq!(xs, want);
}

#[test]
fn every_changed_position_is_reported_at_its_record() {
    let base = fixture();
    let seqs = seqs_of_96(&base);
    assert_eq!(seqs.len(), 42);
    for &seq in &seqs {
        let mut rec = base.clone();
        bump_x(&mut rec, seq);
        let m = replay(&rec, MoveOptions::default()).unwrap_err();
        assert_eq!((m.at, m.field.as_str()), (seq, "WalkVerify.x"), "{m}");
        // Positions only: the same record, as a position.
        let opts = MoveOptions {
            messages: false,
            positions: true,
        };
        let m = replay(&rec, opts).unwrap_err();
        assert_eq!((m.at, m.field.as_str()), (seq, "position.x"), "{m}");
    }
}

#[test]
fn positions_only_needs_no_messages() {
    let m = read_movement(&fixture()).unwrap();
    let mut mover = TestMover::new();
    mover.silent = true;
    let opts = MoveOptions {
        messages: false,
        positions: true,
    };
    let s = replay_movement(&m, &mut mover, opts).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((s.messages, s.positions), (0, 42));
    // With messages compared, the silent mover misses the first 0x96.
    let mut mover = TestMover::new();
    mover.silent = true;
    let e = replay_movement(&m, &mut mover, MoveOptions::default()).unwrap_err();
    let first = seqs_of_96(&fixture())[0];
    let mm = e.mismatch().unwrap();
    assert_eq!((mm.at, mm.field.as_str()), (first, "s2c"), "{mm}");
}

#[test]
fn a_missing_or_extra_message_is_reported() {
    let base = fixture();
    let seq = seqs_of_96(&base)[3];
    // Missing from the recording: the mover's message is extra, reported
    // at the next input (the following tick).
    let mut rec = base.clone();
    rec.records.retain(|r| r["seq"] != seq);
    let next_tick = base
        .records
        .iter()
        .filter(|r| r["type"] == "tick")
        .map(|r| r["seq"].as_u64().unwrap())
        .find(|&s| s > seq)
        .unwrap();
    let m = replay(&rec, MoveOptions::default()).unwrap_err();
    assert_eq!((m.at, m.field.as_str()), (next_tick, "s2c"), "{m}");
    // Recorded twice: the copy is not queued by the mover.
    let mut rec = base.clone();
    let mut copy = by_seq(&mut rec, seq).clone();
    copy["seq"] = 9000.into();
    let at = rec.records.iter().position(|r| r["seq"] == seq).unwrap();
    rec.records.insert(at + 1, copy);
    let m = replay(&rec, MoveOptions::default()).unwrap_err();
    assert_eq!((m.at, m.field.as_str()), (9000, "s2c"), "{m}");
    // Another client.
    let mut rec = base.clone();
    by_seq(&mut rec, seq)["client"] = 1.into();
    let m = replay(&rec, MoveOptions::default()).unwrap_err();
    assert_eq!((m.at, m.field.as_str()), (seq, "client"), "{m}");
}

#[test]
fn a_changed_request_parts_at_the_first_message_it_moves() {
    let base = fixture();
    // The walk to (105, 100) becomes (105, 101): y steps 0x648000 →
    // 0x64E000 (sub-tile 100) → 0x654000 (101), so the second 0x96 is the
    // first whose y differs.
    let mut rec = base.clone();
    let walk = rec
        .records
        .iter()
        .find(|r| r["type"] == "c2s" && r["bytes"].as_str().unwrap().starts_with("01"))
        .unwrap()["seq"]
        .as_u64()
        .unwrap();
    by_seq(&mut rec, walk)["bytes"] = "0169006500".into();
    let second = seqs_of_96(&base)[1];
    let m = replay(&rec, MoveOptions::default()).unwrap_err();
    assert_eq!((m.at, m.field.as_str()), (second, "WalkVerify.y"), "{m}");
    // A dropped tick: the mover is one step behind at the next window.
    let mut rec = base.clone();
    let t = base
        .records
        .iter()
        .filter(|r| r["type"] == "tick")
        .nth(3)
        .unwrap()["seq"]
        .as_u64()
        .unwrap();
    rec.records.retain(|r| r["seq"] != t);
    assert!(replay(&rec, MoveOptions::default()).is_err());
}

#[test]
fn a_second_reassign_is_compared_not_seeded() {
    let mut rec = fixture();
    let last = rec.records.len() - 1;
    let mut r = rec.records[last].clone();
    r["type"] = "s2c".into();
    r["seq"] = 9001.into();
    r["client"] = 0.into();
    r["size"] = 11.into();
    r["bytes"] = "1500010000006e006200 01".replace(' ', "").into();
    rec.records.insert(last + 1, r);
    let m = replay(&rec, MoveOptions::default()).unwrap_err();
    assert_eq!((m.at, m.field.as_str()), (9001, "s2c"), "{m}");
    // Its position (110, 98) is where the test mover stands: positions only passes.
    let opts = MoveOptions {
        messages: false,
        positions: true,
    };
    let s = replay(&rec, opts).unwrap_or_else(|m| panic!("{m}"));
    assert_eq!(s.positions, 43);
}

#[test]
fn not_wired_and_other_formats_are_refused() {
    let m = read_movement(&fixture()).unwrap();
    let e = replay_movement(&m, &mut NotWired, MoveOptions::default()).unwrap_err();
    assert!(
        e.mismatch().is_none() && e.to_string().contains(NOT_WIRED),
        "{e}"
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/units-anim.jsonl");
    let tick = RawRecording::load(&path, TICK_RAW).unwrap();
    assert!(read_movement(&tick).is_err());
    // A 0x96 with no game-entry 0x15 before it names no unit.
    let mut rec = fixture();
    rec.records.retain(|r| {
        !(r["type"] == "s2c" && r["bytes"].as_str().is_some_and(|b| b.starts_with("15")))
    });
    let m = read_movement(&rec).unwrap();
    let mut mover = TestMover::new();
    mover.seed(0, 0, 1, 100, 100).unwrap();
    let e = replay_movement(&m, &mut mover, MoveOptions::default()).unwrap_err();
    assert!(e.to_string().contains("no player"), "{e}");
}

/// The mover the file-driven test uses. Path wiring: return the d2-sim
/// mover (recorded game + `d2_sim::path::walk`) here.
fn d2rs_mover() -> Box<dyn Mover> {
    Box::new(NotWired)
}

/// Every `packets-raw-1` recording in `traces/raw/` (queue:
/// `cargo run -p conformance --bin recordings-needed`, entry
/// `movement-walk`). Fails with MOVER NOT WIRED until the path wiring
/// fills [`d2rs_mover`]; the read counts print first either way.
#[test]
#[ignore = "needs traces/raw/*-packets.jsonl (local) and the d2-sim mover"]
fn recorded_walks_replay_exactly() {
    let files = raw_files("-packets.jsonl");
    assert!(!files.is_empty(), "no traces/raw/*-packets.jsonl");
    let mut failures = Vec::new();
    for path in &files {
        let rec = RawRecording::load(path, PACKETS_RAW).unwrap_or_else(|e| panic!("{e}"));
        let m = read_movement(&rec).unwrap_or_else(|e| panic!("{e}"));
        println!("{}: read {:?}", path.display(), m.read);
        match replay_movement(&m, d2rs_mover().as_mut(), MoveOptions::default()) {
            Ok(s) => println!("  PASS {s:?}"),
            Err(e) => {
                println!("  FAIL {e}");
                failures.push(e.to_string());
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
