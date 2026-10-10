# Spec: Combat — Damage: roll, resistances, application, leech, death trigger

- **Status:** draft: rules read from the 1.14d `Game.exe` disassembly
  (addresses below), including the data tables `0x00732980` (resistance
  rows), `0x007325B0` (event functions), `0x00732B90` (durability
  weights). D2MOO (1.10f) used to name functions; every difference found
  is listed (Provenance). §7 (hit reaction, death) is checked only at the
  call level. The monster-melee-on-a-player path (§10) is checked
  against a 1.14d recording, roll by roll (Test vectors "Recorded",
  2026-10-09); the rest has no trace check yet (Open questions 1–3).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::combat::damage`
- **Related specs:** `combat/hit.md` (result flags, to-hit, block);
  `skills/use.md` (which skill functions start a combat, the skill's
  `SrcDam`, skill damage put into the record); `skills/levels.md` (skill
  damage and elemental lengths, weapon mastery, the skills formula
  evaluator); `sim/rng.md` (draw helpers; unit seed at unit +0x20);
  `sim/stats.md`, `sim/stat-lists.md` (getters, stat lists, states,
  curses); `sim/units.md` (modes, unit events, overlays); `sim/tick.md`
  §5 (timer events 3 STATREGEN, 12 REMOVESTATE, 2 AITHINK); `combat/events.md`
  (event functions other than 15 and 16).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 49–61 |
| Inputs | 62–74 |
| Outputs / state changes | 75–82 |
| Rules | 83–84 |
|   0. Shared integer helpers | 85–107 |
|   1. Damage record | 108–141 |
|   2. Pipeline | 142–156 |
|   3. Rolling: `start_combat` = `0x0057DBF0` | 157–302 |
|   4. Totals and resistances: `totals` = `0x0057C1E0` | 303–404 |
|   5. Application | 405–605 |
|   6. Hit class and hit recovery | 606–636 |
|   7. Reaction and death trigger | 637–742 |
|   8. Event functions (table `0x007325B0`, 32 entries) | 743–806 |
|   9. Durability `0x0057D3D0` | 807–834 |
|   10. Monster melee on a player, end to end | 835–927 |
| Constants & data dependencies | 928–949 |
| Randomness | 950–982 |
| Edge cases & original bugs | 983–1017 |
| Test vectors | 1018–1100 |
| Provenance | 1101–1126 |
| Open questions | 1127–1176 |
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

1. `0x00535D10(attacker, offhand)` (call `0x0057B800`): the dual-weapon
   stat switch, owned by `skills/bodies-2.md` §2.27 (damage weapon
   `0x00535BC0` by the used skill's `weapsel`, which hand's item stat
   lists are on); undone at step 14 by `0x00535E20` (call `0x0057BD84`,
   same section).
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
     `skill_bypass_demons(104)`, 0x400 if `skill_bypass_beasts(106)`
     (unit getter `0x00625480(attacker, s, 0)`, each ≠ 0; the flags are
     record +0x00; 1.14d-confirmed, `0x0057BB24`–`0x0057BB56`).
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
    the defender. `0x00554650(record, h)`: `(+0x60 & 0xF0) ≠ 0` → return
    0, nothing written; else `+0x60 |= h`, return 1 (1.14d-confirmed).
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

Leech rows 9–11 (resist stat −1; they run only for an `att_mon`
attacker, §4.4 step 5) take this path too: step 1 gives 0, step 3
compares −1 with 36 and 37 and so applies the difficulty penalty when
the defender is not `def_mon` (a player or a hireling): `r` = 0, −40,
−100 (expansion) or 0, −20, −50 (classic), and §4.6 step 4 then scales
the leech by `100 − r`: × 1.4 / × 2 (expansion Nightmare / Hell)
against players and hirelings, unchanged against monsters.
1.14d-confirmed (`0x0057BE00`: the −1 test at `0x0057BE06` skips only
the stat read; the 0x24 / 0x25 compares at `0x0057BE47`–`0x0057BE4F`).

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
   1. A player or monster attacker in mode 0 (death): return at once
      (`0x0057D5AC`), **without** freeing the combat record, without
      events, thorns or reaction. The record stays on the attacker
      until freed elsewhere.
   2. Hit flags = 0x20 (replaces all); `apply(game, attacker, defender,
      missile = 0, copy)` (§5.2).
   3. Overlay +0x6C > 0 → on the defender (`0x00621E40`).
   4. Hit class: if the byte +0x64 = 0 and the low nibble of +0x60 is
      0: OR in the attacker's weapon hit class (`0x00623C20`).
      `0x00623C20(unit)`: a player → its weapon (`0x00623990(unit, 0)`)
      gives the item's hit class (`0x0062A180`), no weapon → 1; a
      monster → byte +0x14 (`HitClass`) of its monstats2 row, through
      monstats `MonStatsEx` (`0x00451FE0`; no row → 0); any other type
      or none → 0. Measured: the Fallen's 3 is the hit class byte of
      1.14d's S→C 0x0D on its soft hits (`combat-fallen-hits-player`).
   5. Durability (§9).
5. Unit event 7 (`domeleeattack`) on the attacker, then event 3
   (`attackedinmelee`) on the defender (both with the copy).
6. If the copy has hit: attacker type 2 (object) or a type other than
   0 / 1 → thorns (`0x005D10C0`, skills spec). A player or monster
   attacker: in mode 0 (it died during steps 4–5, e.g. from an event
   function) → return at once (`0x0057D63F`), as in step 4.1: no
   thorns, no reaction, record not freed; any other mode → thorns.
   Without a hit: no thorns.
7. Reaction `0x0057CEE0` (§7.1).
8. Free the combat records for this attacker/defender pair
   (`0x0057C9F0`).

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
11. Total (+0x4C) > 0: `life = life − total`; `< 256` → 0 (signed
    `cmp 0x100` at `0x0057C865`: a life below one whole point is 0).
12. Mana leech (+0x3C) > 0: defender `mana −= value`, `< 256` → 0
    (`0x0057AA60`). Stamina leech (+0x40) likewise on stamina
    (`0x0057AAA0`).
13. Stun (§5.5), cold (§5.6), freeze (§5.7), poison, burn (§5.8), in
    that order.
14. `life > 0`: clear result 2; if total ≠ 0 and the defender is a
    monster: if `hpregen(74) ≠ 0`, cancel its type-3 timers and schedule
    type 3 at frame + 1 (`sim/tick.md` §5.2); then `0x005D6410
    (defender)`. `life ≤ 0`: result |= 2 (signed `test`/`jle` at
    `0x0057C8E7`). With step 11 any hit of total ≥ 1 on a unit at
    life 256 (one point) kills it; a poked life of 1–255 is alive until
    the next hit with total > 0.
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
7. **No attacker** (1.14d-confirmed, `0x0057C5AB`–`0x0057C67D`): the
   mode conversion `0x00645270` returns at once for a missing unit and
   leaves the preset type 6, so the monster rule runs: the fields are
   stored back as `(x << 6) / 64` (= `x` unless the shift overflowed),
   `T` is computed from the defender's mana and stamina, the drain
   scaling applies, then the attacker's max life and life read 0 for a
   missing unit (`0x00625D10`, `0x00625480`), `T = min(T, 0)` ≤ 0 and
   the leech stops: nothing healed, no overlay, no draw.

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
skills register their own. Function table §8. Handler records,
registration (prepend) and unregistration: `skills/bodies.md` §2.13;
iteration: §2.18 there, and `sim/units.md` §6.6 (newest first). The
functions also receive EDX = the event id.

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
cold with this length. Otherwise (other monsters, and any other
defender type): cold effect ≥ 0 (`0x0057AF30`; −50 for a non-monster)
→ nothing; monster: `length /= MonsterFreezeDivisor` (zero is a fatal
assertion). End `e = frame + length` (game of the attacker). Existing
freeze list (state 1, `0x006256B0`): its expiry becomes `max(expiry,
e)`. No list (1.14d-confirmed, `0x0057B337`–`0x0057B37E`): switch
state 1 on (`0x00639DB0(def, 1, 1)`) **first**, then create a list
(expiry `e`, owner attacker or (6, −1)), give it state 1, set its
remove callback `0x0057B170` (freeze end, `monsters/ai.md` §1.1 rule
2) and attach it. Timer 12 at the final expiry. Then a monster
defender: cancel its type-2 (AI) timers and schedule type 2 at frame +
length + 1 (length after the divisor); a non-monster defender here is
a fatal assertion (line 0x276). No draws.

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

ECX game, EDX attacker A; stack defender D, record R. F = R result
flags (u16 +0x04). "Mode request m" for a monster = `0x005A7E60(D, m, &req)`
then `0x005A7C20(game, &req, 1)` (`monsters/ai.md` mode request record);
for a player = `0x005809D0` / `0x00580A70` (`sim/pathing.md` §1.2).
"Soft" = queue D for update (`0x0064C040`) and D unit flags (+0xC4) |=
0x8000. (tA, gA) = A's type and GUID, (6, −1) without A. No draws here
except inside the get-hit test (§6.2).

1. D has a room and it is in town (`0x0061AB00`): continue only when A
   is a monster with monstats `inTown` (flag bit 10); else return.
2. F has 1 (hit): if R +0x64 = 0 and (R +0x60 & 0xF0) = 0: R +0x60 :=
   element hit class (§6.1). Then D +0xB0 := R +0x60.
3. D has state 54 (`uninterruptable`): F has 2 → state 92
   (`death_delay`) on. Return.
4. **Monster D** (not in mode 0 or 12, else return):
   1. F has 8 and the class lacks mode 13 `KB` (`0x0046C140(class,
      13)`) → F := F − 8 + 4 (stored in R).
   2. Sand leaper rule `0x005A54F0`: F has 4, D's `BaseId` is 78
      (`sandleaper1`) and D lacks state 1 (`freeze`) → F |= 8 (stored).
   3. F has 2 → `kill(game, D, A, 1)` (§7.2); return.
   4. F has 8 → mode request 13 (`KB`) with req +0x08 := A; return.
   5. F has 0x10 (block): F has 0x4000, or D's class is 243 (`diablo`),
      333 (`diabloclone`) or 705 (`uberdiablo`), or the class lacks mode
      6 (`BL`) → AI state := 19 (`0x005734C0(D, 19)` at `0x0057D083`,
      `monsters/ai.md` §3 "AI state"; no block mode); else mode request
      6. Return.
   6. F has 4: D has a state-21 (`stunned`) list (`0x006256B0`), or the
      get-hit test (§6.2, `0x0057CB00(D, R, R +0x60)`) is false → mode
      request 3 (`GH`), then `0x005A43A0(game, D)` (umod mode 4,
      `monsters/umod-callbacks.md` §2 rule 5). Test true → step 4.7's
      soft path. Return.
   7. F has 0x4000: soft, AI state := 19 (`0x005734C0(D, 19)` at
      `0x0057D119`), `0x005A43A0(game, D)`.
      Return.
   8. F has 1 and total (R +0x4C) > 0: b := low byte of D's stat 352
      (`last_sent_hp_pct`, total); c := D's life percent byte
      (`0x005A5650`); |b − c| > 4 → soft. Return.
5. **Player D** (first matching flag wins):
   1. F has 0x80 (dodge) → state 65 (`dodge`) list; else F has 0x100
      (avoid) → state 66 (`avoid`) list. No list → return. s := stat
      350 of that list (`0x00625D00`); E := `highest_entry(D, s)`; none
      → return. E flags (+0x0C, `0x006446A0` / `0x00644660`) |= 4; unit
      form request (E, mode 13, tA, gA, 0). Return.
   2. F has 0x200 (evade): state 68 (`evade`) list; s, E as above;
      skill `stsound` (+0xFC, i16) > 0 → sound event 12 (`0x00553380(D,
      12, D)`, `audio/triggers.md`). Return. **Only** the sound: no E
      flags write and no unit form request (asm `0x0057D206`–`0x0057D26D`:
      list `0x006256B0(D, 0x44)`, s = stat 0x15E, E `0x006439F0`, row
      `0x0045C4B0`, `stsound` test, `0x00553380`, return; 2026-10-09).
      E none or row none → return.
   3. F has 0x10 or 0x8000 (block / weapon block): F has 0x4000 →
      return. frame − stat 95 (`lastblockframe`) > `item_fasterblockrate`
      (102) / 8 (toward zero) + 15 → point form (no skill, mode 9 `BL`,
      0, 0, 0), then stat 95 := frame (game +0xA8). Return.
   4. F has 2: D in mode 0 or 17 → return; else unit form (no skill,
      mode 0, tA, gA, 0). Return. This starts a player's death (no other
      caller of `0x00580A70` passes a fixed mode 0; 2026-10-08,
      `0x0057D324`): it resolves (tA, gA)
      (`0x00552F60`); **no unit (no A: (6, −1)) → logged
      (`0x006248E0`) and nothing**, D stays alive in its mode. Else the
      mode check (`sim/pathing.md` §1.3: mode 0 allowed, also with a
      cursor item), the interrupt check (§1.4: refused for state 54 or
      mode 0 / 17, else allowed for m = 0), D's player data +0x154,
      +0x150 := 0, then the DT start `0x00580EC0` with K = the resolved
      A (`combat/vitals.md` §4.8). The kill `0x0057CCB0` (§7.2) never
      changes a player's mode.
   5. F has 8: unit form (no skill, mode 19 `KB`, tA, gA, 0). Return.
   6. F has 4: no state-21 list and the get-hit test true → soft;
      else point form (no skill, mode 4 `GH`, x = R +0x4C, 0, 0).
      Return.
   7. F has 0x4000 → soft.
6. Other unit types: nothing.

#### 7.2 Kill `0x0057CCB0` (D2MOO `SUNITDMG_KillMonster`)

ECX game, EDX victim D; stack killer A, flag (1 at every caller except
`0x00574450` at `0x005744D3`, `world/hirelings.md` §8 rule 1).

1. D a player: D in mode 0 or 17 → stop. D a monster: mode 0 or 12, or
   monstats `killable` false (`0x00457490(class, 15)` at `0x0057CCF4`,
   bit 15) → stop; then flag ≠ 0 and D's owner (`0x0058F0D0`) is a player → pet death
   bookkeeping `0x005751A0(game, owner, D)` (`world/hirelings.md` §8
   rule 1, `sim/pets.md`). Other types (and no D) → stop.
2. A present: experience `0x005A4EF0`: D unit flags lack 0x04000000 →
   distribution `0x0057E990(game, A, D)` (`combat/vitals.md` §4.4). Arena
   kill event `0x0053F720(game, A, D)` (`sim/intents-events.md`). D's
   class < monstats count (unsigned; a player victim's class 0–6 passes
   too) and A a player → `0x0066A220(A, D's class)`: an empty stdcall
   stub (its whole body is `ret 8`; one caller, `0x0057CD78`), so the
   call has no effect. d2rs omits it.
3. D a monster: mode request 0 (death) with direction (req byte +0x14)
   toward A (`0x00621DC0(D, A x, A y)`), or D's current direction
   without A (`0x006487F0`), req +0x08 := A, flag 1. D unit flags lack
   bit 31 → quest kill parse `0x00543A30(game, D, A)` (`world/
   quests.md` §4.4). monstats2 `objCol` (bit 18): class 432
   (`barricadedoor1`) → object 571, class 433 (`barricadedoor2`) →
   object 572: the first object of that class in D's room's unit list
   (room +0x74, next +0xE8) at D's exact position gets mode 2
   (`0x00624690`) and `0x00623830`.

Drops are not started here (`items/treasure.md`).

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
`(arg >> 16, arg)`. 15 and 16 are specified here; every other function is
`combat/events.md`.

**Crushing blow (16)**: `c` = chance stat; `c ≤ 0` → 0. Draw (attacker
seed, inline `lo′ mod 100`); `r ≥ c` → 0. Divisor: player defender 10;
monster: hireling 10 and nothing more (the jump at `0x005C0055` skips
the player-count term); any other monster 4, or 8 if boss or
superunique (flag 2), then `h = 0x005738F0(max(monster_playercount
(100), 1))` (player-count life bonus, monsters branch) and `div +=
pct(div, h, 100)` if `h ≠ 0` (1.14d-confirmed, `0x005C0046`–
`0x005C00A0`). Other defenders 4. Event 6 (missile) doubles it. `x = life / div`; `dr =
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
2. Defender is a player whose inventory pointer (+0x60) is non-null
   (every server player: the player type init `0x005348C0` creates it
   unconditionally at `0x005348F9`; 1.14d-confirmed): candidates from
   the weight table `0x00732B90` (7 rows, count at `0x00732BC8`): (bodyloc 1 head,
   3), (3 torso, 5), (4 right arm, 4), (5 left arm, 4), (8 belt, 2), (9
   feet, 2), (10 gloves, 2); keep each slot holding an item of type 50
   (any armor); `W` = sum of kept weights. `W > 0`: `i = roll(7)`, `w =
   roll(W)` (defender seed). Loop: if slot `i` is kept: `w < weight` →
   `durability_hit(game, defender, item)` and stop; else `w −=
   weight`. `i = (i + 1) mod 7`; continue while `w ≥ 0`.

Checked on `combat-melee-fallen` (rng channel, 2026-10-10): a short
sword hit by a level-3 Barbarian draws physical (range 1664 = 6.5 ×
256 with StrBonus), the burn quirk, then this durability draw on the
attacker's seed (4 draws in all); the breakable test is the weapon row's
`durability` > 0 and `nodurability` = 0 (`0x00629930`).

`durability_hit(game, owner, item)` = `0x00559E30`: items of type 50
(armor) or 45 (weapon) with durability (`0x00629930`); chance 10 % for
armor; for others 4 %, or (if `0x0062BA80(item)`) 10 % in an expansion
game and never in a classic game. Draw (owner seed, inline `lo′ mod
100`); `r < chance` → durability − 1 and the rest of the item rules
(items spec).

### 10. Monster melee on a player, end to end

The whole path of a plain monster melee hit (no used skill, e.g. a
Fallen's A1 or A2) from the mode request to the player's life, in
engine order. Each step names its owner; this section owns the order
and the recorded check (Test vectors, "Recorded").

1. **Mode request** (AI, `monsters/ai.md` §7.1): mode set `0x005A7C20`
   with mode m = 4 (A1) or 5 (A2). For every m ≠ 3 (GH), after the path
   set-up (`monsters/ai.md` §7.5 rules 2–5, `0x005A63F0` at
   `0x005A7D2F`) and **before** umod mode 0 (`0x005A4350` at
   `0x005A7D42`) and the start function: **mode damage**
   `0x005A4F50(unit, m)` at `0x005A7D39` (`skills/bodies-2.md` §2.1)
   rewrites the base list's `tohit(19)`, `mindamage(21)`,
   `maxdamage(22)` (and element stats) from the row of m: 5 → A2, 7 / 8
   → S1, any other (4, and 1 at creation) → A1. Values:
   - **level scaling**: `pct(monlvl DM, A1MinD, 100)`,
     `pct(monlvl DM, A1MaxD, 100)`, `pct(monlvl TH, A1TH, 100)` at the
     unit's `level(12)` (`L-` columns when game +0x6A or +0x74 ≠ 0;
     `monsters/init.md` §8.1); `noRatio` → the raw columns;
   - **difficulty**: the monstats `(N)` / `(H)` columns and the monlvl
     column of difficulty d (game +0x6D); expansion: + trunc(v × mult /
     128) with mult from `monster_playercount(100)` (0 for one player);
     classic, d ≠ 0, `Align` ≠ 1: min, max × 10 / 12, to-hit × 10 / 15.
   Draws only for element slots whose `El{i}Mode` = m with 0 <
   `El{i}Pct` < 100 (unit seed). The A-start `0x005A75C0`
   (`sim/units.md` §4.6 rule 7) then sets the mode.
2. **Strike** at the mode's action frame: event 0 `0x005A7670`, branch
   without a used skill (`skills/use.md` §5.2 "Monsters"): no mode
   missile (`MissA1`/`MissA2` empty) → melee set-up `0x005A5490`
   (`skills/bodies-2.md` §2.13 "Monster pre-hit": T' = `target(game,
   unit)`, zeroed record) and then, in the same event,
   `apply_melee(game, unit, path target)` (§5.1). Roll and application
   happen on one frame.
3. **Hit** `melee_result` (`combat/hit.md` §4): range `1 + 2` for a
   monster; `hit_test` monster terms (`combat/hit.md` §3.2): `AR =
   tohit(19) + 5 × dexterity(2)` (monsters: dex 0), `pctAR =
   item_tohit_percent(119)`; `def = defense(player) +
   armorclass_vs_hth(33)` = `armorclass(31) + dex / 4` + armor percents;
   `chance = clamp(2 × factor × alvl / (alvl + dlvl), 5, 95)`; one draw
   on the **monster's** seed, `lo′ mod 100 < chance`. Then block
   (`combat/hit.md` §5–§6, the player's seed; no shield → chance 0, no
   draw) and dodge / avoid (player passives; none → no draw); get-hit
   flag 4 unless state 54.
4. **Roll** `start_combat` (§3): `bonuses` (§3.2) with no weapon (a
   monster's `0x00535BC0` is none): `min = max(mindamage, 1) << 8`,
   `max = max(maxdamage, 2) << 8`, `pct += strength(0)` (0); `physical
   = min + roll(max − min)` (monster seed). Elements, drains, poison
   draw only when their stats are set (§3.3; `max << 8 ≥ 8`); the burn
   quirk `roll(1)` always draws (§3.1 step 10); monster crit (§3.1 step
   13) draws when `monstats.Crit` ≠ 0, `r < Crit` doubles every damage
   field. Will-die test (§3 step 2.3).
5. **Totals** (§4): damage percent 100 (monster → player, §4.2); row 0
   physical: `v −= normal_damage_reduction(34) << 8` (flat DR), then
   `v × (100 − r) / 100` with r = player `damageresist(36)` capped 50
   (DR %; no difficulty penalty for stat 36, §4.5 step 3); elements by
   their resists with the difficulty penalty; leech rows run (attacker
   monster). Total (+0x4C) = sum (§4.4 step 6).
6. **Life** (§5.2 steps 7–14): `physical = min(physical, life)`; `life
   −= total` in 1/256 points; `< 256` → 0. No hireling or hp-regen work
   for a player defender.
7. **Reaction** (§7.1 step 5): flags 1 | 4. Get-hit test (§6.2) with
   `M = max life`: the test true (total below the class threshold) →
   **soft**: the player is queued for update and gets unit flag 0x8000,
   mode unchanged; false → point form mode 4 (`GH`, `x = total`), the
   player's get-hit animation (its length by faster hit recovery is the
   player animation rule, `sim/units.md`). Block (flag 0x10) → mode 9
   (`BL`) when `frame − lastblockframe(95) > item_fasterblockrate / 8 +
   15`.
8. **Messages** (`sim/intents-events.md` §7.3 rule 1 step 4): unit flag
   0x8000 → `0x00547F70` → S→C **0x0D** (`0x0053B4B0`, 13 bytes:
   type 0, GUID, 0x13, x, y, hit class (unit +0xB0, §7.1 step 2), life
   percent `0x00621F20`). Life itself reaches the client through the
   life sync (`combat/vitals.md` §5.3: 0x95 once the change since the
   last sent life is ≥ 10 % of max). A GH mode sends the player mode
   message instead (unit flag 0x1).

Draws on the monster's seed per strike: miss 1; hit 1 + 1 (physical) +
1 (burn quirk) + 1 (crit, `Crit` ≠ 0) + element / drain rolls of set
stats. The player's seed draws only for block (shield), dodge / avoid /
evade passives, the armor durability pick (§9; no armor → none) and
the get-hit masks (§6.2 steps 5–6).

Monster **missile** on a player (e.g. the quill rat's `spike1`; owner
`missiles/missiles.md` §R5 step 5 and §R6.1): the hit test `0x0057D9B0`
draws on the **owner's** seed (missile 1: `armorclass_vs_missile(32)`,
bonus = missile `tohit(19)`); miss → 1 owner step, missile removed, no
damage; hit → 1 + 1 (monster crit `0x005A5560` at `0x005AD884`, `Crit`
≠ 0) owner steps, the damage rolls on the missile's seed (§R6.2 there),
no `start_combat`, no burn quirk. Recorded 2026-10-09, quill at f46:
`check-combat-arrow-quillrat` 89 → miss (1 step, life unchanged),
`check-combat-arrow-kill` 39 → hit, crit 76 (2 steps, 12800 → 12415).

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
11. Leech rows (9–11) have no resist stat, so §4.5 starts from r = 0
    and step 3 still applies (the test is "not 36 or 37", and −1 is
    neither; `0x0057BE44`–`0x0057BE6D`): against a non-monster defender
    in an expansion game r := `ResistPenalty`, and §4.6 step 4 scales
    the leech by (100 − r) / 100: a monster draining a Hell player
    takes ×2 (Nightmare ×1.4; classic −20 / −50 → ×1.2 / ×1.5).

## Test vectors

Synthetic (CI-safe):

| Input | Expected |
|---|---|
| `pct(0x200000, 50, 100)` | `(2097152 / 100) × 50` = 1,048,550 (exact 1,048,576) |
| `pct(1000, 0x20000, 100)` | `(131072 / 100) × 1000` = 1,310,000 |
| `pct(0x200000, 50, 0x30000)` | d > v >> 4 (0x20000) → 64-bit 104,857,600 / 196,608 = 533 |
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
| `apply`, missile, Fire Bolt level 1 (skills.txt 36: `EMin` 6, `EMax` 12, `HitShift` 7 → fire 768–1536) on Normal Andariel (monstats 156 `ResFi` −50; `DamageRegen` 0 → hpregen 0, `monsters/init.md` §6 r10) at life 256 | §4.5: r = −50 (monster: no cap, no penalty) → fire × 150 / 100 = 1152–2304; step 11: 256 − total < 256 → life 0; step 14: result \|= 2; events 10, 9 |
| same on Normal Radament (monstats 229: `ResFi` empty = 0, `ResPo` 50, `ResCo` 40; superunique row 10 umods 6 `fast` + 1–4, umod 2 unique sets hpregen 0, `monsters/umods.tsv`) at life 256 | fire 768–1536 unscaled; life 0; result \|= 2 |
| open wounds, attacker level 20, monster | ow = 18 × 5 + 126 = 216; h = 256 |
| open wounds, level 70, player defender, missile | 45 × 10 + 15 × 81 + 126 = 1791; +40 = 1831; /4 = 457; /2 = 228 |

Recorded (§10; `traces/raw/check-combat-fallen-hits-player/
orig.state.jsonl`, check `traces/checks/combat-fallen-hits-player.check`):
1.14d, ScnAma (Amazon level 1, no items: `armorclass` 0, dex 25 → def 6,
no block, no armor durability pick, life 12800 = M), Blood Moor Normal,
expansion, one player; a fallen1 party (monstats 19; GUIDs 19, 20, 21)
poked at frame 30. fallen1 Normal: `A1MinD` 51, `A1MaxD` 101, `A1TH`
101, A2 the same, `Crit` 5, `El1Pct` empty (no fire roll), level 1.
Seeds are the snapshot unit seed (lo, hi) of the frame the mode started
(after the start's own draws) and of the hit frame; the steps between
them are `tools/trace-recorder/d2rng.py` `step`:

| Hit frame | Fallen (mode, start) | Seed at start | Draw 1 `lo′ mod 100` | Draw 2 `roll(256)` = `lo′ & 255` | Draw 3 burn `roll(1)` | Draw 4 crit `mod 100` (< 5?) | Physical | Player life |
|---|---|---|---|---|---|---|---|---|
| 77 | 21 (A1, 70) | 2500281817, 685907116 | 25 hit | 1698023647 & 255 = 223 | 0 | 68 no | 256 + 223 = 479 | 12800 → 12321 |
| 98 | 20 (A1, 91) | 2686906702, 557886531 | 29 hit | 2253558215 & 255 = 199 | 0 | 7 no | 455 | → 11866 |
| 124 | 20 (A2, 116) | 2976444768, 790087745 | 25 hit | 3146423253 & 255 = 213 | 0 | 76 no | 469 | → 11397 |
| 137 | 19 (A2, 129) | 3662132598, 1067771371 | 45 hit | 1104800614 & 255 = 102 | 0 | 27 no | 358 | → 11039 |
| 137 | 21 (A1, 130) | 3277392918, 1209584854 | 92 miss | — | — | — | — | (1 step only) |
| 158 | 20 (A1, 151) | 4274727917, 709079844 | 65 miss | — | — | — | — | unchanged |
| 172 | 19 (A2, 164) | 3533759791, 1701973489 | 96 miss | — | — | — | — | unchanged |
| 173 | 21 (A2, 165) | 3740231532, 902428276 | 20 hit | 3976525535 & 255 = 223 | 0 | 55 no | 479 | → 10560 |

Every hit is exactly 4 steps of the attacker's seed and every miss 1
(the end seeds in the snapshot match). What the record fixes:

- the draw order hit → physical → burn `roll(1)` → monster crit (§3.1
  steps 4, 10, 13): with crit before burn the 124 hit would crit (3 <
  5) and deal 938;
- min = 256, max = 512: `mindamage` 1, `maxdamage` 2 after mode damage
  (pct(monlvl `L-DM` level 1, 51 / 101, 100) with `L-DM` = 2, read from
  the shipped `monlvl.bin`, then the 1 / 2 floors of §3.2); no other
  range reproduces all five rolls;
- no DR, no resist, damage percent 100, no regeneration between hits:
  life drops by the physical roll exactly;
- hit chance 46 … 65 (hits at r = 20 … 45, misses at r = 65, 92, 96):
  `chance = 100 × T / (T + 6)` (alvl = dlvl = 1) → to-hit T ∈ 6 … 11,
  i.e. pct(`L-TH` level 1, 101, 100) ∈ 6 … 11. The shipped table
  (`Patch_D2.mpq` `data\global\excel\monlvl.bin`, 111 records of 120
  bytes after the u32 count; record n = level n) holds `L-TH` 8 / 108 /
  216 and `L-DM` 2 / 3 / 4 (Normal / NM / Hell) at level 1, so T =
  pct(8, 101, 100) = 8 and the hit chance is 100 × 8 / 14 = 57 (read
  2026-10-09, settles REC-817; matches the hits at r ≤ 45 and the
  misses at r ≥ 65);
- reaction: the player's seed (85, 666) and mode 5 never change, so the
  get-hit test passed its size test without a mask draw (total < M /
  div: 358 … 479 < 12800 / div → div ≤ 16, i.e. hit class not
  `2hss`/`club`/`2hsl`) and every hit was soft: no GH.

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
- §7 read from `0x0057CEE0`, `0x005A54F0`, `0x0057CCB0`, `0x005A4EF0`,
  `0x0057CC30`, `0x00457490`, `0x00451F80`; `0x005D6410` (OQ10); class
  and state names from 1.14d `monstats.txt` (ids after skipping the
  `Expansion` row: 78, 243, 333, 432, 433, 705), `states.txt` (21, 54,
  65, 66, 68, 92, 153), `itemstatcost.txt` (95, 102, 350, 352).

## Open questions

1. ~~No trace confirms any number here. Request (recording): hook
   `0x0057DBF0` entry and exit (EBX record before/after: +0x08…+0x4C) and
   `0x0057C6C0` entry/exit (defender life stat 6 before/after) during
   melee and missile play; log the attacker's and defender's seeds at
   entry. Compare with §3–§5 recomputed from logged stats.~~ → PC 2 recording list.
2. ~~Leech: hook `0x0057C420` entry (EAX record, EBX attacker, [EBP+8]
   defender) and the calls `0x0057A980` / `0x0057AA00` (amounts) to
   confirm rules H/M and Edge case 6.~~ → PC 2 recording list.
3. Answered: §7.1 and §7.2 are read branch by branch from the 1.14d
   disassembly (block frame test, soft-hit path, sand leaper knockback,
   barricade doors). `0x005734C0` is the AI-state setter
   (`monsters/ai.md` §3). `0x0066A220(killer, class)` is an empty stub
   (`ret 8`, §7.2 step 2).
4. Answered: registration, unregistration and iteration of unit events
   are `skills/bodies.md` §2.13 and §2.18; the record list
   (`0x005C0C30`, add `0x005C0AD0`, newest first) is also
   `sim/units.md` §6.6.
5. Answered: every event function other than 15 and 16 is
   `combat/events.md`.
6. Answered statically: `0x0088CAD0` is read and written only by
   `0x0057CE30` (the two direct references in `all.asm`), lies in the
   zero-filled part of `.data` (past the section's raw data) and is
   never reset: one counter per process, starting at 0, shared by every
   game.
7. Answered: experience on kill, party share and the add function are
   `combat/vitals.md` §4.2–§4.5; player death penalties §4.6; drops
   `items/treasure.md`.
8. Answered: life change from stat 74 each frame is the event-3
   regeneration handler, `sim/stat-lists.md` §10.1.
9. Answered: `0x0062BA80` is "itemtype `throwable`" and `0x00629930`
   "has durability" (`items/generation.md` predicate table).
10. Answered: `0x005A4390` is umod mode 3 (`monsters/umod-callbacks.md`
    §2 rule 4). `0x005D6410(ECX unit)`: for every state the unit has
    (state bitset `0x0063A100`, nothing when `0x0063A360` = 0; lowest
    index first, over a copy of the bitset) that has the `states`
    `remhit` flag (bitset 6, data tables +0xE4): its state list, if any,
    is detached and freed, then the state is turned off. 1.14d: only
    state 153 `cloak_of_shadows` has `remhit`.
11. Answered (`docs/handoff/impl-combat.md` items 11–15, checked against
    the 1.14d disassembly): item 11 corrected, §5.1 steps 4.1 and 6 (a
    dead attacker returns without freeing the record; the mode-0 test
    at `0x0057D63B` ends the whole function, not just thorns); item 12
    specified, §5.3 step 7 (no attacker: monster rule, nothing healed);
    item 13 confirmed, §5.7 (`0x0057B33C` switches state 1 on before the
    list is created; remove callback `0x0057B170` added); item 14
    confirmed, §8 (`0x005C0055`); item 15 confirmed, §9 (inventory
    pointer +0x60, created for every server player at `0x005348F9`).
