# Spec: Tools — Spawning monsters in the original 1.14d `Game.exe`

- **Status:** draft: every entry point, convention and draw site was read
  from the 1.14d disassembly (`tools/ghidra/disasm.py`, `re/exports/all.asm`);
  the call procedure of §5 ran six times in a 1.14d single-player game
  with `tools/trace-recorder/spawn.py` (normal, party, champion, unique,
  random boss, superunique); every recorded draw site is in §4 and every
  draw passed `check_rng.py` (Test vectors).
- **Target version:** 1.14d
- **Crate/module:** `tools/trace-recorder/spawn.py` (debugger tool, Python;
  spec-role tool)
- **Related specs:** `monsters/init.md` (owner of creation, umods, boss and
  superunique init, the draw order), `monsters/population.md` (owner of
  placement §9, boss spawn §6.3, minions §6.4–§6.5, §10, superunique
  checks §11.4), `sim/rng.md` (the generator, helpers §3, unit seed §5.3,
  inline draws §6), `tools/original-hooks.md` (message injection, seed
  override, tick boundary, forced single-player start; branch
  claude/spec-scenario-hooks), `sim/tick.md` §1, `sim/unit-order.md` §1,
  `sim/path-placement.md` §2, `tools/trace-recorder/README.md`

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–54 |
| Inputs | 55–64 |
| Outputs / state changes | 65–72 |
| Rules | 73–74 |
|   1. Entry points | 75–106 |
|   2. Pointers and preconditions | 107–132 |
|   3. Kinds of monster | 133–150 |
|   4. RNG draw sites | 151–209 |
|   5. Calling an entry from the debugger | 210–239 |
| Constants & data dependencies | 240–250 |
| Randomness | 251–255 |
| Edge cases & original bugs | 256–264 |
| Test vectors | 265–288 |
| Provenance | 289–305 |
| Open questions | 306–311 |
<!-- /index -->

## Summary

A debugger tool that wants a given monster at a given place and tick
calls one of the game's own spawn functions on the server thread, between
two ticks, with pointers read from the running game. This spec lists the
callable entry points (arguments, registers, stack, return), how to get
the game, room and position they need, which entry gives a normal,
champion, unique, superunique or minion monster, every RNG draw site the
call can reach (so a recorder can tie each draw to its rule), and the
procedure that makes the call and leaves the thread as it was. What the
game does inside the call is owned by `monsters/init.md` and
`monsters/population.md`; this spec owns only the call interface and the
site list.

## Inputs

| Name | Type | Source |
|---|---|---|
| game | pointer | ESI at the tick-return hook `0x0052FD1E` (`tools/original-hooks.md` §3) |
| player unit | pointer | player hash list of the game, GUID 1 (§2) |
| class | i32, monstats row | the tool's caller |
| kind | normal / champion / unique / boss / superunique row | the tool's caller |
| position | subtile x, y | player position plus an offset (§2) |

## Outputs / state changes

The call creates the monster(s) exactly as a game path would: GUIDs taken
(`sim/unit-order.md` §1), one game-seed step per unit, room-seed and
unit-seed draws (§4), region counters (`population.md` §13), umods and
minions. Nothing else is written by the tool except its scratch page and
the thread context, which it restores (§5).

## Rules

### 1. Entry points

All are server functions; each returns the created unit in EAX (0 =
nothing created). "fastcall" means ECX, EDX hold the first two arguments;
the rest are pushed right to left and the callee pops them (`ret N`).
Registers EBX, ESI, EDI, EBP are preserved by every entry.

| # | Address | Registers | Stack ([ESP+4] first) | `ret` | Effect |
|---|---|---|---|---|---|
| 1 | `0x005B2F20` | ECX game, EDX room | x, y, class, mode, spread, flags (u16 in a dword) | 0x18 | builds the create request with no coordinate list and GUID 0, calls `0x005B2A00` (`init.md` §1–§4) |
| 2 | `0x005B30E0` | ECX game, EDX room | x, y, class, mode, GUID, spread, flags | 0x1C | as 1 with a given GUID; meaningful only with flag 0x20 (`population.md` §9.5) |
| 3 | `0x005A43E0` | ECX game, EDX room | coord list (0), class, champion allowed (0/1), x (u16), y (u16), warp check (0/1) | 0x18 | random boss (`population.md` §6.2; `init.md` §16.1): boss spawn, umod choice, minions, umod init |
| 4 | `0x005A49B0` | ECX game, EDX room | x, y, superunique row | 0xC | superunique (`population.md` §11.4, `init.md` §20) |
| 5 | `0x005A48C0` | ECX game, EDX unit | umod (byte in a dword) | 4 | champion mark of a pack member (`init.md` §16.2); returns nothing useful |
| 6 | `0x00463740` | ECX room, EDX x | y | 4 | lookup only: the room (this one or a neighbour) whose subtile box holds (x, y), or 0; no draw |

