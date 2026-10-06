# Spec: Tools — Scenario hooks in the original 1.14d `Game.exe`

- **Status:** draft: every address read from the 1.14d disassembly
  (`tools/ghidra/disasm.py`) or the file image; thread, client id,
  player GUID and message bytes from the existing recordings; the
  injection, seed and start-up procedures are not yet run (Open
  questions 1–3).
- **Target version:** 1.14d
- **Crate/module:** `tools/trace-recorder/run_scenario.py` (debugger
  recorder, Python; spec-role tool)
- **Related specs:** `sim/intents-events.md` (owner of the message path
  §1–§2), `sim/tick.md` (owner of the host loop §1, tick steps §3),
  `sim/rng.md` (owner of the seed sources §5), `sim/unit-order.md`,
  `sim/units.md` §2, `sim/path-placement.md` §2, `sim/stat-lists.md` §1,
  `sim/stats.md` §2, `drlg/rooms.md` §1, `drlg/levels.md`;
  `tools/trace-recorder/README.md` (hooks the recorders already use)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–50 |
| Inputs | 51–57 |
| Outputs / state changes | 58–62 |
| Rules | 63–64 |
|   1. Client→server message entry (single player) | 65–129 |
|   2. Game seed at game creation | 130–173 |
|   3. Tick boundary | 174–215 |
|   4. Unit snapshot fields | 216–266 |
|   5. Starting single player without a human | 267–372 |
| Constants & data dependencies | 373–385 |
| Randomness | 386–390 |
| Edge cases & original bugs | 391–395 |
| Test vectors | 396–402 |
| Provenance | 403–417 |
| Open questions | 418–456 |
<!-- /index -->

## Summary

A differential test runs one scripted scenario on 1.14d and on d2rs and
diffs the traces. On 1.14d a debugger must (a) start a single-player game
with a chosen character and difficulty without a human, (b) fix every
clock-derived seed, (c) inject client→server messages so they are handled
before a chosen tick N, and (d) read unit snapshots at tick boundaries.
This spec gives the addresses, registers and procedures for each, all
for the reference `Game.exe` (SHA-256 `631066c1…adaaf`, image base
`0x400000`). Rules about what the game does with the messages, seeds and
units live in the owning specs linked above; this spec only says where to
stop, what to read and what to write.

## Inputs

