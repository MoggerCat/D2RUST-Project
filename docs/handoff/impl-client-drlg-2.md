# Handoff: client DRLG answers (PC 1) — `claude/impl-client-drlg-2`

Cloud implementation session, 2026-10-07, task class: implementation
from specs, medium. Base: `claude/specs-staging-2` at `ddbfe0b`. Repo
only: no `game/`, no `re/`, no recordings (M09: every claim below holds on
this branch, on synthetic data). Implements PC 1's answers to
`impl-client-drlg` §3 and the 0x7A order (§7 ninth set I-1). Read:
`specs/drlg/rooms.md` §4, §4.6, §5 r8–r9, §6, §9.3 "Entry identity",
OQ 15–17; `specs/client/model.md` §9 r5, OQ 11–12;
`specs/client/msg-units.md` Randomness; `specs/sim/unit-order.md` §5
r6–r8; `specs/render/draw-order.md` §3, §8, §9, OQ 12;
`specs/render/lighting.md` §6.4; `specs/drlg/levels.md` §9.2, §11.3
step 9, §11.4; `specs/sim/pets.md` §8; `specs/world/hirelings.md` §13.

## 1. What changed

| Path | What | Spec |
|---|---|---|
| `d2-sim` `world/hirelings/pets.rs`, `player/pets.rs` | the **server** 0x7A encoders write owner GUID @5, pet GUID @9 (`pet_action(action, type, class, owner, pet)` now takes the wire order; removes carry the GUID @9) | `pets.md` §8, `hirelings.md` §13 r2 |
| `d2-sim` `drlg/room.rs`, `drlg/level.rs` | client build timer: `Drlg::{build_timer, build_cursor}`, `build_timer_reset` (5 client / 7 server), `client_build_timer` (rules 4–8: B > 1 → B := 0; T − 1, B kept; T = 0 → T := R, cursor reset to the first status-2 room, the circular walk examines rooms until one build, C := the room after the last examined, B := 0); set handler 1 sets T := R after its build | `rooms.md` §4.6 |
| `d2-sim` `drlg/seams.rs`, `drlg/tiles.rs` | `TileInfo { roof_height: u16, height: i32 }`; each cached DT1 keeps its path; `Drlg::dt1_path(TileRef)` (an entry is exactly (file, index)) | `rooms.md` §9.3 "Entry identity" |
| `d2-sim` `drlg/logic.rs` | `Drlg::wall_coord(room, x, y)`: a wall record's coordinate record (+0x10): the record-grid entry in a grid-built room, none in a one-record room | `levels.md` §11.3 step 9 |
| `d2-server` `world_data` | `tile_info` copies roof height and height from the DT1 header | `rooms.md` §9.3 |
| `bridge/msg/session.rs`, `bridge/dispatch.rs` | 0x07 at a point in no room → `HandlerError::Crash { at: 0x0061B672 }` (after the §9 r4 record); 0x08 there → nothing; 0x03 empties the room unit lists with the old act | `model.md` §9 r5 |
| `bridge/drlg.rs` | `ActList` records each created active room; `ClientDrlg::take_created` (act room callback, creation order); `ClientDrlg::client_update(free_levels)` = build timer, then the level free | `rooms.md` §4.6, §5 r9; `levels.md` §9 r2 |
| `bridge/update.rs` | after the queue drains, `drlg_update`: `[0x007A0498]` += 1 (`ClientWorld::drlg_updates`), the timer, the level free on every multiple of 13, then the refresh and callbacks; an error is recorded under id 0 | `rooms.md` §4.6 r1, last paragraph |
| `bridge/world.rs` | `RoomUnits` (per active room, head first: `place` = room recache / insert, `leave`, `free_room`, `set_order` for the draw's sort); `ClientWorld::{room_units, lights, drlg_updates}`; `unit_room` is now list membership (the client's only unit → room link); `refresh_active_rooms` frees the lists of rooms no longer active and runs `LightList::room_created` for each new room; `cell_lookup` (`0x00463740`); `impl LightRooms for ClientWorld` (set S only); `LevelRow::draw_edges` | `unit-order.md` §5 r6–r8; `lighting.md` §6.4; `path-placement.md` §4 r1 |
| `bridge/msg/units.rs`, `bridge/check.rs` | creation at a point links the unit at the head of its creation room's list (0x59, 0xAC, 0x51 objects; (0, 0) → no list); 0x15 and the §6 r8 correction recache dynamic-path units (players, monsters, missiles); 0x0A leaves the list. 0x51 steps the room seed (already did; comment now cites the corrected Randomness) | `unit-order.md` §5 r6; `msg-units.md` Randomness |
| `rules/lighting/records.rs` | trait `LightRooms` (owner lookups, cell lookup) split from `LightWorld`; `room_leaving` renamed `room_created` (it is the new-room callback) | `lighting.md` §6.4 |
| `rules/draw_order/mod.rs` | the fill sorts each room's unit list by client y (stable) before filing its units, for rooms that pass the room test, unless "skip units"; `Room::units_sorted` | `draw-order.md` §3 r4; `unit-order.md` §5 r7 |
| `world_view/near_rooms.rs` (new), `model_feed.rs`, `feed.rs`, `present.rs`, `bridge/mod.rs` | `MapState`: the §9 feed from the client DRLG (adjacency array of the player's room; tile rect, sub-tile origin; wall / floor / shadow records with flags, type, DT1 facts incl. roof height and height, coordinate record; fades and flags persisted per record while the room is active; per record its DT1 (path, index); the room unit lists; player tile and `0x0061B130` index; level id and DrawEdges). `ModelFeed::with_map` (opt-in), `ViewFeed::unit_facts` (default refuses), `ViewFeed::take_unit_orders` → `Bridge::set_room_order` after each drawn frame | `draw-order.md` §9; `unit-order.md` §5 r7 |
| `app/single_player.rs` | `client_level_rows` fills `draw_edges` | — |

## 2. Tests

Added (each with its `// Covers:`): `d2-sim`
`drlg::tests::rooms::client_build_timer_builds_status_2_rooms_one_per_run_out`,
`…::client_build_timer_skips_after_two_builds_and_wraps_from_zero`,
`drlg::tests::tiles::entry_identity_carries_roof_height_and_height`
(rooms.md OQ 17 vector), `drlg::tests::logic::wall_records_point_at_the_record_grid`;
`d2-client` `bridge::msg::tests_drlg::{units_live_in_their_rooms_lists,
the_draw_order_of_a_list_is_written_back, the_update_pass_runs_the_build_timer,
a_new_active_room_drops_light_caches_reaching_it,
near_rooms_come_from_the_client_drlg}`,
`rules::draw_order::tests::the_fill_sorts_each_rooms_units_by_y_stably`.

**Corrected to the spec (never weakened), listed as asked:**

- `d2-sim` `world::hirelings::tests::pets::pet_action_vector`: asserted
  pet @5 / owner @9; now the `pets.md` vector `7A 01 04 6B 01 01 00 00 00
  05 00 00 00` plus a remove with the GUID @9.
- `d2-sim` `player::pets::tests::pet_action_bytes`: same correction (add
  vector; remove `rm(5)` now has 5 @9; the −1 GUID read at 9..13).
- The other hireling / pet tests build their expected bytes with
  `pet_action`, whose argument order is now the wire order (owner, pet);
  their calls were reordered, their expectations unchanged.
- `bridge::msg::tests_drlg::rooms_come_in_sight_and_go`: the 0x07 at no
  room was asserted as "unspecified"; now asserts the §9 r5 crash text and
  that 0x08 there is not refused.
- `rules::draw_order::tests::units_file_by_flatness`: the off-grid unit's
  position (−100000, 0) is now (−100000, 2000) so that the new Y sort
  (§3 r4) keeps the list order the test reads (the unit is still outside
  the grid); its expectations are unchanged.
- `audio::driver::tests::indoors_is_the_player_levels_environment`: the
  fixture puts the player in the room's unit list (the room of a unit is
  now its list, `unit-order.md` §5 r6).
