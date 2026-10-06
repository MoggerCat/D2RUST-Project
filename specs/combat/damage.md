# Spec: Combat — Damage: roll, resistances, application, leech, death trigger

- **Status:** draft: rules read from the 1.14d `Game.exe` disassembly
  (addresses below), including the data tables `0x00732980` (resistance
  rows), `0x007325B0` (event functions), `0x00732B90` (durability
  weights). D2MOO (1.10f) used to name functions; every difference found
  is listed (Provenance). §7 (hit reaction, death) is checked only at the
  call level. No trace check yet (Open questions 1–3).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::combat::damage`
- **Related specs:** `combat/hit.md` (result flags, to-hit, block);
  `skills/use.md` (which skill functions start a combat, the skill's
  `SrcDam`, skill damage put into the record); `skills/levels.md` (skill
  damage and elemental lengths, weapon mastery, the skills formula
  evaluator); `sim/rng.md` (draw helpers; unit seed at unit +0x20);
  `sim/stats.md`, `sim/stat-lists.md` (getters, stat lists, states,
  curses); `sim/units.md` (modes, unit events, overlays); `sim/tick.md`
  §5 (timer events 3 STATREGEN, 12 REMOVESTATE, 2 AITHINK).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 45–57 |
| Inputs | 58–70 |
| Outputs / state changes | 71–78 |
| Rules | 79–80 |
|   0. Shared integer helpers | 81–103 |
|   1. Damage record | 104–137 |
|   2. Pipeline | 138–152 |
|   3. Rolling: `start_combat` = `0x0057DBF0` | 153–292 |
|   4. Totals and resistances: `totals` = `0x0057C1E0` | 293–384 |
|   5. Application | 385–550 |
|   6. Hit class and hit recovery | 551–581 |
|   7. Reaction and death trigger | 582–608 |
|   8. Event functions (table `0x007325B0`, 32 entries) | 609–669 |
|   9. Durability `0x0057D3D0` | 670–689 |
| Constants & data dependencies | 690–711 |
| Randomness | 712–744 |
| Edge cases & original bugs | 745–773 |
| Test vectors | 774–806 |
| Provenance | 807–827 |
| Open questions | 828–860 |
<!-- /index -->

## Summary

A melee attack runs in two moments. At the attack's decision frame the
skill function asks `combat/hit.md` for result flags and, on a hit,
rolls the whole damage record at once and stores it in a combat record
on the attacker (§3–§4). At the damage frame the stored record is
applied: life, mana and stamina loss, leech, stun, cold, freeze, poison,
burn, item events (crushing blow, open wounds, …), durability, then the
defender's reaction (get-hit, block animation, death) (§5–§7). Missiles
and auras apply directly. All life, mana and stamina amounts are fixed
point: 256 units per point. This spec owns the formulas and their draw
order.

## Inputs

| Name | Type | Source |
|---|---|---|
| attacker, defender | units | caller |
| damage record | §1 | caller (skill function pre-fills skill damage, flags, conversion) |
| `SrcDam` | u8, 0–128 (0 means 128) | caller: skills `SrcDam` (record +421), `skills/use.md` |
| stats | i32 | getters (`sim/stats.md`): **unit** `0x00625480`, **item/skill** `0x00625500` |
| `difficultylevels.txt` | `ResistPenalty` +0x00, `MonsterFreezeDivisor` +0x14, `MonsterColdDivisor` +0x18, `LifeStealDivisor` +0x28, `ManaStealDivisor` +0x2C, `HireableBossDamagePercent` +0x38 | record = game difficulty (game +0x6D), `0x00611D30` |
| `monstats.txt` | `Drain`/`(N)`/`(H)` +160–162, `Crit` +166, `ColdEffect`/`(N)`/`(H)` +360–362 (i8), `Velocity` +0x32, flags +0x0C (`inTown` bit 10, `killable` bit 15) | |
| `monstats2.txt` | flags +0x04 (`deadCol` bit 19) | via monstats `MonStatsEx` |
| game | frame (game +0xA8), expansion (+0x70), difficulty (+0x6D) | |

## Outputs / state changes

Defender stats 6 (life), 8 (mana), 10 (stamina); attacker life and mana
(leech); states and stat lists (stun 21, cold 11, freeze 1, poison 2,
burning 115, open wounds 62, shatter 107); timer events 12, 3, 2
(`sim/tick.md` §5); unit events (§5.4); item durability; result flags of
the record; the combat list on the attacker.

## Rules

### 0. Shared integer helpers

**`pct(v, p, d)`** = `0x00483360` (ECX `v`, EDX `p`, stack `d`; D2MOO
`MONSTERUNIQUE_CalculatePercentage`). Signed 32-bit:

1. `d = 0` → 0.
2. `v > 0x100000`: if `d ≤ v >> 4` (arithmetic shift) → `(v / d) × p`;
   else the 64-bit `(v × p) / d`.
3. Else if `p > 0x10000`: if `d ≤ p >> 4` → `(p / d) × v`; else 64-bit
   `(v × p) / d`.
4. Else `(v × p) / d` in 32 bits.

Divisions truncate toward zero; 32-bit products wrap. Cases 2–3 lose
precision on purpose (Test vectors). `combat/hit.md` uses the same
helper.

**Fixed point.** Life, mana, stamina, damage, leech and absorb amounts
are in 1/256 points ("<<8"). Stat values such as `mindamage` are whole
points and are shifted where stated.

**Scaling by `SrcDam`** ("`× s / 128`"): `(x × s) / 128` with a 32-bit
product and truncation toward zero (`cdq; and 0x7F; add; sar 7`).

### 1. Damage record

0x70 bytes (D2MOO `D2DamageStrc`, offsets confirmed by every function
below):

| Off | Field | Meaning |
|---|---|---|
| 0x00 | hit flags (u32) | 1 skip physical roll; 2 skip the whole roll (§3); 4 / 8 / 0x10 life / mana / stamina drain preset (skip monster drain roll); 0x20 set by every roll ("rolled"); 0x80 no `domissiledamage` event; 0x100 / 0x200 / 0x400 bypass undead / demons / beasts; 0x1000 ignore hostility |
| 0x04 | result flags (u16) | 1 hit, 2 will die, 4 get-hit, 8 knockback, 0x10 block, 0x20 "no events", 0x80 dodge, 0x100 avoid, 0x200 evade, 0x2000 critical strike, 0x4000 soft hit, 0x8000 weapon block |
| 0x08 | physical | <<8 |
| 0x0C | enhanced-damage percent | added to the physical roll (§3.2) |
| 0x10 | fire | |
| 0x14 | burn damage | |
| 0x18 | burn length | frames |
| 0x1C | lightning | |
| 0x20 | magic | |
| 0x24 | cold | |
| 0x28 | poison | total over the length |
| 0x2C | poison length | frames |
| 0x30 | cold length | frames |
| 0x34 | freeze length | frames |
| 0x38 | life leech | percent (players) or amount (monsters), §5.3 |
| 0x3C | mana leech | |
| 0x40 | stamina leech | |
| 0x44 | stun length | frames |
| 0x48 | absorbed life | filled by absorbs (§4.4) |
| 0x4C | total | §4.5 |
| 0x54 | pierce percent | of damage reduction, /1024 (§4.1) |
| 0x60 | hit class | low nibble weapon class, high nibble element (§6.1) |
| 0x64 | hit class fixed (u8) | non-zero: do not recompute |
| 0x65 | conversion element (i8) | `elemtypes` index; 0 none |
| 0x68 | conversion percent | |
| 0x6C | overlay | applied to the defender after a melee hit |

### 2. Pipeline

Melee (skill do-functions, `skills/use.md`):

1. `flags = melee_result(…)` (`combat/hit.md` §4), stored at +0x04.
2. `start_combat(game, attacker, defender, record, SrcDam)` (§3,
   `0x0057DBF0`): roll on a hit, then add a copy to the attacker's
   combat list.
3. At the damage frame `apply_melee(game, attacker, defender)`
   (`0x0057D4F0`, §5.1) finds the stored record and applies it.

Missiles, auras and other direct damage call `apply(game, attacker,
defender, missile = 1, record)` (`0x0057C6C0`, §5.2), which computes the
totals itself (§4), then the reaction (`0x0057CEE0`, §7.1).

### 3. Rolling: `start_combat` = `0x0057DBF0`

Fastcall ECX game, EDX attacker; stack defender, record, `SrcDam`.

1. Attacker or defender null: return.
2. If result has hit (1) and none of dodge, avoid, evade, weapon block
   (mask 0x8380):
   1. Unless hit flag 2: `fill(game, attacker, defender, record, 0,
      SrcDam)` (§3.1).
   2. `totals(game, attacker, defender, record)` (§4).
   3. `t = physical + fire + lightning + magic + cold + poison` (+ life
      leech if the attacker is a monster, unit type 1, hireling or not).
      If `(t & ~0xFF) > (life(6) & ~0xFF)` (signed compare): result |= 2.
3. `0x0057CA80`: if the defender is a player or monster, allocate a
   combat record {game, attacker type, attacker GUID, defender type,
   defender GUID, copy of the damage record} and prepend it to the
   attacker's combat list (D2MOO `pCombat`).

#### 3.1 `fill(game, attacker, defender, record, offhand, s)` = `0x0057B7D0`

`s = 0` is treated as 128. In order:

1. `0x00535D10(attacker, offhand)`: dual-wield weapon stat-list
   switching (`sim/stat-lists.md`); undone at step 14 by `0x00535E20`.
2. Hit flags |= 0x20.
3. Attacker is a player, or its alignment (`0x006259B0`) is 2 (good),
   and the defender is a monster:
   1. `t = item_damagetargetac(120)` (item/skill) ≠ 0: set the
      defender's `armorclass(31)` to `max(armorclass + t, 0)`
      (`0x00627260`, a permanent change).
   2. Defender is a demon (`0x0063E940`): `p = item_demondamage_percent
      (121)` (item/skill); `p > 0` → record +0x0C += `p`.
   3. `0x0057B660`: defender undead (`0x0063E990`): `p = 50` if the
      attacker's current weapon (`0x00535BC0`) is of item type 57
      (blunt, `0x00629BB0`), else 0; `p += item_undeaddamage_percent
      (122)` (item/skill); `p > 0` → +0x0C += `p`.
   4. `0x0057B6A0`: for each `damage_vs_montype(180)` entry whose layer
      matches the defender's montype: +0x0C += value (same matching as
      `combat/hit.md` §3.2 step 5).
4. Unless hit flag 1: physical (+0x08) = `bonuses(attacker, get = 1,
   item = none, 0, 0, +0x0C, +0x08, s)` (§3.2). Then critical strike:
   1. If `offhand = 0`, the attacker has a current weapon, and
      `m = weapon_mastery(attacker, weapon, 0, 2)` (`0x00645830`, crit
      mode, `skills/levels.md`) > 0: draw `roll_range(0, 100)` (attacker
      seed, `0x00472280`); `r < m` → crit.
   2. Else (no mastery, or its draw failed): `c =
      passive_critical_strike(337)` (unit getter) > 0: draw; `r < c` →
      crit.
   3. Else: `d = item_deadlystrike(141)` (item/skill) > 0: draw; `r <
      d` → crit.
   Crit: physical × 2 (32-bit), result |= 0x2000.
5. Elements, in this order: fire (+0x10; stats 48/49, mastery 329),
   lightning (+0x1C; 50/51, mastery 330), cold (+0x24; 54/55, mastery
   331), magic (+0x20; 52/53, mastery 357). Each:
   `v = element(maxstat, minstat, mastery, 0, 0, current)` (§3.3); if
   `s ≠ 128`, `v = v × s / 128`. Store.
6. Drains. Attacker is a monster (type 1) and not a hireling
   (`0x0063EE90`):
   - unless hit flag 4: life leech += `element(61, 60, none, 0, 0, life
     leech) × s / 128`;
   - unless 8: mana leech += `element(63, 62, none, …) × s / 128`;
   - unless 0x10: stamina leech += `element(65, 64, none, …) × s / 128`.
   Note the `+=`: the result `old + (old + roll) × s / 128` (Edge case 4).
   Attacker is a player, or a hireling of any type:
   - unless 4: life leech += `lifedrainmindam(60)`; unless 8: mana leech
     += `manadrainmindam(62)` (unit getter, plain percents);
   - hit flags |= 0x100 if `skill_bypass_undead(103)`, 0x200 if
     `skill_bypass_demons(104)`, 0x400 if `skill_bypass_beasts(106)`.
   Other attackers: nothing.
7. Poison: `min = poisonmindam(57)`, `max = poisonmaxdam(58)` (not
   shifted: these stats are already <<8); `m = passive_pois_mastery(332)`
   if `max > 0`, else 0; `v = roll_in_range(attacker, min, max, m, m,
   poison)` (§3.3); poison (+0x28) = `v × s / 128`.
   If poison ≠ 0: `o = skill_poison_override_length(101)`; `o > 0` →
   poison length += `o`; else poison length += `poisonlength(59)`, then
   if `poison_count(326) > 1`, poison length /= `poison_count`.
8. Cold: if cold > 0: cold length += `coldlength(56) × s / 128`.
9. Stun: if stun length = 0: stun length += `stunlength(66) × s / 128`.
10. Burn (1.14d quirk, Edge case 5): `b = burn × s + 316 + roll(1)`
    (one attacker draw that always yields 0); `b = max(b, 0)`; burn
    += `b / 128` (truncating). If burn length ≠ 0: burn length +=
    `firelength(315) × s / 128`.
11. Attacker is a player or a hireling, and result lacks 0x20: unit
    event 3 (`attackedinmelee`) on the defender (§5.4).
12. Conversion: if +0x65 > 0: `c = pct(physical, +0x68, 100)`; physical
    = `max(physical − c, 0)`; element = +0x65; if element = 10
    (`rand`): element = `roll(5) + 1` (attacker seed). Then: 1 fire += c;
    2 lightning += c; 3 magic += c; 4 cold += c and cold length =
    max(cold length, 50); 5 poison += c / 8 (truncating) and poison
    length = max(…, 50); 11 burn: fire += c, burn length = max(…, 50);
    12 freeze: cold += c, freeze length = max(…, 50); 6–9: nothing (jump
    table `0x0057BD94`).
13. Monster critical hit `0x005A5560`: attacker is a monster whose
    `monstats.Crit` (+166, u8) ≠ 0: draw (attacker seed, inline `lo′
    mod 100`); `r < Crit` → physical, fire, lightning, magic, cold and
    poison × 2; then if the defender exists and the hit class high
    nibble is free (`0x00554650(record, 0x10)` sets it), overlay 54 on
    the defender.
14. `0x00535E20(attacker, offhand)` restores step 1.

#### 3.2 `bonuses(unit, get, item, min, max, pct, current, s)` = `0x0057B420`

1. If `get`: if `item` is null, `item` = current weapon. With a weapon:
   wield type (`0x0063D340`) 2 → `min = secondary_mindamage(23)`, `max =
   secondary_maxdamage(24)`; else `min = mindamage(21)`, `max =
   maxdamage(22)`. Without: `min = max(mindamage, 1)`, `max =
   max(maxdamage, 2)`. Both `<<= 8`.
   If not `get`: use the given `min`, `max` as is.
2. `n = item_normaldamage(111) << 8` (item/skill): `min += n`, `max +=
   n`. `min < 1` → 256. `max ≤ min` → `max = min + 256`.
3. `pct += damagepercent(25)`.
4. With an item: `pct += (strength(0) × StrBonus) / 100` if the weapon's
   `StrBonus` (`0x00629860`, i16) ≠ 0; `pct += (dexterity(2) × DexBonus)
   / 100` likewise (`0x006298A0`); `pct += weapon_mastery(unit, item, 0,
   1)` (damage mode). Without an item and `get` set: `pct +=
   strength(0)`.
5. `pct = max(pct, −90)`.
6. If `max > 0`: `minT = min + pct(min, pct + item_mindamage_percent(18),
   100)`; `maxT = max + pct(max, pct + item_maxdamage_percent(17), 100)`;
   `current += minT`; if `maxT > minT`: `current += roll(maxT − minT)`
   (unit seed, `0x0045C3E0`).
7. `current = max(current, 0)`; if `s ≠ 128`: `current = pct(current, s,
   128)`. Return it.

Stats 0, 2, 17, 18, 21–25 use the unit getter.

#### 3.3 Element helpers

`element(maxstat, minstat, mastery, minpct, maxpct, current)` =
`0x0057A8E0` (unit in EDI): `max = stat(maxstat) << 8`; if `max < 8`:
return `max(current, 0)` (no draw). Else `min = stat(minstat) << 8`; if
`mastery ≠ none`: `minpct += stat(mastery)`, `maxpct += stat(mastery)`;
return `roll_in_range(unit, min, max, minpct, maxpct, current)`.

`roll_in_range(unit, min, max, minpct, maxpct, current)` = `0x0057A880`
(D2MOO `SUNITDMG_RollDamageValueInRange`): if `max > 0`: `min +=
pct(min, minpct, 100)`, `max += pct(max, maxpct, 100)`, `current +=
min`, and if `max > min`: `current += roll(max − min)` (unit seed,
`0x0045C3E0`). Return `max(current, 0)`.

### 4. Totals and resistances: `totals` = `0x0057C1E0`

Fastcall ECX game, EDX attacker; stack defender, record. Context:
difficulty record, `att_mon` = attacker is a monster and not a
hireling, `def_mon` = same for the defender.

#### 4.1 Damage reduction

`dr_normal = normal_damage_reduction(34) << 8`; if `> 0` and pierce
percent (+0x54) `> 0`: `dr_normal = pct(dr_normal, +0x54, 1024)`.
`dr_magic` likewise from `magic_damage_reduction(35)`. `dr_none = 0`.
(Defender stats, unit getter.)

#### 4.2 Damage percent `0x0057C060`

Attacker and defender present and different; first match wins:

| Defender | Attacker | Percent |
|---|---|---|
| player | player, hireling, or unit flag 0x80000000 (revived, `0x00451F30`) | 17 |
| hireling | hireling | 25 |
| boss (`0x0063E9F0`) | hireling | `HireableBossDamagePercent` (100/… : Normal 50, Nightmare 35, Hell 25) |
| revived (flag 0x80000000) | prime evil (`0x0063EDC0`) | 200 if the defender is a hireling, else 400 |
| otherwise | | 100 |

If ≠ 100: every row of §4.3 marked *scaled* whose value is > 0 becomes
`pct(value, percent, 100)`.

#### 4.3 Resistance rows (table `0x00732980`, 12 rows of 0x2C bytes)

| # | Field | Resist stat | Max stat | Pierce stat | Absorb % | Absorb flat | DR | Leech row | Scaled |
|---|---|---|---|---|---|---|---|---|---|
| 0 | physical +0x08 | 36 damageresist | — | — | — | — | normal | | yes |
| 1 | fire +0x10 | 39 | 40 | 333 | 142 | 143 | magic | | yes |
| 2 | lightning +0x1C | 41 | 42 | 334 | 144 | 145 | magic | | yes |
| 3 | cold +0x24 | 43 | 44 | 335 | 148 | 149 | magic | | yes |
| 4 | magic +0x20 | 37 | 38 | — | 146 | 147 | magic | | yes |
| 5 | cold length +0x30 | 43 | 44 | 335 | — | — | none | | |
| 6 | freeze length +0x34 | 43 | 44 | 335 | — | — | none | | |
| 7 | poison length +0x2C | 110 poisonlengthresist | — | 336 | — | — | none | | |
| 8 | poison +0x28 | 45 | 46 | 336 | — | — | none | | yes |
| 9 | life leech +0x38 | — | — | — | — | — | none | yes | yes |
| 10 | mana leech +0x3C | — | — | — | — | — | none | yes | yes |
| 11 | stamina leech +0x40 | — | — | — | — | — | none | yes | yes |

#### 4.4 Order

1. Damage percent (§4.2).
2. Unit event 11 (`absorbdamage`) on the defender.
3. `0x0057C140`: if cold length > 0 or freeze length > 0: defender
   `item_cannotbefrozen(153)` (item/skill) → both 0; else
   `item_halffreezeduration(118)` → both halved (truncating). Poison
   length > 0 and defender state 133 (`shrine_resist_poison`) → 0.
   Burn length > 0 and state 131 (`shrine_resist_fire`) → 0.
4. `no_absorb`: defender monster (type 1): set if (undead and hit flag
   0x100) or (demon and 0x200) or (neither and 0x400). Other defenders:
   set if 0x400.
5. Rows 0–11 in order; at the first leech row, stop unless `att_mon`.
   Each row runs §4.6.
6. Total (+0x4C) = physical + fire + lightning + magic + cold + poison,
   plus life leech if `att_mon`.

#### 4.5 Resistance value `0x0057BE00`

1. `r = defender stat(resist)` (0 without a resist stat).
2. Pierce stat present and (`r < 100` or not `def_mon`): `r −=
   attacker stat(pierce)`.
3. Not `def_mon`, and the resist stat is not 36 or 37: expansion game →
   `r += ResistPenalty` (0, −40, −100); classic game → difficulty 1:
   `r −= 20`, difficulty 2: `r −= 50`.
4. `r > 0`:
   - not `def_mon`: cap = 75; with a max stat: `min(stat(max) + 75,
     95)`; without one and resist stat 36: 50. `r = min(r, cap)`.
   - resist stat 36, attacker has state 47 (`sanctuary`), defender is
     undead: `r = 0`.
   `r ≤ 0`: `r = max(r, −100)`.

#### 4.6 Applying a row `0x0057BF80`

1. `v` = field. `v ≤ 0`: field = 0, done.
2. `r` = §4.5.
3. `no_absorb`: `r = min(r, 0)` (keep only negative resists), and no
   damage reduction. Else `v −= DR[row]`.
4. `v > 0` and `r ≠ 0`: `v = pct(v, 100 − min(r, 100), 100)`.
5. Unless `no_absorb`, absorbs (`0x0057BF10`), only for rows with an
   absorb-percent stat: `a = min(stat(absorb%), 40)`; `a > 0` → `x =
   pct(v, a, 100)`, absorbed (+0x48) += `x`, `v −= x`. Then `f =
   stat(absorb flat) << 8`; `f > 0` → `x = min(f, v)`, +0x48 += `x`,
   `v −= x`.
6. Field = `v` (may be negative after damage reduction; later sums use
   it as is).

### 5. Application

#### 5.1 `apply_melee(game, attacker, defender)` = `0x0057D4F0`

1. Find the attacker's combat record for this defender (type and GUID
   match, first in list order). None: return.
2. Copy its damage record.
3. Not in melee range (`0x00622C40(attacker, defender, 1 + 2 ×
   (attacker is a monster))`): free the attacker's combat records for
   this defender (`0x0057C9F0`), return.
4. If the copy has hit (1):
   1. A player or monster attacker in mode 0 (death): free and return.
   2. Hit flags = 0x20 (replaces all); `apply(game, attacker, defender,
      missile = 0, copy)` (§5.2).
   3. Overlay +0x6C > 0 → on the defender (`0x00621E40`).
   4. Hit class: if +0x64 = 0 and the low nibble is 0: OR in the
      attacker's weapon hit class (`0x00623C20`).
   5. Durability (§9).
5. Unit event 7 (`domeleeattack`) on the attacker, then event 3
   (`attackedinmelee`) on the defender.
6. If hit: attacker is an object, or a non-player non-monster, or not
   in mode 0 → thorns (`0x005D10C0`, skills spec).
7. Reaction `0x0057CEE0` (§7.1).
8. Free the combat records for this attacker/defender pair.

#### 5.2 `apply(game, attacker, defender, missile, record)` = `0x0057C6C0`

Fastcall ECX game, EDX attacker; stack defender, missile, record.

1. Hostility (`0x00554200`) fails and hit flag 0x1000 is clear: clear
   result 1 and 2; return.
2. Defender has no room (`0x00620BB0`): same. Room in town
   (`0x0061AB00`): unless the attacker is a monster with `inTown`, clear
   result 2 and return.
3. Defender monster without `killable` (`0x00457490(class, 15)`):
   return. Defender dead (`0x005541B0`): return.
4. Remember the attacker as the defender's last attacker
   (`0x00621D50`).
5. `missile`: `totals(…)` (§4).
6. Result has 1 and lacks 0x20: missile → if hit flag 0x20: unless
   0x80, event 6 (`domissiledamage`) on the attacker; then event 2
   (`damagedbymissile`) on the defender. Melee → event 5
   (`domeleedamage`) on the attacker, event 1 (`damagedinmelee`) on the
   defender.
7. `physical = min(physical, life(6))`.
8. Leech (§5.3).
9. Attacker is a monster and result has 1: `0x005A4390(game, attacker)`
   (monster AI bookkeeping, monsters branch).
10. Absorbed life (+0x48): `heal(defender, absorbed)` (`0x0057A980`,
    §5.3 rule H).
11. Total (+0x4C) > 0: `life = life − total`; `< 256` → 0.
12. Mana leech (+0x3C) > 0: defender `mana −= value`, `< 256` → 0
    (`0x0057AA60`). Stamina leech (+0x40) likewise on stamina
    (`0x0057AAA0`).
13. Stun (§5.5), cold (§5.6), freeze (§5.7), poison, burn (§5.8), in
    that order.
14. `life > 0`: clear result 2; if total ≠ 0 and the defender is a
    monster: if `hpregen(74) ≠ 0`, cancel its type-3 timers and schedule
    type 3 at frame + 1 (`sim/tick.md` §5.2); then `0x005D6410
    (defender)`. `life ≤ 0`: result |= 2.
15. Result lacks 0x20 and has 2: event 10 (`killed`) on the defender,
    then event 9 (`kill`) on the attacker.

#### 5.3 Leech `0x0057C420`

Runs when life leech or mana leech ≠ 0.

1. Drain: defender is a monster → `monstats.Drain[difficulty]` (via the
   monster's data record); `≤ 0` → stop. Other defenders → 100.
2. Life leech and mana leech `<<= 6`.
3. Attacker:
   - player: divide life leech by `LifeStealDivisor` and mana leech by
     `ManaStealDivisor` if non-zero (truncating), then **player rule**;
   - monster that is a hireling → **player rule** (no divisor);
   - other monster, other type, or none: convert its mode
     (`0x00645270`, D2MOO `D2COMMON_11013_ConvertMode`); converted type
     0 → **player rule**, else **monster rule**.
4. **Player rule**, only if physical > 0:
   1. Mana leech ≠ 0: `m = pct(physical, mana leech, 100)`; drain ≠ 100
      → `m = pct(m, drain, 100)`; `add_mana(attacker, m / 64)`
      (`0x0057AA00`, rule M).
   2. Life leech ≠ 0: same with life; `heal(attacker, l / 64)`
      (`0x0057A980`, rule H). Overlay: if mana was leeched too, draw
      `roll(2)` (attacker seed): 0 → overlay 151, else 152; otherwise
      151.
   3. Only mana leeched: overlay 152.
5. **Monster rule**: life leech and mana leech `/= 64` (stored back);
   `L = min(life leech, physical)`, `M = min(mana leech, defender mana)`,
   `S = min(stamina leech, defender stamina)`; `T = L + M + S`; `T = 0` →
   stop; drain ≠ 100 → `T = pct(T, drain, 100)`; `T = min(T, max life −
   life)` of the attacker (`0x00625D10`); `T > 0` → add `T` to its life
   and overlay 151.
6. The leech fields keep their shifted values; step 12 of §5.2 then
   subtracts the (shifted, divided) mana and stamina leech from the
   defender (Edge case 6).

Rule H `heal(unit, x)` = `0x0057A980`: `x ≤ 0`, unit dead, or state 92
(`death_delay`) → 0. Else life = `min(life + x, max life)`; return the
amount added. Rule M `add_mana(unit, x)` = `0x0057AA00`: `x ≤ 0` → 0;
mana = `min(mana + x, max mana)` (`0x00625D60`).

#### 5.4 Unit events

`0x005C0C30(game, event, unit, other, record)` runs the event functions
registered on `unit` for `event` (`events.txt` index: 0 hitbymissile,
1 damagedinmelee, 2 damagedbymissile, 3 attackedinmelee, 4 doactive,
5 domeleedamage, 6 domissiledamage, 7 domeleeattack, 8 domissileattack,
9 kill, 10 killed, 11 absorbdamage, 12 levelup). Item stats register
functions through `itemstatcost` `itemevent1/2` + `itemeventfunc1/2`;
skills register their own. Function table §8. Registration and
iteration order: `sim/units.md` §6.6 (newest first).

#### 5.5 Stun `0x0057AAE0`

Length `n` = stun length; `n ≤ 0` → nothing.

1. Defender is a monster: if unique (`0x005A0180(def, 8)`): draw
   (attacker seed, inline `lo′ mod 100`); `r < 90` → nothing. Then boss →
   nothing; `monstats.Velocity = 0` → nothing; hireling and `n ≥ 13` →
   `n = 13`; else `n = min(n, 250)`.
   Other defenders: `n = min(n, 250)`.
2. End frame `e = frame + n`. If the defender has a stun stat list
   (state 21): set its expiry to `e` and schedule timer 12 at `e`. Else
   create a stat list with state 21, expiry `e`, owner = attacker,
   schedule timer 12 at `e`, attach it and switch state 21 on.

#### 5.6 Cold `0x0057AF80(attacker, length)` (defender in EDI)

1. `length ≤ 0` → nothing.
2. Effect: monster → `monstats.ColdEffect[difficulty]` (i8); others →
   −50. Effect 0 → nothing (no draw).
3. Monster and effect < 0: `length /= MonsterColdDivisor` if non-zero.
4. `length = max(length, 1)`; end `e = frame + length`.
5. No cold stat list (state 11): switch state 11 on, create a list
   (expiry `e`, owner attacker), schedule timer 12 at `e`, set stats
   67 `velocitypercent`, 68 `attackrate`, 69 `other_animrate` = effect,
   refresh animation rate (`0x00623F50`). Existing list: if its expiry
   < `e`, set it to `e` and schedule timer 12 at `e`.
6. Shatter: draw (defender seed, inline `lo′ mod 100`). `r < 20` and the
   defender is a monster without `monstats2.deadCol` → state 107
   (`shatter`) on; otherwise off.

#### 5.7 Freeze `0x0057B230(defender, attacker, length)`

`length ≤ 0` → nothing. Player defender → cold (§5.6) with this
length. Monster: state 54 → nothing; boss, unique (flag 8) or hireling →
cold with this length. Otherwise: cold effect ≥ 0 (`0x0057AF30`) →
nothing; monster: `length /= MonsterFreezeDivisor` (zero is a fatal
assertion). End `e = frame + length`; existing freeze list (state 1)
keeps the later expiry, else a new list with state 1; timer 12 at the
final expiry; cancel the monster's type-2 (AI) timers and schedule type
2 at frame + length + 1. No draws.

#### 5.8 Poison and burn `0x0057AC50`, `0x0057ADD0`

Same shape, poison: state 2, stat `hpregen(74)`; burn: state 115.
`length ≤ 0` or `damage ≤ 0` → nothing. Defender monster: cancel its
type-3 timers and schedule type 3 at frame + 1. End `e = frame +
length`. Existing list for the state: if `−hpregen ≤ damage` (the new
poison is at least as strong), set expiry `e`, schedule timer 12 at
`e`, set `hpregen = −damage`; else nothing. No list: switch the state
on, create one (owner attacker) with `hpregen = −damage`, schedule 12 at
`e`. "damage" here is the record value itself; how `hpregen` drains
life per frame belongs to the regeneration rules (`combat/vitals`, Open
question 8). No draws.

### 6. Hit class and hit recovery

#### 6.1 Element hit class `0x0057CE30`

A process-wide byte counter (`0x0088CAD0`) gives `i = counter mod 4`,
then the counter increments (wrapping at 256). Scan 4 entries of the
table `0x006E16D8` from index `i`: (cold 0x30, fire 0x20, lightning
0x40, poison 0x50, repeated); the first whose damage field is > 0 gives
the element nibble. None and result has 0x2000 → 0x10. Base class 0 →
13 (`over`, the last `HitClass.txt` row).
Result = base | element. Used by §7.1 when the record's class is not
fixed (+0x64 = 0) and has no element nibble.

#### 6.2 Get-hit test `0x0057CB00(unit, record, hitclass)`

Returns true when the unit does **not** enter get-hit:

1. State 1 (`freeze`) → true.
2. Poison ≠ 0 and poison = total → true. Total < 256 → true.
3. `div` by `hitclass − 1` (jump table `0x0057CBF8`): `1hss`, `1ht`,
   `bow`, `xbow` → 8; `2hss`, `club` → 32; `2hsl` → 64; anything else
   (including values with an element nibble) → 16.
4. `M = max life` (`0x00625D10`). `total < M / div` → true.
5. `total < M / (div / 2)` and `mask(2) = 0` (unit's own seed,
   `0x00472210`) → true.
6. `total < M / (div / 4)` and `mask(4) = 0` → true.
7. Monster without the `GH` mode (`0x0046C140(class, 3)`) → true.
   Otherwise false.

Draws 5 and 6 are taken only when their size test passes.

### 7. Reaction and death trigger

#### 7.1 Reaction `0x0057CEE0` (D2MOO `SUNITDMG_ExecuteMissileDamage`)

Checked at the call level only (callees match D2MOO; Open question 3):
town rule as §5.2 step 2; on a hit, compute the hit class (§6.1) unless
fixed, store it on the unit; uninterruptible defender (state 54):
`death_delay` (92) on if result has 2, stop. Player defender: dodge /
avoid → skill mode with the passive's skill; evade → sound only; block
or weapon block → block mode if the last block frame is more than
`item_fasterblockrate(102) / 8 + 15` frames ago; will die → death mode;
knockback → knockback mode; get-hit → get-hit mode unless §6.2 says no
(stun forces it). Monster defender: knockback without the `KB` mode
becomes get-hit; will die → `kill(game, defender, attacker, 1)` (§7.2);
then knockback, block (Diablo and the clone never play it), get-hit
(§6.2), soft hit. Mode changes belong to `sim/units.md`.

#### 7.2 Kill `0x0057CCB0` (D2MOO `SUNITDMG_KillMonster`)

Checked at the call level only: player already in death modes → stop;
monster already dying or dead, or not `killable` → stop; pet kill
credit to a player owner; attacker bookkeeping and arena kill event;
monster: death mode facing the attacker, quest kill parse unless
revived, the act 5 barricade doors (`objCol` monsters open object 571 /
572 at the same spot). Experience distribution, drops and corpse rules
are not specified here (Open question 7).

### 8. Event functions (table `0x007325B0`, 32 entries)

| # | 1.14d | D2MOO name | Used by (`itemstatcost`) |
|---|---|---|---|
| 0 | — | | |
| 1 | `0x005CAB40` | ChillingArmor | skills |
| 2 | `0x005CAC50` | FrozenArmor | skills |
| 3 | `0x005CAD40` | ShiverArmor | skills |
| 4 | `0x005C5F50` | EventFunc04 | skills |
| 5 | `0x005C61E0` | EventFunc05 | skills |
| 6 | `0x005BF670` | AttackerTakesDamage | 78 (damagedinmelee) |
| 7 | `0x005BF960` | Knockback | 81 |
| 8 | `0x005BFA10` | Howl | 112 |
| 9 | `0x005BFAA0` | Stupidity | 113 |
| 10 | `0x005BF720` | AttackerTakesLightDamage | 128 |
| 11 | `0x005BF7D0` | ApplyFireDamage | skills |
| 12 | `0x005BF880` | ApplyColdDamage | skills |
| 13 | `0x005BFBB0` | DamageToMana | 114 |
| 14 | `0x005BFC60` | Freeze | 134 |
| 15 | `0x005BFE60` | OpenWounds | 135 |
| 16 | `0x005BFFC0` | CrushingBlow | 136 |
| 17 | `0x005C0130` | ManaAfterKill | 138 |
| 18 | `0x005C0260` | HealAfterDemonKill | 139 |
| 19 | `0x005C0310` | Slow | 150 |
| 20 | `0x005C0470` | SkillOnAttackHitKill | 195, 196, 198 |
| 21 | `0x005C05A0` | SkillOnGetHit | 201 |
| 22 | `0x005C6370` | EventFunc22 | skills |
| 23 | `0x005C6530` | EventFunc23 | skills |
| 24 | `0x005CA6C0` | EnergyShield | skills |
| 25 | `0x005C87A0` | EventFunc25 | skills |
| 26 | `0x005C6690` | EventFunc26 | skills |
| 27 | `0x005C6760` | EventFunc27 | skills |
| 28 | `0x005C01D0` | HealAfterKill | 86 |
| 29 | `0x005C0770` | RestInPeace | 108 |
| 30 | `0x005C0670` | SkillOnDeathLevelup | 197, 199 |
| 31 | `0x005C09E0` | Reanimate | 155 |

Signature (D2MOO): (game, event, attacker, unit, record, stat id <<16 |
layer, level); the chance stat is read with the item/skill getter from
`(arg >> 16, arg)`. Two are specified here; the rest are Open question 5.

**Crushing blow (16)**: `c` = chance stat; `c ≤ 0` → 0. Draw (attacker
seed, inline `lo′ mod 100`); `r ≥ c` → 0. Divisor: player defender 10;
monster: hireling 10; else 4, or 8 if boss or superunique (flag 2); then
`h = 0x005738F0(max(monster_playercount(100), 1))` (player-count life
bonus, monsters branch) and `div += pct(div, h, 100)` if `h ≠ 0`. Other
defenders 4. Event 6 (missile) doubles it. `x = life / div`; `dr =
min(damageresist(36), 100)`; `dr > 0` → `x −= pct(x, dr, 100)`. Life =
`max(life − x, 0)`; life ≤ 0 → record result |= 2; `x > 0` → overlay 147.
Return 1.

**Open wounds (15)**: chance and draw as above. `lvl = max(attacker
level(12), 1)`; `h = ow(lvl) + 40` with `ow` (`0x005BFDA0`, values
{9, 18, 27, 36, 45}): `lvl ≤ 1` → 0; ≤ 15 → 9 × (lvl − 1); ≤ 30 → 18 ×
(lvl − 15) + 14 × 9; ≤ 45 → 27 × (lvl − 30) + 15 × 18 + 14 × 9; ≤ 60 →
36 × (lvl − 45) + 15 × (18 + 27) + 14 × 9; else 45 × (lvl − 60) + 15 ×
(18 + 27 + 36) + 14 × 9. Player defender: `h /= 4`, and event 6 → `h /=
2`; monster that is unique or champion (flag 0xC) → `h /= 2` (all
truncating). Curse (`0x0056E970`): state 62, stat 74 = `−h`, 200
frames, skill 0, level 1, owner attacker.

### 9. Durability `0x0057D3D0`

1. Attacker is a player with a current weapon: `durability_hit(game,
   attacker, weapon)` (`0x00559E30`).
2. Defender is a player with an inventory: candidates from the weight
   table `0x00732B90` (7 rows, count at `0x00732BC8`): (bodyloc 1 head,
   3), (3 torso, 5), (4 right arm, 4), (5 left arm, 4), (8 belt, 2), (9
   feet, 2), (10 gloves, 2); keep each slot holding an item of type 50
   (any armor); `W` = sum of kept weights. `W > 0`: `i = roll(7)`, `w =
   roll(W)` (defender seed). Loop: if slot `i` is kept: `w < weight` →
   `durability_hit(game, defender, item)` and stop; else `w −=
   weight`. `i = (i + 1) mod 7`; continue while `w ≥ 0`.

`durability_hit(game, owner, item)` = `0x00559E30`: items of type 50
(armor) or 45 (weapon) with durability (`0x00629930`); chance 10 % for
armor; for others 4 %, or (if `0x0062BA80(item)`) 10 % in an expansion
game and never in a classic game. Draw (owner seed, inline `lo′ mod
100`); `r < chance` → durability − 1 and the rest of the item rules
(items spec).

## Constants & data dependencies

| Item | Value |
|---|---|
| PvP damage percent | 17 |
| hireling vs hireling | 25 |
| prime evil vs revived | 400 (200 vs hireling) |
| resist caps | 75 (+ max stat, ≤ 95); damage resist 50; floor −100; no cap for monsters |
| classic penalty | Nightmare −20, Hell −50 (expansion: `ResistPenalty`) |
| absorb % cap | 40 |
| stun cap | 250 frames (hirelings 13); unique monsters stunned 10 % |
| shatter | 20 % |
| leech shift | 6 (×64) |
| hit-class divisors | 8 / 16 / 32 / 64 |
| crushing blow divisors | 4 (monsters), 8 (bosses, superuniques), 10 (players, hirelings), ×2 for missiles |
| open wounds | base 40, values 9/18/27/36/45, 200 frames |
| burn quirk | +316, roll(1) |

`difficultylevels.txt` 1.14d: ResistPenalty 0/−40/−100;
MonsterFreezeDivisor 1/2/4; MonsterColdDivisor 1/2/4; LifeStealDivisor
1/2/3; ManaStealDivisor 1/2/3; HireableBossDamagePercent 50/35/25.

## Randomness

One melee hit with a weapon, in order (all on the attacker's seed
unless stated; a step not reached does not draw). Draws of
`combat/hit.md` come first.

| # | Seed | Form | Range | Decides | Condition |
|---|---|---|---|---|---|
| 1 | attacker | `roll(maxT − minT)` | | physical | §3.2: `maxT > minT` |
| 2 | attacker | `roll_range(0, 100)` | 0–99 | mastery crit | offhand 0, weapon, mastery crit > 0 |
| 3 | attacker | `roll_range(0, 100)` | 0–99 | critical strike | 2 not taken or failed; stat 337 > 0 |
| 4 | attacker | `roll_range(0, 100)` | 0–99 | deadly strike | 2–3 not taken or failed; stat 141 > 0 |
| 5–8 | attacker | `roll(max − min)` | | fire, lightning, cold, magic | max stat << 8 ≥ 8 and range > 0 |
| 9–11 | attacker | `roll(…)` | | monster life, mana, stamina drain | non-hireling monster, flag clear, stat ≥ 8 |
| 12 | attacker | `roll(max − min)` | | poison | max > 0, range > 0 |
| 13 | attacker | `roll(1)` | 0 | nothing (burn quirk) | always |
| 14 | — | event functions of event 3 | | e.g. none for most items | player or hireling attacker |
| 15 | attacker | `roll(5)` | 0–4 | random conversion element | conversion type 10 |
| 16 | attacker | inline `mod 100` | 0–99 | monster crit | `monstats.Crit ≠ 0` |
| — | | (`totals`: no draws; event 11 functions may draw) | | | |
| 17 | — | event functions 5 / 1 (melee) | | crushing blow, open wounds, … | at the damage frame |
| 18 | attacker | `roll(2)` | 0–1 | leech overlay 151/152 | player rule, both leeches |
| 19 | attacker | inline `mod 100` | 0–99 | stun on unique | unique monster, stun length > 0 |
| 20 | defender | inline `mod 100` | 0–99 | shatter | cold applied with effect ≠ 0 (also via freeze) |
| 21 | — | event functions 10 / 9 | | kill effects | will die |
| 22 | defender | `roll(7)`, `roll(W)` | | armor piece | defender player, `W > 0` |
| 23 | owner | inline `mod 100` | 0–99 | durability loss | per `durability_hit` |
| 24 | — | events 7 / 3 | | skill on attack, … | |
| 25 | defender | `mask(2)`, `mask(4)` | | get-hit | §6.2 size tests |

Draw 23 occurs first for the attacker's weapon, then for the chosen
armor piece (order of §9).

## Edge cases & original bugs

1. `pct` loses precision above 0x100000 (value) or 0x10000 (percent)
   (Test vectors). Reproduce.
2. `item_damagetargetac` permanently lowers the monster's base
   `armorclass` on every rolled hit (§3.1 step 3.1).
3. Physical damage is capped at the defender's current life before leech
   (§5.2 step 7), so leech from an overkill hit uses the capped value.
4. Monster drains (§3.1 step 6): 1.14d adds the scaled roll to the old
   value: `old + (old + roll) × s / 128`; D2MOO 1.10f reads
   `(old + roll) × (1 + s / 128)`. The record normally arrives with 0.
5. Burn: every roll adds `(burn × s + 316) / 128` to the burn damage (2
   when burn is 0) and steps the attacker's seed once (`roll(1)`). The
   constants are the stat ids 316/317 (`burningmin/max`) compiled as
   values. Burn length stays 0 unless set by the skill, so the burn
   state is not applied by plain attacks.
6. Player leech fields keep `percent << 6 / divisor`; §5.2 step 12 then
   subtracts that number (in 1/256 points) from the defender's mana
   (e.g. 5 % mana steal on Normal removes 320/256 mana).
7. The element hit-class counter `0x0088CAD0` is process-wide, shared by
   every game on the server. d2rs keeps one counter per server process
   for fidelity (Open question 6).
8. The get-hit divisor uses the full hit class, so any hit with an
   element nibble uses divisor 16.
9. A hit that only dodged/avoided/evaded/weapon-blocked is not rolled
   and its combat record holds no damage.
10. Leech requires physical > 0 for player attackers; monster leech
    uses the amounts themselves.

## Test vectors

Synthetic (CI-safe):

| Input | Expected |
|---|---|
| `pct(0x200000, 50, 100)` | `(2097152 / 100) × 50` = 1,048,550 (exact 1,048,576) |
| `pct(1000, 0x20000, 100)` | `(131072 / 100) × 1000` = 1,310,000 |
| `pct(0x200000, 50, 0x30000)` | 64-bit: 34 |
| `pct(−50, 30, 100)` | −15 |
| `pct(7, 9, 0)` | 0 |
| `bonuses`: weapon 2–7 (min/max stats 2, 7), StrBonus 100, str 30, no other stats, s 128, current 0 | `pct` 30; min 512 → 665; max 1792 → 2329; result 665 + roll(1664) |
| no weapon, mindamage 0, maxdamage 0, str 15 | min 256, max 512, pct 15: 294 + roll(294) (588 − 294) |
| `pct` floor: enhanced −120 | treated as −90 |
| fire 10–20 (stats 10, 20), mastery 50, s 128, current 0 | min 3840, max 7680, 3840 + roll(3840) |
| same, s 64 | (3840 + roll) × 64 / 128 |
| resist: player, fire res 80, max fire 0, Hell expansion (−100) | r = −20 → fire damage × 120 / 100 |
| player, fire res 200, max fire 10, Normal | cap 85 → × 15 / 100 |
| monster fire res 120, pierce 30 | r ≥ 100: no pierce; 120 → capped 100 → 0 damage |
| monster fire res 90, pierce 30 | 60 → × 40 / 100 |
| physical 2560, normal DR 3, damage resist 20 (player) | (2560 − 768) × 80 / 100 = 1433 |
| fire 2560, absorb % 50, absorb flat 2 | % capped 40: absorbed 1024 → 1536; flat 512 → 1024; +0x48 = 1536 |
| leech: player, life steal 5, physical 2560, Normal, monster Drain 100 | `pct(2560, 320, 100)` = 8192; / 64 = 128 (0.5 life) |
| same on Hell vs `Drain(H)` 50 | 320 / 3 = 106; `pct(2560, 106, 100)` = 2713; × 50 % = 1356; / 64 = 21 |
| hit recovery: M = 25600, total 1700, class `1hss` (div 8) | 1700 < 3200 → no get-hit (no draw) |
| M = 25600, total 4000, class `hth` (div 16) | ≥ 1600, < 3200? no; < 6400: draw `mask(4)` |
| crushing blow on a normal monster, life 25600, damage resist 0, players 1 | x = 6400 |
| open wounds, attacker level 20, monster | ow = 18 × 5 + 126 = 216; h = 256 |
| open wounds, level 70, player defender, missile | 45 × 10 + 15 × 81 + 126 = 1791; +40 = 1831; /4 = 457; /2 = 228 |

Real 1.14d data (`#[ignore]`): the resistance rows of §4.3 equal
`itemstatcost` ids by name; `difficultylevels.bin` values above.

