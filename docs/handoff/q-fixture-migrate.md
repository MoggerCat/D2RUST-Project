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
| R4 | The first frame in town panicked: real floor tiles carry 32-row RLE blocks, `shading.md` §4 r3 defines 15 gradient rows, the composer refuses the draw (`GradientArea`) | found here and fixed in parallel on staging (the gradient block draws its 15 lit rows, `shading.md` §4 r3–r4); the merge keeps staging's rule and drops this branch's provisional flat fallback | `world_view/preview_blocks.rs` (staging) |
| R5 | Walking out of the first room was fatal 0x591 (`lighting.md` §6.4): the client unit free (S→C 0x0A) left the fire's kind-2 light behind | **PROVISIONAL**: `ClientWorld::remove` removes the unit's light (`0x00464930`, §6.2 r5) | `bridge/world.rs` |
| R6 | Every player skill with `anim` SQ (mode 18) stalled: no sequence was loaded, so the mode had no frames and no events | the sequence table (`sequences.md` §4, compiled from `sequences.tsv`) and its lookup by `seqnum` and COF weapon class, loaded on a player's mode start | `d2-sim` `skills/sequences.rs`, `wiring/action/units.rs` `load_sequence` |
| R7 | The item type test of `use_state` was a d2rs-own "some hand holds a wanted type" (REC-176): bare-handed Tiger Strike was refused, one claw passed Dragon Claw and one weapon passed Double Swing | the 1.14d rules of `use.md` §2 (sets a and b, `hand(s, X, Y)`, the swapped order, the bare-hand rule, skills 4 / 5); **PROVISIONAL** (q-fix-real-item-type-test): no-inventory, item flags 0x4000 / 0x100 and the `shoots` ammo test not applied (the host's item flags and `shoots` type are not wired) | `d2-sim` `wiring/interaction/skill_use.rs` `weapon_type_ok` |
| R8 | The COF weapon class was the copy's hand class (`1hs` for a katar), so the claw sequences (seqnum 16, `ht1` / `ht2` only) found no frames: Fists of Fire and Dragon Claw dealt nothing | `0x0064F380` per `unit-composite.md` §2.1 from the items' `wclass` / `2handedwclass` / `component` (two weapons: the barbarian's r-dual, the assassin's `ht2`); also feeds the save's appearance (`save.rs`) | `d2-client` `app/weapons.rs` `cof_class`, `d2-sim` `InvItemRec::{wclass, wclass2}` |

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
| q-fix-real-unit-free-light | R5: the client unit free's light removal is not in `model.md` §2 r5 | RE of the client unit free | spec `client/model.md` §2 (local) |
| q-fix-real-corpse-regen | A killed monster's life keeps regenerating (+16 per few frames) after mode 12; `stat-lists.md` §10.1 has no dead-mode stop | `the_live_run` with `SMOKE_DEBUG=1` (life after the kill) | open question, `sim/stat-lists.md` §10.1 / death events |
| q-fix-real-front-save | `smoke_frontend`'s Esc-menu Save and Exit on the install writes a file that reads back as checksum error (`difficulty_popup_starts_the_game_on_nightmare`) or "not a character save" (`play_cli_new_and_save_paths`), while `play_smoke`'s server-side save of the same path loads | `cargo nextest run -p d2-client --test smoke_frontend --run-ignored only` (D2_GAME_DIR) | `app/save.rs` / `flows/save-exit.md` |
| q-fix-real-build-rooms | The play build streams the first room of Cold Plains and Lut Gholein at creation (staging of the old tests, d2rs-own); 1.14d streams rooms around players | — | `single_player::build_with` |
| q-fix-real-leap | A player's Leap (SQ seqnum 13) lands 1 of the 5 aimed sub-tiles ((5007,4249) aimed (5012,4249) → (5008,4249)); the leap draw arc then runs past the landing frame | `cargo nextest run -p d2-client --test app_barbarian --test app_move_anims --run-ignored only -E 'test(leap)'` (D2_GAME_DIR) | `bodies-2.md` §2.13 (the `aurarangecalc` reach and the clamp), `bodies-2b.md` §6.11 |
| q-fix-real-whirlwind | Whirlwind (srvst 38) is refused on the install: no mode 18, no mana spent, the player stays mode 1 | `app_barbarian` / `app_move_anims` `whirlwind_*` | `bodies-2b.md` §8.10 (the start's path compute) |
| q-fix-real-leap-attack | Leap Attack (srvst 41, SQ seqnum 14) spends mana but never strikes the zombie 6 sub-tiles away | `app_barbarian` `leap_attack_leaps_to_the_monster_and_strikes_it` | `bodies-2b.md` §6.11, §6.12 |
| q-fix-real-sentry | A laid Charged Bolt Sentry (`monstats` 411, AI 101) enters the d2rs-own `sentries` map with 5 shots and dies (mode 0) ~30 frames later with no monster near; a Lightning Sentry (412) dies on its first think and never enters the map. AI 101's charges (`ai-bodies-6.md` §14 step 1–2: owner link, the trap's `Skill1` entry and its `calc4`) is the death path to check; the `sentries` map itself is d2rs-own (REC-233) | `app_assassin_gaps` `a_trap_without_a_target_holds_its_shots`, `a_lightning_sentry_fires_its_missile` | `monsters/ai-bodies-6.md` §14, `skills/bodies.md` §8.3 |
| q-fix-real-known-wp | A new character knows Cold Plains' waypoint (the loader's staging, `loader(character, cold_plains_wp)`), which 1.14d does not give | — | `single_player::loader` |

