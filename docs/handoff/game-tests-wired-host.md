# Handoff: game-file tests of the whole wired single-player host (branch `claude/game-tests-wired-host`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7, §8) and `docs/PLAN.md` as of the eighth fold (`claude/docs-fold-8`); this file stays as the detailed record. Step 4 asserts the stub for C→S 0x03 and is stale since `wire-path-server` (HANDOFF §2 step 7u(a)); fix it before the local run C59.

Cloud test session, repo only (no `game/`), medium effort, from
`claude/tender-meitner-mphas3` (merged up to `9fdb785`). Read: `specs/`,
`docs/`, `crates/`. The diff adds one test file and this note. It changes
no library code, spec, `HANDOFF.md` or `PLAN.md`.

## 1. State

- 7 new `#[ignore]` tests in `crates/d2-server/tests/game_wired_host.rs`,
  one per `charstats` class (0 Amazon … 6 Assassin), that read
  `D2_GAME_DIR`. They compile and pass clippy; **none has run** (no game
  files in the cloud, M02).
- **No `Covers:` claim** (`HANDOFF.md` §8 lesson): each test has an
  `// Intended claim (unconfirmed until the first local run): none`
  line. It is an integration run. The spec values it asserts are claimed by
  their own tests.
- Put in `d2-server/tests`, not `d2-client/tests`: the run needs no
  client code, and the client crate would build Bevy for it (CLAUDE.md
  disk rule). The host is the one the client's local link drives
  (`Host<SimGame<…>>`).

## 2. What each test does (`twice(class)` → `run(class)` ×2)

| Step | What | Asserted (source) |
|---|---|---|
| 1 | Live tables and MPQs loaded once (`bin::load` → `fixup::apply` → `LevelTables` → `WorldFiles::load`, as `game_world_data.rs`); act 0 created through `WorldTypes` on the recorded init seed 644409375; `WorldSim` with the population and init tables (the room pass runs; see finding F1), the path provider on (`enable_paths`), `ActionWorld { waypoints: WaypointData::new(levels, objects) }`; the town generated and the room holding its waypoint streamed | no wiring error after creation (`WorldSim::errors`) |
| 2 | Town waypoint = the first town preset unit of type 2 whose `objects` row has `OperateFn` 23 (`waypoints.md` Constants), allocated at room sub-tile origin + preset x, y (`population.md` §9); the player of the class beside it (offset ±5, ±5, the first inside the room: a fixture choice, see F4), `init_player_stats` act 0; the Cold Plains waypoint bit staged; client 0 joined through `Host` | stats 6, 7 = `(vit + hpadd) << 8`, 8, 9 = `int << 8` from the live row (`vitals.md` §1) |
| 3 | 500 host frames (one tick each) | frame + 500; `tick_faults`, `ActionWorld::faults`, `WorldSim::errors()` empty; the player still in the game; the client has a room (`rooms.md` §4.1); position unchanged |
| 4 | Walk to the town's Blood Moor exit. Goals: points 2 tiles inside the Blood Moor along the edge it shares with the Rogue Encampment, from the **generated** level rects, nearest first. Each leg (≤ 40 sub-tiles per axis): C→S 0x03 through the host, then the same request on `wiring::path::walk::walk_message` (the wiring computes the path), frames until the player leaves modes 2, 3, 6, 19. A goal with no progress for 3 legs gives way to the next; at most 200 legs | 0x03 reaches the **stub** (`unhandled` = (0, 0x03, 5), result 0: no server handler owns 0x01–0x04, see F2); the player stops within 400 frames; no fault per leg; the player ends in a Blood Moor room (the task's goal, not a spec value) |
| 5 | Kill: the first live monster within 50 sub-tiles outside the town. If there is one: 0x3C (its `StartSkill`, right hand) and 0x0D on it | both reach the **stub** (F3); without a monster the step is skipped and the run prints why |
| 6 | Pick-up: the first item unit within 50 sub-tiles. If there is one: 0x16 | **stub** (unify-items has not landed; the live host has no inventory parts); skipped without an item |
| 7 | Back to the town waypoint (legs toward the start point, done within the 0x49 range: 22 sub-tiles for the sorceress, else 10, `waypoints.md` §6.2 step 3); C→S 0x49 (waypoint GUID, level 3) | result 0; no fault; the player in a Cold Plains room (§7 rule 6); S→C 0x0D = `0D 00 <guid> 01 <x+3> <y+3> 00 00` with (x, y) the position after the frame (§7 rule 7, edge case 5), with a 0x07 before it in the frame (§8 rule 3) |
| 8 | Invariants and digest | per generated act 0 level: active rooms ≤ rooms; Blood Moor 81 and Cold Plains 98 rooms if generated (`outdoor.md` Test vectors, room allocations `0x0066B42E`); every unit with a path and a room stands inside its room's sub-tile rect. Digest = FNV-1a 64 over frame, game seed, (level id, rooms, active), every unit (type, class, GUID, mode, seed, position, room), player stats 0–15, `unhandled`, the client's whole transcript |

