# Handoff: end-to-end tests of the night's world features — `claude/e2e-night-flows`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Cloud integration-test session, 2026-10-07, task class: tests from
specs, medium. Base: `claude/specs-staging` at `913d3b0` (the ninth
fold). Repo only: no `game/`, no `re/`, no recordings (M09: every claim
below holds on this branch, on synthetic data; nothing here is verified
against 1.14d, M02). Read: `CLAUDE.md`, `docs/METHODS.md`,
`docs/HANDOFF.md` §1–§3 and §7 "Ninth set", the handoff notes
`impl-server-join`, `wire-world-staging`, `impl-objects`,
`impl-hirelings`, `impl-room-population`; `specs/sim/rng.md` §5.2,
`specs/world/objects.md`, `specs/world/hirelings.md` §3, §6, §13,
`specs/sim/pets.md` §8, `specs/client/model.md` §14,
`specs/world/npc.md` §7, §9, `specs/world/quests.md` §2.3, §6,
`specs/world/quests-act1-rest.md` §1, §2, `specs/world/quests-act2.md`
§1, §6.7, `specs/sim/intents-events.md` §7.4, §7.6, `specs/sim/tick.md`
§3, `specs/sim/units.md` §4.

No production code changed. Three new test files and one dev-dependency.

## 1. What was added

