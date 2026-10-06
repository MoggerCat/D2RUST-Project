# Spec: Client — World model (units, local player, receive order, session and map messages)

- **Status:** draft: every rule read from the 1.14d `Game.exe` client
  code (addresses below) and checked against the two single-player
  recordings `traces/raw/20261006-015956-packets.jsonl` and
  `-022633-packets.jsonl` (join → town → Act I fighting); unverified:
  no executable check runs it yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge::world` (model), the handlers
  registered in `d2-client::bridge::dispatch::HANDLERS` for the ids this
  spec owns; ids owned by `client/msg-units.md` and
  `client/msg-stats-items.md` use the model defined here.
- **Related specs:** `client/bridge.md` (transport into the model, §2,
  §5, §6, §8); `sim/intents-events.md` §3.4 (client receive loop);
  `client/msg-units.md` (unit add, remove, movement, vitals);
  `client/msg-stats-items.md` (stats, items); `sim/path-placement.md`
  (path records, teleport, cell lookup); `drlg/rooms.md` §4 (room
  status); `sim/rng.md` (seed step); `render/camera.md` §3 (local player
  position), OQ6 (player seed).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 46–61 |
| Inputs | 62–70 |
| Outputs / state changes | 71–77 |
| Rules | 78–79 |
|   1. Model contents | 80–116 |
|   2. Unit table | 117–162 |
|   3. Local player | 163–185 |
|   4. Receive and the unit message queue | 186–223 |
|   5. Client update pass | 224–258 |
|   6. Position check (`0x004804E0`) | 259–298 |
|   7. Session messages | 299–335 |
|   8. Mode requests | 336–386 |
|   9. Room-in-sight messages | 387–407 |
|   10. Bit reader | 408–422 |
| Constants & data dependencies | 423–435 |
| Randomness | 436–447 |
| Edge cases & original bugs | 448–456 |
| Test vectors | 457–486 |
| Provenance | 487–514 |
| Open questions | 515–538 |
<!-- /index -->

## Summary

The 1.14d client keeps its own copy of the world: a unit table filled
and emptied by S→C messages, a pointer to the local player, the client
DRLG act and a few session globals. Messages are handled in two places.
A message whose id has a *general* handler changes the model when it is
received. A message whose id has a *unit* handler is copied into the
addressed unit's message queue at receive time and applied later, in the
client update pass of the same loop pass, after that unit's own per-frame
update. This spec owns the model, the queue, the update-pass order, the
shared position-correction rule, the session messages (0x00–0x06), the
local player message (0x0B) and the room-in-sight messages (0x07, 0x08).
It also defines the two C→S messages the client sends on its own in
answer to S→C messages: 0x6B after 0x02 and 0x5F after a failed
position check of the local player.

## Inputs

| Name | Type | Source |
|---|---|---|
| S→C message | id + bytes, split by the size rule | `client/bridge.md` §2 |
| loop pass tick flag | whether the server ticked in this client loop pass | `client/bridge.md` §8 |
| unit visibility | whether a unit's sprite at a pixel point is drawable (`0x004DBF20`) | Phase 6 render seam (§6 rule 7) |
| mode machines | per unit type, consume a mode request (`0x00480C10`) | Phase 6 seam (§8) |

## Outputs / state changes

- `ClientWorld` fields of §1, changed only by the rules below.
- C→S messages the client itself sends: 0x6B (§7 rule 3), 0x5F (§6
  rule 8). They go through the bridge send path (`client/bridge.md` §4).
- Mode requests handed to the unit-type mode machines (§8).

## Rules

### 1. Model contents

1. `ClientWorld` holds the following, each with its 1.14d storage. A
   field not listed is not part of the model; it is added by the spec
   that owns the message setting it.

   | Field | 1.14d storage | Written by |
   |---|---|---|
   | `units` (server units) | hash set S, `0x007A5E70` | §2; `msg-units.md`, `msg-stats-items.md` |
   | `local_player: Option<UnitKey>` | `[0x007A6A70]` (unit pointer) | §3 |
   | `difficulty: u8` | `[0x007A060C]` | 0x01 (§7 rule 2) |
   | `expansion: u32` | `[0x007A04F4]` | 0x01 |
   | `ladder: u8` (stored as u32) | `[0x007A04F8]` | 0x01 |
   | `game_flags: u32` | `[0x00712EFC]` | 0x01 |
   | `act: Option<ActLoad>` | client DRLG act `[0x007A0634]`; `[0x007A0638]` → {seed, u32@8}; `[0x007A063C]` | 0x03 (§7 rule 4) |
   | `in_game: bool` | `[0x007A061C]` | 0x04 (1), 0x05 (0) |
   | `unloaded: bool` | `[0x007A0624]` | 0x04 (0), 0x05 (1) |
   | `exit_requested: bool` | `[0x007A0620]` | 0x06 |
   | `rooms_in_sight: Vec<RoomSight>` | client DRLG room status (`drlg/rooms.md` §4) | 0x07, 0x08 (§9) |
   | `outgoing: Vec<Vec<u8>>` | client send path | §6 rule 8, §7 rule 3 |

2. `ClientUnit` (1.14d: the 0xF4-byte unit record allocated by
   `0x00620290`, D2MOO `D2UnitStrc`):

   | Field | 1.14d | Meaning |
   |---|---|---|
   | `key` | +0x00 type, +0x0C GUID | `sim/unit-order.md` §1 rule 1 |
   | `class: u32` | +0x04 | class id (players 0–6, `monstats` row, `objects` row, ...) |
   | `mode: u32` | +0x10 | current mode; set only by the mode machines (§8) and creation |
   | `position: Option<(u16, u16)>` | path (+0x2C) cell | subtile cell; `None` = not placed (created at (0, 0), §2 rule 6) |
   | `server_point: (u16, u16)` | +0x8C / +0x8E | last point given to the position check (§6 rule 3) |
   | `stats: BTreeMap<u16, i32>` | stat list +0x5C, layer 0 base | written by the message rules through set (`sim/stat-lists.md` §5 rule 2) or add (§5 rule 3) |
   | `seed: (u32, u32)` | +0x20 / +0x24 | client copy of the unit seed (§2 rule 6) |
   | `queue: Vec<Vec<u8>>` | +0xD8 message queue | §4 |
   | `last_mode_request: Option<ModeRequest>` | — (d2rs) | §8 rule 3 |
   | kind data | +0x14 type data | per kind, owned by `msg-units.md` §1 and `msg-stats-items.md` §2–§3 |

### 2. Unit table

1. The client keeps two hash sets of units, each 6 types × 128 buckets
   of 4-byte list heads (0x200 bytes per type): set **S** at
   `0x007A5E70` (units the server announced) and set **C** at
   `0x007A5270` (client-only units; unit flag +0xC4 bit 0x200000, set by
   the creator `0x00466730` with 0x600000). Bucket = GUID & 0x7F; the
   chain link is unit +0xE4.
2. Every message lookup (`0x00463990(GUID, type)`; `0x00463940` walks
   the bucket) searches set S only. Set C units are never addressed by a
   message and are not part of `ClientWorld` (their creators are client
   effects, Phase 6).
3. **Insert** (`0x00463A50`): the new unit goes before the first chain
   entry whose GUID is ≤ its own, so every chain is in descending GUID
   order. It fails if (type, GUID) is already in the unit's set.
4. **Add** (`0x00465F20`, used by every creation): try insert. On
   failure look the key up in S: the same record → fatal assert 0x1C6;
   none → the new unit is freed and the add returns none; another record
   → that record is removed and freed (rule 5), then insert is retried;
   a second failure frees the new unit. So **a message that adds an
   existing (type, GUID) replaces the old unit**, which loses its queue,
   stats and local-player status.
5. **Remove** (`0x00465EE0(GUID, type)` → `0x00465E80`): unlink from S
   (a key not in S: nothing). An item also runs `0x004879D0`. Then free
   (`0x00465870`): hover text, light, gfx, per-kind data, the message
   queue (`0x0045F9F0`; queued messages are dropped unapplied); if the
   unit is the local player, `0x00453DE0` runs and `local_player` :=
   none.
6. **Creation fields common to every kind** (`0x00466200` players,
   `0x00465FD0` objects / missiles / items / tiles, `0x00466360`
   monsters): type, class, GUID; flag +0xC8 bit 0x2000000 := `expansion`
   ≠ 0; seed := `init()` = {1, 666} (`0x00650E30`). If the given point
   is not (0, 0): room := the room containing it (§2 rule 7), act :=
   that room's act, and the room's seed is stepped once
   (`sim/rng.md` §2) and the unit seed := `init_low(lo')`
   (`0x00650E40`). At (0, 0) there is no room and the seed stays
   {1, 666}.
7. **Room of a point** (`0x00465420(x, y)`): (0, 0) → none. Else the
   cell lookup from the local player's room (`0x00463740`,
   `sim/path-placement.md` §4 rule 1), else the act lookup
   `0x00619DA0([0x007A0634], x, y)`, else fatal assert 0x13C.
