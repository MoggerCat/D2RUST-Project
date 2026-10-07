# impl-final-sim: maze extras, AI/population rows from the final specs, hook providers, server environment advance

Base `claude/specs-staging-7` @ 431a3fd7; branch `claude/impl-final-sim`.

## What landed

### 1. drlg/maze §6 extras (`drlg/maze/layout.rs`)
The builders test level ids, as `specs/drlg/maze.md` §6 states: cave
(cave_prev always; cave_doe if 8 else cave_down; +coldcrow 9; +next 10),
crypt (crypt_prev always), catacombs (catacombs_next always), dungeon and
Act 3 sewer (both stamps unconditionally), Baal (baal_next always). So
13–16 get cave_prev, cave_down; 25 crypt_prev; 37 catacombs_next; 90, 91
dungeon_prev, dungeon_next; 93 a3sewer_drain, a3sewer_chest; 132
baal_next. The "unlisted maze type draws r, stamps nothing" PROVISIONAL is
gone (an unlisted type is already the fatal `BadLevelType`). New test
`builders_stamp_the_unlisted_levels_of_their_type`.

### 2. AI / population rows (worker; `monsters/ai/*`, `monsters/population/*`)
Behaviour changed to the spec:
- ai.md §1.4: the `SplEndGeneric` test runs before the walk/run table (a
  matching class thinks inline, no neutral first).
- ai.md §2.3: target modes 4/5 wander only when the cell collides **and**
  the class can walk; else idle 20, no draw.
- ai-bodies-6 §2 pet move k 0: the midpoint is tried only when Q is in the
  owner's coordinate area and Q's walk did not start (no second velocity
  call); Q in another area tries nothing.
- ai-bodies-6 §13 step 5: the second skill check is at T's position.
- ai-bodies-7 §18 Allowed: `pettype` read as a signed byte, valid when
  0 ≤ it < the pettype row count (new seam `AiHost::pettype_count`, default
  0; no wiring overrides `pet_type_of` yet, so live behaviour is unchanged).
- population.md §6.3 r4: the nearest free point's **room** is used (the code
  searched in the original room).
- population.md §11.4 hcIdx 60 / init.md §20.1: boss owner data
  `(own GUID, 1, 1, 0)` (was `(GUID, 1, 0, 0)`).
Comment-only (code already matched; PROVISIONAL removed): ai-bodies-6
§24.1 step 7, §25 step 4; ai-bodies-7 §18 init, §19 step 2, §27 init and
init 143; population.md §3.1 / §4 null region.

### 3. Pending hook providers
- `overhead_record`: real provider. `View::replace_overhead` (called by the
  d2-server C→S 0x14 handler) sets unit +0xA4's frame and keeps the record
  in `SessionState::overheads`; the overhead 0x26 form 5 (§7.9 r3) sends it.
- AI walk counter G: `d2_server::adapters::handlers::world::ProcessState`
  (one per server process) hands G to each new game's `AiStore`
  (`start_game`) and reads it back (`end_game`). Nothing in d2-client calls
  it yet (single player runs one game per process today); the client
  session should call `start_game` / `end_game` around each game.
- No provider (the data has no model in the wired host; defaults kept,
  reasons):
  - `player_relation`: no party/hostility model anywhere in d2rs (no
    spec of `0x0055B300`'s state); "no relation" is exact for one player.
  - `portal_owner`: no class-59 portal creation in the sim
    (`create_portal` is a default no-op), so no owner data exists.
  - `monster_add_skills`: no unit skill-list model (the AI's own skill
    queries are `Pending` too).
  - `inventory_messages`: items/inventory area (impl-triage-items).
  - `hireling_owner_guid`: the hireling lists live in the interaction
    layer (`InteractionHooks::hirelings`), not reachable from the action
    layer's monster add; no hireling sets an AI minion owner.
  - `unit_owner`: units carry no +0x94/+0x98 fields (the `SourceFields`
    body effect goes to `Pending::body_effect`).
  - `client_cleanup`: no client sub-record (+0x34) model.

### 4. Server environment advance and handlers/player.rs
- `world/environment.rs`: the record keeps type (+0x04), speed (+0x28) and
  last reported hour (+0x34); `advance` = render/lighting.md §9.3 r1–r3,
  `server_advance` = r5 (report every 17 degrees or on index/type change).
  Tainted Sun start/end now also set speed (4 / 128) and type.
- `ActionSim` tick step 1 (`sim/tick.md` §3): advances each act **a join
  has built** (`ActEntry::built`, set by the join's rule 4, which creates
  a fresh record: in 1.14d the act slot is empty until then, §8.2 r4) and,
  on a report, sends 0x53 to the act's in-game clients after
  `Pending::environment_refresh_items` (`0x0055FDE0`, default nothing).
  Recorded vector reproduced: first report `53 02000000 80080000 00` with
  +0x34 = 17.
- `handlers/player.rs`: §9 r6 (`0x0053FDF0` is `return 0`) and §9 r8
  (params 3, 4 stored 0, 0) are now stated; §8.2 r3.6 hot keys
  `PROVISIONAL (…; REC-02)`; `player/action.rs` pathing §1.2 note is a d2rs
  wiring choice (marker removed).

## Changed test expectations
- `drlg::maze::mutant_tests::special_stamps_by_level`: level 93 `&[]` →
  `a3sewer_drain, a3sewer_chest` (maze.md §6); cases 13, 16, 25, 37, 90,
  91, 132 added.
- `monsters::ai::tests::act6::pet_move_ahead_and_catch_up`: with Q in
  another coordinate area the first walk was the midpoint (115,104), now
  the next direction's Q (122,108) (ai-bodies-6 §2); a case with Q's walk
  failing then the midpoint walk was added.
- `monsters::mutant_tests::population::superunique_hcidx_spawns`: hcIdx 60
  owner data `1 0 0` → `1 1 0` (init.md §20.1), now logged twice (§6.3
  step 5, then the hcIdx call).
- `world::environment::tests::created_record_sends_the_recorded_join_message`
  and the Tainted Sun test: struct literals gained the new fields (same
  values asserted).

## PROVISIONAL list (in the files touched here)
- ai-bodies-7 §27 step 8 (REC-80); §27 step 12 aitype 1 and non-progressive
  aitype 12 (REC-82).
- population.md §6.3 creation mode 1, §6.3 r4, §10.3 owner data, §11.5 r6
  unlisted id (REC-80); §11.4 hcIdx 10 Radament roll (REC-81).
- intents-events.md §8.2 r3.6 new-character hot keys (REC-02).
- Untouched and still provisional: `bodies4.rs` ai-bodies-4 §8 (no REC id in
  the spec), the 0x98 / 0x82 wire bytes in `monster_add.rs` / `switch.rs`.

## Left / notes
- Spec mismatch: hcIdx 10 is settled in `monsters/init.md` §20.1 but still
  PROVISIONAL (REC-81) in `population.md` §11.4; the code agrees with both.
- Init 143 (ai-bodies-7 §27) applies init 106's "O a player" condition: a
  reading, not stated text.
- `ai/mod.rs` `TODO(monsters/init.md)` (no control at install) remains.
- The hooks without a data model above.
- `prop_walk_motion::chase_a_moving_target` flake: see below.
