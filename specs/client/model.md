# Spec: Client — World model (units, local player, receive order, session and map messages)

- **Status:** draft: every rule read from the 1.14d `Game.exe` client
  code (addresses below) and checked against the two single-player
  recordings `traces/raw/20261006-015956-packets.jsonl` and
  `-022633-packets.jsonl` (join → town → Act I fighting); unverified:
  no executable check runs it yet. Implemented in `d2-client::bridge`
  (§12 r1 client DRLG: `bridge::drlg`, `impl-client-drlg` 2026-10-07;
  the recorded join vector on game files is `docs/HANDOFF.md` §5 C81).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge::world` (model),
  `d2-client::bridge::drlg` (the client DRLG of §12), the handlers
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
| Summary | 57–74 |
| Inputs | 75–84 |
| Outputs / state changes | 85–91 |
| Rules | 92–93 |
|   1. Model contents | 94–137 |
|   2. Unit table | 138–183 |
|   3. Local player | 184–206 |
|   4. Receive and the unit message queue | 207–244 |
|   5. Client update pass | 245–293 |
|   6. Position check (`0x004804E0`) | 294–334 |
|   7. Session messages | 335–506 |
|   8. Mode requests | 507–591 |
|   9. Room-in-sight messages | 592–626 |
|   10. Bit reader | 627–641 |
|   11. Current act and level (join and later) | 642–687 |
|   12. Client DRLG and the room of a point | 688–729 |
|   13. Visibility predicate (`0x004DBF20`) | 730–757 |
|   14. Pet list and the hireling GUID | 758–811 |
|   15. Object mode requests in detail (codes 3 and 0x15; shrines) | 812–878 |
|   16. C→S 0x4B after a teleport (the hireling case) | 879–913 |
|   17. Model writes made by 1.14d UI code | 914–1060 |
|   18. Audio driver inputs and the client object functions | 1061–1091 |
| Constants & data dependencies | 1092–1104 |
| Randomness | 1105–1116 |
| Edge cases & original bugs | 1117–1128 |
| Test vectors | 1129–1176 |
| Provenance | 1177–1255 |
| Open questions | 1256–1361 |
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
local player message (0x0B) and the room-in-sight messages (0x07, 0x08),
the current act and level (§11), the client DRLG's room-of-point (§12),
the visibility predicate (§13) and the pet list (0x7A, 0x81; §14).
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
| `now: u32` | wrapping milliseconds (1.14d `GetTickCount`); host clock live, scripted in tests / replays | bridge; read by §5 rule 2, §8 rule 7 (`world/objects-client.md` §25 r6) |

## Outputs / state changes

- `ClientWorld` fields of §1, changed only by the rules below.
- C→S messages the client itself sends: 0x67 (§7 rule 9), 0x6B (§7 rule 3), 0x5F (§6
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
   | `in_game: bool` | `[0x007A061C]` | 0x04 (1), 0x05 (0), 0xB4 through the UI (0, §7 rule 8) |
   | `unloaded: bool` | `[0x007A0624]` | 0x04 (0), 0x05 (1) |
   | `exit_requested: bool` | `[0x007A0620]` | 0x06; 0xB4 through the UI (§7 rule 8) |
   | `connected: bool` | `[0x007A0618]` | 0xAF (1), 0xB0 (0), 0xB4 through the UI (0) (§7 rule 10) |
   | `ping: PingState` | `[0x007A04A0]` … `[0x007A04F0]` | the ping timer and 0x8F (§7 rule 11) |
   | `town_flag: bool` | `[0x007A5260]` | player creation (`msg-units.md` §1.1 r3), §17 rule 6 |
   | `rooms_in_sight: Vec<RoomSight>` | client DRLG room status (`drlg/rooms.md` §4) | 0x07, 0x08 (§9) |
   | `outgoing: Vec<Vec<u8>>` | client send path | §6 rule 8, §7 rule 3 |
   | `pets: Vec<PetRecord>` | pet list `[0x007BB5BC]` | 0x7A, 0x81 (§14) |
   | `palette_act: Option<u8>` | `[0x007A288C]`, then room-change switches | 0x03, §11 rules 2 and 4 |

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
   | `interact_ms: u32` | +0xD4 | monster interact gate (§8 rule 7); objects' `ClientFn` timer T (`world/objects-client.md` §25 r5) |
   | audio inputs | +0x30 … +0x88, +0xB0, monster data +0x16 / +0x26 | §18 rule 1 |
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
   The object update `0x004BDFF0` is the generic object step
   `0x004BCBB0` (`render/lighting.md` §8), then the `ClientFn` / mode
   sound step of `world/objects-client.md` §25 r2 (dispatch `0x004BDEE0`,
   table `0x007277F0`; bodies §26 there; ownership §18 rule 3). The
   update pass takes `now: u32` (wrapping milliseconds, the 1.14d
   `GetTickCount`) from the bridge and hands it to that step
   (`world/objects-client.md` §25 r6); the only other reader in this
   spec is §8 rule 7.
3. Order (`0x00465AA0`, the first part; buckets 0..127, each chain in
   its order, i.e. descending GUID): S missiles, C missiles, C objects,
   S players, S monsters, S objects, S items, C monsters. The S sets are
   walked by `0x00463C90` (next link read before the unit runs, so a
   unit may free itself); the C sets by `0x00463CC0`. The rest of
   `0x00465AA0` and of `0x0044C790` runs no handler (Phase 6).
   `0x00463CC0` runs rule 2's per-unit step (`0x00480810`), then looks
   the unit up again in its C set; still present: type 2 →
   `0x004BDEE0` once more, result ignored (`world/objects-client.md`
   §25 r3), type 1 → `0x0046D780` (Phase 6); other types nothing.
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
   How a server unit (the local player's hireling) gets both bits after
   a teleport: §16.

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
   L = (`ping.rtt` + 0x32) >> 7 (§7 rule 11: the last ping round trip
   in ms, `[0x007A04A4]`, read through `0x0044CE60`; 0 until the first
   0x8F and after the `0x007A0480` block clears of `0x0044E200` and
   `0x0044C890`; L = 1 from a round trip of 78 ms); U is a
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
8. **0xB4** load refusal (2026-10-08; answers `client/bridge.md` open
   question 7). The single-player server sends it as a direct send, so
   it is drained from the system list ahead of buffered game messages
   (`sim/intents-events.md` §3.3 rule 5, §3.4 rules 1–2), when the join's
   load fails (§8.2 rule 2 there: codes 0x13, 0x14, 0x15, 0x17, 0x18),
   and the server then removes the client, so no message follows it.
   1. Handler `0x0045C6D0` (system handler `0x0045C850`, 5 bytes):
      code c = u32@1; n := the error number of the map below (jump table
      `0x0045C7E8`, 26 entries, read from the image); c = 0 or c > 26 →
      n = 9. Then `0x0044E380(n)`, nothing else.

      | c | 1–6 | 7–21 | 22 | 23 | 24 | 25 | 26 |
      |---|---|---|---|---|---|---|---|
      | n | c − 1 | c + 3 | 9 | 0x19 | 0x1A | 0x1C | 0x1B |

      The single-player codes give 0x13 → 0x16, 0x14 → 0x17, 0x15 →
      0x18, 0x17 → 0x19, 0x18 → 0x1A.
   2. `0x0044E380(n)` (the client's "leave with an error" entry, also
      called by rule 7 for hosted game types) is UI code. In order: it does nothing when the 2,500 ms error timer
      `[0x007A0684]` / `[0x007A0688]` is still running (`0x0044E040`,
      wall clock) or `[0x007A0604]` (video-5 flag, `client/msg-ui.md` §1
      r2) or `[0x007A062C]` is set. Else the error number `[0x007A05D4]`
      := n (n ≥ 0x1D → 9; the map above never gives one); every open
      panel is closed (`0x00456300(1, 0)`); the timer is started
      (`GetTickCount`, 0x9C4 ms); **`exit_requested` := true**
      (`[0x007A0620]`); **`in_game` := false** (`[0x007A061C]`); the
      connection flag `[0x007A0618]` (`connected`, rule 10) := 0;
      `[0x0070EE8C]` := 0 (`0x0044B880`, the flag of `client/msg-ui.md`
      open question 8). The error screen
      (`0x0044CB60`: the string of error n; codes 0x14 / 0x15 add a line
      when the character record byte +0x1EF has bit 0x20) and the game
      loop's wait for the timer (`0x0044F360`) are Phase 6 UI.
   3. d2rs: the handler emits one `JoinRefused` output {n}
      (`client/bridge.md` §10 table); it writes no model field itself.
      The UI layer applies `0x0044E380(n)` from its own state (the guard
      is UI state, §10 r5 there) and, when the guard passes, returns the
      two model writes as requests (`exit_requested` := true, `in_game`
      := false; §10 r10 there). Applying them at delivery (§10 r4) is
      exact because no message follows 0xB4 in its frame (rule 8
      preamble).
9. **C→S 0x67** create game (2026-10-08). The client's game start
   `0x0044F360` calls the builder `0x00477CA0` (at `0x0044F45E`) when
   the client game type `[0x007A0610]` is not 3, 7 or 9, with ECX =
   the game name buffer `0x007A05DC` (EDX = `0x0047A990()`, unused).
   The builder fills 46 bytes on the stack and sends them through
   `0x0052AE50(0x2E, 0, msg)` (system queue, `client/bridge.md` §4
   rule 2), then adds 0x2E to `[0x007A6AF8]` and 1 to `[0x007A6B00]`
   (send counters). C = the start-up configuration `[0x007A0438]`
   (`tools/original-hooks.md`):

   | Bytes | Value | Single player (recorded seq 1, both recordings) |
   |---|---|---|
   | @0 | 0x67 | |
   | @1 | game name `0x007A05DC`, copied up to its NUL (`0x004135D0`); the bytes after the NUL keep stack contents | empty (byte 1 = 0) |
   | @0x11 | game type: `[0x007A0610]` 0 → 3, 6 → 1, 8 → 2, else 0 | 3 (type 0) |
   | @0x12 | class: `0x0047AA20()` (`[0x00712F00]`) when `[0x00712EFC]` bit 8, else byte `[0x007A0522]` | the selected character's class |
   | @0x13 | template: C +0x20D | 0 |
   | @0x14 | difficulty: C +0x210 (0–2; read as such by `0x0044CF20`) | 0 (Normal) |
   | @0x15 | character name `0x007A05C4`, copied up to its NUL | the character |
   | @0x25 | u16 C +0x207 (the server never reads it, `sim/intents-events.md` §2.5) | 0 |
   | @0x27 | u32 C +0x209; when it is 0 the builder first stores 4 \| 0x100000 there | 0x00100004 |
   | @0x2B, @0x2C | C +0x20E, C +0x20F (passed, never read by the server) | 0, 0 |
   | @0x2D | language id `0x00525150()` (0–13; the server refuses > 14) | 0 |

   d2rs: the app builds these bytes from its own state: name empty, type
   3, the character's class and name, template 0, the chosen difficulty,
   u16@0x25 = 0, flags 0x00100004 for an expansion character, @0x2B =
   @0x2C = 0, the language id. **Flags u32@0x27** (2026-10-08, static,
   settles REC-46): the front-end writes C +0x209 before the game
   starts, at every site the same way: := 4; := 0x804 when the
   character is hardcore; then |= 0x100000 when the character record's
   status byte +0x1EF has bit 0x20 (expansion). Sites: `0x004365B0`
   (three: `0x004366EC`–`0x00436717`, `0x0043678F`–`0x004367BA`,
   `0x004368B7`–`0x004368E2`; hardcore = status +0x1EF bit 0x04) and
   `0x00434A00` (two: `0x00434AE3`–`0x00434B0D`, `0x00434D19`–
   `0x00434D3F`; hardcore = bit 0x04 of its flags argument), both on
   the launcher's record `[0x007795D4]` (same layout as C: name +0xBD,
   status +0x1EF, flags +0x209); the third writer `0x004456D0`
   (`0x0044585F` / `0x00445873`, record `[0x0077BBD8]`) ORs 0x800 for
   status bit 0x04 and 0x100000 for bit 0x20 the same way. So the
   builder's default (C +0x209 = 0 → 4 | 0x100000) is not reached from
   the menus, and u32@0x27 is: classic softcore 0x00000004, classic
   hardcore 0x00000804, expansion softcore 0x00100004, expansion
   hardcore 0x00100804 (d2s status bits 0x04 / 0x20,
   `formats/d2s.md`). REC-46 still confirms on live bytes. Bytes after a
   name's NUL: zero (the original's stack contents are not
   reproducible and no reader uses them).
10. **0xAF** ConnectionInfo and **0xB0** ConnectionTerminated (system
    handler `0x0045C850`, jump table `0x0045C894` indexed by id − 0xAF;
    2026-10-08 read): 0xAF (`0x0045C86C`) → `connected` := 1; u8@1 is
    not read. 0xB0 (`0x0045C877`) → `connected` := 0. Both are direct
    sends (`sim/intents-events.md` §3.3 rule 5) and occur in single
    player: `af 00` twice (the first at the join) and `b0` once in each
    of both recordings (`-015956`, `-022633`; senders `0x0052B780`,
    `0x0053B220`). Other writers of 0: `0x0044E380` (rule 8.2), the
    exit send `0x00477EE0` (`client/msg-ui.md` §3 code 23) and
    `0x00453910`. Readers (UI / start-up, Phase 6): the system-message
    pump `0x0044BAD0` returns `connected` ≠ 0 after each message,
    `0x0044BD20` returns `connected` = 0, and the watchdog
    `0x0044EEC0` calls `0x0044E380(6)` when `connected`, `in_game`,
    `exit_requested` and `unloaded` are all 0 only for game types 7
    and 9 (hosted, out of scope), so never in single player.
11. **0x8F** Pong (`0x0045EB00` → `0x0044CDB0`, 33 bytes; 2026-10-08
    read) and the ping timer. `PingState` fields: `next_ms`
    `[0x007A049C]`, `sent_ms` `[0x007A04A0]`, `rtt` `[0x007A04A4]`,
    `samples` `[0x007A04CC]`, `mean` `[0x007A04D0]`, `pong: [u32; 8]`
    `[0x007A04D4]`…`[0x007A04F0]`.
    1. Timer (`0x0044CD70`, and the same code inline in the game loop
       `0x0044EFA0`): now := `GetTickCount()`; `next_ms` < now → the
       C→S 0x6D builder `0x00477DD0` runs (reads `pong[5..8]`, Battle.net
       / transport, `sim/intents-events.md` §4 rule 4), `sent_ms` :=
       now, `next_ms` := now + 5000.
    2. 0x8F: `pong[i]` := u32@(1 + 4i), i = 0…7; then `pong[4]` :=
       `GetTickCount()` (overwrites u32@17); `rtt` := `GetTickCount()` −
       `sent_ms`; while `samples` < 10: `mean` := (`mean` · `samples` +
       `rtt`) / (`samples` + 1) (u32, truncating), `samples` += 1. Read
       by §6 rule 4 (`rtt`, `0x0044CE60`) and `0x0044CE70` (`mean`).
    3. Single player (both recordings, 33 and 32 records): every 0x8F is
       33 zero bytes, sent 2.7–48.7 ms after the matching C→S 0x6D
       (server send time; the client's receive time is not recorded).
    4. d2rs: the in-process link has no transport delay, so the bridge
       sends no 0x6D and receives no 0x8F; `rtt` stays 0 and §6 rule 4
       uses L = 0. A 1.14d single-player round trip of 78 ms or more
       (one client frame plus one server tick can exceed it) gives L =
       1 there: open question 18.
12. **0xB3** DownloadSave (handler `0x0045C620`, which appends the
    chunk to a client buffer and at `total` calls the save writer
    `0x0045C520`, `formats/d2s.md` §2.7 rule 3): **unused in single
    player: never sent by the 1.14d server in a single-player game.**
    Its only sender `0x0052E110` runs for game type 1 or 2
    (`sim/intents-events.md` §3.2 rule 5; the leave drain of §2.5
    rule 2 there is gated the same way); the single-player game type is
    3 (§2.5 rule 2 there). No record in either recording. The d2rs
    handler is
    the no-op of `client/bridge.md` §6 rule 7.

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
   | 0x02 | `0x00480930(r0 & 0xFFFF, r1)`: the interact sender, rule 7 |
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
   Both codes in detail, with the shrine table: §15.
6. **Item** (`0x004C1B80`): code 2 → mode := r1 (`0x00624690`), unit
   flag 0x2 := (r0 ≠ 0); other codes do nothing.
7. **Interact sender** `0x00480930(type, GUID)` (ECX = type u16, EDX =
   GUID; callers: rule 4 code 0x02 and `ClientFn` 13,
   `world/objects-client.md` §26.13). P := the local player
   (`0x00463DD0`), U := (type, GUID) looked up in S (`0x00463990`); no U
   → nothing. "send 0x13" = append C→S 0x13 {0x13 u8, type u32, GUID
   u32}, 9 bytes (`0x004786A0`, CL = 0x13; `outgoing`, bridge send path).
   By type:
   - 0 (player): P faces U (`0x00621C00(P, x, y)` with U's client point
     `0x0045AE20` / `0x0045ADF0`); send 0x13.
   - 1 (monster): when `now` − U+0xD4 < 200 (u32 wrapping, unsigned;
     `0x00480AA7`) nothing; else U+0xD4 := `now`, send 0x13. d2rs: U+0xD4
     is `ClientUnit.interact_ms: u32`, 0 at creation.
   - 2 (object): P's path reset `0x00648B90(P path, 0)`. U's flag +0xC4
     bit 0x4 clear → send 0x13. Set → P faces U as for type 0; a
     7-value record R := 0, R[0] := the id (`0x00643CE0`) of P's skill
     on side 1 (`0x006439F0`). By U's class: 404 in mode 0 and 376 in
     mode 2 test the code of P's item in hand (`0x0063BEF0(P+0x60)`,
     `0x00628590`) against `qf2 ` (404) / `hfh ` (376): no item or
     another code → the player event sound `0x004CB9C0(P, 0x13)`
     (`audio/triggers.md` §3) and nothing sent; equal → R[0] := the id
     of P's skill 0 (`0x006439B0(P, 0, −1)`). 376 in mode 0 → send 0x13
     only (no skill start). Every other case continues: R[1] := −1,
     R[2] := 2, R[3] := GUID; the client skill start `0x004C6EB0(P, R)`
     (`render/lighting.md` §8 r3); send 0x13.
   - 4 (item): append C→S 0x16 {0x16 u8, 4 u32, GUID u32, b u32}, 13
     bytes (`0x004786D0`), b = the byte `0x004538D0(1)` zero-extended.
   - other types: nothing.
   The code-0x13 mode request (rule 4) sends nothing itself: `0x00480D20`
   only adds overlays (`0x00470390`) and `0x004CC5B0` and the functions
   it calls (`0x004CA2C0`, `0x004CA320`, `0x004CA380`, `0x004CA410`,
   `0x004CB2C0`, `0x004CB6A0`, `0x004CB860`, `0x004CB890`, `0x004CC410`,
   `0x004B9A00`) do not reach the send path `0x00478350`. The C→S 0x13
   of an interact is this rule's (`ui/controls.md` §6 r8–r11).

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
5. **A point in no room of the level** (answers open question 11):
   the level is got-or-allocated (and generated if it has no rooms,
   `drlg/levels.md` §4 rule 2, §8 rule 1; a level that is still roomless
   after generation is fatal assert 0x2FE in `0x00642630`); the room
   lookup then returns none. **0x07**: `0x0061B640` has no null test
   and reads the count (+0x0E) of the null room at `0x0061B672`, an
   access violation that ends the 1.14d process (not an assert, no
   message). **0x08**: `0x0061B690` tests the room (`0x0061B6C4`) and
   does nothing. d2rs: 0x07 → handler error (the message is refused
   and recorded, `client/bridge.md` §2.4, as for the fatal asserts of
   §12 rule 3), after the rule 4 record; 0x08 → no effect beyond the
   rule 4 record. A 1.14d server sends 0x07 / 0x08 only for its own
   rooms (room origins, rule 3), so neither case arises from original
   input when the client DRLG matches the server's.

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

### 11. Current act and level (join and later)

The single owner of "which act and level the client thinks the local
player is in", the input of `render/composition.md` §3 step 2
(BlankScreen) and §4 (act palette).

1. **No message carries the player's level.** The act comes from S→C
   0x03 and the level is derived from the local player's room (§2 rule
   7, §12). The server builds 0x03 in `0x0053ABE0` → `0x0053B390`: act
   u8@1 = the client record's act byte (`0x005382B0`: client +0x1AC);
   u32@2 = game +0x7C (map seed); u16@6 = the act's town level id
   (`0x0061AE80`: act +0x08, `drlg/levels.md` §2 rule 2: 1, 40, 75, 103,
   109); u32@8 = game +0x80. It then sends 0x53 (10 bytes; the act's
   environment fields from `0x0061C330`, Phase 6). Callers: game entry
   (`0x0052C210`) and act change (`0x0053ACC0`). u16@6 is the act's
   town, not the player's level; at game entry they agree only because
   the entry spawn is in the act's town level (`sim/path-placement.md`
   §11, tile index 0 of act +0x08).
2. **Act.** On 0x03 the client stores the act (`[0x007A288C]` through
   `0x00454790`; the act record of §7 rule 4). The first in-game frame
   loads that act's palette (`render/composition.md` §4: `0x004547B0` →
   `0x004FB480(act)`). Later act switches come from room changes (rule
   4). d2rs: `act.act` of §1 is the palette act until rule 4 changes it.
3. **Level.** The local player's room is set by its placement: S→C 0x15
   (`msg-units.md` §3 rule 4: room of the point, §12 rule 2). At a
   single-player join the order is 0x59 (player at (0, 0): no room),
   0x0B, 0x03 (act built), 0x07 × n (rooms brought in sight), 0x15 (room
   found, level known), then in the next frame 0x04 (which requires the
   room, §7 rule 5). The level is the room's level id (`0x0061A1B0`:
   active room +0x10 → DRLG room → level, `0x0066BAB0`), read each frame
   by `0x0044C990` (`render/composition.md` §3 step 2); no room → no
   level (BlankScreen treated as 0, nothing cleared). The full server
   order (the "…" between 0x0B and 0x03, 0x53 after 0x03, the 0x7E after
   0x15, and the units before 0x04) is `sim/intents-events.md` §8.
4. **Room change** (`0x004654C0`, `msg-units.md` §3 rule 4.4): when the
   local player moves to a room whose level's Levels `Pal` byte (+0x02;
   not `Act`, +0x03, which differs for levels 125–127 and 133–136)
   differs from the old room's, the palette switches to `Pal`
   (`0x004FB480`, `msg-units.md` §3 rule 4.4); the
   first placement (no old room) does not switch.
5. d2rs: `ViewFeed` level := the level id of the local player's room
   (§12 rule 2 on the local player's `position`), none while the local
   player is not placed; act palette := `act.act` (rule 2), replaced by
   the Levels `Act` of the new level on a rule-4 switch. The feed must
   not take the level from 0x03 u16@6 or from a 0x07 level byte.

### 12. Client DRLG and the room of a point

1. **The client act** is the DRLG act of `drlg/levels.md` §2 built from
   0x03's fields with the client flag (`0x006194A0(act, init seed u32@2,
   client 1, …)`: town id 0, DRLG flags 1; §7 rule 4). Its rooms become
   active rooms when they come in sight (0x07, §9 rule 1; `drlg/rooms.md`
   §4); the active rooms of the act form the list at act +0x10, linked
   by active room +0x7C (new rooms per `drlg/rooms.md`). d2rs: the same
   `d2-sim` DRLG code runs for the client from the 0x03 seed; the bridge
   owns its copy and never reads the server's. This answers open
   question 5 for the act build and room-of-point; the tile feed (RW2)
   stays with `drlg/` and the render specs.
2. **Room of a point** (`0x00465420(x, y)`, §2 rule 7) in two steps:
   (a) the cell lookup from the local player's room (`0x00463740`: the
   room and its adjacency array, `sim/path-placement.md` §4 rule 1); (b)
   the act lookup `0x00619DA0(act, x, y)`: act null → none; walk the
   act's active-room list from act +0x10 in list order and return the
   first room whose sub-tile rectangle contains the point: x0 ≤ x <
   x0 + w and y0 ≤ y < y0 + h with (x0, y0, w, h) = active room +0x4C,
   +0x50, +0x54, +0x58 (signed compares); end of list → none. Both
   none → fatal assert 0x13C.
3. **Fatal asserts** of this path are handler errors in d2rs (the
   bridge refuses the message and records it, `client/bridge.md` §2.4):
   0x13C (rule 2, a non-zero point in no active room of the client
   act); 0x168 (`msg-units.md` §3 rule 4.2: 0x15 to a non-zero point
   with no room); 0x166 (no path); 0x538 (0x15: the unit has no room
   after placing); 0x1A9 (forced placement failed). In 1.14d each ends
   the process.
4. **Nearest free point fallback** (`msg-units.md` §3 rule 4.5): when
   the teleport returns 0 the client searches from room' with
   `0x0064E7B0(room', &point, unit size, 0x1C09, fallback 1)`
   (`sim/path-placement.md` §7, max distance 50, step 1, fallback
   allowed) over the client act's collision maps (built with the client
   rooms, `drlg/rooms.md`), then places with the forced move
   `0x00650C20` (`sim/path-placement.md` §6 rule 2). The model's
   `position` is the point the search returns.
