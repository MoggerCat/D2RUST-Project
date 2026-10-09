# Spec: Seam — Movement: client model ↔ walk prediction ↔ server movement

- **Status:** draft. Contract written from both sides' specs and code
  (2026-10-08 seam audit, M23); contract tests in
  `crates/d2-client/src/bridge/seam_movement_tests.rs`; nothing here is
  verified against 1.14d beyond what the owner specs state.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge::{predict, client_path, check,
  click, motion, msg::units}` ↔ `d2-sim::path::walk::{request, resync,
  velocity, step, messages}`, `d2-sim::combat::vitals::sync`,
  `d2-sim::monsters::mode_message`, `d2-server::adapters::handlers::walk`
- **Related specs:** `sim/pathing.md`, `sim/path-placement.md`,
  `sim/tick.md`, `sim/intents-events.md`, `combat/vitals.md`,
  `client/bridge.md`, `client/model.md`, `client/msg-units.md`,
  `ui/controls.md`

## Summary

This spec owns no behaviour. It is the contract of the boundary where
the local player's movement crosses between the client and the server:
what each message carries, in which unit, coordinate space and fixed-point
scale, which side owns each piece of state, and in which order the two
sides act within one frame. Every rule cites the owner spec that states
the behaviour; a rule here only says what both sides must agree on, so a
test can call both sides on the same value.

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S 0x01–0x04, 0x5F | bytes | client controls (`ui/controls.md` §6 r7), position check (`client/model.md` §6 r8) |
| S→C 0x0D, 0x0F, 0x10, 0x15, 0x18, 0x95, 0x96, 0x67, 0x68 | bytes | server update pass, placement, vitals sync |
| charstats `WalkVelocity`, `RunVelocity`, `RunDrain` | table | both sides |

## Outputs / state changes

None of its own: the owner specs' state changes, as listed per rule.

## Rules

### 1. Units and spaces used below

1. **Sub-tile** (cell): the integer world grid every message carries
   (`u16` on the wire). **Precise**: 16.16 sub-tiles; a cell's centre is
   `(c << 16) | 0x8000`, a precise value's cell is its high 16 bits
   (`sim/path-placement.md` §1 r2). Both sides use the same two
   conversions.
2. **Tick**: one server game frame (40 ms, `sim/tick.md` §1 r1). **Bridge
   frame**: one client frame (`client/bridge.md` §8 r1). Movement
   distances are per tick, never per bridge frame or per wall-clock time.
3. **Velocity**: path velocity in 1/256 sub-tile units as
   `sim/pathing.md` §8.1 computes it; the distance of one tick is
   `velocity · 0x400 >> 6` precise units (§9.4 r2.1). **Velocity
   percent**: stat 67 (`velocitypercent`), a percentage (base 100 for a
   player).

### 2. Contract

#### 2.1 Walk and run intents (C→S 0x01–0x04)

1. Layout (`sim/pathing.md` §1.1, `ui/controls.md` §6 r7,
   `sim/intents-events.md` §2.4): 0x01 / 0x03 `x:u16@1 y:u16@3` (cells);
   0x02 / 0x04 `type:u32@1 guid:u32@5`. 0x01 / 0x02 walk (mode 2),
   0x03 / 0x04 run (mode 3). The client's sender and its prediction's
   reader (`predict::walk_of`) and the server's dispatcher parse
   (`handlers::walk::handle`, `form`) read the same fields.
2. The point is the clicked cell, clamped by the client
   (`ui/controls.md` §6 r9.8); the server computes the path from its own
   position (`sim/pathing.md` §1.2–§1.5). The client never sends a path.

#### 2.2 Position resync (C→S 0x5F)

1. Layout `x:u16@1 y:u16@3`, 5 bytes (`client/model.md` §6 r8,
   `sim/pathing.md` §1.6 r1).
2. The point is the client's **own** position of the local player (the
   cell `client/model.md` §6 r3 reads), never the server's point.
3. The server measures it with `resync_distance` (max + min / 2) against
   its path cell: < 5 ignored, 15..=45 snap when reachable, otherwise a
   walk (`sim/pathing.md` §1.6 r2–r4). A 0x5F with a stale point makes
   the server walk the player back to it.

#### 2.3 Placement (S→C 0x15)

1. Layout: `type:u8@1 guid:u32@2 x:u16@6 y:u16@8 flag:u8@10`, 11 bytes
   (`sim/pathing.md` §10 r3, `client/msg-units.md` §3). x, y is the
   server's path cell.
2. The client places the model at (x, y) (`client/msg-units.md` §3 r4);
   the walk prediction then restarts at that cell's centre
   (`client/model.md` §3 r3).

#### 2.4 Other units' movement (S→C 0x0D, 0x0F, 0x10, 0x67, 0x68)

1. 0x0F / 0x10: the check point at bytes 0x0C / 0x0E is the server's
   current cell of the walker; 0x0F's target is at 7 / 9
   (`sim/pathing.md` §10 r2, `client/msg-units.md` §4 r1). The walker's
   own client gets neither (§10 r2).
2. 0x0D code 1 to the local player: "walk to (r0, r1)"; the server has
   already made the same walk request on its side
   (`sim/path-placement.md` §12.2 r5–r6, `client/model.md` §8 r4).
3. 0x67 / 0x68 carry the monster's **velocity percent** (stat 67 total),
   `u16` at 13 (0x67) / 18 (0x68) (`sim/intents-events.md` §7.7 r5,
   test vector `4b00` = 75). It is not a path velocity: a client that
   moves the monster between messages must turn it into one with the
   monster's base velocity (`sim/pathing.md` §8.1 r2), as the server does.

#### 2.5 Vitals with position (S→C 0x18, 0x95, 0x96)

1. Bit layouts as `combat/vitals.md` §5.4 / `client/msg-units.md` §5 r1;
   stamina is the total `>> 8` (15 bits), x, y the server's cell.
2. dx, dy = `(X − target) & 0xFF` (`combat/vitals.md` §5.2); the client
   forms `(x + sdx, y + sdy)` = the target reflected through the server
   point (`client/msg-units.md` §5 r3, 1.14d-confirmed both sides) and
   uses it in the check's rule 5 (`client/model.md` §6 r5): a prediction
   lagging behind the server point is kept, one more than the tolerance
   ahead of it is corrected even when it is on the server's path.
3. Stamina crosses at 1/256 of its server precision: the client's stat
   10 is `stamina << 8`, so the client's "stamina 0" means server raw
   stamina < 256, while the server's run turns into a walk at raw ≤ 0
   (`sim/pathing.md` §1.5 step 2, §9.9 r3). This mismatch is 1.14d's
   own: the client does not drain stamina and ends its run on model
   stat 10 = 0 (`client/model.md` OQ2, stamina answer); the client's
   test stays on the model value, not on the server's scale.

#### 2.6 Speed of the local player

1. Both sides compute one tick's distance from the same inputs:
   charstats `WalkVelocity`, stat 67 total (100 + the run list's
   `100 · RunVelocity / WalkVelocity − 100` while running, §8.2), and
   the stat-96 term of §8.1 r2 (floor 25 %). The prediction's
   `Speeds::step` and `ClientPath` equal the server's
   `mode_velocity` · 0x400 >> 6 for walk (2), town walk (6) and run (3).
2. Stats from items and skills (stat 67 beyond the run list, stat 96) are
   server state; the prediction must read them from the model when the
   server sends them, or it walks at a different speed.

#### 2.7 Direction

1. Facing is `dir64` (0–63) from the direction vector of
   `sim/pathing.md` §8.3 between precise points; the prediction and the
   server use the same table and function (`path::walk::geom`).

#### 2.8 Timing and order within one frame

1. Per bridge frame (`client/bridge.md` §8 r1–r3, `sim/tick.md` §1
   r2–r4): the server drains the intents sent in the previous frame,
   runs at most one tick, flushes; the client receives and dispatches
   every message; then the prediction takes the intents of the previous
   frame and steps once if a tick ran. So the server's request and first
   step and the prediction's request and first step fall in the same
   frame.
2. The prediction never steps more than once per tick and never without
   one.

#### 2.9 Ownership

1. The server owns the player's position, path, mode, velocity and
   stamina (CLAUDE.md rule 7). The client model's position is the last
   placement (`client/model.md` §3 r3). The client's **own** position of
   the local player between placements is the walk prediction (OQ2 of
   `client/model.md`); there is exactly one such value per frame.
2. Every client rule that reads "the local player's position" between
   placements reads that own position: the camera and click
   conversion, the controls' distance and range decisions
   (`ui/controls.md` §6 r9), the position check's (cx, cy) and its
   mode-dependent tolerance (`client/model.md` §6 r3–r5), and the 0x5F
   point (§2.2 r2).
3. The check's correction moves the prediction only when it takes the
   server's point (`client/model.md` §6 r8, the play preview's
   `Checked::Followed`).
4. A player mode request that sets a non-walking mode (every code but
   0x00, 0x01, 0x02, 0x17, 0x18: hit, death, a skill) ends the
   prediction's walk, as the server's mode change ends its walk
   (`client/model.md` §8 r4, PROVISIONAL REC-1250).
5. The prediction's path sees the same blockers as the server's: the
   client DRLG's grids, the living monsters (`client/msg-units.md` §3
   r2) and the objects whose mode has collision (§1.3 r2, PROVISIONAL
   REC-1251).
6. The click decisions that act on a unit at once or walk to it first
   (`ui/controls.md` §6 r9.2) measure with the unit distance
   `0x00641530` (`sim/pathing.md` §9.5), the server's test for the same
   message (`items/inventory-moves.md` §7.1, `world/npc.md` §2 r3); with
   another distance the client sends a pick-up or interact the server
   answers with a walk the client does not see.

## Constants & data dependencies

charstats `WalkVelocity`, `RunVelocity`, `RunDrain`; monstats `Velocity`;
`path-tables.tsv` (`velmod_player`, `animstat`, `tan`). Constants are the
owner specs'.

## Randomness

None of its own. The client path's request draws on a client seed where
the server draws on the unit seed (`sim/pathing.md` §1.4 r5, state 42
only); the local player never has state 42 on the client, so no draw is
made there.

## Edge cases & original bugs

- The 0x96 dx = 0x80 reads +128 on the client (`client/msg-units.md`
  edge cases); the server writes −128 as 0x80.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| C→S `01 64 00 c8 00` | client and server read (100, 200), walk | §2.1 |
| server `reassign_player(0, 1, 120, 130, 0)` | client model at (120, 130) | §2.3 |
| `WalkVelocity` 6, `RunVelocity` 9, walk / run | 0x6000 / 0x9000 precise per tick on both sides | §2.6, `sim/pathing.md` V1–V2 |

## Provenance

Both sides' specs and code, read 2026-10-08 (implementation session; no
`re/`). The contract tests call each side's own builder and reader.

## Open questions

1. (Settled, §2.5 r2.) The sign of 0x96 / 0x95 / 0x18 dx, dy: no
   disagreement; 1.14d's client mirrors the target.
2. How 1.14d's client sets its own player's mode while it predicts a walk
   (the tolerance of `client/model.md` §6 r4 reads it): `0x00463390`,
   `client/model.md` OQ2, REC-51.
