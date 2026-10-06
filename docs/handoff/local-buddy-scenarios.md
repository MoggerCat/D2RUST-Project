# Handoff: starter scenarios on 1.14d vs d2rs (lane T, `claude/local-buddy-scenarios`)

Local run, 2026-10-07. Original side: `tools/trace-recorder/run_scenario.py`
(now reads `.scenario` and writes `scenario-trace` 1 to
`traces/raw/<name>.original.trace.jsonl`, never committed). d2rs side:
`scenario-run run <script>` on the live install. Comparison:
`scenario-run compare <original> <d2rs>`. Probe results:
`local-buddy-scenario-probes.md`.

## What was added

- `scenario-run export <script>`: the parsed script as JSON (header facts,
  canonical SHA-256, snapshot ticks, typed messages as layout fields with
  structured references). `run_scenario.py` uses it instead of a second parser
  (handoff scenario-harness §3 change 1) and keeps the reference text as the
  trace's `unresolved` string.
- `run_scenario.py` with a `.scenario`: relative ticks (F0 = first frame whose
  client state is 4; step t injected at the first stop after frame F0 + t,
  records of tick t after frame F0 + 1 + t; scenario.md §4 rule 2, original-hooks
  §1, §3); `c2s` per injected message; `s2c` of client 0 between that stop and
  the tick's return; `rng` = game +0xD0 at the stop and at the return; `unit`
  records at snapshot ticks (life/mana 0 when absent); `end`. `--save-as NAME`
  loads another save than the script's `char save` (listed in the header `gaps`);
  `--trace-out`. Not done (listed as gaps): `stats` stream, `rng-draws`,
  `spawn` steps (written as `failed`), `@wp` references (written `unresolved`:
  the objects table is not read).
- Determinism: `walk-town` run twice on the original gives byte-identical
  traces (sha256 equal), so the seed overrides and the injection are stable.

## Saves (deviation)

`ScnSor`, `ScnAma`, `ScnSorNinety` do not exist on this PC (the save-creation
worker has not produced them). Stand-ins, read-only and `-nosave`:
`bdSor` for ScnSor (sorceress) and `bdAma` for ScnAma (amazon). They are level 1
characters in Normal; kill-monster's script says amazon level 5, the other scripts
say nothing about stats beyond `char skill 36 1` (cast-firebolt). Each original
trace lists this in `gaps`. `champion-pack` (level 90, Hell) needs the real save:
blocked (below).

## Results

