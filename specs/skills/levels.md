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
| Summary | 41–53 |
| Inputs | 54–64 |
| Outputs / state changes | 65–69 |
| Rules | 70–71 |
|   1. Skill level | 72–133 |
|   2. Special values | 134–179 |
|   3. Skill damage | 180–300 |
|   4. Mana cost | 301–340 |
|   5. To-hit | 341–348 |
|   6. Learning a skill | 349–382 |
|   7. Skill stat callbacks | 383–542 |
| Constants & data dependencies | 543–565 |
| Randomness | 566–577 |
| Edge cases & original bugs | 578–600 |
| Test vectors | 601–652 |
| Provenance | 653–689 |
| Open questions | 690–721 |
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
   `[0x0096C8A8]` (`0x00611830(0)`), 99 in 1.14d, for every unit.
   `[0x0096C8A8]` is the `experience` table (32-byte records: 7 class
   columns, `ExpRatio` at +0x1C), written only by its loader
   (`0x00613D30`, store at `0x00613E46`); record 0 is the `MaxLvl` row,
   so the cap is `experience.txt` `MaxLvl`, Amazon column.
   `0x00611830(c)` reads column c (c outside 0…6 → column 0).

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
them; entries with base 0 are created by the stat 97 / 107 callback
(§7.1). On the server the level bonus +0x2C is always 0: entries are
allocated zeroed (`0x00647110`) and its only writers, set `0x00647AA0`
and add `0x00647B20`, are called from client code alone (`0x004C6140`,
`0x004D88A0`, `0x004C7990`; `client/msg-skills.md` Open question 2).

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
   (`0x0063D340`): `w` = the weapon's `secondary_mindamage(23)` (max:
   `secondary_maxdamage(24)`), total of the item's own stat list
   (item +0x5C, `0x00624F60`, `sim/stats.md`) through
   `0x00625EF0(item, two)` / `0x00625E60(item, two)` (two ≠ 0 → stat
   23 / 24, two = 0 → 21 / 22; no item or list → 0; called here with 1
   and 0x18); else `w = mindamage(21)` (max: `maxdamage(22)`) of the
   unit; `s = (SrcDam × w) / 128`.
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
  "Hit class h" is a plain u32 store: record +0x60 := h, replacing
  whatever was there (not or-ed, not a nibble); e = 3, 6, 7, 8, 11 and
  physical leave +0x60 unchanged. The value goes to its field by `+=`;
  the lengths by `:=` (except stun: length += v + len). `len ≤ 0` → 50
  applies only on the e = 10 path; a given e uses `len` as passed.
  1.14d-confirmed (asm of `0x0056C8E0`).

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
- **Blood-mana payment** `pay_with_life(unit, c)` = `0x005D2B60` (ECX
  unit, EDX c in 1/256 points; returned by `consume_mana`, ignored by
  the aura tick `0x0056C110`, `bodies.md` §4.5 step 6). L = the unit's
  state-114 list (`0x006256B0(unit, 114)`), k = L's skill id (list
  +0x1C, `0x006260E0`; no null test: the callers test the state first).
  1. life total (stat 6, `0x00625480(unit, 6, 0)`) < c (signed): L, if
     any, is detached and freed (`sim/stat-lists.md` §8.2, §8.3 plain
     free `0x00626CD0`; the state's remove callback ends `blood_mana`),
     then base life := 256 (1 point; set `0x00627260(unit, 6, 0x100,
     0)`); return 0 (nothing paid, the skill fails).
  2. Else base life += −c (`0x006272B0(unit, 6, −c, 0)`). Then, with
     k's skills record (k outside 0…count − 1 → none; a none record is
     read anyway, a fault: unreachable since the list always carries
     the casting curse's id): life total < `Param5` (+0x158) << 8 → L,
     if any, is detached and freed as in step 1 (the curse breaks once
     life falls under `Param5` points). Return 1.
  In 1.14d the only state-114 skill is Blood Mana (id 310, `Param5` =
  40), so the curse ends when the cursed unit's life drops below 40.
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

Replies and results (`0x0054BD90`): validator 2 (bad id) and step 3
(at max level) send message 0x21 to the player's client (`0x0053C4A0`,
skill 0, base level 1, remove 0: the client re-assigns Attack at level
1, `client/msg-skills.md` §4) and return 2; validator 3 sends nothing
and returns 3; a failed spend (step 4) sends nothing and returns 2;
success returns 0. The dispatcher ignores the result
(`sim/intents-events.md` §2.2 rule 5).

### 7. Skill stat callbacks

