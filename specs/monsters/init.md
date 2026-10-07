# Spec: Monsters — Creation and initialization

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses given per rule) or from live 1.14d tables;
  the monster-assign messages (0xAC) of two recorded sessions decode
  consistently with it (type flags, umod lists, name seeds, components,
  life 128); no RNG trace of a monster spawn exists yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::init`
- **Related specs:** `sim/rng.md` (generator, helpers, unit seed
  derivation §5.3), `sim/tick.md` (§4 room pass, §5.2 scheduling, §5.6
  handler table), `sim/units.md` (unit allocator, modes, unit flags,
  collision primitives; on branch claude/phase3-units), `sim/stats.md` +
  `sim/stat-lists.md` (stat storage), `sim/unit-order.md` (GUIDs, room
  lists, update queues), `sim/intents-events.md` + `server-messages.tsv`
  (message 0xAC), `monsters/population.md` (who calls creation, with which
  class and position), `monsters/ai.md` (AI state, think scheduling),
  `missiles/missiles.md`, skills spec (skill lists, aura skills), treasure
  spec (TC columns, monequip items), `monsters/umods.tsv` (umod catalogue,
  documented in §19).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 67–85 |
| Inputs | 86–96 |
| Outputs / state changes | 97–121 |
| Rules | 122–123 |
|   1. Entry points | 124–146 |
|   2. The create request | 147–164 |
|   3. Placement | 165–173 |
|   4. Creation sequence after placement (`0x005B2A00`) | 174–232 |
|   5. Monster type init (`0x00574250`) | 233–250 |
|   6. Stats and skills (`0x00573CB0`) | 251–290 |
|   7. Monster level | 291–306 |
|   8. Base values from monlvl | 307–341 |
|   9. Player-count bonus (`0x00573930`) | 342–353 |
|   10. Components (`0x005739D0`) | 354–364 |
|   11. monprop (`monprop.txt`) | 365–373 |
|   12. monequip (`0x005D6B60`) | 374–390 |
|   13. Classic scaling (`0x0063EEF0`) | 391–398 |
|   14. Normal mods and boss mods | 399–500 |
|   15. Party minions | 501–505 |
|   16. Boss spawns | 506–541 |
|   17. Choosing umods (`0x005A0760`) | 542–585 |
|   18. Boss minions and umod init (`0x005A2120`) | 586–603 |
|   19. Umod init functions | 604–689 |
|   20. Superuniques (`0x005A49B0`) | 690–738 |
|   21. Restore paths (`0x005A4440`, `0x005A46E0`) | 739–753 |
|   22. Umod callbacks and the type-7 event | 754–805 |
|   23. Unique names (client) | 806–815 |
|   24. Monster assign message | 816–828 |
|   25. Calling the spawn functions outside population (tools) | 829–919 |
|   26. Making an existing monster unique (`0x005A4940`) and the warping shrine's pick | 920–978 |
|   27. Class reinit (`0x00574370`) | 979–1024 |
| Constants & data dependencies | 1025–1046 |
| Randomness | 1047–1089 |
| Edge cases & original bugs | 1090–1120 |
| Test vectors | 1121–1122 |
|   Synthetic (CI-safe) | 1123–1145 |
|   Real 1.14d values (live tables; `#[ignore]`, `D2_GAME_DIR`) | 1146–1176 |
|   Recorded checks (monster assign 0xAC) | 1177–1189 |
| Provenance | 1190–1268 |
| Open questions | 1269–1325 |
<!-- /index -->

## Summary

Every server-side monster is created by one function, `0x005B2A00`, from
a 0x28-byte create request (class, mode, position, spread, GUID, flags).
It finds a free spot (drawing from the active room seed), allocates the
unit through the generic allocator (which derives the unit seed from the
game seed and runs the monster type init), then applies class-specific
"normal" and "boss" modifiers and spawns the class's party minions.
The type init (`0x00574250`) allocates the AI blocks, rolls the visual
components and the hit points from the new unit seed, computes level,
armor, experience, resistances and skills from `monstats.txt`,
`monlvl.txt` and the area level, applies `monprop.txt` and
`monequip.txt`, and runs the first AI setup. Champions, uniques and
superuniques are created through wrappers (`0x005A09E0`, `0x005A43E0`,
`0x005A49B0`) that pick unique modifiers (`monumod.txt`) from the unit
seed, spawn the boss's minions and run each modifier's init function. A
per-modifier callback table, dispatched on mode changes, hits and the
type-7 timer event, carries the modifiers' later effects.

## Inputs

| Name | Type | Source |
|---|---|---|
| create request | struct, 0x28 bytes (§2) | the caller (§1) |
| game | difficulty (game +0x6D, u8), expansion (game +0x70), game type (game +0x6A, u8), ladder (game +0x74), frame (game +0xA8), boss-spawned bitset (game +0x1D30) | game state |
| active room | room rect, active room seed (room +0x6C) | DRLG (`sim/rng.md` §5.4) |
| monster region | per-level spawn counters and component variant sets | `monsters/population.md` |
| tables | monstats, monstats2, monlvl, levels, monprop, monequip, monumod, superuniques, difficultylevels | `data/loading.md` |
| players-X setting | global `0x00883D70` | host (single player `/players`) |

## Outputs / state changes

A monster unit in the room's unit list (`sim/units.md`) with:

| Monster data field (data at unit +0x14) | D2MOO name | Set by |
|---|---|---|
| +0x00 | pMonstatsTxt | §5 step 3 |
| +0x04 (16 bytes) | nComponent[16] | §10 |
| +0x14 (u16) | wNameSeed | umod 1 (§19), restore (§21) |
| +0x16 (u16) | nTypeFlag: 1 boss-spawned, 2 superunique, 4 champion, 8 unique, 0x10 minion, 0x20 possessed, 0x40 ghostly | §16–§20 |
| +0x1C (9 bytes, 0-terminated) | nMonUmod[] | §14, §17, §18, §20 |
| +0x26 (u16) | wBossHcIdx (superunique row) | §20 |
| +0x28 / +0x2C / +0x30 | pAiControl / pAiParam / pMonInteract | §5 step 2 |
| +0x50 | room coord list ("pVision") | §4 step 3 |
| +0x58 | dwTxtLevelNo (level id of the room) | §5 step 6 |
| +0x5C bit 2 | summoner flag "not counted" | §4 step 2 |

Unit stats (stat ids from `itemstatcost.txt`; storage per `sim/stats.md`):
level 12, monster_playercount 100, the six resistances 36/37/39/41/43/45,
toblock 20, attackrate 68 = 100, velocitypercent 67 = 75, other_animrate
69 = 100, last_sent_hp_pct 352 = 128, maxhp 7 and hitpoints 6 (value ×
256), armorclass 31, experience 13, hpregen 74; modifiers add more (§19).
Unit flags (unit +0xC4): |= 0x0A at init, 0x04 (isAtt), 0x40000000
(petIgnore), 0x20000 (Align 1).

## Rules

### 1. Entry points

All rows end in `0x005B2A00` with a request built as in §2. "near unit"
means: room and x/y taken from that unit.

| 1.14d | D2MOO (1.10f) | Arguments → request | Notes |
|---|---|---|---|
| `0x005B2A00` | D2GAME_SpawnNormalMonster | request pointer in ECX | the creation function (§4; placement `population.md` §9) |
| `0x005B2F20` | D2GAME_SpawnMonster_6FC69F10 | game, room, x, y, class, mode, spread, flags; GUID 0 | 32 callers |
| `0x005B3090` | sub_6FC6A090 | as above, spread −1 | |
| `0x005B30E0` | sub_6FC6A0F0 | game, room, x, y, class, mode, GUID, spread, flags | keeps a given GUID (flag 0x20) |
| `0x005B3040` | sub_6FC6A030 | game, room, coord list, x, y, class, mode, spread, flags | placement limited to the coord list |
| `0x005B23C0` | sub_6FC68D70 | game, unit, class, mode, spread, flags | near unit |
| `0x005B2F70` | sub_6FC69F70 | game, coord list, unit, class, mode, spread, flags | near unit, with coord list |
| `0x005B2490` | sub_6FC6A150 | game, unit, class, … | class remapped for the unit's level (`0x0063EC70`), then `0x005B23C0` |
| `0x005B24E0` | sub_6FC6A230 | owner, class, mode, spread, count, flags | count × near owner; each spawned one becomes the owner's minion (`0x0058F030`, `0x0058F100`) |
| `0x005B31B0` / `0x005B3130` | sub_6FC6A350 family | | N × spawn with owner links / one spawn with owner data |
| `0x005B2570` | sub_6FC6A8C0 | owner, class, mode, count, non-water, flags | tentacle groups |
| `0x005A09E0` | D2GAME_SpawnMonster_6FC6F220 | game, room, coord list, x, y, GUID, class, place flag | boss-capable spawn (`population.md` §6.3) |
| `0x005A43E0` | sub_6FC6E8D0 | game, room, coord list, class, champion allowed, x, y, place flag | unique or champion (§16.1); population `0x0054EC90` passes champion allowed = 1, `0x0054E600` passes 0 |
| `0x005A49B0` | D2GAME_SpawnSuperUnique | game, room, x, y, superunique row | §20 |
| `0x005A4440` / `0x005A46E0` | sub_6FC6FBA0 / sub_6FC6FDC0 | | restore a saved boss / minion (§21) |

### 2. The create request

| Offset | Field | Meaning |
|---|---|---|
| +0x00 | pGame | |
| +0x04 | pRoom | active room |
| +0x08 | pRoomCoordList | 0 = use the room rect |
| +0x0C | nMonsterId | monstats row (class) |
| +0x10 | nAnimMode | initial mode, stored at unit +0x10 before the type init; population uses 1 (neutral) |
| +0x14 | nUnitGUID | used only with flag 0x20 |
| +0x18 / +0x1C | nX / nY | requested subtile position |
| +0x20 | spread | ring search limit (`population.md` §9) |
| +0x24 | nFlags (u16) | below |

Flags: `monsters/population.md` §9.5 (0x01 test only, 0x02 skip normal
mods, 0x04 non-water, 0x08 not counted, 0x20 keep GUID, 0x40 skip party
minions, 0x80 ignore collision).

### 3. Placement

Placement inside `0x005B2A00` (spawnCol masks, the floor-tile path, the
ring search and its active-room-seed draws, the meaning of the spread
and of flags 0x01 and 0x80) is owned by `monsters/population.md` §9; the
flags table is §9.5 there. Facts this spec relies on: placement happens
before allocation, its draws come from the active room seed only, and
flag 0x01 returns after placement without creating a unit.