5. **Unit seed at a point** (§2 rule 6): the room of rule 2's seed
   (active room +0x6C, `drlg/rooms.md` §2) is stepped once and the unit
   seed := `init_low(lo')`. A client room's seed is that of the client
   DRLG of rule 1, so it equals the server room's seed only if both
   rooms were created by the same draws (open question 9).

### 13. Visibility predicate (`0x004DBF20`)

`visible(U, a, b)` of §6 rule 6 (a, b in client pixel space):

1. Screen point: X := a − (cx_u − shiftX), Y := b − (cy_u − 8) with the
   unit origin (cx_u, cy_u) and shiftX of `render/camera.md` §3–§4
   (origin getters `0x0045AFC0`, `0x0045AFD0`).
2. COF box test `0x004709A0(U, X, Y, 0, 0)`: the unit-composite
   pre-test (`render/unit-composite.md` §4, centering off): x_min + X <
   W − 1, x_max + X ≥ 0, y_max + Y ≥ 0, y_min + Y < H − 1 with W, H =
   `[0x0071146C]`, `[0x00711470]`. Fails → not visible.
3. Cel request: a zeroed 0x48-byte cel context (`render/capture.md`
   cel context) with frame := U +0x44 >> 8 (`0x00621810`), component
   byte := 1 (TR), direction := `0x00620100(U)`; built by `0x004DB7B0`
   with mode argument −1 (the unit's current draw-identity mode,
   `render/unit-composite.md` §5.1). Request fails → not visible.
