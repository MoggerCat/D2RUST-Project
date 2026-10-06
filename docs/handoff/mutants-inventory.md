# Handoff: mutation testing of `items::inventory` and `items::moves` — `claude/mutants-inventory`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06 (METHODS M08: prove the checks can fail),
medium effort. Branch `claude/mutants-inventory` from
`claude/tender-meitner-mphas3` at `edd9925`. Repo only (M09): every
count below holds on this branch, with `cargo-mutants` 27.1.0.

## 1. Run

```
cargo mutants -p d2-sim --file 'crates/d2-sim/src/items/inventory/**' \
  --file 'crates/d2-sim/src/items/moves/**' -j 3 --timeout 120 \
  --build-timeout 600 -o <scratch> -- --lib -- items::
```

then `--iterate` on the same output directory after the tests were added.
The test filter `items::` keeps each mutant's run short; a mutant killed
only by a test outside `items::` counts as missed here, so the counts
are conservative. `mutants.out` is not committed.

| | Mutants | Caught | Missed | Unviable |
|---|---|---|---|---|
| Before | 1304 | 1145 | 135 | 24 |
| After | 1304 | 1216 | 64 | 24 |

Missed by area: `items/inventory/` 39 → 13 (of 557), `items/moves/`
96 → 51 (of 747; 33 of the 51 are `MovePending` default bodies).

## 2. Tests added (new files only)

- `crates/d2-sim/src/items/inventory/mutant_tests.rs` (12 tests), declared
  as a child of `inventory/tests.rs` (`#[path = "mutant_tests.rs"] mod
  mutant_tests;`) so it reuses that file's fake world and tables.
- `crates/d2-sim/src/items/moves/mutant_tests.rs` (22 tests), declared in
  `moves/tests/mod.rs` (`#[path = "../mutant_tests.rs"]`) for the shared
  `Fake`.

No test logic, fixture or non-test code outside these two files changed
(two `mod` lines only). Every assertion is a spec outcome
(`specs/items/inventory.md`, plus `items/generation.md` §1.3 for the
type test) except `grid_cell_out_of_bounds_is_none`, which checks the
documented accessor contract of `Grid::cell` (no spec rule; it keeps a
mutant that would index outside the cells from surviving).

**No `Covers:` claims.** Each test checks one clause of a unit that other
tests already claim; per `coverage-claims.md` §1 a partial check gets no
claim. Each test names its unit in a `// Rule (one clause; no claim):`
comment instead. Coverage numbers are unchanged.

Clauses the new tests pin (the mutant each kills in brackets):

| Spec | Clause | Mutant |
|---|---|---|
| §1.4 r1 | unlink takes the item off its grid's list; an item linked without a grid unlinks (no node-grid underflow); `contains` follows link / unlink | `retain` `!=`→`==`, `node_grid > 0`→`>=`, `contains`→true/false |
| §2.3 / §1.2 | the search runs on grid page + 2 (cube page 3) | `PAGE + pg` → `*`, `-` |
| §3 r5, r7 | a beltable item that is not 1 × 1 gets no slot and is not placed | `is_1x1`→true, `\|\|`→`&&` (×2) |
| §4.1 | 11 / 12 allowed when *either* bodyloc is a hand | `\|\|`→`&&` |
| §4.2 r3, r4 | requirement bounds inclusive (dex = reqdex passes; equipping: stat − own contribution = req passes) | `+`→`*`, `<`→`<=` |
| §4.3 r4 | L = 5 and L = 11 see their other hand (result 4) | `other_hand` arms deleted |
| §4.6 r3 | skip = 1 bypasses the requirements | `!skip` → `skip` |
| §4.7 r1 | itemtypes `body` 0 → no location | `\|\|`→`&&` |
| §5.1 | cursor check needs the player's cursor; belt check passes the player's own mode-2 item; ground-or-owned sends mode 4 to the owned check | guards → true, `>`→`>=` |
| generation §1.3 | `type2` counts only when > 0 | `>` → `==`, `<`, `>=`; `&&`→`\|\|` |
| §6.1 r1 | marking a flag already set keeps it | `\|`→`^` (`mark`, `add_cmd`) |
| §6.1 r3 | an item's own update list is walked only with its +0xC8 bit 0 | `& 1`→`\| 1` |
| §11 | category: monster 0x1A1 dual-wields, other monsters do not | `==`→`!=` |
| §7.3 r2 | missing player / existing non-player: page ≤ 4 goes to step 4 | `\|\|`→`&&`, `> 4`→`>= 4` |
| §7.6 | 0x1B at location 5; X loses unit flag 0x2 only | `!=`→`==`, `clear_uflags` `&`→`^`, `!` deleted |
| §7.8 | 0x1D location 8 needs `0x00567840`; N's item flag 0x1 set on a set flag | `!` deleted, `add_iflags` `\|`→`^` |
| §7.12 | q_s + q_d = m merges; equal stat 72 not re-sent; "both books" | `>`→`>=`, `<`→`<=`, `&&`→`\|\|` |
| §7.14 | `0x0055E9B0` needs mode 4 | `\|\|`→`&&` |
| §7.19 r2 | target missing → 3; target not identified → 0; equipped target accepted; filler must be a filler and identified | six mutants |
| §7.20 | a ground scroll is taken; a non-book is refused | `!=`→`==`, `\|\|`→`&&` |
| §7.22 | amount = gold and amount = limit pass | `>`→`>=` (×2) |
| §7.23 r3 | act 5 (0x230): axe one-handed only | `&&`→`\|\|` |
| §8.1 r1, §8.2 | busy without a cursor item → nothing | `\|\|`→`&&` (×2) |
| §8.1 r6, r7 | non-beltable skips the belt; page path leaves the room | `&&`→`\|\|`, `leave_room`→() |
| §8.2, §8.3 | unit flags 0x2 / 0x2000000 cleared, others kept; 0x1000 set on a set flag | `clear_uflags` `!` deleted, `add_uflags` `\|`→`^` |
| §8.4 r2, r4, r6 | carry-one needs quality 7; `g33` needs (19, 7) and (19, 8) clear; held pairs both ways, non-pairs not held | ten mutants |
| §10.1 | g + p; pile owned by a non-player is added; negative sum → 0 | `+`→`*`, guard → true, `\|\|`→`&&`, `<`→`==` |
| §10.3 | new = 0xFF sends 0x1E | `<`→`<=` |