1. Entry 3 calls the boss spawn `0x005A09E0`, whose own convention is
   EDI game, EBX class, stack room, coord list, x, y, GUID, warp check,
   `ret 0x18` (read at `0x005A4400` and `0x005A4A4E`). It is not needed
   from a tool: entry 3 adds the umod and minion steps a boss must have.
2. Entry 4 refuses (EAX 0) when game +0x6D ≥ 3, when the row has no
   record or a negative class, and when the row's `Stacks` byte (+0x26)
   is 0 and its bit in the boss-spawned bitset (game +0x1D30) is already
   set: one spawn per superunique row per game. With the row's +0x24 byte
   set (`AutoPos`) it ignores x, y and searches the room (`population.md`
   §8, which draws).
3. Entry 1 with flags bit 0x01 is a probe: placement only, EAX 1 on
   success, no unit (`population.md` §9.4).
4. The x, y of entries 1–4 are absolute subtile coordinates; entry 3
   zero-extends them from 16 bits; x = y = 0 in entries 3 and 4 means
   "search the room" (`population.md` §6.3 step 1).

### 2. Pointers and preconditions

1. **Game:** ESI at `0x0052FD1E` (tick driver, after the tick returned;
   game +0xA8 = the frame just run). `tools/original-hooks.md` §3 rule 3.
2. **Player unit:** bucket head at game +0x1120 + 4 × (GUID & 0x7F) of
   the player list (unit type 0, offset 0), next at unit +0xE4
   (`tools/original-hooks.md` §4 rule 1); the single-player player has
   GUID 1, so the head of bucket 1, checked by unit +0x00 = 0 and +0x0C
   = 1.
3. **Position and room:** dynamic path = unit +0x2C; subtile x, y = u16 at
   path +0x02, +0x06; active room = path +0x1C
   (`sim/path-placement.md` §2.1).
4. **Room box:** u32 room +0x4C x, +0x50 y, +0x54 width, +0x58 height
   (subtiles; copied out by `0x00619730`). The no-list placement accepts
   only points inside the box of the room passed (`population.md` §9.1
   rule 3, §9.3 step 3.2.1). For a point outside the player's room, pass
   the room from entry 6 (0 = no loaded room there: do not spawn).
5. **Class:** 0 ≤ class < monstats count (u32 at data tables
   `0x00744304` +0xA80; records of 0x1A8 bytes from +0xA78). The
   creation refuses a class with no monstats2 row or with `enabled`
   clear (`init.md` §4 step 1, `population.md` §9.1).
6. **Mode:** 1 (neutral), as population passes (`init.md` §2).
7. **Game state:** at least one tick has run with the player in client
   state 4 (`tools/original-hooks.md` §1 rule 5); single-player game type
   (`0x007A0610` = 0). Difficulty is game +0x6D.

### 3. Kinds of monster

| Kind | Call | Result (owner) |
|---|---|---|
| normal | entry 1, flags 0 | normal mods and party minions as population (`init.md` §4, §14.1; `population.md` §10) |
| normal, alone | entry 1, flags 0x40 | no party minions |
| champion | entry 1 (flags 0), then entry 5 with umod 16 on the result | type flags 1 \| 4 \| 8, umods [normal mods…, 16], name seed (`init.md` §16.2); no minions; this is how population makes champion pack members |
| unique | entry 3 with champion allowed 0 | type flags 1 \| 8, 1 + difficulty unique umods, minions (`init.md` §16.1, §17 step 2; `population.md` §6.5) |
| random boss | entry 3 with champion allowed 1 | champion (umod 16 or 36–39) with chance monumod row 0 `constants` %, else unique (`init.md` §17 step 1) |
| superunique | entry 4 | type flag 2, `Mod1`–`Mod3`, minion group (`init.md` §20) |
| minion | not callable alone | type flag 0x10 is set only on minions created by a boss (`population.md` §6.5); spawn the boss |

1. No entry forces a chosen champion umod or unique umod set: umods are
   drawn from the unit seed (`init.md` §17). A tool that wants a given
   set must choose the seed, not the umods.
