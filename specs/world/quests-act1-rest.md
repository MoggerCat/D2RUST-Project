# Spec: World — Act I quest remainders (Cain's gibbet, Cairn stones, Countess trap, act progression)

- **Status:** draft: every rule read from the 1.14d `Game.exe` disassembly
  (addresses inline) and the live 1.14d `objects.txt` / `missiles.txt` /
  `monstats.txt` rows (`game/extracted/patch_d2`); no recording of a Cain
  rescue, a Cairn stone 0x50 or a Countess kill yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act1`
- **Related specs:** `world/quests.md` (owner of the quest machinery and
  of §10: records, shorthands *broadcast*, *every player*, R, iterate
  ids L1–L5, extra fields of chains 4 and 5); `monsters/init.md` (monster
  spawn `0x005B2F20`: game, room, x, y, class, mode, spread, flags);
  `missiles/missiles.md` §R2.1 (missile parameter record),
  `missiles/bodies.md` (rows cairnstones 288, towerchestspawner 332);
  `drlg/rooms.md` (room lookup `0x00463740`); `sim/tick.md` §5 (unit
  events, object event 7); `sim/intents-events.md` §6 (exact-match
  comparison, masking); `sim/server-messages.tsv` (0x28, 0x50, 0x5D).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–53 |
| Inputs | 54–64 |
| Outputs / state changes | 65–72 |
| Rules | 73–74 |
|   1. A1Q4 gibbet (Cain's cage, object class 26) | 75–147 |
|   2. Cairn stones (object classes 17–21) | 148–195 |
|   3. Town-Cain marker (object class 385, `InitFn` 54) | 196–222 |
|   4. A1Q5 Countess chest trap (`0x005954F0(record, extra)`) | 223–261 |
|   5. Character progression (`0x00538680(client, step, difficulty)`) | 262–285 |
|   6. Party list as read by the quest code | 286–311 |
|   7. Cairn stone-order 0x50: bytes 13–14 | 312–321 |
|   8. Act I clarifications (implementation questions, 2026-10-06) | 322–392 |
|   9. Implementation and wiring questions (2026-10-07) | 393–552 |
| Constants & data dependencies | 553–568 |
| Randomness | 569–575 |
| Edge cases & original bugs | 576–599 |
| Test vectors | 600–631 |
| Provenance | 632–665 |
| Open questions | 666–693 |
<!-- /index -->

## Summary

This file holds the Act I quest functions `world/quests.md` left open:
the A1Q4 gibbet operate and gibbet quest function (Cain freed from
Tristram), the Cairn stone object init and its Tristram-portal timer,
the town-Cain marker object init, the A1Q5 Countess chest trap step, the
character progression update called by the A1Q6 credit, the unwritten
bytes of the Cairn stone-order 0x50, and the party list as the quest code
reads it. Each section answers one `quests.md` open question (5, 7, 11,
12, 13); §8 settles the Act I implementation's reading questions.

## Inputs

| Name | Type | Source |
|---|---|---|
| chain 4 record and extra | quest record, 0x1BC bytes | `quests-act1.md` §10.6 |
| chain 5 record and extra | quest record, 0x120 bytes | `quests-act1.md` §10.7 |
| object init args | dwords: game, object, room, ?, `objects.txt` record, x, y | init dispatcher `0x0054F5D0` (table `0x00731BC0`, index `objects.txt` `InitFn`, record +0x1B1) |
| object operate args | dwords: game, object, player, ?, object class | operate dispatcher `0x00584420` (table `0x00732D18`, index `OperateFn`, record +0x1B3) |
| client save flags | u16 at client +0x0A | client of a player (`0x005531C0`: player data +0x9C) |
| party list | game +0x1D2C | §6 |

## Outputs / state changes

Player quest bits of slot 4 and 6 (via the iterate functions of
`quests-act1.md` §10.6 / §10.8), chain 4 / 5 extra bytes, object modes,
spawned monsters (cain1 146, trap-firebolt 326), portal objects (59, 60),
missiles (332), timers, S→C 0x28 / 0x5D, sound events, the client's
progression field.

## Rules

### 1. A1Q4 gibbet (Cain's cage, object class 26)

`objects.txt` row 26 `Gibbet` has `OperateFn` 10 (pointer `0x00732D40` →
`0x00593480`) and `InitFn` 7 (not quest code).

New chain 4 extra fields (the rest are in `quests-act1.md` §10.6):

| Extra | Type | Field |
|---|---|---|
| +0x3C | u32 | GUID of the player who opened the gibbet (−1 when none) |
| +0x40 | u32 | GUID of the class-17 stone that carries the Tristram portal (§2) |
| +0x44 | u8 | Tristram-portal timer pending |
| +0x45 | u8 | Tristram portal created |
| +0x4C, +0x4D | u8 | quest already done for the game (event 13, `quests-act1.md` §10.6 step 10) |
| +0x5C–+0x60 | u8 × 5 | per-stone reset bytes, index = stone class − 17 (§2; zeroed by the init, never set) |
| +0x62 | u8 | Cain could not be spawned in Tristram |
| +0x66 | u8 | town portal out of Tristram created by §1.2 |
| +0x74 | u32 | scratch: player unit found in Tristram (§1.2 step 4) |
| +0x80 | u32 | cain portal event-7 count in the Rogue Encampment (§9 item 10) |
| +0x84, +0x88 | i32 × 2 | position of the town-Cain marker object (§3) |
| +0x91 | u8 | set to 1 by `0x005944F0` (§3); read by the cain portal's event 7 (§9 item 10) |
| +0x92 | u8 | the town cain portal may advance to mode 3 (§9 item 10) |
| +0x96 | u8 | Cain portal object created by `0x005944F0` (§3) |
| +0xA4 | u32 | GUID of that portal object (§3) |

#### 1.1 Operate `0x00593480` (args: game, object, player, …, class; returns 0)

1. Q = chain 4's record (`0x00543640`). If Q exists: X = Q's extra; end
   unless not-intro ≠ 0, X +0x50 ≠ 1 (Cain not yet gone) and state < 6.
   If Q is absent X is none and the steps go on (chain 4 always exists,
   `quests.md` §2.3).
2. R = the player's current record. R has 4.1 or 4.0: sound event 19 on
   the player, target the player (`0x00553380`). End.
3. D = the `objects.txt` record of the class arg (`0x00640E90`). End if
   the object exists and its mode (+0x10) ≠ 0.
4. Object mode := 1 (`0x00624690`); schedule object event 1 at frame
   (game +0xA8) + (D +0xDC >> 8) (`0x005417D0`, `sim/tick.md` §5).
5. If X: X +0x54 := 3; X +0x3C := the player's GUID (−1 when none).
6. Schedule object event 7 at frame + 17 (event 7 runs §1.2 through
   `quests.md` §9.5).
7. Refresh the object's room (`0x0061AED0(room, 0)`).
8. R: set 4.13, then 4.1; 0x28 to the player (`quests.md` §6.6).
9. Party id of the player (§6) ≠ 0xFFFF: for each party member (§6.2)
   run `0x005930B0` (`quests-act1.md` §10.6 L4 member step: member lacking
   4.0 and 4.1 whose room's level is ≠ 0 and in Act I gets 4.13, 4.1 and
   0x28).

#### 1.2 Gibbet quest function `0x00593290(game, object)` (object event 7)

1. Q = chain 4's record; absent → fatal assert (line 0x68E). X = extra.
2. End unless not-intro ≠ 0 and X +0x50 ≠ 1.
3. X +0x54 := 3; object mode := 3. (x, y) = the object's position + (3,
   3) (`0x0045ADF0`, `0x0045AE20`); room = the object's room.
   C = spawn cain1 (146) at (room, x, y), mode 1, spread −1, flags 0
   (`0x005B2F20`). C none: free spot from (x, y) in room
   (`0x00545340`: size 2, mask 0x100, radius 3 (unused: `0x00545340` never reads this sixth argument, `[ebp+0x14]`; the search runs to the limit), limit 100); found →
   C = spawn at the free spot in its room with the same arguments.
4. C none (both tries failed): a debug log (`0x00544070`, no effect);
   X +0x74 := 0; every player `0x00593220` (the first player, in walk
   order, whose room's level is 38 Tristram is stored at X +0x74 and the
   walk stops). If X +0x74 ≠ 0 and X +0x66 = 0: create a portal object
   (`0x0056D130`: owner = that player, the gibbet's room, (x + 3, y + 3)
   = gibbet position + (6, 6), destination level 1, class 59, last
   argument 0 = search a free spot); created → X +0x66 := 1. Then if X
   +0x51 = 0: X +0x52 := 1 (Cain still to spawn in town). X +0x62 := 1.
5. C exists: C unit +0xC4 |= 0x3000000; P = the player with GUID X +0x3C
   (`0x00552F60`, type 0); P exists → sound event 48 on P, target none.
6. Both cases: every player L4, then every player L5 (`quests-act1.md`
   §10.6); flags (+0x14) := 0; broadcast(6, 0) with iterate L1.

State (+0x0C) is not changed here; the gibbet's quest state was set by
the stones (`quests-act1.md` §10.6 stone operate, state 5).

### 2. Cairn stones (object classes 17–21)

`objects.txt` rows 17–21 (`StoneAlpha`, `StoneBeta`, `StoneGamma`,
`StoneDelta`, `StoneLambda`) have `InitFn` 6 (pointer `0x00731BD8` →
`0x005935E0`) and `OperateFn` 9 (`0x00593710`, `quests-act1.md` §10.6).

#### 2.1 Stone value

The stone value the stone operate compares with the order (args +0x10)
is the operated object's class id: the operate dispatcher `0x00584420`
stores its class argument there (and the gibbet operate reads the
`objects.txt` record of the same slot, §1.1 step 3). So the order values
17–21 are the classes of the five stones.

#### 2.2 Init `0x005935E0` (args: game, object, …)

c = the object's class (−1 when no object).

1. No chain 4 record: object mode := 2 unless it is 2 already. End.
2. not-intro ≠ 0 and X +0x4C = 0 (quest live, starter had not done it):
   1. X +0x4D ≠ 0 or X +0x50 ≠ 0: object mode := 2. End.
   2. Else, if the byte X +0x4B + c = 1: set it to 0 and object mode :=
      0. Else nothing.
3. Otherwise (not-intro = 0, or X +0x4C ≠ 0): X +0x4C := 0. If X +0x45 =
   0 and c = 17: X +0x40 := the object's GUID; if X +0x44 = 0: X +0x44
   := 1 and a timer (record, `0x00592D50`, period 1) (`quests.md` §5).
   Then object mode := 2.

Step 3 clears +0x4C, so only the first stone initialised after a
"done" event 13 takes it; later stones take step 2.1 (+0x4D stays 1).

#### 2.3 Tristram-portal timer `0x00592D50(game, record)`

1. S = the object with GUID X +0x40 (type 2). None: X +0x44 := 0;
   return 1.
2. (x, y) = S's position (static path +0x0C, +0x10 for objects).
   Create a portal object (`0x0056D130`: owner none, S's room, (x + 4,
   y + 4), destination level 38 Tristram, class 60, last argument 1 =
   exactly at that point).
3. Created: X +0x45 := 1; X +0x44 := 0; return 1. Failed: return 0 (the
   timer runs again every 2 updater ticks until it succeeds or S is
   gone).

When the quest is live the stones open the portal through the
cairnstones missile (288) instead: the stone operate's fifth stone
creates that missile (`0x0056EDE0`, §4.3), and the missile creates the
class-60 portal (`missiles/bodies.md`).

### 3. Town-Cain marker (object class 385, `InitFn` 54)

`objects.txt` row 385 `Dummy` has `InitFn` 54 (pointer `0x00731C98` →
`0x005940E0`). Init: chain 4's record must exist (else nothing). X +0x6C
:= the object's GUID (−1 when none); X +0x70 := 1; X +0x84, +0x88 :=
init args x, y. If X +0x52 = 1 and X +0x51 = 0: town Cain spawn
(`quests-act1.md` §10.6 step 15) at (x, y) in the init args' room. The
event-3 spawn of `quests-act1.md` §10.6 step 3.3 looks X +0x6C up as an
object (type 2).

**Cain leaves Tristram** `0x005944F0(game, unit)` is the "spawn the town
portal" call of the NpcOutOfTown AI for class 146 `cain1`
(`monsters/ai-bodies.md` §9.32, `0x005E7880` / portal set-up `0x005E77A0`;
the code pointers at `0x005E77F3` and `0x005E7943` select it by class):

1. Chain 4's record must exist (`0x00543640`), else nothing; X = its
   extra. X +0x91 := 1, X +0x52 := 1.
2. Only when X +0x70 = 1 (the marker registered): the marker object
   (type 2, GUID X +0x6C, `0x00552F60`) must exist; this init runs
   again with {game, marker, its room, its x, its y}, so with X +0x52 =
   1 the town Cain spawns when X +0x51 = 0.
3. Then (still under step 2's condition), when X +0x51 = 1: the Cain
   monster (type 1, GUID X +0x68) must exist; at its position (`0x00620870`), in the room containing it
   (cell lookup `0x00463740` from its room), object 189 `cain portal` is
   allocated (`0x00555230(type 2, class 189, …, mode 1)`); on success X
   +0x96 := 1 and X +0xA4 := the object's GUID.

### 4. A1Q5 Countess chest trap (`0x005954F0(record, extra)`)

Called by the Countess's event 8, by the chest init `0x00595A50` and by
the chest's event 7 `0x005956C0` (`quests-act1.md` §10.7). E = extra; list =
the chest GUIDs at E +0x68 (u16 count at E +0x88).

1. End unless killed (E +0x118) ≠ 0, trapped (E +0x119) = 0 and the
   count ≠ 0.
2. T := none. For each list entry in order (the count is re-read every
   iteration):
   1. C = the object with that GUID (type 2). None → next entry.
   2. (x, y) := the Countess's death position (E +0x110, +0x114).
   3. If T is none:
      1. room := the room holding (x, y) among C's room and its list
         (`0x00463740`). Room found: T := spawn trap-firebolt (326) at
         (room, x, y), mode 12, spread −1, flags 8 (`0x005B2F20`).
      2. No room or T none: (x, y) := C's position (`0x00620870`) + (5,
         5); room := `0x00463740` from C's room (not tested); T := the
         same spawn there. T none → next entry (nothing else for this
         chest).
   4. E +0x119 := 1.
   5. Create missile towerchestspawner (332) through `0x0056EDE0`
      (§4.3): owner T, skill 0, skill level 1, position = C's position.
      Created M: M's missile data +0x28 := C's GUID (`0x0064A710`), +0x2C
      := 0 (`0x0064A760`) (D2MOO: the target-coordinate pair, here a
      chest link read by the missile's server-do function 18,
      `missiles/srvdo.tsv`); refresh M's room (`0x0061AED0(room, 0)`).

So one trap monster is spawned per game (the first success) and every
listed chest gets its own missile owned by it.

#### 4.1 Missile helper `0x0056EDE0(game, owner, skill, level, class, x, y)`

Owner none → none. x = y = 0 → (x, y) from the owner (`0x0056D2C0`);
still 0 → none. Distance from the owner to (x, y) (`0x006417F0`) > 100 →
none. Else a zeroed parameter record (`missiles/missiles.md` §R2.1):
flags 1 (position given), owner, class, x, y, skill id, skill level;
create (`0x0059FA30`) and return the missile.

### 5. Character progression (`0x00538680(client, step, difficulty)`)

Called by the A1Q6 credit (`quests-act1.md` §10.8, `0x00596210`: client of P
via `0x005531C0`, step 1, game difficulty +0x6D) and by six Act II–V
sites (`0x0058DCE2`, `0x0058DD65`, `0x0058E4F1`, `0x0059C848`,
`0x005B4D77`, `0x005BC182`).

Client +0x0A is a u16 of save flags: bit 5 = expansion character, bits
8–12 = progression p.

1. m = 5 if bit 5 is set, else 4.
2. n = m · difficulty + step.
3. If p ≤ n: bits 8–12 := n (bits 0–7 and 13–15 kept). Else nothing.

Registers (re-read 2026-10-06; `quests.md` open question 13 stays
closed): client in ecx, step in edx, difficulty on the stack (`ret 4`);
m = ((flags & 0x20) | 0x80) >> 5, and step 3 is a signed "n < p → skip"
(`0x005386AA`). n is written unmasked (n << 8 or-ed into the kept bits);
every caller keeps n below 32.

The field is never lowered. Nothing is sent here; the save code writes
the flags (future owner: the character save spec, header progression;
not written).

### 6. Party list as read by the quest code

Future owner: `world/party.md` (not written). The quest code only reads:

#### 6.1 Party id `0x00554630(unit)`

0xFFFF unless the unit exists, its game pointer (+0x80) ≠ 0 and it is a
player (type 0). Then `0x00540710(game, unit)`: fatal unless a player
and a game; key = the owner GUID of the unit's inventory (unit +0x60,
inventory magic 0x01020304, owner GUID at +0x24; −1 when the magic
differs) when it has one, else the unit's GUID. Walk the parties from
the list (game +0x1D2C; none → 0xFFFF): first party at list +0x04, next
at party +0x08; each party has a u16 id at +0x00 and a member list at
+0x04 (node: GUID u32 +0x00, next +0x04). Return the id of the first
party with a member equal to the key, else 0xFFFF.

#### 6.2 Members `0x00540510(game, id, fn, arg)`

Fatal if game +0x1D2C is null. Find the first party with that id (none
→ nothing); id 0xFFFF → nothing. For each member node in list order:
the player with that GUID (`0x00552F60`, type 0); present → fn(game,
player, arg). There is no early stop.

A single player is in no party, so every party step of Act I does
nothing there.

### 7. Cairn stone-order 0x50: bytes 13–14

`0x00593CB0` fills a 15-byte stack buffer: byte 0 = 0x50, bytes 1–2 =
u16 4, bytes 3–12 = the five order values − 17 (u16 each). Bytes 13–14
are never written; `0x0053D7E0` copies 15 bytes (u32 × 3, u16, u8) and
sends them. The original therefore sends leftover stack there. d2rs
writes 0; exact-match comparison (`sim/intents-events.md` §6) masks
bytes 13–14 of this message (and bytes 5–14 of the `trs ` form,
`quests.md` §9.4).

### 8. Act I clarifications (implementation questions, 2026-10-06)

Each item settles a reading of `quests-act1.md` §10 that the Act I
implementation left open (`impl-quests-act1` note, items 4–10). Read
from the 1.14d disassembly at the addresses given.

1. **Tree operate drop result (§10.6 tree operate `0x00593AF0`).** The
   drop helper `0x00559A30` returns the item it created (null when
   nothing dropped): its return value is the result of the item-creation
   call `0x00558D90` (`0x00559C96`); the `&out` argument only receives a
   copy of the 0x84-byte creation request. The tree operate tests that
   return (`0x00593C16`) and stores the item's GUID (item +0x0C) at extra
   +0x38 (`0x00593C4C`). Extra +0x38 has exactly three writers in the
   chain-4 code (init `0x005972C2` := 0, message 112 `0x00592330`, tree
   operate) and no reader, so the value is never observable; d2rs keeps
   it for record parity only.
2. **Tree operate order.** State := 4 (`0x00593BFB`) runs before the drop
   (`0x00593C11`). A failed drop keeps state 4 and skips the broadcast,
   the extra flags and the object mode (the object stays mode 0), but
   still sets +0x47 := 1 and +0x30 := the object's GUID (`0x00593C67`).
   The call `0x00592860` between the drop code and the state change only
   writes a debug log. The literal reading is correct.
3. **Town Cain spawn anchor (§10.6 step 15, `0x00592960`).** The anchor
   is the town-Cain marker **object** (unit type 2: `0x00552F60` with
   type 1 + 1 at `0x00596F19`), not a monster. (x, y) are its position
   (`0x0045ADF0` / `0x0045AE20`) and R0 its room (`0x00620BB0`), which
   for an object is the first dword of its static path, read without a
   null test. A marker found by GUID is in its room's unit list, so R0 is
   never null on a 1.14d path; d2rs treats a marker without a room as an
   invariant violation. The point search tests **21** points, i = 0
   through 20 inclusive (`cmp edx, 0x14; jle`, `0x005929D4`).
4. **Den of Evil region (§10.4 event 8 step 2).** The region array at
   game +0xF0 holds one region per level id 1 … count − 1, built at game
   creation (`monsters/population.md` §2.1), so level 8's region exists
   whenever `levels.txt` has more than 8 rows (1.14d: always). The null
   test at `0x0059028E` leads to the internal-error exit (`0x00408A60`,
   line 0x1E4, then `0x00681E09(−1)`); d2rs reports it as a fatal error
   (`QuestError::Fatal(0x00590293)`), not reachable with 1.14d data.
5. **Event 0 without a player (A1Q3 `0x005916A0`, A1Q4 `0x00592580`).**
   Both read the player's data record through `0x006221A0` before any
   test (`0x005916B3`, `0x005925BB`); that helper exits with an internal
   error on a null unit (line 0xFB4) or a non-player (0xFB6). Event 0 is
   raised only by the NPC text builders (`quests.md` §7.1, §7.2), which
   always pass the player, so the case cannot happen; d2rs reports it as
   fatal, not as a silent return. ("−1 when none" in §10.6 step 1 is the
   NPC's class, not the player.)
6. **A1Q6 event 3 (§10.8 step 3.1, `0x00596010`).** The state is
   written only when it is below 3 (`cmp [record+0x0C], 3; jae` at
   `0x00596035`): state 0–2 → state := 3, changed := 1; state 3, 4 or 5
   is kept and changed := 0. Entering Catacombs after the kill does not
   reset state 4 or 5.
7. **A1Q6 O7 (`0x00596490`).** O7 returns 1 for the first player in walk
   order (item 9) whose room's level is 37, whether or not the portal
   creation `0x0056D130` succeeded; that stops the walk, so at most one
   portal per timer firing. A player without a room, or in another
   level, returns 0.
8. **Kashya's mercenary order (§10.5 A1Q2 event 11, message 92).** Call
   order in `0x00590980`: 0x28 to the player (`0x005455B0`, at
   `0x00590ACD`), GUID added to the record list (`0x00545200`,
   `0x00590AE6`), mercenary (`0x00579180`, `0x00590AF6`: its 0x50 and
   the hireling's creation messages), then the text refresh
   (`0x00545780`, `0x00590B04`: 0x27, 0x29). The mercenary's messages
   precede 0x27 / 0x29.
9. **"Every player" order (§10.1).** `0x005537D0(game, 0, arg, fn)`
   walks the player hash (game +0x1120) bucket 0 … 127, each bucket from
   its head (`sim/unit-order.md` §2 r4), skips a player with state 7
   (`0x00639DF0(unit, 7)`), reads the next link (unit +0xE4) after the
   call, and stops at the first call returning 1. It exits with an
   internal error when `fn` fails `IsBadCodePtr`. A host's player list
   must yield exactly this order.

### 9. Implementation and wiring questions (2026-10-07)

Each item answers one question of the `impl-quests-act1-rest` note
(HANDOFF §7 ninth set, QA-1–QA-6) or of the `wire-world-staging` note
(§3 item 1, WW-6), read from the 1.14d disassembly at the addresses
given.

1. **QA-1: monster spawn with a null room (§4 step 2.3.2).** The
   creation function `0x005B2A00` (through `0x005B2F20`, which builds the
   request with no coordinate list) first checks the class (monstats and
   monstats2 rows, table reads only), then reads the room box through
   `0x00619730`, which returns an all-zero box for a null room
   (`0x0061975D`), and then returns null when the room is null
   (`0x005B2B50`). Nothing is allocated and no seed is stepped before that
   test. The room lookup `0x00463740` itself returns null for a null
   start room (`0x0046374B`). So the trap retry with no room spawns
   nothing, draws nothing and goes to the next entry; d2rs's "spawn
   skipped" is the 1.14d behaviour.
2. **QA-2: an object or marker without a room.** The room of an object
   (`0x00620BB0`) is its static path +0x00, read without a test; it is
   null for a unit left in a freed room (`drlg/rooms.md` §8.2 rule 4:
   static room := null, the unit stays allocated and keeps its GUID).
   No site below tests the room itself; 1.14d then does this:

   | Site | With a null room |
   |---|---|
   | gibbet event 7 `0x00593290` | both cain1 spawns return null (item 1); the free-spot search `0x00545340` finds nothing (every point's room lookup is null; out room := 0 at `0x005454CE`); §1.2 step 4 runs. Its portal call `0x0056D130` exits with an internal error on a null room (line 0xE42, `0x0056D147`), so the game ends there when a player is in Tristram and X +0x66 = 0; otherwise step 4 sets X +0x52 / +0x62 and step 6 runs normally |
   | Cain leaves Tristram `0x005944F0` | the marker init re-run (§3 step 2) passes the null room to the town Cain spawn `0x00592960`: every spawn try returns null (item 1): no Cain, X +0x51 stays 0, X +0x52 stays 1, no draw. Step 3: the Cain monster's room is its dynamic path +0x1C (0 without a path); `0x00463740(null)` → null → no portal object |
   | event-3 town Cain (`quests-act1.md` §10.6 step 3.3, §8 item 3) | as the row above: nothing spawns, nothing is drawn |
   | trap chest `0x005954F0` | while T is none: both room lookups are null (`0x00595579`, `0x005955C8`), both spawns null → next entry (no missile for that chest). Once T exists the chest's room is not read: the missile is still created at the chest's position (static +0x0C, +0x10), owned by T |
   | gibbet operate refresh `0x0061AED0(room, 0)` | null room → nothing (`0x0061AED6`) |

   d2rs reproduces these effects instead of reporting a fatal error;
   only the gibbet portal row is fatal (the original's internal-error
   exit at `0x0056D147`). This replaces the "invariant violation" reading of
   §8 item 3 for a marker without a room: the town Cain spawn simply
   finds no room and spawns nothing.
3. **QA-3: the L4 party step (`quests-act1.md` §10.6 L4, `0x00593130`).**
   The party walk (`0x00554630`, then `0x00540510` with `0x005930B0`;
   `0x00593195`–`0x005931AF`) is inside the test: it runs only after P
   passed "neither 4.0 nor 4.1, room not null, room level 38" and got
   4.13, 4.1 and its 0x28. A P that fails any test returns at once. L4
   always returns 0. The member step `0x005930B0` tests the member the
   same way, with "room not null, level ≠ 0 and its act (`0x006427F0`) =
   0" in place of level 38. The reading in §1.1 step 9 and in the
   `quests-act1.md` §10.6 table is correct.
4. **QA-4: a player without a client (§5).** The A1Q6 credit
   `0x00596210` takes the client from `0x005531C0` (player data +0x9C;
   null only for a null unit or a non-player) and passes it to
   `0x00538680` with no test; `0x00538680` reads client +0x0A at once
   (`0x00538684`). A null client would fault, so 1.14d has no "no
   progression" path. d2rs treats a player without a client here as an
   invariant violation (fatal), like item 2's fatal row; a host gives
   every player a client.
5. **QA-5: extra +0x38 has no reader (§8 item 1).** Chain 4's extra is
   reachable only through record +0x18 of chain 4. Every constant lookup
   of chain 4 (`0x00543640` with id 4; an `all.asm` scan of the id
   argument of every call site, including the `lea edx, [eax + k]`
   forms) lies in the chain-4 functions `0x005928C0`–`0x00597310`; the
   sequence walk (`quests-act1.md` §10.1, chain 3's `seq_id` = 4) hands
   chain 4's record only to chain 4's own sequence function
   `0x00593D70`; init 7 (item 8) walks the record list itself and calls
   `0x00594060`. None of these reads +0x38. Confirmed: no reader in
   1.14d.
6. **QA-6: chain 37's event 11 `0x0058F870` before Kashya's reward.** Its
   body is `quests-act1.md` §10.3 (Act I intro, event 11): a jump table
   on NPC class − 147 (`0x0058F8CC`, 8 entries: 147 gheed → messages 45,
   46; 148 akara → 11, 12; 150 kashya → 24, 25; 154 charsi → 36, 37; 149
   and 151–153 nothing); a listed pair sets the player's first-talk bit
   for that NPC (`0x00572360`; `quests.md` §6.7, record +0x00). It sends
   nothing, draws nothing and does not read chain 2. For Kashya's reward
   message 92 it does nothing at all, so running it first (records are
   visited newest first, `quests.md` §2.3: chain 37 before chain 2) has
   no observable effect on the reward's messages.
7. **Quest object init order (wiring note §3 item 1; `sim/rng.md`
   §5.3).** An object's `InitFn` runs inside its allocation
   `0x00555230`: after the unit's game-seed step (`0x00552DF0`, called at
   `0x0055530E`) and before the creator gets the object back
   (`world/objects.md` §3: before the object is added to the world and
   before the `PreOperate` draw). For the town-Cain marker (class 385,
   `InitFn` 54, `0x005940E0`, §3) every draw of the town Cain spawn
   `0x00592960` therefore happens at that point, before any later draw of
   the code that creates the marker (the room's next preset object or
   monster):
   1. X +0x6C, +0x70, +0x84, +0x88 are written first; the spawn runs
      only when X +0x52 = 1 and X +0x51 = 0.
   2. The point and free-spot searches (`0x00619730`, `0x00545340`) draw
      nothing.
   3. Each spawn try (`0x005B2F20`, class 265, mode 1; spread 5, then up
      to 20 tries with spread 10, then one with 15) draws as
      `monsters/population.md` §9.3 states (the active-room seed of the
      try's room, for the ring search) and, when the monster is created,
      as `monsters/init.md` §4 states. Tries stop at the first success.
   4. The marker is added to the world only after the init returns, so
      Cain's placement does not see it (row 385 has size 0 × 0 in 1.14d
      `objects.txt` in any case).

   The same holds for every quest `InitFn` of this file (6, 7, 9, 54,
   61): its draws, timers (6: the Tristram-portal timer) and object
   events (61: event 7) happen inside the allocation, so they take their
   place in the seeds and in the timer lists before the creator's next
   allocation. Inits 6, 7, 9 and 61 also set the object's mode, which
   the add-to-world footprint stamp after the init reads
   (`world/objects.md` §3). A host that runs the init later (the wiring's
   drained queue) is therefore exact for none of the five.
8. **WW-6: gibbet init (`InitFn` 7, `0x00544990`, class 26).** Finds
   chain 4's record by walking the record list from the control's newest
   record (no assert). Not found (or no quest control): object mode := 2
   unless it is 2. Found: `0x00594060(record, init args)`: object mode :=
   X +0x54 (an i32: 3 once the gibbet was opened, else 0); X +0x48 := 1;
   X +0x34 := the object's GUID (−1 when none). No draw, no message.
9. **WW-6: Inifuss tree init (`InitFn` 9, `0x00593FC0`, class 30).**
   Chain 4's record absent: object mode := 2 unless it is 2. Present: X
   +0x47 := 1 (the tree's GUID at +0x30 is not written here; the tree
   operate writes it); if not-intro = 0 or X +0x50 ≠ 0: X +0x58 := 1;
   then object mode := X +0x58 (i32). No draw, no message.
10. **WW-6: Cain portal (class 189 `Dummy` "cain portal", `InitFn` 61,
    `0x00594290`).** Init: object mode := 1; object event 7 at frame
    (game +0xA8) + 25 (`0x005417D0`). Event 7 runs through `quests.md`
    §9.5 (class 189, room level in Act I → chain 4's record and
    `0x005942C0(record, object)`; no record → nothing). `0x005942C0`, by
    the object's mode m (0 when no object):
    - m = 1: mode := 2.
    - m = 2: if the object's room level is 1 (Rogue Encampment): X +0x80
      (u32) += 1, and when it is then > 5, X +0x92 := 1; then if X +0x92
      ≠ 0: mode := 3. Other levels: mode := 3 when X +0x91 ≠ 0 (set when
      Cain leaves Tristram, §3).
    - m = 3: mode := 4.
    - other modes: nothing.

    In every case event 7 is scheduled again at frame + 25, so the
    object keeps a 25-frame event for its lifetime. X +0x80 and +0x92
    are chain-4 fields shared by every Act I cain portal of the game and
    never reset by this code. No draw, no message.
11. **WW-6: Wirt's body (class 268).** Live 1.14d `objects.txt` row 268
    has `InitFn` 0 (no init) and `OperateFn` 33. `InitFn` 37
    (`0x0059DA50`) is not Wirt's: it looks up chain 13 (A2Q6) and no
    1.14d row uses it (`object-functions.tsv` "Unused37"; Act II owner).
    Operate 33 `0x00583E70` (operate args: game, object, player, …,
    class; returns 1 in every case):
    1. End if the object exists and its mode ≠ 0.
    2. Object drop code (+0xB8) := `leg `.
    3. Drop at the object: `0x00559A30(game, object, 2, &out, 0, −1, 0)`
       (the same call as the tree operate). No item → end (the drop code
       stays `leg ` and the mode 0, so the next operate tries again).
    4. Item dropped: object mode := 1; object event 1 at frame + (D
       +0xDC >> 8) + 1, D = the `objects.txt` record of the class
       argument (`0x00640E90`); object event 7 at frame + 10. Event 7
       then runs the gold piles (`quests-act1.md` §10.6 Wirt's body,
       `0x00594630`).
12. **WW-10: `0x0061AED0(room, clear)` ("refresh room").** Room null →
    nothing. Else, on the room's DRLG room (active room +0x10,
    `0x0061BAC0`): clear = 0 → flags (+0x28) |= 0x400000; clear ≠ 0 →
    flags &= ~0x400000. 0x400000 is the flag that makes the room-removal
    test false (`drlg/rooms.md` §8 rule 1), so the quest calls with 0
    (gibbet operate §1.1 step 7, trap missile §4 step 2.5, stone missile
    `quests-act1.md` §10.6) keep that room active for the rest of the
    game; the missile bodies call it with 1 (`missiles/bodies.md`) and
    release it. It sends nothing.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| gibbet | object 26, `OperateFn` 10, `InitFn` 7 | `objects.txt` (1.14d) |
| stones | objects 17–21, `InitFn` 6, `OperateFn` 9 | `objects.txt` |
| town-Cain marker | object 385 `Dummy`, `InitFn` 54 | `objects.txt` |
| portals | object 59 (to town), 60 (to Tristram) | `objects.txt` `Portal` rows |
| monsters | cain1 146, trap-firebolt 326 | `monstats.txt` |
| missiles | cairnstones 288, towerchestspawner 332 | `missiles.txt` |
| init / operate tables | `0x00731BC0` (index record +0x1B1, < 0x50), `0x00732D18` (index +0x1B3, < 0x65; classes 22, 121, 122 skipped) | `0x0054F5D0`, `0x00584420` |
| gibbet delays | event 1 at + (D +0xDC >> 8), event 7 at + 17 frames | §1.1 |
| Cain offset | +3, +3; portal +6, +6; stone portal +4, +4; chest retry +5, +5 | §1.2, §2.3, §4 |
| cain portal | object 189, `InitFn` 61; event 7 every 25 frames | §9 item 10 |
| Wirt's body | object 268, `InitFn` 0, `OperateFn` 33; drop code `leg `; event 7 at + 10 | §9 item 11 |

## Randomness

No quest-seed draws. The monster spawns (cain1, trap-firebolt), the
free-spot searches, the portal creation and the missile creation draw
from their own seeds as their owners state (`monsters/init.md`,
`missiles/missiles.md`); this file adds no draw of its own.

## Edge cases & original bugs

1. One trap monster per game: after the first successful spawn, every
   further chest only gets a missile owned by it (§4).
2. The trap's first try is at the Countess's death position, not at the
   chest (§4 step 2.3).
3. The per-stone reset bytes X +0x5C–+0x60 are zeroed at init and never
   set, so §2.2 step 2.2 never resets a stone. With no object (c = −1)
   the byte read is X +0x4A (stone order computed); the dispatcher always
   passes an object.
4. The Tristram portal timer (§2.3) retries forever while its stone
   exists and creation fails.
5. The gibbet without a player in Tristram and with a failed Cain spawn
   makes no portal and no Cain; X +0x52 then lets the town Cain spawn
   (`quests-act1.md` §10.6 step 3.3).
6. 0x50 bytes 13–14 are stack leftovers (§7).
7. A roomless gibbet whose Cain spawn fails while a player is in
   Tristram ends the game with an internal error (portal creation with a
   null room, §9 item 2).
8. The cain portal's town counter X +0x80 is shared by every cain portal
   of the game and never reset (§9 item 10).
9. A failed Wirt's body drop leaves the drop code `leg ` and mode 0, so
   the body can be operated again (§9 item 11).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| progression: client +0x0A = 0x0520 (exp, p 5), step 1, difficulty 2 | 0x0B20 | §5 |
| progression: 0x0000 (classic), step 1, difficulty 1 | 0x0500 | §5 |
| progression: 0xE720 (exp, p 7), step 1, difficulty 0 | unchanged (7 > 1) | §5 |
| progression: 0x0120 (exp, p 1), step 1, difficulty 0 | 0x0120 rewritten (p = n) | §5 |
| stone 0x50 with order [18, 20, 17, 21, 19] | `50 0400 0100 0300 0000 0400 0200 ?? ??` (bytes 13–14 masked) | §7, `quests-act1.md` §10.6 |
| gibbet operate, R slot 4 = 0x0002 (4.1) | sound 19; nothing else | §1.1 step 2 |
| gibbet operate, state 6 | nothing | §1.1 step 1 |
| gibbet operate, R slot 4 = 0, object mode 0, single player | object mode 1; events 1 and 7 scheduled; X +0x54 = 3; R slot 4 = 0x2002; 0x28 | §1.1 |
| gibbet event 7, Cain spawned, player P in Tristram lacking 4.0/4.1, status default rule (state 5, init_no 6) | P: 4.13, 4.1, 0x28; then `5d 04 00 06 0000` | §1.2, `quests.md` §6.1, `quests-act1.md` §10.6 L1, L4 |
| same, a second player Q in Act I outside Tristram | Q: 4.14, `5d 04 00 0c 0000` (L5), then `5d 04 00 0c 0000` (L1, now → 12) | §1.2, `quests.md` §6.1 |
| trap step, 2 chests, killed 1, trapped 0, first spawn at the death position succeeds | 1 monster 326; 2 missiles 332 (one per chest, data +0x28 = chest GUID); E +0x119 = 1 | §4 |
| trap step again | nothing | §4 step 1 |
| A1Q6 event 3, b = 34, not-intro 1, state 4, status 2 | state stays 4; no O2 walk, nothing sent | §8 item 6 |
| A1Q6 event 3, b = 37, state 1, status 2 | state 3; every player O2; nothing broadcast | §8 item 6, `quests-act1.md` §10.8 |
| tree operate, drop fails, state 3 | state 4; no 0x5D; object mode 0; +0x47 1; +0x30 = object GUID | §8 item 2 |
| A1Q2 message 92 with 2.1, no hireling | 0x28, then 0x50 (u16 2) and the hireling's messages, then 0x27, 0x29 | §8 item 8 |
| stone init, not-intro 1, X +0x4C 0, +0x4D 0, +0x50 0 | mode unchanged | §2.2 |
| stone 17 init, X +0x4C 1, +0x45 0, +0x44 0 | +0x4C 0; +0x40 = GUID; +0x44 1; timer period 1; mode 2 | §2.2 |
| trap step, T none, chest without a room | no spawn, no draw, no missile for that chest; next entry | §9 items 1, 2 |
| trap step, T exists, next chest without a room | missile 332 at the chest's position, owner T | §9 item 2 |
| gibbet init, X +0x54 = 3 | object mode 3; X +0x48 1; X +0x34 = GUID | §9 item 8 |
| tree init, not-intro 1, X +0x50 0, X +0x58 0 | X +0x47 1; mode 0 | §9 item 9 |
| tree init, not-intro 0 | X +0x47 1; X +0x58 1; mode 1 | §9 item 9 |
| cain portal in level 1, mode 2, X +0x80 = 5, +0x92 0 | X +0x80 6; X +0x92 1; mode 3; event 7 at + 25 | §9 item 10 |
| cain portal in Tristram, mode 2, X +0x91 0 | mode 2; event 7 at + 25 | §9 item 10 |
| Wirt's body operate, mode 0, drop succeeds, frame f, D +0xDC = 0x100 | drop code `leg `; mode 1; event 1 at f + 2; event 7 at f + 10 | §9 item 11 |
| `0x0061AED0(room, 0)` then `(room, 1)` | DRLG room flag 0x400000 set, then cleared | §9 item 12 |

## Provenance

- 1.14d `Game.exe` disassembly (`tools/ghidra/disasm.py fn`/`at`):
  `0x00593480`, `0x00593290`, `0x00593220`, `0x005930B0`, `0x005935E0`,
  `0x00592D50`, `0x005940E0`, `0x005944F0` (head only), `0x005954F0`,
  `0x0056EDE0`, `0x0064A710`, `0x0064A760`, `0x0056D130` (argument
  order, free-spot flag), `0x00538680`, `0x00596210`, `0x005531C0`,
  `0x00554630`, `0x00540710`, `0x0063D450`, `0x00540510`, `0x00593CB0`,
  `0x0053D7E0`, dispatchers `0x0054F5D0` and `0x00584420`. Register
  arguments are taken from the disassembly (the decompile drops them).
- §8 (2026-10-06): `0x00593AF0`, `0x00559A30` (return path to
  `0x00558D90`), the +0x38 writers found by an `all.asm` scan of
  0x00592000–0x00597FFF, `0x00596DE0`, `0x00592960`, `0x00620BB0`,
  `0x00619730`, `0x00590260`, `0x00547BB0`, `0x005916A0`, `0x00592580`,
  `0x006221A0`, `0x00596010`, `0x00596490`, `0x00590980`, `0x005537D0`.
- `objects.txt`, `missiles.txt`, `monstats.txt` rows from
  `game/extracted/patch_d2` (1.14d).
- D2MOO 1.10f gave names only: `CLIENTS_UpdateCharacterProgression`
  (client save flags, progression bits 8–12; same formula), the inventory
  magic and owner field, `D2MissileDataStrc` +0x28/+0x2C, party lookup
  by owner. Each was matched to the 1.14d code above.
Ghidra backlog (2026-10-06): `0x005944F0` read in full; its selectors
at `0x005E77F3` (in `0x005E77A0`) and `0x005E7943` (in `0x005E7880`);
object row 189 from live `objects.txt`.
- §9 (2026-10-07): `0x005B2F20`, `0x005B2A00` (head to the room test),
  `0x00619730`, `0x00463740`, `0x00620BB0`, `0x00620870`, `0x00545340`,
  `0x0056D130` (head), `0x0061AED0`, `0x0061BAC0`, `0x00593130`,
  `0x005930B0`, `0x00596210`, `0x005531C0`, `0x00538680`, `0x0058F870`
  (jump table `0x0058F8CC` read from `Game.exe`), `0x00572360`,
  `0x00555230` (call order), `0x00544990`, `0x00594060`, `0x00593FC0`,
  `0x00594290`, `0x005942C0`, `0x00583E70`, `0x0059DA50`; chain-id
  arguments of every `0x00543640` call site in `all.asm` (script outside
  the repo); live `objects.txt` rows 26, 30, 189, 268, 385.

## Open questions

1. ~~`0x005944F0`~~: answered in §3 (caller: the `cain1`
   NpcOutOfTown AI, `monsters/ai-bodies.md` §9.32).
2. Object modes set here (gibbet 1 / 3, stones 0 / 2, `quests-act1.md` §10.6)
   and object events 1 / 7 belong to the objects spec (not written).
3. A recording of a Cain rescue (gibbet operate → event 7 17 frames
   later: 0x28, 0x5D, Cain spawn) and of a Countess kill (trap monster,
   chest missiles) would confirm §1 and §4.
4. The progression's readers (difficulty unlock, character title) belong
   to the save spec; confirm in 1.14d when it is written.
5. A recording that enters Catacombs 1 after Andariel's kill (state 4)
   would confirm §8 item 6 (no 0x5D, state kept), and one Kashya reward
   with a free hireling slot the §8 item 8 message order.
6. QA-1 (null room in the trap retry): Answered (§9 item 1).
7. QA-2 (object or marker without a room): Answered (§9 item 2).
8. QA-3 (L4 party step placement): Answered (§9 item 3).
9. QA-4 (player without a client): Answered (§9 item 4).
10. QA-5 (+0x38 reader): Answered (§9 item 5).
11. QA-6 (chain 37's `0x0058F870`): Answered (§9 item 6).
12. Wiring §3 item 1 (quest init order and the town Cain draws): Answered
    (§9 item 7). A recording of the marker's creation after Cain left
    Tristram (HANDOFF §5 C79) would confirm it.
13. WW-6 Act I bodies (gibbet init 7, tree init 9, cain portal init 61,
    Wirt's body operate 33; init 37 is not Wirt's): Answered (§9 items
    8–11).
14. WW-10 (`0x0061AED0`): Answered (§9 item 12).
