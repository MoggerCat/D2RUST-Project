# Spec: Flows — Act change, waypoint, portal and warp arrival

- **Status:** draft: a flow spec. It owns no rule: each step names its
  owner spec and 1.14d address. Audit: `docs/handoff/q-tick-flow.md`
  (2026-10-08).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim` waypoints, warp, act change, placement;
  `d2-sim::tick` client pass (state 5); `d2-client::bridge::msg::session`
- **Related specs:** `world/waypoints.md` §6–§7, §11 (owner: validation,
  travel, the warp and the act change); `sim/path-placement.md` §10–§12
  (placement, level spawn, warp tiles); `monsters/population.md` §1 r2
  (off-tick population); `sim/tick.md` §6 (state 5, room switch);
  `client/model.md` §7, §11 (0x05, 0x03, level and palette);
  `world/npc.md` §8.3 (NPC travel); `flows/server-tick.md`.

## Summary

Every travel that changes the player's level goes through one of three
server paths, all inside a C→S handler (drain) or a timer event (tick
step 4): the level warp `0x0053AEC0` (same act: spawn search and
placement; other act: the act change `0x0053ACC0`), the warp-tile
arrival `0x005550B0`, or an object or skill placement
(`0x00554EA0`, e.g. a town portal). Only the act change moves the
client to state 5 and resends the act; 0x04 then comes from the tick's
client pass when the new room is ready. A same-act travel is a
placement: S→C 0x15 at the next update, rooms by the room switch.

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S 0x49 (waypoint), 0x13 (warp tile), object operate, NPC travel | messages | client |
| destination level, tile code | ints | caller (`world/waypoints.md` §11 table) |

## Outputs / state changes

Player position, room, act (act change); client state (5 → 4); the
client's act, rooms, units.

## Rules

### 1. Act change (`0x0053ACC0`)

Owner: `world/waypoints.md` §11 (steps 1–19 in that order). Called only
by the level warp, after the town-leave refresh `0x00537340`, when the
destination act differs from the client's.

1. Arena flag 2 → nothing. Same act → fatal. Build act A if missing.
2. **Client state := 5** (step 4), before the spawn search; a failed
   spawn search leaves state 5 and sends nothing (steps 8–9).
3. Leave the old room (step 10, removal to O's other clients); enter R
   (step 11); room switch to none (0x0A, 0x08 of the old act, step 12);
   **0x05** (13); client act := A (14); unit act := A (15); **0x03** then
   **0x53** (16); room switch to R (**0x07** and the new act's unit adds,
   17); update queue, flag-ex 0x10000 (**0x15** at the update), room
   change messages (18); pets follow (19).
4. **0x04**: not sent by the act change. The next tick's client pass,
   state 5, room ready → 0x04, state 4, inventory refresh
   (`sim/tick.md` §6 r4). No join sequence.
5. The caller's own steps follow (waypoint arrival 0x0D, NPC travel's
   quest steps) (`world/waypoints.md` §11 last paragraph).

### 2. Same-act travel

1. Waypoint: validation (`world/waypoints.md` §6.2), travel §7 r1–r4,
   warp same act: spawn search `0x0061B060` then placement
   `0x00554EA0(exact 0, alt 0)` (`sim/path-placement.md` §10, §11),
   then the arrival 0x0D (§7 r7).
2. Warp tile (C→S 0x13): `sim/path-placement.md` §12.2 rules 1–6: find
   the destination tile, free point, quest gate, place, walk-out mode
   request, **0x0D**.
3. Portal object / town portal: object operate code places the player
   with `0x00554EA0` (`sim/path-placement.md` §10 callers). The
   destination room is populated off-tick inside the caller
   (`monsters/population.md` §1 r2, `0x0052D0F0`), so its units exist
   before the next tick's client pass sends them. No portal crosses
   acts (`world/waypoints.md` §11 last paragraph).
4. In every same-act case the client stays in state 4. The room switch
   (0x07, adds, removals) and 0x15 come from the next tick's client
   pass (`sim/tick.md` §6 r5), not from the placement.

### 3. Client side

Owner: `client/model.md` §7, §11.

1. 0x05: `in_game` := false, `unloaded` := true (§7 r6). The update
   pass stops (`flows/client-frame.md` §1 r5).
2. 0x03: the client act is freed and rebuilt, the act-load black frame
   armed (`render/composition.md` §3 r4), act-load hold (`render/camera.md`
   §9 case 2).
3. 0x07 / 0x08: rooms in sight; 0x15: the local player's room, hence
   the level (§11 r3).
4. 0x04: `in_game` := true (§7 r5).
5. Palette: the act from 0x03; a level whose Levels `Pal` differs
   switches on the room change (§11 r4).

## Constants & data dependencies

Tile codes per caller (`world/waypoints.md` §11 table); spawn record
classes (`drlg/levels.md` §10).

## Randomness

Spawn search `roll(n)` on the level seed (`drlg/levels.md` §10 r2);
population draws of the off-tick path inside the caller.

## Edge cases & original bugs

1. A failed spawn search after step 4 leaves the client in state 5 with
   the player in the old act (`world/waypoints.md` §11 r8, test vector).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| act change, client act 3, level 109, tile 5 | 0x05 before 0x03; P in the new room only | `world/waypoints.md` test vectors |
| act change, spawn none | state 5, no message | `world/waypoints.md` §11 r8 |

## Provenance

Compiled 2026-10-08 (q-tick-flow) from the cited owner specs.

## Open questions

1. Unspecified: the recorded message order of an act change across the
   flush (owner open question `world/waypoints.md` OQ1); the order
   above is the static reading.
2. Unspecified: whether the client's world-view caches (ground items,
   missiles, automap) are reset on 0x03 in 1.14d beyond the act free
   (`0x0061AFD0`) of `client/model.md` §7 r4; the owner names the UI,
   automap and sound set-ups as Phase 6.
