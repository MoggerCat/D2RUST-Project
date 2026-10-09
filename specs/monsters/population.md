# Spec: Monsters — Population (which monsters appear in a room, and where)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  code (addresses per rule). The tick recordings back up three rules:
  population happens only in the room step, rooms are filled newest-first,
  and the Act 1 group sizes match. The recordings hold no RNG draws or
  positions, so no draw sequence has been checked against the running game
  yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::population` (regions, room
  population, packs, bosses, presets, ambient spawns);
  `d2-sim::monsters::placement` (the spawn-point search of §9)
- **Related specs:** `sim/tick.md` §4 (when the room pass calls this
  spec); `sim/rng.md` §3, §5.2–§5.4, §6, §7 (seeds and draw helpers);
  `sim/unit-order.md` §1, §4 (GUIDs, room order); `monsters/init.md`
  (what happens inside one monster's creation: level, stats, boss
  modifiers, superunique init, events); `monsters/ai.md` (spawns started
  by AI functions); `sim/units.md` (claude/phase3-units: unit
  allocation, modes, collision primitives); `drlg/levels.md` §11
  (coordinate lists, populated level, room count, warp points, kind-11
  location), `drlg/rooms.md` (rooms, tile records); skills
  spec (summons); quests spec (quest flags read here);
  `monsters/preset-monsters.tsv` (§11 table, machine-readable).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 54–70 |
| Inputs | 71–81 |
| Outputs / state changes | 82–91 |
| Rules | 92–93 |
|   1. Entry points and order within a room | 94–132 |
|   2. Monster regions | 133–252 |
|   3. Room population (`0x0054EC90(game, room)`) | 253–313 |
|   4. Monster pick (`0x005BDE80(game, region, room, &record, chance, umon)`) | 314–340 |
|   5. Boss or pack (`0x005BE020(region, room)`) | 341–358 |
|   6. Random boss (champion or unique) | 359–441 |
|   7. Packs (`0x0054DF80(game, room, cl, min, max)`, class in EBX) | 442–468 |
|   8. Spawn point in a coordinate rectangle (`0x0054DC40`) | 469–504 |
|   9. Placement search and creation call (`0x005B2A00`) | 505–619 |
|   10. Party minions (monstats minion columns, `0x005B2830`) | 620–671 |
|   11. Preset monsters (DS1 presets) | 672–867 |
|   12. Ambient (wandering) spawns (`0x0054F060(game, room)`) | 868–892 |
|   13. Region bookkeeping | 893–925 |
|   14. Other table-driven and AI spawns | 926–950 |
| Constants & data dependencies | 951–1021 |
| Randomness | 1022–1065 |
| Edge cases & original bugs | 1066–1106 |
| Test vectors | 1107–1178 |
| Provenance | 1179–1202 |
| Open questions | 1203–1261 |
<!-- /index -->

## Summary

Each level has a **monster region** that is built when the game is
created. It holds the level's monster list (picked from the levels.txt
`mon`/`nmon` columns with the monster-region seed), weighted by `Rarity`,
plus the level's density, boss limits and counters. When a room is first
populated (`sim/tick.md` §4 step 2), `0x0054EC90` walks the room's DRLG
coordinate rectangles. Each rectangle gets one density test per 3×3 block
of subtiles, drawn from the game seed. A test that passes picks a monster
class from the region list, using the active room seed. It then chooses
either a **random boss** (champion or unique, with minions) or a **pack**
(MinGrp–MaxGrp members, each with its own monstats party). The spawn point
is found by random tries inside the rectangle, then by a ring search
around each try. DS1 preset monsters (including superuniques) are placed
earlier in the same room step, and a rare ambient spawn can add a
wandering monster to any active room.

## Inputs

| Name | Type | Source |
|---|---|---|
| game | game record | difficulty u8 game +0x6D, expansion flag game +0x70, game seed +0xD0, region array +0xF0, superunique flags +0x1D30 |
| room | active room | active room seed +0x6C, client count +0x78, populated bits +0x34 (`sim/tick.md` §4), DRLG room +0x10 |
| coordinate list | DRLG room coordinate rectangles (D2MOO `D2RoomCoordListStrc`) | `0x0061AD50(room)`; clipped rect (tiles) +0x10, node flag +0x20, index +0x28, next +0x2C (`drlg/levels.md` §11; every DRLG read of this spec: §11.6 there) |
| preset units | DS1 preset list of the room | `0x00619FD0(room)` (DRLG spec) |
| tables | levels, monstats, monstats2, superuniques, monumod | Constants & data dependencies |
| monster-region seed | seed | derived at game creation (§2.1, `rng.md` §5.2) |

## Outputs / state changes

- New monster units, created by the monster creation of `monsters/init.md`
  with the arguments listed in §9.6 and §10–§12.
- Region counters (§13), the superunique flag bitset (game +0x1D30, §11.4).
- Seed steps: the game seed (density, sparse, unit allocation), the active
  room seed (picks, boss type, spawn points), the units' own seeds (group
  sizes, minion counts), and the monster-region seed (at game creation
  only).

## Rules

### 1. Entry points and order within a room

1. The tick room pass `0x0052D160` (`sim/tick.md` §4) runs, for each
   active room in act room-list order (newest-activated first): ambient
   spawns `0x0054F060` (§12); then, the first time the room is populated
   (room +0x34 bit 0 clear), preset units `0x005559A0` (§11), inactive
   unit restore `0x00542B40` (`sim/units.md`), object population
   `0x00552610` (objects spec), and room monster population `0x0054EC90`
   (§3).
2. `0x0052D0F0(game, room)` runs the same sequence for one room,
   including the ambient spawns `0x0054F060` of rule 1 (first, for every
   call), without the tick's act-flag test (`sim/tick.md` §4 r5). D2MOO
   1.10f `sub_6FC385A0`. It runs outside the tick room step, inside the
   caller's step (1.14d callers, read from the disassembly):
   - `0x00553720(game, portal)`: resolves a portal object's partner. It
     reads the destination level (object data +0x04) and point (+0x18,
     +0x1C), looks for an active room of that act containing the point
     (`0x00619DA0`); if none, it streams the room there (`0x0061A140`,
     `drlg/rooms.md` §4.3) and, if that returns a room, populates it at
     once. Callers: `0x00535430`, `0x00571F90`, `0x00584870`,
     `0x00585580` and the town-portal cast `0x005BE290`.
   - `0x0056CF40(game, level, …)`: creates a portal object in the
     destination level: spawn point of tile index 11 (`0x0061B060`,
     `sim/path-placement.md` §11), then populates that room **before**
     testing it for null (a level with no such spawn room would pass a
     null room: the first read in `0x0054F060` faults). Callers: object
     event 11 `0x00581410` (`sim/units.md` §6.4), `0x0056D130`,
     `0x00585580`.
   - `0x0059DFD0`: A2Q6 arrival (`0x00545830` for level 73): spawn point
     of tile index 12 in level 40 (act 1), populated when found, then a
     free point (`0x0064E7E0`, step 7).
3. Recording-confirmed (all three tick recordings, 292 new monsters): every
   new monster unit appears during the `rooms` step and none during
   `events`. Rooms activated together are filled in the reverse of their
   activation order. Examples: `20261006-022304` frame 585 fills rooms
   3 → 2 → 1 of the activation list, and frame 1085 does the same.
   Restored units keep their old GUIDs (frame 2088 of `20261006-015554`:
   4, 6, 7, 8). New GUIDs are consecutive in creation order.

### 2. Monster regions

#### 2.1 Creation (game creation)

1. `0x00547D20(game)`, called from game creation `0x00530930` /
   `0x00530BF0` (`rng.md` §5.2), takes one game-seed step. It sets a local
   seed to `{lo', 666}` (the **monster-region seed**), stores `lo'` at
   game +0xEC (D2MOO `dwMonSeed`), and calls `0x005479C0`. That function
   re-initializes the same seed to `{lo', 666}`, which changes nothing.
2. `0x005479C0` builds one region per level id 1 … (levels count − 1)
   in id order and stores a pointer to it at game +0xF0 + 4·id. Slot 0 is
   unused and level 0 has no region. Each region is a zeroed 0x2E4-byte
   record filled from levels.txt as in §2.2. Then the level's monster
   list is drawn (§2.3), and after that the appearance variants of each
   list entry (§2.4, 3 requested per entry, entries in list order). All
   of this uses the monster-region seed, so a level's draws depend on
   every lower level id.
3. The difficulty check `0x00611D30(difficulty)` (difficultylevels row)
   is a fatal error on failure and has no other effect here.
4. Lookup: `0x00547BB0(array, level id)` returns the pointer and does no
   range check.

#### 2.2 Region record (0x2E4 bytes, D2MOO `D2MonsterRegionStrc`)

| Offset | Type | Field | Init / use |
|---|---|---|---|
| +0x000 | u8 | act | levels `Act` (+0x03) |
| +0x004 | i32 | rooms visited | +1 per population attempt (§3.1); boss chance (§5) |
| +0x008 | i32 | rooms with spawns | +1 per room where population created something (§3.4) |
| +0x00C | i32 | room count | −1 at init; set on first use to the level's populated-room count `0x0061ABF0(act, level)` (`drlg/levels.md` §11.5) |
| +0x010 | u8 | monster count (`nMonCount`) | entries the picker may choose (§2.3, §4) |
| +0x011 | u8 | total rarity | sum of entry rarities (u8, wraps) |
| +0x012 | u8 | entry count (`nSpawnCount`) | entries in use, including ones added by §2.5 |
| +0x014 | 13 × 0x34 | entries | +0 class i16, +2 rarity u8, +3 variant count u8, +4 three 16-byte appearance variants |
| +0x2B8 | i32 | MonDen | levels `MonDen` for the difficulty (+0x1C + 4·difficulty); clamped to 10000 on first population (§3.1) |
| +0x2BC | u8 | MonUMin | levels +0x28 + difficulty |
| +0x2BD | u8 | MonUMax | levels +0x2B + difficulty |
| +0x2BE | u8 | MonWndr | levels +0x2E (copy; §12 reads the levels record directly) |
| +0x2C0 | i32 | level id | |
| +0x2C4 | i32 | wanderers spawned | §12 |
| +0x2C8 | i32 | bosses spawned (`dwUniqueCount`) | §6.3, §13; read as a **u8** by §5 |
| +0x2CC | i32 | evil monsters spawned (`dwMonSpawnCount`) | §13 |
| +0x2D0 | i32 | evil monsters killed (`dwMonKillCount`) | §13 |
| +0x2D4 | i32 | −1 at init | read and written by `0x005FB650` (AI; `monsters/ai.md`) |
| +0x2D8 | u8 | Quest | levels `Quest` (+0x30) |
| +0x2DC, +0x2E0 | i32 | monster level | levels `MonLvl1Ex`+2·difficulty (+0x16) when the game is expansion, else `MonLvl1`+2·difficulty (+0x10); both fields hold the same value (used by `monsters/init.md`) |

#### 2.3 Region monster list (`0x005475E0`)

Inputs: the region, the levels record, the monster-region seed, and
nm = (difficulty ≠ 0).

1. n = min(`NumMon` (+0x32), 13). The source list is `nmon1…` (+0x68,
   count u8 +0x34) when nm, else `mon1…` (+0x36, count u8 +0x33). The
   counts come from the post-load fix-up (`data/fixups.md` §11). Let
   avail = that count and n = min(n, avail). The list is copied to a
   scratch array.
2. Repeat for i = 0 … n − 1, stopping early when avail ≤ 0:
   1. idx = `roll(avail)` (inlined; region seed); class = list[idx].
   2. Only for i = 0, and only if levels `rangedspawn` (+0x31) ≠ 0: up
      to 20 times, if class has the monstats `rangedtype` flag, stop;
      otherwise idx = `roll(avail)` again and class = list[idx]. After the
      20th re-draw the class is kept without a check. That is up to 20
      extra draws.
   3. Remove list[idx] (later entries shift down) and set avail −= 1.
   4. If class is a valid monstats row with the `isSpawn` flag, append an
      entry {class, rarity = monstats `Rarity`}, add the rarity to total
      rarity (u8), and add 1 to the entry count and to the monster count.
      Classes without `isSpawn` are dropped, but their draw still counted.
3. 1.14d data: no 1.14d level list holds a non-`isSpawn` class except
   level 120 (Mountain Top, MonDen 0); `rangedspawn` is set only on
   Act 5 levels 110–119, 123–131, 135; the largest `Rarity` is 2, so the
   u8 total cannot wrap.

#### 2.4 Appearance variants (`0x005BDB20(seed, entry, k)`)

Each entry stores up to 3 appearance variants (16 component choices each),
which the monster's init uses (`monsters/init.md`). The draws matter here
because they share the region seed (§2.1) or a unit seed (§2.5). Inputs:
m2 = the monstats2 row of the entry's class (`0x00451FE0`; none → return),
c[i] = component choice count u8 at m2 +0x15 + i (i = 0…15), T = composit
total u8 at m2 +0x25 (`data/callbacks.md` §5), v = variant count (entry
+3).

1. k = min(k, (1 << (T mod 32)) − v); k ≤ 0 → return. (The shift is a
   32-bit shift by T & 31.)
2. If v + k > 3: k = 3 − v; k ≤ 0 → return.
3. If v = 0: variant 0, slot i = `roll(c[i])` if c[i] > 1, else 0, for
   i = 0 … 15 in order. Then v = 1 and k −= 1. k = 0 → return.
4. L = the slots with c[i] > 1, in order; m = |L|. m = 0 → return. If
   m = 1, a = b = L[0]. Otherwise r = `roll(m)`, a = L[r]; L[r] = L[m−1];
   b = L[`roll(m−1)`].
5. For each of the k new variants: copy variant 0; tries = 3; repeat:
   slot a = `roll(c[a])`; if b ≠ a, slot b = `roll(c[b])`; then compare
   the new variant with each existing variant 0 … v−1, and for each
   identical one set dup and tries −= 1. Stop when no duplicate was found
   or tries is exactly 0. Then v += 1.

`roll` here is `0x0045C390` or its inlined copy. A count of 0 gives 0
without a step (`rng.md` §3).

#### 2.5 Entries added on demand (`0x00547BC0(regions, room, unit)`)

Called by monster init (`0x00574250`, `monsters/init.md`) to get the
appearance entry for a monster's class.

1. Classes 195 (act2male), 196 (act2female), 294 (act3male) and 296
   (act3female) get no entry: the call returns null (switch table
   `0x00547CB8`).
2. region = regions[level id of room]. Null → null. Entries 0 … entry
   count − 1 are scanned for the class, and a match is returned.
3. Not found: if the entry count is ≥ 13 → null. Otherwise, if the class
   is valid and its monstats2 `TotalPieces` (+0xEC) > 2, the next free
   entry gets the class, the entry count += 1, and `0x005BDB20(unit seed
   (unit +0x20), entry, 3)` runs. The free entry's address is returned
   whether or not it was filled (a class with `TotalPieces` ≤ 2 gets an
   empty entry: class 0, no variants).
4. The monster count (+0x10) does not change, so added classes are never
   picked by §4.

### 3. Room population (`0x0054EC90(game, room)`)

#### 3.1 Guard (`0x0054EBC0`)

1. region = regions[level id of room] (`0x0061A1B0`). Null → no
   population.
2. rooms visited (+0x04) += 1. This happens for every first population of
   a room whose level has a region, the town included.
3. lvl = `0x0061A1F0(room)` (level id of the populated room; D2MOO
   `DUNGEON_GetLevelIdFromPopulatedRoom`). 0 → no population.
4. If room count (+0x0C) < 0, set it to `0x0061ABF0(act record at game
   +0xBC + 4·region act, lvl)`.
5. MonDen = 0 or room count = 0 → no population.
6. Level 108 (Chaos Sanctum) with `0x005B5210(game)` ≠ 0 (Act 4 Diablo
   quest state; quests spec) → no population.

Then: coordinate list cl = `0x0061AD50(room)` (null → done), and
region = regions[lvl] (looked up again with the populated-room level id).
A null region at either lookup means no population and no draws; §4
and the steps below are only ever reached with a non-null region (the
1.14d region array has an entry for every level, §2.1, so the case is
unreachable; d2rs treats it as "no population").
MonDen > 10000 is stored back as 10000.

#### 3.2 Rectangles and tries

For each coordinate record r, following next (+0x2C) to the end:

1. Skip r if its index (+0x28) is 0, or its node flag (+0x20) is non-zero,
   or its rect (+0x10: left, top, right, bottom, in tiles) has left = 0
   and right = 0.
2. Convert the rect to subtiles (each coordinate × 5, `0x00643560`).
   tries = ((bottom − top) / 3) × ((right − left) / 3), with each division
   signed and truncating. D2MOO 1.10f groups this differently; 1.14d
   multiplies the two quotients.
3. For each try (tries ≤ 0 → none):
   1. **Density:** one game-seed step (inlined). If `lo' mod 100000` ≤
      MonDen, continue; otherwise go to the next try. The spawn chance is about
      (MonDen + 1) / 100000 per try.
   2. **Pick:** class = §4 pick(chance 20, umon 0). If it returns no
      record, **population of the whole room ends here** (§3.4 step 2 is
      skipped as well).
   3. **Kind:** t = §5. Result 1 is turned into 2, so only 0 and 2
      happen.
   4. t = 0 → random boss (§6) at a searched point in r. t = 2 → pack
      (§7) with the class from step 2.
   5. Any boss or pack leader created sets the room's "spawned" flag.

#### 3.3 Dead branch

`0x0054EC90` also has a path for t = 1. It takes a room-seed step
(`0x005BDFF0`) and calls `0x0054E190`, a pattern spawn using the
4-point offset set at `0x007415F0` (D2MOO `sub_6FC67570` +
`sub_6FC6A350`). It cannot run, because step 3.2.3 replaces 1 with 2
before branching. Do not implement it.

#### 3.4 After all rectangles

1. If the spawned flag is set: rooms with spawns (+0x08) += 1.
2. Skipped when step 3.2.3.2 ended the population early.

### 4. Monster pick (`0x005BDE80(game, region, room, &record, chance, umon)`)

All draws use the active room seed of `room` (room +0x6C; `rng.md` §5.4).

1. **Unique list** (umon ≠ 0 and difficulty = 0 (Normal)): count = levels
   umon count (+0x35) of the region's level (`0x0061DB70(level id)`).
   count = 0 → record = null, class = 0. Otherwise class = umon[`roll(count)`]
   (`0x0045C3E0`; list at +0x9A). record = monstats[class], or null if
   class is out of range. Done, with no placespawn step.
2. **Region list** (otherwise, which includes every boss pick in
   Nightmare and Hell): if the region's monster count = 0 → record = null,
   class = 0.
3. w = `roll(total rarity)` + 1; `roll` of 0 gives 0 without a step.
   Walk the entries in order, subtracting each rarity from w, and stop at
   the first entry where w ≤ 0. If none stops, the walk ends on entry
   [monster count], one past the last entry (Edge cases).
4. **placespawn:** if the entry's class is a valid monstats row with
   `spawn` ≥ 0 and the `placespawn` flag: one room-seed step;
   `lo' mod 100` > chance → class = `spawn`. No step without both
   conditions.
5. record = monstats[class] (null if invalid). The return value is the
   class.

1.14d data: `placespawn` is set only on crownest1–4 (spawn
foulcrow1–4). With chance 20, a crow nest pick becomes the crows 79 % of
the time.

### 5. Boss or pack (`0x005BE020(region, room)`)

Room-seed draws, in this order. U = bosses spawned (+0x2C8, low byte),
V = rooms visited (+0x04), N = room count (+0x0C):

1. If U < MonUMin and N ≠ 0: draw `lo' mod 100`. If it is < (100 × V) / N
   (signed integer division) → **0**. With no draw, go on.
2. If U < MonUMax: draw `lo' mod 100`. If it is ≤ 5 → **0**.
3. Draw `lo' mod 100`. If it is > 35 → 2, else 1. Both mean **pack**
   (§3.2.3.3).

So a level forces bosses toward MonUMin as the rooms run out (V/N rises
to 1), and adds more at 6 % per density hit up to MonUMax. Step 3 always
draws, even though its result never matters. Each density hit therefore
costs 1–3 room-seed steps here. Examples: Blood Moor (level 2) in Normal
has MonUMin = MonUMax = 0, so every density hit draws only step 3 and
makes a pack. Cold Plains (3) in Normal has 1/1.

### 6. Random boss (champion or unique)

#### 6.1 Population call

At t = 0 (§3.2): class = §4 pick(chance 0, umon 1) on the same room
seed. Its record is not checked: an empty umon list yields class 0
(Edge cases). Then `0x005A43E0(game, room, cl = r, class, champion
allowed 1, x 0, y 0, warp check 1)`. A non-null result sets the spawned
flag, and the champion minions of §6.4 follow.

#### 6.2 `0x005A43E0` (D2MOO `sub_6FC6E8D0`)

1. Boss unit = `0x005A09E0` (§6.3). Null → return null.
2. Boss modifiers `0x005A0760(boss, game, champion allowed)`
   (`monsters/init.md`). When champions are allowed it decides champion
   versus unique. That is a boss-unit-seed `roll(100)` < monumod row 0
   `constants` (20 in 1.14d) → champion (type flag 4); otherwise unique
   modifiers.
3. Boss minions and modifier init `0x005A2120(game, boss, 1, min 3,
   max 6, cl)`. Minions are spawned by §6.5 (skipped for champions), then
   the init functions of modifiers 1–4 and the boss's own modifiers run
   (`monsters/init.md`).

#### 6.3 Boss spawn (`0x005A09E0`, D2MOO `D2GAME_SpawnMonster_6FC6F220`)

Arguments: game, room, cl, x, y, GUID, class, warp check.

1. If x = 0 and y = 0: the point comes from §8 (cl, class, warp check).
   Failure → null.
2. With cl: placement §9 (r = −1, flags 0x40) inside cl at (x, y).
3. Without cl, GUID = −1: §9 at (x, y) with r = −1, flags 0x40; on
   failure again with r = 5.
4. Without cl, with a GUID (restore paths, not population): flags 0x62,
   r = −1, then r = 5. Then a §8 search without warp check and r = −1.
   Last, the nearest free point from `0x0064E840` (mask 0x3C01, size 1;
   `sim/path-placement.md` §8) with r = −1. Every creation call of
   `0x005A09E0` passes mode 1 (`push 1` before each of `0x005B3040`,
   `0x005B2F20`, `0x005B30E0` at `0x005A0A35`, `0x005A0A54`,
   `0x005A0A76`, `0x005A0A8F`, `0x005A0AB5`, `0x005A0AF7`,
   `0x005A0B48`). The last try searches from the current point:
   the §8 point when the §8 search succeeded but its creation failed
   (`0x0054DC40` writes x, y only on success), else the input (x, y)
   (`0x005A0B0E`–`0x005A0B27`). `0x0064E840` returns its room through
   the &room argument (`[ebp+8]`); null → return null (`0x005A0B34`);
   else the creation gets that room and the moved point
   (`0x005A0B2F`–`0x005A0B4F`), 1.14d-confirmed.
5. On success, `0x005A0320(boss, game)`: if the boss has no type flag 8,
   bosses spawned (+0x2C8) of the region of the boss's level id
   (`0x00573520`) += 1. Then type flag 8 is set, and `0x005A09E0` sets
   type flag 1. Quest hook `0x00544E80(game, boss)` (quests spec), then
   owner data `0x0058F030(game, boss, boss GUID, 1, 1, 0)`
   (`monsters/ai.md`).

Type flags are the u16 at monster data +0x16 (D2MOO `nTypeFlag`): 1 other
boss, 2 superunique, 4 champion, 8 unique (counted), 0x10 minion. Every
boss counts toward the region limits, including champions and
superuniques (§11.4).

#### 6.4 Champion minions (`0x0054E1E0(cl, class)`)

Only when the boss has type flag 4 (`0x005A0180`): count = (`lo' mod 3`)
+ 1 on the **boss's unit seed** (+0x20, inlined). Each one is spawned by
`0x005B2F70(game, cl, boss, class, mode 1, r 4, flags 0)`, which is §9
around the boss's position. Each minion created gets modifier 16 (0x10,
champion mods) via `0x005A48C0` (`monsters/init.md`). The minions are
created with flags 0, so a minion whose class has a monstats party also
gets its party (§10).

#### 6.5 Unique minions (`0x005A0C00`, inside §6.2 step 3)

1. Skipped if the boss has type flag 4 (champion).
2. Minion class = the boss's monstats `minion1` if it is a valid class,
   else the boss's own class (`0x005A0BB0`).
3. count = `roll(max − min + 1)` + min on the boss's unit seed. Random
   bosses use min 3 and max 6. count ≤ 0 → none.
4. Each one: with cl, `0x005B2F70(cl, boss, class, mode 1, r 3, flags
   0x40)`; without cl, `0x005B23C0(boss, class, mode 1, r 3, flags 0x40)`.
   For each minion created: transfer the boss's modifiers that have
   `xfer` (`0x005A0930`, `monsters/init.md`), set owner data
   (`0x0058F030`), add the minion to the boss's minion list
   (`0x0058F100`), set the owner GUID and type (`0x005DD330`), and set
   type flag 0x10.

### 7. Packs (`0x0054DF80(game, room, cl, min, max)`, class in EBX)

1. min = max = 1 if the picked class's `BaseId` is 19 (fallen1) or 91
   (scarab1) (`0x0054EC40`). Otherwise min = `MinGrp`, max = `MaxGrp` of
   the picked record.
2. **sparsePopulate:** if the record's `sparsePopulate` ≠ 0: one
   **game-seed** step; `lo' mod 100` > sparsePopulate → no pack (the try
   ends). 1.14d data: only evilhut (record 528, hcIdx; the compare `0x0054E00F` is against 0x210; file data row 529 counting the dropped `Expansion` separator) has it (40).
3. min = 0, max = 0 or max < min → no pack.
4. Point: §8 (cl, class, warp check 1). Failure → no pack.
5. Leader: §9 at the point inside cl, mode 1, r = −1, flags 0 (party
   included, §10). Failure → no pack.
6. If the class has the monstats2 `objCol` flag (`0x004638A0`, flag
   index 18) and it is 528 (evilhut): object 562 (0x232) is created at the
   leader's position (`0x00555230`, objects spec), allocated in mode 0
   with flag 1 and GUID 0 (`world/objects-2.md` §22 rule 4; the same for
   the barricade-door objects of classes 571 / 572, `0x0054E5AA` /
   `0x0054E5EA`).
7. Members: n = `roll(max − min + 1)` + (min − 1) on the **leader's unit
   seed** (+0x20). Each member: `0x005B2F70(game, cl, leader, class,
   mode 1, r 3, flags 0)`.
8. The pack counts as created if the leader was created.

Real sizes (1.14d monstats): zombie1, quillrat1, cr_lancer1: 1 + `roll(2)`
(1–2); brute1, fallenshaman1: 1; fallen1: 1 leader (forced) plus its party
of 2–3 (§10); corruptrogue1: 1 + `roll(2)` extra members (2–3 in all).

### 8. Spawn point in a coordinate rectangle (`0x0054DC40`)

Arguments: game, room, cl (may be null), class, warp check; outputs x, y.
Draws use the active room seed of `room`.

1. Bounds (`0x0054DAC0`): with cl, take its rect in subtiles; then left =
   x0 + 1, top = y0 + 1, w = x1 − left, h = y1 − top. Without cl, take the
   room's subtile box (`0x00619730`: x, y, width, height); then left =
   x + 1, top = y + 1, w = width − 1, h = height − 1.
2. Up to 20 tries:
   1. x = `roll(w)` + left, then y = `roll(h)` + top (inlined; each with no
      step when w or h < 1).
   2. Warp check (only when asked, `0x0054DB50`): reject if
      dx² + dy² < levels `WarpDist` (+0x0C) for any warp point of the room
      (`0x0061AC10(room)`: x at +4·i, y at +0x24 + 4·i, count at +0x48;
      `drlg/levels.md` §11.5 item 3). Only when no warp point rejects:
      also reject if dx² + dy² < WarpDist for the level's spawn location
      of kind 11 (`0x00619E50` with the act of the level from
      `0x006427F0`; tile coordinates × 5), when that location has x > 0
      and y > 0. That query is the spawn-room choice of `drlg/levels.md`
      §11.5 item 4: it is made again on every such try, can draw on the
      **level seed** and streams the chosen room. `WarpDist` is the
      row of the room's own level (`0x0061A1B0`: level id with no
      flag-0x800000 test, then the levels record `0x0061DB70`); the
      same row serves both tests. 1.14d values in Constants (most rows
      2025 = 45²).
   3. With cl: reject if the coordinate index at (x, y)
      (`0x0061B130`) ≠ cl index.
   4. Probe placement: §9 at (x, y), mode 1, r = −1, flags 1 (test only,
      no unit). With cl it goes through `0x005B3040`, otherwise
      `0x005B2F20`. Success → return (x, y).