8. d2rs: `units` is the `BTreeMap<UnitKey, ClientUnit>` of
   `client/bridge.md` §5 rule 2 holding set S. Where 1.14d order matters
   (the update pass, §5) the order is computed from the keys by §5
   rule 3, not taken from the map.

### 3. Local player

1. `local_player` is set only by S→C 0x0B (`0x0045CC50`): layout type
   u8@1, GUID u32@2. Look the unit up (§2 rule 2); found →
   `0x00463D90`: the previous local player (if any) runs `0x00453DE0`;
   the new one's path gets `0x006488A0(path, 0)`, its class is copied to
   `[0x007A0522]`, then `0x0044BEA0(0)`, `0x00453D90`; `local_player` :=
   it. Not found → unchanged. Both paths then call `0x0046F360` (an
   empty function).
2. It is cleared only by freeing that unit (§2 rule 5), which includes a
   replacing add (§2 rule 4).
3. **The local player position** (the `ViewFeed::player` hook,
   `render/camera.md` §3): the local player's `position` (§1 rule 2). As
   a 16.16 dynamic-path position it is the cell centre:
   `x16 = (x << 16) | 0x8000`, `y16 = (y << 16) | 0x8000`
   (`sim/path-placement.md` §1 rule 2), after every placement this spec
   or `msg-units.md` states (creation, 0x15, a teleport of §6 rule 8).
   Between those, the 1.14d client moves its own player locally on
   input (walk prediction: C→S 0x01–0x04 are sent, the server answers
   only with checks, 0x96 / 0x95 / 0x18) and steps the path every
   client update; that movement is not a message rule (open question 2).
   No local player → `None`.