- `world_view::model_feed::tests::pending_hooks_answer_nothing`:
  `PENDING.len() >= 6` → `== 5` (the three map rows were rewritten into
  two: near rooms now built, unit facts and tile art pending).
- `bridge::gaps_numbered_tests::client_world_holds_only_stated_fields`
  names the three new fields and checks their defaults.

## 3. Still pending (named in code)

- **Unit facts for the near rooms** (`ViewFeed::unit_facts` refuses):
  unit flags +0xC4, flag-ex +0xC8, monstats2 `unflatDead`, objects
  `DrawUnder`, states 7 / 143 / 146 and the sight test are not in the
  model.
- **Tile art / tile blocks**: the DT1 (path, index) of every record is
  known (`MapState::entry`), but shade and blend need the frame's light.
  So `play` keeps `ModelFeed::map` off: with near rooms every drawn tile
  needs `tile_art`.
- **Light records**: `ClientWorld::lights` exists and the act room
  callback runs over it, but no unit code creates records yet.
- **Unit flag 0x800000** of the client room free (`model.md` §5 r5): units
  leave the freed room's list, but the flag and the C→S 0x4B it leads to
  are not modelled (no unit flags in the model).
- **Item / missile / tile room lists**: the item mode set (modes 3, 5),
  missile and tile creation insert into rooms (`unit-order.md` §5 r6
  table), but no handler creates or places those units yet.
