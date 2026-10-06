# Handoff: world systems (quests, waypoints, cube) — `claude/impl-world`

Cloud implementation session, 2026-10-06, branched from
`claude/bold-ptolemy-jvyvxy` at `4c7a7c7`. Specs: `specs/world/quests.md`
(+ `quests.tsv`, `quest-messages.tsv`), `world/waypoints.md` (+
`waypoints.tsv`), `world/cube.md` (+ `cube-ops.tsv`). The coordinator
merges this file into `docs/HANDOFF.md` / `docs/PLAN.md`.

## State

**Implemented, unverified** (the specs are drafts with queued
confirmations; no game-file or trace check has run on this code).

- `cargo test -p d2-sim`: 134 tests (78 new in `world`): every synthetic
  test vector of the three specs, the recorded message bytes they quote
  (0x63, 0x0D, 0x5E/0x28/0x29 at game entry, 0x5D after Akara's message
  64), edge cases, and TSV checks with perturbation tests (M08).
- Gate run before the push: `cargo fmt --all -- --check`,
  `cargo clippy -p d2-sim --all-targets -- -D warnings`,
  `cargo test -p d2-sim`, `cargo run -p depcheck`,
  `python3 tools/spec_index.py --check`, `python3 tools/methods.py check`:
  all pass.