| File | Base | Tests |
|---|---|---|
| `crates/test-fixtures/tests/e2e_night_flows.rs` | the synthetic install (`test_fixtures::install`, `GameData`): `WorldSim` with the path provider, `WorldSim::create_game`, `WiredWorld<Rest>`, `Host`; the session join `enter_game` | 6 |
| `crates/d2-server/tests/e2e_night_world.rs` | the shared e2e act-0 world (`d2-client/tests/e2e_support/{mod,world}.rs` by path: ISLE / GATE preset levels, waypoint, tables) with live room population on ISLE, `WiredWorld<Rest, WiredSkills>`, `Host` | 3 |
| `crates/d2-client/tests/e2e_pet_action.rs` | the server's 0x7A builder → the client's dispatch table | 1, `#[ignore]` (I-1) |
| `crates/d2-server/Cargo.toml` | dev-dependency `d2-sim` with `bench-fixtures` (the shared world fixture's combat tables) | — |

Every test drives C→S bytes through `Host::send_game` / `Host::frame`
and checks the S→C bytes `Host::receive` gives the client, and sim
state; every test has its `// Covers:` line (coverage `--check`: 8,290
claims, 0 errors).

## 2. Flows covered

1. **Game creation and the real join** (`game_creation_then_the_real_join`):
   `create_game` steps the game seed exactly four times before any unit
   (regions, object control, NPC control, quest control, `rng.md` §5.2);
   `dwObjSeed` = the second step's lo' (`objects.md` §2 r2); the quest
   records of chains 4 and 11 and the NPC record of the install's
   `interact` monster exist. The join: exact bytes of 0x59, 0x0B, 0x03
   (act 0, the DRLG init seed, town level, `dwObjSeed`), 0x07 (spawn
   room), 0x15 (spawn point, flag 1), then the first tick's 0x07; nothing
   more while standing.
2. **Chest** (`opening_a_chest`, the install's `objects.txt` row 1,
   operate 4): C→S 0x13 → one `roll(100)` on the control seed, mode 2,
   flag 0x2 cleared, S→C 0x0E `0e 02 <guid> 03 00 02000000`; a second
   operate does nothing (§8.1 r1).
3. **Shrine** (`using_a_shrine`, added row: init 1, operate 2, `SubClass`
   1; `shrines` row 1 code 2, reset 1 minute): the init's `roll(1)`
   draw and shrine id 1; the operate: +0x0C := operator GUID + 1, mode 1,
   event 5 at frame + 1201; S→C 0x0E (mode 1) then 0x4D (2, object GUID,
   operator GUID, code, zeros; 17 bytes); a second use refuses; at frame
   + 1201 event 5: mode 0, +0x0C := 0, 0x0E (mode 0).
4. **Act I quest objects**: `cairn_stone_inits_on_the_created_quest_control`
   (a new game's chain 4 has not-intro 1, `quests.md` §2.3 r1: the stone
   init's step 2 changes nothing; with not-intro 0 staged, step 3: X +0x40
   := the stone, the portal timer, mode 2 → 0x0E; the timer's portal is
   refused by the rest, the timer stays pending);
   `opening_cains_gibbet_and_its_event_7` (not-intro staged; 0x13 →
   mode 1, events 1 and 7 at frame + 15 / + 17, X +0x54 = 3, X +0x3C =
   the player, 4.13 and 4.1, S→C 0x28 (103 bytes, the player's record)
   and 0x0E; at frame + 17 mode 3 and its 0x0E).
5. **Act II quest step** (`reading_horazons_journal`, row 357, operate
   42): mode 1, end-animation event at frame + 15, chain 11 state 5,
   12.14 set (not in level 74: no grants), S→C `5D 0B 00 0C 0000` and
   0x0E; a second read keeps mode 1 and sends the completion flag again
   (the player still lacks 12.0 / 12.1).
6. **Hiring** (`hiring_at_greiz_stops_at_the_unit_spawn`): the talk
   (0x13 type 1) sends 0x4F then one 0x4E per offered slot; the slot
   seeds and offers are recomputed in the test from the NPC-control seed
   by `npc.md` §7.1 (one step per slot, then the 10-offer probe); C→S
   0x36 of the first offered name takes the §7.3 step 5 price, then
   step 7's unit creation fails (Pending, §3), so S→C 0x2A code 15, GUID
   −1, the gold after payment (§9); no pet node, the slot not hired.
7. **Hireling follows a waypoint teleport**
   (`the_hireling_follows_a_waypoint_teleport`): with a living pet node
   on a real monster and `pettype` row 7's flags as `hirelings.md` §6 r2
   states them, C→S 0x49 ISLE → GATE places the player in a GATE room
   (0x07, then 0x0D), the placement queues the pet follow, the host runs
   it: flags 2 bit 0x10000 on the hireling (§6 r5), the follow queue
   drained.
8. **Kill from live room population**
   (`a_population_monster_killed_with_a_missile`): tick 1's room pass
   places the DS1 preset and room population's monsters (class 0, the
   preset told apart by its `preset` log line); the nearest population monster (not the preset)
   is walked to (0x01), shot with the right skill (0x0C): one missile,
   it flies into the footprint, life 0, mode 0, 100 experience, the death
   start, the gold drop at (x + 2, y + 3) in mode 3; the death mode's
   single event 1 fires.
9. **I-1** (`server_0x7a_reaches_the_client_with_the_same_pet_and_owner`,
   ignored `I-1: 0x7A layout disagreement, waiting on PC 1`): the server
   builder writes pet@5 / owner@9; the client handler reads owner@5 /
   pet@9. Run with `--ignored`: the client's record holds (pet, owner) =
   (5, 0x21) for a server (0x21, 5). The spec owner picks a side
   (`hirelings.md` §13 r2 + the TSV vs `pets.md` §8 + `model.md` §14 r2).

## 3. Pending seams hit (each named in its test)

- `0x00623660` interact range (`Pending::object_in_range`): the default
  answers "out of range", so on a host with the default `Pending` (e.g.
  `test_fixtures::game::Seams`) **every** object operate stops at §7.1
  r3 silently. The test's `Night` pending answers "in range".
- Chest drop `D(Q)` `0x00585B90` and code drops (`ChestWorld` defaults):
  no item, nothing reported.
- Shrine effects, hover (`ShrineWorld` defaults): the code-2 effect
  changes nothing, no hover / event 6.
- Quest `create_portal` (Tristram portal timer): refused; the timer keeps
  running.
- Quest `unit_position` (see N-2), `open_quest_message` (0x27 type 2,
  `quests-act2.md` OQ4), `spawn_monster` / `free_spot_at` (Cain).
- Hire unit creation `0x005B23C0` (`NpcRest::spawn_mercenary`): 0x2A
  code 15 with the gold taken (spec'd outcome).
- Hireling warp `0x00574CC0` move part (`HirelingRest::warp_to`): the
  hireling keeps its position; only the flag is set.
- Monster DT event-1 function `0x005A72B0` (no body; `units.md` §4.6
  mode table names it only): the corpse stays in mode 0, never 12.
- The death start body (monster spec): the shared fixture's (mode DT and
  the drop it is known to call), as in `e2e_full_loop.rs`.
- The client pass's per-unit update `0x0053A500` (HANDOFF IS2): no 0x69
  code 8 / 9 (`intents-events.md` §7.4 r7) and no 0x9C for the drop
  (§7.6). The spec covers them; the code sends nothing. The kill test
  asserts the empty transcript and says it must change when IS2 lands.

## 4. Findings

- **N-1 (fixture)**: `e2e_support/world.rs`'s `TestPending` answers
  positions, steps and crossed sub-tiles itself (built for runs without
  the path provider), and the bench `combat_tables()` has blank
  `monstats2` / `charstats`: with the path provider on, a missile never
  collides and the player cannot walk. `e2e_night_world.rs` therefore
  copies `e2e_full_loop.rs`'s path-provider seams and combat tables.
  Not a product bug; a candidate for `e2e_support` (one path-provider
  `TestPending` shared by both e2e files).
- **N-2 (wiring gap)**: `HostQuests` (`wiring/economy/quest_host.rs`)
  forwards `unit_position` to the rest instead of answering it from the
  object's path / static path, so the gibbet's event 7 (`quests-act1-rest.md`
  §1.2 step 3) stops with `QuestError::Fatal(0x00593290)` on the quest
  control before Cain's spawn, and Horazon's journal leaves +0x08 (the
  tome's room) unset. The test asserts the fatal report. Fix: answer
  `unit_position` (and `room_at` / `room_contains`) in `HostQuests` from
  the path provider, like `unit_level`.
- **N-3 (timing, known design item)**: quest object inits and object
  event 7 run in the host's post-tick drain (`wire-world-staging.md` §3
  item 1). On the wire this is one tick late: a stone init's mode 2 and
  the gibbet's event-7 mode 3 reach the client a tick after 1.14d would
  send them (`objects.md` §3: the init runs inside the allocation;
  `tick.md` §3: timer events (step 4) before the client pass (step 5)).
  The tests collect both ticks and do not assert the tick; C78 / C79 in
  HANDOFF §5 would show it. Fix path as in `wire-world-staging.md` §3
  item 1 (lend the quest control into the action wiring).
- **N-4 (state)**: on a new game `QuestControl::new` gives chain 4
  not-intro 1, so the stone init takes `quests-act1-rest.md` §2.2 step
  2 (nothing); `wire-world-staging`'s unit test stages not-intro 0 to
  reach step 3. Both are now covered.
- I-1 confirmed on the wire (§2 item 9).

No spec text was changed; no test was removed or weakened.

## 5. Gate (this branch)

- `CARGO_INCREMENTAL=0 cargo test --workspace --no-fail-fast`: 5,322 passed,
  0 failed, 219 ignored (the d2-client build needs
  `tools/cloud-setup.sh`'s packages).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
  `cargo fmt --all --check`: clean.
- `python3 tools/coverage.py --check`: 8,290 claims, 0 errors.
  `python3 tools/spec_index.py --check`: ok. `tools/methods.py check`: ok.

## 6. Next steps

1. PC 1 settles I-1; then remove the `#[ignore]` and fix the losing side.
2. N-2: answer the quest position seams on the wired host; then the
   gibbet test asserts §1.2 step 4 (Cain's spawn through the monster
   seam, or its failure path) instead of the fatal report.
3. IS2 (the per-unit update): turn the kill test's empty-transcript
   assertion into the §7.4 r7 0x69 code 8 / 9 bytes and the §7.6 0x9C.
4. N-1: one path-provider `TestPending` in `e2e_support`, used by
   `e2e_full_loop.rs` and `e2e_night_world.rs`.
