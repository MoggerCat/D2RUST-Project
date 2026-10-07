# Spec: Simulation — Intents in, events out (client↔server messages)

- **Status:** draft: transport, dispatch, gating, size tables and the
  per-client buffering read from the 1.14d `Game.exe` (addresses below);
  the recorder (`tools/trace-recorder/record_packets.py`) installs on the
  reference `Game.exe` (smoke run, no game entered) and the checker
  passes its self-test (clean synthetic trace, 223/223 single-byte
  perturbations reported); rules R1–R7 hold on a hand-played single-player
  game: `check_packets.py` passes `traces/raw/20261006-015956-packets.jsonl`
  (4,239 ticks, 494 client messages of 25 ids, 1,825 server messages of
  62 ids, 4,239 flushes). `d2-server` implements §1–§3 (transport,
  queues, drain, dispatch gate, size check, point/unit parse, buffers,
  flush, local delivery) against seams for `d2-proto` and `d2-sim`
  (branch `claude/phase3-server`), wired to `d2-proto` and `d2-sim`
  through adapters (`claude/phase3-wiring`; intent handlers are stubs
  until their system specs exist); the synthetic vectors pass as unit
  tests; not yet run on a recording.
- **Target version:** 1.14d
- **Crate/module:** `d2-proto` (message ids, sizes, layouts: the two TSVs);
  `d2-server` (queues, drain, dispatch gate, per-client buffers, flush);
  `d2-sim` (intent handlers, event production)
- **Related specs:** `sim/tick.md` (owns the host loop: drain → tick →
  flush, §1; frame counter §2; client pass §6); `sim/unit-order.md`
  (client list order, GUIDs); `sim/rng.md`; per-system specs (movement,
  skills, items, NPCs, quests; Phase 3+) own what each intent does and
  which events it produces. Machine tables: `sim/client-messages.tsv`,
  `sim/server-messages.tsv` (§5).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 52–68 |
| Inputs | 69–77 |
| Outputs / state changes | 78–84 |
| Rules | 85–86 |
|   1. Loop order (single player) | 87–108 |
|   2. Client → server | 109–278 |
|   3. Server → client | 279–378 |
|   4. d2rs mapping and scope | 379–406 |
|   5. Machine-readable tables | 407–443 |
|   6. Exact-match comparison | 444–529 |
|   7. Unit update messages (`0x0053A500`) and room clean-up (`0x00553220`) | 530–891 |
|   8. Single-player session sequence (C→S 0x67 → 0x6B → first tick) | 892–1008 |
| Constants & data dependencies | 1009–1027 |
| Randomness | 1028–1033 |
| Edge cases & original bugs | 1034–1067 |
| Test vectors | 1068–1131 |
| Provenance | 1132–1207 |
| Open questions | 1208–1264 |
<!-- /index -->

## Summary

In single player the client and the server run in one process and talk
only through byte messages. The client sends a message per request
("intent": walk, cast, pick up, buy, ...). A local transport classifies
it by id and size and appends it to one of three server queues. Before
each tick the server drains the queues in order and dispatches every game
message to its handler, after a per-message gate (player alive, or dead
for resurrect). Handlers validate the exact size and the fields and either
act or refuse with a result code; 1.14d ignores the code. Everything the
server reports ("events": unit moves, stat changes, item actions, ...) is
appended to the receiving client's buffers of up to 0x200 bytes, which
are flushed after the tick, split back into messages by a size table and
handed to the client's receive handlers in the same client frame.
Message bytes, order and per-tick grouping are the exact-match surface of
the simulation (§6).

## Inputs

| Name | Type | Source |
|---|---|---|
| client message | bytes, 1..0x204 | client send path (§2.1) |
| client id | u32 | transport; 0 for the local client |
| game, player unit | records | the server's client record (`unit-order.md`) |
| frame counter | game +0xA8 | `tick.md` §2 |

## Outputs / state changes

- Handler effects on the game (owned by system specs).
- Server→client messages appended to per-client buffers (§3.2), flushed
  after the tick; a few system messages bypass the buffers (§3.3).
- Result code per dispatched message (0–3), discarded (§2.3).

## Rules

### 1. Loop order (single player)

The host schedule is `tick.md` §1 (owner). Message-relevant facts:

1. One client frame (`0x0044EFA0`, game type `0x007A0610` = 0) runs, in
   order: drain all server queues (`0x0052CFE0`, §2.1) → tick driver
   (`tick.md` §1) → if a tick ran, flush every client (`0x0052FD90(1, 0)`,
   §3.2) → client receive (`0x0044C6E0`, §3.4) → rest of the client frame
   (input handling queues new client messages).
2. A client message sent during client frame k is drained at the start of
   frame k+1, before that frame's tick. All messages that arrived since
   the last drain are processed in arrival order, outside any tick, with
   the frame counter of the previous tick.
3. Server messages queued while handling client messages (step 1's
   drain) and during the tick are flushed together after the tick. When
   no tick runs, nothing is flushed and buffers keep accumulating.
4. Flushed messages reach the client's receive lists at once (local mode
   1, no delay; §3.3 rule 4) and are handled in the same client frame.
5. Other flush calls: `0x005303D0` (system message 0x69, leave game,
   flushes the leaving client) and `0x0052E440` (game-server worker path
   `0x005645E0`, not used by single player).

### 2. Client → server

#### 2.1 Transport (local mode)

1. **Client game-message sender** `0x00478350` (EDI = size, [ESP+4] =
   message). Asserts size < 0x200. Duplicate filter: if the message is
   byte-identical (compared over `size` bytes) to the last message this
   sender sent (`0x007BB3B8`, time `0x007BB5B8`) and less than a window
   has passed since then, it is **not sent** and the stored time is not
   refreshed. Window by id: 50 ms for 0x05–0x0A and 0x0C–0x11; never
   filtered: 0x3A; 200 ms for every other id (table `0x00478550`,
   targets `0x00478544`). Sent messages replace the stored copy and time
   and go to `0x0052AE50(size, 1, message)`.
2. System messages (0x67–0x70) are sent by small senders straight to
   `0x0052AE50` (e.g. 0x6B `0x00477DA0`, 0x69 `0x00477EE0`), without the
   filter.
3. `0x0052AE50` in local mode (`0x0052B7E0`: global `0x00882D10` ∈ {1,
   2}; single player sets 1 via `0x0052A750`): asserts size ≤ 0x204 and
   calls `0x0052B690` → `0x006BF370(net, message, size, client id 0)`.
4. `0x006BF370` asks the classifier `0x0052B100` (registered at
   `0x0052B7A0`, net object `0x00882D08`) for a queue:

   | Classifier step | Result |
   |---|---|
   | size < 1 | 3 (incomplete) |
   | id 0x71..0xFE | 4 (invalid) |
   | size rule (`0x0052BC20`, rule 5) gives 0, or > 0x204, or > the given size | 3 |
   | id < 0x67 | queue 1 (game), result 1 |
   | id 0x67..0x70 | queue 0 (system), result 2 |
   | id 0xFF (rule: 16 bytes) | queue 2, result 1, if the net object's gate `0x006BF6C0` passes; else 4 |

   Results 1 and 2 enqueue; 3 and 4 **drop the message silently** in
   local mode (network mode: 3 waits for more bytes, 4 bans the sender,
   `0x006C08A0`). The queued copy is the whole given buffer (`size`
   bytes), not the rule size.
5. **Size rule** `0x0052BC20` (the C→S size table, `0x00730DC0`, 0x71
   i32 entries, one per id; machine copy: `transport_size` column):
   - entry > 0: fixed size;
   - entry 0: the id is never queued (ids 0x00, 0x2B, 0x2C, 0x4A, 0x4E,
     0x55–0x57, 0x5A–0x5C, 0x64, 0x65, 0x6F);
   - entry −1: variable, by id (jump table `0x0052BD40`/`0x0052BD30` on
     id − 0x14):

   | Ids | Rule (all counts in bytes, b = message) |
   |---|---|
   | 0x14, 0x15 (`chat`) | needs ≥ 3; L1 = strlen(b+3), needs ≥ L1+4; L2 = strlen(b+L1+4), needs ≥ L1+L2+5; c = signed byte at b+L1+L2+5; size = L1+L2+6+c, needs ≥ size |
   | 0x66 | needs ≥ 3; w = u16 at b+1, w > 0x1FD → 0; size = w + 3 |
   | 0x6C | needs ≥ 6; size = u8 at b+1, + 7 |
   | other −1 entries | none exist |

6. Queues are FIFO linked lists of 0x5C0-byte nodes (client id u32 at +0,
   message at +4, max 0x5B4 bytes; size at +0x5B4, enqueue
   `GetTickCount` at +0x5B8, next at +0x5BC), guarded by one critical
   section, with a free list.
7. **Drain** `0x0052CFE0`: pops queue 0 until empty, calling
   `0x0053F100` per message; then queue 1 → `0x0053F3D0`; then queue 2 →
   `0x0052CC20`. The pop (`0x006BEAD0`) copies at most 0x200 bytes
   (client id + message) into a stack buffer but returns the full size:
   a message longer than 0x1FC bytes is truncated in the copy while the
   handler sees the full size (only id 0x66 can be that long).

#### 2.2 Game message entry `0x0053F3D0`

ECX = buffer (client id, then message), EDX = size.

1. Look up the client's game (`0x0052FEE0`, enters the game's lock); none:
   log "[ProcessClientMessage] Client %d is not in any game" and drop.
2. Look up the client record (`0x00537810`); none: fatal assert.
3. Client +0x3D8 = `GetTickCount()` (last-message time; host only).
4. Player = `0x00537860(client, 0)`; if it exists and is a player unit
   (unit type 0), dispatch (§2.3). Otherwise the message is dropped.
5. Leave the game's lock (`0x0052DA90`). **The dispatch result is not
   used**: 1.14d has no hack-list action here (D2MOO 1.10f logs
   "[HACKLIST]" and drops the client for a result ≥ 3 outside game type 3).

#### 2.3 Dispatcher `0x0054D750`

ECX = game, EDX = player, [ESP+4] = message, [ESP+8] = size; handlers are
called the same way (fastcall game, player; stack message, size).

1. id = first byte. id 0 or id ≥ 0x67 → return 3.
2. Handler table `0x006E0D18`: 0x67 entries of 8 bytes (handler pointer,
   then a flag word that 1.14d never reads). Null handler → return 3
   (ids 0x2B, 0x4A, 0x4E, 0x55–0x57, 0x5A–0x5C, 0x64, 0x65).
3. Gate (jump table `0x0054D848` → `0x0054D838` on id − 0x14; ids below
   0x14 take the default):
   - **none**: 0x14, 0x15, 0x3C, 0x43, 0x66;
   - **dead**: 0x41: player present and its mode (unit +0x10) = 0x11
     (dead); else return 0;
   - **alive** (all other ids): `0x0057EEC0(game, player, mode 1, 0, 0)`
     must be non-zero, else return 0. With these arguments it returns 0
     when the player has state 0x36 (54, uninterruptable), is null, or is
     in mode 0 (death) or 0x11 (dead); otherwise 1.
4. If game +0x1DC4 > 1, set it to `0x00410A80()` (a sync timer of the
   host; never read by the simulation).