### 4. Creation sequence after placement (`0x005B2A00`)

Steps 1–4 are listed in `monsters/population.md` §9.6; summary:

1. Allocate with `0x00555230(type 1, class, x, y, game, room, alloc
   flag, mode, GUID)` (`sim/units.md`). The allocator refuses classes
   whose monstats `enabled` bit is clear, derives the unit seed from one
   game-seed step (`sim/rng.md` §5.3), stores the mode at unit +0x10,
   then runs the monster type init `0x00574250` (§5).
2. Region count `0x00547D90`, coord list (monster data +0x50), alignment
   (`0x005543B0`).
3. Unless flag 0x02: normal mods `0x005B21B0` (§14.1).
4. Boss mods `0x005B1CF0` (§14.2).
5. Unless flag 0x40: party minions `0x005B2830` (`monsters/population.md`
   §10; one owner-unit-seed `roll(PartyMax − PartyMin + 1)` when
   `PartyMin` < `PartyMax`).
6. Return the unit.

#### 4.1 Inside the allocator after the type init (`0x00555230`)

Population passes allocator flag 1 or 3 (`population.md` §9.6), so
bit 1 ("add") is set. After `0x00574250` returns, in order:

1. Add to the world `0x00554850(unit, x, y, game, room, 1)`
   (`sim/path-placement.md` §2.5): act fields; dynamic path allocation
   (`0x00649D00`); then the monster branch:
   1. `0x005735A0(game, unit, x, y)`: path speed := monstats
      `Velocity` (+0x32) · 256 (`0x00648690`), animation rate
      `0x00623F50` (`sim/units.md` §4.7), then a mode-change request
      for the unit's current mode (+0x10 = the creation mode;
      `0x005A7E60`) with target point (x, y), run through the mode set
      `0x005A7C20(game, request, 1)` (`sim/units.md` §4.6).
   2. `0x00553160(unit)` ≠ 0 → think restart `0x00573780`
      (`monsters/ai.md` §1.5); else room clean-up `0x00553220`
      (`sim/intents-events.md` §7.5).
2. Hash insert and room queueing (`sim/unit-order.md` §3.1).
3. Path settings when `0x0063EA40` holds and `0x004638A0(class, 0x13)`
   does not (`sim/units.md` §3.1 step 8).

Draws in this sequence (static scan of every direct callee, 6–7 call
levels, for the generator constant 0x6AC690C5 and the `rng.md` helpers;
the indirect calls, mode start and umod callbacks, checked by hand):

| Step | Draws |
|---|---|
| path allocation, path settings, hash/room insert, act lookup | none |
| path speed, animation rate `0x00623F50`, request build `0x005A7E60` | none |
| mode set: path target and compute (`0x005A63F0`, `0x005A6290`) | none (`sim/pathing.md` Randomness 1; the only path type that draws is the charged-bolt missile path `0x0067A240`) |
| mode set: monster mode damage `0x005A4F50(unit, creation mode)` | per element slot i with `El{i}Mode` = the creation mode: U `roll(100)` when `El{i}Pct` < 100, U `roll(5)` for type `rand` (`skills/bodies-2.md` §2.1 step 9) |
| mode set: umod dispatch `0x005A4350` | none: the umod list is still empty (umods are assigned after the allocator returns, §4 steps 3–4) |
| mode set: the creation mode's start function (`sim/units.md` §4.6 table) | mode 1 (`0x005A73E0`) and 12 (`0x005A7390`): none; the mode change's animation re-init `0x00624390` draws only for objects (`world/objects.md`); other modes: that start function's owner |
| think restart `0x00573780` / clean-up `0x00553220` | none (the timer path into `0x005544B0` needs state 54, which a new monster lacks) |

1.14d live data: `El1Mode`–`El3Mode` take only A1, A2, S1 and SC
(monstats.txt: 230 + 53 + 12 rows in slot 1, 19 + 13 + 8 in slot 2,
none in slot 3), never NU (1) or DD (12). So a monster created in mode
1 (population) or 12 draws nothing between the type init and the
allocator's return.

### 5. Monster type init (`0x00574250`)

Called by the allocator with (game, room, unit, GUID):

1. GUID → unit +0x0C; unit flags |= 0x0A; cancel the unit's timers
   (`0x00540F30`).
2. Allocate AI control (`0x0058EBC0`), AI params (`0x005A6490`),
   interaction block (`0x00572BA0`) into monster data +0x28/+0x2C/+0x30.
3. Monster data +0x00 = monstats record.
4. Region data for the class (`0x00547BC0`, `population.md`), then stats
   and skills (§6).
5. First AI setup `0x005B0E00(game, unit, AI control, 0)`
   (`monsters/ai.md` owns what it does and any draws it takes).
6. Monster data +0x58 = level id of the room.
7. If the mode is not 0 (death) and not 12 (dead): attach the quest chain
   record (`0x00545CD0(game, unit, room, 1)`); else set combat mode
   (`0x00553570`). Then `0x005533D0` (`sim/units.md`).

### 6. Stats and skills (`0x00573CB0`)

