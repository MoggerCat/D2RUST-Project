# Handoff: impl-app-world (2026-10-07)

Cloud implementation session (medium, METHODS M14). Base
`claude/specs-staging-6` @ `eefcd94`; branch `claude/impl-app-world`.
Repo only, no game files (M09): every claim holds on this branch, on
synthetic data; the live paths are **wired, unverified** (M02, queue
below). Task: `docs/handoff/impl-app-gamestart.md` "Not done" and HANDOFF
step 7v (c): the app's game on the full world, game creation in the
`rng.md` §5.2 order, one unique-bit store, the production drop / save
loaders, `object_drops` and the hireling tables installed, `--save`.

## 1. What landed

| Item | Where | What |
|---|---|---|
| The app's game on the full world | `d2-client` `app/single_player.rs` | `Sim = SimGame<WorldSim<LocalSeams>, WiredWorld<AppRest>>` (was `ActionSim` + `ActionWorld`). The tick's room pass now runs population on the world state; the wired host runs waypoints and the NPC / vendor / quest / cube ids. `LocalSeams` also implements `WorldPending` (defaults). |
| Game creation, `rng.md` §5.2 | `build_with` | Before any unit: `ActionEvents::create_game` (Normal, expansion, game type 3, not ladder; game seed `{N, 666}` unstepped, the fixed-seed branch of §5.2), then `WorldSim::create_game` (regions → object control → NPC control → quest control, one game-seed step each). The NPC and quest controls go to `WiredWorld::new`. S→C 0x03 now carries `dwObjSeed` (game +0x80) instead of 0. |
| Game type 3 | `GAME_TYPE`, `create_request_for` | The client's 0x67 carries game type 3 (`rng.md` §5 open question, answered: single player's create message, `0x00477CDF`); the creation fields and the world state's `GameInfo` get it too. |
| One unique-bit store | `d2-sim` `ActionHooks::uniques`; `economy/{chest_drop,death,quest_host,quest_objects}.rs`; `d2-server` `WiredWorld` | Game +0x1B24 has one home on the action wiring, beside the game seed. The chest drop and the monster drop take it into their `GameFields` and write it back; the quest chest's treasure hands the economy's bits back to the hooks for the drop; `QuestLoan` no longer carries bits (it lends the hooks'); `WiredWorld::uniques` is gone (`with_economy` takes the hooks'). `DeathDrops::fields.uniques` is no longer read (left empty). Closes `impl-world-rest.md` §4 item 9. |
| Production game tables | `d2-server` `world_data::game::GameTables` | Loaded set + `AnimData.d2` + fixed-up set from the archives, and every view a game is built from (action, object, world, vitals, stat, unit, hire rows, item, vendor tables): the production counterpart of `test-fixtures` `game::GameData`, same calls. |
| `DropTables` loader | `world_data::tables::drop_tables` | `ItemTables::from_fixed`, `TreasureClasses::build` (treasureclassex, itemtypes, item list, itemtypes equivalence, uniqueitems, setitems), `item_list`, `superuniques`. |
| `d2s::SaveTables` loader | `world_data::tables::SaveData` | `itemstatcost` `CSvBits` / `CSvParam` / `CSvSigned`; item entry length through `read_save_entry` (§8.1 r2); hireling restored = present (`hirelings.md` §10 r1) and the row of `Id` at level 1 found in the game's version (§10 r2). |
| Installed at game creation | `build_with` (live data) | `ActionHooks::object_drops` (`DeathDrops` on `drop_tables`), `InteractionState::hireling_tables` (`hireling_tables`), anim data, vitals, and the full action / stat / unit / world / object / item / vendor tables. Synthetic data: empty tables, no drop state, no hireling tables. |
| `--save <file.d2s>` | `main.rs`, `play.rs` (`PlayConfig::character`, `send_create_game_for`), `single_player::{load_character, LiveData::read_save, save_name, create_request_for, start_with}` | Reads the file with `SaveData` and the §2.2 rule 4–5 checks against the app's game (client name = the save's, expansion, not hardcore, Normal); the 0x67 carries the save's class and name; the join loads it through `Character::Save` (`LoadContext`: Normal, saved map seed not applied: fixed-seed game, +0x84 = 1). Needs `D2_GAME_DIR` (synthetic data refuses). |
| The wired host's rest | `d2-client` `app/rest.rs` `AppRest` | Narrowest answers for every `NpcRest` / `HirelingRest` / `VendorRest` / `QuestRest` call (none, refused, out of range; state changes logged), kept state (quest records and names set at the join, last-bought GUIDs, the outbox). |
| One interaction store in the app | `rest::Interactions` (`Rc<RefCell<…>>`) | Shared by `LocalSeams` (`Pending`: object operate) and `AppRest` (`NpcRest`: the wired host's waypoints / NPCs), so a waypoint opened by 0x13 is the one 0x49 checks. Goes when impl-items-wiring moves the interaction onto `UnitRecord::interact`. |

## 2. Tests

New (all `// Covers:`):

- `d2-client app_single_player::game_creation_derives_the_four_controls_in_order_before_the_first_unit`
  (four steps then the waypoint's allocation; `dwObjSeed` = step 2's
  lo'; quest records on the host; M08: another seed, another object
  seed), `a_save_needs_the_users_tables_and_names_its_character`; the
  session-flow test now also checks 0x03's `f8` = `dwObjSeed` ≠ 0.
- `d2-server tests/world_data_tables.rs`: every `GameTables` view, the
  drop tables, `SaveData` (save columns, item entry length + M08 unknown
  code, hireling restored + M08 absent / no row) on the synthetic
  install.
- `d2-sim wiring::action::tests::objects::the_chest_drop_walks_…`: the
  hooks' unique bits survive the drop; the drop state keeps no copy.
- Ignored (local queue): `app_single_player::{live_game_creation_installs_the_drops_and_the_hireling_tables, a_save_from_the_command_line_joins}`,
  `world_data_tables::every_loader_builds_from_the_users_install`.

Changed test expectations: none in value. Mechanical changes only:
`events.hooks()` → `events.action.hooks()` (app tests), the staged
interaction goes through the shared store (`.interact.borrow_mut()`),
`world.waypoints` → `world.action.waypoints`; `world().uniques` →
`hooks.uniques` (`d2-server` `items::mutant_tests`, `prop_handle`
digest). Every app test kept its counts (`frame_loop_ticks_…`: 15 / 20
handled, unchanged).

## 3. Gate

`CARGO_INCREMENTAL=0 sh tools/gate.sh` on this branch: **GATE: FAIL**,
only in the two test steps, and only on failures that are not this
branch's. Every other step passes (spec index, methods, conflict
markers, coverage 9,496 claims / 0 errors, trace checkers, hook
selftest, fmt, depcheck, clippy on the whole workspace with
`d2-client`, `d2-sim` + conformance, doc-tests). With `--no-fail-fast`
over the rest of the workspace: 2,137 run, 2,127 passed, 10 failed:

