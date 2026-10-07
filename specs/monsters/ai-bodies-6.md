# Spec: Monsters — AI think bodies: pets, towns, traps, spawners

- **Status:** draft: the AI functions that `ai-functions.tsv` still
  marked `unread` after the Act I–V specs (town NPCs, hirelings,
  summons, traps, spawners, quest and Uber AIs), in descending order of
  how many live monstats rows use them, with the shared pet helpers,
  read from the 1.14d `Game.exe` disassembly (addresses per section;
  register arguments checked in the disassembly). No recording covers
  these bodies yet (Open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions beside the
  `ai.md` §9 bodies)
- **Related specs:** `monsters/ai.md` (owner of scheduling §1, dispatch
  §2, AI control and tables §3, aip reads §4, target search §5,
  distances §6, tactics helpers §7, skill check §7.4, commands §8, the
  conventions of §9; the Npc helpers of §9.9); `monsters/ai-bodies-2.md`
  (the "wait N" and "mode m at (x, y)" conventions);
  `monsters/ai-functions.tsv` (catalogue); `sim/pets.md` (player pet
  lists); `world/hirelings.md` (hireling ownership); `sim/path-placement.md`
  (placement §10, position history §10 rule 7, free points);
  `sim/pathing.md` (path fields and compute); `monsters/init.md` §1
  (spawn wrappers); `monsters/population.md` (free spot, spawn info);
  `skills/bodies.md` (skeleton mage AI param, summon skills);
  `world/quests.md` and the act files (quest seams).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 67–79 |
| Inputs | 80–89 |
| Outputs / state changes | 90–96 |
| Rules | 97–98 |
|   1. Scope and order | 99–134 |
|   2. Shared pet helpers | 135–243 |
|   3. NecroPet (67) `0x005E4CF0` | 244–292 |
|   4. MinionSpawner (121) `0x005E2BD0`, init `0x005F6630` | 293–312 |
|   5. Towner (41) `0x005E7540` | 313–326 |
|   6. EvilHole (76) `0x005FB410` | 327–354 |
|   7. Hireable (61) `0x005E52D0`, alternate `0x005E5280` | 355–475 |
|   8. QuillMother (75) `0x005FB2A0` | 476–490 |
|   9. BaalTentacle (139) `0x005EF820` | 491–504 |
|   10. ElementalBeast (46) `0x005F6B70` | 505–523 |
|   11. NpcStationary (54) `0x005E73A0` | 524–551 |
|   12. MosquitoNest (83) `0x005E0260` | 552–569 |
|   13. DesertTurret (94) `0x005E0980` | 570–610 |
|   14. AssassinSentry (101) `0x005EA3D0`, init `0x005EA290` | 611–645 |
|   15. Catapult (123) `0x005E34C0` | 646–650 |
|   16. CatapultSpotter (126) `0x005EE040` | 651–689 |
|   17. Tentacle (56) `0x005F8F80` | 690–714 |
|   18. TentacleHead (57) `0x005F9270` | 715–732 |
|   19. Hydra (86) `0x005E9E60` | 733–743 |
|   20. Totem (109) `0x005ED9E0` | 744–764 |
|   21. Vendor (42) `0x005E9E00` | 765–770 |
|   22. Trap-Missile (77) `0x005FB5B0` | 771–783 |
|   23. TrappedSoul (99) `0x005E9F10` | 784–800 |
|   24. DruidWolf (108) `0x005ED710` | 801–883 |
|   25. CycleOfLife (111) `0x005EC8C0`, init `0x005EC6A0` | 884–914 |
|   26. NpcBarb (127) `0x005EDC50`, init `0x005EDC40` | 915–935 |
| Constants & data dependencies | 936–960 |
| Randomness | 961–969 |
| Edge cases & original bugs | 970–987 |
| Test vectors | 988–1012 |
| Provenance | 1013–1039 |
| Open questions | 1040–1048 |
<!-- /index -->

## Summary

The 55 AI functions left `unread` by `ai.md` §9 and `ai-bodies-2.md` …
`-5.md` are written here and in `ai-bodies-7.md`, in descending order of
how many live monstats rows use each AI (§1). Every body follows the
`ai.md` §9 conventions: T, D, C from the dispatch record, P(aipN) one
unit-seed step tested `lo' % 100 < aipN`, "draw" = one unit-seed step
read as `lo' % 100`, draws listed in order, "idle N" = `0x005DE080(N)`,
"wait N" = `0x005DE0F0(N)` (no mode change), A1 / A2 = mode request 4 /
5 at T (`0x005DDF90`), "Skill k at X" = `0x005DEAD0(Sk<k>mode, Skill<k>,
X, 0, 0)` (`ai.md` §7.1). Pets and hirelings share the helpers of §2.
Bracketed values are the Normal values of the named live row.

## Inputs

| Name | Type | Source |
|---|---|---|
| tick record | `ai.md` §2.1 | control (+0x00), T (+0x08), D (+0x14), C (+0x18), monstats (+0x1C) |
| AI control | `ai.md` §3.1 | params 0–2 at +0x14, +0x18, +0x1C; minion owner +0x2C / +0x30 |
| monstats | 424 bytes | `aip1`..`aip8` (signed, by difficulty), `Skill1`..`Skill8` (+0x170..), `Sk1mode`..`Sk8mode` (+0x180..), `Velocity` +0x32, `Run` +0x34 |
| owner player | unit type 0 | path (+0x2C) target point +0x10 / +0x12 and final point +0x18 / +0x1A; player data +0x44 pet lists, +0xA0 / +0xA8 position history, +0x148 / +0x14C last placed point |
| game | | +0x6D difficulty, +0xA8 frame |

## Outputs / state changes

Mode requests, velocity requests, think schedules, AI control params,
unit flags (+0xC4, +0xC8), unit placement (pets), spawned monsters
(spawners), states and overlays on spawns, unit-seed draws in the order
given.

## Rules

### 1. Scope and order

Rows counted by script over the live `patch_d2` monstats.txt (`AI`
column; the counts equal `ai-functions.tsv` `monstats_rows`). AI
index, rows, section:

| AI (index) | Rows | § |
|---|---|---|
| NecroPet (67) | 9 | §3 |
| MinionSpawner (121) | 9 | §4 |
| Towner (41) | 6 | §5 |
| EvilHole (76) | 6 | §6 |
| Hireable (61) | 5 | §7 |
| QuillMother (75) | 5 | §8 |
| BaalTentacle (139) | 5 | §9 |
| ElementalBeast (46) | 4 | §10 |
| NpcStationary (54) | 4 | §11 |
| MosquitoNest (83) | 4 | §12 |
| DesertTurret (94) | 4 | §13 |
| AssassinSentry (101) | 4 | §14 |
| Catapult (123) | 4 | §15 |
| CatapultSpotter (126) | 4 | §16 |
| Tentacle (56) | 3 | §17 |
| TentacleHead (57) | 3 | §18 |
| Hydra (86) | 3 | §19 |
| Totem (109) | 3 | §20 |
| Vendor (42) | 2 | §21 |
| Trap-Missile (77) | 2 | §22 |
| TrappedSoul (99) | 2 | §23 |
| DruidWolf (108) | 2 | §24 |
| CycleOfLife (111) | 2 | §25 |
| NpcBarb (127) | 2 | §26 |

The AIs used by one live row (and ShadowMasterNoInit, by none) continue
in `ai-bodies-7.md`, same order.

### 2. Shared pet helpers

U is the pet (the thinking unit), O its owner player. "Walk-del to (x,
y)" = `0x005DEE50` (walk to coordinates, path step 1, think delete on
failure), "run-del to (x, y)" = `0x005DEEB0` (below), "walk to (x, y)"
= `0x005DED90`, "run to (x, y)" = `0x005DEDE0` (`ai.md` §7.2). Each
returns 1 when the mode change started. "Velocity (m, s, n)" =
`0x005DE190(U, m, s, n)` (`ai.md` §7.3). F := O's path final point
(+0x18, +0x1A; `0x00648A20` / `0x00648A30`), the point O is heading
to (`sim/pathing.md` §3 step 8). Positions are path positions.

