# Spec: Combat — Unit event functions (table `0x007325B0`)

- **Status:** draft: every function read from the 1.14d `Game.exe`
  disassembly (addresses below); D2MOO (1.10f) used only for names. No
  trace check yet (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::combat::events`
- **Related specs:** `combat/damage.md` (§5.4 when events fire, §8 the
  table and functions 15 / 16, `apply` §5.2, reaction §7.1, record §1);
  `skills/bodies.md` (§2.13 registration, §2.18 iteration, §2.7
  `apply_state`, the curse per-unit step); `skills/levels.md` (§2
  special values `eval`, §3.6 damage rolls); `sim/rng.md` (`roll`,
  `mask`); `monsters/ai.md` (terror install, AI state).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 33–42 |
| Inputs | 43–56 |
| Outputs / state changes | 57–61 |
| Rules | 62–63 |
|   1. Shared definitions | 64–82 |
|   2. Functions | 83–346 |
|   3. Item cast | 347–395 |
| Constants & data dependencies | 396–414 |
| Randomness | 415–431 |
| Edge cases & original bugs | 432–446 |
| Test vectors | 447–457 |
| Provenance | 458–472 |
| Open questions | 473–481 |
<!-- /index -->

## Summary

Items (`itemstatcost` `itemevent1/2` + `itemeventfunc1/2`) and skills
(`auraevent1–3` + `auraeventfunc1–3`) register event functions on a
unit (`skills/bodies.md` §2.13). When the unit's event fires
(`combat/damage.md` §5.4) each matching function runs once with the
holder, the other unit, the damage record and the registration's
skill and level. This spec states functions 1–14 and 17–31; 15 (open
wounds) and 16 (crushing blow) are in `combat/damage.md` §8.

## Inputs

| Name | Source |
|---|---|
| ECX game, EDX event id | `0x005C0C30` (`skills/bodies.md` §2.18) |
| H (stack 1) | the unit the handler is registered on (the holder) |
| O (stack 2) | the other unit given to the event (`combat/damage.md` §5.4: for 1 / 2 the attacker, for 5 / 6 / 7 / 8 the defender, for 9 the victim, for 0 the missile; may be none) |
| R (stack 3) | the damage record (`combat/damage.md` §1; may be none) |
| k (stack 4) | the registration's skill field: item events (stat << 16 \| layer); skill events: the skill id |
| L (stack 5) | the registration's level |

Every function returns 1 when it acted, else 0 (the iteration keeps
only the last result).

## Outputs / state changes

Stat writes, state lists, damage applied through `apply`, missiles,
S→C 0x99 / 0x9A queue entries, a revive. Listed per function.

## Rules

### 1. Shared definitions

- v = H's item/skill value of stat (k >> 16) at layer (k & 0xFFFF)
  (`0x00625500`) — the item-event chance or amount.
- (s, l) = (layer >> shift, layer & mask) with the global layer split
  (data +0xC6C / +0xC70; `items/properties.md` §5 rule 9).
- "Seed draw" = one step of H's unit seed (+0x20, `sim/rng.md`):
  `roll(100)` = `lo' mod 100`, `mask(128)` = `lo' & 127`.
- "Heal(U, x)" `0x005C5F10` (EBX U, EAX x): x ≤ 0 → 0. life := U's
  life (6, total) + x; above max life (`0x00625D10`) → x −= excess,
  life := max. Set stat 6 := life. Returns x (the amount applied).
- "Elemental record" = zeroed 0x70-byte record with result flags
  (+0x04) 0x4021 (hit, no events, soft hit), one element field, hit
  class (+0x60); then `apply(game, H, O, missile 1, R')` (§5.2) and
  reaction `0x0057CEE0(game, H, O, R')` (§7.1). H is the attacker.
- "Item cast" `cast(U, s, l, T, aim)` = `0x005FDCA0` and
  `cast_point(U, s, l, x, y, aim)` = `0x005FDD60`: §3.
- Levels: stat 12 totals.

### 2. Functions

| # | 1.14d | Event (registrant) | Rule |
|---|---|---|---|
| 1 | `0x005CAB40` | hitbymissile (Chilling Armor) | §2.1 |
| 2 | `0x005CAC50` | damagedinmelee (Frozen Armor) | §2.2 |
| 3 | `0x005CAD40` | attackedinmelee (Shiver Armor) | §2.3 |
| 4 | `0x005C5F50` | domeleedamage (Iron Maiden) | §2.4 |
| 5 | `0x005C61E0` | damagedinmelee, damagedbymissile (Life Tap) | §2.5 |
| 6 | `0x005BF670` | damagedinmelee (`item_attackertakesdamage`) | §2.6 |
| 7 | `0x005BF960` | domeleedamage, domissiledamage (`item_knockback`) | §2.7 |
| 8 | `0x005BFA10` | same (`item_howl`) | §2.8 |
| 9 | `0x005BFAA0` | same (`item_stupidity`) | §2.9 |
| 10 | `0x005BF720` | damagedinmelee (`item_attackertakeslightdamage`) | §2.6 |
| 11 | `0x005BF7D0` | none in 1.14d data | §2.6 |
| 12 | `0x005BF880` | none in 1.14d data | §2.6 |
| 13 | `0x005BFBB0` | damagedinmelee, damagedbymissile (`item_damagetomana`) | §2.10 |
| 14 | `0x005BFC60` | domeleedamage, domissiledamage (`item_freeze`) | §2.11 |
| 17 | `0x005C0130` | kill (`item_manaafterkill`) | §2.12 |
| 18 | `0x005C0260` | kill (`item_healafterdemonkill`) | §2.12 |
| 19 | `0x005C0310` | domeleedamage, domissiledamage (`item_slow`) | §2.13 |
| 20 | `0x005C0470` | domeleeattack, domissileattack, kill, domeleedamage, domissiledamage (`item_skillonattack` / `onkill` / `onhit`) | §2.14 |
| 21 | `0x005C05A0` | damagedinmelee, damagedbymissile (`item_skillongethit`) | §2.15 |
| 22 | `0x005C6370` | absorbdamage (Bone Armor) | §2.16 |
| 23 | `0x005C6530` | domeleedamage (BloodGolem) | §2.17 |
| 24 | `0x005CA6C0` | absorbdamage (Energy Shield) | §2.18 |
| 25 | `0x005C87A0` | absorbdamage (Cyclone Armor) | §2.16 |
| 26 | `0x005C6690` | damagedinmelee, damagedbymissile (BloodGolem) | §2.19 |
| 27 | `0x005C6760` | damagedinmelee (Clay Golem) | §2.13 |
| 28 | `0x005C01D0` | kill (`item_healafterkill`) | §2.12 |
| 29 | `0x005C0770` | kill (`item_restinpeace`) | §2.20 |
| 30 | `0x005C0670` | killed, levelup (`item_skillondeath` / `onlevelup`) | §2.14 |
| 31 | `0x005C09E0` | kill (`item_reanimate`) | §2.21 |

Skill events read the skill record of k (invalid → 0) and use L := 1
when L < 0, unless stated.

#### 2.1 Chilling Armor

O (the missile) and H present. P := O's owner (`0x00552FD0`); none, or
H may not attack P (`combat/hit.md` §7.1) → 0. m := `srvmissilea`
(+0x48) must be a valid missile. O's missile row must have `ReturnFire`
(flags bit 9; `0x0046ACE0`). Create a missile (`0x0059FA30`,
`missiles/missiles.md` request): flags 0x20, owner H, origin H, class m,
target (P x, P y) absolute, skill k, level L. Return 1.

#### 2.2 Frozen Armor

H present; O a player or monster; R none or R physical (+0x08) > 0.
Record R' zeroed, result flags 0x20, freeze length (+0x34) := `eval(
calc1, k, L)` with unit H (`0x00646CA0`). `apply(game, H, O, 1, R')`.
`cltoverlaya` (+0x110) valid (< overlay count) → overlay on O. Return 1.

#### 2.3 Shiver Armor

H present; O a player or monster. R' zeroed; element roll `0x0056E0C0
(H, R', k, L)`: len := `elem_len`, lo := `elem_min`, hi := `elem_max`
(all with flag 1; `skills/levels.md` §3), x := lo + `roll(hi − lo)`
(H's seed, `0x0045C3E0`), `add_element(H, R', EType (+0x1DC), x, len,
0, 0)` (`skills/levels.md` §3.6). Then hit class |= 0xD, +0x64 := 1,
result flags := 0x4021; apply and reaction as in §1 (attacker H);
`cltoverlaya` on O. Return 1.

#### 2.4 Iron Maiden

O present; H a living player or monster; R present with physical > 0;
`auratargetstate` (+0x82) valid, H has it, and its state list's owner
(type / GUID) resolves to unit C (`0x00552F60`). p := `eval(calc, k, L)`
with unit C where calc = `calc1` (+0x138) when H is a player that is
not a hireling, else `calc2` (+0x13C) when O is a player, else `calc3`
(+0x140). R' zeroed; physical := pct(R physical, p, 100); result
flags |= `ResultFlags` (+0x12E) | 0x20; hit flags |= `HitFlags`
(+0x130); hit class := `HitClass` (+0x134). `apply(game, O, H, 1,
R')` and reaction `(game, O, H, R')` — the damage goes back to H with
O as attacker.

Then, only when O is a monster of class 290 (`bloodgolem`): d :=
O's monstats `Drain` of the game's difficulty (+0xA0 + difficulty; ≤ 0
→ stop); O's alignment must be 0; x := R' physical (after `apply`),
pct(x, d, 100) when d ≠ 100. Q := H's owner (`0x0058F0D0`); Q present
and dead → stop; H dead → stop. x := pct(x, 20, 100); x ≤ 0 → stop. Q
present → x −= Heal(Q, pct(x, 50, 100)). x −= Heal(H, x). Q present and
x > 0 → Heal(Q, x). Overlay 151 on H and, with Q, on Q. (The healed
units are H and H's owner, not the golem; Edge case 1.) Return 1.

#### 2.5 Life Tap

H and O players or monsters, O alive; R present with physical > 0;
`auratargetstate` valid, on H, list owner → C. x := pct(R physical,
eval(calc1, k, L) with unit C, 100). l := O's life, M := O's max life;
l ≤ 0 or M ≤ 0 → 0. O's life := clamp(l + x, 1, M). `prgoverlay`
(+0x10C) > 0 and valid → overlay on O. Return 1.

#### 2.6 Attacker takes damage (6, 10, 11, 12)

H and O present; O unit flag 0x4 (+0xC4); v > 0. Elemental record
with field and hit class: 6 physical (+0x08) := v << 8, class 0x8D; 10
lightning (+0x1C), 0x4D; 11 fire (+0x10), 0x2D; 12 cold (+0x24), 0x3D
with cold length (+0x30) := 25, or 10 × (H level − O level) + 25 when
H's level is higher. Apply and reaction (attacker H, defender O).
Return 1.

#### 2.7 Knockback

H, O and R present; v > 0. t := 0x40; O a monster with a monstats2
row: `large` (bit 11) → 0x20, else `small` (bit 10) → 0x80. Seed draw
`mask(128)` < t → R result flags |= 8 (knockback), return 1.

#### 2.8 Howl

O a monster without type flag 4 or 8 (champion, unique;
`0x005A0180(O, 12)`); v > 0; seed draw `mask(128)` < v → terror install
`0x005DDD00(game, H, O, skill 130 Howl, 20, 20)` (`monsters/ai.md`).
Return 1.

#### 2.9 Stupidity

O a monster; v ≠ 0. c := 5 × (H level + 4v − O level + 6); event 6
(domissiledamage) and c > 0 → c := c / 3 (toward zero). c := clamp(c,
1, 99). r := `roll(100)` (H's seed); r ≥ c → 0. d := (c − r) / 5 + 1
(toward zero), clamp 1…20. Dim Vision curse `0x005C39C0(game, H, O,
d)`: skill 71 (needs skills count > 71); context as the curse cast
(`skills/bodies.md`, curse body steps 4–5) with ai = 1, skill 71,
level d, duration `eval(auralencalc, 71, d)` with unit H divided by
`AiCurseDivisor` (when ≠ 0), stats `aurastat1–6` / `eval(
aurastatcalc_i)`, state `auratargetstate`; then the per-unit step
`0x005C35C0(O, context)` on O alone (no scan). Return 1.

#### 2.10 Damage to mana

H a player; R present with total (+0x4C) > 0; v > 0. M := max mana
(`0x00625D60`), m := mana (8); m ≥ M → 0. mana := min(max(m +
pct(R total, v, 100), 0), M); overlay 152 on H. Return 1.

#### 2.11 Freeze

H and O present; v > 0. c := 5 × (4·max(v − 1, 0) − O level + H level
(− 6 on event 6) + 10); event 6 → c / 3; c := clamp(c, 0, 100).
Seed draw `roll(100)` = r (inline, `0x005BFD02`); r ≥ c → 0. len :=
clamp(2(c − r) + 25, 25, 250). R' zeroed, freeze length (+0x34) :=
len; `apply(game, H, O, 1, R')` (no reaction). Return 1.

#### 2.12 Mana / life after kill (17, 18, 28)

O present. 17: H a player; v ≠ 0; M := max mana, m := mana; m ≥ M →
0; mana := min(max(m + (v << 8), 0), M); overlay 152 on H. 18: O a
demon (`0x0063E940`); 18 and 28: v ≠ 0; life likewise with max life
and stat 6; overlay 151. Return 1.

#### 2.13 Slow (19) and Clay Golem (27)

H and O present. 19: v as §1; 27: v := H's item/skill value of stat
150 (`item_slow`, layer 0). v = 0 → 0. Cap: O a player → 50. O a
monster: champion or unique (`0x005A0180(O, 12)`) → 50; 19 only: boss
(`0x0063E9F0(O, 0)`) or hireling → 50; superunique (flag 2) → 75; else
90 (27: every other monster 90). Other unit types → 0. v := min(v,
cap). `apply_state` {source H, target O, skill 0, level 1, duration
750, stat 67 `velocitypercent`, value −v, state 24 `slowed`}
(`skills/bodies.md` §2.7); none → 0. The list also gets stat 68
(`attackrate`) and 69 (`other_animrate`) := −v (`0x006270B0`); O's
animation rate is recomputed (`0x00623F50`, `sim/units.md` §4.7).
Return 1.

#### 2.14 Skill on attack / kill / hit (20) and on death / level-up (30)

H present; v > 0 (the chance). 20: R present without hit flag 0x20
(R +0x00) → 0. Seed draw `roll(100)` < v, and skill s has a record;
else 0.

- 20, s has `ItemTgtDo` (flags bit 29): O present → cast(O, s, l, O,
  aim 0); no O → nothing. Return 1.
- 20 otherwise: O → cast(H, s, l, O, aim 1); no O → cast_point(H, s, l,
  H's path target x, y, aim 1).
- 30: O → cast(H, s, l, O, aim 0); no O → cast_point(H, s, l, path
  target, aim 0).

#### 2.15 Skill on get-hit (21)

H present; v > 0; R present with result get-hit (4); `roll(100)` on H's
seed (`0x0045C3E0`) < v; s has a record. O → cast(H, s, l, O, aim 0);
else cast_point(H, s, l, path target, aim 0). Return 1.

#### 2.16 Bone Armor (22) and Cyclone Armor (25)

H and R present; `aurastate` (+0x80) valid and H has its state list S;
c := S's value of `aurastat2` (+0x56, the maximum) > 0; `aurastat1`
(+0x54) valid; a := S's value of it (absorb left).

- 22: a > 0 absorbs physical: a ≥ R physical → a −= physical,
  physical := 0; else physical −= a, a := 0. S stat1 := a (`0x00627150`).
- 25: a ≠ 0 absorbs fire (+0x10), then cold (+0x24), then lightning
  (+0x1C), each field > 0 in turn (`0x005C8780`, same rule). S stat1 :=
  a.
- a = 0 (25: a ≤ 0 after absorbing) → state `aurastate` off, S
  detached and freed. Else q := pct(a, 100, c); 22: |q − S stat 84| ≥
  5, 25: q − S stat 84 ≥ 5 (no absolute value; Edge case 2) → mark the
  state for update (`0x00639E30`) and S stat 84 := q. Return 1.

#### 2.17 BloodGolem, damage done (23)

H, O, R present; O a player or monster. d := 100; O a monster → its
`Drain` of the difficulty (≤ 0 → 0). O's alignment must be 0. x := R
physical, pct(x, d, 100) when d ≠ 100; x := min(x, O's life). Q := H's
owner; Q present and dead → 0; H dead → 0. f := `0x00645B70(L, k)`:
k valid and L > 0 → min(`Param1` + ((L × 110) / (L + 6)) × (`Param2` −
`Param1`) / 100, `Param2`) (`0x00645B20`; signed, toward zero), else
0; f ≤ 0 → 0. x := pct(x, f, 100) ≤ 0 → 0. g := `Param3` (+0x150). Q
and g > 0 → x −= Heal(Q, pct(x, g, 100)). x −= Heal(H, x). Q and x > 0
→ Heal(Q, x). Overlay 151 on H; on Q when g > 0. Return 1.

#### 2.18 Energy Shield (24)

H and R present; `aurastate` valid with H's state list. p := `eval(
calc1, k, L)` (unit H) ≤ 0 → 0. m := mana (8, total). q := `eval(calc2,
k, L)`, at least 1. A := 1 when there is no O or O is a monster that is
not a hireling, else 0. Rows of table `0x006E30E0` in order: physical
+0x08, fire +0x10, lightning +0x1C, cold +0x24, magic +0x20 always;
life, mana, stamina leech +0x38, +0x3C, +0x40 only when A = 1. Per row
with value w > 0: x := min(pct(w, p, 100), pct(m, 16, q)); absorbed +=
x; field := w − x; m := max(m − pct(x, q, 16), 0). Then mana := m. O
present, absorbed > 0, `prgoverlay` (+0x10C) ≥ 0 and ≤ overlay count →
overlay `prgoverlay` + dir8(direction H → O) on H (`0x00621DC0`, 64 → 8
table `0x00745600`). m ≤ 0 → H's state list of `aurastate` detached and
freed, or, without a list, the state turned off. Return 1.

#### 2.19 BloodGolem, damage taken (26)

H a monster; R present with total (+0x4C) > 0; Q := H's owner present
and alive with life ≥ 256. H's monstats `Skill1` (+0x170) ≥ 0: p :=
`Param5` (+0x158) of that skill; x := pct(R total, p, 100). Q's life :=
max(life − x, 256). R total := max(R total − x, 0). Return 1.

#### 2.20 Rest in peace (29)

H and O present → state 172 (`restinpeace`) on O. Return 1.

#### 2.21 Reanimate (31)

O a monster without type flag 4 or 8; v > 0 (chance; the layer is the
class c); `roll(100)` (H's seed, `0x0045C3E0`) < v. P := H, then its
owner chain (`0x0058F0D0`) until a player; none → 0. Register on O a
handler for event 13 (fired when O's death animation ends,
`sim/intents-events.md`): `0x005C0AD0(game, O, event 13, skill c,
level P's GUID, function `0x005C07A0`, key type 0, key P's GUID)`. A
key type of 0 makes it run once (`skills/bodies.md` §2.18). Return 1.

**Raise `0x005C07A0`** (ECX game; V = the dead unit, skill c, level
G): V a monster; P := the player with GUID G (`0x00552F60(game, 0,
G)`); the raise test `0x00645510(V, 0)` passes; c a valid monstats
class; room R at V's position (`0x00463740`); c has a monstats2 row.
Clear V's pattern (`0x0064EC10(V's room, V x, V y, V pattern,
0x8000)`). N := spawn `0x005B2F20(game, R, V x, V y, c, mode 1, −1,
flags 0x4A)` (`monsters/init.md` §1); none → 0. N flags |= 0x402000E;
AI init `0x005B0E00(game, N, 0x00541860(N, 0))`; `0x00573780(game,
N)`; owner data `0x0058F030(game, N, P GUID, P type, 0, 0)`; leash
`0x005DD330(N, P)`; delete N's type-2 timers, type-2 timer at F + 25;
alignment := 2 (`0x005543B0`); N flags |= 0x80000000; state 96
(`revive`) on N; umod 21 (`0x005A4850(game, N, 21, 0)`); type-7 timer
at F + 1500; `node_insert(game, N, 0, P +0xD0)` (`skills/bodies.md`
§6.3); path reset `0x00649CA0(N)`. c's monstats2 `ResurrectMode`
(+0x10B) = 14 → mode request 14 with N's used skill := its native
entry of skill 158 (`0x00620210`), path step count 1, target point (V
x, V y), flag 0. State 118 on N; state 173 on V. Return 1.

### 3. Item cast

`cast(U, s, l, T, aim)` = `0x005FDCA0` (ECX U, EDX s): U or T none → 0;
U has state 54 → 0; core (below) with target unit T; failure → 0.
Queue S→C 0x99 on U (`0x005717C0`: skill s, level l, target type and
GUID = the core's chosen unit, or T's when the core chose none, aim)
and return 1.

`cast_point(U, s, l, x, y, aim)` = `0x005FDD60`: U none → 0; state 54 →
0; core with point (x, y); failure → 0. Queue S→C 0x9A (`0x00571840`:
s, l, the core's point, or (x, y) when its x is 0, aim). Return 1.
Both queue entries go on the unit's message list (+0xEC / +0xF0) and
queue the unit for update (`0x0064C040`); layouts
`sim/server-messages.tsv`.

**Core `0x005FDA80`** (ECX game, EDX U; s, l, T, x, y, out type, out
GUID, out px, out py, aim):

1. s invalid or 0, or `ItemEffect` (+0x16A) = 0 → 0.
2. Save U's path target unit (`0x00553540`), its target point (path
   +0x10 / +0x12) and unit flag 0x40.
3. T → U's target := T (`0x00620C10`); else path target point := (x,
   y) (`0x00648AD0`). Outs := (6, −1, 0, 0).
4. By `ItemTarget` (+0x120):
   - 1: target := U itself; outs := U's type and GUID.
   - 2: free point `0x005FD850`: up to 10 tries, each two seed draws
     on U (x := U x + (`lo' mod 40`) − 20, then y the same from U y);
     the first point where `0x0064D870(U's room, x, y, U's pattern
     (path +0x48), U's collision mask (path +0x4C))` = 0 becomes the
     path target; px, py := it. No point → 0 (after the restore).
   - 3: corpse `0x005FD9C0`: T0 := U's path target unit; C := first
     unit found by the unit find (`monsters/umod-callbacks.md` §3.1)
     around T0 (radius 10, filter flags 0x1002, callback: the corpse
     test `0x00645680`, `skills/bodies.md` §7.5) in T0's room; T0 none,
     l ≤ 0 or no C → 0. Target := C; outs := C's.
   - 4: target unit kept when U has one; else K := U's last attacker
     (`0x00553010`); target := K; outs := K's (6, −1 without K).
   - other: nothing.
5. s has `ItemCheckStart` (flags bit 33): start core `0x0056F640(game,
   U, s, l, 1)` (`skills/use.md` §5.3) = 0 → result 0.
6. Else result := do core `0x0056F7F0(game, U, s, l, charge 0, item 1,
   aim)` (`skills/use.md` §5.4).
7. Restore the saved target unit, or the saved point when there was
   none; flag 0x40 as saved. Return the result.

1.14d: `ItemTarget` 1 Enchant, 2 Teleport, 3 Corpse Explosion and
Poison Explosion, 4 Bone Spirit and Fist of the Heavens; 72 skills have
`ItemEffect`.

## Constants & data dependencies

- Tables: function table `0x007325B0` (`combat/damage.md` §8);
  Energy Shield rows `0x006E30E0` (8 × {record offset, leech-only}:
  0x08, 0x10, 0x1C, 0x24, 0x20 with 0; 0x38, 0x3C, 0x40 with 1); dir
  table `0x00745600`; item-cast target jump table `0x005FDC88`.
- skills columns: `srvmissilea` +0x48, `aurastat1/2` +0x54 / +0x56,
  `aurastate` +0x80, `auratargetstate` +0x82, `prgoverlay` +0x10C,
  `cltoverlaya` +0x110, `ItemTarget` +0x120, `ResultFlags` +0x12E,
  `HitFlags` +0x130, `HitClass` +0x134, `calc1–3` +0x138…+0x140,
  `Param1/2/3/5` +0x148 / +0x14C / +0x150 / +0x158, `ItemEffect`
  +0x16A, `EType` +0x1DC; flags bits 29 `ItemTgtDo`, 33
  `ItemCheckStart`.
- monstats `Drain` +0xA0…+0xA2, `Skill1` +0x170; monstats2 `small`,
  `large` (bits 10, 11), `ResurrectMode` +0x10B; missiles `ReturnFire`
  (bit 9).
- Stats 6, 8, 12, 67–69, 84 (`unsentparam1`), 150; states 24, 96, 118,
  172, 173; overlays 151, 152; skills 71, 130, 158; class 290.

## Randomness

All draws are on H's unit seed except the item-cast free point (U's).

| Fn | Draws |
|---|---|
| 7 | 1 (`mask(128)`) |
| 8 | 1 (`mask(128)`) when v > 0 |
| 9 | 1 (`roll(100)`) |
| 14 | 1 (`roll(100)` inline) |
| 20, 30 | 1 (`roll(100)` inline), then the cast's draws |
| 21, 31 | 1 (`roll(100)`) |
| 3 | 1 (`roll(hi − lo)`) |
| 2.x item cast kind 2 | 2 per try, up to 10 tries |

Draws happen only after the earlier guards pass, in the order above.

## Edge cases & original bugs

1. Iron Maiden's blood-golem branch heals the cursed attacker H and its
   owner, not the golem (§2.4); reproduced.
2. Cyclone Armor's refresh test has no absolute value: a falling
   percentage never re-marks the state (§2.16); Bone Armor uses |·|.
3. Energy Shield's overlay bound allows index = overlay count
   (`jg`, §2.18).
4. Function 20 with `ItemTgtDo` makes O cast on itself (§2.14).
5. 11 and 12 exist but no 1.14d `itemstatcost` row registers them.
6. Reanimate stores P's GUID as the handler's level and the class as
   its skill (§2.21).
7. Item-cast kind 2 tests every candidate in U's own room, even when
   the point lies outside it.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Fn 12: H level 30, O level 20, v 10 | cold +0x24 = 2560, cold length 125, class 0x3D | §2.6 |
| Fn 14: v 5, H 20, O 25, event 5 | c = 5 × (16 − 25 + 20 + 10) = 105 → 100; r = 37 → len 151 | §2.11 |
| Fn 14: same, event 6 | c = 5 × (16 − 25 + 14 + 10) / 3 = 25; r = 30 → nothing | §2.11 |
| Fn 9: H 30, O 40, v 2, event 5 | c = 5 × (30 + 8 − 40 + 6) = 20; r = 4 → d = 4 | §2.9 |
| Fn 23 f: Param1 0, Param2 100, L 10 | (10 × 110) / 16 = 68 → f = 68 | §2.17 |
| Fn 24: w = 2560, p 50, m 5120, q 16 | x = min(1280, 5120) = 1280; m −= pct(1280, 16, 16) = 1280 → 3840 | §2.18 |

## Provenance

- 1.14d `Game.exe` disassembly (`py tools/ghidra/disasm.py fn`) of each
  function in §2 and §3 and the helpers `0x005C5F10`, `0x005C8780`,
  `0x005C39C0`, `0x0056E0C0`, `0x00645B70`, `0x00645B20`, `0x004EFC50`,
  `0x004E6C70`, `0x0046ACE0`, `0x005A0180`, `0x005FDA80`, `0x005FD850`,
  `0x005FD9C0`, `0x005FDA10`, `0x0056E390`, `0x0056E370`, `0x00553540`,
  `0x005717C0`, `0x00571840`; EDX = event at the call in `0x005C0C30`
  (`0x005C0C6C`). Tables `0x006E30E0`, `0x005FDC88` read from the file
  image.
- Registrants from 1.14d `patch_d2` `skills.txt` (`auraevent*`) and
  `itemstatcost.txt` (`itemevent*`); names of states, stats, skills and
  classes from the same files (monstats ids skip the `Expansion` row).
- D2MOO 1.10f (`SUnitEvent.cpp`, `SkillItem.cpp`) for names only.

## Open questions

1. No trace check. Recording: equip items with knockback, freeze,
   slow, skill-on-hit, damage-to-mana; cast Energy Shield, Bone Armor,
   Iron Maiden; log each event function's entry and return, the seed
   before and after, and the record.
2. `0x00541860(N, 0)` (the AI argument at a raise) and `0x005B0E00`:
   owner `monsters/ai.md` §3; the raise relies on them as stated there.