The server stat callback `0x0055B800` (`sim/stat-lists.md` §7.2) runs,
for these stats, a skill or state handler (jump table `0x0055BDD8`,
index bytes `0x0055BE04`, by stat − 7). Arguments: ECX game, EDX unit
(the list's owner), the propagation unit I (`sim/stat-lists.md` §7.1;
the item whose stats propagate, or none), the stat id with its layer
(low 16 bits = layer), old and new value. G := I's GUID (unit +0x0C),
0 when there is no I. The handlers do not test new ≠ old.

| Stat | Handler |
|---|---|
| 97 `item_nonclassskill`, 107 `item_singleskill` | §7.1 |
| 83 `item_addclassskills`, 188 `item_addskill_tab` | §7.2 |
| 126 `item_elemskill`, 127 `item_allskills` | refresh all (§7.2) |
| 98 `state` | §7.3 |
| 151 `item_aura` | §7.5 |
| 204 `item_charged_skill` | §7.6 |

#### 7.1 Oskill entries (stats 97, 107)

s = the layer.

1. s = 0 → nothing. Stat 107 only: the unit must be a player whose
   class (unit +0x04) equals s's `charclass` (+0x0C); else nothing.
   Stat 97 has no class test.
2. E = the native entry of s (owner −1, `0x006439B0`). None: E = add
   (`0x00647110`, `client/msg-skills.md` §2 rule 1: a new entry, base
   1), then assign (s, level 0, remove 0) (`0x00647280`, same §2 rule
   2.3: base := 0), then message 0x21 (`0x0053C4A0`, skill s, remove 0,
   level argument 0; layout `client/msg-skills.md` §4) to the unit's
   client: a player's own client (`0x005531C0`); otherwise none given,
   and the sender uses the owning player's client of a monster
   (`0x0058F0D0`) and sends nothing for other units.
3. Refresh (`0x00646D60(unit, s)`), then the pet maximum of s (§7.4).
4. New value > 0 → done. Else r := 1; if `skill_level(unit, E, 0)` ≠ 0
   (hard points): r := 0, and `skill_level(unit, E, 1)` ≠ 0 → done.
   (With base 0 the entry is removed even when other bonuses still give
   it a level.)
5. The left skill is E (`0x00620190`) → select Attack (skill 0, owner
   −1) on the left (`0x005701B0`, EDX 1); the right skill is E
   (`0x006201D0`) → Attack on the right (EDX 0).
6. r ≠ 0: assign (s, level 0, remove 1) (`0x00647280`, `client/msg-
   skills.md` §2 rule 2.2: the native entry is removed). No message is
   sent for the removal.

#### 7.2 Class and tab bonuses (stats 83, 188)

c = the layer (stat 83) or layer >> 3 (stat 188). Refresh all when the
unit is a player of class c, or a hireling (`0x0063EE90`), or its draw
identity (`0x00645270(unit, &type, &class, &mode)` on unit type, class
and mode +0x10; `render/unit-composite.md` §1.1) is type 0 with class c;
else nothing.

**Refresh all** `0x0056DFA0` (ECX unit; needs a skill list): for each
entry in list order (`0x00643910`, next `0x006438F0`) whose skill has a
`passivestate` p > 0 (`0x00643690`): state p on (`0x00639DB0(unit, p,
1)`), refresh (`0x00646D60(unit, skill)`).

#### 7.3 Item states (stat 98)

s = the layer; s outside 0…states count − 1 → nothing. New ≠ 0: the
unit has s → nothing; else clear s's group keeping s, state s on
(`0x00639DB0(unit, s, 1)`). New = 0: the unit lacks s → nothing; else
clear s's group including s, state s off (`0x00639DB0(unit, s, 0)`).
Both then mark s for update (`0x00639E30(unit, s, 1)`).

**Clear group** `0x0056C740(ECX unit, EDX s, incl)`: g = `states`
`group` (u16, +0x1E) of s; g = 0 → return 0. For each state i in
0…count − 1 in order (i = s skipped when incl = 0) with group g that
the unit has: state i off, its state list (`0x006256B0`) detached and
freed (`sim/stat-lists.md` §8.2, §8.3). Return 1 when any was cleared.

#### 7.4 Pet maximum of one skill `0x0056BD90`

ECX game, EDX unit, stack skill s. Players only; s valid; t = s's
`pettype` (+0xBE, i8) with 0 < t < pettype count, and its row P
(`0x00478A20`); else nothing. m := 0. For each skill k of P's skill
list (count +0xBC, u16 ids from +0xC0): with a record and an entry E =
`highest_entry(unit, k)` (`0x006439F0`): L = `skill_level(unit, E, 1)`;
v = L > 0 ? max(`eval(petmax` (+0xC0)`, k, L)` (`0x00646CA0`), 1) : 0;
m := max(m, v). Then `set_max(game, unit, t, m)` (`0x00575850`,
`sim/pets.md` §4); m = 0 removes every pet of type t (t ≠ 7).

