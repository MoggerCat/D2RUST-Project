# Spec: Client — Bridge (client↔game boundary)

- **Status:** draft; our own design (d2rs), not original behavior, like
  `data/patch-layers.md`. Implemented as a skeleton in
  `d2-client::bridge` (2026-10-06, branch `claude/phase5-bridge`): receive
  path, intent send path, client world model, Bevy mirror; synthetic
  vectors pass (`cargo test -p d2-client bridge`). No S→C message has an
  owner spec yet (§6), so the world model holds no game facts. The
  in-process link to `d2-server` is a trait until the server's wiring
  lands (§3, open question 1).
- **Target version:** 1.14d (the message bytes it carries); the bridge
  itself has no 1.14d counterpart to match.
- **Crate/module:** `d2-client::bridge` (`link`, `intent`, `receive`,
  `dispatch`, `world`, `mirror`); dispatch table
  `specs/client/bridge-dispatch.tsv`.
- **Related specs:** `sim/intents-events.md` (owner of the message
  transport: §1 loop order, §2 C→S, §3 S→C, §4 scope); `sim/tick.md` §1
  (host schedule), §8 (wall clock); `sim/client-messages.tsv`,
  `sim/server-messages.tsv` (ids, sizes, layouts). Later client specs own
  what each S→C message means (§6).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 46–60 |
| Inputs | 61–69 |
| Outputs / state changes | 70–79 |
| Rules | 80–81 |
|   1. Boundary | 82–97 |
|   2. Receive path | 98–120 |
|   3. Server link | 121–139 |
|   4. Send path (intents) | 140–160 |
|   5. Client world model | 161–180 |
|   6. Dispatch table | 181–203 |
|   7. Bevy mirror | 204–222 |
|   8. Frame pacing | 223–243 |
|   9. Versioning | 244–254 |
| Constants & data dependencies | 255–269 |
| Randomness | 270–273 |
| Edge cases & original bugs | 274–282 |
| Test vectors | 283–308 |
| Provenance | 309–319 |
| Open questions | 320–339 |
<!-- /index -->

## Summary

The bridge is the only path between the Bevy app and the game
(`CLAUDE.md` hard rules 5 and 7, `docs/ARCHITECTURE.md`). Outbound, it
turns client requests into C→S message bytes built with `d2-proto`
(1.14d ids and layouts) and hands them to the server link. Inbound, it
takes the S→C bytes exactly as `d2-server` flushes and delivers them,
splits them into messages with `d2-proto`'s size rule, and dispatches
each by id to a handler that updates a plain-Rust client world model.
Handlers belong to later specs; an id without one is recorded, never
interpreted. Bevy entities only mirror the world model for drawing; no
game state lives in Bevy. Single player runs `d2-server` in-process,
driven once per client frame in the 1.14d order (drain → tick → flush →
receive).

## Inputs

| Name | Type | Source |
|---|---|---|
| S→C chunk | bytes, 1..=0x200 (a flushed buffer) or one delivered message (≤ 0x204) | server link `receive` (§3) |
| link frame report | ticked: bool | server link `pump` (§3) |
| client request | a `d2-proto` typed C→S message, or raw C→S bytes for variable layouts | client input systems (Phase 6) |
| dispatch table | `bridge-dispatch.tsv` + handlers registered in code | §6 |

## Outputs / state changes

- C→S message bytes passed to the server link, one per request (§4).
- The client world model (§5): updated only by dispatched handlers and by
  the bridge's own frame counters.
- A receive log (§2.4): per-id counts of unowned messages, bytes the
  split discarded, handler rejections.
- Bevy mirror entities (§7), spawned, updated and despawned from the
  world model each frame.

## Rules

### 1. Boundary

1. The bridge contains no game rules. It never decides an outcome, never
   predicts one, and never changes the world model except through a
   dispatched S→C message (or its own counters, §5 rule 3).
2. Only the bridge talks to the server link. Bevy systems reach the game
   only through the bridge resource (§7): they read the world model and
   call the send path.
3. Everything in `bridge` except `mirror` is plain Rust with no Bevy
   type, testable without a window, GPU or Bevy `App`.
4. Message bytes cross the boundary unchanged in both directions: the
   bytes the client sends are the 1.14d C→S bytes, the bytes it handles
   are the 1.14d S→C bytes (`intents-events.md` §4). No d2rs-own message
   exists yet; if one is added it is a `d2-proto` message with a version
   bump (§9).

### 2. Receive path