In order (each "draw" uses the new monster's **unit seed**, unit +0x20):

1. Components (§10). Draws.
2. Class 311 (doomknight2): component 10 = `roll(4)`. Class 312
   (doomknight3): components 10 and 11 = one `roll(4)`.
3. Player-count bonus (§9).
4. Difficulty d = game difficulty clamped to 0..2; d = 0 for hirelings
   (`0x0063EE90`: classes 271, 338, 359, 560, 561).
5. Level (§7).
6. New stat list (callback `0x0055B800`); set level, monster_playercount
   (= player count, at least 1), damageresist `ResDm`, magicresist
   `ResMa`, fireresist `ResFi`, lightresist `ResLi`, coldresist `ResCo`,
   poisonresist `ResPo`, toblock `ToBlock` (each the column for d), then
   attackrate 100, velocitypercent 75, other_animrate 100,
   last_sent_hp_pct 128.
7. Base values (§8) with stats-by-level flags 7 (HP, AC, XP).
8. HP: base = minHP + `roll(maxHP − minHP + 1)` (`0x0045C3E0`; no step if
   maxHP < minHP). hp = base + pct(base, HP bonus, 100) (§8.2); if
   hp ≥ 0x800000 then hp = 0x7FFFFF. maxhp = hitpoints = hp × 256. Draws.
9. armorclass = AC; experience = XP + pct(XP, XP bonus, 100).
10. hpregen = (maxhp × `DamageRegen`) >> 12, where maxhp is the ×256
    value; if maxhp > 0x7FFFFFFF / DamageRegen it is computed as
    (maxhp >> 12) × DamageRegen instead. `DamageRegen` must be 0..0xFFF
    (else fatal error).
11. Classic scaling `0x0063EEF0` (§13; no effect in expansion games).
12. Post an extra stat list (`0x006251F0` + `0x00626E10`, `sim/stat-lists.md`).
13. Inventory: monstats `inventory` set → `interact` set: NPC store
    inventory (`0x00536B20`, unit +0x60); else a new inventory
    (`0x0063ABD0`).
14. Skills: new skill list (unit +0xA8); for i = 1..8 with `Skill<i>` ≥ 0
    and `Sk<i>lvl` > 0: give the skill at level `Sk<i>lvl` + monster
    skill bonus (§9); if `Sk<i>mode` ≥ 0 set that skill's mode.
    `0x0063EBC0` follows (an empty stub in 1.14d).
15. monstats2 `isAtt` → unit flag 0x04; monstats `petIgnore` → unit flag
    0x40000000.
16. monprop (§11). Draws.
17. monequip `0x005D6B60(game, 0, unit, −1, level, level, 1)` (§12). Draws.

### 7. Monster level

1. Normal (d = 0): monstats `Level`.
2. d > 0: monstats `Level(N)` / `Level(H)`, unless the game is an
   expansion game (game +0x70 ≠ 0) and the class has neither `noRatio`
   nor `boss`: then the area level `0x0061DCA0(level id of room, d,
   expansion)` = levels `MonLvl2Ex` / `MonLvl3Ex` (with expansion; `MonLvl2`
   / `MonLvl3` without). Level id ≤ 0, ≥ row count, or d ≥ 3 → 1.
3. Hirelings use d = 0 (§6 step 4), hence `Level`.
4. Modifiers change the level stat later: umod 4 +3, umod 16 −1 (§19).
   Champions end at level + 2, uniques and superuniques at level + 3,
   boss minions at their own level + 3 (umod 4 reaches them, §18).
   The HP, AC and XP rolled in §6 are not recomputed.

1.14d differs from D2MOO only in naming; the rule matches 1.10f.

### 8. Base values from monlvl

#### 8.1 Stats by level (`0x006538A0`)

Arguments: class, L-flag, d, level, flags, out. Level is clamped to
(monlvl rows − 1); a negative level gives nothing. d is clamped to 0..2.
Column offset o = 3 if L-flag else 0, where **L-flag = (game +0x6A ≠ 0)
or (game +0x74 ≠ 0)**; it selects the `L-` columns of monlvl.txt. The
recorded single-player 1.14d game sent game type 3 in its create-game
message (client message 0x67, byte 0x11), so single player uses the
`L-` columns.

| Flag | Output | `noRatio` set | Otherwise |
|---|---|---|---|
| 1 | minHP, maxHP | `MinHP`, `MaxHP` for d | pct(monlvl `HP`/`L-HP` for d, `MinHP`, 100), same for `MaxHP` |
| 2 | AC | `AC` | pct(monlvl `AC`/`L-AC`, `AC`, 100) |
| 4 | XP | `Exp` | pct(monlvl `XP`/`L-XP`, `Exp`, 100) |
| 8 | TH, A1 min/max | `A1TH`, `A1MinD`, `A1MaxD` | TH from `TH`/`L-TH`, damage from `DM`/`L-DM` |
| 0x10, 0x20, 0x40… | A2, S1, El1… | | owned by the combat / skills spec |

monstats values are read as signed 16-bit. "pct(a, b, 100)" is §8.2 with
a = the monlvl value and b = the monstats value.

#### 8.2 Percentage helper (`0x00483360`)

pct(v, p, d) = v × p / d, all signed 32-bit, division truncating toward
zero, with these overflow branches (reproduce exactly):

1. d = 0 → 0.
2. v > 0x100000: if d ≤ v >> 4 → (v / d) × p; else the 64-bit (v × p) / d.
3. else p > 0x10000: if d ≤ p >> 4 → (p / d) × v; else 64-bit.
4. else (v × p, 32-bit wrap) / d.

`0x005A0D20` (HP raise used by umods) has the same results with d = 100.

### 9. Player-count bonus (`0x00573930`)

1. If the class's `Align` = 0: n = players in the game (`0x00535790`); if
   game +0x6A is 1, 2 or 3, n = max(players-X global `0x00883D70`, n);
   n ≥ 1. HP bonus = table `0x006E1590`[n] for n < 9, else (n − 2) × 50;
   XP bonus = table `0x006E15B4`[n] for n < 9, else 10n + 260. Both
   tables are 0, 0, 50, 100, 150, 200, 250, 300, 350 (index 0..8).
2. Else n = 1, bonuses 0.
3. Monster skill bonus = difficultylevels `MonsterSkillBonus` for the game
   difficulty (live 0 / 3 / 7). A difficulty ≥ 3 in game +0x6D is
   overwritten with 2 here.

### 10. Components (`0x005739D0`)

1. If the region data for the class exists and its variant count
   (+0x03) > 0: one `roll(variant count)`; copy that variant's 16 bytes
   (region data +0x04 + 16 × index) to the components.
2. Else for each component i = 0..15: n = monstats2 component choice count
   i (record +0x15 + i, built from `HDv`…`S8v`); component i = inline
   `roll(n)` (no step if n ≤ 0; n = 1 steps and gives 0).

1.14d loops 16 components; D2MOO (1.10f) loops 12.

### 11. monprop (`monprop.txt`)

Record = monstats `MonProp` (none if < 0 or ≥ rows). For i = 1..6 of
difficulty d (`prop<i>`, `prop<i> (N)`, `prop<i> (H)`): stop at the first
property id < 0. If `chance<i>` (for d) is 0, apply; else one inline unit
seed step and apply if lo' % 100 < chance. Apply = item property
assignment on the unit (`0x0065FD70`, property spec) with `par`, `min`,
`max`. None of the recorded Act 1 classes has a MonProp.

### 12. monequip (`0x005D6B60`)

Only for units with an inventory (unit +0x60). Rows are contiguous per
class; the first row index is monstats +0x2A (built at load).

1. If the first row's `oninit` is 0, nothing.
2. Skip rows of this class while row `level` > monster level.
3. For that row and every following row of the same class: count the
   item slots with a location in 1..10 (stop at the first that is not);
   if count > 0: inline `roll(count)` from the unit seed picks slot k;
   if the inventory already holds an item at `loc<k>` skip; else create
   item `item<k>` at `loc<k>` with modifier `mod<k>` (0..7, else 0) at
   item level = monster level (`0x00573B20`, treasure spec).

Live rows: bloodraven (`sbw`, oninit 1), flyingscimitar, shadowmaster,
shadowwarrior, valkyrie (oninit 0 → only via other callers).

### 13. Classic scaling (`0x0063EEF0`)

Only when the game is not an expansion game, d > 0 and `Align` ≠ 1: maxhp
(stat 7), armorclass, experience are replaced by pct(value, a, b) with
(a, b) = NM (1, 2), (10, 12), (10, 17); Hell (1, 2), (10, 12), (10, 26)
(table `0x006EA9F0`); then level = `Level` + 25 × d. Hitpoints (stat 6)
are not scaled (original quirk).

### 14. Normal mods and boss mods

#### 14.1 Normal mods (`0x005B21B0`), by monstats `BaseId`

Assign one umod (`0x005A4850`: append to the list and run its init
function, §19; with "unique" = 1 it also marks the monster unique and
counts it in the region):

| BaseId | Class | Umod (unique arg) |
|---|---|---|
| 24 | brute2 (brute1's base) | 13 rage (0) |
| 91 | scarab1 | 20 scarab (0) |
| 96 | mummy1 | 10 poisondead (0) |
| 211 | duriel | 11 durieldead (0), then 22 questcomplete (1) |
| 326–330 | traps | 14 spcdamage (0) |
| 340–343 | boneprison1–4 | 15 partydead (0) |
| 354 | trap-melee | 14 spcdamage (0) |
| 436 | reanimatedhorde1 | 34 ai_after_death (0) |
| 461 | suicideminion1 | 33 suicideminion_explode (0) |
| 501 | frozenhorror1 | 35 shatter_on_death (0) |
| 540–542 | ancientbarb1–3 | 22 questcomplete (1) |

Recording-confirmed: every brute1 assign message carries umod list [13]
(20261006-022633-packets).

#### 14.2 Boss mods (`0x005B1CF0`)

By `BaseId` (and class for the 1.11+ uber monsters 704–709): quest chain
records (`0x005436B0`), states, flags and umod assignments for the act
bosses and quest monsters. The Act 1 case is bloodraven: umod 12
(bloodraven) and 22 (questcomplete) with unique = 1, quest chain 2, state
corpse_noselect. The full case list matches D2MOO
`MONSTERSPAWN_SetupBossMods` (call pattern checked at `0x005B1CF0`;
every 1.14d case: §14.3).

#### 14.3 Boss mods per case (`0x005B1CF0`, 1.14d)

Request in EAX (game +0x00, room +0x04, class +0x0C), unit in ECX. The
switch key is the class's `BaseId` (monstats +0x02; the class itself
when the row is missing; −1, no case, when the BaseId is out of range).
Some cases test the class itself for the uber monsters (704–709).
"umod n" = `0x005A4850(game, unit, n, 1)` (§14.1) unless marked (0);
"chain n" = `0x005436B0(game, unit, n)`. Cases in code order:

| BaseId | Class test | Effect, in order |
|---|---|---|
| 156 andariel | class 707 (uberandariel) | umod 23 poisonhit, 6 fast, 29 multishot |
| | other | umod 22, chain 6 |
| 175 warriv2 | — | class hook `0x005447A0(game, room, x, y, 175)` with the unit's position (`0x0045ADF0`, `0x0045AE20`); the hook acts only for classes 201 and 331 (`world/quests-act2.md` §10), so nothing happens |
| 211 duriel | class 708 (uberduriel) | umod 6, 18 cold |
| | other | chain 13, chain 9 |
| 229 radament | — | chain 8 |
| 242 mephisto | class 704 (ubermephisto) | umod 22, 30 aura, 17 lightning, 8 resist, 6 |
| | other | chain 20, umod 22 |
| 243 diablo | class 705 (uberdiablo) | umod 22, 8, 6 |
| | other | umod 22, chain 23 |
| 250 summoner | — | chain 12; unit flags (+0xC4) \|= 0x800; monster data +0x5C \|= 1 (`0x00573570(unit, 1, set)`) |
| 256 izual | class 706 (uberizual) | umod 6, 18 |
| | other | umod 22, chain 22 |
| 267 bloodraven | — | umod 12 bloodraven, 22; chain 2; state 118 on (`0x00639DB0(unit, 118, 1)`) |
| 284 maggotqueen1 | — | umod 23, 22 |
| 292 firegolem | — | umod 31 (0) |
| 340–343 boneprison1–4 | — | unit flags \|= 0x20000 |
| 366 compellingorb | — | chain 19; unit flags \|= 0x20000; umod 22 |
| 402 smith | — | umod 22 |
| 407 fetish11 | — | chain 17 |
| 409 hephasto | — | chain 24 |
| 434 prisondoor | — | chain 32 |
| 526 nihlathakboss | — | chain 34, umod 22 |
| 540 ancientbarb1 | — | equipment `0x005B1C50`, below |
| 544 baalcrab | class 709 (uberbaal) | umod 22, 18, 8, 6 |
| | other | chain 36, umod 22 |

No case assigns umods 40 or 41. A null unit skips only the flag writes
(the compellingorb case then still runs umod 22).

**Ancient barbarian equipment** `0x005B1C50` (unit in EBX): game =
`0x00554010(unit)`, L = the unit's `level(12)` (`0x00625480(unit, 12,
0)`). For k = 0..3: entry = table `0x006E1BB0` + 8 · class + 0x18 · k
(4-byte item code, then a body-location byte; the key is the class, so
classes 540–542 read their own rows):

| k | 540 | 541 | 542 |
|---|---|---|---|
| 0 | `bsd`, loc 4 | `tax`, loc 4 | `vou`, loc 4 |
| 1 | `tow`, loc 5 | `tax`, loc 5 | `rin`, loc 6 |
| 2 | `fld`, loc 3 | `hgl`, loc 10 | `fld`, loc 3 |
| 3 | `hbt`, loc 9 | `hbt`, loc 9 | `crn`, loc 1 |

Difficulty (game +0x6D) 1 replaces the code by the item's `ubercode`
(+0x88 of its items row, found with `0x00633640`), 2 by `ultracode`
(+0x8C). Then `0x00573B20(game, unit, &entry, L, 4)`, the monster equip
helper also used by monequip (§12): no inventory → one is created
(`0x0063ABD0`); the code is looked up (unknown → nothing); the item is
created by `0x00559CE0` (`items/generation.md` §10.2: source = the
monster, spawn mode 4, quality 4 (magic), no sockets, never ethereal,
ilvl = L, no seeds); item flag 0x1000 set (`0x006280D0`); body location
:= the entry byte (`0x00627D70`); location ≠ 0 → placed with
`0x005606B0`: failure → the item is removed (`0x00555600`), success →
stat 72 (durability) := its maximum (`0x00625E00`). Each creation takes
game-seed and item-seed draws (Randomness step 9).

### 15. Party minions

Owned by `monsters/population.md` §10 (`0x005B2830`, `minion1`/`minion2`,
`PartyMin`/`PartyMax`, `SetBoss`, `BossXfer`, tentacle heads).

### 16. Boss spawns

The boss spawn `0x005A09E0` (placement retries, unique mark
`0x005A0320`: type flag 8 and the region boss counter, type flag 1, quest
hook, own boss data) is `monsters/population.md` §6.3; the random-boss
call `0x005A43E0` is §6.2 there. Every boss created through `0x005A09E0`,
champions included, carries type flags 1 | 8. This spec owns what
follows the spawn:

#### 16.1 Random boss (`0x005A43E0`)

1. `0x005A09E0`. Fail → 0.
2. Choose umods `0x005A0760(unit, game, champion allowed)` (§17).
3. Minions and umod init `0x005A2120(min 3, max 6, coord list, game,
   unit, spawn minions 1)` (§18).

#### 16.2 Champion pack members (`0x005A48C0(game, unit, umod)`)

Population spawns 1–3 extra monsters around a champion
(`monsters/population.md` §6.4) and calls this with umod 16 on each:

1. Nothing if the unit already has type flag 4.
2. Mark unique (`0x005A0320`: type flag 8, region boss counter), type
   flags |= 1 | 4.
3. If it has fewer than 9 umods, append the umod (after any normal mods,
   e.g. brute1 → [13, 16]).
4. `0x005A2120(min 0, no list, max 0, game, unit, 1)`: the champion flag
   makes the minion step a no-op; umods 1–4 and the list run with
   unique = 1 (§18 step 2). So each pack member draws its own name seed
   and gets champion HP, level and damage, but always umod 16, never the
   leader's 36–39.

Recording-confirmed: a ghostly champion fallenshaman1 (umods [36]) next
to a fallenshaman1 with [16] (20261006-015956, frame 3649); three brute1
with [13, 16] (20261006-022633, frame 678).

### 17. Choosing umods (`0x005A0760`)

Nothing if the difficulty is ≥ 3, the unit has no monster data, or it
already has ≥ 8 umods.

1. If champion allowed: chance = `constants` of monumod row 0 (live 20;
   0 if the table is empty). `roll(100)` from the unit seed; if < chance:
   type flag |= 4 (champion), pick one champion umod (17.1), append it,
   return.
2. Count = `roll(1)` (always steps, gives 0) + 1 + difficulty; if count +
   existing ≥ 9, count = 9 − existing.
3. Used set = the existing umods. Repeat count times: pick a unique umod
   (17.2); stop at 0; append it and mark it used.

#### 17.1 Champion pick (`0x005A0500`)

Candidates in row order: `cpick` (for d) > 0, `champion` ≠ 0, eligible
(17.3). Used umods are not excluded. Total = sum of `cpick`; `roll(total)`
(`0x0045C390`) from the unit seed; walk the candidates subtracting
weights; return the first whose weight exceeds the remainder; none → 0.
Live candidates: 16 champion, 36 ghostly, 37 fanatic, 38 possessed,
39 berserk, weight 1 each; 36–39 have `version` 100 (expansion only).

#### 17.2 Unique pick (`0x005A0600`)

Same with `upick`, `champion` = 0, and not in the used set. The draw is
an inline `roll(total)`; total ≤ 0 → no step, result 0.

#### 17.3 Eligibility (`0x005A03E0`)

1. `enabled` = 0 → no. Not expansion and `version` ≥ 100 → no.
2. `exclude1`, `exclude2` (> 0): `0x005A0070(unit, exclude)` set → no.
   It reads the montype equivalence matrix (`data/runtime-maps.md` §2)
   at row = the exclude type, column = the class's monstats `MonType`
   (+0x1C, i16): set when the exclude type **is the class's MonType or
   a sub-type of it** (MonType reachable from the exclude type by its
   `equiv` links). Unit null or not a monster, class outside 0 … rows
   − 1 of monstats, or MonType / exclude outside 0 … montype rows − 1 →
   not set. The provider answers from the matrix itself (same bits, so
   the walk's depth limit and bad links come out as the matrix has
   them).
3. `fPick` 1: class must have mode A1 in monstats2; 2: no if `isMelee` or
   `nomultishot`; 3: class must have mode WL. Else yes.

### 18. Boss minions and umod init (`0x005A2120`)

Arguments: min (ECX), coord list (EDX), max (EAX), game, unit, spawn
minions.

1. If spawn minions: `0x005A0C00` (`monsters/population.md` §6.5: not for
   champions; class `minion1` or own class; count = min + boss
   unit-seed `roll(max − min + 1)`; each spawned with spread 3, flags
   0x40). For each minion this spec owns the umod transfer `0x005A0930`:
   the boss's umods whose `xfer` = 1 are appended to the minion's list;
   the slot index counts every boss umod visited, so at most 9 − (the
   minion's own count) boss umods are visited. Then owner data, minion
   list, owner GUID, type flag |= 0x10.
2. For umod in 1, 2, 3, 4 (table `0x006E2168`), then each entry of the
   unit's umod list in order (stop at 0, at most 9): run the umod's init
   function (§19) on the unit with unique = 1, then on every unit in its
   minion list with unique = 0 (`0x0058F380` → `0x005A2100`).

### 19. Umod init functions

Table `0x0073C008`, 43 entries (ids 0–42), called as (unit, umod,
unique). Catalogue with every id, its init function and its callbacks:
`monsters/umods.tsv` (columns: `id`; `uniquemod` name; `init_fn` 1.14d
address or `-`; `unique_gate` yes = does nothing when unique = 0, no =
ignores it, branch = different constants; `init_effect`; `cb_mode0`…
`cb_mode5` callback addresses of §22; `status`). K[i] below is monumod row
i `constants` (live: 20, 100, 75, 50, 200, 150, 100, 300, 200, 100, 75,
100, 50, 100, 75, 150, 0, 33, 33, 0, 50, 50, 33, 33, 33, 50, 50, 50, 66,
66, 66, 100, 100, 100); B = difficultylevels `ChampionDamageBonus` (90 /
75 / 66). Divisions truncate toward zero.

#### 19.1 Fixed mods

| Umod | Effect |
|---|---|
| 1 | unique only: one inline unit-seed step; name seed = low 16 bits of lo' |
| 2 | hp' = hp + pct(hp, K, 100) on the ×256 max life, written to maxhp and hitpoints; K = K[d+4] if champion, K[d+7] if unique, K[d+1] if called with unique = 0 (minions); unique = 1 also sets hpregen = 0 |
| 4 | level += 3; experience = 5 × base experience |

#### 19.2 Champion function (umod 16, also called by 36, 37, 38)

Unique only. level −= 1; experience = e − 2 × (e / 5) with e = base
experience; damagepercent += K[11] × B / 100; item_tohit_percent +=
K[10] × B / 100; if the class's BaseId is 118 (willowisp1) the damage
value is halved (signed / 2) before adding. Then, if monstats `Velocity`
> 0 and the umod is not 36: umod 37 → velocitypercent += clamp(2048 /
Velocity − 128, 10, 100); others → +20.

Live Normal: damagepercent +90, item_tohit_percent +67.

#### 19.3 Resistances (`0x005A1370`; umods 8, 27, 28 and the tail of 9, 17, 18, 23, 25)

Unique only. Umod 28 first doubles armorclass. Count immunities
(resistances ≥ 100 among fire, light, cold, poison, damage, magic). If
fewer than 2:

| Umod | Change |
|---|---|
| 8 | cold +40 if < 100 (count it if it reaches 100); if still < 2: fire +40 if < 100 (count); if still < 2: light +40 if < 100 |
| 9 / 17 / 18 / 23 | fire / light / cold / poison +75 |
| 25 | magic +20 |
| 27 | as 8 with +20 and the test "< 75" |
| 28 | damage +50 |

#### 19.4 Damage modifiers

| Umod | Effect |
|---|---|
| 5 strong | damagepercent += K[15] (unique) or K[14] × B / 100, halved for BaseId 118; item_tohit_percent += K[13] or K[12] × B / 100 |
| 6 fast | velocitypercent += clamp(2048 / `Velocity` − 128, 10, 100) if `Velocity` > 0; any unique value |
| 9 fire | DM = monlvl `DM`/`L-DM` (L-flag of §8.1) for d at the level clamped to 1..rows−1; firemindam += DM × K[d+28] / 100, firemaxdam += DM × K[d+31] / 100 (unique) or K[d+16], K[d+19] (unique = 0); then 19.3 |
| 17, 18, 23, 25 | same body for light, cold (+ coldlength 5 × r + 100), poison (+ poisonlength 2 × (5 × r + 150)), mana drain (× 256), r = the clamped monlvl row; then 19.3 (bodies of 9, 17, 18, 23, 25: `monsters/umod-init-bodies.md` §2–§3) |

#### 19.5 Aura enchanted (umod 30, `0x005A1650`)

Unique only. lvl = level stat (≥ 1). Aura table `0x0073BF68` (min level,
level offset, multiplier, divisor, skill): (0,0,1,6, might 98),
(0,0,1,6, holyfire 102), (0,0,1,5, blessedaim 108), (0,0,1,7,
holyfreeze 114), (0,0,1,8, conviction 123), (0,0,1,8, fanaticism 122),
(20,0,1,8, holyshock 118), (999,0,0,1, thorns 103). n = rows with min
level ≤ lvl (at least 1). A **temporary seed** {name seed, 666} (name
seed 0x1506 without monster data) gives `roll(n)`; the unit seed is not
touched. Superunique row 37 (Lord De Seis) forces index 5. Class 704
instead gets conviction level 20. Skill level = clamp((lvl + offset) ×
multiplier / divisor, 1, 99); the skill is given and assigned
(`0x0056DEB0`, `0x005701B0`, skills spec).

#### 19.6 Champion types and others

| Umod | Effect |
|---|---|
| 36 ghostly | type flag 0x40; damageresist = 80; champion function; at the new level: coldmindam += DM × K[d+22] / 100, coldmaxdam += DM × K[d+25] / 100, coldlength += 150 |
| 37 fanatic | item_armor_percent = −70; champion function (velocity rule of 37) |
| 38 possessed | type flag 0x20; maxhp and hitpoints += 100 %; champion function |
| 39 berserk | maxhp and hitpoints += pct(maxhp, −75); damagepercent += 300 × B / 100 (halved for BaseId 118); item_tohit_percent += 300 × B / 100; no champion function |
| 26 teleport | unique only: skill MonTeleport (184) level 1, skill mode 4, AI flag 0x20 (`monsters/umod-init-bodies.md` §4) |
| 41 always_run_ai | schedule a type-7 event at frame + 75 (`0x005417D0`), any unique value |

#### 19.7 Bodies owned elsewhere

The full step lists of umods 9, 17, 18, 23, 25 (elemental body) and 26
(teleport) are in `monsters/umod-init-bodies.md`; §19.4 and §19.6 keep
the summary.

### 20. Superuniques (`0x005A49B0`)

Checks, `AutoPos`, the spawn, the boss bitset, type flag 2, the minion
group (`MinGrp`/`MaxGrp` + difficulty when both are non-zero) and the
per-`hcIdx` extra spawns are `monsters/population.md` §11.4. Owned here:

1. Monster data +0x26 = superunique row (`0x005A0200`).
2. If the unit has fewer than 5 umods: append `Mod1`..`Mod3` in order,
   stopping at the first 0, skipping 24 (thief); remember whether 30
   (aura) was one. Then difficulty times: unique pick (17.2) with the
   used set; stop at 0.
3. `0x005A2120(min, no list, max, game, unit, 1)` (§18): minions, then
   umods 1–4 and the list with unique = 1.
4. If aura was in Mod1–3: umod 30 runs again (`0x005A1650`); the second
   run picks the same aura (same name seed).
5. Quest records by `hcIdx` (quests spec); for the Countess row (hcIdx 6)
   also state 118 and the first AI install `0x005B0E00(game, unit, AI
   control, 13)` (special AI state; `monsters/ai.md` §3.3). Every path
   ends with umod 22 questcomplete, unique = 1 (`0x005A4850`).

The `TC`, `TC(N)`, `TC(H)` columns are read when the boss dies (treasure
spec); `Utrans` columns are client colour. `MonSound`, `EClass`,
`Replaceable` are not read here.

#### 20.1 Per-`hcIdx` cases (`0x005A49B0`, 1.14d)

After §20 step 4, a switch on the row's `hcIdx` (+0x08; −1 for a
missing row), tables `0x005A4EA4` / `0x005A4E7C` on hcIdx − 6. Every
case ends with umod 22, unique = 1. "spawn(c, r, n, f)" =
`0x005B24E0(game, boss, c, mode 1, r, n, f)` (`population.md` Open
question 4); "chain n" as in §14.3; `0x00545B50(game, unit)` is the
quest preset-boss hook (`world/quests-act3.md`, `world/quests-act5.md`).

| hcIdx | Rows (1.14d) | Effect before umod 22 |
|---|---|---|
| 6 | The Countess | state 118 on; chain 5; AI install `0x005B0E00(game, unit, AI control, 13)` |
| 10 | Radament | U `roll(5)` + 2 times `0x005B23C0(game, unit, class 4, mode 1, r 4, flags 0x40)`; then one each of classes 276, 382, 385, 389 the same way |
| 26, 27, 29 | Ismail Vilehand, Geleb Flamefinger, Toorc Icefist | chain 19; `0x00545B50` |
| 36, 37, 38 | Infector of Souls, Lord De Seis, Grand Vizier of Chaos | chain 23 |
| 39 | The Cow King | chain 4 |
| 42 | Siege Boss | spawn(453, 20, 20, 0); chain 31; `0x00545B50`; state 118 on |
| 43, 44, 45 | Ancient Barbarian 1–3 | chain 35; `0x00545B50` |
| 60 | Nihlathak Boss | owner data `0x0058F030(game, unit, own GUID, 1, 1, 0)`; spawn(class-for-level(the unit's room, 453) by `0x0063EC70`, 10, 20, 0x40); chain 34 |
| 62 | Baal Subject 2 | spawn(381, 20, 10, 0x40) |
| any other | — | nothing |

Draws: only Radament's `roll(5)` (unit seed) and the creations of the
spawned monsters (their own §4 draws).

### 21. Restore paths (`0x005A4440`, `0x005A46E0`)

Called from the inactive-unit restore `0x005424F0` (room reactivation,
`sim/units.md`) with saved class, GUID, umods and name seed. The monster
is created anew (new unit seed from the game seed, new components and HP
rolls, §6), then the saved umods are copied in and applied:

- boss (`0x005A4440`): `0x005A09E0` with the GUID; champion flag from the
  saved state; umods copied; `0x005A2120(…, spawn minions 0)`; name seed
  = saved value (after umod 1 drew a new one); umod 30 again if present;
  superunique flag, hcIdx and quest records as in §20.
- minion (`0x005A46E0`): `0x005B30E0` with the GUID, flags 0x62 (retries
  as `population.md` §6.3 step 4); umods copied; type flag |= 0x10; umods 1–4 and the
  list run with unique = 0.

### 22. Umod callbacks and the type-7 event

`0x005A4270(game, unit, arg, mode)`: if the monster's umod list is
non-empty, for each of the 9 slots (zero slots included, they hit row 0
which is empty) call callback table `0x0073C0B8`[umod × 6 + mode] if
non-null, with (game, unit, umod, unique) where unique = type flag 8.
Mode 5 passes `arg` instead of the unit. Callers:

| Mode | Wrapper | Called from |
|---|---|---|
| 0 | `0x005A4350` | monster mode change `0x005A7C20` |
| 1 | `0x005A4360` | monster mode change `0x005A7C20` (second site) |
| 2 | `0x005A4370` | monster timer type 7 (`sim/tick.md` §5.6) |
| 3 | `0x005A4390` | `0x0057C6C0` (combat) |
| 4 | `0x005A43A0` | `0x0057CEE0` (combat, two sites) |
| 5 | `0x005A43B0` | `0x0059FA30` (missiles) |

`0x005A4370` is not in the Ghidra export; its 23 bytes at that address
call `0x005A4270(game, unit, 0, 2)`. Mode-1 callbacks schedule the
type-7 event: 9 fire (`0x005A25F0`: unique and new mode 0 → frame + 4),
17 lightning (`0x005A37D0`: unique and new mode 3 → frame + 2), and
`0x005A3800` for 10, 18, 31, 32, 42 (new mode 0, and for 18 only when
unique → frame + 4), and 33 suicideminion_explode (`0x005A3E70`,
table entry 199 = 33 × 6 + 1: new mode 0 or 12 → frame + 4; new mode 3
(GH) first sets mode 0, whose own mode-1 pass schedules one, then
schedules a second at frame + 4; `umod-callbacks.md` §22.1). The only
type-7 event scheduled at init is umod 41's
(frame + 75); its handler `0x005A4230` checks the monster is alive
(`0x005541B0`), runs `0x00573780` and re-schedules at frame + 75.

Umod 34 (ai_after_death; reanimatedhorde1–5 get it from §14.1) mode-1
callback `0x005A3840`: when the new mode is 0 (death) and `0x006259B0`
is false: cancel the unit's pending type-7 events (`0x00540E60`), one
inline unit-seed step, and if lo' % 100 < monstats `aip8` (for d) schedule
a type-7 event at frame + 10 × `aip1` (for d) + 1; its mode-2 handler
`0x005A3910` brings the monster back (`monsters/ai.md`).

Missile hook (mode 5, `0x005A43B0(game, unit, missile)`): called by the
missile creation code `0x0059FA30` only for monster owners and a non-null
missile (gating by `NoMultiShot` / `NoUniqueMod`: `missiles/missiles.md`
rule 28). Callbacks: 19 hireable `0x005A2E00`, 27 spectralhit
`0x005A30B0`, 29 multishot `0x005A3610`.

What the remaining callbacks do (death explosions, curses, hit effects)
is outside init (open question 8); addresses in `umods.tsv`.

Callback bodies and the exact places where the dispatcher runs (mode 0
before the start function and never for GH, mode 1 after it and before
the animation schedule, mode 4 on the defender in the reaction) are
owned by `monsters/umod-callbacks.md`; the short forms above summarise
its §4, §6.1, §10.1, §23 and §26.

### 23. Unique names (client)

The server stores only the 16-bit name seed. The client (`0x004AC870`)
sets its own unit seed to {name seed, 666} and, for monsters without a
fixed name, draws in this order: suffix `roll(uniquesuffix rows)`
(`0x00653F10`), prefix `roll(uniqueprefix rows)` (`0x00653ED0`), then
`roll(100)`; if < 50: appellation `roll(uniqueappellation rows)`
(`0x00653F50`), suffix again, prefix again (the second pair replaces the
first). Superuniques use their `Name`. Display only; no sim effect.

### 24. Monster assign message

Creation sends nothing. Message 0xAC (`server-messages.tsv`, built by
`0x0053E2E0`) goes out with the unit update queues (`sim/unit-order.md`
§6). Init-owned fields in it: class, life byte (last_sent_hp_pct, 128
at spawn), mode (only skill1/skill2/death/dead modes are sent, else 1),
the 16 components (1 bit when the class's choice count is < 3, else
bit length of count − 1; omitted when all are 0), and, when the monster
has umods: type flags champion 4, unique 8, superunique 2, minion 0x10,
ghostly 0x40 (one bit each, in that order), hcIdx (16 bits, superunique
only), umods (8 bits each, 0-terminated), name seed (16 bits). Bits are
written low bit first.

### 25. Calling the spawn functions outside population (tools)

For a recorder that injects a spawn into the original
(`tools/original-hooks.md`, scenario spawn steps): register conventions
read from the 1.14d disassembly, the effects of each call, and what
differs from a population-made unit. Behavior is owned by §16–§18 and
`monsters/population.md` §6–§10; this section adds no rule.

#### 25.1 Conventions

All are callee-cleans-stack; "stack" lists arguments in push-reverse
order (first = lowest address, `[esp+4]` at entry). Return in EAX.

| 1.14d | Registers | Stack | Pops | Returns |
|---|---|---|---|---|
| `0x005B2A00` creation | ECX = request (§2, 0x28 bytes, caller-owned) | — | 0 | unit, 0, or 1 for a probe (flag 0x01) |
| `0x005A09E0` boss spawn | EDI = game, EBX = class | room, cl, x, y, GUID, warp check | 0x18 | unit or 0 |
| `0x005A43E0` random boss | ECX = game, EDX = room | cl, class, champion allowed, x (low 16 bits used), y (low 16 bits used), warp check | 0x18 | boss or 0 |
| `0x005A0760` choose umods | ECX = unit | game, champion allowed | 8 | — |
| `0x005A2120` minions + umod init | ECX = min, EDX = cl, EAX = max | game, unit, spawn minions | 0xC | — |
| `0x005A48C0` champion pack member | ECX = game, EDX = unit | umod (low byte) | 4 | — |
| `0x0054E1E0` champion minions | ESI = boss, EDI = game | cl, class | 8 | — |

`0x005A09E0` takes the game in EDI and the class in EBX (not ECX /
EDX); its callers load them before the call (`0x005A43E0` at
`0x005A43F4`, `0x005A4400`). x = y = 0 asks for a searched point
(`monsters/population.md` §6.3 step 1); cl may be 0 (room box,
§6.3 steps 3–4); GUID −1 for a new unit. The population call of
`0x005A43E0` is (game, room, cl, class, 1, 0, 0, 1)
(`0x0054EF13`–`0x0054EF23`), then `0x0054E1E0(cl, class)` with ESI =
the boss (`0x0054EF2C`–`0x0054EF36`).

**Plain spawn** (`scenario.md` §3.1 rule 2: room, no cl, class, mode 1,
x, y, r −1, flags 0): fill the request (§2) with game, room, cl 0,
class, mode 1, GUID 0, x, y, r −1, flags 0 and call `0x005B2A00`.
r = −1 tests only (x, y) itself (`monsters/population.md` §9.3); flags 0
spawns the class's party (§10 there). Room for a sub-tile point: walk
the act's active room list (act = game +0xBC + 4·act, head act +0x10,
next active room +0x7C, `sim/unit-order.md` §4) and take the first room
whose sub-tile box (active room +0x4C: x, y, w, h) holds the point,
half-open (`drlg/levels.md` §8 rule 2); or, from a known room (the
player's), `0x00463740(ECX = room, EDX = x, stack y; pops 4)`, which
searches that room and its adjacency array (`drlg/rooms.md` §6).

#### 25.2 Effects and differences from room population

1. Draws: active room seed of the room passed (placement, search,
   minion placement), the game seed (one step per allocated unit; GUIDs
   come from a counter, `sim/unit-order.md` §1), unit seeds (umod choice §17, minion counts, umod init
   §19, champion minion count), and the level seed only through the
   kind-11 query (x = y = 0 with warp check 1, `drlg/levels.md` §11.5
   item 4). All are game-owned: the unit seed's clock fallback
   (`sim/rng.md` §5.3) runs only without a parent seed, and allocation
   always passes the game seed.
2. Region counters touched exactly as for a population-made unit: the
   spawned count +0x2CC (`0x00547D90`, §4 step 2), and for every
   `0x005A09E0` boss and `0x005A48C0` member the boss count +0x2C8 of
   the region of the unit's level (`0x005A0320`). So a tool boss lowers
   the later random-boss chances of its level (`monsters/population.md`
   §5 steps 1–2).
3. Not touched (they belong to `0x0054EC90`): rooms visited (+0x04),
   rooms with spawns (+0x08), the region room count (+0x0C), the room's
   spawned flag and the active room's populated bit (+0x34 bit 0). A
   room not yet populated is still populated later on top of the tool's
   units.
4. Order of a population random boss: `0x005A43E0` (= `0x005A09E0`,
   `0x005A0760`, `0x005A2120` with min 3, max 6, spawn minions 1), then
   `0x0054E1E0` (champions only: 1–3 members, each `0x005A48C0` with
   umod 16). A tool that calls them in this order with population's
   arguments leaves the state population would, except item 3.

#### 25.3 Explicit umods

1. List: monster data (unit +0x14) +0x1C, 9 bytes. Count = bytes
   before the first 0, at most 9 (`0x005A0260`); with 9 entries there
   is no terminator.
2. `0x005A0760` writes only this list and, on the champion branch, type
   flag 4 (monster data +0x16). To give a boss scripted umods: spawn
   with `0x005A09E0`; append each umod id as a byte at index count
   while count < 9 (umod ids 1–42, `monsters/umods.tsv`); for a
   champion also OR 4 into +0x16; then `0x005A2120(min 3, cl, max 6,
   game, unit, 1)` (min 0, max 0 gives no minions); for a champion then
   `0x0054E1E0(cl, class)`.
3. Skipping `0x005A0760` skips its unit-seed draws (the champion
   `roll(100)`, the count `roll(1)`, the picks of §17.1–§17.2), so every
   later unit-seed draw of that boss (minion count, umod init §19, name
   seed) differs from a population boss with the same umods. A tool
   that must match a population boss calls `0x005A0760` instead.
4. With type flag 4, `0x005A2120` spawns no minions (§18 step 1) and
   runs the umod inits with unique = 1, as for a population champion.

### 26. Making an existing monster unique (`0x005A4940`) and the warping shrine's pick

Used by the shrine effect 20 `0x00583050` (`world/objects.md` §9.2);
no other caller (xref).

**Make unique** `0x005A4940(ECX game, EDX unit)`: unit none, not a
monster or without monster data → nothing. Then, in order:

1. Type flags (monster data +0x16) |= 1.
2. Unique mark `0x005A0320(unit, game)` (`population.md` §6.3 step 5:
   type flag 8 and the region boss counter when not yet set).
3. Choose umods `0x005A0760(unit, game, champion allowed 1)` (§17):
   the champion test runs, so the result is a champion in `constants`
   % of cases (live 20).
4. `0x005A2120(min 0, cl none, max 0, game, unit, spawn minions 0)`
   (§18): no minions; umods 1–4 and the list run with unique = 1.
5. Unit flags (+0xC4) |= 0x800; monster data +0x5C |= 1
   (`0x00573570(unit, 1, set)`), as for the Summoner (§14.3).

Draws: those of §17 and of the umod init functions (Randomness step
11 without minions), on the unit's seed.

**Nearest eligible monster** `0x0065A800(P, x, y, limit, cb)` (`ret
0x14`), called with P = the shrine's operator, (x, y) = P's position
(`0x00648900` / `0x006488C0` from the dynamic path for a player; the
static path's +0x10 / +0x0C for unit types 2, 4, 5), limit 0, cb
`0x00582750`:

1. P's room (`0x00620BB0`); none → fatal assertion. Its room list
   (`0x00619790`, the adjacency array, in order).
2. limit 0 → 0x10000. best := 0xFFFF, M := none. cb must be a valid
   code pointer (`IsBadCodePtr`), else fatal.
3. Per room passing the overlap test `0x0065A710(room, x, y, limit)`
   (`umod-callbacks.md` §3.1 step 3: it never rejects), per unit U of the
   room's list (+0x74, next +0xE8) in order: d := `0x006417F0(U, x, y)`;
   d < limit, d < best and cb(ECX U, EDX P) ≠ 0 → M := U, best := d. The
   first of equal distances wins.
4. Return M. The shrine then runs make unique on M when found.

**Eligibility** `0x00582750(M, P)` is 1 only when all hold, tested in
this order:

1. M ≠ P, M exists and is a monster (type 1).
2. `0x00650D70(P, M)` ≠ 1 (the alignment test between them).
3. M's alignment (`0x006259B0`) = 0 (evil).
4. `0x0063EA40(M)` = 0.
5. M's mode is 1 (NU) or 2 (WL).
6. M's class has monstats2 mode bit 2 (`0x0046C140(class, 2)`: the
   +0xF0 mode bits through monstats +0x18, `render/unit-composite.md`
   §1.1).
7. M's monstats2 record exists (`0x00451FE0`) and its byte +0x0B ≠ 0.
8. v := `0x0055B7E0(M)` (monster data +0x14, dword 0) ≠ 0 and
   `0x0063E9F0(v, M)` = 0 (not a boss).
9. `0x0063EDC0(M)` = 0 (not a prime evil).
10. `0x005A0180(M, 0x1F)` = 0: none of type flags 1, 2, 4, 8, 0x10
    (already unique, superunique, champion, boss or minion).

The search and the test draw nothing.

### 27. Class reinit (`0x00574370`)

`reinit(game, unit, class, mode)` (ECX game, EDX unit; stack class,
mode; `ret 8`) turns an existing monster into another class in place.
Returns 1 when done, else 0 with nothing changed.

1. Unit missing or not a monster (type ≠ 1) → 0.
2. `class` < 0, ≥ the monstats count, or its row lacks `enabled`
   (monstats byte +0xF & 0x02, bit 25 of the flags dword +0x0C) → 0.
3. R := the unit's room (`0x00620BB0`), G := unit +0x0C (its GUID),
   both read before the teardown.
4. Teardown `0x005736A0(game, unit)` (also the monster branch of the
   unit free, call at `0x005556C9`), with the old class:
   1. Drop the unit's own combat-list entries (`0x0057C980`,
      `sim/units.md` §2 row +0xAC).
   2. Unless the old class is a valid row with `interact` (monstats
      byte +0xD & 0x02): remove the inventory's items (`0x00555AE0`)
      and free the inventory (`0x0063AC40(unit +0x60)`). An `interact`
      monster (NPC) keeps its inventory.
   3. Hover record (unit +0xA4) non-zero → returned to the game pool
      (`0x006611A0`); the pointer is not cleared (Edge cases 13).
   4. Free monster data +0x28 AI control (`0x0058F810`), +0x2C AI
      params (`0x005A64D0`), +0x30 interaction block (`0x00572BC0`).
      The monster data record itself is kept.
   5. Cancel all the unit's timers (`0x00540EE0`).
5. unit +0x04 := `class`.
6. Rebuild with the type init `0x00574250(game, R, unit, G)` (§5, with
   §6): new AI control, AI params and interaction block, monstats
   pointer, region data, stats and skills (draws on the unit seed, which
   is not re-derived), first AI setup with state 0 (the call at
   `0x00574307`; its draws: Open question 2), level id. §5 step 7 tests
   the unit's **old** mode (+0x10 is not yet changed). Monster-data
   fields that §5 and §6 do not write (type flags, umods, name seed)
   keep their values; no umod init or boss mods run.
7. Mode := `mode` through the plain mode set `0x00624690`
   (`sim/units.md` §4.1), not the monster mode change `0x005A7C20`.
8. Return 1.

Callers (all three in 1.14d):

| Call site | Function | Class, mode |
|---|---|---|
| `0x005A733E` | `0x005A72B0`, the death mode's event-1 function (`sim/units.md` §4.6 table, mode 0 DT) | monstats `SplEndDeath` (+0x1A4) = 1 and `minion1` (+0x26, s16) ≥ 0 and ≤ the count: `minion1`, mode 1; then think restart `0x00573780` (`monsters/ai.md` §1.5). 1.14d data: fetishshaman1–5 (278–282) and 6–8 (662–664) become fetish1–5 (141–145) and 6–8 (656–658). `SplEndDeath` 2 (barricadetower, 435) takes the other branch, no reinit |
| `0x005D1EA1` | `0x005D1E10`, the skill body Transform (`skills/bodies-4.md`) | the summon class mapped along `NextInClass`, the skill's mode |
| `0x005EF450` | `0x005EF320`, the BaalThrone AI (`monsters/ai-bodies-5.md` §20 step 6) | 559 baalcrabstairs, mode 1 |

## Constants & data dependencies

| Table | Columns read at creation (fields.tsv names) |
|---|---|
| monstats | `Id` (row), `BaseId`, `MonStatsEx`, `MonProp`, `MonType`, `minion1`, `minion2`, `PartyMin`, `PartyMax`, `Velocity`, `Align`, `Level`/`(N)`/`(H)`, `MinHP`, `MaxHP`, `AC`, `Exp` (each with `(N)`/`(H)`), `ResDm`, `ResMa`, `ResFi`, `ResLi`, `ResCo`, `ResPo`, `ToBlock` (each with `(N)`/`(H)`), `DamageRegen`, `Skill1`–`Skill8`, `Sk1lvl`–`Sk8lvl`, `Sk1mode`–`Sk8mode`; flags `enabled`, `noRatio`, `boss`, `SetBoss`, `BossXfer`, `neverCount`, `petIgnore`, `inventory`, `interact`, `isMelee`, `nomultishot`; runtime field +0x2A (monequip index) |
| monstats2 | `spawnCol`, `SizeX`, `isAtt`, component choice counts (+0x15..+0x24, from `HDv`…`S8v`), mode bits `mA1`, `mWL` (umod `fPick`) |
| monlvl | `HP`, `AC`, `XP`, `DM` and their `L-` forms, per difficulty; row = level |
| levels | `MonLvl1Ex`–`MonLvl3Ex` (expansion), `MonLvl1`–`MonLvl3` |
| monprop | `prop<i>`, `par<i>`, `min<i>`, `max<i>`, `chance<i>` with ` (N)`, ` (H)` |
| monequip | `monster`, `level`, `oninit`, `item1`–`3`, `loc1`–`3`, `mod1`–`3` |
| monumod | `enabled`, `version`, `xfer`, `champion`, `fPick`, `exclude1`, `exclude2`, `cpick`, `upick` (with ` (N)`, ` (H)`), `constants` |
| superuniques | `Class`, `hcIdx`, `Mod1`–`Mod3`, `MinGrp`, `MaxGrp`, `AutoPos`, `Stacks` |
| difficultylevels | `MonsterSkillBonus`, `ChampionDamageBonus` |

Not read at creation: monstats `Drain`, `Crit`, `TreasureClass*`,
`A1MinD`… (combat / treasure specs); superuniques `TC*`, `Utrans*`.

Fixed constants: collision masks 0x3C01 / 0x1C0 / 0x3F11 / 0; ring step
3; HP cap 0x7FFFFF; attackrate 100, velocitypercent 75, other_animrate
100, last_sent_hp_pct 128; unique minion group 3..6; champion BaseId
exception 118; umod 41 period 75 frames.

## Randomness

Seeds: U = the new monster's unit seed (unit +0x20); R = active room
seed (room +0x6C); G = game seed (game +0xD0). For one call of
`0x005B2A00` that succeeds, in order:

1. Placement: active room seed draws, order in `monsters/population.md`
   §9.
2. Allocation: G one step → U (`sim/rng.md` §5.3).
3. Components (§10): U `roll(variant count)`, or 16 × U `roll(count i)`
   (no step when count ≤ 0).
4. Class 311 / 312: U `roll(4)`.
5. HP: U `roll(maxHP − minHP + 1)`.
6. monprop: per property with chance > 0, U step, lo' % 100.
7. monequip: per processed row with slots, U `roll(slot count)`; item
   creation draws (treasure spec).
8. First AI setup `0x005B0E00` (draws, if any, per `monsters/ai.md`).
   Then the add-to-world sequence §4.1: U draws only from mode damage
   when an `El{i}Mode` equals the creation mode (none for modes 1 and
   12 with 1.14d data).
9. Normal / boss mods: umod init functions draw nothing except as listed
   in 11; superunique extra spawns per `population.md` §11.4. Boss mods
   (§14.3): only BaseId 540 draws: four item creations (`0x00559CE0`,
   each one game-seed step for the item's unit seed and one for its
   item seed, then the item's own rolls; `items/generation.md`), in
   table order k = 0..3.
10. Party minions: U `roll(PartyMax − PartyMin + 1)` when `PartyMin` <
    `PartyMax`; then each minion's own creation (1–10, recursively,
    with that minion's unit seed).

Then, for bosses:

11. `0x005A43E0`: U `roll(100)` (champion test, if allowed); champion:
    U `roll(total cpick)`; else U step (`roll(1)`), then per unique umod
    U `roll(total upick)` (none when the total is 0); then minions: U
    `roll(4)` (count 3..6) and each minion's creation; then umod 1:
    U step (name seed); umod 30: temporary seed only. Champion pack
    members (§16.2, after their own creation): umod 1 U step each.
12. `0x005A49B0`: per difficulty level U `roll(total upick)`; minions U
    `roll(MaxGrp − MinGrp + 1)` (after the difficulty add); each
    minion's creation; umod 1: U step; per-row extras (Radament: U
    `roll(5)`).

## Edge cases & original bugs

1. Champion pack members always get umod 16, whatever the leader's
   champion umod (§16.2).
2. Boss creation counts toward the region boss limit before the
   champion/unique decision (`0x005A0320`), so champions use it too.
3. HP is rolled once at the base level; umod 4's +3 levels (and
   champion −1) do not change HP, AC or experience except through the
   modifiers' own percentages.
4. Champion experience uses e − 2 × (e / 5) (e = 3 stays 3); D2MOO
   computes e − 2e/5.
5. Willowisp-based champions (BaseId 118) get half the champion,
   strong and berserk damage bonus; their to-hit bonus is not halved.
   Not in D2MOO (1.10f).
6. Boss minions receive umod 4 (+3 levels, ×5 experience) and umod 2
   minion HP, because those functions run on the minion list with
   unique = 0 and umod 4 ignores the flag.
7. The xfer copy counts skipped (non-xfer) boss umods against the
   9-slot limit.
8. Superunique aura runs twice (§20 step 4).
9. The umod dispatcher walks all 9 slots even after a 0.
10. Restored monsters get new HP and component rolls (§21).
11. Classic scaling leaves hitpoints at the unscaled value while maxhp
    is halved.
12. `0x00573930` writes difficulty 2 into the game when it finds ≥ 3.
13. The class reinit teardown (§27 step 4.3) frees the hover record
    (unit +0xA4) but does not clear the pointer; whether a later write
    replaces it before any read is Open question 11.
14. A reinit monster keeps its umods, type flags and name seed but gets
    the new class's stats with no umod init re-run (§27 step 6).

## Test vectors

### Synthetic (CI-safe)

| Input | Expected | Source |
|---|---|---|
| pct(1792, 200, 100) | 3584 | §8.2 |
| pct(0x7FFFFF00, 50, 100) | 1073741650 ((v/100)×50) | §8.2 branch 2 |
| pct(5, 70000, 100) | 3500 ((p/100)×5) | §8.2 branch 3 |
| pct(7, −75, 100) | −5 | §8.2 |
| HP roll, minHP 7, maxHP 12, bonus 0 | maxhp = (7 + roll(6)) × 256 | §6 step 8 |
| HP base 0x900000, bonus 0 | maxhp = 0x7FFFFF × 256 | cap |
| maxhp 1792, DamageRegen 2 | hpregen 0 | §6 step 10 |
| player count n = 1, 2, 3, 8, 9 | HP/XP bonus 0, 50, 100, 350, HP 350 / XP 350 | §9 |
| champion fn, base exp 160, Normal, BaseId ≠ 118 | exp 96, damagepercent +90, item_tohit_percent +67 | §19.2 |
| same, BaseId 118 | damagepercent +45, item_tohit_percent +67 | §19.2 |
| fast, Velocity 4 | velocitypercent +100 (2048/4 − 128 = 384 → 100) | §19.4 |
| fast, Velocity 15 | +10 (136 − 128 = 8 → 10) | §19.4 |
| umod count, unique, d = 0/1/2, no umods | 1 / 2 / 3 | §17 step 2 |
| aura, level 5, roll index 3 | holyfreeze level max(5 / 7, 1) = 1 | §19.5 |
| aura, level 40, index 6 | holyshock level 5 | §19.5 |
| montype rows 1 `a`, 2 `b` (equiv1 `a`); class MonType 1, exclude 2 | excluded (2 is a sub-type of 1) | §17.3 r2 |
| same, class MonType 2, exclude 1 | not excluded (1 is not of type 2) | §17.3 r2 |
| class MonType 0, any exclude | not excluded (montype column 0 never set) | §17.3 r2 |

### Real 1.14d values (live tables; `#[ignore]`, `D2_GAME_DIR`)

