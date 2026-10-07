# Spec: World — Quests, shared helper functions (free spot, quest spawns, quest missiles, interaction end, game end)

- **Status:** draft: every rule read from the 1.14d `Game.exe`
  disassembly (`tools/ghidra/disasm.py`, addresses inline) on
  2026-10-07; no recording.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests` (the `QuestWorld` seams
  `free_spot`, `free_spot_at`, `critical_spawn`, `spawn_superunique`,
  `spawn_superunique_at_unit`, `quest_missile`, `create_missile_at`,
  `end_interaction`, `end_game`, `kill_monster`, `close_town_portal`)
  and the host (`d2-server`) for §6.
- **Related specs:** `world/quests.md` (record layout, §4 events);
  the per-act files that call these helpers (`quests-act1.md` …
  `quests-act5-2.md`); `drlg/rooms.md` §1 (room sub-tile box `0x00619730`),
  §6 (room lookup `0x00463740`); `world/object-population.md` §6 (box
  query `0x0064D800`); `monsters/init.md` (spawn wrapper `0x005B2F20`);
  `monsters/population.md` §11.2–§11.4 (`0x0054E600`); `missiles/missiles.md`
  §R2 (creation record, `0x0059FA30`); `world/npc.md` §3 (chat nodes);
  `world/cube.md` (cube close); `sim/server-messages.tsv` (0x62).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 44–53 |
| Inputs | 54–61 |
| Outputs / state changes | 62–66 |
| Rules | 67–68 |
|   1. Free-spot search (`0x00545340(room R0, &point, size s, mask m, &out room, unused, limit L)`) | 69–101 |
|   2. Critical monster spawn (`0x005459A0(game, x, y, room R, flag, class)`) | 102–127 |
|   3. Superunique spawn at a point (`0x00545C30(game, unit U, &point, kind, id)`) | 128–145 |
|   4. Quest missiles | 146–198 |
|   5. End a player's interaction (`0x005351C0(game, player)`) | 199–231 |
|   6. End the game (`0x00530590(game, client c)`) — host | 232–254 |
|   7. Close a player's town portal (`0x00535430(game, player)`) | 255–278 |
|   8. Find a player's item by code (`0x00558110(game, player, code)`) | 279–301 |
| Constants & data dependencies | 302–315 |
| Randomness | 316–320 |
| Edge cases & original bugs | 321–329 |
| Test vectors | 330–343 |
| Provenance | 344–356 |
| Open questions | 357–362 |
<!-- /index -->

## Summary

Small functions in the quest code (0x0054xxxx quest core and the
0x0058xxxx act files) that several quests call and no other spec
states: the quest free-spot search, the "critical" monster spawn with
its retry ladder, the superunique spawn at a point, the Ancients'
missile and the generic missile-at-a-point wrapper, ending a player's
interaction, and the game-end host call. Each section is one function;
the callers' rules stay in their act files.

## Inputs

| Name | Type | Source |
|---|---|---|
| room, point (x, y) | active room, sub-tile coordinates | the calling quest rule |
| size, mask, limit | u32 | the calling quest rule (literal arguments) |
| monster / superunique class | i32 | the calling quest rule |

## Outputs / state changes

A spot (point + room), a monster or missile unit, an ended interaction
(+ S→C 0x62), or a host request (§6). No quest-record writes.

## Rules

### 1. Free-spot search (`0x00545340(room R0, &point, size s, mask m, &out room, unused, limit L)`)

ECX = R0, EDX = &point (in: (x, y); out: the spot), stack: s, m,
&out, a sixth argument that is never read (the "radius" of the quest
specs), L. Returns nothing; *out := the spot's room or 0.

1. h := s >> 1; corner origin (bx, by) := (x − h, y − h). B := the
   sub-tile box (X, Y, W, H) of R0 (`0x00619730`). "Lookup(r, px, py)"
   = `0x00463740(r, px, py)` (the room holding the point among r and
   its adjacent rooms; none → 0). xl := bx (the x last tested).
2. For k = 1, 2, …, L − 1 (nothing when L ≤ 1); lo := −k:
   for j = k, k − 2, … while j > lo (rows, top first):
   1. yy := by + j. Row room RR := R0 when Y ≤ yy < Y + H for the box B
      **as last read** (R0's box before the first column test, then the
      box of the previous column's row room); else RR := Lookup(R0, xl,
      yy). RR = 0 → next row.
   2. For i = lo, lo + 2, … while i < k (columns, left first): xx := bx
      + i; xl := xx. B := the box of RR. Column room CR := RR, except
      when xx < X or **yy** ≥ X + W (the second test compares the row
      coordinate with the box's x end; 1.14d bug, reproduced) → CR :=
      Lookup(RR, xx, yy).
      Accept when CR ≠ 0, the box query `0x0064D800(CR, xx, yy, 2s + 1,
      2s + 1, m)` returns 0 (free), and X ≤ xx < X + W and Y ≤ yy < Y +
      H (unsigned compares, B = RR's box). Accept → point := (xx + h, yy
      + h); *out := Lookup(R0, xx + h, yy + h); return.
3. Nothing accepted → *out := 0; the point is left as passed.

So ring k tests k × k corners on a step-2 lattice, the parity of the
offsets alternating with k: k = 1 tests offset (−1, +1) only; k = 2
tests (−2, 2), (0, 2), (−2, 0), (0, 0); the result is the spot centre
(x + i, y + j). A candidate outside its row room's box is never
accepted. No RNG draw.

### 2. Critical monster spawn (`0x005459A0(game, x, y, room R, flag, class)`)

ECX = game, EDX = x; stack: y, R, flag, class (`ret 0x10`). Callers:
`0x005891D0`, `0x0058AAB0`, `0x0058EAC0` (Act V, flag 1). Returns the
monster (or 0).

1. Inside point: B := R's box. (px, py) := (x, y); up to 21 steps
   (d = 0…20): (px, py) inside the box shrunk by one (X ≤ px, Y ≤ py,
   px < X + W − 1, py < Y + H − 1, signed) → stop; else px += 1, py +=
   1. No step inside → (px, py) := (y, y + 21) (the x slot is
   overwritten with y; 1.14d bug, reproduced).
2. Free spot from (px, py) in R (§1, s 2, m 0x100, limit 100). Found →
   (room, x', y') := (found room, spot); none → (R, x, y).
3. M := `0x005B2F20(game, room, x', y', class, mode 1, spread −1, flags
   0)` (`monsters/init.md`); none → again with spread 5.
4. Still none: 20 tries n = 0…19, each from the previous (room, x', y'):
   (x', y') += (1, 1); room' := Lookup(room, x', y'); room' = 0 →
   (room', x', y') := (R, x, y). Free spot from (x', y') in room' (§1,
   s 2, m 0x100, limit 100): found → (room, x', y') := (found room,
   spot); none → (R, x, y). M := spawn with spread 10; made → step 6.
5. After 20 failures: M := spawn at (R, x, y) with spread 15; none →
   return 0.
6. flag ≠ 0 → M unit flags (+0xC4) |= 0x3000000. Return M.

No draw of its own (the spawn wrapper's draws are `monsters/init.md`'s).

### 3. Superunique spawn at a point (`0x00545C30(game, unit U, &point, kind, id)`)

ECX = game, EDX = U; stack: &point (x, y), kind, id (`ret 0xC`).

1. class := `0x00659B80(kind, id)`: kind 2 → id + the `monstats.txt`
   row count (data tables +0xA80), so `0x0054E600` reads it as
   superunique id (`monsters/population.md` §11.2 rule 2); kind 0 → id
   + that count + `[0x0096C70C]`; any other kind → id.
2. R := Lookup(U's room, x, y) (`0x00620BB0`, `0x00463740`). R = 0 →
   return 0.
3. Return `0x0054E600(game, R, class, x, y, mode 1)` (preset spawn,
   `monsters/population.md` §11.2–§11.4: difficulty, `Stacks` and
   `AutoPos` rules apply).

Callers: Act IV seal bosses (`0x005B5750`, `quests-act4.md` §5.4, kind
2) and the Ancients (`0x0058C0E0`, `quests-act5-2.md` §7.6, kind 2,
point = the statue's position).

### 4. Quest missiles

#### 4.1 Ancient to statue (`0x0058C8D0(args)`)

Called by chain 35's kill (`0x0058C9A0`, `quests-act5-2.md` §7.6) with
EDI = the quest record, EBX = the statue GUID (extra +0x1C for victim
540, +0x14 for 541, +0x18 for 542), stack = the event arguments (+0x08
victim).

1. S := the object (type 2) with that GUID (`0x00552F60(game, 2,
   GUID)`); none → nothing.
2. A zeroed creation record (`missiles/missiles.md` §R2.1): flags 0x420
   (0x20 target absolute, 0x400 frames from distance), owner (+0x04) =
   origin (+0x08) = the victim, class (+0x10) 541 (`ancient death
   center`), target (+0x1C, +0x20) = S's position (static path +0x0C /
   +0x10 for unit types 2, 4, 5; the path x / y `0x006488C0` /
   `0x00648900` otherwise, 0 without a path), level (+0x30) 1.
3. Create (`0x0059FA30`); made → missile data +0x28 := the statue GUID
   (`0x0064A710`).

#### 4.2 Missile at a point (`0x0056EDE0(game, owner, skill, level, class, x, y)`)

1. owner = 0 → 0.
2. (x, y) = (0, 0) → the owner's path-target position (`0x0056D2C0`:
   `0x00553540` target unit, its position as in §4.1 step 2); still (0,
   0) → 0.
3. Distance owner → (x, y) (`0x006417F0`) > 100 → 0.
4. Zeroed record: flags 1 (position given), owner, class (+0x10), (x,
   y) at +0x14 / +0x18, skill (+0x2C), level (+0x30); return
   `0x0059FA30`'s missile.

Quest use: Baal's death (`0x0058E0F9`, `quests-act5-2.md` §8.5): owner =
the victim, skill 0, level 1, class 625 (`baalfx control`), (x, y) = the
victim's position. That missile later spawns Tyrael (`0x0058E920`,
`missiles/bodies-2.md` §60).

#### 4.3 Orb missile (`0x005DFEE0(game, monster M)`, Khalim's Will)

Called by the Compelling Orb operate (`0x005BB980` at `0x005BBA9D`,
`quests-act3.md` §6) right after `0x005DDFC0(game, M, 0, 0, 0)` (mode
request mode 0 at (0, 0), `monsters/ai.md` §7.1) on the orb monster M.

1. O := the first unit of type 2 and class 386 (`Dummy` / `stairsr`)
   among M's room and its adjacent rooms (`0x0065A620(room, 2, 386)`,
   room list `0x00619790`); none → fatal 0x8F.
2. Zeroed record: flags 0x420, owner = origin = M, class 368
   (`orbmist`), target = O's position (as §4.1 step 2), level 1. Create
   (`0x0059FA30`).
3. Made → missile data +0x28 := O's GUID (`0x0064A710`); O's room
   refresh `0x0061AED0(room, 0)` (`drlg/rooms.md` §8). Not made and O's
   mode is 0 → O mode := 1 and ENDANIM on O at frame + (`FrameCnt1` of
   object 386 >> 8) (`0x005417D0`).

### 5. End a player's interaction (`0x005351C0(game, player)`)

Caller (quest code): the classic Diablo credit `0x005B4A80`
(`quests-act4.md` §5.8), once per player.

1. (kind, GUID) := the player's interaction (`0x00554100`); none, or
   the unit (`0x00552F60(game, kind, GUID)`) is gone → step 3 with
   kind 6, GUID 0.
2. By kind (table `0x005352A4`):
   - 0 (another player: trade): the C→S 0x4F body with button 6
     (`0x00568060(game, player, 6, 0)`, `sim/intents-events.md` rule
     15); **return** (no 0x62).
   - 1 (NPC): reset the interact unit (`0x00554190`, GUID −1, type 6,
     flag 0), then unlink and free the player's chat node from the NPC's
     interaction list (`0x00572E00`; NPC monster data +0x30, none when
     the unit is not a monster). No state test, no quest event 2, no
     gamble-list drop (unlike C→S 0x30, `world/npc.md` §3). Then 0x62
     with kind 1 and the NPC's GUID.
   - 2 (object, `0x005854D0`): class 80 or 90 (obelisks) → the C→S 0x44
     body `0x005852E0(game, player GUID or −1, object GUID, 0, 2)`
     (`quests-act2-2.md` §3.2); class 337 (Steeg Stone) → object data +0
     := −1 when it holds this player's GUID (`0x00584820`); class 152
     (orifice) → object mode 0 (`0x00624690`); then (every class) reset
     the interact unit. Then 0x62 with kind 2 and the object GUID.
   - 3 → 0x62 with kind 6, GUID 0.
   - 4 (item): the item's code is `box ` (Horadric Cube) → reset the
     interact unit and close the cube (`0x00567330` → `0x0055FA40`, as
     `world/cube.md` button 0x17); then 0x62 with kind 4 and the item
     GUID. Another code → return (no 0x62).
3. S→C 0x62 (7 bytes, `0x0053D6D0`) to the player's client: u8 0x62,
   kind u8@1, GUID u32@2; byte 6 is not written (stale stack byte;
   d2rs writes 0, traces mask it).

### 6. End the game (`0x00530590(game, client c)`) — host

1. C1 := the **last** client of the game list (game +0x88, next +0x4A8)
   in state 4 for which `0x00539030` holds (callback `0x0052CBB0`
   through `0x0052DED0`).
2. C1 found → drop it (`0x005303D0(0x00538590(C1), 0)`: the host's
   client removal: save, transport, disconnect). Not found and c = 0 →
   C2 := the **first** client in state 4 (callback `0x0052CBD0`); found
   → drop it the same way. Not found and c ≠ 0 → drop c.
3. The quest code calls it with c = 0 (`quests-act4.md` §5.8).
4. The removal `0x005303D0` tests the dropped client's flag (+0x3D4
   bit 5, `0x00539030`) once: set → it keeps dropping the game's head
   client (game +0x88, `0x00539070`) until none is left (the whole game
   ends); clear → only that client is dropped (`quests-act4.md` §5.8,
   2026-10-08).

Owner (answers `quests-act4.md` open question 10): §6 and the save pass
`0x0052E2A0` (`quests-act5-2.md` open question 3) are host code:
`d2-server` owns them. `d2-sim` raises them as host requests at the
point the quest rule calls them (`QuestWorld::end_game`,
`QuestWorld::save_pass`) and changes no game state itself; the host
runs them after the current frame's quest step, in call order.

### 7. Close a player's town portal (`0x00535430(game, player)`)

`0x005353F0(player)` = the GUID of the player's town portal (player
data +0x48; null player → fatal 0x24D, no player data → 0x250).

1. Null player → fatal 0x25A; no player data → fatal 0x25D.
2. P := the object (type 2) with that GUID; none, or its class ≠ 59
   (town portal) → nothing.
3. Chain 35 hook `0x0058CF50(game, P)` (`quests-act5-2.md` §7.6
   "Closed"). Q := P's partner (`0x00553720`: streams the room at P's
   destination point (object data +0x14: act of byte +4, x +0x18, y
   +0x1C; `0x00619DA0`, else `0x0061A140` + `0x0052D0F0`), then the unit
   whose type / GUID are P +0x94 / +0x98).
4. P leaves its room when it has one (`0x0061A270(room, 2, GUID)`), is
   freed (`0x00555600`), and P's room (possibly null) is refreshed
   (`0x0061AED0(room, 1)`).
5. Q exists → the same three steps for Q (hook `0x0058CF50(game, Q)`
   first).

Player data +0x48 is not cleared here. Caller in the quests: the altar
(`0x0058D2C0`: only when P's room's level is 120, `quests-act5-2.md`
§7.8); the other callers (`monsters/population.md`, `sim/units.md`) use
the same function.

### 8. Find a player's item by code (`0x00558110(game, player, code)`)

Used by the quest item deletion (`world/quests.md` §9.2) and every
quest "holds item c" test that calls it.

1. No inventory (player +0x60) → none.
2. The cursor item (`0x0063C1E0`), when its code (`0x00628590`) is c:
   returned when its `items.txt` record is missing, or `quest` (+0x12A)
   = 0, or its stat 356 (`questitemdifficulty`, total value
   `0x00625480`) ≥ the game difficulty (game +0x6D). (`questdiffcheck`
   is not tested for the cursor item.) Otherwise the search goes on.
3. Then every item of the inventory list in list order (first
   `0x0063B2C0`, next `0x0063DFA0`, item `0x0063DFD0`; equipped, belt,
   cube, stash and inventory items alike), skipping items on page 1
   (`0x00628250` = 1, a trade page, `items/inventory.md` §1.2): code c →
   returned when the record is missing, or `quest` = 0, or
   `questdiffcheck` (+0x12B) = 0, or stat 356 ≥ difficulty.
4. None → none.

So a quest item with stat 356 < difficulty is skipped in the list when
its record has `questdiffcheck`, and skipped on the cursor whenever its
record has `quest` (rule 2 ignores `questdiffcheck`; reproduced).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| free-spot box size | 2s + 1 per axis | `0x00545427` |
| critical spawn spreads | −1, 5, 10 (× 20), 15 | `0x00545A50`, `0x00545A6A`, `0x00545AF2`, `0x00545B18` |
| critical inside walk | 21 steps (d ≤ 20) | `0x005459FF` |
| superunique kind 2 offset | `monstats.txt` row count (data tables +0xA80) | `0x00659BA6` |
| missile 541 flags / level | 0x420 / 1 | `0x0058C902`, `0x0058C979` |
| missile wrapper range | 100 | `0x0056EE3C` |
| interaction kinds | table `0x005352A4` (0, 1, 2, 3, 4) | image |
| orb missile | class 368, object 386, flags 0x420, level 1 | `0x005DFF32`, `0x005DFF39`, `0x005DFEE9` |
| town portal class | 59 | `0x00535482` |

## Randomness

None in these functions. The spawns they call draw on the unit / game
seeds as `monsters/init.md` and `monsters/population.md` state.

## Edge cases & original bugs

1. §1 compares the row coordinate with the box's x end when choosing
   the column room; candidates outside the row room never succeed.
2. §1's row-room test reads the box left by the previous column (not
   necessarily R0's).
3. §2 with no inside point uses (y, y + 21) as the search start.
4. §5 kind 0 and a non-cube item send no 0x62; byte 6 of 0x62 is stale.

## Test vectors

Synthetic (from the rules; no recording).

| Input | Expected | Rule |
|---|---|---|
| §1: one room box (0, 0, 100, 100), point (50, 50), s 2, limit 3, everything free | k = 1 corner (48, 50) accepted → spot (49, 51) | 1 |
| §1: same, limit 1 | *out 0, point unchanged | 1 |
| §1: only corner (49, 49) free, s 2 | k = 2 last candidate (i 0, j 0): spot (50, 50) | 1 |
| §2: R box (0, 0, 10, 10), x 20, y 3 | inside walk fails → search from (3, 24) | 2 |
| §3: kind 2, id 42, `monstats.txt` with N rows | class N + 42 → superunique 42 | 3 |
| §4.2: owner at (10, 10), (x, y) = (200, 10) | distance 190 > 100 → no missile | 4.2 |
| §5: player chatting with NPC GUID 7 | node freed, interact reset, `62 01 07000000 ??` | 5 |

## Provenance

1.14d `Game.exe` (`tools/ghidra/disasm.py fn` / `at`; exports
`functions.tsv`, `index/calls.tsv`): `0x00545340`, `0x005459A0`,
`0x00545C30`, `0x00659B80`, `0x0058C8D0` (caller `0x0058C9A0`),
`0x0056EDE0` (`0x0056D2C0`; caller `0x0058E0F9`), `0x005351C0` (table
`0x005352A4`, `0x00579130`, `0x00572E00`, `0x005854D0`, `0x00584820`,
`0x00567330`, `0x0053D6D0`), `0x00530590` (`0x0052CBB0`, `0x0052CBD0`,
`0x0052DED0`, `0x005303D0`), `0x005DFEE0` (caller `0x005BBA9D`,
`0x0065A620`), `0x00535430` (`0x005353F0`, `0x00553720`, caller
`0x0058D2C0`). Object and missile names from the 1.14d
`objects.txt` / `missiles.txt` (Patch_D2). D2MOO not used.

## Open questions

1. §6 drops one client per call; whether the classic Diablo end reaches
   it once per game or per client is the recording of
   `quests-act4.md` open question 2.