1. Input unit: a **chunk** of S→C bytes in delivery order, as the link
   returns it. A chunk is either a whole flushed buffer
   (`intents-events.md` §3.2) or one node of the client's receive lists
   (§3.3); both are split the same way, because 1.14d's game-list handler
   also walks a node's bytes message by message with the size rule
   (§3.4 rule 3).
2. Split with `d2_proto::transport::split_server_buffer` (the S→C size
   rule, `intents-events.md` §3.1). The messages before the stop are
   dispatched in order (§2.3).
3. Size rule result 0 (id with size 0, id ≥ 0xB5, too few bytes) ends
   the split; the rest of the chunk is discarded, as in 1.14d (§3.3 rule
   3). The bridge records it (§2.4) with the first discarded byte (the id
   the rule refused) and the byte count. It never guesses a size.
4. A message over 0x204 bytes, or one whose size runs past the chunk's
   end, is a fatal assert in 1.14d (§3.3 rule 2, §3.2 rule 2): the whole
   chunk is refused with an error (nothing in it is dispatched) and the
   error stops the frame (§8 rule 4).
5. Chunks are processed in the order the link returns them. The link
   returns the client's system list before its game list
   (`intents-events.md` §3.4 rule 1); the bridge does not reorder.

### 3. Server link

1. The link is a narrow trait (`bridge::ServerLink`) in `d2-client`, so
   the bridge builds and tests without the server's wiring:

   | Method | Does | `d2-server` host equivalent |
   |---|---|---|
   | `protocol_version()` | the server's `d2_proto::PROTOCOL_VERSION` | constant |
   | `send(queue, bytes)` | hand one C→S message to the server's sender for `queue` (game or system); returns queued or filtered | `Host::send_game` (duplicate filter, `intents-events.md` §2.1 rule 1), `Host::send_system` |
   | `pump()` | one server frame: drain → tick driver → flush if a tick ran; returns whether a tick ran | `Host::frame` |
   | `receive()` | the chunks delivered to this client since the last call, system list first | `Host::receive(client)` |

2. The client id of the local player is 0 (`intents-events.md` Inputs).
3. The link owns the host clock (`tick.md` §1, §8). The bridge never
   reads a clock for game purposes and never passes Bevy time to the
   server.
4. Online play (Phase 7, deferred) is another `ServerLink` over
   `d2-net`; the bridge does not change.

### 4. Send path (intents)

1. A typed request is encoded with `d2-proto`
   (`FixedMessage::write`, the TSV layout; unlisted bytes 0). Variable
   C→S messages (chat 0x14/0x15, save chunks, warden) are passed as raw
   bytes built by their owner spec.
2. Every message is classified with `d2_proto::transport::classify_client`
   before it is sent. Queue game (ids < 0x67) → `send(Game, ..)`; queue
   system (0x67..=0x70) → `send(System, ..)`.
3. Refused, with an error and nothing sent: any other classifier result
   (incomplete, invalid id, negative chat size), id 0xFF (queue 2, realm
   / admin, out of scope, `intents-events.md` §4 rule 4), and a game
   message of 0x200 bytes or more (the 1.14d sender asserts size < 0x200,
   §2.1 rule 1). The bridge never sends a message 1.14d's transport
   would drop or assert on.
4. Sending is immediate: the bytes reach the server queues during the
   client frame, and the next `pump` drains them before its tick
   (`intents-events.md` §1 rule 2).
5. The duplicate filter belongs to the link (the 1.14d client sender);
   a filtered send is reported as `Filtered`, not an error.

### 5. Client world model

1. `ClientWorld` is plain Rust: the client's knowledge of the game, as
   the S→C messages have stated it. It is not game state: the server's
   `d2-sim` is the only game state. It holds no field whose meaning no
   owner spec has stated.
2. Units are keyed by (unit type, GUID) (`sim/unit-order.md` §1 rule 1),
   in a `BTreeMap` (iteration order is the key order, so the mirror is
   deterministic). Which messages create, change or remove a client unit
   is owned by later specs (§6); today `ClientUnit` holds only its key.
3. Bridge-owned counters: `frames` (bridge frames run) and
   `server_ticks` (frames whose `pump` ran a tick). They are bookkeeping
   for views and tests, never input to an outcome.
4. Handlers receive the message (id, bytes) and the unit it addresses
   when the receive table has a unit handler for the id
   (`intents-events.md` §3.4 rule 3, `client_unit_handler` column):
   ids 0x67–0x6D → (1 monster, u32 at +1); other ids with a unit handler
   → (u8 at +1, u32 at +2). A message too short for its lookup gives no
   unit (open question 3).

### 6. Dispatch table