Expansion single-player game (L-flag 1), player count 1. Level and
(minHP, maxHP, AC, XP) after §7–§8; HP is then rolled in [minHP, maxHP].

| Class | Normal (any Act 1 area) | NM Blood Moor / Cold Plains | Hell Blood Moor | Hell Cold Plains |
|---|---|---|---|---|
| zombie1 (5) | 1: 7, 12, 5, 33 | 36: 551, 787, 422, 2696 | 67: 4317, 6168, 1067, 28069 | 68: 4438, 6340, 1081, 29496 |
| fallen1 (19) | 1: 1, 4, 5, 18 | 36: 131, 288, 369, 1669 | 67: 1028, 2261, 933, 17376 | 68: 1056, 2324, 946, 18259 |
| brute1 (28) | 2: 11, 19, 10, 48 | 36: 761, 1102, 448, 3081 | 67: 5962, 8635, 1133, 32079 | 68: 6129, 8876, 1149, 33710 |
| fallenshaman1 (58) | 2: 5, 9, 10, 32 | 36: 288, 472, 396, 3852 | 67: 2261, 3700, 1000, 40099 | 68: 2324, 3804, 1014, 42138 |
| quillrat1 (63) | 1: 1, 5, 5, 21 | 36: 105, 367, 422, 1797 | 67: 822, 2878, 1067, 18713 | 68: 845, 2958, 1081, 19664 |
| cr_lancer1 (165) | 2: 7, 11, 10, 36 | 36: 420, 603, 501, 2182 | 67: 3289, 4728, 1267, 22723 | 68: 3381, 4861, 1284, 23878 |