3. No success after 20 tries → failure.

D2MOO 1.10f inverts step 2.2 (it spawns only near warps). 1.14d rejects
points near warps.

### 9. Placement search and creation call (`0x005B2A00`)

All monster creation by class goes through `0x005B2A00(spawn record)`.
The callers fill a 0x28-byte record (D2MOO `D2UnkMonCreateStrc`): game
+0x00, room +0x04, cl +0x08, class +0x0C, mode +0x10, GUID +0x14, x
+0x18, y +0x1C, radius r +0x20, flags u16 +0x24. Wrappers: `0x005B2F20`
(room, x, y; no cl), `0x005B3040` (room, cl, x, y), `0x005B30E0` (with
GUID), `0x005B2F70` (cl, near a unit), `0x005B23C0` (near a unit, no cl).
"Near a unit" means the unit's room (`0x00620BB0`) and its current
subtile position (monsters and players: path x/y `0x006488C0` /
`0x00648900`; types 2, 4, 5: static path +0x0C/+0x10).

#### 9.1 Checks and parameters

1. The class needs a valid monstats row and a valid monstats2 row
   (`MonStatsEx`). Otherwise → null.
2. Collision mask from monstats2 `spawnCol` (+0x0A): 0 or > 3 → 0x3C01;
   1 → 0x1C0; 2 → 0x3F11; 3 → 0 (no collision). Jump table `0x005B2F04`.
