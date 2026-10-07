# Spec: Combat — Attack rating, defense, chance to hit, block, avoid

- **Status:** draft: every rule read from the 1.14d `Game.exe`
  disassembly (addresses below); D2MOO (1.10f) used only to name
  functions, differences noted. No trace check yet (Open questions 1–2).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::combat::hit`
- **Related specs:** `combat/damage.md` (the percent helper `pct` §0,
  the damage record, what happens after a hit); `skills/use.md` (which
  skill functions call the hit test, with which to-hit bonus);
  `sim/rng.md` (draw helpers, unit seed at unit +0x20); `sim/stats.md`
  (stat getters, stat-list internals); `sim/units.md` (modes, melee
  range, hostility).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 37–49 |
| Inputs | 50–67 |
| Outputs / state changes | 68–75 |
| Rules | 76–77 |
|   1. Attack rating | 78–89 |
|   2. Defense | 90–114 |
|   3. Chance to hit | 115–194 |
|   4. Melee result flags | 195–219 |
|   5. Block chance | 220–242 |
|   6. Block, weapon block, dodge, avoid, evade | 243–300 |
|   7. Hostility and melee range | 301–363 |
| Constants & data dependencies | 364–380 |
| Randomness | 381–392 |
| Edge cases & original bugs | 393–413 |
| Test vectors | 414–438 |
| Provenance | 439–464 |
| Open questions | 465–483 |
<!-- /index -->

## Summary

A melee attack asks one question before any damage is rolled: what
happened to the defender (hit, block, weapon-block, dodge, avoid, evade,
miss). The answer is a set of result flags. The hit test compares the
attacker's attack rating with the defender's defense, scaled by both
levels, clamped to 5–95 %, and draws once from the attacker's seed. A
hit is then subject to the defender's block (shield), weapon block
(dual claws), and the Amazon passives (evade while moving, dodge against
melee, avoid against missiles), each one draw from the defender's seed.
This spec owns those formulas, the attack-rating and defense totals, and
their draw order.

## Inputs

| Name | Type | Source |
|---|---|---|
| attacker, defender | units (player 0 or monster 1 for the melee path) | caller (a skill do-function, `skills/use.md`) |
| to-hit bonus | i32, percent | caller: the skill's to-hit value (`ToHit`/`ToHitCalc`, `skills/use.md`) |
| range offset | i32 | caller; 0 = no range adjustment |
| missile flag, avoid flag | bool | caller (missile path: monsters branch) |
| stats | i32 | stat getters (`sim/stats.md`), ids from `itemstatcost.txt` |
| `charstats.txt` | `ToHitFactor` (+0x3C, i32), `BlockFactor` (+0x49, u8) | record = player class id |
| `monstats.txt` | flags dword +0x0C (`NoShldBlock` bit 26), `MonType` (+0x1C) | record = monster class id |
| game | expansion flag (game +0x70), difficulty (game +0x6D) | game record |

Stat getters (named by the stats spec): **unit** = `0x00625480(unit,
stat, layer)` (D2MOO `STATLIST_UnitGetStatValue`); **item/skill** =
`0x00625500(unit, stat, layer)` (D2MOO
`STATLIST_UnitGetItemStatOrSkillStatValue`). Layer is 0 unless stated.

## Outputs / state changes

- Result flags (u16, the damage record's +0x04, `combat/damage.md` §1):
  `0x0001` hit, `0x0004` get-hit, `0x0010` block, `0x0080` dodge,
  `0x0100` avoid, `0x8000` weapon block. Evade sets no flag (§6.3).
- Seeds of attacker and defender (draws, §Randomness).
- On a successful hit test, the prevent-heal curse (§3.5).

## Rules

### 1. Attack rating

`attack_rating(player)` = `0x00622560` (D2MOO `UNITS_GetAttackRate`):

    AR = tohit(19) + 5 × (dexterity(2) − 7) + charstats.ToHitFactor

Both stats through the unit getter. The function accepts only players
(any other unit is a fatal assertion). 32-bit wrapping arithmetic.

Monster attackers do not use it; their attack rating is computed inside
the hit test (§3.2).

### 2. Defense

`defense(unit)` = `0x006223F0` (D2MOO `UNITS_GetDefense`), any unit:

1. `base = armorclass(31) + dexterity(2) / 4` (unit getter; division
   truncates toward zero).
2. `pct = skill_armor_percent(171) + item_armor_percent(16)`
   (item/skill getter, in that order).
3. Holy Shield: if the unit has state 101 (`holyshield`): take the
   state's stat list (`0x006256B0(unit, 101)`); `skill =
   modifierlist_skill(350)`, `level = modifierlist_level(351)` from that
   list (`0x00625D00`); if both > 0, the skill record exists, and a
   shield is equipped (`0x0063C8F0(inventory, &item)`), `pct +=
   skillcalc(unit, skills.calc1, skill, level)` (`0x00646CA0`, the
   skills formula evaluator, `data/calc-expressions.md` §3).
4. `bonus = (pct × base) / 100` if `base > 0`, else
   `(pct × base) / −100` (32-bit product, truncating division). A
   positive percent therefore raises a negative base toward zero.
   `total = base + bonus`.
5. `ovr = armor_override_percent(182)` (unit getter). If `ovr ≠ 0`:
   `total += pct(total, ovr, 100)` (`combat/damage.md` §0).

The hit test adds a per-attack term (§3.1): `armorclass_vs_missile(32)`
for missiles, `armorclass_vs_hth(33)` for melee.

### 3. Chance to hit

`hit_test(attacker, defender, bonus, missile)` = `0x0057D9B0` (D2MOO
`SUNITDMG_IsHitSuccessful`). Fastcall: ECX attacker, EDX defender;
stack bonus, missile. Returns 0 (miss) or 1 (hit). Null attacker or
defender: 0, no draw.

#### 3.1 Common terms

- `dlvl = level(12)` of the defender, `alvl = level(12)` of the
  attacker (unit getter).
- `def = defense(defender) + (missile ? armorclass_vs_missile(32) :
  armorclass_vs_hth(33))`.

#### 3.2 Attacker terms

Player attacker (unit type 0), in this order:

1. `AR = attack_rating(attacker)`.
2. `0x0057D8A0` adjusts AR and def:
   1. If `item_ignoretargetac(115)` (item/skill) ≠ 0, the defender is a
      monster, not unique or superunique (`0x005A0180(def, 0x0A)` false),
      not a boss (`0x0063E9F0`), not a hireling (`0x0063EE90`): `def = 0`.
   2. `f = item_fractionaltargetac(116)` (item/skill). If `f > 0`:
      halve it (truncating toward zero) if the defender is a player, or a
      monster that is superunique (`0x005A0180(def, 2)`), a boss or a
      hireling; clamp to at most 100 (negative values become 0); `def −=
      pct(def, f, 100)`.
   3. `AR += item_demon_tohit(123)` if non-zero and the defender is a
      demon (`0x0063E940`); then `AR += item_undead_tohit(124)` if
      non-zero and undead (`0x0063E990`).
3. `pctAR = 0`; if not a missile and the attacker has a current weapon
   (`0x00535BC0`): `pctAR = weapon_mastery(attacker, weapon, 0, 0)`
   (`0x00645830`, to-hit mode; owned by `skills/levels`).
4. `pctAR += bonus + item_tohit_percent(119)` (item/skill).
5. Defender is a monster with `MonType > 0`: for every stat
   `attack_vs_montype(179)` on the attacker (`0x006261D0` copies at
   most 128 entries, layer = montype), add its value when the layer
   matches the defender's montype (`0x0057A830`, the montype nest
   bitmap: layer and montype both in `1 … count−1`).

Monster or other attacker (unit type ≠ 0):

    AR    = tohit(19) + bonus + 5 × dexterity(2)
    pctAR = item_tohit_percent(119)            (item/skill)

(no −7, no `ToHitFactor`, no weapon mastery; the skill bonus is added
flat, not as a percent).

#### 3.3 Formula

    toHit = AR + pct(AR, pctAR, 100)
    if def < 0:    toHit −= def;  def = 0
    if toHit < 0:  def −= toHit;  toHit = 0
    if def < 0:    def = 0
    sum    = toHit + def
    factor = sum ≠ 0 ? (100 × toHit) / sum : 100
    chance = (2 × factor × alvl) / (alvl + dlvl)
    chance = clamp(chance, 5, 95)

All products and quotients are 32-bit signed, truncating. `alvl + dlvl
= 0` divides by zero (fault; Edge cases 2).

#### 3.4 Draw

One inline step of the attacker's seed: `r = lo′ mod 100`
(`sim/rng.md` §6). Hit if `r < chance`. The draw happens on every call
that reaches §3.3, hit or miss.

#### 3.5 On a hit

`0x0057D810(attacker, defender)` (D2MOO `SUNITDMG_PreventMonsterHeal`,
address confirmed by its 1.14d comment): player attacker, monster
defender, `item_preventheal(117)` (item/skill) ≠ 0 → apply the curse
state 52 (`preventheal`) for 120,000 frames (curse record: skill 0, level
1, stat 31 value 0). The curse helper belongs to the skills spec.

`0x005347B0` (a debug hook, `ret 0xC`) is called between draw and test;
it does nothing.

### 4. Melee result flags

`melee_result(game, attacker, defender, bonus, range_offset)` =
`0x0057EC10` (D2MOO `SUNITDMG_GetResultFlags`). Fastcall: ECX game, EDX
attacker; stack defender, bonus, range offset. Returns the u16 flags.

1. Return 0 unless both units exist, each is a player or a monster, and
   the hostility test `0x00554200(game, attacker, defender)` passes
   (§7.1). A monster defender also passes `0x005A4EE0` (true
   for every monster; kept for fidelity).
2. Defender is a player in mode 3 (run): flags = hit (no hit test).
3. `range = 1 + 2 × (attacker is a monster)`; if `range_offset ≠ 0`:
   `range −= range_offset + melee_range(attacker)` (`0x00622870`, §7.3).
   If `0x00622C40(attacker, defender, range)` (in melee range, §7.2) fails:
   return 0 (no draw).
4. If not yet hit: `hit_test(attacker, defender, bonus, missile = 0)`;
   hit → flags |= `0x0001`.
5. If hit: `b = block_or_dodge(game, attacker, defender, avoid = 0,
   block = 1)` (§6). Map `b`: 2 → `0x0100` avoid, 4 → `0x0080` dodge,
   `0x10` → `0x8000` weapon block, 1 → `0x0010` block. If `b ≠ 0`, clear
   `0x0001`.
6. If `0x0010` is set, clear `0x0001` (again).
7. If `0x0001` is set and the defender lacks state 54
   (`uninterruptable`): flags |= `0x0004` (get-hit).

### 5. Block chance

`block_chance(unit, expansion)` = `0x00622720` (D2MOO
`UNITS_GetBlockRate`):

Player:
1. No shield equipped (`0x0063C8F0(inventory, null)` = 0): 0.
2. `b = toblock(20) + charstats.BlockFactor`.
3. Expansion game: `lvl = max(level(12), 1)`;
   `b = ((dexterity(2) − 15) × b) / (2 × lvl)` (32-bit, truncating).
4. Result `min(b, 75)` (no lower clamp; it may be negative).

Monster:
1. `NoShldBlock` set: `min(toblock(20), 75)`.
2. Else `0x006225F0(unit)` decides whether it carries a usable shield:
   class 243 (`diablo`), 310 (`doomknight1`), 333 (`diabloclone`) yes;
   359 (`act3hire`) no; any other class: the composit item in slot 7
   (`SH`, `0x00664860(unit, 7, component)`) is a code other than `tch `
   (table `0x00744580`, 1 entry) whose items record has type 2 (shield).
   Yes → `min(toblock(20), 75)`, no → 0.

Other unit types: 0.

### 6. Block, weapon block, dodge, avoid, evade

#### 6.1 `block_or_dodge(game, attacker, defender, avoid, block)` = `0x0057DFB0`

1. `block = 0`: return `dodge(attacker, defender, avoid)`.
2. `c = block_chance(defender, game expansion)`. `c ≤ 0`: return
   `dodge(…)`.
3. Defender is a player in a moving mode (`0x00622D00`) other than mode
   2 (walk): `c = c / 3` (truncating). Running therefore divides; walking
   does not.
4. Draw: inline step of the **defender's** seed, `r = lo′ mod 100`.
   `r < c`: return 1 (block). Else return `dodge(…)`.

#### 6.2 `dodge(attacker, defender, avoid)` = `0x0057DD60`

1. Moving defender: player in mode 2 or 3 (walk, run), or monster in mode
   2 or 15 (walk, run):
   - `e = passive_evade(340)` (unit getter). `e > 0`: draw
     (defender seed, inline `lo′ mod 100`); `r < e` → return 8 (evade).
   - Return 0. (Moving units never weapon-block, dodge or avoid.)
2. Otherwise:
   1. `w = weapon_block(defender)` (§6.4). If `w > 0` and the defender's
      weapon class (`0x0064F380`) is 13 (`ht2`, two claws; index from the
      1.14d code table at `0x0072EFA8`): draw `roll(100)` (defender
      seed, helper `0x0045C390`); `r < w` → return `0x10`.
   2. `avoid ≠ 0`: `a = passive_avoid(339)` (unit getter); `a > 0`: draw
      `roll(100)`; `r < a` → return 2.
      `avoid = 0`: `d = passive_dodge(338)`; `d > 0`: draw `roll(100)`;
      `r < d` → return 4.
   3. Return 0.

`roll(100)` and the inline `lo′ mod 100` give the same value and both
step once (`sim/rng.md` §3).

#### 6.3 Evade

`melee_result` (§4) maps no flag for return value 8: an evaded melee hit
loses its hit flag and sets nothing else. The missile path maps it to
`0x0200` (monsters branch).

#### 6.4 `weapon_block(unit)` = `0x0057DCA0`

No unit: 0. Otherwise take the right-hand (bodyloc 4) and left-hand
(bodyloc 5) items from the unit's inventory (+0x60, `0x0063BDE0`; no
inventory: both none) and copy the unit's `passive_weaponblock(348)`
entries (`0x006261D0`, at most 32, each an 8-byte {u16 layer, u16 stat,
i32 value}). `w` starts at 0; walk the entries in copy order:

1. Layer (u16, an item type) = 0: `w := value` (unconditional, even
   when smaller than `w`).
2. Layer ≠ 0: if the left item is of that type (`0x00629BB0`), or else
   the right item is: `w := value` when `value > w` (signed).
3. Neither: unchanged.

Result `w`; no matching entry gives 0 (1.14d-confirmed, `0x0057DCA0`–
`0x0057DD50`). A layer-0 entry after a larger typed entry lowers `w`
(Edge cases 8). No draws.

### 7. Hostility and melee range

#### 7.1 Hostility `0x00554200` (ECX game, EDX attacker A; stack defender D)

Returns 1 when A may attack D. No draws.

1. A' := A. A is a monster with an owner (`0x0058F0D0`) → A' := the
   owner. A is a missile with the source-unit link (flag-ex +0xC8 bit
   0x400; `skills/bodies.md` §6.20) and the unit of type +0x94 / GUID
   +0x98 exists (`0x00552F60`) → A' := that unit.
2. D' := D; while D' is a monster whose owner exists and is not D'
   itself: D' := the owner.
3. A' = D' (also both none) → 0.
4. A' and D' both players: the relation entry of D' in A's player
   data (`0x006221A0`, list at +0x38: entries GUID +0x00, flags +0x04,
   next +0x10; first entry with GUID = D's GUID) → its flags & 8 (the
   hostile bit) ≠ 0; no entry → 0. Nothing writes this list in single
   player (relations are multiplayer, Phase 7), so players never attack
   players there.
5. Otherwise: 1 unless A' and D' are on the same side (`0x00650D70`):
   both evil (alignment 0) or both good (2). A neutral unit (1) is
   hostile to everyone.

Alignment `0x006259B0(unit)`: none, or no stat holder (+0x5C) → 0;
players and monsters → stat 172 `alignment` in the unit's state-105
(`alignment`) list (`0x006256B0`, `0x00625420`), 0 without the list;
other unit types → 2. Players carry state 105 with 172 = 2
(`client/msg-units.md` test vector); monsters by monstats `Align`
(`monsters/population.md` §9.6 rule 4).

#### 7.2 In melee range `0x00622C40(a, b, extra)`

1. a or b none → 0.
2. b is a monster whose `BaseId` (`0x00463860`) is 258 (`tentacle1`) or
   261 (`tentaclehead1`): d := unit distance(a, b) (`0x00641530`,
   `sim/pathing.md` §9.5); `melee_range(a)` + 8 > d → 1; else go on.
3. r := `melee_range(a)` + extra + 1; d := unit distance(a, b). d ≤ 0
   → 1. r < d → 0. Else 1 when the collision line a → b with mask
   0x804 is clear (`0x00622AA0(a, b, 0x804)` = 0, `monsters/ai.md`
   function table), else 0.

#### 7.3 Melee range `0x00622870(unit)`

1. Unit none → fatal (assert line 0x11BE).
2. Player: the weapon pick `0x0063C9B0` (`skills/bodies-3.md` §3.3
   step 2: body location 4, else 5, usable, type `weap`) → that item's
   `rangeadder` (weapons +0x104, u8; `0x006288D0`); none → 0.
3. Monster: monstats2 row of its class (`0x00451FE0`; none → 0).
   `MeleeRng` (u8 +0x0E) ≠ 255 → `MeleeRng`. 255 → 2 when the unit's
   weapon class in its current mode (`0x0064F380(unit, inventory, &c,
   mode −1, 1)`) is 6 (`2ht`), else 0.
4. Other unit types → 0.

#### 7.4 Callers of `hit_test` and `block_or_dodge`

| Caller | Call |
|---|---|
| `0x0057EC10` melee result (§4) | `hit_test(…, bonus, missile 0)`; `block_or_dodge(avoid 0, block 1)` |
| `0x005ADF10` missile hit handler (`0x005AE06F`) | `hit_test(owner, unit, missile stat 19 `tohit`, missile 1)` (`missiles/missiles.md` §R5 step 5) |
| `0x005A88E0` (no direct caller; ECX missile) | `hit_test(attacker, defender, missile total stat 19 (0 without a missile), missile 1)`; returns 1 on hit |
| `0x005AD730` missile result flags (`0x005AD806`) | `block_or_dodge(avoid 1, block = record physical +0x08 ≠ 0)` |
| `0x0056B9C0` area damage (`0x0056BA0D`) | `block_or_dodge(avoid 1, block 0)` (`missiles/missiles.md`, `area_damage`) |

## Constants & data dependencies

| Item | Value | Where |
|---|---|---|
| hit clamp | 5 … 95 % | `0x0057DB22` |
| block cap | 75 % | `0x0062280E` |
| run block divisor | 3 | `0x0057E011` |
| weapon-block weapon class | 13 (`ht2`) | `0x0057DDB0` |
| prevent-heal duration | 120,000 frames | `0x0057D810` |
| `charstats` | `ToHitFactor` +0x3C (u32): Ama 5, Sor −15, Nec −10, Pal 20, Bar 20, Dru 5, Ass 15; `BlockFactor` +0x49 (u8): 25, 20, 20, 30, 25, 20, 25 | `charstats.txt` 1.14d |
| `monstats` | `NoShldBlock` (flags bit 26), `MonType` | `fields.tsv` |
| stats | 2, 12, 16, 19, 20, 31, 32, 33, 115, 116, 117, 119, 123, 124, 171, 179, 182, 338, 339, 340, 348, 350, 351 | `itemstatcost.txt` |
| states | 52 `preventheal`, 54 `uninterruptable`, 101 `holyshield` | `states.txt` |

Player modes used: 2 walk, 3 run. Monster modes: 2 walk, 15 run
(`sim/units.md`).

## Randomness

Melee (`melee_result`), in order; a draw that is not reached is skipped:

| # | Seed | Form | Range | Decides | When |
|---|---|---|---|---|---|
| 1 | attacker | inline `lo′ mod 100` | 0–99 | hit | defender is not a running player, units in range |
| 2 | defender | inline `lo′ mod 100` | 0–99 | block | hit, and `block_chance > 0` before the /3 |
| 3a | defender | inline `lo′ mod 100` | 0–99 | evade | no block; defender moving; `passive_evade > 0` |
| 3b | defender | `roll(100)` | 0–99 | weapon block | no block; not moving; weapon block > 0; class `ht2` |
| 3c | defender | `roll(100)` | 0–99 | dodge (melee) or avoid (missile) | no block or weapon block; not moving; stat > 0 |

## Edge cases & original bugs

1. A block chance of 1 or 2 for a running player becomes 0 after the
   division but still draws (row 2): the draw count changes, the block
   never happens.
2. `alvl + dlvl = 0` (both levels 0) divides by zero in §3.3. Units in
   play have level ≥ 1; d2rs treats it as a fatal error (reproducing the
   fault is pointless).
3. Players' block can be negative (dexterity below 15 in an expansion
   game); `c ≤ 0` skips the draw.
4. Monster attackers add the skill bonus flat to AR and use 5 × dex with
   no −7 offset (§3.2).
5. A running player defender is hit with no hit test and no draw, but
   still gets the block and dodge steps.
6. The montype loop reads at most 128 `attack_vs_montype` entries; the
   weapon-block loop at most 32.
7. Evade in melee clears the hit with no result flag (§6.3).
8. Weapon block (§6.4): a layer-0 `passive_weaponblock` entry replaces
   the running value instead of taking the maximum, so the result
   depends on entry order. Reproduce.

## Test vectors

Synthetic (CI-safe), from the formulas above:

| Input | Expected |
|---|---|
| player AR 300, `pctAR` 0, def 100, alvl 10, dlvl 8 | factor 75, chance 83 |
| AR 50, def 1000, alvl 1, dlvl 30 | factor 4, chance 0 → 5 |
| AR 200, def −50, alvl 20, dlvl 20 | toHit 250, def 0, factor 100, chance 100 → 95 |
| AR −10, def 40 | toHit 0, def 50, factor 0, chance 5 |
| sum 0 (AR 0, def 0), alvl = dlvl | factor 100, chance 100 → 95 |
| Amazon (BlockFactor 25), toblock 30, dex 100, level 20, expansion | (85 × 55) / 40 = 116 → 75; running: 25 |
| same, classic game | 55 |
| Sorceress (BlockFactor 20), toblock 10, dex 10, level 5, expansion | (−5 × 30) / 10 = −15 → no draw |
| defense: armorclass 100, dex 50, armor% 50 | base 112, bonus 56, 168 |
| defense: armorclass −32, dex 50, armor% 50 | base −20, bonus 10, −10 |
| defense 168, armor_override_percent 10 | 168 + 16 = 184 |
| weapon block: entries (layer 0, 20), (layer 27, 35); right item type 27 | 35 |
| weapon block: entries (layer 27, 35), (layer 0, 20); right item type 27 | 20 (Edge cases 8) |
| weapon block: entry (layer 27, 35); no item of type 27 | 0 |
| draw: seed (1, 0) attacker, chance 83 | step gives lo′ = 0x6AC690C5 = 1791398085; mod 100 = 85 → miss |

Real 1.14d data (`#[ignore]`): `charstats.bin` `ToHitFactor` and
`BlockFactor` per class equal the table above.

