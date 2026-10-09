# Spec: Client — Bridge (client↔game boundary)

- **Status:** draft; our own design (d2rs), not original behavior, like
  `data/patch-layers.md`. Implemented as a skeleton in
  `d2-client::bridge` (2026-10-06, branch `claude/phase5-bridge`): receive
  path, intent send path, client world model, Bevy mirror; synthetic
  vectors pass (`cargo test -p d2-client bridge`). Owner specs for 51
  S→C ids exist (`client/model.md`, `client/msg-units.md`,
  `client/msg-stats-items.md`); their handlers are not registered yet
  (§6), so the world model holds no game facts. The
  in-process link to `d2-server` is a trait until the server's wiring
  lands (§3, open question 1). 2026-10-07: 62 ids have an owner in
  `bridge-dispatch.tsv` (new: `audio/triggers.md`, `render/lighting.md`,
  `client/msg-ui.md`, `client/msg-skills.md`); the output channel (§10)
  is specified, not implemented. 2026-10-07 (area 3): owners for the
  ids of `sim/intents-events.md` OQ18 (`msg-ui.md` §4–§11,
  `msg-skills.md` §7–§8, 0x89 `render/lighting.md` §10 r4) and the
  no-effect ids of §6 rule 6; the out-of-scope ids stay `TBD` (§6
  rule 7). 2026-10-07 (area 4): the last 32 in-scope ids have owners
  (`msg-units.md` §7–§8, `msg-stats-items.md` §5, `msg-skills.md`
  §9–§10, `msg-ui.md` §16–§22); only the §6 rule 7 ids are `TBD`.
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
| Summary | 58–72 |
| Inputs | 73–81 |
| Outputs / state changes | 82–93 |
| Rules | 94–95 |
|   1. Boundary | 96–119 |
|   2. Receive path | 120–142 |
|   3. Server link | 143–161 |
|   4. Send path (intents) | 162–182 |
|   5. Client world model | 183–202 |
|   6. Dispatch table | 203–254 |
|   7. Bevy mirror | 255–273 |
|   8. Frame pacing | 274–312 |
|   9. Versioning | 313–323 |
|   10. Client outputs (bridge → UI and audio) | 324–509 |
| Constants & data dependencies | 510–524 |
| Randomness | 525–528 |
| Edge cases & original bugs | 529–537 |
| Test vectors | 538–568 |
| Provenance | 569–579 |
| Open questions | 580–620 |
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
- Client outputs (§10): UI and sound requests that handlers emit, in
  1.14d call order, delivered once per frame to the UI and audio layers.

## Rules

### 1. Boundary

