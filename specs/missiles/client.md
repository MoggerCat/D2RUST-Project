# Spec: Missiles — client-side create, update, hit and removal

- **Status:** draft (2026-10-08): create, per-update dispatch, the
  default step, the end/hit function, the client collide table and the
  client function table read from the 1.14d `Game.exe` asm (addresses
  per rule; tables dumped from the image). No capture has checked it
  yet; the seed draws of §C14 are capture-only. Bodies past §C13 and
  the client hit functions: `missiles/client-bodies.md`.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::effects::missiles` (client effect
  layer, to write; consumes the bridge outputs `ClientMissile` and
  `ShrineFx`, `client/bridge.md` §10)
- **Related specs:** `missiles/missiles.md` (server side; §R1.4 missile
  data layout, §R2.1 the 0x5C-byte create record, §R4.1 path movement,
  §R11 client-only columns); `client/model.md` §2 (client unit sets,
  creation fields, client room seed), §5 (update pass), §15 r6 (shrine
  on-use records); `client/msg-units.md` §7 r6 (S→C 0x73);
  `client/msg-skills.md` §7 (client skill do); `render/camera.md` §8
  (screen shake, function 66), §9 (steps per client update);
  `render/unit-composite.md` §8 (motion record); `render/lighting.md` §8
  (missile light, flicker); `render/draw-order.md` §5 (not-drawn flag);
  `audio/triggers.md` §8 r3, `audio/triggers-2.md` §16 (missile sounds);
  `monsters/umod-callbacks.md` (callback 4); `combat/damage.md` §0
  (`pct`); `sim/pathing.md` (distance `0x006417F0`); `sim/rng.md`;
  `missiles/client-bodies.md` (function and hit bodies, path new-step
  flag)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 57–71 |
| Inputs | 72–82 |
| Outputs / state changes | 83–89 |
| Rules | 90–91 |
|   C1. The client missile unit | 92–117 |
|   C2. Create `0x004CD540` — start, room, target | 118–150 |
|   C3. Create — allocation and frames | 151–187 |
|   C4. Create — tail | 188–215 |
|   C5. Callers | 216–227 |
|   C6. Per-update dispatch `0x004D2C70` | 228–245 |
|   C7. Default step `0x004D30C0` (function 1) | 246–301 |
|   C8. Client collide table `0x0072A350` | 302–322 |
|   C9. End `0x004D2D70(m, U, forced)` | 323–374 |
|   C10. Removal and lifetime | 375–387 |
|   C11. `InitSteps` and `ExplosionMissile` | 388–398 |
|   C12. Client function table `0x0072A398` | 399–485 |
|   C13. Function bodies specified here | 486–541 |
|   C14. Seeds (capture-only) | 542–588 |
| Constants & data dependencies | 589–610 |
| Randomness | 611–614 |
| Edge cases & original bugs | 615–634 |
| Test vectors | 635–650 |
| Provenance | 651–667 |
| Open questions | 668–728 |
<!-- /index -->

## Summary

The 1.14d client draws most missiles from its own simulation: client
code (S→C 0x73, client skill do functions, shrine on-use, monster and
object hooks, other client missiles) fills the same 0x5C-byte record the
server uses and calls the client create `0x004CD540` (185 call sites in
146 functions). The result is a client-only type-3 unit in set C
(`client/model.md` §2 r1). Every client update runs its row's client
function (`pCltDoFunc`, table `0x0072A398`); most functions call the
default step `0x004D30C0` (animation, path step, countdown, wall and unit
collision). A missile ends through `0x004D2D70` (hit effects, client hit
function, explosion missile, removal) or the default removal
`0x004CD390`. Nothing here is sent to the server and nothing decides a
game outcome; the client's results are pixels, lights and sounds.

## Inputs

| Name | Type | Source |
|---|---|---|
| create record | 0x5C bytes, layout `missiles.md` §R2.1 | callers (§C6) |
| `missiles` row | 420-byte record, `class × 0x1A4 + [data +0xB64]`, valid when 0 ≤ class < `[data +0xB6C]` | `data/fields.tsv` |
| client units | owner, origin, target, the local player | `client/model.md` §1–§3 |
| client room / DRLG | room of a point, town test `0x0061AB00`, level of a room `0x0061A1B0` | `client/model.md` §12, `drlg/rooms.md` |
| game type | `[0x007A0610]` | `client/model.md` §5 r1 |
| light quality | `[0x0072A348]` ≠ 0 = high | `render/lighting.md` §5 |

## Outputs / state changes

Client units of type 3 (set C) and their removal; the client room seed
(one step per create, §C14); stats 328 / state 86 on client units;
light records; sound requests; screen-shake starts (`render/camera.md`
§8); motion records (`render/unit-composite.md` §8).

## Rules

### C1. The client missile unit

Fields a client missile uses (unit record `0x00620290`, `client/model.md`
§1 r2):

| Field | Meaning | Written by |
|---|---|---|
| +0x00 type 3, +0x04 class, +0x0C GUID | GUID = the client counter `[0x00711F30]` after its increment (−1 wraps to 0), shared by every `0x00466730` creation | `0x00466730` |
| +0x10 mode | = `CollideType` (+0x183) | type init `0x004CD0A0` |
| +0x14 missile data | layout `missiles.md` §R1.4 (activate +0x08, skill +0x0A, level +0x0C, total +0x0E, current = frames left +0x10, last-collided +0x18/+0x1C, data +0x28) | §C3, functions |
| +0x20/+0x24 seed | `init_low` of one client-room-seed step | `0x00465FD0` (`client/model.md` §2 r6) |
| +0x2C path | direction (+0x64 and +0x65 := b & 63, `0x006488A0`), velocity, target, flags +0x34 | §C4 |
| +0x44, +0x48, +0x4C | animation frame (8.8), length = `AnimLen` << 8, speed (i16) = `AnimSpeed` << 4 clamped to −0x8000…0x7FFF (`0x0064A690`) | §C3 |
| +0x64 | light record | §C4 |
| +0x94/+0x98, flag-ex 0x400 | owner (type, GUID) (`0x00621CE0`); owner lookup `0x004639D0` (unit null → fatal 0x83) | §C4 |
| +0xC4 flags | 0x200000 + 0x400000 (creator); 0x20 set, 0x2 and 0x8 cleared (type init; 0x8 again in §C4); 0x4 := `CanDestroy`; 0x10000 by functions 2, 11 | |
| +0xC8 flag-ex | 0x40000 = not drawn (`render/draw-order.md` §5 r1) while in `InitSteps` (§C11) | §C3, §C7 |
| stat list | allocated empty (`0x00626D40`); stat 328 pierce count (§C4) | |
| motion record | gfx +0x30 (`render/unit-composite.md` §8) | §C3 |

Type init `0x004CD0A0` (from `0x00465FD0` for type 3; nothing without a
row): missile data (`0x0064A100`); mode := `CollideType`; path; footprint
mask 0x40 when `Collision` (+0x184) ≠ 0, else 0 (`0x00648C30`); move-test
mask := the mask column of the client collide table (§C8) for
`CollideType` (`0x00648CE0`); flags as in the table.

### C2. Create `0x004CD540` — start, room, target

ECX = create record R. Returns the missile or none. Steps in order:

1. Row: class R+0x10 out of range or no row → none.
2. Start (x, y): R flag 1 → (R+0x14, R+0x18); else the origin R+0x08
   (none → none) at its position (`0x0045ADF0`, `0x0045AE20`).
3. Room test. Owner R+0x04 given: the room containing (x, y) searched
   from the owner's room (`0x00620BB0`, `0x00463740`), else the client
   DRLG room lookup `0x00619DA0([0x007A0634], x, y)`; neither → none.
   No owner: the same test from the local player's room; no local
   player → no test.
4. Target point (tx, ty): flag 2 → (x + R+0x1C, y + R+0x20); else flag
   0x20 → (R+0x1C, R+0x20); else (x, y).
5. v (velocity): flag 4 clear → (`Vel` + trunc(`VelLev` × R+0x30 / 8))
   << 8 (signed); flag 4 → R+0x28, << 8 unless flag 0x10.
6. Slow: s := 100. Owner given, row `CanSlow`, owner has state 87
   (`0x00639DF0`) and its state list (`0x006256B0`): s := stat 161 of
   that list (`0x00625D00`), v := `pct(v, s, 100)` (`0x00483360`,
   `combat/damage.md` §0).
7. v ≠ 0: v ≤ 0x100000 → v := trunc(v × 75 / 100); v > 0x100000 → v :=
   trunc(v / 100) × 75 (`0x004CD6D9`–`0x004CD736`).
8. Aim, only when v ≠ 0 (T := target unit R+0x0C):
   - Owner given, T none, (tx, ty) = (x, y) → nudge.
   - Owner given, T given, T ≠ owner, T's subtile = owner's subtile
     (x compared first, then y) → nudge and T := none.
   - Owner none: nothing (no nudge).
   Nudge `0x004C51E0`: d := owner direction (`0x00620100`, 0…63) >> 3;
   (tx, ty) := owner position + (DX[d], DY[d]), DX = (0, −1, −2, −1, 0,
   1, 2, 1), DY = (2, 1, 0, −1, −2, −1, 0, 1). Then aim := T's position
   when T is set, else (tx, ty); |aim.x − x| ≥ 100 or |aim.y − y| ≥ 100
   → none.

### C3. Create — allocation and frames

9. Allocate: `0x00466730(class, x, y, type 3, 0)`: GUID counter (§C1),
   flags 0x600000, `0x00465FD0` (common fields and the client room seed
   step, `client/model.md` §2 r6; type init §C1); none → none.
10. `0x00624390(m)` (animation init: +0x4C from `animrate`, +0x48 :=
    `AnimLen` << 8), stat list, then +0x48 := `AnimLen` << 8 and +0x4C :=
    `AnimSpeed` << 4 (clamped). `AnimSpeed` wins over `animrate`.
11. Frames F: flag 0x8000 → R+0x4C; else `Range` + `LevRange` × R+0x30
    (signed i16 columns), plus with flag 8 and `SubLoop` ≠ 0: (`SubStop`
    − `SubStart`) × R+0x34; then, when s ∉ {0, 100}: F += `pct(F, 100,
    s)` (client only: a slowed missile lives longer; the server does not
    do this, `missiles.md` §R2.3 step 10).
12. Flag 0x200: F −= R+0x40, +0x44 := R+0x40 << 8.
13. Total := current := F (clamped −0x8000…0x7FFF); activate frame := F
    − (flag 0x800 ? R+0x44 : `Activate`).
14. No path → remove the unit (`0x00465F00`) and return none.
15. Path: velocity 0; target unit T (`0x00648B90`) or point (tx, ty)
    (`0x00648AD0`); v ≠ 0 → velocity v and build the path
    (`0x00649970(path, m, 0)`, `missiles.md` §R2.3 step 16).
16. `InitSteps` ≠ 0 → flag-ex |= 0x40000 (not drawn, §C11).
17. Last-collided := owner (`0x0064A400`; only with `LastCollide`, as
    the server). Path acceleration := `Accel` (signed), max velocity :=
    `MaxVel` << 8.
18. Flag 0x2000 → direction := rnd(64) on the missile's seed
    (`0x0045C390`, `sim/rng.md`).
19. Motion record: create (`0x004DA000`) and restart (`0x004DA690`).
20. Flag 0x400: d := max(`0x006417F0(m, tx, ty)`, 1) (`sim/pathing.md`:
    max(|dx|, |dy|) + ⌊min/2⌋); when (v << 4) ≠ 0: current := total :=
    (d << 16) / (v << 4) (unsigned). The server sets current only
    (`missiles.md` §R2.3 step 19).
21. Flag 0x100: timed arc on the motion record (height R+0x24, n = total
    frames; `render/unit-composite.md` §8), one motion update
    (`0x004DA350`), direction := (tx + ty) & 63, path flag 0x40 set
    (`0x00649030(path, 0)`).
22. Skill := R+0x2C (`0x0064A240`), level := R+0x30 (`0x0064A1E0`).

### C4. Create — tail

23. Owner given: store it (`0x00621CE0`), then the pierce count
    `0x004CD420` (rule 24).
24. Pierce count (owner O, type 0 or 1; skipped for other owner types).
    Only for rows with `Pierce`. P := O's stat 166 (`0x00625480`) + stat
    156 (`0x00625500`); P = 0 → nothing. O a player other than the local
    player → P := 100 and counter c := 0; else c := O's base stat 328
    (`0x006253B0`). Local seed {c, 666} (`0x00650E40`); n := 0; up to 4
    times: step, `lo' % 100` (unsigned) < P (signed compare) → n += 1,
    else stop. Missile stat 328 := n (`0x00627260`). So a remote
    player's pierce-capable missile always gets n = 4.
