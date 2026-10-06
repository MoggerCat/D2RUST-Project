# Handoff: world wiring of objects, quest objects and hireling callers — `claude/wire-world-staging`

Cloud wiring session, 2026-10-06, task class: implementation from clear
specs, medium (METHODS M14). Base: `claude/specs-staging` at `2f860d1`.
Repo only, synthetic tables, no game files (M09). Inputs: the handoff
notes `impl-objects`, `impl-quests-act1-rest`, `impl-quests-act2`,
`impl-hirelings`, `impl-room-population`, `impl-missile-bodies-2`;
`specs/sim/rng.md` §5.2, `specs/world/objects.md`,
`specs/world/object-functions.tsv`, `specs/world/quests*.md`,
`specs/world/hirelings.md` §6, §8, §10, §11,
`specs/sim/path-placement.md` §10. Edits are confined to wiring / host
modules and are additive (new files, new default trait methods, one new
`ActionHooks` field); no quest, object or hireling rule module changed.

## 1. Result (wired, unverified — M02)

| Task | Where | What |
|---|---|---|
| Game-creation chain | `d2-sim` `wiring/worldgen/creation.rs` (`WorldSim::create_game`, `CreationTables`, `CreatedControls`, `CreationError`) | Regions → object control → NPC control → quest control, each one game-seed step, in `rng.md` §5.2 order (sim-0002). The NPC and quest controls are handed to the host. `test-fixtures` `GameData::world_sim` now creates the object control after the regions (`object_tables()` added). |
| Object host tick | `d2-server` `Intents::set_host_tick` (default no-op) ← `Host::frame` (once per frame, before the drain) → `SimGame` → `WorldHost::host_tick` → `ActionWorld` / `WiredWorld` → `ActionHooks::set_host_tick` | The frame's host clock is the object code's `GetTickCount` (`objects.md` edge case 9). `WiredWorld::now` (vendors) stays the caller's (its fixtures pin it; TODO in `wired.rs`). |
| Quest object routing | `d2-sim` `wiring/action/objects.rs` (`QuestObjectCall`, `ObjectState::route_quests` / `take_quest_calls`, `View::object_route`), `ActionSim::route_quest_objects` / `take_quest_calls`; `wiring/economy/quest_objects.rs` (`init_fn`, `operate_fn`, `run`, `run_all`) | With the queue on, quest inits, quest operates (0x13 and the AI door path) and object event 7 are queued with the object's class and the init's room / position instead of `Pending::object_route`. The dispatcher runs: inits 4, 6, 15, 20, 21, 29, 30, 31–33 (bare `ret`), 38, 47, 54; operates 6, 9, 10, 12, 21, 24, 25, 34 (only its `0x0059BAF0(level)` call, then handed back), 39, 40, 41, 42; event 7 through `quests::object_event` (`0x005449E0`). Everything else is handed back to `Pending::object_route` (as before). The index → address table is checked against `object-functions.tsv` (owner `world/quests.md`, address, `Route::Quest`), with a perturbation test (M05, M08). |
| Drain points | `d2-server` `WiredWorld`: after the 0x13 dispatch, before and after every quest call, `WorldHost::after_tick` (new default hook, called by `SimGame::tick` after `tick::tick`, before the sends are taken); `d2-sim` `QuestTick::run_quest_objects` after `run_event`, `update_quests` and the room-pass hooks that allocate (presets, inactive restore, objects, monsters) | The queue is turned on by `WiredWorld::host_tick` from the first frame (`ActionSim::route_quest_objects` is idempotent; a creator can call it right after `create_objects`). The quest call runs with the deferred mercenary reward as before (`quest_call`). |
| Quest seams on the wired host | `d2-sim` `wiring/economy/quest_host.rs` (`HostQuests`), used by `WiredWorld` for every quest call | On top of `EconomyQuests`: object mode (+0x10) and the mode set `0x00624690` on objects with object data; `object_anim_length` = `objects.txt` `FrameCnt1` as stored (+0xDC, `0x00640E90`); object events and event 7 on the game's timer queue; the Act I `spawn_object` (`0x00555230` with a mode) through `View::create_object`; `free_object_collision` → the object code's footprint seam; `unit_level` from the DRLG room; `interact_unit` / `set_interact_unit` (`0x00554120` / `0x00554190`) on the host's one owner (`NpcRest`); `identify_item` (`0x006280D0(item, 0x10)`) on the item store. Each falls back to the rest when the object has no object data, the unit no DRLG room, the item is not in the store. The other 81 `QuestWorld` calls forward to `EconomyQuests` (generated forwarding). |
| Hireling teleport follow | `d2-sim` `ActionHooks::pet_follows` (new field, `None` default), `wiring/path/place.rs` `PlaceHost::pets_follow` → queue; `d2-server` `WiredWorld::pet_follows` (drained after skill, walk, waypoint handlers and after each tick) | `path-placement.md` §10 rule 6 → `hirelings.md` §6 rule 1 (`life::follow`) on `InteractionState::hirelings`; no hireling tables → the queue is dropped. |

Tests (all `// Covers:`):
- `d2-sim` `wiring::worldgen::tests::creation` (seed order against the
  four steps by hand; M08 without the object step);
  `wiring::economy::quest_objects::tests` (table check + perturbations);
  `tests/prop_wired_path.rs` `a_warped_player_queues_its_pet_follow`.