### 4. Receive and the unit message queue

1. The receive loop (`0x0045F7B0`, `sim/intents-events.md` §3.4 rule
   3) takes each message in order. If its id has a unit handler
   (`server-messages.tsv` `client_unit_handler`) and the unit it
   addresses is in set S (ids 0x67–0x6D: (1, u32@1); others: (u8@1,
   u32@2)), the whole message is appended to that unit's queue
   (`0x0045F730`). A missing unit drops it. Then the general handler
   runs (`client_handler`; for every id with a unit handler it is the
   empty `0x0045C900`).
2. The per-id pre-steps of the loop (0x0D with type byte 1: a monster
   lookup; 0x18, 0x95, 0x96: bit decodes `0x0045D900`, `0x0045DA90`,
   `0x0045DBE0` into a scratch buffer) write nothing the model or any
   handler reads: they have no effect. The general handlers of 0x18,
   0x95, 0x96 decode again themselves (`msg-units.md` §5).
3. Queue record (`[unit +0xD8]`): capacity (slots), count, data. Slot
   size `[0x007A5230]` = the largest receive-table expected size among
   ids with a unit handler + 5 = 0x68's 21 + 5 = 26 bytes, computed on
   first use (`0x0045F610`). Full queue → capacity += 5 (reallocate,
   copy). A message longer than a slot → fatal assert 0x127F (cannot
   happen: every id with a unit handler is ≤ 21 bytes).
4. Ids 0x6E–0x72 (1 byte) have a unit handler; the lookup reads the
   bytes after the message in the receive node. Their unit handlers
   (`0x0045D0A0`–`0x0045D0E0`) are a bare `ret`, so a queued copy has no
   effect when drained. d2rs: no addressed unit, no queue entry
   (`client/bridge.md` §5 rule 4); equal in effect.
5. **Drain** (`0x0045FA40(unit)`), called from the per-unit update
   (§5 rule 2): count > capacity → fatal 0x1525. For each slot in order:
   re-apply the size rule (result 0 → stop), id ≥ 0xAF → fatal 0x1531,
   size ≠ expected → fatal 0x1534, no unit handler → fatal; call the
   unit handler with (unit, message). Then zero the used slots and set
   count := 0.
6. Consequences: a unit-handler message takes effect after every
   general handler of the same receive, in update-pass order (§5), not
   in receive order. A message for a unit added later in the same
   receive is dropped. A unit removed in the same receive drops its
   queue (§2 rule 5).

### 5. Client update pass

1. When: the client loop (`0x0044EFA0`) runs the receive (`0x0044C6E0`)
   every pass and, in single player (game type `[0x007A0610]` 0 or 1),
   the client update `0x0044C790` only in a pass where the server ticked
   and only while `in_game` (§7 rule 5). d2rs: the bridge runs the pass
   after `receive` in a frame whose pump ran a tick, while `in_game`
   (`client/bridge.md` §8 rule 1).