25. Flags +0xC4 &= ~0x8.
26. Light: `[0x0072A348]` = 0 or flag 0x4000 → remove the unit's light
    (`0x00464930`); else `Light` ≠ 0 or byte R+0x50 ≠ 0 → remove it and
    create one (`render/lighting.md` §8, row "Missile").
27. Init callback R+0x54 ≠ 0 → call (ECX = m, EDX = R+0x58).
28. Sounds: `TravelSound` (i16) ≠ 0 → request on m; then with an owner,
    `0x004CA900(owner, 314)` found → `0x004BA840` (`audio/triggers.md` §8
    r3).
29. Owner given → umod callback `0x004ADE80(owner, m)`
    (`monsters/umod-callbacks.md`, callback 4).
30. Return m.

Flags of R that `0x004CD540` reads: 1, 2, 4, 8, 0x10, 0x20, 0x100, 0x200,
0x400, 0x800, 0x2000, 0x4000, 0x8000. Others (0x1000, 0x10000,
0x40000000 of S→C 0x73) are ignored.

### C5. Callers

| Caller | Record | Spec of the record |
|---|---|---|
| S→C 0x73 `0x0045E6D0` | owner = local player, flags 0x40000001 / 0x40000021 | `client/msg-units.md` §7 r6 |
| shrine on-use (Storm, Exploding, Poison) | flags 3 / 0x520 | `client/model.md` §15 r6 |
| client skill do / start functions (`0x004E…`, `0x004F…`; skill tables `0x00727BA8`) | per skill | `client/msg-skills.md` §7 |
| client missile functions (§C12) and the end function (§C9) | per function | here |
| `0x004CDB40(owner, class, x, y, skill, level)` (53 sites) | flags 1, owner, class, start, skill, level | here |
| `0x004CDBA0(U, class, tx, ty, skill, level)` (25 sites) | flags 0x20; owner := U when U is a player or monster, else U's owner (none → return none); origin := U; absolute target (tx, ty) | here |
| monster / stat / object hooks | per site | `client/stat-lists.md`, `monsters/umod-callbacks.md`, `audio/triggers-2.md` |

