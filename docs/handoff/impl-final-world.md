# Cloud impl-final-world (2026-10-07): world / server leftovers

Branch `claude/impl-final-world`, base `claude/specs-staging-7` @ 431a3fd.
Implementation session (specs/docs/crates only, M04). Inputs: the "Left"
lists of `impl-pc2-s4-world.md` §3 and `impl-world-rest.md` §4. Two
workers ran in worktrees (hirelings; quests) and were merged here; their
worktrees were based on `9d076a1b` (an ancestor of the base), merged
cleanly except the `wired.rs` spec header. Everything is **wired,
unverified** (M02), on synthetic data only.

## 1. Landed

| Item | Where | What |
|---|---|---|
| 1. Object item drops | `d2-sim` `treasure/class_pick.rs` (new), `wiring/economy/drop_helpers.rs` (new); `ObjectView` (`wiring/action/objects.rs`) | Part picks `0x00555E70`/`0x00555FB0`/`0x005560F0` with filter `0x00555E00`, A(L) `0x006427F0`, random class `0x00556240`, weapon rack's 6 tries, class of `0x00559A30` with the quality-4 loop (`objects-2.md` §20.2–§20.6, `treasure.md` §9). Helpers armor `0x005594C0` / weapon `0x00559630` (room seed, area level − 1, flags2 0x40), gold `0x00559300` (flag 0x2000 cleared), by source unit `0x00559A30` (unit seed, unit level), code drop `0x00585970` (PROVISIONAL) — real item units via `Economy::create_item`, placed by the floor drop. `DeathDrops` gains `picks` (`with_picks`) and `pick_errors`. Providers: `ChestWorld::code_drop` / `drop_item_code`, `MechWorld::stand_drop` / `drop_code_quality`, `ObjectWorld::gold_drop`; quests: `HostQuests::drop_gold`, `quest_drop`, `drop_item_at` (levels from the action tables). |
| 2. Footprints | `wiring/action/objects.rs` (`object_shape`, `apply_object_footprint`, `View::free_object_footprint`) | Stamp `0x00620A70(O, room, x, y)` = box set `0x0064DE30`, free `0x00623830` = box clear `0x0064DC00` (PROVISIONAL), `SizeX` × `SizeY` with the §3 mask (`IsDoor`, `BlocksVis`, `BlockMissile`, `SubClass`) on the DRLG collision grids; `MiscWorld::footprint_collides` = the `0x0064D800` box query. `ObjectWorld::stamp_footprint` now takes (room, x, y) (inits 11/12 pass the init record's, door and gate O's own). **No codegen change**: the typed `Objects` record already carries `blockmissile` (+438). `Pending::object_stamp_footprint` / `object_free_footprint` removed. |
| 3. Hireling callers (worker) | `wiring/inventory/merc.rs`, `wiring/action/hirelings.rs` (`HirelingCall` queue `ActionHooks::hireling_calls`), `wiring/path/place.rs`, `d2-server` `world/hireling_host.rs`, `character.rs`, `wired.rs` | C→S 0x61 seams (`hireling`, `owns_hireling`, `equip_on_merc`) on the lent hireling lists (`InvState::hirelings`), `MercItems` (`HirelingItems`, item copy `0x0055A2A0` = `InvDesk::copy_of(…, fillers)`); act change queued by `level_warp` across acts (classic: `classic_act_change` + follow; expansion: follow); `JoinFollow` queued at `game_entry`; save restore queued by `ActionCharacter::restore_hireling` (loader `Current`), run by `WiredWorld::hireling_calls` (plan, hire slot, unit, `life::restore`, experience, `restore_tail`, inventory). |
| 4. Quests (worker + lead) | `wiring/economy/quest_reward.rs` (new), `quest_host.rs`, d2-server `wired.rs`, `SimGame` | `reward_item` (`quests.md` §9.1) on the host inventory (`QuestInventory` / `QuestInv`); seams `path_target_xy`, `trade_button` (`vendors-2.md` §10.1), `free_chat_node`, `clear_npc_chats`, `obelisk_close` (0x44 cancel, `act2::q6::insert_cancel`), `close_cube`, `town_portal_guid` / `portal_partner` / `free_portal_object` (to the `Pending` portal seams), `monster_mode_at`; `quest_item_level` (`0x00558200`). `WorldHost::take_host_requests` drains `QuestControl::take_host_requests()`; `SimGame::tick` collects them into `SimGame::host_requests` (session layer takes them with `SimGame::take_host_requests()`). |
| 5. Loaders | `d2-server` `GameTables::class_picks`; `tests/world_data_tables.rs` | `#[ignore]` game-file test `object_tables_load_objgroup_leveldefs_and_the_pick_columns`; the synthetic test checks `class_picks` (6 rows, 2/2/2). |