5. `IsBadCodePtr(handler)` → fatal assert; else return the handler's
   result.

Result codes (all handlers): 0 = done; 1 = refused (target missing, out of
range, wrong state); 2 = invalid field (bad unit type, wrong act, out of
range index); 3 = malformed (wrong size, stub, bad id). §2.2 rule 5: none
of them has a consequence in 1.14d; d2rs keeps them as a diagnostic only.

#### 2.4 Handler contract

1. Every handler first checks the **exact** size (`handler_size` column,
   `==N`) and returns 3 otherwise; transport and handler sizes agree for
   every id (TSV). Exceptions: 0x14 accepts 4..275 bytes; 0x15 checks its
   strings (§2.4 rule 6); 0x66 does nothing.
2. Stubs: 0x2C, 0x2D, 0x39, 0x45, 0x52 return 3; 0x2E, 0x42, 0x43, 0x66
   return 0 (`kind` column). 0x45 (D2MOO: change town-portal location) and
   0x39 (D2MOO: purchase life) are stubs in 1.14d.
3. Point messages (0x01, 0x03, 0x05, 0x08, 0x0C, 0x0F) share the parser
   `0x005496F0`: size 5; player data (`0x006221A0`) required, else 2;
   target (x, y) must be within 50 subtiles of the player on both axes
   (`0x00548EF0`, Chebyshev test |dx| ≤ 50 and |dy| ≤ 50; position from
   the dynamic path, or the static path for unit types 2, 4, 5; the
   message's x, y are zero-extended u16 and a player's dynamic position
   is a u16 sub-tile word, so the signed 32-bit difference never wraps
   and the test is exact for every input). Out of
   range → 1, and if more than 25 frames passed since player data +0x168,
   the server queues message 0x15 (reassign player) to resync the client;
   in range → player data +0x168 = frame, accept.
4. Unit messages (0x02, 0x04, 0x06, 0x07, 0x09, 0x0A, 0x0D, 0x0E, 0x10,
   0x11) share `0x00549830`: size 9; unit type < 6 else 2; `0x00548F80`:
   unit missing → 1; an item the player owns → accept; target in another
   act than the player → 2; else the same 50-subtile test (→ 1).
5. Skill messages (0x05–0x11 except 0x0B), after parsing: player stat
   `pierce_idx` (328, `0x148`) += 1, then the skill request.
6. 0x14 (`0x0054A290`): msg = cstr at +3, 1 ≤ strlen < 256 else 2; name =
   cstr after it (≤ 16). 0x15 (`0x0054A5D0`): msg strlen < 256 and
   strlen + 4 < size, else rejected.
7. 0x3C: u32 at +1, bit 31 = left hand, bits 0–30 = skill id, must be <
   the skills count (data +0xBA0); item u32 at +5. 0x51: u32 at +1: bits
   0–14 skill (> count → unbind, −1), bit 15 left hand, bits 16–31 slot
   (≤ 15 else 3); item u32 at +5.
8. Expansion-only: 0x60, 0x61, 0x62 check game +0x70 (expansion); 0x62
   returns 3 in a classic game.
9. Several handlers read a 1-byte field at +5 although the message is 9
   bytes (0x1A, 0x1B, 0x1D, 0x1E: body location); bytes +6..+8 are
   ignored. D2MOO 1.10f declares them u32.
10. Layouts: `layout` column, `name:type@offset`; types `u8`, `u16`,
    `u32` little-endian, `cstr` NUL-terminated, `cstr16` 16-byte field,
    `uN@k` / `bitN@k` = bit fields of the u32 at k. Only offsets a 1.14d
    handler reads are listed.

#### 2.5 System messages `0x0053F100`

ECX = buffer (client id, message), EDX = size. Switch on the id:

| Id | Does (1.14d) |
|---|---|
| 0x67 | if `0x0052C330` allows: create a game with a client (`0x00530BF0`) from the fields in the TSV layout |
| 0x68 | join: optional check `0x0053EFF0` when host callbacks `0x00883D50` are absent; `0x0052C690` validates, `0x0052FA50` joins |
| 0x69 | leave (`0x0052C8E0` → `0x005303D0`, which flushes the client) |
| 0x6A | `0x0052DAF0` → `0x0052E9B0` |
| 0x6B | `0x0052C550` → `0x00530190` |
| 0x6C | total (u32 at +2) ≥ 0x2000 → fatal assert; `0x0052DB00` → `0x0052DB10` (save upload chunk) |
| 0x6D | `0x0052C400(u32 at +5, 0)` (ping) |
| 0x6E | `0x0052CBE0` → `0x00530270` |
| 0x70 | client record +0x504 = 1, `0x005377A0` |
| others | ignored |

The ids are D2MOO's 1.10f system ids + 1: 1.14d inserted 0x66 (warden
response) and shifted 0x66–0x6F of 1.10f to 0x67–0x70. Which of them a
single-player session sends, and in which order, is open (question 2).

Queue 2 (id 0xFF, 16 bytes, `0x0052CC20`) runs only when host callbacks
(`0x00883D50`) exist: realm/admin commands, out of scope.

### 3. Server → client

#### 3.1 Size rule and receive table

1. S→C size table `0x00730AE8`: 0xB5 i32 entries (ids 0x00–0xB4), read by
   `0x0052B920` (machine copy: `size` column). id ≥ 0xB5 → 0. Entry 0 →
   result 0 (the id is not a valid message). Entry −1 → variable (jump
   table `0x0052BB80` → `0x0052BB44` on id − 0x16):

   | Id | Needs ≥ | Size |
   |---|---|---|
   | 0x16 | 13 | u16 at +1 |
   | 0x26 | 10 | 10 + (strlen(b+10)+1) + (strlen of the next string + 1) |
   | 0x3E | 2 | u8 at +1 |
   | 0x5B | 34 | u16 at +1 |
   | 0x94 | 9 | (u8 at +1 + 2) × 3 |
   | 0x9C, 0x9D | 3 | u8 at +2 |
   | 0xA6 | 4 | u16 at +2 |
   | 0xA8, 0xAA | 7 | u8 at +6 |
   | 0xAC | 13 | u8 at +12 |
   | 0xAE | 3 | u16 at +1 (> 0x1FD → 0) + 3 |
   | 0xAF | 2 | u8 at +1 = 0 → 2, else u8 at +1 + 1 |
   | 0xB3 | 8 | u8 at +1 + 7 |

   Fewer bytes than "needs" → result 0 (incomplete).
2. Client receive table `0x007114D0`: 0xAF entries of 12 bytes: general
   handler, expected size (−1 = variable), unit handler. `0x0045F7B0`
   asserts expected = size-table size for every received message
   (fatal `0x1450` otherwise); they agree for every id except 0x80
   (expected 4, table 0, no handler: 0x80 can never be received).
   `0x0045C900` is an empty handler.
3. Ids with size 0 are not receivable even where a server builder exists:
   0x83, 0x84, 0x88 (`0x0053DC20`, `0x0053DCC0`, `0x0053DD20`). Sending one
   would end the split of its buffer (§3.3 rule 3) and lose the rest.

#### 3.2 Per-client buffers and flush

1. **Queue a message** `0x0053B280` (EDI = client record, [ESP+4] =
   message, [ESP+8] = size; D2MOO `D2GAME_PACKETS_SendPacket`). Client
   null → nothing. Tail buffer = client +0x1BC (head +0x1B8). If there is
   no tail, or tail.size + size > 0x200, take a buffer from the client's
   free list (+0x1C0) or allocate one (0x208 bytes), zero it, append it
   (`0x00539240`). Copy the message at tail.data + tail.size; tail.size
   += size. Buffer layout: size u32 at +0, data at +4 (0x200 bytes), next
   at +0x204.
2. A buffer holds whole messages only; messages are never split across
   buffers; a buffer is full at exactly 0x200 bytes (`>` test).
3. **Flush** `0x0052FD90(force, 0)`: unless `force`, runs only when ≥ 40
   ms (`GetTickCount`) passed since the last flush. For every live game
   (slot order), for every client in the game's client list (game +0x88,
   next at client +0x4A8): `0x0052E320(game, 1)` with EAX = client. Then
   host bookkeeping (empty-game timeout: no client for 300 s, or frame in
   1501..7499 with no client → the game is closed by `0x0052E770`).
4. `0x0052E320`: pops buffers from the head (`0x005392A0`) and sends each
   whole with `0x0052B330(1, client id (client +0), data, size)`; a sent
   buffer goes back to the free list. A failed send counts client +0x1C4
   and, after 3, drops the client (`0x0052CAF0`). Order = queue order.
5. If game +0x6A (game type) is 1 or 2: before sending, `0x0052E110`
   sends message 0xB3 (`len` + 7 bytes), and at most 3 buffers are sent
   per flush; the rest wait. Single-player game type: open question 3.
6. **Net send** `0x0052B330(type, client id, data, size)`: asserts size ≤
   0x204. Local mode: `0x0052AEB0(data, size)` (§3.3); `type` is ignored.
   Network mode (out of scope): compression and a 1- or 2-byte length
   prefix.

#### 3.3 Local delivery

1. `0x0052AEB0` splits the buffer into messages with the size rule
   (§3.1): for each message, a 0x210-byte node (data 0x204, size at
   +0x204, `GetTickCount` at +0x208, next at +0x20C) is appended to one of
   two client lists: id < 0xAF → game list `0x00882CE4`; 0xAF..0xB4 →
   system list `0x00882CDC`; id ≥ 0xB5 → fatal assert.
2. Size > 0x204 or ≤ 0 → fatal assert.
3. Size rule result 0 (unknown id, incomplete) **ends the split**: the
   rest of the buffer is discarded.
4. Client pop `0x0052B820`: with global `0x00882D10` = 2 (game type 1) a
   node is held until 500 ms after it was queued; with 1 (single player)
   it is returned at once.
5. **Direct sends** bypass the buffers and reach the lists immediately,
   ahead of anything still buffered: 0xAF (`0x0052B720`, `0x0052B780`),
   0xB0 (`0x0053B220`), 0xB2 (`0x0053B1B0`), 0xB4 (`0x0053B260`), 0x06
   (`0x0053B240`, also queued normally via `0x0053B320`), 0xB3
   (`0x0052E110`), 0xFA/0xFB (queue-2 replies, out of scope).

#### 3.4 Client receive `0x0044C6E0`

1. Drains the system list first (each message → `0x0045C850`, then
   `Sleep(0)`), then the game list (→ `0x0045F7B0`, `Sleep(0)`), each until
   empty (−1); both handlers ignore the final −1 call.
2. `0x0045C850` handles 0xAF–0xB4 (jump table `0x0045C894`): 0xAF sets
   `0x007A0618` = 1, 0xB0 sets it to 0, 0xB1/0xB2 nothing, 0xB3 →
   `0x0045C620`, 0xB4 → `0x0045C6D0`.
3. `0x0045F7B0` walks the node's bytes message by message (size rule;
   result 0 ends the walk): id ≥ 0xAF → fatal; expected size mismatch →
   fatal; unit handler: looks up the unit (ids 0x67–0x6D: monster, id u32
   at +1; others: type u8 at +1, id u32 at +2, `0x00463990`) and calls it
   through `0x0045F730`; per-id pre-steps for 0x0D (type byte 1), 0x18,
   0x95, 0x96; then the general handler. What each handler does belongs to
   the client specs.