## Provenance

- 1.14d `Game.exe` disassembly (`re/exports/all.asm`) for every address;
  tables read from the file image (`.data`): `0x00732980` (12 × 0x2C;
  row fields offset, resist, max, pierce, absorb %, absorb, DR type,
  leech row, scaled flag; matches D2MOO `sgDamageStatTable` exactly),
  `0x007325B0` (event functions), `0x00732B90` (durability weights),
  `0x0057BD94` (conversion jump table), `0x0057CBF8` (hit-class
  divisors), `0x006E16D8` (element hit classes). Damage-record offsets
  agree with D2MOO `D2DamageStrc` in every access read.
- Anchors: D2MOO's `1.14d` comments on `0x0057CEE0` and `0x0057D810`;
  call graph from `combat/hit.md` (`0x0057DBF0` is the 34-caller
  function that calls `0x0057B7D0` and `0x0057C1E0`).
- Differences from D2MOO 1.10f found in 1.14d: monster drains (Edge case
  4); burn implemented (D2MOO TODO); shatter is `r < 20` and *not*
  `deadCol` (D2MOO reads `r ≥ 20` and `deadCol`); stun rules restructured
  (the unique 90 % gate applies to monsters only); the element hit-class
  counter is a byte.
- `elemtypes.txt`, `events.txt`, `HitClass.txt`, `difficultylevels.txt`,
  `itemstatcost.txt` (1.14d `patch_d2`) for indices and values.