4. Cel load `0x006001F0(context, 0, 1)`: fails or no cel → not
   visible.
5. Cel box test `0x004DAB40(cel, X, Y, 0)` with the cel's width w (+4),
   height h (+8), x offset ox (+0x0C), y offset oy (+0x10): left := ox +
   X, top := oy + Y; visible iff left ≤ W and left + w ≥ 0 and top − h ≤
   H and top + h ≥ 0 (signed). (The vertical span tested is [top − h,
   top + h], 2h tall: reproduced as read.)
6. d2rs: the bridge takes the predicate as an input (`ModelInputs`); the
   render side implements rules 1–5 from its camera, COF and cel state.
   With no render state (headless) the predicate is absent and §6 rule 6
   is a handler error (unchanged).

### 14. Pet list and the hireling GUID

1. The client keeps a pet list at `[0x007BB5BC]`: 0x34-byte records,
   new records prepended, link +0x30. Fields: +0x00 class, +0x04 pet
   type, +0x08 pet GUID, +0x0C owner GUID, +0x1C := 100 at creation,
   +0x20 gone flag.
2. **S→C 0x7A** PetAction (`0x0045E860`, 13 bytes): u8@1 ≠ 0 → set
   (`0x00478B10(pet GUID u32@9, owner GUID u32@5, type u8@2, class
   u16@3)`): for type 7 first remove the type-7 record whose pet GUID
   matches (`0x00478AB0`, freed); then a record with that pet GUID gets
   type, owner, class and gone := 0, else a new record is prepended.
   u8@1 = 0 → remove (`0x00478C90(u32@9)`): the first record with that
   pet GUID; if it is type 7 and its owner is the local player's GUID it
   is kept with gone := 1, else it is unlinked and freed.
