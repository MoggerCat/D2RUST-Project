# Spec: Missiles — damage setup at creation

- **Status:** draft: read from the 1.14d `Game.exe` code
  (`0x0059F900`, `0x0064B860`, `0x0064A850`, `0x0064A930`, `0x0064AA60`,
  `0x0064ABA0`, `0x0064AC60`; addresses per rule); no trace check yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::missiles` (the `MissileCombat::damage_setup`
  seam; today the no-op `Pending::missile_damage_setup`)
- **Related specs:** `missiles/missiles.md` §R2.3 step 23 (the caller),
  §R6 (reads the stats back); `skills/levels.md` §3.1–3.4 (skill and
  missile damage formulas, `weapon_mastery`); `combat/damage.md` §0
  (`pct`), §3.1 (the same weapon switch / restore and bonus rules for
  melee); `skills/bodies-2.md` (weapon switch `0x00535D10` / restore
  `0x00535E20`); `sim/units.md` (attack weapon `0x00623990`, dual-wield
  toggle `0x00623C80`); `sim/stats.md` (unit getter `0x00625480`, item /
  skill getter `0x00625500`, set `0x00627260`); `sim/rng.md` §3 (unit
  seed).

## Summary

Every new missile gets its damage as missile stats at creation
(`missiles.md` §R2.3 step 23). `0x0059F900` fills a 0x7C-byte damage
record from the missile row, its skill row and the owner's (or the
origin's) stats (`0x0064B860`, D2MOO `MISSILE_CalculateDamageData`),
then writes the record as stats on the missile (`0x0064AC60`, D2MOO
`MISSILE_SetDamageStats`). The hit code later reads only those stats
(§R6). Up to three unit-seed draws happen (§3).

## Inputs

| Name | Type | Source |
|---|---|---|
| owner | unit | creation parameters +0x04 (ESI) |
| origin | unit or none | creation parameters +0x08 (stack) |
| missile | unit (type 3) | the new missile (EBX) |
| level | i32 | creation parameters +0x30 (EDI); the skill level |
| missile row | `missiles.txt` | by the missile's class |
| skill row | `skills.txt` | missile row `Skill`, or the missile's skill (§1 step 3) |

## Outputs / state changes

Missile stats (§2), missile data flags +0x14 bits 1 / 2, and the
owner's / origin's unit seed (§3).

## Rules

### 1. Damage record `0x0064B860(rec, owner, origin, missile, level)`

Record (i32 fields, offset → meaning): +0x00 flags; +0x04/+0x08 phys
min/max; +0x0C/+0x10 fire min/max, +0x14 fire length; +0x18/+0x1C
lightning; +0x20/+0x24 magic; +0x28/+0x2C cold, +0x30 cold length;
+0x34/+0x38 poison, +0x3C poison length, +0x40 poison count;
+0x44/+0x48 life drain; +0x4C/+0x50 mana drain; +0x54/+0x58 stamina
drain; +0x5C stun length; +0x60/+0x64 burning min/max, +0x68 burn
length; +0x6C demon %, +0x70 undead %, +0x74 target AC; +0x78 damage %.

1. Zero the record. Stop (record stays zero) unless owner ≠ none,
   missile ≠ none, missile type = 3 and 0 ≤ class < missile row count.
2. **Plain missile** (row `Skill` (+0x194, i16) < 1 and flag
   `MissileSkill` (bit 15) clear): `S = SrcDamage` (+0x12D, u8), `M =
   SrcMissDmg` (+0x12E, u8); `pmin/pmax` = missile physical min/max
   `0x0064AF20` / `0x0064AFF0(missile, owner, class, level)`; `E =
   EType` (+0xE4); `emin/emax` = `0x0064B100` / `0x0064B1D0`, `len =
   0x0064B2A0` (same arguments; all `skills/levels.md` §3.4). If flag
   `ApplyMastery` (bit 8): `emin += mastery(owner, E, emin)`, then `emax
   += mastery(owner, E, emax)` (`0x0064ABA0`: E 1 → stat 329, 2 → 330,
   4 or 12 → 331, 5 → 332, other → 0; stat ≠ 0 → `pct(v, stat, 100)`).
3. **Skill missile** (otherwise): `k = Skill`; with `MissileSkill`, `k
   = ` the missile's stored skill (`0x0064A280`, data +0x0A, i16) when
   that is > 0, else the row's `Skill`. Skill row missing → stop (record
   zero). `S = SrcDam` (skill +0x1A5, u8), but 0 when the missile row's
   `SrcDamage` is 255; `M = 0`. `pmin/pmax = phys_min/phys_max(owner, k,
   level, 0)` (`0x00647BC0` / `0x00647D00`); `E = EType` (skill +0x1DC);
   `emin/emax = elem_min/elem_max(owner, k, level, 1)` (`0x00644D50` /
   `0x00644E40`, mastery inside); `len = elem_len(owner, k, level)`
   (`0x00644F20`).
4. Missile row `Holy` (+0x12F) bit 1 → flags |= 0x100; bit 2 → 0x200;
   bit 4 → 0x400.
5. Locals `p = 0` (percent), `b = 0`, `mn% = 0`, `mx% = 0`, `smin = smax
   = 0`.
6. **S ≠ 0** (owner-sourced): flags |= 1. Weapon W: owner type 0 →
   attack weapon `0x00623990(owner, 1)`; type 1 with an inventory →
   `0x00622830` (the weapon pick `0x0063C9B0`); else none.
   `bonus(owner, W)` (§3) true → flags |= 2. Then:
   1. W is an item (type 4): if `Half2HSrc` (bit 14) and W is
      two-handed (`0x006289C0`): `S = S / 2` (unsigned). `b =
      weapon_mastery(owner, W, 0, 1)` (`0x00645830`); `StrBonus`
      (`0x00629860`, i16) ≠ 0 → `b += strength(0) × StrBonus / 100`;
      `DexBonus` (`0x006298A0`) ≠ 0 → `b += dexterity(2) × DexBonus /
      100`. Base: W throwable (`0x0062BA80`) → stats 159/160; else owner
      inventory present and wield type `0x0063D340` = 2 → 23/24; else
      21/22 (owner unit getter), each `<< 8`. W of item type 57 (blunt,
      `0x00629BB0`) → +0x70 += 50.
   2. No item: owner is a monster and a hireling (`0x0063EE90`) → `p =
      dexterity(2)`. Base = `mindamage(21) << 8`, `maxdamage(22) << 8`.
   3. `smin = trunc(base_min × S / 128)`, `smax` likewise (S after the
      halving).
   4. `p += damagepercent(25) + b`; `mn% = stat 18`, `mx% = stat 17`;
      +0x6C += stat 121, +0x70 += 122, +0x74 += 120; stats 103, 104,
      106 ≠ 0 → flags |= 0x100, 0x200, 0x400 (all owner, unit getter).
7. **S = 0, M ≠ 0 and origin ≠ none** (origin-sourced): `bonus(origin,
   none)` true → flags |= 2. `smin = trunc(mindamage(21) × M / 128)`,
   `smax` from 22 (origin, not shifted); `p = stat 25`, `mn% = 18`,
   `mx% = 17`; +0x6C/+0x70/+0x74 += origin stats 121/122/120.
8. +0x78 := `max(p + missile damagepercent(25) + max(mn%, mx%), −90)`.
   +0x04 := `pmin + smin`; +0x08 := `pmax + smax`.
9. Element by E: 1 fire → +0x0C/+0x10 = emin/emax, +0x14 = len; 2
   lightning → +0x18/+0x1C; 3 magic → +0x20/+0x24; 4 cold, 12 freeze →
   +0x28/+0x2C, +0x30 = len; 5 poison → +0x34/+0x38, +0x3C = len; 6
   life → +0x44/+0x48; 7 mana → +0x4C/+0x50; 8 stamina → +0x54/+0x58;
   9 stun → +0x5C = len; 11 burn → +0x60/+0x64, +0x68 = len; 0, 10 and
   others: nothing.
10. S ≠ 0: dual-wield toggle `0x00623C80(owner, 1)`; `add(owner, 8)`;
    `scale(S)`; `0x00622E90` (empty in 1.14d). Else M ≠ 0 and origin:
    `add(origin, 0)`; `scale(M)`.

`add(u, sh)` = `0x0064A930`: fire min/max += stats 48/49 `<< sh`,
lightning 50/51, magic 52/53, cold 54/55 likewise; cold length += 56;
poison min/max += 57/58 (never shifted); `o = stat 101`: `o ≥ 1` →
poison length += o, else poison length += 59 and poison count += 326;
life drain min += 60; mana drain min += 62; burning min/max += 316/317;
burn length += 315. (Fire length, the drain maxima and stamina are not
touched.)

`scale(s)` = `0x0064AA60`: s = 128 → nothing; else each of fire
min/max, lightning, magic, cold min/max, cold length, poison min/max,
poison count, life drain min, mana drain min, burning min/max, burn
length := `trunc(v × s / 128)`. Not scaled: fire length, poison length.

### 2. Stats `0x0064AC60(owner, missile, rec, level)`

Missile type ≠ 3 → nothing. Missile data flags (+0x14): rec flags 1 →
|= 1, 4 → |= 2 (`0x0064B860` never sets 4). Then set (`0x00627260`, in
this order): 12 := level; 21, 22; 48, 49, 315 := fire length; 50, 51;
52, 53; 54, 55, 56; 57, 58, 59, 326; 60, 61; 62, 63; 64, 65; 66; 316,
317, **315 := burn length** (overwrites the fire length, Edge case 1);
121, 122, 120; 25. Flags 0x100 → 103 := 1; 0x200 → 104 := 1; 0x400 →
106 := 1; 2 → 141 := 1. Owner ≠ none: every `damage_vs_montype(180)`
entry of the owner (`0x006261D0`, up to 128) is set on the missile with
its layer.

### 3. Bonus check `bonus(u, W)` = `0x0064A850` (u in ESI)

1. `c = stat 337` (unit getter); draw; `r < c` → true.
2. `d = stat 141` (item / skill getter `0x00625500`); `d ≠ 0`: draw;
   `r < d` → true.
3. W ≠ none and `m = weapon_mastery(u, W, 0, 2)` ≠ 0: draw; `r < m` →
   true.
4. False.

A draw is one step of u's unit seed (+0x20/+0x24, `rng.md` §3) and `r =
lo′ mod 100` (unsigned), compared signed.

### 4. Wrapper `0x0059F900` (only caller `0x0059FE55`)

Weapon switch `0x00535D10(owner, 0)`; §1 into a stack record; §2;
restore `0x00535E20(owner, 0)`.

## Constants & data dependencies

`missiles.txt`: `Skill` +0x194, flags +0x04 (`ApplyMastery` 8,
`Half2HSrc` 14, `MissileSkill` 15; masks from table `0x006CE268`),
`SrcDamage` +0x12D, `SrcMissDmg` +0x12E, `Holy` +0x12F, `EType` +0xE4
and the damage columns of `skills/levels.md` §3.4. `skills.txt`:
`SrcDam` +0x1A5, `EType` +0x1DC and the columns of §3.1–3.3 there.

## Randomness

In order: the formula evaluations of §1 steps 2–3 draw only as
`skills/levels.md` §3 says; then `bonus` (§3): 1 draw always, +1 when
`item_deadlystrike` ≠ 0, +1 when a weapon and its crit mastery ≠ 0 —
on the owner's seed (S ≠ 0) or the origin's (M ≠ 0, never the third).
None when S = 0 and (M = 0 or no origin). §2 does not draw.

## Edge cases & original bugs

1. Stat 315 is written twice; the burn length wins, so a fire-element
   missile keeps only `firelength(315)` added from the owner, never its
   own `ELen` (reproduce).
2. In a plain missile `SrcDamage` 255 is used as 255/128; only skill
   missiles treat 255 as "none".
3. `Half2HSrc` halves S before the base and the final `scale`.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| skill missile, S = 128, no weapon, owner 21/22 = 2/5, stat 25 = 0, missile stat 25 = 0, E = 0 | +0x04 = pmin + 512, +0x08 = pmax + 1280, +0x78 = 0, one owner draw | §1 steps 6–8 |
| plain missile, S = 0, M = 64, origin 21/22 = 10/20 | smin 5, smax 10; `scale(64)` halves the added origin elements | §1 step 7, 10 |

## Provenance

1.14d `Game.exe`: `0x0059F900` (asm: owner ESI, origin stack, missile
EBX, level EDI from the creation record at `0x0059FE4B`–`0x0059FE55`),
`0x0064B860` (register args to `0x0064A850` ESI = owner / origin at
`0x0064BAD1` / `0x0064BD13`; `add` / `scale` at `0x0064BEC4`–
`0x0064BF14`), `0x0064ABA0` (`pct` via `0x00483360`), `0x0064A930`,
`0x0064AA60`, `0x0064AC60`, `0x0064A280`, `0x00622830`, `0x00622E90`
(empty). D2MOO 1.10f `Missile.cpp` names the functions (hint only); every
field, order and draw above is read from the 1.14d code.

## Open questions

None.
