# Handoff: movement and placement replay harnesses, recordings list — `claude/conformance-path-render`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, medium (METHODS M14). Base
`claude/tender-meitner-mphas3` at `edd9925`. Repo only: no `game/`, no
`traces/raw/`. Read: `specs/sim/pathing.md`, `path-placement.md`,
`intents-events.md`, `server-messages.tsv`, `client-messages.tsv`,
`traces/FORMAT.md`, `tools/trace-recorder/README.md`,
`docs/handoff/conformance-harness.md`, `render-capture.md`. Owned files:
`crates/conformance/` and this note. `crates/d2-sim/src/path/` was not
touched or referenced (parallel `impl-path-core` / `impl-walk`
sessions): both harnesses reach d2-sim only through their own seams.

## 1. State

**Harnesses ready, proven on synthetic recordings, never run on a 1.14d
recording; no d2-sim provider behind either seam yet** (M02:
unverified). No `Covers:` claim added (a claim here is the `trace` tier;
only a passing recording earns it).

| Kind | Harness | Input | Seam (until wired) | CI fixture | Recording test (ignored) |
|---|---|---|---|---|---|
| walk / run | `conformance::movement::{read_movement, replay_movement}` | `traces/raw/*-packets.jsonl`, `packets-raw-1` | `movement::Mover` (`NotWired` → `MOVER NOT WIRED`) | `movement-walk.jsonl` + test mover: pass; each of 42 0x96 perturbed (message and position channels), missing / extra / wrong client, changed request target, dropped tick, second 0x15, no seed | `movement_replay::recorded_walks_replay_exactly` |
| placement | `conformance::placement::{read_player_placements, replay_placement}` | same files (players); `Spawn` records for monsters / items | `placement::PlacementModel` (`NotWired` → `PLACEMENT NOT WIRED`) | `placement-players.jsonl` (R1–R3 shaped) + synthetic monsters / item in two rooms: pass; every spawn × guid / class / x / y perturbed, missing, extra, reordered, wrong room | `placement_replay::recorded_player_placements_replay_exactly` |
| recordings list | `conformance::needs::{NEEDED, report}`; `cargo run -p conformance --bin recordings-needed` | `traces/raw/` (or `$D2_TRACES_RAW`) | — | `recordings_needed.rs`: every script / test named exists, every ignored `needs traces/raw/` test is listed | — |

Both new fixtures are hand-built (our own numbers) and pass
`py tools/trace-recorder/check_packets.py` (R1–R7, `OK`). The walk
fixture's first 14 0x96 follow `pathing.md` vector M1 (x per tick
0x648000 + k·0x6000, arrival 0x698000); the test mover (per-axis clamp
to 0x6000 / 0x9000 per tick) is a stand-in, not the spec rules: CI
proves the harness, not d2-sim. Both ignored tests read the recordings
and print what they found first, then fail with the NOT WIRED status
(never a pass) until their `d2rs_mover()` / `d2rs_model()` return the
d2-sim provider.

## 2. Formats used (none invented)

No recording format holds per-tick path state or placement calls
(`pathing.md` open question 1, `path-placement.md` open question 1).
Both readers read `packets-raw-1` as `record_packets.py` writes it and
`check_packets.py` reads it (key `type`, `seq`, `frame`, `client`,
`size`, `bytes` hex); message layouts come from `d2-proto`'s generated
tables (`server-messages.tsv`, `client-messages.tsv`). Mismatches number
records by `seq`, as `check_packets.py` and `conformance::packets` do.

**Movement** (`movement.rs`):

- `c2s` id 0x01–0x04 → `MoveRequest` → `Mover::request`; other C→S ids
  counted (`other_inputs`), not replayed.
- `tick` → `Mover::tick`.
- `s2c` ids 0x0D, 0x0F, 0x10, 0x15, 0x96 (`MOVEMENT_IDS`, `pathing.md`
  §10 rule 6), size checked against the fixed layout. The first 0x15
  of each (type, GUID) is its game-entry placement (§11): it seeds the
  mover (`Mover::seed`, cell centre) and is not compared here (the
  placement harness compares it); later 0x15 are compared.
