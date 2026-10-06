# Handoff: mutation testing of the d2-sim core and d2-proto — `claude/mutants-core`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06, base `claude/tender-meitner-mphas3`
`4b5b0bf`. Repo only (M09): no game files, so the conformance replay is the
committed traces (`traces/sim/tick/*.json`, `traces/sim/rng/*.json`).
METHODS M08 (prove the check can fail), applied to the unit tests of
`d2-sim::{tick,units,stats,rng,game}` and `d2-proto`.

## Method

`cargo-mutants` 27.1.0, one run per module group, `--timeout 60`
(proto first ran with 120), `--cargo-test-arg=--tests` (no doctests).

| Run | Command (from the repo root) | Tests |
|---|---|---|
| sim core | `cargo mutants --file 'crates/d2-sim/src/tick/**' --file 'crates/d2-sim/src/units/**' --file 'crates/d2-sim/src/stats/**' --test-package d2-sim --cargo-test-arg=--tests -j 3 --timeout 60` | d2-sim |
| sim survivors + replay | the same with `--test-package d2-sim --test-package conformance` and `--re` = the survivor names (line numbers wildcarded) | d2-sim + conformance (`tick_replay`, `tick_traces`, `rng_traces`) |
| rng, game | `cargo mutants --file crates/d2-sim/src/rng.rs --file crates/d2-sim/src/game.rs --test-package d2-sim --test-package conformance -j 3 --timeout 120` | d2-sim + conformance |
| proto | `cargo mutants --file 'crates/d2-proto/src/**' --test-package d2-proto --cargo-test-arg=--tests -j 1 --timeout 60` | d2-proto |

"Before" ran on a clean worktree of the base commit; "after" on this
branch. `mutants.out/` stays out of git (written to a scratch directory).
A full sim-core run takes ~75 min with `-j 3` on the 4-core cloud box;
adding conformance to every mutant doubles it, so the replay ran on the
survivors only.

## Counts

