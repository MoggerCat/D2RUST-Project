# Spec: World — Hirelings part 2 (player death, restore details, swap timers, entry points)

- **Status:** draft: read from the 1.14d `Game.exe` disassembly and the
  decompiled exports (addresses inline) on 2026-10-07; no recording of
  a player death with a hireling, a restore or a give / take. Not
  implemented yet (§15, §16, §19); §17 and §18 correct or complete
  rules already implemented from `world/hirelings.md`.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::hirelings`
- **Related specs:** `world/hirelings.md` (part 1: §1–§14; its
  notation, node layout, "living" / "any" finds, threshold(n) and
  messages are used here unchanged; its Constants, Randomness and
  Provenance lists also cover this part), `sim/pets.md` (pet types
  other than 7), `sim/tick.md` §5 (timer events, types 2, 3, 9),
  `sim/units.md` §4.5 (player mode 17), `combat/vitals.md` §4.7
  (corpse creation), `formats/d2s.md` §2.5, §9 (save block, load
  sequence), `formats/d2s-load.md` §2, `sim/path-placement.md` §10
  (placement, pet follow on teleport), `items/inventory-moves.md` §7.23
  (C→S 0x61), `world/npc.md` §7 (hire, resurrect).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–50 |
| Inputs | 51–55 |
| Outputs / state changes | 56–60 |
| Rules | 61–62 |
|   12. Services (links) | 63–75 |
|   15. Player death (`0x0057FCA0` → `0x00575BC0`) | 76–110 |
|   16. Restore details (part 1 §10) | 111–166 |
|   17. Item swap timers and a failed duplicate (part 1 §11) | 167–201 |
|   18. Pet-follow details (part 1 §6) | 202–237 |
|   19. Entry points and tables | 238–270 |
| Constants & data dependencies | 271–277 |
| Randomness | 278–283 |
| Edge cases & original bugs | 284–297 |
| Test vectors | 298–310 |
| Provenance | 311–322 |
| Open questions | 323–329 |
<!-- /index -->

## Summary

Part 2 of `world/hirelings.md`, split at § boundaries to keep each file
under 60 KB; section numbers continue from part 1. §15 is the player's
death (the hireling dies with the player), §16 the details of the save
restore that part 1 §10 left open, §17 the timer cancels of the item
swap (part 1 §11 named them "notices"), §18 the pet-follow details of
part 1 §6, and §19 the entry points and tables a game must wire.

## Inputs

As `world/hirelings.md` Inputs, plus the timer lists of the hireling
and the player (`sim/tick.md` §5).

## Outputs / state changes

As `world/hirelings.md`: the hireling node, unit flags and mode, room
removal notices, S→C 0x7A and 0x9B, cancelled timers.

## Rules

### 12. Services (links)

Moved here from `world/hirelings.md` (number kept); the § numbers in
the table are part 1's.

| Service | Owner | Hireling-side rule here |
|---|---|---|
| hire (C→S 0x36) | `npc.md` §7.3 | offer §2, init §3, replace §3.2 rule 4 |
| resurrect (C→S 0x62) | `npc.md` §7.4 | cost §9 rule 1, revive §9 |
| heal on chat open | `npc.md` §5 step 5 | life to max, curable states; mana not touched |
| quest-granted hireling | `npc.md` §7.5, `quests.md` | init §3 |
| command (C→S 0x46, 0x47) | AI spec | — |

### 15. Player death (`0x0057FCA0` → `0x00575BC0`)

1. The player mode-17 start (`0x0057FCA0`, `sim/units.md` §4.5) creates
   the corpse (`0x0057F700`, `combat/vitals.md` §4.7) and then calls
   `0x00575BC0(game, player)` at `0x0057FD25`, **in every game type**
   (no expansion test on this path; the act-change call of part 1 §6
   rule 3 is the classic-only one). So the hireling dies when its owner
   dies.
2. `0x00575BC0` is part 1 §6 rule 4 unchanged: for each pet type t = 1
   … count − 1, `0x00574570(player, list t, keep = keep_dead = (t =
   7))`; then the pet maxima are recomputed (`0x00575900`, part 1 §5
   rule 3). Types other than 7 are killed and freed (`sim/pets.md`).
3. For the hireling node (type 7; one node, part 1 §5 rule 3) whose
   unit exists, in order:
   1. broadcast S→C 0x7A action 0 with only the GUID (`0x005538D0`);
   2. post a removal notice {type 1, GUID} on the unit's room
      (`0x0061A270(room, 1, GUID)`; no room → nothing). The unit is
      **not** unlinked from its room here and not freed;
   3. node living (bit 0 clear): bit 0 := 1, list head := this node
      (§18 rule 3); if the unit is in a room, a mode request for mode 0
      (death) on it (`0x005A7E60(unit, 0, &req)`, request +0x08 := the
      `0x00552FD0` result, the target as in `sim/pets.md`'s expired-pet
      kill, then `0x005A7C20(game, &req, 1)`; `monsters/ai.md` mode
      request); S→C 0x9B {u16 name id of the node, u32 resurrect cost
      of the unit (part 1 §9 rule 1)} to the player's client
      (`0x0053E0E0`);
   4. node already dead: only steps 1 and 2.
   A node whose unit does not exist (GUID not found) is left as it is.
4. The node stays, so the hireling is resurrected at the seller in an
   expansion game (part 1 §9) and replaced by the next hire in a classic
   one (part 1 §3.2 rule 4). Count and max are unchanged for type 7.
5. Messages, in order, all in the phase of the mode-17 start: the
   corpse's messages, then 0x7A, then (living node) 0x9B; the death mode
   request's own messages follow the request.

### 16. Restore details (part 1 §10)

1. **Class argument.** `0x005774F0(game, player, name, seed, Id, class,
   dead)` allocates the unit with the class argument (ECX type 1, EDX =
   class; `0x0057762E`), not with the class of the row it looks up. The
   ≥ 0x5C loader (`0x0056AA50`) and the old loader's main path
   (`0x00533C70`) pass the `Class` (+0x08) of row (`Id`, level 1); the
   old loader's version-0x47 path (`0x00533CB1`, header word `JM`
   0x4D4A, only when seed ≠ 0 and the name word ≠ 0) passes `Id` 0xFFFF,
   class 0 and dead 0.
2. **`Id` 0xFFFF** (part 1 §10 rule 3, reached only from rule 1's
   version-0x47 path): row := the first candidate of (game expansion,
   act of the name, game difficulty +0x6D) (part 1 §1.2 rule 1, `after`
   0); none → fatal (string 0x744). The unit is monster class 0 (the
   class argument). The init (part 1 §3.2, `saved_id` 0xFFFF) takes the
   new-hire branch: the node gets the offer's row `Id` (rule 6 there)
   and the unit the offer's experience and level (rule 9), from the
   saved seed and name; the 0x47 path writes no node values or
   experience of its own. Original bug, reproduced.
3. **Placement.** The unit is allocated with **no room** and point
   (0, 0) (`0x00555230(1, class, 0, 0, game, room 0, flag 1, mode, 0)`,
   mode 12 when dead else 1). The seller's hire list is the one of the
   NPC record `0x00535EA0` finds for the row's `Seller` (+0x14, passed
   in EDX). A living restored hireling gets a room at the join
   placement: `0x005394A0` calls `0x005773D0(game, player)` at
   `0x005396C3`, which runs the pet follow (part 1 §6 rule 1) to the
   player's position when the player has a living hireling (`(7, 0)`).
   A dead one stays roomless, as part 1 §8 rule 5.
4. **Row lookup inside the level loop.** Both loaders look up row
   (`Id`, lvl) once per level step (`0x006562F0`). The ≥ 0x5C loader:
   a null row ends the loop, frees the unit (`0x00555600`) and returns
   0 (no hireling, no error). The old loader: a null row is fatal
   (string 0x1660). Live rows never give null (part 1 §1.2 rule 2
   returns the first bracket below every `Level`). The first lookup
   (part 1 §10 rule 2, row (`Id`, 1) before the unit exists) is the
   same split: the ≥ 0x5C loader skips the hireling and goes on
   (`0x0056AA93`, no error), the old loader fails the load with
   "Unable to load merc." (error 0xE).
5. **Rule 7 of part 1 §10, in each loader's order.** ≥ 0x5C loader,
   after §4 at the final level: dead → `0x005751A0(player, merc)` (part
   1 §8 rule 2: node dead, 0x9B, 0x7A), unit flags +0xC4 |= 0x10000,
   mode 12 (`0x00553570(merc, 12)`), `0x005738D0`; then, dead or alive,
   no inventory (+0x60 = 0) → create one (`0x0063ABD0(game, merc)`,
   `items/inventory.md`). Old loader: the inventory step first, then
   the dead steps.
6. **`0x005738D0(game, merc)`** cancels the merc's pending timer events
   of type 2 (AI think) and type 3 (stat regeneration), any argument
   (`0x00540E60(game, merc, 2, 0)`, then `(…, 3, 0)`; `sim/tick.md`
   §5). A dead restored hireling therefore has no AI think and no
   regeneration timer until the revive starts them again. Other
   callers: the old loader `0x00533EEE`, `0x0053A9B0` (`0x0053AA2A`),
   `0x005A6520` (`0x005A65EC`).
7. **Pet node.** `0x005749B0(merc GUID, {seed, name, Id})` (part 1 §10
   rule 4) runs right after the unit exists and before the experience
   step, so §4's row lookups use the saved `Id`.

### 17. Item swap timers and a failed duplicate (part 1 §11)

1. **The "notices" of part 1 §11 rule 3 are timer cancels.**
   `0x00540E60(game, unit, type, a)` cancels the unit's pending timer
   events of that type whose first argument (+0x14) equals a (a = 0:
   any). In the shared tail (`0x0054D06B`, `0x0054D07B`):
   `0x00540E60(game, merc, 9, C GUID)`, then `0x00540E60(game, player,
   9, C GUID)` cancel the periodic-stats events (type 9) tied to the
   given item on the merc and on the player. At the end
   (`0x0054D106`, `0x0054D11F`): cancel the merc's type-3 events (any
   argument), then schedule type 3 on the merc at game frame + 1 with
   arguments 0, 0 (`0x005417D0`): the merc's regeneration restarts next
   frame. Nothing is sent by these calls.
2. **Failed duplicate.** `0x0055A2A0(game, item, owner, 1)` serialises
   the item and creates a new one from the bytes (`0x006313E0`,
   `0x00558CB0`); it returns none when that creation fails, and also
   when the creation of a socketed child fails (the half-built parent is
   then left without an owner). With no copy of C: the mode set is
   skipped (`0x00624690` ignores a null unit), the two type-9 cancels
   run, the equip runs with GUID −1 (`0x005606B0` finds no unit and
   returns 0: nothing equipped), C is still consumed (`0x0055EEA0`) and
   the cursor cleared; with an old item, its copy still goes to the
   player's cursor (a null old copy: cursor := none, `0x0055FB10` with
   none). The tail and result 1 are unchanged. So the item is lost and
   the merc's slot stays empty. Live data does not reach it (item pool
   exhaustion only).
3. Socketed items: each child is recreated in mode 4 and inserted into
   the copy (`0x00562660`; a failed insert is fatal, string 0xDD4); the
   copies get item flags 0x80000 set and 0x2000 cleared, and the source
   gets 0x8000000 (`0x006280D0`). Copies with stat 252 or 253
   (`item_replenish_durability`, `item_replenish_quantity`) ≠ 0 schedule
   a type-3 event on the copy at frame + 2500 / value + 1 (signed
   division) when it has none pending (`0x005415A0`), the replenish timers
   of `items/generation.md` §9 step 6.

### 18. Pet-follow details (part 1 §6)

1. **Range pets** (part 1 §6 rule 1, `0x00575380(game, player, list,
   1600)`, `push 0x640` at `0x0057554A`): for each node with bit 0
   clear whose unit exists: d := dx² + dy² (`0x006492A0`) between the
   pet's position and the **player unit's** position (static path
   +0x0C / +0x10 for objects, items and tiles, else the path's x / y
   `0x006488C0` / `0x00648900`; no path → 0); d > 1600 (signed) →
   remove with kill (`0x005750E0(game, player, GUID, 1)`, part 1 §5
   rule 5). The (x, y) arguments of the follow are not used by this
   branch. Pet type 7 has `range` 0 in 1.14d, so hirelings never take
   it.
2. **Free branch** (no `warp`, no `range`): every node of the type is
   freed with its unit (`0x00574C60`, `sim/pets.md`).
3. **List head in the classic act change / player death**
   (`0x00574570` with keep = keep_dead = 1): the walk visits nodes from
   the head; a living node with a unit becomes the head (`list head :=
   node`), which drops every earlier node from the list without freeing
   or counting it. With type 7 the list holds at most one node (part 1
   §5 rule 3), so the node is already the head and nothing is dropped.
   After the walk, a list with count 0 gets head := none. Keep = 1
   leaves count and max unchanged; keep = 0 (other types) decrements
   both and frees each node (`sim/pets.md`).
4. **Flags 2 bit 0x100 on a revived hireling** (`world/hirelings.md`
   §8 rule 5, open question 10): two readers in 1.14d (`all.asm`: the
   80 loads of +0xC8 followed by a bit-8 test, and direct `test`/`and`
   of +0xC8 / +0xC9). `0x0056D840` (the owner's linked-unit kill,
   `skills/bodies.md`) frees a unit with the bit instead of killing
   it; `0x00574450` (expired-pet kill) asserts (fatal, line 0x5A) when
   the bit is set. Neither reaches a type-7 node with live data:
   hirelings are not skill-linked units, and no type-7 path calls
   `0x00574450` (the eviction of §5 rule 3 cannot happen after §3.2
   rule 4, and the classic act change, player death and player free
   keep or free type-7 units without it). So the bit changes nothing
   for a revived hireling.

### 19. Entry points and tables

The callers a game must wire to the rules of part 1 and this part, in
1.14d, with the phase they run in.

| Rule | Entry (1.14d) | Called from | Phase |
|---|---|---|---|
| hire, init (§2, §3) | `0x00573270` | C→S 0x36 `npc.md` §7.3; quest grant `npc.md` §7.5 (`0x005770E0`) | input phase |
| revive (§9) | `0x00579C00` → `0x00579AA0` | C→S 0x62 handler `0x0054BC00` (`npc.md` §7.4) | input phase |
| death (§8) | `0x0057CCB0` → `0x005751A0` | the 14 kill callers of part 1 §8 rule 1, inside the kill, before the killer bookkeeping and the death mode | where the kill runs (combat, skills, missiles: tick) |
| player death (§15) | `0x00575BC0` | player mode-17 start `0x0057FCA0` (`sim/units.md` §4.5) | the player's mode change |
| classic act change (§6 r3–r4) | `0x00575BC0` | `0x0053ACC0` (from the level warp `0x0053AEC0`), classic only | where the warp runs |
| follow (§6 r1) | `0x005754B0` | placement `0x00554EA0` at `0x00555057` (`sim/path-placement.md` §10); act change `0x0053ACC0` at `0x0053AEA6`; revive `0x00579BE0`; `0x005773D0` (join placement `0x005394A0`, §16 rule 3) | caller's |
| kill share (§7.1) | `0x0057E990` | `0x005A4EF0` (`combat/vitals.md` §4.3) | in the kill |
| restore (§10, §16) | `0x0056AA50`, `0x0056AC10` | save load `0x0056B180` (`formats/d2s.md` §9 rule 3) from `0x00534330`; old saves `0x00533C70` | game join |
| item swap (§11, §17) | `0x0054CED0` | C→S 0x61 handler `0x0054D230` at `0x0054D3A2` (`inventory-moves.md` §7.23) | input phase |
| room deactivation (§8 r5) | `0x005752B0` | `0x005431F0` (tick step 9, `drlg/rooms.md` §8) | room pass |
| join sync (§5 r6) | `0x00574F80` | `0x0053CA60` | game join |
| player free | `0x005746D0` | `0x005349D0`, `0x00539DA0` (`sim/pets.md`) | leave |
| AI skill pick (§14) | `0x005E4D30` | `0x005E5050` ← AI think `0x005E52D0` (`monsters/ai-bodies-6.md` §7) | AI think event |

Tables (loaded once with the game data; `hireling.txt` by `0x00655720`
from the data loader `0x0065A400`): `hireling` rows and the per-(version,
`Id`) first-row table (`data/runtime-maps.md` §8), `pettype` row 7 (flag
byte +4, `basemax`), `experience` (class-0 `MaxLvl` table word 0, the
`MaxLvl` row's `ExpRatio`, per-level `ExpRatio`; part 1 §7.2), `skills`
(count, `reqlevel` +0x174, `pettype` +0xBE, `petmax` calc +0xC0 for the
max recompute), `monstats` (`killable` flag bit 15, the four hireling
classes), `itemtypes` (`BodyLoc1` / `BodyLoc2` for §11 rule 2) and
`string.tbl` (0xD7C `merclevelup`). A game whose tables lack row 7 of
`pettype` or the `hireling` rows has no hireling rules (the 1.14d data
always has both).

## Constants & data dependencies

As `world/hirelings.md`. Added: range-pet squared distance 1600
(`0x0057554A`); timer types 2, 3 (`0x005738D0`), 9 and 3 (`0x0054CED0`);
fatal strings 0x744 (restore row), 0x1660 (old loader level loop), 0xDD4
(socket insert of a duplicate).

## Randomness

None of §15–§18 draws. The death mode request (§15 rule 3) and the
recreated items (§17) draw only in the code they call (monster mode and
item creation specs).

## Edge cases & original bugs

Reproduced by default.

1. **The hireling dies with its owner** (§15) in expansion games too;
   the expansion player resurrects it at the seller.
2. **Removal notice without unlink** (§15 rule 3 step 2): clients drop
   the hireling while the server keeps it in its room in death mode.
3. **Version-0x47 saves restore a class-0 monster** as the hireling
   (§16 rule 2).
4. **A failed duplicate destroys the item** (§17 rule 2).
5. **A dead restored hireling has no AI or regeneration timers**
   (§16 rule 6) until it is revived.

## Test vectors

Synthetic (CI):

| Input | Expected | Source |
|---|---|---|
| player dies, living hireling L 20 in a room, name id 4000 | 0x7A {GUID only}; room notice {1, GUID}; node bit 0 := 1; mode request 0; `9b a00f b80b0000` (cost (400/2)·15 = 3000) | §15 |
| player dies, hireling node already dead | 0x7A, room notice; no 0x9B | §15 rule 3 step 4 |
| range pet at (100, 100), player at (130, 125) | 900 + 625 = 1525 ≤ 1600: kept | §18 rule 1 |
| range pet at (100, 100), player at (141, 100) | 1681 > 1600: removed with kill | §18 rule 1 |
| restore, dead, ≥ 0x5C save | 0x9B, 0x7A, flag 0x10000, mode 12, merc timers of types 2 and 3 cancelled, inventory created | §16 rule 5 |
| give C to an empty slot | merc and player type-9 timers with arg C GUID cancelled; merc type-3 timers cancelled, one type-3 at frame + 1 | §17 rule 1 |

## Provenance

- 1.14d `Game.exe`, `tools/ghidra/disasm.py` and the decompiled exports:
  `0x0057FCA0` (`0x0057FD1C`, `0x0057FD25`), `0x00575BC0`, `0x00574570`,
  `0x0061A270`, `0x005A7E60`, `0x005A7C20` (§15); `0x005774F0`
  (`0x00577504`–`0x0057765A`), `0x0056AA50`, `0x00533C70`
  (`0x00533CB1`), `0x005738D0`, `0x00540E60`, `0x005773D0`, `0x005394A0`
  (`0x005396C3`) (§16); `0x0054CED0` (`0x0054D03B`–`0x0054D12D`),
  `0x0055A2A0`, `0x005606B0`, `0x00624690` (§17); `0x005754B0`
  (`0x0057554A`), `0x00575380`, `0x006492A0` (§18); `index/calls.tsv`
  for the callers of §19.

## Open questions

1. **Needs recording**: a player death with a living hireling (packets):
   confirm the 0x7A, the 0x9B bytes and what the owner's client shows of
   the hireling afterwards (§15 rule 3: removal notice while the server
   keeps the unit in its room).
