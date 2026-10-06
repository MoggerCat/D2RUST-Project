//! Placement replay (`conformance::placement`): player placements read
//! from a synthetic packets recording, synthetic monster / item spawns in
//! two rooms, a table model, every perturbation reported at the changed
//! spawn (METHODS M08), and the file-driven test over `traces/raw/`.
//!
//! `fixtures/placement-players.jsonl` is hand-built (passes
//! `check_packets.py`), shaped like `path-placement.md` R1–R3: game entry
//! (three 0x07, then 0x15 at (4863, 5653): room unnamed), a waypoint trip
//! (one 0x07 for level 3 tile (976, 992), 0x0D, next tick 0x15 at
//! (4893, 4993)), and one 0xAC (counted, not decoded).

use std::collections::BTreeMap;
use std::path::PathBuf;

use conformance::placement::{
    read_player_placements, replay_placement, NotWired, Placed, PlacementModel, PlacementStats,
    RoomKey, Spawn, SpawnKind, NOT_WIRED,
};
use conformance::raw::{raw_files, Mismatch, RawRecording, PACKETS_RAW};

fn fixture() -> RawRecording {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/placement-players.jsonl");
    RawRecording::load(&path, PACKETS_RAW).unwrap()
}

/// A model answering from a table.
#[derive(Default)]
struct Table(BTreeMap<(Option<RoomKey>, SpawnKind), Vec<Placed>>);

impl Table {
    /// The table that places exactly `spawns`.
    fn of(spawns: &[Spawn]) -> Self {
        let mut t = Self::default();
        for s in spawns {
            t.0.entry((s.room, s.kind)).or_default().push(Placed {
                guid: s.guid,
                class: s.class.unwrap_or(0),
                x: s.x,
                y: s.y,
            });
        }
        t
    }
}

impl PlacementModel for Table {
    fn placed(
        &mut self,
        room: Option<RoomKey>,
        kind: SpawnKind,
        _: u32,
    ) -> Result<Vec<Placed>, String> {
        Ok(self.0.get(&(room, kind)).cloned().unwrap_or_default())
    }
}

const ROOM_A: RoomKey = RoomKey {
    level: 2,
    tile_x: 1000,
    tile_y: 1200,
};
const ROOM_B: RoomKey = RoomKey {
    level: 2,
    tile_x: 1008,
    tile_y: 1200,
};

/// Monsters and an item drop in two rooms (our own numbers).
fn synthetic() -> Vec<Spawn> {
    let s = |at, room, kind, unit_type, guid, class, x, y| Spawn {
        at,
        frame: 10,
        room: Some(room),
        kind,
        unit_type,
        guid,
        class: Some(class),
        x,
        y,
    };
    vec![
        s(1, ROOM_A, SpawnKind::Monster, 1, 20, 19, 5003, 6004),
        s(2, ROOM_A, SpawnKind::Monster, 1, 21, 19, 5006, 6004),
        s(3, ROOM_B, SpawnKind::Monster, 1, 22, 31, 5045, 6010),
        s(4, ROOM_A, SpawnKind::Monster, 1, 23, 19, 5004, 6008),
        s(5, ROOM_B, SpawnKind::Item, 4, 24, 587, 5047, 6013),
    ]
}

fn mismatch(spawns: &[Spawn], model: &mut dyn PlacementModel) -> Mismatch {
    let e = replay_placement("synthetic", spawns, model).unwrap_err();
    e.mismatch()
        .cloned()
        .unwrap_or_else(|| panic!("not a mismatch: {e}"))
}

#[test]
fn reader_finds_the_player_placements() {
    let (spawns, stats) = read_player_placements(&fixture()).unwrap();
    assert_eq!(
        (stats.players, stats.with_room, stats.monster_assigns),
        (2, 1, 1)
    );
    assert_eq!(spawns.len(), 2);
    assert_eq!(
        (spawns[0].room, spawns[0].x, spawns[0].y),
        (None, 4863, 5653)
    );
    assert_eq!(
        (spawns[1].room, spawns[1].x, spawns[1].y),
        (
            Some(RoomKey {
                level: 3,
                tile_x: 976,
                tile_y: 992
            }),
            4893,
            4993
        )
    );
    assert!(spawns
        .iter()
        .all(|s| s.kind == SpawnKind::Player && s.guid == 1));
}

#[test]
fn player_fixture_replays_and_a_changed_byte_is_reported() {
    let base = fixture();
    let (spawns, _) = read_player_placements(&base).unwrap();
    let s = replay_placement(&base.name, &spawns, &mut Table::of(&spawns)).unwrap();
    assert_eq!(
        s,
        PlacementStats {
            groups: 2,
            spawns: 2
        }
    );
    for sp in &spawns {
        // y + 1 in the recorded 0x15 bytes (u16 at 8).
        let mut rec = base.clone();
        let r = rec.records.iter_mut().find(|r| r["seq"] == sp.at).unwrap();
        let mut b = r["bytes"].as_str().unwrap().to_owned();
        let y = u16::from_str_radix(&format!("{}{}", &b[18..20], &b[16..18]), 16).unwrap() + 1;
        b.replace_range(16..20, &format!("{:02x}{:02x}", y & 0xFF, y >> 8));
        r["bytes"] = b.into();
        let (changed, _) = read_player_placements(&rec).unwrap();
        let m = mismatch(&changed, &mut Table::of(&spawns));
        assert_eq!((m.at, m.field.as_str()), (sp.at, "y"), "{m}");
    }
}