Every comparison diverges at the very first record: the game seed at tick 0.
To see past it, `out-scenarios\norng\` holds both traces with the `rng` records
removed (analysis copies, scripts and expected values untouched); that run
diverges at the first `unit` record in every scenario.

| Scenario | Save used | Original | d2rs | First divergence (full traces) | First divergence without `rng` |
|---|---|---|---|---|---|
| walk-town | bdSor | ran, 120 ticks, 3 injected, 492 records | ran | tick 0, rng[0], before: expected `[148302771, 253542080]`, got `[2714123707, 838425138]` | tick 0, unit[0] (player), mode: expected 5, got 1 (also x,y: original 5473,4708, d2rs 4889,5634; life 10240 and mana 8960 equal) |
| run-cold-plains | bdSor | ran, 250 ticks, 803 records | ran | same as walk-town | same as walk-town |
| kill-monster | bdAma | ran, 400 ticks, 1186 records | ran | tick 0, rng[0], before: expected `[1767830925, 304583884]`, got `[2714123707, 838425138]` | tick 0, unit[0], mode 5 vs 1 (life 12800, mana 3840 equal) |
| pickup-drop | bdAma | ran, 450 ticks, 1327 records | ran | same as kill-monster | same |
| vendor-buy-sell | bdSor | ran, 200 ticks, 527 records | ran | as walk-town (rng) | as walk-town |
| waypoint-travel | bdSor | ran, 150 ticks, 403 records; both steps `@wp` unresolved (tool limit) | ran | as walk-town (rng) | as walk-town; partial: the original side injected nothing |
| cast-firebolt | bdSor | ran, 300 ticks, 936 records | ran | as walk-town (rng) | as walk-town |
| champion-pack | needs ScnSorNinety (level 90, Hell) | BLOCKED: with bdSor, Hell is not reached; the game never gets past tick 1 (client never in state 4, killed at the 300 s limit, no trace) | ran | compare: error, original trace has no `end` | — |

## Findings (each is a divergence or a fact the specs should state)

1. **Game seed at tick 0** (`sim/rng.md` §5.2, M01). The original's game seed
   at the first stop after F0 is `{T, 666}` stepped 44 times for the sorceress
   save and 46 times for the amazon save (found by stepping `x → x·0x6AC690C5 + hi`
   from `{0x1234, 666}`; the step at `0x0052C2C6` is one of them). d2rs has
   stepped it 3 times (`[2714123707, 838425138]`). So game creation and the
   player join take class-dependent draws from the game seed that d2rs does not
   make (or makes elsewhere). Afterwards the seed changes only in a few ticks
   in town: walk-town tick 20 (10 steps), run-cold-plains ticks 20 (10) and
   133, kill-monster / pickup-drop tick 14 (10 steps), cast-firebolt ticks 32
   (10) and 143, vendor-buy-sell and waypoint-travel none; never between ticks.
2. **Player mode in town** (`sim/units.md`). The original's player is in mode 5
   (town neutral) at tick 0 and stays 5 while walking or running in town; d2rs
   starts with mode 1.
3. **Start position** (scenario.md open question 1, `drlg/levels.md` §10). The
   original (sorceress and amazon, Rogue Encampment, Normal, seed 0x1234,
   init 644409375) places the player at sub-tile (5473, 4708) at tick 0; d2rs stages
   (4889, 5634). Records for the rewrite of the starters (§7 step 2 of the
   scenario-harness handoff): run-cold-plains ends at (5538, 4723), cast-firebolt
   at (5532, 4721), kill-monster / pickup-drop at (5492, 4709).
4. **Units present** (kill-monster, tick 0, original): player, 7 monsters
   (classes 147, 150, 152, 154, 155 among them), 17 objects, 7 items (type 4);
   no fallen (class 19) appears after running 30+10+10 sub-tiles, so the five
   `LeftSkillOnUnit` steps are `unresolved` on both sides. Akara (monster 148)
   is not among the town units near the start position in vendor-buy-sell: all
   seven of its steps are `unresolved` in the original; d2rs resolves `@4` /
   `@4#1` items instead (its steps at ticks 100 and 130 are unresolved, the
   others are not): another c2s divergence after the rng one.
5. **S→C volume** (not compared, hidden behind finding 1): the original sends
   25–83 messages to client 0 per run (ids 0x07, 0x08, 0x0a, 0xac, 0xaa, 0x6d,
   0x51, 0x0e, 0x8a, 0x67, 0x2c, 0x8f ...), d2rs none (walk events are not wired to
   messages). Waypoint-travel on d2rs sends 0x07 and 0x0D; the original sent
   nothing because `@wp` is unresolved.
6. **Hell on a save that has not reached it** (original-hooks open question 6):
   the client does not reach state 4; ticks stop after frame 1 until the tool's
   300 s limit.
7. Same seed T gives different starting seeds for different saves/classes
   (finding 1), so the comparison must start with the character the script names.

## To unblock

- Create `ScnSor`, `ScnAma`, `ScnSorNinety` (scenario-harness §3 change 7) and
  rerun with `run_scenario.py traces/scenarios/<name>.scenario` (no
  `--save-as`).
- Resolve `@wp` (read the objects table, operate function 23).
- Spawn steps, the `stats` stream and a draw log (`rng-draws`) are still
  gaps on the original side.
- After the rng finding is understood, rerun: the next divergences are the
  start position and mode (findings 2, 3).