**Run-del** `0x005DEEB0(x, y)`: mode 15 (run), or mode 2 with the
velocity reset `0x005A61F0` when U has state 60 (`decrepify`); path
step count 1; mode request at (x, y) with flag 1; on failure delete U's
thinks (`0x00540E60(2, 0)`). Returns the mode-change result. No draws.

**Wander'** `0x005DF400(U, X, n)`: X is not read. The same three or four
draws as wander n (`ai.md` §7.2 `0x005DE200`, around U's own position,
n as a byte), then mode 2 at the point with path step 1 and request
flag 1. Returns the mode-change result (no fallback, no think delete).

**Search capped** `0x005DDE50(U, control, &E, &M, k)`: the main search
`0x005DD7F0` for U (`ai.md` §5.2); its result only when E ≤ k (signed),
else 0 (E and M keep what the search wrote).

**Pet move** `0x005E3EA0(game, O, U, k, run, speed, n)` (D2MOO
`D2GAME_AI_PetMove`) → 1 when something was started or scheduled, else
0. By k:

- **0 (go ahead of the owner):** e := the 64-step direction from O's
  position to F (`0x00621DC0` → `0x0064FDC0`), reduced to 8 directions
  by table `0x00745600` (`ai-bodies-4.md` §2 birth). c0 := the
  coordinate index at O's position (`0x0061B130(O's room, x, y)`,
  `monsters/population.md`). For i = 0..7: j := {4, 3, 2, 1, 0, 7, 6,
  5}[e] (table `0x006E34F8`); Q := F + 8 × (dx[j], dy[j]) (tables
  `0x006EA998` / `0x006EA978`, as `ai-bodies-4.md` §2). When the
  coordinate index at Q (looked up from O's room) is c0: velocity (0,
  speed, 40); run ≠ 0 → run-del to Q, else walk-del to Q; started →
  1. Else the same to the midpoint ((U.x + Q.x) >> 1, (U.y + Q.y) >>
  1) (unsigned shift); started → 1. The midpoint try belongs to the
  "index is c0" case only (Q's try did not start; no second velocity
  call); an index ≠ c0 tries nothing (`0x005E4049` skips both to the
  loop step `0x005E40D2`). Then e := (e + 1) & 7. After the 8
  tries: delete U's thinks; wander' 4 → 1 if started. run ≠ 0 →
  run-del to ((U.x + O.x) / 2, (U.y + O.y) / 2) (signed halving) → 1 if
  started. Walk-del to the same midpoint → 1 if started. Return walk to
  O's position.
- **1 (retrace the owner's steps):** O in mode 2 (walk): velocity (0, 0,
  100); walk-del to F → 1; walk-del to ((U.x + F.x) / 2, (U.y + F.y) /
  2) → 1. Then s := speed, or `roll(40)` + 40 (one draw) when speed =
  0. Walk O's position history (player data +0xA0 next index, +0xA8 +
  8i entries, `sim/path-placement.md` §10 rule 7) backwards from the
  newest: b := index − 1 (index 0 → 19), then b − 1 each step (0 → 19),
  20 entries. An entry (x, y) with x ≠ 0, y ≠ 0 and path distance U→(x,
  y) > 5 (`0x005DC5C0`, unsigned): velocity (0, s, 100); run ≠ 0 → run
  to it (`0x005DEDE0`), else walk-del → 1 if started. Delete U's
  thinks; velocity (15, s, 100); run-del / walk-del → 1. If not yet
  done on this call: write the AI param record directly
  (`0x005A6260(record, method 1, s, 100)`; record = monster data +0x2C,
  `0x005A6190`); run-del / walk-del → 1. After the 20 entries: d := the
  full-size distance U→O (`0x005DC380`) >> 2 (unsigned), at least 4;
  velocity (15, 0, 0); wander' d → 1 if started, else 0.
- **2 (loiter):** `roll(100)` < 10: wander' (`roll(3)` + 3) → 1;
  delete U's thinks; velocity (0, 0, 40); walk-del to F → 1. Then (also
  when the first draw is ≥ 10) idle 15, return 1.
- **3 (catch up):** cl := `0x0061AD30(O's room, O.x, O.y)`; free spot
  `0x0054DC40(game, O's room, cl, class 363 necroskeleton (−1 when
  monstats has ≤ 363 rows), &x, &y, 0)` (`monsters/population.md`);
  none → 0. Room at (x, y) from O's room (`0x00463740`); none → 0. Place
  U there (`0x00554EA0(game, U, room, x, y, exact 0, alt 0)`,
  `sim/path-placement.md` §10); fails → 0. Queue U for update
  (`0x0064C040`), U +0xC8 |= 0x10000, idle 5, return 1.
- **4 (make room):** count O's other pets crowding U: every living node
  of O's pet lists (`0x00574DE0`: types 1 … count − 1, nodes without
  flag bit 0, unit found by GUID; `sim/pets.md` §1) other than U whose
  full-size distance from U is ≤ 1 (callback `0x005E3900`). Count ≥ 1:
  delete thinks; escape from O by n with think delete (`0x005DEFE0(O,
  n, 1)`) → 1; delete thinks; wander' n → 1. Then (also with count 0)
  idle 15, return 1.
- **5 (stay near):** velocity (7, 0, 0); wander near O by n
  (`0x005DF530(O, n)`) → 1; delete thinks; escape from O by n with
  delete → 1; velocity (0, 0, 40); walk-del to F → 1. Else 0.
- Any other k: 0.

Draws: only `roll(40)` (case 1, speed 0), the case-2 draws and the
wander / escape helpers' own draws, all on U's seed. The free-spot
search has its own (`ai.md` open question 4).

**Pet follow** `0x005E45D0(game, U, S, O, M, tick, quiet, n)` (D2MOO
`sub_6FCE34E0`) → the pet-move result, or 0:

