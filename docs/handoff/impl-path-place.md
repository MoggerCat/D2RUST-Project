# Handoff: free-point searches, unit placement and warps (`d2_sim::path`) — `claude/impl-path-place`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Base: `claude/specs-staging` at
`c6e40f9`. Repo only, no game files (M09). Spec:
`specs/sim/path-placement.md` §7–§12 (draft). Parallel sessions:
`impl-path-core` (§1–§6, path records, collision, footprints) and
`impl-walk` (`sim/pathing.md`); this session uses none of their code.

## 1. State

Implemented, **unverified** (no position trace exists, spec open
question 1; the unit tests prove the spec's synthetic vectors on a fake
grid only):

| File | Spec | What |
|---|---|---|
| `crates/d2-sim/src/path/place_seams.rs` | §1–§6 (as called), §10–§12 | the seam traits (§3 below), `SubPoint`, `RoomRect`, masks, `PlaceMessage`, `PlaceError` |
| `crates/d2-sim/src/path/search.rs` | §7, §7.3, §8 | `nearest_free_point` (`0x0064DEA0`) and wrappers `free_point` (`0x0064E7B0`), `free_point_step` (`0x0064E7E0`), `free_point_field` (`0x0064E810`); `ExpField` + `walk_back` (`0x0066A670`); `coarse_free_box` (`0x0064E840`) |
| `crates/d2-sim/src/path/place.rs` | §9, §10, §11 | `floor_drop` (`0x00555DA0`), `place_unit` (`0x00554EA0`), `level_spawn_point` (`0x0061B060`), `game_entry` (`0x005394A0`), `level_warp_place` (the placement part of `0x0053AEC0`) |
| `crates/d2-sim/src/path/warp.rs` | §12 | `warp_tile_preset` (`0x0066E1C0`), `warp_player` (`0x005550B0`) → `WarpOutcome` |
| `crates/d2-sim/src/path/mod.rs` | — | only `pub mod place; place_seams; search; warp;` (the coordinator unions it with `impl-path-core` / `impl-walk`) |
| `crates/d2-sim/src/lib.rs` | — | `pub mod path;` |

Tests: 25 pass + 1 ignored in `path::` (`cargo test -p d2-sim --lib
path::`). Vectors: P1, P1b, P2, P2b, P3, P4, P5, P5b, D1, D2, D3 exact;
F1–F3 on a synthetic "sign" field (each cell steps one cell toward the
centre per axis; it reproduces the measured F1–F3 bytes and walks) and
on the 1.14d file in the ignored `expfield_live` (§5 queue below).
`field_tables_match_tsv` checks `FIELD_DX` / `FIELD_DY` against
`sim/path-tables.tsv` with a perturbation (M05, M08). `Covers:` claims on
§7.2 r1–r4 / text, §7.3 r1–r3, §8 r1–r4 / text, §9 r1–r3, §10 r1–r6,
§11 r1–r4 / text, §12.1 r1–r3, §12.2 r1–r6, edge cases r2–r7, r9.

Not done: R1–R3 (need the recorded game's DRLG regenerated; queue
below); §10 rule 7 position history (wall-clock, kept out of `d2-sim` as
the spec says); edge cases r1 and r8 belong to §4 / §5 (`impl-path-core`).

## 2. Public API

- `search::{nearest_free_point, free_point, free_point_step,
  free_point_field, coarse_free_box, walk_back}`, `FreeSearch`,
  `FieldTest`, `ExpField::{from_bytes, from_cells, byte, PATH}`,
  `FieldError`, `FIELD_DX`, `FIELD_DY`, `FREE_MAX_DISTANCE`,
  `COARSE_PASSES`.
- `place::{floor_drop, place_unit, level_spawn_point, game_entry,
  level_warp_place}`, constants `DROP_START_DX/DY`, `SPAWN_OFFSET`.
- `warp::{warp_tile_preset, warp_player, warp_slot, WarpOutcome}`,
  `GATED_LEVELS`, `EXIT_LEFT`.
- Fatal asserts return `place_seams::PlaceError` (`NoPath`, `NoAct`,
  `SpawnNotFree`, `NoLvlWarp`) instead of exiting; malformed fields
  (`FieldOutOfRange`, `FieldDirection`, `FieldNoEnd`) cannot occur with
  the 1.14d file (measured in the spec).

Callers to switch over (not touched, other owners): the treasure drop
seam `DropSink::place` (`treasure/walk.rs`, `wiring/economy/death.rs`,
`treasure_items.rs`) can be `floor_drop(.., size 1, fallback true)` —
note `treasure.md` §7 calls `0x0064E810` directly with the same
arguments, which is exactly `floor_drop`; the `LevelTypes::warp_unit`
seam (`drlg/seams.rs`) can call `warp_tile_preset`; the waypoint
same-act placement (`world/waypoints.md` §7, HANDOFF §2 step 7c) is
`level_spawn_point` + `place_unit`; the population "nearest free point
`0x0064E840`" (HANDOFF §1 3l) is `coarse_free_box`.

## 3. Seams (no provider yet)

`place_seams::CollisionView` — exactly the §1–§6 operations called;
provider: `impl-path-core`. Associated types `Room`, `Unit`.

