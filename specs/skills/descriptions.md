# Spec: Skills — Description functions (`descdam` / `descatt` tables)

- **Status:** draft (2026-10-07): every rule read from the 1.14d
  `Game.exe` disassembly and image (table words read from the file
  image, addresses below). Covers: the two function tables `0x0072D768`
  (`skilldesc.descdam`) and `0x0072D7F8` (`skilldesc.descatt`), all 24 +
  5 non-null entries, and the helpers they share. Not covered: where and
  how the character panel lays the lines out (owner `ui/panels-2.md`
  §17.5). No capture yet (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::ui::skill_desc` (pure functions of the
  local player's client-side stats; no RNG, no state change except the
  temporary weapon stat-list toggles of §2.10)
- **Related specs:** `ui/panels-2.md` §17.5 (caller `0x004ED570`, entry
  rectangles, labels, the attack-rating line) and §17.6 (chance to hit
  from `descatt` v1); `skills/levels.md` (§1 `skill_level`, §3 skill
  damage helpers, §3.5 weapon mastery, concentration, kick damage, §5
  `to_hit`); `combat/damage.md` §0 (`pct`); `combat/hit.md` (attack
  rating `0x00622560`); `sim/stat-lists.md` §8.4 (`0x00627910`);
  `data/calc-expressions.md` (skills / skilldesc evaluators
  `0x00646CA0` / `0x00646D00`); `data/fields.tsv` (skills and skilldesc
  offsets used below).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–53 |
| Inputs | 54–65 |
| Outputs / state changes | 66–72 |
| Rules | 73–74 |
|   1. Tables and call contract | 75–92 |
|   2. Shared helpers | 93–284 |
|   3. `descdam` entries (`0x0072D768`) | 285–317 |
|   4. `descatt` entries (`0x0072D7F8`) | 318–328 |
| Constants & data dependencies | 329–344 |
| Randomness | 345–348 |
| Edge cases & original bugs | 349–369 |
| Test vectors | 370–388 |
| Provenance | 389–405 |
| Open questions | 406–415 |
<!-- /index -->

## Summary

`skilldesc.txt` columns `descdam` (record +0x12, u16) and `descatt`
(+0x14, u16) select one function each from two tables in the image.
The character panel's damage block (`0x004ED570`, per hand) calls the
`descdam` function to compute and draw the skill's damage range, and the
`descatt` function to compute the attack rating it draws (and that
`0x004EDA80` feeds the chance-to-hit popup). Each function reproduces,
on the client, the damage or attack-rating sum the server uses for that
kind of skill, from the local player's current stats.

## Inputs

| Name | Type | Source |
|---|---|---|
| U | unit | the local player (ECX) |
| S | skill entry | the hand's skill (EDX) |
| R | skills record | `0x00644140(S)` |
| L | i32 | `skill_level(U, S, 1)` (`0x006442A0`, with bonuses) |
| D | skilldesc record | `0x004A89D0(skill id)` |
| x1, y, x2 | i32 | `descdam` only: entry e`p+2` of table `0x0072D840` (`ui/panels-2.md` §17.5) |
| &v1, &c1, &v2, &c2 | i32 out | `descatt` only, zeroed by the caller |

## Outputs / state changes

`descdam`: draws one or two text lines; returns nothing. `descatt`:
writes (v1, c1, v2, c2). Both may toggle weapon stat lists of U and
restore them (§2.10). Colors are text-color indices: 0 white, 1 red, 2
green, 3 blue, 9 yellow.

## Rules

### 1. Tables and call contract

1. `descdam` table `0x0072D768`: 36 words; entries 1–24 non-null, 0 and
   25–35 null. `descatt` table `0x0072D7F8`: 18 words; entries 1–5
   non-null, 0 and 6–17 null. The bound tests of the caller are `descdam
   < 0x90` and `descatt < 0x48` (index units, `0x004ED6A0`,
   `0x004ED6B9`): larger than the tables (Edge case 1).
2. Only `0x004ED570` (calls both) and `0x004EDA80` (calls `descatt`)
   reference the tables. The skill hover boxes (`0x004EEF00`) do not.
3. `descdam` call: ECX U, EDX S; stack R, L, x1, y, x2; callee pops 20
   bytes. Every function returns at once when S or R is null.
4. `descatt` call: ECX U, EDX S; stack R, L, &v1, &c1, &v2, &c2. The
   caller draws v1 (and v2 when ≠ 0) in colors c1, c2.
5. All arithmetic is i32, `/` truncates toward zero, `>>` is
   arithmetic. "`x × s / 128`" is computed as `(x × s + (x × s < 0 ?
   127 : 0)) >> 7` (same result). `pct(v, p, d)` is `combat/damage.md`
   §0 (`0x00483360`).

### 2. Shared helpers

#### 2.1 Range draw `0x004E96E0`

ECX min, EDX max, EDI x1, ESI x2; stack color, y.

1. max ≤ min → max := min + 1 (so a single value never prints alone;
   Edge case 2).
2. Text: both < 10000 → `%d-%d`; min < 10000 ≤ max → `%d-%dK` with max
   := (max + 500) / 1000; min ≥ 10000 → `%dK-%dK` with both rounded that
   way. (The `%d` path for min = max is unreachable.)
3. Converted to UTF-16 (`0x00526320`, at most 32 chars). Width w is
   measured in the current font (Font6, set by the caller) by
   `0x00502520`; w' = w × 11 / 7. w' > x2 − x1 → Font6 and y := y − 1;
   else Font1.
4. Drawn centered in [x1, x2] (`0x004A7080`) at screen x = `sx` + x,
   screen y = `H + sy − 480` + y (`ui/panels-2.md` §17.5), in color.

#### 2.2 Two-line draw `0x004E9870`

Used when a function has two non-zero ranges, A (top) and B (bottom).
Each range is formatted as in §2.1 rule 2 **without** rule 1 (min = max
prints `%d`). Font6; once per run the top offset `[0x0072D8D4]` := −6,
or −7 when the language id (`0x00525150`) is 6–9. A at y +
`[0x0072D8D4]` in color cA, B at y + `[0x0072D8D0]` (2) in cB, both
centered in [x1, x2]. A function with two ranges draws: A both zero →
B alone with §2.1 in cB; else B both zero → A alone in cA; else this
rule.

#### 2.3 Weapon physical `weapon_phys` `0x004E86F0`

ECX U, EAX extra percent E; stack &min, &max, &c, flat F0, skill S,
source s, weapon W, own-list flag o. Adds to *min, *max.

1. S null, no R, or U has no inventory → nothing (returns 0).
2. W = given, else the weapon in use (`0x0063BEF0`). None: b = U's
   `mindamage(21)` + 1, B = `maxdamage(22)` + 2. Else: o ≠ 0 → b, B =
   W's own stats 21 / 22 (`0x00626070`); else grip (`0x0063D340`) = 2 →
   U's `secondary_mindamage(23)` / `secondary_maxdamage(24)`, else 21 /
   22.
3. s = 0 → s := `SrcDam` (R +0x1A5). s ≠ 128 → b := b × s / 128, B := B
   × s / 128. s ≠ 0 → *min += max(b, 1), *max += max(B, 2).
4. P = E + U's `damagepercent(25)` + `weapon_mastery(U, W, S, 1)`
   (`0x00645830`). R `finishing` (flag byte +4, mask `[0x006CE274]` = 8)
   and the charge bonus §2.9 rule 1 ≠ 0 → P += it, *c := 3. No W → P +=
   U's `strength(0)` (whole value); else P += str × `StrBonus` / 100 +
   dex(2) × `DexBonus` / 100 (each only when the bonus ≠ 0,
   `0x00629860` / `0x006298A0`) and F1 = W's `item_normaldamage(111)`
   (`0x00625500`), else F1 = 0.
5. U has a `damblue` state (states flag list at data tables +0xE8,
   `0x0063A380`) → *c := 3; a `damred` state (+0xEC, `0x0063A3A0`) →
   *c := 1.
6. P < −89 → P := −90. *min := (`item_mindamage_percent(18)` + 100 + P)
   × *min / 100 + F1 + F0; *max likewise with
   `item_maxdamage_percent(17)`.

#### 2.4 Stat elements `0x004E89A0` (U, &min, &max, &c, S)

1. For fire (48/49, mastery 329, color 1), lightning (50/51, 330, 9),
   cold (54/55, 331, 3), magic (52/53, no mastery, 3) in that order
   (tables `0x006DBEF4` stride 12 and colors `0x006DBEE4`): mn, mx = U's
   totals; mastery m ≠ 0 → mn += pct(mn, m, 100), mx += pct(mx, m, 100);
   mn := min(mn, mx); *min += mn, *max += mx; mn or mx ≠ 0 → *c := the
   element color.
2. Poison: mn = U's `poisonmindam(57)`, mx = `poisonmaxdam(58)`; mx ≠
   0: `passive_pois_mastery(332)` ≠ 0 → both += pct(·, m, 100); n =
   `skill_poison_override_length(101)` if > 0, else `poisonlength(59)` /
   q with q = `poison_count(326)`, q < 2 → 1; *c := 2; *min += (n × mn)
   >> 8, *max += (n × mx) >> 8.
3. *max > 0 → *min := max(*min, 1), *max := max(*max, *min + 1).
4. S given with R `finishing` → charge damage §2.9 rule 2 into (*min,
   *max, *c).
5. `damblue` → *c := 3; `damred` → *c := 1 (as §2.3 rule 5).

Magic damage has no mastery here (the server uses 357, `combat/damage.md`
§3.1 rule 5).

#### 2.5 Skill elements `0x004E72D0` (U, &min, &max, &c; EBX L, EDI R)

`EType` (R +0x1DC) = 5 (poison): n = `elem_len(U, id, L, 1)`, a =
`elem_min(…, 1)` × n, b = `elem_max(…, 1)` × n; else a = `elem_min`, b
= `elem_max` (`skills/levels.md` §3). *min += a >> 8, *max += b >> 8.
*c := element color: 1 → 1, 2 → 9, 3 or 4 → 3, 5 → 2; other types
leave *c. The same map is `0x004E6FB0` ("EType color"; other types →
0).

#### 2.6 Weapon part `0x004E9660` (ESI s, EDI S, EBX &c, ECX W; stack U, &min, &max)

s = 0 → nothing. Else a = b = 0; `weapon_phys(U, &a, &b, c, 0, S, 128,
W, 0)` with E = 0 (§2.3; 128 = no scaling there); stat elements (U, &a,
&b, c, S) (§2.4); *min += a × s / 128, *max += b × s / 128. Callers pass
W = 0 (weapon in use) and s = `SrcDam`.

#### 2.7 Throw damage `0x004EA4A0`

ECX U, EDX &min; stack &max, &c, extra percent E, (unused), S, (unused),
W. *min := *max := 0.

1. S null; W null and no weapon in use; or W not `throwable`
   (`0x0062BA80`) → done.
2. W is a missile potion (item type 38 `tpot`, equivalence
   `0x00629BB0`): m = W's `missiletype` (`0x006288A0`); *c := 0; a =
   missile phys min + elemental min, b = phys max + elemental max (all
   `(0, U, m, 1)`, `0x0064AF20` + `0x0064B100`, `0x0064AFF0` +
   `0x0064B1D0`, `skills/levels.md` §3.4). Missile `EType`
   (`0x0064B0C0`): 1 → *c := 1; 2 → 4; 4 → 3; 5 → 2 and a, b /= max(1,
   `elem length` (`0x0064B2A0`) / 25). *min := a >> 8, *max := max(b >>
   8, *min).
3. Else: P = str × `StrBonus` / 100 + dex × `DexBonus` / 100 (when ≠ 0)
   + `damagepercent(25)`; P < −89 → −90. *min := t + (18 + P) × t / 100
   with t = `item_throw_mindamage(159)`, stat 18 = U's
   `item_mindamage_percent`; *max likewise with 160 and 17. q = E, plus
   `weapon_mastery(U, W, S, 1)` when U has state 78 (`throwingmastery`).
   *min += pct(*min, q, 100); *max += pct(*max, q, 100); *max :=
   max(*max, *min). Stat elements (§2.4) into fresh (a, b, c); b :=
   max(b, a); *min += a, *max += b.

#### 2.8 Dual-hand driver `0x004E93A0` and `0x004E99F0`

`0x004E93A0` (EAX U; stack &min1, &max1, &c1, &min2, &max2, &c2, S,
callback f, A, B):

1. U has no inventory, or cannot dual-wield (`0x006235A0`: player class
   4 or 6, or monster class 417–418) → returns 0, nothing written.
2. R4, R5 = items at body locations 4 and 5 (`0x0063BDE0`), each kept
   only if usable (`0x0062A4E0`) and not type 38 (`tpot`).
3. Both kept: O = swap to the other hand (§2.10); O found → f(U, &min2,
   &max2, &c2, A, B, S, 0, O), else min2 := max2 := 0. W = swap back
   (§2.10); W null → min1 := max1 := 0, done; else f(…, &min1, &max1,
   &c1, …, W).
4. Otherwise: W = weapon in use; usable → `0x00627910(U, W, 1)`; f(U,
   &min1, &max1, &c1, A, B, S, 0, 0).

Callbacks: `0x004E9610` = `weapon_phys(U, …, F0 = B, S, s = 0, W, 0)`
with E = A, then stat elements (U, …, S); `0x004EA4A0` = throw damage
with E = A; `0x004E8FE0` = attack rating `0x004E8EC0` (§2.11) with
(level B, S, W).

`0x004E99F0` (ECX S, EAX R; stack U, L, f, A, B, x1, y, x2, k): S, R or
f null → nothing. k ≠ 0 → skill elements (§2.5) into (e, E, c), else 0,
0, 0; c1 := c2 := c. Driver (rule 1–4). Each range i with mini ≠ 0 and
maxi ≠ 0: mini := mini × `SrcDam` / 128 + e, maxi := maxi × `SrcDam` /
128 + E. Draw (§2.2) with A = range 1 (weapon in use), B = range 2
(other hand). Edge case 3 (double `SrcDam`).

#### 2.9 Progressive charges (`finishing` skills)

States listed at data tables +0x16C (i16 ids, count i16 +0x170). For
each listed state the unit has (`0x00639DF0`) with a stat list
(`0x006256B0`): k = list stat 350 (`modifierlist_skill`), v = stat 351
(`modifierlist_level`), both > 0, k a valid skills row; a = k's
`aurastat1` (+0x54, ≥ 0 and a valid stat id); n = the list's value of
stat a > 0 (charges).

1. Charge bonus `0x004E6FF0` (no arguments; the local player
   `0x00463DD0`): Σ over those states whose k has `prgdam` (+0x44) = 1
   of (`Param1` + (v − 1) × `Param2` of k, `0x004E6CA0`) × n.
2. Charge damage `0x004E7120(U, &min, &max, &c)`: for every such state
   (any `prgdam`): k has no skilldesc row → *min += `elem_min(U, k, v,
   1)` >> 8, *max += `elem_max` >> 8. Else j = clamp(n, 1, 3); *min +=
   eval(`p{j}dmmin`), *max += eval(`p{j}dmmax`) (skilldesc evaluator
   `0x00646D00`, context k, v); element `p{j}dmelem` in 1–12 → *c :=
   EType color of it (§2.5).

#### 2.10 Hand swap `0x004E9260` / `0x004E9300` (EBX U)

Both need a dual-wield unit (§2.8 rule 1) with an inventory and a
weapon in use W. O = the item at location 5 if W is at 4, else 4
(`0x00627D40`). "Pair" = O ≠ none, O ≠ W, O is type 45 (`weap`), W and
O usable.

1. `0x004E9260`: pair → W attached (`0x00625820`) → `0x00627910(U, W,
   0)` (W's damage-related stats leave U's totals); O attached →
   `0x00627910(U, O, 1)`; return O. Else 0.
2. `0x004E9300`: pair → `0x00627910(U, O, 0)`, `0x00627910(U, W, 1)`;
   return W. No pair: W usable → `0x00627910(U, W, 1)`; return W.

#### 2.11 Attack-rating helpers

`AR(U, W, &v, &c, L, S)` = `0x004E8EC0` (ESI U, EAX W): U or S null, or
no R → return 0. c := 0; v := `attack_rating(U)` (`0x00622560`). W :=
W or the weapon in use; W of primary type 38 (`0x0062B400`) → v := 0.
P = `weapon_mastery(U, W, S, 0)` + `item_tohit_percent(119)`
(item/skill getter) + `to_hit(U, id, L)` (`0x006449F0`). R `finishing`
and `progressive_tohit(325)` ≠ 0 → P += it, c := 3. v += v × P / 100.
`attblue` state (flag list +0xF0, `0x0063A3C0`) → c := 3; `attred`
(+0xF4, `0x0063A3E0`) → c := 1. Return 1.

`ARthrow` = `0x004E9160`: as `AR` except: no null-unit test; weapon
mastery only when U has state 78 (`throwingmastery`); no `finishing`
term; c := 0 after the sum, before the state colors.

### 3. `descdam` entries (`0x0072D768`)

Live users: `skilldesc.txt` rows (count, first names). "Two lines" =
§2.2 with ranges A, B.

<!-- rows -->
| # | Address | Live users | Computes and draws |
|---|---|---|---|
| 1 | `0x004EA170` | 3: attack, whirlwind, maul | D required. A = eval D `ddam calc1` (+0x18), B = eval `ddam calc2` (+0x1C) (`0x00646D00`, context S, L). Dual-wield unit → `0x004E99F0`(f = `0x004E9610`, A, B, k = 1) (§2.8). Else entry 7 |
| 2 | `0x004EA450` | 1: kick | v = (R `MinDam` << ((`HitShift` − 8) & 31)) + U's `item_kickdamage(137)` (`0x00625500`); §2.1 with (v, v) color 0 → prints `v-(v+1)` |
| 3 | `0x004EA990` | 1: throw | Throw damage (§2.7, E = 0, weapon in use) → (a, b, c); a, b := × `SrcDam` / 128; skill elements (§2.5); §2.1 in c |
| 4 | `0x004EAA30` | 1: left hand throw | O = swap to other hand (§2.10); none → nothing drawn. Throw damage with W = O; swap back; §2.1 in c. No `SrcDam`, no skill elements |
| 5 | `0x004EAD60` | 57: magic arrow, fire arrow, cold arrow, multiple shot, power strike … | c = EType color; weapon part (§2.6) into (a, b, c); a += `phys_min(U, id, L, 0)` >> 8, b += `phys_max` >> 8; the §2.5 sums added inline (c not changed); §2.1 in c |
| 6 | `0x004E9D20` | 3: exploding arrow, immolation arrow, freezing arrow | Range A: weapon part (§2.6) with its own color cA (initially 0). Range B: (`phys_min(…, 0)` >> 8) + (`elem_min(…, 1)` >> 8), max likewise, plus U's stat damage of the skill's `EType` (`0x004E8D90`: 1 → 48/49, 2 → 50/51, 3 → 52/53, 4 or 12 → 54/55, 5 → (57, 58) × n >> 8 with n as §2.4 rule 2, no mastery); cB = EType color. Two lines |
| 7 | `0x004EA010` | 15: jab, impale, guided arrow, strafe, fend, sacrifice, zeal, charge, bash … | D required. A, B as entry 1. `weapon_phys(U, a, b, c, F0 = B, S, 128, 0, 0)` with E = A; stat elements (§2.4); `SrcDam` ≠ 128 → a := pct(a, `SrcDam`, 128), b likewise; a += `phys_min(…, 0)` >> 8 + `elem_min(…, 1)` >> 8, b += `phys_max` >> 8 + `elem_max` >> 8; §2.1 in c |
| 8 | `0x004EB290` | 3: inferno, arctic blast, inferno sentry | D required. m = `elem_min(…, 1)`, M = `elem_max(…, 1)`; A = eval `ddam calc1`, 0 → 1; B = eval `ddam calc2`, 0 → 1; min = (A × m × 25 / B) >> 8, max = (A × M × 25 / B) >> 8; §2.1 in EType color |
| 9 | `0x004EB390` | 3: blaze, fire wall, firestorm | min = ((`MinDam` << `HitShift`) + `elem_min(…, 1)`) × 75 >> 8, max = ((`MaxDam` << `HitShift`) + `elem_max`) × 75 >> 8 (shift count & 31); c = EType color; weapon part (§2.6); §2.1 in c |
| 10 | `0x004EB460` | 1: smite | Needs a shield (`0x0063C8F0`), else nothing. b, B = shield's items bytes +0xFE, +0xFF. U has skill 117 (`0x006439B0`) and state 101 (`holyshield`) → l = `skill_level(U, that entry, 1)`, b += `phys_min(U, 117, l, 1)` >> 8, B += `phys_max` >> 8. P = `Param3` + (L − 1) × `Param4` of the skill (`0x004CC7C0`) + str × `StrBonus` / 100 + dex × `DexBonus` / 100 (shield's bonuses) + `damagepercent(25)`; P < −89 → −90. min = (P × b + stat 18) / 100 + b, max = (P × B + stat 17) / 100 + B (Edge case 4). c: 0, `damblue` 3, `damred` 1. §2.1 |
| 11 | `0x004EB640` | 1: vengeance | c = EType color; weapon part (§2.6) into (a, b, c). Base: `0x00623990(U, 1)` ≠ 0 and grip 2 → b0, B0 = U's 23 / 24, else 21 / 22; b0 ≥ 1, B0 ≥ 2, B0 ≥ b0. For (`calc1`, mastery 329), (`calc2`, 331), (`calc3`, 330): p = skills evaluator (`0x00646CA0`, R +0x138 / +0x13C / +0x140, context id, L); p ≠ 0 and mastery m ≠ 0 → p += pct(p, m, 100); a += pct(b0, p, 100), b += pct(B0, p, 100). c = 0 → 3. §2.1 |
| 12 | `0x004EB830` | 1: blessed hammer | Skill elements (§2.5) → (a, b, c). Expansion game (`0x0044DCC0`) → p = concentration (`0x006461D0(U, id)`, `skills/levels.md` §3.5); else p = `damagepercent(25)`, < −89 → −90. a += p × a / 100, b += p × b / 100. §2.1 in c |
| 13 | `0x004EB240` | 1: double throw | A = skills evaluator of `calc1` (R +0x138). `0x004E99F0`(f = throw damage `0x004EA4A0`, A, 0, k = 1) (§2.8) |
| 14 | `0x004EB940` | 1: hunger | `weapon_phys(U, a, b, c, 0, S, 0, 0, 0)` with E = 0; q = skill's `Param5` (+0x158; invalid id → 0); a += q × a / 100, b += q × b / 100; stat elements (§2.4); §2.1 in c |
| 15 | `0x004EBA30` | 2: dragon talon, dragon flight | s = `phys_min(…, 1)` >> 8, S′ = `phys_max(…, 1)` >> 8. p = charge bonus (§2.9 rule 1) + `Param1` + (L − 1) × `Param2` (`0x004E6CA0`); p ≠ 0 → c := 3. Kick (`0x00646280(U, &k, &K, &kp)`, kp starts at p). min = k + pct(k, kp, 100) + s + pct(s, p, 100); max = K + pct(K, kp, 100) + S′ + pct(S′, p, 100). Charge damage (§2.9 rule 2) into fresh (a, b, c); min += a, max += b. §2.1 in c |
| 16 | `0x004EBB50` | 1: dragon tail | c = EType color. s, S′ = `phys_min/max(…, 0)` >> 8; t = charge bonus; kick with kp starting at t; min = k + pct(k, kp, 100) + s + pct(s, t, 100), max likewise. p = skills evaluator of `calc1`; min += pct(min, p, 100), max += pct(max, p, 100). With an inventory: W in use and O (other location, as §2.10): W `weap`, usable and attached → `0x00627910(U, W, 0)`; O ≠ W likewise → `0x00627910(U, O, 0)`; stat elements (§2.4) into (min, max, c); W (if switched) → `0x00627910(U, W, 1)`; O is not restored (Edge case 5). §2.1 in c |
| 17 | `0x004EBD50` | 2: charged strike, lightning strike | D required. A, B as entry 1. Range A: `weapon_phys(U, …, F0 = B, S, 128, 0, 0)` with E = A, stat elements (§2.4), color cA. Range B: skill elements (§2.5), cB. Two lines |
| 18 | `0x004EC040` | 1: royal strike | D required; then entry 7 |
| 19 | `0x004EA310` | 3: double swing, frenzy, dragon claw | D required; A, B as entry 1. Dual-wield unit → `0x004E99F0`(f = `0x004E9610`, A, B, k = 1). Else `0x004E9610` with weapon in use (E = A, F0 = B, s = `SrcDam`), §2.1 in c |
| 20 | `0x004EA240` | 3: fists of fire, claws of thunder, blades of ice | As entry 1 with k = 0 (no skill elements) |
| 21 | `0x004EA790` | 1: lightning bolt | Needs a throwable weapon in use, else nothing. P = str × `StrBonus` / 100 + dex × `DexBonus` / 100 + `damagepercent(25)`, < −89 → −90; a = t + (stat 18 + P) × t / 100 (t = stat 159), b likewise (160, 17). q = `weapon_mastery(U, W, S, 1)` with state 78, else 0; a += pct(a, q, 100), b += pct(b, q, 100). U's stat damage of `EType` (as entry 6). b := max(b, a). a, b := pct(·, `SrcDam`, 128). Skill elements (§2.5, c from 0). §2.1 in c |
| 22 | `0x004EAAA0` | 3: poison javelin, plague javelin, lightning fury | Range A: throw damage (§2.7, E = 0) then × `SrcDam` / 128 (pct), cA. Range B: skill elements (§2.5), cB. Two lines |
| 23 | `0x004EAF90` | 1: rabies | Range A: `weapon_phys(U, …, 0, S, 128, 0, 0)` with E = 0, stat elements, cA. Range B: skill elements, cB. Two lines |
| 24 | `0x004EAE70` | 1: blade fury | Same sum and color as entry 5 (physical first, then the weapon part, then skill elements) |

### 4. `descatt` entries (`0x0072D7F8`)

<!-- rows -->
| # | Address | Live users | Computes |
|---|---|---|---|
| 1 | `0x004E9000` | 28: magic arrow, fire arrow, jab, cold arrow, multiple shot, power strike … | (v1, c1) := `AR(U, weapon in use, …, L, S)` (§2.11); v2 = c2 = 0 |
| 2 | `0x004E9040` | 16: attack, zeal, charge, double swing, double throw, leap attack, frenzy … | v1 = c1 = 0; S null → done. No weapon in use → v1 = `attack_rating(U)`. Else: r from `0x00625820(W, &r)`; `0x00627910(U, W, 0)`; v1 = `attack_rating(U)`; then W was attached → `0x00627910(U, W, r)`, else detached (`0x006277F0`). P = `item_tohit_percent(119)` + `to_hit(U, id, L)`; `finishing` and stat 325 ≠ 0 → P += it, c1 := 3. v1 += v1 × P / 100; `attblue` → 3; `attred` → 1. No weapon mastery |
| 3 | `0x004E9510` | 1: throw | Dual-wield unit → driver (§2.8) with f = `0x004E8FE0`: (v1, c1) for the weapon in use, (v2, c2) for the other hand when both hands qualify. Else as entry 1 |
| 4 | `0x004E9590` | 1: left hand throw | (v1, c1) := `ARthrow(U, weapon in use, …)` (§2.11); v2 = c2 = 0 |
| 5 | `0x004E95C0` | 3: dragon talon, dragon tail, dragon flight | Driver as entry 3, then v1 := v2, c1 := c2, v2 := c2 := 0: only the other-hand AR, and nothing without dual weapons (Edge case 6) |

## Constants & data dependencies

- Image tables: `0x0072D768` (36 × u32), `0x0072D7F8` (18 × u32),
  `0x006DBEE4` (6 × u32: colors 1, 9, 3, 3), `0x006DBEF4` (4 × (min,
  max, mastery): (48, 49, 329), (50, 51, 330), (54, 55, 331), (52, 53,
  −1)), bit masks `0x006CE268` (`[0x006CE274]` = 8), `[0x0072D8D0]` =
  2, `[0x0072D8D4]` = −6 / −7 (set at run time).
- `skilldesc.txt`: `descdam`, `descatt`, `ddam calc1/2`, `p1–3dmmin`,
  `p1–3dmmax`, `p1–3dmelem`. `skills.txt`: `SrcDam`, `HitShift`,
  `MinDam`, `MaxDam`, `EType`, `Param1–5`, `calc1–3`, `prgdam`,
  `aurastat1`, `finishing`. Weapons: `StrBonus`, `DexBonus`; shields /
  boots: bytes +0xFE / +0xFF (`armor.txt` min / max damage).
- Stats: 0, 2, 17, 18, 21–25, 48–59, 101, 111, 119, 137, 159, 160, 325,
  326, 329–332, 350, 351. States: 78 `throwingmastery`, 101
  `holyshield`; state flags `damblue`, `damred`, `attblue`, `attred`.

## Randomness

None.

## Edge cases & original bugs

1. Bound tests 0x90 / 0x48 exceed the 36 / 18 table words: a `descdam`
   36–53 reads `descatt` words (called with the wrong argument layout),
   54+ reads the rectangle table `0x0072D840` as code addresses; a
   `descatt` 18+ reads the rectangle table too. Live data uses ≤ 24 and
   ≤ 5; d2rs treats `descdam` ≥ 25 and `descatt` ≥ 6 as a data
   error (no live row).
2. §2.1 never prints a single number: min = max = v prints `v-(v+1)`
   (kick always does). The two-line format (§2.2) prints `%d`.
3. `0x004E99F0` scales the weapon range by `SrcDam` / 128 a second time
   (`weapon_phys` already applied it). Live users of entries 1, 19, 20
   all have `SrcDam` 128, so it has no visible effect in 1.14d.
4. Smite (entry 10) adds `item_min/maxdamage_percent` before the / 100
   without multiplying them by the damage: they change the value by at
   most a point.
5. Dragon tail (entry 16) switches the other-hand weapon's list off and
   never back on (cf. `skills/levels.md` §3.5 kick, Edge case 6).
6. `descatt` 5 reports only the other-hand weapon's AR and reports
   nothing for a single weapon.

## Test vectors

Synthetic (unit tests of the formulas):

| Input | Expected | Source |
|---|---|---|
| §2.1 (5, 5) | `5-6` | §2.1 r1 |
| §2.1 (8, 3) | `8-9` | §2.1 r1 |
| §2.1 (3, 8) | `3-8` | §2.1 r2 |
| §2.1 (9999, 12345) | `9999-12K` | §2.1 r2 |
| §2.1 (12345, 23456) | `12K-23K` | §2.1 r2 |
| §2.2 range (7, 7) | `7` | §2.2 |
| entry 8: m = 2560, M = 5120, calc1 = 0, calc2 = 0 | 250-500 | §3 row 8 |
| entry 10: b 10, B 20, P 50, stats 18 = 17 = 100 | min 16, max 31 | §3 row 10 |
| `AR`: attack_rating 100, P = 20, no states | v 120, c 0 | §2.11 |
| entry 9: MinDam 4, MaxDam 8, HitShift 4, elem 0/0, SrcDam 0 | 18-37 | §3 row 9 |

Live values: Open question 1.

## Provenance

Read from the 1.14d disassembly (`re/exports/all.asm`) and the exported
function texts: tables `0x0072D768` / `0x0072D7F8` / `0x006DBEE4` /
`0x006DBEF4` / `0x006CE268` read from the `Game.exe` image; callers by a
scan of `all.asm` for the table addresses (only `0x004ED570`,
`0x004EDA80`). Entries: the 29 addresses in §3–§4. Helpers:
`0x004E96E0`, `0x004E9870`, `0x004E86F0`, `0x004E89A0`, `0x004E72D0`,
`0x004E6FB0`, `0x004E9660`, `0x004EA4A0`, `0x004E93A0`, `0x004E99F0`,
`0x004E9610`, `0x004E8FE0`, `0x004E8EC0`, `0x004E9160`, `0x004E9260`,
`0x004E9300`, `0x004E6FF0`, `0x004E7120`, `0x004E6CA0`, `0x004CC7C0`,
`0x004E8D90`, `0x006235A0`, `0x0063A130` (state flag list test; list
for flag bit b at data tables +0xCC + 4b, so `damblue` 7 → +0xE8,
`attblue` 9 → +0xF0). Register arguments read from the call sites'
assembly where the decompiler lost them. Live-user counts from the
1.14d `skilldesc.txt` (`patch_d2`). D2MOO not used.

## Open questions

1. No capture: record the character panel's damage and attack-rating
   lines for one skill of each `descdam` 1–24 and `descatt` 1–5 with
   known stats (PC 2 recording list, `docs/HANDOFF.md` §7).
2. `ui/panels-2.md` §17.5 names the title string at skilldesc +0x0E
   `str name`; `data/fields.tsv` puts `str alt` at +0x0E (`str name` at
   +0x08). `0x004E6E30` reads +0x0E. Settle the column name with a
   capture of a skill whose `str name` and `str alt` differ.