1. D := full-size distance U→O. R := n + (O's total pet count >> 1)
   (sum of the count words of all pet types, `0x00574F40`), at most 36.
2. D ≤ 1, O in mode 1 (neutral) and M = 0 → pet move k 5 (n := R).
3. S ≠ 0 and U's room not in town (`0x0061AB00`): D ≤ 80 → return 0;
   else pet move k 3.
4. k := 2. O in mode 2, 3 or 6 (walk, run, town walk) → k := 0. O's
   path target point (+0x10, +0x12) differs from F in **both** x and y →
   k := 0. The coordinate index at O's position ≠ the one at U's → k
   := 1. D > R → k := 1. D > 50 → k := 3. Path distance from **U** to
   O's last placed point (player data +0x148, +0x14C; unsigned) < 28:
   k = 1 and D < 30 → k := 4; k = 2 → k := 4.
5. quiet ≠ 0 and k = 2 → return 0. Else pet move k (run 0, speed 0, n
   := R).

No draws of its own. D2MOO measures step 4's last distance from the
pet as well. 1.14d-confirmed (`0x005E3EA0`, `0x005E45D0`, `0x005DF400`,
`0x005DEEB0`, `0x005DDE50`, `0x00574DE0`, `0x005E3900`, `0x00574F40`).

### 3. NecroPet (67) `0x005E4CF0`

Target mode 0. Rows: claygolem, bloodgolem, irongolem, firegolem,
valkyrie, necroskeleton, necromage, wolf, bear. AI params: 0 = ranged
(set to 1 for the skeleton mage by the raise skill, `skills/bodies.md`
§6.15; 0 for every other pet), 2 = the owner's GUID. O := the minion
owner (`0x0058F0D0`). First, delete the unit's thinks (`0x00540E60(2,
0)`); then param 0 ≠ 0 → **ranged** (`0x005E4AC0`), else **melee**
(`0x005E4830`).

Both start alike: O = 0 → param 2 ≠ 0 and a player with that GUID
exists (`0x00552F60(game, 0, param 2)`) → pet move k 3 toward it (run,
speed, n = 0); else idle 10. End. O ≠ 0 → param 2 := O's GUID.
"Owner view" T0 := the main search `0x005DD7F0` run for **O** with the
pet's control record (`ai.md` §5.2); T0 with unit flag 0x40000000
(`petIgnore`, `monsters/init.md`) → T0 := 0.

**Melee** (aips unused):

1. D := full-size distance U→O. D > 50 → pet move k 3 (0, 0, 0). End.
2. v := `Run` × 100 / `Velocity` − 100 (signed, truncating); v := 100
   when `Velocity` ≤ 0 or v ≥ 100 (negative v is kept).
3. D > 28 → pet move k 0 (run 0, speed v, n 0). End.
4. T0 as above. S, E, M := search capped (U, control, 24). S = 0 or E
   > 6 → S := 0, and when T0 ≠ 0 and the full-size distance U→T0 < 36:
   S := T0 (M stays as the search wrote it).
5. Direct reach `0x005DC640(U, S)` = 0 (also for S = 0) → S := 0.
6. Draw r. Pet follow (S, O, M, quiet = 1, n = 8) when r < 15, else
   (quiet 0, n 7). Nonzero → end.
7. S = 0, or U's room is in town → wander' 4. End.
8. M ≠ 0: `roll(100)` < 80 → A1 at S; else idle 10. End.
9. M = 0: velocity (0, 0, 12); lunge at S with flags 7 (`0x005DED40`).

**Ranged** (the skeleton mage, `Skill1` NecromageMissile):

1. T0 as above (the full-size distance U→O is computed and discarded).
2. S, E, M := `0x005DDC30(U)` (`ai.md` §5.3). S = 0 or E > 15 → S :=
   0, and when T0 ≠ 0, the line U→T0 is clear (`0x00622AA0(U, T0, 4)`
   = 0) and the full-size distance U→T0 < 20: S := T0.
3. Draw r; pet follow as melee step 6. Nonzero → end.
4. S = 0, or U's room is in town → wander' 4. End.
5. `roll(100)` < 80 → `Skill1` at S. End. `roll(100)` < 75 → idle 10.
   End. Circle 3 at S (`0x005DF7D0(S, 3, 0)`).

Draws, in order: r; then step 8 / step 5's `roll(100)`s; plus the
helpers'. Both "in town" branches after the follow (idle 20) are
unreachable (the same test passed just before). 1.14d-confirmed;
D2MOO same.

### 4. MinionSpawner (121) `0x005E2BD0`, init `0x005F6630`

Brackets: minionspawner1 [100, 25, 100, 20, 25]; `Skill1`
MinionSpawner (`A1`). Init (`ai.md` §9.17): AI param 0 := frame, param
1 := 0. Params: 0 = next frame f, 1 = summons made n.

1. n ≥ aip1 [100] (signed) → return, **nothing scheduled**.
2. D > aip4 [20; 22, 25] → idle aip2 [25]. End.
3. frame < f → idle aip2. End.
4. Count scan: scan 1 (`ai.md` §5.4), callback `0x005E2B70`, arg {c =
   0}. U counts when it is a monster other than the scanner, not in
   mode 0 or 12 (`0x0063EA40`), with `BaseId` 453 (minion1) or 461
   (suicideminion1) (−1 for a missing row) and alignment 0
   (`0x006259B0`): c += 1. Never stops early.
5. `Skill1` ≥ 0 and c < aip2 [25] → n += 1; f := frame + aip3 [100];
   `Skill1` at T. End.
6. Idle aip2.

No draws. 1.14d-confirmed.

### 5. Towner (41) `0x005E7540`

Target mode 0. Rows: act2male, act2female, act2child, act3male,
act3female, act3child (town walkers). The Npc helpers of `ai.md` §9.9,
in order, each ending the think when it returns nonzero:

1. Home `0x005E6800` (`ai.md` §9.9 step 1).
2. Commands `0x005E6AE0`.
3. Map AI `0x005E7080`.
4. Idle 12.

No interaction handler and no class cases (unlike Npc). Draws: those
of the commands and map-AI handlers (`ai.md` §9.9). 1.14d-confirmed.

### 6. EvilHole (76) `0x005FB410`

Brackets: evilhole1 [10, 50]; demonhole [3, 25]. Params: 0 = next
spawn frame f, 1 = spawns left n. m := the unit's anim mode (+0x10).

1. f ≤ 0: f := frame + aip2 [50]; n := aip1 [10]. (The first think.)
2. m = 1 (neutral): D > 5 → idle 5, end. Else mode 10 at (0, 0)
   (`0x005DDFC0`, `ai.md` §7.1); wait 20. End.
3. m = 10 → mode 11 at (0, 0); wait 20. End.
4. m = 11 and n > 0: if frame > f: f := frame + aip2; spawn info
   `0x0063EFA0(unit, &class, &x, &y, &mode, 0, 0)`: for `BaseId` 321
   (evilhole1) class := 19 (fallen1), or 712 (megademon6) when the
   unit's class is 711 (demonhole) (−1 when monstats is too short),
   (x, y) := the unit's position, mode 1, no draws. Room at (x, y)
   from the unit's room (`0x00463740`); found → spawn
   `0x005B2F20(game, room, x, y, class, mode, spread 2, flags 0x42)`
   (`monsters/init.md` §1); spawned M → n −= 1, M +0xC4 |= 0x04020000;
   the unit's class is 711 → state 184 (`uberminion`) on M
   (`0x00639DB0(M, 184, 1)`) and overlay 202 on M (`0x00621E40(M, 202,
   0)`: stat 178 `unit_dooverlay` in M's flag-0x80 stat list,
   `combat/damage.md`). Then (spawned or not, due or not) wait aip2.
   End.
5. Any other mode, or m = 11 with n ≤ 0 → mode 0 (death) at (0, 0).

The `spawn` column (fallen1, megademon6) is not read here; the spawn
info table gives the same classes. No draws (the spawn's own:
`monsters/population.md` §9). 1.14d-confirmed.

### 7. Hireable (61) `0x005E52D0`, alternate `0x005E5280`

Target mode 0. Rows: roguehire (271), act2hire (338), act3hire (359),
act5hire1 (560), act5hire2 (561); aip1 = 1 on 338, 560, 561 (melee),
0 on 271, 359. AI param 0 = "frustration" p (raised by the attack
helper below). cls := the unit's class (−1 for none). O := the minion
owner (`0x0058F0D0`). The alternate is Nihlathak's (`ai-bodies-5.md`
§23). "Hireling move" = `0x005E3930` (below).

1. O = 0, or O is not a player → install special state 5
   (GoodNpcRanged, `ai.md` §9.31) when cls = 271, else 6
   (SpecialState06, `0x005E7C10`) through `0x005B0E00(game, unit,
   control, state)` (`ai.md` §3.3); idle 10. End.
2. The unit has state 12 → state 12 off (`0x00639DB0(unit, 12, 0)`).
3. m := the unit's anim mode; m = 2 (walk) or 15 (run) → return,
   nothing scheduled (the move's end re-thinks inline, `ai.md` §1.4).
4. Range: 16 < p < 20 → far := 2p, near := p, h := p >> 1
   (arithmetic); else far := 24, near := 16, h := 5. (The current
   command is read, `0x0058EE80`, and not used.)
5. D := full-size distance unit→O (`0x005DC380`, compared unsigned).
   D > 100 → hireling move k 3 (run 0, speed 0, n 0). End. D > far →
   hireling move k 1 (run 1, speed 60, n 0). End. D > near: O in mode 2
   or 6 → hireling move k 0 (0, 0, 0), end; O in mode 3 → k 0 (1, 60,
   0), end.
6. m ≠ 1 → delete the unit's thinks; idle 5. End.
7. q := 0 for cls 338, 560, 561, else 1. B := the unit's position
   collides with mask 0x40 over its pattern (`0x0064D910(room, x, y,
   pattern 0x00649180, 0x40)`). B: one step `mask(128)`
   (`0x00472210(seed, 128)`: `lo' & 127`) < (q ? 12 : 6) → wander 5.
   End.
8. The unit's room not in town: S, E := `0x005DDC30(unit)` (`ai.md`
   §5.3); S ≠ 0 and E < 25 (unsigned) → hireling attack (game, unit,
   cls, O, S, tick) (below). End.
9. B → wander 5. End.
10. The coordinate index at O's position ≠ the one at the unit's
    (`0x0061B130`, each from its own room) → hireling move k 0 (1, 60,
    0). End.
11. D2 := full-size distance unit→O ≤ 1: h −= 1 (as a byte); velocity
    (7, 0, 0); wander near O by h (`0x005DF530`) started → end. Delete
    thinks; escape from O by h with think delete (`0x005DEFE0(O, h,
    1)`) started → end. Velocity (0, 0, 40); walk to O's final point F
    (`0x005DED90`). End.
