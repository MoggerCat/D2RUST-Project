# Gap tests: treasure, combat/hit, skills/levels, monsters/population

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/gaps-combat-items-monsters`, from `main` at `edad871`.
Cloud session, tests from specs only (M14 medium, M08). Test code only;
no non-test code changed. All new claims are `unit` tier (unverified
against 1.14d, M02), so verified coverage is unchanged.

## 1. State

Coverage (`python3 tools/coverage.py --summary`, any tier):

| Spec | Units | Before | After | Still unclaimed |
|---|---|---|---|---|
| `specs/items/treasure.md` | 85 | 62 (72.9%) | 77 (90.6%) | 8 |
| `specs/combat/hit.md` | 49 | 41 (83.7%) | 46 (93.9%) | 3 |
| `specs/skills/levels.md` | 47 | 40 (85.1%) | 43 (91.5%) | 4 |
| `specs/monsters/population.md` | 165 | 156 (94.5%) | 158 (95.8%) | 7 |
| repository total | 2704 | 2401 (88.8%) | 2426 (89.7%) | |

The other in-scope specs were left unchanged, because an earlier gap
session already reviewed their open units and judged them unclaimable:
`combat/damage.md` (11), `missiles/missiles.md` (13) and `monsters/ai.md`
(11) in `docs/handoff/gaps-combat-ai.md`, and `items/{affixes,generation,
properties,quality}.md` (8) in `docs/handoff/gaps-items-stats.md`. Their
reasons still hold on `edad871`. `affixes.md §5 r0` belongs to another
session. `monsters/init.md §1` is covered under §4 below.
`combat/vitals.md` and `skills/use.md` are at 100%.

24 new tests, all passing. They claim 25 units: one treasure test claims
both §5.3 text and §5.3 r1. `cargo test -p d2-sim` reports 1230 passed and
5 ignored.

M08: each claimed test was checked by breaking its rule in the code by
hand, seeing the test fail, then restoring the file (`git diff` shows no
non-test change). That was 15 breaks in treasure, 10 in hit/levels and 2
in population. Examples:
- the TC limit `>` changed to `>=`;
- the monster or chest `max` changed from 6 to 7;
- `continue` changed to `break` when no entry is found;
- a signed max of the mods;
- the `Version ≤ 100` filter removed;
- AC_VS_MISSILE swapped with AC_VS_HTH;
- weapon mastery added for missiles too;
- the `!block` early return removed;
- the copy limits changed from 128 to 129 and from 32 to 33;
- the reqvit check dropped;
- the level clamp applied to players only;
- an extra room-seed step on a §5 result of 1;
- offset set 2 made possible for a tentacle.

## 2. Code map rows

| File | Tests | Claims |
|---|---|---|
| `crates/d2-sim/src/treasure/gap_tests.rs` | 14 | treasure §1.2, §3.5, §4 r6, §5.3 text, §5.3 r1, §5.3 r5–r7, §5.4 text, §5.6, §5.7 r6, §7 r1, edge r1, r6, r8 |
| `crates/d2-sim/src/combat/hit_gap_tests.rs` | 5 | hit §2 text, §3.2 r3, §5 text, §6.1 r1, edge r6 |
| `crates/d2-sim/src/skills/levels_gap_tests.rs` | 3 | levels §1 l2 r1, §6 r4, edge r9 |
| `crates/d2-sim/src/monsters/population/gap_tests.rs` | 2 | population §3.3, edge r15 |

Test-only edits beside the new files:
- one `#[cfg(test)] mod …;` line each in `treasure/mod.rs`,
  `combat/mod.rs`, `skills/mod.rs` and `monsters/population/mod.rs`;
- the fixtures in `treasure/tests.rs` made `pub(super)`;
- `monsters/population/tests/mod.rs` changed from `mod fake` to
  `pub(super) mod fake`.

## 3. Signature changes

None. No public item changed.

## 4. Seams reached / rules left unclaimed

| Rule | Why |
|---|---|
| treasure §1.1 | byte offsets and record sizes: a memory layout d2rs doesn't reproduce. The append rule is exercised by `treasureclassex_rows`. |
| treasure §1.5 text | counts measured on 1.14d; needs game files |
| treasure §3.3 | quest-owner resolution sits behind the `quest_open` callback (quests and units specs). Only the code's side of the gate is tested (`monster_tc_choice`). |
| treasure §3.6 | the Find Item caller skips the gate and makes the ignored `roll(100)`; the skills spec owns that. Treasure exposes only the `find_item` flag. |
| treasure §4 text | calling convention and the caller/`Q` table (objects and quests specs) |
| treasure §6 text | inputs and calling convention; the draw seed is proven by the §6 vectors |
| treasure §6 r4 | `M` (stat 80 plus the minion owner's) comes from the caller through `Recipient.magic_find`, a units/stats seam |
| treasure §7 r2 | the start point and free-spot search are inside `DropSink::place` (collision spec) |
| hit §4 text | calling convention only |
| hit §4 r6 | the second clear of the hit flag can't be observed on its own, because step 5 already clears it; no perturbation can fail |
| hit §6.2 text | `roll(100)` equal to inline `lo′ mod 100` is owned by `sim/rng.md` §3 |
| levels §3.1 text, §3.3 text | function names and addresses only |
| levels §6.4 text | prose pointing to open question 5 (return codes 2/3) |
| levels edge r7 | `toht` recursing without end overflows the stack; it can't be unit-tested, and no 1.14d row uses it |
| population §1 r2 | callers and triggers are open question 1; see question 2 |
| population §1 r3 | facts from recordings: room fill order, kept GUIDs, consecutive new GUIDs. Needs a trace replay; the order is owned by tick and units. |
| population §14 r2–r6 | each one names another spec as owner (death spawns, AI/skill reuse, revives, `MonSpcWalk`, missile presets) |
| monsters/init §1 | a table of wrapper entry points, mostly in callers; `monsters/init` has no such wrappers. Population builds the create request itself. |

## 5. Deviations and fixes

None found and no code fixes. The hit edge r6 test checks that `hit.rs`
passes the limits 128 and 32 to the `stat_entries` seam; the cut itself
happens behind the seam (the fake's behavior).

## 6. Questions

1. `treasure.md §1.2`: a lookup "sees only TCs created before". Can a row
   name its own TC? The record exists before its items are parsed, but the
   code registers the name after the items, so a self-reference misses.
   Untested; to confirm in a spec session.
2. `population.md §1 r2`: `0x0052D0F0` "runs the same sequence for one
   room". Does that include the §1 r1 ambient spawns? `populate_once`
   (`monsters/population/mod.rs` ~146) leaves ambient out on purpose (impl
   note 14). This ties into open question 1.

## 7. Local checks to queue

None new. Every claim is unit tier. The game/trace checks these rules would
need (treasure §1.5 counts on 1.14d data, population §1 r3 recorded fill
order) are already listed as open questions or spec text; they need a spec
session to state their comparison first.

## 8. Gate (on this branch, before the push)

- `cargo fmt --check`: ok
- `cargo clippy --workspace --all-targets -- -D warnings`: ok (after
  `sh tools/cloud-setup.sh`)
- `cargo test -p d2-sim`: 1230 passed, 5 ignored (doc-tests: 1 ignored)
- `cargo run -p depcheck`: OK (8 crates)
- `python3 tools/spec_index.py --check`: ok
- `python3 tools/methods.py check`: 21 methods OK
- `python3 tools/coverage.py --check`: 3220 claims, 0 errors;
  `--selftest`: ok

Non-source files in the diff: this note only. No generated files.
