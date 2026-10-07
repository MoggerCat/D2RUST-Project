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
  tests; not yet run on a recording. §7.4 / §7.7 (the monster mode
  message, S→C 0x67–0x6D, and the death pair 0x69 codes 8 / 9) and the
  flag part of §7.5 are implemented in `d2-sim`
  (`monsters::mode_message`, `wiring::action::unit_update`; branch
  `claude/impl-monster-death`): the §7.4 / §7.7 Test vectors pass as unit
  tests; unverified against a recording of the wired host.
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
| Summary | 58–74 |
| Inputs | 75–83 |
| Outputs / state changes | 84–90 |
| Rules | 91–92 |
|   1. Loop order (single player) | 93–114 |
|   2. Client → server | 115–382 |
|   3. Server → client | 383–602 |
|   4. d2rs mapping and scope | 603–634 |
|   5. Machine-readable tables | 635–671 |
|   6. Exact-match comparison | 672–779 |
|   7. Unit update messages (`0x0053A500`) and room clean-up (`0x00553220`) | 780–1202 |
|   8. Single-player session sequence (C→S 0x67 → 0x6B → first tick) | 1203–1417 |
|   9. C→S handlers: owners, and the small handlers owned here | 1418–1574 |
| Constants & data dependencies | 1575–1593 |
| Randomness | 1594–1599 |
| Edge cases & original bugs | 1600–1645 |
| Test vectors | 1646–1732 |
| Provenance | 1733–1849 |
| Open questions | 1850–1969 |
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

   | Classifier step (in this order) | Result |
   |---|---|
   | size < 1 | 3 (incomplete) |
   | size rule (`0x0052BC20`, rule 5) gives 0 (this includes ids 0x71..0xFE, entry-0 ids and too few bytes) | 3 |
   | rule size, low 16 bits compared unsigned, > 0x204 (`0x0052B139`) | 4 (invalid); a **negative** chat size (rule 5) always lands here, since its low 16 bits are ≥ 0x8000 |
   | rule size (low 16 bits) > the given size | 3 |
   | id < 0x67 | queue 1 (game), result 1 |
   | id 0x67..0x70 | queue 0 (system), result 2 |
   | id 0xFF (rule: 16 bytes) | queue 2, result 1, if the net object's gate `0x006BF6C0` passes; else 4 |

   The classifier's own id test for 0x71..0xFE (result 4) is dead code:
   the size rule already returned 0 for those ids (result 3).
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
8. **Client builders** (2026-10-08; answers open questions 7 and 8).
   Every C→S game message (ids < 0x67) passes the sender of rule 1,
   whose 13 call sites are all in the builders `0x00478590`–`0x00478830`
   (by layout: `0x00478590` [id], `0x004785B0` [id][u16], `0x004785D0`
   [id][u16][u16], `0x00478600`, `0x00478640` (no caller), `0x00478680`
   [id][u32], `0x004786A0` [id][u32][u32], `0x004786D0`, `0x00478700`
   take the id in CL; `0x00478740` writes 0x5D, `0x00478780` 0x5E,
   `0x004787B0` chat, `0x00478830` 0x66). The system senders (rule 2)
   use only ids 0x67–0x70. The ids at every builder call site are
   constants, except:
   - `0x00480B40` (the only caller passing a variable id to
     `0x004785D0` / `0x004786A0`): its id comes from `0x00481030`,
     whose switch (table `0x004812B4`) sends ids 1–0x11 and 0x13 to
     their cases and takes 0x0B, 0x12 and every id outside 1–0x13 to a
     fatal assert (0x352) before anything is sent; `0x00480B40` sends nothing for 0x13;
   - `0x004B2650` → `0x00478700` with 0x32, 0x33 or 0x35;
   - the chat builder `0x004787B0` with the record's id byte, written
     as 0x14 or 0x15 by its two callers (`0x0047C1F0`, `0x0047C420`).
   So the 1.14d client never sends 0x0B, 0x2E, 0x42 or 0x43.
   **Chat third term** (rule 5): `0x004787B0` copies id, type and
   language bytes, the text (at most 0x100 bytes with its NUL), then
   the name (at most 0x10 with its NUL; the name pointer is never
   null), then writes **one 0 byte** and sends L1 + L2 + 6 bytes. The
   byte c of the size rule is therefore always 0 from the 1.14d
   client; a non-zero c reaches the server only from another client.
   Recorded (`pc2rec-p1-packets`, `pc2rec-p2-packets`, single player,
   9 typed chat lines): every C→S 0x15 is `15 01 00 <text> 00 00 00`
   (type 1, language 0, empty name, c = 0), also for lines starting
   `/w`, `/m`, `/msg`, `/whisper`, `*` or `@`: the single-player client
   never fills the name, the whole line is the text.

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
6. 0x14 (`0x0054A290`): msg = cstr at +3; strlen ≥ 256 → 2; strlen 0
   → 0 with no effect (`0x0054A2D5` jumps to the `xor eax, eax` exit at
   `0x0054A3F0`; §9 rule 3; corrected 2026-10-08 for
   `docs/handoff/impl-umods-cs-handlers.md`, which found "else 2" here);
   name = cstr after it (≤ 16). 0x15 (`0x0054A5D0`): msg strlen < 256 and
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
    `bytesN` N raw bytes (S→C records carried whole: 0x28, 0x29, 0x52,
    0x5E), `uN@k` / `bitN@k` = bit fields of the u32 at k. Only offsets a 1.14d
    handler reads are listed.

#### 2.5 System messages `0x0053F100`

ECX = buffer (client id, message), EDX = size. Switch on the id:

| Id | Does (1.14d) |
|---|---|
| 0x67 | `0x0052C330` refuses (nothing happens) when host callbacks exist and u8@0x11 ≠ 0, the character name (cstr16@0x15) or the game name (cstr16@1) fails `0x0053EFC0(name, 16)`, u8@0x2D > 14, u32@0x27 has neither bit 1 nor bit 2, the name checks `0x00538B70` / `0x00538C60` fail, or class u8@0x12 ≥ 7 (checks: rule 1 below). Else `0x00530BF0(client, game name, u8@0x11 → game +0x6A (type), class u8@0x12, character name, u16@0x25 (passed in EDX to the arena record `0x0053F4B0`, which overwrites EDX before any use: never read), u32@0x27 (& 0x3179C7 → arena record +0x08, §8.1 rule 1; bit 20 → game +0x70 expansion, bit 21 → +0x74 ladder), u8@0x13 → game +0x6B and arena record byte +0x0C, u8@0x2B, u8@0x2C (passed, never read), u8@0x14 → game +0x6D (difficulty), u8@0x2D → client +0x50C)` |
| 0x68 | join: optional check `0x0053EFF0` when host callbacks `0x00883D50` are absent; `0x0052C690` validates, `0x0052FA50` joins |
| 0x69 | leave (`0x0052C8E0` → `0x005303D0`, which flushes the client; rule 2) |
| 0x6A | `0x0052DAF0` → `0x0052E9B0` (game list, rule 3) |
| 0x6B | `0x0052C550` → `0x00530190` |
| 0x6C | total (u32 at +2) ≥ 0x2000 → fatal assert; `0x0052DB00` → `0x0052DB10` → `0x00538CE0(client, data @6, len u8@1, total, 0, 0, 0)` (save upload chunk, rule 4) |
| 0x6D | `0x0052C400` → `0x005389A0(client, tick u32@1, value u32@5)`: the client's game found → `GetTickCount` − tick − value goes into a 16-entry ring (client +0x4B4, count +0x500), the mean of the stored entries → client +0x4F8 (64-bit), S→C 0x8F (`0x0053E020`: no arguments besides the client; 0x21 bytes, the id then 32 zero bytes, corrected 2026-10-08), client +0x3D8 := now (ping) |
| 0x6E | `0x0052CBE0` → `0x00530270` (no effect, rule 5) |
| 0x70 | client record +0x504 = 1, `0x005377A0` (rule 6) |
| others | ignored |

Bodies (1.14d asm, 2026-10-08). "Client table" = the client records
hashed by client id (bucket id & 0xFF at `0x008842A8`, chain +0x4AC,
lock `0x008846A8`; "initialised" = `[0x008846D8]` ≠ 0).

1. **0x67 checks** (`0x0052C330`, in its order): host callbacks and
   u8@0x11 ≠ 0 → refuse; `0x0053EFC0(cstr@0x15, 16)` = 1 iff a NUL is
   within the first 16 bytes (the character name), else refuse; u8@0x2D
   > 14 → refuse; the same test on the game name cstr@1; u32@0x27 & 6
   = 0 → refuse; `0x00538B70(client id, out)`: passes when the client
   table is not initialised or holds no record with this id; a record
   with the id (the client already has a game) → copies its character
   name (+0x0D, 16) to `out` and refuses; `0x00538C60(character name,
   out)`: the name table (bucket = `0x004112C0(name)` & 0xFF at
   `0x00883EA8`, chain +0x4B0; `0x004112C0` is a 16-bit CRC-style hash
   of the lower-cased bytes, table `0x00708140`, start 0xFFFF): passes
   when no record has the same name by `_strnicmp(…, 16)`
   (case-insensitive, `0x00413590`), else copies it and refuses; class
   u8@0x12 ≥ 7 → refuse. A refusal sends nothing.