## 3. Surviving mutants (64): dispositions

**Equivalent (no input distinguishes them):**

- `inventory/mod.rs:296` `Grid::set_rect` (5): the bounds test inside
  the loop; every caller places only after the §2.1 bounds check, so no
  cell outside the grid is ever written.
- `belt.rs:72` `c < n`→`<=`: for c = n the column range `c..n` is empty,
  so the next column is tried either way.
- `|`→`^` of disjoint flag bits (7): `belt.rs:154` (0x400 | 0x1),
  `equip.rs:422` (0x100 | 0x4000), `deferred.rs:113, 135,
  201` (row flag pairs), `deferred.rs:418` `^`, `handlers.rs:481`
  (0x40 | 0x1).
- `grid.rs:157` `best_w > 0`→`>=`: `best` is set only when a weight is
  strictly above `best_w ≥ 0`.
- `grid.rs:281` `iw == 0 || ih == 0`→`&&`: `search` repeats the zero-size
  guard; the only difference is creating the page grid early, at its
  fixed size, which nothing observes.
- `ground.rs:350` `g + p > limit`→`>=`: at equality take = p and rest = 0
  either way. `ground.rs:362` `sum < 0`→`<=`: sum 0 sets 0 either way.

**Unreachable (an earlier check, stated by the spec, decides first):**

- `deferred.rs:418` `|`→`&` (row 20's "not 0x100/0x200"): rows 18 and 19
  match every item with 0x100 or 0x200 first and end the walk.
- `handlers.rs:192` (§7.3 step 3 "busy = 0 and page ≠ 0"): the spec
  itself notes it is unreachable (the cursor item makes the player busy).
- `handlers.rs:324, 329` (`equip_at` swap locations 11 / 12): its callers
  pass 1..10 only (0x1B: 4 / 5).
- `handlers.rs:622` (§7.12 "either missing"): the owned item check has
  already rejected a missing item.
- `handlers.rs:1135` (§7.24 r3 page / mode re-check): the stored item
  check (mode 0) and step 2's page check come first.

**Not decided by the spec here:**

- `seams.rs` (33): `MovePending` default bodies (`distance`, `collides`,
  `room_at`, `in_town`, `in_room`, `is_active`, `carry_one`,
  `quest_flag`, `held_test_units`, `create_gold`, `copy_item`,
  `consume_one`, `query_0044be50`, `merge_allowed`, `use_grid_item`,
  `use_item`, `use_item_action`, `swap_1h_with_2h`, `pickup_special`,
  `equip_picked`, `runeword`, `not_dead`, `alive`, `owns_hireling`,
  `item_bits`, `store_messages`). They are the narrowest readings of
  unwritten owner specs (`impl-moves.md` §4); the test `Fake` overrides
  every one, and no spec states their value.
- `handlers.rs:507` (0x1E gate `!` deleted): a refused gate and the
  `swap_1h_with_2h` seam both end in result 0; `0x00561220`'s body is
  unwritten (open question 14), so nothing distinguishes them yet.

**Game-file tier only:** `tables.rs:84, 111, 118` (`InvTables::from_fixed`
and its helpers): only the `#[ignore]` test `real_grid_belt_and_type_tables`
(D1–D3, already in the local run queue per `impl-inventory.md` §6) reads
real tables.

**Spec-decided but not observable through the shared fake:**
`ground.rs:245` (4: `x + 2`, `y + 3` → `-` / `*` in the `room_at` probe of
§9.1 step 2). The `Fake` answers `room_at` from a knob and ignores the
coordinates, so the probed point cannot be asserted. Follow-up for the
owner of `moves/tests/mod.rs`: record `room_at`'s arguments (like
`spot_calls`) and assert (x + 2, y + 3). Not done here (new files only).

## 4. Code vs spec

No code deviation found; no non-test code changed.

Observation (spec text, not code): §7.6 says X "becomes the cursor item"
and then N goes in "as §4.6 step 5", whose step list includes "Cursor :=
none". Read literally (and so coded and tested in `impl-moves`), the
player ends 0x1B with no cursor item while X is in mode 4. Worth a check
against `0x00563D20` or a recording when §7.6 is next verified.

## 5. Gate

`sh tools/gate.sh all` on this branch: GATE: PASS (every step, `d2-client`
included).