1. One row per S→C id 0x00..=0xB4 in `bridge-dispatch.tsv` (header `id
   name owner`; `0x`-prefixed hex ids in order; `name` = the
   `server-messages.tsv` name). `owner` is the spec that owns what the
   message means for the client model, or `TBD`.
2. Today every row is `TBD`. A spec that takes an id sets its path as
   owner and the implementation registers one handler for the id in
   `dispatch::HANDLERS`.
3. A message whose row has no handler is **unowned**: counted per id in
   the receive log and otherwise ignored. It is not an error: the server
   sends ids the client cannot yet use.
4. A handler that cannot read its message (wrong size for its layout, a
   field out of the range its spec allows) returns an error; the bridge
   records the rejection (id, error) and continues with the next message.
   The message's effect is then missing from the model, which the
   owner's check must catch.
5. Mechanical check (METHODS M05, M08): the TSV has exactly the ids
   0x00..=0xB4 in order, each name equals `d2_proto::SERVER_MESSAGES`,
   and a row has an owner other than `TBD` if and only if
   `dispatch::HANDLERS` registers a handler for its id, with the same
   owner. A perturbed TSV must fail with exactly the changed row.

### 7. Bevy mirror

1. `BridgePlugin` inserts the bridge as a resource and adds two systems
   to `PreUpdate`, in order: `bridge_frame` (§8) then `mirror_units`.
   Input and UI systems run later in the frame and send through the
   resource.
2. Mirror entities carry `UnitView` (the unit key and copies of the
   model fields the views need). Components are views: overwritten from
   the model every frame, never written back, never read by the bridge.
3. Mirror rule per frame: a unit in the model without an entity →
   spawn; an entity whose unit left the model → despawn; every remaining
   entity's view components are updated from the model. Iteration in key
   order.
4. Rendering (position projection, animation, draw order, interpolation
   between ticks) is owned by the Phase 6 render specs. The bridge does
   not interpolate or predict: 1.14d's client advances its own views
   between server messages, and that behaviour must be specified and
   matched pixel for pixel (`CLAUDE.md` hard rule 10), not invented here.

### 8. Frame pacing

1. One Bevy frame runs one bridge frame, before game input is read: `pump`
   (the server drains the intents sent last frame, runs the tick driver,
   flushes if a tick ran) → `receive` and dispatch every chunk →
   `frames` += 1, `server_ticks` += 1 if a tick ran. This is the 1.14d
   single-player client frame (`tick.md` §1 rule 4,
   `intents-events.md` §1 rule 1); the input systems that follow are
   "the rest of the client frame".
2. The tick schedule is the host's (`tick.md` §1 rules 2–3): at most one
   tick per pump, with the lag carried forward capped at one tick. A
   Bevy frame faster than 40 ms runs pumps without a tick; a slower one
   runs one tick per frame, so the game slows like 1.14d's. The bridge
   adds no catch-up loop and never calls `pump` twice in a frame.
3. Messages sent during frame k are drained by the pump of frame k+1,
   and their effects arrive no earlier than that frame's flush.
4. Errors stop the frame: a link error or a refused chunk (§2 rule 4)
   is returned from `bridge_frame` as an error (Bevy's error handler
   reports it). Unowned ids, discarded bytes and handler rejections are
   recorded, not errors.

### 9. Versioning

1. The bridge persists nothing. The wire is the `d2-proto` message set,
   versioned by `d2_proto::PROTOCOL_VERSION` (METHODS M20); the bridge
   refuses a link whose `protocol_version()` differs from its own.
2. `bridge-dispatch.tsv` is source, not a persisted format.
3. A later feature that writes bridge data to disk (received-chunk
   recordings for replay, client settings) is a format with its own
   version field from its first commit; recordings go in
   `traces/FORMAT.md`.

## Constants & data dependencies

| Constant | Value | Owner |
|---|---|---|
| S→C ids | 0x00..=0xB4 | `intents-events.md` §3.1 |
| max message | 0x204 bytes | §3.3 rule 2 |
| buffer size | 0x200 bytes | §3.2 rule 1 |
| C→S game send limit | < 0x200 bytes | §2.1 rule 1 |
| local client id | 0 | `intents-events.md` Inputs |
| tick length | 40 ms (host) | `tick.md` §1 rule 1 |

Data: `sim/server-messages.tsv` (sizes, names, unit-handler column),
`sim/client-messages.tsv` (layouts, classifier), through `d2-proto`;
`client/bridge-dispatch.tsv`.

## Randomness

None. The bridge draws no random numbers.

## Edge cases & original bugs

