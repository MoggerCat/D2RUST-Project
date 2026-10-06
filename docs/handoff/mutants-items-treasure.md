# Handoff: mutation testing of `d2_sim::{items, treasure}` — `claude/mutants-items-treasure`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06, base `claude/tender-meitner-mphas3` at
`4b5b0bf`. Repo only (M09). METHODS M08 applied to the item and treasure
code: every mutant `cargo mutants` (v27.1.0) makes in
`crates/d2-sim/src/items/**` and `crates/d2-sim/src/treasure/**` either
fails a test or is listed below with the reason it cannot.

## Counts

`cargo mutants -p d2-sim --file 'crates/d2-sim/src/items/**' --file
'crates/d2-sim/src/treasure/**' --timeout 120 -j 3` (1,819 mutants), then
`--iterate --timeout 60` on the new tests, then a final `-F` run on the
survivors.

| | caught | missed | unviable | timeout | total |
|---|---|---|---|---|---|
| before | 1,512 | 222 | 27 | 58 | 1,819 |
| after | 1,686 | 48 | 27 | 58 | 1,819 |

"Before" counts one mutant (`walk.rs:430 k -= 1 → /=`) that the first run
did not reach before its 2-hour limit; it ran in the `--iterate` run and
timed out (an endless walk), so it is in the timeout column of both rows. A
timeout is a detected mutant: the tests stop finishing. Per file, missed
before → after: items `affixes` 15 → 2, `create` 38 → 15, `mod` 2 → 1,
`props` 39 → 3, `quality` 53 → 4, `tables` 10 → 6; treasure `drop`
8 → 0, `quality` 5 → 0, `runtime` 21 → 3, `softfloat` 24 → 11, `walk`
7 → 3.

## Tests added (62, unit tier)

- `crates/d2-sim/src/items/tests/mutants.rs` (`mod mutants;` in
  `items/tests/mod.rs`): 43 tests (rare kind-step parity against a
  step-by-step model of `affixes.md` §7; crafted minimums; staffmods at
  every threshold against a spec model over all 100 first-pct values;
  socket caps and roll boundaries; forced and ear flag copies; property
  functions 5, 6, 7, 11, 13, 14, 19, the generic layers, poison count on
  set, base reset weapon-only, set partial states, runeword match and
  server rows, set bonuses of a full set, craft lists; ratio row class and
  uber; quality roll `L`; the rare override; superior fits as a full
  matrix; unique candidates, preference, marking at 4096/4097 and quest
  items; set candidates and the weight walk; table projection helpers).
- `crates/d2-sim/src/treasure/mutant_tests.rs` (`mod mutant_tests;` in
  `treasure/mod.rs`): 19 tests (soft-float signed zeros compared bit for
  bit, rounding carry, normal-range ends, truncation of huge values;
  NoDrop q = 0; automatic TC level window; `atol` values and the `cr`,
  `cs`, `cg` keys; item list order; the `type2` test; `get` stepping;
  area-level columns; chest tier boundaries; drop quality row class /
  uber / ultracode and the 128 threshold; walk TC index, `max` 1, drop
  flag thresholds).

Coverage claims are on rules whose outcome the assertions check (M14,
`docs/handoff/coverage-claims.md` §1); helper checks without a spec rule
(projection, item list order, soft-float bits) carry no claim. All new
claims are unit tier. `coverage.py --check`: 3,315 claims, 0 errors.

## Surviving mutants (48)

Equivalent: the mutated code computes the same result on every input
the code can receive.

