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
| Summary | 42–52 |
| Inputs | 53–63 |
| Outputs / state changes | 64–71 |
| Rules | 72–73 |
|   1. A1Q4 gibbet (Cain's cage, object class 26) | 74–144 |
|   2. Cairn stones (object classes 17–21) | 145–192 |
|   3. Town-Cain marker (object class 385, `InitFn` 54) | 193–219 |
|   4. A1Q5 Countess chest trap (`0x005954F0(record, extra)`) | 220–258 |
|   5. Character progression (`0x00538680(client, step, difficulty)`) | 259–282 |
|   6. Party list as read by the quest code | 283–308 |
|   7. Cairn stone-order 0x50: bytes 13–14 | 309–318 |
|   8. Act I clarifications (implementation questions, 2026-10-06) | 319–389 |
| Constants & data dependencies | 390–403 |
| Randomness | 404–410 |
| Edge cases & original bugs | 411–427 |
| Test vectors | 428–450 |
| Provenance | 451–475 |
| Open questions | 476–490 |
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
| chain 4 record and extra | quest record, 0x1BC bytes | `quests.md` §10.6 |
| chain 5 record and extra | quest record, 0x120 bytes | `quests.md` §10.7 |
| object init args | dwords: game, object, room, ?, `objects.txt` record, x, y | init dispatcher `0x0054F5D0` (table `0x00731BC0`, index `objects.txt` `InitFn`, record +0x1B1) |
| object operate args | dwords: game, object, player, ?, object class | operate dispatcher `0x00584420` (table `0x00732D18`, index `OperateFn`, record +0x1B3) |
| client save flags | u16 at client +0x0A | client of a player (`0x005531C0`: player data +0x9C) |
| party list | game +0x1D2C | §6 |

## Outputs / state changes

Player quest bits of slot 4 and 6 (via the iterate functions of
`quests.md` §10.6 / §10.8), chain 4 / 5 extra bytes, object modes,
spawned monsters (cain1 146, trap-firebolt 326), portal objects (59, 60),
missiles (332), timers, S→C 0x28 / 0x5D, sound events, the client's
progression field.

## Rules

### 1. A1Q4 gibbet (Cain's cage, object class 26)

`objects.txt` row 26 `Gibbet` has `OperateFn` 10 (pointer `0x00732D40` →
`0x00593480`) and `InitFn` 7 (not quest code).

New chain 4 extra fields (the rest are in `quests.md` §10.6):

| Extra | Type | Field |
|---|---|---|
| +0x3C | u32 | GUID of the player who opened the gibbet (−1 when none) |
| +0x40 | u32 | GUID of the class-17 stone that carries the Tristram portal (§2) |
| +0x44 | u8 | Tristram-portal timer pending |
| +0x45 | u8 | Tristram portal created |
| +0x4C, +0x4D | u8 | quest already done for the game (event 13, `quests.md` §10.6 step 10) |
| +0x5C–+0x60 | u8 × 5 | per-stone reset bytes, index = stone class − 17 (§2; zeroed by the init, never set) |
| +0x62 | u8 | Cain could not be spawned in Tristram |
| +0x66 | u8 | town portal out of Tristram created by §1.2 |
| +0x74 | u32 | scratch: player unit found in Tristram (§1.2 step 4) |
| +0x84, +0x88 | i32 × 2 | position of the town-Cain marker object (§3) |
| +0x91 | u8 | set to 1 by `0x005944F0` (§3); no reader in this chain's code |
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
   run `0x005930B0` (`quests.md` §10.6 L4 member step: member lacking
   4.0 and 4.1 whose room's level is ≠ 0 and in Act I gets 4.13, 4.1 and
   0x28).

#### 1.2 Gibbet quest function `0x00593290(game, object)` (object event 7)

1. Q = chain 4's record; absent → fatal assert (line 0x68E). X = extra.
2. End unless not-intro ≠ 0 and X +0x50 ≠ 1.
3. X +0x54 := 3; object mode := 3. (x, y) = the object's position + (3,
   3) (`0x0045ADF0`, `0x0045AE20`); room = the object's room.
   C = spawn cain1 (146) at (room, x, y), mode 1, spread −1, flags 0
   (`0x005B2F20`). C none: free spot from (x, y) in room
   (`0x00545340`: size 2, mask 0x100, radius 3, limit 100); found →
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
6. Both cases: every player L4, then every player L5 (`quests.md`
   §10.6); flags (+0x14) := 0; broadcast(6, 0) with iterate L1.

State (+0x0C) is not changed here; the gibbet's quest state was set by
the stones (`quests.md` §10.6 stone operate, state 5).

### 2. Cairn stones (object classes 17–21)

