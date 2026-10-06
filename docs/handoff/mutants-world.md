# Handoff: mutation testing of `d2-sim::world` — `claude/mutants-world`

> Folded into `docs/HANDOFF.md` (§1–§5, §7, §8) and `docs/PLAN.md` as of the eighth fold (`claude/docs-fold-8`); this file stays as the detailed record.

Cloud test session, 2026-10-06, medium effort (METHODS M08, M14). Base:
`claude/tender-meitner-mphas3` at `4b5b0bf`. Repo only, synthetic data,
no game files (M09). Scope: `crates/d2-sim/src/world/**` (quests,
waypoints, cube, npc, vendors and the shared TSV reader). Parallel
sessions ran other `d2-sim` modules; only files under `world/` changed.

## 1. Counts

`cargo mutants -p d2-sim --file 'crates/d2-sim/src/world/**' -j 4
--timeout 60` (cargo-mutants 27.1.0), then `--iterate` twice on the same
output directory after the tests were written. `mutants.out` is not
committed.

| | Mutants | Caught | Missed | Timeout | Unviable |
|---|---|---|---|---|---|
| before | 2385 | 1961 | 389 | 13 | 22 |
| after | 2385 | 2316 | 34 | 13 | 22 |

Timeouts are mutants that loop forever (e.g. `type_pick` wrap, quest
updater tick); the 60 s limit detects them, so they count as detected.

Missed before, by file: `cube.rs` 57, `mod.rs` 4, `npc.rs` 13,
`npc/hire.rs` 8, `npc/services.rs` 1, `quests.rs` 35, `quests/act1.rs`
68, `quests/tables.rs` 6, `vendors.rs` 12, `vendors/gamble.rs` 13,
`vendors/price.rs` 115, `vendors/store.rs` 16, `vendors/trade.rs` 39,
`waypoints.rs` 2. The 355 killed mutants were all case (a): the spec
decides the outcome and no test checked it (boundaries such as `<` vs
`<=`, conjunctions tested only with every term true, S / B / R terms
checked only through the one a transaction returns, guards whose two
forms coincide on the values the old tests used).

## 2. Tests (115, all unit tier, new files only)

Each test names the spec rule it pins in a `// From specs/...` comment.
One coverage claim was added (`specs/data/callbacks.md §2 text`, the full
input-slot table at every offset); the other tests check one case of a
rule, not its whole outcome, so they get no claim (HANDOFF §8 lesson on
overclaiming). `coverage.py --summary` is unchanged (2404/2704 any,
217 verified).

| File (`crates/d2-sim/src/world/`) | Tests | What they pin |
|---|---|---|
| `mutant_tests.rs` (child of `world`) | 2 | TSV reader: 1-based line numbers in rows and `Columns` errors (reader contract, not a spec rule) |
| `cube/mutant_tests.rs` | 19 | slot offsets of every input / output slot; stat ops 4–6 and record 0; `upg` with an elite item; `nru` and op-28 tests only when selected; slot-0 stat op; capture level store-back, `mod` key, `exc`/`eli`/version upgrade; type pick stop item, version filter, 256-candidate cap; kind 0 with `mod`; `mod` copy class per kind; `flags2` socket bit with quantity; `uns` without fillers; craft property 0 and chance 100 (no draw); `rep` / quantity / socket steps; quest hook only for `hst`/`qf2`; socket test on slot 0 only; put-in cube and trading tests |
| `npc/tests/mutant_tests.rs` (child of `npc::tests`) | 7 | unit type 5; AI halt needs `npc` and `interact`; distance 6 starts; chat needs an interaction list; heal sound from each single change; hire list sizes 1 and 69 (and refusal of an empty range); Kashya gate at level 8, name below `first` |
| `quests/mutant_tests.rs` | 34 | `clear_callback`; timer wrap 0xFFFFFFFF − due; `link_monster`; forced class 242; Den leave-town arg; chat end by act; default status branches; chain 40 assert; barbarians only for list[36] / filter 36; Warriv chain-6 condition; Tyrael completion; 0x91 act V; warp checks 100, 132; `read_clue`; every Act I start / reward guard; start bit skips; area event bits and states; Flavie kill `ret`; Blood Raven / Cow King kills; unhandled status functions; Den 76 without 1.13; Den few-left conditions; stone order on the quest seed; Wirt with no piles; Malus skips; imbue grant; `messages_for` |
| `vendors/tests/mutant_tests.rs` (child of `vendors::tests`) | 17 | `is_type` reads `type2` only when > 0; `valid_code` refuses 0 and four spaces; own chain node; price predicates (durability-applicable, charges, replenishable stack, repairable); affix guard above 0x10000; item-skill small form; store quality band at ilvl 5; Hell ultra boundary; `mark` with sockets; page-1-only retry; item at store level; ilvl 25; item format 100; permanent nulls and the 32 limit |
| `vendors/gamble/mutant_tests.rs` (child of `gamble`) | 4 | upgrade `w_u` / `w_x` boundaries and the skipped ultra draw; T = 1 draws; only flag 0x10 cleared |
| `vendors/price/mutant_tests.rs` (child of `price`) | 13 | (A) both forms and the S = 0x10000 edge on S, B and R; by-time fields (`sim/stats.md` §8 r1); (B) `bonus_term` both forms and every `encode`; (C) charged-skill terms; inferior halves; `div` needs stackable; stack apply, R·(M − qty), M·B; sale keeps R unscaled; ethereal sale at durability 1; repair over max durability; ammunition R; gamble price parts (L = 6, level 50, stacks, `0   `) |
| `vendors/tests/trade/mutant_tests.rs` (child of `vendors::tests::trade`) | 17 | quest slots only for flags ≠ 0; stash cap and receive cap boundaries; `repair_item` conditions; other transactions keep the gamble list; fill-stack rules; copy flag 2; unique no-sell bit; quest item refused; failed unequip removes only the copy; repair-all needs; throwing-stack rule; no partial step at full durability; partial repair `per` and its bounds; shown items keep flag 1; refresh needs all four; `town_act` |
| `waypoints/mutant_tests.rs` | 2 | waypoint class needs operate 23 and init 17; bottom edge of the room rectangle |

