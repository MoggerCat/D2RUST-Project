# Handoff: mutation testing of the walk / run handlers and `d2_sim::wiring::path` — `claude/mutants-walk-handlers`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).


Cloud test session, 2026-10-06, medium effort (METHODS M08). Base:
`claude/tender-meitner-mphas3` at `01dff69`, merged again at the end
(see §5). Repo only, synthetic tables and DRLG, no game files (M09).
`cargo-mutants` 27.1.0. Specs: `sim/pathing.md` §1, §10;
`sim/path-placement.md` §2–§12; `missiles/missiles.md` §R2.3, §R4;
`sim/units.md` §3.2, §6.1; `drlg/levels.md` §2. No production code
changed. `mutants.out` is not committed.

## 1. Counts

### d2-server: C→S 0x01–0x04 (`adapters/handlers/walk.rs` + the `WorldHost::walk` impls)

| Run | Mutants | Caught | Missed | Unviable |
|---|---|---|---|---|
| `walk.rs`, before | 19 | 17 | **0** | 2 |
| `WorldHost::walk` impls (`world.rs`, `world/action.rs`, `world/wired.rs`), before | 5 | 3 | 2 | 0 |
| same, after (`-F 'WorldHost.*::walk'` re-run) | 5 | 5 | **0** | 0 |

The 2 survivors were `WiredWorld::walk → None / Some(Default)`: no test
walked on the wired single-player host. Killed by
`crates/d2-server/tests/mutants_walk.rs` (measured: the re-run lists both
as caught).

### d2-sim: `crates/d2-sim/src/wiring/path/**`

`cargo mutants -p d2-sim --file 'crates/d2-sim/src/wiring/path/**'
--timeout 120 -j 3 --test-tool nextest`: 328 mutants. **The run was
stopped at 209 / 328** (coordinator wrap-up; about 40 s per mutant on
this container). Before, on the 209 tested:

| File | Caught | Missed | Unviable |
|---|---|---|---|
| `mod.rs` | 5 | 0 | – |
| `place.rs` | 39 | 38 | – |
| `rooms.rs` | 13 | 0 | – |
| `units.rs` | 70 | 26 | – |
| `walk.rs` (first 18 only) | 13 | 5 | – |
| total | 108 | 69 | 32 |

**After: not re-measured.** The new tests (§2) target 58 of the 69
survivors; each assertion was checked against the spec by hand and
passes, but the kill is a claim until the re-run in §4 confirms it.

## 2. Tests added

- `crates/d2-server/tests/mutants_walk.rs` (new, 2 tests): the wired
  host `SimGame<ActionSim<_>, WiredWorld<_>>` with the path provider on:
  0x01 walks the player exactly as vector M1 translated (13 ticks of
  +0x6000, the 14th on the target centre, neutral), client 1 gets 0x0F
  in tick 1's update pass (bytes by `d2-proto`'s `PlayerMove`), nothing
  unhandled; M08: the same host without the provider leaves 0x01 to the
  stub.