P's skill list is built after loading (`0x00613F80`, at `0x00617BB2`):
every `skills` row in id order whose `pettype` p is in 0…count − 1 is
appended to row p's list while it holds fewer than 15. In 1.14d
`pettype.bin` leaves these bytes unwritten (`data/tables.tsv`).

#### 7.5 Item auras (stat 151)

s = the layer, L = new. New ≠ 0 → aura on `0x005BF510(game, unit, G,
s, L)`; new = 0 → aura off `0x005BF5D0(game, unit, G, s)`. Both do
nothing unless: unit present; s in 0…skills count − 1 with a record;
s has `aura` (flags bit 5, mask byte `0x006CE27C`); its `aurastate`
(i16 +0x80) is in 0…states count − 1.

**On:**
1. Cancel the unit's type-9 timers whose arg1 is G, all type-9 timers
   when G = 0 (`0x00540E60(unit, 9, G)`, `sim/tick.md` §5.4).
2. Schedule a type-9 timer (arg1 G, arg2 s) at `period(s, L)`
   (`0x0056CD50`, `use.md` §7; `0x005417D0`, `sim/tick.md` §5.2). Its
   handler re-applies the aura each period (`sim/stat-lists.md` §10.3).
3. s has `immediate` (flags bit 15, mask byte `0x006CE284`): do core
   `0x0056F7F0(game, unit, s, L, charge 1, item 1, aim 0)` (`use.md`
   §5.4) now.

**Off:**
1. State `aurastate` off (`0x00639DB0(unit, state, 0)`); its state list
   (`0x006256B0`), if any, detached and freed (`0x006277E0`,
   `0x00626CD0`; `sim/stat-lists.md` §8.2, §8.3).
2. Cancel type-9 timers as in On step 1.

Consequences: a level change (new ≠ 0 after new ≠ 0) restarts the
period and, for `immediate` skills, re-applies at once. Two items with
the same aura: removing one turns the state off and cancels only its
own timer; the other's timer re-applies at its next period (total of
151 still > 0). A null I (G = 0) cancels every type-9 timer of the unit.

1.14d data: 26 `skills.txt` rows have `aura`; 10 also `immediate`
(Might, Resist Fire, Defiance, Resist Cold, Blessed Aim, Resist
Lightning, Concentration, Vigor, Fanaticism, Salvation). Sword, Axe
and Mace Mastery have `aura` but no `aurastate`: the handler ignores
them. Oak Sage, Wolverine and Barbs Aura have no `perdelay`: period 5.

#### 7.6 Item charged skills (stat 204)

