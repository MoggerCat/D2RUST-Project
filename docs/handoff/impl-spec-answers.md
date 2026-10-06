# Handoff: spec answers implemented — `claude/impl-spec-answers`

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `b435f5a`, which already carries the two spec branches
`claude/spec-answers-tick-messages` (merge `c22a266`) and
`claude/spec-path-inv-answers` (merge `9944d0c`). Each changed rule was
read as the diff against the merge's first parent. Repo only (M09):
nothing here is checked against 1.14d; every item stays **unverified**.

## 1. Answers implemented

| Spec rule (answer) | Code | Test (`// Covers:`) |
|---|---|---|
| `tick.md` §1 r2 exact arithmetic; edge case 8 (`last == 0` is the first-use test) | `d2-server/src/host.rs` `TickDriver::last` is now `u32` (0 = first use), was `Option<u32>` | `tests/host.rs` `driver_first_use_and_mask`, `driver_zero_clock_reinitialises` (§edge-cases r8) |
| `tick.md` edge case 7 (31-bit clock wrap stalls the games) | unchanged arithmetic (signed wrapping difference) | `driver_clock_wrap_stalls` (both new vectors) |
| `tick.md` §4 r5 off-tick population `0x0052D0F0` | `d2-sim/src/tick/mod.rs`: new `pub fn populate_room` (room body without act flag test or clear); `room_pass` uses the same `room_body` | `tick/tests.rs` `off_tick_population_runs_now_and_step3_revisits` |
| `tick.md` §5.2 r2 (expire −1 loses the callback; `units.md` §5 r4) | `tick/timer.rs` `schedule`: `expire == -1` → `schedule_every_tick(…, None, …)` | `tick/tests.rs` `every_tick_schedule_drops_the_callback` |
| `tick.md` §5.6 freeze gate (due timer freed, not rescheduled; every-tick kept) | already so (`units/dispatch.rs` skips the call, the runner frees) | `units/gap_tests.rs` `freeze_gate_frees_due_timers_and_keeps_every_tick` |
| `inventory.md` §6.3 / WN1 (unit flag 0x10 = not yet announced; part 1 unit-add, part 2 ground update) | `items/moves/deferred.rs`: new `item_unit_update` (0x10 → part 1; else bit 0 → part 2) and `announce_item` (0x9C action 2 for mode 3 + 0x1000, else action 0); `uflag::NO_GROUND_MSG` renamed `NOT_ANNOUNCED`, `uflag::CHANGED` added | `items/moves/tests/deferred.rs` `item_unit_update_announces_once_then_updates` (§6.3 text, r1, r2); wiring helper `assert_ground_update` (see §2) |
| `pathing.md` PQ1 (stat 67 = unit total, base 100; V1–V4) | no code change (the formula already read the total); seam doc names `0x00625480` | `path/walk/tests/mod.rs` `v_velocities` now asserts V3 = 2565 and V4 = 384 literally |
| `pathing.md` §9.8 (removal = `0x00571600` S→C 0x0A, add = `0x00571F90`; previous room via `0x005545C0`) | `path/walk/seams.rs` doc addresses swapped to the spec's; new `PathWorld::room_in_unit_act` (default true), used by `room_change_messages`; wiring provider in `wiring/path/walk.rs` (room exists and its act = unit's act) | `room_change_without_previous_room_only_adds` |
| `inventory.md` §1.3 / GX1 (record 29 = 255 × 255 from `.txt` −1; 16–31 are not copies) | comment only in the ignored game test | `tests/game_inventory_path.rs` (expectation unchanged) |
| `population.md` §1 r2 (callers of `0x0052D0F0`) | TODO in `monsters/population/mod.rs` replaced by a pointer to `tick::populate_room` | — |

## 2. Test expectations changed (only where the spec now says so)

1. `d2-server` `driver_first_use_and_mask`: the wrap half moved to the
   new `driver_clock_wrap_stalls` (spec vectors); the `Option` checks
   became `u32` (§1 r2: no separate first-use flag).
2. `d2-sim` `wiring/inventory/tests/ground.rs` `assert_ground_update`
   (used by the refused-pickup and drop tests): it pinned WN1's open
   reading ("flag 0x10 set → nothing; clear it by hand → action 2"). Now
   it asserts §6.3 as answered: with 0x10 set, `ground_update` (part 2)
   is still `None` but `item_unit_update` gives part 1's 0x9C action 2;
   after the clean-up's clear of 0x10 and 0x1 nothing is sent until bit
   0 is set, then part 2's action 2.
3. `v_velocities`: the V3 / V4 asserts use the spec's new literal values
   (2565, 384) instead of the expressions; same numbers as before.

No other test changed; nothing was skipped, ignored or weakened.

## 3. Not done / open

- The per-unit update `0x0053A500` and the room clean-up `0x00553220`
  are still not wired (IS2, IS3; `TickHooks::unit_update` keeps its
  default), so the server's `update_pass` sends no ground-item messages
  yet; `item_unit_update` is the library piece for that wiring. The
  comments in `d2-server` `items/moves.rs` and its test no longer cite
  WV1 as the reason. The clean-up's clear of flags 2 bits 0x10000 /
  0x800 is still not written (`wiring/path/walk.rs` TODO updated).
- No portal / A2Q6 arrival code exists, so nothing calls
  `tick::populate_room` yet (`population.md` §1 r2 callers).
- §5.2 r4 (state-54 check) is still the caller's (`needs_uninterruptable_check`).

## 4. Gate

`sh tools/gate.sh` (after `tools/cloud-setup.sh`): **FAIL, only on tests
that already fail on the base `b435f5a`**. This diff touches no file they
read. Everything else passes: spec_index, methods, coverage
check/selftest, trace checkers, hook selftest, fmt, depcheck, clippy
workspace, rest tests (879), doc-tests. d2-sim: 990 of 992 run pass.
nextest stopped at the first failures, so the rest of d2-sim did not
run in the gate; the targeted run of every module this diff touches
passed (298 tests).

- d2-sim `monsters::ai::tests::implemented_matches_catalogue` and
  `rules::d2moo_only_act1_ais_are_stubs`: `ai-functions.tsv` now marks
  more thinks `spec'd-here` (merged spec branch) than `IMPLEMENTED` has.
- d2-client `bridge::local_tests::{protocol_version_check,
  unknown_and_unowned_ids, spec_table_records_the_server_message_as_unowned}`,
  `bridge::tests::bridge_modules_except_mirror_have_no_bevy_type`: the
  client message table (merged `spec-client-model`) now names handlers
  for ids 0–8, 10–16, … 172 that the bridge does not have (`NoHandler`).

Both belong to the sessions that implement those specs.
