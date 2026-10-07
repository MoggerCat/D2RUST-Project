# Handoff: seed finder for differential-testing scenarios (branch `claude/seed-finder`, 2026-10-06)

Cloud tooling session, repo only (no `game/`), medium effort, from
`claude/tender-meitner-mphas3` at `f6cadb9`. Read: `specs/`, `docs/`,
`crates/`, `tools/`. The diff adds the workspace member
`tools/seed-finder` and this note. It changes no library code, spec,
`HANDOFF.md` or `PLAN.md`. For the coordinator to fold.

## 1. What it is

`tools/seed-finder` (binary + library): searches seeds for a wanted level
and monster situation through the same code the wired host uses, so a
scenario can then run that seed on the original and on d2rs.

Per searched value `s` (`world.rs`, `lib.rs`):

1. Map seed (`rng.md` §5.4, scenario `init_seed`) and game seed value
   (`rng.md` §5.2, scenario `seed`): both `s` by default, as
   `run_scenario.py` defaults `init_seed` to `seed`; `--init-seed` /
   `--game-seed` fix one and search the other.
2. A `WorldSim` built as `test_fixtures::game::GameData::world_sim` does
   (tables decoded once in `Prepared`, DRLG files shared by `Arc`), for
   the level's act and the query's difficulty: `Drlg::create` on the map
   seed with the act placer and the act's town (`levels.md` §3), the
   action hooks on `init_low(game seed)` with AnimData, vitals and paths,
   the regions created (`population.md` §2.1).
3. The level allocated and generated (`levels.md` §4–§5), every room of
   it streamed in level-list order (`rooms.md` §4.5), then one
   `d2_sim::tick::tick`: its room pass (`tick.md` §4) runs presets,
   objects and monster population on every new active room.
4. Read back: monsters of the level (monster data class, type flags →
   kind, umods, superunique row; path position in sub-tiles), preset
   units (absolute sub-tiles), anchors: **entrance** = centre of the
   first room with a warp to `--from` (or with any warp whose id ≠ −1,
   `levels.md` §10 step 3); without one and with `--from`, the first room
   touching that level's rectangle (outdoor borders such as Blood Moor ↔
   Rogue Encampment); **waypoint** = the waypoint preset of the waypoint
   room (`levels.md` §10.4).
5. Any DRLG, level-type, population, init or wiring error (`WorldSim::errors`)
   → "unsupported level" with the reason; the search stops at that seed
   (exit 2). Never a guess.

Query (flags; `seed-finder --help`): `--level`, `--difficulty`,
`--monster class=<row|Id>,kind=<normal|minion|champion|unique|superunique>,su=<row>,umod=<a>+<b>,count=<n>`
(repeatable; each want is checked on its own, one monster may satisfy
two wants), `--near entrance:<tiles>|waypoint:<tiles>` (Euclidean, integer
sub-tile math), `--from`, `--preset <type>:<class>`, `--seeds a-b`,
`--limit n`, `--threads n`, `--town-only` (fixture act creation).
`Id` names resolve only when the install has `monstats.txt` with the
same row count as the loaded table; otherwise use row numbers.

Determinism: std threads over chunks of 16 seeds, results sorted by
chunk; `--limit` keeps the lowest matching seeds (whole chunks finish
before the limit applies), so output never depends on the thread count
(test `same_seed_same_result_twice`).

Output per match: `seed S (init_seed I, game seed G): level L, R rooms,
M monsters, entrance (x, y), waypoint (x, y)` then each want's monsters
(`unit, kind, class, position, superunique, umods, distance`).

Scenario link (`origin/claude/tool-run-scenario`, `run_scenario.py`
phase 2): a hit's `game seed` is the scenario `seed` (time value, must be
< 2^31 there) and `init_seed` its `init_seed`. No shared Rust types
exist on that branch (Python + JSON), so the API stays self-contained.

## 2. Findings / limits (read before trusting a hit)