3. Bounds: with cl, its rect in subtiles plus "index check on"
   (index = cl +0x28). Without cl, the room box (x, y, x + width,
   y + height) with no index check.
4. room = null → null.

#### 9.2 spawnCol 1 classes (water placement)

If `spawnCol` = 1 and the class is not 258–263 (tentacles) and not 153
(hellmeteor), so in 1.14d only frogdemon1–3 (247–249): the point comes
from `0x005B2700(room)` and the room from `0x00463740(room, x, y)`, and
the ring search is skipped.

`0x005B2700`: list = `0x00619660(room, &n)`, the room's tile records
(0x30 bytes each; DRLG spec); n = 0 or no list → none. s = `roll(n)`
(room seed); s = 0 → s = 1. Then visit indices s, s+1, … mod n until
back at s − 1 (never visiting s − 1 itself). For a tile record with tile
data (+0x18) whose material flags (`0x00604BC0`, DT1 header +0x06,
`drlg/rooms.md` §9.3) have bit 0x2: the point is
x = (rec+8 + room tile x) × 5 + 3, y = (rec+0xC + room tile y) × 5 + 3.
The point must pass `0x0064CB30(room, x, y, 0x100)` = 0, and at least one
of (x, y) + (0,−3), (3,0), (0,3), (−3,0) (table `0x006E2D50`) must be
free under `0x0064D9B0(room, ·, ·, 2, 0x1C09)`. The first such point
wins. With n = 1, s becomes 1 (out of range), and the visit loop is
Open question 5.

