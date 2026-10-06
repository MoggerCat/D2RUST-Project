# Handoff: property-test fixes — `claude/prop-fixes`

> Folded into `docs/HANDOFF.md` (§1–§5, §7, §8) and `docs/PLAN.md` as of the eighth fold (`claude/docs-fold-8`); this file stays as the detailed record.

Cloud implementation session, 2026-10-06. Task: `docs/HANDOFF.md` §2 step
7n (the fixes `prop-sim-core` §5 and `fuzz-server` §5 deferred). Task class:
implementation, medium effort (METHODS M14). Base: `01dff69`
(`claude/tender-meitner-mphas3`). Inputs read: `specs/sim/{stat-lists,
stats,tick,intents-events}.md`, `specs/drlg/rooms.md` §7–§8, `docs/`,
`crates/`. Repo only (no `re/`, `../refs/`, `game/`).

Each item: the test was written first and seen failing on the base code
(a worktree at `01dff69` with only the new test added), except item 4
(see there).

## 1. Results

| # | Item | Test (failing first) | Fix |
|---|---|---|---|
| 1 | `stat-lists.md` edge case 4: an expired extended list panicked in `expire_lists` | `stats::gap_tests::expired_extended_list_stops_the_walk` (did not compile on the base: `expire_lists` returned `()`, and the base panicked on the case); the property `stat_lists_match_the_model` now runs the case instead of skipping it (79 of 1,000 cases hit it) | `StatLists::expire_lists` returns `Result<(), StatListError>`; on the first expired extended list met it stops with `StatListError::EndlessExpiry(list)`. This is the state 1.14d spins in: the walk restarts at the head after each free, so once it meets the extended list it never reaches a list behind it and changes nothing more. Event 12 (`units::dispatch` `remove_state`) passes the error up as `UnitError::Stats`; the conformance replay reports it as a mismatch (`expiry`) since a recording that finished the walk contradicts the loop |
| 2 | Stale `ListId` (freed list, slot maybe reused) panicked in every `StatLists` reader / writer | `stats::gap_tests::stale_list_handles_act_as_null_lists` (panicked on the base, `lists.rs:228`) | A stale handle names no list. Every public `StatLists` method taking a `ListId` treats it as the original treats a null list pointer where the spec has a rule (`stats.md` §4.2 "a null list reads 0"; `stat-lists.md` §5.1 / §5.3 set and add: no-op; §8.4 "I's list missing → nothing"), and otherwise does nothing and answers 0 / none / empty. The lists are byte-for-byte unchanged (test compares the `Debug` dump) and no callback fires. `TODO(spec: stat-lists.md §1)` on `ListId` (Q1). Internal `l()` / `lm()` still panic: they only follow links of live lists |
| 3 | `TimerQueue::cancel` of a reused `TimerId` cancelled the new timer | `tick::gaps_tests::stale_timer_id_does_not_reach_the_reused_record` (failed on the base: the stale id equals the new one) | `TimerId { slot, generation }`. The slot is the original's record (free list LIFO, `tick.md` §5.2 r5 / §5.4 r4: the freed record is the next reused, unchanged); the generation is d2rs's own, bumped when a record is freed. A stale id is a free timer to `cancel` / `expire` / `flags` / `event` (§5.4 r1: cancelling a free timer does nothing). `TimerId::slot()` added; `cancelled_timer_goes_to_the_free_list` now asserts slot reuse with `t3.slot() == t1.slot()` (was `t3 == t1`, same meaning) |
| 4 | `ActRooms::remove_active_room` frees rooms holding units (PW1) | `drlg::tests::rooms::removal_of_a_room_with_units_unlinks_them` (passes on the base too: it pins the current behaviour at the DRLG seam; the panic PW1 found was already fixed in `UnitLists::free_room`, which unlinks units in list order, `unit-order.md` §5.3) | Not implemented further: `rooms.md` §8.2 says `0x0061A840` gives each remaining unit "flag 0x800000 and its path updated, non-client units flag-ex 0x20", but which unit fields "flag" / "flag-ex" are and what the path update does belong to the unit specs and are not stated. TODO comment in `drlg/active.rs` rewritten to say so (Q3) |
| 5 | `d2-server` `dispatch::in_range` subtracted `i32` positions (overflow panic, FS3) | `tests::gaps::in_range_takes_any_positions` and the property `in_range_is_the_exact_chebyshev_test` (both "attempt to subtract with overflow" on the base) | `abs_diff` per axis against 50: the spec's |dx| ≤ 50 and |dy| ≤ 50 exactly, for any positions; identical to the old result wherever the `i32` difference fits (the property checks it against 64-bit arithmetic around any position) |

T1 (`bucket_slot` past 2^31 − 1) left as is, as the task says.

Added by the coordinator mid-session (`ci-nightly-props.md`
§"anim_schedule"): `anim_schedule_matches_the_closed_form`
(`crates/d2-sim/tests/prop_units.rs`) failed with "Too many global
rejects" at `PROPTEST_CASES=20000`. The inputs now come from
`schedule_inputs()`, which doubles a positive step until the §4.2 loop is
under `MAX_LOOP` (200,000 iterations), so the `prop_assume!` (kept as a
guard) no longer rejects. What the property asserts is unchanged.
`PROPTEST_CASES=20000 cargo test -p d2-sim --test prop_units`: 3 passed.

## 2. Open questions

- **Q1** (`stat-lists.md` §1 / §8): the spec gives the null-list rule for
  readers and base writes only. The original never passes a freed list to
  attach, detach, free, the toggles or the field accessors (no caller
  found by the specs). d2rs makes them no-ops on a stale handle; should the
  spec state a rule (or an error) for them? Unverified behaviour choice,
  not a fidelity claim.
- **Q2** (`stat-lists.md` edge case 4, open question 5): the error is the
  d2rs answer to an endless loop; the spec should name it next to edge
  case 4 if the coordinator agrees (the original's state is the one the
  error leaves, see item 1).
- **Q3** (`rooms.md` §8.2, PW1): which unit fields `0x0061A840` sets
  (flag 0x800000: +0xC4? flag-ex 0x20: +0xC8?), what "its path updated"
  does, and which units count as "client units". A spec session (unit /
  path specs) settles it; then `UnitLists::free_room` (or a hook it calls)
  applies it. Reachability is still PW1 (`docs/HANDOFF.md` 7h).
- **Q4** (`intents-events.md` §2.4 r3): `in_range` is now the exact
  difference. The original subtracts in 32 bits; at |dx| ≥ 2^31 its
  wrapped result could differ. Unreachable (positions are server-staged,
  FS3), and the spec states the rule mathematically; noting it only. The
  other copies of the 50-subtile test in `d2-sim` (`skills/use_`,
  `world/waypoints.rs`) still subtract `i32` and were not in this task.

No local game-file checks needed: nothing here reads game data.

## 3. Gate

`sh tools/gate.sh`: **PASS**, 2026-10-06, on this branch (all 13 steps:
spec index, methods, coverage `--check` / `--selftest`, trace checkers,
hook selftest, fmt, depcheck, clippy `-D warnings`, nextest d2-sim +
conformance 1,880 passed / 107 skipped, rest 601 / 75, d2-client 362 / 16,
doc-tests). The first run failed only on fmt (fixed) and on `wayland-sys`
missing `wayland-client.pc` in this container (installed the CI's apt
packages: `pkg-config libasound2-dev libudev-dev libwayland-dev
libxkbcommon-dev`). Extra hunts: `stats::prop_tests` at
`PROPTEST_CASES=20000`, `prop_timer` at 3000, `prop_units` at 20000: ok.