- `crates/d2-sim/src/wiring/path/mutant_tests.rs` (new, `#[cfg(test)]
  mod mutant_tests;` in `wiring/path/mod.rs`, 12 tests). An integration
  file under `crates/d2-sim/tests/` was asked for, but the action
  fixture (`wiring::action::tests::Fx`) is `pub(crate)`; a unit module
  reuses it instead of copying ~400 lines of fixture.
  - `placing_a_player_searches_a_free_point_and_tells_its_client`:
    §7.2 ring order with a wall next to the target → (49, 12) in room B;
    0x07 (8, 0, level 2) to the player, nothing else; update queue;
    flags 2 0x10000 (alt 0); path flag 0x2 cleared; event 14 at f + 50
    with callback `0x00554570` (cancel by callback removes it).
  - `placing_a_monster_sends_nothing_and_sets_the_alt_flag`,
    `a_null_room_places_from_the_units_own_room`,
    `placing_a_unit_without_a_path_is_fatal` (§10 r1, r2, r5).
  - `the_collision_views_answer_the_section_4_queries`: `Rooms` and
    `Shared` both: sub-tile rects, cell value 0x9 / 0 / 0x27, point,
    size (point vs plus) and box queries with masks.
  - `floor_drop_starts_below_right_and_walks_back`: synthetic
    `ExpField` (every cell one step toward the centre): start (12, 13);
    a wall on the walk back → (11, 13); a PET bit on the start →
    (11, 13).
  - `game_entry_places_the_player_in_the_town_and_sends_0x07_then_0x15`
    (`path::place::game_entry` on three `Shared` handles; §11),
    `every_acts_start_level_is_its_town` (1, 40, 75, 103, 109).
  - `a_warp_tile_without_a_destination_does_nothing` (§12.2 r1),
    `the_warp_walk_out_requests_walk_mode_to_the_target` (§12.2 r5).
  - `a_missile_path_takes_its_set_up_and_a_still_missile_stays_put`
    (provider on: §R2.3 steps 15, 18 on the path record; velocity 0 →
    no step, frames count down; target unit with type and GUID; §5.3 r1
    restamp by size; removal clears).
  - `floor_items_stamp_by_size_and_removal_clears_each_kind` (item
    mode 3 stamps 0x200; removal clears types 0–3 only, `units.md`
    §3.2: the item's footprint stays).
- `crates/d2-sim/src/wiring/action/tests/mod.rs`: `TestPending` records
  `Pending::send` (`sent`), so placement bytes are observable (5 lines;
  no existing test reads it).

## 3. Survivors of the 209 tested, by outcome

Targeted (a), kill not yet re-measured: `place.rs` 35 (every `Rooms` /
`Shared` query, `has_path`, `unit_room`, `unit_size`,
`add_player_to_world`, `map_reveal`, `is_player`, `queue_update`,
`room_change_messages`, `send`, `schedule_event ==`, `request_walk`,
`act_start_level` ×3, `place_unit` ×2, `warp_player`, `floor_drop`,
`log`); `units.rs` 22 (`path_has`, `path_velocity` ×2,
`path_set_target_point`, `path_set_move_mask`,
`path_set_acceleration`, `footprint_of` monster / missile / item arms,
`path_shape`, `path_size` ×2, `path_set_target_unit`,
`path_set_foot_mask` ×2, `path_place` missile arms ×2, the
object / item / tile arm, the `mode != 3` guard ×3); `walk.rs` 1
(`PathCtx::room_change_messages`): 58 in all.

Equivalent / unobservable (b):

| Mutant | Why |
|---|---|
| `place.rs` `Rooms::has_path → true`, `Rooms::unit_size → 1 / −1` | `Rooms` serves only the §7–§9 searches, which never call the unit methods |
| `units.rs` `footprint_of` object arm deleted | every caller passes no object shape (objects.txt not in `ActionTables`, TODO in `path_shape`), so the arm always returns `None` |
| `units.rs` `footprint_of` tile arm deleted | tile size 0: the §5.1 size stamp / clear sets nothing for size 0 |
| `units.rs` `path_cached_word → None` | read only for a missile with velocity ≠ 0; with the provider on such a missile has no path points (`pathing.md` OQ3) and expires at step 2 before step 6 reads the word |
| `walk.rs` `PathCtx::step → None`, `unit_step → true / false` | the unit step is the missile step: with the provider on, OQ3 (no points); observable once missile paths are specified |

Not decided here (open): `units.rs` `path_add_footprint → ()` (its
callers: the static teleport branch, a TODO, and `PathWorld::
add_footprint`), `monster_velocity → Default` (needs a monster walk
through the wiring), `walk.rs` `walk_error → ()` (only logs a fatal
walk error; no fixture reaches one).

Spec note (c, no code change): `path-placement.md` §3 says size-based
shapes "0 or 1 → the cell" for tiles, §5.1 says the size stamp sets
nothing for size 0; the code follows §5.1 (tiles stamp nothing). A spec
session should make the two agree.

## 4. Next steps (exact)

1. Finish and re-measure the d2-sim run (about 3.5 h at `-j 3`):
   `cargo mutants -p d2-sim --file 'crates/d2-sim/src/wiring/path/**'
   --timeout 120 -j 3 --test-tool nextest` — confirm the 58 targeted
   kills and classify the 119 untested mutants (rest of `walk.rs`).
2. Kill or classify the three open survivors above.
3. Fold this note into `docs/HANDOFF.md` and `docs/PLAN.md`.

## 5. Merge and gate

Merged `origin/claude/tender-meitner-mphas3` (at the time of the merge)
before the gate, no conflicts. `sh tools/gate.sh all`: every step PASS
(clippy, d2-sim + conformance, rest, d2-client, doc-tests) except
coverage, which flagged a `Covers:` line using `§R2.3` (not an anchor);
that claim was removed (comment-only change) and `coverage.py --check`
(4,276 claims, 0 errors) / `--selftest`, `cargo fmt --check` and the new
tests were re-run and pass.

## 6. Second session (2026-10-07): merge, open survivors, partial re-run

Cloud test session, wrapped up early on a coordinator budget cut.

**Merge.** `origin/main` merged (a merge, no rebase). One conflict: the
action fixture's `TestPending` (`wiring/action/tests/mod.rs`); main
already records `Pending::send` as `sent` (plus the interact-range
log), so main's side was taken. Follow-ups to main in
`wiring/path/mutant_tests.rs`: `path::place::game_entry` takes the size
from the unit (argument dropped); main's `path-placement.md` §5.1 now
says pattern 0 stamps nothing, so
`floor_items_stamp_by_size_and_removal_clears_each_kind` asserts a
pattern-0 monster leaves its cell unchanged and checks removal on a
`SizeX` 2 monster (plus + NO_PATH marker). That test is stricter than
before, not weaker. The §3 spec note (c) above is settled by main.

**Re-run, partial.** The same `cargo mutants` command found **340**
mutants (main added code). Stopped at **193 / 340** (budget cut):
156 caught, 11 missed, 26 unviable, 0 timeout; the 193 cover `mod.rs`,
`place.rs`, `rooms.rs` and `units.rs`, not `walk.rs`. **All 58 kills
the first session targeted are confirmed** (none of them is in the
missed list). The run used the tree as of commit `48c3f1a`.

**Survivors of the 193 and outcome:**

| Mutant | Outcome |
|---|---|
| `place.rs` `Rooms::has_path → true`, `Rooms::unit_size → 1 / −1` | equivalent (§3 table (b)) |
| `units.rs` `footprint_of` object arm / tile arm deleted | equivalent (§3 table (b)) |
| `place.rs` `add_player_to_world` `|= → &=` | killed: game entry leaves path flag 0x2 set (§11), added to `game_entry_places_the_player_in_the_town_and_sends_0x07_then_0x15` |
| `place.rs` `Shared::queue_update → ()` | killed: `placing_a_unit_in_its_own_room_queues_it_for_update` (§10 r5) |
| `place.rs` `Shared::history_write → ()` | killed: `placing_a_player_writes_its_position_history` (§10 r6, r7; new in main) |
| `place.rs` `game_entry → true / false` (wiring) | killed: `the_wired_game_entry_reports_placed_or_a_missing_spawn_room` (§11; new in main) |
| `units.rs` `path_cached_word → None` | **reclassified** from equivalent: main specified missile paths, so a moving missile has points. Killed: `a_moving_missile_reads_the_collision_word_its_step_cached` (`missiles.md` §R4 step 6) |

**The three open survivors of §3 are killed:**
`a_walk_request_puts_the_walkers_footprint_back` (`path_add_footprint`,
`pathing.md` §3 r6, r9), `a_monsters_velocity_comes_from_its_monstats_row`
(`monster_velocity`, §8.1 r2),
`a_fatal_type_set_in_a_walk_request_is_reported` (`walk_error`, §2;
the test sets prev type 8 directly, no normal walk reaches it).

Every kill above was measured by applying the mutant by hand and
running `wiring::path::mutant_tests` (each fails its test); the
cargo-mutants run started before these tests existed, so it lists them
as missed. Counted: **10 survivors killed** (3 open + 7 from the
re-run), **5 equivalent left**.

**Left (exact):**
1. Finish the re-run on the remaining 147 mutants, mostly `walk.rs`:
   `CARGO_INCREMENTAL=0 cargo mutants -p d2-sim --file
   'crates/d2-sim/src/wiring/path/walk.rs' --timeout 120 -j 3
   --test-tool nextest` (about 1.5 h), then re-check the earlier
   `walk.rs` classifications (`PathCtx::step → None`, `unit_step →
   true / false`): main's missile paths may make them observable now,
   as with `path_cached_word`.
2. Re-run `place.rs` / `units.rs` with `-F` on the 10 mutants killed by
   hand to record the kill by the tool.
3. Fold this note into `docs/HANDOFF.md` and `docs/PLAN.md`.

**Merge follow-up and gate.** After the merge, `d2-server/tests/
mutants_walk.rs` no longer compiled (main grew `NpcRest`, `QuestRest`
and added `HirelingRest`); its no-op `Rest` was replaced by the shared
`mutants_handlers_fx::Rest`, assertions unchanged. Gate on this head:
`cargo fmt --check`, `cargo clippy -p d2-sim -p d2-server --all-targets
-- -D warnings`, `CARGO_INCREMENTAL=0 cargo test -p d2-sim -p d2-server`
(40 test binaries, all ok), `coverage.py --check` (8,194 claims, 0
errors), `spec_index.py --check`: all PASS.