## 2. Changed test expectations

| Test | Old → new | Spec |
|---|---|---|
| `wiring::action::tests::objects::object_timer_event_reaches_object_event` | ENDANIM free logged as `Pending` "free footprint N" → the 2 × 2 box (mask 0x404) stamped, then cleared on the collision grid | `objects-2.md` §18.6, `path-placement.md` §3, §5.1 |
| `wiring::action::tests` fixture `tables()` | waypoint row gains `SizeX`/`SizeY` 2, `BlockMissile` 1 | as above |

No other expectation changed (workers: none).

## 3. PROVISIONAL (all `REC-none`: no spec names a capture)

- `objects.md` §8 code drop `0x00585970` (`drop_helpers::code_drop`): read as `0x00559A30` with the code argument as drop code (U's level, no draw, floor search, request unit U, the quality argument).
- `objects.md` §8.1 r7: the chest's drop-code drop uses quality 2.
- `objects.md` §8.2 / §10, `path-placement.md` §5.2: the free `0x00623830` = box clear with the class's footprint mask at O's room and position, no `HasCollision` test.
- `inventory-moves.md` §7.23 r2: owner test `0x0065A590` = the 0x4B room test (player's room in the hireling's room's adjacency).
- `hirelings.md` §11 r4: "old leaves the merc's inventory" = the unlink at the start of rule 4.
- `hirelings.md` §10 r3: hire slot = first slot with the name id not offered.
- `quests.md` §9.1: reward creation arguments (source = player, no player data, spawn mode 4, no seed, never-ethereal 0); no free spot → drop at the player's own position.
- `vendors.md` "Stat readers": `0x00625E00` max durability, base stat 73 = 0 → no entry.
- `vendors-2.md` §10.1 r5: `0x00597A20` read as 0 when the trade partner is gone.

## 4. Local run queue (game files)

```
D2_GAME_DIR=<install> cargo test -p d2-server --test world_data_tables -- --ignored
```
Look for: `every_loader_builds_from_the_users_install` and
`object_tables_load_objgroup_leveldefs_and_the_pick_columns` pass —
objgroup 133 records (record 97 zero), `leveldefs.len() == levels.len()`,
objects > 500 rows, 306 weapon rows of which 200 have `bitfield1` bit 1,
pick rows = items rows. Then the existing world-data game tests:
`cargo test -p d2-server -- --ignored`.

## 5. Left

- **Drop state in production:** nothing on the server sets
  `ActionHooks::object_drops` (only the app, `d2-client`), so the drop
  helpers and chest drops run only where a host installs a
  `DeathDrops` — it must call `.with_picks(Arc::new(tables.class_picks()?))`
  or every pick finds nothing.
- **Object add-to-world footprint:** `View::path_shape` still returns
  `None` for objects (objects.txt is not in `ActionTables`), so a placed
  object gets no footprint at `SUNIT_Add` (§5.2 add `0x00649400`); only
  the explicit stamp / free above touch the grid.
- `set_drop_code` (quests, object +0xB8 for monsters) still unhandled;
  `quest_drop` passes the code straight to the helper instead.
- Hirelings: calls run after the handler / tick (as `pet_deaths`), not
  inside the caller; the owner's act change `0x0053ACC0` has no owner
  spec (warp stays `Pending::warp`); socketed items lost in a swap
  (`copy_of` fillers TODO); 0x61 take's `copy_item` to the rest;
  `0x0055F4F0`, `0x0055C460`, `0x00621000`, `0x00628280` no-ops; rule 8
  (`hireling_items_loaded`) unapplied; `Old` / `OldV47` loaders
  unreachable (parser refuses < 0x5C).
- Quests: `steeg_release` (no Steeg Stone object data/spec); portal seams
  need a host implementing the `Pending` portal methods; rewards inside
  the action wiring's lent quest call use the rest's `reward_item` (no
  inventory model on the loan); message order of a reward vs. the
  quest's own messages unspecified (TODO in `quest_call`); `QuestTick`
  not wired into `WiredWorld`'s tick; game end / save of the drained
  host requests is session flow (impl-pc1-s7).
- Flaky: `d2-sim prop_walk_motion chase_a_moving_target` failed in
  worker runs (passes on rerun); path code untouched here.

## 6. Gate

See the final message to the coordinator (head SHA and result).