Without the L-flag (L-flag 0) only NM/Hell AC and Hell HP change, e.g.
zombie1 Hell Blood Moor 3238, 4626, 907, 28069. Resistances (Dm, Ma, Fi,
Li, Co, Po) and ToBlock, Normal / NM / Hell: zombie1 0,0,0,0,0,50 / …75 /
50,0,0,0,120,75, block 3; fallen1 0s / 0s / 15,0,100,0,40,0, block 9;
brute1 0s / 0s / 50,0,0,0,100,0, block 4; fallenshaman1 fire 25 / 50 /
15,0,100,0,0,0, block 4; quillrat1 0s / 0s / 50,0,0,0,50,0, block 3;
cr_lancer1 0s / 0s / 45,0,25,100,25,25, block 4.

Bosses, Normal, Blood Moor (L-flag 1):

| Monster | Expected |
|---|---|
| champion fallenshaman1, umods [16] | level 4; maxhp = 3 × base (base ∈ 5..9) × 256; hpregen 0; experience 96; damagepercent +90; item_tohit_percent +67; velocitypercent 75 + 20; type flags 1, 4, 8; name seed set; no minions |
| ghostly champion fallenshaman1, [36] | as above but damageresist 80, velocitypercent 75, type flag 0x40, coldmindam 1, coldmaxdam 2 (L-DM 4 at level 4 × 33 / 50 %), coldlength 150 |
| unique fallen1, Normal | level 4; maxhp = 4 × base (1..4) × 256; experience 90; 1 unique umod; 3..6 fallen1 minions, each with HP × 2, level own + 3, experience × 5 |
| champion HP factor NM / Hell | × 2.5 / × 2; unique × 3 / × 2; boss minion × 1.75 / × 1.5 |