12. `roll(100)` < 5 → wander near O by near. End.
13. Idle 5.

**Hireling attack** `0x005E5050(game, U, cls, O, S, seed, tick)`:

1. a := 98 for cls 338, 560, 561; else a := p + 40 + 2 × U's stat 12
   (`level`, `0x00625480`), at most 95.
2. d := full-size distance U→S.
3. Node := U's node in O's pet lists by U's GUID (`0x00574BD0`,
   `world/hirelings.md` §5 rule 4); none → idle 10. End. w := the
   hireling `Id` at +8 of the record `0x00574BD0` returns (`0x005E50ED`).
4. Draw r. r < a → ok := 1, p := 0; else ok := 0, p += 10.
5. aip1 = 0 (ranged): d < 4 and a second draw < 50 → wander near O 4
   started → end; delete thinks; escape from S by 4 with delete started
   → end; else hireling skill (step 7) whatever ok. Otherwise ok →
   hireling skill; not ok → idle 10.
6. aip1 ≠ 0 (melee): d ≥ 3, or U not in melee range of S
   (`0x00622C40(U, S, 0)` = 0) → path step count := monstats2
   `MeleeRng` (byte +0x0E, `0x00649070`; overwritten at once by the
   run's step count 1, so without effect); run to S with flags 1
   (`0x005DED00(S, 1)`). End. Else ok → hireling skill; not ok → idle
   10.
7. **Hireling skill** `0x005E4D30(game, U, cls, w, S, seed)`:
   1. cls has no monstats row → return, **nothing scheduled**.
   2. L := U's level; H := the hireling row of id w at level L
      (`0x006562F0(game +0x70, w, L)`, `world/hirelings.md` §1.2 rule
      2); none → return, nothing scheduled.
   3. Δ := max(0, L − H.`Level`). acc := H.`DefaultChance` (+0x64).
      For i = 0..5 with k := H.`Skill<i+1>` (+0x78 + 4i): stop at the
      first k < 1 or ≥ the skills count. c_i := 0. When U has the skill
      (entry `0x006439B0(U, k, −1)` with level `0x006442A0(U, entry, 1)`
      > 0) and (skills `aitype` (+0x230) ≠ 1, or its `aurastate` is
      outside the states, or U lacks that state): k = 41 (Inferno), S ≠
      0 and level / 2 + 4 < `0x006416D0(U, S)` → skip (c_i stays 0);
      else acc += H.`Chance<i+1>` (+0x90 + 4i) + H.`ChancePerLvl<i+1>`
      (+0xA8 + 4i) × Δ / 4 (signed, toward 0); c_i := acc.
   4. r := `roll(acc + 1)` (one step; none when acc + 1 < 1, r := 0).
   5. r ≥ H.`DefaultChance`: i := the first index 0..5 with c_i ≥ r
      (signed; c_i of the slots after the step-3 stop were not written
      by this call, Edge case 4); found and k := H.`Skill<i+1>` > 0:
      k's skills row has
      the `aura` flag (+4 bit 5) → make k the right skill
      (`0x005701B0(U, k, −1)`) and idle 10; else `0x005DEAD0(H.`Mode<i+1>`
      (+0xC0 + i), k, S, 0, 0)`. End.
   6. Fallback: cls 271 → `Skill1` at S. cls 338, 359, 560, 561 → U in
      melee range of S → A1 at S, else idle 10. Any other → idle 10.

**Hireling move** `0x005E3930(game, O, U, k, run, speed, n)` → 1 / 0;
F, walk-del, run, velocity and wander' as §2. By k:

- **0:** as pet move k 0 (`§2`, the same direction table, here
  `0x006E34F0`), but each try is only velocity (0, speed, 40) and
  walk-del to Q (run is ignored; no midpoint). After the 8 tries: delete
  thinks; wander' 4; return 1.
- **1:** O in mode 2 → velocity (0, 0, 100); walk to F; return 1.
  Else s := speed, or `roll(15)` + 50 when speed = 0; the 20 history
  entries as pet move k 1, without the x, y ≠ 0 test: path distance > 5
  → velocity (0, s, 100), walk-del → 1; delete thinks; velocity (15, s,
  100); run ≠ 0 → run, else walk-del → 1; delete thinks; once per call:
  `0x005A6260(record, 1, s, 100)`, run / walk-del → 1; delete thinks.
  After the 20: d := full-size distance >> 2, at least 4; velocity (15,
  0, 0); wander' d; return 1.
- **2:** `roll(100)` ≥ 10 → idle 15, return 1. Else wander' (`roll(3)`
  + 3) → 1; delete thinks; velocity (0, 0, 40); walk to F; return 1.
- **3:** as pet move k 3 (free spot near O for class 363, place U, idle
  5).
- **4:** O's other pets within 1 of U (as pet move k 4): none → idle
  15, 1. Else delete thinks; escape from O by n with delete → 1; delete
  thinks; wander' n; return 1.
- **5:** velocity (0, 0, 0); wander near O by n → 1; delete thinks;
  escape from O by n with delete → 1; velocity (0, 0, 40); walk to F;
  return 1.
- Other k: 0.

Draws, in order: step 7's `mask(128)`; the attack's r, the ranged
second draw, the skill `roll(acc + 1)`; step 12's `roll(100)`; the
helpers'. 1.14d-confirmed (`0x005E52D0`, `0x005E5050`, `0x005E4D30`,
`0x005E3930`, `0x00574BD0`); D2MOO same.

### 8. QuillMother (75) `0x005FB2A0`

Brackets: quillbear1 [60, 50, 16, 15].

1. AI state 3/19 (`0x005DD2B0`): command {type 1, T's unit type (6
   when T = 0), T's GUID (−1)} copied to every minion of the unit's
   minion owner (`0x0058F730`, `ai.md` §8; params 3 and 4 are not
   written: uninitialised stack words in 1.14d, read by no minion AI
   here). Then C → A1 at T, else walk to T with flags 7. End.
2. Not C: draw < aip2 [50] → walk to T with flags 7; else idle aip4
   [15]. End.
3. C: draw < aip1 [60] → A1 at T; else idle aip3 [16].

1.14d-confirmed.

### 9. BaalTentacle (139) `0x005EF820`

Brackets: baaltentacle1 [70, 24, 10]. AI param 2 = expiry frame e.

1. The unit's owner (`0x00552FD0`) missing or dead (`0x005541B0`) →
   kill the unit (`0x0057CCB0(game, unit, 0, 1)`, `combat/damage.md`
   §7.2). End.
2. e = 0 → e := frame + (`roll(aip3)` + aip3) × 25 [250–475 frames].
3. frame > e → kill as step 1. End.
4. C and `roll(100)` < aip1 [70] → A1 at T. End.
5. Idle aip2 [24].

1.14d-confirmed.

### 10. ElementalBeast (46) `0x005F6B70`

Brackets: firebeast [20, 16, 20]; firebeast and iceglobe have
`SplEndGeneric`, so they re-think inline at the end of any mode
(`ai.md` §1.4). AI param 0 = state s.

1. s = 2 → kill the unit with T as killer (`0x0057CCB0(game, unit, T,
   1)`). End.
2. s = 0 and (D < aip2 [16; 20, 24] or AI state 3/19) → S1 (mode 8)
   at T; s := 1. End.
3. s ≠ 1 → idle aip3 [20]. End.
4. C → S1 at T; add a timer event of type 0 (MODECHANGE) on the unit
   at frame + 1 (`0x005417D0(game, unit, 0, frame + 1, 0, 0)`, no
   delete; `sim/tick.md` §5); s := 2. End.
5. `roll(100)` < aip1 [20] → walk to T with flags 0; else wander 8.

So a beast wakes with S1 when a target comes within aip2, chases, casts
S1 again on contact and dies on the next think. 1.14d-confirmed.

### 11. NpcStationary (54) `0x005E73A0`

Target mode 0. Rows: tyrael1 (251), tyrael2, izualghost (406), tyrael3.
AI param 1 = greeting countdown g.

1. P := the nearest interacting player within 15 (`0x005DDF20(game,
   unit, &close)`, `ai.md` §5.3; the NPC itself when none).