2. Per unit (`0x00480810`): unless the unit is not the local player
   and has unit flag 0x800000 (rule 5), run the per-type update
   (player `0x00463390`, monster `0x004B13A0`, object `0x004BDFF0`,
   missile `0x004D2C70`, item `0x004C1AD0`; Phase 6), then look the unit
   up again by (type, GUID) in its own set and, if it still exists,
   drain its queue (§4 rule 5).
3. Order (`0x00465AA0`, the first part; buckets 0..127, each chain in
   its order, i.e. descending GUID): S missiles, C missiles, C objects,
   S players, S monsters, S objects, S items, C monsters. The S sets are
   walked by `0x00463C90` (next link read before the unit runs, so a
   unit may free itself); the C sets by `0x00463CC0`. The rest of
   `0x00465AA0` and of `0x0044C790` runs no handler (Phase 6).
4. d2rs order for the queue drains: types in the order above (S only:
   missiles 3, players 0, monsters 1, objects 2, items 4), then
   `GUID & 0x7F` ascending, then GUID descending.
5. Unit flag 0x800000 is set only by the active-room free `0x0061A840`
   (`drlg/rooms.md` §8 rule 4: a client room unloaded under the unit;
   no other instruction in `Game.exe` sets it). In the update pass a
   unit other than the local player with this flag runs nothing of rule
   2 (no per-type update, no queue drain); when its flag-ex (`+0xC8`)
   bit 0x20 is set (the room free sets it for units without flag
   0x400000) the client sends C→S 0x4B (9 bytes: 0x4B, type u32, GUID
   u32; `0x004786A0`, appended to `outgoing`) and clears 0x800000 and
   0x20; then a client-only unit (flag 0x200000, set C) is removed
   (`0x00465F00` → `0x00465E80`); a server unit stays until the server
   answers.

### 6. Position check (`0x004804E0`)

`check(U, x, y, kind, tx, ty)`, called by the message rules of
`msg-units.md` (kind 0) and by the player mode machine (codes 6, 0x12,
0x13 with kind 1; codes 8, 0x19 with kind 0; Phase 6).

1. x = 0 or y = 0 → nothing.
2. U dead (`0x00464820`: unit flag 0x10000; player mode 0 or 0x11;
   monster mode 0 or 0xC) → nothing.
3. `server_point` := (x, y). (cx, cy) := U's position (path cell;
   static-path kinds 2, 4, 5 read the static path).
4. Tolerance T: kind 1 → 10; kind 2 → 0; any other kind: U is the local
   player → mode 1 → 3 + L, mode 3 → 7 + L, other modes → 5 + L, with
   L = (`[0x007A04A4]` + 0x32) >> 7 (L = 0: the global is only ever
   zeroed, by the `0x007A0480` block clears of `0x0044E200` and
   `0x0044C890`; no other writer); U is a
   monster in mode 3–5 → 5, mode 6–11 → 7; otherwise 15.
5. far := |x − cx| > T. If far or |y − cy| > T: if kind = 0 and tx > 0
   (signed): d1 := (cx − x)² + (cy − y)² (`0x006492A0`); d1 ≥ 100 →
   rule 8; d2 := (cx − tx)² + (cy − ty)²; d2 ≥ d1 → rule 8; else
   (the unit is already closer to the target than to the stated point)
   far := false and continue with rule 6. Kind ≠ 0 or tx ≤ 0 → rule 8.
6. (Within tolerance, or accepted by rule 5.) If x = cx or y = cy: go
   to rule 7 as visible. Else (a, b) := U's client pixel point
   (`0x00620650`, `0x006206B0`: static path +0x04 / +0x08, dynamic path
   through `0x006489C0` / its y twin), (x', y') := ((x − y) · 16,
   (x + y) · 8) (`0x00643260`), and visible := `0x004DBF20(U, a, b)` or
   `0x004DBF20(U, x', y')`.
7. Visible: far → rule 8, else nothing. Not visible (neither point):
   rule 8.
8. **Correct**: room' := cell lookup from U's room (`0x00463740`), else
   act lookup `0x00619DA0`; none → nothing. U is the local player → the
   client does not move it; it sends C→S **0x5F** (`0x004785D0` →
   `0x00478350`, 5 bytes: 0x5F, x u16 = cx, y u16 = cy: its own
   position) and appends it to `outgoing`. U is another player →
   `0x00463180(U, room', x, y)` (re-places and restarts walking modes;
   other players are out of scope, `sim/intents-events.md` §4 rule 4).
   Else → teleport (`0x00650C60` → `sim/path-placement.md` §6 rule 4:
   position := (x, y), room recache) and path +0x38 := 0xF.