1. The bridge contains no game rules. It never decides an outcome, never
   predicts one, and never changes the world model except through a
   dispatched S→C message (or its own counters, §5 rule 3), with one
   exception: the draw's Y sort of a room's unit list. The sort runs on
   the client room lists and persists there (`sim/unit-order.md` §5
   rule 7), so the draw hands each filled room's sorted order back
   (`Bridge::set_room_order`, refused unless it is a permutation of the
   list; checked by `bridge::msg::tests_drlg`'s `set_order` tests). The
   write happens on every frame the fill ran (`seams/bridge-app.md` §2.8).
   *Revision 2026-10-09 (q-fix-seam-room-order): the exception was code
   without a spec line.*
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
2. A spec that takes an id sets its path as
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
6. **No-effect ids** (owner `specs/client/bridge.md`): the
   implementation registers one shared no-op handler for each.
   - Size 0 in the S→C size table (`sim/intents-events.md` §3.1): 0x17,
     0x2B, 0x2D–0x3D, 0x41, 0x43, 0x44, 0x46, 0x49–0x4B, 0x55, 0x56,
     0x64, 0x80, 0x83–0x88, 0xAD, 0xB1. The split stops before such an
     id (§2 rule 3), so the handler is never called.
   - Handlers that do nothing: 0x12, 0x13, 0x14, 0x45, 0x66 (a bare
     `ret`: `0x0045D130`, `0x0045D140`, `0x0045D150`, `0x0045E290`,
     `0x0045E6C0`); 0x24, 0x25 (the empty handler `0x0045C900`); 0xB2
     GameList (system handler `0x0045C850`, jump table `0x0045C894`
     entry 3 = `0x0045C876`, a bare `ret`; 2026-10-08 read).
   - Never produced (no 1.14d function queues them,
     `sim/intents-events.md` §3.5 rule 2, so their handlers never run):
     0x16 (`0x0045D2E0`), 0x54 (`0x0045E3B0` → `0x00473CA0`).
7. **Out of scope** (`sim/intents-events.md` §4 rule 4: multiplayer,
   Battle.net, transport) and unused-in-single-player ids are owned by
   the spec that carries their one-line scope note; the implementation
   registers the shared no-op handler of rule 6 for each until Phase 7
   (2026-10-08, replaces `TBD`): 0x79 (`client/msg-ui.md` §11 r3);
   0x7F, 0x8B–0x8D, 0x90 (`client/msg-units.md` §8 r11); 0xB3
   (`client/model.md` §7 r12); 0xAE WardenRequest (handler
   `0x0045F5F0`): **Out of scope (Phases 0–6): Battle.net anti-cheat**
   (`sim/intents-events.md` §4 rule 4; no record in either recording).
   0x75, 0x8F, 0xAF and 0xB0 are sent in single player (recorded) and
   have behaviour rules: `client/msg-units.md` §8 r10, `client/model.md`
   §7 r10–r11. 0xB4 is also the single-player load refusal
   (`sim/intents-events.md` §8.2 rule 2; client `0x0045C6D0` maps its
   code to `0x0044E380(n)`): owner `client/model.md` §7 rule 8
   (2026-10-08, open question 7).

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
5. Skill fallback (2026-10-08; `client/model.md` §17 rule 4 owns the
   rule and its 1.14d pass analysis): the last step of `bridge_frame`,
   run only when the frame's pump ran a tick and `in_game` holds (the
   1.14d drawn pass, `0x0044F28B`). A paused single-player frame (UI
   state 9 or 11 open, `0x0044EFE3`–`0x0044F029`) runs no `pump` and no
   `receive` (so no server tick, no drain) and only the fallback, once.
   Confirmed 2026-10-09 (pc1-data Step 4 item 2): 1.14d stops the game
   loop, and the stop is in the client loop pass itself (`0x0044EFA0`,
   entered through the pointer set at `0x0044F566`), not in the menu
   code: game type `[0x007A0610]` 0 or 1, `0x00453A90(9)` or
   `0x00453A90(11)` set, a player unit (`0x00463DD0`) in a room
   (`0x004646A0`) → update clock `[0x007A0490]` := now, the draw
   `[0x007A0484]`(0), the sound tick `0x00482C20`, return (`0x0044F029`)
   before `0x004519C0` and the rest of the pass. Open / multiplayer
   games skip the check (`0x0044EFC3`).
   ```
   if sp && (ui(9) || ui(11)) && player && in_room(player) { clock = now(); draw(); sound_tick(); return }
   ```

### 9. Versioning

1. The bridge persists nothing. The wire is the `d2-proto` message set,
   versioned by `d2_proto::PROTOCOL_VERSION` (METHODS M20); the bridge
   refuses a link whose `protocol_version()` differs from its own.
2. `bridge-dispatch.tsv` is source, not a persisted format.
3. A later feature that writes bridge data to disk (received-chunk
   recordings for replay, client settings) is a format with its own
   version field from its first commit; recordings go in
   `traces/FORMAT.md`.

### 10. Client outputs (bridge → UI and audio)

Some S→C messages mean a UI or sound action rather than (or as well as)
model state: 1.14d's handler calls a UI or sound function directly
(0x2C `0x004CBDE0`, 0x5D `0x004A2CB0`, 0x63 `0x0049CF90`, 0x77
`0x004B8CF0`). The bridge carries those calls out as **outputs**.

1. A handler may append outputs to the bridge's output list (`outputs`,
   a field of the bridge, not of `ClientWorld`; §5 rule 1 is
   unchanged). An output is one 1.14d UI or sound entry point with the
   arguments it was called with. The bridge defines only the transport;
   each variant, its payload and what the consumer does is owned by the
   spec that owns the producing message (table below). A spec adding a
   variant adds a row here.
2. **Order.** Outputs are appended in the order 1.14d makes the calls:
   message order within the frame (§2), and within one message the
   handler's own call order. Messages applied in the update pass
   (`client/model.md` §4–§5) emit there, in update order. The list is
   never reordered, merged or de-duplicated.