### C6. Per-update dispatch `0x004D2C70`

Runs once per client update of the missile (`client/model.md` §5 r2;
the motion update `0x004DA350` runs before it, `render/unit-composite.md`
§8):

1. No row → default removal (§C10 r1).
2. `Flicker` ≠ 0 → light flicker `0x004CD1C0` (`render/lighting.md` §8).
3. Room := the missile's room, else the room containing its path
   position (`0x00463740` with no start room).
4. Row without `Town` (flags bit 10) and the room is a town room
   (`0x0061AB00`) → default removal.
5. f := `pCltDoFunc` (i16). 0 < f < `[0x0072A34C]` (91) → jump to
   table `0x0072A398` entry f (§C12). f ≤ 0 → nothing: the missile is
   never stepped, never counts down and stays until its room is freed
   (`client/model.md` §5 r5). Live: rows 83 `stuckarrow` and 84
   `footprint` have f = 0.

### C7. Default step `0x004D30C0` (function 1)

No row → return. One pass:

1. active := frames left ≤ activate frame.
2. active and elapsed (`missiles.md` §R1.4) > `InitSteps` → flag-ex &=
   ~0x40000 (drawn from now on).
3. Motion record reports "not moving" (`0x004DA6B0`: flag 1 set and
   flag 8 clear) → end(none, 1) (§C9); return. The create restarts
   every missile's record (flag 8) and the flag-0x100 arc ORs flag 2
   onto it (`render/unit-composite.md` §8), so a landed create arc does
   **not** end the missile here: it holds at x = y = 0 until frames
   left runs out (r6). Only a record made later without a restart (flag
   8 clear) can end it here.
4. active → animation:
   - `LoopAnim` = 0: advance unless at the end (`0x006217C0`: frame +
     speed ≥ length); at the end the frame holds.
   - `LoopAnim` ≠ 0, `SubLoop` = 0: advance with wrap (`0x00621310`:
     frame += speed; frame ≥ length → frame −= length).
   - `LoopAnim` ≠ 0, `SubLoop` ≠ 0: frames left > `AnimLen` − `SubStop`
     + 1 → sub-loop advance (`0x00621330`: frame += speed; frame ≥
     `SubStop` << 8 → frame += (`SubStart` − `SubStop`) << 8); else
     advance with wrap.
   Inactive missiles do not animate.
5. Path velocity ≠ 0 → one path step (`0x00650840`, step size and the
   path rules: `render/camera.md` §9, `missiles.md` §R4.1).
6. Frames left −= 1; result < 1 → end(none, 0); return.
7. Not active → return.
8. `CltSrcTown` T ≠ 0 and (no owner, or the owner's room exists and is
   a town room): frames left ≥ T → frames left := T and, when
   `LoopAnim` = 0 and `AnimLen` − T > 0, frame := (`AnimLen` − T) << 8.
9. Mode (= `CollideType`) 0 → return.
10. w := the path's collision word (`0x00648EB0`, `missiles.md` §R4
    step 6). w & 5 → end(none, 1); return.
