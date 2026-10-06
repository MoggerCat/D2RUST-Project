//! Adjacency arrays (`specs/drlg/rooms.md` §6) through
//! `conformance::rooms::replay_rooms`:
//!
//! * `fixtures/rooms-adjacency.json`, the rooms.md synthetic vector of
//!   `check_rooms.py --selftest` as a format-1 trace (`check_rooms.py`
//!   passes it: C1–C4, 0 violations), through [`NearOrder`], a model
//!   of §6.1–§6.3 over the trace's `setup.near` (the test's input, not
//!   recorded data); every swapped pair in a recorded array is reported
//!   at its snapshot and index (METHODS M08);
//! * the committed tick traces `traces/sim/tick/*.json`, through a model
//!   that tells no arrays: the room events and snapshots read, and the
//!   snapshots' rooms equal the replayed active set.
//!
//! The d2-sim model (`d2_sim::drlg::Drlg`) needs the rooms' rects, which
//! no recording has yet (`docs/handoff/conformance-harness.md`). No
//! `Covers:` claims: they count as verified against 1.14d.

use std::collections::BTreeMap;
use std::path::PathBuf;

use conformance::rooms::{replay_rooms, RoomModel, RoomStats};
use conformance::Trace;
use serde_json::Value;

fn fixture() -> Trace {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rooms-adjacency.json");
    let json: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    Trace::from_json(&json).unwrap()
}

/// §6.1 fill from a fixed near order, §6.2 refill of the new room and
/// its neighbours, §6.3 swap-with-last removal.
struct NearOrder {
    near: BTreeMap<String, Vec<String>>,
    arrays: BTreeMap<String, Vec<String>>,
}

impl NearOrder {
    fn new(trace: &Trace) -> Self {
        let near = trace.setup["near"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(r, n)| {
                let n = n
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap().to_owned());
                (r.clone(), n.collect())
            })
            .collect();
        Self {
            near,
            arrays: BTreeMap::new(),
        }
    }

    fn fill(&mut self, room: &str) {
        let arr = self.near[room]
            .iter()
            .filter(|n| self.arrays.contains_key(*n) || *n == room)
            .cloned()
            .collect();
        self.arrays.insert(room.to_owned(), arr);
    }
}

impl RoomModel for NearOrder {
    fn activate(&mut self, _act: u8, room: &str) {
        self.arrays.insert(room.to_owned(), Vec::new());
        self.fill(room);
        for n in self.arrays[room].clone() {
            if n != room {
                self.fill(&n);
            }
        }
    }

    fn deactivate(&mut self, _act: u8, room: &str) {
        let arr = self.arrays.remove(room).unwrap_or_default();
        for n in arr.iter().filter(|n| *n != room) {
            if let Some(a) = self.arrays.get_mut(n) {
                if let Some(p) = a.iter().position(|x| x == room) {
                    a.swap_remove(p);
                }
            }
        }
    }

    fn adjacency(&self, room: &str) -> Option<Vec<String>> {
        self.arrays.get(room).cloned()
    }
}

/// Tells no arrays.
struct Unknown;

impl RoomModel for Unknown {
    fn activate(&mut self, _act: u8, _room: &str) {}
    fn deactivate(&mut self, _act: u8, _room: &str) {}
    fn adjacency(&self, _room: &str) -> Option<Vec<String>> {
        None
    }
}

#[test]
fn synthetic_arrays_replay_exactly() {
    let trace = fixture();
    let stats = replay_rooms(&trace, &mut NearOrder::new(&trace)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stats,
        RoomStats {
            activations: 6,
            deactivations: 1,
            player_moves: 3,
            snapshots: 4,
            arrays: 12,
            arrays_unknown: 0,
        }
    );
}

#[test]
fn every_swapped_pair_is_reported_at_its_snapshot() {
    let base = fixture();
    let mut checked = 0;
    for (j, e) in base.expected.iter().enumerate() {
        let rooms = e["data"]["acts"][0].as_array().cloned().unwrap_or_default();
        for (r, room) in rooms.iter().enumerate() {
            let n = room["adj"].as_array().unwrap().len();
            for k in 0..n.saturating_sub(1) {
                let mut trace = base.clone();
                let adj = trace.expected[j]["data"]["acts"][0][r]["adj"]
                    .as_array_mut()
                    .unwrap();
                adj.swap(k, k + 1);
                let m = match replay_rooms(&trace, &mut NearOrder::new(&trace)) {
                    Err(e) => e.mismatch().cloned().unwrap(),
                    Ok(s) => panic!("swap {j}/{r}/{k} replayed: {s:?}"),
                };
                let seq = e["data"]["seq"].as_u64().unwrap();
                assert_eq!(
                    (m.at, m.field.clone()),
                    (seq, format!("acts[0][{r}].adj[{k}]")),
                    "{m}"
                );
                checked += 1;
            }
        }
    }
    // Snapshots 25, 50, 75: 4, 3 and 5 rooms.
    assert_eq!(checked, 4 * 3 + 3 * 2 + 5 * 4);
}

#[test]
fn a_room_the_snapshot_lacks_is_reported() {
    let mut trace = fixture();
    let j = trace.expected.len() - 1;
    trace.expected[j]["data"]["acts"][0]
        .as_array_mut()
        .unwrap()
        .pop();
    let m = replay_rooms(&trace, &mut Unknown).unwrap_err();
    let m = m.mismatch().unwrap();
    assert_eq!(m.field, "rooms", "{m}");
}

#[test]
fn committed_tick_traces_read_and_match_the_active_set() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../traces/sim/tick");
    let mut total = RoomStats::default();
    for id in ["sim-0006", "sim-0007", "sim-0008"] {
        let trace = Trace::load(&dir.join(format!("{id}.json"))).unwrap_or_else(|e| panic!("{e}"));
        let s = replay_rooms(&trace, &mut Unknown).unwrap_or_else(|e| panic!("{e}"));
        total.activations += s.activations;
        total.snapshots += s.snapshots;
        total.arrays_unknown += s.arrays_unknown;
    }
    // rooms.md Test vectors: 446 snapshots, 5,808 arrays (C1).
    assert_eq!((total.snapshots, total.arrays_unknown), (446, 5808));
    assert!(total.activations > 0);
}