- `0x0061B130` "null record → −1": unreachable on the client (every built
  room has its coordinate info); the code maps a missing record to −1.

## 4. Questions for the spec owners

1. **Build-timer cursor on the list head** (`rooms.md` §4.6 r6, r8):
   after a walk that ends on the tail, C is the status-2 list's head node
   (drlg +0x278). Rule 6 then reads the head node's status (+0x44 =
   drlg +0x2BC). d2rs treats it as ≠ 2 (DRLG zeroed at allocation,
   `levels.md` §3 step 1), so C resets to the first room. Does
   `0x0061B7E0` write a status into the head nodes? (If it wrote 2, a
   walk would start on the head node and, with B = 1 on entry, build
   nothing that call.)
2. **The `[0x007A0498]` counter**: d2rs keeps it per client world, from
   0, incremented on every client update (also before any act). Is it
   reset anywhere (new game, 0x03)?
3. **A cursor on a freed room**: the level free can free a room the cursor
   points at (status 4, no active room); 1.14d then reads freed memory
   in rule 6. d2rs treats a freed room as not status 2.

## 5. Gate

Stopped early on the coordinator's budget cut; partial gate:

- `cargo test -p d2-sim --lib drlg::tests` and `-- pets`: pass (new
  timer, entry-identity and wall-coordinate tests included).
- `cargo test -p d2-client --no-fail-fast` with a **local, uncommitted**
  bypass of the dispatch-table check (so the `Model::recv` tests run past
  the base's known `Mismatch`): every target passes except the three
  dispatch-table tests (`dispatch_table_matches_spec`,
  `dispatch_check_catches_perturbations`,
  `owned_rows_are_exactly_the_registered_handlers`), red on the base. On
  the committed code the `Model::recv` tests stay red until the parallel
  session fixes the dispatch table (as on the base).
- `cargo fmt --all`: clean. `tools/coverage.py --check`: 8,352 claims, 0
  errors. `tools/spec_index.py --check`: ok. `tools/methods.py check`: ok.
- **Not run:** `cargo clippy --all-targets -D warnings`, the full
  `CARGO_INCREMENTAL=0 cargo test --workspace` (d2-server built its
  touched tests only through the `TileInfo` field change, not run).

What is left: the clippy and workspace runs above; then §3 / §4.

## 6. Local run queue

- C83 (client vs server room seeds, `model.md` OQ 9) now also covers the
  timed builds: with the timer, the client's active rooms should match a
  recording's client DRLG (memory read of +0x98, +0x45C, +0x460 per client
  update, `rooms.md` OQ 15).
- `unit-order.md` OQ 5: one room's list (active room +0x74, unit +0xE8)
  before and after a drawn frame, against `RoomUnits` after
  `set_room_order`.