`mod` lines: test-only `mod mutant_tests;` at the end of `world/mod.rs`,
`cube.rs`, `quests.rs`, `waypoints.rs`, `vendors/gamble.rs`,
`vendors/price.rs` (end of file, so mutant line numbers stay those of the
baseline run), and after the `use` lines of `npc/tests.rs`,
`vendors/tests/mod.rs`, `vendors/tests/trade.rs`. No library code
changed.

## 3. Surviving mutants (34), with reasons

**(b) Equivalent or unobservable — 33**

| Mutant | Reason |
|---|---|
| `cube.rs:866` `level < 1` → `<= 1` | a level of 1 is "stored back" as 1: no change |
| `cube.rs:936`, `:945` `p < USED_MARKS` → `<=` | each alone touches `used[47]`, which the other never reads / writes (p = 48 is neither marked nor tested, §6.1) |
| `cube.rs:1053` `UNS \| REM` → `^`; `:1137` `\|` → `^` | disjoint bits: `^` = `\|` |
| `npc.rs:791` `count < 2` (×3); `npc/hire.rs:366` `count < 2` (×3) | the `first` argument of `send_hire_list` has no effect in `npc.md` §7.2 and is unused |
| `npc/services.rs:177` `0x20 \| (4 or 2)` → `^` | disjoint bits |
| `quests.rs:557, 745, 746, 753, 754, 763, 775, 804` (deleted `EventArgs` fields) | no callback the spec describes reads that field for that event (player enters / leaves, item events, `target` of events 2 and 3); the unspecified ones are reported unhandled without their args |
| `vendors.rs:330, 389, 432, 493, 497, 506` (`From` impls, `typed`, `cost_mod`, `from_fixed`) | the projection from a `FixedSet` needs the fixed-up 1.14d tables; unit tests build `VendorTables` directly. No game-tier test loads it yet (gap, §4) |
| `price.rs:301` `x < 0x10000` → `<=` | at x = 0x10000 both forms give the same (x divisible by 1024) |
| `price.rs:498` `qty < M` → `<=` | at qty = M: R·0 = 0 and B := M·B − 0 = M·B, as the else branch |
| `price.rs:506` `t = 3 && !eth` → `\|\|` | R after rule 10 is read only for t = 3, and an ethereal item is not repairable (rule 0) |
| `trade.rs:57:15` `s2 < 0` → `==` / `<=` | s2 = s + g − c ≥ 0 once g + s ≥ c passed; s2 = 0 → 0 either way |
| `trade.rs:71` `g + a > cap` → `>=` | at equality the mutant drops a 0-gold pile; the fake sums dropped gold, so a zero drop is invisible. Whether `0x0055A090` makes a pile of 0 belongs to the drop routine |
| `trade.rs:643:14` `g > 0` → `>=` | g = 0 makes `per < g·1024` false anyway |

**(c) Not decided by the spec — 1**

| Mutant | Note |
|---|---|
| `cube.rs:1172` `fillers.len() < 18` → `<=` | `cube.md` §7 gives the fillers list "max 18" but not what a 19th does (the original's stack array would overrun; the code's TODO keeps 18). A spec decision first, then a test |

No code was found wrong against its spec: every test written from the
spec's numbers passed on the unchanged code.

## 4. Follow-ups

- Game-tier check for `VendorTables::from_fixed` (and its `From`
  projections): load the live tables and compare a few rows (e.g. Charsi
  `npc.txt` multipliers, `difficultylevels` gamble odds, an `itemtypes`
  row) with `vendors.md` §9.3 / Constants. Queue in HANDOFF §5 when
  folding.
- `cube.md` §7: say what happens past 18 fillers (or that it cannot
  happen in 1.14d data).
- `npc.md` §7.2: the `first` argument has no described effect; confirm
  it is unused in 1.14d or describe it.

## 5. Gate

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings` (after `sh tools/cloud-setup.sh`); `cargo test -p d2-sim`;
`cargo run -p depcheck`; `python3 tools/spec_index.py --check`;
`python3 tools/methods.py check`; `python3 tools/coverage.py --check`
and `--selftest`. Results in the commit message of this branch.