### 7. Session messages

1. **0x00** GameLoading: general handler `0x0045C900` (empty). No
   effect.
2. **0x01** GameFlags (`0x0045C8B0`): `difficulty` := u8@1, `expansion`
   := u8@6 (zero-extended), `ladder` := u8@7, `game_flags` := u32@2;
   then `0x00456970` (in-game UI set-up, Phase 6).
3. **0x02** LoadSuccessful (`0x0045C910` → `0x00477DA0`): the client
   sends the system message **0x6B** (1 byte) through `0x0052AE50`
   (appended to `outgoing`; queue system, `client/bridge.md` §4 rule 2)
   and counts two send counters.
4. **0x03** LoadAct (`0x0045C8E0` → `0x0044E100(act, u32@2, u32@8,
   u16@6)`): the act number is stored (`0x0044DB60`); an existing client
   act is freed (`0x0061AFD0`); a new client DRLG act is built
   (`0x006194A0(act, init seed = u32@2, client 1, no game,
   difficulty, no pool, town level = u16@6, automap callbacks
   0x00459150 / 0x004591A0)`, `drlg/levels.md` §2 rule 3, §3: the
   client's DRLG copy is seeded like the server's) and stored in
   `[0x007A0634]`; `[0x007A0638]` := {u32@2, u32@8}; `[0x007A063C]` :=
   u16@6. In between, act 1 with a pending flag `[0x007A060E]` runs
   `0x0061C240`; then UI, automap and sound set-ups (`0x00454790`,
   `0x004565E0`, `0x0045A620`, `0x00475B40`, `0x00452130`,
   `0x0046F440`, `0x0046F310`; Phase 6). `act` := {act u8@1, init seed
   u32@2, town level u16@6, u32@8}.
5. **0x04** LoadComplete (`0x0045C9A0`): `in_game` := true, `unloaded`
   := false. The local player must have a room (`0x004646A0`), else
   fatal assert 0x527. `0x00470B10` with that room's act (Phase 6). The
   local player's player data (+0x14) fields +0x150, +0x154, +0x158,
   +0x15C := 0 (if the local player is a player with data).
6. **0x05** UnloadComplete (`0x0045CA30`): `0x0044C880`,
   `0x00409B80(0)`; `in_game` := false; `unloaded` := true;
   `[0x007A0524]` := `GetTickCount()` (wall clock; not modelled).
7. **0x06** GameExit (`0x0045CA60`): game types 7 and 9 (hosted;
   out of scope) may call `0x0044E380(8)`; types 2 and 3 with `in_game`
   call `0x0044E380(7)`; single player (type 0) does neither;
   `exit_requested` := true.

### 8. Mode requests

1. Every unit-handler message of `msg-units.md` §4 ends in a **mode
   request** `0x00480C10(code, U, record, 1)`: `code` u8, `record` 7
   signed 32-bit values. It dispatches on U's type: player
   `0x00461250`, monster `0x004AFF60`, object `0x004BD6D0`, missile:
   nothing (returns 1), item `0x004C1B80(record[0], record[1])`.
2. What a mode machine does with a request (mode, path, animation
   start, the position checks it runs) is client animation behaviour:
   Phase 6; the dispatch per code is rules 4–6, the monster machine is
   open question 1.
3. d2rs: until that spec exists the model stores the request as
   `last_mode_request = {code, record}` on the unit; record entries
   1.14d leaves unset (stack contents) are 0 in the model and marked in
   `msg-units.md` §4.
