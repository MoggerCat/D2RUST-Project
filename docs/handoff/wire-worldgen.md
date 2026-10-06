# Handoff: wiring of the world-generation seams (`d2_sim::wiring::worldgen`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/wire-worldgen`, from `claude/bold-ptolemy-jvyvxy` at
`1470723` (cloud session, 2026-10-06). Task class: integration from
clear specs, medium (METHODS M14). Scope of every claim: this branch,
synthetic tables, synthetic DS1 files and the recorded Act I placement
vector, no game files (M09). For the coordinator to fold into
`docs/HANDOFF.md` and `docs/PLAN.md` (not edited here).

## 1. State

**Wired, unverified** (M02): every module behind these adapters is a
draft-spec implementation; the adapters add no rule of their own.

Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim -p
conformance --all-targets -- -D warnings`, `cargo test -p d2-sim -p
conformance` (d2-sim 840 pass, 5 ignored; `tick_replay` 7 pass, unchanged),
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`, `python3 tools/coverage.py --check` (2,457 claims,
0 errors). `cargo check -p d2-server` also builds.

12 integration tests in `crates/d2-sim/src/wiring/worldgen/tests/` run the
real modules together. Fixture: act 0's DRLG created through the
dispatcher with the Act I tables of `outdoor.md`'s recorded placement
(init seed 644409375, so `dwStartSeed` 4014346869); real `UnitLists`,
`StatLists`, AI store, timer queue; game seed `{1234, 666}`.

