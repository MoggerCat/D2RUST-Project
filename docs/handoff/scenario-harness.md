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

All starters use seed `0x1234` and map 644409375 (the test character's
map ID, `sim/rng.md` §5.4). Targets are relative to the character
(`@x+30`): the town layout depends on the map seed, so the first
original run of each script shows where the exit, the fallen and Akara
are; then the targets are rewritten (§7 step 2).

## 3. The original-side runner (`run_scenario.py`): what it must do

A Windows tool in `tools/trace-recorder/` (Python 3.10, standard
library, the debugger code of `record_packets.py` / `record_tick.py`;
reference-hash check and kill guarantees as `record_rng.py`). Spec-role
facts it needs are the questions in §4; it must not be written before
they are answered in a spec.

```
py tools/trace-recorder/run_scenario.py traces/scenarios/<name>.scenario [--out FILE] [--game PATH] [-- Game.exe args]
py tools/trace-recorder/run_scenario.py --selftest
```

1. **Read the script** with the rules of `specs/tools/scenario.md` §2–§3
   (strict: any error stops the tool before the game starts). The
   canonical text and its SHA-256 must equal d2rs': port the writer's
   rules exactly (§2 rule 6) and check with `--selftest` against
   `cargo run -p scenario-run -- check` output for every starter (the
   hash is in each d2rs trace header).
2. **Start the game** (`Game.exe -w -ns`), create a single-player game
   with `difficulty` and `expansion`, forcing the game seed to
   `init_low(seed)` and the map ID to `map` (Q1, Q2), with the
   character of the script (Q3).
3. **Tick 0** is the first server tick after the character's client is
   in game (Q5). For each tick t = 0 … `end` (`scenario.md` §4 rule 2):
   at the drain entry of frame t, resolve the steps of tick t against
   the game state (Q8; references `scenario.md` §3) and, in order,
   inject each message through the net send (Q4) or run each spawn
   step's call sequence (`scenario.md` §3.1 rule 2; Q12–Q14); record
   what the rules say.