3. **Captured values.** Each payload holds the values the 1.14d call
   read when the handler ran: message fields, and model facts read by the
   handler (e.g. a unit's key, type, class). A consumer never reads the
   model to fill a payload field, because a later message in the same
   frame may have changed it. A consumer may resolve a unit key for
   something 1.14d tracks over time (a sound's position follows its unit,
   `audio/triggers.md` §1); a key that no longer resolves is rule
   3.1.
   1. **A unit freed after its sound output** (2026-10-08; answers open
      question 6). In 1.14d the request is made inside the handler,
      while the unit exists, and the later free of that unit
      (`0x00465870`, `client/model.md` §2 rule 5) detaches every
      request of the unit without force before anything else
      (`0x004CA9C0(U, 0)`, then the sample-lock release
      `0x004CC160(U, −1)`): a loop whose last unit was U stops, a
      one-shot keeps playing at its last position
      (`audio/triggers-2.md` §19 r5). The request took its position
      from the unit when it was made (`audio/sound-table.md` §5 r3),
      and no sound update runs between the receive and the free. So:
      (a) every model unit free (0x0A and every other free path of
      `client/model.md` §2 r5) appends one `UnitFreed` output {unit
      key} in list order, which the audio layer applies as that
      detach and lock release; (b) an audio output that names a unit
      also captures the unit's position x, y at the call (rule 3), and
      the audio layer uses the captured position when the key no
      longer resolves at delivery. The request is then made, attached
      and detached in 1.14d order, with the 1.14d position. The
      request-log recording of open question 6 stays as the
      conformance check (capture).
4. **Delivery.** At the end of `bridge_frame` (§8 rule 1, after every
   chunk is dispatched and the counters are updated) the list is handed
   over whole and cleared. One dispatcher applies it in list order,
   calling the UI handler or the audio handler per item (§7 rule 1: before
   the input and UI systems of the frame). One frame's outputs are all
   applied before the next frame's receive.
5. **Consumers.** Each variant has exactly one consumer: UI, audio or
   effects (the client effect layer: client missiles, overlays and the
   sounds of client skill code, Phase 6). A 1.14d UI
   function that itself plays sounds or changes UI states is one UI
   output; the UI layer makes those sounds through its own request path
   (`audio/triggers.md` §11). Likewise a 1.14d sound function that also
   shows text is one audio output, and the audio layer makes the text
   request (`audio/triggers.md` §2 r4). A 1.14d function whose result decides
   whether the rest of the handler runs (`SetUIState` returning 1,
   `ui/panels.md` §2) is inside the output, so the UI layer, not the
   bridge, evaluates it.
6. **No feedback into the model.** A consumer never writes `ClientWorld`.
   A UI action that sends a C→S message (e.g. 0x77 → C→S 0x4F) uses the
   send path (§4), so it reaches the server at the next pump (§8 rule 3),
   as in 1.14d where these sends happen inside the receive.
7. Outputs are not persisted and not part of a check by themselves; the
   consumer's check (UI pixels, audio request log) covers them.
8. Mechanical check (with §6 rule 5): every variant in code has exactly
   one row in the table below with the same producer id, and every row
   has a variant.
9. **UI-keyed lookups** (2026-10-08; answers `client/msg-ui.md` open
   question 7). Some 1.14d handlers look a unit up by a key held in UI
   state; the interact NPC (`[0x007C0D25]` GUID, `[0x007C0D29]` active)
   is written only by UI code (`ui/messages.md` §14), so it is UI state
   in d2rs. Such a lookup is made by the UI layer when it applies the
   output, from its own fields at that moment and the facts captured in
   the payload (rule 3); the handler never reads UI state and the UI
   layer never reads the model. This is exact because every UI write
   that 1.14d makes before the handler runs is, in d2rs, either an
   earlier output of the same list (rule 2 keeps their order) or made by
   input between frames, as in 1.14d. A lookup whose unit is not the
   message's own (so its presence cannot be captured at receive) is
   allowed only when its result has no observable effect; otherwise the
   owner spec must add a captured field. Cases:
   - 0x8A (`client/msg-ui.md` §9 r4): exact; the test compares the
     message key with the UI fields, presence is captured.
   - 0x50 code 3 (`client/msg-ui.md` §7 r5): the unit is not the
     message's; its only effect writes two fields that nothing reads,
     so the UI layer may skip it.
   One UI writer runs outside any message: the town exit `0x004B3E10`
   from the local player's update (`client/model.md` §17 r5–r6); it is
   the update-pass output `TownExit` of rule 11.
10. **UI-requested model writes** (2026-10-08, user decision; answers
   `client/msg-ui.md` open question 10). Rule 6 stands: a consumer never
   writes `ClientWorld`. The model writes that 1.14d makes inside UI code
   (the 0x28 dialog branch, `client/msg-ui.md` §16 r4.3 / r5; interaction
   end, menu open and stock discard, `client/model.md` §17 r1–r4) go
   through the bridge: the UI layer, while it applies the output, returns
   each write as a request (unit, the field and value of the owning model
   rule); the bridge applies the requests to the model in request order
   before it handles the next message of the frame, which is the point
   1.14d makes them (inside the receive). The UI layer decides from its
   own state; the bridge does not re-decide. A C→S send the same code
   makes (0x28's 0x31) uses the send path of rule 6. Confirmed
   2026-10-08 (impl-pc1-s5): the 0x28 handler `0x0045D370` calls its UI
   code `0x004B6DD0` directly (`0x0045D37F`), inside the receive.
11. **Update-pass outputs** (2026-10-08; answers `client/model.md` open
   question 16). The client update pass (`client/model.md` §5) may emit
   an output too; its producer in the table is `update`, not a message
   id. The one such output delivered out of order is `TownExit`
   (`client/model.md` §17 r6 step 4); the client object outputs
   `ObjectSound` and `ObjectFx` (the object update and the mode
   requests of the drains, `world/objects-client.md` §28 r3) stay in
   the list in update order under rule 4. In 1.14d the town exit runs inside the local player's
   update, after every UI call of the frame's receive and before the
   rest of the pass, so the bridge delivers it at the point it is
   emitted, as an exception to rule 4: it first hands the UI layer
   every output still in the list (rule 2 order), then `TownExit`, then
   applies the requests the UI layer returns (rule 10: `E`'s model
   writes, `client/model.md` §17 r1) and only then continues the pass.
   The payload captures the local player's key and the GUIDs of every
   monster (type 1) in S at that point, because the UI layer's test
   "the interact NPC (1, `[0x007C0D25]`) is present" (`0x00463990` at
   `0x004B3E71`) looks up a unit that is not the payload's own (rule 9);
   with the GUID set the test is exact. The UI layer then runs
   `0x004B3E10` (greeting re-arm; with the interaction active and the
   NPC present: `[0x007C0C6B]` := 0, `0x00487990`, `E(G)`,
   `0x00455F20(8, 1, 0)`, interaction active := 0;
   `ui/messages.md` §13 r4). Its C→S 0x30 (inside `E`) uses the send
   path (rule 6). Confirmed 2026-10-08 (impl-pc1-s5): `0x004B3E10` has
   one caller, `0x00460E70` (`0x00460EDE`), itself called only from the
   player update `0x00463390` (`0x004636D5`); the room-change step goes
   on after it (`0x0061AA40`, `0x00473C90`, …).

<!-- rows -->
| Variant | Payload | Producer | Consumer | Owner (what the consumer does) |
|---|---|---|---|---|
| `ServerSound` | unit key (type, GUID), unit class, unit x, y (§10 r3.1), event u16 | 0x2C | audio | `audio/triggers.md` §2 r4 |
| `QuestUi` | chain u8, flags u8, status u8, extra i16 | 0x5D (the rows marked output in `client/msg-ui.md` §1 r2; none for model rows or "nothing" rows, §1 r5) | UI | `client/msg-ui.md` §1 |
| `WaypointMenu` | object GUID u32, record 16 bytes (as received) | 0x63 | UI | `client/msg-ui.md` §2 |
| `TradeAction` | code u8, local player absent or dead (`0x00463DF0`, captured) | 0x77 | UI | `client/msg-ui.md` §3 |
| `ChatLine` | the 0x26 record (type, lang, unit type, GUID, u8@8, u8@9, name, text); unit present; a player unit's name | 0x26 | UI | `client/msg-ui.md` §4 |
| `NpcText` | the 40 bytes; unit present; object class (type 2) | 0x27 | UI | `client/msg-ui.md` §5 |
| `HireOffer` | name u16, seed u32 | 0x4E | UI | `client/msg-ui.md` §6 |
| `HireListReset` | — | 0x4F | UI | `client/msg-ui.md` §6 |
| `QuestSpecial` | code u16, six u16 words | 0x50 (codes 1–4, 23, 36) | UI | `client/msg-ui.md` §7 |
| `OpenUi` | GUID u32, code u8, arg u8 | 0x58 | UI | `client/msg-ui.md` §8 |
| `TradePartner` | name 16 bytes, GUID u32 | 0x78 | UI | `client/msg-ui.md` §11 |
| `NpcInteract` | unit key; present; class; monster-data +0x3C; blocker-open flag | 0x8A | UI | `client/msg-ui.md` §9 |
| `NpcIntro` | 12 class slots u16 | 0x91 | UI | `client/msg-ui.md` §10 |
| `GameQuestFlags` | 96 bytes | 0x29 | UI | `client/msg-ui.md` §12 |
| `QuestLog` | 41 bytes | 0x52 | UI | `client/msg-ui.md` §13 |
| `QuestAvailability` | 37 bytes | 0x5E | UI | `client/msg-ui.md` §14 |
| `MercRevive` | u16, u16 | 0x9B | UI | `client/msg-ui.md` §15 |
| `SkillEvent` | unit key, skill, level, target key or point, w | 0x99, 0x9A | effects | `client/msg-skills.md` §7 |
| `SkillDo` | unit key, target key or none, skill, level, x, y, v | 0xA3 | effects | `client/msg-skills.md` §8 |
| `ShrineFx` | kind (on-mode / on-use), shrine code u8, object key, player key or none, overlay ids (two i32, −1 = none) | 0x0E (code 3), 0x4D (code 0x15), 0x51 (shrine, `client/msg-units.md` §1.3 r3) | effects | `client/model.md` §15 rules 3–4 |
| `ShrineSound` | sound id u32, player key | 0x4D (code 0x15) | audio | `client/model.md` §15 rule 4 (request: `audio/triggers.md` §1 rule 1) |
| `UnitOverlay` | unit key, overlay u16, mode (2), sound id (0, 396 or 397) | 0x11 | effects | `client/msg-units.md` §7 r2 |
| `UmodFx` | unit key, the nine umod bytes, flag bit 3 | 0x57 | effects | `client/msg-units.md` §7 r3 |
| `ClientMissile` | local player key, the fields of 0x73 | 0x73 | effects | `client/msg-units.md` §7 r6 |
| `CommonCof` | act index | 0x7E | effects | `client/msg-units.md` §7 r8 |
| `MonsterPreload` | monster class u16 | 0xA4 | effects | `client/msg-units.md` §7 r10 |
| `RosterChanged` | the active roster records (§8 r1 fields) | 0x5B, 0x5C, 0x65 | UI | `client/msg-units.md` §8 |
| `SkillEndFx` | unit key, skill u16, srvdofunc | 0xA5 | effects | `client/msg-skills.md` §10 |
| `QuestFlags` | 96 bytes | 0x28 (type 6) | UI | `client/msg-ui.md` §16 r2 |
| `NpcGone` | GUID u32 | 0x28 (unit absent) | UI | `client/msg-ui.md` §16 r3 |
| `NpcDialog` | type, GUID, 96 bytes, unit key, class, `interact` flag, `0x004B1A10(class)`, cursor item present, keys of `npc` monsters | 0x28 (unit present) | UI | `client/msg-ui.md` §16 r4 |
| `NpcDialogEnd` | type u8 | 0x62 | UI | `client/msg-ui.md` §17 |
| `NpcTransaction` | 15 bytes, local player gold | 0x2A | UI | `client/msg-ui.md` §18 |
| `EventText` | 40 bytes, local player name | 0x5A | UI | `client/msg-ui.md` §19 |
| `ActVideo` | video u8 | 0x61 | UI | `client/msg-ui.md` §20 |
| `OverheadClear` | unit key | 0x76 | UI | `client/msg-ui.md` §21 |
| `HotkeyAssign` | slot u8, skill i32, left u8, item GUID u32 | 0x7B | UI | `client/msg-ui.md` §22 |
| `JoinRefused` | error number u8 (the mapped code) | 0xB4 | UI | `client/model.md` §7 rule 8 |
| `TownExit` | local player key, GUIDs of the S monsters | update | UI | `client/model.md` §17 rule 6; delivery `client/bridge.md` §10 r11 |
| `StateFx` | unit key, state u16, phase (on / hooks / off), bit set before, unit dead, hook number u8 (setfunc / remfunc, 0 = none), two hook values i32 (`client/stat-lists.md` §3 r6.7) | 0xA8 (also 0xA7, 0xA9, 0xAA) | effects | `client/stat-lists.md` §3 rule 6 |
| `UnitFreed` | unit key | 0x0A and every unit free of `client/model.md` §2 r5 | audio | §10 r3.1; `audio/triggers-2.md` §19 r5 |
| `ObjectSound` | the call (mode sound: unit key, set S or C, class, mode; request: id, unit; player event: player key, event) | update | audio | `world/objects-client.md` §25 r2, §26, §28 r3; `client/model.md` §8 rule 7 |
| `ObjectFx` | the call (graphics refresh, graphics load, overlay create / remove, object light, client skill start) with the values read | update | effects | `world/objects-client.md` §26, §28 r3; `render/overlay.md` §5; `render/lighting.md` open question 11 |
| `HoradricItem` | item code 4 bytes | 0x9C (actions 0x04, 0x0B, 0x0C; 0x15 with header mode 0: an `hst ` / `qf2 ` placed in the local player's page 3) | UI | `ui/panels-2.md` §20 r7 |

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
| chunk `12 …` (26 bytes) | no-op handler runs; nothing recorded as unowned | §6 rule 6 |
| chunk `79 …` (6 bytes) | no-op handler runs; nothing recorded as unowned | §6 rules 6, 7 |
| chunk [0x2C for (1, 0x26) event 18; 0x77 code 0x10] | outputs = [`ServerSound` (1, 0x26) 18, `TradeAction` 0x10], in that order, delivered once after the frame | §10 rules 2, 4 |
| frame with no outputs | dispatcher not called; list empty | §10 rule 4 |
| 0x2C for (1, 0x26), then 0x0A removing (1, 0x26), same chunk | `ServerSound` still delivered with the class captured at receive | §10 rule 3 |

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
2. Answered by `client/model.md` §1–§2 (`ClientUnit` fields, the unit
   table, add-replaces and remove rules) and `client/msg-units.md` §1–§2
   (0x59, 0xAC, 0x51 add; 0x0A removes; items: `client/msg-stats-items.md`
   §2). New for the bridge: unit-handler messages are queued on the unit
   at receive and applied in an update pass after the receive, only in
   frames where the server ticked (`client/model.md` §4–§5); §5 rule 4
   and §8 rule 1 follow that once implemented.
3. Answered by `client/model.md` §4 rule 4: the unit handlers of
   0x6E–0x72 are a bare `ret`; whatever unit the lookup finds, the
   message has no effect. "No unit" is equal in effect.
4. Partly answered by `client/model.md` §5: in single player the client
   update (unit updates and queue drains) runs only in loop passes where
   the server ticked. Path stepping and animation between messages stay
   with the Phase 6 movement and unit-modes specs (`client/model.md`
   open questions 1, 2).
5. Answered by `client/model.md` §4 rule 2: the pre-steps of 0x0D, 0x18,
   0x95, 0x96 write nothing any handler reads; they have no effect.
6. *Answered (2026-10-08)*: §10 rule 3.1 (`UnitFreed` output; the
   free's detach is `audio/triggers-2.md` §19 r5); the request-log
   recording below stays as its check. Original question: A sound
   output whose unit is removed later in the same frame (§10
   rule 3): 1.14d starts the sound inside the handler, while the unit
   exists; whether the audio layer must then keep its last position or
   drop the position tracking follows `audio/triggers.md` §1's rule for a
   freed unit. Settle with the request log (`audio/triggers.md` Checks)
   on a 0x2C followed by 0x0A in one chunk.
7. *Answered (2026-10-08)*: `client/model.md` §7 rule 8 (the code map
   read from the jump table, `0x0044E380` in full, the `JoinRefused`
   output) and §6 rule 7 here (owner). Original question: 0xB4 in single player (the load refusal, `sim/intents-events.md`
   §8.2 rule 2): its client handler (`0x0045C6D0`: code u32@1, 1–26 →
   `0x0044E380(n)` with a fixed code map, else 9) ends the game with an
   error screen; owner `client/model.md` §7 (session messages) when the
   join failure path is specified.
