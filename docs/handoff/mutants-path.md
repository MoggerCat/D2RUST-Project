# Handoff: mutation testing of `d2_sim::path` (`claude/mutants-path`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).


Cloud test session, 2026-10-06, METHODS M08 (prove the check can fail).
Base: `claude/tender-meitner-mphas3` at `729c76e`. Repo only, no game
files (M09). Tool: `cargo-mutants` 27.1.0, `-p d2-sim --file
'crates/d2-sim/src/path/**' -j 3 --timeout 60 --cargo-arg=--lib`
(`CARGO_PROFILE_DEV_DEBUG=0`). **Stopped early on the coordinator's
budget call**: the run is incomplete (below) and no "after" run was made.

## 0. Status (2026-10-07, test-hardening session)

The merge `6b6aa1b` took the 50 tests of §1 out of the tree (they no
longer compiled against the reworked walk API). This session merged
`origin/main` (clean; at `674996d` the branch differed from main only
by this note, so no test of main was disabled, ignored or weakened by
this branch) and **restored all 50 tests** from `e7c10b5`, ported to the
current API:

- `path/mutant_tests.rs` (7): `TileRect` / `Point` for the removed
  `RoomRect` / `SubPoint`; the test rooms implement `PathMotion`
  (teleport now takes one context).
- `walk/mutant_tests.rs` (43): uses the walk tests' `fake` module
  directly (`tests/mod.rs`: `pub(super) mod fake`) instead of the
  `#[path]` re-include; one `Ctx` for world + units + game; `WalkPath`
  → `DynamicPath` (`set_path_type`, `put_target`, `final_target()`,
  `cur_point` / `point_count`); `request` / `interrupt_check` /
  `mode_check` take the context. `tests/fake.rs` gains two seams the
  old wrapper overrode: `FakeUnits::door` (door orientation) and
  `FakeUnits::no_town` (`monster_can_be_in_town` false).
- Same scenarios and expected values everywhere except the two lead
  tests (`refresh_point_lead_only_for_players_and_monsters`,
  `compute_target_lead`): pathing.md §3 "Target lead" now states the
  lead byte has no writer in 1.14d, so the lead is 0 and the code
  dropped the lead seam. Both now assert the spec's value (target =
  the unit's position); the old lead offsets (22, 21) have no spec
  basis any more.

Gate on this branch: `CARGO_INCREMENTAL=0 cargo test -p d2-sim -p
d2-server` all pass (d2-sim lib 3,195 pass; ignored ones are the
game-file tests, as on main), `cargo clippy -p d2-sim -p d2-server
--all-targets -D warnings`, `cargo fmt --check`, `coverage.py --check`
(0 errors), `spec_index.py --check`, `methods.py check`. No test
disabled. Next: §4 (measure kills; `step.rs` survivors). Sections 1–3
describe the tests as written at `e7c10b5`.

## 0.1 step.rs survivors (2026-10-07, branch `claude/mutants-path-step`)

Cloud session from `claude/mutants-path` @ `7d72615`; repo only (M09).
Tool: `cargo-mutants` 27.1.0, `-p d2-sim --file
'crates/d2-sim/src/path/walk/step.rs' -j 4 --timeout 60
--cargo-arg=--lib`, `CARGO_PROFILE_DEV_DEBUG=0`. Run with
`CARGO_INCREMENTAL=1` (one mutant build ~20 s instead of ~100 s; scratch
target dirs, deleted after). **Stopped on the coordinator's budget call
after step.rs**: the full `path/**` baseline (1,757 more mutants) was
started and cancelled at 10 tested, so the other files are not measured.

| step.rs (250 mutants) | Caught | Timeout | Missed | Unviable |
|---|---|---|---|---|
| before (tree at `7d72615`) | 169 | 4 | 74 | 3 |
| after (this branch, `--iterate` on the 78 not caught) | 234 | 2 | 11 | 3 |

