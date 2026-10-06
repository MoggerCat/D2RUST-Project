# Handoff: path seams wired into the sim (`d2_sim::wiring::path`) — `claude/wire-path-sim`

> Not yet folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file is the detailed record until a docs session folds it.

Cloud implementation session, 2026-10-06, task class: architecture /
integration across three branches (high, METHODS M14). Base:
`claude/tender-meitner-mphas3` at `729c76e` (has `path::{coords, record,
collision, footprint, tables, search, place, place_seams, warp, walk}`).
Repo only, synthetic tables and the action fixture's DRLG, no game files
(M09). Specs: `sim/path-placement.md`, `sim/pathing.md`,
`world/waypoints.md` §7. Parallel sessions: `wire-inventory-sim`,
`host-merge-port` (neither of their files touched).

## 1. State

**Wired, unverified** (M02): every rule behind these adapters is a
draft-spec implementation; per-tick positions have no recording yet
(`pathing.md` OQ1). The adapters add no rule.

- The path provider is **opt-in per game**: `ActionHooks::paths:
  Option<Box<PathState>>`, `None` by default; `ActionHooks::enable_paths()`
  turns it on. Off: every path seam keeps its `Pending` answer (all
  existing tests, d2-server and d2-client fixtures unchanged). On: the
  seams in §2 are answered by `d2_sim::path` and `Pending`'s path methods
  are not called.
- The unit's path record (unit +0x2C) lives in `PathState::records`
  (`BTreeMap<UnitId, UnitPath>`), created by the allocation's
  `SUNIT_Add` path part, freed by the unit removal (`free_kind`). The
  unit record type is `units/`' (not this session's files); the store is
  keyed by the unit and has the record's lifetime.
- Per-tick movement runs from the tick: walk / run start schedules the
  every-tick event 0 (`0x00553F00`); the unit dispatch's player event 0
  (`units.md` §4.5) calls `UnitHooks::player_movement_step`, now
  `wiring::path::walk::player_step` (`0x00580C20`, `pathing.md` §9.2);
  a stop returns 2 and the ENDANIM handler follows.

Tests (`crates/d2-sim/src/wiring/path/tests.rs`, 7, on the action
fixture `wiring::action::tests::Fx` with `enable_paths`):

| Test | What | Spec |
|---|---|---|
| `allocation_gives_the_unit_its_path_and_footprint` | player at (26, 10): precise 0x1A8000 / 0xA8000, masks 0x80 / 0x1C09, type 7, size 2, pattern 1, velocity 0x800; plus footprint with NO_PATH on the centre; removal clears it and the record | path-placement §2.4, §2.5, §5 |
| `walk_request_moves_the_player_sub_tile_by_sub_tile` | C→S 0x01 (26, 10) → (31, 10): `Moving(1)`, mode 2, velocity 0x600; through `tick::tick`: x = 0x1AE000 + k·0x6000 (k = 0..12), tick 14 lands on 0x1F8000 and starts neutral; footprint moved; no event 0 left | pathing vector M1 (translated), §1, §8.1, §9 |
| `walking_across_a_room_edge_changes_the_room` | (37, 10) → (43, 10): 16 ticks, room A → B in both the path (prev room A) and the act room lists, flag 0x2 cleared by the room-change messages | pathing §9.6 r9, §9.8 |
| `a_wall_on_the_way_changes_the_walk` | M08: a wall at (29, 10) changes the outcome; the player never stands on it | pathing §9.6 |
| `waypoint_warp_places_the_player_in_the_spawn_room` | real `WaypointData::travel` level 2 → level 1: the player's room (lists and path) **equals the spawn room** (the e2e step-6 condition), position inside it, old footprint cleared, flags 2 0x10000, event 14 at f + 50, arrival mode 5 (town neutral) | path-placement §10, §11; waypoints §7 r5, r7 |
| `without_the_provider_the_warp_stays_pending` | M08: same travel with the provider off: the player stays in room A | |
| `coarse_free_box_avoids_a_monster_footprint` | `0x0064E840` from (30, 10), n 1, mask 0x3C01: (29, 9) on an empty grid; a size-2 monster allocated at (30, 10) (pattern 1, 0x100 + NO_PATH) moves it to (28, 8) (M08) | path-placement §8 |

`cargo test -p conformance` (tick replay): unchanged, passes.

## 2. Provider table