- Windows as `conformance::packets`: the recorded movement messages
  between two inputs must equal the mover's queued ones (any other ids
  the mover queues are filtered out) — client, bytes, order. A differing
  message names its layout field (`WalkVerify.x`, `ReassignPlayer.y`;
  packed 0x96 fields included), else `bytes[k]`; an extra one is
  reported at the next input's `seq` (`u64::MAX` at the end).
- Per-tick positions: the x, y a window message carries (0x0F / 0x10 /
  0x15: type, GUID; 0x96: the client's seeded player) against
  `Mover::position` (16.16 precise, `>> 16`) at the window close, i.e.
  after the tick that sent it. 0x0D's x, y is a walk-out target
  (§12.2 rule 6): bytes only. `MoveOptions { messages, positions }`
  lets the wiring start with positions only (0x96 is sent by the
  status routine `0x00548760`, whose owner spec is open: `pathing.md`
  open question 6).
- In single player no 0x0F / 0x10 reaches the walking player's client
  (R4), so a real walk recording compares 0x96 and 0x15 only.

**Placement** (`placement.rs`):

- `Spawn { at, frame, room: Option<RoomKey>, kind, unit_type, guid,
  class: Option<u32>, x, y }`; `RoomKey` = the 0x07 MapReveal fields
  (level, tile x, tile y), the only room name the protocol has.
- Core: group by (room, kind) in order of first appearance; ask
  `PlacementModel::placed(room, kind, frame)`; per index compare guid,
  class (when recorded), x, y; `missing` at the recorded spawn, `extra`
  at the group's last recorded spawn.
- Reader: players only, from 0x15 (`path-placement.md` §10 rule 6, §11,
  R1–R3); room = the single 0x07 sent since the previous 0x15
  (warp / waypoint arrival, R3), unnamed when there are several (game
  entry sends one per nearby room, R2). 0xAC and 0x9C are counted, not
  decoded: no spec gives their position bytes (`server-messages.tsv`
  has no 0xAC layout; 0x9C's `data@8` is not broken down;
  `docs/handoff/s2c-builders.md`).

## 3. Seams for the wiring sessions

- `movement::Mover` — `seed(client, unit_type, guid, x, y)`,
  `request(client, &MoveRequest)` (as the 0x01–0x04 handlers pass it,
  `pathing.md` §1.1), `tick()`, `take_sent()` (S→C bytes, any ids),
  `position(unit_type, guid) -> Option<Precise>`. Fill
  `tests/movement_replay.rs::d2rs_mover()` with the recorded game +
  `d2_sim::path::walk` behind the server's handlers and tick.
- `placement::PlacementModel` — `placed(room, kind, frame) ->
  Vec<Placed { guid, class, x, y }>`, placement order. Fill
  `tests/placement_replay.rs::d2rs_model()` with the recorded game's
  DRLG (seeds) and `d2_sim::path` §10–§12 (players),
  `monsters::population` (monsters), §9 / `treasure.md` §7 (items).
- Per-tick precise state: `Precise` is already 16.16, so a reader for
  the recorder extension below adds a `MoveEvent` with precise values
  and compares them without a seam change.

## 4. What the local recorder must add (not available today)

1. **Per-tick path state** (`pathing.md` open question 1): hook
   `0x00650840` entry and exit; per call frame, unit type, GUID, precise
   x / y (16.16), point index, count, points, velocity, path flags.
   Record walking, running, clicks into walls in town and in a
   wilderness area. The record format is the spec session's to define.
2. **Placement calls** (`path-placement.md` open question 1):
   `0x0064DEA0` entry / exit (room, start, size, mask, result),
   `0x00554EA0` arguments and result, monster creation
   (`monsters/population.md`) with room and point, `0x00555DA0` (room,
   from, out) for drops; each with frame, unit type, GUID, class and the
   room's level and tile rect. Alternatively the 0xAC / 0x9C position
   bytes in `server-messages.tsv` would let the packets reader take
   monsters and items from existing recordings.

## 5. Local run queue (`docs/HANDOFF.md` §5)

`cargo run -p conformance --bin recordings-needed` prints all of it,
with MISSING / present / BLOCKED per entry (`D2_TRACES_RAW=<dir>` for
another folder).

1. Existing recordings (`20261006-015956-packets.jsonl`,
   `-022633-packets.jsonl`), no play:
   `cargo test -p conformance --test placement_replay recorded_player_placements_replay_exactly -- --ignored --nocapture`.
   Expect today: printed player placements — 015956: the frame-1 0x15,
   guid 1 at (4863, 5653) (R1); 022633: (4673, 4548), room unnamed (ten
   0x07 before it, R2), and the waypoint 0x15 at (4893, 4993) with room
   level 3 tile (976, 992) (R3) — then FAIL `PLACEMENT NOT WIRED`. Record the
   printed counts (players, with_room, monster_assigns, item_messages)
   as the baseline; different R1–R3 numbers are a reader finding.
2. Same files:
   `cargo test -p conformance --test movement_replay recorded_walks_replay_exactly -- --ignored --nocapture`.
   Expect: per file `read MoveReadStats { requests, ticks, seeds,
   sent, other_inputs }` with requests = the 0x01–0x04 counts of
   `pathing.md` R4 (121 + 47 + 215 + 2 = 385, all accepted) and R5
   (49 + 9 + 72 = 130), no 0x0F / 0x10 in `sent` (R4), then FAIL
   `MOVER NOT WIRED`. Different request counts are a reader finding
   (the reader counts every `c2s`, R4 / R5 count dispatched ones: equal
   when nothing was dropped).
3. New walk recording (entry `movement-walk`):
   `py tools/trace-recorder/record_packets.py --seconds 120`, then
   `py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl`
   (OK). Play: new single-player character, Rogue Encampment only:
   ~10 single clicks on open ground (walk, 0x01), the same with run on
   (0x03), one click on an NPC walking and one running (0x02, 0x04),
   hold the button 3 s once (R6), one click into a wall or tent; no
   waypoint, warp, skill, item or NPC dialog. Then run item 2. Expect
   until wired: read counts with `other_inputs` small, FAIL
   `MOVER NOT WIRED`; after wiring: PASS with messages = positions =
   the 0x96 count.
4. Blocked: items 1–2 of §4 (recorder extensions); entries
   `movement-path-state`, `placement-monsters-items`.

## 6. Code map

| File | What |
|---|---|
| `crates/conformance/src/movement.rs` | `MOVEMENT_IDS`, `MoveRequest`, `Precise`, `MoveEvent`, `MovementRecording`, `MoveReadStats`, `read_movement`, `Mover`, `NotWired` / `NOT_WIRED`, `MoveOptions`, `MoveStats`, `replay_movement`, `replay_recording`, `first_field_difference`, `packet_record` |
| `crates/conformance/src/placement.rs` | `RoomKey`, `SpawnKind`, `Spawn`, `Placed`, `PlacementModel`, `NotWired` / `NOT_WIRED`, `PlacementReadStats`, `read_player_placements`, `PlacementStats`, `replay_placement` |
| `crates/conformance/src/needs.rs`, `src/bin/recordings-needed.rs` | `Needed`, `NEEDED` (7 entries: units, stats, packets, movement-walk, movement-path-state, placement-players, placement-monsters-items), `report` |
| `crates/conformance/tests/{movement_replay,placement_replay,recordings_needed}.rs` | fixture tests, perturbations, ignored recording tests |
| `crates/conformance/tests/fixtures/{movement-walk,placement-players}.jsonl` | synthetic `packets-raw-1` recordings |
| `crates/conformance/Cargo.toml` | + `d2-proto` (no Bevy) |

## 7. Gate

`sh tools/gate.sh all`: see the commit message for the summary of this
branch's run.