#### 9.3 Ring search (all other classes)

r < 0 → one ring d = 0. r = 0 → **failure with no draws**. r ≥ 1 → rings
d = 3, 6, …, 3r. For each ring, in order, stopping at the first accepted
point:

1. Four room-seed draws: p = step parity (`lo' & 1`); q = `roll(d)`
   (`0x0045C3E0`, no step when d = 0); sx = parity of the next step; sy =
   parity of the next step.
2. Start offset and direction: p = 1 → offset (d, q), direction (0, +1).
   p = 0 → offset (q, d), direction (+1, 0). If sx = 1, negate the x
   offset; if sy = 1, negate the y offset. Position = (X0 + ox, Y0 + oy),
   where (X0, Y0) is the record's x, y.
3. Square L = X0 − d, R = X0 + d, T = Y0 − d, B = Y0 + d. Repeat 8d
   times (once when d = 0):
   1. Corner turns, tested in this order on the current position (a later
      match overrides an earlier one): (L,T) → (+1,0); (R,T) → (0,+1);
      (R,B) → (−1,0); (L,B) → (0,−1); L = R and T = B (d = 0) → (0,0).
   2. Move one step in the direction, then test the new position:
      1. inside the bounds (left ≤ x < right, top ≤ y < bottom; Win32
         `PtInRect`);
      2. with the index check: the coordinate index at (x, y)
         (`0x0061B130`) equals cl's;
      3. special footprints `0x005FD350(class, room, x, y, 1)`, keyed on
         the class's `BaseId`: 206 crownest1 → test (x, y+3); 228
         sarcophagus → (x, y+2); 298 vilemother1 → always passes (the
         5th argument is 1); 334 suckernest1 → (x−2, y−2) with mask 0x1C0;
         528 evilhut → (x+2, y+4). The tested point's room is
         `0x00463740(room, x', y')`. It fails if that room is null or
         `0x0064D9B0(that room, x', y', 2, mask)` ≠ 0 (mask 0x3C01 unless
         noted). Other classes pass;
      4. collision: `0x0064D9B0(room, x, y, SizeX, mask)` = 0, where SizeX
         is the monstats2 byte +0x08, sign-extended. It is skipped when
         flags & 0x80. The collision test is `sim/path-placement.md` §4.
   3. Accept: the point is kept and the ring loop ends.
4. With an inconsistent start direction (on the left or bottom edge), the
   walk goes the wrong way to the next corner and then turns back, so
   some perimeter cells are tested twice and others never. Reproduce
   exactly. Example in Test vectors.

#### 9.4 Result

1. No accepted point (x = 0 or y = 0 also counts as none) → null.
2. flags & 1 (probe) → return 1, create nothing.

#### 9.5 Creation flags (u16 at +0x24)

| Bit | Meaning | Who reads it |
|---|---|---|
| 0x01 | probe only, no unit | §9.4 |
| 0x02 | skip per-class extras `0x005B21B0` | `monsters/init.md` |
| 0x04 | tentacle offset set 0 instead of 1 (§10.3) | §10.3 |
| 0x08 | do not count in region spawned (+0x2CC); flag 2 on the monster instead | §13 |
| 0x20 | allocate with the caller's GUID (allocation flag 3, else 1; `sim/unit-order.md` §1 rule 4) | allocation |
| 0x40 | no party minions (§10) | §9.6 |
| 0x80 | ignore collision at placement | §9.3 |

#### 9.6 Creation call (what population passes; internals in `monsters/init.md`)

1. Unit allocation `0x00555230(type 1, class, x, y, game, room, flag 1 or
   3, mode, GUID)`. This takes one game-seed step for the unit seed
   (`rng.md` §5.3) and a GUID (`sim/unit-order.md` §1). Null → null.
2. Region spawn count `0x00547D90(regions, room, unit, nc)`, where nc =
   flags & 8, or 1 when the monstats `neverCount` flag is set (§13).
3. Without cl: the monster's coordinate record is set from
   `0x0061AD30(room, x, y)`. With cl: from cl. Done by `0x00552D60`.
4. Alignment from monstats `Align` (+0x4C): 1 → alignment 2 and unit flag
   0x20000 (unit +0xC4); 2 → 1; else 0 (`0x005543B0`; `monsters/init.md`).
5. Unless flags & 2: `0x005B21B0`. Then `0x005B1CF0` (monster init,
   `monsters/init.md`).
6. Unless flags & 0x40: party minions §10.

### 10. Party minions (monstats minion columns, `0x005B2830`)

Runs at the end of every creation without flag 0x40. Its own minions are
created with 0x40, so parties never nest.

#### 10.1 Common

1. m1 = monstats `minion1` (+0x26). If it is invalid (< 0 or ≥ count),
   there is no party.
2. count = `PartyMin` (+0x2C) when `PartyMin` ≥ `PartyMax`; otherwise
   `PartyMin` + `roll(PartyMax − PartyMin + 1)` on the **leader's unit
   seed** (`0x004CC790`).

#### 10.2 Normal classes (BaseId ≠ 261 tentaclehead1)

1. If `SetBoss`: owner data `0x0058F030(game, leader, leader GUID, 1, 1,
   BossXfer flag)` (`monsters/ai.md`). This happens before the count
   draw.
2. two = (`minion2` (+0x28) is a valid class). Minion i (i = 0 …
   count − 1) has class m1 when i is even or when two is false, and m2
   otherwise.
3. Each minion: `0x005B23C0(game, leader, class, mode 1, r 4, flags 0x40)`,
   which is §9 around the leader's position, bounded by the leader's
   room box. For each one created, if `SetBoss`: owner data `0x0058F030
   (game, minion, leader GUID (`0x00451F50`: unit +0x0C, −1 for none), 1, 0, 0)` and
   `0x0058F100(game, leader, minion)` (minion list).
4. Act 1 data: fallen1 → 2–3 fallen1 (SetBoss, BossXfer); fallenshaman1 →
   2–6 fallen1 (SetBoss).

#### 10.3 Tentacle heads (BaseId 261)

Owner data is set first, as in 10.2.1 (always, with `BossXfer`). Then
count is drawn as in 10.1.2, and `0x005B2570(game, leader, m1, mode 1,
count, set = ¬(flags >> 2) & 1, flags 0x40)` runs:

1. k = `lo' mod 6` on the leader's unit seed (inlined). For each of count
   minions: point = leader position + offset[6·set + k] (table
   `0x006E2CF0`, i32 pairs); §9 with r = −1; k = (k + 5) mod 6. For each
   one created: owner data and minion list as in 10.2.3, with no
   `SetBoss` test: `0x005B2570` calls `0x0058F030(game, minion, leader
   GUID (+0x0C, read once at `0x005B25A8`), 1, 0, 0)` and
   `0x0058F100(game, leader, minion)` for every created unit
   (`0x005B26BF`–`0x005B26E4`), 1.14d-confirmed. A set outside 0 … 2
   becomes 0 (`0x005B2591`–`0x005B259A`). The leader position is the
   path position (`0x006488C0` / `0x00648900`) for players, monsters
   and missiles, else the static path's +0x0C / +0x10
   (`0x005B25AE`–`0x005B260D`).
2. Offsets, set 0: (−1,−4) (1,4) (1,−3) (−1,3) (0,2) (0,−2). Set 1:
   (−3,−1) (3,1) (2,−1) (−2,1) (1,0) (−1,0). The function accepts set = 2,
   which would read the next table (`0x006E2D50`) and then 16 bytes of
   unrelated data. This caller can only pass 0 or 1.

### 11. Preset monsters (DS1 presets)

#### 11.1 Order (`0x005559A0`, monster part only)

The room's preset list (`0x00619FD0`) is walked twice. The first pass
places every non-monster preset (objects, etc.; objects spec). Level 136
(0x88) has its own special cases. The second pass places the monster
presets (type 1) whose "done" bit (+0x1C bit 0) is clear, in list order.
Level 136 skips the second pass. Each preset goes through `0x00555910`:
class = preset +0x04, x = preset +0x08 + room subtile x (`0x00619730`), y = preset
+0x18 + room box y, mode = preset +0x00. Then `0x005557D0` →
`0x0054E600(game, room, class, x, y, mode)`. A monster created there gets
`0x0058F000` and, if the preset has data at +0x10, `0x00666120`
(DRLG/AI data; `monsters/ai.md`). `0x005557D0` sets unit flags
0x3000000 on every unit it creates.

#### 11.2 Class ranges (`0x0054E600`)

Let M = monstats count and S = superuniques count.

1. class < 0 → nothing.
2. M ≤ class < M + S → superunique class − M (§11.4).
3. class ≥ M + S → special preset id = class − M − S (§11.5,
   `monsters/preset-monsters.tsv`).
4. Otherwise a regular monster (§11.3).

#### 11.3 Regular preset monster

1. Swaps before the spawn: class 498 (catapult2) → 499 and 517
   (catapultspotter2) → 518, when quest flag 0x1F (`0x005444B0`, `world/quests.md` §2.3; Act 5
   Shenk; quests spec) is set and the level is 110 (Bloody Foothills).
   In Nightmare and Hell, classes 453 (minion1) and 529 (deathmauler1) in
   level 110 are not placed.