- `d2-client`
  `e2e_full_loop::{full_single_player_loop, same_seed_same_run, other_seed_other_run}`,
  `e2e_single_player::{single_player_end_to_end, same_seed_same_run, other_seed_other_run}`,
  `e2e_vendor::{vendor_end_to_end, same_seed_same_run, other_seed_other_store, one_gold_more_changes_only_the_gold_fields}`:
  the base's known e2e fixture failures (the purchase now completes,
  `(50, Some(Done))`, since impl-items-rest's vendors' copy, where the
  fixtures expect the refusal); the `impl-items-wiring` session owns them.
- Also seen in one of two gate runs: `d2-server
  prop_unified_items::item_moves_keep_one_place` (random input). Its
  minimal input (`seed = 613126552`, ops `[OpenTrade, Msg { id: 51, a:
  21, .. }, Msg { id: 50, a: 244, b: 0, x: 137, y: 0 }]`: "item UnitId(4)
  is in 2 places: [Page(UnitId(1), 0), Ground]") **fails identically on
  the base `eefcd94`** (checked with the input pinned in a scratch test):
  the vendor purchase's copy, items area, not this branch.

## 4. Local run queue (add to HANDOFF §5)

- `D2_GAME_DIR=<install> cargo test -p d2-server --test world_data_tables -- --ignored`:
  expect PASS (every view builds; no TC notes; 66 superuniques; one item
  list entry per items row; one save column per `itemstatcost` row).
- `D2_GAME_DIR=<install> cargo test -p d2-client --test app_single_player -- --ignored live_`:
  expect PASS for `live_data_generates_the_levels_…` and
  `live_game_creation_installs_the_drops_and_the_hireling_tables` (drop
  state and hireling tables set, NPC records non-empty, one vendor record
  per NPC record).
- `D2_GAME_DIR=<install> D2_SAVE=<an expansion softcore Normal .d2s> cargo test -p d2-client --test app_single_player -- --ignored a_save_from_the_command_line_joins`:
  expect PASS (joined); record the printed load log (the save-load steps
  the providers could not apply).
- The two ignored app tests of `impl-app-gamestart.md` §"Local run queue"
  (`the_session_join_on_the_install`, `frame_loop_runs_on_the_users_levels`)
  now run on the full world (population in the town's rooms, real
  tables): expect PASS; if a message count changes, it is the town's
  population (NPCs, objects) arriving with the room switch — record it.
- `D2_GAME_DIR=<install> cargo run -p d2-client -- play --save <file.d2s> --frames 300`:
  expect "play: character from …" and a clean exit.

## 5. Gaps (not guessed)

| Where | Missing |
|---|---|
| `formats/d2s-load.md` (quest section → `PlayerQuests`) | A loaded save's quest flags are not applied to the wired host's quest record: it starts new (logged at the join). |
| `vendors.md` edge case 10 | `WiredWorld::now` (host ms for store generation) stays 0: nothing in the app updates it. |
| `AppRest` | Every rest call answers the narrowest; NPC talk and trade are refused (distance out of range, start not allowed), item placement fails. Providers come with their specs (`wire-interaction.md` §6). |
| Synthetic install | `pettype` has no row 7, so `hireling_tables` does not load on it (the synthetic app game has none). |
| Hardcore saves | The app's game is softcore (create flags carry no hardcore bit; no spec of the client's flag for it), so a hardcore save is refused by §2.2 rule 5.3 (code 11). |
| Interaction store | `rest::Interactions` is an app-local bridge until the interaction moves onto the unit record (impl-items-wiring). |
