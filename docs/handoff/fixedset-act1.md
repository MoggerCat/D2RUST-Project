# Handoff: Act I on synthetic data — `claude/fixedset-act1`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-06, follow-up of
`docs/handoff/fixedset-game.md`. Medium effort. Wrapped up early on the
coordinator's budget call: the full `tools/gate.sh` was **not** run. What was
run: `cargo fmt --all`, `cargo clippy -p test-fixtures -p d2-server
--all-targets -D warnings` (clean), `cargo test -p test-fixtures` (all pass),
and `cargo test -p d2-server --test game_wired_host` (builds, 7 ignored).

## 1. Done

1. **Act I-shaped synthetic set** (`crates/test-fixtures/src/act1.rs`, new):
   `act1::act1()` is `synth::synthetic()` with `levels`, `lvlprest`, `lvltypes`,
   `lvlsub` and `lvlmaze` replaced and the DRLG files rewritten. Levels 0..39
   are all act 0. The placer's levels (1–7, 17, 26, 27, 39) have the spec's
   DrlgType, LevelType 2 for wild levels, and the sizes and offsets of the
   `outdoor.md` placement vector. The town is 56×40 (as §7.5 path starts need)
   with `Files` 0 and File1..4 set. lvlprest has Def = row; rows 2..163 are
   one-cell 8×8 presets (Files 4, File1..6 the same DS1). Rows 164 and 165 are
   the preset levels 26 and 27. There is one lvlsub row for each of types 0..3.
   Levels 8..38 outside the chain are DrlgType 0 placeholders.
2. **Shared session** (`crates/test-fixtures/src/host.rs`, new): moved out of
   `d2-server/tests/game_wired_host.rs`. Covers game creation, finding the town
   waypoint, allocating the player (with the `vitals.md` §1 creation-stat
   asserts), join, frames, send, `assert_stub`, `assert_clean`, walk legs, and
   `border_goals`. `Setup` chooses creation mode, seeds, class and known
   waypoints.
3. **E2E in CI** (`crates/test-fixtures/tests/act1_game.rs`, new):
   - `placement_matches_the_spec_vector`: `ActCreation::Full` on synthetic
     data reproduces the `outdoor.md` vector. That covers start seed, DRLG
     seed, allocation order 4,3,2,1,17,39,26,7,6,27,5, every rect, and the
     35-room town.
   - `every_chain_level_generates`: every placer level generates and streams
     with no error. Preset levels have only preset rooms; outdoor levels have
     outdoor rooms (Act I generator §7, §12).
   - `join_town_and_walk_into_blood_moor`: join in the town → 50 idle frames →
     0x03 legs through the host → the player stands in a Blood Moor room
     (Blood Moor: 76 rooms, 40 of them outdoor). No fault and no wiring error;
     units are inside their rooms; two runs give the same digest.
4. **Live refactor**: `d2-server/tests/game_wired_host.rs` now builds the game
   with `GameData::load` and drives it with `host::Session`. This adds a
   `test-fixtures` dev-dependency to `d2-server` (a dev-dependency cycle,
   which cargo allows). Steps 3–8 are unchanged, apart from the two fixes in
   §2. Sim behaviour is unchanged; no `d2-sim` file was edited.

## 2. Findings (escapes for HANDOFF §8 Lessons)

- **The live host test could never have passed.** It looked for the town
  waypoint in `room_units` before any room was streamed. Preset units reach a
  room list only at tile build (`preset.md` §9), and reach the map list only at
  the first activation (§7–§8). The harness now also searches the map lists and
  the picked DS1 file (`Ds1File::from_input`, read-only).
- **0x03 is no longer a stub.** `adapters::handlers::walk` owns it, so the live
  test's "asserted stub plus a direct `walk_message`" would have walked twice
  or failed. Legs now send 0x03 through the host only, and assert result 0 with
  no stub record.
- The 0x3C, 0x0D and 0x16 stub assertions in the live test are kept but are
  unverified. The skills and item handlers return `None` without their
  providers, so those steps should still reach the stub.

## 3. Limits / open

- The synthetic `itemstatcost` has no stat 67, so the run velocity sits at the
  `pathing.md` §8.1 floor (p = 25), about 11 frames per sub-tile.
  `host::LEG_FRAMES` was raised from 400 to 1000 to cover this.
- The idle joined client received 0 messages over the whole walk. No spec
  value is asserted; the count is in the digest.
- Cold Plains has no waypoint object (no SubWaypoint rows), so there is no
  0x49 step on synthetic data. Vis links stop at the chain: there are no caves,
  Tristram or towers.
- The town DS1 has no walls: the town can be walked out of from any side.
  Made-up content.

## 4. Next steps

1. Run `sh tools/gate.sh` (coverage `--check` and the d2-client steps were not
   run here) and fix anything it reports.
2. Local run queue: `cargo test -p d2-server --test game_wired_host --
   --ignored --nocapture` on `D2_GAME_DIR`. Expect it to reach Blood Moor now,
   and check the 0x3C, 0x0D and 0x16 stub assumptions.
3. Optional: a variant `itemstatcost` with stat 67 for realistic run speed;
   SubWaypoint rows plus a Cold Plains waypoint for a synthetic 0x49 step.

## 5. Files

| Path | What |
|---|---|
| `crates/test-fixtures/src/act1.rs` | Act I set, its files, unit tests |
| `crates/test-fixtures/src/host.rs` | shared wired-host session |
| `crates/test-fixtures/src/lib.rs` | module lines |
| `crates/test-fixtures/tests/act1_game.rs` | the e2e |
| `crates/d2-server/tests/game_wired_host.rs` | folded onto `GameData` + `Session` |
| `crates/d2-server/Cargo.toml` | dev-dependency `test-fixtures` |

## 6. Merge with main (2026-10-07)

`origin/main` merged in (a merge, no rebase). The one conflict,
`d2-server/tests/game_wired_host.rs`, kept this branch's side: the town
waypoint search lives in `test_fixtures::host` (room, map and DS1 lists);
main's post-stream `assert_transferred` check (`preset.md` §9) was ported
there. Main's outdoor neighbour walk now get-or-allocates 1..17, so
`placement_matches_the_spec_vector` asserts the 20-level `levels.md` vector
(…, 5, 8, 9, …, 16). `CARGO_INCREMENTAL=0 sh tools/gate.sh`: **PASS**, all
13 steps (the cloud container needed CI's apt libraries for `d2-client`).