2. `0x0054E490`: class 434 (prisondoor) → x − 1, and mode 12 (dead)
   unless quest flag 0x20 is set. Classes with the monstats2 `critter`
   flag (index 13) are **not placed** (the client makes critters from
   Levels.txt `cmon`, §11.7; a DS1 preset with its flag bit 0 set is
   made by the client instead, `client/model.md` §5 r6.2). Flags = 8 if `neverCount`, else 0.
   §9 with r = −1; on failure, unless the class is in the no-retry set
   {229 radament, 284–288 maggotqueen1–5, 392–393 window1–2}
   (`0x0054E3A0`, table `0x0054E3E0`), again with r = 4.
3. If the monster has the monstats2 `objCol` flag: class 432
   (barricadedoor1) and 433 (barricadedoor2) each create an object at the
   monster's position (`0x00555230`, objects spec; object ids are Open
   question 3).

#### 11.4 Superunique (`0x005A49B0(game, room, x, y, su)`)

1. Difficulty ≥ 3 → nothing. Record = superuniques[su] (`0x006556E0`).
   It must exist and have `Class` ≥ 0. If `Stacks` (+0x26) = 0, the bit su
   of game +0x1D30 must be clear (bit = su & 7 of byte su >> 3).
2. `AutoPos` (+0x24) ≠ 0 → x = y = 0 (the point comes from §8 without a
   coordinate record and without warp check).
3. Spawn: §6.3 with cl = null, GUID −1, warp check 0, class = `Class`.
   Failure → nothing. Superuniques count as bosses (§6.3 step 5).
4. Set bit su of game +0x1D30 and type flag 2. Modifiers, name and the
   rest are in `monsters/init.md` (`0x005A0200`, the superunique `Mod1–3`,
   difficulty extra modifiers via `0x005A0600`).
5. Minions: min = `MinGrp` (+0x1C), max = `MaxGrp` (+0x20). If both are
   ≠ 0, add the difficulty (0/1/2) to each. Then §6.5 without cl (r 3,
   flags 0x40) through `0x005A2120(game, boss, 1, min, max, cl = null)`.
   With min = max = 0 it still draws `roll(1)` (one unit-seed step) and
   spawns none.
6. Extra spawns by `hcIdx` (+0x08), after the minions:

| hcIdx | Superunique | Population effect (quest effects: quests spec) |
|---|---|---|
| 10 | Radament | `roll(5)` + 2 skeleton5 (4) via `0x005B23C0` (r 4, flags 0x40), then one each of 276, 382, 385, 389 (skeleton mages). The `roll(5)` is on the boss's unit seed (`0x0045C390` with seed boss +0x20, `0x005A4CD1`–`0x005A4CD9`); count = roll + 2 (never 0); every spawn is `0x005B23C0(game, boss, class, mode 1, r 4, flags 0x40)` and its result is dropped: no owner data and no minion list for any of them (`0x005A4CE5`–`0x005A4D46`; `0x005B23C0` only places, through `0x005B2A00`), 1.14d-confirmed |
| 42 | Siege boss (Shenk) | `0x005B24E0(boss, 453 minion1, 1, 20, 20, 0)` (count/radius: Open question 4) |
| 60 | Nihlathak boss | owner data, then `0x005B24E0(boss, class-for-level(453), 1, 10, 20, 0x40)` |
| 62 | Baal subject 2 | `0x005B24E0(boss, 381 skmage_cold3, 1, 20, 10, 0x40)` |
| 6, 26–27, 29, 36–39, 43–45 | Countess, Act 3/4/5 quest bosses | no spawns; quest records and states only |

Every path ends with modifier 22 (`0x005A4850(…, 0x16, 1)`, quest mod;
`monsters/init.md`).

Act 1 rows (1.14d superuniques.txt): Bishibosh (0, fallenshaman1, MinGrp =
MaxGrp = 2, AutoPos 1), Bonebreak (1, skeleton1, 5/5), Coldcrow (2,
cr_archer1, 4/4), Rakanishu (3, fallen2, 8/8, AutoPos 0), Treehead
WoodFist (4, brute2, 2/2), Griswold (5, griswold, 0/0), The Countess (6,
corruptrogue3, 6/6), Pitspawn Fouldog (7, bighead2, 4/4), Flamespike the
Crawler (8, quillrat4, 6/6), Boneash (9, skmage_pois3, 0/0). Corpsefire
and Blood Raven are not superunique rows. Blood Raven is preset id 5
(§11.5). Where each superunique preset lies is DRLG data (DS1 / outdoor
presets).

#### 11.5 Special preset ids

See `monsters/preset-monsters.tsv` (columns: `id`, `condition`, `class`,
`mode`, `radius`, `flags`, `then`, `source`). Rules that need more than a
row:

1. **id 2:** a random unique. The class comes from the §4 pick (chance 0,
   umon 1) for the region of the room's populated level. If it returns a
   record: `0x005A43E0(cl null, class, champion allowed 0, x 0, y 0, warp
   check 1)`, so never a champion and the point is searched. Returns
   nothing to the preset caller.
2. **id 3:** a champion at the preset point. Same pick. `0x005B2F20(x, y,
   class, preset mode, r −1, flags 0)` (party included), modifier 16
   (`0x005A48C0`), then the champion-minion rule of §6.4 with cl null. Its
   type flag 4 is set by the modifier (`monsters/init.md`).
3. **ids 10, 11:** the tentacle-head chain. Class = row 261's BaseId
   followed `n` steps along `NextInClass` (`0x0054DA60`; stops early at
   an invalid class). n by level (switch tables `0x0054EB50` /
   `0x0054EB60`): 76–78 → 1; 92–93 → 2; all others 0. Then
   `0x005B2F20(x, y, class, preset mode, r −1, flags)` with flags 4 for
   id 10 and 0 for id 11. There is no retry.
4. **ids 17, 18, 22, 23, 29–32:** a fixed class (TSV). Then class-for-level
   `0x0063EC70(room, class)` (§11.6), then the fallen/shaman swap
   `0x0054E2A0`, keyed on BaseId: fallen1 → fallen2 (20) in level 6,
   fallen3 (21) in levels 7, 12, 16; fallenshaman1 → fallenshaman2 (59)
   in levels 6, 7, fallenshaman3 (60) in levels 12, 16. Flags = 8 if
   `neverCount`. §9 r = −1, then r = 4. Ids 29–32 use mode 12 (dead) and
   set unit flag 0x2000000. If the created class is 438
   (reanimatedhorde3), it schedules event 7 (MONUMOD) at frame + 250 +
   `roll(50)` (`0x005417D0`; `monsters/init.md` owns the event).
5. **ids 24, 26:** Normal difficulty only. Class 529 deathmauler1 in
   level 110, else 492 imp1 (id 24); 453 minion1 (id 26). Then
   `0x0063EC70` and `0x0054E090`, a pack at the preset point: MinGrp ≥ 1,
   MaxGrp ≥ MinGrp; cl = `0x0061AD30(room, x, y)`; leader §9 with r −1
   and flags 0; then `roll(MaxGrp − MinGrp + 1)` + MinGrp − 1 members via
   `0x005B2F70` (r 3, flags 0), drawn on the leader's unit seed.
6. **Any other id** (not a TSV row and not above): nothing is created
   and nothing is drawn. The dispatcher `0x0054E600` switches on id − 2
   (`0x0054E76F`–`0x0054E782`): above 30 unsigned (id < 2 or id > 32)
   and the byte-table entries of ids 6, 7, 9, 12–16, 19–21 (table
   `0x0054EB30`, entry 8 → `0x0054EAFE`) return null with no call; the
   steps before the switch (`0x0061A1B0`, the superunique count
   `0x00655710`) draw nothing. The handled ids are exactly the TSV ids
   (2, 3, 4, 5, 8, 10, 11, 17, 18, 22–32), 1.14d-confirmed.

#### 11.6 Class for level (`0x0063EC70(room, class)`, D2MOO `D2Common_11063`)

1. L = levels record of the room's level. If its `mon` count (+0x33) is
   0 → class unchanged.
2. b = BaseId of class (monstats +2, i16); an invalid class (outside
   0 … count − 1) is its own b. b invalid → class unchanged. If some
   `mon` entry has BaseId b (an invalid entry compared as itself, so it
   never matches) → that entry, in `mon` order (1.14d-confirmed,
   `0x0063ECA9`–`0x0063ED34`).
3. Otherwise walk the chain from b's `NextInClass`, for up to the class's
   chain length (`0x006510C0`). Each step takes the next class while its
   `Level` (+0xAA) ≤ L `MonLvl1Ex` (+0x16) + 1. Stop at the first invalid
   or too-high class and return the last accepted one; the input class if
   none was accepted.

#### 11.7 Client-made critters (Levels.txt `cmon`, `0x0046C460`)

Read 2026-10-09 from the 1.14d disassembly. The server never places a
critter (§11.3 r2); the **client** makes them, once per client active
room, from the Levels.txt critter columns (not from DS1 presets). When
the pass runs, the GUIDs and the set-up: `client/model.md` §5 rule 6.

1. **Columns** (levels record, 1.14d parser table at `0x0061D7xx`–
   `0x0061D94F`): `cmon1`–`4` +0xCC (i16 monstats ids, an empty cell is
   negative), `cpct1`–`4` +0xD4 (i16 percent), `camt1`–`4` → **all four
   are parsed into +0xDC** (each field record carries offset 0xDC; the
   last, `camt4`, wins). So the amount of slot 0 is `camt4` and slots
   1–3 read +0xDE/+0xE0/+0xE2, which are always 0 (original bug, also in
   1.10f). 1.14d data: every `camt` cell is empty, so the multiplier is
   never applied. Act 1 town: `cmon1` = chicken (149), `cpct1` = 30.
