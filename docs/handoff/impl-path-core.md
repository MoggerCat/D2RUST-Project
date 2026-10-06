# Handoff: path core (`d2_sim::path`: coordinates, records, collision, footprints)

> Not yet folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file is the detailed record until a docs session folds it.

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Branch `claude/impl-path-core`,
from `claude/specs-staging` at `c6e40f9`. Repo only, synthetic rooms, no
game files (M09). Spec: `specs/sim/path-placement.md` §1–§6 (draft) +
`specs/sim/path-tables.tsv`. Parallel sessions on the same base:
`impl-path-place` (§7–§12 in `path::{search,place,warp}`) and
`impl-walk` (`sim/pathing.md` in `path::walk`); each adds one `pub mod`
line to `crates/d2-sim/src/path/mod.rs`.

## State

**Implemented, unverified.** §1–§6 and the edge cases they own (r1, r8)
pass as unit tests (26 tests in `crates/d2-sim/src/path/tests.rs`, all
synthetic rooms). No position trace exists (spec open question 1), so
nothing here is checked against 1.14d yet. The spec's P/F/D vectors
test §7–§9 (`impl-path-place`).

Changes outside `crates/d2-sim/src/path/`: one line `pub mod path;` in
`crates/d2-sim/src/lib.rs`. No dependency changes. No function draws
(spec "Randomness").

