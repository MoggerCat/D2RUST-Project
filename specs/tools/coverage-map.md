# Spec: Tools — behaviour coverage map

- **Status:** implemented; d2rs-own tool (no 1.14d behaviour). Evidence:
  `docs/handoff/coverage-map.md` produced from the playthrough, the check
  suite's d2rs side and the game-file tests on the real install.
- **Target version:** 1.14d tables (read only)
- **Crate/module:** `tools/coverage-map` (crate `coverage-map`, `report.py`),
  `d2-sim::debug::coverage` (`cov!`)
- **Related specs:** `tools/playthrough.md`, `tools/scenario-diff.md`,
  `tools/rng-trace.md` (the same off-by-default debug pattern)

## Summary

Counts which behaviours a run of d2-sim exercised (skills, monster classes,
AI functions, operated objects, created items and their quality, NPC
topics, quest flags, levels entered, missiles, states) and lists what the
install's tables hold that no run reached, ranked by how early a player
meets it. It tells where untested code hides. It never changes a game
outcome.

## Inputs

| Name | Type | Source |
|---|---|---|
| `D2_COVERAGE_DIR` | directory | environment of the run |
| excel tables | `.txt` | the install (Patch_D2 > d2exp > d2data) |

## Outputs / state changes

`cov-<pid>-<n>.tsv` files (`coverage-map 1`: a format line, then
`<category>\t<a>\t<b>\t<count>`), and the report (`--md`, `--json`).

## Rules

1. d2-sim's `coverage-map` feature is off by default. Without it `cov!`
   expands to nothing; with it, and `D2_COVERAGE_DIR` unset, each call is
   one branch. Counters are per thread, copy ids the game computed anyway,
   and are never read by the game (CLAUDE.md rule 6).
2. A thread's counts are written when the thread ends; a binary's main
   thread writes them through `debug::coverage::flush()` (`d2-client` main).
3. Count sites (a, b per category): skill start before its start function
   (`skills::use_::start_core_with`, skill, 0) and every do
   (`do_core`, skill, 1); monster type init (`monsters::init::create::
   type_init`, monstats row, 0: also re-inits); AI think before the
   function runs (function address, monstats row); object operate after the
   refusal checks (objects row, operate function); item creation success
   (items row, final quality); NPC interact (class, 0), menu action
   (class, 1<<16 | action), quest message (class, 2<<16 | message); quest
   flag set when the bit was clear (slot, bit; player and game records
   alike); the client's level change (`client_level_change`, new level);
   missile creation after the owner check (missiles row); state toggled on
   (states row).
4. The report ranks a never-exercised entry by the earliest level (act, then
   Levels.txt order) where a player meets it: monsters from the Levels.txt
   mon/nmon/umon columns (minions and spawns inherit), monster skills and
   mode missiles from those monsters, player skills by required level
   mapped to the act a character of that level plays, missiles and states
   from the skills that make them, objects by operate-function commonness,
   items by base level. Entries with no such link are counted as
   "unreachable from the tables" and listed after.

## Constants & data dependencies

Levels, monstats, skills, Missiles, states, objects, weapons, armor, misc.

## Randomness

None; the counters draw nothing.

## Edge cases & original bugs

None (d2rs-own tool).

## Test vectors

| Input / seed | Expected output | Source (trace id) |
|---|---|---|
| `report.py --selftest` | merges counts; rejects a bad category and a bad format line | selftest |

## Provenance

d2rs-own design (2026-10-09, `q-tool-coverage-perf`).

## Open questions

None.