The run is done twice per test; the two digests must be equal (same seed,
same data). The test prints `class N: digest <hex>`; two local runs
(or two machines) compare those lines.

## 3. Findings and blind spots (to settle on the first run)

- **F1 Population "on" places presets only.** `WorldSim` runs the room
  pass, but room population (`population.md` §3) reads the coordinate
  lists and the populated level / room count through `WorldPending`,
  which no spec provides (`HANDOFF.md` §1 row 3l). So the Blood Moor
  likely has no monster within reach. Step 5 then prints `kill: no
  monster within range` and the kill and the drop are not exercised.
- **F2 No walk handler on the server.** `SimGame::handle` routes no
  0x01–0x04 (`sim.rs` `handle`: items, moves, world, skills, stub); the
  sim's `walk_message` exists (`wire-path-sim.md`). The test drives both
  (stub on the host, request on the sim) and says so. Wiring 0x01–0x04
  into `SimGame::handle` would let step 4 send one message per leg.
- **F3 No skill-use provider on a live host.** `ActionWorld<WiredSkills>`
  needs `X: SkillRest` (`Pending + UseRest + LearnRest`). Only the
  synthetic e2e fixture (`TestPending` + `Book`) implements it; no spec
  provides the per-unit skill list. So the live host uses `NoSkills`, and
  0x3C / 0x0D are stubs.
- **F4 Join placement.** No spec places a joining player (`levels.md`
  §10 spawn room is not wired at join, `p6-integrate.md`). The player
  stands at the town waypoint ± 5 sub-tiles. A start on a blocked cell
  shows as a walk that never moves (step 4 assertion with the
  position).
- **F5 Town waypoint from presets.** The object is allocated by the test
  from the town's preset list (`create_object` is a `WorldPending`
  default that creates nothing). If no town preset has `OperateFn` 23,
  the fixture panics with that message. Then check `act_presets(0)` for
  outdoor towns before the preset list.
- **F6 The walk's goal.** "Blood Moor exit" = the shared edge of the two
  generated level rects (expected west edge x = 960 per `outdoor.md`'s
  derived rects, not asserted). The path search may stall at the
  palisade: the next goal along the edge is tried after 3 legs without
  progress. A failure prints the position and level. It is a wiring or
  path finding (activation of the neighbour level's rooms, `rooms.md`
  §5 rule 3, or the greedy search limit, `pathing.md` §5.2), not a test
  value.
- **F7 0x0D position.** The expected x, y are read after the 0x49 frame's
  tick. §7 rule 7 sets mode 2 at the player's own position, so the tick
  should not move it. If the bytes differ by a step, the arrival walk
  moved the player.
- Fixture choices, each named in the file: game seed 1234, leg 40
  sub-tiles, 400 frames per leg, 200 legs, 3 stuck legs, the client
  resending a message its duplicate filter dropped (200 ms, §2.1 rule
  1) one frame later.

## 4. Local run queue (fold into `HANDOFF.md` §5 C)

With `D2_GAME_DIR=<install>`:

```
cargo test --release -p d2-server --test game_wired_host -- --ignored --nocapture --test-threads 1
```

Expected: **7 passed, 0 failed** (`wired_host_amazon`, `_sorceress`,
`_necromancer`, `_paladin`, `_barbarian`, `_druid`, `_assassin`). Look at
the printed lines per class: the Blood Moor arrival position and frame,
the kill / pick-up notes (F1), and `digest <hex>`. Run the command a
second time and compare the seven digest lines: they must be equal
(determinism on real data across processes; in-process equality is
asserted).

Record the results in `HANDOFF.md` §5 Done and §1. A failing assertion
names its step. Read it against F1–F7 before changing a test. No claim to
turn on: the run proves integration only.

## 5. Gate (this branch)

`sh tools/gate.sh all` after `sh tools/cloud-setup.sh`: see the commit
(result recorded below). The 7 new tests are listed as ignored.