| Method | Spec |
|---|---|
| `cell_room(hint, x, y)` | §4 rule 1, `0x00463740` |
| `room_rect(room)` | §4 rule 1 (room +0x4C..+0x58) |
| `cell_value(room, x, y)` | §4 rule 2 (unmasked; only §8 with n + 2 < 2) |
| `point_query(room, x, y, mask)` | `0x0064CB30` |
| `size_query(room, x, y, size, mask)` | `0x0064D9B0` |
| `box_query(room, x, y, sx, sy, mask)` | `0x0064CEB0`, §4 rule 4 |
| `has_path(unit)` | §2.1 (unit +0x2C) |
| `unit_room(unit)` | `0x00620BB0` |
| `unit_size(unit)` | `0x00620510` |
| `teleport(unit, room, x, y)` | §6 rule 4, `0x00650910` |
| `add_player_to_world(unit, room, x, y)` | §2.5 `0x00554850` as game entry calls it |

`place_seams::PlaceHost<U>` — units, messages, timers, walk (defaults
do nothing, `life_percent` 0):
`is_player` (units §2), `queue_update` (`0x0064C040`), `or_flags2`
(+0xC8: `flags2::PLACED` 0x10000 / `PLACED_ALT` 0x800), 
`room_change_messages` (`pathing.md` §9.8, `impl-walk`), `send(player,
PlaceMessage)` (0x07 MapReveal, 0x15 ReassignPlayer, 0x0D PlayerStop;
type / GUID filled by the host), `set_player_point` (player data
+0x148 / +0x14C), `schedule_event(unit, 14, 50)` (callback
`0x00554570`), `pets_follow` (`0x005754B0`), `request_walk`
(`0x005809D0`, mode 2, `impl-walk`), `life_percent` (`0x00621F20`).

`place_seams::LevelView<R>` — DRLG (associated `Act`): `spawn_room(act,
level, tile_index)` (`drlg/levels.md` §10, `0x0066B2B0`, draws on the
level seed), `act_start_level(act)` (act +0x08), `room_reveal(room)`
(tile x, tile y, level id for 0x07), `warp_destination(tile_room,
class)` (`0x006195A0`, spec OQ3; default `None`), `quest_gate(source,
dest)` (`0x00545B80`, OQ5; default 0).

`place_seams::WarpTileView` — DRLG side of §12.1 (associated
`DrlgRoom`): `tile_rect(room)`, `lvlwarp(room, slot, letter)`
(`levels.md` §7 rule 4), `add_preset_unit(room, type, class, mode, x,
y)` (`0x0066BF30`).

## 4. Questions (spec readings; each has a `TODO(spec: …)` in code)

1. §7.3 rule 3 / edge case 4 "the room the search passed": read as the
   candidate cell's room (the search's current hint). The vectors use one
   room, so they do not decide it. Settle: Ghidra `0x0064DEA0`'s call to
   `0x0066A670`.
2. §8 rule 3 "n + 2 < 2 → the cell's grid value": read as the unmasked
   value (§4 rule 2) of the cell room. Settle: `0x0064CEB0`.
3. §8 "inside the rect's rows / columns": read as half-open (as
   `levels.md` §8 rule 2).
4. §11 game entry and level warp: the `size` argument `0x005394A0` /
   `0x0053AEC0` pass to `0x0061B060`, the `flag` game entry passes to
   `0x00554850`, and what either does when no spawn room is found:
   callers pass the size; no spawn → `Ok(false)`.
5. Recipients of the 0x07 / 0x15 of game entry and the 0x0D of warp
   arrival (§10 rule 6 names the player's client only for its 0x07).
   R2 shows ten 0x07 at game entry (the spawn room and neighbours); §11
   names one: the other nine come from code the spec does not cover.
6. §12.2 rule 5: the walk-out target uses the point of rule 2, not the
   point `0x00554EA0` may have moved to (same point unless the placement
   search moves it).
7. §12.2 rule 3 source / destination levels are taken from the
   `warp_destination` seam (with OQ3).

## 5. Local run queue (for HANDOFF §5 C)

- `D2_GAME_DIR=… cargo test -p d2-sim --lib path::search::tests::expfield_live -- --ignored`:
  `ExpField.D2` is 65,546 bytes, header (0x010A, 256, 256), F1 bytes
  `3 4 5 / 2 8 6 / 1 0 7`, F2 / F3 walks as the spec lists. Expected
  values are the spec's measurements; the test was written without the
  file (`HANDOFF` §8 lesson: unconfirmed until its first run). Its
  `Covers:` (§7.3 r1, r2, game tier) counts only after it passes.
- R1–R3: once a DRLG of the recorded game can be regenerated, run game
  entry and the waypoint travel through `level_spawn_point` /
  `place_unit` and compare with the recorded 0x15 / 0x0D.

## 6. Gate (this branch)

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D
warnings`; `cargo test -p d2-sim`; `cargo run -p depcheck`; `python3
tools/spec_index.py --check`; `python3 tools/methods.py check`; `python3
tools/coverage.py --check` and `--selftest`. Results in the commit
message of this branch.