2. **0x69 leave**: only when the client's record exists and its state
   (client +0x04) = 4 (`0x00538930(id, 4)`). Then `0x005303D0(id, 1)`:
   the game of the client (`0x0052B610`, `0x0052E860`, locked) and its
   record in the game (`0x005381C0` / `0x00537810`; none → unlock,
   stop). e := client +0x3D4 bit 5 (`0x00539030`). If e or game +0x6A
   ≠ 0 (single player: type 3, so always): `0x0052CA10(game)`: every
   client of the game with a player has its character saved
   (`0x00532400`); the ladder report after it needs host callbacks and
   game +0x74, so not in single player. Then for the client C: S→C **0x05**
   (`0x0053B320(C, 5)`); game type 1 or 2: drain C's pending save
   download (`0x00538FC0`, `0x0052E110`: 0xB3); S→C **0x06**; direct
   **0xB0** (`0x0053B220`); flush C's buffers (`0x0052E320(game, 0)`);
   build a 40-byte **0x5A** code 3 (u8@2 = 4, u32@3 = 0, character name
   cstr16@8 from client +0x0D, account name from client +0x1D at @0x18
   cut by byte 0x27 := 0); remove C (`0x00539DA0(game, id, e = 0)`);
   then the 0x5A to the remaining clients (`0x0054AA40`, §8.3). If e: repeat for the game's next client (game
   +0x88) until none. Unlock; game type 1 or 2: `Sleep(100)` (wall
   clock, not modelled); `0x0052B570`. Single player: the leaving client
   is the only one, so the 0x5A reaches nobody.
3. **0x6A game list** (`0x0052DAF0` always 1; `0x0052E8E0`): for each
   slot of the game table `0x00882D38`–`0x00883D37` (1,024 u32, skipping
   0 and −1) whose game is found (`0x0052E860`): direct **S→C 0xB2**
   (`0x0053B1B0`, 0x35 bytes) with name = game +0x2A (copied to 16
   bytes), u16@0x31 = game +0x8C, u16@0x33 = game +0x28; bytes 0x11–0x30
   are never written (stack contents; d2rs: zero). Then a terminator
   0xB2 with an empty name, u16@0x31 = 0, u16@0x33 = 0xFFFF. The client
   ignores 0xB2 (§3.4 rule 2).
4. **0x6C save upload** (`0x0052DB00` always 1; `0x00538CE0`): the
   client record (none, or table not initialised → nothing): when its
   received count (+0x180) is 0, a buffer of `total` bytes from the
   game's pool → +0x17C; count + len > total → fatal 0xB2F; the chunk
   is appended, count += len, +0x178 := 0; count = total → client
   +0x3D4 |= 8 and +0x18C := `0x00531E30(buffer)` (the save checksum,
   `formats/d2s.md`).
5. **0x6E**: proceeds only when `0x00538B70` passes (no client record
   with the id), then `0x00530270(id, 0)`: game and record lookups as
   rule 2; with argument 0, `0x0052CAF0` does nothing; unlock. No state
   change and no message in 1.14d.
6. **0x70**: the record found (`0x00537760`, which leaves the table
   locked) → client +0x504 := 1, unlock. +0x504 is read only by the
   heartbeat `0x0052D350` (`sim/tick.md` §6 rule 2), which with
   host callbacks drops the client; single player: no effect.

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
   The split never tests the size against the bytes left (`0x0052AEB0`;
   the size rule `0x0052B920` checks only each variable id's "needs ≥"
   prefix, and 0x26 the whole size). A message whose size runs past the
   buffer's end is copied whole (the bytes after the used part: zero up
   to the end of the 0x200-byte data area, which is zeroed when the
   buffer is taken, §3.2 rule 1; past it the buffer record's next field
   and heap memory), queued with its full size, and ends the split
   (bytes left ≤ 0). Unreachable while every builder queues the size its
   id's rule gives (§3.2 rule 2).
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

#### 3.5 Senders (the TSV `sender` column)

1. `sender` names the function that calls `0x0053B280` with the
   message; a record's `caller` (`record_packets.py`) lies in it. Many
   senders are copiers: they queue a fixed size from a message their
   caller built, id included. Rule 4 names the id writers.
2. Search (static, `tools/ghidra/disasm.py`): 122 functions hold the 129
   calls of `0x0053B280`; no pointer refers to it, to `0x0052B330` or to
   `0x0053E8D0`. Each was traced to the ids it can queue (an immediate id
   byte, DL from its callers, or the id store before each call of a
   copier). No function queues 0x12, 0x13, 0x14, 0x16, 0x24, 0x25, 0x45,
   0x54, 0x66, 0x6E–0x72 (`produced_by` none: client handlers exist and
   never run). The only 1-byte builder, `0x0053B320`, gets DL 0, 2, 4,
   5, 6, 0x4F. `0x0053DDF0`, `0x0053DE90`, `0x0053DF50` (copiers run by
   `0x005538D0`) carry 0x8C, 0x8D, 0x8E.
3. Recorded check: in both recordings every record of an id below has
   its `caller` inside the listed sender (counts in the table).