2. **Pass** `0x0046C460(room)`; R = the room's seed (active room +0x6C,
   `drlg/rooms.md` §2), `rand(n)` = `0x0045C3E0` (n < 1 → 0, **no
   draw**; else one step, then `lo & (n − 1)` for a power of two, else
   `lo % n`), `pct()` = one step, `lo % 100` (unsigned):

   ```text
   L = levels[level_of(room)]                     // 0x0061A1B0, 0x0061DB70
   for i in 0..4:
       c = L.cmon[i]; if c < 0: return             // first empty slot ends
       if pct(R) >= L.cpct[i]: continue            // drawn for every slot
       m = monstats[c]                              // invalid c → null record
       n = rand(R, m.MaxGrp - m.MinGrp) + m.MinGrp  // +0x30, +0x2F (u8)
       if L.camt[i] != 0: n *= L.camt[i]            // +0xDC + 2i
       repeat n times: place_critter(room, c)
   ```

   `rand(MaxGrp − MinGrp)` gives MinGrp … MaxGrp − 1: MaxGrp itself is
   never reached unless MaxGrp = MinGrp (then n = MinGrp, no draw).
   chicken 3/3 → always 3; rat 4/4 → 4; bat 1/3 → 1 or 2.
3. **place_critter** `0x0046C1A0(room, c)`: (x0, y0, w, h) = the room's
   subtile rect (`0x00619730`); size = `monstats2` `SizeX` (+0x08) of
   c's row (c or its `monstats2` row invalid → nothing). Up to 10 tries:

   ```text
   x = x0 + rand(R, w - 1); y = y0 + rand(R, h - 1)   // x drawn first
   if size_query(room, x, y, size, 0x3F11) == 0:       // 0x0064D9B0
       create_client_unit(class c, x, y, type 1, 0)    // 0x00466730
       return
   // 10 occupied points → this critter is skipped
   ```

   The creation steps one more room seed (the unit seed at a point,
   `client/model.md` §12 r5), so per critter R advances x, y (per try)
   then 1. Critter AI: `client/model.md` §5 rule 6.4.

### 12. Ambient (wandering) spawns (`0x0054F060(game, room)`)

Runs for every active room on every tick (§1.1), populated or not. Draws
use the active room seed:

1. One step. `lo' & 0x7FFF` ≠ 0 → done (a 1/32768 chance to go on).
2. The room's client count (+0x78) ≠ 0 → done.
3. L = levels record of the room's level. `MonWndr` (+0x2E) = 0 → done.
4. `0x0054EFF0`: one step; `lo' mod 100` ≥ 3 → done. region =
   regions[level]. Wanderers spawned (+0x2C4) ≥ 3 → done.
5. `0x0054EF50`: L `Act` ≥ 5 → done. Per-act table at `0x00731B30`, 2
   bytes per act {first, count}. 1.14d: act 0 = {0, 1}, acts 1–4 = {0,
   0}. count = 0 → done. class = list[`roll(count)` + first], from the i32
   list at `0x00731B2C` = {270 rogue2}.
6. Point: §8 with cl null and no warp check. Failure → done. Then
   `0x005B2F20(x, y, class, mode 1, r −1, flags 0)`. Created → 
   `0x005B1990(game, unit, 0, 8)` (insert into the game's target-node
   list 8, the targets of evil monsters; Open question 7)
   and wanderers spawned += 1.

So in 1.14d only Act 1 levels with `MonWndr` get them, and only rogue2
(Align 1, `neverCount`), at most 3 per level per game. That makes the
monster for the levels.txt `MonWndr` column fixed by the binary. No
recording has one (no class 270 appears).

### 13. Region bookkeeping

| Counter | Changed by | When |
|---|---|---|
| rooms visited +0x04 | `0x0054EBC0` | +1 per first population of a room (§3.1) |
| rooms with spawns +0x08 | `0x0054EC90` | §3.4 |
| bosses spawned +0x2C8 | `0x005A0320` +1 (§6.3); `0x00547ED0` −1 | the −1 applies to an evil unit with type flag 1 whose record is dropped (below) |
| evil spawned +0x2CC | `0x00547D90` +1 at creation unless nc; `0x00547DD0` ±1; `0x00547ED0` −1 | below |
| evil killed +0x2D0 | `0x00547E50` +1, `0x00547E90` −1 | below |
| wanderers +0x2C4 | `0x0054EF50` | §12 |

1. `0x00547D90(regions, room, unit, nc)`: nc ≠ 0 → monster flag 2
   (`0x00573570(unit, 2, 1)`, D2MOO "summoner flag 2"). Otherwise
   regions[level of room] +0x2CC += 1.
2. `0x00547DD0(regions, unit, old, new)`, called when an alignment is
   set (`0x00554340`). It applies only to monsters that are not dead
   (`0x005541B0`) and lack flag 2, whose level region exists with Quest =
   0, and only when old ≠ new. old = 0 (evil) and new ∈ {1, 2} → +0x2CC
   −1. new = 0 and old ≠ 4 → +0x2CC +1.
3. `0x00547E50` (from `0x005A6FF0`, death; D2MOO `sub_6FC68240`) and
   `0x00547E90` (from `0x005CC960`; D2MOO `sub_6FC68280`): for monsters
   without flag 2 whose alignment (`0x006259B0`) is 0, they move +0x2D0
   by +1 or −1.
4. `0x00547ED0(regions, lvl A, lvl B, align, dead, keep, unit)` (from
   `0x005424F0`, inactive-unit handling; `sim/units.md`). With a unit: if
   the regions differ, set the unit's level id (`0x00573500`). If keep =
   0 and the unit's alignment is 0: region B +0x2CC −1, and −1 on +0x2C8
   if the unit has type flag 1. Without a unit: if keep = 0, dead = 0
   and align = 0, region A +0x2CC −1.
5. Consumer: the Den of Evil quest `0x00590260` (quests spec) reads
   region 8: remaining = +0x2CC − +0x2D0, and the quest completes when
   rooms visited ≥ room count and killed = spawned.

### 14. Other table-driven and AI spawns

1. monstats `spawn`, `spawnx`, `spawny`, `spawnmode` (+0x20, +0x22,
   +0x23, +0x24) are read in two places. The placespawn swap of §4 reads
   `spawn`. `0x0063EA70` (D2MOO `MONSTERS_GetSpawnMode_XY`) reads all
   four: it returns `spawn` and the point (unit x + `spawnx`, unit y +
   `spawny`), both **signed bytes**. The mode is `spawnmode`, or 1
   (neutral) if it is outside 0–15. Its server caller `0x0056E620` is the
   skill summon resolver (skills spec; called from monster skills such as
   nest summons). These columns are skill/AI data, not population.
2. No monstats column triggers a spawn on death. Death-time spawns come
   from skills, missiles or AI (skills spec, `monsters/ai.md`).
3. AI and skill code reuse population's helpers: `0x0054DC40` is called
   from `0x005E0160`, `0x005E3930`, `0x005E3EA0`, `0x005FCB60` (AI) and
   `0x005B11F0`, `0x005A46E0`, `0x005424F0`; `0x005A43E0` from
   `0x0054E260`, `0x00586520`, `0x005B9930`. Their triggers belong to
   `monsters/ai.md` and the skills spec. The rules of §8–§10 apply
   unchanged.
4. Minion respawn: none in the population code. Revives and resurrects
   are AI and skills.
5. `MonSpcWalk` is read only by AI (D2MOO `AiThink.cpp`; `monsters/ai.md`).
6. Preset-like spawns from missiles (D2MOO `MissMode.cpp`, hit function
   calling the preset spawner with a missile `HitPar` mode) use §11.2;
   see `missiles/missiles.md`.

## Constants & data dependencies

Column meanings owned here (names as in `data/fields.tsv`):

| Table | Column (offset) | Use here |
|---|---|---|
| levels | Act (+0x03) | region act; §12 act gate |
| levels | WarpDist (+0x0C, u32) | §8 squared-distance limit to warps and the kind-11 spawn location |
| levels | MonLvl1/2/3 (+0x10), MonLvl1Ex/2Ex/3Ex (+0x16) | region monster level (§2.2); MonLvl1Ex also in §11.6 |
| levels | MonDen, MonDen(N), MonDen(H) (+0x1C, u32) | density per 100000 per try (§3) |
| levels | MonUMin… (+0x28 + d), MonUMax… (+0x2B + d) | boss limits (§5) |
| levels | MonWndr (+0x2E) | ambient spawns allowed (§12) |
| levels | Quest (+0x30) | region Quest byte; quest levels skip alignment bookkeeping (§13) |
| levels | rangedspawn (+0x31) | first list pick prefers `rangedtype` (§2.3) |
| levels | NumMon (+0x32) | how many classes the region list keeps (≤ 13) |
| levels | mon1–25 (+0x36), nmon1–25 (+0x68), umon1–25 (+0x9A); counts +0x33/+0x34/+0x35 | candidate lists (§2.3, §4, §11.6) |
| monstats | BaseId (+0x02), NextInClass (+0x04) | fallen/scarab override, special footprints, chains |
| monstats | MonStatsEx (+0x18) | monstats2 row |
| monstats | spawn (+0x20), placespawn (flag bit 23) | pick swap (§4) |
| monstats | spawnx, spawny, spawnmode (+0x22–0x24) | not population (§14) |
| monstats | minion1 (+0x26), minion2 (+0x28), PartyMin (+0x2C), PartyMax (+0x2D), SetBoss (bit 4), BossXfer (bit 5) | §10; minion1 also §6.5 |
| monstats | Rarity (+0x2E) | list weight (§2.3) |
| monstats | MinGrp (+0x2F), MaxGrp (+0x30), sparsePopulate (+0x31) | packs (§7) |
| monstats | Align (+0x4C) | creation alignment (§9.6) |
| monstats | isSpawn (bit 0), rangedtype (bit 28), neverCount (bit 18) | §2.3, §9.6 |
| monstats | Level (+0xAA) | §11.6 |
| monstats2 | SizeX (+0x08), spawnCol (+0x0A) | placement (§9) |
| monstats2 | composit counts (+0x15…), total (+0x25), TotalPieces (+0xEC) | appearance variants (§2.4, §2.5) |
| monstats2 | critter (flag 13), objCol (flag 18) | §11.3, §7 |
| superuniques | Class (+0x04), hcIdx (+0x08), MinGrp (+0x1C), MaxGrp (+0x20), AutoPos (+0x24), Stacks (+0x26) | §11.4 |
| monumod | row 0 constants (+0x1C) | champion chance (read by `monsters/init.md`; 20) |

Flag bytes are tested through the byte table at `0x006CE268` (1, 2, 4,
…, 0x80). For example `placespawn` is byte +0x0E & 0x80, and `rangedtype`
and `SetBoss` both use 0x10, on bytes +0x0F and +0x0C.