| Mutant | Why equivalent |
|---|---|
| `items/affixes.rs:48:14` `<` → `<=` (alvl) | at i = 99 − h both branches give 99 − 2h |
| `items/affixes.rs:151:30` `>` → `>=` (preferred) | preferred 0: `−1 == i` is never true |
| `items/create.rs:82:16`, `items/create.rs:156:14`, `items/mod.rs:337:22` `<` → `<=` | the value 1 is replaced by 1 |
| `items/create.rs:96:38`, `:130:39` `map_or(-1)` → `1` | 96: the record exists (checked above); 130: a missing record is fatal on every path, type 1 is neither gold nor quiver |
| `items/create.rs:353:46` `count > 0` → `>=` | `class_skills` returns no first skill for count 0 |
| `items/create.rs:395:15` `tr < 1` → `<=` | tr 1 is set to 1 |
| `items/create.rs:487:20` `cur < 1` → `<=`, `==` (§7.3 set/unique) | cur 1: cap 1 vs n 1 give 1; cur < 1 with `==`: n = cur gives max(n, 1) = 1 |
| `items/create.rs:551:80`, `:552:33`, `:556:31`, `:556:46`, `:581:48`, `:588:48` `\|` → `^` | the two operands have disjoint bits |
| `items/props.rs:115:27` `max < min` → `<=` | equal returns earlier |
| `items/props.rs:422:25` `c < 0` → `<=` | c = 0 is handled by the branch before |
| `items/props.rs:446:33` `len > 152` → `>=` | at len 152 stat 152 is invalid, so the add writes nothing either way |
| `items/quality.rs:197:31`, `:210:31` file index `-1` → `1` | the routine sets it on success; on failure the downgrade's clear sets −1 |
| `items/quality.rs:311:41` `map_or(-1)` → `1` | type 1 is none of the tested types |
| `items/quality.rs:442:48` `&&` → `\|\|` (accept test) | a set bit fails the mark instead; idx > 4096 reads as set; result and file index are the same |
| `items/tables.rs:708:25`, `:708:27` (`n > 0` guard) | the only caller filters count 0 |
| `treasure/runtime.rs:432:33`, `:436:64`, `:439:64` `\|` → `^` | disjoint flag bits |
| `treasure/softfloat.rs:58:37`, `:58:54` `\|` → `^` | disjoint bit fields |
| `treasure/softfloat.rs:69:24` `\|\|` → `&&` (zero product) | the general path gives the same signed zero through `round` |
| `treasure/softfloat.rs:93:46` `%` → `+`, `/` (division sticky bit) | exact quotients of 53-bit significands have ≤ 53 bits, so a sticky bit on an exact quotient never reaches the rounding |
| `treasure/softfloat.rs:124:23` `\|` → `^`, `:124:55` mask `- 1` → `+ 1`, `/ 1` (addition sticky) | the shifted operand is < 2^53 while the rounding half is ≥ 2^62, so bit 0 never decides a tie |
| `treasure/softfloat.rs:145:23` `e > 10` → `>=` | e = 10 is ≥ 2^62, rejected by the i32 conversion anyway |
| `treasure/softfloat.rs:148:20` `<<` → `>>` | e ≥ 0 means ≥ 2^52; either shift is outside i32 |
| `treasure/softfloat.rs:167:25` (sign of `round`'s zero) | only `from_i64(0)` reaches it, with a positive sign |
| `treasure/walk.rs:148:20` `k >= 2` guard → true | k is a count of living party members including `O`, ≥ 1; for k = 1 both give 1 |
| `treasure/walk.rs:197:18` `<` → `<=` (binary search) | starts strictly increase (prob ≥ 1, §1.5 r1, §1.3), so the search lands on the same entry |
| `treasure/walk.rs:461:23` `\|` → `^` | `d` holds only 0x04 / 0x10; the hellbovine bit is 0x01 |

Unobservable without game files or a caller that does not exist yet:

| Mutant | Why |
|---|---|
| `items/create.rs:595:5` `personalize` → `Ok(())` | `generation.md` §3 r9: no path through `create_item` sets flag 0x1000000 before step 9 (also left unclaimed by `gaps-items-stats`) |
| `items/tables.rs:595:5` `typed` → `Ok(vec![])`, `:601:9` `from_fixed` → default, `:610:36` `-` → `+`, `/` | `ItemTables::from_fixed` runs only on a fixed-up 1.14d table set (ignored game-file tests); a synthetic `FixedSet` needs every runtime map |

## Fixes

None. No surviving mutant showed the code disagreeing with its spec.
Two readings stay as the code had them (no test asserts them):
`properties.md` §5 r6 for a cap < 1 (the TODO OQ-P2 in
`items/props.rs`: the text can be read as "return 0" or as "set stat 194
:= 0"); `treasure/walk.rs` maps a TC index ≥ the count to `NoTc`, which
`walk_tc_index_at_count` tests via §2's "none".

## Gate (this branch)

`cargo fmt --check`; `sh tools/cloud-setup.sh` then `cargo clippy
--workspace --all-targets -- -D warnings`; `cargo test -p d2-sim` (1,294
passed, 5 ignored; base 1,232); `cargo run -p depcheck`; `python3
tools/spec_index.py --check`; `python3 tools/methods.py check`; `python3
tools/coverage.py --check` and `--selftest`: all pass. No `mutants.out`
or other generated file is committed.