4. **Player** (`0x00461250`, ECX code, EDX unit, stack record, flag):
   first the unit's cast light is detached and removed (`0x00643A00(U,
   0)`, `0x004743D0`); unless code = 0x13, `0x004611F0(U is the local
   player)` and `0x00620210(U, 0)`. A unit in mode 0x13 with flag 0
   ignores the request (returns 1); with flag 1 its path is stopped
   (`0x00650590`). Then `0x00648DC0(path)`. Neutral / walk modes: 5 / 6
   when the unit's room is in town (`0x0061AB00`), else 1 / 2.

   | Code | Effect |
   |---|---|
   | 0x00 | `0x00480780(U, r0, r1)`; mode := walk (`0x00480E70`) |
   | 0x01 | `0x004804A0(U, r0, r1)`; mode := walk |
   | 0x02 | `0x00480930(r0 & 0xFFFF, r1)` |
   | 0x06 | `+0xB0` := r2; mode := 4; `check(U, r0, r1, 1, 0, 0)` |
   | 0x07 | was-dead := `0x00464820(U)`; `0x004647D0(U)`; flag 0x2 set; mode := neutral; with a record `0x00480EF0(U, r0, r1, was-dead)` |
   | 0x08 | local player → `0x00456300(1, 0)`; `+0xB0` := r2; U the hover target (`0x00467A10`) → `0x00466FE0` unless `0x0044BF00` or `0x0044BF10` has bit 8, else `0x0044DA40` + `0x00467A70(0)`; `check(U, r0, r1, 0, 0, 0)`; mode := 0; `0x00461010(U)`; `0x0045C470(U)` |
   | 0x09 | mode := 0x11; `0x00461010(U)` |
   | 0x12 | `+0xB0` := r2; mode := 9 when `0x0063C8F0(inventory, 0)` or the COF weapon class (`0x0064F380`) is 0xD; `check(U, r0, r1, 1, 0, 0)` |
   | 0x13 | `+0xB0` := r2; `0x00480D20(U, 0x13)`; `0x004CC5B0(U, 3 if 0x0063A400(U) else 0x13, 1)`; `check(U, r0, r1, 1, 0, 0)` |
   | 0x14 | `+0xB0` := r2; path: stop, type 0xB (`0x00648CF0`), `0x00648E40(path, 5)`, target (r0, r1) (`0x00648AD0`), compute (`0x00649970`); mode := 0x13 |
   | 0x15 | target fix-up `0x00461180(U, record, 0)`, then `0x004C6F40(U, record)` (client skill start, `render/lighting.md` §8 r3) |
   | 0x16 | the same with `0x004C6EB0` |
   | 0x17 | `0x004804A0(U, r0, r1)`; mode := 3 |
   | 0x18 | `0x00480780(U, r0, r1)`; mode := 3 |
   | 0x19 | mode := 0xD; `check(U, r0, r1, 0, 0, 0)` |
   | 3–5, 0x0A–0x11, > 0x19 | fatal 0x432 |

   A helper of codes 0, 1, 0x15–0x18 returning 0 sends the unit to the
   neutral mode (`0x00460830(U, neutral, 1)`) and the request returns 0.
5. **Object** (`0x004BD6D0`): code 3 → `0x004BCF60(U, record)` (object
   mode change: lights `render/lighting.md` §8), then `0x004BD650` when
   `0x00621B00(U)`; code 0x15 → `0x004BD5C0(record)`; any other code is
   fatal 0x39C.
6. **Item** (`0x004C1B80`): code 2 → mode := r1 (`0x00624690`), unit
   flag 0x2 := (r0 ≠ 0); other codes do nothing.

### 9. Room-in-sight messages

1. **0x07** MapReveal (`0x0045CAB0`): x u16@1, y u16@3, level u8@5.
   The client act must exist (fatal 0x58A). Calls `0x0061A070(act, level,
   x, y, local player's room or none)` → `0x0061B640`: find the level
   (`0x00642BB0`), find its DRLG room at tile point (x, y)
   (`0x00642C30`; the local player's DRLG room is the hint only when it
   is in that level); if the room's status count (+0x0E) is 0, set it
   in sight with propagation (`0x0061B490`, status 1; `drlg/rooms.md`
   §4).
2. **0x08** MapHide (`0x0045CB20`), same fields, fatal 0x59E without an
   act, `0x0061A0C0` → `0x0061B690`: the same lookup; a room with count
   ≠ 0 gets count −= 1, the act's unset callback (`[0x00744398]`) and
   un-propagation (`0x0061B5B0(room, 2)`).
3. In the recordings (x, y) are room origins in tiles (multiples of 8,
   e.g. (0x3A0, 0x388) of level 1); the act's DRLG builds the room and
   its tiles when it comes in sight (`drlg/rooms.md` §4, §9).
4. d2rs: the client DRLG is not in the model yet (open question 5);
   the handlers append `{show, level, x, y}` to `rooms_in_sight` in
   order, which is the input the map-tile feed (RW2) needs.

### 10. Bit reader

Shared by the bit-packed messages (0x18, 0x95, 0x96, 0xAC, 0xA8, 0xAA,
the 0x9C / 0x9D item stream).

1. Init (`0x00410E40(reader, buffer, bytes)`): capacity = bytes × 8
   bits, position 0; a null buffer is fatal assert 0x13C.
2. Read n unsigned bits (`0x00411020` → `0x00410F60`): bits are taken
   from the lowest unread bit of the current byte upward, byte by byte;
   the first bit read is bit 0 of the result. If fewer than n bits
   remain, only the remaining bits are read (the missing high bits are
   0) and the reader's overflow flag (+0x10) := 1.
3. Read n signed bits (`0x00411030`): read unsigned; if n < 32 and bit
   n − 1 is set, sign-extend.

## Constants & data dependencies

| Constant | Value | Source |
|---|---|---|
| hash sets S / C | `0x007A5E70` / `0x007A5270`, 6 × 128 heads | §2 |
| bucket | GUID & 0x7F | `0x00463990` |
| unit record | 0xF4 bytes | `0x00620290` |
| queue slot | 26 bytes (21 + 5), growth 5 slots | `0x0045F610` |
| local player | `[0x007A6A70]` | `0x00463DD0`, `0x00463DE0` |
| tolerances | 10, 0, 3/7/5 (+L), 5/7, 15 | `0x004804E0` |
| correction distance | d² ≥ 100 | `0x00480645` |
| client-only unit flag | +0xC4 0x200000 (with 0x400000) | `0x00466437`, `0x004667BF` |

## Randomness

1. Unit creation at a point (§2 rule 6): one step of the room's seed
   (`sim/rng.md` §2), unit seed := `init_low(lo')`.
2. Player creation, unless the new player is already the local player
   (`0x00460BF0`): one more step of the unit seed (+0x20) and
   `0x006488A0(path, lo' & 0xFFFFFF3F)`. At a single-player join 0x59
   comes before 0x0B, so the local player is none at that moment and
   the step runs on {1, 666}: seed := {0x6AC6935F, 0} (recorded joins
   have (x, y) = (0, 0), so rule 1 does not run). This answers
   `render/camera.md` OQ6 up to later draws (open question 6).

## Edge cases & original bugs

- §6 rule 7 corrects an off-screen unit even inside the tolerance.
- 0x6A / 0x6C and 0x0D / 0x0E / 0x0F / 0x10 leave record entries unset
  (`msg-units.md` §4); the model uses 0.
- A replacing add (§2 rule 4) of the local player clears
  `local_player` until the next 0x0B.
- `[0x007A04A4]` has no direct writer (§6 rule 4, open question 3).

## Test vectors

From `traces/raw/20261006-022633-packets.jsonl` (seq numbers), unless
marked synthetic.

| Input | Expected | Source |
|---|---|---|
| 0x01 `01 00 04 00 10 00 01 00` | difficulty 0, game_flags 0x00100004, expansion 1, ladder 0 | seq 5 |
| 0x00 `00` | no change | seq 6 |
| 0x02 `02` | `outgoing` = [`6B`] (system queue) | seq 7; recorded C→S 0x6B at seq 101 |
| 0x59 (player 1, at (0, 0)) then 0x0B `0b 00 01 00 00 00` | local_player = (0, 1); player seed {0x6AC6935F, 0} | seq 102, 113 |
| 0x0B for an unknown (0, 9) | local_player unchanged | synthetic |
| 0x03 `03 00 c4 88 38 10 01 00 61 d1 e0 9f` | act = {0, init seed 0x103888C4, town level 1, 0x9FE0D161} | seq 142 |
| 0x07 `07 a0 03 88 03 01` | rooms_in_sight += {show, level 1, 0x3A0, 0x388} | seq 144 |
| 0x08 `08 d0 03 60 04 01` | rooms_in_sight += {hide, level 1, 0x3D0, 0x460} | `-015956` seq 7231 |
| 0x04 `04` with local player placed | in_game true | seq 219 |
| 0x04 with no local player | fatal assert 0x527 (bridge: handler error) | synthetic |
| receive [0xAC adds (1, 6); 0x6D for (1, 6)] | 0x6D queued on (1, 6), applied in the update pass | seq 157, 159 |
| receive [0x6D for (1, 6); 0xAC adds (1, 6)] | 0x6D dropped (unit absent at receive) | synthetic |
| receive [0x6D for (1, 6); 0x0A removes (1, 6)] | queue dropped, unit gone | synthetic |
| add (1, 6) twice | second replaces first; first's queue dropped | synthetic |
| monsters (1, 5), (1, 0x85), (1, 0x105) (all bucket 5) and (1, 6) | drain order 0x105, 0x85, 5, 6 | synthetic, §5 rule 4 |
| check(local player at (100, 100), mode 1, (104, 100), kind 0, tx 0) | far (4 > 3): correction → `outgoing` += `5F 64 00 64 00`, position unchanged | synthetic |
| check(monster mode 1 at (100, 100), (110, 100), kind 0, tx 0) | 10 ≤ T 15, y = cy (visible): nothing; `server_point` (110, 100) | synthetic |
| check(monster mode 1 at (100, 100), (110, 105), kind 0, tx 0), both points not visible | teleport to (110, 105) | synthetic, §6 rule 7 |
| check(monster mode 4 at (100, 100), (106, 100), kind 0, tx 0) | far (6 > 5): teleport to (106, 100) | synthetic |
| check with kind 0, tx > 0, d1 = 50, d2 = 20 | accepted (rule 5), no teleport if visible | synthetic |
| check with x = 0 | nothing | synthetic |
| frame where the server did not tick | no update pass: queued messages wait | §5 rule 1 |

## Provenance

1.14d `Game.exe`, read in a spec session (Ghidra exports and
`tools/ghidra/disasm.py`): receive loop `0x0045F7B0`; queue
`0x0045F610`, `0x0045F730`, `0x0045FA40`, `0x0045F9F0`; unit sets
`0x00463940`, `0x00463990`, `0x004639B0`, `0x00463A50`, `0x00465E80`,
`0x00465EE0`, `0x00465F20`, `0x00465870`; creation `0x00466200`,
`0x00465FD0`, `0x00466360`, `0x00460BF0`, `0x00465420`; local player
`0x0045CC50`, `0x00463D90`; update `0x0044EFA0`, `0x0044C790`,
`0x00465AA0`, `0x00463C90`, `0x00463CC0`, `0x00480810`; check
`0x004804E0`, `0x00464820`, `0x00643260`, `0x004DBF20`, `0x004785D0`;
mode request `0x00480C10` (jump table `0x00480C68`); session
`0x0045C8B0`, `0x0045C8E0`, `0x0044E100`, `0x0045C910`, `0x00477DA0`,
`0x0045C9A0`, `0x0045CA30`, `0x0045CA60`; rooms `0x0045CAB0`,
`0x0045CB20`, `0x0061A070`, `0x0061A0C0`, `0x0061B640`, `0x0061B690`.
D2MOO's `D2UnitStrc` (1.10f) names the record fields (+0xD8
`pPacketList`, +0xE4 `pListNext`); every offset used here was read in
1.14d code. Message bytes and order checked against both recordings
(join order 0x01, 0x00, 0x02, then frame 1: 0x59, 0x0B, ..., 0x03,
0x07 × 10, 0x15; frame 2: monsters, objects, 0x04). No C→S 0x5F occurs
in either recording (no correction of the local player happened).
Ghidra backlog (2026-10-06): mode machines `0x00461250` (jump tables
`0x004616A4` / `0x004616E4` read from the file), `0x004BD6D0`,
`0x004C1B80`; flag 0x800000: every write of unit `+0xC4` in the export
(only `0x0061A840` sets bit 23), update `0x00480810`, C→S 0x4B
`0x004786A0`; `[0x007A04A4]`: all references (reader `0x0044CE60`,
block clears `0x0044E200` / `0x0044C890`).