| Constant | Value | Where |
|---|---|---|
| density modulus | 100000 | §3.2 |
| MonDen clamp | 10000 | §3.1 |
| sub-block size | 3 subtiles | §3.2 |
| tile → subtile | × 5 | `0x00643560` |
| region list max | 13 | §2.3 |
| ranged re-draws | 20 | §2.3 |
| normal pick placespawn chance | 20 | §3.2 |
| boss-type thresholds | ≤ 5 (extra boss), > 35 (unused) | §5 |
| point tries | 20 | §8 |
| ring step | 3 subtiles | §9.3 |
| collision masks | 0x3C01 / 0x1C0 / 0x3F11 / 0 | §9.1 |
| random-unique minions | 3–6 | §6.2 |
| champion minions | 1–3 | §6.4 |
| ambient gate | `lo' & 0x7FFF` = 0, then `lo' mod 100` < 3, max 3 per level | §12 |
| wanderer table | `0x00731B2C` = {270}; act table `0x00731B30` | §12 |

`WarpDist` in 1.14d (patch_d2 `levels.txt`, column `WarpDist`; the
live `levels.bin` agrees for level 15 = 3800, `game_monsters::
real_levels_rows`). 2025 for every row not listed:

| Value | Levels |
|---|---|
| 0 | 0 (Null), 120 Rocky Summit, 132 Worldstone Chamber |
| 100 | 20, 21, 23, 25 (Forgotten Tower, Tower Cellar 1, 3, 5); 47 Sewers 1; 94–99 (Kurast temples) |
| 1000 | 55–61 (Stony Tomb, Halls of the Dead, Claw Viper Temple); 86–91 (Swampy Pit, Flayer Dungeon); 124 Halls of Vaught; 131 Throne of Destruction |
| 3000 | 122 Halls of Anguish |
| 3700 | 101 Durance of Hate 2 |
| 3800 | 15 Hole Level 2; 44 Lost City; 46 Canyon of the Magi; 74 Arcane Sanctuary; 100, 102 Durance of Hate 1, 3; 107 River of Flame; 125–127 (Hell1–3); 134, 135 (Pandemonium Run 2, 3) |
| 3900 | 104 Outer Steppes; 111 Rigid Highlands |

Act I (0): 2025 except 15 (3800) and 20, 21, 23, 25 (100). With 0 no
point is ever rejected (dx² + dy² < 0 never holds).

## Randomness

Draws in order. "Room seed" = active room seed of the room being
populated (room +0x6C) unless a rule says the leader's or boss's room.

**Game creation:** game seed 1 step → monster-region seed. Then, for
levels 1… in order: §2.3 (n draws plus up to 20 ranged re-draws), then
§2.4 for each list entry.

**Per room, in the tick room step** (`sim/tick.md` §4): ambient (§12:
room seed 1 step; if passed, more steps), then presets (§11, each one's
draws below), restore, objects (objects spec), then population.

**Population, per try of each rectangle:**

1. Game seed: density.
2. On a hit: room seed: `roll(total rarity)` (no step if 0); placespawn
   step if the class has it.