### 4. d2rs mapping and scope

1. **Intents** = the C→S messages with `scope` = sim (80 ids). `d2-proto`
   carries them with the 1.14d ids and layouts (TSV); the local transport
   keeps byte identity so traces replay unchanged. The server applies
   §2.2–2.4 (gate, exact size, parse validation, result code) before the
   sim handler runs; per-intent behaviour is owned by the system specs.
2. **Events** = the S→C messages with `produced_by` = sim. The sim
   produces them in the original's order; `d2-server` packs them into
   per-client buffers (§3.2) and flushes after the tick.
3. **Session** rows (C→S 0x67, 0x69–0x6C, 0x6E, 0x70; S→C 0x00, 0x02,
   0x04, 0x05, 0x06, 0x0B, 0x5B, 0x5C): game creation, loading, joining,
   leaving; produced by `d2-server`'s session code, not by game logic.
   Their bytes are still part of the comparison (§6).
4. **Out of scope** (Phases 7–9 or never):
   - multiplayer only: C→S 0x5D, 0x5E (party/hostility), 0x68 (join a
     hosted game); S→C 0x75, 0x77–0x79, 0x7F, 0x8B–0x8D, 0x90 (party,
     trade, relations): need a second player (Phase 7 multiplayer);
   - Battle.net / realm / anti-cheat: C→S 0x66 (warden response, handler
     does nothing), 0x6D (ping), queue 2 (0xFF); S→C 0x8F (pong), 0xAE
     (warden request), 0xAF–0xB4 (connection, game list, save download,
     refusal): transport and account layers, not game behaviour;
   - network mode of the transport (compression, length prefix, ban
     lists, `0x006C08A0`): d2rs uses its own transport (`d2-net`).
5. **None** rows: ids that are never queued or never received (size 0,
   no handler, stubs); d2rs rejects them like 1.14d (stubs return their
   code and do nothing).

### 5. Machine-readable tables

`client-messages.tsv` (one row per id 0x00–0x70) and
`server-messages.tsv` (one row per id 0x00–0xB4), tab-separated, header
row, `0x`-prefixed hex ids. Checked by
`tools/trace-recorder/check_packets.py` (reads both files, §6).

Size-rule grammar (`transport_size`, `size`): a decimal number = fixed
size (0 = never valid); `<u8|u16>@<off>[*<mul>][+<add>][;cap=<n>][;min=<n>]`
= read the field (little-endian), replace it by 0 when it exceeds `cap`,
multiply, add; the message needs at least `min` bytes and the field's
bytes; `chat` (C→S 0x14/0x15), `chat26` (S→C 0x26) and `af` (S→C 0xAF)
as in §2.1 rule 5 and §3.1 rule 1.

| client-messages.tsv | Meaning |
|---|---|
| `id`, `name` | id; D2MOO 1.10f handler name (shortened) or `UnusedNN` / `SysNN` |
| `transport_size` | size rule of `0x00730DC0` (§2.1 rule 5) |
| `handler_size` | the handler's own check: `==N`, `4..275`, `chat`, `any`, `-` (stub or none) |
| `layout` | fields a 1.14d handler reads (§2.4 rule 10) |
| `handler` | 1.14d handler address (`0x0053F100` for system ids) |
| `kind` | `handler`, `stub0`, `stub3`, `system`, `none` |
| `gate` | `alive`, `dead`, `none`, `system`, `-` (§2.3 rule 3) |
| `request` | what it asks for, in a few words |
| `scope` | `sim`, `session`, `out`, `none` (§4) |
| `confirmed` | `yes`: size, gate, handler and layout read in 1.14d; `partial`: size and handler confirmed, field meanings from D2MOO |

| server-messages.tsv | Meaning |
|---|---|
| `id`, `name` | id; community label (not a 1.14d fact) |
| `size` | size rule of `0x00730AE8` (§3.1) |
| `layout` | fields where a 1.14d builder's stores were read; empty = unconfirmed; `bits:` prefix = bit-packed message, fields `name:width` written LSB-first from bit 0 of byte 0 |
| `sender` | 1.14d builder(s) that write the id into a message passed to `0x0053B280` or `0x0052B330`; `-` = not found statically |
| `client_handler`, `client_unit_handler` | `0x007114D0` columns 1 and 3; system ids: `0x0045C850` targets |
| `produced_by` | `sim`, `session`, `transport`, `out`, `none` (§4) |
| `confirmed` | `yes`: size, handler and sender read in 1.14d; `partial`: sender not found |

### 6. Exact-match comparison

