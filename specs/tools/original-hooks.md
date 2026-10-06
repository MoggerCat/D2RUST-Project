# Spec: Tools — Scenario hooks in the original 1.14d `Game.exe`

- **Status:** draft: every address read from the 1.14d disassembly
  (`tools/ghidra/disasm.py`) or the file image; thread, client id,
  player GUID and message bytes from the existing recordings; the
  injection and start-up procedures are not yet run (Open questions 1–3).
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
| Summary | 37–49 |
| Inputs | 50–56 |
| Outputs / state changes | 57–61 |
| Rules | 62–63 |
|   1. Client→server message entry (single player) | 64–128 |
|   2. Game seed at game creation | 129–172 |
|   3. Tick boundary | 173–214 |
|   4. Unit snapshot fields | 215–218 |
|   5. Starting single player without a human | 219–222 |
| Constants & data dependencies | 223–230 |
| Randomness | 231–235 |
| Edge cases & original bugs | 236–240 |
| Test vectors | 241–247 |
| Provenance | 248–255 |
| Open questions | 256–262 |
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

Written in the next push.

### 5. Starting single player without a human

Written in the next push.

## Constants & data dependencies

| Address | Value / use |
|---|---|
| `0x007A0610` | client game type (0 single player) |
| `0x00882D10`, `0x00882B34` | local mode (1), connected flag |
| `0x00731004` | fixed game seed (−1 = none) |

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
- Single thread, client id 0, player GUID 1: the two packet recordings
  named in Test vectors.

## Open questions

1. Injection: does a 0x01 walk message (5 bytes) queued through
   `0x0052AE50` at `0x0044F136` give `c2s`, `dispatch` and `result` 0 in
   the same drain, with `dispatch.game_frame` = N−1? Probe: inject at a
   known tick with `record_packets.py` hooks armed.