2. The `-seed` path, the tick and injection points are
   `tools/original-hooks.md` §2–§3; nothing here depends on them.

### 4. RNG draw sites

Seeds: R = room seed (room +0x6C) of the room passed; G = game seed (game
+0xD0); U = the new unit's seed (unit +0x20); B = the boss's or leader's
U. "helper" = a call of the `roll` helper (`sim/rng.md` §3); "inline" =
the `mov reg, 0x6AC690C5` of an inlined step (`sim/rng.md` §6), the
address a recorder hooks. The order within one creation is
`init.md` Randomness; this table gives each step its site.

| Step | Seed | Site | Kind | Rule |
|---|---|---|---|---|
| ring parity p | R | `0x005B2BEC` | inline | `population.md` §9.3 step 1 |
| ring offset q = `roll(d)` | R | `0x005B2C20` (p = 1), `0x005B2C38` (p = 0) | helper `0x0045C3E0` | same |
| ring signs sx, sy | R | `0x005B2C60`, `0x005B2C8A` | inline | same |
| water point start `roll(n)` | R | call at `0x005B2737` | helper `0x0045C3E0` | `population.md` §9.2 (frogs only) |
| room point x, y | R | `0x0054DCA9`, `0x0054DCEE` | inline | `population.md` §8 step 2.1 (x = y = 0 only) |
| unit seed | G | `0x00552E31`, then `init_low` at `0x00552E50` | inline + setter | `sim/rng.md` §5.3 |
| appearance variants, variant 0 | U | call at `0x005BDBF1` (per slot with a count > 1) | helper `0x0045C390` | `population.md` §2.4 step 3, via §2.5 (first monster of its class in the level's region only) |
| appearance variants, slot pair | U | calls at `0x005BDC57`, `0x005BDC74` | helper `0x0045C390` | `population.md` §2.4 step 4 |
| appearance variants, new variant | U | `0x005BDCEE` (slot a), `0x005BDD48` (slot b) | inline | `population.md` §2.4 step 5 |
| components, variant set | U | call at `0x00573A03` | helper `0x0045C3E0` | `init.md` §10 step 1 |
| components, per slot | U | `0x00573A8E` | inline | `init.md` §10 step 2 |
| doomknight components | U | calls at `0x00573D6E`, `0x00573D9B` | helper `0x0045C390` | `init.md` §6 step 2 |
| hit points | U | call at `0x00573F8F` | helper `0x0045C3E0` | `init.md` §6 step 8 |
| monprop chance | U | `0x005741C5` | inline | `init.md` §11 |
| monequip slot | U | `0x005D6C19` | inline | `init.md` §12 |
| party size | B | call at `0x004CC7A9` | helper `0x0045C3E0` | `population.md` §10.1 |
| champion test `roll(100)` | U | call at `0x005A0825` | helper `0x0045C390` | `init.md` §17 step 1 |
| unique count `roll(1)` | U | call at `0x005A0876` | helper `0x0045C390` | `init.md` §17 step 2 |
| champion umod pick | U | call at `0x005A05BC` | helper `0x0045C390` | `init.md` §17.1 |
| unique umod pick | U | `0x005A06D8`, `0x005A06F6` | inline | `init.md` §17.2 |
| boss minion count | B | call at `0x005A0C50` | helper `0x0045C3E0` | `population.md` §6.5 step 3 |
| name seed (umod 1) | U | `0x005A0CF0` | inline | `init.md` §19.1 |
| aura pick (umod 30) | temporary seed on the stack | `init` at `0x005A1712`, `init_low` at `0x005A171D`, call at `0x005A1727` | setters + helper `0x0045C390` | `init.md` §19.5 |
| superunique minion count | U | call at `0x005A4CD9` | helper `0x0045C390` | `init.md` §20, `population.md` §11.4 |

1. These are all the helper calls and inline steps inside the functions
   of `init.md` Provenance that the five entries reach directly, plus
   the appearance-variant builder `0x005BDB20` reached through the
   region lookup `0x00547BC0` (found by the recordings). The
   first AI setup `0x005B0E00` and item creation from monequip are owned
   elsewhere (`init.md` Open questions 2, 4); a recorder logs every draw
   on the calling thread between the call and its return, so draws from
   those (or any other) callees appear with their own sites.
2. A recorder ties a draw to a seed by the seed address (helpers: ECX
   at entry) and by the before/after state chain (inline steps): R lives
   in the room, G in the game, U in the unit. The two setters of the
   unit-seed step (`init` at `0x00552E26`, `init_low` at `0x00552E50`)
   give U's address before its first draw.
3. A creation that fails placement makes only R draws (or none when the
   spread is 0; `population.md` §9.3); a refused class makes none.
4. A ring with d = 0 (spread −1, the boss and superunique placements)
   still calls the helper at `0x005B2C20` / `0x005B2C38` with n = 0: a
   recorder sees the call, but the seed does not step (`sim/rng.md` §3
   detail 1). Byte-for-byte comparisons count steps, not helper calls.
5. In the recordings the first AI setup `0x005B0E00` made no draw for
   zombie1, fallen1 and fallenshaman1 (Test vectors), and no thread other
   than the calling one drew during any call.

### 5. Calling an entry from the debugger

1. **Thread:** the single-player server and client run on one thread
   (`tools/original-hooks.md` §1 rule 1); the call runs on the thread
   that hits the hook.
2. **Hook point:** INT3 on `0x0052FD1E` (tick-return; bytes `8B 76 18`, the load of the next game);
   act on the hit whose game +0xA8 equals the requested tick N. The tick
   driver holds the game lock there, no unit is mid-update, and the
   flush that follows sends the new units' messages with the other
   updates of tick N.
3. **Scratch:** once per run, `VirtualAllocEx` one RWX page S in the game;
   byte S+0 = `0xCC` (return trap).
4. **Call:** with EIP rewound to `0x0052FD1E`, save the full thread
   context. ESP −= 4 × (stack argument count) + 4; write [ESP] = S and the
   stack arguments above it in order (first argument at [ESP+4]); set
   ECX, EDX (and nothing else) as §1 says; EIP = the entry; continue.
5. **Return:** the INT3 at S. EAX is the result; the callee has popped
   its arguments, so ESP = saved ESP. Further calls (entry 6 before 1,
   entry 5 after 1) repeat step 4 from this state.
6. **Restore:** set the saved context back (all registers, EFLAGS, EIP =
   `0x0052FD1E`), then step over the original byte of the hook as for any
   INT3 (the recorders' step-over: restore the byte, single-step, re-arm).
7. **What must hold during the call:** no other breakpoint of the tool
   may redirect the thread; RNG hooks (`record_rng.py`) may be armed and
   are stepped over normally; other threads may run (none of them takes
   part in the spawn; their draws are told apart by thread id).
8. **What must not be done:** calling from the client receive path, from
   inside the tick (game +0xA8 not yet the finished frame), or from
   another thread; reusing a saved context after the thread ran on.

## Constants & data dependencies

| Address | Use |
|---|---|
| `0x0052FD1E` | tick-return hook (ESI = game) |
| `0x005B2F20`, `0x005B30E0`, `0x005A43E0`, `0x005A49B0`, `0x005A48C0`, `0x00463740` | entries of §1 |
| `0x00744304` | data tables: +0xA78 monstats records (0x1A8 bytes), +0xA80 count |
| game +0x1120, +0xA8, +0x6D, +0xD0, +0x1D30 | player hash list, frame, difficulty, game seed, boss-spawned bitset |
| unit +0x2C, path +0x02/+0x06/+0x1C | path, subtile x, y, room |
| room +0x4C..+0x58, +0x6C | subtile box, room seed |

## Randomness

The tool draws nothing. The call makes the draws of §4, in the order
`init.md` Randomness gives.

## Edge cases & original bugs

1. Champions made by entry 5 always get umod 16 (`init.md` Edge cases 1).
2. A superunique row spawns once per game unless its `Stacks` byte is
   set (§1 rule 2); a second request returns 0.
3. Spawning shifts every later draw of R, G and the GUID counter, so a
   recording made with a spawn is not comparable to one without it after
   the spawn tick.

## Test vectors

| Input | Expected | Source |
|---|---|---|
Recorded with `spawn.py` (`traces/raw/20261007-0230*-spawn.jsonl`,
gitignored): save `bdAma` (fresh amazon, Rogue Encampment, Normal),
`-nosave`, tick 60, player at subtile (5473, 4708), target (5477, 4712),
spread 4 unless noted, room seed before = {3856415219, 178918734} in
every run. Draw sequences in site order (×n = repeated):

| Request | Created | Draws (helper calls / seed steps) and order |
|---|---|---|
| entry 1, zombie1 (5) | 1: GUID 8 at (5479, 4715), level 1, maxhp 11 × 256, xp 33, components [1,1,2,1,0,0,0,0,1,0,2,…] | 21 / 21: R `0x005B2BEC`, `0x005B2C20`(3), `0x005B2C60`, `0x005B2C8A`; G `0x00552E31`; U `0x005BDBF1`(3) ×8, `0x005BDC57`(8), `0x005BDC74`(7), (`0x005BDCEE`, `0x005BDD48`) ×2, `0x00573A03`(3), `0x00573F8F`(6) |
| entry 1, fallen1 (19) | 4: leader + party of 3 (`0x004CC7A9`(2) after the leader's HP roll), all type flags 0 | 38 / 38: leader as zombie1 (variant counts 3, 4, 2); then per member R ring (d = 3), G, U `0x00573A03`(3), `0x00573F8F`(4) |
| entry 1 fallen1 + entry 5 (umod 16) | 3: leader type flags 0xD, umods [16], level 3, maxhp 3 × base, xp 54, name seed drawn at `0x005A0CF0`; its 2 party members level 4, xp 90 (they are in its minion list, `init.md` §18 step 2) | 32 / 32: as the normal fallen1 spawn with 2 members, then U `0x005A0CF0` |
| entry 3, fallen1, champion allowed 0 | 5: boss type flags 9, umods [17], level 4; 4 minions type flags 0x10, umods [17] (xfer), level 4 | 50 / 49: R d = 0 ring (`0x005B2C20`(0) no step); G; U variants (3 new), `0x00573A03`(3), `0x00573F8F`(4), `0x005A0876`(1), `0x005A06D8`, `0x005A0C50`(4); per minion R ring d = 3, G, U `0x00573A03`, `0x00573F8F`; last U `0x005A0CF0` |
| entry 3, fallenshaman1 (58), champion allowed 1 | 4: unique (roll at `0x005A0825`(100) ≥ 20), umods [9], level 5; 3 fallen1 minions | 43 / 42: R d = 0; G; U `0x00573A8E` ×2 (no variant entry), `0x00573F8F`(5), `0x005A0825`(100), `0x005A0876`(1), `0x005A06D8`, `0x005A0C50`(4); minions (the first builds fallen1's variants); U `0x005A0CF0` |
| entry 4, row 0 (Bishibosh, AutoPos) | 3: fallenshaman1 type flags 0xB, umods [8, 9, 22], at (5460, 4716); 2 minions umods [8, 9] | 45 / 42: R `0x0054DCA9`, `0x0054DCEE` + probe ring d = 0, twice; placement ring d = 0; G; U `0x00573A8E` ×2, `0x00573F8F`(5), `0x005A0C50`(1); 2 minions; U `0x005A0CF0` |

| Input | Expected | Source |
|---|---|---|
| entry 1, spread 0 | EAX 0, no draw | `population.md` §9.3 (r = 0) |
| entry 1, valid class, free point, spread −1, flags 0x40 | draws in the order: R `0x005B2BEC`, `0x005B2C20`/`0x005B2C38`, `0x005B2C60`, `0x005B2C8A`; G `0x00552E31`; then U only | §4, `init.md` Randomness |

## Provenance

- 1.14d `Game.exe`, disassembly via `tools/ghidra/disasm.py fn` of
  `0x005B2F20` (request layout from the stores to [EBP−0x28]…[EBP−4]),
  `0x005B30E0`, `0x005A43E0`, `0x005A49B0`, `0x005A48C0`, `0x005A09E0`,
  `0x00463740`, `0x00619730`; draw sites from `re/exports/all.asm` (every
  `mov reg, 0x6AC690C5` and every call of `0x0045C390`, `0x0045C3E0`,
  `0x00650E30`, `0x00650E40` inside the functions of `init.md` Provenance).
- Confirmed by six `spawn.py` runs (2026-10-07, Test vectors): every
  entry returned a unit with the expected ESP after the callee's `ret`,
  the game ran 25 more ticks after each call, and `check_rng.py` passed
  every recorded draw.
- D2MOO (1.10f) names only (`D2GAME_SpawnMonster_6FC69F10`,
  `D2GAME_SpawnSuperUnique`, as in `init.md` §1); every convention above
  was read on 1.14d, where several entries pass the game in ECX or EDI
  and the room in EDX, unlike 1.10f's stack arguments.

## Open questions

1. Does a spawn at `0x0052FD1E` reach the client in the flush of the same
   tick (0xAC message), or one tick later? Probe: `record_packets.py`
   hooks with a spawn.
