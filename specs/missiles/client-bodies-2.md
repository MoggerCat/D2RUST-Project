# Spec: Missiles — client bodies part 2 (more do and hit bodies, search helpers)

- **Status:** draft (2026-10-08): 15 more client do-function bodies, 13
  more client hit bodies and their helpers (client unit search, retarget,
  fire patch, shard, ring of 8, lob, rocks), read from the 1.14d
  `Game.exe` asm. No capture has checked them; their seed draws are
  capture-only (`missiles/client.md` §C14). One PROVISIONAL point (hit
  26, Edge case 2).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::effects::missiles` (with
  `missiles/client.md`, `missiles/client-bodies.md`)
- **Related specs:** `missiles/client-bodies.md` (conventions §B1, path
  flag §B2, helpers §B3, hit table §B6); `missiles/client.md` (create
  §C2–§C4, step §C7, end §C9, table §C12, seeds §C14); `sim/rng.md`
  (`roll`, `init_low`, `get_lo`); `sim/pathing.md` §8.1 (path velocity
  +0x7C), client target position `0x004C52E0`; `sim/path-placement.md`
  (point test `0x0064CB30`); `combat/hit.md` §16 (line walk
  `0x0064E260`); `ui/controls.md` (hostility `0x00465C60`);
  `render/unit-composite.md` §8 (motion setters); `audio/triggers-2.md`
  §16 (`ProgSound`)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–51 |
| Inputs | 52–60 |
| Outputs / state changes | 61–66 |
| Rules | 67–68 |
|   B8. Conventions (part 2) | 69–84 |
|   B9. Client unit search | 85–134 |
|   B10. Shared helpers (part 2) | 135–187 |
|   B11. Do bodies | 188–210 |
|   B12. Hit bodies | 211–230 |
| Constants & data dependencies | 231–251 |
| Randomness | 252–259 |
| Edge cases & original bugs | 260–296 |
| Test vectors | 297–314 |
| Provenance | 315–335 |
| Open questions | 336–350 |
<!-- /index -->

## Summary

Second continuation of `missiles/client.md`: the client do bodies and
client hit bodies that `missiles/client-bodies.md` left open, most-used
skills first (Frozen Orb, Blizzard, Chain Lightning, Holy Bolt, Bone
Spirit / Guided Arrow, Meteor, Fist of the Heavens, Lightning Fury,
Immolation Arrow, Molten Boulder, Volcano, Fissure, Tiger Strike, Blade
Shield …), and the client unit search several of them use. All of it is
client-only effect state; no server state and no server RNG.

## Inputs

| Name | Type | Source |
|---|---|---|
| missile m, hit unit U | as `client-bodies.md` | `client.md` §C1, §C9 |
| `missiles` row of m | 0x1A4 bytes | `data/fields.tsv` |
| skills row of m's skill | 0x23C bytes (`[data +0xB98]`, count `[data +0xBA0]`; `0x0045C4B0` returns the same record) | — |
| client rooms and their unit lists | room +0x74 head, unit +0xE8 next | `drlg/rooms.md` §10.4 (adjacency `0x00619790`) |

## Outputs / state changes

New client missiles; m's path target, d28 / d2C, frames; m's seed
(draws and re-seeds); children's motion records, facing, light radius,
path type.

## Rules

### B8. Conventions (part 2)

All terms of `client-bodies.md` §B1 apply (O, (x, y), skill, level,
left, elapsed, d28, d2C, P1–P3, S1–S3, H1–H4, c1–c3, rnd, seed step,
step, remove, record R, spawn, B2 flag). Added here:

| Term | Meaning |
|---|---|
| P4, P5 | `CltParam4`, `CltParam5` (+0x64, +0x68, i32) |
| eval(e) | `0x00646CA0(O, e, skill, level)` (skill evaluator, `data/calc-expressions.md`) on a skills-row column: `aurarangecalc` (+0x64), `calc1` (+0x138), `calc2` (+0x13C), `calc4` (+0x144) |
| reseed(v) | m's seed := `init_low(v)` = {v, 666} (`0x00650E40`, `sim/rng.md`) |
| pos(V) | V's position: objects, items, tiles (types 2, 4, 5) the static path +0x0C / +0x10; others `0x006488C0` / `0x00648900` on the path (0 without a path) |
| target point | m's path target (`0x00648A00` / `0x00648A10` read, `0x00648AD0` write) |
| re-path | `0x00649970(path, m, 0)` |
| facing copy | X's direction := m's (`0x006487F0` → `0x006488A0`) |

### B9. Client unit search

Core `0x004C5C40(C, x, y, r, mask, cb, ctx, skipflag)` (ECX = C, EDX =
x):

1. C, r and cb required; C's room (`0x00620BB0`) required; x = 0 or y
   = 0 → (x, y) := pos(C); mask 0 → 0x583.
2. Rooms: C's room's adjacency array (`0x00619790`) in order. Mask 0x2000
   → skip town rooms (`0x0061AB00`). The near-box test `0x004C5BD0`
   (room box from `0x00619730`) never rejects a room for r ≥ 0 (Edge
   case 1).
3. Units of each room in list order, C itself skipped: accept V when
   `0x006492A0(x, y, pos(V))` (squared distance) ≤ r², the filter
   `0x004C5960(C, V, mask)` passes, and not (skipflag ≠ 0, V a monster,
   monstats flag 27 of V's class (`0x00457490(class, 0x1B)`; D2MOO
   `noAura`, name unconfirmed)).
4. For each accepted V: cb(ECX = {C, n, ctx}, EDX = V); a non-zero
   return increments n (starts at 0).

Filter `0x004C5960(C, V, mask)`; 0 = reject:

| Bit | Test |
|---|---|
| type 0 player | needs 0x1; mode ∉ {0, 17}; with 0x1000 instead mode = 17 |
| type 1 monster | needs 0x2; mode ∉ {0, 12}; with 0x1000 instead mode = 12; 0x4 → undead (`0x0063E990`); 0x4000 → `0x0063E9F0(0, V)` = 0; 0x40000 → `0x0063EDC0(V)` = 0 |
| type 2 object | needs 0x10 |
| type 3 missile | needs 0x8 and a `0x0046ACE0` test (no caller here sets 0x8) |
| type 4 item | needs 0x20 |
| other types | reject |
| 0x80 | V +0xC4 has 0x4 |
| 0x400 | V +0xC4 has 0x8 |
| 0x100 | V's room not a town room |
| 0x10000, 0x20000 | `0x004C5910` / `0x00622C40` tests (no caller here) |
| 0x8000 | hostile: `0x00465C60(C, V)` ≠ 0 (`ui/controls.md` hostility) |
| 0x80000 | V without state 86 (`0x00639DF0(V, 0x56)` = 0) |
| 0x200 | C and V have rooms, and the line walk `0x0064E260(C's room, pos(C), pos(V), 4)` = 0 (`combat/hit.md` §16) |

**Next-GUID pick** `0x004C5EB0(C, x, y, r, mask, g, &n)` (ECX = C, EDX
= x): core with mask := (mask or 0x583) | 0xA280, skipflag 0, callback
`0x004C5E70`. Among accepted units (GUID u32 +0x0C): best := the
smallest GUID > g; fallback := the smallest GUID ≤ g (ties: the later
unit); returns best, else fallback, else none. With g = U's GUID this
walks units in cyclic GUID order; g = 0xFFFFFFFF gives the smallest
GUID.

**Bolt to each** callbacks `0x004CFD70` and `0x004CFFA0` (identical):
ctx = {O, origin, skill, level, max, class}. n ≥ max → 0. Else R:
flags 0x20, owner O, origin, class, target pos(V), skill, level;
create; return 1. So at most max bolts, in search order.

### B10. Shared helpers (part 2)

1. **Retarget** `0x004CCB50` (EAX = m) → 1 when there is no row, d28 has
   0x4 or O is none; else T := next-GUID pick(O, x, y, r := `Param2`
   (+0x3C, the server column), mask 3, g := 0xFFFFFFFF) (lowest GUID in
   range; LOS and hostility measured from O), then `0x004CCA00(m, T)`;
   return 0. `0x004CCA00(m, T)`: no row → nothing. T dead
   (`0x00464820`) → T := none. Frames: total := left := `Range` +
   `LevRange` × (level − 1) (`0x0064A2B0`, `0x0064A330`). T given:
   path target unit := T (`0x00648B90`), d28 := 5, re-path when
   `0x006416D0(m, T)` < 25. T none: a := (i16) d2C, b := (i16)(d2C
   >> 16); target point := (x + a, y + b), d28 := 6, re-path when
   `0x006417F0(m, x + a, y + b)` < 25.
2. **Fire patch** `0x004CDDB0(m, list, n, k, v, chance)` (ECX = m, EDX
   = list of n classes): m's room required. R: flags 1 (0x8001 when v
   ≠ 0), owner O, skill, level. For j := −k … k (x offset, outer), for i
   := −k … k (y offset, inner), P := (x + j, y + i), when m has a room,
   i² + j² ≤ k², the line walk `0x0064E260(m's room, P, (x + 2j, y +
   2i), 4)` = 0 (Edge case 6), and the room containing P (searched from
   m's room, `0x00463740`) exists and is not a town room:
   1. v ≠ 0: seed step; frames R+0x4C := v + (lo' mod 25) − 12.
   2. class := list[roll(n)] on m's seed (n = 1 also draws).
   3. Seed step; lo' mod 100 < chance → create at start P.
3. **Shard** `0x004CE320(m, r, k, c)` (ECX = m, EDX = r): k ≤ 0,
   elapsed mod k ≠ 0, O none or m's room none → none. reseed(x +
   elapsed). R: flags 0x2003, owner O, class c, skill, level; start (x
   + r − 1 − rnd(2(r − 1)), y + r − 1 − rnd(2(r − 1))) (x draw first),
   relative target (0, 0). Point test `0x0064CB30(m's room, start, 4)`
   ≠ 0 → none; else create and return the child. **Fall**
   `0x004CE420(X)`: no row → remove X. v := max(X's P2, 1), h :=
   max(X's P1, v); motion position (0, 0, h) ≪, velocity (0, 0, −v) ≪;
   total := left := ⌊h / v⌋.
4. **Ring of 8** `0x004D01D0(O, P, c, s, l, step)` (ECX = O, EDX = P):
   c out of range → none. R: flags 0x1F, owner O, origin P, class c,
   start pos(P), skill s, level l, loops R+0x34 := l − 1, velocity
   R+0x28 := c's `Param1` (+0x38) << 7; for i = 0, step, 2·step, … <
   8: relative target (R8X[i], R8Y[i]); create.
5. **Lob** `0x004CDC30(U, c, dx, dy, tx, ty, s, l, h)` (ECX = U, EDX =
   c): owner := U for a player or monster, else U's owner (none →
   none). tx = 0 or ty = 0 → (tx, ty) := U's client target position
   (`0x004C52E0`, `sim/pathing.md`); still 0 → none. R: flags 0x521,
   owner, class c, start pos(U) + (dx, dy), absolute target (tx, ty),
   R+0x24 := h, or 30 when h = 0 (arc height, `client.md` §C3 r21),
   skill s, level l; create.
6. **Rocks** `0x004D0B90(m, n, v, g, f)` (EAX = m): O none → nothing.
   1. g ≥ 0 (g only gates): R: flags 0x524, owner O, origin m, class
      **456** `moltenboulder-flyingrocks` (fixed), velocity R+0x28 := v,
      skill, level. n times: tx := x + rnd(4n + 1) − 2n, ty := y +
      rnd(4n + 1) − 2n (x first); `0x006417F0(m, tx, ty)` > 3 → create
      with target (tx, ty).
   2. f ≥ 0: R: flags 1, owner O, class f, skill, level; for i = 0…17
      start (x + F18X[i], y + F18Y[i]); create.

### B11. Do bodies

Rows: `patch_d2` `missiles.txt` rows with that `pCltDoFunc` (all have
one live row except 51).

| f | Rows | Body |
|---|---|---|
| 19 `0x004D46D0` | 260 `frozenorb` | no row or S1 < 0 → remove. elapsed mod max(P1, 1) = 0 → d := \|d28 rem 64\| (C remainder, then absolute); R: flags 2 (start = origin m, relative target), owner O, origin m, class S1, skill, level, target offset (OX[d], OY[d]); d28 := (P2 + d) rem 64; create. Step |
| 20 `0x004D47F0` | 262 `frozenorbnova` | no row → remove. elapsed < P1 and elapsed mod max(P2, 1) = 0 → a := d28, b := d2C, u := trunc((a − b) / 2), v := trunc((a + b) / 2); target point := (x + u, y + v), re-path, d28 := u, d2C := v. Step |
| 13 `0x004D3F10` | 158 `blizzardcenter` | no row or S1 < 0 → remove. c := S1; S1 < S2 → c := S1 + rnd(S2 − S1 + 1). No skills row → remove. r := eval(`calc1`), k := eval(`calc2`); k = 0 or r ≤ 0 → remove. X := shard(m, r, k, c); X → fall(X). Step |
| 10 `0x004D3C40` | 106 `monblizcenter` | no row or S1 < 0 → remove. c as function 13. q := trunc(level / max(P3, 1)); r := P1 + max(q, 2); k := max(P2 − q, 3); X := shard(m, r, k, c); X → fall(X). Step |
| 53 `0x004D6080` | 520 `tigerfury` | no row or S1 < 0 → remove. B2 flag → R: flags 0, owner O, origin m, class S1, skill, level; create. Then function 7 (`client-bodies.md` §B5 r2), which steps |
| 57 `0x004D62B0` | 553 `blade shield attachment` | no row or S1 < 0 → remove. O none → remove. Function 43 (`client.md` §C13: follow O). (a, b) := O's client target position (`0x004C52E0`). d2C < d28 → d2C += 1. R: flags 0x20, owner O, origin O (start pos(O)), target (a, b), class S1, skill, level; create; X → path type 0xE (`0x00648CF0`), re-path X. d2C := 0 (Edge case 8). Step |
| 47 `0x004D5950` | 452 `moltenboulder` | no row → remove. `ProgSound` > 0 → sound test (`audio/triggers-2.md` §16), d28 := motion +0x40 (`0x004DA320`). Then function 6 (`client-bodies.md` §B4), which steps |
| 44 `0x004D55C0` | 393 `distraction` | m none → `0x00465F00(−1, 6)`. No row, S1 < 0, O none, O's current skill entry (`0x00620250`) none or its id (`0x00643CE0`) ≠ skill → remove **directly** (`0x00465F00(GUID, type)`). B2 flag → spawn(S1). m takes O's position (`0x006505E0(m, O)`). Step |
| 45 `0x004D56B0` | 394 `distraction fog` | no row or S1 < 0 → remove. Scatter(m, chance P1, count P2, spread P3, S1) (`client-bodies.md` §B3 r2). Step |
| 50 `0x004D5C10` | 479 `volcano` | no row, S1 < 0 or no skills row → remove. r := P2 > 0 ? P2 : max(eval(`aurarangecalc`), 1); k := P1 > 0 ? P1 : max(eval(`calc4`), 1). P3 < elapsed < P4 and elapsed mod k = 0 → reseed(d28); dx := rnd(2r + 1) − r, dy := rnd(2r + 1) − r; d28 := `get_lo` (`0x00650E50`); lob(m, S1, 0, 0, x + dx, y + dy, skill, level, P5). Step |
| 48 `0x004D59E0` | 461 `erruption center` | no row, S1 < 0, O none, m's room none or no skills record (`0x0045C4B0`) → remove. r := eval(`calc1`) (not clamped), k := max(eval(`calc2`), 1). elapsed mod k = 0 → reseed(x + elapsed); P := (x + rnd(2(r − 1)) − (r − 1), y + rnd(2(r − 1)) − (r − 1)); point test `0x0064CB30(m's room, P, 0x45)` = 0 → R: flags 0x2001, owner O, start P, class S1, skill, level; create, X → dead flag (`0x00464810`); S2 ≥ 0 → class S2, create. Step |
| 51 `0x004D5DD0` | 498 `recycler delay`, 540 | no row → remove. S1 ≥ 0 and elapsed = P1 → R: flags 0x2001, owner O, start (x, y), class S1, skill, level; create; then P3 times: start (x − P4 + rnd(2P4 + 1), y − P4 + rnd(2P4 + 1)); create. S2 ≥ 0, elapsed = P2, O given → O +0xC8 \|= 0x40000; R: flags 0x2000, owner O, origin m, class S2, skill, level; create (sound: `audio/triggers-2.md` §16). Step |
| 68 `0x004D5880` | 441 `sucfireball` | no row or S1 = 0 → remove (S1 = −1 passes). B2 flag → R: flags 9, owner O, start (x, y), class S1, skill, level, loops R+0x34 := P1; create. Step |
| 15 `0x004D4180` | 177 `fingermagespider` | no row or O none → remove. f := d28; f ≤ −P2 → remove; f ≤ 0 → d28 := f − 1. Path velocity (+0x7C, `0x006486C0`) 0 → step. T := path target unit (`0x00648BF0`), else O's target (`0x004648F0`). k := P3 > 0 ? P3 : 5. T, left mod k = 0, f > 0 and `0x006416D0(m, T)` < P4 → s := max(P5, 1); target point := (x + s·sgn(T.x − x), y + s·sgn(T.y − y)), re-path. S1 > 0, left mod (P1 < 2 ? 1 : P1) = 0 and f > 0 → R: flags 5 (velocity 0), owner O, start (x, y), class S1, skill, level (R+0x4C := total − 1, not read); create; X's d28 := 0. Step |
| 58 `0x004D63E0` | 569 `royalstrikechaosice` | no row → remove. elapsed mod max(`Param1` (+0x38, server column), 1) ≠ 0 → step. reseed(d28); a := (i16) d2C, b := (i16)(d2C >> 16); seed step: lo' even → (p, q) := (b, −a); odd → (−b, a). dx := trunc((p + 4a) / 4), dy := trunc((q + 4b) / 4); 0 → 1 each. Target point := (x + dx, y + dy), re-path; d28 := `get_lo`; d2C := (dy << 16) + (dx & 0xFFFF). Step |

### B12. Hit bodies

Return 1 unless stated (0 keeps m: `client.md` §C9 step 4.3).

| h | Rows | Body |
|---|---|---|
| 16 `0x004CF800` | 93 `chainlightning`, 232, 267 | row, O and U required. `ProgOverlay` > 0 → overlay on U (`0x00470390(U, ProgOverlay, 2, 0, …)`). n := d28; n ≤ 1 → done. r := c1 > 0 ? c1 : max(eval(`aurarangecalc`), 1) (no skills row → done). X := next-GUID pick(O, x, y, r, 0x88583, U's GUID). X given and ≠ U → R: flags 0x21, owner O, target unit X, class = m's class, start (x, y), target pos(X), skill, level; create; made → its d28 := n − 1, last-collided := U (`0x0064A400`) |
| 9 `0x004CF3C0` | 55 `holybolt`, 234 | no row or U none → 1. c1 ≠ 0, O given and O may target U as an ally (`0x00464EC0(O, U)` or `0x00464F70(O, U)`) → `ProgOverlay` > 0 → overlay on U; return 1. Else: U a player: c2 ≠ 0 → **0**; U a monster: c2 = 2 → demon (`0x0063E940`), else undead (`0x0063E990`); fails → **0**; other U types → **0**. Passed: H1 ≥ 0 → R: flags 0 (start = origin m), owner O, origin m, class **204** `teethexplode` (fixed), skill, level; create. H1 only gates |
| 13 `0x004CCBE0` | 86 `guidedarrow`, 193 `bonespirit`, 329 | O none or dead, or m's room a town room → 1. f := d28. U none and f & 4 → 1. f & 1: U none → 1; U ≠ path target unit (`0x00648BF0`) → **0**; else 1. Else f & 2: left > 0 → (f & 4 ? 1 : 0); left ≤ 0 → retarget (B10 r1), return its result ≠ 0. Else 1 |
| 12 `0x004CF500` | 85 `immolationarrow` | row and skills row required. list := H1, H2, H3 up to the first negative (n ≤ 3); n = 0 → 1. k := c1 > 0 ? c1 : max(eval(`calc1`), 1). v := `0x0064B7C0(m, O, SHitCalc1 (+0x88), class, level)` (the server column); v ≤ 0 → v := `Range` + `LevRange` × (level − 1). Fire patch(m, list, n, k, v, c2) |
| 18 `0x004CF9B0` | 101 `meteorcenter`, 133, 564 | row and skills row required. R: owner O, skill, level, start (x + dx, y + dy). H1 ≥ 0: flags 0x4001 (no light), class H1, c1 times i: (dx, dy) := (M5X[i mod 5], M5Y[i mod 5]); create. H2 ≥ 0: flags 0x8001, class H2, frames R+0x4C := level ≥ 1 ? skills `Param3` (+0x150) + (level − 1) × `Param4` (+0x154) : 0; at (x, y); create; made with a light → radius 12 (`0x004742D0`). Then flags 0xC001 (no light, frames := R+0x4C as left, 0 when H2 < 0): H3 ≥ 0 → class H3, c2 times i: (F18X[i mod 18], F18Y[i mod 18]); create. H4 ≥ 0 → class H4, for j = j0 … j0 + c3 − 1 (j0 = c2 when H3 ≥ 0, else 0): (F18X[j mod 18], F18Y[j mod 18]); create |
| 25 `0x004CFE30` | 206 `lightningfury` | row, H1 ≥ 0, skills row and O required. r := c1 > 0 ? c1 : max(eval(`aurarangecalc`), 1); max := c2 > 0 ? c2 : max(eval(`calc1`), 1); mask := skills `aurafilter` (+0x50), 0 → 0xA783. Search(O, x, y, r, mask, bolt to each {O, m, skill, level, max, H1}, skipflag 1) |
| 26 `0x004D0060` | 233 `fistoftheheavensdelay` | row, `HitSubMissile1` (+0x24, the **server** column) ≥ 0, O and skills row required. V := client unit with (type d28, GUID d2C) (`0x00463990`); none → **0**. r := c1 > 0 ? c1 : max(eval(`aurarangecalc`), 1); c2 ≤ 0 → eval(`calc4`) (result unused). mask := `aurafilter` ≠ 0 ? 0xA683 : 0 (core → 0x583). Search(O, x, y, r, mask, bolt to each {O, m, skill, level, max = **unset**, H1}, 0) (Edge case 2) |
| 28 `0x004D02E0` | 239 `pantherpotgreen` | row, H1 ≥ 0, O required. Ring of 8(O, U if given else m, H1, skill, level, max(c1, 1)) |
| 30 `0x004D04D0` | 260 `frozenorb` | row, H1 ≥ 0, O required. R: flags 2, owner O, origin m, class H1, skill, level; for i = 0, k, 2k, … < 64 (k = max(c1, 1)): offset (OX[i], OY[i]); create; made → its d28 := OX[i], d2C := OY[i] |
| 52 `0x004D0E00` | 452 `moltenboulder` | no row → 1. U given: not a monster, or monstats2 flag 11 of its class clear (`0x004638A0(class, 11)`; D2MOO `large`) → **0**. H1 ≥ 0 → `0x004CDB40(O, H1, x, y, skill, level)`. Rocks(m, c1, c2, H2, H3) |
| 53 `0x004D0EB0` | 453 `moltenboulderemerge` | row, H1 ≥ 0: R: flags 0x21, owner O, class H1, start pos(m), target = m's target point, skill, level; create; made → bounce `0x004DA2F0(X, c1, c2)`, timed arc `0x004DA5B0(X, h 0, n c3)`, `0x004DA340(X)` (`render/unit-composite.md` §8) |
| 54 `0x004D0FF0` | 472 `vines trail`, 475 | row, H1 ≥ 0: U given → **0**; else spawn facing(m, H1) (`client-bodies.md` §B3 r6) |
| 56 `0x004D1110` | 499 `recycler vine` | row, H1 ≥ 0, O given: R: flags 0, owner O, origin m, class H1, skill, level; create; made → facing copy |

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| OY, i = 0…63 (orb and orb-nova y) | 0, 2, 5, 8, 11, 14, 16, 19, 21, 23, 24, 26, 27, 28, 29, 29, 30, 29, 29, 28, 27, 26, 24, 23, 21, 19, 16, 14, 11, 8, 5, 2, 0, −2, −5, −8, −11, −14, −16, −19, −21, −23, −24, −26, −27, −28, −29, −29, −30, −29, −29, −28, −27, −26, −24, −23, −21, −19, −16, −14, −11, −8, −5, −2 | `0x006DB140` (hit 30), `0x006DB5D0` (function 19), equal |
| OX | OX[i] = OY[(i + 16) mod 64] (30, 29, 29, 28, …) | `0x006DB240`, `0x006DB6D0`, equal |
| M5X, M5Y (meteor H1) | 0, 4, 0, −4, 0 · 0, 0, 4, 0, −4 | `0x006DB0EC`, `0x006DB0D8` |
| F18X, i = 0…17 | 2, −2, 0, 0, −3, 0, 3, −1, 1, −1, 2, −4, −3, −1, 0, 1, 3, 4 | `0x006DB090` (hit 18), `0x006DB540` (rocks), equal |
| F18Y | −2, −2, 2, 5, 3, 3, 3, 2, 1, −1, −1, −2, −2, −3, −4, −3, −3, −2 | `0x006DB048`, `0x006DB588`, equal |
| R8X, R8Y (ring of 8) | 0, 2, 2, 2, 0, −2, −2, −2 · 2, 2, 0, −2, −2, −2, 0, 2 | `0x006DB100`, `0x006DB120` |
| fixed classes | hit 9: 204; rocks: 456 (0x1C8) | `0x004CF4DE`, `0x004D0C62` |
| meteor light radius | 12 | `0x004CFB93` |
| search masks | hit 16: 0x88583 \| 0xA280; retarget: 3 \| 0xA280; hit 25 default 0xA783; hit 26: 0xA683 / 0x583 | B9, B12 |
| lob default arc height | 30 | `0x004CDC98` |

Columns read: `CltParam1`–`5`, `CltSubMissile1`–`2`,
`CltHitSubMissile1`–`4`, `cHitPar1`–`3`, `ProgOverlay`, `ProgSound`,
`Range`, `LevRange`, and the server columns `Param1`, `Param2`,
`HitSubMissile1`, `SHitCalc1`; skills `aurarangecalc`, `aurafilter`,
`calc1`, `calc2`, `calc4`, `Param3`, `Param4`.

## Randomness

All draws are on m's own seed (+0x20) and listed with their order in
`missiles/client.md` §C14 r9. Functions 13, 10 (shard), 48, 50 and 58
re-seed m's seed with `init_low` before drawing, so their draws do not
depend on earlier client history (only on position, elapsed or d28).
Every create also steps the client room seed (§C14 r1).

## Edge cases & original bugs

1. The client search's room box test `0x004C5BD0` compares a far edge
   with a near edge and never rejects a room for r ≥ 0: every room of
   the adjacency array is scanned. Reproduce (order matters for
   capped callbacks).
2. Hit 26 never writes the bolt cap (ctx +0x10): the callback compares
   with a stale stack word, and its eval of `calc4` is discarded.
   PROVISIONAL: no cap (every accepted unit gets a bolt) (because the
   live row's skill caps nothing else and the word is not
   reproducible); settled by a Fist of the Heavens capture counting
   client bolts (`client.md` Open question 8).
3. Hit 26 gates on the server column `HitSubMissile1` but creates the
   client column `CltHitSubMissile1`; its mask ignores the `aurafilter`
   value (any non-zero → 0xA683; 0 → 0x583).
4. Hit 9 always makes class 204 and rocks always class 456; H1 / H2 only
   gate (live rows name the same classes).
5. Hit 18 builds its H3 / H4 records with flags 0xC001: they take the
   frame count left in R+0x4C by H2 (0 when H2 < 0, giving 0 frames).
   The H4 index starts at c2 only when H3 ≥ 0.
6. Fire patch tests the line from P to P + (j, i), not from m to P.
7. Function 19 takes |d28 rem 64| (C remainder); function 58 reads the
   server `Param1`; hit 12 reads the server `SHitCalc1`; hit 13's
   retarget radius is the server `Param2`. All live values are equal or
   unused on the client otherwise.
8. Function 57 increments d2C up to d28 and then sets it to 0 in the
   same run: d2C stays 0. It emits a child every update with no period.
9. Function 15 fills R+0x4C without flag 0x8000 (unused); its children
   are function-15 missiles with d28 = 0 that count d28 down to −P2
   and remove themselves.
10. Function 44 removes m directly (no sound stop, no light removal)
    when the owner stops using the skill.
11. Hit 13's retarget refreshes the frame count (total and left) to the
    full `Range`, so a bone spirit that loses its target lives on.
12. Hit 16 stops at d28 ≤ 1 (n hops in all); the next target is the
    next GUID after U in range, not the nearest.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Next-GUID pick, accepted GUIDs {5, 9, 12}, g = 9 / 12 / 0xFFFFFFFF | 12 / 5 / 5 | B9 |
| Hit 30, row 260 (c1 4) | 16 novas, offsets i = 0, 4, … 60: (30, 0), (27, 11), (21, 21), (11, 27), (0, 30), …; each child's (d28, d2C) = its offset | B12, constants |
| Function 20, (d28, d2C) = (30, 0), P1 6, P2 2 | elapsed 0: target (x + 15, y + 15), (15, 15); elapsed 2: (x + 0, y + 15), (0, 15); elapsed 4: (x − 7, y + 7), (−7, 7) | B11 |
| Function 19, d28 = 5, P2 19 | bolt offset (OX[5], OY[5]) = (26, 14); d28 := 24 | B11 |
| Function 19, d28 = −70 | d = 6 | B11 |
| Function 10, row 106 (P1 5, P2 8, P3 4), level 12 / 1 | (r, k) = (8, 5) / (7, 8) | B11 |
| Fall, blizzard shard (P1 120, P2 5) | motion z 120 ≪ 11, vz −5 ≪ 11, 24 frames | B10 r3 |
| Hit 18, row 101 (c1 5, c2 3, c3 15) | 5 `meteorexplode` at (0, 0), (4, 0), (0, 4), (−4, 0), (0, −4); 1 `whitelightmissile` radius 12; 3 `firemedium` at (2, −2), (−2, −2), (0, 2); 15 `firesmall` at F18 entries 3…17 | B12 |
| Fire patch k = 1 | points in order (−1, 0), (0, −1), (0, 0), (0, 1), (1, 0) | B10 r2 |
| Hit 12, row 85, list (69, 104, 105), v = 100 | per kept point: frames 88…112 (lo' mod 25), class list[lo' mod 3], create when lo' mod 100 < 100 | B10 r2 |
| Hit 28, row 239 (c1 0) | 8 `rancidgascloud` at R8 offsets, flags 0x1F, loops level − 1 | B10 r4 |
| Function 58, (a, b) = (8, 0) | lo' even → (8, −2); odd → (8, 2); d2C = 0xFFFE0008 / 0x00020008 | B11 |
| Rocks n = 10 | offsets in −20…20 per axis; kept when distance > 3 | B10 r6 |

## Provenance

1.14d `Game.exe` asm read 2026-10-08 (`re/exports/all.asm`,
`tools/ghidra/disasm.py` for `0x004C5E70` and `0x004D0FF0`): do bodies
`0x004D46D0`, `0x004D47F0`, `0x004D3F10`, `0x004D3C40`, `0x004D6080`,
`0x004D62B0`, `0x004D5950`, `0x004D55C0`, `0x004D56B0`, `0x004D5C10`,
`0x004D59E0`, `0x004D5DD0`, `0x004D5880`, `0x004D4180`, `0x004D63E0`;
hit bodies `0x004CF800`, `0x004CF3C0`, `0x004CCBE0`, `0x004CF500`,
`0x004CF9B0`, `0x004CFE30`, `0x004D0060`, `0x004D02E0`, `0x004D04D0`,
`0x004D0E00`, `0x004D0EB0`, `0x004D0FF0`, `0x004D1110`; helpers
`0x004C5C40`, `0x004C5960`, `0x004C5BD0`, `0x004C5EB0`, `0x004C5E70`,
`0x004CFD70`, `0x004CFFA0`, `0x004CCB50`, `0x004CCA00`, `0x004CDDB0`,
`0x004CE320`, `0x004CE420`, `0x004D01D0`, `0x004CDC30`, `0x004D0B90`,
`0x004638A0`, `0x00464F70`, `0x0064A300`, `0x006486C0`, `0x004DA1D0`,
`0x004DA200`, `0x004DA320`. Tables `0x006DB048`, `0x006DB090`,
`0x006DB0D8`, `0x006DB0EC`, `0x006DB100`, `0x006DB120`, `0x006DB140`,
`0x006DB240`, `0x006DB540`, `0x006DB588`, `0x006DB5D0`, `0x006DB6D0`
dumped from the image (`re/scripts/rd.py`). Live rows and values from
`patch_d2` `missiles.txt`; column offsets from `data/fields.tsv`. D2MOO
not used (flag names marked as D2MOO hints only).

## Open questions

1. Names of monstats flag 27 (search skip) and monstats2 flag 11 (hit
   52): D2MOO `noAura` / `large` by index; confirm against the
   `monstats` / `monstats2` column order (`data/fields.tsv`). *Flag 27
   answered from `data/fields.tsv`:* the monstats flag word +12 bit 27
   is `noaura` (bit 9 is `interact`, the `0x00457490(class, 9)` of
   `client/msg-ui.md`); monstats2 flag 11 still open (which flag word
   `0x004638A0` reads).
2. Writers of d28 / d2C for functions 15, 19, 20, 50, 57, 58 and hits
   13, 16, 26 (client skill functions, `client/msg-skills.md` §7): not
   traced per row (`client-bodies.md` Open question 1).
3. Hit 26 bolt cap: Edge case 2.
4. Still open: `client.md` Open questions 1–2.
