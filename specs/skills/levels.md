# Spec: Skills — Level, special values, skill damage, mana cost, to-hit

- **Status:** draft: every rule read from the 1.14d `Game.exe`
  disassembly and image (addresses below); D2MOO (1.10f) used to name
  functions, with six differences found (Provenance). Worked examples
  computed from the 1.14d `skills.txt`. No trace check yet (Open
  question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::levels` (level, special values,
  damage, mana); evaluator entry points in `d2-sim::calc`
- **Related specs:** `data/calc-expressions.md` (bytecode, evaluator,
  context; §3.5 points here for the special values); `skills/use.md`
  (when mana is checked and charged, which roll a skill function calls);
  `combat/damage.md` (what a rolled skill damage does; `pct` helper §0);
  `combat/hit.md` (skill to-hit as the hit-test bonus); `sim/stats.md`
  (getters). Machine tables: `skills/skillcalc.tsv` (73 skills special
  values), `skills/misscalc.tsv` (43 missile special values).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–52 |
| Inputs | 53–63 |
| Outputs / state changes | 64–68 |
| Rules | 69–70 |
|   1. Skill level | 71–125 |
|   2. Special values | 126–171 |
|   3. Skill damage | 172–282 |
|   4. Mana cost | 283–304 |
|   5. To-hit | 305–312 |
|   6. Learning a skill | 313–340 |
| Constants & data dependencies | 341–363 |
| Randomness | 364–375 |
| Edge cases & original bugs | 376–398 |
| Test vectors | 399–435 |
| Provenance | 436–455 |
| Open questions | 456–477 |
<!-- /index -->

## Summary

A skill's numbers (damage, length, mana, to-hit, aura and passive
values) come from the `skills.txt` record at a level: the unit's hard
points plus item and shrine bonuses (§1). Formulas reach those numbers
through *special values* (`ln12`, `dm34`, `edmn`, …; §2), which also
serve skill descriptions. Damage uses level brackets, `HitShift`
fixed point, synergy percents from formulas, and elemental masteries
(§3). Mana cost is linear in level with a shift (§4); to-hit has a
formula or a linear fallback (§5); learning a skill checks level, skill
and attribute requirements (§6). Nothing here draws randomness except
the two damage rolls (§3.6) and `rand()` inside formulas.

## Inputs

| Name | Type | Source |
|---|---|---|
| unit | player or monster, may be null for special values | caller |
| skill id, level | i32 | caller; level from §1 or the formula context |
| `skills.txt` record (0x23C bytes) | §Constants | `data/fields.tsv` |
| `skilldesc.txt` | `skillpage` +2 (i8), `descmissile1..3` +0x3C/+0x3E/+0x40 | via skills `skilldesc` (+0x194) |
| `missiles.txt` record (0x1A4) | `misscalc.tsv` offsets | |
| stats | unit getter `0x00625480`, base getter `0x006253B0`, item/skill getter `0x00625500` | `sim/stats.md` |

## Outputs / state changes

Values only, except: the two rolls step the unit seed (§3.6), the
skill-point message (§6.4) changes stat 5 and the skill list.

## Rules

### 1. Skill level

Skill list entry (D2MOO `D2SkillStrc`; 1.14d offsets): +0x28 base (hard
points), +0x2C level bonus, +0x34 owner GUID (−1 = native skill), +0x38
charges, +0x3C has-charges flag (new in 1.14d, set by the set-charges
function `0x00643B70`).

**`skill_level(unit, entry, bonus)`** = `0x006442A0` (D2MOO
`SKILLS_GetSkillLevel`):

1. No unit or no entry: 0.
2. `L = base`. If `bonus ≠ 0` and owner GUID = −1: `L += bonus_level
   (unit, skill)`. Item-granted (charge) entries never get bonuses.
3. Clamp `0 ≤ L ≤ cap`, cap = entry 0 of the max-level table
   `[0x0096C8A8]` (`0x00611830(0)`), 99 in 1.14d, for every unit
   (Open question 2).

`bonus = 1` means "with bonuses" (answers `data/calc-expressions.md`
Open question 8): the formula function `skill(s, c)` passes 1; the
special value `blvl` and the skill-point handler pass 0.

**`bonus_level(unit, skill)`** = `0x00644180` (D2MOO
`SKILLS_GetBonusSkillLevel`; unit in ESI). All stats through the unit
getter with the stated layer:

1. Skill record missing or id out of range: 0.
2. `b` = entry level bonus (+0x2C) + shrine bonus (`0x00644150`: 2 if
   state 134 `shrine_skill`, else 0) + `item_allskills(127)`.
3. Player whose class equals `skills.charclass` (i8, +0x0C):
   1. `+ item_addclassskills(83)` [layer = class].
   2. If `skilldesc.skillpage` ≠ 0: `+ item_addskill_tab(188)` [layer =
      page + 8 × class − 1]. The skilldesc record is not null-checked
      (Edge case 4).
   3. `n = item_nonclassskill(97)` [layer = skill]; `n > 3` → 3
      (negative values pass); `+ n`.
4. Player, skill of another class: `+ item_nonclassskill(97)` [skill],
   uncapped.
5. Non-player (or no unit): base ≤ 0 → `+ item_nonclassskill(97)`
   uncapped; else `n` = that stat, `n ≠ 0` → `+ min(n, 3)`.
6. `skills.EType` (u8, +0x1DC) ≠ 0: `+ item_elemskill(126)` [layer =
   EType].
7. Return `b + item_singleskill(107)` [layer = skill].

There is no "base > 0" condition: any native entry gets bonuses. Class
skills without hard points have no entry, so `+skills` cannot reach
them; entries with base 0 come from the level bonus (+0x2C) path (Open
question 3).

**`highest_entry(unit, skill)`** = `0x00643810` (wrapper `0x006439F0`):
walk the unit's skill list (unit +0xA8 → +0x04 first, +0x04 next);
consider entries with this id and has-charges flag 0. Take the first; a
later native entry always replaces the current; once a native entry is
held, item entries are skipped; otherwise an item entry with a strictly
higher base replaces the current one.

### 2. Special values

**Skills** `special(unit, c, skill, lvl)` = `0x00646460` (stdcall, `c` a
byte; D2MOO `SKILLS_GetSpecialParamValue`): skill id out of range → 0;
`c > 72` → 0; otherwise per `skills/skillcalc.tsv` (jump table
`0x00646ABC`, 73 entries). The unit may be null. `lvl` is the caller's
level as given.

**Missiles** `miss_special(missile_unit, owner, c, missile, lvl)` =
`0x0064B340`: missile unit present and not a missile (type 3) → 0; bad
missile id → 0; otherwise per `skills/misscalc.tsv` (jump table
`0x0064B62C`, 43 entries).

Building blocks used by the TSVs:

- **Linear** `lnXY = ParamX + (lvl − 1) × ParamY`, i32 wrapping; `lvl ≤ 0`
  → 0 (skills).
- **Diminishing** `DM(lvl, a, b)` = `0x00645B20`:
  `q = (110 × lvl) / (lvl + 6)`; `r = (q × (b − a)) / 100`; `v = a + r`;
  `v > b` → `b`. Signed 32-bit, truncating. Only the upper clamp exists.
  This order differs from D2MOO's `(b − a) × 110 × lvl / (lvl + 6) /
  100`: Lower Resist `dm56` (25, 70) is 31 at level 1 and 62 at level 20
  in 1.14d (D2MOO order: 32, 63). The skills wrappers dm12/34/56
  (`0x00645B70`/`BC0`/`C10`) return 0 for `lvl ≤ 0`; dm78 and all
  missile DMs have no guard.
- **Calc-backed** indices (skills 54–72) evaluate the column through
  `eval(unit, field, skill, lvl)` = `0x00646CA0` (D2MOO
  `SKILLS_EvaluateSkillFormula`): field −1, no buffer, or offset ≥ buffer
  size (unsigned) → 0; otherwise `data/calc-expressions.md` §3 with the
  context (unit, skill, level) and parameter callback `0x00646BE0`
  (`special` of the context skill at the context level).
- `mastery(type)` = `0x00647E00`: `lvl ≤ 0` → 0; stat pair by type 0
  {342, 345}, 1 {343, 346}, 2 {344, 347}; return `eval(passivecalc_i)`
  for the first `i` in 1…5 whose `passivestat_i` (i16) is either stat;
  none → 0.

Formula functions (`data/calc-expressions.md` §3.4), 1.14d:

- `skill(s, c)` = `0x00646C00`: `L = skill_level(unit,
  highest_entry(unit, s), 1)`, or 0 with no unit or no entry; result
  `special(unit, c, s, L)`. **Correction to `calc-expressions.md`
  §3.5**: with no entry the result is the special value at level 0, not
  0 (e.g. `par8` still returns Param8).
- `sklvl(s, c1, c2)` = `0x00646C60`: `special(unit, c2, s, special(unit,
  c1, ctx skill, ctx level))`.

### 3. Skill damage

**Level brackets** `bracket(lvl, L1…L5)` = `0x00644B70`: `lvl ≤ 1` → 0;
2–8 `(lvl−1)L1`; 9–16 `7L1 + (lvl−8)L2`; 17–22 `7L1 + 8L2 + (lvl−16)L3`;
23–28 `… + 6L3 + (lvl−22)L4`; 29+ `… + 6L4 + (lvl−28)L5`.

`pct` below is `combat/damage.md` §0 (`0x00483360`).

#### 3.1 Elemental min / max

`elem_min(unit, skill, lvl, mastery)` = `0x00644D50`; `elem_max` =
`0x00644E40` (D2MOO `SKILLS_GetMin/MaxElemDamage`):

1. Bad skill or `lvl ≤ 0` → 0.
2. `v = (EMin + bracket(lvl, EMinLev1…5)) << HitShift` (max: `EMax`,
   `EMaxLev`).
3. Synergy: `EDmgSymPerCalc ≠ −1` → `p = eval(EDmgSymPerCalc)`; `p ≠ 0`
   → `v += pct(v, p, 100)`. **Min only**: applied only if `v > 256` or
   `EMinLev1 ≠ 0` (Edge case 1).
4. `mastery ≠ 0` and a unit: `v += pct(v, stat, 100)`, stat by EType
   (byte table `0x00644D38`, jump `0x00644D24`): 1 fire → 329, 2
   lightning → 330, 4 cold → 331, 5 poison → 332, 12 freeze → 331, any
   other → none. Magic mastery (357) is not read here.

#### 3.2 Elemental length

`elem_len(unit, skill, lvl)` = `0x00644F20`: `lvl ≤ 0` → 0. `L` = `lvl ≤
8`: `(lvl−1)LL1`; `≤ 16`: `7LL1 + (lvl−8)LL2`; else `7LL1 + 8LL2 +
(lvl−16)LL3` (`ELevLen1…3`). `v = ELen + L`; `ELenSymPerCalc ≠ −1` and
`p ≠ 0` → `v += pct(v, p, 100)`. Frames.

#### 3.3 Physical min / max

`phys_min(unit, skill, lvl, use_srcdam)` = `0x00647BC0`; `phys_max` =
`0x00647D00`:

1. Bad skill → 1 (max: 2), unshifted.
2. `Kick` flag (flags bit 9): `x` = player ? `dex(2) + str(0) − 20` : `3
   × level(12)` (also with no unit); `x = max(x, 1)`; min = `(x / 4) <<
   8`, max = `(x / 3) << 8` (`0x00644C20`).
3. Otherwise: `s = 0`; if `SrcDam ≠ 0` (u8, +0x1A5) and `use_srcdam`:
   weapon `0x00623990(unit, 0)`; with a weapon and wield type 2
   (`0x0063D340`): `w` = weapon min `0x00625EF0(item, 1)` (max:
   `0x00625E60(item, 24)`); else `w = mindamage(21)` (max:
   `maxdamage(22)`); `s = (SrcDam × w) / 128`.
   `v = s + MinDam + bracket(lvl, MinLevDam1…5)` (max: MaxDam,
   MaxLevDam). `DmgSymPerCalc ≠ −1` and `p ≠ 0` → `v += pct(v, p, 100)`.
   Return `v << HitShift`. No `lvl ≤ 0` guard; synergy before the shift;
   no mastery.

#### 3.4 Missile damage

`0x0064AF20` / `0x0064AFF0` (physical min/max), `0x0064B100` /
`0x0064B1D0` (elemental), `0x0064B2A0` (length): a missile unit that is
not type 3 → 0; missile id < 0 with a unit → the unit's class; `lvl ≤ 0`
with a unit → the missile's stored level (data +0x0C, u16). `v = base +
bracket(lvl, LevDam)`; synergy (`DmgSymPerCalc` +0xE0 or
`EDmgSymPerCalc` +0x118 ≠ −1) through the missiles evaluator
`0x0064B7C0` → `v += pct(v, p, 100)`; return `v << missiles.HitShift`
(+0x196). Synergy applies before the shift (skills elemental: after);
no gate, no mastery. Length: `lvl ≤ 0` → ELen; same 8/16 brackets; no
synergy.

#### 3.5 Helpers for skill functions

- Weapon mastery `weapon_mastery(unit, item, skill, type)` =
  `0x00645830` (type 0 to-hit, 1 damage, 2 crit; used by
  `combat/hit.md` and `combat/damage.md`): null unit or item → 0; null
  skill → the used skill (`0x00620250`). Throw path `0x00645720` when
  `0x0062BA80(item)`, the skill's `itypea1` is-a 48 (`thro`) and range =
  2 (stats 345/346/347); otherwise stats 342/343/344. Result = max(0,
  values of up to 32 copied entries `0x006261D0` whose layer is an item
  type the item is, `0x00629BB0`).
- Concentration `0x006461D0`: state 42 present → `(damagepercent(25)
  of that state's list × Param1) / 8`.
- Kick damage `0x00646280` (unit, &min, &max, &pct): `k =
  item_kickdamage(137)` (item/skill); `*min += k`, `*max += k`; weapon
  stat lists toggled off (`0x00627910`); boots (bodyloc 9): `bmin = k +
  items byte +0xFE`, `bmax = k + byte +0xFF`; `p = pct(str, StrBonus,
  100) + pct(dex, DexBonus, 100) + damagepercent(25)`, at least −90;
  `bmin ≥ bmax` → `bmax = bmin`; `*pct += maxdamage_percent(17) + p`;
  `*min += bmin`; `*max += bmax`; only the left weapon list is re-merged
  (Edge case 6).

#### 3.6 Damage rolls

- `roll_physical(unit, record, skill, lvl)` = `0x0056E170` (D2MOO
  `D2GAME_RollPhysicalDamage_6FD14EC0`, 10 callers): `a = phys_min(…,
  0)`, `b = phys_max(…, 0)`; record physical (+0x08) += `a + roll(b −
  a)` (unit seed, `0x0045C3E0`).
- `roll_elemental(unit, record, skill, lvl)` = `0x0056E0C0` (37
  callers): `len = elem_len`, `a = elem_min(…, 1)`, `b = elem_max(…,
  1)` (formulas evaluated in this order), `v = a + roll(b − a)`; then
  `0x0056C8E0(unit, record, EType, v, len, 0, 0)` puts `v` and `len`
  into the record by element (below). Returns `v`.
- `add_element(unit, record, e, v, len, &res, &e_out)` = `0x0056C8E0`
  (ECX unit, EDX record; `ret 0x14`): e = 10 (random): one step of the
  unit's seed, e = {1, 2, 4, 5}[`lo' & 3`] (table `0x006E1278`), `len ≤
  0` → 50, and `e_out` (if given) := e. Then by e (`combat/damage.md`
  §1 offsets): 1 fire += v, hit class 0x20, res 39; 2 lightning += v,
  0x40, res 41; 3 magic += v, res 37; 4 cold += v, cold length := len,
  0x30, res 43; 5 poison += v, poison length := len, 0x50, res 45; 6
  life leech += v; 7 mana leech += v; 8 stamina leech += v; 9 stun
  length += v + len, 0x60; 11 burn += v, burn length := len; 12 cold +=
  v, freeze length := len, 0x30, res 43; any other e (0, after the
  remap never 10): physical += v. `res` (if given) := the resist stat,
  −1 for 6–9, 11 and physical. Jump table `0x0056CA0C`.

`roll(n)` with `n < 1` does not step (`sim/rng.md` §3). Rolled values
are below the maximum by at least one 1/256 point.

### 4. Mana cost

- **Cost** `0x00644B10(skill, lvl)` = `(mana + lvlmana × (lvl − 1)) <<
  manashift`, 1/256 points; `mana`, `lvlmana` are i16 sign-extended,
  `manashift` the low byte. No level check, no `minmana`.
- **Shifted** `0x006459F0` = `max(cost >> 8, 0)` (arithmetic); an
  invalid skill is a fatal assertion.
- **Afford** `can_afford(unit, skill)` = `0x00647540`: charge entry
  (owner ≠ −1) → charges > 0. Else `n = max(skill_level(unit, entry, 1)
  − 1, 0)`, `c = (mana + lvlmana × n) << manashift` (no `minmana`);
  state 114 (`blood_mana`) → `life(6) ≥ c`; else `srvdofunc` = 116
  (were-forms) and `0x0063A400(unit)` → true; else `mana(8) ≥ c`.
- **Consume** `consume_mana(game, unit, skill, lvl)` = `0x0056BFE0`
  (D2MOO `D2GAME_SKILLMANA_Consume`): non-player or null → 1; the used
  skill is this id and item-owned → charges path `0x0056BEC0`; bad id →
  0; `mana = 0` and `lvlmana = 0` → 0; `c = (mana + lvlmana × max(lvl −
  1, 0)) << manashift`, then `c = max(c, minmana(i16) << 8)` (signed);
  `blood_mana` → pay with life (`0x005D2B60(unit, c)`); else `mana < c`
  → 0, nothing deducted; else `mana −= c` → 1. (D2MOO's "cost 0 at level
  1" does not hold in 1.14d.) When it runs: `skills/use.md`.
- There is no mana-cost-reduction stat in 1.14d.

### 5. To-hit

`to_hit(unit, skill, lvl)` = `0x006449F0` (D2MOO
`SKILLS_GetToHitFactor`): `lvl ≤ 0` or bad skill → 0; `ToHitCalc ≠ −1`
→ `eval(ToHitCalc)` (an invalid offset gives 0, not the fallback); else
`ToHit + (lvl − 1) × LevToHit`. The result is the hit-test bonus of
`combat/hit.md` §3.

### 6. Learning a skill

1. `max_level(skill)` = `0x004AA8B0`: `maxlvl` (i16) if > 0, else 20.
2. `req_level(unit, skill)` = `0x00644750`: `reqlevel` + base of the
   native entry; bad id → 0x7FFFFFFF.
3. `0x006447D0` (required skills): `level(12) ≥ req_level`; each
   `reqskill1…3 ≥ 0` must exist, be in the unit's list (`0x006446C0`)
   with native base > 0.
4. `0x00644920` (attributes): `InGame` flag (bit 10) set; `level(12) ≥
   req_level`; `reqstr ≤ str(0)`, `reqdex ≤ dex(2)`, `reqint ≤
   energy(1)`, `reqvit ≤ vitality(3)` (unit getter; requirements i16).

#### 6.4 Message 0x3B AddSkillPoint (`0x0054BD90`)

1. Size 3; skill = u16 at +1.
2. Validator `0x00549490`: id out of range → 2; not a class skill of the
   player (`0x0056C700`) → 3; §6 step 3 or 4 fails → 3.
3. Native entry with `skill_level(…, 0) ≥ max_level` → 2.
4. Spend `0x00570080`: cost 1 if `skpoints` = −1, else `eval(skpoints,
   unit, skill, skill_level(entry, 1) or 0)` (empty in every 1.14d
   row). Base `newskills(5)` (base getter) < cost → nothing. Else stat
   5 −= cost; add a level (`0x00647110`); refresh (`0x00646F20`);
   failure refunds; success toggles the passive state
   (`0x00643690` → `0x00639DB0`) and calls `0x00646D60`.
5. The handler then calls `0x0055F4F0(…, 1)` and `0x0056DE40(unit)`.

Return codes 2/3 and their messages to the client: Open question 5.

## Constants & data dependencies

`skills` record offsets (1.14d, confirm `data/fields.tsv`): charclass
+0x0C; flags +0x04 (Kick bit 9, InGame 10, usemanaondo 37); range
+0x14 (u8: 0 none, 1 h2h, 2 rng, …); maxlvl +0x12C; calc1–4
+0x138…+0x144; Param1–8 +0x148…+0x164; auralencalc +0x60;
aurarangecalc +0x64; aurastatcalc1–6 +0x68…+0x7C; passivestat1–5 (i16)
+0x98…; passivecalc1–5 +0xA4…+0xB4; petmax +0xC0; skpoints +0x170;
reqlevel/str/dex/int/vit +0x174…+0x17C; reqskill1–3 +0x17E…+0x182;
minmana +0x186; manashift +0x188; mana +0x18A; lvlmana +0x18C; skilldesc
+0x194; ToHit/LevToHit/ToHitCalc +0x198/+0x19C/+0x1A0; HitShift +0x1A4;
SrcDam +0x1A5; MinDam/MaxDam +0x1A8/+0x1AC; MinLevDam1–5 +0x1B0;
MaxLevDam1–5 +0x1C4; DmgSymPerCalc +0x1D8; EType +0x1DC; EMin/EMax
+0x1E0/+0x1E4; EMinLev1–5 +0x1E8; EMaxLev1–5 +0x1FC; EDmgSymPerCalc
+0x210; ELen +0x214; ELevLen1–3 +0x218; ELenSymPerCalc +0x224.

1.14d `skills.txt` (357 rows): EDmgSymPerCalc 64 rows, DmgSymPerCalc 6,
ELenSymPerCalc 4, ToHitCalc 3, SrcDam 63, skpoints 0; HitShift 8 in
316 rows, 7 in 20; special-value codes used: blvl 275, lvl 157, ln12
140, par8 118, ln34 78, dm34 37, dm12 28, dm56 21, mana 13, ulvl 10,
toht 4, edmn 4. Negative `lvlmana`: Magic Arrow, Guided Arrow,
Dopplezon, Teleport, Bone Prison, Summon Resist, Double Swing.

## Randomness

| Site | Seed | Helper | Range | Decides | Skipped |
|---|---|---|---|---|---|
| `0x0056E170` | rolling unit (+0x20) | `roll(n)` `0x0045C3E0` | `n = max − min` | physical | `n < 1` |
| `0x0056E0C0` | rolling unit | `roll(n)` | `n = max − min` | elemental | `n < 1` |
| `0x0056C8E0` | rolling unit | inline step, `lo' & 3` | 4 | element of EType 10 (random), after the roll | e ≠ 10 |

Formula `rand()` draws on the context unit's seed
(`data/calc-expressions.md` §Randomness); 1.14d uses it once (Imp
Inferno `calc1`). Everything else here draws nothing.

## Edge cases & original bugs

1. Elemental-min synergy gate (§3.1 step 3): skills with `EMin ≤ 1` point
   and `EMinLev1 = 0` keep their minimum without synergy (1.14d rows:
   Power Strike, Lightning Bolt, Charged Strike, Lightning Strike,
   Lightning Fury, Lightning, Chain Lightning, Holy Shock, Shock Field,
   Charged Bolt Sentry, Claws of Thunder, Lightning Sentry, Death
   Sentry).
2. `m2eo`/`m2ey` read `descmissile1` and `me3o`/`me3y` read
   `descmissile2` (index arithmetic); no 1.14d formula uses them.
3. dm78 and missile DMs: `lvl = −6` divides by zero; d2rs treats it as a
   fatal error.
4. A class skill with no skilldesc record faults in `bonus_level`.
5. `can_afford` ignores `minmana`; `consume_mana` applies it. A cost ≤ 0
   passes the check; a failed consume is ignored by its callers
   (`skills/use.md`).
6. Kick: `item_kickdamage` counted twice with boots; only the left
   weapon's list is re-merged.
7. `toht` inside its own `ToHitCalc` recurses without bound (no 1.14d
   row).
8. Physical damage of an invalid skill is 1–2 unshifted; elemental 0.
9. The skill-level cap is the class-0 maximum (99) for every unit.

## Test vectors

Real 1.14d `skills.txt` values (game-file tests, `#[ignore]`), computed
with the rules above:

| Case | Stored (1/256) | Displayed |
|---|---|---|
| Fire Bolt L1 (EMin 6, EMax 12, HitShift 7) | 768–1536 | 3–6 |
| Fire Bolt L20 | 11,648–15,488 | 45–60 |
| Fire Bolt L20, synergy p = 80 | 20,966–27,878 | 81–108 |
| Fire Bolt L1, p = 80 | 1,382–2,764 | 5–10 |
| Fire Ball L1 / L20 | 1,536–3,584 / 51,072–57,984 | 6–14 / 199–226 |
| Frozen Orb L1 / L20 | 10,240–11,520 / 67,072–70,784 | 40–45 / 262–276 |
| Frozen Orb cold length L1 / L10 / L20 | 200 / 425 / 675 frames | |
| Tornado L1 / L20 / L20 with p = 90 | 6,400–8,960 / 69,888–75,520 / 132,608–143,360 | |
| Lightning L20 min, p = 64 | 256 (gate) | 1 |
| Mana: Fire Bolt (5, 0, shift 7) L1 | 640 | `mana` 2 |
| Frozen Orb (50, +1, shift 7) L1 / L10 / L20 | 6,400 / 7,552 / 8,832 | |
| Teleport (24, −1, shift 8) L1 / L10 / L20 | 6,144 / 3,840 / 1,280 | |
| Teleport L25: `usmc` / consume / shifted | 0 / 256 (minmana 1) / 0 | |
| Teleport L30: `usmc` / `mana` / consume | −1,280 / −5 / 256 | |
| dm12 Critical Strike (5, 80) L1/2/5/10/20/30/99 | 16/25/42/56/68/73/80 | |
| dm12 Dodge (10, 65) L1 / L20 | 18 / 56 | |
| dm56 Lower Resist (25, 70) L1 / L20 | 31 / 62 | |
| Amplify Damage ln34 (200, 75) L1 / L10 | 200 / 875 frames | |
| Sacrifice to-hit (ToHit 20, LevToHit 7) L1 / L20 | 20 / 153 | |
| Bash damage % ln12 (50, 5) L1 / L20 | 50 / 145 | |
| Kick, player str 100, dex 60 | 8,960–11,776 | 35–46 |

Synthetic: `DM(1, 25, 70)`: q = 110/7 = 15, r = 15 × 45 / 100 = 6, 31.
`bracket(29, 1, 2, 3, 4, 5)` = 7 + 16 + 18 + 24 + 5 = 70. Mastery and
synergy use `pct` (`combat/damage.md` vectors).

Mechanical check (M05, to add with the code): `skillcalc.tsv` and
`misscalc.tsv` index/code columns equal the 1.14d `skillcalc.bin` /
`misscalc.bin` code order (`data/calc-expressions.md` §5).

## Provenance

- 1.14d `Game.exe` disassembly (`re/exports/all.asm`) for each address;
  jump tables `0x00646ABC` (skills, 73), `0x0064B62C` (missiles, 43),
  `0x00644D38`/`0x00644D24` (mastery by EType); skills formula function
  table `0x00745774` (slot 3 `skill`, slot 6 `sklvl`); parameter
  callback `0x00646BE0` (bytes after the jump table, not in the Ghidra
  function list). Anchors: `0x00646460` and `0x0064B340` from
  `data/calc-expressions.md` Open question 5; `0x00483360` shared with
  `combat/damage.md`.
- D2MOO 1.10f (`D2Common/src/D2Skills.cpp`, `D2Game/src/SKILLS/Skills.cpp`)
  for names. 1.14d differences: DM operand order; `mps` multiplies the
  whole sum by 25; `m?rn` use `lvl` and have no guard; dm78 unguarded;
  has-charges flag +0x3C skipped by `highest_entry`; consume charges
  from level 1; physical damage synergy uses `pct`; invalid skill in
  shifted cost is fatal.
- 1.14d `skills.txt`, `skilldesc.txt`, `missiles.txt` (`patch_d2`) for
  counts and examples; 13,720 sampled (level, params) cases differ
  between the 1.14d and D2MOO DM orders.

## Open questions

1. No trace check. Request (recording): hook `0x00646460` and
   `0x00644D50`/`0x00644E40` entry/return during play with known skill
   levels; and `0x0056BFE0` (mana before/after) for a few skills.
2. Max-level table `[0x0096C8A8]` (`0x00611830`): confirm it is filled
   from `experience.txt` `MaxLvl`. Ghidra: find writers of `0x0096C8A8`.
3. Who writes the entry level bonus (+0x2C) and how oskill entries are
   created. Ghidra: writers of `[entry+0x2C]` near `0x00643000`–
   `0x00648FFF` (D2MOO `D2Common_11030/11031`).
4. Answered: `0x0056C8E0` is `add_element` (§3.6).
5. Results 2/3 of the 0x3B validator and what the client sees:
   `sim/intents-events.md`.
6. Skills `range` getter (D2MOO `SKILLS_GetRange`) not located;
   `0x00645720` reads +0x14 = 2 directly.
7. `0x00623990`, `0x0063D340`, `0x00625EF0`, `0x00625E60` (weapon,
   wield type, item damage getters) named from D2MOO use: stats/items
   specs.
8. Blood-mana life payment `0x005D2B60`: rule unspecified.
9. DM with `b < a` overshoots below `b`; whether any 1.14d dm user has
   `Param_b < Param_a` was not measured.