- `d2-server` `world/tests/objects.rs`
  `the_object_host_tick_is_the_frames_host_clock`;
  `world/tests/quest_objects.rs` (5): Cairn stone init after the tick
  (§2.2 r3), gibbet operate via 0x13 and its event 7 at frame + 17 on
  the real object (§1.1, §1.2), an unstated quest operate handed back,
  `HostQuests` answers (owner, FrameCnt1, mode set), the hireling follow
  after a tick (living warped, flags 2 |= 0x10000; dead stays).
- Test-fixture changes (additive): `ActionRest` (trade_quests) gained
  `in_range` and a route log; its `Rest::warp_to` logs.

## 2. Gate (this branch)

`cargo test -p d2-sim -p d2-server -p conformance -p test-fixtures`: all
green except the 8 known reds of other sessions
(`monsters::ai::tests::specd_here_*` 2, `skills::use_::tests::*` 4,
`skills::mutant_tests::table_check_mutants::*` 2); clippy `-D warnings`
(d2-sim, d2-server, test-fixtures, all targets), `cargo fmt --check`,
`py tools/coverage.py --check` (7,081 claims, 0 errors),
`py tools/spec_index.py --check`. `cargo check --tests -p d2-client`
could not run in this container (`wayland-sys` build script: no
Wayland development files); the shared items touched are additive for
it: `Intents::set_host_tick` and the new `WorldHost` hooks have
default bodies (its one `Intents` impl, `prop_client_bridge::NoGame`,
needs nothing), `ActionHooks` is built through `ActionHooks::new`
everywhere. Run it locally (HANDOFF §5 is not needed: it is a build
check).

## 3. Not wired, with the reason

1. **Quest routes run when drained, not inside the call.** In 1.14d a
   quest init runs inside the allocation and an operate inside the
   dispatch. The quest control is the host's, outside the action
   wiring's call stack. Exact for the 0x13 operate (nothing follows it in
   the handler, `waypoints.md` §5.2) and for event 7 under `QuestTick`;
   an init's draws and allocations (the marker init's town Cain, `0x005940E0`)
   come after the rest of the hook's allocations. Fix path: lend the
   quest control (and the quests' rest) into the action wiring the way
   `monster_world` is lent. Queued as HANDOFF §5 #77.
2. **Hireling death** (`hirelings.md` §8 r1): the `0x00457490` test and
   the death flag argument in the kill path are open question 8, and
   `damage.md` §7.2 does not place `0x005751A0` among the kill's steps.
   `life::death` keeps no caller.
3. **C→S 0x61 swap** (§11): needs the item copy `0x0055A2A0` (no items
   spec body; it is a `TradeRest` call) and a `MoveWorld::equip_on_merc`
   with player and result; `HirelingItems` has no provider.
4. **Save restore** (§10): the `.d2s` format layer exists but the load
   effects (`d2s.md` §9, `d2-server` character storage) have no code;
   `life::restore*` keeps no caller.
5. **Classic act change** (§6 r3–4): the act change `0x0053ACC0` has no
   code (`path/place.rs` names it out of scope).
6. **Quest seams still on the rest** (Pending with reason): `client_in_act`
   (client +0x1AC has no home in `ClientEntry`), the Tainted Sun
   environment (`0x0061C450`, environment spec), `quest_chest_gate`,
   `object_treasure`, `drop_gold` (object chest drop, `ChestWorld` not
   provided), `quest_drop` (`0x00559A30` not in the items specs),
   `set_room_portal` / `refresh_room` (`0x0061AED0` not specified),
   `spawn_quest_object` (the Act II argument form's mode is not stated),
   `open_insert_dialog` (0x58 layout), `missile_range`, `is_trading`,
   `remove_unit`, `npc_hold_chat`, `npc_intro_heard` / `set_npc_intro`
   (storage open, quests-act2 OQ5), `spawn_location` / `free_spot_near`
   (on `WorldHost` / path provider, not reachable from the action-only
   economy), `unit_distance`, `living_player_within`,
   `spawn_monster_flags`, `open_portal`, `create_missile`,
   `set_missile_target`, `client_save_flags`. Jerhyn inits 18 / 19
   (`0x0059F380`, `0x0059F440`), the gibbet init 7, the tree init 9,
   the Cain portal init 61, Wirt's body operate 33 and init 37 are handed
   back (bodies not stated).
7. Missile-body seams (`impl-missile-bodies-2.md` §1 "Seams") are not
   touched here: their providers sit in skills / monsters / quests and
   the missile wiring is another session's.

## 4. Local run queue (added to HANDOFF §5 as #76–#78)

- #76: trace a Cairn stone, gibbet and tome interaction in Act I with
  packets + RNG (tick numbers of the 0x0E mode messages, event 7 at
  operate frame + 17): the wired host must give the same tick and bytes.
- #77: trace the creation of the town-Cain marker object (385) after
  Cain left Tristram: the RNG draw order of Cain's spawn against the
  room's other object allocations decides whether the drained init
  (§3 item 1) is exact.
- #78: a teleport with a living hireling (packets): the hireling's warp
  messages relative to the player's 0x15 / room messages (the follow runs
  after the handler here).
