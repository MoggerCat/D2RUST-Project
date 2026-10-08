# Spec: Missiles — client function and client hit bodies

- **Status:** draft (2026-10-08): 16 client do-function bodies (table
  `0x0072A398`), the client hit table `0x0072A508` and 11 of its bodies,
  and the shared create helpers, read from the 1.14d `Game.exe` asm
  (addresses per rule; tables dumped from the image). No capture has
  checked them; their seed draws are capture-only (`missiles/client.md`
  §C14).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::effects::missiles` (with
  `missiles/client.md`)
- **Related specs:** `missiles/client.md` (create §C2–§C4, dispatch §C6,
  default step §C7, end §C9, removal §C10, table §C12, seeds §C14);
  `missiles/missiles.md` §R2.1 (create record), §R4.1, §R9.5 (server
  twins); `missiles/bodies.md` (server bodies); `sim/rng.md` §3
  (`roll`); `sim/pathing.md` §8.5 (facing); `render/unit-composite.md`
  §8 (motion record); `render/lighting.md` §5–§8 (light radius);
  `audio/triggers-2.md` §16 (`ProgSound`); `data/calc-expressions.md`
  (evaluators); `client/msg-units.md` §4 r6 (dead flag)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–51 |
| Inputs | 52–60 |
| Outputs / state changes | 61–66 |
| Rules | 67–68 |
|   B1. Conventions | 69–93 |
|   B2. Path new-step flag (path +0x34 bit 3) | 94–116 |
|   B3. Shared create helpers | 117–159 |
|   B4. Do bodies: emitters | 160–180 |
|   B5. Do bodies: animation, steering, timed effects | 181–235 |
|   B6. Client hit table `0x0072A508` | 236–275 |
|   B7. Hit bodies | 276–296 |
| Constants & data dependencies | 297–314 |
| Randomness | 315–320 |
| Edge cases & original bugs | 321–345 |
| Test vectors | 346–362 |
| Provenance | 363–382 |
| Open questions | 383–390 |
<!-- /index -->

## Summary

Continuation of `missiles/client.md` past its size limit: the bodies of
the client missile functions (`pCltDoFunc`) that live `missiles.txt`
rows use, most-used first, and the client hit functions (`pCltHitFunc`)
that the end function `0x004D2D70` calls (`client.md` §C9 step 4.3).
All of it is client-only effect state: client missiles, motion, path,
light; no server state and no server RNG.

## Inputs

| Name | Type | Source |
|---|---|---|
| missile m | client type-3 unit (set C) | `client.md` §C1 |
| its `missiles` row | 0x1A4 bytes | `data/fields.tsv` |
| hit unit U (hit functions) | client unit or none | `client.md` §C9 (EDX) |
| skills row of m's skill | 0x23C bytes, records `[data +0xB98]`, count `[data +0xBA0]` | functions 9, hit 29 |

## Outputs / state changes

New client missiles; m's path target, velocity, direction; m's motion
record; m's light radius; m's data +0x28 / +0x2C; m's animation frame;
draws on m's seed (§C14 of `client.md`).

## Rules

### B1. Conventions

| Term | Meaning |
|---|---|
| O | m's owner, `0x004639D0` (none when gone) |
| (x, y) | m's position (`0x0045ADF0`, `0x0045AE20`: path x, y) |
| skill, level | m's data +0x0A / +0x0C (`0x0064A280` / `0x0064A210`) |
| left, elapsed | frames left `0x0064A380`; elapsed `0x0064A3B0` (`missiles.md` §R1.4) |
| d28, d2C | m's data +0x28 / +0x2C (i32), read `0x0064A730` / `0x0064A780`, written `0x0064A710` / `0x0064A760`; set by m's creator unless a body writes them |
| d04, d06 | m's data +0x04 / +0x06 (i16), read `0x0064A510` (−1 without data) / `0x0064A570` (0 without data); written only by `0x0064A4E0` / `0x0064A540`, from the client skill functions `0x004E26C0`, `0x004F2790`, `0x004F3530` |
| P1, P2, P3 | `CltParam1`–`3` (+0x58, +0x5C, +0x60, i32) |
| S1, S2, S3 | `CltSubMissile1`–`3` (+0x1E, +0x20, +0x22, i16) |
| H1 … H4 | `CltHitSubMissile1`–`4` (+0x2C … +0x32, i16) |
| c1, c2, c3 | `cHitPar1`–`3` (+0x6C, +0x70, +0x74, i32) |
| rnd(n) | `roll(n)` on m's unit seed (+0x20) (`0x0045C390` / `0x0045C3E0`, `sim/rng.md` §3): n < 1 → 0, no draw |
| seed step | one advance of m's seed (lo' = lo × 0x6AC690C5 + hi, `sim/rng.md`), the body using lo' (u32) itself |
| step | the default step `0x004D30C0` (`client.md` §C7) |
| remove | default removal `0x004CD390` (`client.md` §C10 r1) |
| record R | a zeroed 0x5C-byte create record (`missiles.md` §R2.1) passed to the client create `0x004CD540` (`client.md` §C2–§C4); fields named by offset; unnamed fields 0 |
| spawn(c) | `0x004CDBA0(m, c, 0, 0, skill, level)`: flags 0x20, origin m, owner m's owner, target (0, 0) absolute (`client.md` §C5) |
| no row | m's class out of range (`client.md` §C2 r1) |

Every body ends by calling step exactly once unless it says remove or
return. "Signed" comparisons unless said otherwise.

### B2. Path new-step flag (path +0x34 bit 3)

Read by `0x006505C0` (0 without a path). Answers `client.md` Open
question 6.

1. The path step `0x00650840` clears it first, on every call (the call
   needs only a path).
2. The step walks the new position subtile by subtile (`0x00650660` →
   `0x00650150`, list of up to 10 points at path +0x1D4/+0x1D8). The
   flag is set when the walk ends by reaching the new subtile or by the
   10-point cap, having entered at least one new subtile. A walk
   stopped by a subtile that fails the move test (`0x0064FF90`) leaves
   it clear, even with points listed.
3. Position setters also write it: `0x006505E0(a, b)` (a takes b's
   position: set when the integer subtile x or y differs, else clear)
   and `0x00650910` (same test against the new point).
4. `client.md` §C7 r5 calls the path step only when the path velocity
   is non-zero. A body that reads the flag before its step sees the
   value from m's last path step: a stopped missile keeps its last
   value.

So "new-step" = "the last path step entered a new subtile".

### B3. Shared create helpers

1. **Sub-at-step** `0x004CE050(m, c, range, loops)` (EDX = c): nothing
   unless the B2 flag is set. R: flags 5 (start given, velocity given
   = 0); range > 0 → flags 0x8005, frames R+0x4C := range; loops > 0 →
   flags |= 8. Owner O (may be none), origin m, class c, start (x, y),
   skill, level, R+0x34 := 2 × level − 2 (whatever loops is). Create.
   Server twin `sub_at_step` (`missiles.md` §R9.5 item 1) also needs an
   owner; the client does not.
2. **Scatter** `0x004CE140(m, chance, count, spread, c)` (EDX =
   chance): c out of range → nothing. left ≠ 0 → rnd(chance) ≠ 0 →
   nothing (left = 0 → no draw). R: flags 0x20, owner O, origin m,
   class c, skill, level. count times: dx := rnd(2·spread) − spread, dy
   := rnd(2·spread) − spread; target (x + dx + σ(dx)·spread, y + dy +
   σ(dy)·spread) with σ(v) = −1 for v < 0, else +1; create. The start is
   the origin's position.
3. **Disc** `0x004CEF50(m, r, c, skill, level, chance, s)` →
   `0x004CED60(m, x, y, r, c, skill, level, chance, s)`: owner := m when
   m is a player or monster, else O (none → nothing). R: flags 0x201,
   owner, class c, skill, level. For i := −r, −r + s, … ≤ r (y offset),
   for j := −r, −r + s, … ≤ r (x offset), when i² + j² ≤ r²:
   1. chance > 0 → rnd(chance) ≠ 0 → skip the point.
   2. Start (x + j, y + i). R+0x40 := c's `RandStart` > 0 ?
      rnd(`RandStart`) : 0 (frame offset, `client.md` §C3 r12).
   3. The room containing the start, searched from m's room
      (`0x00620BB0`, `0x00463740`), none → skip; else create.
   c out of range reads `RandStart` through a null row (no live caller
   passes one).
4. **Ring** `0x004CEFE0(m, c, a, b, loops)` (EDX = c): c = 0 → nothing.
   R: flags 0x17 (start given, relative target, velocity given
   unshifted); loops > 0 → flags 0x1F, R+0x34 := loops. Owner O, origin
   m, class c, start (x, y), skill, level. Table RX / RY (B6 constants)
   holds 16 points of a 5 × 5 square ring.
   1. k := max(b, 1); velocity R+0x28 := c's `Param1` (+0x38) << 7; for
      i = 0, k, 2k, … < 16: target offset (RX[i], RY[i]); create.
   2. a > 0: velocity := c's `Param2` (+0x3C) << 7; for i = 1, 1 + a, …
      ≤ 15: same.
   c out of range reads `Param1` through a null row (no live caller).
5. **Range pick** `0x004CC790(seed, lo, hi)`: lo ≥ hi → lo; else lo +
   roll(hi − lo + 1) on that seed.
6. **Spawn facing** `0x004CED20(m, c)`: X := spawn(c); X made → X's
   facing := m's path direction (`0x006487F0` → `0x006488A0`).

### B4. Do bodies: emitters

| f | Rows | Body |
|---|---|---|
| 3 `0x004D3460` | 38 `poisonjav`, 43, 245, 246, 648 | B2 flag set, row, S1 ≥ 0 → v := `0x0064B7C0(m, O, CltCalc1 (+0x84), class, level)` (missiles evaluator; no formula → 0, `data/calc-expressions.md`); sub-at-step(m, S1, 0, v). Then step |
| 4 `0x004D34E0` | 39 `poisonjavcloud` … (14) | row → scatter(m, P1, P2, P3, S1). Then step (no row: step only) |
| 6 `0x004D3630` | 68 `firewallmaker`, 130, 458, 653 | B2 flag clear → step. No row or S1 < 0 → remove. R: flags 0x21, owner O, start = target = (x, y), skill, level; c := S1, out of range → remove; S1's `Range` ≠ 0 → flags 0x8021, frames := S1's `Range`. Pick: S2 ≥ 0 and S3 ≥ 0 → rnd(3) = 0 → S2, 1 → S3, 2 → S1; S2 ≥ 0, S3 < 0 → rnd(2) = 0 → S2, else S1; S2 < 0 → S1. Light: P1 = 0 → flags \|= 0x4000 (none); else rnd(P1) ≠ 0 → \|= 0x4000. Create; made → copy m's two path fields to the child (`0x006203B0` → `0x006489A0`, `0x00620410` → `0x006489B0`). Step |
| 27 `0x004D5090` | 300 `mephistofirewallmaker`, 308 | no row → remove. O none → step. S1 > 0 and B2 flag → R: flags 0 (start = origin m's position), owner O, origin m, class S1, skill, level; create. Then elapsed mod max(P1, 1) = 0 → wander (below). Step |
| 46 `0x004D5710` | 431 `lightingtrailingjavalin`, 438 | no row, S1 < 0 or no path → remove **directly** (`0x00465F00(GUID, type)`; no sound stop, no light removal). elapsed < 2 → d28 := y − ty, d2C := tx − x ((tx, ty) = path target point +0x10 / +0x12, `0x00648A00` / `0x00648A10`). B2 flag → R: flags 0xB (start, relative target, loops), owner O, start (x, y), class S1, skill, level, R+0x34 := P1, offset (d28, d2C); create; offset (−d28, −d2C); create. Step |
| 52 `0x004D5F80` | 517 `wake of destruction maker`, 589 | no row, S1 < 0 or O none → remove. B2 flag → R: flags 2 (start = origin m, relative target), owner O, origin m, class S1, skill, level, offset (d28, d2C); create; offset (−d28, −d2C); create. Step |
| 25 `0x004D4DC0` | 291 `towermist`, 368 | no row or S1 < 0 → remove. elapsed mod max(P3, 1) = 0 → u := rnd(2P1 + 1) − P1, v := rnd(2P1 + 1) − P1, sx := x + rnd(2P2 + 1) − P2, sy := y + rnd(2P2 + 1) − P2 (draws in this order); R: flags 0x21, owner O, start (sx, sy), target (sx + u, sy + v), class S1, skill, level; create. Step |
| 17 `0x004D44B0` | 191 `cursecenter`, 576 | no row → remove. c := d04; c = 0 → c := S1; c < 0 → remove. n := d06; n ≤ 0 → remove. elapsed < P1 and elapsed mod 3 = 0 → disc(m, n, c, skill, level, n > 4 ? ⌊n/2⌋ : n, n > 4 ? 2 : 1). Step |
| 49 `0x004D5BA0` | 471 `vines`, 474 | no row or S1 < 0 → remove. elapsed mod max(P1, 1) = 0 → spawn facing(m, S1). Step |

Wander (function 27): a := d28, b := d2C; one seed step, s := lo' & 3:
s = 0 → (dx, dy) := (trunc(3(a − b) / 4), trunc(3(a + b) / 4)); s = 2 →
(trunc(3(a + b) / 4), trunc(3(b − a) / 4)); s = 1 or 3 → (a, b). Path
target point := (x + dx, y + dy) (`0x00648AD0`, which also drops a
target unit), re-path (`0x00649970(path, m, 0)`), d28 := dx, d2C := dy.
trunc rounds toward 0.

### B5. Do bodies: animation, steering, timed effects

1. **5** `0x004D3540` (19 rows: 67 `blaze`, 69 `firewall`, 104, 105,
   …): no row → remove. f := frame >> 8 (arithmetic); A := `SubStart`,
   Z := `SubStop` (u8):
   - f = A − 1 → frame := (f + rnd(Z − A)) << 8;
   - else left = A → frame := max(A − 3, 0) << 8;
   - else left < A → frame := max(f − 2, 0) << 8.
   Then step. Server twin: `missiles.md` §R9.5 item 3 (it also stamps
   collision; the client does not).
2. **7** `0x004D37F0` (86 `guidedarrow`, 193, 329): no row → remove. m's
   room (`0x00620BB0`) a town room (`0x0061AB00`) → remove. d28 bit 0
   set → drop a stale target unit (`0x00480420`: the path's target unit
   pointer differs from the set-S unit with its stored (type, GUID) →
   target unit := none), T := the path target unit (`0x00648BF0`); T
   dead (`0x00464820`), or O given and not hostile to T (`0x00465C60` =
   0) → T := none. k := P1 > 0 ? P1 : 5. T given, elapsed mod k = 0 and
   4 ≤ `0x006416D0(m, T)` ≤ 24 → re-path. Step. Server twin
   `missiles.md` §R9.5 item 4 (it counts frames left, the client
   elapsed).
3. **60** `0x004D8400` (581–586 hurricane debris) and **63**
   `0x004D7880` (607–611 Nihlathak debris), orbit: target point := (x +
   (y − d2C), y − (x − d28)) (a quarter turn around the point (d28,
   d2C)), re-path; 60 does this only on even elapsed, 63 every update.
   Then step.
4. **65** `0x004D7E00` (626–630 `baalfx spirit`): b := d2C. No path →
   step. U := the set-C missile with GUID d28 (`0x004639B0(d28, 3)`),
   none → step. k := U's d28.
   1. b ≠ k and k ≥ 2 → rnd(25) = 0 → d2C := k; k = 2 → path velocity
      0xF00 (`0x00648690`), target point (15135, 5900), re-path; step.
      (rnd(25) ≠ 0 → go on with 2.)
   2. b = 0: seed step; lo' mod 10 = 0 → d2C := 1, motion restart
      (`0x004DA690`), path velocity 0xF00, rnd(2) ≠ 0 → motion velocity
      (0, 0, 1) ≪ (`0x004DA200`); step.
   3. b = 1: (dx, dy) := (x − U.x, y − U.y), negated when m's GUID has
      bit 1; (0, 0) → step; else target point := (x − dy, y + dx),
      re-path; step.
   4. Else step.
5. **9** `0x004D39C0` (101 `meteorcenter`, 133, 564): no row or S1 < 0
   → remove; m's skill out of the skills table → remove. a, b, c :=
   max(P1, 1), max(P2, 1), max(P3, 1). n := `0x00646CA0(O, calc1
   (+0x138) of m's skill row, skill, level)` clamped to 1 … 60; q := 60
   / n.
   1. Light L (+0x64): r := its radius (`0x00474350`), 0 without L.
      (elapsed + 1) mod q = 0, r < n and L → radius := r + 1
      (`0x004742D0`).
   2. elapsed = 0: R: flags 1, owner O, start (x, y), class S1, skill,
      level; create; made → motion position (−c·a, 0, b·a)
      (`0x004DA1D0`), velocity (c, 0, −b) ≪ (`0x004DA200`). S2 > 0 →
      the same with class S2.
   3. elapsed = a − 2: sound, `audio/triggers-2.md` §16.
   Then step. The falling child reaches height 0 after a frames.
6. **59** `0x004D7FC0` (570–573 `world stone chip`): z := m's motion z
   (`0x004DA150`). −d28 ≤ z ≤ d2C → step; else remove.

### B6. Client hit table `0x0072A508`

81 slots (`[0x0072A504]`), 47 non-null, all in 1–64. Called by
`client.md` §C9 step 4.3 with ECX = m, EDX = U (none for a wall,
landing or forced end); result 0 keeps m. "Live" = `patch_d2` rows
with that `pCltHitFunc`.

| h | Address | Live rows | Body |
|---|---|---|---|
| 1 | `0x004CF250` | 41, 62 `fireball`, 129 | B7 |
| 2 | `0x004CF2B0` | 43, 47–49, 419, 436 | B7 |
| 3 | `0x004CF300` | 44–46, 238 | B7 |
| 4 | `0x004D05E0` | none | open |
| 9 | `0x004CF3C0` | 55 `holybolt`, 234 | open |
| 10 | `0x004CC9A0` | 23 (56 `chargedbolt`, 90 `nova`, …) | B7 |
| 11 | null | 283 `desertfireball` | not called |
| 12 | `0x004CF500` | 85 | open |
| 13 | `0x004CCBE0` | 86, 193, 329 | open |
| 14 | `0x004CF640` | 87, 96, 271 | B7 |
| 16 | `0x004CF800` | 93, 232, 267 (568 `*16`: compiled value per `data/field-types.md`) | open |
| 18 | `0x004CF9B0` | 101, 133, 564 | open |
| 19 | `0x004CFC80` | 11 (107–110, 159–162, 312–314) | B7 |
| 24 | `0x004CFD00` | 192 `bonespear`, 652 | B7 |
| 25, 26, 28 | `0x004CFE30`, `0x004D0060`, `0x004D02E0` | 206, 233, 239 | open |
| 29 | `0x004D0380` | 249–256 (6 `grimward…`) | B7 |
| 30 | `0x004D04D0` | 260 `frozenorb` | open |
| 31 | `0x004D14A0` | 272–274, 417 | B7 |
| 32–34 | `0x004D06F0`, `0x004D07E0`, `0x004CCCA0` | 277, 302, 306 | open |
| 36–43 | `0x004D0920`, `0x004D1BD0`, `0x004D1D70`, `0x004D1EC0`, `0x004CD240`, `0x004D1F50`, `0x004D1820`, `0x004D1890` | 336, 348, 351, 354, 357, 364, 374, 375 | open |
| 44 | `0x004D09E0` | 385, 388, 412, 496, 503, 588 | B7 |
| 46–48 | `0x004CCD20`, `0x004D0A80`, `0x004D2760` | 407, 411, 416 | open |
| 50–54 | `0x004D0DB0`, `0x004CCE40`, `0x004D0E00`, `0x004D0EB0`, `0x004D0FF0` | 422, 431/438, 452, 453, 472/475 | open |
| 55 | `0x004D1040` | 481, 578, 671 | B7 |
| 56, 57 | `0x004D1110`, `0x004D11D0` | 499, 550 | open |
| 60–63 | `0x004D1240`, `0x004CCF00`, `0x004D2AF0`, `0x004D2C10` | 603, 639, 654, 655 | open |
| 64 | `0x004CF150` | none | open |

Slots 0, 5–8, 11, 15, 17, 20–23, 27, 35, 45, 49, 58, 59 and 65–80 are
null (slot 0 is never called: h > 0).

### B7. Hit bodies

All return 1 unless stated.

| h | Body |
|---|---|
| 1 | row, H1 ≥ 0 → disc(m, c1, class 265 `fireexplosion2` (fixed), skill, level, c2, 1). H1 only gates |
| 2 | row, H1 ≥ 0 → ring(m, H1, c1, c2, c3) |
| 3 | row, H1 ≥ 0 → spawn(H1); then H2 ≥ 0 → X := spawn(range pick(m's seed, H2, H3)); X made → X's dead flag (+0xC4 0x10000, `0x00464810`) |
| 10 | U and row given, `ProgOverlay` (+0x36, i16) > 0 → overlay on U (`0x00470390(U, ProgOverlay, 2, 0, 0, 0, 0, 0)`; type 2: no draws) |
| 14 | row; H1 ≥ 0 → spawn(H1). H2 ≥ 0: seed step, a := lo' mod 3; seed step, b := lo' & 7; for each entry e0 of pattern a (B6 constants): e := (e0 + b) & 7, X := spawn(H2); X made → X faces (x + DX8[e], y + DY8[e]) (`0x00649EF0(X path, …, 0)`: direction toward that subtile's centre through `0x006485F0`, `sim/pathing.md` §8.5) |
| 19 | row, H1 ≥ 0 → c := H1; H2 > H1 → c := H1 + rnd(H2 − H1 + 1); spawn(c) |
| 24 | U none → 1. U given: H1 ≥ 0 → spawn(H1), return **0** (m flies on); H1 < 0 → 1 |
| 29 | row, H1 ≥ 0, O given, m's skill in the skills table: n := c1; n = 0 → n := max(`0x00646CA0(O, calc1 (+0x138), skill, level)`, 5). R: flags 1 (0x8001 and frames := n when n > 0), owner O, start (x, y), class H1, **skill := level**, level; create; made → its facing := m's (`0x006487F0` → `0x006488A0`) |
| 31 | U given → **0**. Else c := 344 `icebreaksmallmelt` for m's class 272, 345 `icebreaklargemelt` for 273, 274, 417, else −1 (jump table `0x004D1594` / `0x004D15A0`); X := `0x004CDB40(O, c, x, y, skill, level)`; X made → X's facing := m's, X animation speed (+0x4C) := 0x80 (`0x00621780`). H1–H4 not read |
| 44 | U given → **0**. Row, H1 ≥ 0 → X := spawn(H1); X made → X's facing := m's, X path flag 0x40 (`0x00649030(path, 0)`), X's d28 := m's d28 |
| 55 | row, O given: R: flags 0 (start = target = origin m's position), owner O, origin m, skill, level; class H1, H2, H3 in turn, each ≥ 0 → create |

A return of 0 keeps m alive and skips its hit sound and explosion
(`client.md` §C9 step 4.3; Edge case 5 there).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| ring RX (x offsets), i = 0…15 | 0, 1, 2, 2, 2, 2, 2, 1, 0, −1, −2, −2, −2, −2, −2, −1 | `0x006DAF48` |
| ring RY (y offsets) | 2, 2, 2, 1, 0, −1, −2, −2, −2, −2, −2, −1, 0, 1, 2, 2 | `0x006DAF08` |
| shard DX8, e = 0…7 | 0, 1, 1, 1, 0, −1, −1, −1 | `0x006DAFC8` |
| shard DY8 | 1, 1, 0, −1, −1, −1, 0, 1 | `0x006DAFE8` |
| shard patterns (count; entries), stride 20 bytes | a = 0: 4; 0, 1, 4, 5 · a = 1: 4; 0, 2, 4, 6 · a = 2: 3; 0, 3, 5 | `0x006DB008` |
| function 65 fixed target | (0x3B1F, 0x170C) = (15135, 5900); velocity 0xF00 | `0x004D7E87`–`0x004D7EA1` |
| hit 1 class | 265 (0x109) | `0x004CF29B` |
| hit 31 classes | 272 → 344; 273, 274, 417 → 345 | `0x004D14B3`–`0x004D14E2` |

Columns read here: `CltParam1`–`3`, `CltSubMissile1`–`3`, `CltCalc1`,
`CltHitSubMissile1`–`3`, `cHitPar1`–`3`, `ProgOverlay`, `SubStart`,
`SubStop`, `Range` (of S1, function 6), `RandStart` (of the disc class),
`Param1`, `Param2` (of the ring class); skills `calc1`.

## Randomness

All draws are on m's own seed (+0x20) and are listed with their order
in `missiles/client.md` §C14 r9; every create also steps the client
room seed (§C14 r1). Client only.

## Edge cases & original bugs

1. Hit 29 stores m's level in the child's skill field (R+0x2C); the
   child's skill is m's level. Reproduce.
2. Function 46 removes m without stopping its travel sound or removing
   its light when the row, S1 or the path is missing (no live row hits
   this).
3. Function 6 takes the frame count from S1's `Range` for every pick
   (S2 / S3 rows use S1's count).
4. Hit 1 always makes class 265; hit 31 ignores the H columns.
5. Function 27's wander loses length: (1, 0) with s = 0 becomes (0, 0),
   after which the target is m's own position until d28 / d2C change.
6. Scatter skips its chance draw when frames left = 0 (only a missile
   created with 0 frames).
7. Function 17 removes a curse centre whose d06 ≤ 0 on its first
   update.
8. `pCltHitFunc` 11 (283 `desertfireball`) points at a null slot: no
   hit function runs.
9. Ring and disc read a null row for an out-of-range class (no live
   caller); ring does nothing for class 0.
10. Every child that these bodies spawn with spawn(c) aims at (0, 0)
    absolute; all live children of functions 49 and hit 3, 14, 19,
    24, 44 have `Vel` 0, so the distance test of `client.md` §C2 r8
    never fails for them (checked on `patch_d2` `missiles.txt`).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Hit 2, row 43 `plaguejavelin` (c1 1, c2 2, c3 3, H1 221 `plaguejavcloud`: `Param1` 2, `Param2` 4) | ring 1: 8 clouds at offsets (0, 2), (2, 2), (2, 0), (2, −2), (0, −2), (−2, −2), (−2, 0), (−2, 2), velocity 256 → 192 after 75 %; ring 2: 15 clouds at i = 1…15, velocity 512 → 384; flags 0x1F, R+0x34 = 3 | B3 r4, B6 |
| Hit 2, row 47 `rancidgasepotion` (c1 0, c2 2, c3 3) | 8 clouds (ring 1 only) | B3 r4 |
| Disc r = 3, s = 1 | 29 points; first (x, y − 3), then (x − 2, y − 2) … row by row, x offset inner | B3 r3 |
| Hit 1, row 62 `fireball` (c1 3, c2 1) | 29 `fireexplosion2`; per point rnd(1) (always 0, one draw) then rnd(5) | B3 r3, B7 |
| Function 5, `SubStart` 12, `SubStop` 36 | frame 11 → 11 + rnd(24); left = 12 → frame 9; left < 12 → frame − 2, floored at 0 | B5 r1 |
| Functions 60 / 63, position (10, 20), d28 = 7, d2C = 16 | target (14, 17) | B5 r3 |
| Function 27 wander, (a, b) = (4, 0) | s = 0 → (3, 3); s = 2 → (3, −3); s = 1, 3 → (4, 0) | B4 |
| Function 9, row 101 (P1 59, P2 25, P3 15) | at elapsed 0: `meteor` at motion (−885, 0, 1475) ≪ 11, velocity (15, 0, −25) ≪ 11; `meteortail` the same | B5 r5 |
| Scatter spread 6 | each target offset in −12…−7 or 6…12 per axis | B3 r2 |
| Hit 31, m class 273 | creates 345 with animation speed 0x80 | B7 |

Capture checks: `client.md` Open questions 3–4, 7.

## Provenance

1.14d `Game.exe` asm read 2026-10-08 (`re/exports/all.asm`,
`tools/ghidra/disasm.py` for the bodies outside the Ghidra function
list): do bodies `0x004D3460`, `0x004D34E0`, `0x004D3540`,
`0x004D3630`, `0x004D37F0`, `0x004D39C0`, `0x004D44B0`, `0x004D4DC0`,
`0x004D5090`, `0x004D5710`, `0x004D5BA0`, `0x004D5F80`, `0x004D7880`,
`0x004D7E00`, `0x004D7FC0`, `0x004D8400`; hit bodies `0x004CF250`,
`0x004CF2B0`, `0x004CF300`, `0x004CC9A0`, `0x004CF640`, `0x004CFC80`,
`0x004CFD00`, `0x004D0380`, `0x004D14A0`, `0x004D09E0`, `0x004D1040`;
helpers `0x004CE050`, `0x004CE140`, `0x004CEF50`, `0x004CED60`,
`0x004CEFE0`, `0x004CC790`, `0x004CED20`, `0x004CDB40`, `0x00480420`,
`0x0064A510`, `0x0064A570`, `0x0064A4E0`, `0x0064A540`, `0x0064B7C0`
(no-formula return), `0x00649EF0`, `0x00648A00`, `0x00648AD0`; path
flag `0x006505C0`, `0x00650840`, `0x00650660`, `0x00650150`,
`0x006505E0`, `0x00650910`. Tables `0x0072A508` (81 dwords), `0x006DAF08`,
`0x006DAF48`, `0x006DAFC8`, `0x006DAFE8`, `0x006DB008`, `0x004D15A0`
dumped from the image (`re/scripts/rd.py`). Live rows and values from
`patch_d2` `missiles.txt`. D2MOO not used.

## Open questions

1. Writers of d28 / d2C (and of d28 bit 0 for function 7) on the
   client: the client skill functions that create these missiles
   (`client/msg-skills.md` §7) — not traced per row.
2. Remaining live bodies: `missiles/client.md` Open question 1 (do)
   and 2 (hit).