New tests (15, `walk/mutant_tests.rs` §9 block, each with `// Covers:`):
movement guards (flag 0x20, count > 0, index < count), base ≤ 0,
acceleration (counter, + / clamp / max clears), no aim after the last
point, arrival refresh > 5 per axis and the target-unit tail (re-path
result decides), distance budget (types 8 / 11), cell-walk halving
boundary (exactly 0x10000), saved steps and flag 0x8 (blocked walk too:
`path-placement.md` §2.3 "the step crossed at least one cell"), forced
move walkers only, missile refusal 0x1 / 0x4 only, mask 0x3401 → 0x3C01,
set position recache only with flag 0x1, room-change flag / monster memo
/ merge order, run drain floor 1 and stamina exactly 0. One test seam
added: `tests/fake.rs` `clear_ai_room_memo` logs `memo <unit>`. No
production code changed: no survivor showed code contradicting a spec.

The 11 missed after are exactly the equivalent survivors below (the two
timeouts are the halving loop `> → <`, an endless loop: not missed).

Equivalent survivors (no test possible from the spec):
- `step` 124 `&` → `|` / `^` (×2): `room_change_messages` tests flag 0x2
  itself and returns (§9.8 "only when path flag 0x2 is set").
- `one_step` 236 `PATH_POINTS − 1` → `+` / `/` (×2): the index clamp acts
  only at index ≥ 78; rule 2 moves only with index < count ≤ 78.
- `cell_walk` 302 `guard += 1` → `*=`, 303 `>` → `==` / `>=` (×3): the
  1 << 20 loop guard has no spec value; a valid walk never reaches it.
- `room_change_messages` 460 `<` → `<=` (×2): with both indexes at their
  ends both elements are none and the `_ => break` arm ends the loop;
  468 `x < y` → `x <= y`: equal clients are taken by the earlier arm.
- `run_drain` 497 `d < 1` → `d <= 1`: d = 1 is set to 1.

Gate on this branch: `CARGO_INCREMENTAL=0 cargo test -p d2-sim -p
d2-server` all pass (d2-sim lib 3,211), `cargo clippy -p d2-sim -p
d2-server --all-targets -- -D warnings`, `cargo fmt --check`,
`coverage.py --check` (0 errors), `spec_index.py --check`, `methods.py
check`. No test disabled or weakened.

Left: §4 step 1 for every file but step.rs (full `path/**` run, ~5 h at
4 cores with incremental builds; the old m1 / m2 counts of §2 are the
only data), then tests for those survivors (§4 steps 2–3 beyond step.rs),
then §4 step 4.

## 1. State

New tests only, in two new files plus one `mod` line each:
`crates/d2-sim/src/path/mutant_tests.rs` (7 tests, path-placement §2–§7
and the TSV parser) and `crates/d2-sim/src/path/walk/mutant_tests.rs`
(43 tests, pathing §1–§9). The walk file includes `tests/fake.rs` a
second time (`#[path]`, `allow(dead_code, clippy::duplicate_mod)`) so
`tests/` stays untouched. No code was changed: no mutant showed code
contradicting a spec.

Checks run on this branch: `cargo fmt --check`, `cargo clippy -p d2-sim
--all-targets -D warnings`, `cargo test -p d2-sim` (1,476 pass, 7
ignored), `coverage.py --check` (0 errors), `spec_index.py --check`,
`methods.py check`. **Not run**: the full `sh tools/gate.sh` (client
build skipped for the budget); next session runs it.

## 2. Counts (before the new tests)

| Run | Files | Tested | Caught | Timeout | Missed | Unviable |
|---|---|---|---|---|---|---|
| m1 | all of `path/` in order, stopped at 1,389 / 1,828 (through `walk/request.rs`) | 1,389 | 1,061 | 13 | 288 | 27 |
| m2 | `walk/request, seams, step, tables, velocity`, `warp.rs`, stopped at 328 / ~566 (in `step.rs` `cell_walk`) | 328 | 193 | 2 | 122 | 10 |

Not reached by either run: the rest of `walk/step.rs` (after line 300),
`walk/tables.rs`, `walk/velocity.rs`, `warp.rs`.

**After**: not measured. Every new test was written against a named
mutant; each passes on the unmutated code. Next step 1 measures the
kills.

## 3. Survivors by outcome