| Scope | Mutants | Unviable | Caught + timeout before | Missed before | Missed after |
|---|---|---|---|---|---|
| `d2-sim` tick, units, stats | 1342 | 100 | 1063 + 30 | 149 | **47** |
| `d2-sim` rng.rs, game.rs | 48 | 7 | 41 + 0 | 0 | 0 |
| `d2-proto` (src/**) | 844 | 16 | 796 + 0 | 32 | **9** |

The conformance tick/RNG replay killed **0** of the 149 sim survivors (all
149 survive d2-sim + conformance on the base commit): the traces exercise
the tick order, timers and lists, which the unit tests already pin; the
survivors sit in stats, regeneration and the stat-list chain operations,
which the traces do not cover yet.

Every remaining survivor is equivalent or unobservable (below). No code
was found wrong against its spec; no fix was made.

## Tests added (all new files, `#[cfg(test)] mod mutant_tests;` lines)

| File | Kills |
|---|---|
| `crates/d2-sim/src/stats/mutant_tests.rs` (31 tests) | MulDiv branch boundaries and truncation (`stats.md` §5.2–5.3); by-time fold above 180 (§8); life fraction outside the ratio branch (§9.3); table size / ValShift (§1.1, §2.2); charstats columns; x87 rescale of 0 and its signs; minimum rule at v = MinAccr (§4.3); `unit_pm`, `owner_player`, `owner_item_base` guards (§6.2); set-full / set of 0 on absent (stat-lists §6.3, §5.1); damage-related propagation through a static list (§6.1); op-2 recompute block with strength 0 (§6.4); item events and skill-stat handler calls, max-life rescale conditions, stat 74 only for monster max life (§7.2); mod insert only for players (§5.1); TEMPONLY → NEWLENGTH (§8.1.4); `unit_detach`, `free_plain` (§8.2, §8.3); equip on an attached list (§8.4); make static/dynamic on an unattached list (§8.6); state list owner, list by flags (§9.3); expiry needs NEWLENGTH (§10.4); state flag bounds; `set_state_changed` (§9.2); `set_flags` |
| `crates/d2-sim/src/units/mutant_tests.rs` (10 tests) | variant start index c − 1 (`units.md` §4.2); player life bounds and healthpot, stamina mode 2, manapot release (`stat-lists.md` §10.1); monster life bounds, heal to max keeps regen, no death above 0, uninterruptable death sets disguise (§10.1, §9.2); neutral start pending test (`units.md` §4.6); `RoomEntry::is_active`, `unit_mut` |
| `crates/d2-proto/src/mutant_tests.rs` (5 tests) | every one of the 115 typed messages decodes/encodes exactly its layout's bits, and its decode errors (§2.4 rules 1, 10); size-rule field bytes (§5 grammar); `Fixed(0)` is not a size; chat negative size reaches the caller; S→C split at the 0x204 limit and at the buffer end (§3.3) |
| `crates/d2-proto/src/tsv/mutant_tests.rs` (8 tests) | perturbation tests of the strict TSV parser (M07/M08): numbers, addresses, names, repeated size options, one-value handler ranges, bit-field widths, one-bit overlaps, offsetless non-cstr fields |
| `crates/d2-proto/src/codegen/mutant_tests.rs` (1 test) | a `cstr`/tail field makes a fixed row untyped (generator rule, module doc) |

Coverage claims (`docs/COVERAGE.md`, only where the assertions check the
whole outcome): `stats.md` §2 r2, §5 r2, §5 r3, §8 r2, §8 r3, §9 r3;
`intents-events.md` §2.4 r10. Perturbation tests and the rest carry no
claim.

## Remaining survivors (category b)

d2-sim (47):

| Mutants | Why it cannot be killed |
|---|---|
| `TickHooks` defaults (4: `advance_environment`, `client_room_ready`, `room_inactivity`, `act_allows_room_removal`), `StatHost::act_time` (3), `UnitHooks` defaults (9: `anim_rate` ×2, `has_path`, `room_flag`, `player_request_check`, `player_movement_step`, `player_action_frame`, `player_item_row_flagged`, `monster_mode_function`) | placeholder seam defaults ("nothing happens") for systems other specs own; not spec behavior, every real provider overrides them |
| `stats::key` `\|`→`^`; `alloc_extended` `(flags & BASIC) \| EXTENDED` `\|`→`^` | equivalent: the operands share no bit |
| `by_time` `a > 180`→`>=` | equivalent: 360 − 180 = 180 |
| `life_fraction` `h < m`→`<=` | equivalent: h = m gives (h << 7) / m = 128 = the else value |
| `x87_rescale` sign tests `< 0`→`<= 0` (3) | equivalent: a zero operand returned 0 before |
| `Soft::round_shifted`: initial exponent estimate (3), the `m < 2^(p−1)` branch (3), the carry renormalisation (1) | equivalent: the loop corrects any estimate; after the estimate m > 2^(p−1) always, so the decrement branch is dead; m = 2^p · 2^e and 2^(p−1) · 2^(e+1) are the same value |
| `StatTable::from_fixed` deps terminator `< count`→`<=` | unobservable: the load fix-up (`fixups.md` §2) writes only valid stat ids, then 0xFFFF; the code stops at the first id ≥ n, the spec says "until 0xFFFF": the same on every fixed-up table |
| `add_full` `new > 0`→`>=` (PERMANENT) | unreachable: an A53 stat has op stats (A51), so it is recomputed, never add-full'd |
| `recompute` `os == NO_STAT \|\| invalid`→`&&` | unreachable on fixed-up data (op stat ≥ n other than 0xFFFF is the code's TODO, not described by `stat-lists.md` §6.4) |
| `attach` / `detach` PERMANENT recompute condition (3 + 3) | equivalent: an A53 key in a full array implies PERMANENT (set-full sets it on every store of an A53 stat) |
| `clamp_to_max` `c > m`→`>=` | equivalent: adding m − c = 0 is a no-op (§5.3) |
| `StatLists::lm` generation guard → true | stale-handle guard of the arena (API misuse), not spec behavior |
| `anim::schedule` `s < 0`→`<=`; `player_regen` `hp < 256`→`<=`; `monster_regen` `r < 0`→`<=` | equivalent: s = 0 / r = 0 returned before; hp = 256 sets 256 either way |
| `monster_regen` disguise else-branch (2) | unreachable: the toggle is always "on", so `disguise` is `Some(true)` |
| `first_from_bucket` `from == 0` guard → true | unreachable: `next_of_type` returns before calling it with `from > 0` for a type without hash list |

d2-proto (9):

| Mutants | Why |
|---|---|
| `codegen::rust_type` `Bits(n) if n <= 8` → false | no `uN` field with N ≤ 8 in the TSVs (only u15, u31); the Rust width is a generator choice, not spec behavior; `generated_file_is_current` proves the output unchanged |
| `SizeRule::eval` chat `b.len() < 3` → `==`/`<=`, chat26 `< 10` → `==`/`<=` (4) | equivalent: the following `cstrlen` from that offset is `None` for those lengths |
| `SizeRule::eval` `n < 0`→`<=` | equivalent: n = 0 matched the arm before |
| `split_server_buffer` `at < len`→`<=` | equivalent: at = len evaluates the empty rest as incomplete and stops |
| `tsv::field_type` guard `n != 8 && n != 16` → true / `\|\|` (2) | unreachable: `u8` / `u16` match their literal arms first and leading zeros are rejected |

## Gate (this branch)

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`
(after `sh tools/cloud-setup.sh`); `cargo test -p d2-sim -p d2-proto -p
conformance` (d2-sim 1273 passed, 5 ignored; d2-proto 25 + 8; conformance
7 + 2 + 2); `cargo run -p depcheck`; `python3 tools/spec_index.py
--check`; `python3 tools/methods.py check`; `python3 tools/coverage.py
--check` and `--selftest`: all pass.

## Notes for the coordinator

- No public signature changed; only `#[cfg(test)]` module lines were added
  to `d2-proto/src/{lib,tsv,codegen}.rs`, `d2-sim/src/{stats,units}/mod.rs`.
- Next M08 step for the core: traces of stat changes (`record_stats.py`,
  HANDOFF §5) would let the conformance replay reach the stat code, which
  today it does not touch.
