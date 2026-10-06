# Gap tests: items, stats, units

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Session: unit tests from specs for the uncovered rules of
`specs/items/{affixes,generation,quality,properties}.md` and
`specs/sim/{stats,stat-lists,units}.md` (METHODS M14, M08). Branch
`claude/gaps-items-stats`, from `claude/bold-ptolemy-jvyvxy` at `ac01471`.
All new claims are `unit` tier (unverified against 1.14d, M02).

## Coverage (`python3 tools/coverage.py --summary`, any tier)

| Spec | Units | Before | After | Still uncovered |
|---|---|---|---|---|
| `items/affixes.md` | 46 | 18 | 44 | 2 |
| `items/generation.md` | 72 | 31 | 70 | 2 |
| `items/properties.md` | 37 | 21 | 36 | 1 |
| `items/quality.md` | 48 | 23 | 45 | 3 |
| `sim/stat-lists.md` | 86 | 46 | 81 | 5 |
| `sim/stats.md` | 38 | 19 | 36 | 2 |
| `sim/units.md` | 41 | 16 | 34 | 7 |
| **these 7 specs** | 368 | 174 (47.3%) | 346 (94.0%) | 22 |
| repository total (any) | 2704 | 1815 (67.1%) | 1987 (73.5%) | |

New test files: `crates/d2-sim/src/items/tests/gaps_{affixes,quality,generation,props}.rs`,
`crates/d2-sim/src/stats/gap_tests.rs`, `crates/d2-sim/src/units/gap_tests.rs`
(140 tests). Test-only helpers: `pub(super)` on fixtures in
`stats/tests.rs` and `units/tests.rs`, `itemstatcost_with` in `stats/tests.rs`,
`#[cfg(test)] StateTable::set_srvactivefunc` in `stats/states.rs`.

M08: each writer broke the code by hand (items 13 + 7, stats 12) and
confirmed a new test fails; the uncaught mutants were unobservable
(low-quality tier `>3`/`>4`, `hi < lo` stack roll, a duplicate row-count
check) except MulDiv's 32-bit product, for which
`muldiv(-0x20_0000, 0x1_0000, 3) == 0` was added to the §5 text test.

## Code fixes

1. `units.md §3.1 r7` and the §1 table — `crates/d2-sim/src/units/lifecycle.rs`
   (~line 117): allocation set `mode := arg` for tiles too; the tile init
   only sets `flags |= 0x2`, so tiles now keep mode 0.
2. `quality.md §edge-cases-original-bugs r4` — `crates/d2-sim/src/items/tables.rs`:
   unique rarity is read as 32 bits at `+0x30` (`UniqueRec::from_record(r, raw)`;
   `ItemTables::from_fixed` builds uniques from the raw records like
   setitems). No change on 1.14d data (the upper bytes are 0).

## Deviation found, not fixed (outside this session's modules)

- `units.md §5 r4`: an event scheduled through `0x005416B0` with expire −1
  becomes every-tick **and loses its callback**. `TimerQueue::schedule`
  (`crates/d2-sim/src/tick/timer.rs`) passes the callback to
  `schedule_every_tick`; `tick.md §5.2` does not mention dropping it.
  Fix (tick module owner): pass `None` there, after reconciling the two
  specs. Rule left unclaimed.

## Rules left uncovered

| Rule | Why |
|---|---|
| `affixes.md §3.1` | calling-convention wrappers (`0x005C18E0`/`0x005C1940`); d2rs has one `roll_affix`, format-0 roller unspecified |
| `affixes.md §5 r0` | not a rule: spec line ~152 wraps so a line starts with "0.", which the tool reads as item 0 and the claim grammar can't name. Spec needs rewrapping |
| `generation.md §3 r9` | personalized flag `0x1000000` can't be set before step 9 through `create_item`; needs a caller/hook that sets it |
| `generation.md §9 r2` | spec question 1 below |
| `properties.md §9 r3` | quality-5 non-gem/rune filler `0x00663CC0` unspecified (spec open question 2); code does nothing |
| `quality.md §2` | ownership statement (treasure.md §6 picks drop quality); nothing in quality to test |
| `quality.md §8 r1` | format-0 ×5 durability "not specified further"; code TODO |
| `quality.md §edge-cases-original-bugs r5` | >10 qualityitems rows is UB in the original; 1.14d has 8; d2rs sizes the array to n |
| `stat-lists.md §4 r3` | a list of 1.14d callers, not stats behaviour |
| `stat-lists.md §6.4 text` | spec question 3 |
| `stat-lists.md §9.3` | "by state and flags `0x006257D0`" has no rule; only a TODO reading in `wiring/economy/item_stats.rs` |
| `stat-lists.md §edge-cases-original-bugs r4` | expired extended list loops forever in 1.14d; d2rs panics deliberately (`assert!` in `expire_lists`) |
| `stat-lists.md §edge-cases-original-bugs r5` | spec question 4 |
| `stats.md §2 r2` | 1.14d data fact (which stats have ValShift 8): needs the game-file check (`check_stats.py --files`) |
| `stats.md §edge-cases-original-bugs r5` | op-graph cycle recurses without end: stack overflow aborts the test process |
| `units.md §3.1 r8` | d2rs always requires `SUNIT_Add`; path settings belong to the path spec (`init_kind` hook) |
| `units.md §4.5` | tests exist (`player_mode_starts`) but the `0x006E1740` position/unit forms and the GH argument −1 aren't modelled (path spec targets); claim withheld |
| `units.md §5 r4` | deviation above (tick module) |
| `units.md §6.1` | spec question 5 |
| `units.md §6.4` | object handler bodies delegated to `object_event` hook (objects spec) |
| `units.md §6.5` | item replenish body delegated to `item_replenish` hook (items spec) |
| `units.md §edge-cases-original-bugs r4` | `GetTickCount` wall-clock history: open question 3, not modellable under determinism |

## Spec questions

1. `generation.md §9 r2`: does "Format 0 only" cover only the forced socket
   count, or also the flag copies after "Then"? Code copies flags for every
   forced request (`TODO(items OQ-G2)` in `create.rs` `forced()`).
2. `generation.md §4 r5`: does the §5.3 quest-difficulty step run only when
   "quest" and a request are given, or always? Tested only with a request.
3. `stat-lists.md §6.4`: the "Consequences" text (and edge case 2) say
   per-level stats never reach a player's full array, but rule 5 does
   `set-full(d, recompute(L, d))`; after setting 216 = 8 and level 20 the
   full array holds `(216, 8)`. Code follows rule 5. Text or rule needs fixing.
4. `stat-lists.md` edge case 5: "collect at most 16 A53 keys" vs "16-slot
   buffer, no bound check" (overflow). Code takes the first 16.
5. `units.md §6.1` vs `stat-lists.md §10.1`: units says stamina and mana
   regen run only "if `0x00580610`"; stat-lists runs all three steps
   unconditionally. Code follows stat-lists (TODO in `dispatch.rs`
   `player_regen`).
6. `units.md §4.5`: what event 1 in mode KB does when `0x0057EEC0(4, 1, 0)`
   fails (code: nothing; not asserted).
7. `units.md §2`: "Dead" lists only players and monsters; `stat-lists.md
   §10.1` adds "any other unit type" (code follows stat-lists; test checks
   only the units.md cases).
8. `stat-lists.md §10.3` / `dispatch.rs`: is skill 0 valid for event 9?
   (code treats it as valid, TODO; test avoids it).
9. `units.md §3.1 r3`: quest chain field `+0x74` isn't modelled; the claim
   rests on the act field.