#[test]
fn synthetic_rooms_replay_exactly() {
    let spawns = synthetic();
    let s = replay_placement("synthetic", &spawns, &mut Table::of(&spawns)).unwrap();
    // Groups: A monsters, B monsters, B items.
    assert_eq!(
        s,
        PlacementStats {
            groups: 3,
            spawns: 5
        }
    );
}

#[test]
fn every_changed_field_is_reported_at_its_spawn() {
    let truth = synthetic();
    for i in 0..truth.len() {
        for field in ["guid", "class", "x", "y"] {
            let mut rec = truth.clone();
            let s = &mut rec[i];
            match field {
                "guid" => s.guid += 100,
                "class" => s.class = s.class.map(|c| c + 1),
                "x" => s.x += 1,
                _ => s.y -= 1,
            }
            let m = mismatch(&rec, &mut Table::of(&truth));
            assert_eq!((m.at, m.field.as_str()), (truth[i].at, field), "{m}");
        }
    }
}

#[test]
fn missing_extra_and_reordered_spawns_are_reported() {
    let truth = synthetic();
    // d2rs places one fewer in room A: the last recorded one is missing.
    let mut model = Table::of(&truth);
    model
        .0
        .get_mut(&(Some(ROOM_A), SpawnKind::Monster))
        .unwrap()
        .pop();
    let m = mismatch(&truth, &mut model);
    assert_eq!((m.at, m.field.as_str()), (4, "missing"), "{m}");
    // One more in room B's items: reported at the group's last spawn.
    let mut model = Table::of(&truth);
    model
        .0
        .get_mut(&(Some(ROOM_B), SpawnKind::Item))
        .unwrap()
        .push(Placed {
            guid: 25,
            class: 587,
            x: 5048,
            y: 6013,
        });
    let m = mismatch(&truth, &mut model);
    assert_eq!((m.at, m.field.as_str()), (5, "extra"), "{m}");
    // Order inside a room counts.
    let mut model = Table::of(&truth);
    model
        .0
        .get_mut(&(Some(ROOM_A), SpawnKind::Monster))
        .unwrap()
        .swap(0, 1);
    let m = mismatch(&truth, &mut model);
    assert_eq!((m.at, m.field.as_str()), (1, "guid"), "{m}");
    // A spawn recorded in another room.
    let mut rec = truth.clone();
    rec[2].room = Some(ROOM_A);
    let m = mismatch(&rec, &mut Table::of(&truth));
    assert_eq!((m.at, m.field.as_str()), (3, "guid"), "{m}");
}

#[test]
fn not_wired_is_refused() {
    let e = replay_placement("synthetic", &synthetic(), &mut NotWired).unwrap_err();
    assert!(
        e.mismatch().is_none() && e.to_string().contains(NOT_WIRED),
        "{e}"
    );
}

/// The model the file-driven test uses. Placement wiring: return the
/// d2-sim placement of the recorded game (seed → DRLG → `d2_sim::path`
/// §10–§12, `monsters::population`) here.
fn d2rs_model() -> Box<dyn PlacementModel> {
    Box::new(NotWired)
}

/// Every `packets-raw-1` recording in `traces/raw/` (player placements;
/// queue: `cargo run -p conformance --bin recordings-needed`, entry
/// `placement-players`). Fails with PLACEMENT NOT WIRED until
/// [`d2rs_model`] is filled; the read placements print first.
#[test]
#[ignore = "needs traces/raw/*-packets.jsonl (local) and the d2-sim placement"]
fn recorded_player_placements_replay_exactly() {
    let files = raw_files("-packets.jsonl");
    assert!(!files.is_empty(), "no traces/raw/*-packets.jsonl");
    let mut failures = Vec::new();
    for path in &files {
        let rec = RawRecording::load(path, PACKETS_RAW).unwrap_or_else(|e| panic!("{e}"));
        let (spawns, stats) = read_player_placements(&rec).unwrap_or_else(|e| panic!("{e}"));
        println!("{}: {stats:?}", path.display());
        for s in &spawns {
            println!(
                "  seq {} frame {} {:?} {}:{} ({}, {}) room {:?}",
                s.at, s.frame, s.kind, s.unit_type, s.guid, s.x, s.y, s.room
            );
        }
        match replay_placement(&rec.name, &spawns, d2rs_model().as_mut()) {
            Ok(s) => println!("  PASS {s:?}"),
            Err(e) => {
                println!("  FAIL {e}");
                failures.push(e.to_string());
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
