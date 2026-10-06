# Spec: Combat — Vitals: creation values, stat points, level-up, experience

- **Status:** draft: creation (§1), stat points (§2), level-up (§3) and
  the experience table lookups (§4.1) read in full from the 1.14d
  `Game.exe`; the experience-on-kill level factor (§4.2) read in full;
  the rest of §4 (experience ratio, party share, hireling experience) is
  D2MOO 1.10f structure that is only partly confirmed (Open question 2).
  No trace check yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::combat::vitals` (player creation and
  level-up may live in `d2-sim::units::player`)
- **Related specs:** `sim/stat-lists.md` §10.1 (owns regeneration:
  timer event 3, life / mana / stamina per tick, poison and open-wounds
  life loss through stat 74, monster regeneration and death by
  regeneration) and §7.2 (current life/mana/stamina rescaled when the
  maximum changes; monster `DamageRegen` → stat 74); `sim/stats.md` (op
  9: item vitality / energy → max life / mana / stamina); `combat/damage.md`
  (§7.2 kill, which triggers experience); `skills/levels.md` (skill
  points, max skill level from the same table); monster vitals at spawn:
  the monsters spec (Open question 5).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–55 |
| Inputs | 56–64 |
| Outputs / state changes | 65–68 |
| Rules | 69–70 |
|   1. Creation values | 71–93 |
|   2. Spending stat points (message 0x3A) | 94–131 |
|   3. Level-up `0x00570880` (D2MOO `PLAYERSTATS_LevelUp`) | 132–153 |
|   4. Experience | 154–193 |
| Constants & data dependencies | 194–210 |
| Randomness | 211–214 |
| Edge cases & original bugs | 215–226 |
| Test vectors | 227–247 |
| Provenance | 248–259 |
| Open questions | 260–280 |
<!-- /index -->

## Summary

A new player gets its attributes and life, mana and stamina from
`charstats.txt`. Spending a stat point raises the base attribute and,
for vitality and energy, the base maximum and current life, stamina and
mana by quarter points per point. Gaining a level adds the per-level
amounts, refills life (if alive), mana and stamina, and grants stat and
skill points. Levels come from the experience table. Experience for a
kill is scaled by the level difference. Regeneration over time is not
here (`sim/stat-lists.md` §10.1).

Units: life, mana and stamina stats (6–11) are 1/256 points. The
charstats per-level and per-point columns are quarter points, so a
column value `v` adds `v << 6` units.

## Inputs

| Name | Type | Source |
|---|---|---|
| `charstats.txt` (0xC4-byte records at `[0x744304]+0xBC4`) | str +0x30, dex +0x31, int +0x32, vit +0x33, stamina +0x34, hpadd +0x35, LifePerLevel +0x43, StaminaPerLevel +0x44, ManaPerLevel +0x45, LifePerVitality +0x46, StaminaPerVitality +0x47, ManaPerMagic +0x48, StatPerLevel +0x50 (i8) | class id |
| experience table `[0x0096C8A8]` | u32 rows of 8 (Amazon … Assassin, ExpRatio) | `experience.txt` |
| client message 0x3A | `client-messages.tsv` | §2 |
| stats | base getter `0x006253B0`, unit getter `0x00625480`, set `0x00627260`, add `0x006272B0` | `sim/stats.md` |

## Outputs / state changes

Base stats 0–5, 6–13, 19, 20, 30 (`nextexp`), 67–69 of the player.

## Rules

### 1. Creation values

`init_player_stats(game, unit, act)` = `0x005706D0` (D2MOO
`PLAYERSTATS_SetStatsForStartingAct`). `act ≥ 5` is treated as 4. With
the class's charstats record (none → nothing), set base stats:

| Stat | Value |
|---|---|
| 0 strength, 1 energy, 2 dexterity, 3 vitality | `str`, `int`, `dex`, `vit` |
| 19 tohit, 20 toblock | 0 |
| 6 hitpoints, 7 maxhp | `(vit + hpadd) << 8` |
| 8 mana, 9 maxmana | `int << 8` |
| 10 stamina, 11 maxstamina | `stamina << 8` |
| 12 level | 1 |
| 30 nextexp | `threshold(class, 1)` (§4.1) |
| 68 attackrate, 67 velocitypercent, 69 other_animrate | 100 |

Then, if `act > 0` (and `act <` the table count 5 at `0x007326B4`,
else index 0): raise experience to the target level of table
`0x006E1520` {1, 15, 20, 26, 32} by index `act` (`0x0057EB10`, D2MOO
`SUNITDMG_SetExperienceForTargetLevel`: add `threshold(class, target) −
experience` if positive, through §4.3, which levels up).

### 2. Spending stat points (message 0x3A)

Handler `0x0054BD10`: size must be 3, else 3. Byte +1 is the stat id
`s`, byte +2 is `count − 1`. `s > 15` or `count − 1 > 99` → 3. Repeat
`count` times `spend(unit, s)` (`0x00570D60`); the first failure stops
and returns 2; else 0. (`client-messages.tsv` describes the field as a
u16 stat: Open question 4.)

`spend(unit, s)`: `statpts(4)` (unit getter) = 0 → fail. By `s`:

- 0 strength, 2 dexterity: `statpts −= 1`, stat `+= 1` (add), then the
  refresh `0x0064C040(unit)`.
- 1 energy: `gain_energy(unit, 1)` (`0x00570A80`).
- 3 vitality: `gain_vitality(unit, 1)` (`0x00570B60`).
- any other id (4…15): fail.

`gain_energy(unit, n)`: `statpts −= n`; `energy += n`; if `n > 0`: `mana
+= (ManaPerMagic × n) << 6`; `maxmana += (ManaPerMagic × n) << 6`; if
the unit's mana (unit getter) > its max mana: set mana = max mana.

`gain_vitality(unit, n)`: `statpts −= n`; `vitality += n`; `maxhp +=
(LifePerVitality × n) << 6`; if `n > 0`: `hitpoints += (LifePerVitality
× n) << 6`; life > max life → life = max life; `maxstamina +=
(StaminaPerVitality × n) << 6`; if `n > 0`: `stamina += (…) << 6`;
stamina > max stamina → clamp.

All additions go to base stats (add `0x006272B0`). A negative `n`
(stat reset, §2.1) lowers the maximum but not the current value, which
is then clamped.

#### 2.1 Stat reset `0x00570C80`

Players only: for strength, energy, dexterity, vitality in that order,
`d = charstats start value − base value`; strength and dexterity: if `d
≠ 0`, `statpts −= d`, stat `+= d`, refresh; energy: `gain_energy(unit,
d)`; vitality: `gain_vitality(unit, d)`. Its callers (the Akara reset
quest reward) belong to the quests spec.

### 3. Level-up `0x00570880` (D2MOO `PLAYERSTATS_LevelUp`)

1. `old = level(12)` (base); `new = level_from_exp(class,
   experience(13))` (§4.1); set level = `new`; set `nextexp(30) =
   threshold(class, new)`.
2. `d = new − old`; `d ≤ 0` → stop.
3. `maxhp = base maxhp + (LifePerLevel × d) << 6`; if life (unit getter)
   > 0: life = max life (`0x00625D10`).
4. `maxmana = base + (ManaPerLevel × d) << 6`; mana = max mana
   (`0x00625D60`).
5. `maxstamina = base + (StaminaPerLevel × d) << 6`; stamina = max
   stamina (`0x00625DB0`).
6. `statpts(4) += StatPerLevel × d` (signed byte; 5 for every 1.14d
   class); `newskills(5) += d`.
7. Notifications: `0x00536850` (party roster, every living player),
   `0x00553380(unit, 2)` (level-up sound), `0x005538D0(game, unit,
   0x00570850)`, `0x0055F500`, `0x0055FDE0(…, 1)` (client updates),
   host callback `[0x00883D50]+0x2C` if present.

The caller of level-up (§4.3) triggers unit event 12 (`levelup`) after
it (D2MOO; Open question 2).

### 4. Experience

#### 4.1 Table lookups

`[0x0096C8A8]` points to `experience.txt` as u32 rows of 8 columns
(classes 0–6, then `ExpRatio`): row 0 is `MaxLvl`, row `L + 1` is level
`L`. Class ids outside 0–6 use class 0.

- `max_level(class)` = row 0 (99 for every 1.14d class; `0x00611830`).
- `threshold(class, L)` = `0x00611800`: row `L + 1` (the experience
  needed to reach level `L + 1`; level 1 → 500).
- `level_from_exp(class, exp)` = `0x00611860`: `i = 0`; while `exp ≥
  row(i + 1)` (unsigned) and `i < max_level`: `i += 1`. Return `i`.
  (Level 0 has threshold 0, so any experience gives at least 1.)

#### 4.2 Level factor `0x0057E2F0(exp, alvl, dlvl)`

`dlvl ≤ alvl`: `f = T1[min(alvl − dlvl, 10)]`, `T1` (`0x006E1668`) =
256, 256, 256, 256, 256, 256, 207, 159, 110, 61, 13. `dlvl > alvl`: if
`alvl ≥ 25` and `dlvl > 0` → result `pct(exp, alvl, dlvl)`; else `f = T2[min(dlvl −
alvl, 10)]`, `T2` (`0x006E1694`) = 256, 256, 256, 256, 256, 256, 225,
174, 92, 38, 5. Result `f = 256` → `exp`; else `pct(exp, f, 256)`
(`combat/damage.md` §0). Signed comparisons.

#### 4.3 Gain on a kill (D2MOO structure, partly confirmed)

`0x0057E480` (D2MOO `SUNITDMG_ComputeExperienceGain`) calls §4.2 with
the defender's base experience and both levels; D2MOO then applies the
`ExpRatio` column of the attacker's level, `item_addexperience` (stat
85) as a percent, and for hirelings a cap from `hireling.txt`. The kill
path (D2MOO `SUNITDMG_DistributeExperience`: pets and minions credit
their player owner; a hireling gets 86/256 when it did not land the
kill; a party of `n` members within 80 units (squared distance 6400)
shares `exp + 89 × exp × (n − 1) / 256` in proportion to member levels,
using floating point) and the add function (D2MOO
`SUNITDMG_AddExperienceForPlayer`: cap at `threshold(class, max_level −
1)`, set `lastexp(29)`, level-up §3 and event 12 when the level changes)
are not yet confirmed in 1.14d (Open question 2). The party share's
float arithmetic must be reproduced exactly once confirmed.

## Constants & data dependencies

1.14d `charstats.txt` (fourths for the per-level / per-point columns):

| Class | str dex int vit | stamina | hpadd | Life/Lvl | Stam/Lvl | Mana/Lvl | Life/Vit | Stam/Vit | Mana/Magic | StatPerLevel |
|---|---|---|---|---|---|---|---|---|---|---|
| Amazon | 20 25 15 20 | 84 | 30 | 8 | 4 | 6 | 12 | 4 | 6 | 5 |
| Sorceress | 10 25 35 10 | 74 | 30 | 4 | 4 | 8 | 8 | 4 | 8 | 5 |
| Necromancer | 15 25 25 15 | 79 | 30 | 6 | 4 | 8 | 8 | 4 | 8 | 5 |
| Paladin | 25 20 15 25 | 89 | 30 | 8 | 4 | 6 | 12 | 4 | 6 | 5 |
| Barbarian | 30 20 10 25 | 92 | 30 | 8 | 4 | 4 | 16 | 4 | 4 | 5 |
| Druid | 15 20 20 25 | 84 | 30 | 6 | 4 | 8 | 8 | 4 | 8 | 5 |
| Assassin | 20 20 25 20 | 95 | 30 | 8 | 5 | 6 | 12 | 5 | 7 | 5 |

`experience.txt`: MaxLvl 99 for every class; level 1 → 500; level 99 →
3,837,739,017 (all classes equal).

## Randomness

None. (Regeneration: `sim/stat-lists.md`.)

## Edge cases & original bugs

1. Stat ids 4–15 in message 0x3A are accepted by the range check and then
   fail in `spend`.
2. `gain_energy` / `gain_vitality` with `n ≤ 0` do not change the current
   value except by the clamp.
3. Level-up refills life only for a living player; mana and stamina are
   always refilled.
4. Level-up for several levels at once multiplies every gain by `d`.
5. `level_from_exp` compares unsigned; experience above the level-99
   threshold stops at `max_level`.

## Test vectors

Real 1.14d data (game-file tests), from §1–§3:

| Case | Expected (1/256 units; points in parentheses) |
|---|---|
| Sorceress created | life 10240 (40), mana 8960 (35), stamina 18944 (74) |
| Barbarian created | life 14080 (55), mana 2560 (10), stamina 23552 (92) |
| Sorceress level 1 → 10 (d = 9), no stat points spent | max life 10240 + 9 × 4 << 6 = 12544 (49); max mana 8960 + 9 × 8 << 6 = 13568 (53); max stamina 18944 + 2304 = 21248 (83); statpts +45, newskills +9 |
| Barbarian level 1 → 10 | max life 14080 + 4608 = 18688 (73); max mana 2560 + 2304 = 4864 (19); max stamina 23552 + 2304 = 25856 (101) |
| Barbarian, +10 vitality spent | max life + 16 × 10 << 6 = +10240 (+40); max stamina +2560 (+10); statpts −10 |
| Sorceress, +10 energy | max mana +5120 (+20) |
| `level_from_exp(0, 499)` / `(0, 500)` | 1 / 2 |
| `threshold(0, 1)` | 500 |
| level factor: alvl 30, dlvl 22, exp 1000 | T1[8] = 110: `pct(1000, 110, 256)` = 429 |
| alvl 10, dlvl 18, exp 1000 | alvl < 25: T2[8] = 92: 359 |
| alvl 40, dlvl 50, exp 1000 | `pct(1000, 40, 50)` = 800 |

Synthetic: message 0x3A bytes `3A 03 04` (vitality, count 5) with 3
stat points: three spends succeed, the fourth fails, result 2.

## Provenance

- 1.14d `Game.exe` disassembly: `0x005706D0`, `0x0054BD10`,
  `0x00570D60` (jump table `0x00570DF8`), `0x00570A80`, `0x00570B60`,
  `0x00570C80`, `0x00570880`, `0x00611800`, `0x00611830`, `0x00611860`,
  `0x0057E2F0`. Image data: `0x006E1520`, `0x007326B4`, `0x006E1668`,
  `0x006E1694`. Charstats offsets match `data/fields.tsv`.
- D2MOO 1.10f `PlayerStats.cpp`, `SUnitDmg.cpp` for names; §1–§3 and
  §4.2 agree with it. Answers `skills/levels.md` Open question 2:
  `[0x0096C8A8]` is the experience table and entry 0 is `MaxLvl`.
- 1.14d `charstats.txt`, `experience.txt` (`patch_d2`).

## Open questions

1. No trace check. Recording request: hook `0x00570880` entry/exit and
   `0x00570D60` (log stats 4–13 before/after) during a level-up and
   while spending points.
2. Ghidra request: read `0x0057E480` (experience gain: `ExpRatio`, stat
   85, hireling cap) and its callers `0x0057E6C0` / `0x0057E990`
   (distribution, party share with its float math, add function and
   event 12) against §4.3.
3. §4.2 branch for `dlvl > alvl`: confirm the operand roles of the
   `pct(exp, alvl, dlvl)` call (the read gives EAX = defender level,
   EDX = attacker level, ECX = experience).
4. `client-messages.tsv` row 0x3A says `stat:u16@1`; the handler reads
   byte +1 as the stat and byte +2 as count − 1.
5. Monster life, mana, attack rating, defense, damage and experience at
   spawn (`monstats` + `monlvl`, champion / unique bonuses, player
   count; D2MOO `D2Common` `Monsters.cpp`, 1.14d `0x0063EFA0` area):
   owner is the monsters spec; not specified here.
6. Player death penalties (`DeathExpPenalty`, gold loss) and the stat
   reset callers: not specified.
