# Handoff: mutation testing of last night's code — `claude/mutants-night-code`

Cloud test session, 2026-10-07 (METHODS M08). Base: `claude/specs-staging`
at `913d3b0`. Repo only, synthetic data, no game files (M09). **Stopped
early on a budget cut from the coordinator**: only part of the first module
was run; see §4 for what is left.

## 1. How it was run

`cargo mutants -p d2-sim --file <module files> -j 3 --timeout 60
--cargo-test-arg=--lib` (cargo-mutants 27.1.0, installed this session).
Two lessons for the next run (HANDOFF §8 candidates):

- `-- --lib` limits only the test *run*; the build still compiles every
  integration-test target of `d2-sim` (~40). `--cargo-test-arg=--lib`
  limits both. With the integration targets a mutant took ~100 s.
- With `CARGO_INCREMENTAL=0` one `d2-sim` lib-test build is ~55 s; with
  incremental on it is ~14 s (≈ 20 s per mutant at `-j 3` on 4 cores,
  ≈ 1.4 GB of incremental data per worker). The mutation runs used
  `CARGO_INCREMENTAL=1 CARGO_PROFILE_DEV_DEBUG=false`; the gate used
  `CARGO_INCREMENTAL=0`. At that speed the four modules (≈ 2,490 mutants)
  need about 5 h.

Mutant counts per module (`--list`): `world::objects` 712,
`world::hirelings` 295, `player::pets` 101, `missiles::{bodies, bodies_ext,
bodies_ext2}` 249 + 639 + 493.

## 2. Counts

| Module | Mutants | Tested | Caught | Missed before | Unviable | Missed after |
|---|---|---|---|---|---|---|
| `world::objects` | 712 | 463 (stopped) | 404 caught or unviable in total, 24+ unviable | 35 | — | 15 expected (not re-run) |
| `world::hirelings` | 295 | 0 | — | — | — | not run |
| `player::pets` | 101 | 0 | — | — | — | not run |
| `missiles::bodies*` | 1,381 | 0 | — | — | — | not run |

"Missed after" for objects is by reading: 20 of the 35 survivors have a
test aimed at them (§3); the `--iterate` re-run that confirms the kills was
not done (§4). No timeouts.

## 3. Objects survivors (35)

**Killed by new tests (20)** — files: `world/objects/mutant_tests.rs`
(child of `objects`, `mod` line at the end of `objects.rs`) and
`world/objects/chests/tests/mutant_tests.rs` (child of `chests::tests`, one
`mod` line after `impl ChestWorld for Fake {}`, to reuse its harness). 9
tests, each with a `// Covers:` claim:

| Mutants | Test | Rule |
|---|---|---|
| `objects.rs:137` `mon_lvl1` → 0 / 1 / −1 | `urn_trap_threshold_reads_monlvl1_as_i16` | §5.2 |
| `objects.rs:417` `\|` → `^` in `set_flag` | `create_keeps_attackable_and_selectable_already_set` | §3 r3, r7 |
| `objects.rs:632` `n < 1` → `==` / `<=` | `shrine_without_parm0_draws_once_with_one_row_besides_row_0` | §5.1 r2 |
| `objects.rs:661`, `:662`, `:685` (`/` → `%` / `*`, `<` → `<=`) | `urn_and_chest_thresholds_match_the_rule_at_every_roll` (model of §5.2 over 3,000 seeds, both boundaries reached), `chest_lock_threshold_is_signed` | §5.2 |
| `objects.rs:1135` `subclass & 1` → `\|` | `update_sends_0x4d_only_for_subclass_bit_0` | §14 r1 |
| `chests.rs:223` `< 5` → `<=` | `chest_sparkle_q6_is_below_5_only` | §8.1 r3 |
| `chests.rs:237` `< 25` → `<=` | `chest_empty_quarter_is_below_25_only` | §8.1 r5 |
| `chests.rs:260, 261, 263, 280, 295` band bounds | `class_397_band_edges_belong_to_the_upper_band` | §8.1 r4 |

**Equivalent or unreachable (6)**

| Mutant | Reason |
|---|---|
| `objects.rs:573` delete arm 5 in `run_init` | arm 5 returns `Ok(())`, as the `_` arm (§5.4 "returns at once") |
| `objects.rs:691` `\|` → `^` in `init_chest` | `init_urn` just wrote `InteractType` 0–8; bit 0x80 is never set before |
| `objects.rs:712` `> 255` → `==` / `>=` | `TOWNS` holds at most 109 (0 for act ≥ 5): the fatal check of §5.5 never fires |
| `chests.rs:311` `n < 7` → `<=` | at n = 7 the loop `0..7 − n` runs 0 times either way |
| `chests.rs:336` `k < 4` → `<=` | at k = 4 the loop `0..4 − k` runs 0 times |

**Not decided by the spec (9)**: `chests.rs:76, 81, 96, 132, 139, 150`
(`ChestWorld::unit_type`, `item_quality`, `trap_monster_id`, `room_units`,
`within`, `in_room` default bodies, 9 mutants). They are the seam's
"narrowest reading" placeholders that the wired host (`wiring/action/
objects.rs`) still uses; the spec does not state them, the provider does.

No code disagreed with a spec; no production code changed (only the two
test-only `mod` lines and `mutants.out/` in `.gitignore`).

## 4. Left to do

1. Finish `world::objects` (249 mutants untested, from `chests.rs`
   `special_chest` onward: rest of chests, `misc.rs`, `shrines.rs`), then
   `--iterate` to confirm the 20 kills above.
2. `world::hirelings`, `player::pets`, `missiles::{bodies, bodies_ext,
   bodies_ext2}`: not run. Command (per module, detached with `setsid
   nohup`): `CARGO_INCREMENTAL=1 CARGO_PROFILE_DEV_DEBUG=false cargo mutants
   -p d2-sim --file '<glob>' -j 3 --timeout 60 --cargo-test-arg=--lib -o
   <dir>`; delete the `/tmp/cargo-mutants-*` copies and incremental data
   after.
3. Pets tests would need a `mod mutant_tests;` line in `pets_tests.rs` to
   reuse its fake `Fx`.

## 5. Gate

`CARGO_INCREMENTAL=0 cargo test -p d2-sim -p d2-server`: 3,572 passed,
0 failed, 118 ignored; `cargo clippy -p d2-sim -p d2-server --all-targets
-- -D warnings` clean; `cargo fmt --all --check` clean; `py
tools/coverage.py --check` 8,235 claims, 0 errors; `py tools/spec_index.py
--check` passes. `mutants.out` and the mutation copies were deleted.
