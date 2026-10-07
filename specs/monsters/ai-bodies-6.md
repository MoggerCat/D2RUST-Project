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
| Summary | 47–59 |
| Inputs | 60–69 |
| Outputs / state changes | 70–76 |
| Rules | 77–78 |
|   1. Scope and order | 79–94 |
|   2. Shared pet helpers | 95–200 |
|   3. NecroPet (67) `0x005E4CF0` | 201–249 |
|   4. MinionSpawner (121) `0x005E2BD0`, init `0x005F6630` | 250–269 |
|   5. Towner (41) `0x005E7540` | 270–283 |
|   6. EvilHole (76) `0x005FB410` | 284–311 |
| Constants & data dependencies | 312–322 |
| Randomness | 323–331 |
| Edge cases & original bugs | 332–340 |
| Test vectors | 341–354 |
| Provenance | 355–371 |
| Open questions | 372–379 |
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

(Continued as the bodies are written; the remaining `unread` rows stay
listed in `ai-functions.tsv`.)

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
  1) (unsigned shift); started → 1. Then e := (e + 1) & 7. After the 8
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

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| pet move directions | j = {4, 3, 2, 1, 0, 7, 6, 5}[e], 8 tiles ahead of F | `0x006E34F8`, `0x005E3EA0` |
| pet move | velocity steps 40 / 100; history 20 entries, > 5 tiles; catch-up class 363 | `0x005E3EA0` |
| pet follow | R = n + pets / 2 ≤ 36; D 1 / 28 / 30 / 50 / 80; last placed point < 28 | `0x005E45D0` |
| NecroPet | far 50, ahead 28, owner-view range 36 (melee) / 20 (ranged), own search 24 / 15, r < 15 quiet | `0x005E4830`, `0x005E4AC0` |
| minion scan | `BaseId` 453, 461; alignment 0 | `0x005E2B70` |
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

## Test vectors

Synthetic (CI-safe), draws given as `lo' % 100`:

| AI, input | Draws | Result |
|---|---|---|
| MinionSpawner, minionspawner1 Normal, n = 3, D = 10, frame ≥ f, 4 minion1 around, `Skill1` ≥ 0 | none | n := 4, f := frame + 100, `Skill1` at T |
| MinionSpawner, n = 100 | none | nothing scheduled |
| EvilHole, evilhole1, f = 0, m = 1, D = 3 | none | f := frame + 50, n := 10, mode 10, wait 20 |
| EvilHole, m = 11, n = 0 | none | mode 0 |
| NecroPet melee, D = 60 | none | pet move k 3 |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005E3EA0` (jump table `0x005E45B8`, 6 cases),
  `0x005E45D0`, `0x005E3900`, `0x00574DE0`, `0x00574F40`, `0x005DF400`,
  `0x005DEEB0`, `0x005DDE50`, `0x005E4CF0`, `0x005E4830`, `0x005E4AC0`,
  `0x005E2BD0`, `0x005E2B70`, `0x005F6630`, `0x005E7540`, `0x005FB410`,
  `0x0063EFA0` (cases 0x141, 0x220, default), `0x00621E40`; tables
  `0x006E34F8` read from the file. Decompiler text read first; every
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
2. Pet move k 3 calls the free-spot search with class 363 for every
   pet; its draws are `ai.md` open question 4.