| Seam (owner) | Provider | Notes |
|---|---|---|
| `CollisionRooms` (path core) | `DrlgWorld` (`wiring/path/rooms.rs`) | active room by `RoomId` in any act DRLG; adjacency = the DRLG's array mapped to active rooms (same order as the list copy); grids via the new `Drlg::active_grid_mut` |
| `PathWorld` + `WalkUnits` (walk) | `PathCtx` (`wiring/path/walk.rs`): one context = game + `View` | records: `PathState`; footprints by type (§5.2); room list ops `UnitLists::room_remove` / `room_insert` (which queues, `0x0064C350` + `0x0064C040`); clients: active room's array; mode set `units::modes::set_mode`; event 0 `anim::every_tick_movement`; other starts `player_start` / `monster_set_mode`; charstats / monstats velocities from `ActionTables::combat`; stats / states / seeds from `View`; `item_stat`, `used_skill` via `Pending` |
| `PathMotion` (core teleport) | `PathCtx` → `Walk::set_position` / `Walk::reset` | |
| `CollisionView`, `PlaceHost`, `LevelView` (placement) | `place::Shared` (three handles on one `RefCell<PathCtx>`) and `place::Rooms` (DRLG only, for the searches) | 0x07 built from its `server-messages.tsv` layout, 0x15 / 0x0D by `walk::messages`, all sent through `Pending::send`; event 14 with callback `0x00554570`; spawn room `Drlg::spawn_room`; act start level = the act's town |
| `Pending::position` / `has_path` / `size` | `ActionHooks::path_position`, `path_has`, `View::path_size` | §2.1, §3 |
| `Pending::place` (allocation) | `View::path_place` | §2.5 path part (player / monster / missile §2.4; item mode 3, tile: static + footprint; object: static, no footprint) |
| `Pending::set_velocity` / `velocity` / `set_target_unit` / `set_target_point` / `set_footprint_mask` / `set_move_mask` / `set_acceleration` | `ActionHooks::path_*`, `View::path_set_target_unit`, `View::path_set_foot_mask` | `0x00648690`, `0x00648B90`, `0x00648AD0`, `0x00648C30`, `0x00648CE0`, +0x88 / +0x84 |
| `Pending::build_path` | `walk::build_path` → `find::compute` (§3) | town access 0 (TODO) |
| `Pending::step` | `walk::unit_step` → `Walk::step` (`0x00554CA0`) | |
| `Pending::cached_collision_word` / `crossed_subtiles` | path +0x54 / saved steps (`0x00648F40`) | velocity 0: size query at the position, all bits |
| `MissileRooms::collision_mask` / `collision_at` / `clear_footprint` | `size_value` / `point_value` / `clear_size` | settles wire-action W5 / WG7 when on |
| `AiWorld::collides` (`0x0064D910`) | `pattern_collides` with the path's pattern and room | |
| `Pending::warp` (`0x0053AEC0`, same act) | `place::level_warp` → `level_warp_place` (§11) | act change stays `Pending::warp` |
| `Pending::set_player_mode_arrival` | `PathCtx::walk_to(player, 2, own x, y)` | waypoints §7 r7 |
| `WorldPending::nearest_free_point` (`0x0064E840`) | `place::coarse_free_box`, n 1, mask 0x3C01 | population §6.3 r4 |
| `UnitHooks::has_path`, `player_movement_step`, `free_kind` (path free) | `ActionHooks` | |

## 3. Removed duplicate seams and types (path/, folded onto the core)

| Removed | Replacement |
|---|---|
| `walk::seams::Point`, `place_seams::SubPoint` | `path::coords::Point` (re-exported as `walk::Point`, `path::Point`) |
| `place_seams::RoomRect`, `place_seams::TileRect` | `drlg::TileRect` |
| `walk/tables.rs` (`PathTables`, `VelMod`, `TanRow`, `AnimStat`, `TableError`, `PATH_TABLES_TSV`, `PATH_TABLE_ROWS`) | `path::tables::PathTables`; walk's table checks ported to `path/tests.rs` |
| `walk::geom::centre` | `coords::to_fp16_center` |
| `walk::seams::WalkPath` | `record::DynamicPath` (+ helpers `cell`, `target`, `put_target`, `prev_target`, `final_target`, `point`, `live_points`) |
| `DynamicPath::{target_unit, target_type, target_guid}` | one field `target_unit: Option<TargetUnit>` (`TargetUnit` moved to `record.rs`) |
| walk `flag` module, `MAX_POINTS`, `MAX_SAVED_STEPS` | `record::flags` (+7 flags), `PATH_POINTS`, `SAVED_STEPS` |
| walk `path_type` module | `record::path_types` (+`ASTAR`, `TOWARD_FINISH`, `WALL_FOLLOW`; `KNOCKBACK` → `KNOCKBACK_SERVER`) |
| `walk::find::set_type` | `DynamicPath::set_path_type` (`WalkError::Path(PathError)`) |
| `PathWorld::{cell_room, room_rect, pattern_collides, try_move, forced_move, missile_move}` | `PathWorld: CollisionRooms` + `collision::{find_room, pattern_collides}`, `footprint::{try_move, forced_move, missile_move}` |

`place_seams::CollisionView` stays a seam (its tests trace calls); its
provider maps each query to the core function.

## 4. Public signature changes