`objects.txt` rows 17–21 (`StoneAlpha`, `StoneBeta`, `StoneGamma`,
`StoneDelta`, `StoneLambda`) have `InitFn` 6 (pointer `0x00731BD8` →
`0x005935E0`) and `OperateFn` 9 (`0x00593710`, `quests.md` §10.6).

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
(`quests.md` §10.6 step 15) at (x, y) in the init args' room. The
event-3 spawn of `quests.md` §10.6 step 3.3 looks X +0x6C up as an
object (type 2).

**Cain leaves Tristram** `0x005944F0(game, unit)` is the "spawn the town
portal" call of the NpcOutOfTown AI for class 146 `cain1`
(`monsters/ai.md` §9.32, `0x005E7880` / portal set-up `0x005E77A0`;
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
the chest's event 7 `0x005956C0` (`quests.md` §10.7). E = extra; list =
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

Called by the A1Q6 credit (`quests.md` §10.8, `0x00596210`: client of P
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

Each item settles a reading of `quests.md` §10 that the Act I
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
   (`quests.md` §10.6 step 3.3).
6. 0x50 bytes 13–14 are stack leftovers (§7).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| progression: client +0x0A = 0x0520 (exp, p 5), step 1, difficulty 2 | 0x0B20 | §5 |
| progression: 0x0000 (classic), step 1, difficulty 1 | 0x0500 | §5 |
| progression: 0xE720 (exp, p 7), step 1, difficulty 0 | unchanged (7 > 1) | §5 |
| progression: 0x0120 (exp, p 1), step 1, difficulty 0 | 0x0120 rewritten (p = n) | §5 |
| stone 0x50 with order [18, 20, 17, 21, 19] | `50 0400 0100 0300 0000 0400 0200 ?? ??` (bytes 13–14 masked) | §7, `quests.md` §10.6 |
| gibbet operate, R slot 4 = 0x0002 (4.1) | sound 19; nothing else | §1.1 step 2 |
| gibbet operate, state 6 | nothing | §1.1 step 1 |
| gibbet operate, R slot 4 = 0, object mode 0, single player | object mode 1; events 1 and 7 scheduled; X +0x54 = 3; R slot 4 = 0x2002; 0x28 | §1.1 |
| gibbet event 7, Cain spawned, player P in Tristram lacking 4.0/4.1, status default rule (state 5, init_no 6) | P: 4.13, 4.1, 0x28; then `5d 04 00 06 0000` | §1.2, `quests.md` §6.1, §10.6 L1, L4 |
| same, a second player Q in Act I outside Tristram | Q: 4.14, `5d 04 00 0c 0000` (L5), then `5d 04 00 0c 0000` (L1, now → 12) | §1.2, `quests.md` §6.1 |
| trap step, 2 chests, killed 1, trapped 0, first spawn at the death position succeeds | 1 monster 326; 2 missiles 332 (one per chest, data +0x28 = chest GUID); E +0x119 = 1 | §4 |
| trap step again | nothing | §4 step 1 |
| A1Q6 event 3, b = 34, not-intro 1, state 4, status 2 | state stays 4; no O2 walk, nothing sent | §8 item 6 |
| A1Q6 event 3, b = 37, state 1, status 2 | state 3; every player O2; nothing broadcast | §8 item 6, `quests.md` §10.8 |
| tree operate, drop fails, state 3 | state 4; no 0x5D; object mode 0; +0x47 1; +0x30 = object GUID | §8 item 2 |
| A1Q2 message 92 with 2.1, no hireling | 0x28, then 0x50 (u16 2) and the hireling's messages, then 0x27, 0x29 | §8 item 8 |
| stone init, not-intro 1, X +0x4C 0, +0x4D 0, +0x50 0 | mode unchanged | §2.2 |
| stone 17 init, X +0x4C 1, +0x45 0, +0x44 0 | +0x4C 0; +0x40 = GUID; +0x44 1; timer period 1; mode 2 | §2.2 |

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

## Open questions

1. ~~`0x005944F0`~~: answered in §3 (caller: the `cain1`
   NpcOutOfTown AI, `monsters/ai.md` §9.32).
2. Object modes set here (gibbet 1 / 3, stones 0 / 2, `quests.md` §10.6)
   and object events 1 / 7 belong to the objects spec (not written).
3. A recording of a Cain rescue (gibbet operate → event 7 17 frames
   later: 0x28, 0x5D, Cain spawn) and of a Countess kill (trap monster,
   chest missiles) would confirm §1 and §4.
4. The progression's readers (difficulty unlock, character title) belong
   to the save spec; confirm in 1.14d when it is written.
5. A recording that enters Catacombs 1 after Andariel's kill (state 4)
   would confirm §8 item 6 (no 0x5D, state kept), and one Kashya reward
   with a free hireling slot the §8 item 8 message order.
