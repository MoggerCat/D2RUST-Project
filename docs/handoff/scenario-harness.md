# Handoff: differential scenario harness — `claude/scenario-harness`

Cloud implementation session, 2026-10-06, high effort. Repo only,
synthetic install, no game files. Not folded into `docs/HANDOFF.md` or
`docs/PLAN.md` (this session does not edit them); the coordinator folds
§6 into the local run queue and §7 into the plan.

**Goal.** One scripted scenario runs on the original 1.14d and on d2rs;
each side writes a scenario trace; a comparator reports the first
divergence. This branch builds the d2rs half, the shared formats and the
contract of the original half.

## 1. What exists

| Piece | Where | Spec |
|---|---|---|
| Script format (strict parser, canonical writer, typed messages from `client-messages.tsv`, references `@player`, `@x±N`, `@<type>[:<class>][#n]`, `@wp`; `spawn` steps; inline characters with stats, skills, waypoints, quest flags and items with quality, affixes, unique / set / runeword ids, sockets and socketed items) | `crates/conformance/src/scenario/script.rs` | `specs/tools/scenario.md` §1–§3 |
| Scenario trace (`scenario-trace` 1, JSON lines; strict reader, canonical writer) | `crates/conformance/src/scenario/trace.rs` | `traces/FORMAT.md` §Scenario traces |
| Comparator (first divergence, masks, summary, verdict) | `crates/conformance/src/scenario/compare.rs` | `scenario.md` §5–§6, `specs/tools/scenario-masks.tsv` |
| d2rs runner + CLI | `tools/scenario-run` | `scenario.md` §4 |
| Starter scenarios (8) | `traces/scenarios/*.scenario` | — |
| `GameData::drlg_world_in` (difficulty-aware act 0) | `crates/test-fixtures/src/game.rs` | — |

```
cargo run -p scenario-run -- check traces/scenarios/*.scenario
cargo run -p scenario-run -- run traces/scenarios/walk-town.scenario --synthetic   # CI data
cargo run -p scenario-run -- run traces/scenarios/walk-town.scenario               # $D2_GAME_DIR (live)
cargo run -p scenario-run -- compare traces/raw/walk-town.original.trace.jsonl traces/raw/walk-town.d2rs.trace.jsonl
```

`run` writes `traces/raw/<name>.d2rs.trace.jsonl` (gitignored) and
prints run notes (dispatch result per message, unresolved references,
transport refusals) and the trace's gaps. `compare` exits 0 match,
1 diverged, 2 partial, 3 error.

Tests (CI, synthetic): `cargo test -p conformance scenario` (script,
trace, comparator unit tests incl. every perturbation kind;
`tests/scenario_props.rs`: random scripts round-trip), `cargo test -p
scenario-run` (every starter parses and round-trips; runs twice
byte-identically; the trace reads back; S→C capture at its tick; the
comparator finds every perturbed record of two real runs at exactly
that record — 300+ perturbations; unsupported characters are errors).
`#[ignore]` `starters_run_on_the_live_install` (needs `D2_GAME_DIR`).

## 2. Starter scenarios

| Script | Does | d2rs on synthetic data today |
|---|---|---|
| `walk-town` | three relative walks in the Act I town | walks (positions move); no S→C messages yet (walk events not wired to messages) |
| `run-cold-plains` | walk, then five relative runs towards the exit | runs; town only (the synthetic set has no Cold Plains) |
| `kill-monster` | runs out, attacks the first fallen (monstats 19) five times | runs; attacks unresolved (no fallen) |
| `pickup-drop` | `kill-monster`, then run to and pick up the lowest-GUID item | as above; pick-up unresolved |
| `vendor-buy-sell` | Akara (148): talk, chat, trade, buy, sell the cap | unresolved (no NPC preset spawned); `char item` gap |
| `waypoint-travel` | operate the town waypoint, travel to level 3 | 0x13 done; 0x49 result 3 (level 3 has no waypoint in the synthetic set); travel to level 1 sends 0x07, 0x0D, 0x15 (test `server_messages_are_captured_at_their_tick`) |
| `cast-firebolt` | select Fire Bolt (36) right, cast in town, run, cast twice more | dispatched, result 0, no messages |
| `champion-pack` | level-90 sorceress (stats, 4 skills, waypoints, a quest flag, unique / rare / magic items, a socketed rune), Hell; spawns a champion pack of fallen (19, umod 16) beside her, selects Fire Bolt, casts at the champion and a minion | fallen (19) does not exist in the synthetic set; with class 1 (test `a_spawned_champion_pack_is_recorded`) the leader and 1–3 minions spawn, draws recorded at tick 5; item and quest gaps |