- `path::walk`: `Walk<'a, C> { t, c }` (was `{ t, w, u }`), its methods
  without `game`; `handle_message(t, c, player, id, a, b)`, `request(t,
  c, …)`, `interrupt_check`, `start_movement`, `compute(t, c, path, unit,
  town)`, `neutral_start(c, unit, path)`, `mode_check(u, unit, m)`,
  `set_mode_and_velocity(t, u, unit, path, m)`; `WalkUnits` methods lose
  every `Game` argument and gain `frame()`; `PathWorld: CollisionRooms`
  with nine methods; `PathInfo.{path_type, pattern}` u32; `WalkError` not
  `Copy`; no re-export of `PathTables` / `WalkPath`.
- `path::footprint::{teleport, teleport_and_clear}(c, path, is_missile,
  room, x, y)` with `C: CollisionRooms + PathMotion` (was `(rooms,
  motion, …)`): set position's room recache reads the rooms the forced
  move just wrote, so one context provides both.
- `path::place_seams`: `CollisionView::room_rect`,
  `WarpTileView::tile_rect` return `drlg::TileRect`; `SubPoint` → `Point`.
- `wiring::action`: `ActionHooks::paths` (new pub field),
  `ActionHooks::enable_paths()`, `WiringError::{Path, Walk, Place}`;
  `mod tests` is `pub(crate)` (fixture reuse).
- `drlg::Drlg::active_grid_mut(DrlgRoomId)` (new; footprints write a
  room's grid — the accessor `impl-path-core` asked for).
- New: `wiring::path::{PathState, PathCtx, rooms, units, walk, place}`.

## 5. Still pending (unspecified; each has a TODO at its site)

- Missile flight with the provider on: the missile path (`0x00649760`,
  flag 0x40000) / type 4 function is `pathing.md` OQ3
  (`WalkUnits::other_path_function` → 0 points), so a built missile has
  no points and its first step expires it. Hosts with missiles keep the
  provider off or fake via `Pending` until specified.
- `0x006417F0` (target distance), `0x0054DC40` (teleport spot), AI path
  steps / blocked (flag 0x800 vs `0x00648EB0` in `ai.md`) / stop / walk in
  radius: stay `Pending`.
- §2.5: `set0x10` of `0x00554850`, the monster calls after the
  allocation, `units.md` §3.1 step 8 corpse settings (player mode 0 / 17 —
  the player init's mode is not specified; `0x0063EA40`).
- Objects: no objects.txt in `ActionTables`: static path, no footprint.
- Removal's footprint clear `0x00649F50`: conditions not stated; cleared
  unconditionally.
- `player_knockback_path`, the run stat list (`attach_run_stats`,
  `0x00620E80`), torso speed, cursor item, door orientation, target lead
  (OQ4), re-path budget (OQ8), unit add / removal messages, AI room memo,
  life percent of 0x0D (`0x00621F20`), player data +0x148 / +0x14C, pets:
  `WalkUnits` / `PlaceHost` defaults.
- **FreeSpot (treasure drop `0x0064E810`)**: the provider exists
  (`wiring::path::place::floor_drop(drlg, field, room, from, size,
  fallback)`; field `PathState::field`, loaded by the host from
  `ExpField.D2`), but the seam cannot reach the rooms: `FreeSpot` /
  `DropPlacer::place` are called while `ItemDrops` holds the hooks.
  Needs a seam change in `wiring/economy` (owner: economy / inventory
  wiring): pass the rooms into `DropPlacer::place`. Also
  `economy/death.rs` still reads `h.x.position` (→ `h.path_position`).

## 6. Questions found (spec)

- **Teleport footprint room** (`path-placement.md` §6 r4, core TODO):
  the core clears and stamps from the path's room before the move; a
  warp to a room not adjacent to it (waypoint travel) stamps nothing at
  the destination. The waypoint test records this. Settle: Ghidra
  `0x00650910` → `0x0064EFA0` room argument.
- Second spawn search in `waypoints.md` §7 r7 draws again on levels with
  `Position` 0 (room roll); with several rooms the arrival test may
  compare against a different room than the placement used. The test
  uses a one-room level. (wire-action W14 concerns the same search.)

## 7. For the server walk/run handler session (`d2-server`)

- Turn the provider on at game creation (`ActionHooks::enable_paths()`,
  before any unit allocation) and give players a path: allocate the
  player through `View::allocate` (or call `View::path_place` after the
  join puts it in a room).
- C→S 0x01–0x04: `wiring::path::walk::walk_message(v, game, player, id,
  a, b)` after `intents-events.md` §2.4 (returns `(0, Some(Outcome))`).
- Nothing else: event 0 already runs through `ActionSim`'s unit hooks
  every tick; 0x0F / 0x10 / 0x15 of the update pass (`pathing.md` §10.2,
  §10.3; builders `walk::messages::{mode_update, reassign_flag}`) still
  need a caller in the server's update pass.
- d2-client e2e step 6 then holds with the provider on (see the
  waypoint test).

## 8. Gate

`sh tools/gate.sh all` — results in the commit message.
