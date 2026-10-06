# Handoff: economy wiring (`d2_sim::wiring::economy`)

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Branch `claude/wire-economy`, from
`claude/bold-ptolemy-jvyvxy` at `ed7236e`. Repo only. For the
coordinator to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (not edited
here).

## 1. State

**Wired, unverified**: every spec involved is a draft. Its checks are
already queued by the module handoffs (`impl-items.md`,
`impl-treasure.md`, `impl-world.md`, `impl-units-stats.md`).

- New module `crates/d2-sim/src/wiring/economy/`, one file per seam pair,
  plus `wiring/mod.rs` and one line `pub mod wiring;` in `lib.rs`.
- **No module code changed.** No seam signature turned out wrong for the
  real provider, so this list is empty. The two places where a seam
  needed something it does not carry are handled in the adapter (see §4).
- Tests: 17 integration tests in `wiring/economy/tests/` run the real
  modules together on synthetic tables with the fixed game seed `0x5EED`.
  The only fakes are the seams this wiring leaves open. All earlier
  per-module fake tests still pass (`cargo test -p d2-sim`: 512 pass, 3
  ignored).
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
  --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run -p
  depcheck`, `python3 tools/spec_index.py --check`, `python3
  tools/methods.py check`, `python3 tools/coverage.py --check`.

**Merge note.** The action-wiring session also creates
`crates/d2-sim/src/wiring/mod.rs` and a `pub mod wiring;` line. The merge
keeps both `pub mod action;` and `pub mod economy;`.

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `wiring/economy/mod.rs` | module root, `EconomyError` | |
| `wiring/economy/game_fields.rs` | `GameFields` (game seed, +0x6D, +0x70, +0x6A, +0x74, +0x1B24) implements `items::ItemGame`; `treasure_facts` → `treasure::GameFacts` | `items/generation.md` Inputs, `quality.md` §8, `treasure.md` Inputs |
| `wiring/economy/item_stats.rs` | `StatCtx`, `UnitStats` implements `items::ItemStats` on `stats::StatLists`; `find_list` (list by state and flags) | `properties.md` §2, §4.2; `stat-lists.md` §4.1, §5, §8.1, §9.3 |
| `wiring/economy/item_units.rs` | `Economy` (game, units, stats, unit data, hooks, fields, item tables, `ItemStore`): `create_item` (allocator + pipeline), `free_item`, `with_item` (item put together from record + store + lists), `with_stats`, `request_unit`; `ItemStore`, `ItemSpawn` | `generation.md` §2–§3, `units.md` §3, `rng.md` §5.3 |
| `wiring/economy/treasure_items.rs` | `ItemDrops` implements `treasure::DropSink`; `DropPlacer` seam, `DropSpot`; `dropper` / `recipient` from unit records and stats; `drop_request` | `treasure.md` §5.4, §6, §7, §8 |
| `wiring/economy/cube_items.rs` | `EconomyCube` implements `world::cube::CubeWorld`; `CubeRest` (the rest) | `cube.md` §4–§7, `generation.md` §4, §7.2, §7.3, `properties.md` §12 |
| `wiring/economy/quest_items.rs` | `EconomyQuests` implements `world::quests::QuestWorld`; `QuestRest` (the rest) | `quests.md` §4.4, §4.5, §9 |
| `wiring/economy/tests/` | `mod.rs`: synthetic itemstatcost (through the d2-data fix-up), item tables, `World`, the map oracle `MapStats`; `items.rs`, `treasure.rs`, `cube.rs`, `quests.rs` | |

## 3. What is wired

| Seam (module) | Provider | Integration test |
|---|---|---|
| `items::ItemStats` | `StatLists`: unit total / base / unit set on the unit's extended list; a `ListKey` list is a plain list (state, flags) attached to the unit, created when missing | `items::unique_item_on_real_units_and_stat_lists`: a unique cap's property (mode 3) lands in the real list (state 0, flags 0x40, owner type 4). Every base stat, list stat and total equals the same pipeline run on the map oracle. Seeds, item data and the unique bit are equal too |
| `items::ItemGame` | `GameFields` | same test: the game seed steps exactly twice |
| item allocation (items handoff "allocation" seam) | `units::lifecycle::allocate` / `remove`; seeds kept in the `UnitRecord` | same test; `early_failures_allocate_nothing`, `late_failure_removes_the_unit`, `replenish_schedules_event_3` (event 3 on `Game::timers`, cancelled on free) |
| `items::RequestUnit` | `UnitRecord.class` + stat 12; name and hardcore flag from the caller | `request_unit_from_unit_fields` |
| item stats → owner | `StatLists::equip` (stat-lists §8.4) | `item_properties_reach_the_equipping_player` |
| `treasure::DropSink` (`create`, `gold`, `set_gold`) | `Economy::create_item`; stat 14 base | `monster_walk_creates_a_real_unique` (item level = monster stat 12, spawn mode 3); `unique_entry_prefers_its_row`; `gold_amount_through_real_stats` (gold base on the unit seed, ×row/256, gold find 79 of R + owner, computed independently); `walk_is_deterministic` |
| `treasure::Dropper` / `Recipient` | `dropper` / `recipient` from unit records and stats (12, 100, 79, 80) | same tests |
| `world::cube::CubeWorld`: game fields, `game_seed`, `player_class`, `stat`, `set_stat`, `item_by_guid`, `item_guid`, page / mode / class / quality / file index / level / flags, `class_is_type`, `item_sockets`, `max_sockets`, `add_sockets`, `item_seed`, `item_init`, `create_item`, `set_tempered`, unique bits, `add_craft_property`, `free_item` | `GameFields`, `UnitRecord`, `StatLists`, `ItemStore`, `items::create` / `props`, lifecycle | `transmute_makes_a_real_item_with_its_craft_property` (V12 recipe shape: ring → amulet with craft property 7; equal to a direct create + craft-list run); `cube_fields_on_real_providers` |
| `world::quests::QuestWorld`: frame, game fields, `guid`, `player_by_guid`, `player_class`, `unit_seed`, `stat`, `add_stat`, `monster_by_guid`, `monster_class`, `has_item`, `quest_items` | `Game`, `GameFields`, `UnitRecord`, `StatLists`, `ItemStore` + item tables | `tools_of_the_trade_reads_real_items` (message 163 with a real `hdm`), `den_reward_writes_real_stats` (stat 5 +1, real GUID), `malus_level_gate_reads_real_stats`, `quest_unit_fields_on_real_records` |

## 4. Design points (no module edits)

1. **Seed derivation done twice.** Both the allocator (`units.md` §3.1
   step 4) and `create_item` (`generation.md` §2.1) derive the unit seed
   and item seed from the game seed. `Economy::create_item` runs the
   allocator on a copy of the game seed and checks that both end in the
   same state (`EconomyError::SeedMismatch` otherwise). So the game seed
   steps exactly twice per item. The cleaner fix, suggested in
   `impl-items.md`, is to move the derivation out of `create_item` into
   the allocator. That is a module change and is left to an items
   session.
2. **One owner per item field.** The unit record holds the unit seed,
   init seed, item seed and start seed. `ItemStore` holds the rest of
   the item data as `Item<()>`. Stats live in `StatLists`.
   `Economy::with_item` builds an `Item<UnitStats>` from the three and
   writes the seeds back afterwards.
3. **Two units' stats at once** (`props::set_bonuses` writes the owner
   while it holds the item). `UnitStats` handles share one
   `RefCell<StatCtx>`. Host callbacks get the lists directly, so a
   borrow never nests.
4. **Game-creation fields** (seed, difficulty, expansion, +0x6A, +0x74,
   unique bits) live in `GameFields`. No written spec places them in
   `game::Game`.
5. The cube's own `world::cube::ItemRequest` is converted to
   `items::ItemRequest` per `cube.md` §7.4. The source unit is the
   player: class and stat 12 come from d2-sim, and player data comes
   from `CubeRest::player_info`.

## 5. Remaining seams and why

| Seam | Why not wired |
|---|---|
| `DropPlacer` (treasure §7 step 2: start offset, free-spot search `0x0064E810`) | collision / rooms spec not written (`treasure.md` OQ 8) |
| treasure `MonsterRank`, `quest_open`, `monster_drop_gate` inputs, party count, minion owner | monsters / quests / party specs; the caller passes them |
| `CubeRest`: interaction, inventory, sockets list, placement, removal, targeting, put / cube checks, sounds, messages, local date, player data | interaction / inventory / UI / transport specs not written |
| `CubeRest::duplicate` (`0x0055A2A0`), `drop_runeword_stats` (`0x00558C50`), `repair` (`0x0055F900`), `recharge` (`0x0055FE80`) | no items spec writes them |
| `CubeRest::tempered_affix` (`0x005C1BC0`) | open question WE6 |
| `CubeRest::quest_item_hook`, `cow_portal` | Act II/III quests not specified; `cow_portal` needs the game's `QuestControl` (a later integrator) |
| `QuestRest`: player quest records, player byte +0x4C, quest chain (+0x74), room act / level, unit kind (superunique, owner), players / first client, players near, sounds, messages, text lists, inventory, `delete_item`, DRLG / object / portal / NPC calls, `unhandled` | player data, unit +0x74, DRLG, objects, NPC and inventory specs, or no d2-sim home yet |
| `QuestRest::reward_item` (`0x005466B0`), `drop_item_at` (`0x00559A30`) | open question WE9 |
| `StatHost::item_event` (stat-lists §7.2 rule 1: `0x005C0BE0`, `0x0056E740`, `0x005C0B50`) | not in the items specs; default no-op |
| `UnitHooks::item_replenish` (`units.md` §6.5 event 3 handler) | the value / maximum stat per replenish stat and whether each stat reschedules are not written; `items::replenish_timer` covers only the start |
| `LifecycleHooks::init_kind` / `free_kind` for items (`0x00623520`) | not specified |
| item-ratio row duplicated (`items::quality::ratio_row` vs `treasure::ratio_row`) | merging them is a module change; both still run their own tests |
| `world::waypoints::WaypointWorld` | not an economy seam (DRLG, units, interaction) |

## 6. Open questions (each has a `TODO` at the site)

- **WE1** (`item_stats.rs` `find_list`, `stat-lists.md` §9.3): "by
  state and flags `0x006257D0`" has no rule. Read as the by-flags query
  (the parked chain when 0x2000 is asked, else the active chain): the
  first list whose state equals the key's and whose flags hold every
  asked bit.
- **WE2** (`item_stats.rs` `list_for`, `properties.md` §4.2 "created if
  missing"): the creation arguments are not written. Read as: the key's
  flags, expire 0, the unit list's owner type and GUID, state := key
  state, attach with reset 1.
- **WE3** (`item_units.rs`, `units.md` §3.1 step 8): "flags bit 1" is
  read as the value 0x1. Drop and cube requests pass init flags 1, and
  their items are added.
- **WE4** (`item_units.rs`, `units.md` §6.5): the arguments of the
  replenish event 3 are not written. Scheduled with (0, 0).
- **WE5** (`treasure_items.rs`, `treasure.md` §7 step 4): the drop
  request's source unit (offset 0x00) is not listed. Left none, so drops
  have no request unit (this affects `generation.md` §6.1 class skill
  mods).
- **WE6** (`cube_items.rs`): `affixes.md` names `0x005C1BC0` the
  tempered routine (§9: two §5 rare-name picks, no arguments beyond the
  item). `cube.md` §7.3 calls `0x005C1BC0(item, prefix)` once per side.
  The two readings disagree, so it is not wired.
- **WE7** (`cube_items.rs`, `cube.md` §7.6 step 3): `0x00660240`'s
  expansion argument has no rule in `properties.md` §12. Not passed.
- **WE8** (`quest_items.rs`, `quests.md` §9.2 `0x00558110`): which of
  the player's items `has_item` searches (inventory, equipped, cursor,
  belt) is not written. The rest's inventory list is searched.
- **WE9** (`quest_items.rs`, `quests.md` §9.1): the reward creation
  `0x00559CE0` and the level default `0x00558200` have no request layout
  in the items specs. Left as a seam.
- Reader choices (where a spec writes "stat" without naming the reader):
  - unit total: monster `level` and `monster_playercount` (treasure §5.4
    / §7), MF stat 80 and GF stat 79 (§6 / §8), the request unit's
    level, the quest `stat`;
  - base: player `level` (treasure §7 step 3 says "base"), and gold
    stat 14 (the setter `0x00530EA0` writes the base).
- A missing unit record: `player_class` reads 0xFF in the cube and 0 in
  the quests; `item_seed` / `unit_seed` panic. Callers ask only for live
  units.

## 7. Checks to queue (for `docs/HANDOFF.md` §5)

1. Once the item-creation recording R1 exists (`impl-items.md` check
   2), replay it through `Economy::create_item` (real allocator + real
   lists), not only through `create_item` on fakes. This also settles
   WE2 and WE3 against the recorded list layout and unit flags.
2. Once a drop recording exists (`impl-treasure.md` check 3), replay it
   through `walk` with `ItemDrops`, comparing the created units' stat
   lists.
3. Cube recordings (`impl-world.md` check 3): replay them through
   `EconomyCube` once `CubeRest` has providers.
