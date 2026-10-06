//! The recordings the file-driven (`#[ignore]`) harness tests need, with
//! the exact recorder invocation to queue (`docs/HANDOFF.md` §5) and
//! whether such a recording is in [`raw_dir`] now.
//! `cargo run -p conformance --bin recordings-needed` prints [`report`];
//! `tests/recordings_needed.rs` checks every script and test it names
//! exists.

use crate::raw::{raw_dir, raw_files};

/// One recording to queue.
#[derive(Clone, Copy, Debug)]
pub struct Needed {
    /// Short id (`movement-walk`).
    pub id: &'static str,
    /// Harness module (`conformance::movement`).
    pub harness: &'static str,
    /// File name suffix in `traces/raw/` and its header format.
    pub suffix: &'static str,
    pub format: &'static str,
    /// Recorder commands, run from the repo root on the local PC, in
    /// order. Empty when [`Needed::blocked`].
    pub record: &'static [&'static str],
    /// What to do in the game while it records.
    pub play: &'static str,
    /// The test file (`tests/<name>.rs`) and ignored test to run after.
    pub test_file: &'static str,
    pub test: &'static str,
    /// What a pass needs besides the recording (seam wiring, ...).
    pub needs: &'static str,
    /// Set when no recorder records it yet: what must be added.
    pub blocked: Option<&'static str>,
}