## 5. Real-data run of the migrated tests

Command (debug build, 5-minute cap per test, nextest config `slow-timeout = { period = "60s",
terminate-after = 5 }`):
`D2_GAME_DIR=$HOME/game cargo nextest run -p d2-client --run-ignored only -E 'kind(test) & not test(/gpu_/)'`
on `77a1a9bf` (after the staging merge). 177 tests: 110 pass, 65 fail, 2 time out.

- **Migrated tests: 86 of 150 pass on the install** (the class skill tests of amazon, sorceress,
  necromancer, paladin, druid, assassin, the act travel by NPC, Mephisto's quest, levelling,
  stamina, HUD, items, gold, the hardcore and death flows, save files, …). The other 27 are the
  pre-existing game-file tests: 24 pass.
- The 64 migrated failures, by cause (none is a new engine fault; each group is a test that
  still carries an invented-world assumption, to rewrite as §6 says):

| Group | Tests | Cause | Next |
|---|---|---|---|
| G1 rigs leave town by the Den cave (**done**, 26 / 33 pass) | app_barbarian 10, app_skill_gaps 12, app_assassin_gaps 5, app_move_anims 2, app_play_monster_ai 2, app_town_portal 1 | `Rig::leave_town` expects the cave entrance tile in the model at the join (the invented town bordered it) and then teleports to invented Den coordinates; the rigs also install made-up skill rows | rewrite the rigs: route into the Blood Moor (`test_fixtures::host::route`), the install's skill rows, expected numbers from `skills.bin` |
| G2 Akara at the join | app_play_quests 4 | Akara's preset room is not active at the join on the real map | walk to her (`play_smoke`'s `approach`) |
| G3 waypoints and travel | app_a3_a5_waypoints 2, app_act_travel 1, app_waypoint_warp 2, app_play_npc 1, app_town_portal 1 | the tests stage the waypoint unit and its known indexes the old build placed; the real town waypoint appears only after the population | find the preset waypoint as `play_smoke` does |
| G4 invented counts and streams | app_frame_loop 1, app_single_player 1, app_server_skills 1, app_levelup 1, app_client_drlg 1, seam_drlg_coords 2 | message counts, join streams and skill lists derived from the invented world (e.g. "the join's two 0x23 (no StartSkill on synthetic data)") | expected values from a recorded join (`traces/`), queued; `seam_drlg_coords` and `the_session_join_on_the_install` now get the install's client tables (they failed on fatal 0x668: no skills table) |
| G5 invented items on the server | smoke_save 5, smoke_frontend 2 | `smoke_save`'s fixture places invented items ("item 0 worn at body location 4" panics on the server thread); `smoke_frontend`'s Esc-menu save reads back as checksum error / bad magic | rewrite the item fixture on real item records; the `smoke_frontend` read-back is a finding (q-fix-real-front-save) to reproduce first |
| G6 other position assumptions | app_cain_quest, app_level_border, app_town_gaps, app_play_preview, app_server_core, app_play_npc | invented positions (a free spot in the first streamed room, Gheed placed by the build, frames of the invented tile set) | each against the real presets |
| G7 time | play_smoke `the_live_run` | over the 5-minute cap in a debug build (it passes the cap in release: 166 s for the whole run before the merge) | run the gate in release (`realdata-gate.sh` default) |

**G1 after the rewrite** (`tests/real_rig/mod.rs`, one rig for the barbarian, skill-gaps,
assassin-gaps, move-anim and monster-AI tests; the old `app_barbarian/rig.rs` and
`app_skill_gaps/rig.rs` with their made-up skill, state, monster and AnimData rows are gone):
`cargo nextest run -p d2-client --test app_skill_gaps --test app_assassin_gaps --test app_barbarian
--test app_move_anims --test app_play_monster_ai --test app_town_portal --run-ignored only`
(D2_GAME_DIR): 33 tests, 26 pass; the 7 failures are q-fix-real-leap (2), q-fix-real-whirlwind (2),
q-fix-real-leap-attack (1), q-fix-real-sentry (2). What the rewrite learned from the real rows:

- A real zombie regenerates its life back within ~25 frames (12 / 256 a frame), so a hit is read
  as the lowest life in the window (`Rig::lowest_life`), never the life at its end.
- Smite, Zeal, Tiger Strike, Fists of Fire, Dragon Talon, Dragon Claw and Double Swing have
  `AttackNoMana`: a failed item type test makes the request an Attack (`use.md` §2 step 4), so
  "Smite without a shield" and "Dragon Claw with one claw" assert the Attack (used skill 0),
  not "nothing happens".
- The start kits (paladin `ssd` + `buc`, assassin `ktr` + `buc`, barbarian `hax` + `buc`) are
  the install's; a second weapon is staged by `Rig::wear` (the install's item record created and
  equipped from the cursor), a hand emptied by `Rig::take_off` (C→S 0x1C, 0x17).
- Old tests that leaned on invented rows were re-aimed at the real skill of the same body:
  "Fire Blast" is `Fire Trauma` (251); the finisher is Dragon Talon after Tiger Strike's charge;
  "Tiger Strike without a claw is refused" became the `use.md` vector "bare-handed passes"; the
  dual-claw strike is Dragon Claw; the trap is Charged Bolt / Lightning Sentry with the shots of
  its own row (read after laying, never typed in).

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