## Open questions

1. The mode machines: dispatch of the player, object and item machines
   answered in §8 rules 4–6. Open: the monster machine `0x004AFF60`
   (3,678 bytes, 10 callers) per code, and the effects of the helpers
   the tables name (`0x00480E70` mode set, `0x004804A0` / `0x00480780`,
   `0x00480930`, `0x00480EF0`, `0x004BCF60`, `0x004BD5C0`): Phase 6
   client unit-modes spec.
2. Local walk prediction and per-update path stepping of the local
   player (input → path, `0x00463390`): needed for a smooth
   `ViewFeed::player`; Phase 6 movement spec; check against
   `record_frames.py` positions.
3. ~~`[0x007A04A4]`~~: answered in §6 rule 4 (only zeroed; L = 0). A
   memory read during play confirms.
4. ~~Unit flag 0x800000~~: answered in §5 rule 5 (room free
   `0x0061A840`).
5. The client DRLG (act build, rooms in sight, tiles) as a d2rs
   component: which spec owns it (`drlg/` client parts, map-tile feed
   RW2).
6. Later draws on the local player's client seed (animation, sounds)
   before a shake reads it (`render/camera.md` OQ6).
7. The visibility predicate `0x004DBF20` (§6 rule 6): Phase 6 render
   seam; until then d2rs takes it as an input of the check.