All starters use seed (T) `0x1234` and init (I) 644409375 (the test
character's map ID, `sim/rng.md` §5.4, used as the DRLG seed; a save
that has reached the difficulty uses its own map ID instead, which the
script then states as `char map`). Targets are relative to the character
(`@x+30`): the town layout depends on the map seed, so the first
original run of each script shows where the exit, the fallen and Akara
are; then the targets are rewritten (§7 step 2).

## 3. Original side: `run_scenario.py` 0.2.0 → this format (to PC 2, Local2 testing lane)

Read: `tools/trace-recorder/run_scenario.py` on
`origin/claude/tool-run-scenario` (63ae730, "trace-recorder run_scenario
0.2.0", selftest ok, not run on 1.14d) and `specs/tools/original-hooks.md`
on `origin/claude/specs-staging` (161ba9d). Not merged here: this branch
cites the spec by name only.

**Already aligned** (this branch changed to match the original side):
`seed` is the time value T (< 2^31) and `init` the value I of game +0x7C
(original-hooks §2 rule 1; the tool's `seed` / `init_seed`); the game
seed is `{T, 666}` stepped once, d2rs does the same step; the DRLG seed
is `char map` when given (a save that has reached the difficulty), else
I (§2 rule 2); game type 3 (§5.2); `char save <name>` is the tool's
`save` (same name rule) and may stand beside the inline lines; life and
mana are the raw full-array values (§4 rule 3); injection at
`0x0044F136` through `0x0052AE50` after the return of the previous tick
(§1, §3 rule 1) is `scenario.md` §4 rule 2 (a)–(c); snapshot after the
tick at `0x0052FD1E` (§3 rule 3); units sorted by (type, GUID) (§4 rule
1).

**Changes `run_scenario.py` needs** to run `traces/scenarios/*.scenario`
and write a trace `scenario-run compare` reads (exact; each names the
rule):

1. **Input.** Read `.scenario` scripts (`scenario.md` §2–§3), not the
   JSON v1 file: strict line parser; typed `msg` fields encoded from
   `specs/sim/client-messages.tsv` (`layout` column: u8/u16/u32
   little-endian at `@off`, `uN`/`bitN` OR-ed into the u32; unset bytes
   0); `hex` steps as is. Map: `seed` → `seed`, `init` → `init_seed`,
   `char save` → `save`, `char class` → `class`, `difficulty`
   normal/nightmare/hell → 0/1/2, `end` → see change 3. `char at`,
   `char stat|skill|item|waypoint|quest|map` lines: not applied by the
   tool (it loads the save); list them in `gaps` unless the save is
   known to match. Scripts without `char save` cannot run on the
   original: error.
2. **References** (`scenario.md` §3 rules 3–5) resolved at the
   injection stop from the unit walk the tool already does
   (`server_units`): `@player` = GUID of the type-0 unit (1 in single
   player); `@x`/`@y` its dynamic-path sub-tile x/y ± N; `@T[:C][#n]`
   the n-th GUID in ascending order among server units of type T
   (class C); `@wp` objects whose objects row has `OperateFn` 23
   (read the objects table once, as `dump_tables.py` does, or from
   `traces/raw/<time>-tables/`). Unresolved → write the `c2s` (or
   `spawn`) record with `unresolved` and inject nothing.
3. **Ticks.** The tool's `tick` is the absolute frame (game +0xA8) and
   it injects steps with `tick == done_frame + 1`. Scripts use
   relative ticks: after the `ready` probe's rule (first frame F0 whose
   client state is 4), inject step tick t at the stop after frame
   F0 + t returned, i.e. absolute tick F0 + t + 1 (`scenario.md` §4
   rule 2), and stop after frame F0 + 1 + `end`. Record `t` relative.
4. **Records** (`traces/FORMAT.md` §Scenario traces; one JSON object
   per line, `sort_keys=True, separators=(",", ":")`, as the tool's
   `ScenarioWriter.line` already does):
   - header `{"k":"header","format":"scenario-trace","version":1,
     "game_version":"1.14d","side":"original","tool":...,"data":"1.14d",
     "scenario":<name>,"scenario_sha256":<sha256 of the canonical
     text, scenario.md §2 rule 6>,"seed","init","end","streams":[sorted],
     "gaps":[...]}` (not the raw file's header; keep the raw file
     beside it if wanted);
   - `inject` → `{"k":"c2s","t","i","b"}` (i = step index in the tick);
   - `units` (one record per tick holding a list) → one `{"k":"unit",
     "t","type","guid","class","mode","x","y","life","mana"}` per unit,
     only at the snapshot ticks (`snapshot every|at` and `end`), with
     `life`/`mana` = 0 (not null) for a unit without a stat list;
   - `s2c` hook records (`0x0053B280`) between the injection stop of
     tick t and the return of tick t, client 0 only →
     `{"k":"s2c","t","c":0,"b"}` in hook order;
   - new: `{"k":"rng","t","before":[lo,hi],"after":[lo,hi]}` = game
     +0xD0 read at the stop at `0x0044F136` (before injecting) and at
     `0x0052FD1E` of tick t;
   - optional `rng-draws`: `{"k":"draw","t","n","before","after",
     "site":"0x…"}` for draws whose seed address is game +0xD0
     (helper draws carry it; inline draws need the address of the
     `mul` operand — if that is not available, do not list
     `rng-draws`);
   - `{"k":"stats","t","type":0,"guid","base":[[stat,layer,value]...]}`
     needs the base array of the extended list (not in original-hooks
     §4; leave `stats` out of `streams` until it is specified);
   - last line `{"k":"end","t":<end>}`; a run that stops early writes
     no `end` record.
5. **Spawn steps** (`scenario.md` §3.1): not in the tool. Until the
   spawn calls are specified (§4 Q12–Q14), write `{"k":"spawn","t","i",
   "failed":true}` and a gap `spawn: not supported`, so the comparison
   reports the divergence instead of failing.
6. **Self-test:** port the canonical writer and compare the hashes
   with `cargo run -p scenario-run -- check` and a d2rs trace header of
   each starter; perturb each record kind of a written trace and check
   `scenario-run compare` (exit 1, the record found).
7. **Saves to create** on the PC for the starters (`-nosave` keeps them
   unchanged): `ScnSor` (sorceress, level 1, Normal; walk-town,
   run-cold-plains, cast-firebolt — Fire Bolt 1 point —, waypoint-travel
   — knows waypoints 1 and 3 —, vendor-buy-sell — 5000 gold, a cap in
   the inventory at 0,0), `ScnAma` (amazon level 5; kill-monster,
   pickup-drop), `ScnSorNinety` (the champion-pack character: level 90,
   Hell reached, its stats, skills, items and waypoints as in the
   script). The inline lines of each script must describe its save.

## 4. Questions for the PC 1 spec writer

`specs/tools/original-hooks.md` answers the former Q1 (seed, §2),
Q2 (map seed rule, §2 rule 2), Q4 (injection, §1), Q5 (tick 0, §1 rule 5,
§3), Q6 in part (`s2c` hook kept from `record_packets.py`), Q7 (game
seed reads: +0xD0 at the stops of §1 and §3), Q8 (unit walk, §4 rule 1),
Q9 (snapshot fields, §4 rule 2–3), Q11 (game type 3, §5.2). Still open
(numbers kept):

3. **Inline character.** Starting from a save only (§5.3): either
   `specs/formats/d2s.md` (header, stats, skills, items by code with
   quality, item level, affixes, unique / set / runeword id, sockets
   and socket fillers, positions; waypoints; quest flags; map ID), so a
   tool writes the save of a script's inline lines and d2rs reads it;
   or the in-memory route after join (stat set, skill points, item by
   code, waypoint bit, quest flags). Say which. The `champion-pack`
   starter needs the full late-game state.
6. **Direct sends** (`intents-events.md` §3.3 rule 5): how they appear
   relative to the `0x0053B280` hook, for the `s2c` stream.
10. **Default start.** Where a loaded character stands at tick 0 in
    `char area` (its town start, `drlg/levels.md` §10), and how a tool
    would place it at `char at <x> <y>`.
12. **Spawn: placement.** `0x005B2A00` (`monsters/population.md`
    placement): convention, the arguments `scenario.md` §3.1 rule 2
    fixes (room, no coordinate list, class, mode 1, x, y, radius −1,
    flags 0), the return, called on the game thread at the
    `0x0044F136` stop (as the injection). How to find the active room
    holding a sub-tile point (the walk of original-hooks §4 gives a
    unit's room; a room list walk is needed).
13. **Spawn: bosses.** The conventions of the random boss `0x005A43E0`,
    boss spawn `0x005A09E0`, champion pack member `0x005A48C0`,
    champion minions `0x0054E1E0` and `0x005A2120` (unique minions and
    modifier init) with the arguments of §3.1 rule 2, and whether
    calling them outside room population leaves region counts and game
    state as the population path would.
14. **Spawn: explicit umods.** For `unique`: the monster data field of
    the umod list and its count, to append the script's umods after the
    boss spawn and before `0x005A2120`, and anything else `0x005A0760`
    would have set. Confirm the calls draw only from game-owned seeds
    (original-hooks open question 9, `time_value`).
15. **Masks.** Every S→C builder that leaves message bytes unwritten or
    fills them from the clock (beyond `specs/tools/scenario-masks.tsv`),
    with its bytes.
16. **Stats stream.** The base array of an extended stat list (offsets,
    entry layout), for `stats` records.

## 5. d2rs side: what the runner stages or lacks

Written to each trace's `gaps` (so a comparison is at best `partial`
until they close):

- `char at default`: 5 sub-tiles right of and below the area's first
  waypoint object (the `synthetic_game.rs` staging; TODO(spec: unit
  placement)).
- Room presets: only waypoint objects are allocated (NPCs such as Akara
  and other objects are not), as the server tests stage them.
- `char item`: not created (TODO(spec: item creation from a code and
  inventory placement)).
- `char quest`: flags not applied (TODO(spec: quest flags of a joining
  character)).
- `char waypoint L` with no waypoint of level L in the data.
- `record frames`: no renderer in the runner.
- d2rs faults during a tick (`tick_faults`, `world.faults`, sim errors).

Spawn steps run on d2rs' own population functions
(`d2_sim::monsters::population::{placement, spawn}`,
`monsters::init::champion_pack_member`); a point in no active room or a
class the data does not have gives `failed`.

Errors (no trace): `char save` (TODO(spec: formats/d2s.md)); an area
other than the act 0 town (the runner creates act 0 only); `char at x y`
outside every room. Staging that mirrors sim state (no gap): the
dispatcher's unit facts (`SimGame::set_unit`) are set from each unit's
path position before every drain, as `d2-client/tests/e2e_walk.rs` does.
No `draw` records: d2rs seeds have no draw log (`rng-draws` not listed;
the comparator reports it not compared when the original has it).

## 6. For the local run queue (`docs/HANDOFF.md` §5)

1. `cargo test -p scenario-run -- --ignored` with `D2_GAME_DIR`: every
   starter builds on the live install and runs twice identically (the
   test prints records and gaps per script). Expect the gaps of §5;
   any error is a finding.
2. `cargo run -p scenario-run -- run traces/scenarios/waypoint-travel.scenario`
   (live): 0x13 and 0x49 dispatch results in the notes; the trace's
   s2c records at tick 50.

## 7. Next steps, in order

1. PC 2 (Local2): apply §3's changes to `run_scenario.py`, create the
   saves, run each starter twice on 1.14d (identical traces prove the
   seed overrides); PC 1: answer §4's open questions in a spec.
2. Rewrite the starters' relative targets with what the first original
   runs show (exit, fallen, Akara positions for these seeds).
3. Compare each starter with d2rs (`scenario-run compare`); every first
   divergence is a finding for its system spec (M01, M21).
4. d2rs runner: `char save` once `specs/formats/d2s.md` exists; items by
   code; NPC and object presets; start in any level; a draw log for
   `rng-draws` (a sim-side observer that keeps determinism).

## 8. Gate

`sh tools/gate.sh` (nextest, after `tools/cloud-setup.sh`): every step
passes except the tests already red on the base, owned by other
sessions (this branch changes no d2-sim or d2-client code):
`d2-sim monsters::ai::tests::implemented_matches_catalogue`,
`missiles::tests_bodies::bodies_match_catalogue_status`,
`missiles::tests_bodies::bodies_check_catches_perturbations`,
`d2-client scene::gaps_numbered_tests::world_is_skipped_in_open_mode_3_and_the_ui_still_drawn`.
No new failure.