Coverage (`python3 tools/coverage.py`): `specs/sim/path-placement.md`
§1 r1–r4, §2.1, §2.2, §2.3, §2.4 r1–r6, §3 text, §3 r1, §4 text, §4 r1–r5, §5.1,
§5.2, §5.3 r1–r4, §6 r1–r4, edge cases r1 and r8 claimed: 33 of 84 units
(unit tier; §7–§12 are `impl-path-place`'s).
Unclaimed in §1–§6: §2.5 (`SUNIT_Add`: monster init, room list insert,
hash insert, update queue, room-changed flag — a wiring step; the path
part it needs is `alloc_dynamic_path`, `StaticPath::set` and
`add_footprint` with the per-type conditions below).

## Code map rows (for HANDOFF §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/path/mod.rs` | module index, `PathError`, re-exports | `sim/path-placement.md` |
| `crates/d2-sim/src/path/coords.rs` | sub-tiles, 16.16 centres, client coordinates, `dist_sq` | §1 |
| `crates/d2-sim/src/path/record.rs` | `PathKind`, `StaticPath`, `DynamicPath` (+ `set_path_type`, `set_target_point`), `UnitPath`, `UnitShape` / `MonsterShape` / `ObjectShape` (size, footprint mask, move mask), `pattern_of_size`, `alloc_dynamic_path`, `flags`, `path_types` | §2, §3, `pathing.md` §2 set-type |
| `crates/d2-sim/src/path/collision.rs` | `CollisionRooms` seam, `find_room`, point / plus / box / size / pattern queries, `box_apply` (object set / clear), masks | §4 |
| `crates/d2-sim/src/path/footprint.rs` | pattern and size stamps / clears with markers, `Footprint` add / remove, mask change, pattern set, corpse, try / forced / missile move, teleport, `PathMotion` seam | §5, §6 |
| `crates/d2-sim/src/path/tables.rs` | `PathTables` (all 18 tables of `path-tables.tsv`, embedded, strict parse) | Constants |
| `crates/d2-sim/src/path/tests.rs` | rules, edge cases, TSV parse + perturbation test (M08) | |

## Public API

Coordinates are `i32` sub-tiles; rooms are `crate::units::RoomId`
(active room records); masks are `u16`; patterns `u32` and sizes `i32`
raw, so the "other → 0xFFFF / 1 / nothing" branches stay reachable.

- `path::tables::PathTables::{spec, from_tsv}`: pub fields
  `pathtype_flags`, `pathtype_diroff`, `pattern_of_size`, `dir8_toward`,
  `dir8_target`, `testdir`, `altdir`, `dist8_path`, `dist8_unit`,
  `snap9`, `tan`, `dirdiff`, `field_dx`, `field_dy`, `velmod_player`,
  `velmod_monster`, `velmod_monster_x`, `animstat` (rows in index order,
  filled columns only). No global copy: callers hold one.
- `path::coords`: `tile_to_subtile`, `to_fp16_center`, `subtile_of`,
  `client_from_precise`, `client_from_subtile`, `dist_sq`,
  `FP16_CENTER`.
- `path::record`: `PathKind::of(UnitType)`; `StaticPath::set(room, x,
  y)`; `DynamicPath` (every §2.3 field, D2MOO names: `precise_x/y`,
  `room`, `prev_room`, `cur_point`, `point_count`, `flags`, `path_type`,
  `unit_size`, `pattern`, `foot_mask`, `move_mask`, `collided_mask`,
  `velocity` …, `points[78]`, `saved_count`, `saved_steps[10]`) with
  `x()`, `y()`, `update_client()`, `set_target_point(x, y)`,
  `set_path_type(tables, is_player, t)` (`0x00648CF0`, fatal asserts as
  `PathError::PathType`); `UnitPath::{position, room}`,
  `unit_position(Option<&UnitPath>)`; `UnitShape::{size, foot_mask}`,
  `MonsterShape::{can_be_in_town, move_mask}`, `ObjectShape::{foot_mask,
  collides_in}`; `pattern_of_size(tables, size, shape)`;
  `alloc_dynamic_path(tables, rooms, DynamicKind, owner, room, x, y,
  set_0x10)`; consts `flags::*`, `path_types::*`, `PATH_POINTS`,
  `SAVED_STEPS`, `DEFAULT_VELOCITY`, …
- `path::collision`: `find_room`, `point_value` (`0x0064CB30`),
  `plus_value`, `box_value(…, (sx, sy), mask)`, `size_value`
  (`0x0064D9B0`), `pattern_value` (`0x0064D870`), `pattern_collides`
  (`0x0064D910`), `box_apply(…, (sx, sy), mask, set)` (`0x0064DE30` /
  `0x0064DC00`), `MISSING_ROOM` = 0x27, `UNKNOWN_SHAPE`, `masks::*`.
- `path::footprint`: `stamp_pattern` / `clear_pattern` (`0x0064EA90` /
  `0x0064EC10`), `stamp_size` / `clear_size` (`0x0064EA00` /
  `0x0064EBA0`), `Footprint { room, x, y, shape: FootShape, mask }`,
  `add_footprint` (`0x00649400`), `remove_footprint(rooms, fp,
  RemoveRule, force) -> bool` (`0x00649560`), `set_foot_mask`
  (`0x00648C30`), `set_pattern` (`0x00649190`), `make_corpse_footprint`
  (§5.3 r3), `try_move` (`0x0064EDA0`), `forced_move` (`0x0064EFA0`),
  `missile_move` (`0x0064ED20`), `teleport` / `teleport_and_clear`
  (`0x00650910` / `0x00650BE0`).

For `impl-walk`: the footprint move of pathing §9.6 rule 6 is
`forced_move` / `missile_move` / `try_move` here; set position and
reset (§9.6 r8–9, §9.7) are yours, and the wiring implements
`PathMotion` with them. For `impl-path-place`: queries are
`size_value` / `pattern_value` / `point_value` on any `CollisionRooms`.

## Seams (for the wiring session)

| Seam | Provider | Notes |
|---|---|---|
| `path::collision::CollisionRooms` | `crate::drlg::Drlg` (active room by `RoomId`: `drlg_room_of`, then `ActiveRoom::subtiles`, `adjacency` mapped to `RoomId`s in order, `collision`) | `ActiveRoom` is `pub(super)` on `DrlgRoom`; a `grid_mut` accessor is needed in `drlg/` |
| `path::footprint::PathMotion` | `path::walk` set position (`0x0064FB90`) + room recache, reset (`0x006507B0`) | teleport calls set position, then reset |
| room list insert after `alloc_dynamic_path` with a room | `units::UnitLists` (`0x0064C350`, `unit-order.md` §5) | §2.4 rule 5; the alloc only stamps |
| §2.5 `SUNIT_Add` | wiring | per type: player / monster / missile `alloc_dynamic_path`; object `StaticPath::set` + `add_footprint` (Box) only when `ObjectShape::collides_in(mode)`; item only in mode 3 (insert without queue); tile `StaticPath::set` + footprint |

## Open points (`TODO(spec: …)` in the code)

1. §4 r4 box strips: right strip full height, top strip at the inside
   width (D2MOO's reading; the spec's open question 2). Also used for
   the object box set / clear, whose clipping the spec does not state
   separately.
2. §5.1: which room each stamped cell is looked up from is not stated;
   the code uses the centre's room (else the given room), as the
   queries of §4 rule 3 do. Patterns outside 0..5 stamp nothing.
3. §2.4 r4: a missile path gets "type 4" stored directly; whether the
   type table's flags (0x60000, incl. 0x40000 missile path) are applied
   is not stated.
4. §6 r4: teleport clears and queries with the path's room (as §6 r1),
   and "flags 0x8 := moved" is read as "the cell changed".
5. §5.2: object `HasCollision` for modes above 7 reads as 0.

## Local checks to queue

None runnable yet: §1–§6 need a per-tick position / footprint trace
(spec open question 1, `sim/pathing.md` open question 1). When it
exists: replay allocations and footprint moves and compare the room
grids cell by cell.

## Gate (run before push)

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
-D warnings`; `cargo test -p d2-sim`; `cargo run -p depcheck`; `python3
tools/spec_index.py --check`; `python3 tools/methods.py check`; `python3
tools/coverage.py --check` and `--selftest`. Results in the commit
message.

Results (2026-10-06, this branch): fmt clean; clippy clean; `cargo test -p
d2-sim` 1265 passed, 5 ignored; depcheck OK; spec index OK; 21 methods
OK; coverage 0 errors, selftest ok.