/// Every recording a harness waits for.
pub const NEEDED: &[Needed] = &[
    Needed {
        id: "units-anim",
        harness: "conformance::units",
        suffix: "-tick.jsonl",
        format: "tick-raw-1",
        record: &[
            "py tools/trace-recorder/record_tick.py --seconds 200",
            "py tools/trace-recorder/check_units.py traces/raw/<time>-tick.jsonl",
        ],
        play: "record_tick.py 0.2.0 (anim records); as queued in docs/handoff/conformance-harness.md §5 item 1",
        test_file: "units_replay",
        test: "recorded_mode_schedules_replay_exactly",
        needs: "nothing: d2_sim::units::anim is wired",
        blocked: None,
    },
    Needed {
        id: "stats-lists",
        harness: "conformance::stats",
        suffix: "-stats.jsonl",
        format: "stats-raw-1",
        record: &[
            "py tools/trace-recorder/record_stats.py --seconds 240",
            "py tools/trace-recorder/check_stats.py traces/raw/<time>-stats.jsonl",
        ],
        play: "as queued in docs/handoff/conformance-harness.md §5 item 2",
        test_file: "stats_replay",
        test: "recorded_stat_lists_replay_exactly",
        needs: "nothing: d2_sim::stats is wired",
        blocked: None,
    },
    Needed {
        id: "packets",
        harness: "conformance::packets",
        suffix: "-packets.jsonl",
        format: "packets-raw-1",
        record: &[
            "py tools/trace-recorder/record_packets.py --seconds 180",
            "py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl",
        ],
        play: "the two existing recordings (20261006-015956, -022633) suffice (conformance-harness.md §5 item 3)",
        test_file: "packets_replay",
        test: "recorded_messages_replay_exactly",
        needs: "session code that rebuilds the recorded game (expected to fail today)",
        blocked: None,
    },
    Needed {
        id: "movement-walk",
        harness: "conformance::movement",
        suffix: "-packets.jsonl",
        format: "packets-raw-1",
        record: &[
            "py tools/trace-recorder/record_packets.py --seconds 120",
            "py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl",
        ],
        play: "new single-player character, stay in the Rogue Encampment: ~10 single left-clicks on open ground \
               (walk, 0x01), the same with run on (0x03), one click on an NPC (0x02) and one with run on (0x04), \
               hold the button for 3 s once (repeats every 7 frames, pathing.md R6), one click into a wall or tent; \
               no waypoint, warp, skill, item or NPC dialog (inputs other than 0x01–0x04 are not replayed)",
        test_file: "movement_replay",
        test: "recorded_walks_replay_exactly",
        needs: "a d2-sim Mover (d2_sim::path::walk behind the seam); today the test reads the recording, prints its \
                counts and fails with MOVER NOT WIRED",
        blocked: None,
    },
    Needed {
        id: "movement-path-state",
        harness: "conformance::movement",
        suffix: "-path.jsonl",
        format: "(none yet)",
        record: &[],
        play: "as movement-walk, plus the same in the Blood Moor (monsters walking) and clicks behind walls (A*)",
        test_file: "movement_replay",
        test: "recorded_walks_replay_exactly",
        needs: "a reader for the new records (MoveEvent gains precise positions; Mover::position is already 16.16)",
        blocked: Some(
            "recorder extension (pathing.md open question 1): hook 0x00650840 entry and exit and log per call \
             frame, unit type, GUID, precise x/y (16.16), point index, count, points, velocity, path flags; \
             its format must be defined by that spec session (no format exists)",
        ),
    },
    Needed {
        id: "placement-players",
        harness: "conformance::placement",
        suffix: "-packets.jsonl",
        format: "packets-raw-1",
        record: &[
            "py tools/trace-recorder/record_packets.py --seconds 180",
            "py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl",
        ],
        play: "the existing recordings hold R1–R3 (game entry, one waypoint trip); a new one adds: enter the game, \
               take the waypoint to Cold Plains and back, walk into the Blood Moor / Den of Evil entrance (warp, §12.2)",
        test_file: "placement_replay",
        test: "recorded_player_placements_replay_exactly",
        needs: "a d2-sim PlacementModel for the recorded game (seed → DRLG → §10–§12); today: PLACEMENT NOT WIRED",
        blocked: None,
    },
    Needed {
        id: "placement-monsters-items",
        harness: "conformance::placement",
        suffix: "-place.jsonl",
        format: "(none yet)",
        record: &[],
        play: "enter the Blood Moor and Cold Plains (room population), kill monsters (item drops), open chests",
        test_file: "placement_replay",
        test: "recorded_player_placements_replay_exactly",
        needs: "a reader producing placement::Spawn for monsters and items",
        blocked: Some(
            "either the 0xAC and 0x9C position bytes in sim/server-messages.tsv (docs/handoff/s2c-builders.md), \
             or a recorder extension (path-placement.md open question 1): 0x0064DEA0 entry/exit (room, start, \
             size, mask, result point), 0x00554EA0 arguments and result, the monster creation of \
             monsters/population.md with its room and point, 0x00555DA0 (room, from, out) for drops; \
             each with frame, unit type, GUID, class and the room's level and tile rect",
        ),
    },
];

/// The list with what is present now, one block per entry.
pub fn report() -> String {
    let dir = raw_dir();
    let mut out = format!(
        "Recordings for the conformance harnesses (raw dir: {})\n",
        dir.display()
    );
    for n in NEEDED {
        let present = if n.blocked.is_some() {
            0
        } else {
            raw_files(n.suffix).len()
        };
        let state = match (n.blocked, present) {
            (Some(_), _) => "BLOCKED".to_owned(),
            (None, 0) => "MISSING".to_owned(),
            (None, k) => format!("present ({k} file(s) *{})", n.suffix),
        };
        out += &format!("\n[{}] {} — {}\n", n.id, n.harness, state);
        out += &format!("  input: traces/raw/*{} ({})\n", n.suffix, n.format);
        if let Some(b) = n.blocked {
            out += &format!("  blocked on: {b}\n");
        }
        for c in n.record {
            out += &format!("  record: {c}\n");
        }
        out += &format!("  play: {}\n", n.play);
        out += &format!(
            "  then: cargo test -p conformance --test {} {} -- --ignored --nocapture\n",
            n.test_file, n.test
        );
        out += &format!("  pass also needs: {}\n", n.needs);
    }
    out
}