3. **S→C 0x81** AssignMerc (`0x0045E890`, 20 bytes): `0x00478BB0(pet
   GUID u32@8, owner GUID u32@4, type u8@1, class u16@2, {u32@0xC,
   u32@0x10, 0})`: set as rule 2 (a missing record afterwards is fatal
   0x95), then pet GUID, owner GUID and type are rewritten and **+0x24
   := u32@0xC, +0x28 := u32@0x10, +0x2C := 0**. Type 7 with the
   monster (1, pet GUID) in S (`0x00463990`): the client hireling setup
   `0x004B1090(U, {u16 := u32@0x10 & 0xFFFF, u32 at +4 := u32@0xC})` (U +0xC4 |= 0x202, hireling
   skills from the hireling record, `client/msg-ui.md` OQ 2), the dead
   flag clear `0x004647D0(U)`, `0x004AE210(U)` and mode set
   `0x00624690(U, 1)` (2026-10-08 read of `0x00478BB0`). The 0xAC create
   (`0x00466360`, pet type 7) passes the same pair from the record
   (`0x00478E40`): +0x28 low 16 bits and +0x24; `0x004B1090` hands the
   u16 to `0x00663750` / `0x0044DCC0` (hireling row lookup) and the u32
   to `0x00463DD0` / `0x006637F0`.
   **+0x1C is a life percent that stays 100** (2026-10-08, scan of
   every use of `[0x007BB5BC]`: the 19 functions `0x00478A50`–`0x00479230`).
   Written only at creation (`0x00478B10`, 100; the update of an
   existing record there leaves it) and by `0x00479010` (+0x1C := pct,
   +0x10 := u16; a type-7 pet's stat 6 := max life × pct / 100), which
   has no caller and no pointer to it in 1.14d `Game.exe` (no `call`
   in `all.asm`, no little-endian 0x00479010 in the file): dead code.
   Read only by the getter `0x00479080(pet GUID)` (0 for no record),
   whose three callers are UI draws, none in the model: `0x004525F0`
   (a monster's hover life bar on a 0–128 scale, `0x00452644`: a
   monster with a pet record and a result ≥ 1 draws 100 × 128 / 100
   = 128, full, instead of stat 6 / max life, unless its unit flag 0x4
   path `0x00465C60` ≠ 0), `0x0048A990` (`0x0048AA74`) and
   `0x00493A00` (`0x00493A0A`, a 0x2E-pixel bar); the UI owners of
   those draws take the value 100. The model stores +0x1C = 100 and
   never changes it. REC-10 (0x81 bytes of a hireling) still confirms
   the 0x81 field writes.
4. **Hireling GUID** (`0x00478F20(player, 7, any = 1)`, the call of
   `msg-units.md` §1.2 rule 2 and §2 rule 2): no player → −1; else the
   first record in list order with type 7 and owner GUID = the player's
   GUID (gone records included) → its pet GUID; none → −1
   (0xFFFFFFFF).
5. d2rs: `pets: Vec<PetRecord>` in list order (newest first) is a model
   field written only by 0x7A and 0x81; the hireling GUID is rule 4 on
   it. Neither id occurs in the two recordings (no hireling).

### 15. Object mode requests in detail (codes 3 and 0x15; shrines)

Owner of the client side of S→C 0x0E for objects and of 0x4D in its
shrine form (`world/objects.md` §14 owns the server side). Object U,
record r (`msg-units.md` §4 rows 0x0E, 0x4D).

1. **Shrine test** `0x00621B00(U)`: U is type 2 and its objects.txt
   record's `SubClass` (+0x167, `data/fields.tsv` objects 118) has bit 0.
   **Shrine data** `0x00621B70(U)`: the shrines.txt record pointer at
   object data +0x08 (fatal 0xE4E for a null U, 0xE55 for a non-object).
   Its byte 0 is the shrine `Code`.
2. **Shrine table** `0x006DA8C0`: 23 entries (count dword `0x0072779C`
   = 0x17, static) of 20 bytes, indexed by shrine code: +0x00 on-mode
   function, +0x04 on-use function, +0x08 / +0x0C overlay ids (−1 =
   none), +0x10 sound id (0 = none). Values read from the image:

   | Code | +0x00 | +0x04 | +0x08, +0x0C | +0x10 |
   |---|---|---|---|---|
   | 0 | – | – | – | 0 |
   | 1, 2 | – | – | – | 0xA71 |
   | 3 | – | – | – | 0xA70 |
   | 4, 5 | – | – | – | 0xA6A |
   | 6, 7, 8 | `0x004BD4A0` | – | (0x3B, 0x39), (0x3C, 0x39), (0x3E, 0x39) | 0xA68, 0xA69, 0xA73 |
   | 9, 10, 11 | `0x004BD4A0` | – | (0x3F, 0x3A), (0x3D, 0x3A), (0x40, 0x3A) | 0xA72, 0xA74, 0xA75 |
   | 12, 13, 14, 15 | `0x004BD4A0` | – | (0x41, 0x39), (0x42, 0x3A), (0x43, 0x3A), (0x44, 0x39) | 0xA77, 0xA70, 0xA70, 0xA6B |
   | 16 | – | `0x004BD090` | – | 0xA76 |
   | 17, 18 | – | – | – | 0xA6F, 0xA6D |
   | 19 | – | `0x004BD0C0` | – | 0xA78 |
   | 20 | – | – | – | 0xA6F |
   | 21 | – | `0x004BD220` | – | 0xA6C |
   | 22 | – | `0x004BD360` | – | 0xA6E |

3. **Code 3** (S→C 0x0E with u8@6 = 3; the server writes 3 for every
   object, `world/objects.md` §14 rule 1): the object mode change
   `0x004BCF60(U, r)` (r0 = u8@7, r1 = mode u32@8; lights:
   `render/lighting.md` §8). Then, for a shrine (rule 1), `0x004BD650`:
   shrine data null → fatal 0x37A; code ≥ 23 → fatal 0x37B; the entry's
   +0x00 function, if any, runs with (U, shrine data). `0x004BD4A0`
   (codes 6–15), for each of +0x08 and +0x0C that is not −1:
   `0x0046F0C0(U, 0, id)`, then, when U is null or U's mode (+0x10) is
   0, `0x00470390(U, id, 3, 0, 0, 0, 0, 0)` (overlay calls; Phase 6
   effects).
4. **Code 0x15** (`0x004BD5C0`, S→C 0x4D for an object): reads only
   r0 (= u32@6, the operator's GUID in the shrine form). r1 = −1 and
   r2..r4 (u16@11, u16@13, u8@10) are never read; u16@15 is not copied
   into the record (`msg-units.md` §4). Steps:
   1. code := shrine `Code` (rule 1) when U is a shrine, with shrine
      data D; else code := objects.txt `ShrineFunction` (+0x16F,
      objects 122) of U's class and D := none.
   2. P := the player unit with GUID r0 (`0x00463990(r0, 0)`). None →
      the request ends; nothing else runs.
   3. 0 < code < 23 and the entry's +0x04 function set → call it with
      (U, P, D) (codes 16, 19, 21, 22; Phase 6 effects).
   4. `0x004BD550(U, P)`: shrine data of U null → fatal 0x34D; its code
      ≥ 23 → fatal 0x34E; entry +0x10 ≠ 0 → sound request
      `0x004B9A00(id, P, 0, 0, 0)` (request rule: `audio/triggers.md`
      §1 rule 1; the sound plays on the operator).
   So a 0x4D code-0x15 request on a non-shrine object whose operator is
   a known player is fatal 0x34D in 1.14d (a server sends 0x4D for an
   object only for shrines, `world/objects.md` §14).
5. d2rs: the model stores the mode request as §8 rule 3; the effects of
   rules 3–4 are client outputs (`client/bridge.md` §10, rows
   `ShrineFx` and `ShrineSound`) emitted in the update pass in the
   order above, with the captured code, object key, player key and
   overlay or sound ids. The fatal asserts are handler errors
   (`client/bridge.md` §6 rule 4).

### 16. C→S 0x4B after a teleport (the hireling case)

Answers PC 2's question on `tp80-packets.jsonl` (C→S 0x4B `4b
01000000 01000000` in the input phase after tick 2919, a town-portal
teleport while the hireling walked; server answer
`world/hirelings.md` §6 rule 7).

1. The only setter of unit flag 0x800000 and of flags-2 0x20 is the
   client active-room free (§5 rule 5, `drlg/rooms.md` §8 rule 4). On
   the client it runs from exactly two paths:
   - a DRLG room leaving sight: S→C 0x08 (§9 rule 2) un-propagates;
     the status-3 unset handler `0x0061B560` recomputes the status, and
     when it became 4 in a client DRLG (`0x00642A00`) tail-calls
     `0x0066F1A0(room, 0)`, which removes the room's active room
     (`0x0061A910` at `0x0066F1CA`, when DRLG room +0x30 is set) and so
     frees it (`0x0061A840`); `drlg/rooms.md` §4 unset handlers;
   - the whole client act being freed (`0x0061AFD0`): by the act load
     `0x0044E100` (§7 rule 4) and by the game teardown `0x0044C890`
     (called from `0x0044F360`, after its unit clear `0x004659C0`).
2. Every unit still linked in the freed room gets 0x800000; a unit
   without flag 0x400000 (a server unit; client-only units carry
   0x200000 with 0x400000, Constants) also gets flags-2 0x20. The
   local player's hireling is a server unit (0xAC) that 0x0A never
   removes (`msg-units.md` Edge cases), so when the player leaves by a
   town portal and the 0x08 messages for the old rooms take the room
   holding the hireling to status 4, the hireling is left roomless with
   both bits.
3. The next update pass (§5 rules 1, 5) skips the hireling's update and
   queue, sends C→S 0x4B (type 1, the hireling's GUID) and clears both
   bits. That is the recorded `4b 01000000 01000000` for type 1, GUID 1.
4. No 0x4B is sent when the hireling's client unit is no longer in that
   room when the room is freed (it was placed elsewhere first, or its
   room stays in sight). Which of these applied at the recording's
   second teleport is open question 14.

### 17. Model writes made by 1.14d UI code

Several 1.14d UI functions write the client world. Each is stated here
as a model rule; the bridge reproduces it as a `ClientWorld` operation
with exactly this effect, at the point of 1.14d order where the UI
function runs. Rules 1–3 run when the UI layer performs the 1.14d UI
function that contains them (`client/msg-ui.md` §16, `ui/messages.md`
§11, §13, §14); the UI request reaches the model through the
bridge (`client/bridge.md` §10 r10: the UI layer returns the write as a
request, the bridge applies it before the next message; §10 r6 stands). Rule 4 is model-side and decided here. "Flag bit n" is
the unit flag word +0xC4 (`client/msg-ui.md` OQ2 owns the full word;
bit 0x2 is the bit of `client/msg-ui.md` §1 r4 and §16).

1. **Interaction end** `E(G)` (`0x004B3C20`, NPC GUID G in ECX; 23
   call sites, all UI: menu cancel, dialog ends, town exit, the 0x28
   dialog branch). In 1.14d order, model parts in bold:
   1. **the local player (type 0, with player data) gets data +0x150,
      +0x154, +0x158, +0x15C := 0** (`0x004B3C42`–`0x004B3C60`; the
      same write as 0x04, §7 rule 5);
   2. U := the monster (1, G) of S (`0x00463990`, `0x004B3C6D`); U
      present → **stock discard of U** (rule 3, `0x004B3790` at
      `0x004B3C78`);
   3. greeting stop (`[0x007C0DB4]` set → `0x004BA840([0x007C0DB8])`;
      both := 0): UI / audio;
   4. U present → **U flag bit 0x2 := 1** (`0x004B3CAE`);
   5. C→S **0x30** [u32 1][u32 G] (`0x004786A0` at `0x004B3CBD`): send
      path (`client/bridge.md` §4);
   6. `0x004A1730` (NPC text list freed), interact NPC active
      `[0x007C0D29]` := 0, `SetUIState(8, off, 0)`: UI;
   7. U present: **U's monster data +0x28 bit 0 := 0** (`0x004B3CEB`;
      U is always type 1 here), then **the mode set `0x00480E70(U, 1)`:
      flag bit 0x40 := 0, mode := 1 (`0x00624690`)**; when the mode set
      returns non-zero, the client graphics, overlay and mode-sound
      refresh of U (`0x00470610`, `0x00480D20`, `0x004CC5B0`; effects,
      open question 1).
2. **NPC menu open** `M(U, a)` (`0x004B66B0`, NPC unit in ESI; callers
   the 0x28 branch B6 and `0x004B6A30`). With U absent and a = 0: `E`
   of the interact NPC GUID and `SetUIState(8, off, 0)` (rule 1). Else,
   after the UI fields (interact NPC := U, `ui/messages.md` §14):
   **U flag bit 0x2 := 0** (`0x004B6794`). `SetUIState(8, on, 0)`
   refused (`0x004B687E` returns 0): `[0x007C0C6B]` := 0 (UI), **U flag
   bit 0x2 := 1** (`0x004B6890`), C→S **0x30** [u32 1][U's GUID]
   (`0x004B689F`), interact NPC active := 0 (UI), then `0x004B3830`(U's
   GUID): **stock discard** of (1, GUID) when present (rule 3) and the
   greeting stop.
3. **Stock discard** of an NPC U (`0x004B3790`; from rule 1.2 and from
   `0x004B3830`, which is also the `NpcGone` output of 0x28,
   `client/msg-ui.md` §16 r3). The UI hire table's used flags (10 × 16
   bytes from `0x007C0C8D`) := 0 (UI state, `client/msg-ui.md` §6).
   Model: when U has an inventory (+0x60), every item of its item list
   (first item inventory +0xC, next = item data +0x64, read before the
   item is removed) **is removed from S and freed** (`0x00465EE0(item
   GUID, type 4)`, the table remove of §2), then **U's inventory list
   is emptied** (`0x0063CB70`). These are the NPC's shop items (S→C
   0x9C into the NPC's store, `client/msg-stats-items.md`).
4. **Skill fallback** (`0x00496CF0`, the control panel's skill-button
   draw; `ui/control-panel.md` §7 r1). For the local player P (none →
   nothing), first the left skill (`0x00620190(P)`, +8 of the skill
   list), then the right skill (`0x006201D0(P)`, +0xC): when the entry
   exists and its level with bonuses `0x006442A0(P, entry, 1)` ≤ 0: the
   native entry of that skill id (`0x00643CE0`) is removed with d = 1
   (`0x006470F0` → `0x00646FD0`, `client/msg-skills.md` §2 rule 5),
   then left (right) := the entry (0, owner −1) (`0x00643BC0` /
   `0x00643C50`, `client/msg-skills.md` §2 rule 3; not found →
   unchanged). 1.14d runs it in every frame's UI draw (`0x0044C990` →
   `0x00456EE0` → `0x00499450` at `0x0045709E`, unconditional; →
   `0x00496CF0` at `0x004994C4`). **d2rs:** the bridge runs it as the
   last step of `bridge_frame` (`client/bridge.md` §8 rule 5). Nothing
   writes `ClientWorld` between that point and the UI draw (§10 r6
   there), so the draw sees the same model as in 1.14d.
   **Runs per loop pass** (2026-10-08, static, answers open question
   17). `0x0044C990` is called only through the pointer `[0x007A0484]`
   (set at `0x0044E300` in `0x0044E200`; no other reference in
   `Game.exe`), from two sites of the loop pass `0x0044EFA0`, at most
   once per pass. In single player (game type 0 / 1):
   1. `0x004F6070` ≠ 0 → the pass returns at once: no receive, no
      draw, no fallback.
   2. Paused (UI state 9 or 11, `0x00453A90(9)` / `(0xB)`, a local
      player with a room `0x004646A0`): draw at `0x0044F017` and
      return, **no receive, no tick, no update**; so the fallback runs
      once per loop pass while paused.
   3. Otherwise the receive `0x0044C6E0` runs every pass, and the draw
      (`0x0044F28B`) only when the skip counter `[0x007A0704]` is 0
      and `in_game` and `0x004646A0` ≠ 0; the counter is incremented
      at the end of every in-game pass (drawn or not). It is reset to 0
      in a pass whose server tick ran with elapsed time e < 2 × 40 ms
      (`[0x0070EF1C]` = 40); with e ≥ 80 ms it becomes (counter < 2)
      (so a tick pass directly after a drawn tick pass, counter 1, is
      skipped); and in any pass with e ≥ 40 ms while now <
      `[0x007A04BC]` (the 10 s hold after an act load,
      `ui/frontend-loading.md`; `0x0044F0CA`). So, outside that hold
      and that lag case, the draw runs exactly in the passes where the
      server ticked, and a receive in a pass without a tick has no
      fallback after it (`render/camera.md` §9 states the same draw
      schedule).
   **d2rs:** run the fallback in `bridge_frame` only in a frame whose
   pump ran a tick (the drawn pass), and once per frame while the game
   is paused (rule 2, with no receive); not in a frame without a tick.
   The two wall-clock cases of 3 (lag skip, act-load hold) depend on
   real time, which d2rs's tick-stepped frame does not have; they are
   not reproduced (equal for every pass that does not lag).
5. **Town exit** (`0x004B3E10`, UI): called by the local player's
   room-change step `0x00460E70` (from the player update `0x00463390`
   at `0x004636D5`, §5) when the town flag `[0x007A5260]` (`0x0061AB00`
   of the new room) goes from 1 to 0. It re-arms the NPC greetings and,
   with an active interaction whose NPC is present, runs `E` (rule 1)
   (`ui/messages.md` §13 r4). It is reached from a model update, not
   from a message output: rule 6 hands it to the UI layer.
6. **Local player room change** (`0x00460E70`; 2026-10-08, answers open
   question 16 and `client/msg-units.md` open question 8). In the player
   update `0x00463390` of any player U, when U's path room-changed flag
   is set (path +0x34 bit 0x2, `0x00620F50` → `0x00648B30`; set by the
   path code when the unit's room changes, the flag `sim/pathing.md`
   §9.8 names; client movement is Phase 6) the flag is cleared
   (`0x00620FA0(U, 0)`) and, only when U is the local player, this step
   runs (`0x004636D5`). In 1.14d order:
   1. R := U's room (`0x00620BB0`); none → nothing more.
   2. **Portal flags**: L := R's level id (`0x0061A1B0`); the local
      player's player data +0x2C (`pdata_2c` of `client/msg-units.md`
      §7 r4, read `0x00622230`, written `0x006221E0`) |= 1 << i, i = the
      index of L in the portal level list (leveldefs rows whose `Portal`
      +0x8C ≠ 0, in row order; `0x0061AE30` over `0x0096C9F4` / count
      `0x0096C9F8`, `data/runtime-maps.md` `leveldefs_portals`); L not in
      the list → no bit. This is the only client reader of +0x2C: the
      getter's other three callers are server code (`0x00537B00`, no
      static caller; `0x00537B50`, the same OR on the server,
      `sim/intents-events.md` §7.8 rule 4; `0x00539760`, the join, which
      sends the value as 0x5F). Nothing on the client consumes the
      value, so it has no observable client effect; the model keeps it
      so a memory read can compare it with the server's.
   3. `0x00648AA0(path, R)` → `0x0061AF10(client act, …)`: Phase 6
      (not a model write named here).
   4. **Town flag**: T := (`0x0061AB00(R)` ≠ 0) (R is a town room). When
      `town_flag` ≠ T and T is false, the **town exit** `0x004B3E10(U)`
      (rule 5) runs; then `town_flag` := T. d2rs: `ClientWorld.town_flag:
      bool` (`[0x007A5260]`), added by this rule; also written by every
      player creation (`client/msg-units.md` §1.1 r3), which this rule
      does not change.
   5. `0x0061AA40(R)` → `0x00473C90`, `0x0046BF30(R)`: Phase 6.
   d2rs: the update pass runs steps 1–5 at the local player's per-type
   update (§5 rule 2; the rest of `0x00463390` stays Phase 6). The town
   exit is the output `TownExit` (`client/bridge.md` §10 r11): the pass
   emits it at step 4, the bridge delivers it to the UI layer at once
   and applies the UI's model-write requests (rule 1's parts of `E`)
   before the pass continues, so later units' updates and queue drains
   see the writes as in 1.14d.

### 18. Audio driver inputs and the client object functions

1. **Per-unit audio inputs.** `ClientUnit` holds, beside §1 rule 2, the
   fields the sound triggers read (`audio/triggers-2.md` §21, which
   names the owner of each value): sequence mode +0x30 / +0x40; flag-ex
   +0xC8 (bit 3) and the transform states; frame +0x44, frame count
   +0x48, speed +0x4C (i16), frame event +0x4E; last hit class +0xB0
   (written by the mode machines, §8 rule 4); for monsters the monster
   data +0x16 (type flags) and +0x26 (superunique row) from S→C 0xAC
   (`client/msg-units.md`); the unit sound fields +0x70, +0x74, +0x78
   (request list), +0x7C, +0x80, +0x84, +0x88, zero at creation and
   written only by the audio rules; and, through U's room, the floor
   material (`drlg/rooms.md`, `formats/dt1.md`). The model stores them;
   it decides none of them.
2. **Order of the audio calls inside the model passes.** The object
   update of §5 rule 2 and the C-set second call of §5 rule 3 make their
   mode sound calls in the order of `audio/triggers-2.md` §20 r1, r2,
   r4; an object mode change (§8 rule 5, code 3, `0x004BCF60`) makes its
   mode sound call inside the change (§20 r3). The model hands each call
   to the audio driver at that point of the pass, so the driver sees
   them in 1.14d order. **Unit free** (§2 rule 5, `0x00465870`): before
   the per-type frees, every request handle of U is detached without
   force and U's sample locks released (`audio/triggers-2.md` §19 r5);
   the per-type branches' second detach finds the list empty.
3. **`ClientFn` owner** (answer to PC 2): the bodies of `ClientFn`
   1–18 (table `0x007277F0`, 19 entries) and the per-call-site rules
   are owned by `world/objects-client.md` §25–§26; `client/model.md`
   owns where the dispatch `0x004BDEE0` is called from (§5 rules 2–3,
   once per object update and once more for C objects) and the `now`
   input it reads (§5 rule 2).

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
- `[0x007A04A4]` (`ping.rtt`) is written only by the 0x8F handler
  `0x0044CDB0` (§7 rule 11), code the `all.asm` export does not cover
  (it is reached by the jump at `0x0045EB00`), which is why open
  question 3 first found no writer.

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
| join: 0x03 seq 142 then 0x15 `15 00 01000000 4112 c411 01` seq 154 | palette act 0 (`act1`); local player at (4673, 4548) = tile (934, 909), in the level-1 room of origin tile (928, 904) brought in sight by 0x07 seq 144; level 1 (Rogue Encampment), BlankScreen 1 | §11 rules 2–3 |
| frames between 0x59 seq 102 and 0x15 seq 154 | no room, no level (BlankScreen 0) | §11 rule 3 |
| 0x03 u16@6 = 1 while the player is placed in level 2 | level stays the room's (2), not 1 | synthetic, §11 rule 5 |
| act lookup: active rooms A (x 100..139, y 200..239), B (x 140..179, y 200..239), point (139, 239) | A | synthetic, §12 rule 2 |
| same, point (180, 200) and no local-player room | fatal 0x13C → handler error | synthetic, §12 rules 2–3 |
| visible: cel w 40, h 80, ox −20, oy −80, X 400, Y 300, W 800, H 600, COF box passes | left 380 ≤ 800, 420 ≥ 0, top 220: 140 ≤ 600, 300 ≥ 0 → visible | synthetic, §13 rule 5 |
| same with X = 900 | left 880 > 800 → not visible | synthetic |
| 0x7A `7a 01 07 4f01 05000000 21000000` | pets = [{class 0x14F, type 7, pet 0x21, owner 5}] | synthetic, §14 rule 2 |
| then 0x7A `7a 00 07 4f01 05000000 21000000` with local player GUID 5 | record kept, gone 1; hireling GUID(player 5) = 0x21 | synthetic, §14 rules 2, 4 |
| hireling GUID with an empty pet list | −1 | synthetic |
| shrine object (2, 9), shrine `Code` 1, player (0, 1) known; 0x4D `4d 02 09000000 01000000 01 0000 0000 0000` drained | mode request 0x15; no on-use function; outputs [`ShrineSound` 0xA71 on (0, 1)] | synthetic, §15 rule 4 |
| same, operator GUID 7 unknown | no output | synthetic, §15 rule 4 step 2 |
| shrine object, `Code` 6; 0x0E `0e 02 09000000 03 00 01000000` drained, U mode 0 | `0x004BCF60` mode change; outputs [`ShrineFx` on-mode code 6, overlays 0x3B, 0x39] | synthetic, §15 rule 3 |
| hireling (1, 1) in a client room whose last 0x08 drops it to status 4; next ticked update pass | `outgoing` += `4b 01000000 01000000`; hireling skipped, both bits cleared | §16; PC 2 `tp80-packets.jsonl` after tick 2919 |
| `E(6)`, NPC (1, 6) present in mode 3 with items (4, 20), (4, 21) in its inventory, local player data +0x150 = 5 | items (4, 20), (4, 21) gone from S, NPC inventory empty; NPC flag bit 0x2 set, bit 0x40 clear, monster data +0x28 bit 0 clear, mode 1; player data +0x150…+0x15C = 0; `outgoing` += `30 01000000 06000000` | synthetic, §17 rule 1 |
| `E(6)`, (1, 6) absent | player data cleared; `outgoing` += `30 01000000 06000000`; nothing else in the model | synthetic, §17 rule 1 |
| end of `bridge_frame`, local player left skill = native entry of skill 36 with base 0 and no bonus; skill 0 native entry present | entry 36 unlinked; left = skill 0 entry; right unchanged | synthetic, §17 rule 4 |
| same, entry 36 base 2 with a −2 bonus (level 0) | entry 36 base 1, kept; left = skill 0 entry | synthetic, §17 rule 4 |

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
`0x0045CB20`, `0x0061A070`, `0x0061A0C0`, `0x0061B640`, `0x0061B690`
(§9 rule 5: the null read at `0x0061B672`, the null test at
`0x0061B6C4`, `0x00642C30` → `0x00642630` returning none after its
level walk, from the disassembly).
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
Dispatch owners session (2026-10-08, `tools/ghidra/disasm.py fn` over the
image, since `all.asm` lacks the code at `0x0044CDB0`): 0x8F
`0x0045EB00` → `0x0044CDB0`, ping timer `0x0044CD70` / `0x0044EFA0`,
0x6D builder `0x00477DD0`; system handler `0x0045C850` and its jump
table (0xAF, 0xB0, 0xB2, 0xB3 `0x0045C620`); `[0x007A0618]` writers and
readers in `all.asm`; packet counts and 0x6D → 0x8F delays from both
`traces/raw/*-packets.jsonl` recordings.
Join-update session (2026-10-06): 0x03 builder `0x0053ABE0` (callers
`0x0052C210`, `0x0053ACC0`), `0x0053B390`, `0x005382B0`, `0x0061AE80`,
`0x0061C330`; act lookup `0x00619DA0`, room level `0x0061A1B0`;
visibility `0x004DBF20`, `0x0045AFC0`, `0x0045AFD0`, `0x004709A0`,
`0x00621810`, `0x004DB7B0` (mode −1 branch at `0x004DB7FB`),
`0x004DAB40`, cel getters `0x006018C0`, `0x006018F0`, `0x00601920`,
`0x00601950`; pet list `0x00478AB0`, `0x00478B10`, `0x00478BB0`,
`0x00478C90`, `0x00478F20` (all three message callers pass any = 1:
`0x0045CC1D`, `0x0045F42C`, `0x00466375`), handlers `0x0045E860`,
`0x0045E890`. §11 rule 3's order and the level-1 room checked on
`-022633` seq 102–219.
Area 4 session (2026-10-07, PC 2 requests): object requests
`0x004BD6D0`, `0x004BD5C0`, `0x004BD550`, `0x004BD650`, `0x004BD4A0`,
`0x00621B00`, `0x00621B70`; shrine table `0x006DA8C0` and count
`0x0072779C` read from the image; `SubClass` / `ShrineFunction`
offsets from `data/fields.tsv`. Room free paths: `0x0061B560`,
`0x0066F1A0` (call `0x0066F1CA`), `0x0061AFD0` (callers `0x0044C8A9`,
`0x0044E11A`, and the server's `0x0052C887`), `0x0061A840`.
§17 (2026-10-08, PC 2 requests from spec-ui pass 3, re-read on the
1.14d export with `tools/ghidra/disasm.py`): `0x004B3C20` (whole),
`0x004B3790`, `0x004B3830`, `0x00480E70`, `0x00465EE0` → `0x00465E80`,
`0x0063B2C0`, `0x0063DFA0`, `0x0063DFD0`, `0x0063CB70`; `0x004B66B0`
(`0x004B66B5`–`0x004B68B7`); `0x00496CF0`, `0x00643CE0`, `0x006470F0`,
`0x00646FD0`, caller chain `0x00456EE0` (`0x0045709E`), `0x00499450`
(`0x004994C4`); `0x00460E70` (call `0x00460EDE`), `0x00463390`
(`0x004636D5`). The export adds to PC 2's list: rule 1 also runs the
stock discard, clears monster data +0x28 bit 0, flag bit 0x40 and sets
mode 1; rule 2's refusal also runs the stock discard.
§5 r2–r3, §8 r7, §18 (2026-10-08, PC 2 requests from spec-objects-client
and spec-audio-s4): `0x004BDFF0`, `0x00463CC0` (type 1 / 2 branches),
`0x00480930` whole (disassembly: CL / EDX / stack of the `0x004786A0`
and `0x004786D0` calls, the 200 ms compare at `0x00480AB5`, record
build `0x004809A2`–`0x00480A34`), `0x004786A0`, `0x004786D0`,
`0x006439B0`; code-0x13 path `0x00480D20`, `0x004CC5B0` and its direct
callees (no call of `0x00478350`). §18 r1–r2 restate
`audio/triggers-2.md` §19–§21 (PC 2, read there).

§7 rule 9 (2026-10-08, asm): `0x00477CA0` (`0x00477CDF` game type),
its only caller `0x0044F360` (`0x0044F43E`–`0x0044F45E`),
`0x0047AA30`, `0x0047AA20`; the recorded C→S 0x67 is seq 1 of
`20261006-022633-packets.jsonl`.

## Open questions

1. The mode machines: dispatch of the player, object and item machines
   answered in §8 rules 4–6. Open: the monster machine `0x004AFF60`
   (3,678 bytes, 10 callers) per code, and the effects of the helpers
   the tables name (`0x00480E70` mode set, `0x004804A0` / `0x00480780`,
   `0x00480930` (answered: §8 rule 7), `0x00480EF0`, `0x004BCF60`, `0x004BD5C0`): Phase 6
   client unit-modes spec. PROVISIONAL: a client monster changes mode
   only as the S→C messages state (§8), with no client-side mode steps
   and no client seed draws beyond those the message rules name
   (because the server stream carries every mode change the recordings
   show); settled by REC-51 (HIGH-PRIORITY CAPTURE: client seed draws).
   PROVISIONAL: the client skill start of codes 0x15 / 0x16 (S→C 0x4D /
   0x4C, `0x004C6F40` / `0x004C6EB0`) sets the unit's mode to the
   skill's `anim` (player) or `monanim` (monster) mode, record[0] being
   the skill id; a skill without a row leaves the mode (because the
   attack art follows `unit.mode` and the start is not read); settled by
   REC-51.
2. Local walk prediction and per-update path stepping of the local
   player (input → path, `0x00463390`): needed for a smooth
   `ViewFeed::player`; Phase 6 movement spec; check against
   `record_frames.py` positions. PROVISIONAL: the local player is drawn
   at the last server-sent position (the message rules of §8 /
   `client/msg-units.md`), no local prediction (because d2rs runs client and server in one
   process with no latency); settled by REC-51.
3. ~~`[0x007A04A4]`~~: answered in §6 rule 4 and §7 rule 11
   (2026-10-08 correction: the ping round trip written by 0x8F, not
   only zeroed); the single-player value is open question 18.
4. ~~Unit flag 0x800000~~: answered in §5 rule 5 (room free
   `0x0061A840`).
5. ~~The client DRLG as a d2rs component~~: answered in §12 (the
   `d2-sim` DRLG act built from 0x03 with the client flag; room of a
   point, fatal asserts, free-point fallback). Open: the map-tile feed
   (RW2) from the client rooms stays with `drlg/` and the render specs.
6. Later draws on the local player's client seed (animation, sounds)
   before a shake reads it (`render/camera.md` OQ6).
7. ~~The visibility predicate `0x004DBF20`~~: answered in §13. Open:
   the 0x48-byte cel context fields beyond frame, component and
   direction, and the `0x006001F0` load arguments (`render/capture.md`
   cel context); a replay of recording A's 0x68 / 0x6B / 0x6C checks
   with a live camera confirms (no C→S 0x5F recorded).
8. Which 0x03 fields the act change path (`0x0053ACC0`) sends for a
   game whose client changes act (a waypoint to another act): settle
   from a recording with an act change (expected: same builder, new act
   byte).
9. Client room seeds versus server room seeds (§12 rule 5): whether a
   client active room's seed (+0x6C) equals the server's for the same
   DRLG room; check by comparing a monster's client `+0x20` seed after
   0xAC at a non-zero point with the server unit's seed (memory read).
10. *Answered (2026-10-08)*: the 0x81 writes are §14 rule 3 (+0x24,
    +0x28, +0x2C and the type-7 unit setup); +0x1C (2026-10-08,
    static, settles REC-50): §14 rule 3 (always 100; read by UI draws
    only).
    Original question: the pet record fields +0x24… written by 0x81 and who
    reads +0x1C: UI (Phase 6); and a recording with a hireling (0x7A /
    0x81 seen) to confirm §14.
11. *Answered:* 0x07 / 0x08 at a point in no DRLG room of the level
    (`impl-client-drlg` §3 Q1): §9 rule 5 (0x07 reads through the null
    room, an access violation at `0x0061B672`; 0x08 tests it and does
    nothing).
12. *Answered:* whether object creation steps the room seed
    (`impl-client-drlg` §3 Q3): yes, §2 rule 6 stands. 0x51 creates
    through `0x00466300` → `0x00465FD0` (call at `0x00466332`), which
    for (x, y) ≠ (0, 0) steps the room's active-room seed (+0x6C/+0x70)
    and sets the unit seed `init_low(lo')` (`0x0046606A`–`0x00466093`),
    the same step as `0x00466200` (0x59) and `0x00466360` (0xAC).
    `client/msg-units.md` Randomness, which named only 0x59 and 0xAC,
    is corrected.
13. *Answered* (static, open question 8): the act change path
    `0x0053ACC0` uses the same builder. After its room switch
    (`0x005381F0`, call `0x0053AE30`) it queues S→C 0x05
    (`0x0053B320(client, 5)`, `0x0053AE39`), stores the new act in client
    +0x1AC (`0x005382E0`, `0x0053AE43`), then calls `0x0053ABE0`
    (`0x0053AE5B`): 0x03 with u8@1 = the new act, u32@2 = game +0x7C
    (the map seed, unchanged), u16@6 = the new act's town level id,
    u32@8 = game +0x80, followed by 0x53 (§11 rule 1). A recording with
    an act change still confirms the bytes (PC 2 recording list).
14. `tp80-packets.jsonl` (PC 2's local recording, not in `traces/raw/`):
    confirm §16 with the S→C order before the 0x4B (the 0x08 that drops
    the hireling's room, no 0x0A / 0x15 / 0xAC for the hireling before
    it), and at the second teleport which message moved the hireling
    out of the freed room (or kept the room in sight).
15. The on-mode / on-use shrine functions (`0x004BD4A0`, `0x004BD090`,
    `0x004BD0C0`, `0x004BD220`, `0x004BD360`) and the overlay calls
    `0x0046F0C0` / `0x00470390` (§15): Phase 6 effects spec.
16. *Answered (2026-10-08)*: §17 rule 6 (the room-change step that
    reaches the town exit, emitted as `TownExit`, `client/bridge.md` §10
    r11 and table). Original question: §17 rule 5: the town exit runs UI code (greeting re-arm, interaction
    end with model writes) from the local player's update, outside any
    message output. d2rs needs a way for the update pass to hand it to
    the UI layer in 1.14d order (e.g. a new `client/bridge.md` §10
    output variant emitted by the update pass; a code-table change) —
    (`client/msg-ui.md` open question 10 is answered by
    `client/bridge.md` §10 r10, which covers the model writes; the hand-off
    of the update pass to the UI layer stays open.)
17. *Answered (2026-10-08)*: §17 rule 4 "Runs per loop pass" (the draw
    runs in tick passes and in every paused pass, not in other passes;
    two wall-clock exceptions). Original question: whether 1.14d ever
    runs the in-game UI draw a different number of times than the
    receive per frame (frame skip, minimized window).
18. §7 rule 11.4: how often a 1.14d single-player ping round trip
    (`ping.rtt`) reaches 78 ms, giving the local player's position
    tolerance L = 1 in §6 rule 4 where d2rs uses 0. Settle with a memory
    read of `[0x007A04A4]` during play, or by recording the client's
    receive time of 0x8F beside the 0x6D send time.
