# Spec: World — Vendors part 2 (the item copy `0x0055A2A0`; C→S 0x4F buttons)

- **Status:** draft: read from the 1.14d `Game.exe` (addresses inline);
  the two recorded copies (buy GUID 0x36, sell GUID 0x35) fit rule 3's
  GUID numbering. Moved out of `world/vendors.md` (size), text and
  numbers unchanged. §10 (C→S 0x4F buttons) read from `0x00568060`,
  `0x00564D50`, `0x0053FF00` (disassembly); not recorded.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::vendors` (copy provider; §10 the 0x4F button handler, stash gold)
- **Related specs:** `world/vendors.md` (part 1; its Constants,
  Randomness, Edge cases, Provenance and Open questions (8, 9) also cover
  this part); `items/bitstream.md`, `items/generation.md` §1.4, §9,
  `items/inventory-moves.md` §6.1, §7.19, `sim/units.md` §2, §6.5,
  `sim/rng.md` §5.3; §10: `ui/panels-2.md` §20, §21,
  `world/objects-2.md` §16.10, `world/cube.md` §1, `client/msg-ui.md` §3,
  `audio/triggers-2.md` §14, `sim/intents-events.md` §9 rule 15. A § number without a file name (§7.1, §7.2, §8.2)
  is `world/vendors.md`'s, except §7.3, §7.3.1 and §10.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 29–38 |
| Rules | 39–40 |
|   7.3 Item copy (`0x0055A2A0`, ECX game, EDX source S, owner, fillers) | 41–116 |
|   7.3.1 Fields the decoder rebuilds (Open question 8) | 117–170 |
|   10. C→S 0x4F buttons (`0x0054C7C0` → `0x00568060`; answers `ui/panels.md` OQ 6) | 171–295 |
<!-- /index -->

## Summary

`0x0055A2A0` makes a second item equal to an existing one by writing
the source as a save-format stream and decoding it into a new item unit
(§7.3); §7.3.1 lists the fields the decoder rebuilds instead of reading,
so the copy can differ from its source there. §10 is the server side of
C→S 0x4F ClickButton: the stash gold buttons (withdraw 0x13, deposit
0x14), stash close (0x12), and the routing of the cube and player-trade
buttons.

## Rules

### 7.3 Item copy (`0x0055A2A0`, ECX game, EDX source S, owner, fillers)

The copy routine of every caller that needs a second item equal to an
existing one: buy (§7.1 rule 9.2), sell (§7.2 rule 8), cube outputs
(`world/cube.md` §7.3), hireling take (`items/inventory-moves.md` §7.23), NPC
socketing (`world/npc.md` §8.1); 17 call sites. The owner argument (stack
1) is not read in 1.14d. Result: the copy, or none.

1. R := S's room (`0x00620BB0`; none when S is not on the ground).
   1.1. **A ground source** (2026-10-08). Every 1.14d caller passes a
   held S (the sell item is the player's cursor or inventory item,
   `world/vendors.md` §7.2 rule 3; the buy source is a store item; cube,
   hireling and socketing sources are inventory items), so R is none and
   the copy is created with no room. With a ground S the copy would be
   allocated in R at S's position (step 3), i.e. on the ground, and no
   later step unlinks it: the callers' mode set `0x00624690(copy, 4)`
   only queues an update (`0x0064C040`), sets unit +0xC4 bit 0x1 and the
   mode field, and refreshes; it does not take the copy out of R. d2rs:
   a ground S is a caller error (assert); never place a copy that has a
   room.
2. Write S as a **save-format** stream with children
   (`items/bitstream.md`, `0x006313E0(S, buffer, 0x400, save 1,
   children 1, alt 0)`) into a 1,024-byte buffer. A stream that would
   not fit gives length 0 and the read in step 3 fails.
3. Read the first record (`0x00558CB0`): peek its header (`0x0062E410`:
   flags, version, mode, location, item code → class; filled-socket
   count N; the `ear` flag 0x10000 maps to code `ear `); class outside
   the items table → none. Allocate a new item unit of that class in R
   at the stream's position and mode (`0x00555230`, `sim/units.md`; a
   new GUID). Decode the record into it (`0x0062E430`); a decode error
   or a missing record frees the unit (`0x00555600`) and the result is
   none. Then item flag 0x80000 (init) set, 0x2000 (in store) cleared
   (`items/generation.md` §1.4), replenish timers
   (`items/generation.md` §9 step 6: `0x00558530`, `0x00558580`).
4. Item flag 0x80000 set, 0x2000 cleared on the copy again.
5. fillers ≠ 0 and N ≠ 0: for each of the N child records in stream
   order: read it as in step 3 with no room (failure → result none;
   the copy and the children read so far are not freed); child mode
   := 4; socket it into the copy through `0x00562660(child GUID, copy
   GUID, &out, 0, 1, 0, 0)` with EDX = the copy (`items/inventory-moves.md`
   §7.19 rule 4: no target mode test; the copy's inventory cursor
   cleared; a runeword match runs the runeword stats without
   `0x0055FE80` and then the tail; no match returns before the tail;
   result 0 → fatal assert, line 0xDD4); child item flags 0x80000 set, 0x2000
   cleared; child command flag 0x1 cleared (`0x00628170`).
   fillers = 0: the children are not read; the copy keeps the stream's
   socket flags and its stat lists but has no fillers.
6. S item flag 0x8000000 set ("copy source", `items/generation.md`
   §1.4: no reader; it rides in S's later streams, so a second copy
   of the same S, e.g. a permanent store item bought twice, carries
   it too).
7. Replenish: for stat 252 (`item_replenish_durability`) and then 253
   (`item_replenish_quantity`), total r ≠ 0 and no type-3 timer on the
   copy (`0x005415A0`) → a type-3 timer at game frame (+0xA8) + 2500 / r
   + 1 (`0x005417D0`; `sim/unit-events.tsv` rows `0x0055a4be`,
   `0x0055a500`; the handler is `sim/units.md` §6.5).
8. Per-item reset of the deferred-message bits (`0x005979B0`,
   `items/inventory-moves.md` §6.1 rule 4); command flag 0x1 cleared. Result:
   the copy.

What carries over is exactly what the save stream carries
(`items/bitstream.md` §2–§5): stats with `Save Bits` 0, values the clamp
changes (§1 rule 3) and unit state outside the item record (timers
other than step 7, owner links, unit flags) are not copied. No draw
decides any property of the copy (the stream holds the seeds, §4.1 rule
7), but the **game seed does advance**: each unit allocation of steps 3
and 5 (`0x00555230`, type 4) makes one game-seed step for the unit seed
(`0x00552DF0` at `0x0055530E`) and one for the item seed (`0x00552E90`
at `0x0055532A`), both on game +0xD0 (`sim/units.md` §2 rule 4,
`sim/rng.md` §5.3); the decode then overwrites the item's seeds from the
stream. So a copy with k fillers read costs 2 · (1 + k) game-seed steps,
copy first, children in stream order; the GUID (`0x00552EE0`) and the
rest of the routine draw nothing. The decode rules of
`0x0062E430` (`0x0062CBE0` full record, `0x0062A970` compact) are the inverse
of `items/bitstream.md` except for the fields of §7.3.1.

### 7.3.1 Fields the decoder rebuilds (Open question 8)

The copy passes save version 0x60 (`0x0055A2FE`), so none of the
decoder's old-version conversions apply. The decoder entry (`0x0062E430`)
stores the stream's flags without 0x2000000 and 0x80000 and zeroes the
affix and rare-name slots before the record is read. Not read from the
stream but rebuilt:

1. Full record (`0x0062CBE0`), weapon (type `weap`): stat 68 := −items
   `speed`; stats 22, 21, 24, 23 :=
   `maxdam`, `mindam`, `2handmaxdam`, `2handmindam`, and, when
   `maxmisdam` ≠ 0, 159 := `minmisdam`, 160 := `maxmisdam` (unit base
   set `0x00627260`). Quality 1: each is ⌊3v / 4⌋ instead, then 22, 24,
   160 at least 2 and 21, 23, 159 raised from 0 to 1. Item flag
   0x400000 (`0x0062A8D0`): each of the six := base × 3 / 2 (signed).
   Durability (73, then 72 when 73 ≠ 0) is read.
2. Full record, armor (type `armo`): stat 20 := items `block`, 67 :=
   −items `speed`; defense (31) and durability are read.
3. While the stat lists are read: an entry for stat 17 (read with its
   pair 18) first raises base 22, 24 and, for a throwable item
   (`0x0062BA80`), 160 to the items column when the base is below it,
   and before 18 raises 21, 23, 159 the same way (`0x0062C9F0`, a
   "raise when below" form of `items/properties.md` §4.3); an entry for
   stat 57 (read with 58 and 59) sets stat 326 (`poison_count`) := 1 in
   that list.
4. Item level: a read value < 1 → 1. Unique (quality 7): a file index ≥
   the uniqueitems count → −1.
5. Compact record (`0x0062A970`): item level := 1, quality := 2, unit
   seed field +0x28 := 0 and the item seed initialised from it
   (`0x00650E40`), suffix slot 0 := 0 for `tsc ` and 1 for `isc `.
6. Stat lists (`0x0062CBE0`, list loop end): for each set bit i of the
   5-bit mask the list of state S_i (table `0x006E90B8`) is looked up and,
   when missing, created with flags **0x2040** (`0x00625790(item, S_i,
   0x2040)`); a stored list of S with flags 0x40 only therefore comes
   back as 0x2040. The runeword list is state 171, flags 0x40; the main
   list state 0, flags 0x40 (`items/bitstream.md` §4.6).

So a copy differs from S (whose base values come from
`items/generation.md` §6 and `items/quality.md` §6) in: the item level
and item seed of a compact item; stat 326 when S's list holds a value ≥
2 (two poison properties added to one list, `items/properties.md` §4.2);
the base damage of a low-quality weapon whose runeword list holds stat
17 or 18 (the runeword's own reset acts on the filler, §10.2 there, so S
keeps ⌊3v / 4⌋ while the copy gets the full column); any weapon base
damage S holds other than the values of rule 1 (e.g. the craft list
re-applying × 3 / 2 to an ethereal weapon, `items/properties.md` §12).
The low-quality missile floors differ from creation (creation 159 ≥ 2,
160 ≥ 1; decode 159 ≥ 1, 160 ≥ 2), but no live `weapons.txt` row with
`maxmisdam` ≠ 0 reaches a floor.

Recorded: the buy of rule 10 creates the copy GUID 0x36 from store item
0x12; the sell of §7.2 creates GUID 0x35 from GUID 7 (each the next
item GUID; `sim/units.md` numbering).

### 10. C→S 0x4F buttons (`0x0054C7C0` → `0x00568060`; answers `ui/panels.md` OQ 6)

The server side of C→S 0x4F ClickButton [button u16@1][p1 u16@3][p2
u16@5] (`sim/client-messages.tsv`; size ≠ 7 → result 3,
`sim/intents-events.md` §9 rule 15). The entry passes `v` = (p1 << 16)
| p2 as one u32 to `0x00568060(game, player P, button, v)`; the client
senders build p1 = `v >> 16`, p2 = `v & 0xFFFF` (`ui/panels-2.md` §21
rule 8). "Interaction" is P's interact record (`0x00554100`: GUID +0x64,
type +0x68, active byte +0x6C). Results: `sim/intents-events.md`
(1 and 2 enqueue, 3 drops).

#### 10.1 Dispatch (`0x00568060`)

1. No P → result 1, nothing else.
2. Interaction not active → S→C 0x77 code 0x0C to P's client
   (`0x005531C0`, `0x0053CAB0`; `client/msg-ui.md` §3: close trade);
   result 0. This comes before the button test: a stash, cube or trade
   button with no interaction gets 0x77 0x0C.
3. Button 0x12, 0x13 or 0x14 (u16 compare `button − 0x12 ≤ 2`):
   interaction type ≠ 2 → result 1, nothing sent. Type 2 → §10.2
   (`0x00564D50`, ECX = v, EAX = the interaction GUID, ESI = P, stack
   game, button); result 0 whatever §10.2 does.
4. Button 0x17 or 0x18: the cube (`world/cube.md` §1: type ≠ 4 →
   result 3; else `0x00566AE0`, result 0).
5. Any other button: interaction type ≠ 0 → S→C 0x77 code 0x0D to P
   (`0x005531C0`; close trade(1)), result 3. Type 0
   (player trade): partner Q := the player unit (type 0) with the
   interaction GUID (`0x00552F60`). No Q → `0x00597A20(game, P)` ≠ 0 →
   result 0; else 0x77 0x0C to P, result 0. Else the trade switch
   (§10.3) on button − 2 (table `0x00568620`, 7 entries for buttons
   2–8); result 0. Buttons 0, 1, 9–0x11, 0x15, 0x16 and ≥ 0x19 reach
   this rule and, after its checks, do nothing.
6. `0x00597A20(game, U)` (also used by §10.3 rows 2, 3, 4, 8; read
   2026-10-09): 1 when U's flags 2 (+0xC8) has bit 0x400000 or
   0x800000; else 1 when any GUID of U's inventory update list
   (inventory +0x2C, `0x0063CBB0`, `items/inventory.md` §1 rule 2;
   node GUID +0, next +4) names an
   existing item unit (`0x00552F60(game, 4, GUID)`); else 0. A null U
   skips the flag test and then reads U +0x60 (crash; every caller
   passes a live player). A player with neither (no flag, empty list)
   gets 0, so rule 5 sends 0x77 0x0C.

#### 10.2 Stash buttons (`0x00564D50`)

Common checks (any failure → nothing, no message): the stash object O
:= the object unit (type 2) with the interaction GUID (`0x00552F60`)
exists and has class 0x10B (267, `world/objects-2.md` §16.10); P's room
(`0x00620BB0`) exists and is in a town level (`0x0061AB00`); O's room
the same. Then by button:

1. **0x12 stash close.** If the interaction is active and of type 2
   (always true here): reset it (GUID −1, type 6, active 0;
   `0x00554190`). Then the scroll / tome recount (`0x0055FA40`,
   `items/inventory.md` §5.5). Nothing is sent: the client has already
   closed its panel (`ui/panels-2.md` §20), and no 0x77 0x11 follows.
   v is not read. A second 0x12 (the client sends two, `ui/panels-2.md`
   §20 rule 7) finds the interaction inactive and gets 0x77 0x0C
   (§10.1 rule 2).
2. **0x13 withdraw v.** All compares signed. v ≤ 0 → nothing. v >
   stat 15 `goldbank` (full value, `0x00625480(P, 15, 0)`) → nothing.
   stat 14 `gold` + v > the carried cap (`0x00622E70`: stat 12 `level`
   × 10000) → S→C 0x2C event 19 (`impossible`) on P with target P
   (`0x00553380`, `audio/triggers-2.md` §14), nothing else. Else: gold
   += v through Receive (`0x0055B060`, `world/vendors.md` §9.1; with
   the cap checked it never drops a pile here), then goldbank −= v
   through the clamped add (rule 4).
3. **0x14 deposit v.** v ≤ 0 → nothing (signed). v > gold (signed) →
   nothing. cap := the stash cap (`0x00623460`): the constant
   **2,500,000** in 1.14d, not level-dependent. s := goldbank. Unsigned
   s + v ≤ cap → gold −= v, then goldbank += v. Else, when s < cap
   (unsigned): goldbank += cap − s, then gold −= cap − s (a partial
   deposit filling the stash). Else (stash full) nothing. No sound or
   message in any case.
4. **Clamped add** (`0x0053FF00(unit, stat, d)`): n := current full
   value + d (signed). n < 0 → stat := 0 (base set `0x00627260(unit,
   stat, 0, 0)`). Else, unit a player: stat 14 with n > level × 10000,
   or stat 15 with n > 2,500,000 → stat := **0** (not the cap). Else
   base add d (`0x006272B0(unit, stat, d, 0)`). The rule-2 and rule-3
   checks keep both writes in range, so the zeroing never fires from a
   stash button.

The stat changes reach P's client through the ordinary player stat
updates (`sim/intents-events.md` §7); the stash buttons send no
dedicated message.

#### 10.3 Player-trade buttons (no owner spec; one line each)

Player data D_P, D_Q (`0x006221A0`): trade state +0x50, tick +0x58,
gold record +0x5C. `0x005679E0(Q, D_P, D_Q, code)` ends or answers the
trade with a 0x77 code (its unit events are `sim/unit-events.tsv` rows
`0x00567ad1`, `0x00567aea`); `0x00597A20(game, unit)` ≠ 0 skips the
answer. Out of scope (Phases 0–6): the player-trade flow (multiplayer
player trade; `sim/unit-events.tsv` rows 55–63, event 13); what is below
is the switch as read, not a full rule set.

| Button | Behaviour (1.14d) |
|---|---|
| 2 decline | both states in {1, 2}, or `0x00597A20` = 0 for P and Q → `0x005679E0(…, 9)`, both ticks := `GetTickCount` |
| 3 accept request | `0x00597A20` ≠ 0 for P or Q: P state 7 → 0xE, 8 → 0xF, else `0x005679E0(…, 0xC)`. Else classic gate `0x005B5810` ≠ 0 → code 9; states ≠ (P 2, Q 1) → code 0xC; else start (`0x00567020`), then `0x00566B30` for P, then for Q (Q failing first undoes P, `0x00566BF0(game, P)`): a failure → 0x77 0x0C to P and to Q; both pass → S→C 0x78 (`0x0053CAD0`) to P with Q's client name and GUID, then to Q with P's |
| 4 commit | `0x00597A20` ≠ 0 → 0x77 0x06 (assert 0xBEE); `0x005B5810` ≠ 0 → code 9; P state ≠ 3 → 0x77 0x06 (0xBD2); P has a cursor item (`0x0063C1E0`) → 0x77 0x06 (0xBD7); else 0x77 0x05 to Q, P state := 4, and when Q's state is 4 too → exchange (`0x00567C70`), both ticks := `GetTickCount` |
| 5, 6 | nothing |
| 7 uncommit | each state 4 → 3; 0x77 0x06 to P and to Q |
| 8 gold offer v | v > P's gold (unsigned) → code 9 unless `0x00597A20` ≠ 0; else v := min(v, Q's cap − Q's gold) (unsigned), and when D_P +0x5C exists: its +0xC := v, S→C 0x79 (`0x0053CBE0`) (0, v) to Q and (1, v) to P |

#### 10.4 Edge cases and test vectors

1. A 0x12 while P is out of town or the stash object is gone leaves the
   interaction set (the common checks fail before the reset); a later
   0x4F of any button other than 0x12–0x14 then gets 0x77 0x0D, result 3.
2. p1 ≥ 0x8000 makes v negative: 0x13 and 0x14 do nothing.
3. Opening the cube while at the stash clears the type-2 interaction
   itself (`world/cube.md` §1); a later 0x12 gets result 1.

| Input (P level 10, gold g, goldbank s; interaction (2, stash)) | Result |
|---|---|
| 0x13 v = 50,000, g = 60,000, s = 100,000 | 0x2C event 19; g, s unchanged (60,000 + 50,000 > 100,000) |
| 0x13 v = 40,000, g = 60,000, s = 100,000 | g = 100,000, s = 60,000 |
| 0x13 v = 100,001, s = 100,000 | nothing |
| 0x14 v = 30,000, g = 30,000, s = 2,490,000 | g = 20,000, s = 2,500,000 (partial 10,000) |
| 0x14 v = 1, s = 2,500,000 | nothing |
| 0x14 v = 0x00010000 (p1 1, p2 0), g = 70,000, s = 0 | g = 4,464, s = 65,536 |
| 0x12, interaction (2, stash) in town | interaction (−1, 6, inactive); recount; no message; result 0 |
| 0x12 sent twice | second: 0x77 0x0C, result 0 |
| 0x13 with interaction type 4 (cube) | result 1, nothing |