1. S→C ids with size 0 that a server builder can still produce (0x83,
  0x84, 0x88, `intents-events.md` §3.1 rule 3) end the split: the bridge
  discards the rest of the chunk exactly as 1.14d does and records it.
2. 0x80 can never be received (size 0, §3.1 rule 2); same handling.
3. Ids 0x6E–0x72 have a unit handler but are 1 byte long: the unit lookup
  of §5 rule 4 has no bytes to read (open question 3).

## Test vectors

Synthetic, run as unit tests in `crates/d2-client/src/bridge/tests.rs`.

| Input | Expected | Source |
|---|---|---|
| send `Walk { x: 0x1234, y: 0x5678 }` | link gets (game, `01 34 12 78 56`): `x` at the layout offset of `x`, `y` at that of `y` | `client-messages.tsv` 0x01 |
| send `WalkToUnit { type: 1, id: 0xAABBCCDD }` | (game, `02 01 00 00 00 DD CC BB AA`) | `client-messages.tsv` 0x02 |
| send raw `6B` | (system, `6B`) | §4 rule 2 |
| send raw `80 80 80 80 80` | error invalid id, nothing sent | §4 rule 3 |
| send raw `01 00 00` | error incomplete, nothing sent | §4 rule 3 |
| send raw 16 × `FF` | error admin queue, nothing sent | §4 rule 3 |
| chunk `1A 07 5F 01 02 03 04` | two messages 0x1A, 0x5F; both unowned (count 1 each) | §2, §6 rule 3 |
| chunk `1A 07 80 1A 07` | one message 0x1A; discarded 3 bytes, first byte 0x80 | §2 rule 3 |
| chunk `1A 07 5F 01` | error truncated at byte 2 (size 5); nothing dispatched | §2 rule 4 |
| chunk with 0x16, u16 at +1 = 0x205 | error too large | §2 rule 4 |
| 0x6D, 10 bytes, u32 at +1 = 7 | addressed unit (1, 7) | §5 rule 4 |
| 0x0E, 12 bytes, +1 = 2, u32 at +2 = 9 | addressed unit (2, 9) | §5 rule 4 |
| 0x6E (1 byte) | no addressed unit | §5 rule 4 |
| 0x1A (no unit handler) | no addressed unit | §5 rule 4 |
| frame with link reporting a tick, then one without | `frames` 2, `server_ticks` 1 | §8 rule 1 |
| intent sent after frame 1 | link sees it before pump 2 | §8 rule 3 |
| link version ≠ `PROTOCOL_VERSION` | bridge refuses the link | §9 rule 1 |
| test handlers add units (1, 7), (2, 9); Bevy `App` update | 2 `UnitView` entities in key order; after removing (1, 7) and one update, 1 entity | §7 rule 3 |
| dispatch TSV with one owner changed | check reports exactly that id | §6 rule 5 |

## Provenance

d2rs design, decided 2026-10-06 (`docs/PLAN.md` decisions log). Built on
`docs/ARCHITECTURE.md`, `docs/EARLY_DECISIONS.md` items 1–3, `CLAUDE.md`
hard rules 5–7 and 10, and the 1.14d facts it relies on, each owned and
sourced elsewhere: loop order and frame structure (`tick.md` §1,
`intents-events.md` §1), transport split and size rules
(`intents-events.md` §3), unit lookup of the receive table (§3.4 rule 3),
unit identity (`unit-order.md` §1). Written in an implementation session
from `specs/`, `docs/` and `crates/` only.

## Open questions

1. The `d2-server` host's concrete wiring (`d2-proto` sizes, `d2-sim`
   intents and tick, session code) is on another branch. The `ServerLink`
   adapter over `d2_server::host::Host` (method mapping in §3 rule 1) is
   written once that lands; until then the link is the trait plus test
   fakes.
2. Whether `ClientUnit` needs fields beyond its key, and which S→C ids
   create and remove client units, belongs to the first client-model
   spec (unit add/remove messages, Phase 5–6).
3. Ids 0x6E–0x72 (1 byte) have a unit handler in the receive table: what
   1.14d's unit lookup reads for them (bytes past the message in the
   node) is not in `intents-events.md` §3.4. The bridge gives no unit.
4. 1.14d's client frame rate and what its client does between ticks
   (path stepping, animation) decide how views advance (§7 rule 4,
   §8 rule 2); owned by the Phase 6 animation spec.
5. The per-id pre-steps of the receive handler (0x0D type byte 1, 0x18,
   0x95, 0x96; `intents-events.md` §3.4 rule 3) are client semantics; the
   owner spec of those ids states them.