## Open questions

1. No trace confirms any number here. Request (recording): hook
   `0x0057DBF0` entry and exit (EBX record before/after: +0x08…+0x4C) and
   `0x0057C6C0` entry/exit (defender life stat 6 before/after) during
   melee and missile play; log the attacker's and defender's seeds at
   entry. Compare with §3–§5 recomputed from logged stats.
2. Leech: hook `0x0057C420` entry (EAX record, EBX attacker, [EBP+8]
   defender) and the calls `0x0057A980` / `0x0057AA00` (amounts) to
   confirm rules H/M and Edge case 6.
3. §7.1 / §7.2 were checked only by callee sets. Ghidra request: read
   `0x0057CEE0` (size 1,254) and `0x0057CCB0` (375) branch by branch
   against §7, especially the block-animation frame test and the soft-hit
   path.
4. Answered: unit-event registration and iteration order
   (`0x005C0C30`, `0x005C0AD0`) are `sim/units.md` §6.6.
5. Event functions other than 15 and 16 (table §8): behaviour and draws
   unspecified. Ghidra request: each address in §8.
6. Hosted games: confirm the element hit-class counter is shared across
   games (it is a single global byte at `0x0088CAD0`).
7. Experience on kill (`0x0057E2F0` level-difference scaling with tables
   `0x006E1668` / `0x006E1694`, 11 entries each; D2MOO
   `SUNITDMG_ComputeExperienceGain`), party sharing, drops and player
   death penalties: not specified; owner to be decided (vitals or a
   death/experience spec).
8. How `hpregen` (stat 74) from poison, burn and open wounds removes life
   each frame (regeneration, timer type 3): `combat/vitals.md`, not yet
   written.
9. `0x0062BA80` (durability predicate for non-armor items) and
   `0x00629930` (has durability): items spec.
10. `0x005D6410` (called after a non-lethal hit on a monster) and
    `0x005A4390` (after a monster's hit): monsters branch.