## Provenance

- 1.14d `Game.exe` disassembly (`re/exports/all.asm`) for every function
  named above; table contents read from the file image: weapon-class
  code table `0x0072EFA8` (`ht2` → 13), shield-exception code list
  `0x00744580` (`tch `, count 1 at `0x00744584`), monster class jump
  table `0x00622698`/`0x006226A4`, bit-mask table `0x006CE268` (1, 2, 4,
  …).
- Anchors: `0x0057D810` carries D2MOO's `1.14d` comment; it has exactly
  one caller, `0x0057D9B0`, which is the hit test. `0x0057D9B0`'s
  callers include `0x0057EC10` (result flags), which also calls
  `0x0057DFB0` (block or dodge).
- D2MOO 1.10f (`SUnitDmg.cpp`, `Units.cpp`): same structure. 1.14d
  differences: the player AR/def adjustments are a separate function
  (`0x0057D8A0`); the monster block rule is restructured into
  `0x006225F0` with the same outcome; D2MOO's "can never succeed"
  monster check in `GetResultFlags` is `0x005A4EE0` (always true).
- §7 read from `0x00554200`, `0x006221A0`, `0x0055B300`, `0x00650D70`,
  `0x006259B0`, `0x00622C40`, `0x00622870`, `0x006288D0`, `0x0064F380`,
  `0x00463860`; callers by `disasm.py xref 0x57D9B0 0x57DFB0`
  (`0x0057ECCD`, `0x005A88FB`, `0x005AE06F`; `0x0056BA0D`, `0x0057ECE8`,
  `0x005AD806`); 1.14d `monstats.txt` rows 258 / 261, `WeaponClass.txt`
  order (`2ht` = 6 counting `hth` as 0), `states.txt` row 105 and
  `itemstatcost.txt` row 172 (`alignment`).
- charstats values: `game/extracted/patch_d2/.../charstats.txt` (1.14d).

## Open questions

1. No recording confirms the hit chance. Request: hook `0x0057DB61`
   (after the draw; ESI = r, EDI = chance, EBX = attacker, [EBP−0xC] =
   defender) and `0x0057DFB0` entry/return during melee play; compare
   with §3–§6 recomputed from logged stats.
2. Block: hook `0x0057E04B` (EDI = r, EBX = chance, ESI = defender) to
   confirm the /3 rule and the zero-chance draw (Edge case 1).
3. Answered: hostility `0x00554200` is §7.1, melee range `0x00622C40`
   / `0x00622870` are §7.2 / §7.3 (owned here). Still unowned: the
   collision-line test `0x00622AA0` used by §7.2 (owner `sim/units.md`
   per `monsters/ai.md`).
4. Answered: §7.4 lists every caller of `hit_test` and
   `block_or_dodge` with the bonus and flags each passes.
5. Answered (impl-combat item 7): weapon block with no matching entry
   is 0, confirmed at `0x0057DCF8` (`w` starts at 0). The "largest value"
   reading was corrected: layer-0 entries assign unconditionally (§6.4,
   Edge cases 8).
