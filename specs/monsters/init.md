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
| Summary | 64–82 |
| Inputs | 83–93 |
| Outputs / state changes | 94–118 |
| Rules | 119–120 |
|   1. Entry points | 121–143 |
|   2. The create request | 144–161 |
|   3. Placement | 162–170 |
|   4. Creation sequence after placement (`0x005B2A00`) | 171–188 |
|   5. Monster type init (`0x00574250`) | 189–206 |
|   6. Stats and skills (`0x00573CB0`) | 207–246 |
|   7. Monster level | 247–262 |
|   8. Base values from monlvl | 263–297 |
|   9. Player-count bonus (`0x00573930`) | 298–309 |
|   10. Components (`0x005739D0`) | 310–320 |
|   11. monprop (`monprop.txt`) | 321–329 |
|   12. monequip (`0x005D6B60`) | 330–346 |
|   13. Classic scaling (`0x0063EEF0`) | 347–354 |
|   14. Normal mods and boss mods | 355–389 |
|   15. Party minions | 390–394 |
|   16. Boss spawns | 395–430 |
|   17. Choosing umods (`0x005A0760`) | 431–466 |
|   18. Boss minions and umod init (`0x005A2120`) | 467–484 |
|   19. Umod init functions | 485–564 |
|   20. Superuniques (`0x005A49B0`) | 565–588 |
|   21. Restore paths (`0x005A4440`, `0x005A46E0`) | 589–603 |
|   22. Umod callbacks and the type-7 event | 604–645 |
|   23. Unique names (client) | 646–655 |
|   24. Monster assign message | 656–668 |
| Constants & data dependencies | 669–690 |
| Randomness | 691–726 |
| Edge cases & original bugs | 727–752 |
| Test vectors | 753–754 |
|   Synthetic (CI-safe) | 755–774 |
|   Real 1.14d values (live tables; `#[ignore]`, `D2_GAME_DIR`) | 775–805 |
|   Recorded checks (monster assign 0xAC) | 806–818 |
| Provenance | 819–856 |
| Open questions | 857–880 |
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
`MONSTERSPAWN_SetupBossMods` (call pattern checked at `0x005B1CF0`; per
case details open question 6).

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
2. `exclude1`, `exclude2` (> 0): the class's MonType is that type or
   nested in it (`0x005A0070`) → no.
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
| 17, 18, 23, 25 | same pattern for light, cold (+ coldlength 5 × level + 100), poison (+ poisonlength 2 × (5 × level + 150)), mana drain (× 256); then 19.3 (pattern from D2MOO; table entries 1.14d-confirmed) |

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
| 26 teleport | skill monteleport level 1, mode 4, AI flag 0x20 (D2MOO; open question 7) |
| 41 always_run_ai | schedule a type-7 event at frame + 75 (`0x005417D0`), any unique value |

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
unique → frame + 4). The only type-7 event scheduled at init is umod 41's
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
9. Normal / boss mods: umod init functions draw nothing except as listed
   in 11; superunique extra spawns per `population.md` §11.4.
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
  `0x005A49B0`, `0x005A4440`, `0x005A46E0`; dispatcher `0x005A4270`,
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
  `0x005A09E0` bosses (as in D2MOO), umod 41 handler details. Rows marked
  "D2MOO" in `umods.tsv` were not re-read.

## Open questions

1. The value of game +0x6A in other modes (classic SP, TCP/IP, realm);
   settle by recording client message 0x67 in each mode. Rules use the
   L-columns only through §8.1's test.
2. The first AI setup `0x005B0E00` may draw from the unit seed at init;
   `monsters/ai.md` to state its draws; settle with an RNG trace of one
   spawn.
3. Draws taken by the allocator after the type init (room insert, path
   init) are not listed here; `sim/units.md` to confirm none.
4. No RNG trace of a spawn exists: record one population pass (rng hook
   with caller addresses) to confirm the order in Randomness.
5. Position finder `0x0054DC40` (x = y = 0) draws: `population.md`.
6. Per-case details of boss mods `0x005B1CF0` and superunique hcIdx cases
   in `0x005A49B0` beyond those listed: read the asm case by case.
7. Umods 17, 18, 23, 25, 26 init bodies follow D2MOO; read
   `0x005A1B00`, `0x005A1C70`, `0x005A1E00`, `0x005A1F90`, `0x005A1600`.
8. Behaviour of the mode 0/1/3/4/5 and type-7 callbacks other than those
   in §22 (death explosions, curses, hit effects): owner to be decided
   (monster death/combat spec); catalogue in `umods.tsv`.
9. Whether `0x00573780` (umod 41 event) draws RNG.
10. The client name draw order of §23 assumes the C argument order seen
    in `0x004AC870`; confirm with a client RNG trace showing a unique.