Needs I present; otherwise nothing. s = layer >> shift, l = layer &
mask (shift, mask: data +0xC6C / +0xC70; `items/properties.md` §5 rule
9). c := (I's own total of 204 at this layer (`0x00625480(I, 204,
layer)`)) & 0xFF: the current charges. The value argument is not read.

1. c > 0 and I's stats are attached to the unit (`0x00625820(I)`: I's
   list's parent is extended and owned by the unit): **set charges**
   (unit, G, s, l, c).
2. Else: **remove charges** (unit, G, s, l), then the pet maximum of s
   (§7.4).

`0x00647320(unit, G, s, l, c, remove)` (also via `0x00647530`). Does
nothing unless: unit is a player (type 0); l > 0; s in 0…count − 1
with a record; the unit has a skill list. Entries match on skill and
owner GUID (+0x34) = G; the level is not compared.

- **Set** (remove = 0): the first matching entry in list order gets
  base (+0x28) := l, charges (+0x38) := c, has-charges (+0x3C) := 1.
  None: a new 0x40-byte zeroed entry, skill s, mode := `anim` when it
  is 7 A1, 8 A2, 10 SC, 11 TH or 18 SQ, else 10 SC (jump table
  `0x00647518`, index bytes `0x00647520`); base l, charges c,
  has-charges 1, owner G; appended at the tail.
- **Remove** (remove = 1): old left / right read first. Left has skill s
  and owner G → left := Attack (skill 0, owner −1, `0x00643BC0`). Right
  has skill s and owner G → right := Attack (`0x00643C50`). Current
  skill (+0x10) has skill s (owner not compared) → current := none.
  Then the first matching entry is unlinked and freed.

Neither path turns passive states on or off, refreshes (`0x00646D60`)
or sends a message. Monsters and hirelings never get charge entries.
A charge used up (current 0) removes the entry, and the selected
left / right falls back to Attack.

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

Synthetic `pay_with_life` (Blood Mana, `Param5` 40 → threshold 10,240):
life 100 pt (25,600), c = 1,280 → life 24,320, curse kept, 1; life 45
pt (11,520), c = 1,280 → 10,240, kept (not below), 1; c = 1,536 →
9,984 < 10,240, curse removed, 1; life 4 pt (1,024), c = 1,280 → curse
removed, base life := 256, 0.

Synthetic stat callbacks (§7.5, §7.6; 1.14d `skills.txt` rows): item
GUID 7 gives 151 Might (98) = 5 at frame 1000 → type-9 timers with arg1
7 cancelled, a type-9 timer (7, 98) at frame 1001 (`perdelay` 50:
((1000 + 49) / 50) × 50 + 1), do core now (`immediate`); the same with
Prayer (99) → timer only, no do. 204 on GUID 7 with layer (54 << shift)
+ 3 and item value 0x0F0A (max 15, current 10), item attached → entry
(Teleport, owner 7) base 3, charges 10, has-charges 1; value 0x0F00 →
entry removed, left / right on it → Attack. A monster owner → no entry.

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
- `pay_with_life` read from `0x005D2B60`–`0x005D2C1D` (callers
  `0x0056C0CA`, `0x0056C131`); 1.14d `skills.txt` row Blood Mana (310):
  `auratargetstate` `blood_mana`, `Param5` 40; `states.txt` row 114
  `blood_mana` is referenced by no other skill row.
- §7.5 read from `0x005BF510`, `0x005BF5D0`, `0x00540E60`; §7.6 from
  `0x0055BCF9`–`0x0055BD7B`, `0x00625820`, `0x00647320` (jump table
  `0x00647518` / `0x00647520` decoded from the image); flag masks from
  the bit table at `0x006CE268` (bit n at +4n) and `data/fields.tsv`
  (`aura` bit 5, `immediate` bit 15); counts from 1.14d `patch_d2`
  `skills.txt`.
- §7 read from `0x0055B800` (jump table decoded from the image),
  `0x0056DFA0`, `0x0056C740`, `0x0056BD90`, `0x00617BB2`–`0x00617C17`;
  §1 cap from `0x00613D30` (strings `experience`, `ExpRatio` in the
  image); §6.4 replies from `0x0054BD90`; +0x2C writers found by a scan
  of `all.asm` for stores to +0x2C in `0x00643000`–`0x00648FFF`.
- DM `b < a` count: scan of the 1.14d `patch_d2` `skills.txt`,
  `skilldesc.txt` and `missiles.txt` calc cells.

## Open questions

1. No trace check. Request (recording): hook `0x00646460` and
   `0x00644D50`/`0x00644E40` entry/return during play with known skill
   levels; and `0x0056BFE0` (mana before/after) for a few skills.
2. Answered: `[0x0096C8A8]` is the `experience` table; record 0 is
   `MaxLvl` (§1 rule 3).
3. Answered: the server never writes +0x2C (§1, after `bonus_level`);
   oskill entries are created and removed by the stat 97 / 107
   callback (§7.1).
4. Answered: `0x0056C8E0` is `add_element` (§3.6).
5. Answered: results of the 0x3B handler and the 0x21 reply: §6.4.
6. Answered: the raw getter is `0x00643890(entry)` (entry → record,
   `range` i8 +0x14; no direct caller in the export); the effective
   range used by the game is `0x00645460(unit, entry)` (`skills/use.md`
   §3 step 6: value 3 resolves to 1 or 2).
7. Partly answered: `0x00625EF0` / `0x00625E60` are specified in §3.3;
   `0x0063D340` (grip / wield type) is owned by
   `render/unit-composite.md` and `combat/damage.md`. Open:
   `0x00623990(unit, 0)` (the weapon a skill uses: skill weapon kind
   via `0x00644140` +0x168, hands 4 / 5 through `0x0063C050`, item type
   45 tests `0x00629BB0`, `0x0062A4E0`): owner `items/inventory.md`.
8. Answered: blood-mana life payment `0x005D2B60` is specified in §4
   (`pay_with_life`).
9. Answered: no 1.14d user has `b < a`. `skills.txt` uses dm 86 times
   (dm12 28, dm34 37, dm56 21, dm78 0) and `skilldesc.txt` 44 times,
   all with `Param_b ≥ Param_a`; no 1.14d `missiles.txt` calc uses a
   missile DM (sd12, sd34, cd12, cd34, shd1, chd1, dd12). The
   overshoot stays reproduced for mod data.
10. Answered: stat 151 `item_aura` is §7.5, stat 204
    `item_charged_skill` is §7.6.
