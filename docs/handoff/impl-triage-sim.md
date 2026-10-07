# impl-triage-sim: d2-sim triage rows (skills, monsters/AI, drlg, path/units/action wiring, missiles)

Base `claude/specs-staging-6` @ 740449b2; branch `claude/impl-triage-sim`.
Rows from `docs/handoff/todo-spec-triage.md` part 1 ("sim: skills / bodies",
"sim: monsters / AI", "sim: drlg / worldgen", "sim: path / units / action
wiring", "sim: missiles") and part 2 NOT SPECIFIED rows in those areas
(implemented as PROVISIONAL per M22).

`grep -rn "TODO(spec" crates/d2-sim/src/{skills,monsters,drlg,missiles,wiring/path,wiring/action,units}`
now gives only `wiring/path/walk.rs:79` (PENDING-BY-DESIGN, left as is).

## What landed

### drlg / worldgen
- `drlg/outdoor/jungle.rs`: T row 15 reads four zeros → fatal 0x78C (§2.7 table).
- `drlg/room.rs` + `level.rs`: the client build cursor is a three-state
  `BuildCursor` (none / head node / room); the status-2 head reads status 2
  and is kept (rooms.md §4.6 rules 6–9).
- `monsters/init/umods.rs` (code differed): the umod exclusion asks
  `montype_is(exclude, MonType)` (row = exclude type, column = MonType,
  init.md §17.3 r2 vectors); `wiring/worldgen/init_units.rs` marker removed.

### path / units / action wiring
- `wiring/path/units.rs:128`: removal clears with force (path-placement §5.3 r4); comment only.
- `wiring/path/units.rs:311`: `set0x10` = 0 stated; monster calls PROVISIONAL.
- `wiring/path/walk.rs` used skill: `skill_flags` = the used skill entry's E-flags (pathing.md §8.1), was 0.
- `wiring/action/waypoints.rs`: `0x00554D00` documented (objects-2.md §16.3).
- `wiring/action/switch.rs`: overhead 0x26 form 5 with the +0xA4 text and the
  `0x0055B300` relation skip (§7.9 r3; hooks `Pending::overhead_record`,
  `player_relation`); class 59 sends 0x82 (`Pending::portal_owner`).
- `wiring/action/monster_add.rs` (new): monster add messages (§7.2): 0xAC in the
  full layout of init.md §24, stat 328, 0x98 (class 528), 0x21 skill messages
  (`Pending::monster_add_skills`), 0xAA, part B mode message (needs the path
  provider) and inventory (`Pending::inventory_messages`). Sent by the room
  switch and, for a not-yet-announced monster (flag 0x10), by
  `monster_update` (§7.1 r2.1).
- `wiring/action/unit_update.rs`: room clean-up runs §7.5 steps 1 (mod array),
  4 (state-changed bits), 6 (overlay removal ×2), 7 (lastexp := −1, monster
  data +0x5C bit 0 via new `MonsterWorld::monster_mut`, client part via
  `Pending::client_cleanup`). Step 5 / item flags stay with the server's item pass.
  Death mode: always `0x00553570` (units.md §4.6), comment only.
- `wiring/path/monsters.rs:94`: AI requests can override the request path byte
  (`AiModes::change_mode_path_byte`, `PathState::mode_request_byte`); the
  Ancient whirlwind (bodies5.rs, ai-bodies-5 §12) sends 100 = no path.
- `wiring/action/pending.rs` layer split: from itemstatcost record 0 `stuff`
  (`BodyTables::layer_split`, runtime-maps.md §3); Pending default (6, 0x3F) (was (0, 0)).
- `wiring/action/vitals_sync.rs`: cache starts 0; non-player client unit is a
  debug assert; `0x0052DA00` not run in Original.

### missiles
- `create.rs` signed compare, `mod.rs` collide table, `bodies.rs` area hit
  (attacker = owner, evade no bit, body-4 len): comments only (code matched).
- `hit.rs`: missing record is a debug assert (§R5 step 1).

### skills / bodies (worker, merged)
levels.rs hit class stored; throw mastery 0 off the throw gate (code differed);
summon mode 0 when R invalid; negative mastery / negative burst step debug
asserts; Vengeance C remainder; Strafe rewinds only with K2 (code differed);
Baal Tentacle spawn info for every key (+ `BodyWorld::class_for_level`);
Imp Teleport point (code differed); FetishAura finder from the caster's room;
`combat/events.rs` live-record iteration (code differed); comment-only rows.

### monsters / AI (worker, merged)
T and point: T wins (hydra, Summoner); G (Npc walk counter) is a host input
(`AiStore::with_npc_walk_counter`; d2-server does not carry it across games yet);
T = 0 rows assert; npc.rs other classes take cain1's Act 1 functions (code
differed); AI state toggle `0x00639DB0` queues the unit; command ring §8
wrap/allocator; bodies7 null owner debug assert; population seams turned into
plain code (owner data, group spawn, barricade objects 571/572);
`init/message.rs` component presence bit.

## Changed test expectations
- `drlg::tests::rooms::client_build_timer_builds_status_2_rooms_one_per_run_out`:
  cursor values now `BuildCursor`; after calls 11–15 the cursor is the head
  node, was `Some(r[0])` (rooms.md §4.6 r9). Rule 9 B = 1 case added.
