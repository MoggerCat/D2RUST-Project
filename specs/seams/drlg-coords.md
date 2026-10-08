# Spec: Seams — DRLG rooms and coordinates across sim, server and client

- **Status:** draft: a contract between existing owner specs, read on
  both sides of the code (`d2-sim` DRLG and wiring, `d2-server`
  session, `d2-client` bridge and world view) by the 2026-10-08 seam
  audit; checked by `crates/d2-client/tests/seam_drlg_coords.rs` on the
  synthetic data only (no real-data run yet, METHODS M23).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg`, `d2-sim::wiring::{action, path}`,
  `d2-server::adapters::session`, `d2-client::bridge::{drlg, world}`,
  `d2-client::world_view::near_rooms`
- **Related specs:** `drlg/levels.md`, `drlg/rooms.md`, `drlg/preset.md`,
  `client/model.md` §9, §11, §12, `sim/intents-events.md` §7.8, §8,
  `sim/path-placement.md` §11, `render/draw-order.md` §9,
  `seams/sim-server.md`

## Summary

The server and the client each run the same `d2-sim` DRLG code on their
own copy of one act: the server's copy is the game's (`levels.md` §2),
the client's is rebuilt from S→C 0x03 with the client flag
(`model.md` §12 rule 1) and grown room by room by S→C 0x07 / 0x08.
Nothing but those messages and the unit positions crosses. This spec
states what each crossing value means (coordinate space, unit, owner)
and the invariants that make the two copies the same act. It adds no
behavior: every rule cites its owner.

## Inputs

| Name | Type | Source |
|---|---|---|
| 0x03 act, init seed, town level, object seed | u8, u32, u16, u32 | server: game entry / act change |
| 0x07 / 0x08 x, y, level | u16 tiles, u16 tiles, u8 level id | server room switch |
| 0x15 / 0x0D / 0x59 x, y | u16 sub-tiles | server path record |
| 0x01 difficulty | u8 | game creation |

## Outputs / state changes

None of its own: the client DRLG copy (`bridge::drlg::ClientDrlg`), the
client's active-room list and the near-room feed are the owner specs'.

## Rules

### 1. Coordinate spaces

| Space | Unit | Origin | Where used |
|---|---|---|---|
| Act tile | 1 tile = 5 sub-tiles | act (0, 0); all levels of an act share it | DRLG room `rect`, level position (`levels.md` §6), 0x07 / 0x08 x, y |
| Act sub-tile | 1/5 tile | act (0, 0) | unit positions, active room +0x4C…+0x58, collision grids, every position on the wire |
| Room-relative tile | tile | the DRLG room's `rect` origin | tile records (+0x08, +0x0C; `rooms.md` §9), `Drlg::wall_coord` |
| Level-relative | — | — | not used on any seam: no message or seam carries a level-relative coordinate |

Conversions (owner `rooms.md` §5 rule 1, `levels.md` §11.4): sub-tile
rect = tile rect × 5 (`SUBTILES`); a sub-tile point's tile is the C
division by 5 (non-negative in every act, so floor). Acts share
coordinates: a point names a room only together with its act (and, for
0x07 / 0x08, its level).

## 2. Contract

### 2.1 The client act is the server act

The client builds its act from 0x03 u8@1 (act) and u32@2 (the server
act DRLG's `init_seed`, game +0x7C) with the difficulty of 0x01, town
id 0 and the client flag (`model.md` §11 rule 1, §12 rule 1;
`levels.md` §2–§3). u16@6 is the act's town level (`TOWN_LEVELS`),
never the player's level (`model.md` §11 rules 1, 5). Both 0x03
builders send the same bytes (`seams/sim-server.md` §2.4).

### 2.2 0x07 / 0x08 name a room by its tile origin and level id

x, y = the DRLG room's `rect.x`, `rect.y` in act tiles; level = the
room's level id (`levels.txt` row, fits u8 in 1.14d: ids ≤ 136). The
server fills them from the room it joins or leaves
(`intents-events.md` §7.8, `wiring::action::rooms::SwitchedRoom`); the
client looks the room up at that tile point in that level
(`model.md` §9 rule 1, `rooms.md` §4.2, `Drlg::set_in_sight_at`). The
point is the room's origin, so the half-open containment
(`levels.md` §8) finds exactly that room.

### 2.3 The two copies generate the same rooms

For every room the server reveals, the client's room at the 0x07 point
has the same level id, tile rectangle and creation seed `dwInitSeed`
(`rooms.md` §2: fixed by the level seed and the room's creation index).
Level generation must not depend on which other levels of the act were
generated first or on the client flag beyond what `levels.md` §3 step 8
and `rooms.md` §2 step 4 state. A room whose rectangle differs breaks
every later rule; `model.md` §9 rule 5 (a point in no room) is then a
fatal handler error on 0x07.

### 2.4 Active rooms are in sub-tiles, tile records room-relative

An active room's rectangle is its DRLG room's tile rectangle × 5 on both
sides (`rooms.md` §5 rule 1); the client's `ActiveRoom` (`x0, y0, w, h`)
is that rectangle. Tile records are room-relative tiles: the world
view's `TileRecord::tile` is the record's (x, y) unchanged, and
`Drlg::wall_coord` takes room-relative tiles while `Drlg::coord_at`
takes act sub-tiles (`levels.md` §11.4).

### 2.5 Unit positions are act sub-tiles; the room of a point agrees

Every position on the wire (0x15, 0x0D, 0x59, walk and skill points) is
an act sub-tile, u16. The client's room of a point (`model.md` §12
rule 2) of the local player's position is the active room of the
server's room for that unit: same level id and tile rectangle, and the
point lies inside it. The client's level is that room's level
(`model.md` §11 rule 3); it is never taken from 0x03 or 0x07.

### 2.6 The client's active rooms follow the server's room switch

The rooms the server's room switch joins (new adjacency order) each get
one 0x07, the rooms it leaves one 0x08 (`intents-events.md` §7.8, owner
of the order; game entry adds one 0x07 for the spawn room first,
`path-placement.md` §11). Each client active room therefore has a server
room at its origin (§2.3) at all times; crossing a level border keeps
§2.5 true on every frame.

### 2.7 Act change

On an act change the old act's rooms are left (0x08) before 0x05 and
0x03; 0x03 frees every client active room of the old act
(`model.md` §16 r1, `rooms.md` §8 r4) and builds the new act (§2.1);
the new act's 0x07 follow 0x03 (`waypoints.md` open question 1, the
recorded order). Rooms of two acts never coexist in the client list.

### 2.8 Ownership

| State | Owner | Copies |
|---|---|---|
| Act DRLG (levels, rooms, seeds, tiles) | server game (`d2-sim`) | the client's `ClientDrlg`, rebuilt from 0x03 + 0x07 (never shared) |
| Active-room list | server: `UnitLists` act room list; client: `ActList` | independent, same rule (`rooms.md` §5 rule 5) |
| Active-room seed (+0x6C) | each side's own; stepped by its own unit creation | equal at creation only (`model.md` §12 rule 5, OQ 9) |
| Unit position | server path record | client `ClientUnit::position` (0x15 / 0x0D), the walk prediction |
| Player level | derived from the room on each side | none sent |

## Constants & data dependencies

`SUBTILES` = 5 (`d2_sim::drlg`); `TOWN_LEVELS` = [1, 40, 75, 103, 109]
(`levels.md` §6 rule 3); message layouts `sim/server-messages.tsv`
0x03, 0x07, 0x08, 0x0D, 0x15, 0x59.

## Randomness

None of its own. §2.3 holds only if both copies make the same draws in
the same order (`drlg/*` Randomness sections).

## Edge cases & original bugs

1. Game entry sends the spawn room's 0x07 twice (game entry, then the
   room switch); the second finds the room's count non-zero and does
   nothing (`model.md` §9 rule 1).
2. A tile coordinate above 0xFFFF or below 0 would wrap in the u16
   field; no 1.14d act has one.

## Test vectors

| Input / seed | Expected output | Source (trace id) |
|---|---|---|
| 0x03 act 0 seed 0x103888C4, 0x07 (0x3A0, 0x388) level 1, 0x15 (4673, 4548) | client room of origin tile (928, 904), level 1, holds the player | `20261006-022633` (`app_client_drlg::the_recorded_join_on_the_install`) |
| synthetic app game, seed 1234 | §2.2–§2.5 after the join and on every frame of a walk from the town into the Blood Moor | `seam_drlg_coords.rs` |

## Provenance

Read from the owner specs and both sides of the code by the 2026-10-08
seam audit (implementation session; no `re/`). The tile-origin meaning
of 0x07 is confirmed by the recorded join (`model.md` §9 rule 3).

## Open questions

1. §2.3 on the user's files: run `seam_drlg_coords` against live data
   (a game-file variant) across several levels and an act change.