2. P = 0 or P = the unit (nobody near):
   - class 251: quest seam `0x0059DF50(game, unit)` ≠ 0 and "free"
     `0x005E7350` (below) → mode 0 at (0, 0) again, quest seam
     `0x0059C750(game)`; return (nothing scheduled).
   - class 406: quest seam `0x005B43F0(game, unit)` ≠ 0 and free →
     mode 0 at (0, 0) again, quest seam `0x005B4440(game)`; return.
   - Otherwise idle 20. End.
   **Free** `0x005E7350`: the unit's interaction list is empty
   (`0x00572DC0`, `world/npc.md` §2) → mode 0 (death: the NPC leaves)
   at (0, 0), return 1; else 0.
3. d := full-size distance unit→P. P is a player and `0x00535060(P)`
   = 1 → idle 10. End. The interaction list (monster data +0x30) is not
   empty → idle 10. End.
4. d ≥ 24: g ≤ 0 → g := 60; g −= 1; idle 20. End.
5. g ≠ 0 → g −= 1; idle 20. End. g = 0 → g := 60; P a player → sound 18
   on the unit toward P (`0x00553380(unit, 18, P)`); idle 20.

No draws. The quest seams are `world/quests-act2.md` (Tyrael, class
251) and `world/quests-act4.md` (Izual's ghost, class 406).
1.14d-confirmed.

### 12. MosquitoNest (83) `0x005E0260`

No init: AI params start at 0. Brackets: suckernest1 [16, 25, 200];
`Skill1` Nest (`seq_mosquitonest`). Params: 0 = next frame f, 1 =
summons made n.

1. D > aip2 [25; 26, 27] → idle 25. End.
2. n > aip1 [16; 17, 21] → unit flag 0x20000 (no drop); mode 0 (death)
   at (0, 0). End.
3. `Skill1` ≥ 0, frame > f and the footprint test `0x005FD350(334
   suckernest1 (−1 when monstats is too short), room, x, y, 0)` passes
   (`monsters/population.md` §9.3: point (x − 2, y − 2), mask 0x1C0) →
   n += 1; f := frame + aip3 [200]; `Skill1` at T. End.
4. Idle 25.

No draws. Like FoulCrowNest (`ai.md` §9.17) but with a range gate and
a cooldown instead of a quota-and-idle. 1.14d-confirmed.

### 13. DesertTurret (94) `0x005E0980`

Brackets: turret1 [10, 5, 120, 30, 5]; `Skill1` DesertTurret
(`seq_desertturret`). Params: 0 = next frame f, 1 = shots n, 2 = aim
index j (0..7).

1. `Skill1` ≥ 0 and f = 0 → `0x005DEAD0(Sk1mode, Skill1, 0, 0, 0)`
   (the deploy, no target); j := 0; f := frame. End.
2. frame < f → idle 10. End.
3. D > aip4 [30] → n > 0 → n −= 1; idle 15. End.
4. e := the 64-step direction from the unit to T's position reduced to
   8 (`0x00621DC0`, table `0x00745600`, `ai-bodies-4.md` §2); j :=
   J[e][j mod 8] (table `0x006E33E0`, below); Q := own position + aip5
   [5] × V[j] (table `0x006E33A0`). Path target unit := T
   (`0x00620C10`).
5. `Skill1` ≥ 0, the skill check `0x005FD470(Skill1, T, Q)` ≠ 0 and
   the second check `0x005FD470(Skill1, T, T.x, T.y)` ≠ 0 (T's own
   position, `0x0045ADF0` / `0x0045AE20` on T, `0x005E0BA6`–`0x005E0BCD`;
   `ai.md` §7.4) → `0x005DEAD0(Sk1mode,
   Skill1, 0, Q.x, Q.y)`; path target point := Q (`0x00648AD0`); n += 1;
   n > aip2 [5; 6, 7] → f := frame + aip3 [120; 100, 80], n := 0; else
   f := frame + aip1 [10; 7, 3]. End.
6. n > 0 → n −= 1. Idle 10.

V[0..7] = (1, 1) (0, 1) (−1, 1) (−1, 0) (−1, −1) (0, −1) (1, −1) (1, 0).
J[e][j] (the aim turns at most a step or two toward e; read from the
file):

| e \ j | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| 0 | 0 | 0 | 1 | 2 | 5 | 6 | 7 | 0 |
| 1 | 1 | 1 | 1 | 2 | 3 | 6 | 7 | 0 |
| 2 | 1 | 2 | 2 | 2 | 3 | 4 | 7 | 0 |
| 3 | 1 | 2 | 3 | 3 | 3 | 4 | 5 | 6 |
| 4 | 1 | 2 | 3 | 4 | 4 | 4 | 5 | 6 |
| 5 | 7 | 2 | 3 | 4 | 5 | 5 | 5 | 6 |
| 6 | 7 | 0 | 3 | 4 | 5 | 6 | 6 | 6 |
| 7 | 7 | 0 | 1 | 4 | 5 | 6 | 7 | 7 |

No draws. 1.14d-confirmed.

### 14. AssassinSentry (101) `0x005EA3D0`, init `0x005EA290`

Target mode 0. Rows: wakeofdestruction, chargeboltsentry,
lightningsentry, infernosentry (the assassin traps). Brackets:
lightningsentry [100, 10, 15, 25]. Init: AI param 0 := frame, param 1
:= −1. Param 1 = charges c (−1 = not yet counted).

**Charges** `0x005EA2B0(game, unit, tick, use)` → 1 when the trap was
removed:

1. The minion owner (`0x0058F0D0`) missing, without a room, or in a
   town room → mode 0 (death) at (0, 0); return 1.
2. c < 0: `Skill1`'s row missing → death, return 1. E := the unit's
   entry of `Skill1` (`0x006439F0`); none → death, return 1. c := the
   skill's `calc4` (skills +0x144) evaluated at E's level
   (`0x00646CA0(unit, calc4, Skill1, 0x006442A0(unit, E, 1))`,
   `data/calc-expressions.md`).
3. c > 0: use ≠ 0 → c −= 1; return 0.
4. c ≤ 0 → death; return 1.

Think:

1. Charges (use 0) = 1 → end.
2. E := the unit's entry of `Skill1`; none → death at (0, 0). End.
3. State 12 → off.
4. S, E2 := `0x005DDC30(unit)` (`ai.md` §5.3). S = 0 or E2 ≥ aip4
   [25] → idle aip3 [15]. End.
5. `roll(100)` ≥ aip1 [100] → idle aip2 [10]. End.
6. Charges (use 1) = 1 → end.
7. Delete the unit's thinks; `0x005DEAD0(E's mode (entry +0x08,
   `0x00644360`), Skill1, S, 0, 0)`.

So each shot spends a charge and the trap dies on the think after the
last one, or as soon as its owner leaves for town. 1.14d-confirmed.

### 15. Catapult (123) `0x005E34C0`

Brackets: catapult1 [20; 22, 25]. Draw < aip1 → A1 (mode 4) with no
target (`0x005DDF90(4, 0)`); else idle 15. 1.14d-confirmed.

### 16. CatapultSpotter (126) `0x005EE040`

Brackets: catapultspotter1 [8, 25, 25, 10, 10]; skills 287 Catapult
Charged Ball, 288 Catapult Spike Ball, 303 CatapultBlizzard, 304
CatapultPlague, 305 CatapultMeteor (table `0x006E3514`, first 5 of 8
entries). Params: 0 = ammo index a, 1 = volley count v, 2 = last shot
frame f (0 before the first think).

1. f = 0, or a draw ≤ 2 (drawn only when f ≠ 0): **catapult check**
   (below) ≠ 0 → mode 0 (death) at T (`0x005DDF90(0, T)`). End.
2. f = 0 → f := 1.
3. frame − f < aip2 [25] → idle aip2. End.
4. T = 0, or a draw ≥ aip1 [8; 12, 16] → idle aip2. End.
5. v < 1 → a := `roll(5)`, v := aip5 [10]. Else v −= 1.
6. x := own x + `roll(2·aip4)` − aip4 [10], then y := own y +
   `roll(2·aip4)` − aip4.
7. Free point near (x, y) in the unit's room (`0x0064E7E0(room, &pt,
   2, 0x805, 3)`, `sim/path-placement.md`); none → idle 15. End.
8. `0x005DEAD0(4, skill[a], 0, x, y)` (no target, at the point); f :=
   frame.

**Catapult check** `0x005EDF70(game, unit)` → 1 when the spotter's
catapult is dead: R := the unit's room (none → fatal); (rx, ry, w, h)
:= R's sub-tile box (`0x00619730`, `drlg/levels.md`); (dx, dy) :=
−table `0x006EA9D0`[p] with p the class's chain position (monstats
+0x4B, `0x006510C0`, `data/fixups.md`; spotter1..4 → 0..3; p outside
0..3 → fatal): table (0, 1), (1, 0), (1, 0), (−1, 0); class c := the
unit's class − 19 (catapultspotter1 516 → catapult1 497). For k = 1..3:
(rx, ry) += (w·dx, h·dy); `0x005429B0(game, R, rx, ry, c)` ≠ 0 →
return 1. Return 0.
`0x005429B0`: the room at (rx, ry) (from R, else from the act's room
list, `0x00619DA0`): a monster of class c in its unit list that is dead
(`0x005541B0`) → 1; else an inactive monster record of class c with
the dead bit (+0x18 & 4) in the area node of rx (`0x00541D20`,
`sim/units.md` §3.4) → 1; else 0.

Draws, in order: step 1's draw (when f ≠ 0), step 4's, `roll(5)` (step
5, when v < 1), the two point draws. 1.14d-confirmed.

### 17. Tentacle (56) `0x005F8F80`

Target mode 2. Brackets: tentacle1 [70, 5, 16, 12, 20, 12]; `Skill1`
Submerge (`seq_tentaclesubmerge`), `Skill2` Emerge (`S1`). AI params:
1 = timer frame t, 2 = state s (0 new, 1 submerged, 2 up). O := the
minion owner (the tentacle head, §18).

1. O = 0 → kill the unit (`0x0057CCB0(game, unit, 0, 1)`). End.
2. O in mode 12 (dead) and draw < 40 → kill the unit with killer O's
   path target unit (`0x00553540(game, O)`). End.
3. `Skill1` ≥ 0:
   1. s = 0 → **submerge**: `Skill1` at T; wait 8; t := frame + aip3 ×
      25 [400]; s := 1. End.
   2. s = 2, frame > t and (D > aip6 [12], or (C = 0 and draw < aip2
      [5]), or (O in mode 14 and draw < 50)) → submerge as 3.1. End.
      (Each draw only when the tests before it in its group hold.)
4. `Skill2` ≥ 0: s ≠ 1 → step 6. s = 1, frame > t and (C ≠ 0, or D <
   aip6, or (O not in mode 14 and draw < 5)) → **emerge**: `Skill2` at
   the unit itself (`0x005DEAD0(Sk2mode, Skill2, unit, 0, 0)`); t :=
   frame + aip4 × 25 [300]; s := 2. End.
5. s = 1 → wait aip5 [20]. End.
6. C and draw < aip1 [70] → A1 at T. Else idle aip5.

1.14d-confirmed.

### 18. TentacleHead (57) `0x005F9270`

Target mode 2. Brackets: tentaclehead1 [70, 5, 16, 12, 20, 12];
`Skill1` Submerge, `Skill2` Emerge; minion tentacle1. Params as §17.

1. `Skill1` ≥ 0: s = 0 → `Skill1` at T; wait 8; t := frame + aip3 ×
   25; s := 1. End. s = 2, frame > t and (D > aip6 or (C = 0 and draw
   < aip2)) → `Skill1` at T; **wait 20**; t := frame + aip3 × 25; s :=
   1. End.
2. `Skill2` ≥ 0: s ≠ 1 → step 4. s = 1, frame > t and (C ≠ 0 or D <
   aip6) → `Skill2` at the unit itself; t := frame + aip4 × 25; s := 2.
   End.
3. s = 1 → wait aip5. End.
4. S := `0x005DDC30(unit)` (`ai.md` §5.3; S may be 0). Draw < aip1 →
   A1 at S (`0x005DDF90(4, S)`); else idle aip5.

No owner checks and no random emerge (unlike §17). 1.14d-confirmed.

### 19. Hydra (86) `0x005E9E60`

Target mode 2. Rows hydra1–3; `Skill1` HydraMissile (`A1`). AI param 0
is the expiry frame set by the Hydra skill (`skills/bodies-2b.md` §8.5).

1. frame > param 0 → mode 0 (death) at (0, 0). End.
2. T ≠ 0, D < 25 and draw < 60 → `Skill1` at T. End.
3. Idle 10.

1.14d-confirmed.

### 20. Totem (109) `0x005ED9E0`

Target mode 0. Rows: spiritofbarbs, heartofwolverine, oaksage (druid
spirits). Brackets [20, 30, 30, 20]. O := the minion owner.

1. O = 0 → idle 10. End.
2. S, E, M := search capped (unit, control, 24) (§2). M ≠ 0 and S ≠ 0:
   draw < aip1 [20] and escape from S by 6 with think delete started →
   end.
3. Draw < aip2 [30] → S := 0, M := 0.
4. d := `0x006416D0(unit, O)` (`missiles/missiles.md`). d > aip3 [30]:
   place the unit at O's position (`0x00554EA0(game, unit, O's room,
   O.x, O.y, 0, 0)`, `sim/path-placement.md` §10); placed → idle 25.
   End.
5. d > aip4 [20]: O in mode 2 or 6 → pet move k 0 (0, 0, 0) started →
   end; O in mode 3 → pet move k 0 (run 0, speed 60, 0) started → end.
6. Pet follow (S, O, M, quiet 0, n 6) ≠ 0 → end.
7. Idle 25.

Draws: step 2's (when M, S), step 3's, the helpers'. 1.14d-confirmed.

### 21. Vendor (42) `0x005E9E00`

Target mode 0. Rows act2vendor1, act2vendor2. Draw < 20 → mode 8 at (0,
0) (`0x005DDFC0`, the vendor's idle animation); else idle 30.
1.14d-confirmed.

### 22. Trap-Missile (77) `0x005FB5B0`

Target mode 2. Brackets: trap-firebolt [25, 1, 15] (`MissA1`
trapfirebolt); trap-lightning (chainlightning). AI params: 0 = shots n,
1 = toggle.

1. T ≠ 0, D ≤ aip1 [25; 30, 32] and n < aip2 [1]: toggle = 0 → A1 at
   T, n += 1, toggle := 1; else toggle := 0, idle aip3 [15]. End.
2. Otherwise unit flag 0x20000 (no drop); mode 0 (death) at (0, 0).

So the trap fires aip2 times on alternate thinks and dies on the first
think without a target in range. 1.14d-confirmed.

### 23. TrappedSoul (99) `0x005E9F10`

Rows trappedsoul1, trappedsoul2 (`SplEndGeneric`: inline re-think at
every mode end, `ai.md` §1.4). AI params: 0 = awake, 1 = next frame f.
(xu, yu) := the unit's position, (xt, yt) := T's (`0x00620870`),
compared unsigned.

1. Unit flag 0x20000 (no drop), every think.
2. T = 0 or D > 4: param 0 ≠ 0 → S1 (mode 8) at T; else idle 15. End.
3. param 0 = 0 → param 0 := 1, f := frame, S2 (mode 9) at T. End.
4. C = 0 or frame ≤ f → S1 at T. End.
5. xt ≤ xu and yt ≥ yu → A1 at T, f := frame + 35. End.
6. xt ≥ xu and yt ≤ yu → A2 at T, f := frame + 35. End.
7. S1 at T, f := frame + 5.

No draws. 1.14d-confirmed.

### 24. DruidWolf (108) `0x005ED710`

Target mode 0. The unit's class 420 (spiritwolf) → **wolf**
`0x005ECEE0`; any other (421 fenris) → **fenris** `0x005ED2A0`.
Brackets: spiritwolf [22, 20, 14, 20, 26]; fenris [22, 20, 25, 24,
30]. Spiritwolf `Skill1` Teleport 2; fenris `Skill1` fenris rage,
`Skill2` Teleport 2. "Port" := the teleport skill (wolf `Skill1`, fenris
`Skill2`) in its mode with no target at a point (`0x005DEAD0(mode,
skill, 0, x, y)`). AI params: 0 = rage flag (fenris), 1 = rage corpse
GUID (fenris, written only), 2 = owner GUID. v := `Run` × 100 /
`Velocity` − 100, 100 when `Velocity` ≤ 0 or v ≥ 100. r := aip4. O :=
the minion owner.

Common start (both):

1. O = 0: param 2 ≠ 0, the player with that GUID exists and the port
   skill ≥ 0 → port to that player's position; a timer event of type 0
   (MODECHANGE) at frame + 6 (`0x005417D0`, no delete); wait 8. End.
   Else idle 10. End.
2. param 2 := O's GUID.
3. The unit's room in town → pet follow (0, O, 0, quiet 0, n 6) ≠ 0 →
   end; else idle 33. End.
4. S, E, M := search capped (unit, control, r). T0 := the main search
   for O with the unit's control (as §3).

#### 24.1 Wolf `0x005ECEE0`

After the common start of §24:

5. S ≠ 0 and not directly reachable (`0x005DC640(unit, S)` = 0) → S :=
   0. Then, when S is now 0: T0 ≠ 0, full-size distance unit→T0 < r and
   T0 **not** directly reachable → S := T0 (bug kept: the reachable
   case is the one refused).
6. d := `0x006416D0(O, unit)`. `Skill1` ≥ 0 and d > 50 → port to O's
   position; MODECHANGE timer at frame + 4; wait 10. End.
7. d > aip5 [26] → pet move k 0 (run 1, speed 100, n 0) started → end.
   d > aip3 [14]: O in mode 2 or 6 → pet move k 0 (0, 0, 0) started →
   end; O in mode 3 → k 0 (1, 100, 0) started → end. Pet follow (S, O,
   M, quiet 1, n 6) ≠ 0 → end. The pet follow is evaluated whatever d
   is (d ≤ aip3 and every not-started pet move fall through to it,
   `0x005ED199`); a not-started aip5 move still goes on to the aip3
   test.
8. S ≠ 0: M ≠ 0 → A1 at S; wait aip1 [22]. End. `0x006416D0(O, S)` <
   r → velocity (0, v, 0); run to S (`0x005DED20`). End. d > 10 → walk
   in radius of O (`0x005DE6D0(O, 8, 6)`). End. Else idle 15. End.
9. `roll(100)` < aip2 [20] → wander 10; else idle 15.

#### 24.2 Fenris `0x005ED2A0`

After the common start of §24:

5. S ≠ 0 and not directly reachable → S := 0. When S is now 0: T0 ≠ 0,
   T0's search distance (from O) < r and T0 directly reachable → S :=
   T0, E := that distance.
6. d := `0x006416D0(O, unit)`. S ≠ 0, d > r and `0x006416D0(O, S)` >
   r → S := 0.
7. `Skill2` ≥ 0 and d > 50 → port to O's position; MODECHANGE timer at
   frame + 2; wait 10. End.
8. d > aip5 [30] → pet move k 0 (1, 100, 0); end (whatever it
   returned). d > r: O in mode 3 → the same, end; O in mode 2 or 6 →
   pet move k 0 (0, 0, 0), end.
9. Pet follow (S, O, M, quiet 1, n 6) ≠ 0 → end.
10. Rage: `Skill1` < 0, or the unit has state 138, or (`roll(100)` ≥
    aip3 [25], S ≠ 0 and param 0 = 0) → param 0 := 0, step 11. Else K
    := the corpse search `0x005D2F80(game, unit, O, 10)` (§24.3); K and
    `0x006416D0(K, unit)` < r / 2 (signed halving): the unit in melee
    range of K → param 0 := 0, `Skill1` at K, end; else M = 0 → run to
    K, param 0 := 1, param 1 := K's GUID, end; else step 11's A1 branch.
    No K, or too far → step 11.
11. M ≠ 0 → A1 at S; wait aip1. End. S ≠ 0 → velocity (0, v, 0); run
    to S. End. `roll(100)` < aip2 → wander 10; else idle 15.

#### 24.3 Corpse search `0x005D2F80(game, U, X, n)`

X is not read. Unit find
(`0x0065A950` init, `0x0065AC70` collect, `0x0065AA00` free; flags
0x1002, size n, around U's position, U's room; `ai-bodies-3.md` open
question 3) → the first found unit that passes the corpse test
`0x00623600` (skills spec) and the hostility test `0x00554200(game, U,
·)` (`combat/hit.md`); else 0. No draws.

1.14d-confirmed (all four functions).

### 25. CycleOfLife (111) `0x005EC8C0`, init `0x005EC6A0`

Target mode 2. Rows: cycleoflife (426, `Skill1` CorpseCycler),
vinecreature (427, `Skill1` VineCycler); brackets [50, 20, 25, 10, 35].
Init: AI param 0 := 0. AI param 1 = last cast frame f. O := the minion
owner.

1. O = 0 → return, nothing scheduled.
2. d := `0x006416D0(unit, O)`. d ≥ aip5 [35] → pet move k 3 (run 0,
   speed 0, n 6) ≠ 0 → end.
3. T ≠ 0 and T dead (`0x005541B0`) → T := 0, C := 0.
4. K := 0, dK := 0. `Skill1` > 0, its row exists and the unit has its
   entry (E := `highest_entry(unit, Skill1)` `0x006439F0`, `skills/levels.md`;
   none → K stays 0): n := the skill's `aurarangecalc` (skills +0x64) at
   the entry's level L = `skill_level(unit, E, 1)` (`0x006442A0`, bonus
   1; then `0x00646CA0`), clamped to 5..50; K := corpse search (game,
   unit, O, n) (§24.3); K → dK := `0x006416D0(unit, K)`.
5. dK < aip2 [20]: K → mK := melee range unit→K. Else K := 0.
6. Pet follow (K, O, mK, quiet 0, n 6) ≠ 0 → end.
7. need := 1; class 426: need := O's life (stat 6) < O's max life
   (`0x00625D10`); class 427: need := O's mana (stat 8) < max mana
   (`0x00625D60`).
8. K, mK, need and frame > f + aip1 [50] → `0x005DEAD0(8, Skill1, K, 0,
   0)` (mode 8 fixed); f := frame. End.
9. C, T and `roll(100)` < 25 → escape from T by aip4 [10] (byte), no
   delete. End.
10. K = 0 → idle aip3 [25]. End.
11. Walk to K with flags 7.

1.14d-confirmed.

### 26. NpcBarb (127) `0x005EDC50`, init `0x005EDC40`

Target mode 2. Rows act5barb1, act5barb2 (Harrogath's defenders);
brackets [15, 85, 15]. The init returns at once.

1. The unit's target-node slot (unit +0xD0) is 11 (none) → register it
   in slot 8 of the game's target-node lists (`0x005B1990(game, unit,
   0, 8)`: a node {unit, 0} pushed at the head of game +0x10F8 + 8 × 4;
   unit +0xD0 := 8; `ai.md` §5.2 slot 8).
2. T ≠ 0: C → A1 at T; wait aip1 [15]. End. D < aip3 [15] and draw <
   aip2 [85] → velocity (0, 100, 0); run to T (`0x005DED20`). End.
3. Roam, three tries, each a walk-del to a point (§2), ending on the
   first that starts:
   1. Two steps a, b: (own x + a % 20 − 40, own y + b % 20 − 10).
   2. Three steps c, d, e; σ := +1 when `c & 1` = 1, else −1: (own x + d
      % 20 − 10, own y + e % 20 − 10 − 30σ).
   3. Two steps f, g: (own x + f % 20 − 10, own y + g % 20 − 10 + 30σ).
   All three fail → idle 15.

`%` is unsigned on the step's low word. 1.14d-confirmed.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| pet move directions | j = {4, 3, 2, 1, 0, 7, 6, 5}[e], 8 tiles ahead of F | `0x006E34F8`, `0x005E3EA0` |
| pet move | velocity steps 40 / 100; history 20 entries, > 5 tiles; catch-up class 363 | `0x005E3EA0` |
| pet follow | R = n + pets / 2 ≤ 36; D 1 / 28 / 30 / 50 / 80; last placed point < 28 | `0x005E45D0` |
| NecroPet | far 50, ahead 28, owner-view range 36 (melee) / 20 (ranged), own search 24 / 15, r < 15 quiet | `0x005E4830`, `0x005E4AC0` |
| minion scan | `BaseId` 453, 461; alignment 0 | `0x005E2B70` |
| hireling | range 24 / 16 / 5 or 2p / p / p >> 1 (17 ≤ p ≤ 19); D 100; attack chance 98 or p + 40 + 2·level ≤ 95; p += 10 on a miss; skill weights from hireling.txt `DefaultChance`, `Chance*`, `ChancePerLvl*` / 4; Inferno reach level / 2 + 4 | `0x005E52D0`, `0x005E5050`, `0x005E4D30` |
| hireling move | k 1 speed `roll(15)` + 50 | `0x005E3930` |
| BaalTentacle | life (`roll(aip3)` + aip3) × 25 frames | `0x005EF820` |
| NpcStationary | greeting countdown 60, range 24, sound 18 | `0x005E73A0` |
| MosquitoNest | footprint class 334 at (x − 2, y − 2), mask 0x1C0 | `0x005E0260` |
| DesertTurret | aim tables J (8 × 8) and V (8) | `0x006E33E0`, `0x006E33A0` |
| sentry charges | skills `calc4` at the entry's level | `0x005EA2B0` |
| spotter | skills 287, 288, 303, 304, 305; check 3 % of thinks; catapult class − 19; 3 rooms back along table `0x006EA9D0` | `0x005EE040`, `0x005EDF70` |
| tentacles | timers aip3 / aip4 × 25 frames; owner-dead kill 40 %; random emerge 5 %, owner-skill submerge 50 % | `0x005F8F80`, `0x005F9270` |
| Hydra | range 25, 60 % | `0x005E9E60` |
| druid summons | teleport beyond 50; MODECHANGE +6 / +4 / +2; rage corpse within r / 2, state 138 | `0x005ECEE0`, `0x005ED2A0` |
| corpse search | unit find flags 0x1002 around the unit | `0x005D2F80` |
| CycleOfLife | search size `aurarangecalc` 5..50; mode 8; cooldown aip1 | `0x005EC8C0` |
| NpcBarb | target-node slot 8; roam offsets x −40..−21 then ±30 rows | `0x005EDC50`, `0x005B1990` |
| EvilHole | modes 1 → 10 → 11 → spawn → 0; trigger D ≤ 5; spawn classes 19 / 712; flags 0x04020000; state 184, overlay 202 for class 711 | `0x005FB410`, `0x0063EFA0` |

## Randomness

All draws are on the thinking unit's seed (`rng.md` §7), in the order
of each section; a test that is not reached draws nothing. Forms: `lo'
% 100` (draw, P(…), `roll(100)` `0x0045C390`), `roll(n)` for other n,
plus the helpers' draws (`ai.md` §7.2; wander' as wander). Scans and
callbacks draw nothing. Skill checks, spawns, free-spot searches and
quest seams have their own draws, owned by their specs.

## Edge cases & original bugs

1. MinionSpawner §4 step 1: at the summon cap the spawner stops
   thinking for good.
2. Wander' ignores its unit argument: "wander near O" callers in fact
   wander around the pet.
3. Pet move k 1 runs (`0x005DEDE0`) on the first try but run-dels on the
   second when run ≠ 0.
4. Hireling skill §7 step 7.5: the weights c_0..c_5 live on the stack
   and only the slots up to the step-3 stop are written. With no
   counting slot and r = `DefaultChance` > 0 (live rows have at most 3
   skills), the search reads stack words this call never wrote, so
   1.14d's pick is not defined by the inputs. d2rs: an unwritten c_i
   counts as −1, so the search ends in the step-6 fallback
   (`world/hirelings.md` edge case 12). After c_5 the search would read
   the next stack word (the monstats row pointer) and always stop there
   without a pick.

## Test vectors

Synthetic (CI-safe), draws given as `lo' % 100`:

| AI, input | Draws | Result |
|---|---|---|
| MinionSpawner, minionspawner1 Normal, n = 3, D = 10, frame ≥ f, 4 minion1 around, `Skill1` ≥ 0 | none | n := 4, f := frame + 100, `Skill1` at T |
| MinionSpawner, n = 100 | none | nothing scheduled |
| EvilHole, evilhole1, f = 0, m = 1, D = 3 | none | f := frame + 50, n := 10, mode 10, wait 20 |
| EvilHole, m = 11, n = 0 | none | mode 0 |
| NecroPet melee, D = 60 | none | pet move k 3 |
| QuillMother, C = 1, quillbear1 Normal, no AI state | 59 (< 60) | A1 at T |
| BaalTentacle, e = 0, frame 1000, baaltentacle1 | `roll(10)` = 3 | e := 1325 |
| ElementalBeast, s = 0, D = 10, firebeast Normal | none | S1 at T, s := 1 |
| DesertTurret, turret1 Normal, f = 0, `Skill1` ≥ 0 | none | deploy, j := 0, f := frame |
| CatapultSpotter, f = 500, frame 510, T ≠ 0 | 50 (> 2: no check) | idle 25 (510 − 500 < 25) |
| AssassinSentry, c = 1, S at E2 10, lightningsentry | 40 (< 100) | c := 0, shot; next think: death |
| Tentacle, s = 1, frame > t, C = 1 | none | Emerge at itself, t := frame + 300, s := 2 |
| Hydra, frame > expiry | none | mode 0 |
| TrappedSoul, awake, C = 1, frame > f, T at (xu − 1, yu + 1) | none | A1, f := frame + 35 |
| Trap-Missile, n = 1 = aip2 | none | death |
| Hireable, act2hire, p = 0, level 20, S at d = 2 in melee range, draw 79 | 79 (< 98) | p := 0; hireling skill |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005E3EA0` (jump table `0x005E45B8`, 6 cases),
  `0x005E45D0`, `0x005E3900`, `0x00574DE0`, `0x00574F40`, `0x005DF400`,
  `0x005DEEB0`, `0x005DDE50`, `0x005E4CF0`, `0x005E4830`, `0x005E4AC0`,
  `0x005E2BD0`, `0x005E2B70`, `0x005F6630`, `0x005E7540`, `0x005FB410`,
  `0x0063EFA0` (cases 0x141, 0x220, default), `0x00621E40`, `0x005E52D0`,
  `0x005E5050`, `0x005E4D30`, `0x005E3930` (jump table `0x005E3E7C`),
  `0x00574BD0`, `0x005DED00`, `0x005FB2A0`, `0x0058F730`, `0x005EF820`,
  `0x005F6B70`, `0x005E73A0`, `0x005E7350`, `0x005E0260`, `0x005FD350`,
  `0x005E0980`, `0x005EA3D0`, `0x005EA290`, `0x005EA2B0`, `0x00646CA0`,
  `0x005E34C0`, `0x005EE040`, `0x005EDF70`, `0x005429B0`, `0x0063EE10`,
  `0x005F8F80`, `0x005F9270`, `0x005E9E60`, `0x005ED9E0`, `0x005E9E00`,
  `0x005FB5B0`, `0x005E9F10`, `0x00620870`, `0x005ED710`, `0x005ECEE0`,
  `0x005ED2A0`, `0x005D2F80`, `0x005EC6A0`, `0x005EC8C0`, `0x005EDC40`,
  `0x005EDC50`, `0x005B1990`; tables
  `0x006E34F8`, `0x006E34F0`, `0x006E33E0`, `0x006E33A0`, `0x006E3514`,
  `0x006EA9D0` read from the file. Decompiler text read first; every
  call's register and stack arguments checked in the disassembly
  (`tools/ghidra/disasm.py`).
- Live data (`patch_d2`): monstats.txt (`AI`, `aip*`, skills, `spawn`,
  `BaseId`), states.txt (184), itemstatcost.txt (178) — counted by a
  throwaway script (scratch, not committed).
- D2MOO 1.10f `AiThink.cpp`: names (`D2GAME_AI_PetMove`,
  `sub_6FCE34E0`, `AITHINK_Fn067_NecroPet`) and the pet-follow case
  logic; compared for §2 and §3 (same rules).

## Open questions

1. No recording covers any AI of this file: record a game with a
   necromancer's summons, a hireling, a town walk and the spawners and
   traps of §4–§6 (tick recorder) and compare think schedules and draws.
2. Answered (2026-10-07): the free-spot search `0x0054DC40` is
   `monsters/population.md` §8 (room seed, x then y, up to 20 tries,
   test-only probe with class 363).