4. Senders found in this pass (layouts: the TSV):

   | Id | Sender | Id written by (store) | Records | Notes |
   |---|---|---|---|---|
   | 0x26 | `0x0053C750` | `0x0054A5D0` (`0x0054A895`, u8@1 := 6), `0x00571620` (`0x005716A4`, u8@1 := 5); `0x0054A470`, `0x0054A510` pass a built message | 0 | the builder copies u8@1–3, u32@4, u8@8–9, the name (≤ 15 chars + NUL, `0x004135D0`) at 10 and the text (length ≥ 256 → fatal 0x5BA) after the name's NUL. Form 5 (overhead, §7.9 r3): u8@2 = the overhead record's byte +8 (C→S 0x14 u8@2), u8@3 unit type, u32@4 GUID, empty name; bytes 8–9 never written. Form 6 (chat, open question 14): u8@2 0, u8@3 2, u32@4 = ESI, u8@8 0; byte 9 never written |
   | 0x27 | `0x0053C8D0` | §6 rule 6 | 7 | |
   | 0x2A | `0x0053D740` | callers pass DL 0x2A (e.g. `0x0057737A`) | 0 | 15 bytes from registers: kind u8@1 (4th stack arg), code u8@2 (1st), GUID u32@7 (3rd; −1 without a unit), gold u32@11 (2nd, e.g. stat 14 via `0x00625480`); bytes 3–6 never written (`tools/original-hooks.md` §6.2) |
   | 0x2C | `0x0053D780` | `0x00571740` (`0x00571775`) | 4 | unit type, GUID (+0x0C), event = unit u16 +0x6E; only when unit +0x70 is 0 or the client's player; callers `0x00580917`, `0x00581B07`, `0x00586000`, `0x00598369`; the event setter `0x00553380` (78 call sites by event, last event before the flush wins: u16 +0x6E overwritten) is owned by `audio/triggers-2.md` §14 |
   | 0x4C, 0x99 | `0x0053D530` | itself | 2, 0 | rule 5 |
   | 0x4D, 0x9A | `0x0053D4D0`, `0x0053D530` | itself | 0 | rule 5 |
   | 0x4E | `0x0053D7B0` | `0x00576770` (BL 0x4E, `0x0057686C`) | 0 | `world/npc.md` §7.2 |
   | 0x50 | `0x0053D7E0` | §6 rule 6 | 0 | words after the code: code 1 (`0x00546040`) u16@3, @5, @7, rest 0; codes 2, 0x24, 13 u16@3; code 4 u16@3–@11 (the quest record's five words − 0x11, `0x00593D10`); code 0x17 none |
   | 0x53 | `0x0053C900` | `0x0052D7B0` (`0x0052D800`), `0x0053ABE0` (`0x0053AC4F`), `0x0059A100` (`0x0059A12A`), `0x0059A170` (`0x0059A19C`) | 6 | `0x0053ABE0` takes the values from `0x0061C330(act)`; `render/lighting.md` §9.2 |
   | 0x58 | `0x0053D8D0` | `0x00579D60` (`0x00579F52`, `0x0057A28B`, `0x0057A4B3`: codes 6, 7), `0x00582610` (`0x005826D9`: 0), `0x005852E0` (`0x00585348`: 1, 4, 5), `0x0059DC70` (`0x0059DD54`: 0) | 0 | GUID u32@1 (−1 without a unit); u8@6 written only with code 5 (`0x005853CD` := 1, or `0x005853E4` := the result of `0x00585240`) |
   | 0x5D | `0x0053D710` | 18 call sites, `world/quests.md` §6.3 | 1 | |
   | 0x5A | `0x0053C850` | its callers build all 40 bytes: `0x00549A60` (code 0x0E, u8@2 1, rest 0; `skills/use.md` §2 step 6), `0x0054A5D0` (codes 0x0D, 4, §9 rule 16) | 0 | a copier: queues 40 bytes from the caller's buffer; asserts the name at @8 is shorter than 16 chars (fatal 0x5DA). Code u8@1, u8@2, u32@3, name @8 |
   | 0x60 | `0x0053D900` | itself (`0x0053D90D`); callers: the add messages `0x00571F90` (§7.2, call `0x00572082`) and the object update `0x00581A20` (call `0x00581A72`, `world/objects.md` §14) | 0 | 7 bytes: portal flags u8@1 = object data +0x05 (`0x006222C0`, fatal 0xFE4 / 0xFE5 for a null or non-object unit), destination level u8@2 = object data +0x04, GUID u32@3 = unit +0x0C; every byte written |
   | 0x63 | `0x0053D960` | `0x00584E30` (`0x00584EEA`) | 3 | `world/waypoints.md` §5.3 |
   | 0x78 | `0x0053CAD0` | `0x00568060` (`0x005682F7`) | 0 | the other player's client name (`0x00538830`, 16 bytes, byte 16 := 0), u32@17 = the other player's GUID; trade only |
   | 0x89 | `0x0053DFE0` | `0x005456F0` (`0x00545700`), `0x00546270` (`0x00546687`, event 0) | 0 | `world/quests.md` §6.5 |
   | 0x8A | `0x0053DFF0` | `0x00544590` (`0x005446EE`), `0x005EE3C0` (`0x005EE57B`) | 136 | type 1, GUID = unit +0x0C; `world/quests.md` §6.4 |
   | 0x91 | `0x0053E060` | `0x00545100` (`0x00545172`) | 0 | `world/quests.md` §6.7 |
   | 0x94 | `0x0053C5D0` | itself (`0x0053C65F`); called at `0x00532F03` (join) and `0x0056A7B7` | 4 | `client/msg-skills.md` §3 |
   | 0xA3 | `0x0053C0E0` | itself; record from `0x00571AA0` (`skills/bodies-2.md` §2.21) | 0 | 24 bytes, zeroed: v u8@1 (DL), skill u16@2, level u16@4, unit type u8@6, GUID u32@7, target type u8@0xB, target GUID u32@0xC, x u32@0x10, y u32@0x14; one caller `0x00571DEC` (§7.9 rule 2) |
   | 0xA5 | `0x0053C190` | itself | 0 | 8 bytes, zeroed first: unit type u8@1 (DL), GUID u32@2, skill u16@6 (stack); `skills/bodies-2.md` §2.13 |
   | 0xA8 | `0x0053E8D0` | `0x005711D0` (`0x00571359`) | 5 | rule 6 |
   | 0xAA | `0x0053E8D0` | `0x00570E30` | 269 | §7.9 rule 1 |
   | 0xAC | `0x0053E2E0` | itself; one caller `0x005720D8` | 0 | header GUID u32@1, class u16@5, x u16@7, y u16@9, life u8@11, size u8@12, bit stream @13: `monsters/init.md` §24 |

4.1. **Rows decided against PC 2** (2026-10-08; PC 2's
   `spec-answers-tick-messages` on `claude/specs-staging-4` gave other
   layouts; this table and the TSV are the owners). 0x2A and 0xAC: PC
   2's layouts match the builders (table above). 0x50: `0x0053D7E0`
   copies a 15-byte caller record; no caller writes bytes 13–14 (the
   widest, `0x00593CB0`, loops five u16 @3–@11, `0x00593D10`), so the
   TSV has code u16@1 and five u16 v0–v4 @3–@11 (PC 2's reading; the
   former u16@13 field is dropped; masks §6 rule 6). 0x58: kept as
   GUID u32@1, code u8@5, effect u8@6, because code 5 writes byte 6
   (`0x005853CD`, `0x005853E4`); PC 2's 7-byte form without @6 loses
   it. 0x63: kept as GUID u32@1, magic u16@5, bits u32@7, @11, @15,
   u16@19: `0x006610B0` copies the 16-byte waypoint record (u16 0x0102,
   then 14 bytes of bits) to @5; PC 2's single 16-byte field is the same
   bytes unsplit. C→S 0x3A (`client-messages.tsv`): kept as
   `repeat:u8@2`; `0x0054BD10` spends repeat + 1 points (stat < 16,
   repeat < 100, else result 3), the same quantity PC 2 calls
   `count_minus_one` (`combat/vitals.md` owns the spend).
   Field names: C→S 0x32 / 0x33 keep PC 1's (`transaction`,
   `client_price`, `item_mode`; the staging-5 code uses them; PC 2's
   `mode` / `tab` / `cost` are the same bytes, vendor meaning
   `world/vendors.md`); S→C 0x22 takes PC 2's `unit:u32@3
   body_state:u8@11` (`items/inventory-moves.md` §11).
   Rows with an empty `layout` that are complete (2026-10-08): the
   1-byte S→C messages 0x00, 0x02, 0x04, 0x05, 0x06, 0x4F, 0x97 and 0xB0
   carry only the id (builders `0x0053B320`, `0x0053B240`, `0x0053E110`,
   `0x0053B220` write nothing else); 0x7E (5 bytes) has no field either:
   `0x0053DB70` writes only the id and queues 5 bytes, so bytes 1–4 are
   stack leftovers (Edge case 10; masked in `tools/original-hooks.md`
   §6.2). An empty layout is the correct TSV value for all nine.
5. **0x4C / 0x4D / 0x99 / 0x9A.** `0x0053D530` (ECX client, DL unit
   type; stack: GUID, target type u8, target GUID, skill u16, w u16, b
   u8, flag): id base 0x4C (16 bytes) or 0x4D (17 bytes), + 0x4D when
   flag ≠ 0 (0x99, 0x9A). The target is looked up (`0x00552F60`). Found,
   and the client's player has no room or the target's room
   (`0x00620BB0`) is not in the list of the player's room
   (`0x00619790`) → 17-byte form: skill zero-extended to u32@6, b @10,
   x u16@11 / y u16@13 = the target's path target point (path +0x10 /
   +0x12, `0x00648A00` / `0x00648A10`, `sim/path-placement.md` path
   table), w @15. Else (not found, or in that list) → 16-byte form:
   skill u16@6, b @8, target type @9, target GUID u32@10, w @14.
   `0x0053D4D0` (DL type; stack: GUID, x u16, y u16, skill u32, w, b,
   flag) always writes the 17-byte form. Callers: `0x00548090` and
   `0x00597D70` (flag 0, w 0), `0x00581A20` (0x4D, type 2, point
   (0, 0)), `0x00571CD0` (flag 1, pending records, §7.9 rule 2).
6. **0xA7 / 0xA8 / 0xA9** `0x005711D0(unit, client)` (called at
   `0x00571592`): for each state s whose bit is set in the unit's
   state-change array (`0x00639F70`, copied; 32-bit words, ascending),
   s < the `states` count and the row's `nosend` bit clear: the unit
   has s (`0x00639DF0`) and its stat list (`0x006256B0`) has ≥ 1 entry
   (`0x00625C90`, ≤ 16) → 0xA8: type, GUID, state u8@7, bit stream @8 =
   the entries exactly as §7.9 rule 1 step 3 after its list bit, then
   0x1FF; size u8@6 = 8 + the stream's bytes. Has s, no list or no
   entry → 0xA7 (`0x0053E260`). Not set → 0xA9 (`0x0053E290`).
7. **0x1D / 0x1E / 0x1F choice** (2026-10-08; answers
   `docs/handoff/impl-server-join-2.md` §3 "0x0053BE40's choice").
   `0x0053BE40(client, stat s in DX, value v)`: s > 0xFE → fatal assert
   0x3CB. Then by v as an unsigned u32: v < 0xFF → **0x1D** [s u8@1][v
   u8@2] (3 bytes); else v < 0xFFFF → **0x1E** [s u8@1][v u16@2] (4
   bytes); else **0x1F** [s u8@1][v u32@2] (6 bytes). So 0xFF goes to
   0x1E, 0xFFFF to 0x1F, and a negative v (≥ 0x80000000 unsigned) to
   0x1F. The gold sender `0x0053E9B0(client, new n, old o)` first
   tests d = n − o: 1 ≤ d ≤ 0xFE (unsigned `d − 1 < 0xFE`) → **0x19**
   [d u8@1] (2 bytes); else the same three-way choice with s = 14
   (gold) and v = n.

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
     hosted game); S→C 0x75, 0x78, 0x79, 0x7F, 0x8B–0x8D, 0x90 (party,
     trade, relations): need a second player (Phase 7 multiplayer).
     S→C 0x77 is a `sim` row: its builder `0x0053CAB0` (2 bytes, code
     u8@1) has 41 call sites, among them the single-player cube paths
     (`world/cube.md` §1: codes 0x0C, 0x11, 0x15); the trade codes are
     the same message;
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
3. Nothing is ignored by default, except bytes no 1.14d builder writes
   (stack contents of the sender's frame), which are masked: S→C 0x2A
   bytes 3–6 (`0x0053D740`, `world/npc.md` §9), 0x58 byte 6 (`effect`)
   for every code except 5 (only `0x005852E0`'s code-5 path writes it:
   `0x005853CD` := 1 for object class 0x98, else `0x005853E4` := the
   return of `0x00585240`; keyed, rule 6), 0x50
   bytes 12–14 for kind 1 and bytes 13–14 for kind 4
   (`world/quests.md` §6.2, §10.6). Values that come from the clock
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
   - Two more keyed ids (§3.5 rule 4; no record yet): **0x26** form 5
     never writes bytes 8–9, form 6 never writes byte 9 (rows `0x26
     u8@1=0x05` 8 2, `0x26 u8@1=0x06` 9 1); **0x58** writes byte 6
     (`effect`) only with code 5, so byte 6 is masked for every code
     except 5 (rows `0x58 u8@5=0x00`, `=0x01`, `=0x04`, `=0x06`,
     `=0x07`, each 6 1: every code a 1.14d caller sends; a code-5 record
     is compared whole). Confirmed per form: the buffer is built on
     the caller's stack and only byte 5 is stored before the copy for
     result 1 (`0x0058547C`), 4 (`0x00585392`), codes 6 / 7 of
     `0x00579D60` (`0x00579FC6`, `0x0057A002`, `0x0057A0E2`,
     `0x0057A1B4`, `0x0057A2E6`, `0x0057A33B`, `0x0057A488`,
     `0x0057A54A`, `0x0057A63F`) and code 0 (`0x005826EF`,
     `0x0059DD6A`); code 5 stores byte 6 at `0x005853CD` / `0x005853E4`
     (PC 2 request, `world/objects-2.md` §16.3: the obelisk's form is
     one of these, byte 6 unwritten).

### 7. Unit update messages (`0x0053A500`) and room clean-up (`0x00553220`)

The owner of "the unit-update spec" named by `sim/pathing.md` §10 and
OQ7, `missiles/missiles.md` R2.4 and `items/inventory-moves.md` §6.3: which
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
| item | only with unit flag 0x10: mode 3 and unit flag 0x1000 → 0x9C action 2 (`0x0053EC90`), else 0x9C action 0 (`0x0053EC00`) (`items/inventory-moves.md` §6.3) | |
| other (5) | 0x09 (`0x0053BCD0`: type, GUID, class u8, x, y) | |

Part B by type: player → `0x005489F0`, then a corpse 0x74
(`0x0053DA40`) when `0x005541B0(unit)` and `0x00639DF0(unit, 7)` hold,
else `0x00534F80`;
`0x00570E30`, `0x005484B0`, `0x00571CD0`, `0x00571620`; monster → its
mode message (§7.4), then `0x00534F80` when `0x00639DF0(unit, 93)` holds or the
class is 291, 417 or 418; item → `0x0055BED0` (§7.3 rule 4); others nothing.

#### 7.3 Type updates

1. **Player** `0x00580860`: `sim/pathing.md` §10 rules 2–3 (0x15, 0x0F,
   0x10) and `items/inventory-moves.md` §6.1 rule 2 (item dispatcher, 0x47,
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
      `items/inventory-moves.md` §6.1 rule 3 for P); else a hireling class
      (`0x0063EE90`) whose owner (`0x0058F0D0`) is P →
      `0x00597890(game, unit, client, 0)`.
   6. Unit flag 0x400: `0x00571740` (S→C 0x2C, 8 bytes, `0x0053D780`:
      §3.5 table row 0x2C: unit type, GUID, event = unit u16 +0x6E;
      only when unit +0x70 is 0 or the client's player).
   7. Unit flag 0x8000 (hit): S→C **0x0C** (`0x00597CF0(unit, client)`
      → `0x0053B430`, 9 bytes): u8@1 = 1 (monster), u32@2 = GUID (+0x0C),
      u8@6 = 0x13, u8@7 = unit +0xB0, u8@8 = h where p :=
      `0x005A5650(unit)` (life in 128ths, §7.4 rule 5) is first stored
      as stat 352 `last_sent_hp_pct` (`0x00627260(unit, 0x160, p, 0)`),
      h := p − 1 when p > 1, else p, | 0x80 when monster data +0x16 has
      0x100 (`0x005A0180(unit, 0x100)`).
   8. announced = 0 and `0x00639F20(unit)`: the unit's stat list is
      extended (list +0x10 bit 31) and one of its W state-change words
      (`sim/stat-lists.md` §9.1, the second W words at +0x58) is non-zero
      → `0x00571580(unit, client, 0)` (`0x00570E30`, then
      `0x005711D0`: the 0xA7 / 0xA8 / 0xA9 of §3.5 rule 6).
   9. `0x00625A20(unit)`: the unit's stat list has flag 0x100 (list
      +0x10, `sim/stat-lists.md` §2: set from the overlay list) →
      `0x005715A0(unit, client)`: the unit's list with flag 0x80
      (`0x00625760`; none → nothing); v := its stat 178
      `unit_dooverlay` (`0x00625A50`, layer 0); 0 ≤ v ≤ the overlay
      count (data tables +0xBC0; inclusive, so v = count is sent) →
      S→C **0x11** (`0x0053D850`, 8 bytes): type u8@1 = unit +0x00, GUID
      u32@2, overlay u16@6 = v.
   10. Unit flag 0x800: `0x00597C70(unit, client)`: only when monster
      data +0x5C has bit 1 (`0x00573540(unit, 1)`) and the unit has
      monster data → S→C **0x57** (`0x0053D880`, 14 bytes): GUID u32@1,
      type u8@5 = 1, u16@6 = the name seed (monster data +0x14,
      `0x005A0140`), u16@8 = umod list bytes 0 | 1 << 8, u16@10 = byte
      2 (monster data +0x1C; `monsters/umod-callbacks.md` §28.4), u16@12
      = 1 when monster data +0x16 has bit 4 (`0x005A0180(unit, 4)`),
      else 0.
3. **Object** `0x00581AD0`: unit flag 0x1 → `0x00581A20` (0x0E
   `0x0053B470`; portal rows → 0x60; a mode-1 case → 0x4D with type 2);
   flag 0x400 → `0x00571740`; flag 0x100 → `0x00571620`; flag-ex 0x1 →
   `0x00597890(game, unit, client, 0)`; always `0x00571CD0`.
4. **Item** `0x0055BF30`: only with unit flag 0x1 → `0x0055BED0`: an
   item in mode 3 without unit flag 0x10 → 0x9C action 2 when unit flag
   0x1000, else action 3 (`items/inventory-moves.md` §6.3 part 2).

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
   PROVISIONAL: the level byte b is the used skill entry's base + bonus
   level (+0x28 + +0x2C), as a byte (because the rule names only
   "level"); settled by REC-94.
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
5. The update-list reset `0x00597B00` (`items/inventory-moves.md` §6.1 rule
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
5. **0x65 kill count** (the only builder call is `0x0053FB30` →
   `0x0053D9C0`; layout in the TSV). The kill `0x0057CCB0` calls the
   arena event `0x0053F720(game, killer, victim)` at `0x0057CD5B`. A
   player killer of a monster (branch `0x0053FA11`): killer's arena
   record (player data +0x34) +0x00 += the game's `arena` row
   `MonsterKill` (+0x0C; `0x00664AB0(arena type)`), record +0x04 := 1,
   arena flags (game +0x1D28 → +0x08) |= 0x400, the victim queued for
   update (`0x0064C040`). The other branches (`PlayerKill`, `PlayerKillPercent`,
   `Suicide`, `PlayerDeath`, `PlayerDeathPercent`, `MonsterDeath`, and
   `0x0053F630`) set the same two flags. Then in the same tick, client
   pass (`sim/tick.md` §6 rule 5): per-client update `0x005380D0` → arena
   sync `0x0053FC20` (at `0x005381A4`, after the unit update and stat
   messages): flag 0x400 and the client's player record +0x04 ≠ 0
   (`0x0053FB00`) → 0x65 for that player (`0x0053FB30`); then, when
   `0x005388C0(client)`, `0x0053FB90(game, client, 0)`: 0x65 for each
   other in-game player whose record +0x04 ≠ 0. Tick step 6 clears 0x400
   (`0x0053FAE0` at `0x0053B079`), so one 0x65 per kill tick; record
   +0x04 is never cleared. `arena.txt` row `Deathmatch` has
   `MonsterKill` 1: the recorded counts 1, then 2. The join's 0x65
   (`0x0053FC70`, §8.3) is the other path; `0x00538860`, its second
   caller, has no reference in `Game.exe` (dead code).

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
   **Writers** (the only stores to unit +0xEC besides the clears): nine
   functions (ECX unit, EDX first value) allocate a zeroed record,
   write link +0x00 := 0 and the id byte +0x04, append it (head +0xEC
   when empty, else the tail's link; tail +0xF0 := it) and call
   `0x0064C040(unit)`:

   | Writer | Id | Record bytes | Callers |
   |---|---|---|---|
   | `0x005717C0` | 0x99 | 0x14 | 1 |
   | `0x00571840` | 0x9A | 0x14 | 1 |
   | `0x005718C0` | 0x9E | 0x14 | 16 |
   | `0x00571960` | 0xA1 | 0x18 | none (never runs) |
   | `0x00571A10` | 0xAB | 0x14 | 5 (`0x00580610`, `0x005A6920`, `0x005BE3F0`, `0x005BE7B0`, `0x005BEAC0`) |
   | `0x00571AA0` | 0xA3 | 0x28 | 5 |
   | `0x00571B70` | 0xA5 | 0x14 | 10 |
   | `0x00571C00` | 0xA4 | 0x0C (u16 class @8 := EDX) | 2 (`0x005EF320`) |
   | `0x00571C60` | 0x23 | 0x10 | 2 |
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

1. The arena record (`0x0053F4B0(game, flags, template)` at
   `0x00530D75`, game +0x1D28): 16 bytes from the game's pool; +0x00,
   +0x04 := 0; +0x08 := 0x67's u32@0x27 & 0x3179C7; +0x0C (u32) := 0,
   then its low byte := 0x67's u8@0x13. `0x0053FD40(game)` returns
   +0x08 (no record → fatal 0x139); `0x0053FCE0` its bit 1. Then the
   client record is allocated and prepended (`0x00539A30` at
   `0x00530D9F`, `sim/unit-order.md` §7), and after it the party
   record (`0x0053FF90` at `0x00530DC3`, game +0x1D2C: 8 bytes, u16
   +0x00 := 3; already present → fatal 0x19). Corrected 2026-10-08:
   `0x0053FF90` was named as the arena record.
2. The seed derivations of `sim/rng.md` §5.2 (`0x00547D20`;
   `0x00546C60`, its result stored in game +0x80; `0x00536070`;
   `0x00545D80`), then `0x0052C110`.
3. **S→C 0x01** (`0x0053B340(client, 1, game)`, 8 bytes): u8@1 = game
   +0x6D (difficulty); u32@2 = the arena record's flags (game +0x1D28 →
   +0x08, `0x0053FD40`; = the client's flags & 0x3179C7, rule 1;
   recorded 0x00100004 in every join of both recordings, the client
   default of `client/model.md` §7 rule 9); u8@6 = 1 when game +0x70 ≠ 0 (expansion), else 0; u8@7
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
      Static call order of the save path (`0x005344B0` → `0x005343A0` →
      `0x00534330`, which sends versions ≥ 0x5C, every 1.14d save (0x60),
      to loader `0x0056B180` (`0x00534387`) and older ones to the legacy
      `0x00534020` (`0x0053437B`, `formats/d2s.md` §1 rule 6); then
      `0x00546270`), which is the recorded order (sections:
      `formats/d2s-load.md` §2): (a) header `0x0056A090` (`0x0056B1BB`):
      player creation, then `0x00571F90` (`0x0056A24F`): 0x59, 0xAA,
      0x76; (b) skills `0x0056A710` (`0x0056B2AF`) → 0x94
      (`0x0053C5D0` at `0x0056A7B7`); (c) player items `0x0056A7E0`
      (`0x0056B2D3`) → `0x005337F0` (`0x0056A802`) → … → `0x0055C110`:
      0x22 (`0x0055C216`) and, through `0x00570080`, 0x21
      (`0x0057017B`); the corpse (`0x0056A830`, `0x0056B2F6`) and, in an
      expansion game only, the hireling's items (`0x0056AC10`,
      `0x0056B32D`, with inventory refresh `0x0055DF00` at `0x0056ACA1`
      on the hireling) run the same item reader; (d) post-load
      `0x0056AF80` (`0x0056B3C3`) selects the mouse skills through
      `0x005701B0` (`0x0056B0DF`, `0x0056B0F4`; 0x23 at `0x0057026C`,
      conditional): the recorded single 0x23; (e) after the loader,
      `0x00546270` (`0x005344EF`, mode 0): 0x5E (`0x005465FE`), 0x28
      (`0x0054662D`), 0x29 (`0x00544520` at `0x00544578`). 0x22 and
      0x21 are reachable only through the item calls of (c); their
      per-item conditions belong to the item and save-load specs.
      **New character (stub) load**: when the header returns 2 (status
      bit 0, `formats/d2s.md` §9 rule 1), `0x0056B180` calls
      `0x00569F80` (`0x0056B1E2`) instead of (b)–(d)
      (`formats/d2s-load.md` §1): `0x00571F90` (`0x00569FC8`: 0x59,
      0xAA, 0x76), start stats and start items, then, when the class's
      `StartSkill` applies, one extra **S→C 0x23** from `0x005701B0(P,
      hand 0, StartSkill, −1)` (`0x0056A05A`; item = the skill entry's
      owner item +0x34 from `0x00643B00` when not −1, else −1 for a class
      skill), then `0x00546270` mode 1
      (`0x0056A072`) before the caller's mode-0 call. Nothing on this path
      writes the record's item fields +0x78 / +0x7C, so the join's two 0x23 (rule
      3.7) carry item 0, not −1 (`formats/d2s-load.md` §8 rule 3; static
      reading, no new-character join recorded).
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
      Slots come only from the save or `0x005390A0` (the sole direct
      writer of +0x3DC). PROVISIONAL: a brand-new character's client
      record holds skill −1 in all 16 slots, so no 0x7B is sent
      (because a d2s stores unbound slots as 0xFFFF and the recorded
      join of a character with no hot key sends none; a zero-filled
      record would instead send 16 × 0x7B with skill 0); settled by
      REC-02 (count of S→C 0x7B in its new-character runs).
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
7. **A new character in single player** (2026-10-08). The join's load
   flag is 1 and there are no host callbacks, so `0x005345A0` always
   takes the save path; `0x00532590` is not reached from a
   single-player join (its callers: `0x005345A0` with flag 0, i.e.
   host or the dead act re-creations, and the legacy parser,
   `formats/d2s-legacy.md` §2 rule 6). A new character is the client's
   335-byte stub save (`formats/d2s.md` §9 rule 1), loaded by the stub
   branch of rule 3.1 (`0x00569F80`, `formats/d2s-load.md` §1).
   `0x00532590` does the same steps (`0x00532520` instead of
   `0x00569F20`): client +0x3D4 |= 1, the player allocated in no room,
   the add messages (`0x00571F90`), start stats for the client act
   (`0x005706D0`), start items (`0x00534F10`), player data +0x70 / +0x74
   := 0, the `StartSkill` right skill (0x23 from `0x005701B0(P, 0,
   skill, −1)`) when the act byte is 0 and the unit has the skill, the
   quest entry mode 1 (`0x00546270`); result 0. Both allocate the
   player with its player data record (`0x00555230`), so
   `0x006221A0(P)` is never null and rules 3.2–3.10 run unchanged:
   **S→C 0x5F** (rule 3.3) and the two **S→C 0x23** of rule 3.7 (item
   0, rule 3.1) are sent for a new character as for a loaded one. A
   loader that sends neither (d2rs `Character::New`) does not match
   1.14d; it is the stub path with the class's start stats, items and
   `StartSkill`.

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

**The join 0x5A** (`0x0052D63D`–`0x0052D6F2`, after the host callback,
which needs host callbacks and is skipped in single player): a 40-byte
message, all bytes zero first: u8@0 = 0x5A, code u8@1 = 2 (player
joined), u8@2 = 4, u32@3 = 0, u8@7 = 0; the account name (client +0x1D,
`0x005392F0`) is copied to a local buffer and, when not empty, its
first 16 bytes to @0x18 with byte 0x27 := 0; the character name
(client +0x0D, 16 bytes) to @8. Sent through `0x0054AA40(game, msg)`:
only when a NUL is within bytes 8–23 (the character name shorter than
16), then `0x0052DED0(game, 0x0054AA30)` sends it to every client of
the game in state 4 (client-list order), the joiner included (its
state became 4 just before, `0x0052D5AE`). Single player has no account name, so @0x18–@0x27
are zero. Recorded seq 224: `5a 02 04 00000000 00` + "werwer" padded
with zeros to 40 bytes. The leave 0x5A (code 3) is §2.5 rule 2.

The first 0x48 is the per-client update's inventory refresh: in
`0x005380D0` the order is removals (`0x0053A770`), unit updates
(`0x0053A620`, where the player update `0x00580860` could send 0x48 at
`0x005808D9`), stat messages (`0x006258D0`: the 0x1D / 0x1E), then,
when the player's flag-ex (+0xC8) has bit 0x200000 (set by §8.2 rule
3.5), `0x0055DF00(…, 1, 1)` at `0x00538146` → `0x0055DBC0` → 0x48 at
`0x0055DEEB`. A 0x48 after the stat messages can only come from there.
The second 0x48 (after 0x04) is the state-3 inventory refresh
`0x0055DF00` (`sim/tick.md` §6 rule 4), the same function. The only
two 0x48 senders (`0x0053D3C0` call sites) are `0x0055DEEB` and
`0x005808D9`.

### 9. C→S handlers: owners, and the small handlers owned here

Handler entry (size, gate, result codes) is §2.3–2.4 for every id; this
section names who owns what each `sim` id **does** when no system spec
owned it yet, and states the handlers that are only message handling.
"Player" = the dispatcher's EDX unit, "game" = ECX.

1. **Owners.** Ids whose behaviour lives in another spec link there;
   rules 2–13 own the rest.

   | Id | Owner of the behaviour |
   |---|---|
   | 0x12 EndInferno | rule 2 |
   | 0x14 OverheadChat | rule 3 |
   | 0x15 Chat | §2.4 rule 6 (checks); relay: rule 16 |
   | 0x3D HighlightDoor | rule 4 (message part); `0x005845D0`: open question 15 |
   | 0x3E ActivateInifussScroll | `world/quests.md` §9.4 |
   | 0x3F PlayAudio | rule 5; the event's sound: `audio/triggers.md` §3 rule 3 |
   | 0x41 Resurrect | rule 6 |
   | 0x44 StaffInOrifice | `world/quests-act2.md` §8.6 (entry: rule 7) |
   | 0x4F ClickButton | rule 15 (entry); the buttons: `0x00568060` (trade and UI buttons, `ui/panels.md`) |
   | 0x46 MercInteract, 0x47 MoveMerc | rule 8; the command: `monsters/ai.md` (commands `0x005E6AE0`) |
   | 0x48 TurnOffBusyState | rule 9 |
   | 0x4B RequestEntityUpdate | rule 10 |
   | 0x4C Transmogrify | `world/cube.md` §10 (item-use spec) |
   | 0x4D PlayNpcMessage | rule 11; the intro record: `world/quests.md` §6.7 |
   | 0x51 BindHotkey | rule 12 (fields: §2.4 rule 7) |
   | 0x53 StaminaOn, 0x54 StaminaOff | rule 13 |
   | 0x59 MakeEntityMove | `monsters/ai-bodies.md` §9.9 (AI params from NPC messages); client sender: the interact code `0x00461DC0` (`0x004620F1`, through builder `0x00478700`) sends [type][GUID][x][y] with the NPC's current position when the clicked monster's `monstats` has `npc` and `interact` (`ui/controls.md` §6 rule 9) |
   | 0x5F UpdatePlayerPos | `sim/pathing.md` §1.6 |
   | 0x60 SwapWeapons | rule 14 (message part); `0x005616A0`: open question 16 |

2. **0x12** (`0x0054A260`): size 1 else 3; state 12 (`inferno`) off on
   the player with an update-queue insert (`0x00639DB0(player, 12, 0)`,
   `sim/stat-lists.md` §9.2); 0.
3. **0x14** (`0x0054A290`): size 4–275 else 3 (§2.4 rule 1); text =
   cstr @3, length n (`0x00413750`): n ≤ 0 → 0; n ≥ 256 → 2; the name
   cstr after it is copied (≤ 16 bytes) and not used. The text test
   `0x00413490(text, −1)` ≠ 0 → 0, nothing else. Else: free the
   player's overhead record (unit +0xA4, `0x006611A0`), make a new one
   (`0x00661110(game +0x1C, text, frame)`: duration d = 8·min(n, 254) +
   0x7D frames, end = frame + d), its byte +8 := message u8@2
   (`0x00661230`), unit +0xA4 := it, queue the player for update
   (`0x0064C040`), unit flags +0xC4 |= 0x100, and schedule timer event
   6 on the player at max(frame + 1, end) (`0x005417D0`, `sim/tick.md`
   §5); 0. The update pass sends it (overhead 0x26, §7.9).
4. **0x3D** (`0x0054BF10`): size 5 else 3; object GUID u32@1; the unit
   test `0x00548F80` (§2.4 rule 4) with type 2 and range 10: non-zero →
   that code; else `0x005845D0(game, player, GUID)`, 0.
5. **0x3F** (`0x0054C070`): size 3 else 3; sound u16@1 outside 25–32 →
   0, nothing; player unit flags (+0xC4) bit 0x400 → 1; else sound
   event (`0x00553380(player, sound, 0)`); 0.
6. **0x41** (`0x0054C0E0`, call level; gate: §2.3 rule 3 "dead"): size 1
   else 3; player missing or mode ≠ 0x11 → 0. Client flag 4
   (`0x00538670`, hardcore) → drop the client (`0x0052CAF0(game,
   client, 3)`, `tools/original-hooks.md` §6.1 rule 3), 0. Else in
   order: `0x0053FDF0` (returns 0, no other effect: its whole body is
   `return 0`; its value is the warp's last
   argument); passive skill states re-applied (`0x0056DFA0`: each skill
   of the player with a passive state gets the state on and
   `0x00646D60`); stats 6, 8, 10 (life, mana, stamina) := their maxima
   (`0x00625D10`, `0x00625D60`, `0x00625DB0`; set `0x00627260`, each
   sent with `0x00548520`); unit flags +0xC4 |= 2; state 0x36 on then
   off (`0x00639DB0`); level warp `0x0053AEC0(game, player, town of the
   act of the player's current level, 0)` (act `0x006427F0`, town
   `0x0061AB70`: `drlg/levels.md` §6 rule 3; warp
   `sim/path-placement.md` §11); mode 1 started without the mode gate
   (`0x005809D0(game, player, target none, mode 1, 0, 0, skip gate
   1)`); the left skill (`0x00620190`) re-selected with EDX = 1 and the
   right skill (`0x006201D0`) with EDX = 0 (`0x005701B0`, as in
   `items/inventory.md` §5.5); 0. What the life / corpse rules of a player
   death are belongs to the player-death spec (not written).