1. **Unit of comparison: one frame F.** Input I(F) = the ordered list of
   game messages (bytes) dispatched between the end of tick F−1 and the
   start of tick F, plus their system messages. Output O(F) = per client,
   the ordered list of messages (bytes) queued with `0x0053B280` from
   the end of tick F−1 to the end of tick F (handler replies to I(F),
   then tick F's events), plus direct sends (§3.3 rule 5) in their
   position. d2rs, from the same state and I(F), must produce O(F)
   byte-identical and in the same order.
2. Flushed buffers are derived from O(F) by §3.2 rules 1–2 and are
   compared too (packing is part of the protocol).
3. Nothing is ignored by default. Values that come from the clock
   (ping/pong contents, 0x8F) are transport and excluded with their rows
   (§4). GUIDs are deterministic (`unit-order.md`) and compared.
4. **Proof, two layers:**
   - Transport rules (this spec): `record_packets.py` records every
     message at the server's queue read, dispatch, result, queueing and
     flush, with tick markers and the frame number (game +0xA8 read at
     the tick's entry, + 1). `check_packets.py` checks R1–R7 (its
     docstring): sizes against both TSVs, client-to-server byte and
     order identity, dispatch order and fixed result codes, no message
     inside a tick, frames +1, buffer packing equals flushed bytes,
     flush only after a tick. `--perturb N` flips one byte of event N
     and must report seq N (M08); `--selftest` does this for every event
     of a synthetic trace built from the TSVs.
   - Simulation output (later, `conformance`): a format-1 trace per
     recording, area `sim`, behavior `intents-events`, `inputs` = I(F)
     as kind `intent` (`tick` = F, data `{client, bytes}`), `expected` =
     O(F) as kind `event` (data `{client, bytes}`), mode `exact`. Written
     by a converter once `d2-sim` can replay a game (not yet built).
5. **Trace to record** (`packets-0001`): single player, an existing
   character, Normal, Act 1: walk, run, cast a left and a right skill,
   pick up and drop an item, equip and unequip, use a belt potion, talk
   to an NPC, buy and sell one item, spend a stat point if available,
   take a waypoint if available, exit. Command and expected output:
   `docs/HANDOFF.md` §5.
6. **Keyed masks** (the exception to rule 3 for bytes the original never
   writes; which builders leave which bytes: `tools/original-hooks.md`
   §6.2; d2rs writes 0 there). A record carries no sender, so a mask
   whose bytes are written in another form of the same id needs a key
   read from bytes every form writes, or a content-dependent offset. The
   three ids an id / offset / length row cannot express:

   | Id | Key (bytes every form writes) | Masked bytes | Why the key is exact |
   |---|---|---|---|
   | 0x27 | count u8@6 = 1 | 7, 9, 12–39 | below |
   | 0x50 | u16@1 = 4 | 13–14 | `0x00593CB0` writes u16 4 (`0x00593CF6`) |
   | 0x50 | u16@1 = 2, 0x24, 13 | 5–14 | `0x00579180` (`0x005792C0`), `0x0058E120` (`0x0058E140`), `0x0059D6A0` (`0x0059D6D3`) |
   | 0x50 | u16@1 = 0x17 | 3–14 | `0x005B4A80` (`0x005B4B37`) |
   | 0x50 | u16@1 = 1 | none | the status reply `0x00546040` zeroes its buffer and writes u16 1 (`0x00546248`) |
   | 0x82 | none | from the byte after the first 0 in bytes 5–20, through byte 20 | below |

   - **0x27** (40 bytes, all senders copy through `0x0053C8D0`; its four
     call sites `0x005456E4`, `0x00545811`, `0x00572D55`, `0x005DE397`):
     unit type u8@1, GUID u32@2, entry count u8@6 (low byte of the text
     list's u16 count), entry k < 8: kind u8@8+4k, string id
     u16@10+4k. The one-entry senders `0x005456A0` (u8@1 = 2, count 1,
     kind 0) and `0x005DE330` (u8@1 = 1, count 1, kind 3) leave 7, 9 and
     12–39 unwritten. The list senders `0x00545780` and `0x00572C10`
     zero bytes 6–39 (`0x00661480`), then write the count and the
     entries; 7 and 9 + 4k are never written (stay 0), and more than 8
     entries is a fatal assert (`0x00661509`). So with count 1 every
     masked byte of a list form is 0 on both sides; count ≠ 1 is never
     a one-entry form. Cost: a d2rs list form with one entry that wrote a
     non-zero padding byte would pass.
   - **0x50**: the six call sites of the copier `0x0053D7E0` (15 bytes)
     are the five rows above; each writes a constant u16@1.
   - **0x82** (29 bytes, `0x0053DB90`, call site `0x005720B1`): owner
     GUID u32@1, byte 5 := 0, then the owner's name is copied to
     bytes 5–20 by `0x004135D0` (at most 15 characters + NUL; it writes
     exactly through the NUL, also in its dword path, never past it);
     u32@21 and u32@25 are written. A 15-character name leaves nothing
     unwritten.
   - Proposed columns for the mask table (the format is
     `tools/scenario.md` §6's): `id`, `key` (`-`, or `u8@<off>=<v>` /
     `u16@<off>=<v>`, little-endian, v hex), `offset` (decimal, or
     `nul@<n>` = the byte after the first 0 byte at or after n; no 0 in
     n…last → nothing masked), `length` (decimal, `*` = to the end, or
     `..<n>` = through byte n), `source`. Rows: `0x27 u8@6=0x01` 7 1,
     9 1, 12 28; `0x50 u16@1=0x0004` 13 2; `0x50 u16@1=0x0002`,
     `=0x0024`, `=0x000D` 5 10; `0x50 u16@1=0x0017` 3 12; `0x82 -`
     `nul@5` `..20`. The present unkeyed `0x50 13 2` row becomes the
     `u16@1=0x0004` row (unkeyed, it hides written bytes of the u16 1,
     2, 0x24 and 13 forms).

### 7. Unit update messages (`0x0053A500`) and room clean-up (`0x00553220`)

The owner of "the unit-update spec" named by `sim/pathing.md` §10 and
OQ7, `missiles/missiles.md` R2.4 and `items/inventory.md` §6.3: which
S→C messages a changed unit produces at the end of a tick, and what the
clean-up resets so that they are sent once.

#### 7.1 Walk (`0x0053A620` → `0x0053A5D0`)

1. Called from the per-client update (`sim/tick.md` §6 rule 5) with
   (game, client, the client player's room). The client's act (client
   +0x1AC) must have its pending-update flag (act +0x00) ≠ 0, else
   nothing. For each room of the room's adjacent-room array
   (`0x00619790`, array order; `sim/unit-order.md` §9) walk the room's
   update queue from the head (room +0x1C, link unit +0xE0; next read
   before the unit runs) and run rule 2 per unit. Queue order:
   `sim/unit-order.md` §6 (most recently queued first).
2. **Per unit** `0x0053A500(unit, client, game)`; P := the client's
   player (`0x00537860(client, 0)`); announced := 0.
   1. Unit flag (+0xC4) 0x10 ("not yet announced", `sim/units.md`) set
      and unit ≠ P: a missile stops here (nothing at all is sent for
      it); any other type gets its add messages (§7.2) and announced :=
      1.
   2. By type: player → `0x00580860(game, unit, client, announced)`
      (§7.3 rule 1); monster → `0x00598220(game, unit, client,
      announced)` (§7.3 rule 2); object → `0x00581AD0(game, unit,
      client)` (§7.3 rule 3); missile → nothing; item →
      `0x0055BF30(unit, client)` (§7.3 rule 4); type ≥ 5 → nothing.

#### 7.2 Add messages (`0x00571F90(game, unit, client)`)

Also called by the room switch (`sim/path-placement.md` §11) and the
room-change merge (`sim/pathing.md` §9.8). Part A by type:

| Type | Message | Fields / condition |
|---|---|---|
| player | 0x59 (`0x0053E8F0`) | GUID, class u8 (+0x04), name (player data), x, y (`0x00620870` position) |
| monster | 0xAC (`0x0053E2E0`, `monsters/init.md` §24) | skipped when `0x005541B0(unit)` and `0x0063A320(unit)` both hold; then stat 328 := (x + y) & 0xFFFF; class 528 → 0x98 (`0x0053E0A0`); for each `monstats` skill slot i = 0..7 whose bit i of row +0x16C is set and skill id (row +0x170 + 2i) is valid and the unit has that skill: 0x21 (`0x0053C4A0`); then `0x00570E30`, `0x00571CD0` (§7.3 rule 2 steps 3 and 8) |
| object | 0x51 (`0x0053BD10`, type 2: GUID, class u16, x, y, mode u8 +0x10, interact u8) | then `objects` row +0x167 bit 2 → 0x60 (`0x0053D900`); class 59 → 0x82 (`0x0053DB90`) |
| missile | 0x73 (`0x0059FEE0`, `missiles/missiles.md` R2.4) | with `0x006486C0(path)` |
| item | only with unit flag 0x10: mode 3 and unit flag 0x1000 → 0x9C action 2 (`0x0053EC90`), else 0x9C action 0 (`0x0053EC00`) (`items/inventory.md` §6.3) | |
| other (5) | 0x09 (`0x0053BCD0`: type, GUID, class u8, x, y) | |

Part B by type: player → `0x005489F0`, then a corpse 0x74
(`0x0053DA40`) when `0x005541B0(unit)` and `0x00639DF0(unit, 7)` hold,
else `0x00534F80`;
`0x00570E30`, `0x005484B0`, `0x00571CD0`, `0x00571620`; monster → its
mode message (§7.4), then `0x00534F80` when `0x00639DF0(unit, 93)` holds or the
class is 291, 417 or 418; item → `0x0055BED0` (§7.3 rule 4); others nothing.

#### 7.3 Type updates

1. **Player** `0x00580860`: `sim/pathing.md` §10 rules 2–3 (0x15, 0x0F,
   0x10) and `items/inventory.md` §6.1 rule 2 (item dispatcher, 0x47,
   0x48).
2. **Monster** `0x00598220(game, unit, client, announced)`, in order:
   1. Flag-ex (+0xC8) bit 0x10000: S→C 0x15 (`0x0053BC10`: type, GUID,
      x, y, flag 1; the dynamic path's cell, a static path's +0x0C /
      +0x10), then the room-change messages `0x00554670(game, unit, 0)`
      (`sim/pathing.md` §9.8).
   2. Unit flag 0x1 (mode changed): the mode message (§7.4); then unit
      flag 0x80000 := 0.
   3. `0x00571CD0(unit, client)`: class and hireling messages (senders
      of 0x9E–0xA2, 0xA3, 0xA4, 0xA5, 0xAB, 0x23; conditions: open
      question 10).
   4. Unit flag 0x100: `0x00571620` (0x76 and `0x0053C750`).
   5. Flag-ex bit 0x1 (inventory changed): if P exists and
      `0x00572EE0(unit, P)` → `0x00537680` (item dispatcher of
      `items/inventory.md` §6.1 rule 3 for P); else a hireling class
      (`0x0063EE90`) whose owner (`0x0058F0D0`) is P →
      `0x00597890(game, unit, client, 0)`.
   6. Unit flag 0x400: `0x00571740` (an 8-byte message, `0x0053D780`,
      to P's own client only).
   7. Unit flag 0x8000 (hit): S→C 0x0C (`0x00597CF0` → `0x0053B430`).
   8. announced = 0 and `0x00639F20(unit)`: `0x00571580(unit, client,
      0)` (`0x00570E30`, `0x005711D0`: stat messages).
   9. `0x00625A20(unit)`: `0x005715A0` (0x11 `0x0053D850` under state
      conditions).
   10. Unit flag 0x800: `0x00597C70` (0x57 `0x0053D880`).
3. **Object** `0x00581AD0`: unit flag 0x1 → `0x00581A20` (0x0E
   `0x0053B470`; portal rows → 0x60; a mode-1 case → 0x4D with type 2);
   flag 0x400 → `0x00571740`; flag 0x100 → `0x00571620`; flag-ex 0x1 →
   `0x00597890(game, unit, client, 0)`; always `0x00571CD0`.
4. **Item** `0x0055BF30`: only with unit flag 0x1 → `0x0055BED0`: an
   item in mode 3 without unit flag 0x10 → 0x9C action 2 when unit flag
   0x1000, else action 3 (`items/inventory.md` §6.3 part 2).

#### 7.4 Monster mode message (`0x00597E20(game, unit, client)`)

1. Mode row E := table `0x006E1D90` (24 bytes per mode, 16 modes):
   {use target, target from path, moving, attack, code to point, code
   to unit}:

   | Mode | E | Mode | E |
   |---|---|---|---|
   | 0 DT | 0, 1, 0, 0, 8, 8 | 8 S1 | 1, 1, 0, 1, 12, 13 |
   | 1 NU | 0, 0, 0, 0, 7, 7 | 9 S2 | 1, 1, 0, 1, 14, 15 |
   | 2 WL | 1, 1, 1, 0, 1, 0 | 10 S3 | 1, 1, 0, 1, 26, 27 |
   | 3 GH | 0, 0, 0, 0, 6, 6 | 11 S4 | 1, 1, 0, 1, 28, 29 |
   | 4 A1 | 1, 1, 0, 1, 11, 10 | 12 DD | 0, 0, 0, 0, 9, 9 |
   | 5 A2 | 1, 1, 0, 1, 17, 16 | 13 KB | 0, 1, 1, 0, 20, 20 |
   | 6 BL | 1, 1, 0, 0, 18, 18 | 14 SQ | 1, 1, 0, 1, 12, 13 |
   | 7 SC | 1, 1, 0, 1, 4, 5 | 15 RN | 1, 1, 1, 0, 23, 24 |

2. Target T := the unit's target (`0x00553540`); T is dropped when E's
   "use target" is 0 or T's room is not in the client's room list
   (`0x005387F0`: T's room client array, room +0x48 / count +0x78).
3. Mode 14, or a skill in use (`0x00620250`: skill list +0xA8 current
   skill ≠ none): the skill message instead (`0x00597D70`): T kept →
   0x4C (`0x0053D530`: type 1, unit GUID, skill id, level, T type, T
   GUID); else 0x4D (`0x0053D4D0`: path target x, y). Both builders
   send 0x99 / 0x9A instead when their last argument is non-zero; this
   caller passes 0. No skill → nothing. Stop.
4. The unit must have a path (fatal 0xE6). With T: id 0x68, code := E
   code to unit, (a, b) := (T type, T GUID); modes 2 and 15 with path
   type 5 or 6 (`0x00648E30`) instead id 0x67, code to point, (a, b) :=
   the path target. Without T: id 0x67, code to point, (a, b) := the
   path target (`0x00648A40`, `0x00648A60`) when "target from path",
   else the unit's cell (`0x006488C0`, `0x00648900`).
5. Per-mode bytes d (direction-like) and e: mode 0 and 12: d := path
   direction (`0x006487F0`), e := unit +0xB0 for mode 0 and 0 for 12;
   mode 3: d := `0x005A5650(unit)`, minus 1 when above 1, | 0x80 when
   `0x005A0180(unit, 0x100)`, e := +0xB0; mode 6 without T: (a, b) :=
   (0, −1); mode 1: 0x6D (`0x0053BB70`: GUID, unit cell x, y
   (`0x0045ADF0`, `0x0045AE20`), d := `0x005A5650(unit)`) and stat 328
   += 1, stop; the moving modes (2, 15): s := `0x006490A0(path)` (path
   stop distance +0x93, plus 1); mode 13: no s, instead d := path +0x90
   (`0x00648E60`), e := `0x005A5650(unit)`, f := unit +0xB0 (§7.7).
   `0x005A5650(unit)` = the life fraction in 128ths: L = stat 6 >> 8, M
   = max life (`0x00625D10`) >> 8; M > 0 and L < M → L · 128 / M
   (truncated), else 0x80; no unit → 0x80.
6. Send by E: moving → 0x68 (`0x0053B5F0`) or 0x67 (`0x0053B710`)
   with (code, a, b, s); mode 13 → 0x68 (`0x0053B7F0`) or 0x67
   (`0x0053B910`); attack → 0x6C (`0x0053BAA0`: code, a, b, d, unit
   cell) or 0x6B (`0x0053BB00`: code, a, b, d, e, unit cell); else → 0x6A (`0x0053B9F0`: code,
   a, b, d) or 0x69 (`0x0053BA40`: GUID u32@1, code u8@5, a u16@6, b
   u16@8, d u8@10, e u8@11).
7. **Death**: the kill sets mode 0 (flag 0x1), so the next client pass
   sends 0x69 code 8 with (a, b) = the path target (mode 0 has "target
   from path"; (0, 0) when the path never had a target, recorded `69
   1b000000 08 0000 0000 38 06` at frame 2882) and d, e (mode 0 has no
   target); when mode 12 is set (§7.7 rule 3) 0x69 code 9 at the unit's
   cell with e = 0 follows. Recorded: `69 13000000 08 9512 5515 38 06`
   (frame 2724) and `69 13000000 09 9412 5515 38 00` (frame 2748) in
   `20261006-015956`; 0x69 code 6 (mode 3, get-hit) 15 times.

#### 7.5 Room clean-up (`0x00553220(game, unit)`)

Run by tick step 6 (`sim/tick.md` §3) on every unit of every active
room's update queue after all clients' updates; it sends nothing. In
order:

1. The stat list's changed-stat buffer is freed and its counts zeroed
   when the list has flag 0x80000000 (`0x00625960`: list +0x50, +0x54,
   +0x56).
2. The unit's pending event records are freed (`0x00571F40`: list unit
   +0xEC, +0xF0 := 0).
3. Unit flags 0x1 and 0x10 := 0; path flag 0x2 (room changed) := 0
   (`0x00620FA0(unit, 0)`); unit flags 0x400, 0x8000 := 0; flag-ex
   0x800, 0x1000, 0x10000, 0x200000 := 0.
4. The unit's state-changed bits are zeroed (`0x00639EE0`).
5. The update-list reset `0x00597B00` (`items/inventory.md` §6.1 rule
   4).
6. Twice: if `0x00625A20(unit)`, `0x00627410(unit)`.
7. By type: player: stat 29 `lastexp` := −1 (`0x00627260(unit, 29, −1,
   0)`), unit flag 0x100 := 0, the client record's +0x34 → +4 := 0
   (`0x0053FA90`); monster: flag 0x100 := 0; flag 0x800 set → cleared
   and monster data +0x5C bit 0x1 := 0 (`0x00573570`); flag-ex 0x10000
   := 0; object: flag 0x100 := 0; missile: nothing; item: unit flag
   0x1000 := 0 and item flags 0x20, 0x2000 := 0 (`0x006280D0`).
8. So each message of §7.1–§7.4 is sent once per change; a new unit is
   announced in the first client pass after its creation and never
   again by §7.1 rule 2.1.

#### 7.6 Missiles, deaths and drops in one tick (e2e order)

For a single-player tick in which a player's missile kills a monster
that drops gold:

1. The missile itself: no message from §7 (rule 7.1.2.1); the client
   creates its own missiles from skill messages; 0x73 only through the
   add messages of a room switch (§7.2).
2. The monster: 0x69 code 8 (§7.3 rule 2 step 2), then 0x0C when the
   hit flag 0x8000 is set (step 7).
3. The gold: a new item (flag 0x10) dropped by the death (`0x00558AA0`
   sets 0x1000) → 0x9C action 2 in the same pass (§7.1 rule 2.1).
4. Monster and item are in the same room's update queue: whichever was
   queued last goes first (`sim/unit-order.md` §6 rule 5); the item is
   queued at its creation after the kill set the monster's mode, so the
   item's 0x9C precedes the monster's 0x69 in that room. Open question
   11 asks for a recording to confirm.

#### 7.7 Monster messages 0x67–0x6D (senders, triggers, layouts)

1. **Only sender**: §7.4. Its two callers: the monster update (§7.3
   rule 2 step 2, unit flag 0x1) and the monster part B of the add
   messages (§7.2), i.e. a monster announced by §7.1 rule 2.1 or met by
   a room switch (§7.8). Every builder below has `0x00597E20` as its
   only caller. All run in tick step 5 (`sim/tick.md` §6, per client in
   client-list order, rooms and units in §7.1 order); the room switch at
   a join runs in the C→S 0x6B handling (§8.2) but finds no monster
   there.
2. **Trigger** = unit flag 0x1, set by every mode set (`0x00553570`,
   `sim/units.md` §4.1; a monster staying in mode 1 does not get it) and
   cleared by §7.5 step 3. So one message per mode set and per client,
   in the client pass of the tick of the set (mode sets run in tick step
   4, timer events, or in the message drain before the tick). A monster
   re-set to the same moving mode (a new walk target) sends again.
3. **Death pair** (§7.4 rule 7): code 8 in the tick of the kill
   (`0x0057CCB0`, `combat/damage.md` §7.2: mode 0 facing the attacker);
   code 9 in the tick mode 12 is set, by mode 0's event-0 function
   `0x005A7350` or event-1 function `0x005A72B0` (`sim/units.md` §4.6):
   - event 0 (an action-frame event of the DT animation, `sim/units.md`
     §4.2): a monster whose `monstats` base id (row +0x02) is 78
     (`0x0063E8D0(unit, 0)`) takes a path step (`0x00554CA0`), refreshes
     its animation (`0x00623E00`) and sets mode 12 only once its
     animation is complete (`0x006217C0`: no sequence → current frame
     +0x44 + bonus +0x4C ≥ frame count +0x48; with a sequence → +0x48 ≤
     0); every other monster sets mode 12 at once;
   - event 1 (end of the DT animation): mode 12, then `0x005C0C30(game,
     13, unit, 0, 0)` and the `monstats` row +0x1A4 death action (1 →
     `0x00574370(game, unit, row +0x26, 1)` then `0x00573780`; 2 → kill
     of `0x00552FD0(unit)` with `0x0057CCB0`; other values nothing).
   Recorded: code 8 at frames 2724 / 2882, code 9 at 2748 / 2906 (24
   frames) in `-015956`. Of the 20 code-8 messages in the two
   recordings the next message of the frame is 0x65 14 times (open
   question 11), 0x9C twice, 0x96 and 0x1D once each, none twice; never
   0x0C.
4. Field sources: GUID = unit +0x0C; code, a, b, d, e, s from §7.4 rules
   4–5; x, y = the unit's cell (dynamic path `0x006488C0` /
   `0x00648900`; types 2, 4, 5 the static path +0x0C / +0x10; no path
   0); velocity = stat 67 `velocitypercent` (`0x00625480(unit, 67, 0)`)
   clamped to −32768..32767 and stored as a 16-bit two's complement;
   maxd = path +0x91 (`0x00648EA0`); t = path type +0x3C
   (`0x00648E30`), rewritten as in the table.
5. **Layouts** (little-endian; bytes not listed are 0; the id byte is
   the builder's first argument except for 0x6D):

   | Id | Builder (mode) | Size | Fields |
   |---|---|---|---|
   | 0x67 | `0x0053B710` (2, 15) | 16 | GUID u32@1, code u8@5, x u16@6, y u16@8, s u8@10, t' u8@12, velocity u16@13, maxd u8@15; t' = t, but t 5 or 6 → t' 1 and (x, y) := the path target (`0x00648A40`, `0x00648A60`), t 8 → t' 11 |
   | 0x67 | `0x0053B910` (13) | 16 | GUID u32@1, code u8@5, x u16@6, y u16@8, d u8@10, f u8@11, t' u8@12 (as above), velocity u16@13, e u8@15 |
   | 0x68 | `0x0053B5F0` (2, 15) | 21 | GUID u32@1, code u8@5, unit x u16@6, y u16@8, a (target type) u8@10, b (target GUID) u32@11, s u8@15, t' u8@17 (t 5 or 6 → 2, t 8 → 11), velocity u16@18, maxd u8@20 |
   | 0x68 | `0x0053B7F0` (13) | 21 | GUID u32@1, code u8@5, unit x u16@6, y u16@8, a u8@10, b u32@11, d u8@15, f u8@16, t' u8@17 (t 8 → 11 only), velocity u16@18, e u8@20 |
   | 0x69 | `0x0053BA40` | 12 | GUID u32@1, code u8@5, a u16@6, b u16@8, d u8@10, e u8@11 |
   | 0x6A | `0x0053B9F0` | 12 | GUID u32@1, code u8@5, a (target type) u8@6, b (target GUID) u32@7, d u8@11 |
   | 0x6B | `0x0053BB00` | 16 | GUID u32@1, code u8@5, a u16@6, b u16@8, d u8@10, e u8@11, unit x u16@12, y u16@14 |
   | 0x6C | `0x0053BAA0` | 16 | GUID u32@1, code u8@5, a (target type) u8@6, b (target GUID) u32@7, d u8@11, unit x u16@12, y u16@14 |
   | 0x6D | `0x0053BB70` | 10 | GUID u32@1, x u16@5, y u16@7, d u8@9 (the life fraction, §7.4 rule 5) |

   0x67 / 0x68 (`0x0053B710`, `0x0053B910`, `0x0053B7F0`) look the
   monster up again by GUID through the client's game (`0x005387C0`,
   `0x00552F60(game, 1, GUID)`) and send nothing when it is gone;
   `0x0053B5F0` does not check (a missing unit would crash; never seen).
   For mode 13, f = unit +0xB0 and d, e as §7.4 rule 5.
6. Ids by mode (with no skill in use, §7.4 rule 3): 0x67 / 0x68 for
   modes 2, 13, 15; 0x6B / 0x6C for the attack modes 4, 5, 7, 8, 9, 10,
   11; 0x6D for mode 1; 0x69 / 0x6A for modes 0, 3, 6, 12. The "to unit"
   id (0x68, 0x6C, 0x6A) needs a kept target T (§7.4 rule 2). Counts in
   the two recordings (two games each): 0x67 207, 0x68 81, 0x69 61, 0x6A
   0, 0x6B 8, 0x6C 28; every 0x67–0x6D carries the builder return
   address of its table row (Test vectors).

#### 7.8 Room switch (`0x00537B50(client, new room)`, ESI = client)

The owner of the messages a client gets when its room changes
(`sim/tick.md` §6 rule 5 from the per-client update; `0x005381F0` from
game entry, `sim/path-placement.md` §11; the third caller `0x0053A400`
is the act change). Nothing when the client has no game (client +0x1A8)
or the new room equals the client's room (client +0x1B4, the old room).

1. New array N := the new room's adjacency array, old array O := the old
   room's (`0x00619790`; empty for a null room). First `0x0061A110`,
   `0x0061A9F0` (old, new) (room bookkeeping, `drlg/rooms.md`).
2. **Joins**, for each room R of N (array order) not in O: `0x0053A8E0`:
   1. **S→C 0x07** (`0x0053BC50`: R's tile x u16@1, tile y u16@3, level
      id u8@5).
   2. The client is added to R's client list (`0x0061A660`).
   3. If R now has at most one client (room +0x78 ≤ 1): every monster
      in R (unit list room +0x74, next unit +0xE8) gets `0x00573780`
      (schedules its AI, event 2 at f + 2, `sim/units.md` §6.2).
   4. The add messages (§7.2) of every unit in R's unit list (list
      order) except the client's player.
3. **Leaves**, for each room L of O (array order) not in N: `0x0053A9B0`:
   1. For every unit in L (list order): **S→C 0x0A** (`0x00571600` →
      `0x0053BDA0`: type u8@1, GUID u32@2; a missile, type 3, gets
      nothing).
   2. The client is removed from L's client list (`0x0061A700`); if L
      has no client left: every monster in L gets `0x005738D0`.
   3. **S→C 0x08** (`0x0053BC90`, the only sender of 0x08, same layout
      as 0x07: L's tile x u16@1, tile y u16@3, level id u8@5).
   4. Then, if L is the client's room (the old room): the player update
      `0x00580860(game, P, client, 0)` (§7.3 rule 1), P = client +0x174
      when client +0x3D4 bit 0, else looked up by client +0x16C / +0x170
      (`0x00552F60`; not found → client +0x174 := 0, nothing).
4. Client +0x1B4 := the new room. If it is non-null and client +0x174 ≠
   0: the record `0x006221A0(P)` gets `0x006221E0(P, 0x00622230(P) |
   0x0061AE30(level id of the new room))` (no message; the value 0x5F
   carries, §8.2).
5. So a switch sends every 0x07 (new array order) before any 0x0A /
   0x08 (old array order), each 0x07 followed by that room's add
   messages. At game entry O is empty: one 0x07 per room of the spawn
   room's array, the spawn room included, so the spawn room's 0x07 is
   sent twice (once by game entry, once here). Recorded: `-022633` seq
   144–153, `07 a003 8803 03` at seq 144 and 150 (R2: 1 + 9); 0x08 in
   48 tick frames of the two recordings, e.g. `08 c803 6004 01` (frame
   149, `-015956`), every one after all 0x07 of its frame (mostly 3 +
   3 for a walking switch, 9 + 9 for a waypoint warp).

#### 7.9 Add-message parts (`0x00570E30`, `0x00571CD0`, `0x00571620`, `0x00534F80`)

1. **S→C 0xAA, unit states** `0x00570E30(unit, client)` (monster part
   A, player part B): byte 0 0xAA, unit type u8@1, GUID u32@2, size u8@6
   = 7 + the bit stream's byte length, bit stream from byte 7 (the bit
   writer of `client/model.md` §10, mirrored: LSB-first, unused high
   bits of the last byte 0). The stream, for each state s the unit has
   (state bit array `0x0063A100`, words of 32 bits, ascending s):
   1. Skipped when s ≥ the `states` count or the row's `nosend` bit
      (row +0x10 bit 0) is set.
   2. Before writing s: when the stream's byte length + 7 > 218, no
      further state is written.
   3. s (8 bits); then the state's stat list (`0x006256B0(unit, s)`)
      with up to 16 entries {param u16, id u16, value i32} in list order
      (`0x00625C90`): none or no entry → bit 0; else bit 1, then per
      entry whose `itemstatcost` row exists with `send bits` n (+0x08) ≠
      0: id (9 bits), the param in `send param bits` (+0x09) bits when
      that is ≠ 0, the value in n bits, clamped first when n < 32 —
      signed rows (+0x04 bit 1, `signed`) to −2^(n−1)..2^(n−1)−1,
      others to 0..2^n−1 (values < 1 → 0); then 0x1FF (9 bits).
   4. After the states: 0xFF (8 bits).
   Recorded `aa 00 01000000 0c 69 59 f9 ff 1f` (`-022633` seq 103, the
   player): state 105 `alignment`, bit 1, stat 172 `alignment` (send
   bits 2) value 2, 0x1FF, 0xFF: 37 bits = 5 bytes, size 12. The writer
   has 0xF4 bytes; a stream past that sets the writer's overflow flag
   and drops the rest (not seen).
2. **Pending event records** `0x00571CD0(unit, client)`: for each record
   of the unit's list (unit +0xEC, next at record +0x00; freed by §7.5
   step 2) by its id byte (+0x04): 0x23 → `0x0053C590`; 0x99 →
   `0x0053D530` and 0x9A → `0x0053D4D0`, with the last argument 1 (so
   they send 0x99 / 0x9A, §7.4 rule 3); 0x9E → `0x0053BEE0`; 0xA1 →
   `0x0053BFD0`; 0xA3 → `0x0053C0E0`; 0xA4 → `0x0053E1A0`; 0xA5 →
   `0x0053C190`; 0xAB → `0x0053C150` only when `0x00451F30(unit)` is 0,
   the client has a player and `0x00554200(unit)` holds; other ids
   nothing. The record fields are passed through. An empty list (every
   unit in both recordings' joins) sends nothing.
3. **Overhead text** `0x00571620(unit, client)`: unit +0xA4 = 0 →
   **S→C 0x76** (`0x0053B3D0`: type u8@1, GUID u32@2; recorded `76 00
   01000000` at the join). Else, unless the unit is a player and the
   client's player relates to it (`0x0055B300(P, unit, 4)` or
   `0x0055B300(unit, P, 2)`), an overhead chat message 0x26 (type 5,
   `0x0053C750`) with the text of the +0xA4 record (`0x006611E0`).
4. **Inventory** `0x00534F80(unit, client)` (unit +0x60 null is fatal
   0x187): for each inventory node (`0x0063B2C0`, `0x0063DFA0`) by kind
   (`0x0063E020`): 3 → 0x9D (`0x0053D090(item, 0)`); 1 or 2, only when
   the unit is the client's player → 0x9C (`0x0053ED50` / `0x0053EE70`);
   the actions are `items/inventory.md` §6.
5. Player part B for another player (`0x005489F0`: the stat list
   `0x00731AD8` sent through `0x00548520`; `0x005484B0`: the per-mode
   function table `0x007319E8`, 12-byte rows) is multiplayer only: the
   room switch skips the client's own player, and on P's own load
   (§8.2 step 3) `0x005489F0` returns at once.

### 8. Single-player session sequence (C→S 0x67 → 0x6B → first tick)

Owner of which server code sends the session and join messages of a
single-player game, and in which order; field meanings stay with the
owners cited. §8.1 and §8.2 run inside the drain (§1 rule 1), outside
any tick; §8.3 is the next tick.

#### 8.1 Game creation (C→S 0x67, `0x00530BF0`)

Reached from §2.5. After the game record, its acts and the game seed
(`sim/rng.md` §5.2), in order:

1. The client record is allocated and prepended (`0x00539A30`,
   `sim/unit-order.md` §7); the arena record (`0x0053FF90`, game
   +0x1D28).
2. The seed derivations of `sim/rng.md` §5.2 (`0x00547D20`;
   `0x00546C60`, its result stored in game +0x80; `0x00536070`;
   `0x00545D80`), then `0x0052C110`.
3. **S→C 0x01** (`0x0053B340(client, 1, game)`, 8 bytes): u8@1 = game
   +0x6D (difficulty); u32@2 = the arena record's flags (game +0x1D28 →
   +0x08, `0x0053FD40`; recorded 0x00100004 in every join of both
   recordings); u8@6 = 1 when game +0x70 ≠ 0 (expansion), else 0; u8@7
   = 1 when game +0x74 ≠ 0 (ladder), else 0.
4. **S→C 0x00** (`0x0053B320(client, 0)`, 1 byte); client state (client
   +0x04) := 1 (`0x005386D0`).
5. The client's act byte from the global `0x00883D44`
   (`sim/path-placement.md` §13 rule 2); host bookkeeping (`0x00564A00`).
6. **S→C 0x02** (`0x0053B320(client, 2)` at `0x00530E91`).

Recorded (both recordings, seq 5–7): `01 00 04001000 01 00`, `00`,
`02`. The other senders of 0x01 / 0x00 (`0x0052C260`, from the hosted
join `0x0052FA50`) and of 0x02 (`0x0052FC0C`, `0x00530805`) are not on
this path. The client answers 0x02 with C→S 0x6B (`client/model.md` §7
rule 3), drained in a later frame (recorded: after tick 1).

#### 8.2 Join (C→S 0x6B, `0x0052C550` → `0x00530190`)

1. Game lookup by client id (`0x0052B610`, `0x0052E860`); no game, or no
   client record for the id (`0x005381C0`): log, stop.
2. **Load** `0x00539760(client, game, 1, 0, 0, 0)`: client +0x3D4 |= 1;
   a classic game (client +0x70 = 0) with an expansion class (client +8
   ≥ 5) → result 0x18; else the character is loaded (`0x005345A0`: the
   save path `0x005344B0` when its flag argument is 1 and there are no
   host callbacks, else a new character `0x00532590`) and checked
   against the arena flags (`0x0053FD40` bit 0x800, client +0x0A bits
   0x4, 0x8, 0x20: results 0x13, 0x14, 0x15, 0x17, 0x18). A non-zero
   result → **S→C 0xB4** (direct, `0x0053B260`, §3.3 rule 5) with the
   code, the client is removed (`0x00539DA0`), stop.
3. Messages of a successful load, in order:
   1. From the loader (the player is allocated nowhere, position
      (0, 0)): the player's own add messages `0x00571F90(game, P,
      client)` (§7.2: 0x59; part B: 0xAA §7.9 rule 1, 0x76 §7.9 rule
      3), then the loader's other messages. Recorded (`-022633` seq
      102–112): 0x59, 0xAA, 0x76, 0x94 (`0x0053C5D0`, from
      `0x00532EB7`), 0x22 × 2 (`0x0053C520`), 0x21 (`0x0053C4A0`),
      0x23 (`0x0053C590`), 0x5E (`0x0053D830`, from `0x00546270`), 0x28
      (`0x0053D670`), 0x29 (`0x0053D700`, from `0x00544520`).
   2. **S→C 0x0B** (`0x00537930` → `0x0053B3D0`: type u8@1, GUID u32@2
      of P; no P → type 6, GUID −1).
   3. **S→C 0x5F** (`0x0053B400`): u32@1 = `0x00622230(P)`.
   4. P's stat messages (`0x006258D0(P, P, 0x00548520)`: 0x1D / 0x1E).
   5. If `0x00463720(P, 1)`: P's item messages (`0x00597890(game, P,
      client, 0)`: 0x9C, 0x9D), then P flag-ex |= 0x200000.
   6. For each hot-key slot i = 0..15 (client +0x3DC + 8i: skill i16,
      flag u8 at +2, item u32 at +4) whose skill is in 0..skills count
      − 1: **S→C 0x7B** (`0x0053DB20`, 8 bytes: slot u8@1 = i, u16@2 =
      skill & 0xFFF, | 0x8000 when the flag is set, u32@4 = item).
   7. When `0x006221A0(P)` gives a record: two **S→C 0x23**
      (`0x0053C590`, 13 bytes: type u8@1, GUID u32@2, hand u8@6, skill
      u16@7, item u32@9): hand 1 with record +0x74 / +0x7C, then hand 0
      with +0x70 / +0x78.
   8. P's stat messages again (as 4).
   9. `0x00548760(P, client, force 1)`, the life / mana message against
      the client's cache (`0x00539330`): **S→C 0x95** (`0x0053C320`; the
      cache-dependent variants `0x0053C230`, `0x0053C3F0` are not taken
      at a join); then, when stat 14 `gold` differs from the cache, its
      stat message (`0x0053E9B0`); then, when stat 13 `experience`
      differs, the experience message `0x0053BDD0(new, old)`: delta =
      new − old (unsigned); < 0xFF → **0x1A** (u8@1 = delta), ≤ 0xFFFE
      → **0x1B** (u16@1 = delta), else **0x1C** (u32@1 = new).
   10. `0x00597B00(client, P)` (`items/inventory.md` §6.1 rule 4); if
       client +0x70 ≠ 0, `0x0058A0A0(client, P)`.
   Recorded seq 113–141: 0x0B, 0x5F, 8 stat messages, 0x9C × 6, 0x9D,
   0x23 (hand 1, skill 0), 0x23 (hand 0, skill 36), 8 stat messages,
   0x95, 0x1B (no 0x7B: no hot key set). The four joins of the two
   recordings end step 3 with 0x95 then 0x1B, 0x1A, or 0x1E 0x1B.
4. **Act** `0x0052C210(game, client)` (null client fatal 0x1C4): build
   the client's act when its slot is empty (`0x0053AFB0` →
   `0x0053AC70`, `drlg/levels.md` §2); **S→C 0x03** then **S→C 0x53**
   (`0x0053ABE0`; 0x03 `client/model.md` §11 rule 1; 0x53: u32@1, u32@5,
   u8@9 = the three outputs of `0x0061C330(act)`, recorded `53 02000000
   00000000 00`); client state := 2.
5. **Game entry** `0x005394A0(client, P, game, room 0, 0, 0)`
   (`sim/path-placement.md` §11, §13): S→C 0x07 for the spawn room; the
   room switch (§7.8, through `0x005381F0`; old room none: 0x07 and add
   messages for every room of the spawn room's adjacency array);
   placement (`0x00554850`); S→C 0x15; **S→C 0x7E** (`0x0053DB70`, 5
   bytes, Edge cases); followers and `0x005773D0` (`sim/path-placement.md`
   §13 rule 4).
6. Client state := 3; unlock, log.

Recorded (`-022633` seq 142–155): 0x03, 0x53, 0x07 × 10, 0x15, 0x7E;
the switch sent no add message (the town rooms are populated by the
next tick, `sim/tick.md` §4).

#### 8.3 First tick after the join

Client state 3 (`sim/tick.md` §6 rule 4): the per-client update
(`0x005380D0`) announces the units the room pass just created (§7.1
flag 0x10: monsters 0xAC, 0xAA, mode message; objects 0x51, then 0x0E
from §7.3 rule 3) and sends P's own updates; then, the room being ready
(`sim/tick.md` §6 rule 6): **S→C 0x04**, client state 4, the inventory
refresh `0x0055DF00`, the join sequence `0x0052C410` (0x5B
`0x0053C940`, 0x65 via `0x0053FC70`), `0x0055B620` (0x8D), host
callback, 0x5A to all. Recorded frame 2 (`-022633` seq 157–224): units,
0x1D / 0x1E, 0x48, **0x04**, 0x48, 0x5B, 0x65, 0x8D, 0x5A.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| C→S size table | `0x00730DC0`, 0x71 × i32 | §2.1 |
| C→S handler table | `0x006E0D18`, 0x67 × 8 bytes | §2.3 |
| gate jump table | `0x0054D848` (bytes) → `0x0054D838` | §2.3 |
| S→C size table | `0x00730AE8`, 0xB5 × i32 | §3.1 |
| client receive table | `0x007114D0`, 0xAF × 12 bytes | §3.1 |
| largest message | 0x204 (516) bytes | §2.1, §3.2 |
| client buffer | 0x200 (512) bytes per node | §3.2 |
| drain copy | 0x200 bytes incl. 4-byte client id | §2.1 rule 7 |
| point / unit range | 50 subtiles per axis | §2.4 |
| resync interval | > 25 frames | §2.4 rule 3 |
| duplicate windows | 50 ms / 200 ms / none | §2.1 rule 1 |
| flush throttle | 40 ms unless forced | §3.2 rule 3 |

No `.txt` data is read by the transport; handlers read their tables.

## Randomness

The transport, dispatcher, gate and parsers draw nothing. Handlers draw in
their systems (`rng.md` §7), in dispatch order, before the tick's draws
(§1 rule 2).

## Edge cases & original bugs

1. Dispatch results are ignored (§2.2 rule 5): a malformed message costs
   nothing but its own rejection. Reproduce as is.
2. Local transport drops a message whose size rule fails or whose id is
   0x71–0xFE without any trace (§2.1 rule 4).
3. Drain truncation: messages over 0x1FC bytes are copied short with their
   full size (§2.1 rule 7).
4. C→S chat size rule adds the signed byte after the second string's NUL
   (§2.1 rule 5): a well-formed message has 0 there; any other value
   changes the size.
5. Body-location fields read as u8 inside a 4-byte slot (§2.4 rule 9).
6. Client duplicate filter (§2.1 rule 1): identical repeated requests
   within 50/200 ms never reach the server; d2rs's client reproduces it so
   recorded inputs replay.
7. Unknown or size-0 S→C id inside a buffer ends the local split and
   loses the rest of that buffer (§3.3 rule 3); builders for 0x83, 0x84,
   0x88 exist although those ids are not receivable (§3.1 rule 3).
8. 0x80 in the receive table expects 4 bytes but has no size and no
   handler (§3.1 rule 2).
9. 0x45 (change portal location), 0x39, 0x52 are stubs returning 3; 0x2E,
   0x42, 0x43 stubs returning 0.
10. **S→C 0x7E bytes 1–4 are uninitialised stack memory**: `0x0053DB70`
    writes only the id byte of its 5-byte message (its EDX argument,
    game +0xEC, is never stored). Recorded `7e 000000ff` (`-015956`) and
    `7e 00000041` (`-022633`). Not reproducible: d2rs sends zeros there,
    and the §6 comparison excludes bytes 1–4 of 0x7E (the one exception
    to §6 rule 3; a d2rs decision, the client handler `0x0045E970` is
    `client/`'s to check).
11. The spawn room's 0x07 is sent twice at game entry (§7.8 rule 5);
    the client handles both (`client/model.md` §9).
12. `0x0053B5F0` (0x68 for a moving monster) dereferences the unit
    looked up by GUID without a null check (§7.7 rule 5).

## Test vectors

Synthetic (from the rules; CI-safe):

| Input | Expected | Source |
|---|---|---|
| C→S size `15 01 00 'hi' 00 'bob' 00 00` | 11 | §2.1 rule 5 |
| same with last byte 05 / FF | 16 / 10 | §2.1 rule 5 |
| C→S size `66 05 00` / `66 FD 01` / `66 FE 01` | 8 / 512 / 3 | §2.1 rule 5 |
| C→S size `6C 10 00 00 00 00` / `6C 10` | 23 / incomplete | §2.1 rule 5 |
| C→S size of 0x2C, 0x4A, 0x64 | 0: never queued | §2.1 rule 5 |
| classifier: id 0x80 / 0xFF (16 bytes) / 0x6B / 0x01 (5 bytes) | 4 drop / queue 2 / queue 0 / queue 1 | §2.1 rule 4 |
| classifier: 0x01 with 4 bytes | 3 drop | §2.1 rule 4 |
| dispatch id 0x00 / 0x67 / 0x2B | 3 / 3 / 3 | §2.3 |
| dispatch 0x41, player mode 0x11 / mode 1 | handler runs / 0 | §2.3 rule 3 |
| dispatch 0x01, player in state 54 / mode 0 / mode 1 | 0 / 0 / handler | §2.3 rule 3 |
| dispatch 0x3C or 0x14 with the player dead | handler runs (no gate) | §2.3 rule 3 |
| handler 0x01 with size 6 / 0x45 any / 0x42 any | 3 / 3 / 0 | §2.4 |
| 0x01 target (x+50, y−50) / (x+51, y) | accepted / 1 (+0x15 if > 25 frames since last accept) | §2.4 rule 3 |
| 0x3C bytes `3C 05 00 00 80 FF FF FF FF` | left hand, skill 5, item −1 | §2.4 rule 7 |
| 0x51 bytes `51 06 80 03 00 FF FF FF FF` | skill 6, left, slot 3, item −1 | §2.4 rule 7 |
| S→C size `94 03` + 7 bytes / `AF 00` / `AF 05` / `AE 10 00` | 15 / 2 / 6 / 19 | §3.1 |
| S→C size `26` + 9 bytes + `'a' 00 'bc' 00` | 15 | §3.1 |
| S→C size `16 20 00` (3 bytes) / with 13 bytes | incomplete / 32 | §3.1 |
| queue messages of 300 then 250 bytes | buffers [300], [250] | §3.2 rule 1 |
| queue 200, 200, 112, 1 | buffers [512], [1] | §3.2 rules 1–2 |
| client sends `03 10 00 20 00` twice 120 ms apart | second not sent (200 ms window) | §2.1 rule 1 |
| client sends `0C 10 00 20 00` twice 60 ms apart | both sent (50 ms window) | §2.1 rule 1 |
| unit update: missile with unit flag 0x10 | nothing sent | §7.1 rule 2.1 |
| unit update: new item (flag 0x10), mode 3, flag 0x1000 | 0x9C action 2 once; after the clean-up and no change: nothing | §7.1, §7.5 |
| unit update: monster mode 0 (flag 0x1), no target, cell (4757, 5461), d 0x38, +0xB0 6, GUID 0x13 | `69 13000000 08 9512 5515 38 06` | §7.4; recorded `-015956` frame 2724 |
| same monster later in mode 12 at (4756, 5461) | `69 13000000 09 9412 5515 38 00` | §7.4; recorded frame 2748 |
| monster mode 1 (flag 0x1) with GUID 6 at (4634, 4537), d 0x80 | `6d 06000000 1a12 b911 80`; stat 328 += 1 | §7.4 rule 5; recorded `-022633` seq 159 (after its 0xAC) |
| monster mode 6 (block), no target, GUID 0x22 | `69 22000000 12 0000 ffff 00 00` | §7.4 rule 5; recorded `-015956` seq 264400 |
| monster with a skill in use, target in the client's rooms | 0x4C, not a mode message | §7.4 rule 3 |
| clean-up of a player | stat 29 = −1; flags 0x1, 0x10, 0x100, 0x400, 0x8000 clear | §7.5 |
| monster mode 2, no target, GUID 6, cell (4825, 5636), stop distance 0, path type 7, velocitypercent 75, max path distance 5 | `67 06000000 01 d912 0416 01 00 07 4b00 05` | §7.7 rule 5; recorded `-015956` frame 24 |
| monster mode 2, target player GUID 1, GUID 0x23 at (4671, 5397), stop distance 0, path type 13 | `68 23000000 00 3f12 1515 00 01000000 01 00 0d 4b00 05` | §7.7 rule 5; recorded `-015956` frame 3149 |
| monster mode 4 (A1), target monster GUID 0x29, GUID 0x26 at (4700, 5340), d 0 | `6c 26000000 0a 01 29000000 00 5c12 dc14` | §7.7 rule 5; recorded `-015956` frame 3080 |
| monster mode 8 (S1) to point (0, 0), GUID 6 at (4825, 5636) | `6b 06000000 0c 0000 0000 00 00 d912 0416` | §7.7 rule 5; recorded `-015956` frame 177 |
| monster mode 0, path without target, GUID 0x1B | `69 1b000000 08 0000 0000 38 06` | §7.4 rule 7; recorded `-015956` frame 2882 |
| room switch leaving room at tile (968, 1120), level 1 | `08 c803 6004 01` after the switch's 0x07s | §7.8; recorded `-015956` frame 149 |
| player with state 105 (`alignment`) whose list holds stat 172 = 2, GUID 1 | `aa 00 01000000 0c 69 59 f9 ff 1f` | §7.9 rule 1; recorded `-022633` seq 103 |
| game creation, Normal, expansion, not ladder, arena flags 0x00100004 | `01 00 04001000 01 00`, then `00`, then `02` | §8.1; recorded seq 5–7 of both |
| hot-key slot 3, skill 36, flag set, item −1 | `7b 03 2480 ffffffff` | §8.2 rule 3.6 (synthetic) |
| join of a character with no hot key, then the town spawn | 0x59 … 0x0B, 0x5F, …, 0x95, 0x1B, 0x03, 0x53, 0x07 × (1 + array), 0x15, 0x7E; next tick … 0x04 | §8; recorded `-022633` seq 102–219 |

The vectors for size rules are implemented in `check_packets.py`'s
`size_of` (same inputs, same outputs); `--selftest` runs the packing and
order rules.

Recorded (`record_packets.py`, 2026-10-06, `Game.exe -w -ns`, existing
character, Normal, Act 1: walk, run, skills, items, belt, NPC trade,
played by hand; raw file gitignored): `20261006-015956-packets.jsonl`,
`check_packets.py` OK (R1–R7, 0 failures); `--perturb 1271` reported at
that event. Client ids seen: 0x01–0x04, 0x06, 0x09, 0x0D, 0x10, 0x13,
0x18, 0x19, 0x1D, 0x23, 0x26, 0x2F–0x33, 0x38, 0x49, 0x4F, 0x53, 0x54,
0x59; system ids 0x67, 0x69, 0x6B, 0x6D. Game type (game +0x6A) = 3 on
every tick. The committed format-1 trace (`packets-0001`) is written by
the converter of §6 rule 4 once `d2-sim` can replay a game.
Second recording `20261006-022633-packets.jsonl` (waypoint, stat and
skill points, stash): `check_packets.py` OK, `--perturb` reported; adds
client ids 0x0C, 0x3A, 0x3B (28 client ids seen in total).

## Provenance

- **1.14d `Game.exe`** (SHA-256 `631066c1…adaaf`, image base `0x400000`).
  Tables were read from the file image: size tables `0x00730DC0`,
  `0x00730AE8`; handler tables `0x006E0D18`, `0x007114D0`; jump tables
  `0x0052BD40`, `0x0052BB80`, `0x0054D848`, `0x0045C894`, `0x00478550`.
  Functions were read in the disassembly (capstone over the Ghidra
  function list; register arguments from the disassembly, since the
  Ghidra export drops them). Writers and readers of globals were found by
  scanning `.text` for rel32 calls and 4-byte references.
- Handler sizes and field offsets: each of the 95 non-null handlers'
  first size check and its reads of the message (directly or through its
  parser) were extracted with a scan of the disassembly and checked by
  hand; stubs read in full.
- S→C senders: the 113 callers of `0x0053B280` and the 11 of `0x0052B330`
  were scanned for the header byte (immediate store into the message, or
  the immediate passed in DL by the caller of a shared builder; bit-packed
  builders via their first 8-bit write). Builders whose header comes from
  a caller-built struct are left `-`; the recorder logs every caller and
  settles them.
- **1.14d data**: state 12 = `inferno`, 54 = `uninterruptable`
  (`states.txt` rows, patch_d2 and d2exp); stat 328 = `pierce_idx`
  (`itemstatcost.txt`, patch_d2).
- **Smoke run** (2026-10-06): `record_packets.py --seconds 10` on the
  reference `Game.exe`: all 13 hook sites matched their expected bytes,
  0 events at the main menu, game terminated.
- **D2MOO** (1.10f): `D2Game/src/PLAYER/PlrMsg.cpp`
  (`D2GAME_PACKET_Handler_6FC89320`: same table shape, same gate, same
  dead-only 0x41, same IsBadCodePtr check; names of the C→S handlers),
  `GAME/CCmd.cpp` (`CCMD_ProcessClientMessage`: the hack-list consequence
  1.14d removed), `GAME/SCmd.cpp` (`D2GAME_PACKETS_SendPacket`: same
  0x200 buffer rule), `PLAYER/PlrModes.cpp` (the gate function),
  `D2CommonDefinitions/include/D2PacketDef.h` (field names). Differences
  in 1.14d: per-message parsers with exact size, range and act checks
  (§2.4); 0x66 is a new no-gate id (warden) and system ids moved up by
  one; 0x0B, 0x2C, 0x2D, 0x39, 0x45, 0x52 are stubs; the dispatcher's
  result is ignored; 0x3A is 3 bytes (D2MOO: 7); 0x3C carries the hand
  in bit 31 of a u32 (D2MOO: two u16); 0x1A/0x1B/0x1D/0x1E read a u8
  body location.

Unit-update session (2026-10-06): per-unit update `0x0053A500` (type
table `0x0053A5B4`), walk `0x0053A620`, `0x0053A5D0`; add messages
`0x00571F90` (tables `0x005722F8`, `0x0057230C`); monster update
`0x00598220`; mode message `0x00597E20` (mode table `0x006E1D90`, switch
`0x00598208` / `0x005981F0` read from the file), skill message
`0x00597D70`, builders `0x0053D530`, `0x0053D4D0`, `0x0053B710`,
`0x0053B910`; object update `0x00581AD0`, `0x00581A20`; item update
`0x0055BF30`, `0x0055BED0`; clean-up `0x00553220` (type table
`0x00553364`), `0x00625960`, `0x00571F40`, `0x00620FA0`, `0x00639EE0`,
`0x00573570`, `0x0053FA90`; target in client's rooms `0x005387F0`;
skill in use `0x00620250`. Stat 29 = `lastexp` (patch_d2
`itemstatcost.txt` row 29). 0x69 codes 6 / 8 / 9 counted in
`20261006-015956` (15 / 20 / 24, plus 2 × 0x12) and 0x51 type bytes in
both recordings (206 × type 2).

Server-join session (2026-10-07): game creation `0x00530BF0`
(`0x00530DF3`–`0x00530E91`), `0x0053B340`, `0x0053B320`, `0x0053FD40`;
join `0x00530190`, load `0x00539760`, `0x005345A0`, `0x00532590`,
`0x00537930`, `0x0053B400`, `0x0053DB20`, `0x0053C590`, `0x00548760`;
act `0x0052C210`, `0x0053ABE0`; game entry `0x005394A0`, `0x005381F0`,
`0x0053DB70`; room switch `0x00537B50`, join `0x0053A8E0`, leave
`0x0053A9B0`, builders `0x0053BC50`, `0x0053BC90`, `0x0053BDA0`,
`0x00571600`; 0xAA `0x00570E30` with the bit writer `0x00410E40`,
`0x00410E90`, `0x00410EB0`; `0x00571CD0`, `0x00571620`, `0x00534F80`;
monster builders `0x0053B5F0`, `0x0053B710`, `0x0053B7F0`, `0x0053B910`,
`0x0053B9F0`, `0x0053BA40`, `0x0053BAA0`, `0x0053BB00`, `0x0053BB70`
(callers by `disasm.py xref`: each only `0x00597E20`); `0x00648E30`,
`0x00648E60`, `0x00648EA0`, `0x006490A0`, `0x005A5650`; DT functions
`0x005A7350`, `0x005A72B0`, `0x0063E8D0`, `0x006217C0`; 0x65
`0x0053D9C0`, `0x0053FB30`, `0x0053FB90`, `0x0053FC20`, `0x0053FC70`.
Checked on both recordings (caller field = the builder's return
address): join order seq 4–224 of `-022633` (and the second join of
each file), every 0x67–0x6D, 0x08 order (48 frames), the 0xAA vector
against patch_d2 `states.txt` row 105 and `itemstatcost.txt` row 172
(send bits 2), 0x7E bytes.

## Open questions

1. R1–R7 on a hosted game and with more message ids (the single-player
   recordings cover 28 client ids).
2. *Answered* (§8, from the code and both recordings): C→S 0x67 and
   0x6D in the first drain (0x01, 0x00, 0x02 from `0x00530BF0`; 0x8F for
   the ping), C→S 0x6B after tick 1 (the join of §8.2: 0x59 … 0x7E in
   that drain), 0x04 / 0x5B / 0x5A in the next tick (§8.3); 0x69 on
   leaving. Still open: 0x05, 0x06, 0x5C, 0xAF, 0xB0 at exit.
3. Game types 1 and 2 (the 3-buffers-per-flush limit and message 0xB3,
   §3.2 rule 5): single player is type 3 (recorded), so they apply only
   to other hosting modes; unconfirmed.
4. S→C senders left `-` in the TSV (35 receivable ids, among them 0x16,
   0x26, 0x27, 0x4C, 0x4D, 0x67, 0xA8, 0xAA): settle from the `caller`
   field of `packets-0001` and later traces. Answered in prose (TSV
   sender cells unchanged): 0x4C / 0x99 `0x0053D530`, 0x4D / 0x9A
   `0x0053D4D0`, 0x67 `0x0053B710` and `0x0053B910` (§7.4). 0x67 –
   0x6D layouts: §7.7; 0x08 (`0x0053BC90`) and 0x0A: §7.8; 0xAA
   (`0x00570E30`): §7.9 rule 1; 0x01, 0x23, 0x53, 0x7B, 0x7E: §8.
5. S→C field layouts: only the builders in the `layout` column were read;
   every other layout is unconfirmed (one owner per system spec later).
6. C→S field meanings marked `partial` (0x14, 0x15, 0x26, 0x2F–0x33, 0x35,
   0x38, 0x3D, 0x3F, 0x44, 0x4D, 0x4F, 0x58, 0x59): offsets and widths
   are 1.14d, names are D2MOO's; each system spec confirms its own.
7. The odd third term of the C→S chat size rule (§2.1 rule 5): does the
   1.14d client ever send a non-zero byte there?
8. 0x0B, 0x2E, 0x42, 0x43: does the 1.14d client ever send them?
9. Client receive handlers' behaviour (§3.4 rule 3) belongs to client
   specs (Phase 5–6); not covered here.
10. The conditions inside `0x00571CD0`, `0x00571620`, `0x005715A0`,
    `0x00570E30` / `0x005711D0` (§7.3 rule 2 steps 3, 4, 8, 9) and which
    of their senders fire for a plain Act I monster; and the meaning of
    `0x005A5650` (the d byte, recorded 0x80 on 0x6D) and `0x00572EE0`.
    *Partly answered*: `0x00570E30` (0xAA), `0x00571CD0` (pending event
    records), `0x00571620` (0x76 / overhead 0x26): §7.9; `0x005A5650` =
    the life fraction (§7.4 rule 5). Open: `0x005711D0`, `0x005715A0`,
    `0x00572EE0`, and who writes the unit +0xEC records.
11. The §7.6 order (item 0x9C before the monster's 0x69 in a kill tick
    with a drop) and where 0x65 (`0x0053D9C0`, caller `0x0053FB30`,
    recorded right after 0x69 code 8) is sent from: a recording of a
    kill that drops an item. Narrowed: 0x65 = `65`, player GUID u32@1
    (−1 without a player), kill count u16@5 (the player record's +0x34
    → +0x00; recorded 1, then 2); `0x0053FB30`'s callers are the arena
    sync `0x0053FC20` (per-client update; needs arena flag 0x400 and the
    player's arena record +0x04 ≠ 0), `0x0053FB90` and `0x0053FC70`
    (from the join sequence `0x0052C410`). The recorded arena flags
    (0x00100004, §8.1) lack 0x400, so which path sends the kill-tick
    0x65 needs a breakpoint on `0x0053FB30` during a kill (its return
    address), or a write watch on arena flags.
12. The loader's messages between 0x59 and 0x0B (§8.2 rule 3.1): their
    order inside the save path `0x005344B0` → `0x00534020` is recorded,
    not read; owner: the save-load spec (`formats/d2s.md` and the
    character-load spec, not yet written).
13. The first 0x48 of the first tick (§8.3, before 0x04): its caller
    (`0x0055DBC0` inventory refresh or `0x00580860` player update);
    settle with the recorder's caller field one level up.