3. Room seed: boss type, 1–3 steps (§5).
4. Boss (t = 0):
   1. room seed: unique `roll(count)` (Normal) or rarity pick plus
      optional placespawn (NM/H);
   2. §8 point search: per try, room seed `roll(w)`, `roll(h)`, then the
      probe's ring draws (§9.3: 3 steps for d = 0);
   3. creation at the point: room seed 3 steps (ring d = 0) plus
      allocation (game seed 1 step) plus init draws (`monsters/init.md`);
   4. champion decision on the boss's unit seed (`monsters/init.md`);
   5. minions: boss unit seed count, then per minion the ring draws on the
      seed of the room passed (the boss's room) plus allocation and init.
5. Pack (t = 2):
   1. game seed: sparse step (if `sparsePopulate` ≠ 0);
   2. §8 point search and the leader's creation as in 4.2–4.3;
   3. leader's party (§10: leader unit seed count, then per minion ring
      draws on the leader's room seed plus allocation);
   4. member count on the leader's unit seed, then per member ring draws
      (r 3: up to 3 rings × 4 steps) on the leader's room seed plus
      allocation, init and the member's own party.

Every unit allocation steps the **game seed** (`rng.md` §5.3), between the
density draws. Draw order therefore depends on how many units each hit
creates. Ring draws for a unit placed "near" another use the room that
unit is standing in (`0x00620BB0`), which can be a neighbouring active
room.

## Edge cases & original bugs

1. **A failed pick ends the whole room** (§3.2.3.2). If the region has no
   list, the first density hit stops population of the room, and rooms
   with spawns is not incremented even if earlier tries spawned.
2. **Boss class without a check** (§6.1): an empty umon list in Normal
   gives class 0 (skeleton1), which is then spawned as a boss. 1.14d data
   cannot reach it (level 110 has MonUMin > 0 and no umon but MonDen 0 in
   Normal; level 136 has MonDen 0).
3. **Rarity walk overrun** (§4.3): if every entry has rarity 0, w = 1
   never reaches 0. The walk ends on entry [monster count], a zeroed slot
   (class 0) unless the list is full (13), which reads past the entries
   into MonDen. Classes with rarity 0 in a mixed list are never picked.
4. **Unused third boss-type draw** (§5.3): it always steps the room seed.
5. **Bosses spawned compared as a byte** in §5 (u8 of an i32 counter).
6. **r = 0** in §9 fails without drawing.
7. **Inconsistent ring start direction** (§9.3.4): cells are tested
   twice or never.
8. **0x5B2700 start index** (§9.2): a draw of 0 is turned into 1, so tile
   0 is tested last. The loop stops before s − 1, so tile s − 1 is never
   tested.
9. **Composit total shift** (§2.4.1): `1 << T` with T ≥ 32 uses T & 31.
10. **Variant duplicate counter** (§2.4.5): tries drops once per matching
    old variant, so it can skip past 0 and keep looping until a unique
    variant comes up.
11. **Superunique counts as region boss** (§11.4.3). A level's superunique
    uses up MonUMin/MonUMax slots (e.g., Bishibosh in Cold Plains, Normal
    1/1, when his preset room is populated before the random boss).
12. **Placement probe and creation draw twice**: §8 probes a point (3
    room-seed steps) and the creation repeats the ring (3 more).
13. **Champion with party** (§6.4): champion minions are created with
    flags 0, so a champion fallen shaman's minions each bring a fallen
    party, while the boss itself (flags 0x40) has none.
14. **Region list entries added by §2.5 do not change the monster count**.
    A returned unfilled slot (class 0) is used as the appearance entry.
15. **Tentacle offset set 2** (§10.3) would read unrelated data. This
    caller cannot pass it.
16. D2MOO 1.10f differences confirmed absent in 1.14d: the warp test
    sense (§8), the try-count grouping (§3.2), and the preset fallen swap
    levels (§11.5.4: 1.14d also swaps in levels 7, 12, 16).

## Test vectors

Synthetic (CI-safe, from the rules and `rng.md` §2–§3):

| Input | Expected | Rule |
|---|---|---|
| game seed {1, 666}, MonDen 520, 6 tries | lo' = 1791398751, 791599131, 671516612, 3064641593, 3217527747, 716489901; mod 100000 = 98751, 99131, 16612, 41593, 27747, 89901: no spawn | §3.2.3.1 |
| game seed {12345, 666}, 6 tries | mod 100000 = 52887, 85264, 82871, 88125, 94168, 24880: no spawn | §3.2.3.1 |
| game seed {429, 666}, 1 try | lo' = 4005600443, mod 100000 = 443 ≤ 520: hit | §3.2.3.1 |
| MonDen 520 | hit chance 521/100000 per try | §3.2 |
| MonDen 12000 | stored 10000 at first population | §3.1 |
| rect tiles (0,0)-(8,8) | subtiles 40×40, tries 13 × 13 = 169 | §3.2.2 |
| rect tiles (2,3)-(10,7) | subtiles (10,15)-(50,35): (20/3)·(40/3) = 6·13 = 78 (D2MOO grouping would give 86) | §3.2.2 |
| rect left 0, right 0 | skipped, no draw | §3.2.1 |
| U 0, MonUMin 1, V 2, N 10, room seed draw `lo' mod 100` = 19 | 19 < 20 → boss (0), 1 draw | §5.1 |
| U 0, MonUMin 1, V 2, N 10, draws 20 then 5 | step 1 fails; step 2 (U < MonUMax 1): 5 ≤ 5 → boss, 2 draws | §5.2 |
| U 1, MonUMin 1, MonUMax 1, draw 99 | no draws in 1–2; step 3 draw 99 > 35 → pack, 1 draw | §5.3 |
| room seed {5, 666}: three `mod 100` draws | 99, 73, 5 | helper |
| leader unit seed {1, 666}, MinGrp 1, MaxGrp 2 | `roll(2)` = 1 → 1 member (pack of 2) | §7.7 |
| leader unit seed {2, 666}, MinGrp 1, MaxGrp 2 | `roll(2)` = 0 → no member | §7.7 |
| shaman unit seed {3, 666}, PartyMin 2, PartyMax 6 | 2 + `roll(5)` = 2 + 0 = 2 fallen | §10.1 |
| champion unit seed {3, 666} | `lo' mod 3` = 2 → 3 minions | §6.4 |
| ring d = 3, room seed {1, 666} | parity 1, `roll(3)` = 0, sx 0, sy 1 → offset (3, 0), dir (0, +1); tests (3,1),(3,2),(3,3),(2,3) … (3,−1),(3,0) (24 cells, clockwise, each once) | §9.3 |
| ring d = 3, room seed {42, 666} | parity 0, `roll(3)` = 2, sx 0, sy 0 → offset (2, 3), dir (+1, 0); first tests (3,3), then turns: (2,3),(1,3) … ; last test (3,2) | §9.3 |
| r = −1, seed {7, 666} | 3 steps (parity, `roll(0)` none, sx, sy), one test at (X0, Y0) | §9.3 |
| r = 0 | null, 0 steps | §9.3 |
| region list {zombie1 2, fallen1 2, quillrat1 2}, `roll(6)` = 2 | w = 3: 3−2 = 1, 1−2 = −1 → fallen1 | §4.3 |
| crownest1 pick, placespawn draw 21 | > 20 → foulcrow1 | §4.4 |
| fallen1 pack | min = max = 1: leader only, plus party 2 + `roll(2)` | §7.1, §10 |
| tentacle head, flags 0, k = 4, count 3 | offsets set 1 indices 4, 3, 2 → (1,0), (−2,1), (2,−1) | §10.3 |

Real (1.14d tables, `game/extracted/patch_d2`; `#[ignore]`, needs
`D2_GAME_DIR`):

| Data | Expected | Source |
|---|---|---|
| levels 2 Blood Moor | MonDen 520/520/520; MonUMin/Max Normal 0/0, NM 4/5, Hell 7/9; MonWndr 1; NumMon 3; mon/nmon/umon = zombie1 (5), fallen1 (19), quillrat1 (63) | levels.txt |
| levels 3 Cold Plains | MonDen 520 ×3; U Normal 1/1, NM 4/5, Hell 7/9; NumMon 3; mon = brute1 (28), corruptrogue1 (43), fallenshaman1 (58), cr_lancer1 (165): the region keeps 3 of 4 | levels.txt |
| levels 8 Den of Evil | MonDen 600; U 0/0 all difficulties; Quest 1; MonWndr 0 | levels.txt |
| levels 18/19 Crypt / Mausoleum | MonDen 1056 | levels.txt |
| levels 1 Act 1 town | MonDen 0 → guard stops population (rooms visited still +1) | §3.1 |
| Act 1 WarpDist | 2025 for levels 1–14, 16–19, 22, 24, 26–39; 3800 for 15; 100 for 20, 21, 23, 25 (full table: Constants) | levels.txt |
| monstats Rarity | zombie1 2, fallen1 2, quillrat1 2, brute1 1, cr_lancer1 1, corruptrogue1 2, fallenshaman1 2 → Blood Moor list total 6 | monstats.txt |
| monstats groups | zombie1 1/2, fallen1 2/3 (forced 1/1), quillrat1 1/2, brute1 1/1, fallenshaman1 1/1, cr_lancer1 1/2, corruptrogue1 2/3 | monstats.txt |
| monstats parties | fallen1: minion1 fallen1, Party 2/3, SetBoss, BossXfer; fallenshaman1: fallen1, 2/6, SetBoss | monstats.txt |
| superuniques 0–9 | as listed in §11.4 | superuniques.txt |
| monumod row 0 constants | 20 | monumod.txt |
| placespawn rows | 206–209 crownest1–4 → foulcrow1–4 | monstats.txt |
| sparsePopulate rows | record 528 evilhut (file data row 529, 0-based, counting the `Expansion` separator at row 410) = 40 only | monstats.txt |
| `0x00731B2C` | {270}; `0x00731B30` act bytes {0,1},{0,0}… | Game.exe .data |
| `0x006E2CF0` | 12 offset pairs as §10.3; `0x006E2D50` = (0,−3),(3,0),(0,3),(−3,0) | Game.exe .rdata |

Recordings (tick-raw-1; hold GUIDs, classes and rooms, but no positions
and no RNG):

| Recording | Observation | Checks |
|---|---|---|
| `20261006-015554` | 90 monster inserts, all in `rooms` steps; new GUIDs 12–56 consecutive; restored GUIDs reappear (frames 2088, 2828, 3194, 3583, 3619, 3960, 4151, 4737, 4819) | §1.3 |
| `20261006-015554` f1710 | room …15000: 15–18 fallen1 (leader + 3) | §7.1, §10.2 (party 3 = PartyMax) |
| `20261006-015554` f3311 | room …1d380: 44–50 fallen1 = two groups (4 + 3) | §10 |
| `20261006-022304` f746 | rooms …31280, …31200: 3 fallen1 each (leader + PartyMin 2), populated newest first | §1.3, §10 |
| `20261006-022304` f4175 | room …8b580: 86 fallenshaman1, 87–92 six fallen1 (PartyMax 6), then 93 cr_lancer1 | §10 (party after leader, before next try) |
| `20261006-022304` f4431 | 100 fallenshaman1 + 101–102 (PartyMin 2) | §10 |
| `20261006-022304` f585 | rooms filled in order 3rd, 2nd, 1st activated | §1.3 |
| `20261006-021854` f686 | 33–36 fallen1, 37–38 zombie1, 39–42 fallen1: leader + party per pack; zombie pack of 2 | §7, §10 |
| all three | no class 270 (no wanderer); zombie and quill rat runs come in 1s and 2s, consistent with 1 + `roll(2)` | §7, §12 |

Not checkable from these recordings: densities and tries (no positions or
rectangles), the pick draws (no RNG), boss versus pack (type flags not
recorded; the 4 cr_lancer1 at `022304` f4123 could be two packs or a
unique with 3 minions), and placement points.

## Provenance

- 1.14d `Game.exe` (SHA-256 631066c1…adaaf): every function address
  named in a rule was read in the disassembly (`re/exports/all.asm`),
  with the Ghidra decompiles as a guide (register arguments checked in
  the asm). `0x005A0760` was read only up to the champion decision.
- Tables dumped from the PE image (`.rdata`/`.data`): jump tables
  `0x005B2F04`, `0x00547CB0`/`0x00547CB8`, `0x0054E3D8`/`0x0054E3E0`,
  `0x0054EB74`, `0x0054EB50`/`0x0054EB60`, `0x0054EB0C`/`0x0054EB30`
  (special preset ids, §11.5 rule 6); bit bytes `0x006CE268`; offsets `0x006E2CF0`,
  `0x006E2D50`; wanderer tables `0x00731B2C`/`0x00731B30`.
- D2MOO 1.10f (`MonsterRegion.cpp`, `MonsterChoose.cpp`,
  `MonsterSpawn.cpp`, `MonsterUnique.cpp`, `Monsters.cpp`) was used as a
  map of names only. Each rule was re-read on 1.14d. Differences found:
  try-count grouping (§3.2), warp test sense (§8), fallen/shaman preset
  swap levels (§11.5), and that D2MOO's `SpawnNormalMonster` placement is
  undecompiled there (§9 is 1.14d-only). The dead t = 1 branch exists in
  both.
- Live 1.14d tables (`game/extracted/patch_d2/data/global/excel`): every
  real value in Test vectors, measured with scratch scripts.
- Recordings `traces/raw/20261006-015554-tick.jsonl`,
  `20261006-021854-tick.jsonl`, `20261006-022304-tick.jsonl` (hin/rin/
  ract per step): §1.3 and the group-size rows.

## Open questions

1. Answered (§1 r2): `0x0052D0F0` runs from portal and arrival paths
   outside the tick room step. A recording that logs population calls
   with their step would confirm it on the running game.
2. ~~Draw-level check of §3–§10: record room-seed and game-seed draws
   (call site and `lo'`) during the first population of a Blood Moor room,
   then replay.~~ → PC 2 recording list.
3. Answered (2026-10-07): class 432 (barricadedoor1; raw class test,
   only when the class is below the monstats count) creates object 571
   and class 433 (barricadedoor2; through the bounds helper
   `0x00463900`) object 572 (`0x00555230(type 2, class 0x23B / 0x23C)`
   at the monster's position from `0x0045ADF0` / `0x0045AE20`); both
   objects.txt rows are `Dummy` "door blocker".
4. Answered (2026-10-07): `0x005B24E0(game, boss, class, mode, r,
   count, flags)` (`ret 0x14`; boss none → 0) runs `count` times
   `0x005B23C0(game, boss, class, mode, r, flags)`; each created unit
   gets owner data with the boss GUID (`0x0058F030(game, unit, GUID, 1,
   0, 0)`) and joins the boss's minion list (`0x0058F100`); returns 1
   when at least one was created. So §11.4: Shenk mode 1, r 20, 20
   spawns, flags 0; Nihlathak r 10, 20 spawns, 0x40; Baal subject 2
   r 20, 10 spawns, 0x40.
5. Answered (2026-10-07): the loop tests index s first, then (i + 1)
   mod n until that equals s − 1 (never tested). With n = 1 it tests
   exactly one record, index 1, one past the end of the list (the next
   0x30 bytes in memory; original bug), then stops; index 0 is never
   tested. For n ≥ 2 with s = 1, index 0 is never tested either.
6. Answered (2026-10-07): 4 is not an alignment. The setter
   `0x005543B0(unit, new)` (new ≥ 3 fatal) passes old = 4 when the unit
   has no alignment state list (state 105) yet, else old = stat 172 of
   that list; `0x00554340` then calls `0x00547DD0(old, new)`. So "old ≠
   4" skips the first alignment set, which `0x00547D90` already counted
   at creation.
7. Answered (2026-10-07): not an alignment change. `0x005B1990(game,
   unit, a, slot)` (`ret 8`): only when unit +0xD0 = 11 (in no
   target-node list), slot ∈ {8, 9} and the unit is a player or monster:
   a 0x10-byte node {unit, a, next = old head, prev 0} becomes the head
   of the game's target-node list `slot` (game +0x10F8 + 4·slot; the old
   head's prev := it) and unit +0xD0 := slot. Slot 8 units are targets
   for evil monsters (`monsters/ai.md` §5.2 step 5.2), so §12's rogue2
   wanderers are attacked by the area's monsters. §12 step 6's
   "alignment change" wording is wrong; this answer supersedes it.
   Slot 9 (bone wall): `missiles/bodies-2.md` Open question 2.
8. Answered (2026-10-07): no. `0x005B1CF0` and `0x005B21B0` and their
   callees (2 call levels) contain no generator step (constant
   0x6AC690C5) and no `rng.md` helper call, on any seed; §Randomness
   step 4.3 is complete for them (superunique extra spawns: §11.4).
   Corrected 2026-10-07: wrong for `0x005B1CF0`. Its BaseId 540 case
   (ancientbarb1, and ancientbarb2/3 whose BaseId is 540) creates four
   items three call levels down (`0x005B1C50` → `0x00573B20` →
   `0x00559CE0`), each taking game-seed steps (the item's unit seed and
   item seed) and the item's own rolls (`monsters/init.md` §14.3,
   Randomness step 9). `0x005B21B0` draws nothing (6 levels scanned).
9. Answered (2026-10-08, pc1-s8, from the asm): the four PROVISIONAL
   lines are now 1.14d-confirmed rules: §6.3 step 4 (mode 1, the
   returned room, the search origin), §10.3 step 1 (no `SetBoss` test),
   §11.4 hcIdx 10 (boss unit seed; no owner data), §11.5 rule 6 (the
   default switch entry). REC-80 / REC-81 remain as checks only.
