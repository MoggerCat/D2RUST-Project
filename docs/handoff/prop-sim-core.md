# Handoff: property tests of the d2-sim core — `claude/prop-sim-core`

> Not yet folded into `docs/HANDOFF.md` (§1, §3, §7, §8) and `docs/PLAN.md`; a docs session folds it, then this file stays as the detailed record.

Cloud implementation session, 2026-10-06. Task class: tests from specs plus
root-cause fixes, medium effort (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `4b5b0bf`. Repo only, synthetic data, no
game files (M09). Inputs read: `specs/sim/{tick,unit-order,units,stats,
stat-lists,rng}.md`, `specs/drlg/rooms.md` §8, `docs/`, `crates/`.

## 1. State

Proptest state-machine and invariant tests of the `d2-sim` core, each
against a reference model written from the spec rules (not from the code).
`proptest` is a `[dev-dependencies]` entry of `crates/d2-sim/Cargo.toml`
(workspace version; the crate had no dev-dependencies section before).
Case counts follow the repo convention: a default per property,
`PROPTEST_CASES` overrides it, no failure persistence (minimized failures
become `regress_*` tests).

| File | Property | Model (spec) |
|---|---|---|
| `crates/d2-sim/tests/prop_rng.rs` | `step_is_the_multiply_with_carry`, `hi_recovers_from_consecutive_lows`, `n_steps_jump_ahead`, `helpers_match_the_table`, `setters_and_copies`, `fixed_points` | `rng.md` §2 step and its bounds (hi' ≤ K, = K only from {2^32−1, 2^32−1}), hi recovery, the stepping identity Z' ≡ K·Z mod (K·2^32 − 1) with Z = lo·K + hi (n steps = multiply by K^n: an independent jump-ahead), §3 helpers and their step counts (n < 1 draws nothing) over random draw sequences, §4 setters, edge case 4 copies, the two fixed points |
| `crates/d2-sim/tests/prop_timer.rs` | `timer_queue_matches_the_model`, `spec_vectors_through_the_driver` | `tick.md` §5.2–§5.5 and `unit-order.md` §3.2, §8: random spawn / schedule (types 0–299, expire −1, relative, absolute any i32) / cancel / unit-event cancels / unit removal, at top level and *inside running events* (cancel self, remove own unit, schedule, spawn); every run checked in order (class, list, owner, expire, args), every bucket, every-tick list and unit timer list checked after each step; tiles refused with no change |
| `crates/d2-sim/tests/prop_lists.rs` | `unit_lists_match_the_model`, `guid_wrap_duplicate_is_rejected`, `regress_free_room_unlinks_its_units` | `unit-order.md` §1–§7: GUID counters and wrap, hash buckets (descending GUID) and type iteration, `SUNIT_Add` / allocation apart, removal, act room lists, room unit lists, room changes, update queues (flag bit 2, no room, queued once), clients, allied counts, pending flags; duplicate GUID / unknown ids / unit already in a room / act ≥ 5 rejected with no change |
| `crates/d2-sim/tests/prop_units.rs` | `anim_schedule_matches_the_closed_form`, `anim_schedule_near_the_i32_limit`, `lifecycle_and_modes` | `units.md` §4.2 against a closed form of its loop (J = ⌈(F − a0)/s⌉ iterations, byte i read in the first iteration with a_j ≥ 256·i, end at f + max(J, 1) + 1; main and the three variants, index −1, the 144 bound, out-of-record reads); §3.1 / §3.2 allocation and removal (class checks with no draw, `rng.md` §5.3 seed steps per kind, GUIDs, mode, flags, stat lists, the missile's every-tick event), §4.1 set mode, §4.4 / §4.5 player starts through the queue; rejected requests leave no trace |
| `crates/d2-sim/src/stats/prop_tests.rs` (module declared in `stats/mod.rs`) | `stat_lists_match_the_model`, `stat_lists_with_ops_and_callbacks`, `regress_parked_child_outlives_its_parent`, `helpers_take_any_input`, `muldiv_rules`, `by_time_between_lo_and_hi`, `regen::player_regeneration` | `stat-lists.md` §3 array invariants, §1 / §8 chain invariants, which lists live after free / alloc / death / expiry (§8.3, §8.8, §10.4), §11.1 mod arrays; totals: every extended list's full value of every stat without ops equals the `stats.md` §6.1 sum over its current chain (and `StatLists::eval` equals that sum), plus the amounts the spec sends with no counting child (edge case 3; §6.1 r1 for a parked list's own writes; §8.6 toggles of a list outside the active chain); `keepzero` entries persist (§6.3). The wide variant adds the op stats, the server callback and an act time. `stats.md` §5 MulDiv rules, §8 ByTime bounds, no panic on any input; `stat-lists.md` §10.1 player regeneration (reschedule, dead, life, stamina, mana) in closed form |

Gate (all on this branch, §4): fmt, workspace clippy, `cargo test -p d2-sim
-p conformance`, depcheck, `spec_index.py --check`, `methods.py check`,
`coverage.py --check` and `--selftest`. No coverage claims were added: the
properties overlap the existing unit-tier claims, and the lesson of
`coverage-claims.md` §1 asks for a claim only where the assertions check
the whole rule; a docs session may add them.

M08: each model was perturbed through the code under test and fails:
timer `expire <= frame` → `<`, cursor not advanced on cancel; anim `n == f`
test, the type-3 event byte; allocation item seed, the seed undo; stat
lists: the edge-case-6 fix reverted, detach's DYNAMIC skip, eval's DYNAMIC
skip, the mod exclusion list, add-full `keepzero`; regeneration's mode-6
stamina shift. Each mutant failed at least one property; all restored.

## 2. Bugs found and fixed (root cause, input, fix)

1. **`UnitLists::hash_bucket(ty, b)` with b ≥ 128 panicked** (index out
   of bounds; public reader). Input: `hash_bucket(Monster, 128)`. Fix
   (`units/lists.rs`): a bucket past 127 is empty.
2. **`UnitLists::free_room` left units linked to the freed room.** The
   DRLG path `ActRooms::remove_active_room` (`drlg/active.rs`, `TODO
   (rooms.md §8.2)`) frees rooms that still hold units. The arena reuses
   slots LIFO, so the next `create_room` returned the same `RoomId`; the
   stale units then read as members, and removing one rewrote the new
   room's lists. Input: room R with monsters a, b; `free_room(R)`; new
   room R' (= R) with player p; `remove_unit(b)` → R' list `[p, a]`,
   allied count 1 → 0. Fix (`units/lists.rs`): `free_room` unlinks the
   remaining units first (`room_remove`, list order: room list, update
   queue, allied count); the original's flag 0x800000 / path update of
   those units stays a `TODO(rooms.md §8.2)` for the unit specs. No
   conformance path calls `free_room`.
3. **`anim::schedule` overflow panic** (debug) on `c − 1` for
   `Form::StartFrame(i32::MIN)` and wrapped `Frames` / `Percent` starts.
   Fix (`units/anim.rs`): the start index uses 32-bit wrapping
   arithmetic, as the original's register does.
4. **`anim::schedule` endless loop**: a frame count near 2^31 − 1 makes
   `a += s` wrap before reaching F; the signed 1.14d loop never ends (as
   with a negative speed, `units.md` edge case 3). Input: `Main { bonus:
   0 }`, f 0, s 1000, F `i32::MAX` (killed after 60 s). Fix: new
   `AnimError::Endless { speed, frame_count }`, detected before the loop
   (i64 bound); no conformance input is near it (frames · 256 from
   AnimData).
5. **`lifecycle::allocate` left a trace when it failed**: with an unknown
   room it read act 0, stepped the game seed (1 or 2 steps), moved the
   GUID counter and then failed in `add_unit`; a duplicate GUID (a fixed
   GUID of a live monster, or a wrapped counter, `unit-order.md` edge
   case 2) did the same. Fix (`units/lifecycle.rs`): an unknown room is
   refused before any draw; a refused `SUNIT_Add` restores the game seed
   and the GUID counter. Signature unchanged.
6. **`StatLists::detach` panicked on a parked child of a freed list**
   (`stat-lists.md` edge case 6: free leaves parked children pointing at
   the freed parent). `detach` fixed the heads of the dead parent through
   `ext_mut` ("live stat list" panic), so such a child could never be
   detached, re-attached (attach detaches first) or freed; attach's
   ancestor walk also read a freed parent. Input: `regress_parked_child_
   outlives_its_parent`. Fix (`stats/lists.rs`): `try_ext_mut` / `try_l`;
   a freed parent has no heads to fix, only the child's own links are
   cleared (the rest of detach is unchanged: a parked list moves no
   values).
7. **`stats::fraction_changed(f, last)` overflow panic** (debug) for f
   near `i32::MIN`. Fix (`stats/mod.rs`): `wrapping_sub` /
   `wrapping_abs`, identical on every life fraction (0–128).

No public signature changed; `AnimError` gained a variant (no exhaustive
match on it in the workspace).

## 3. Spec observations (not fixed here; owners decide)

- **`tick.md` §5.5 consequence 1 and edge case 4** say an event (every-tick
  event) scheduled during the queue run never runs in the same tick. By
  §5.5 r3 (each list's cursor starts at the head when the run reaches that
  list) this holds only for lists already started: an every-tick event of
  a later class (e.g. a monster mode change scheduled from a missile hit)
  runs in the same tick. The code and the `sim/tick` replays follow r3;
  `prop_timer.rs` models r3. Owner: `tick.md` (wording).
- **`stat-lists.md` open question 5** ("Is an extended list ever given
  NEWLENGTH? None found statically"): §8.1 r4 sets NEWLENGTH on the
  unit's list R, which is extended, whenever a TEMPONLY list is attached.
  If R is an item's list worn by a unit, §10.4 expiry on the wearer meets
  R (expire 0 ≤ frame): endless in 1.14d, the deliberate d2rs panic of
  edge case 4. Reachable through the public API; the property does not run
  that expiry. Owner: `stat-lists.md`.
- **`stat-lists.md` §6.1 r1 with §8.1 / §8.2**: attaching to or detaching
  from a unit whose own list is parked moves no values (the propagation
  starts at a parked list). Spec behaviour, not a bug; the model avoids it
  by parking only by states ≠ 0 (callers park real states).
- **§8.4 / §8.6**: equip sets the list's unit even when attach refused it
  (no unit list, or a cycle), and a later dynamic toggle then propagates
  into the unit's list without a counted child; the same for a parked
  list. Spec text as written; the model counts it.

## 4. Gate

Run on this branch after the fixes (2026-10-06, all pass):

- `cargo fmt --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings` (after
  `sh tools/cloud-setup.sh`): clean.
- `cargo test -p d2-sim -p conformance`: 1,264 passed, 0 failed, 6 ignored
  (game-file tests). `tick_replay` and the trace tests are unchanged.
- `cargo run -p depcheck`: OK (8 crates, determinism lint clean).
- `python3 tools/spec_index.py --check`, `python3 tools/methods.py check`
  (21 methods): OK.
- `python3 tools/coverage.py --check`: 3,259 claims, 0 errors;
  `--selftest`: ok.

## 5. Left as is

- Handoff T1: a frame past 2^31 − 1 still panics in `bucket_slot`
  (documented, unreachable).
- `stat-lists.md` edge case 4: an expired extended list still panics.
- Stale handles: `StatLists` readers and writers panic on a freed
  `ListId` (a loud refusal of a use after free); `TimerQueue::cancel` of a
  freed `TimerId` whose slot was reused cancels the new timer (timer ids
  carry no generation). The properties never pass stale handles.
- §7 GI (`units.md` §5 r4 vs `TimerQueue::schedule` keeping the callback
  of an expire −1 event): untouched; `prop_timer.rs` passes no callbacks.