- Files changed outside `crates/d2-sim/src/world/`: one line in
  `crates/d2-sim/src/lib.rs` (`pub mod world;`) and this file. No
  dependency, spec, `tick`, `units`, `game.rs` or `rng.rs` change; no
  public signature of another module changed.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/world/mod.rs` | module root; strict TSV row/number parser shared by the three modules (`TsvError`) | METHODS M05, M07 |
| `crates/d2-sim/src/world/waypoints.rs` (+ `waypoints/tests.rs`) | `WaypointMap` (index ↔ level from `levels`), `is_town`, `tile_code`, `WaypointRecord` (test/set/allocate/load copy/out copy), `WaypointRecords`, save section `write_section`/`read_section`, `WaypointData` (`init_object` = init 17, `operate` = operate 23, `validate` = `0x00549570`, `take_or_close` = C→S 0x49, `travel`), `ArrivalList`, `menu_message` (0x63), `arrival_message` (0x0D) | `world/waypoints.md` |
| `crates/d2-sim/src/world/cube.rs` (+ `cube/tests.rs`) | `Recipe::decode` / `recipes` (typed `Cubemain` + callback slot bytes), `CubeData` (recipes, item records of weapons+armor+misc, `ValShift`, max level), `op_info`, `transmute`, `type_pick`, `ratio`, `click_button` (0x4F 0x17/0x18), `open` (cube use), `put_in` (C→S 0x2A) | `world/cube.md` |
| `crates/d2-sim/src/world/quests.rs` | `QuestFlags`, `PlayerQuests`, `QuestChain`, `GuidList`, `QuestRecord`, `QuestControl` (creation, `player_enters`, dispatch to all / along a chain, kill parse, player leave, updater + timers, default status rule, 0x40/0x5D/0x8A/0x89/0x31/0x58, act completion, Durance warp), `send_player_flags` (0x28), `npc_gossip` (0x91), `grant_pending`, `warp_check`, `portal_check`, `cow_portal`, `read_clue`, `object_event` | `world/quests.md` §1–§9 |
| `crates/d2-sim/src/world/quests/tables.rs` | `quests.tsv` / `quest-messages.tsv` embedded (`include_str!`) and parsed strictly (`QuestTables::load`) | `world/quests.md` §2.4, §7.1 |
| `crates/d2-sim/src/world/quests/act1.rs` | Act I callbacks: A1Q0, Den of Evil (start, chat end, area, kill, reward, status timer), Sisters' Burial Grounds, Tools of the Trade (Malus, message 163, imbue grant), Search for Cain (97/112/118, stone order, Wirt's body, Cow King), Forgotten Tower (127, 140–145), Sisters to the Slaughter (166, Andariel gems, 183, 179/181/184), Flavie (both records), respec flags, sequence functions of chains 1–6 | `world/quests.md` §10 |
| `crates/d2-sim/src/world/*/tests.rs` | fakes of the three seams; vectors | Test vectors of each spec |

## How the machine tables are consumed (M05/M17)

- `quests.tsv`, `quest-messages.tsv`: describe hard-coded `Game.exe`
  tables, so they are the data. Embedded with `include_str!` and parsed
  strictly at `QuestControl::new` time (as `d2-data::schema` does with
  `fields.tsv`). Test `tables_parse_and_check` runs structural checks
  (row/index shape, unique chains and flag slots, filters, every message
  table referenced and existing, ≤ 16 entries per state);
  `tables_check_catches_perturbations` proves they fail on a changed cell.
- `waypoints.tsv`: expectation only, never read by code. Test
  `tsv_matches_town_and_tile_rules` derives the rows from a levels table
  holding the TSV's (index, level, act) and checks the `town` and
  `tile_calc` columns against `is_town` / `tile_code`; the live-data
  comparison is queued (below). Perturbation test included.
- `cube-ops.tsv`: test `ops_match_tsv` checks every row (scope, source,
  pass rule) against `op_info`; `ops_check_catches_perturbations`.
- Recipes, levels, objects, items, `itemstatcost`, `experience` come from
  `d2_data::tables` records (`WaypointData::new`, `CubeData::new`,
  `cube::recipes`); nothing is hard-coded from game data.

## Seams (trait methods → expected provider)

Every seam is a trait in its module; tests use small fakes.

`waypoints::WaypointWorld`:
- `frame` → `Game::frame`; `difficulty` → game +0x6D (game creation).
- `records(player)` → player data +0x1C (units group: embed
  `WaypointRecords` in player data; load via `read_section`).
- `object(guid)`, `player(unit)`, `room_rect(room)` → unit lists / units
  / DRLG (static path position, room level, room rectangle).
- `set_object_mode` → objects spec `0x00624690` (incl. the anim-setup
  draw for `Sync` = 0); `schedule_endanim` → `Game::schedule_event`
  (event type 1, object timer class).
- `player_busy`, `set_interact`, `reset_interact`, `interact_guid` →
  interaction (units group).
- `hostile_delay` → host clock (`d2-server`), false in single player.
- `attach_sound` → units (`0x00553380`); `send` → `d2-server` transport.
- `warp`, `spawn_room` → DRLG / level change (`0x0053AEC0`,
  `0x00619E50`); `set_player_mode_arrival` → player modes.
- The caller keeps one `ArrivalList` per game (object control).

`cube::CubeWorld`:
- game fields (expansion, game type, ladder, difficulty, item format,
  game seed) → game creation; `local_date` → host input.
- `player_class`, `stat`, `set_stat`, `attach_sound`, `send` → units /
  stats spec / transport.
- interaction and stash/trade tests → interaction / trade owner.
- items: `inventory` (list order, open question 5), `item_by_guid`,
  page/mode/class/quality/file index/level/flags/sockets/max sockets,
  `class_is_type`, `duplicate`, `item_init`, `create_item`
  (`ItemRequest`), `tempered_affix`/`set_tempered`, unique bitset,
  `drop_runeword_stats`, `add_craft_property`, `repair`, `recharge`,
  `place`, `free_item`, `remove_cube_item` (0x9D + free),
  `targeting_reset`, `put_item_check`, `cube_check` → items group
  (creation, properties) and the inventory owner.
- `quest_item_hook` → Act II/III quest hooks (`0x0059E5C0`,
  `0x005B86E0`; not specified yet).
- `cow_portal` → `world::quests::cow_portal` (the provider wires the
  game's `QuestControl` and `QuestWorld` into it).

`quests::QuestWorld`:
- game fields; `players` (`unit-order.md` §7 order), first client,
  GUIDs, `quests(player)` (player data: embed `PlayerQuests`; load with
  `QuestFlags::copy_in(buf, true)`), unit act/level, class, unit seed,
  stats, sounds, player data +0x4C, `quest_chain(unit)` (unit +0x74:
  embed `QuestChain`), `unit_kind` (player / monster with class,
  superunique hcIdx, owner), monster lookups, `players_near` → units
  group.
- `send`, `send_text_list` (0x27 from `world/npc.md`) → transport / NPC.
- items: `has_item`, `delete_item` (`0x00544160`), `reward_item`
  (`0x005466B0`, §9.1), `drop_item_at` (`0x00559A30`), `quest_items` →
  items group.
- `den_region`, `true_tomb_level`, `free_spot`, `create_portal`,
  `schedule_quest_event` (object event 7), `set_object_opened`,
  `mercenary_reward` → DRLG / objects / NPC spec.
- `unhandled(chain, function)`: a function the spec names but does not
  specify was reached (the host logs it). Callers that need tick step 8
  call `QuestControl::update` when frame % 20 = 0 (`tick.md` §3).

## Open questions and narrowest readings (TODO in code)

Each has a `TODO(<spec> …)` comment at the site.

Waypoints:
- W1 §6.2 step 2 with a player or object without a room: compared as "no
  act".

Cube:
- C1 §6.4 `exc`/`eli` capture upgrade: read as `if exc {…} else if eli
  {…}`; the sentence also admits "eli when the exc upgrade fails". No live
  record sets both.
- C2 §7.3 a `mod` or `useitem` output with no captured item: nothing made
  (the original dereferences null).
- C3 §7.3 tempered rolls when the duplicate failed: skipped.
- C4 §7 more than 18 socket fillers: extra ones dropped.
- C5 §7.5 type pick with 0 items: returns 0 without a draw.
- Spec open questions 1, 2, 4–8 untouched (message bytes of removal /
  placement, sound message, draws of owners, inventory order, full-cube
  0x2A, cube-use table, `0x0055FA40`).

Quests:
- Q1 §2.3 the active/state bytes the init functions store: only A1Q1's
  (active 1, state 1) is in the spec; all others start 0. Row 40 filter
  (OQ6) taken as 42.
- Q2 §10.1 sequence functions (chains 1–6): raise the `seq_id` quest's
  state 0 → 1, never lower; chains 8, 18, 22, 31 are `unhandled`.
- Q3 §10.1 chat end sends status 1 only when the start message was given
  since the last chat end (A1Q1 +0x86 read as that flag; applied to
  chains 1, 2, 3, 4, 6).
- Q4 §10.1 event 3: bits 3/4 set on the moving player when it has bit 2
  and lacks 0/1; A1Q2 area level 17 is D2MOO's.
- Q5 OQ7 per-quest iterates (`0x00590190`, `0x00590080`, `0x005900E0`,
  A1Q3 party bits, A1Q6 iterates): `unhandled`; `set_status_all` sends
  0x5D to every player.
- Q6 §6.3 a status function returning false: 0x5D carries the record's
  status byte.
- Q7 OQ10 0x61 byte and intro-flag act: D2MOO's (2, 3, 5; acts I, II,
  II, III); Tyrael's 0x5D before 0x61 is `unhandled`.
- Q8 §8.4 the refusal sound is also played when the spot search or the
  portal creation fails.
- Q9 §10.3 respec done: "a record's active byte" read as chain 30's.
- Q10 §10.5/§10.7/§10.8 unspecified: A1Q2 timer 15, A1Q5 levels and
  kill, A1Q6 timer 1, Malus refusal sound, event-0 text of chains 1–6 and
  the intros, Act II–V callbacks, status and active functions: all
  `unhandled`, no timer created.
- Q11 OQ5 the stone-order 0x50 layout: the order is computed (and
  cached) on the quest seed; the message is `unhandled`.
- Q12 OQ4 which entry mode single player uses: the caller passes it.

## Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. **Waypoints derivation on live data** (needs a home: `d2-sim` cannot
   load MPQs without a new dependency; add to `data-tool` or
   `crates/conformance`): build `WaypointMap::new(&levels)` from the
   loaded `levels` table and compare `rows()` with `specs/world/waypoints.tsv`
   (all 39 rows equal); `WaypointData::new(..).waypoint_classes()` =
   the 16 classes of `waypoints.md` §5 rule 1.
2. **Cube recipes on live data**: `cube::recipes(cubemain)` on the 1.14d
   set: 151 records, the live facts of `cube.md` Test vectors (enabled
   146, ladder 21, version 100 on 96, op 28 on records 0, 1, 2, 148, 149,
   output kinds 0xFC 78 / 0xFF 48 / 0xFE 21 / 0xFD 1 / portals 1+1+1, 133
   mods all chance 0); then V1–V23 with a real items table.
3. **Recordings** already queued by the specs (waypoints OQ1–3, cube OQ1–4,
   quests OQ2, OQ4, OQ5, OQ10) decide the TODOs above; replay their
   packets against these modules once the seams have providers.

## For the integrator

- New public module `d2_sim::world` with `cube`, `quests`, `waypoints`.
- Types other groups should embed: `waypoints::WaypointRecords` (player
  data), `waypoints::ArrivalList` (game), `quests::PlayerQuests` (player
  data), `quests::QuestChain` (every unit), `quests::QuestControl`
  (game, created after the game seed with `QuestTables::load()`).
- `QuestControl::new` steps the game seed once (the fifth seed derived
  at game creation, `rng.md` §5.2): call it in that position.