(a) killed by a new test (spec decides; test names in brackets):
- collision `plus_value` OR [`plus_query_ors_equal_bits`]; record
  `set_path_type` 0x10000 / type-4 assert
  [`set_type_saves_velocity_and_allows_short_missile`]; missile teleport
  flag 0x8 [`missile_teleport_sets_only_the_moved_flag`]; `ExpField`
  10-byte header [`field_header_only_file_parses`]; §7.2 ring offsets,
  rows and the `rk + 1 < D` limit with k = 2
  [`ring_step_two_rows_and_offsets`, `ring_limit_with_step_two`]; TSV
  index order [`tables_rows_out_of_index_order_are_an_error`].
- walk `find.rs`: `set_type`, `reset_type`, `refresh_point`, compute
  steps 4–12 (lead, door shift, item slack, range 100, (0, 0) target,
  town access, target footprint, flag 0x1000, flag 0x10, room-exit flag,
  type 1 / 17), preparation probes and push, toward tail / reverse /
  t2 = 255, straight radius and n = 0, A* best node, open / closed
  updates, children, propagation, neighbour order, step compression, 78
  outputs, target probes.
- walk `geom.rs`: octant table, path distance, unit distance, ray test
  (cells tested and returned cell per block), direction vector in all
  quadrants, missile facing.
- walk `request.rs`: 0x02 / 0x04 mapping, queued-action clear, knockback
  ignore, `SeqInput`, rule 1 mode 17, rule 4 frame test, rule 5 roll
  boundary.

The A* and toward scenarios were found with scratch models written from
the spec text (`astar.py`, `toward.py`, `ray.py`; not committed); the
expected values are the models' outputs, and the Rust code agrees with
every one (an independent check of both).

(b) equivalent / unobservable (no test possible from the spec):
- `walk_box` `> → >=` (×2): the extra sub-box is empty and returns.
- `ReadOnly::grid_mut`: never called by the read-only walk.
- `to_fp16_center`, `ObjectShape::foot_mask`, `set_type` merge `| → ^`:
  the operands share no bits.
- `alloc_dynamic_path` `delete !` and deleted missile arm: flags and the
  missile mask are 0 at allocation.
- `FIELD_MAX_STEPS` `* → +` and `ExpField::byte` `|| → &&`: guards;
  unreachable with the measured field (§7.3 r2) and the 50-ring search.
- `PathTables::from_tsv` line number `+ → − / *`: error text only.
- `nearest_free_point` `d = |dx| + |dy| → *`: within one ring both are
  increasing in min(|dx|, |dy|), so the kept cell is the same.
- `PlaceHost::life_percent`, and 58 `WalkUnits` default-method mutants
  in `walk/seams.rs`: seam defaults with no spec value; providers
  override them.
- octant `< → <=`, `2·ay → 2 + ay`, `2·ax → 2 + ax`: the branch result is
  the same for every input (worked case by case).
- toward `steps += 1 → −=`: steps is only compared with 0.
- straight `n != 0 → ==`: toward again gives the same points.
- A* rule 1 `+ → −` (×2): the probe set is symmetric; closed-node `f`
  (`+ → − / *`): never read; guard `false`: open nodes have no children.

(c) code wrong vs spec: none found.

Spec reading to settle (spec session): §5.1 rule 1 "o = 7 + clamp(dy)"
names no range; the code and the tests clamp to [−2, 2] ("dy < −1 gives
o = 5").

## 4. Left / next steps

1. Measure the kills: `cargo mutants -p d2-sim --file
   'crates/d2-sim/src/path/**' -j 3 --timeout 60 --cargo-arg=--lib
   --iterate -o <dir>` reusing the m1 output (or a fresh full run,
   ~2.5 h at 4 cores); record before/after here. Never commit
   `mutants.out`.
2. Not yet handled from m2 (50 survivors outside `seams.rs` /
   `request.rs`): `step.rs` lines 89–300 — event-0 restart (89), room
   messages flag (122), movement guards and acceleration (144–173),
   arrival rules (196–221, §9.5), monster re-path flag and distance
   budget (235–266), reach index (283), cell-walk halving (300). All are
   spec-decided (§9.2–§9.6); tests go in `walk/mutant_tests.rs`.
3. Run the untested files: rest of `step.rs`, `walk/tables.rs`,
   `walk/velocity.rs`, `warp.rs`.
4. Full `sh tools/gate.sh`.