7. **0x44** (`0x0054C380`): size 17 else 3; busy (`0x00535060`) and
   trading (`0x005678A0(…, 1)`) → 3; else `0x00549520(game, player,
   u32@5, u32@9, u16@13)` and its result (`world/quests-act2.md` §8.6).
   u16@13 is an action code (`0x00549520`): the orifice unit test
   (`0x00548F80`, type 2, range 50) non-zero → that code; action 2 →
   0 and `0x005852E0` runs; action 3 → the cursor-item check
   `0x005490E0`, and `0x005852E0` runs when it returns 0; any other
   action → 2, nothing runs.
8. **0x46** (`0x0054C4B0`): size 13 else 3; merc GUID u32@1, target
   GUID u32@5, target type u32@9 (≥ 6 → 2); the unit test `0x00548F80`
   (range 50) non-zero → that code. **0x47** (`0x0054C520`): size 13
   else 3; merc GUID u32@1, x u16@5, y u16@9; the point test
   `0x00548EF0` (range 50, §2.4 rule 3) non-zero → that code. Both then
   `0x0054C430(merc, a, b, command)` (0x46: a = target GUID, b = type,
   command 0x0C; 0x47: a = x, b = y, command 0x0D): no monster with the
   merc GUID → 1; it is not the player's hireling (`0x00574EC0(game,
   player, 7, 0)`) → 1; else free its AI commands (`0x0058EDE0`), add
   the command {command, a, b} (`0x0058EF40`; it copies five dwords,
   record +0x08..+0x18, from a 0x1C-byte stack record of which
   `0x0054C430` writes only +0x08 command, +0x0C a, +0x10 b: params 3
   and 4, +0x14 / +0x18, are uninitialized stack bytes; d2rs stores 0,
   0), sound event 15 on the
   merc toward the player (`0x00553380`), 0. 0x47 returns 0 whatever
   `0x0054C430` returns; 0x46 returns its result.
9. **0x48** (`0x0054C590`): size 1 else 3; player data +0x4C (busy,
   `items/inventory.md` §7) = 0 → 2; else +0x4C := 0 (`0x005350B0`); 0.
10. **0x4B** (`0x0054C6D0`): size 9 else 3; type u32@1 ≥ 6 → 2; unit :=
    (type, GUID u32@5) (`0x00552F60`); type 0 and unit ≠ the player → 3;
    unit missing → 1; the player's room not in the unit's room's room
    list (`0x0065A590` → `0x00619790`) → 1; else queue the unit for
    update (`0x0064C040`) and set its flag-ex (+0xC8) bit 0x10000, which
    makes its next update send S→C 0x15 (§7.3 rule 2 step 1 for
    monsters; `sim/pathing.md` §10 for players); 0.
11. **0x4D** (`0x0054C690`): size 3 else 3; u16@1 = an NPC class id; ≥
    the monstats row count (data tables +0xA80) → 2; else clear that
    NPC's intro bit in the player's record for the game's difficulty
    (`0x005724C0`; class not in the intro list → bit 0); 0.
12. **0x51** (`0x0054C870`): size 9 else 3; fields as §2.4 rule 7; slot
    > 15 → 3; skill > count → −1 (unbind); skill = count → 3; 0 ≤ skill
    < count and the player lacks it (`0x006439B0(player, skill, item)`
    = 0) → 3. Store into the client's hot-key slot (`0x005356B0` →
    `0x005390A0`: client +0x3DC + 8·slot: skill i16, left u8 at +2, item
    u32 at +4; read by the join's 0x7B, §8.2 rule 3.6); no client → 3;
    else 0. Nothing is sent.
13. **0x53** (`0x0054C940`): size 1 else 3; a player in mode 2 (walk)
    with stamina (stat 10, `0x00625480`) ≠ 0 → mode 3 (run,
    `0x00624690`). **0x54** (`0x0054C990`): size 1 else 3; a player in
    mode 3 → mode 2. Both 0.
14. **0x60** (`0x0054CE70`): size 1 else 3; classic game → 3; a used
    skill (`0x00620250`) or dead (`0x005541B0`) → 0; else `0x005616A0(game,
    player, &fail)`: 0, or 3 when it returns 0 with fail ≠ 0.
15. **0x4F** (`0x0054C7C0`): size 7 else 3; `0x00568060(game, player,
    button u16@1, (p1 u16@3 << 16) | p2 u16@5)` and its result: the
    two words reach the button handler as one u32, p1 high.
16. **0x15 relay** (`0x0054A5D0`, after §2.4 rule 6): u8@1 is copied
    and never read. Target t = the cstr after the text (≤ 15 chars
    kept). The text test `0x00413490(text, −1)` ≠ 0 → 0, nothing sent.
    The line: 0x26 with u8@1 = 2 (whisper) when 1 ≤ strlen(t) ≤ 15,
    else 1 (broadcast); u8@2 = message u8@2 (lang); u8@3 = 2; u32@4 =
    0; u8@8 = 0; u8@9 = the sender's stat 12 (level, `0x00625480`);
    name @10 = the sender's client name (`0x00538830`); then the text.
    Every client c of the game is visited (`0x0052DED0`) with s = the
    sender's player unit (type 0, GUID unit +0x0C; none if the sender
    is not a player) and p = c's player:
    - broadcast (`0x0054A470`): sent to c unless s and p both exist,
      s ≠ p and one of the relation tests holds (`0x0055B300(p, s, 4)`,
      `0x0055B300(s, p, 2)`, `0x0054A420(game, s, p)`,
      `0x0054A420(game, p, s)`);
    - whisper (`0x0054A510`): only a c whose client name equals t
      (`0x00413590`); s, p exist and `0x0055B300(p, s, 4)` or
      `0x0055B300(s, p, 2)` → not sent, flag `[0x008846E0]` := 1;
      else sent, flag `[0x008846E4]` := 1.
    A whisper then answers the sender (both flags cleared before the
    visit): `[0x008846E0]` set → S→C 0x5A code 0x0D (`0x0053C850`, name
    @8 = t); else `[0x008846E4]` clear (no client named t) → 0x5A code
    4 with t; else 0x26 form 6 to the sender (`0x0053C750`: u8@1 6,
    u8@2 0, u8@3 2, u32@4 0, u8@8 0, name @10 = t, the text). 0.
    Recorded (`pc2rec-p1-packets`, `pc2rec-p2-packets`, single player,
    9 lines, all with an empty t, §2.1 rule 8): each 0x15 gets exactly
    one S→C 0x26 in the same input phase, from `0x0053C82F`, form 1:
    `26 01 00 02 00000000 00 1e "TestSor" 00 <text> 00` (level 30 at
    u8@9, the sender's name), and no 0x5A. The whisper branch is not
    reachable from the single-player client (it never sends a
    non-empty t).

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
   changes the size. A negative size (e.g. `15 01 00 'hi' 00 'bob' 00
   80` → −117) passes the rule's "needs ≥ size" test and is rejected by
   the classifier with result 4 (§2.1 rule 4), dropped silently in local
   mode.
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
13. **0x3E padding** (2026-10-08). The builder `0x0053D130` writes the
    item stream into a 0x20-byte area, sets size u8@1 = stream length L
    + 2, but always queues 0x22 bytes (`push 0x22` at `0x0053D1F5`). The
    0x20 − L bytes after the stream are 0, so the client's split (§3.4
    rule 3, size rule u8@1, `server-messages.tsv`) reads them as that
    many 1-byte S→C 0x00 messages, whose handler `0x0045C900` is empty
    (`client/model.md` §7 rule 1). d2rs reproduces the padding bytes
    exactly (they are flushed bytes, §6), and the client counts them as
    0x00 messages with no effect.

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
| unit update: monster mode 0 (flag 0x1), no target, path target (4757, 5461) (the (a, b) of §7.4 rule 7, not the unit's cell; the next row's mode 12 sends the cell (4756, 5461)), d 0x38, +0xB0 6, GUID 0x13 | `69 13000000 08 9512 5515 38 06` | §7.4; recorded `-015956` frame 2724 |
| same monster later in mode 12, its cell (4756, 5461) | `69 13000000 09 9412 5515 38 00` | §7.4; recorded frame 2748 |
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
| changed state 105 on player 1, list {172: 2} | `a8 00 01000000 0b 69 ac fc 0f` (stat 9 bits 172, value 2 bits 2, 0x1FF) | §3.5 rule 6; recorded `-015956` seq 234 |
| changed state 100, list entries all with send bits 0 | `a8 00 01000000 0a 64 ff 01` (only 0x1FF) | §3.5 rule 6; recorded `-022633` seq 12320 |
| monster 0x32 uses skill 181 on monster 0x34 in view, b 1 | `4c 01 32000000 b500 01 01 34000000 0000` | §3.5 rule 5; recorded `-022633` seq 52921 |
| same with the target out of the player's rooms, path target (x, y) | `4d 01 32000000 b5000000 01 <x u16> <y u16> 0000` | §3.5 rule 5 (synthetic) |
| same, pending record (flag 1) | id 0x99 (in view) / 0x9A | §3.5 rule 5 (synthetic) |
| sound event 18 on monster 0x26; event 2 on player 1 | `2c 01 26000000 1200`; `2c 00 01000000 0200` | §3.5; recorded `-015956` seq 293505, `-022633` seq 46356 |
| act environment period 2, ticks 0x880, no eclipse | `53 02000000 80080000 00` | §3.5; recorded `-015956` seq 170841 |
| waypoint 10, record magic 0x0102, indexes 0 and 1 known | `63 0a000000 0201 03000000 00000000 00000000 0000` | §3.5; recorded `-022633` seq 7780 |
| NPC 7 wants to talk | `8a 01 07000000` | §3.5; recorded `-022633` seq 1563 |
| NPC text list of monster 0x10: strings 0x40, 0x0B, kind 0 | `27 01 10000000 02 00 00 00 4000 00 00 0b00` + 24 zero bytes | §6 rule 6; recorded `-015956` |
| skill list of player 1, 8 entries | `94 08 01000000` then `0000 01`, `0200 01`, `0100 01`, `d900 01`, `da00 01`, `db00 01`, `dc00 01`, `0300 01` | §3.5; recorded `-022633` seq 105 |
| game creation, Normal, expansion, not ladder, arena flags 0x00100004 | `01 00 04001000 01 00`, then `00`, then `02` | §8.1; recorded seq 5–7 of both |
| hot-key slot 3, skill 36, flag set, item −1 | `7b 03 2480 ffffffff` | §8.2 rule 3.6 (synthetic) |
| join of a character with no hot key, then the town spawn | 0x59 … 0x0B, 0x5F, …, 0x95, 0x1B, 0x03, 0x53, 0x07 × (1 + array), 0x15, 0x7E; next tick … 0x04 | §8; recorded `-022633` seq 102–219 |
| one-entry 0x27 (count 1) vs d2rs, differing only in bytes 7, 9, 12–39 | match | §6 rule 6 (synthetic) |
| 0x27 count 2 differing in byte 12 (second entry's kind) | mismatch at byte 12 | §6 rule 6 (synthetic) |
| 0x50 u16 1 differing in byte 13 | mismatch (no mask) | §6 rule 6 (synthetic) |
| 0x82 name "ab" (NUL at byte 7), differing in bytes 8–20 | match; differing in byte 21 → mismatch | §6 rule 6 (synthetic) |
| player kills a monster in a Deathmatch game, arena record +0x00 = 0 | that tick: 0x65 `65 <player GUID> 0100` after the monster's 0x69, from the per-client update; next kill `…0200` | §7.6 rule 5; recorded counts 1, 2 |
| C→S `48` with player data +0x4C = 0 | result 2, nothing | §9 rule 9 (synthetic) |
| C→S `54` with the player in mode 3 | mode 2; result 0 | §9 rule 13 (synthetic) |
| C→S `51 24 00 03 00 ff ff ff ff` (skill 36 the player has, right hand, slot 3) | client +0x3DC + 24: skill 36, left 0, item −1; result 0; nothing sent | §9 rule 12 (synthetic) |
| C→S `51 24 00 10 00 ff ff ff ff` (slot 16) | result 3 | §9 rule 12 (synthetic) |
| C→S `3f 1d 00` (sound 29), player flags without 0x400 | sound event 29 on the player; result 0 | §9 rule 5 (synthetic) |
| C→S `3f 18 00` (sound 24) | result 0, nothing | §9 rule 5 (synthetic) |
| C→S `4b 00000000 <other player GUID>` | result 3 | §9 rule 10 (synthetic) |

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
  settles them. Settled 2026-10-07 (§3.5): the id store before every call
  of each copier (`lea edx, [ebp − n]` and the byte store at ebp − n in
  the same function), the DL set before every call of the shared
  builders, and the `caller` field of every record in both recordings
  (ids 0x27, 0x2C, 0x4C, 0x53, 0x5D, 0x63, 0x8A, 0x94, 0xA8, 0xAA: 437
  records, all in the listed sender); layouts from the stores read in
  the id writers and the builders.
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

Senders session (2026-10-07, static reads with `disasm.py` and a call
graph of `all.asm`): masks (§6 rule 6) `0x0053C8D0` and its four call
sites, `0x00661480`, `0x00661240`, `0x006612F0`, `0x005456A0`,
`0x005DE330`, `0x00545780`, `0x00572C10`; `0x0053D7E0` call sites and
`0x00546040`; `0x0053DB90`, `0x004135D0`. 0x77 call sites of
`0x0053CAB0` (§4 rule 4). 0x65 (§7.6 rule 5): `0x0057CD5B`,
`0x0053F720`, `0x0053F630`, `0x0053FAE0` (caller `0x0053B079`),
`0x0053FB00`, `0x00538860` (no reference: byte search of `Game.exe` for
its address finds none), `arena.txt`. Loader order (§8.2 rule 3.1) and
first 0x48 (§8.3): `0x00534020`, `0x005344B0`, `0x00532EB7` (the tail of
`0x00532E90`), `0x00533C70`, `0x0055DA70`, `0x005380D0`, `0x0055DBC0`;
reverse call search from `0x0053C520` / `0x0053C4A0`.
Handlers of §9: `0x0054A260`, `0x0054A290` (with `0x00661110`,
`0x00661230`), `0x0054BF10`, `0x0054C070`, `0x0054C0E0` (with
`0x0053FDF0`, `0x0056DFA0`, `0x00625D10`, `0x005809D0`, `0x006427F0`,
`0x0061AB70`), `0x0054C380`, `0x0054C4B0`, `0x0054C520`, `0x0054C430`,
`0x0054C590` / `0x005350B0`, `0x0054C6D0` / `0x0065A590`, `0x0054C690` /
`0x005724C0`, `0x0054C870` / `0x005356B0` / `0x005390A0`, `0x0054C940`,
`0x0054C990`, `0x0054CE70`, `0x0054BF60`, `0x0054C760`, `0x0054CA10`.

2026-10-08 (pc1-s7, asm): §2.5 rules 1–6: `0x0053F100` (cases
`0x0053F130`–`0x0053F384`), `0x0052C330`, `0x0053EFC0`, `0x00538B70`,
`0x00538C60`, `0x004112C0`, `0x00413590`, `0x0052C8E0`, `0x00538930`,
`0x005303D0`, `0x00539030`, `0x005392F0`, `0x00538830`, `0x0052CA10`,
`0x0052E8E0`, `0x0053B1B0`, `0x0052DB10`, `0x00538CE0`, `0x0052CBE0`,
`0x00530270`, `0x0052CAF0`, `0x00537760`, `0x0052D350` (+0x504 reader).
§8.1 rule 1: `0x00530BF0` (`0x00530CF6`–`0x00530DC3`), `0x0053F4B0`,
`0x0053FD40`, `0x0053FCE0`, `0x0053FF90`. §8.2 rule 7: `0x00532590`,
`0x005398E8`–`0x005398FF` (0x5F unconditional). §8.3 join 0x5A:
`0x0052D440` (`0x0052D5CC`–`0x0052D6F2`), `0x0054AA40`, `0x0052DED0`;
recorded seq 224. §7.3 rule 2 steps 6–10: `0x00597CF0`, `0x0053B430`,
`0x00639F20`, `0x00625BE0`, `0x00625A20`, `0x005715A0`, `0x0053D850`,
`0x00597C70`, `0x0053D880`, `0x00573540`, `0x005A0120`, `0x005A0140`,
`0x005A0180`.

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
   *Answered* (§3.5, TSV updated): every receivable id has its sender
   or `produced_by` none; every row is `yes`.
5. S→C field layouts: only the builders in the `layout` column were read;
   every other layout is unconfirmed (one owner per system spec later).
   Still empty after §3.5: ids whose builder layout was not needed by a
   spec yet (e.g. 0x09, 0x0B, 0x11, 0x20–0x22, 0x28–0x2A, 0x3E, 0x40,
   0x51, 0x52, …): read when their owner is written.
6. C→S field meanings marked `partial` (0x14, 0x15, 0x32, 0x33, 0x35,
   0x44, 0x4F): offsets and widths are 1.14d, names are D2MOO's; each
   system spec confirms its own. *Partly answered* (§9; TSV rows 0x26,
   0x2F–0x31, 0x38, 0x3D, 0x3F, 0x4D, 0x58, 0x59 now `yes`). Known name
   fixes for a later code-table change (the client encodes these
   fields, so names, not bytes): 0x32 u32@9 = transaction (bits 0–15) |
   fill (bit 31), u32@13 never read (`world/vendors.md` §7.1); 0x33
   u16@9 = item mode, u32@13 not read (§7.2); 0x35 u16@9 not read, u32@13
   bit 31 = repair all (§8.1); 0x14 u8@1 is not read (§9 rule 3).
   *Names fixed* (2026-10-07, TSV): 0x14 `unread`@1; 0x32
   `transaction`@9, `client_price`@13; 0x33 `item_mode`@9,
   `client_price`@13; 0x35 `unread`@9, `repair_flags`@13; 0x32, 0x33,
   0x35 now `yes`. Open: 0x14 / 0x15 `lang`@2, 0x15 `type`@1, 0x44,
   0x4F. *Answered* (2026-10-07, TSV, every row `yes`): `lang`@2 is a
   language id: the server copies it into S→C 0x26 u8@2 (§9 rules 3,
   16) and the client compares it with its own language id
   (`0x00525150`, 0–13) to pick the text conversion (`0x0049E280`,
   `client/msg-ui.md` §4); 0x15 u8@1 is copied and never read (§9
   rule 16), renamed `unread`; 0x44 u16@13 renamed `action` (§9 rule
   7); 0x4F (§9 rule 15); system ids 0x67 (adds `game_name`@1), 0x6C,
   0x6D (adds `tick`@1): §2.5.
7. *Answered (2026-10-08)*: §2.1 rule 8 (the chat builder `0x004787B0`
   always writes c = 0; recorded c = 0 in all 9 chat lines of
   `pc2rec-p1-packets` / `pc2rec-p2-packets`). Original question: the odd third term of the
   C→S chat size rule (§2.1 rule 5): does the 1.14d client ever send a
   non-zero byte there?
8. *Answered (2026-10-08)*: §2.1 rule 8 (no builder call site sends
   them; 0x0B is a fatal assert in `0x00481030`). Original question:
   0x0B, 0x2E, 0x42, 0x43: does the 1.14d client ever send them?
9. Client receive handlers' behaviour (§3.4 rule 3) belongs to client
   specs (Phase 5–6); not covered here.
10. The conditions inside `0x00571CD0`, `0x00571620`, `0x005715A0`,
    `0x00570E30` / `0x005711D0` (§7.3 rule 2 steps 3, 4, 8, 9) and which
    of their senders fire for a plain Act I monster; and the meaning of
    `0x005A5650` (the d byte, recorded 0x80 on 0x6D) and `0x00572EE0`.
    *Partly answered*: `0x00570E30` (0xAA), `0x00571CD0` (pending event
    records), `0x00571620` (0x76 / overhead 0x26): §7.9; `0x005A5650` =
    the life fraction (§7.4 rule 5); `0x005711D0` (0xA7 / 0xA8 / 0xA9
    per changed state): §3.5 rule 6; who writes the unit +0xEC records:
    §7.9 rule 2 (nine writers). Open: `0x005715A0`, `0x00572EE0`, and
    who sets the state-change bits `0x005711D0` reads.
11. The §7.6 order (item 0x9C before the monster's 0x69 in a kill tick
    with a drop): a recording of a kill that drops an item. *0x65 part
    answered* (§7.6 rule 5, static): the kill sets arena flag 0x400
    (`0x0053F720`), the same tick's arena sync `0x0053FC20` sends it
    (recorded right after the monster's 0x69 code 8, both from the
    per-client update), tick step 6 clears the flag.
12. *Partly answered* (§8.2 rule 3.1, static call order of the save path
    = the recorded order). Open: the per-item conditions that send 0x22
    and 0x21 during the item load (owner: `formats/d2s.md` and the
    character-load spec, not yet written) and when the loader's own
    0x23 calls (`0x005341FD`, `0x00534210`) fire.
13. *Answered* (§8.3, static): the first 0x48 is the per-client update's
    inventory refresh (`0x00538146` → `0x0055DBC0`, 0x48 at `0x0055DEEB`);
    the player update's 0x48 (`0x005808D9`) runs before the stat
    messages that precede it.
14. *Answered (recorded, `pc2rec-p1-packets` / `pc2rec-p2-packets`)*:
    §9 rule 16 (single player: one 0x26 form 1 per line from
    `0x0053C82F`, no 0x5A; the whisper forms are not reachable from the
    single-player client, §2.1 rule 8). Original question: C→S 0x15
    Chat (`0x0054A5D0`) after its checks: which messages it sends (0x26
    through `0x0053C750`, `0x0053C850`) and to whom.
15. `0x005845D0` (0x3D, §9 rule 4): what a door highlight does to the
    object (object spec).
16. `0x005616A0` (0x60, §9 rule 14): the weapon switch and its fail
    flag (item spec).
17. This spec is past 60 KB (`specs/README.md` Process): split §3.5
    and the S→C parts of §6–§7 into a sender spec in a later pass.
18. Client side of ids §3.5 confirmed that still have no client owner
    (UI entry points: 0x26 → `0x0049F490`, 0x27 → `0x004A1600`, 0x4E →
    `0x004B3240`, 0x50 → `0x004B9210`, 0x58 → `0x004C0550`, 0x78 →
    `0x004B9010`, 0x89 → `0x004B9330`, 0x8A → `0x004B3380`, 0x91 →
    `0x004B3510`; skill events 0x99 / 0x9A → `0x004CA200` /
    `0x004CA230` → `0x004CA060`): owners the `ui/*` specs (requested in
    `docs/handoff/xpc-to-pc2.md`) and a client skill-event spec.
    *Answered* (2026-10-07, `client/bridge-dispatch.tsv`): client model
    side and `client/bridge.md` §10 outputs in `client/msg-ui.md` §4
    (0x26), §5 (0x27), §6 (0x4E, 0x4F), §7 (0x50), §8 (0x58), §9 (0x8A),
    §10 (0x91), §11 (0x78) and `client/msg-skills.md` §7 (0x99, 0x9A),
    §8 (0xA3); 0x89 is `render/lighting.md` §10 r4. Model writes: 0x50
    code 23 (C→S 0x69, `exit_requested`), 0x58 code 5 (`cursor_item`);
    the rest is UI or effect state. Display rules stay with `ui/*`
    (`docs/handoff/xpc-to-pc2.md`).
19. *Answered (2026-10-08)* (`docs/handoff/impl-monster-death.md` §3
    Finding 1): the first death test vector's (4757, 5461) is the path
    target sent as (a, b), not the unit's cell; the row now says so.
20. *Answered (2026-10-08)* (`docs/handoff/impl-server-join-2.md` §3):
    the 0x1D / 0x1E / 0x1F choice of `0x0053BE40` and the 0x19 case of
    `0x0053E9B0`: §3.5 rule 7.
21. *Answered (2026-10-08)* (`docs/handoff/impl-umods-cs-handlers.md`,
    subagent questions): C→S 0x14 with empty text returns 0 (§9 rule 3
    is right; §2.4 rule 6 corrected). The 0x59 owner pointer already
    names `monsters/ai-bodies.md` §9.9 (§9 table).