- `prop_walk_motion::chase_a_moving_target` (flaky on the base too): a monster
  re-path while moving may end type 15, was "13 only" (pathing.md §9.10: type 13
  compute gives 0 → type 15 and compute again).
- `skills::tests::add_element_by_etype`: final `hit_class` 0 → 0x30 (levels.md §3.6).
- `bodies::tests2::summon_class_from_the_record_or_the_ai_control`: (−1, 3) → (−1, 0), (1, 3) → (1, 0) (bodies-3.md §2 a3).
- `monsters::ai::tests::act3::high_priest_vectors`: hydra `point_mode(10, T ± off)` → `unit_mode(10, player)` (ai.md §7.1 OQ13).
- `monsters::ai::tests::npc::npc_out_of_town_portal_setup`: other class → Cain setup (ai-bodies.md §9.32).
- `monsters::population::tests::superunique_presets`, `monsters::mutant_tests::population::superunique_hcidx_spawns`:
  log lines (`group`, `barricade`, `suowner`) → real units / objects / owner data (population.md OQ3, OQ4).

- `missiles::tests::flight_without_record_is_an_expiry_hit`: now `should_panic` in debug
  builds (§R5 step 1: a missing record is unreachable and asserted); release unchanged.
- `d2-client` `e2e_full_loop` (3 tests): the join now carries each monster's add
  messages (0xAC, 0xAA, 0x6D; intents-events.md §7.2) after the waypoint's 0x51;
  client log handled 20 → 24, dropped gains 0x6D ×1 (the client creates nothing
  from 0xAC yet: no `ClientTables` monster rows), queued 0 → 1 (Akara's 0x6D).
- Coverage claims of the skills worker's new tests reformatted (`;` between specs).

## PROVISIONAL list (code: `// PROVISIONAL (<spec ref>)`)
RNG-order / wire / saved bytes first (HIGH PRIORITY captures):
1. monsters/ai.md §2.3 can-walk = collision test only (target.rs) — wander-draw recording. RNG.
2. ai-bodies-6.md §2 pet move k 0 "else … midpoint" scope (bodies6.rs) — pet-move recording. RNG.
3. ai-bodies-6.md §24.1 step 7 pet follow always evaluated — DruidWolf recording. RNG.
4. ai-bodies-7.md §27 step 12 aitype 1 "lacks it" ends case; aitype 12 non-progressive rule — Shadow Master recording. RNG.
5. population.md §11.4 hcIdx 10 roll(5) on the boss seed, mode 1 — Radament spawn recording. RNG.
6. population.md §6.3 creation mode 1 — bin read 0x005A09E0 (spawn state).
7. sim/units.md §4.5 rest of 0x00580EC0 / save in 0x0057FCA0 not run — save capture on death. Saved bytes.
8. intents-events.md §7.2 0x98 u16@5 = 0 — class-528 add capture. Wire.
9. intents-events.md §7.2 / §6 r6 0x82 u32@21 = portal GUID, u32@25 = pair (−1 none) — town-portal join capture. Wire.
Others (bin reads):
- ai.md §1.4 SplEndGeneric no neutral first (mod.rs); ai-bodies-4 §8 Diablo X = 0 → (0,0);
  ai-bodies-6 §13 step 5 (Skill1, T, own x, y); §25 step 4 highest entry with bonus;
  ai-bodies-7 §18 init / §19 step 2 / §27 init and init 143 highest entry with bonus;
  §18 pettype ≠ 0xFF; §27 step 8 independent tests.
- population.md §6.3 r4 nearest free point's room; §10.3 owner data unconditional;
  §11.5 unlisted special id nothing; hcIdx 60 boss owner data (GUID, 1, 0, 0);
  §4 / §3.1 null region nothing (unreachable).
- drlg/maze.md §6 unlisted maze type draws r, stamps nothing.
- sim/units.md §2 interact info allocated as reset (−1, 6, inactive).
- combat/vitals.md §4.7 no corpse → +0x508 unchanged.
- skills/bodies.md §2.4 stale target → path point; bodies-2 §2.3 negative n unreachable;
  bodies-3 §3.8 sub-tile coordinates.
- missiles.md §R2.3 step 16 town access 0; ai.md §7.1 set type + town access 0;
  units.md §4.6 walk/run start = mode set started; pathing.md §9.1 no gate.
- path-placement.md §2.5 monster calls after allocation not run.

Not done (M22 spec side): the matching `PROVISIONAL:` lines in the spec files.

## Rows left
- `wiring/path/walk.rs:79`: PENDING-BY-DESIGN (not in scope).
- Out of my areas (other sessions): `world/environment.rs:10` (tick/lighting),
  d2-server `handlers/player.rs` rows, items/inventory/economy rows.
- Hooks with no provider yet (default nothing): `Pending::overhead_record`,
  `player_relation`, `portal_owner`, `monster_add_skills`, `inventory_messages`,
  `hireling_owner_guid`, `unit_owner`, `client_cleanup`; d2-server does not yet
  carry the AI walk counter G across games.