### Recorded checks (monster assign 0xAC)

`traces/raw/20261006-015956-packets.jsonl` and `-022633-packets.jsonl`
(291 messages, decoded with §24's layout):

| Check | Observed |
|---|---|
| life byte at first assign | 128 in all 291 |
| brute1 normal | umods [13], no type flags, name seed 0 (frames 134, 301, 678, 1108) |
| brute1 champion pack (frame 678, GUIDs 60–62) | umods [13, 16], champion + unique flags, name seeds 56351, 54141, 54177 |
| fallenshaman1 champions (frame 3649, GUIDs 50, 51) | [16] champion+unique, seed 8013; [36] champion+unique+ghostly, seed 62586 |
| components | zombie1 [2,2,1,1,0,0,0,0,1,1,…] within counts [3,3,3,3,3,0,0,0,3,3,3]; brute1 / quillrat1 none (all counts ≤ 1) |

## Provenance

- §27 class reinit from the 1.14d asm: `0x00574370` (`ret 8`, unit
  type test, bounds and `enabled` test against the bit table
  `0x006CE26C` = 2 read from `game/Game.exe`), `0x005736A0` (`interact`
  test byte +0xD), the tail of `0x00574250` (`0x00574300`–`0x0057434A`),
  callers from `disasm.py xref 0x574370` and `0x005A72B0`–`0x005A734E`;
  `SplEndDeath` / `minion1` rows from the live 1.14d monstats.txt;
  monstats bits from `data/fields.tsv` (`enabled` bit 25, `interact`
  bit 9).
- §25 conventions from the prologues, `ret n` and call sites of
  `0x005B2A00`, `0x005A09E0`, `0x005A43E0` (`0x005A43E0`–`0x005A4437`),
  `0x005A0760`, `0x005A2120` (`ret 0xc` at `0x005A21CA`), `0x005A48C0`,
  `0x0054E1E0` (`ret 8` at `0x0054E25B`), `0x00463740`, and the callers
  `0x0054EC90`, `0x0054E260`; `0x005A0260` (umod count);
  `time_value` (`0x00650DE0`) callers listed from the disassembly
  (`0x00476290`, `0x00476460`, `0x0052C280`, `0x00552DF0` only).
- 1.14d `Game.exe` (asm `re/exports/all.asm`, table bytes from
  `game/Game.exe`): `0x005B2A00` and wrappers `0x005B2F20`,
  `0x005B3040`, `0x005B3090`, `0x005B30E0`, `0x005B23C0`, `0x005B24E0`,
  `0x005B31B0`; placement `0x005B2700` and jump table `0x005B2F04`;
  allocator dispatch `0x00555230` (table `0x005554E8`, monster case
  `0x00555393`); `0x00574250`, `0x00573CB0`, `0x005739D0`,
  `0x00573930` (tables `0x006E1590`, `0x006E15B4`), `0x0061DCA0`,
  `0x006538A0`, `0x00483360`, `0x0063EE90`, `0x0063EEF0` (table
  `0x006EA9F0`), `0x00573470`, `0x005D6B60`; bit masks `0x006CE268`;
  `0x005B21B0` (tables `0x005B22DC`, `0x005B22F4`), `0x005B1CF0`,
  `0x005B2830`, `0x004CC790`; `0x005A09E0`, `0x005A0320`, `0x005A43E0`,
  `0x005A0760`, `0x005A0500`, `0x005A0600`, `0x005A03E0`, `0x005A2120`
  (list `0x006E2168`), `0x005A0C00`, `0x005A0930`, `0x005A4850`; umod
  table `0x0073C008` and functions `0x005A0CE0`, `0x005A0DC0`,
  `0x005A0D20`, `0x005A0E40`, `0x005A0E80`, `0x005A1080`, `0x005A11F0`,
  `0x005A1230`, `0x005A1280`, `0x005A1330`, `0x005A1370`, `0x005A17E0`,
  `0x005A1910`, `0x005A1990`, `0x005A1650` (aura table `0x0073BF68`);
  `0x005A49B0` (hcIdx tables `0x005A4EA4`, `0x005A4E7C`), `0x005A4440`,
  `0x005A46E0`; boss-mod case tables `0x005B20A4` / `0x005B2060`,
  `0x005B1C50` (table `0x006E1BB0`), `0x00573B20`, `0x005447A0`;
  make unique `0x005A4940` (caller `0x005830CC`), `0x0065A800`,
  `0x00582750`;
  add-to-world `0x00554850` (table `0x00554A18`), `0x005735A0`,
  `0x005A7C20`, `0x00624390` (type table `0x0062467C`); dispatcher
  `0x005A4270`,
  callback table `0x0073C0B8`, `0x005A4370` (bytes), monster handler
  table `0x006E2490`; `0x005A4230`, `0x005A25F0`, `0x005A37D0`,
  `0x005A3800`; client names `0x004AC870`, `0x00653ED0`–`0x00653F50`;
  message builder `0x0053E2E0`; game type store `0x00530CFF` from the
  create-game handler `0x0053F100`.
- Live tables `game/extracted/patch_d2/data/global/excel/*.bin` and
  `.txt` (monstats 734 rows, monlvl 111, monumod 43, superuniques 66,
  levels 137); vectors computed with these files and the §8 rules.
- Recordings: game type 3 in client message 0x67 (20261006-015956,
  seq 1, byte 0x11); assign messages as in Test vectors.
- D2MOO (1.10f) `MonsterSpawn.cpp`, `Monster.cpp`, `MonsterUnique.cpp`,
  `MonsterTbls.cpp`, `Monsters.cpp`, `SCmd.cpp` used as a map; each rule
  above was re-read on 1.14d. Differences found: 16 components (not 12),
  champion experience rounding, the BaseId 118 halving, the empty skill
  bonus stub, uber cases in boss mods, the always-present unique flag on
  `0x005A09E0` bosses (as in D2MOO), umod 41 handler details. The init
  bodies of umods 17, 18, 23, 25, 26 were re-read 2026-10-07
  (`umod-init-bodies.md`); no `umods.tsv` row is D2MOO-only.
- §17.3 rule 2: asm of `0x005A0070` (unit in ECX, exclude in EDX; row
  = EDX, column = monstats +0x1C; matrix count / words / width at data
  tables +0xC40 / +0xC44 / +0xC48, filled by `0x006C2110`). 1.14d live
  data: one `monumod.txt` row has an exclude (`lightning`, id 17:
  `exclude1` `sandleaper`, montype row 22, no `equiv` links and no
  sub-types), so the matrix direction does not change a live answer:
  exactly the 7 classes with MonType `sandleaper` (sandleaper1–7) are
  excluded.
  Re-read 2026-10-06 (implementation question IH1): `0x005A0070` does
  no walk of its own; after the bounds tests it loads one word of the
  built matrix (row = exclude, word = row × width + MonType / 32) and
  masks it with the bit table at `0x006CE268` (bit MonType mod 32), so
  an answer from the matrix is exact, including the walk's depth limit
  and bad links as `0x006C2110` built them. Direction: row = exclude,
  column = MonType ("is the exclude type of type MonType?"), as in rule
  2 and its three test vectors; a description as "MonType nested in the
  exclude type" is the reverse and wrong. The return value is the
  masked word (non-zero = set), not 1.

## Open questions

1. Answered (2026-10-07): game +0x6A is the game type from C→S 0x67
   byte +0x11, which the client sets to 3 for single player (client game
   type 0), 1 for type 6, 2 for type 8, else 0 (`0x00477CA0`); +0x74 is
   the ladder flag (creation flags bit 21). Owner: `sim/units.md` OQ7.
   Single player (client game type 0) therefore takes the L-columns
   in §8.1's test.
2. Answered (2026-10-07): `0x005B0E00` (`ai.md` §3.3) draws nothing
   itself; the only draws are the AI record's init function
   (`ai-functions.tsv` `init_1_14d`). Of the 16 init functions, Raven
   `0x005ECB70`, NpcBarb `0x005EDC40` and Nihlathak `0x005EE5C0` step a
   seed and BaalThrone `0x005EF310` creates monsters (their draws); the
   other 12 draw nothing (scan for the generator constant 0x6AC690C5 and
   the `rng.md` helpers, 3 call levels deep). Ordinary room monsters
   (AIs without init) draw nothing at step 8 of Randomness.
3. Answered (2026-10-07): §4.1 lists the allocator's steps after the
   type init and their draws; for creation modes 1 and 12 with 1.14d
   data there are none (Randomness step 8).
4. No RNG trace of a spawn exists: record one population pass (rng hook
   with caller addresses) to confirm the order in Randomness.
5. Answered (2026-10-07): `0x0054DC40` and its draws are owned by
   `monsters/population.md` §8.
6. Answered (2026-10-07): every case read from the asm: boss mods §14.3
   (with the ancient barbarian equipment and its item draws),
   superunique hcIdx cases §20.1.
7. Answered (2026-10-07): umods 17, 18, 23, 25, 26 init bodies read on
   1.14d (`0x005A1B00`, `0x005A1C70`, `0x005A1E00`, `0x005A1F90`,
   `0x005A1600`): `monsters/umod-init-bodies.md`.
8. Behaviour of the mode 0/1/3/4/5 and type-7 callbacks other than those
   in §22 (death explosions, curses, hit effects): owner to be decided
   (monster death/combat spec); catalogue in `umods.tsv`.
9. Whether `0x00573780` (umod 41 event) draws RNG.
10. The client name draw order of §23 assumes the C argument order seen
    in `0x004AC870`; confirm with a client RNG trace showing a unique.

Answered 2026-10-07: 8 → every callback body is in
`monsters/umod-callbacks.md` (owner). 9 → `0x00573780` draws nothing
(`umod-callbacks.md` §3.5).

Answered 2026-10-07: 7 → read on 1.14d; bodies in
`monsters/umod-init-bodies.md` (one difference from the old summary: the
cold / poison length stats use the clamped monlvl row, not the level);
no `umods.tsv` row is D2MOO-only any more.
11. After a class reinit (§27), unit +0xA4 still points at the freed
    hover record (Edge cases 13): find every reader of +0xA4 on a
    monster and whether one can run before a new record is written.
12. Size: this spec is ~71 KB, over the 60 KB guideline. Split
    proposal (not done; needs every inbound `init.md` §N link updated in
    the same change): move §14–§22 (normal and boss mods, boss spawns,
    umod choice and init, superuniques, restore paths, umod callbacks;
    ~25 KB with their test vectors and provenance) to a new
    `monsters/bosses.md`, keeping section numbers as a stub table here;
    optionally move §25–§26 (tool spawns, making an existing monster
    unique; ~11 KB) to `monsters/init-tools.md`. §1–§13, §23, §24 and
    §27 (plain creation, the class reinit) stay.
