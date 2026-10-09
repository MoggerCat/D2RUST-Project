# q-fixture-migrate: Wave 1, the play path on the real install

Task `q-fixture-migrate`, branch `claude/q-fixture-migrate` (from `claude/specs-staging-7`).
Plan: `docs/handoff/q-fixture-inventory.md` §2 Wave 1 (rows CS01, CS02, CS11, CS13 of
`fixture-inventory.tsv`). Data: the private repo's `install/` (6a5b2aa7) assembled to
`$HOME/game` with `tools/cloud-game/fetch.sh`; every run below is on that 1.14d install
(`D2_GAME_DIR=$HOME/game`). M23 throughout: no expected value was typed in; each one names
the real table or trace it comes from.

## 1. What changed

| Part | Before | After |
|---|---|---|
| `GameData` | `Synthetic` or `Live`; `select(None, _)` silently played invented data | `Live` only; `select(None)` is `BuildError::NoGameDir` ("set D2_GAME_DIR"); `--synthetic` flag gone |
| Invented play world | 11 `app/synthetic_*.rs`, `town_npcs.rs`, `merc_rows.rs`, the synthetic half of `single_player.rs` (flat rooms, invented chains, NPC rows, vitals, charstats, vendors) | deleted (about 2,900 lines); the real ids those modules carried (Duriel's Lair 73, Duriel 211, Izual 256 / chain 22, Blood Raven 267 / chain 2, Kurast Docks 75, Pandemonium Fortress 103, stash 267) are named constants in `single_player.rs` |
| Build-time units | the build allocated a waypoint in the town's first room at an invented offset (and, synthetic, NPCs, chests, stash, the Moldy Tome) | none; the towns' units are their DS1 presets, placed by the room population after the join (`LocalGame`/`Started` lose `waypoint`, `waypoint_guid`) |
| Client wiring | `play::run` wired the client inline; tests re-wired it by hand with invented art | `play::add_live_client` (one function, used by `run` and by `play_smoke`), so the headless smoke runs the client `d2-client play` runs |
| Tests | 209 client tests on `GameData::Synthetic` (CI) | 139 marked `#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]` (exactly the ones that panic on the missing install), read the install through `app_support::game_data()`; 76 tests in 25 files whose subject was the invented world removed and listed for rewrite (`q-fixture-migrate-removed.tsv`); `data_selection_needs_the_game_files` stays in CI |
| Shared rig | `app_support::synthetic_skill_rows`, `single_player::synthetic_unit_rows` | `app_support::{game_data, live, live_tables, warp_id}`: the install, its client tables, and the `levels` Vis/Warp lookup that replaces the invented `BLOOD_MOOR_TO_DEN` / `DEN_TO_BLOOD_MOOR` |
| `test_fixtures::host` | greedy border walk | `route` (BFS over the server's active collision, move mask 0x1C09) and `Session::walk_route`, used by `play_smoke` and `act5_play` |

CI stays data-free: `cargo nextest run -p d2-client` without `D2_GAME_DIR` runs 2,034 tests,
all pass, 172 ignored (139 new real-data marks + the existing ones).

## 2. Fixes made on the way (each found on the real install)

| # | Break on the real install | Fix | Where |
|---|---|---|---|
| R1 | The Rogue Encampment had no NPCs: the play build gave the DRLG one `WorldTypes` and the world state a second, fresh one, so population's preset lookup (`act_presets`) never saw a generated room | one `SharedTypes` for both (its documented use) | `single_player.rs` `LevelSource::shared` |
| R2 | No preset object (stash, waypoint, fires, chests) was ever created: the tick's `populate_objects` called the `WorldPending` default (no-op) instead of the object state, and no code ran the object half of `0x005559A0` | the tick runs `MonsterInit::populate_objects` (object state first); **PROVISIONAL** `View::spawn_preset_objects` (every type-2 preset, mode 0, list order) before the monster pass | `d2-sim` `wiring/worldgen/dispatch.rs`, `wiring/action/warp_tile.rs` |
| R3 | The real town waypoint stayed in mode 0, so the first click only animated it and no 0x63 menu came: init function 17 (`waypoints.md` §5.1) was implemented but never called; the route fell to `Pending::object_route` (no-op) | init 17 runs inside the object's creation on `ActionHooks::waypoint_init` with the arrival list now owned by the action hooks (the host's 0x49 borrows it) | `d2-sim` `wiring/action/{mod,objects}.rs`, `d2-server` `handlers/world/action.rs` |
| R4 | The first frame in town panicked: real floor tiles carry 32-row RLE blocks, `shading.md` §4 r3 defines 15 gradient rows, the composer refuses the draw (`GradientArea`) | **PROVISIONAL**: a floor block taller than 15 rows takes the flat branch's map `e[g+9] >> 3` | `world_view/preview_blocks.rs` (+ test `a_32_row_floor_block_takes_the_flat_map`) |
| R5 | Walking out of the first room was fatal 0x591 (`lighting.md` §6.4): the client unit free (S→C 0x0A) left the fire's kind-2 light behind | **PROVISIONAL**: `ClientWorld::remove` removes the unit's light (`0x00464930`, §6.2 r5) | `bridge/world.rs` |

## 3. `play_smoke` on the real install (all `#[ignore]`, real-data gate)

| Test | Result | Steps |
|---|---|---|
| `the_play_game_builds_on_the_install` | pass | the play build, every act created |
| `the_scripted_play_run` | fails only on finding F1 | join → walk to Akara (found by walking the town's rooms until her preset spawns) → C→S 0x04 + 0x13 → menu Talk / Trade → Trade opens the shop (41 store items from the real tables), close sends 0x30 → the real town waypoint (mode 2, S→C 0x63) → 0x49 → Cold Plains, real monsters in the model |
| `the_live_run` | fails on F1, F2, F3 (collected, run goes on) | join → route out of town into the Blood Moor → kill (death by mode 0/12) → gold drop picked up (0x04, 0x16) → kills until level 2 (500 experience, `experience` row 2) → stat and skill points 5 and 1 (`charstats` StatPerLevel) spent (0x3A, 0x3B Fire Bolt = skill 36) → client learns skill 36 → TP scroll (0x20) → portal (class 59) → town → unequip / equip → save, drop, load, join → stats and skills equal |
| `a_blocked_run_is_drawn_where_the_server_stops` | see §5 run table | the wall stamped in both grids, now on the real town |

The run collects divergences in `Run::findings` and asserts them empty at the end, so one
finding does not hide the steps after it.

## 4. Findings (q-fix-real-* rows)

| Row | What | Repro | Owner |
|---|---|---|---|
| q-fix-real-client-path (F1) | After a run leg the drawn player stands up to 6 sub-tiles past where the server's stopped (e.g. leg to (4932,4206): drawn (4930,4208), server (4924,4209), a fire object at (4925,4210) between). The client path's grids lack the objects' and units' footprints (REC-277 (d) gap); no 0x96 snaps it back | `cargo nextest run -p d2-client --test play_smoke --run-ignored only` (D2_GAME_DIR) | client path (`bridge/client_path.rs`), `objects-client.md` footprints |
| q-fix-real-equip-2h (F2) | C→S 0x1A of the new sorceress's staff (`sst`, two-handed) back to the right hand after 0x1C answers 0 and leaves it on the cursor; with an item on the cursor every later attack is ignored (the player stays mode 1) | `the_live_run` (the equip step; run with `SMOKE_DEBUG=1` for the attack trace) | `items/inventory.md` §4.3/§4.6 path in `d2-sim` `items/moves` (body slot state after `remove_body_item`, or `body_location_allowed` on the real `itemtypes`) |
| q-fix-real-start-belt (F3) | The new sorceress's four `hp1` (charstats item2, count 4) never reach the client model; after save → load they come back in the stored inventory (mode 0), not the belt | `the_live_run` (the save step's item diff) | new character start items (`generation.md` §10.3) / save items (`save_full`) |
| q-fix-real-preset-objects | R2's object pass is provisional: mode, order, skipped classes unspecified | capture: S→C 0x51 of a 1.14d join in the Rogue Encampment | spec `population.md` §11.1 first pass (local) |
| q-fix-real-floor-rle-rows | R4: what 1.14d's RLE floor drawer does with rows 15–31 of a 32-row block | a capture of a lit floor tile with a 32-row RLE block | spec `render/shading.md` §4 r3 (local) |
| q-fix-real-unit-free-light | R5: the client unit free's light removal is not in `model.md` §2 r5 | RE of the client unit free | spec `client/model.md` §2 (local) |
| q-fix-real-corpse-regen | A killed monster's life keeps regenerating (+16 per few frames) after mode 12; `stat-lists.md` §10.1 has no dead-mode stop | `the_live_run` with `SMOKE_DEBUG=1` (life after the kill) | open question, `sim/stat-lists.md` §10.1 / death events |
| q-fix-real-build-rooms | The play build streams the first room of Cold Plains and Lut Gholein at creation (staging of the old tests, d2rs-own); 1.14d streams rooms around players | — | `single_player::build_with` |
| q-fix-real-known-wp | A new character knows Cold Plains' waypoint (the loader's staging, `loader(character, cold_plains_wp)`), which 1.14d does not give | — | `single_player::loader` |

## 5. Real-data run of the migrated tests

RESULTS_PLACEHOLDER

## 6. Not done (Wave 1 rest, in order)

1. Rewrite the 25 removed files on the install (`q-fixture-migrate-removed.tsv`: test names,
   Covers claims, the real source each needs). Their positions and counts must come from the
   real DS1s by a tool (`facts/`, Wave 3), never typed in.
2. Fix F2 and F3 (they block every item and combat test after an equip).
3. `test_fixtures::install` users outside the client (TF01/TF02: 3 `d2-server`, 2 `d2-native`
   files, `play_native.rs`) still build a made-up install; move them to `$D2_GAME_DIR`.
4. Tests in the migrated files whose assertions were derived from the invented world
   (message counts in `app_frame_loop.rs`, positions in rigs) fail on the install: each
   needs its expected value from a recorded trace (`traces/`), queued in §5 of HANDOFF.