| Seam pair | Test | Spec vector / rule |
|---|---|---|
| DRLG ↔ outdoor placer ↔ preset init | `outdoor::act_placer_allocates_preset_levels_and_hands_over_their_directions` | `outdoor.md` vector through the dispatcher: DRLG seed {1406222081, 1674353446}, allocation order 4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5 unchanged; preset `roll(Files)` on the level seeds of 26 and 27, none for the town (Files 0); directions town 3, Outer Cloister 1 (`§2.3` step 4 overwriting `preset.md` §3.1) |
| outdoor → preset cell | `outdoor::outdoor_preset_cell_builds_real_preset_rooms` | `outdoor.md` §12.1: map at the cell origin, lvlprest size 16 × 16, default `roll(2)` then the cell's file, 4 rooms (one level-seed step each), room flags |
| maze → preset | `maze::den_of_evil_cells_build_real_preset_rooms` | `maze.md` Den of Evil vector: maps 57/86/96 at the normalized cells, F rotated to file 0, level seed = 4 cell steps + 31, 27 preset rooms of 8 × 8; one room streams with its DS1 |
| DRLG ↔ preset level | `levels::preset_level_generates_through_the_dispatcher` | `preset.md` §3.2, §6: 40 × 18 → 15 rooms, seed reset + map roll + 15 room steps, picked file = direction, no DS1 load (Scan/Pops 0) |
| | `levels::preset_room_streams_with_its_ds1_and_units` | §8 lazy DS1 load at status 3, §9 grids and unit transfer, `LevelTypes::preset_units` |
| | `levels::level_reset_frees_the_preset_maps_and_regenerates_the_same_rooms` | `levels.md` §9.4 / `preset.md` §3.3 keep = 1 |
| population → DRLG rooms | `population::population_queries_read_the_real_rooms` | room level, box, `0x00463740`, preset units (mode, done, data), floor records, collision, active-room seed |
| population → init | `population::preset_monster_is_created_through_monster_init` | §11.1/§11.2 preset spawn → §9.6 creation: one game-seed step, type init (flags 0x0A, AI control + install, level id), region count, preset flags 0x3000000 |
| | `population::room_population_creates_packs_through_monster_init` | §3 density / pick / pack with a pending coordinate list, every monster through init; deterministic |
| init → regions | `init::components_come_from_the_region_entry_on_the_unit_seed` | `init.md` §10 / `population.md` §2.5: components = the region entry's variant `roll(n)` on the unit seed |
| init → regions (boss count) | `init::champion_pack_member_counts_in_the_real_region` | `init.md` §16.2, §18 step 2 (umod 1 name seed), `0x005A0320` on the real region |
| end to end | `e2e::generated_level_is_populated_and_ticks_deterministically` | `tick::tick` × 10 through `WorldSim`: room pass populates both streamed rooms, the DS1 monster is created on tick 1, its think at 3 reschedules at 203; two runs identical |

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/wiring/worldgen/mod.rs` | `WorldgenError`, `WorldPending` (seams without a provider), `WorldTables`, `WorldState` (regions, monster data, minions, superunique tails), `WorldHost` (game + action `View` + world state) | |
| `…/worldgen/levels.rs` | `WorldTypes`: the act DRLGs' `LevelTypes`, dispatching by `DrlgType` to `Maze` / `Presets` / `Outdoor`; `AllocView` (type inits while a generator is busy); `SharedTypes` handle | `levels.md` §3.7, §4.3, §5.2, §9.4; `rooms.md` §4, §9.2 |
| `…/worldgen/maze_presets.rs` | `MazeToPreset`: `MazePresets` on the act's `Presets` | `maze.md` §7, §9; `preset.md` §4, §6 |
| `…/worldgen/outdoor_presets.rs` | `OutdoorToPreset`: `OutdoorPresets` on the act's `Presets` | `outdoor.md` §12.1 |
| `…/worldgen/population.rs` | `PopWorld` on `WorldHost` (seeds, rooms, collision, preset units) | `population.md` §3, §8, §9, §11 |
| `…/worldgen/population_init.rs` | `MonsterInit` on `WorldHost` (allocation + type init, normal / boss mods, umods, minion lists) | `population.md` §6, §9.6, §11.4; `init.md` §4–§5, §14, §16–§20 |
| `…/worldgen/init_units.rs` | `InitHost` on `WorldHost` (units, stats, AI control and install, regions, prepare animation) | `init.md` §5, §6, §10, §16, §18 |
| `…/worldgen/dispatch.rs` | `WorldSim`: `ActionSim` + `WorldState`; `EventDispatch` and every `TickHooks` method to the action systems except step 3's population hooks | `tick.md` §3, §4 |
| `…/worldgen/tests/` | fixture + integration tests (§1) | |

## 3. How it is put together

- `WorldSim { action: ActionSim<X>, world: WorldState }`, `X: WorldPending`
  (which extends the action `Pending`). The act DRLGs hold the level types
  as `DrlgWorld::types = Box<SharedTypes>` (`Rc<RefCell<WorldTypes>>`);
  `WorldState::types` is a clone, read by population for the preset rooms'
  full unit records (mode, flags, path), which the DRLG seam view
  (`drlg::PresetUnit`) lacks. Calls never nest through the handle: a
  generator allocates other levels through its own `AllocView`.
- Population state: `WorldHost::population` lends `WorldState::pop` to
  population's `Ctx` for the call. Seam calls that reach init code
  needing the regions (allocation → type init → `0x00547BC0`; modifier 16
  → `0x005A0320`) get the state passed back (`with_state` swaps it into
  place for the call).
- One creation path: population's `0x005B2A00` (`population.md` §9.6).
  `allocate_monster` = `View::allocate` (`units.md` §3.1, game seed) then
  `init::type_init` (the allocator's `init_kind` hook is its last step, so
  running the type init right after it returns is the same order; the
  action hooks' `init_kind` is not this module's to change). Extras
  `0x005B21B0` = `init::normal_mods`, `0x005B1CF0` = `init::boss_mods`.
- Superuniques: population §11.4 runs the spawn, the minions (init §20
  step 3) and the closing umod 22; the `superunique_init` seam runs init
  §20 steps 1–2 (`init::superunique_mods`) and records steps 4–5, which
  run right before that unit's modifier 22 (`init::superunique_finish`).
- Errors: `WorldTypes::errors` (level types, reported to the DRLG as
  `DrlgError::LevelType(id)`), `WorldState::errors`, plus the action
  errors; `WorldSim::errors()` lists them all; tests assert it empty.

## 4. Changes outside `wiring/worldgen` (smallest, each provably needed)

1. `wiring/mod.rs`: `pub mod worldgen;` (doc comment untouched).
2. `drlg::DrlgError::LevelType(u32)`: `LevelTypes` returns `DrlgError`,
   which had no way to report a maze / preset / outdoor error (requested
   by the `impl-drlg-maze`, `-preset`, `-outdoor` notes). The adapter keeps
   the typed error and returns this variant.
3. `drlg::LevelTypes::door_unit(…, orientation)`: `preset.md` §11 looks up
   the door table by the cell's tile type (9 = right door); the seam did
   not pass it (preset note "Door seam"). Call sites in `tiles.rs` pass
   the type they already hold; `OutdoorTypes` forwards it; the DRLG test
   fake takes it.
4. `drlg::Drlg::active_room_seed_mut`: population draws on the active-room
   seed (+0x6C, `population.md` §3.2, §9.3); the DRLG had no mutable
   access to it.
5. `monsters::population::MonsterInit::{allocate_monster, add_modifier}`
   take `state: &mut PopState`: the provider (init's type init
   `0x00547BC0`, `init.md` §5 step 4 / `population.md` §2.5; champion mark
   `0x005A0320`, `init.md` §16.2) reads and writes the game's regions,
   which population holds in its `Ctx`. Three call sites pass `cx.state`;
   the test fake ignores it.
6. `monsters::init::superunique_init` split into `superunique_mods` (§20
   steps 1–2) and `superunique_finish` (steps 4–5 without umod 22);
   `superunique_init` composes them as before (same behaviour, its tests
   unchanged). Needed because population §11.4 owns step 3 and the
   closing 22 and calls the seam before them; the whole function would
   spawn the minions twice.

## 5. Remaining seams (pending) and why

`WorldPending` (defaults = nothing):

- **DRLG data population reads, not in any DRLG spec**: coordinate lists
  `0x0061AD50` / `0x0061AD30` / `0x0061B130` (D2MOO
  `D2RoomCoordListStrc`; `RoomGrids` has no logicals either), populated
  level `0x0061A1F0`, populated-room count `0x0061ABF0`, warp points
  `0x0061AC10`, the level spawn location of kind 11, nearest free point
  `0x0064E840`. **Consequence: with the defaults no room population (§3)
  runs; only presets place monsters.** Tests supply a coordinate list.
- **Quests**: quest flags, Chaos state, boss quest hook.
- **Objects / units restore**: object creation, barricade objects, object
  population `0x00552610`, inactive restore `0x00542B40`, preset-created
  hook `0x0058F000` / `0x00666120`.
- **Monster pieces without a provider**: coordinate record `0x00552D60`,
  alignment `0x005543B0`, monster flags other than 2, owner data
  `0x0058F030` (AI spec), group spawn `0x005B24E0` (population OQ4), event
  7 scheduling `0x005417D0` (seed not stated), `0x005B1990`.

`InitHost` methods left at their defaults: the items / vendors calls
(inventory, equip items, properties), skills (give skill, aura), quests
(chain, quest records), `set_corpse_noselect`, `set_ai_flag`,
`run_ai_tick`, `umod34_gate`, `montype_is` (montype nesting), and
`set_combat_mode` (`0x00553570` for a dead monster: mode not stated).
The creation seams (`place`, `allocate`, `register_spawn`,
`party_minions`, `boss_spawn`, `spawn_boss_minion`, `spawn_with_guid`)
stay default because no init entry point that creates is called here
(see §3).

`LevelTypes::warp_unit` (`0x0066E1C0`) and the type-2 tile free
`0x00666610`: not specified, nothing done.

### Follow-ups that need an edit in `wiring/action` (not this session's files)

- `ActionHooks`'s `LifecycleHooks::init_kind` is still the default: a
  monster allocated by other action code (missile/skill spawns) gets no
  type init. Route it to `init::type_init` once the world state is
  reachable from `ActionHooks` (e.g. move `WorldState` into it).
- Monster event 7 (`UnitHooks::monster_umod`) → `init::handle_event7`
  and the umod callbacks of `init.md` §22 (mode change, combat, missile
  creation) are not routed: their hooks are `ActionHooks`'s.
- `MonsterStore::remove` and the minion / owner maps on unit removal
  (`ActionHooks::free_kind`).
- Action `Pending` monster-data queries (`monster_flag` `0x005A0180`,
  monster level, AI state …) now have a provider (`WorldState::monsters`)
  but are still answered by `Pending`.

## 6. Open questions (each has a `TODO` at its site)

- WG1. `maze.md` §9 step 3 / `preset.md` §6: the room flags F of the
  maze's build call are not stated; 0 used.
- WG2. `maze.md` §9 step 4 (maze OQ4): DRLG rooms have no orth links; the
  cells' init-flag links are not carried to the built rooms (no draw
  depends on them).
- WG3. `outdoor.md` §12.1: the single-room argument of the outdoor
  preset-cell build is not stated; multi-room used.
- WG4. `preset.md` §11: door record flag 0x20 (`DoorOutcome::Placed`) is
  the tile code's; the seam returns nothing, so it is not set. Today's
  call sites create new records (flag clear) or none.
- WG5. `population.md` §11.4 step 6 vs `init.md` §20 steps 4–5: the order
  of the hcIdx extra spawns and the aura re-run / quest records is not
  stated; the spawns run first.
- WG6. `population.md` §9.2: which DT1 header field `0x00604BC0` reads is
  not stated (`rooms.md` §9.3 lists accessors and fields in one
  sentence); no floor record counts as water, so frog demons find no
  water point (the `roll(n)` on the room seed is still drawn).
- WG7. `0x0064D9B0` footprint by size (wire-action W5): one sub-tile read.
- WG8. The first think of a created monster: nothing in init or AI
  schedules it (`ai::install` does not; `units.md` §4.6 schedules it on a
  mode change, which creation does not call); the e2e test schedules it.
  Which creation step starts the monster's mode is not specified.
- WG9. Outdoor level generation through the dispatcher is not exercised:
  generating an Act I outdoor level needs lvlsub rows and substitution
  DS1s (`outdoor-tilesub.md`); streaming a chain room generates its
  outdoor neighbours (`NoSubRows(0)` without them), so the population
  tests use a preset level (id 30) placed apart from the chain.

## 7. Checks to queue (local, `docs/HANDOFF.md` §5)

1. With game files (a crate holding both `d2-formats` and `d2-sim`, e.g.
   an ignored `conformance` test): build `WorldTypes` from the live
   tables (`DrlgData`, `PresetData`, `OutdoorData`, `MazeData`, DS1s from
   `d2_formats::ds1`, lvlsub DS1s), create act 0 with init seed 644409375
   and compare `20261005-232125-rng.jsonl`: the preset draws at
   `0x0066749F` (seq 2433–2453), the town generation (seq 2455–2559), Blood
   Moor (seq 2561–6331) — the queued checks of the preset and outdoor
   notes, now runnable through one dispatcher.
2. Recording replay of a first room population (population note check 2)
   through `WorldSim` once the DRLG coordinate lists are specified.

## 8. Coordination

- `wiring/mod.rs`: one `pub mod worldgen;` line; the `interaction`
  session adds its own line: trivial merge.
- The population and init test fakes changed only their signatures.