4. **Write the trace** exactly as `traces/FORMAT.md` §Scenario traces:
   header `side: "original"`, `data: "1.14d"`, `tool: "run_scenario.py
   <version>"`, `streams` = what it produced (`rng-draws` if it logs the
   game seed's single draws, which `record_rng.py` can), `gaps` = what it
   could not apply; records in tick order; `json.dumps(o,
   sort_keys=True, separators=(",", ":"))`, one per line, LF; the `end`
   record last. A run that cannot finish writes no `end` record (the
   comparator then refuses the trace).
5. **Records** (`scenario.md` §4 rules 4–8): `c2s` per message step
   (bytes injected or the reference text); `spawn` per spawn step (the
   first call's unit GUID, `failed`, or the reference text); `s2c` for every message queued for
   client 0 from the drain entry of frame t to the end of tick t (Q6);
   `rng` = game seed (game +0xD0) before the first step of tick t (the
   drain entry) and at the tick's end (Q7); `draw` per game-seed draw in that window with `site` = the
   call site `"0x…"` (optional); `unit` and `stats` at the snapshot
   ticks after the tick (Q8, Q9).
6. **Self-test** (`--selftest`, M08): parse every starter and compare
   its hash with the d2rs trace headers given on the command line; write
   a synthetic trace, read it back with the same strict rules, perturb
   each record and check `scenario-run compare` reports that record.
7. **Acceptance:** the same script run twice on 1.14d gives
   byte-identical traces (`scenario-run compare a b` → no divergence);
   that proves the seed and map overrides took. Then compare with d2rs.

## 4. Questions for the PC 1 spec writer

Answers go into specs (a new `specs/tools/scenario-original.md`, or the
owning system specs), with 1.14d addresses, calling conventions and the
evidence, never as code. Each question says what the answer must state.

1. **Game seed override.** `0x0052C280` sets game +0xD0 = `init()`,
   then `init_low(time_value(QPC low))` (`sim/rng.md` §5.2). State where
   a debugger can replace the `init_low` argument with the script's
   `seed` before the first derived draw (`0x0052C2C6`), or whether the
   fixed-seed global `0x00731004` (≠ −1, "would replace this") can be
   written before game creation and what value form it takes (the
   `init_low` argument or the whole seed).
2. **Map ID.** Where the joining character's map ID (`.d2s` 0xAB, §5.4)
   is read and passed to the DRLG seed (`0x00642DA0`), and the point to
   override it with `map` for every act.
3. **Inline character.** How to start a game with the character of a
   script without a save from the user: (a) the `.d2s` layout needed to
   write one (header, stats section, skills, items by code at a grid
   position / belt slot / body location, waypoints, map ID) as
   `specs/formats/d2s.md` (d2rs needs the same spec for `char save`),
   or (b) the in-memory route after join: the functions and conventions
   to set a base stat (stat list set; level 99 and its experience), give
   hard skill points per skill id, create an item by code with a
   quality, item level, given magic prefixes / suffixes (rows), a unique,
   set or runeword id and a socket count, place it (grid cell, belt
   slot, body location) and put socket fillers into it, set a waypoint
   bit and the quest flags of a quest index on the game's difficulty.
   Say which route the runner should use and why. The full late-game
   state must be reachable (the `champion-pack` starter uses it).
4. **C→S injection.** The net send `0x0052AE50(size, 1, message)` in
   local mode (`sim/intents-events.md` §2.1 rule 3): its calling
   convention (registers, stack, cleanup, return), the thread it runs on
   in single player, and whether a debugger may call it (or the queue
   append `0x006BF370(net, message, size, client 0)`) on the game thread
   stopped at the drain entry `0x0052CFE0` of frame t so the message is
   drained in that same drain. Which memory the message buffer must live
   in (it is copied by the queue append?).
5. **Tick 0.** How to detect the first server tick after client 0's
   state (client record +0x04, `sim/tick.md` §6) becomes 4 (in game):
   the client record of client 0, the game pointer, and the frame value
   (game +0xA8) at that tick.
6. **S→C capture.** `0x0053B280` (EDI = client record, [ESP+4] message,
   [ESP+8] size, `intents-events.md` §3.2 rule 1) is already hooked by
   `record_packets.py`; state the client record field that holds the
   client id, and how direct sends (§3.3 rule 5) are seen (hook and
   position relative to queued messages).
7. **Game seed reads.** The game +0xD0 read points: at the drain entry
   and after the tick (`record_packets.py` `tick_end` hooks
   `0x0052FD1E`, `0x00564608`); confirm both are the end of tick t for
   single player.
8. **Unit lists and references.** How to walk every unit of the game
   (per type hash lists, `sim/unit-order.md`) from the game pointer; the
   unit fields: type, class, GUID, mode, act; the `objects` row's
   operate function of an object (for `@wp`); the player unit of
   client 0.
9. **Snapshot fields.** Position: the path's sub-tile x, y
   (`sim/path-placement.md` §2.1) for dynamic paths and the static
   path's for objects and items (field offsets); life and mana: stats 6
   and 8 layer 0 as the unit's stat value (function and convention, or
   the list walk of `sim/stat-lists.md`); a player's base stat entries
   (list layout).
10. **Default start.** Where 1.14d places a character entering a level
    (`drlg/levels.md` §10), so `char at default` means the same on both
    sides; and how to place a character at `char at <x> <y>`.
11. **Game type** +0x6A of a single-player game (d2rs creates 0;
    `GameFields::game_type` notes 3).
12. **Spawn: placement.** `0x005B2A00` (`monsters/population.md`
    placement): calling convention, the arguments `scenario.md` §3.1
    rule 2 fixes (room, no coordinate list, class, mode 1, x, y, radius
    −1, flags 0), the return (unit or null), and the game pointer and
    thread it must run on (the game thread stopped at the drain entry).
    How to find the active room that holds a sub-tile point.
13. **Spawn: bosses.** The conventions of the random boss `0x005A43E0`,
    boss spawn `0x005A09E0`, champion pack member `0x005A48C0`,
    champion minions `0x0054E1E0` and `0x005A2120` (unique minions and
    modifier init), with the argument values of §3.1 rule 2, and
    whether calling them outside room population leaves the region
    counts and the game in the state the population path would.
14. **Spawn: explicit umods.** For `unique`: the monster data field
    that holds the umod list (and its count), so the runner can append
    the script's umods after the boss spawn and before `0x005A2120`,
    and whether anything else (`0x005A0760`'s flag writes) must be set
    by hand when the choose step is skipped. Every draw these calls
    make is from seeds the game owns (unit seeds, the game seed), so
    the `rng` and `draw` records cover them; confirm no clock value
    enters (`sim/rng.md` §5.1).
15. **Masks.** Every S→C builder that leaves message bytes unwritten or
    fills them from the clock (beyond the four rows of
    `specs/tools/scenario-masks.tsv`), each with its bytes, so the mask
    table is complete before the first comparison.

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

1. PC 1: answer §4 in a spec; write `run_scenario.py` (§3); run each
   starter twice on 1.14d (acceptance §3 step 7).
2. Rewrite the starters' relative targets with what the first original
   runs show (exit, fallen, Akara positions for map 644409375).
3. Compare each starter with d2rs (`scenario-run compare`); every first
   divergence is a finding for its system spec (M01, M21).
4. d2rs runner: `char save` once `specs/formats/d2s.md` exists; items by
   code; NPC and object presets; start in any level; a draw log for
   `rng-draws` (a sim-side observer that keeps determinism).

## 8. Gate

See the commit message of this branch's last commit for the gate result.