11. Mode 6 → return.
12. `ClientCol` ≠ 0 and (w & 0xFFFF) ≠ 0: for each point of the path's
    step list (`0x00648F40`, in order): find a unit at that point in the
    missile's room with the collide test of the mode (§C8) and the
    missile's size (`0x00620510`) (`0x00641CB0`); a unit found that is
    not dead (`0x00464820` = 0) → end(U, 0); return. A dead unit found
    → next point. Search order (first accepted unit wins):
    `sim/path-placement.md` §4 r6 (the room's adjacency array from index
    0, each room's unit list from its head); the client list order is
    `sim/unit-order.md` §5 r6–r7 (prepend on insert, then the draw's
    stable Y sort). A dead unit found first hides a live one behind it
    on the same point: the search moves to the next point, not the
    next unit.
13. Second pass: `render/camera.md` §9 "Steps per client update", row
    Missile (local player's missile, game types 0, 1, 6, 8, first pass,
    elapsed = 1, path velocity ≠ 0) → back to step 1 once.

Differences from the server flight (`missiles.md` §R4): the activate
frame gates animation and unit collision only, not walls; no per-point
mask test before the unit search; no missile-barrier rule; no "no room
→ removed" rule.

### C8. Client collide table `0x0072A350`

9 entries of (test, move mask) indexed by `CollideType`, dumped from
`Game.exe`. Common test `0x004CCF40` (candidate U, context {missile m,
owner O, row}) accepts U when all hold: U flags +0xC4 has 0x8 and 0x4;
not (`NextHit` and U has state 86 `justhit`); not (`LastCollide` and U is
m's last-collided unit, `0x0064A470`); O none, or O hostile to U
(`0x00465C60` ≠ 0), or `CollideFriend` ≠ 0.

| Mode | Test | Mask | Accepts | Live rows |
|---|---|---|---|---|
| 0 | none | 0 | (step 9 returns) | 301 |
| 1 | `0x004CCFE0` | 0x84 | players by the common test; monsters with alignment (`0x006259B0`: state 105 list stat 172) = 2, without the common test | 6 |
| 2 | `0x004CCFB0` | 0x104 | monsters by the common test | 0 |
| 3 | `0x004CD020` | 0x184 | players and monsters by the common test | 339 |
| 4 | none | 0 | (null test passed to the search) | 0 |
| 5 | `0x004CCFB0` | 0x104 | as 2 | 0 |
| 6 | none | 4 | (step 11 returns) | 24 |
| 7 | `0x004CD050` | 0x40 | missiles whose row has `CanDestroy`, no common test | 0 |
| 8 | `0x004CD020` | 0x185 | as 3 | 14 |

### C9. End `0x004D2D70(m, U, forced)`

Callers: the default step (none, 1), (none, 0), (U, 0); function 2
(none, 0); function 43 (none, 0); function 56 (two sites); `0x004AE3C0`;
`0x004F0F30`. Returns the explosion missile X or none.

1. O := m's owner (`0x004639D0`; m none → fatal 0x83). r := 3.
2. U given (the hit tests):
   1. `NextHit` and U in state 86 → return none.
   2. `LastCollide` and U = m's last-collided unit → return none.
   3. Last-collided := U (`0x0064A400`, only with `LastCollide`).
   4. U without flag +0xC4 0x4 → return none.
   5. O given, O not hostile to U (`0x00465C60` = 0) and
      `CollideFriend` = 0 → return none.
   6. O given, row `Pierce`, O type 0 or 1, and (O in state 69 `pierce`
      or O's stat 156 ≠ 0): r := `0x004CD310`: m's stat 328 ≠ 0 → stat
      328 −= 1, r := 2; else r := 3.
3. U none and forced = 0 and `AlwaysExplode` = 0 → go to step 5.
4. Hit effects (r & 2, always true for r ∈ {2, 3}):
   1. `NextHit` and U: U gets state 86 for `NextDelay` frames (a state
      list `0x006251F0(0, 2, NextDelay, U type, U GUID)`, state 86,
      end callback `0x004CD380`, attached `0x00626E10`, state on
      `0x00639DB0`). `NextDelay` is a count, not a frame: the list
      has flag 2 (NEWLENGTH) and the only client expiry walk is
      `0x00627460(U, 0)` at the start of U's own per-unit update
      `0x00480810` (`client/model.md` §5 r2; before the motion update
      `0x004DA350` and the type update): with frame 0 each NEWLENGTH
      list's expire −= 1 and a list at ≤ 0 is freed
      (`sim/stat-lists.md` §10.4), so state 86 goes in U's
      `NextDelay`-th update after the hit (1.14d asm 2026-10-09).
   2. U's client event hooks of kind 0 (`0x004DC210(0, U, m, 0)`: list
      U+0x90).
   3. h := `pCltHitFunc` (i16): 0 < h < `[0x0072A504]` (81) and entry h
      of table `0x0072A508` ≠ 0 → call (ECX = m, EDX = U); result 0 →
      return none (the missile stays). Table and bodies:
      `client-bodies.md` §B6–§B7.
   4. `HitSound` ≥ 0 → request on m (`audio/triggers.md` §8 r3).
   5. E := `ExplosionMissile` (i16) ≥ 0 → X := `0x004CDBA0(m, E, 0, 0,
      m's skill, m's level)` (flags 0x20: target (0, 0) absolute). X
      made: X's motion position := m's motion (x, y, z) `>> 11` then `<<
      11` (getters `0x004DA110`, `0x004DA130`, `0x004DA150` shift, the
      setter `0x004DA1D0` shifts back: m's position with the low 11
      bits cleared), done (`0x004DA640`),
      restart (`0x004DA690`); X class 146 `spidergoo` → direction :=
      rnd(64) on X's seed; else X direction := m's direction
      (`0x006487F0` → `0x00648820`).
5. U given and `CollideKill` = 0 → return none (X stays made).
6. `TravelSound` ≥ 0 → stop it (`0x004CA900` → `0x004BA790`).
7. m's light → remove (`0x00621150` → `0x00474470`).
8. r & 1 (r = 3) → remove m: flag 0x200000 → set C (`0x00465F00`), else
   set S (`0x00465EE0`). Return X.

### C10. Removal and lifetime

1. Default removal `0x004CD390`: with a row and `TravelSound` ≥ 0 stop
   it; remove the light; remove the unit (set by flag 0x200000).
2. A client missile ends only by: §C9 step 8; §C10 r1 (no row, town
   room without `Town`, and the function bodies that call it); room
   freed under it (`client/model.md` §5 r5). It is never removed by a
   server message (set C is not addressed, `client/model.md` §2 r2).
3. Lifetime with the default step: removed in the update where frames
   left reaches 0 (F updates; F − 1 for a moving local-player missile,
   whose first update double-steps), earlier on a wall, a unit
   hit with `CollideKill`, a landed arc or `CltSrcTown`.

### C11. `InitSteps` and `ExplosionMissile`

The only readers (`missiles.md` §R11):

| Reader | Use |
|---|---|
| `0x004CD540` (§C3 step 16) | `InitSteps` ≠ 0 → created not drawn |
| `0x004D30C0` (§C7 step 2) | drawn again once active and elapsed > `InitSteps` |
| `0x004D38D0` (function 8), `0x004D4590` (function 18) | trail starts when elapsed ≥ `InitSteps` (§C13) |
| `0x004D2D70` (§C9 step 4.5) | `ExplosionMissile` created at the end |

### C12. Client function table `0x0072A398`

91 slots (`[0x0072A34C]` = 91); entries 69–90 are null (a row with f in
69…90 would jump to 0; no live row has one). "Live" = rows of
`patch_d2` `missiles.txt` with that `pCltDoFunc`. Columns: steps = calls
the default step; makes = calls a create (`0x004CD540` direct /
`0x004CDB40` / `0x004CDBA0`); rnd = seed draws (`0x0045C390`,
`0x0045C3E0`) in the body; rm = calls the default removal. Body
spec'd: § of this spec, or "open" (Open question 1).

| f | Address | Live rows | steps | makes | rnd | rm | Body |
|---|---|---|---|---|---|---|---|
| 1 | `0x004D30C0` | 511 (0 `arrow`, 1 `javelin`, …) | — | — | — | — | §C7 |
| 2 | `0x004D33E0` | 6 (18 `blood1` …) | y | — | — | — | §C13 |
| 3 | `0x004D3460` | 5 (38 `poisonjav`, 43 `plaguejavelin`, …) | y | `0x004CE050` | — | — | `client-bodies.md` §B4 |
| 4 | `0x004D34E0` | 14 (39 `poisonjavcloud` …) | y | `0x004CE140` | 1 + 2 per puff | — | `client-bodies.md` §B4 |
| 5 | `0x004D3540` | 19 (67 `blaze`, 69 `firewall` …) | y | — | 1 | y | `client-bodies.md` §B5 r1 |
| 6 | `0x004D3630` | 4 (68 `firewallmaker` …) | y | y | ≤ 2 | y | `client-bodies.md` §B4 |
| 7 | `0x004D37F0` | 3 (86 `guidedarrow`, 193 `bonespirit`, 329) | y | — | — | y | `client-bodies.md` §B5 r2 |
| 8 | `0x004D38D0` | 16 (93 `chainlightning`, 98 `lightningbolt` …) | y | y | — | y | §C13 |
| 9 | `0x004D39C0` | 3 (101 `meteorcenter` …) | y | y | — | y | `client-bodies.md` §B5 r5 |
| 10 | `0x004D3C40` | 1 (106) | y | shard `0x004CE320` | 1; reseed, 2 | y | `client-bodies-2.md` §B11 |
| 11 | `0x004D3D00` | 16 (115 `corpseexplosion` …) | y | — | — | — | §C13 |
| 12 | `0x004D3D30` | none | y | — | — | y | §C13 |
| 13 | `0x004D3F10` | 1 (158 `blizzardcenter`) | y | shard `0x004CE320` | 1; reseed, 2 | y | `client-bodies-2.md` §B11 |
| 14 | `0x004D4040` | none | y | — | — | y | open |
| 15 | `0x004D4180` | 1 (177) | y | y | — | y | `client-bodies-2.md` §B11 |
| 16 | `0x004D43A0` | 1 (179 `diabwallmaker`) | y | y | 1 | y | open |
| 17 | `0x004D44B0` | 2 (191 `cursecenter`, 576) | y | `0x004CEF50` | per point | y | `client-bodies.md` §B4 |
| 18 | `0x004D4590` | 2 (192 `bonespear`, 652) | y | y | — | — | §C13 |
| 19 | `0x004D46D0` | 1 (260 `frozenorb`) | y | y | — | y | `client-bodies-2.md` §B11 |
| 20 | `0x004D47F0` | 1 (262) | y | — | — | y | `client-bodies-2.md` §B11 |
| 21 | `0x004D48B0` | 1 (284) | y | y | 3 | y | open |
| 22 | `0x004D49F0` | 1 (285) | y | y | — | y | open |
| 23 | `0x004D4B80` | 1 (287 `denofevillight`) | y | — | — | — | §C13 |
| 24 | `0x004D4BB0` | 1 (288 `cairnstones`) | y | y | 1 | y | open |
| 25 | `0x004D4DC0` | 2 (291, 368) | y | y | 4 | y | `client-bodies.md` §B4 |
| 26 | `0x004D4F20` | 1 (299) | y | y | 5 | y | open |
| 27 | `0x004D5090` | 2 (300, 308) | y | y | 1 step | y | `client-bodies.md` §B4 |
| 28 | `0x004D5200` | 1 (306) | y | y | — | — | open |
| 29 | `0x004D5310` | 1 (307 `andycontrol0`) | y | sub-missile helpers | — | y | §C13 |
| 30 | `0x004D5470` | 1 (332) | y | y | 2 | y | open |
| 31 | `0x004D6820` | 1 (338 `horadricstaff`) | y | 3 | — | — | shake: `render/camera.md` §8; rest open |
| 32 | `0x004D6C00` | 1 (347) | y | y | — | — | open |
| 33 | `0x004D6D40` | 1 (350) | y | y | — | — | open |
| 34 | `0x004D6FA0` | 1 (352) | y | — | 2 | — | open |
| 35 | `0x004D72C0` | 1 (353) | y | y | — | — | open |
| 36 | `0x004D7400` | 1 (363 `durieldeathcontrol`) | y | 2 | — | — | shake: `render/camera.md` §8; rest open |
| 37 | `0x004D6540` | 1 (372 `diablo appears`) | y | — | — | — | §C13 |
| 38 | `0x004D6680` | 1 (373 `hfcontrol`) | y | — | — | — | §C13 |
| 39 | `0x004D6660` | 2 (374, 375) | y | — | — | — | §C13 |
| 40 | `0x004D65E0` | 1 (376) | y | — | — | — | open |
| 41 | `0x004D6590` | 1 (377) | y | — | — | — | open |
| 42 | `0x004D66E0` | 1 (379) | y | y | 3 | — | open |
| 43 | `0x004D3070` | 6 (392 `blade creeper`, 406, 410, 415, …) | — | — | — | y | §C13 |
| 44 | `0x004D55C0` | 1 (393) | y | `0x004CDBA0` | — | direct | `client-bodies-2.md` §B11 |
| 45 | `0x004D56B0` | 1 (394) | y | `0x004CE140` | 1 + 2 per puff | y | `client-bodies-2.md` §B11 |
| 46 | `0x004D5710` | 2 (431, 438) | y | 2 | — | direct | `client-bodies.md` §B4 |
| 47 | `0x004D5950` | 1 (452 `moltenboulder`) | via 6 | via 6 | via 6 | y | `client-bodies-2.md` §B11 |
| 48 | `0x004D59E0` | 1 (461) | y | 2 | reseed, 2 | y | `client-bodies-2.md` §B11 |
| 49 | `0x004D5BA0` | 2 (471, 474) | y | `0x004CDBA0` | — | y | `client-bodies.md` §B4 |
| 50 | `0x004D5C10` | 1 (479) | y | lob `0x004CDC30` | reseed, 2 | y | `client-bodies-2.md` §B11 |
| 51 | `0x004D5DD0` | 2 (498, 540) | y | 2 + P3 | 2 per extra | y | `client-bodies-2.md` §B11 (sound: `audio/triggers-2.md` §16) |
| 52 | `0x004D5F80` | 2 (517, 589) | y | 2 | — | y | `client-bodies.md` §B4 |
| 53 | `0x004D6080` | 1 (520 `tigerfury`) | via 7 | y | — | y | `client-bodies-2.md` §B11 |
| 54 | `0x004D8000` | 1 (528 `anya center`) | y | 2 | — | — | shake: `render/camera.md` §8; rest open |
| 55 | `0x004D8260` | 1 (541) | y | y | — | — | open |
| 56 | `0x004D6130` | 1 (546) | y | y | local seed | y | open |
| 57 | `0x004D62B0` | 1 (553) | y | y | — | y | `client-bodies-2.md` §B11 |
| 58 | `0x004D63E0` | 1 (569) | y | — | reseed, 1 step | y | `client-bodies-2.md` §B11 |
| 59 | `0x004D7FC0` | 4 (570–573) | y | — | — | y | `client-bodies.md` §B5 r6 |
| 60 | `0x004D8400` | 6 (581–586) | y | — | — | — | `client-bodies.md` §B5 r3 |
| 61 | `0x004D7690` | 1 (602) | y | — | — | — | open |
| 62 | `0x004D7710` | 1 (606) | y | 2 | — | — | open |
| 63 | `0x004D7880` | 5 (607–611) | y | — | — | — | `client-bodies.md` §B5 r3 |
| 64 | `0x004D7930` | 1 (625) | y | 2 | — | — | open |
| 65 | `0x004D7E00` | 5 (626–630) | y | — | rnd(25), 1 step, rnd(2) | — | `client-bodies.md` §B5 r4 |
| 66 | `0x004D2610` | 1 (639 `worldstone shake`) | — | — | inline | y | `render/camera.md` §8 rule W |
| 67 | `0x004D3E00` | none | y | 3 | — | y | open |
| 68 | `0x004D5880` | 1 (441 `sucfireball`) | y | y | — | y | `client-bodies-2.md` §B11 |

Each function calls the default step at most once per run
(`render/camera.md` §9). The columns of the rows that point to
`client-bodies.md` or `client-bodies-2.md` were corrected from the body
reads there ("via n": through function n's body; "reseed": m's seed
re-initialised before the draws).

### C13. Function bodies specified here

More bodies, the shared create helpers and the hit functions:
`missiles/client-bodies.md` (conventions §B1) and
`missiles/client-bodies-2.md` (more bodies, the client unit search
§B9, helpers §B10).

"P1", "P2" = `CltParam1`, `CltParam2` (+0x58, +0x5C); "S1", "S2", "S3" =
`CltSubMissile1`–`3` (+0x1E, +0x20, +0x22, i16); "step" = §C7; "remove" =
§C10 r1. The shake call parameters and conditions are
`render/camera.md` §8's.

- **2** (`0x004D33E0`, blood): at the animation end (`0x006217C0`) set
  flag 0x10000 and path velocity 0. Then, when m's screen position
  relative to the view origin (`0x00620650` − `0x0045AFC0`,
  `0x006206B0` − `0x0045AFD0`) is in [0, `[0x007A5220]`] ×
  [0, `[0x007A521C]` + 64]: frames left := 128 and step; else end(none,
  0). A blood missile on screen never expires.
- **8** (`0x004D38D0`) and **18** (`0x004D4590`), trails: no row or S1
  < 0 → remove (8) / step only (18). Else when elapsed ≥ `InitSteps`
  and the path new-step flag (path +0x34 bit 3, `0x006505C0`,
  `client-bodies.md` §B2) is set: create S1 with
  flags 1 at m's position, owner = m's owner, skill and level of m.
  Function 8 adds the init callback `0x004CC870` (argument m): when the
  child's P2 > 0, z := rnd(P2) on **m's** seed − ⌊P2 / 2⌋; child motion
  position (0, 0, z); child direction := m's; path flag 0x40; child
  frame := rnd(`AnimLen`) on the child's seed << 8. Function 18 then
  copies m's direction, two path fields (`0x006203B0` → `0x006489A0`,
  `0x00620410` → `0x006489B0`) and motion position (0, 0, m's z). Then
  step.
- **11** (`0x004D3D00`): at the animation end set flag 0x10000 and
  remove the light (`0x00464930`); else step. No countdown at the end:
  the missile stays.
- **12** (`0x004D3D30`): row with P1 > 0 and P2 > 0 → (shake when
  elapsed = 0) step; else remove.
- **23** (`0x004D4B80`): frames left < 100 → frames left := 500; step
  (never expires).
- **29** (`0x004D5310`): no row → remove. No owner → step only. Else, e
  := elapsed: S1 > 0 and e > 10 → `0x004CE850(S1, e, …)` (sound:
  `audio/triggers-2.md` §16); S2 > 0 and e mod max(P1, 1) = 0 →
  `0x004CE530(owner, S2, 20)`; shake (P2 > 0, e = 90); S3 > 0 and e −
  116 in 0…198 → `0x004CECC0(S3, e)`; step. The three helper bodies:
  open.
- **37** (`0x004D6540`): frames left = 150 → shake, step; = 50 → sound
  4,638 on none (`audio/triggers.md` §12 r6), step; else step.
- **38** (`0x004D6680`): client object (type 2) with GUID = missile data
  +0x28 absent → step. Else elapsed = 10 → `0x004D19D0(object)` and
  shake; step.
- **39** (`0x004D6660`): owner given → frame := 0; step.
- **43** (`0x004D3070`): no owner → remove. Owner type ≤ 1 and dead
  (`0x00464820`) → end(none, 0). Else m takes the owner's position and
  room (`0x006505E0`, `skills/bodies-4.md`) and advances its animation
  with wrap; no step, no countdown.
- **66**: `render/camera.md` §8 rule W (owner in level 131–132, else
  remove).

### C14. Seeds (capture-only)

All client-only; none touches server RNG.

1. Every create steps the client room seed of the start room once
   (`client/model.md` §2 r6); the missile seed := `init_low(lo')`.
   This shifts every later client-unit seed of that room: order of
   creates matters.
2. Flag 0x2000: rnd(64) on the missile seed (direction).
3. Pierce count: local seed {c, 666}, not stored (§C4 r24).
4. S→C 0x73: frame := rnd(`AnimLen`) on the missile seed after create
   (`client/msg-units.md` §7 r6).
5. Explosion missile 146: rnd(64) on X's seed (§C9 step 4.5).
6. Function 8 child: rnd(P2) on the parent's seed, then rnd(`AnimLen`)
   on the child's (§C13).
7. Flicker: `render/lighting.md` §8. Function 66: `render/camera.md` §8.
8. Bodies still "open" in §C12 with draws: draw order not specified.
9. Draws of the `client-bodies.md` bodies, all on m's seed, in order
   (rnd(n) = `roll(n)`, none when n < 1; "step" = one seed advance
   whose lo' the body uses):

   | Body | Draws per run, in order |
   |---|---|
   | function 4 (scatter) | frames left ≠ 0: rnd(P1); then per puff rnd(2·P3), rnd(2·P3) |
   | function 5 | frame = `SubStart` − 1: rnd(`SubStop` − `SubStart`) |
   | function 6 | B2 flag set: rnd(3) (S2, S3 ≥ 0) or rnd(2) (S2 ≥ 0, S3 < 0); then rnd(P1) when P1 ≠ 0 |
   | function 17, hit 1 (disc) | per point inside the disc: rnd(chance) when chance > 0; then, for a kept point, rnd(`RandStart` of the class) when > 0 |
   | function 25 | on emission: rnd(2P1 + 1) twice, rnd(2P2 + 1) twice |
   | function 27 | on emission (elapsed mod max(P1, 1) = 0): 1 step (lo' & 3) |
   | function 65 | d2C ≠ k ≥ 2: rnd(25); then with d2C = 0: 1 step (lo' mod 10), and when that is 0, rnd(2) |
   | hit 3 | H3 > H2 ≥ 0: rnd(H3 − H2 + 1) |
   | hit 14 | H2 ≥ 0: 2 steps (lo' mod 3, lo' & 7) |
   | hit 19 | H2 > H1 ≥ 0: rnd(H2 − H1 + 1) |
   | functions 13, 10 | S2 > S1: rnd(S2 − S1 + 1); then on a shard frame (elapsed mod k = 0, owner and room present): reseed {x + elapsed, 666}, rnd(2(r − 1)) twice (x, y) (`client-bodies-2.md` §B10 r3) |
   | function 19 (frozen orb), 20, 15, 57, 68, 53, 44 | none of their own (53 then runs function 7: none; 44 spawns) |
   | function 45 | as function 4 (scatter): frames left ≠ 0: rnd(P1); per puff rnd(2·P3) twice |
   | function 48 | on emission: reseed {x + elapsed, 666}; rnd(2(r − 1)) twice |
   | function 50 | on emission: reseed {d28, 666}; rnd(2r + 1) twice; d28 := seed lo |
   | function 51 | at elapsed = P1, per extra (P3): rnd(2P4 + 1) twice |
   | function 58 | on emission: reseed {d28, 666}; 1 step (lo' bit 0); d28 := seed lo |
   | hit 12 (fire patch) | per kept point: v ≠ 0 → 1 step (lo' mod 25); 1 step for the class (`roll(n)`, n = 1 included); 1 step (lo' mod 100) |
   | hit 52 (rocks) | H2 ≥ 0: per rock rnd(4c1 + 1) twice |
   | hits 9, 13, 16, 18, 25, 26, 28, 30, 53, 54, 56 | none |

   Every create among them also steps the room seed (r1), after the
   body's draws for that create.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| client function table | `0x0072A398`, 91 slots, 68 non-null (1–68) | `[0x0072A34C]` |
| client hit table | `0x0072A508`, 81 slots (`[0x0072A504]`), 47 non-null, all in 1–64 | dumped |
| client collide table | `0x0072A350`, 9 × (test, mask) | dumped |
| client GUID counter | `[0x00711F30]` | `0x00466730` |
| speed factor | 75 / 100 | §C2 r7 |
| distance limit | < 100 subtiles per axis | §C2 r8 |
| nudge tables DX, DY | §C2 r8 | `0x004C51E0` |
| blood hold | 128 frames, screen box (`[0x007A5220]`, `[0x007A521C]` + 64) | function 2 |

Columns read (client): `pCltDoFunc`, `pCltHitFunc`, `CltParam1`–`2`,
`CltSubMissile1`–`3`, `TravelSound`, `HitSound`, `ExplosionMissile`,
`Vel`, `VelLev`, `MaxVel`, `Accel`, `Range`, `LevRange`, `Activate`,
`InitSteps`, `LoopAnim`, `AnimLen`, `AnimSpeed`, `animrate`, `SubLoop`,
`SubStart`, `SubStop`, `CollideType`, `Collision`, `ClientCol`,
`CollideKill`, `CollideFriend`, `NextHit`, `NextDelay`, `AlwaysExplode`,
`CltSrcTown`, `Light`, `Flicker`, `Red`, `Green`, `Blue`, `Size`, flags
`LastCollide`, `Pierce`, `CanSlow`, `CanDestroy`, `Town`.

## Randomness

§C14. No server draw.

## Edge cases & original bugs

1. Explosion missiles with `Vel` > 0 are never made by §C9: the record
   aims at (0, 0) absolute, so the distance test (§C2 r8) fails unless
   the missile is within 100 subtiles of the map origin. Live: 433
   `lightjavalinexplosion` (Vel 12, of 431), 440 `advlightjavexplode`
   (12, of 438), 430 `explodingjavalinexp` (25, of 429) and 370
   `orbmistfade` (3, of 369). Reproduce.
2. A piercing hit (r = 2) with `CollideKill` stops the travel sound and
   removes the light but keeps the missile (§C9 steps 6–8). Reproduce.
3. Rows with `pCltDoFunc` 0 (83, 84) never update (§C6 r5).
4. The pierce test of §C9 step 2.6 checks state 69 or stat 156, not
   stat 166 (the create's test, §C4 r24, uses 166 + 156).
5. A client hit function returning 0 keeps the missile alive and skips
   the explosion and hit sound (§C9 step 4.3).
6. Mode 4 passes a null test to the unit search (no live row).
7. `r & 2` in §C9 step 4 is always true.
8. `pCltHitFunc` `*16` (row 568) is not a number; its compiled value is
   the data loader's (`data/field-types.md`).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Exploding shrine (class 45, `Vel` 16, `VelLev` 0, flags 0x520, +0x24 = 1) at shrine (X, Y), any level | v = 3072; offsets (−6, +6), (−6, −6), (+6, +6), (+6, −6): d = 9, frames = 12; (0, ±6): d = 6, frames = 8; total = current; direction = (tx + ty) & 63 | §C2, §C3 r20–21; `client/model.md` §15 r6 |
| Storm shrine (class 62 `fireball`, `Vel` 20, `Range` 50, `Activate` 0, `InitSteps` 1, flags 3) | v = 3840; frames 50; activate frame 50 (active at once); created not drawn; drawn from the update where elapsed > 1 | §C2–§C3, §C7 r2 |
| Pierce count, local owner, counter c = 0 | P = 66 → n = 0; P = 67 → n = 4 (draws 66, 18, 35, 30) | §C4 r24 |
| Pierce count, c = 1 | P = 50 → 0; P = 52 → 3 (draws 51, 31, 12, 93) | §C4 r24 |
| Pierce count, remote player owner with P ≠ 0 | n = 4 | §C4 r24 |
| Nudge: owner at (100, 200), direction 16 (d = 2), no target | target (98, 200) | §C2 r8 |
| v before 75 % = 0x100064 (1,048,676) | trunc(v / 100) × 75 = 786,450 (the other branch would give 786,507) | §C2 r7 |
| v before 75 % = 0x2D00 (`Vel` 45) | 8,640 | §C2 r7 |
| `pCltDoFunc` 72 | jumps to address 0 (crash); no live row | §C12 |

Pixel/trace checks (capture-only): §Open questions 3–4.

## Provenance

All rules: 1.14d `Game.exe` asm read 2026-10-08 (`re/exports/all.asm`;
addresses per rule): `0x004CD540` (`0x004CD540`–`0x004CDB38`),
`0x004CD420`, `0x004C51E0`, `0x00466730`, `0x004CD0A0`, `0x004D2C70`,
`0x004D30C0` (game-type table `0x004D33CC` / `0x004D33D4`: types 0, 1,
6, 8), `0x004D2D70`, `0x004CD390`, `0x004CCF40`, the collide tests
(`0x004CCFB0`–`0x004CD091`, disassembled from the image), the function
bodies of §C13, `0x004CC870`, `0x004CDB40`, `0x004CDBA0`. Tables
`0x0072A350`, `0x0072A398`, `0x0072A508` and their counts read from
`Game.exe` (`re/scripts/rd.py`). Live-row counts from `patch_d2`
`missiles.txt` (684 rows). D2MOO (1.10f) not used beyond names
(`UNITFLAGEX_NODRAW` for flag-ex 0x40000). Shake, flicker, motion,
light and sound sub-rules are their owners'. §C7 r12 search order:
the owners named there. §C14 r9: the body reads listed in
`client-bodies.md` Provenance.

## Open questions

1. Bodies marked "open" in §C12 (live: 16, 21, 22, 24, 26, 28, 30, 31
   rest, 32–36, 40–42, 54–56, 61, 62, 64; unused: 14, 67) and the helpers `0x004CE530`,
   `0x004CE850`, `0x004CECC0`, `0x004D19D0`: static read per function,
   one session per ~10 functions.
2. Client hit functions still open (`client-bodies.md` §B6): live
   32–34, 36–43, 46–48, 50, 51, 57, 60–63; unused 4, 64.
3. Capture: a fireball (Storm shrine) and an Exploding shrine — the
   missile pixels per frame, GUID order and `[0x00711F30]` advance,
   confirming §C3 r20 frames and the not-drawn first frame (§C7 r2).
4. Capture: client room seed before/after a burst of client missiles
   (S→C 0x73 and a skill do) to confirm one step per create (§C14 r1),
   the 0x73 frame draw and the function-8 trail draws (chain lightning).
5. *Answered (static):* unit search order, §C7 r12 (owners
   `sim/path-placement.md` §4 r6, `sim/unit-order.md` §5 r6–r7). A
   capture of two units on one subtile hit by one client missile would
   confirm it.
6. *Answered (static):* path flag 0x08 = "the last path step entered a
   new subtile", `client-bodies.md` §B2.
7. Capture: the draws of §C14 r9 — a Fire Wall cast (function 6 picks
   and lights), a fireball explosion (hit 1: 29 `fireexplosion2`,
   frame offsets), a Plague Javelin hit (hit 2: 23 clouds) and a
   Freezing Arrow hit (hit 14 shard facings); compare the client room
   seed and the missile seeds before and after.
8. Capture (`client-bodies-2.md`): a Fist of the Heavens cast on a
   group (count of client `fistoftheheavensbolt` per hit: settles the
   unset cap of hit 26, Edge case 2 there); a Chain Lightning cast on
   three monsters (hop order = cyclic GUID order, hit 16); a Blizzard
   and a Fissure cast (shard / crack positions from the re-seeded
   missile seed, §B10 r3, function 48); a Frozen Orb (bolt directions
   d28 += 19 mod 64 per frame, 16 novas at the end).
9. Timed arc `0x004DA5B0` (`render/unit-composite.md` §8): does "flag
   2" set flag 2 (keeping the restart's flag 8 of §C3 r19) or store
   flags := 2? §C7 r3 ("a flag-0x100 arc that landed") reads only with
   the store. Is the vz division by `n` or by the clamped ticks max(n,
   1), and is az·n² halved before the subtraction? PROVISIONAL: flags :=
   2, ticks, halved first (because the landing rule of §C7 r3 needs flag
   8 cleared); settled by REC-540 (PC 1 Step 4 item 24: the asm of
   `0x004DA5B0`).
10. §C9 r4.5 copies m's motion position to X through the getters
    `0x004DA110` / `0x004DA130` / `0x004DA150` and the setter
    `0x004DA1D0`, which shifts its arguments `<< 11`
    (`render/unit-composite.md` §8): do the getters return the stored
    16.16 value (then X's position is m's << 11) or `>> 11`? Also the
    reader of function 59 (`client-bodies.md` §B5 r6). PROVISIONAL: the
    stored values are copied as they are and function 59 reads the
    stored z (because no shift is stated); settled by REC-541 (PC 1 Step
    4 item 24).
11. §C9 r4.1: the state list of state 86 is made by `0x006251F0(0, 2,
    NextDelay, U type, U GUID)`; is `NextDelay` the list's expire frame
    (+0x18, `sim/stat-lists.md` §10.4) as an absolute client frame, or
    a count from now, and which client pass removes it? PROVISIONAL: a
    count of client updates, removed before the set-C missile walk
    (because the column is a delay); settled by REC-545 (PC 1 Step 4
    item 24).
12. §C9 r6 detaches the travel sound with `0x004BA790(h, U, force)`:
    which force? PROVISIONAL: 0 (because a looping travel sound then
    stops with its last unit, as the unit free does); settled by REC-548
    (PC 1 Step 4 item 24).