| Name | Type | Source |
|---|---|---|
| scenario | character name, class, difficulty, game-seed time value, list of (tick N, message bytes) | the scenario file (tool's format) |
| save | `<save dir><name>.d2s`, version 0x60 | user's `game/` install |

## Outputs / state changes

The tool writes process memory only at the points named here; it writes
no file in `game/` (with `-nosave`, §5.4, the game writes none either).

## Rules

### 1. Client→server message entry (single player)

The path a client message takes is owned by `intents-events.md` §2.1–§2.3;
the facts the injector needs:

| Function | Convention | Arguments | Return |
|---|---|---|---|
| `0x0052AE50` transport send | stdcall, `ret 0xC` | [ESP+4] size (compared as u16, ≤ 0x204 else fatal assert), [ESP+8] channel (callers pass 1 or 0; not read on the local path), [ESP+0xC] message pointer | EAX 1 = queued; 0 = not queued (connected flag `0x00882B34` = 0, or the classifier drops it, `intents-events.md` §2.1 rule 4) |
| `0x0052B690` local enqueue | ECX message, EDX size | calls `0x006BF370(net 0x00882D08, message, size, client id 0)` | 1/0 |
| `0x0052CFE0` drain | no arguments | queue 0 → `0x0053F100`, queue 1 → `0x0053F3D0`, queue 2 → `0x0052CC20`, each until empty | — |
| `0x0053F3D0` game message entry | fastcall, plain `ret` | ECX = buffer: u32 client id, then the message; EDX = message size (without the id) | none used |
| `0x0054D750` dispatcher | fastcall ECX game, EDX player unit; [ESP+4] message, [ESP+8] size; `ret 8` | | EAX result 0–3 (`intents-events.md` §2.3) |

1. **Single thread.** In single player the client frame, the drain, the
   tick and the flush all run on one thread: every record of both packet
   recordings (`traces/raw/20261006-015956-packets.jsonl`,
   `…-022633-packets.jsonl`) carries one `tid`. That thread is the one
   that hits `0x0044F136` (§3).
2. **Client id 0.** The local transport always queues with client id 0
   (`0x0052B6B6` pushes 0); both recordings show client 0 in every `c2s`
   and `c2s_sys` record. The player unit has GUID 1 (`dispatch` records,
   `unit` field).
3. **Preferred method: call the transport send `0x0052AE50` from the
   breakpoint at `0x0044F136`** (the drain call of the single-player
   frame, §3). The message then takes the real path: classifier (size
   rules, silent drop of bad sizes), queue 1 in FIFO order after
   anything the client queued during the previous frame, then the drain
   that runs immediately after the breakpoint. Only the client-side
   duplicate filter `0x00478350` is skipped, which a scenario wants
   (it sends exactly what it lists). Calling `0x0053F3D0` directly
   (method a) would also be safe at that point (no lock is held; it takes
   the game lock itself, `0x0052FEE0`/`0x0052DA90`), but skips the
   classifier and the queue order, so its result can differ from a real
   client's; writing queue nodes by hand (method b) needs the net
   object's critical section (`net +0x854`) and its node allocator, and
   is not needed.
4. **Steps** (once per message, all at one stop at `0x0044F136`):
   1. Once per run: `VirtualAllocEx` one RWX page in the game (scratch
      S). Write `0xCC` at S+0 (return trap).
   2. At the breakpoint, after the debugger has rewound EIP to
      `0x0044F136`: save the full thread CONTEXT.
   3. Write the message bytes at S+0x10 (≤ 0x204 bytes; the drain copies
      at most 0x1FC bytes of message, `intents-events.md` §2.1 rule 7).
   4. ESP −= 16; write [ESP] = S (return address), [ESP+4] = size,
      [ESP+8] = 1, [ESP+0xC] = S+0x10. EIP = `0x0052AE50`. Continue.
   5. At the INT3 on S+0: EAX is the result (must be 1; 0 = not queued,
      abort the scenario). The callee popped its 12 argument bytes.
      Repeat 3–5 for the next message of the same tick.
   6. Restore the saved CONTEXT (EIP = `0x0044F136`, ESP as before) and
      let the debugger step over the original `call` as for any hook.
      The drain then handles the queued messages in order.
5. **What must hold** before the first injection: game type global
   `0x007A0610` = 0 (single-player branch at `0x0044F132`); local mode
   `0x00882D10` = 1 and connected flag `0x00882B34` ≠ 0 (both set by
   `0x0052A750`, the single-player connect); the game exists (first tick
   seen, §3) and the client is in state 4 (client list head game +0x88,
   state client +0x04, next client +0x4A8, `tick.md` §6). Before that,
   `0x0053F3D0` drops game messages (no player unit, `intents-events.md`
   §2.2 rule 4). Gate rules (alive/dead) and size rules of
   `intents-events.md` §2.3–§2.4 still apply to injected messages.
6. **Check** each injection with the `record_packets.py` hooks: a `c2s`
   record at `0x0053F3D0` with the injected bytes, a `dispatch` at
   `0x0054D750` and its `result` at `0x0053F45E` (EAX), all in the drain
   before tick N.

### 2. Game seed at game creation

The seed sources are owned by `rng.md` §5.2 and §5.4; in single player
two values are clock-derived and one comes from the save:

| Value | Set at | Normal source |
|---|---|---|
| game seed, game +0xD0 | `0x0052C280` (EDI = game; callers `0x00530B3E`, `0x00530D5C`) | `{time_value(QPC low), 666}`, then one step at `0x0052C2C6` |
| `dwInitSeed`, game +0x7C | `0x0052C2E3` | `time_value(lo')`; replaced at save load (below) |
| map seed into game +0x7C | `0x0056A211`–`0x0056A217` (save load, v0x60 saves; ESI = save buffer, EBX = game) | u32 at save +0xAB, only when game +0x6A = 3 (single player), game +0x84 = 0 and bit 7 of save byte +0xA8 + difficulty is set |
| DRLG seed | `0x0053AC8E` reads game +0x7C → `0x006194A0` → `0x00642DA0` | — |

`0x00532A2E`–`0x00532A45` is the same map-seed rule for saves of version
≤ 0x5B (`0x00534330` routes ≤ 0x5B to `0x00534020`, others to
`0x0056B180`).

1. **Preferred override (keeps the normal path that `rng.md` §5.2
   specifies):** breakpoint at `0x0052C2BB` (`mov edx, eax` after the
   first `time_value` call; bytes `8B D0`): set EAX = the scenario's
   time value T (0 ≤ T < 2^31, as `time_value` returns). The game seed
   becomes `{T, 666}` and is stepped once at `0x0052C2C6`, as in a normal
   game. Breakpoint at `0x0052C2E3` (`mov [edi+0x7C], eax`; bytes
   `89 47 7C`): set EAX = the scenario's init value I. Both run before
   the first draw from the game seed (`rng.md` §5.2: the first recorded
   step is `0x0052C2C6`). d2rs takes T and I as its inputs (`rng.md`
   §5.1).
2. The DRLG then uses the save's map seed (deterministic per save), or I
   when the save's difficulty byte has bit 7 clear. Overriding I is
   therefore required, not optional.
3. **Alternative: `-seed N`** (1.14d command-line switch, §5.1): the
   client-start handler `0x0044D860` calls `0x0052C320(N)`, which stores
   N in the fixed-seed global `0x00731004` (initial −1). `0x0052C280`
   then takes its other branch: game +0x84 := 1, game +0x7C := N, game
   seed := `{N, 666}` with **no** step; with +0x84 = 1 the save's map
   seed is not applied, so the DRLG uses N. No debugger is needed, but
   it is a different seed path from a normal game; d2rs would have to
   model it. N = 0 is ignored by the handler (`test eax, eax`); N = −1
   selects the normal path. This corrects `rng.md` §5.2, which called
   `0x0052C320` uncalled.
4. Other clock inputs (`rng.md` §5.1): the unit-seed fallback (no parent
   seed) and two client particle globals. Neither feeds a normal
   single-player server trace; a recorder sees the first as a
   `time_value` call (`0x00650DE0`) from `0x00552DF0` (Open question 9).

### 3. Tick boundary

The host loop is owned by `tick.md` §1 and `intents-events.md` §1. The
single-player client frame `0x0044EFA0` runs, at its end:

| Address | Instruction | Role |
|---|---|---|
| `0x0044F12B` | `mov eax, [0x007A0610]` | game type; 0 takes the single-player branch |
| `0x0044F136` | `call 0x0052CFE0` (bytes `E8 A5 DE 0D 00`) | drain all server queues: **injection point** (§1) |
| `0x0044F13D` | `call 0x0052FC20` (`push 1` before it) | tick driver: runs ≤ 1 tick per game (`tick.md` §1 rule 2) |
| `0x0044F162` | `call 0x0052FD90` | flush, only when a tick ran |
| `0x0044F167` | `call 0x0044C6E0` | client receive |

Inside the driver: tick entry `0x0052D870` (ECX = game; frame = game
+0xA8 + 1 after its first instruction), return at `0x0052FD1E` (ESI =
game). `record_tick.py` (`TICK`) and `record_packets.py` (`tick`,
`tick_end`) hook exactly these.

1. **Message at tick N.** Inject at the first stop at `0x0044F136` after
   the return of tick N−1 (`0x0052FD1E` with game +0xA8 = N−1). The drain
   that follows handles the message with game +0xA8 = N−1, before tick N's
   step 0; its effects are simulated from tick N on (timer events run in
   tick N's step 4, `tick.md` §3). This holds however many client frames
   pass between two ticks: every frame drains before it calls the
   driver, and the driver never runs a tick before the drain of its own
   frame. Injecting in a later frame of the same gap gives the same
   frame number; only the order relative to client-generated messages
   can change (§1 rule 3).
2. For N = 1 (before the first tick), inject after game creation and
   player join, which happen in the drain of the 0x67 message (system
   queue, `intents-events.md` §2.5); the player is not in state 4 before
   the client pass of a tick (`tick.md` §6 rule 4), so in practice the
   first usable N is the tick after the client reaches state 4 (§1 rule
   5).
3. **Snapshot at tick N** (state after tick N): at `0x0052FD1E` for the
   game whose +0xA8 = N. The driver holds the game lock there; the
   debugger only reads. The flush and client receive that follow change
   no server unit state.
4. A client frame that returns early (`0x0044EFD9`, when `0x004F6070`
   returns non-zero, or `0x0044F022`) runs neither drain nor tick; the
   rule above is unaffected.

### 4. Unit snapshot fields

Read at the snapshot point of §3 rule 3, game = the ESI of `0x0052FD1E`
(or the ECX of `0x0052D870`). Every offset below is owned by the spec in
the last column; this table only gathers them.

1. **Walking the units** (`unit-order.md` §2 rule 4): per type, buckets
   0..127 at game +0x1120 + `offset(type)` + 4·bucket, offsets player
   0x000, monster 0x200, object 0x400, item 0x600, missile 0x800 (table
   `0x006E10E0`); each bucket from its head through unit +0xE4. Tiles:
   one list at game +0x1B20, same link. This is the order
   `record_tick.py` snapshots use (`HASH_BASE`, `HASH_OFFSETS`,
   `TILE_LIST`, `U_HASH_NEXT`). Every unit in these lists is a server
   unit (unit +0xC8 bit 0x4000000, `units.md` §2); a trace should sort
   by (type, GUID) rather than rely on walk order, unless walk order is
   itself under test.
2. **Fields:**

| Field | Read | Owner |
|---|---|---|
| type | u32 unit +0x00 | `units.md` §1–§2 |
| class (txt row) | u32 unit +0x04: player class 0–6, monstats row, objects row, missiles row, item row | `units.md` §2 |
| GUID | u32 unit +0x0C | `unit-order.md` §1 |
| mode | u32 unit +0x10 | `units.md` §1, §4 |
| act | u8 unit +0x18 (act record at unit +0x1C) | `units.md` §2 |
| flags, flags 2 | u32 unit +0xC4, +0xC8 | `units.md` §2 |
| path | u32 unit +0x2C; null for items not on the ground and for some units in transit | `path-placement.md` §2.1 |
| precise x, y (types 0, 1, 3) | u32 dynamic path +0x00, +0x04 (16.16) | `path-placement.md` §1 rule 2, §2.3 |
| sub-tile x, y (types 0, 1, 3) | u16 dynamic path +0x02, +0x06 (high words of the above) | `path-placement.md` §2.1 |
| sub-tile x, y (types 2, 4, 5) | u32 static path +0x0C, +0x10 | `path-placement.md` §2.2 |
| target x, y (types 0, 1, 3) | u16 dynamic path +0x10, +0x12 | `path-placement.md` §2.3 |
| active room | dynamic path +0x1C; static path +0x00 (`0x00620BB0`) | `path-placement.md` §2.1 |
| level id | active room +0x10 → DRLG room +0x58 → level +0x1D0 | `drlg/rooms.md` §1, `drlg/levels.md` |
| unit seed | u32 × 2 unit +0x20 | `rng.md` §5.3 |
| stat list | u32 unit +0x5C (extended list; null: no stats) | `stat-lists.md` §1 |

3. **Life and mana.** From the extended list at unit +0x5C read the
   full array (pointer list +0x48, i16 count list +0x4C), entries of 8
   bytes: u16 layer, u16 stat, i32 value; the first dword is the key
   `(stat << 16) | layer`, sorted ascending (`stat-lists.md` §1,
   `stats.md` §1). Keys with layer 0: life 6 (`hitpoints`), max life 7,
   mana 8, max mana 9, stamina 10, max stamina 11. Their values are 8.8
   fixed point (`ValShift` 8, `stats.md` §2 rule 2): record the raw i32;
   points = value >> 8 (arithmetic). An absent key reads 0. The raw
   full-array value is what the game stores; the unit-total reader
   `0x00625480` additionally applies the minimum rule (`stats.md` §4.3),
   so a trace that records "total" must say which one it records.
4. Player-only: the client record (game +0x88, next +0x4A8) holds the
   client state at +0x04 (`tick.md` §6); the player unit is found in the
   player hash list (GUID 1 for the only player, Test vectors).

### 5. Starting single player without a human

#### 5.1 Command line (1.14d)

1. **Parser** `0x004058A0` (ECX = command line, [ESP+4] = config
   record): scans for `-`, looks the switch up (`0x00405710`, exact
   byte compare `0x00405570`, so case-sensitive) in the switch table
   `0x00705040` (58 records of 0x5C bytes, to `0x00706518`). Record:
   ini section char[0x1B] +0x00, ini key char[0x1B] +0x1B, switch
   char[0x1B] +0x36, type u8 +0x51, config offset u32 +0x54, ini default
   u32 +0x58. Type 0: config byte := 1 (flag, no value); 1: config u32 :=
   `atol(value)`; 2: string copied into the config.
2. **Order** (`0x004059A0`, called at `0x0040657F` with the config on the
   start-up function's stack, `[ebp−0x4D8]`): zero 0x3CD config bytes;
   read every switch's ini key from `D2.ini` (`0x00405450`, default from
   the record); then the command line, which overrides.
3. **Applied at every client-mode start**: client entry `0x0044B8A0`
   (stdcall; [ESP+8] = config, kept in `0x007A0438`) → `0x0044B6B0` →
   `0x0044D9F0`, which runs the 52-entry handler table `0x0070F488` on
   the config. Switches the tool needs:

| Switch | Config | Handler | Effect |
|---|---|---|---|
| `-w` (`-window`, `-windowed`) | +0x08 | `0x0044D760` | windowed |
| `-ns`, `-nosound` | +0x220 | not traced | no sound |
| `-name S` | +0xBD (string) | `0x0044D890` | character name := first ≤ 15 chars of S into `0x007A05C4` (16 bytes, zeroed first); skipped when S is `0` |
| `-ama`, `-sor`, `-nec`, `-pal`, `-bar` | +0x85, +0x87, +0x88, +0x86, +0x89 | `0x0044D6B0` | class `0x007A0522` := 0, 1, 2, 3, 4; config +0x8A → 5, +0x8B → 6 exist with no switch; the last set byte in the order +0x85…+0x8B wins |
| `-seed N` | +0x21A (u32) | `0x0044D860` | N ≠ 0: fixed game seed (§2 rule 3) |
| `-nosave` | +0x219 | `0x0044D580` | `0x007310CC` := 0 (`0x00530F30`); the server save routines `0x00531EB0` and `0x00532240` return without writing while it is 0 (initial 1) |
| `-act N` | +0x1FF (u32) | `0x0044D5A0` | 1 ≤ N ≤ 5 else fatal assert; `0x0052DFA0(N − 1)` stores it in `0x00883D44`, read at game creation (`0x00530E17`) |
| `-gametype N`, `-gamename S` | +0x19, +0x1F | `0x0044D5F0` | client game type `0x007A0610` := N (0 single player); game name `0x007A05DC` |
| `-txt` | +0x215 | `0x0044D9C0` | `0x006125A0(config +0x215 == 0)` |
| `-direct` | +0x204 | not traced | file I/O |
| `-skiptobnet` | +0x35D | none | read by the menu `0x004359D0` (`0x00435BB3`) |

4. **There is no difficulty switch**: no record of the table names one.
   Difficulty comes from config +0x210 (§5.2), written only by menu code
   (`0x00439840`, `0x00439AF0`, `0x0043AE30`, `0x0043B080`, `0x00445FF0`).

#### 5.2 Game creation message 0x67

Built by `0x00477CA0` (called at `0x0044F45E` from client state handler
`0x0044F360` when the client game type is not 3, 7 or 9; ECX = game
name `0x007A05DC`), sent through `0x0052AE50` (46 bytes) and handled by
system message 0x67 → `0x00530BF0` (`intents-events.md` §2.5; layout in
`client-messages.tsv`):

| Byte | Source | Server use (`0x0053F141`–`0x0053F17A` → `0x00530BF0`) |
|---|---|---|
| +0x01 | game name (16) | |
| +0x11 | 3 when the client game type is 0 (2 for 8, 1 for 6, else 0) | game +0x6A |
| +0x12 | class: `0x00712F00` (the `-ctemp` value) when bit 8 of `0x00712EFC` is set, else `0x007A0522` | |
| +0x13 | config +0x20D | game +0x6B |
| +0x14 | **difficulty**, config +0x210 | game +0x6D |
| +0x15 | character name `0x007A05C4` (16) | save file name (§5.3) |
| +0x25 | config +0x207 (u16) | |
| +0x27 | config +0x209 (u32; 0 → 0x100004) | bit 0x100000 tested at `0x00530D05` |
| +0x2B, +0x2C | config +0x20E, +0x20F | |
| +0x2D | `0x00525150()` | |

Recorded: `…-015956-packets.jsonl` sends +0x11 = 3, +0x12 = 4, +0x14 =
0, name `charactertest`; `…-022633` sends class 1, name `werwer`.

#### 5.3 Save load (single player)

1. Player creation calls `0x005345A0` (caller `0x00539804`): with game
   +0x6A ∉ {1, 2} and no host callbacks (`0x00883D50` = 0, single
   player) it goes to `0x005344B0` → `0x005343A0`, which reads the save
   **from disk itself** (no 0x6C upload: none in either recording).
2. `0x005343A0`: path = `sprintf("%s%s.d2s", save dir, name)` (format
   `0x006D4230`); save dir from `0x00407050` (registry values
   `NewSavePath`, then `Save Path`, of the `Diablo II` key, read through
   `0x00414E50`; the fallback is not traced, Open question 4); `fopen`
   mode `rb` at `0x00534410`; reads ≤ 0x2000 bytes; then `0x00534330`:
   needs ≥ 8 bytes and magic 0xAA55AA55 at +0; version (+4) ≤ 0x5B →
   `0x00534020`, else `0x0056B180`.
3. The v0x60 loader sets the client's act from save byte +0xA8 +
   difficulty (low 7 bits, ≥ 5 → 0; `0x0056A1D8`–`0x0056A1F7`) and the
   map seed of §2.

#### 5.4 Procedure

1. Launch `Game.exe -w -ns -nosave -name <name> -<class>` under the
   debugger (same reference-hash check as the recorders). `-nosave`
   keeps the `.d2s` byte-identical across runs and untouched in `game/`.
2. Force the menu to end as `dump_tables.py` does (README, "When the
   tables load"): with launcher mode `0x0074C704` = 4, write next mode
   1 to `0x007795E8` and 0 to the menu-loop flag `0x0072DDD4`.
3. Breakpoint `0x0044B8A0` (bytes `55 8B EC 56 8B 75`): config = [ESP+8].
   Write config +0x210 := difficulty (0, 1, 2). For class 5 or 6 write
   config +0x8A or +0x8B := 1. The handlers of §5.1 rule 3 run after this
   point and copy name and class.
4. Arm the seed overrides of §2 (`0x0052C2BB`, `0x0052C2E3`) before
   0x67 is drained.
5. The client loads the tables, then its state machine (`0x0070EE4C`,
   handlers `0x0070EE54`: state 3 `0x0044D080` moves on when
   `0x0070EF18` = 5; state 2 `0x0044F360`) sends 0x67 (§5.2). The server
   creates the game and loads the save in the next drain; the first tick
   (frame 1) follows.
6. What must hold: the save exists at the path of §5.3 rule 2, version
   0x60, for that name; the chosen difficulty is one the save has reached
   (save byte +0xA8 + difficulty bit 7; Open question 6); client game
   type stays 0 (no `-gametype`); nothing else is needed from a human
   once 0x67 is sent (the hand-played recordings send nothing but system
   messages 0x67, 0x6D, 0x6B before the first game message).

## Constants & data dependencies

| Address | Value / use |
|---|---|
| `0x007A0610` | client game type (0 single player) |
| `0x00882D10`, `0x00882B34` | local mode (1), connected flag |
| `0x00731004` | fixed game seed (−1 = none) |
| `0x007A05C4`, `0x007A0522` | character name (16 bytes), class |
| `0x007310CC` | save enabled (1; `-nosave` → 0) |
| `0x0074C704`, `0x007795E8`, `0x0072DDD4` | launcher mode, next mode, menu-loop flag (`dump_tables.py`) |
| `0x00705040` | command-line switch table, 58 × 0x5C bytes |
| `0x0070F488` | switch handler table, 52 entries |

## Randomness

No draw of its own. The overrides of §2 replace the clock-derived inputs
of `rng.md` §5.1–§5.2 before the first draw.

## Edge cases & original bugs

1. A message longer than 0x1FC bytes is truncated by the drain copy
   (`intents-events.md` §2.1 rule 7); only id 0x66 can be that long.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| any single-player recording | one thread id for drain, tick, dispatch | `traces/raw/20261006-015956-packets.jsonl` |
| first `c2s`/`dispatch` of a hand-played game | client 0, player GUID 1, `dispatch.game_frame` = previous tick's frame | `traces/raw/20261006-022633-packets.jsonl` (frame 24) |

## Provenance

- 1.14d `Game.exe`, disassembly via `tools/ghidra/disasm.py` (`fn`, `at`,
  `xref`) over `re/exports/functions.tsv` and `re/exports/all.asm`;
  instruction bytes read from the file image (pefile).
- Single thread, client id 0, player GUID 1, 0x67 bytes, absence of a
  0x6C save upload: the two packet recordings named in Test vectors.
- Switch table: decoded from the file image (58 records, section, key,
  switch, type, offset, default); parser and handler tables from the
  disassembly. D2MOO (1.10f) was used only for field names (game +0x6B,
  +0x6D, +0x7C, +0x84); each read or write named here was located in the
  1.14d code.
- Correction made to `sim/rng.md` §5.2: `0x0052C320` has two callers
  (`0x0044D86B`, `0x00451909`).

## Open questions

1. Injection: does a 0x01 walk message (5 bytes) queued through
   `0x0052AE50` at `0x0044F136` give `c2s`, `dispatch` and `result` 0 in
   the same drain, with `dispatch.game_frame` = N−1? Probe: inject at a
   known tick with `record_packets.py` hooks armed.
2. Forced start: after §5.4 steps 1–3, does the client send 0x67 with
   the given name, class and difficulty, and does tick 1 run with no
   input? Probe: `record_packets.py` hooks plus the §5.4 writes; expect a
   `c2s_sys` 0x67 with those bytes and a `tick` with frame 1. If state 3
   never advances, log `0x0070EF18` per frame.
3. Seed override: with T and I set, does the game seed at game +0xD0
   after `0x0052C2C6` equal one step of `{T, 666}`, and do two runs give
   identical `record_rng.py` chains? Probe: `record_rng.py --no-inline`
   plus the two overrides, run twice, diff.
4. Save dir fallback when neither registry value exists: probe a
   breakpoint at `0x00534410`, read the path string at [ESP].
5. Second caller of `0x0052C320` at `0x00451909` (code outside the
   Ghidra function list, near `0x004518E0`): what triggers it, and can
   it change `0x00731004` during a scenario? Probe: breakpoint on it.
6. A difficulty the save has not reached (save byte bit 7 clear): does
   the join fail, or does the game start with the clock-derived +0x7C?
   Probe: §5.4 with D = 2 on a fresh character.
7. Does the server use the 0x67 class byte (+0x12) when it loads a save
   of another class? Probe: start with a mismatched `-<class>`.
8. `-act N` in single player: the v0x60 load sets the act again
   (`0x0056A1F7`); does `0x00883D44` have any effect after that? Probe:
   `-act 2` with an act-1 save.
9. Does any server path in a scenario call `time_value` (`0x00650DE0`)
   after game creation (unit-seed fallback, `rng.md` §5.3)? Probe:
   breakpoint on `0x00650DE0`, log the caller, over a full scenario.
10. Are client frames skipped (`0x0044EFD9` early return, `0x004F6070`)
    when the game window is not focused or minimized under the debugger?
    Probe: count `0x0044F136` hits per second with the window in the
    background.
11. Item position for items not on the ground (path null): which fields
    (owner, inventory grid, body location) a snapshot should read; owned
    by the item specs.