- **F1 (closed by `impl-room-population`, `levels.md` §11: the live host now populates rooms and answers champion / unique queries; text kept for history.) Room population does not run on the live host.** Population's
  DRLG reads (coordinate lists `0x0061AD50` / `0x0061AD30` /
  `0x0061B130`, populated level `0x0061A1F0`, populated-room count
  `0x0061ABF0`) are in no DRLG spec (`HANDOFF.md` 3l, GH1); with the
  `WorldPending` defaults only presets place monsters. The tool refuses
  `kind=champion` and `kind=unique` queries on that host ("unsupported
  query", exit 2) and prints a note on every run. So **"Den of Evil /
  Blood Moor seeds with a champion pack near the entrance" cannot be
  answered yet**; preset and superunique queries can (Corpsefire in the
  Den of Evil). The `FinderHost` trait is the switch: a host providing
  the coordinate lists sets `ROOM_POPULATION` and learns the streamed
  rooms (`rooms_streamed`). Once a DRLG spec owns those reads, wire them
  into `WorldPending` and the live `Seams` gets them for free.
- **F2 Activation order.** Population draws from the game seed
  (density) and the room seeds; the game seed is shared, so a level's
  population depends on the order rooms are populated and on every
  game-seed draw before it. The tool populates all rooms of the level in
  level-list order in one room pass right after game creation. A
  scenario reproduces a hit only if the original populates the same rooms
  in the same order with the same game-seed state; in a real run the
  player activates rooms as it walks, and join, town and item draws come
  first. Treat a hit as a *candidate* until the scenario's own d2rs run
  shows the same units.
- **F3 Game creation draws.** `WorldSim::create_regions` is the only
  creation step of `rng.md` §5.2 the host runs (no object-control, NPC
  or quest seed derivation yet), as in the wired host test; the game seed
  after creation therefore differs from the original's. A wiring gap,
  not a tool choice; the tool follows the host.
- **F4 Acts II–V.** Built through the same path; a missing piece shows as
  "unsupported level" with the error, never as a guess.

## 3. Tests (CI, synthetic install)

`tools/seed-finder/tests/synthetic.rs` (7 tests), `src/query.rs` (5 unit
tests). The synthetic field (level 2, one preset room) is searched with a
test host whose coordinate list is one record per streamed room (the
fake the worldgen tests use) and two table tweaks (`Prepared::tweak_world`:
the two synthetic monsters `isSpawn`, field `MonUMax` 2), so uniques with
minions appear by seed:

- known seed: `--level 2 --from 1 --seeds 1-100 --monster kind=unique
  --near entrance:2` → only seed 88 (unique #1 at (11, 22), 1 tile from
  (20, 20)); without `--near`, seeds 16, 29, 45, 77, 88;
- perturbations (M08): radius 1, fixed map seed, fixed game seed,
  count 2 each lose seed 88;
- same seed twice → identical view (seeds 1–40); 1 vs 8 threads →
  identical outcome; `--limit 2` → 16, 29;
- level 3 (synthetic maze; `LevelType(3)` on this data) → unsupported at
  the first seed; level 99 → no levels.txt row;
- the live `Seams` host refuses champion / unique queries and finds no
  monster in the field;
- the binary on a synthetic install (`D2_GAME_DIR`, `--town-only`): exit
  0 / 2 / 1 and the printed lines.

## 4. Local run queue (for `HANDOFF.md` §5; M02)

1. `cargo test -p seed-finder --release -- --ignored` with `D2_GAME_DIR`:
   `den_of_evil_and_blood_moor_build_on_live_tables` builds the Den of
   Evil (8, `--from 2`) and the Blood Moor (2, `--from 1`) for seeds 1–8
   with no unsupported step, twice identical. Look for: pass; the printed
   lines show the Den's room count varying by seed and an entrance on
   every line (a missing entrance on the Blood Moor means no room touches
   the Rogue Encampment's rectangle: report the rects).
2. `cargo run -p seed-finder --release -- --level 8 --from 2 --seeds 1-200 --monster kind=superunique --near entrance:30`
   → seeds whose Corpsefire stands within 30 tiles of the Den entrance
   (record the first five lines and the timing).
3. `cargo run -p seed-finder --release -- --level 2 --from 1 --monster kind=champion,count=2 --near entrance:20`
   → today "unsupported query" (F1), exit 2. Rerun when the coordinate
   lists are specified and wired: then it prints the seeds with a
   champion pack near the Blood Moor entrance; take one into a scenario
   and compare the original's units (F2).

## 5. Next

- Specify the DRLG population reads (F1) in `specs/drlg/` (spec session,
  local), wire them into `WorldPending`, drop the refusal.
- Once `run_scenario.py` records unit snapshots, add `--emit-scenario`
  (a scenario JSON with `seed`, `init_seed`, `difficulty`) and a local
  check that the original's first room-pass units equal the hit's.
