# Spec: Monsters — AI think, per-AI behaviours (§9 of AI think)

- **Status:** draft: think scheduling, dispatch, AI tables and the
  shared helpers read from the 1.14d `Game.exe` (addresses below; AI
  tables dumped from the file); 37 AI functions read in full; think
  intervals checked against 4,409 recorded type-2 runs in three tick
  recordings (mode-end re-think: 79/79 at aidel; no rule contradicted).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions)
- **Related specs:** `monsters/ai.md` (owner of scheduling §1, dispatch §2, AI control and tables §3, aip reads §4, target search §5, distances §6, tactics helpers §7, commands §8, the catalogue §10, and of its Constants, Randomness, Edge cases, Test vectors, Provenance and Open questions; this file holds §9 moved out of it unchanged, rule ids kept); `monsters/ai-functions.tsv`; `monsters/ai-bodies-2.md` … `ai-bodies-5.md`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 20–26 |
| Rules | 27–28 |
|   9. Per-AI behaviours | 29–938 |
<!-- /index -->

## Summary

§9 of `monsters/ai.md`, split out of that file to keep it readable
(section numbers and rule ids are unchanged, so a reference names
this file and the same §). Status, evidence and open questions are
those of `monsters/ai.md`.

## Rules

### 9. Per-AI behaviours

Conventions: T = the dispatch target, D = its distance, C = combat flag,
P(aipN) = one unit-seed step tested `lo' % 100 < aipN` (`ai.md` §4), "pct(k)" =
the same against the constant k, "idle N" = `0x005DE080(N)`, A1/A2 =
mode request 4/5 at T. Draws are listed in order; a test that is not
reached draws nothing. Values in brackets are the Normal values of the
recorded classes.

#### 9.1 Recorded classes

| Class | Row | AI (index) | aip1..aip5 [Normal] | aidel |
|---|---|---|---|---|
| zombie1 | 5 | Zombie (3) | 30, 10, 0, 20, 0 | 15 |
| fallen1 | 19 | Fallen (6) | 30, 10, 50, 20, 0 | 15 |
| brute1 | 28 | Brute (7) | 0, 0, 100, 45, 0 | 15 |
| fallenshaman1 | 58 | FallenShaman (13) | 45, 60, 100, 24, 15 | 15 |
| quillrat1 | 63 | QuillRat (14) | 10, 35, 0, 2, 0 | 15 |
| cr_lancer1 | 165 | CorruptLancer (36) | 60, 75, 9, 0, 15 | 15 |
| cow, rogue1 | 179, 152 | Idle (1) | – | 15 |
| gheed, akara, kashya, charsi, warriv1 | 147, 148, 150, 154, 155 | Npc (32) | – | 15 |
| navi | 266 | Navi (58) | – | 15 |

All from the live monstats.txt; target mode 1 for indices 3, 6, 7, 13,
14, 36 and 0 for 1, 32, 58.

#### 9.2 None (0), Idle (1), Buffy (100)

- 0 `0x005B0CC0`: returns; schedules nothing.
- 1 `0x005B0CD0` and 100 `0x005E7F50` (two byte-identical copies): if
  the unit has a room, idle 200. No draws. Recorded: cow and rogue1 thinks
  are 200 frames apart (81/81 own schedules).

#### 9.3 Zombie (3) `0x005EFE20`

1. C: P(aip4) [20] → A1, else A2. End.
2. Not C: if AI state 3/19 → run (step 3). Else if D < aip2 [10] and
   P(aip1) [30] → run. Else if the unit's level is not 17 (Burial
   Grounds) → wander 3, end; level 17 → run.
3. Run: velocity request speed 100; run to T (`0x005DED20`, flags 0).

Note the draw happens only when D < aip2. Zombies never schedule a
think themselves: next think comes from the move end (inline) or a
failed move (`aidel`). 1.14d-confirmed; same as D2MOO.

#### 9.4 Fallen (6) `0x005F02C0`

1. Not if the anim mode is death (0) or dead (12). Delete all thinks.
2. Corpse check: for each adjacent room, for each of its four last-dead
   GUIDs (room +0x38..+0x44; `sim/units.md`): a monster in death mode
   (0) at no-size distance < 15 from the fallen →
   AI param 0 := 1; free the current command; velocity speed 50; escape
   from T by 12 with think delete. If the escape started: draw
   `lo' % 20`; 0 → sound 17. End. If not: stop the corpse check, go on.
3. Anim mode not neutral → idle 10. End.
4. Current command present:
   - type ≠ 1 → free it, idle 10. End.
   - C: P(aip3) fails → idle 5; else P(aip4) → A1 else A2. End.
   - Not C: walk to T (flags 0); if that fails free the command. End.
5. No command:
   1. Not C and AI state 3/19 → walk to T (flags 0). End.
   2. D < 15 and the fallen is its own minion owner (pack leader) and
      P(aip1) [30] → command type 1 to all minions (`0x0058F730`),
      copy it to itself, mode 9 (S2) at T. End.
   3. Not C: D ≤ aip2 [10] → walk to T (flags 7). Else pct(30) → wander
      3; else idle 10. End.
   4. C: if AI param 0 = 0 and P(aip3) [50] fails: pct(30) → mode 9 at T,
      else idle 10. End. Otherwise param 0 := 0; P(aip4) [20] → A1 else
      A2.

1.14d-confirmed; same as D2MOO. Bug kept: a failed walk with flags 0
in steps 4 and 5.1 leaves only the `aidel` think from the failed mode
start (`ai.md` §1.3), since the fallen deleted its thinks in step 1.

6. Leader test of 5.2 (`0x005F047F`–`0x005F04A8`): D < 15 first, then
   `0x0058F0D0(unit)` (control +0x28 game ≠ 0 → the unit with GUID
   +0x2C, type +0x30) compared by pointer with the fallen itself, and
   only then the P(aip1) draw (`roll(100)` at `0x005F0498`). A fallen
   whose owner data was never set (control +0x28 = 0) and every party
   member (owner = the leader) skip 5.2 **without a draw**. The leader's
   owner data is set by the party code (`population.md` §10.2.1:
   fallen1 has `SetBoss`, so `0x005B28E9` calls `0x0058F030(game,
   leader, leader GUID, 1, 1, BossXfer)` before the count draw), so a
   pack leader always draws aip1 at D < 15.
7. Recorded check (1.14d, `check-combat-fallen-hits-player`; fallen1
   party spawned at frame 30, leader GUID 19, player at D = 2, C set,
   no command, param 0 = 0; Normal aip1 30, aip3 50, aip4 20). Leader
   seed at frame 30 {2409280208, 8913528}. Frame 30 think: 64 ≥ 30
   (5.2 fails), 89 ≥ 50 (aip3 fails), 39 ≥ 30 (pct(30) fails) → idle 10;
   seed {3163442939, 446165621}. Frame 41 think: 76 ≥ 30, 74 ≥ 50,
   27 < 30 → mode 9 (S2) at T; seed {516034227, 1787617634}. Drawing
   without the 5.2 draw (leader not its own owner) gives 64 ≥ 50 →
   89 ≥ 30 → idle 10, then 39 < 50 → param 0 := 0, 76 ≥ 20 → A2 at frame
   41 — the d2rs result of that recording (q-fix-c2-fallen-s2-choice).

#### 9.5 Brute (7) `0x005EFB80`

1. C: P(aip3) [100] → P(aip4) [45] → A1 else A2. Else a second P(aip3):
   pass → circle 4 at T (one more draw, `ai.md` §7.2); fail → idle 15.
2. Not C: speed = 100 − clamp(life%, 40, 100); velocity speed request;
   `0x005DED40` (method 13) walk to T with flags 7.

1.14d-confirmed. D2MOO names aip2 "circle?" but the code tests aip3 both
times.

#### 9.6 FallenShaman (13) `0x005F1440`

1. C and P(aip3) [100] → A1. End.
2. Corpse scan, max squared distance aip4² [576]: non-unique shamans
   (or champions) scan their own minions (scan 9: monsters in dead mode
   12, targetable, no "udead" state mask, whose minion owner is this
   shaman, full-size distance ≤ the max); uniques scan adjacent rooms
   with callback `0x005F1380` (base class fallen1 or fallenshaman1, evil,
   targetable, dead mode, not hidden, squared distance ≤ max, not unique
   or champion). The scan keeps the last match and a count.
3. P(aip1) [45] → command type 1 to all minions (not to itself).
4. If a corpse was found (count > 0): P(aip1) and the skill check
   `0x005FD470(Skill1, corpse)` → sequence skill `Skill1` (Resurrect) on
   the corpse (`0x005DE000`). End.
5. If T and D < aip5 [15]: P(aip2) [60] → use `Skill2` (ShamanFire) in
   `Sk2mode` at T (`0x005DEAD0`). End.
6. Secondary target S (`0x005DDC30`): if S and its distance < aip5:
   P(aip2) → `Skill2` at S. End.
7. P(aip3) → circle 3 at T. Else idle 10.

Draw order: up to 1 (step 1) + 1 (step 3) + 1 (step 4) + 1 (5) + 1 (6)
+ 1 (7) + 1 (circle). The resurrect itself is a skill (skills spec).
1.14d-confirmed.

#### 9.7 QuillRat (14) `0x005F1140`

1. Current command: if the unit (type param 1, GUID param 2) exists →
   A2 at T, free the command, end; else free it.
2. C → A1. End.
3. AI state 3/19 → A2. End.
4. D ≥ aip1 [10] → wander max(aip4, 3) [3] (aip4 as a byte). End.
5. P(aip2) [35] → A2. End.
6. Escape from T by aip4 [2] with think delete; started → end.
7. D > 3 → wander max(aip4, 3); else A2.

1.14d-confirmed. A2 is the quill (missile `spike1`, `MissA2`).

8. Thresholds and timer, quillrat1 Normal: D < 10 (aip1) reaches the
   draw; P(35) (aip2) shoots; a failed draw tries escape 2 (aip4) and
   only then D > 3 wanders 3, D ≤ 3 shoots. Exactly one draw (step 5)
   before the escape; steps 1–4 draw nothing. The think comes back at
   the A2 mode end + `aidel` 15 (`ai.md` §1.3); a started escape or
   wander thinks again at its own end. **What makes it fire again
   without a draw:** step 3, AI state 3 or 19 (`ai.md` §3.1 "AI
   state"): any hit on the rat that starts no get-hit/block mode sets
   19 (`combat/damage.md` §7.1 steps 4.5–4.7), and leaving the
   following non-neutral mode turns 19 into 3, so a rat that was hit
   since its last mode change shoots every think while D < 10 and C is
   clear. Only an unhit rat (state = its last left mode, e.g. 5) draws.
9. Recorded check (1.14d, `check-combat-arrow-quillrat`; quillrat1
   GUID 19 poked at frame 30 at (+4, +4) from the player, D = 4, no
   command, C clear; player arrows (missile 0, skill 0 level 1, no
   damage) poked at frames 38, 50, 62, 74, 86). Rat seed at frame 30
   {21370634, 838424606}.
   - f31 think: state 0, P → 8 < 35 → A2; seed {2409280208, 8913528}.
   - f36 quill created (one owner-seed step) → {4094205064, 1004892389}.
   - f42 arrow 1 reaches the rat (player seed steps; rat hp 1280
     unchanged, mode stays 5): reaction without a mode → AI state 19.
   - f44 A2 ends → mode 1; 19 → 3 (`0x005A68E0`); think at 44 + 15 = 59.
   - f46 the quill reaches the player and is removed; one rat-seed step
     → {1069704589, 1707661690} (`lo' % 100` = 89), player hp unchanged
     (PROVISIONAL REC-826: the drawing call of a monster missile that
     reaches the player without damage is not read here).
   - f54 arrow 2 reaches the rat → AI state 19.
   - f59 think: step 3 (state 19) → A2, **no draw** (seed unchanged
     46–63); f64 quill 2 created → {3163442939, 446165621}; it
     re-fires the same way at f87 (arrow at 74; no AI draw).
   d2rs (same seeds, GUID 17): never stores the AI state (reads 0), so
   f59 reaches step 5 and draws {4094205064, …} → `lo'` 1069704589,
   89 ≥ 35 → escape 2 started → walk to (5149, 4269), mode 2. The
   d2rs quill also flies past the player at f46 without the rat-seed
   step; with that step its f59 draw would be 39 ≥ 35, still an escape,
   so the AI state alone decides the frame-59 mode
   (q-fix-c3-quillrat-choice). 1.14d-confirmed except REC-826.

#### 9.8 CorruptLancer (36) `0x005F5D50`

1. D > aip5 [15]: path step count := monstats2 `MeleeRng`; velocity speed
   100 (method 13, D2MOO); run to T with `MeleeRng` steps; AI param 0 :=
   1. End.
2. C: if param 0 = 0 and P(aip2) [75] fails → idle aip3 [9]. End.
   Param 0 := 0. If `Skill1` ≥ 0 and P(aip6) → `Skill1` in `Sk1mode`;
   else if `Skill2` ≥ 0 and P(aip7) → `Skill2`; else if `Skill3` ≥ 0 and
   P(aip8) → `Skill3`; else A1.
3. Not C: P(aip1) [60] fails → idle aip3. Else path steps := `MeleeRng`;
   P(aip4) [0] → velocity 100, run with `MeleeRng` steps; else walk to T
   with 3 steps.

cr_lancer1 has no skills, so step 2 draws once at most. 1.14d-confirmed.

#### 9.9 Npc (32) `0x005E7130`

Target mode 0 (no T). "Path distance" = `0x005DC5C0` (`ai.md` §6), compared
unsigned. "Walk to (x, y)" = `0x005DED90`; "walk step 0" = `0x005DEF30`
(`ai.md` §7.2). Command params are numbered as in `ai.md` §8 (param 0 = type).

1. **Home** (`0x005E6800`): H = command 10 (`0x0058EFA0(10, 0)`, created
   if absent). If H exists and its params 1 and 2 are both 0: they become
   the unit's position, idle 20, end. This is the first think of every
   Npc-AI unit.
2. **Class cases** (unit class, unit +0x04). The functions called are
   quest seams (`world/quests.md`; D2MOO names in brackets):

   | Class | Rule |
   |---|---|
   | 201 jerhyn | `0x0059F570(game)` [`ACT2Q4_IsJerhynPalaceActivated`] = 0 → idle 40, end. Else `0x0059F580(game, unit, &a, &b)` [`ACT2Q4_GetAndUpdatePalaceNpcState`]: b ≠ 0 → idle 20 (no end); a = 0 → end. `0x0059B6E0(game, unit)` [`ACT2Q4_IsGuardMoving`] ≠ 0 and H found or created → H := (own x + 9, own y), idle 50, end. Otherwise step 3. |
   | 254 alkor | `0x005BAD20(game)` [`ACT3Q4_GoldenBirdBroughtToAlkor`] ≠ 0 → mode 8 at (0, 0) (`0x005DDFC0`), `0x005BAD40(game)` [`ACT3Q4_ResetAlkor`], end. |
   | 255 ormus | `0x005B9CA0(game, &x, &y)` [`ACT3Q3_GetAltarCoordinates`] ≠ 0: path distance to (x, y) > 3 → walk to (x, y), end; else mode 8 at (0, 0), `0x005B9CD0(game)` [`ACT3Q3_SetAltarMode`], end. |
   | 265 cain5 | `0x00594360(game, unit, &x, &y)` [`ACT1Q4_GetCainPortalInTownCoordinates`] ≠ 0: path distance > 2 → walk to (x, y), end; else `0x005945F0(game, unit)` [`ACT1Q4_OnCainInTownActivated`], step 3. |
   | 512 drehya | `0x0058BC80(game, unit)` [`ACT5Q4_AnyaOpenPortal`], step 3. |

3. Interaction handler (below); returns nonzero → end.
4. Command handler (below); nonzero → end.
5. Map-AI handler (below); nonzero → end.
6. Idle 8.

**Interaction `0x005E68F0`** (D2MOO `sub_6FCE5EE0`):

1. P = nearest interacting player (`0x005DDF20`, `ai.md` §5.3; the NPC itself
   when none); d = full-size distance NPC → P (`ai.md` §6).
2. Return 0 if the unit is not a monster or has no interaction block
   (monster data +0x30; `world/npc.md` §2).
3. busy := P is in the NPC's interaction list (`0x00572DE0`), or P is a
   player and `0x00535060(P)` = 1. talking := the list is not empty
   (`0x00572DC0`).
4. talking, busy, or AI param 0 > 0: if the path distance to (param 1,
   param 2) > 2 and param 0 > 36 → walk to (param 1, param 2); else if
   param 0 > 0 → stop the path (`0x00648730`, when there is one) and idle
   8. Then param 0 := 0 if it is negative, else param 0 − 1. Return 1,
   also when nothing was scheduled.
5. P = 0 or P = the NPC → return 0.
6. d < 3 or d > 23: stop the path. If param 1 = 0: param 1 := 60; if P
   is a player, sound 18 on the NPC toward P (`0x00553380(NPC, 18, P)`),
   idle 20, return 1. If param 1 ≠ 0: negative → 0, positive → minus 1.
   Idle 20, return 1.
7. 3 ≤ d ≤ 23: home check `0x005E6860(16)`: H = `0x0058EEF0(10, 0)`;
   none → return the result of main step 1; path distance to H > 16 →
   command 4 (found or created) := (H.x, H.y, 12, 10), idle 10, return 1;
   else 0. If the home check returned 0: velocity request (method 1 → 7,
   speed 0, steps 0), walk in radius of P (`0x005DE6D0`) with (d − 2, 2)
   when d < 5, else (3, 2). Return 1.

AI params 0–2 come from NPC messages: `0x00548B00` (interaction start,
`world/npc.md` §2) sets param 0 := 40; `0x0054CA10` (C→S 0x59
MakeEntityMove, `sim/client-messages.tsv`: unit type, GUID, x, y) sets
param 0 := 40 and params 1, 2 := x, y; both stop the path and schedule a think at +1 (`ai.md` §1.2). Param 1 is
also the greeting countdown of step 6 (one field, two uses: kept).
Effect of param 0 := 40 (every C→S 0x13 within distance 50,
`0x00548D4A`): the next 40 thinks take step 4; while param 0 > 36 (the
first 4) an NPC more than 2 from (param 1, param 2) walks there
(params 1 and 2 are set only by 0x59; a 0x13 leaves them as they are);
otherwise it stops its path and idles 8. So a click holds the NPC in place for about 40 thinks
of 8 frames, and each further 0x13 restarts the count.

**Commands `0x005E6AE0`** (D2MOO `sub_6FCE69A0`). Commands are found
with `0x0058EEF0(type, 0)`. G = the u32 at `0x0088CADC` (zero-initialised
`.data`, read and written only here; process-wide, not per game).

1. Command 4 (walk to: x, y, tries n, delay t) with n > 0: if x ≠ 0,
   y ≠ 0 and the path distance to (x, y) > 3 → velocity request with
   method 5 when G's low two bits are 0, else method 1 (→ 7), speed 0,
   steps 0; walk step 0 to (x, y); G += 1; n −= 1; return 1. Otherwise
   idle t, n −= 1, return 1.
2. Command 5 (wander: count c, distance w, idles k, delay t):
   - c > 0 and odd: wander w (`0x005DE200`, w as a byte); c −= 1; return
     1.
   - c > 0 and even: k > 0 → idle t, c −= 1, k −= 1, return 1; else
     c −= 1 and go to step 3.
   - c ≤ 0: k > 0 → idle t, k −= 1, return 1; else step 3.
3. Command 7 (mode action: mode m, x, y, tries n), present with m ≠ 0;
   otherwise return 0:
   1. m ∉ {8, 9, 10, 11} → m := 0, idle 50, return 1.
   2. e = path distance to (x, y) (signed). e > 0: if n > 0 → velocity
      request (7, 0, 0), walk step 0 to (x, y), idle 25 if that failed,
      n −= 1, return 1; else if e > 1 → m := 0.
   3. Facing (`0x00648820(path, dir)`, path spec): class 154 charsi → 56;
      155 warriv1 → 52; 178 fara → 4; 405 jamella → 52 if m = 8, 48 if
      m = 9. Class 511 larzuk: `roll(100)` > 4 → m := 0, idle 50,
      return 1.
   4. m ≠ 0: the unit's mode (unit +0x10) = m → idle 50, return 1; else
      mode m with the unit itself as target (`0x005DDF90`), m := 0,
      return 1.
   5. m = 0: class 178 fara and `roll(100)` < 66 → m := 8. Return 0.

**Map AI `0x005E7080`** (D2MOO `AITHINK_ExecuteMapAiAction`):

1. M = control +0x38 (`ai.md` §3.1: node count u32, pointer to 12-byte nodes
   {action, x, y}). None → return 0, no draw.
2. Draw `lo' % 100`; ≥ 66 → return 0.
3. Count 0 → return 0. i = `roll(count)` (`0x0045C3E0`); action a of
   node i; a outside 0..6 or its entry in table `0x00741D74` (7 pointers:
   0, `0x005E6DE0`, `0x005E6E80`, `0x005E6EF0`, `0x005E6F00`,
   `0x005E6FC0`, 0) is 0 → return 0. Else return the handler's result:

| Action | Handler | Effect |
|---|---|---|
| 1, 3 | `0x005E6DE0` (3 jumps to it) | x or y = 0 → 0; path distance to (x, y) = 0 → 0; else velocity request (7, 0, 0), walk step 0 to (x, y) (result ignored), command 4 := (x, y, 12, 10), return 1 |
| 2 | `0x005E6E80` | x or y = 0 → 0; r = action 1; command 4 := (x, y, 20, 10) also when r = 0; return r |
| 4, 5 | `0x005E6F00`, `0x005E6FC0` | x or y = 0 → 0; r = action 1; command 7 := (m, x, y, 4) with m = 8 (action 4) or 9 (action 5) if the class has that mode (`0x0046C140(class, m)`), else 1; command 4 := (x, y, 12, 10); return r |

Draws per think, in order: map-AI `lo' % 100` and `roll(count)`;
command 5 wander (3–4); larzuk and fara `roll(100)`. Steps 1, 2,
the interaction handler and command 4 draw nothing.

Rhythm (settles question 8): a map-AI walk leaves command 4 with 12
tries and delay 10; the walk ends inline (`ai.md` §1.4), and from then on each
think within 3 of the node is "idle 10" until the tries run out. So
idle 10 is command 4's delay, and idle 8 is the fallback of step 6.
**Path node choice, summary** (2026-10-09, re-read `0x005E7080`,
`0x0045C3E0`, `0x005E6DE0`; the questions of the Act I arrival
compare, `docs/handoff/q-scenes-compare.md`):

1. *Order.* None. A think that reaches the map AI (steps 3 and 4
   returned 0) draws the unit seed (+0x20) once, `lo' % 100`
   (unsigned); ≥ 66 → no node, step 6 idle 8. Else a second draw picks
   the node uniformly, i = `roll(count)`: count a power of two → `lo' &
   (count − 1)`, else `lo' % count`; **count 1 still draws**. The same
   node can come again; there is no next / previous index, no wrap and
   no reverse, and nothing remembers the last node.
2. *Walk target.* The node's own (x, y) (DS1 path point plus the preset
   offset, `ai.md` §3.1, `drlg/preset.md` §7), not a point near it. At
   the node already (path distance 0) the handler returns 0 → idle 8.
3. *Wait.* After a walk starts, command 4 holds (x, y, 12, 10) (20
   tries for action 2): each later think while it has tries walks again
   when farther than 3, else idles 10; the map AI is not reached until
   the tries run out. Then every think: 34 % idle 8, 66 % a new random
   node.
4. *Draws per think.* Map AI: 2 (1 when the first is ≥ 66; 0 when the
   unit has no nodes). Command 4 and the walk itself draw nothing here.
5. *Precedence.* The interaction handler (step 3) runs first: while it
   takes a player (`ai.md` §5.3 scan 2: within 15, and for `interact`
   NPCs only when the quest active test `world/quests.md` §6.4 holds),
   a player 3–23 away gives walk in radius (step 7) and the map AI is
   not reached. A path-node walk therefore means no player was taken
   that think.

d2rs (`monsters/ai/npc.rs` `npc_map_ai`, `rng.rs` `roll`) follows rules
1–4. Recorded 2026-10-09 (Windows, SceSor, `-seed 1234`, 120 ticks):
at the Act I arrival Warriv (1:7) takes no map-AI path at all. His
unit seed draws once (frame 2, `0x00573F8F`, `roll(1)`), the active
test passes for the player at every think (S→C `8a 01 07000000` at
frames 24, 45, 56, 67, 87, 107; `world/quests.md` §6.4), and his walks
are step 7's walk in radius: (4866, 4235) → (4868, 4233) at frame 24,
→ (4869, 4232) at 45, → (4870, 4231) at 56, stop at 67. A d2rs
state-dump of the same save (release build of 2026-10-09) equals the
1.14d state recording for Warriv in mode, position, sub-tile fraction
and target on every frame 22–47, and has the same four walk / stop
frames to 120. So the scene note "d2rs's `wa` stands on (4866, 4235)
at r110" (`docs/handoff/q-scenes-compare.md`, scene tick 42) came from
an older build.

1.14d-confirmed (all functions above); D2MOO differs: it tests the
interaction block the other way round (returns 0 when one exists) and
reconstructs G as a local that is always 0 (so always method 5).

#### 9.10 Navi (58) `0x005E7E20`, TownRogue (62) `0x005E7D60`

Navi (Flavie, target mode 0):

1. Interacting (`0x00572DC0`) → idle 10. End.
2. P = nearest player within 15 (`0x005DDF20`). If P is a player, not
   busy (`0x00535060`), `roll(3)` ≠ 0, and P is closer than 4: AI param 1
   = 0 → param 1 := 60, sound 18 to P, idle 20; else clamp param 1 at 0
   and count it down, idle 20. End. (`roll(3)` is drawn only when P is a
   non-busy player.) "Clamp and count down", exactly: param 1 < 0 →
   param 1 := 0 (and stays 0); param 1 > 0 → param 1 − 1
   (1.14d-confirmed, `0x005E7ED4`–`0x005E7EE7`).
3. S = secondary target (`0x005DDC30`); S and distance < 25 → A1 at S
   (arrow `rogue1`). Else idle 50.

TownRogue: step 3 only. 1.14d-confirmed.

#### 9.11 Skeleton (2), Wraith (9), Goatman (12), Swarm (19)

One pattern, 1.14d-confirmed (`0x005EFCF0`, `0x005F0A20`, `0x005F12A0`,
`0x005F2460`; 12 and 19 are byte-identical copies):

| AI | C | Not C | Fallback |
|---|---|---|---|
| Skeleton | P(aip3) → P(aip4) A1 / A2 | P(aip1) → walk to T flags 7 | idle aip2 |
| Wraith | P(aip3) → A1 | P(aip1) → walk in radius (12, 0) of T | idle aip2 |
| Goatman, Swarm | P(aip3) → A1 | P(aip1) → walk to T flags 7 | idle aip2 |

#### 9.12 Andariel (34) `0x005F5830`

1. C: if `Skill1` ≥ 0 and P(aip1) → `Skill1` in `Sk1mode` at T; else
   A1. End.
2. P(aip2) → idle 5. End.
3. P(aip3): if `Skill1` ≥ 0 and P(aip4) → `Skill1`, end; else if
   `Skill2` ≥ 0 → `Skill2` in `Sk2mode`, end.
4. Velocity request (method 1 → 7); walk to T, flags 7.

1.14d-confirmed.

#### 9.13 CorruptArcher (35) `0x005F5A20`

S = secondary target (`0x005DDC30`) with distance E.

1. No S: draw `lo' % 100` > 49 → idle aip3; else circle 3 at T (T may
   be 0). End.
2. Not C and AI state 3/19 → A1 at S. End.
3. E < 6 and P(aip4): speed 100; run away from S by 12 with think
   delete; started → end.
4. 0 < aip8 < E and P(aip1): speed 10; walk to S with aip8 steps. End.
5. E > aip5: speed 100; run to S with aip5 steps. End.
6. P(aip2) fails → idle aip3. End.
7. `Skill2` ≥ 0 and P(aip6) → `Skill2`; `Skill3` ≥ 0 and P(aip7) →
   `Skill3`; `Skill1` < 0 → A1 at S; else `Skill1` in `Sk1mode` at S.

1.14d-confirmed (same as D2MOO).

#### 9.14 Other Act 1 AIs (D2MOO-only)

None left: the Act 1 AIs that were D2MOO-only are read in 1.14d in
§9.15–§9.18 (CorruptRogue, SkeletonBow, FoulCrowNest, BloodRaven) and
§9.23–§9.24 (Bighead, BloodHawk).

#### 9.15 CorruptRogue (10) `0x005F0B00`

1. Player-count record of the unit (`0x00573930` with the unit's room,
   `monsters/init.md` §9; no draws). L = 20 − 3 × its difficulty field
   (game +0x6D, which that call clamps to 2): 20 / 17 / 14.
2. D > L: run (below). End.
3. C: P(aip3) [75] → A1; else idle aip2 [15]. End.
4. Not C: P(aip1) [60] fails → idle aip2. End. P(aip5) [20] → run;
   else walk to T with flags 7 (`0x005DEC80`).

Run: velocity request (method 13, speed aip4 [100], steps 0); run to
T with 3 steps (`0x005DEFB0`). Draws: 0 (step 2), 1 (step 3) or 1–2
(step 4). 1.14d-confirmed; same as D2MOO.

#### 9.16 SkeletonBow (37) `0x005F6070`

S, E = secondary target and its distance (`0x005DDC30`, `ai.md` §5.3, second
argument 0).

1. AI state 3/19: S, E := `0x005DDC30`; S → A1 at S, end.
2. S, E := `0x005DDC30` (a second search when step 1 found nothing).
3. No S, or E ≥ 20: P(aip3) [50] → walk in radius of T (`0x005DE6D0`,
   a = aip4 [5], b = aip5 [6]); else idle 20. End.
4. P(aip1) [75] → A1 at S. Else draw `lo' % 100` < 20 → circle 3 at T
   (`0x005DF7D0`, no think delete; one more draw); else idle aip2 [15].

1.14d-confirmed; same as D2MOO. The walk and the circle use T, the
arrow uses S.

#### 9.17 FoulCrowNest (43) `0x005F6650`, init `0x005F6630`

Init (also of Sarcophagus 45 and MinionSpawner 121): AI param 0 := the
game frame, param 1 := 0.

1. D > 20 → idle 25. End.
2. Param 1 ≥ aip3 [6] (the nest has spawned its quota): set unit flag
   0x20000 (unit +0xC4: no drop, `items/treasure.md`), request mode 0
   (death) at (0, 0) (`0x005DDFC0`). End.
3. If `Skill1` ≥ 0 and |frame − param 0| ≥ aip1 [100]: param 0 :=
   frame; footprint test `0x005FD350(class, room, x, y, 0)` with class
   206 (crownest1; −1 if monstats has ≤ 206 rows), the unit's room and
   position (`monsters/population.md` §9: tests (x, y + 3)). Passes →
   param 1 += 1, use `Skill1` in `Sk1mode` at T (`0x005DEAD0`), end.
4. Idle `lo' % 10` + 20 (one step). End.

So the nest summons (skill `Nest`, skills spec) at most every aip1
frames, idles 20–29 between tries, and dies after aip3 summons.
1.14d-confirmed. D2MOO passes x twice to the footprint test; 1.14d
passes x, y.

#### 9.18 BloodRaven (59) `0x005E6320`, init `0x005E6300`

Init: AI param 2 := 0. Params: 0 = raise chance, 1 = raises done, 2 =
"returning home". Distances to the home point use `0x005DC480` (`ai.md` §6).

1. H = command 10 (`0x0058EEF0(10, 0)`). None: copy a new command (type
   10, params 1, 2 := own position; params 3, 4 are uninitialised stack
   in 1.14d and unused) and look it up again.
2. h := 0. If H:
   1. h = distance from T to H.
   2. D > 45 → idle 5. End.
   3. h ≥ 50, or own distance to H > 50: param 2 := 1; velocity request
      (7, 100, 0); run to H (`0x005DEDE0`); started → end; else delete
      thinks.
   4. Param 2 ≠ 0 and own distance to H > 5: velocity (7, 100, 0); run to
      H; started → end; else delete thinks.
   5. Param 2 := 0.
3. D > 20 and h < 50: D := max(D / 2, 12) (the halved value is kept for
   the steps below); velocity (7, 100, 0); run near T by D
   (`0x005DF680`, D as a byte; its draws); started → end; else delete
   thinks.
4. Param 0 += 3. If `Skill1` ≥ 0, not C, param 1 < 2 × difficulty + 8
   (game +0x6D) and `roll(100)` < param 0 (drawn only when the first
   three hold):
   1. L = `roll(15)` + 5; draw bit 0: 1 → dx = L, dy = `roll(L)`; 0 →
      dx = `roll(L)`, dy = L; draw bit 0 → dx := −dx; draw bit 0 → dy
      := −dy (bits by `0x00472210(seed, 2)`, one step each).
   2. Use `Skill1` in `Sk1mode` with no target at (T.x + dx, T.y + dy)
      (`0x005DEAD0`); param 1 += 1; param 0 := 0. End.
5. D > 5:
   1. Draw `lo' % 100` < 5 and h < 50 → velocity (7, 100, 0); run near
      T by 12; end (whether or not it started).
   2. S = secondary target (`0x005DDC30`). If S, AI state not 3/19, and
      `roll(100)` < 80: if `Skill2` ≥ 0 and `roll(100)` < 10 ×
      (difficulty + 4) → `Skill2` in `Sk2mode` at S; else A1 at S. End.
   3. Velocity request (method unchanged, speed 50, steps 0); circle 4
      at T with think delete (`0x005DF7D0`); started → end.
6. Draw `lo' % 100` < 30 and D < 12: velocity (7, 100, 0); run away from
   T by 12 − D (`0x005DF140`, a byte, no think delete); started → end;
   else delete thinks.
7. A1 at T.

Live skills: `Skill1` = Nest (`Sk1mode` `seq_bloodravencast`; the
summon at the point is the skills spec's), `Skill2` = Quick Strike.
"Delete thinks" = `0x00540E60(2, 0)`: it also removes the `aidel` think
that the failed mode start scheduled (`ai.md` §1.3); the think then goes on to
a later step, which always schedules or starts a mode. 1.14d-confirmed; same as D2MOO except that 1.14d writes the
secondary search's melee flag over h in step 5.2 (h is not read
afterwards).

#### 9.19 SkeletonMage (64) `0x005F96C0`

Brackets: skmage_fire1 Normal [aip1..aip8 = 35, 9, 30, 5, 0, 18, 20, 5].
S, E := secondary target and distance (`0x005DDC30`, second argument 0;
E = 0x7FFFFFFF when there is no S). `Skill1` (SkeletonRaise) is not used
here.

1. If S:
   1. E > aip2 [9] and P(aip3) [30] → velocity request (method
      unchanged, speed 10, steps 0); walk to S with aip2 steps (a byte,
      `0x005DEF80`). End.
   2. E ≤ aip4 [5] and P(aip5) [0] → velocity (speed 25); escape from S
      by 5 with think delete (`0x005DEFE0`); not started → A1 at T. End.
   3. E < aip6 [18] and P(aip1) [35] → A1 at S. End.
2. E > aip2 and P(aip3) → velocity (speed 10); walk to T with aip2
   steps. End. (With no S, E is 0x7FFFFFFF and this test always draws.)
3. P(aip7) [20] → circle 4 at T (no think delete; one more draw); else
   idle aip8 [5].

Each P(…) is drawn only when its distance test holds. 1.14d-confirmed;
same as D2MOO.

#### 9.20 Arach (26) `0x005F4510`

Brackets: arach1 Normal [45, 33, 15, 8, 25]; `Skill1` SpiderLay in
`Sk1mode` A2. AI params: 0 = state (0 idle, 1 retreating, 2 engaged),
1 = approach latch, 2 = think counter. L = the unit's life percent
(`0x00621F20`). "Lunge" = `0x005DED40(T, 0)` (velocity method 13, then
walk to T, flags 0).

1. State 1:
   1. Param 1 := 0.
   2. L > 75: state := 0; P(aip3) [15] → state := 2, lunge; else circle
      6 at T (no delete). End.
   3. C and aip1 > 25 and `roll(100)` < aip1 − 25 → A1 at T. End. (No
      draw unless C and aip1 > 25.)
   4. D ≥ aip4 [8] and AI state not 3/19 → state := 0; circle 12 at T
      (no delete). End.
   5. Escape from T by 4 (no delete). End.
2. State ≠ 1, C:
   1. State := 2. P(aip1) [45] → A1 at T. End.
   2. L < aip5 [25]: state := 1; if `Skill1` ≥ 0 and the unit lacks
      state 22 (`0x00639DF0`): use `Skill1` in `Sk1mode`, no target, at
      (0, 0) (`0x005DEAD0`); else escape from T by 8 (no delete). End.
   3. `roll(100)` < aip2 [33] → circle 4 at T (no delete); else idle 15.
3. State ≠ 1, not C:
   1. AI state 3/19 or param 1 = 1 → param 1 := 1, lunge. End.
   2. Param 2 += 1, wrapping to 0 when it passes 20; param 1 := 0.
   3. Param 2 = 1 and `roll(100)` < aip3 → param 1 := 1, lunge. End.
   4. `roll(100)` < 20 → wander 6; else idle 15.

So an idle spider rolls its engage chance once every 21 thinks.
1.14d-confirmed.

#### 9.21 Fetish (30) `0x005F53E0`

Brackets: fetish1 Normal [100, 10, 4, 33]. AI params: 0 = state (0, 1
attacking, 2 backing off), 1 = counter. L = life percent of **T**
(`0x00621F20(T)`).

1. Current command K (`0x0058EE80`): if K's type is 1 or 14 and the unit
   it names exists (`0x00552F60`, type param 2, GUID param 1): params 0,
   1 := 0; velocity request (13, 50, 0); walk to that unit (flags 0,
   `0x005DEC80`); free K. End. Any other K: free it and go on.
2. State 0: C → param 1 := 0, state := 1, P(aip1) [100] → A1 at T, else
   idle aip2 [10]; end. Not C → step 5.
3. State 1: param 1 += 1. If param 1 > aip3 [4] and L > aip4 [33]:
   state := 2, param 1 := 0, velocity (method 2, speed 50, steps 0),
   escape from T by 14 with think delete; end (started or not). Else C →
   `roll(100)` < aip1 → A1 at T, else idle aip2; end. Not C → step 5.
4. State 2: D > 12: param 1 += 1, reset state and param 1 to 0 when
   param 1 > 1; draw `lo' % 100` < 20 → circle 4 at T (no delete), else
   idle 10; end. D ≤ 12: velocity (2, 50, 0); escape from T by 14 with
   think delete; started → end; else state, param 1 := 0, idle 10; end.
   Any other state value: idle 10.
5. Velocity (13, 50, 0); walk to T with flags 7. End.

1.14d-confirmed. The minion's attack-then-back-off rhythm: aip3 attack
thinks, then back off while the target is above aip4 % life.

#### 9.22 Vampire (28) `0x005F4A70`

Brackets: vampire5 Normal [85, 40, 28, 25, 1]. F = aip5 as spell flags:
F1 (bit 0) Skill1 / Skill4 bolts, F2 (bit 1) Skill2, F4 (bit 2) Skill3.
AI params: 0 = state (0, 1 engaged, 2 fleeing), 1 = farthest D seen
under 30 while in AI state 3/19, 2 = spell cooldown. L = own life
percent. S, E as in §9.19. "Bolt at X" = `roll(100)` < 50 → `Skill1` in
`Sk1mode` at X, else `Skill4` in `Sk4mode` (byte +0x183) at X; neither
skill id is tested for < 0. "Upgrade" = F2, param 2 ≤ 0 and
`roll(100)` < aip4 → `Skill2` at T, param 2 := 11, end; else F4, param 2
≤ 0 and `roll(100)` < aip4 → `Skill3` at T, param 2 := 11, end (each
roll drawn only when its flag and cooldown hold).

1. Param 2 > 0 → param 2 −= 1. S, E := `0x005DDC30`.
2. AI state 3/19: state 0 → 1; D < 30 and D > param 1 → param 1 := D.
   If C: draw `lo' % 100`; > 30 or no F1 → A1 at T; else bolt at T. End.
3. State 2:
   1. L ≥ 75 → state := 1, walk to T flags 7. End.
   2. D < 14 or D ≤ param 1: velocity request (method unchanged, speed
      v, steps 0) with v = `Run` × 100 / `Velocity` − 100 (monstats +52,
      +50, signed, truncating; 0 when `Velocity` ≤ 0), clamped to 0..120;
      escape from T by 8 with think delete; started → end.
   3. D ≥ aip3 [28] → idle 15. End. `roll(100)` ≥ aip2 [40] → idle 15.
      End.
   4. Upgrade.
   5. F1, S and E ≤ 20 → bolt at S; else circle 4 at T (no delete). End.
4. State ≠ 2:
   1. L < 33: state := 2; escape from T by 8 (no delete, no velocity);
      started → end.
   2. C: state := 1. P(aip1) [85]: no F1 or no S → A1 at T, end;
      `roll(100)` > 30 → A1 at T, end; E ≤ 20 → bolt at S, end. Then
      (P(aip1) failed, or E > 20): `roll(100)` < 33 → circle 4 at T
      (no delete), else idle 10. End.
   3. Not C, D ≥ aip3: state 1 → walk to T flags 7; else idle 15. End.
   4. Not C: state := 1. `roll(100)` ≥ aip2: D > 20 → walk to T flags 7;
      else D < 9 and `roll(100)` < 50 → escape from T by 8 (no delete);
      else `roll(100)` < 50 → circle 4 at T, else idle 10. End.
   5. Upgrade.
   6. No F1, no S, or E > 20 → walk to T flags 7. Else `roll(100)` < 75
      → bolt at S; else circle 4 at T. End.

1.14d-confirmed; same as D2MOO.

#### 9.23 Bighead (4) `0x005EFF50`

Brackets: bighead1 Normal [88, 40, 0, 60]. A2 is the ranged attack. L =
own life percent (`0x005DD280`, `ai.md` §6). S = secondary target
(`0x005DDC30`), searched only where stated; its distance overwrites D
(not read afterwards).

1. Not C and AI state 3/19 → A2 at T. End.
2. L ≥ aip1 [88] (healthy):
   1. C → A1 at T. End.
   2. D < 15: search S; S and `roll(100)` < aip3 [0] → A2 at T. End.
   3. Walk to T with flags 7. End.
3. L < aip1 (hurt):
   1. D < 3: velocity request (method unchanged, speed 50, steps 0);
      escape from T by 5 with think delete; not started → A2 at T. End.
   2. D > 15: walk to T with 6 steps (`0x005DEF80`). End.
   3. Search S; S and `roll(100)` < aip4 [60] → A2 at T. End.
   4. `roll(100)` < aip2 [40] → circle 3 at T (no delete); else idle 10.

A2 always targets T, also when S decided it. 1.14d-confirmed.

#### 9.24 BloodHawk (5) `0x005F00E0`

Brackets: foulcrow1 Normal [30, 90, 5, 50, 100]. AI param 0 = "charged
last think".

1. Param 0 = 1 and C → param 0 := 0, A1 at T. End.
2. Param 0 := 0.
3. C: P(aip3) [5] → A1 at T, end; else back off (step 5).
4. Not C:
   1. P(aip1) [30] → velocity request (method unchanged, speed aip5
      [100], steps D, capped by `ai.md` §7.3); param 0 := 1; walk to T (flags 0).
      End.
   2. D > 3: P(aip2) [90] → velocity (speed −50) and wander 4; else
      velocity (0, 0, 0) (writes nothing, `ai.md` §7.3) and wander 3. End.
   3. D ≤ 3: back off.
5. Back off: velocity (speed aip4 [50], steps 0); escape from T by 4
   with think delete; not started → A1 at T.

1.14d-confirmed; same as D2MOO.

#### 9.25 HellMeteor (33) `0x005F56D0`

Brackets: hellmeteor Normal [50, 50, 10]; `Skill1` HellMeteor in A1.

1. `Skill1` ≥ 0 and P(aip1) [50]: x = own x − aip3 + `roll(2·aip3)`,
   then y = own y − aip3 + `roll(2·aip3)` (`0x0045C3E0`, two draws, x
   first); use `Skill1` in `Sk1mode`, no target, at (x, y). End.
2. Idle aip2 [50] (also when `Skill1` < 0, without a draw).

1.14d-confirmed.

#### 9.26 SandRaider (8) `0x005F0700`

Brackets: sandraider1 Normal [40, 70, 75, 70, 18, 0, 50]; `Skill1` Fire
Hit. AI params: 0 = charge counter, 1 = charged, 2 = help searches.
States 90 (blue) and 91 (red) and overlays 150 (sricehit) and 46
(srfirehit) by aip6: 1 → blue / sricehit, else red / srfirehit.

1. Param 0 = 0: clear states 90 and 91 (`0x00639DB0(unit, s, 0)`,
   `sim/stat-lists.md` §9.2); param 1 := 0.
2. Param 0 += 1. Param 0 = aip5 [18]: start the overlay
   (`0x00621E40(unit, overlay, 0)`, stat 178 `unit_dooverlay`); idle
   `aidel` + 1 (monstats +79 + difficulty, a byte read with no game-type
   gate). End.
3. Param 0 > aip5: set the state (on); param 1 := 1.
4. Param 2 < 7 and own life percent < aip1 [40]: scan 1 (`ai.md` §5.4, every
   unit of the adjacent rooms) for the nearest other monster with
   alignment 0 (`0x006259B0`) that is not in mode 0 or 12
   (`0x0063EA40`), by squared distance (`0x005B0BD0`, strictly smaller
   wins). Found → walk to it (flags 0), end. Else param 2 += 1.
5. D > 4, param 1 = 0 and P(aip2) [70] → circle 0 at T (no delete).
   End.
6. Not C: param 1 = 0 and P(aip4) [70] fails → rest (step 8); else walk
   to T (flags 0). End.
7. C: param 1 = 1 and `Skill1` ≥ 0 → `Skill1` in `Sk1mode` at T, params
   0, 1 := 0, end. P(aip3) [75] → `roll(100)` < aip7 [50] → A2, else A1,
   at T; end. Else rest.
8. Rest: k = max(24 − aip5, 6); param 0 > aip5 + k → params 0, 1 := 0.
   Idle 15.

So a raider charges for aip5 thinks, glows, hits once with `Skill1`,
and the counter resets after a further k thinks without a hit.
1.14d-confirmed.

#### 9.27 Baboon (11) `0x005F0CD0`

Brackets: baboon1 Normal [33, 20, 55, 0, 1]. AI params: 0 = regen
countdown, 1 = "attacked", 2 = hpregen bonus added. L = own life
percent. v = `Run` × 100 / `Velocity` (signed, truncating; monstats
+52, +50) − 100, clamped to 0..120, and 0 when `Velocity` ≤ 0 or the
quotient < 100. "Stat 74" is `hpregen`, read with `0x00625480(unit, 74,
0)` and written with `0x00627260(unit, 74, value, 0)`.

1. Param 0 ≠ 0 (regenerating):
   1. Param 0 −= 1; param 1 := 0. Param 0 = 0 or L > 75 → stat 74 :=
      stat 74 − param 2 (param 2 is kept).
   2. Not C: L > 75 → param 0 := 0, velocity request (13, v, 0), lunge
      (`0x005DED40(T, 7)`), end; else step 4.
   3. C: draw `lo' % 100` < 33 → draw `lo' % 100` < aip4 [0] → A1, else
      A2, at T; end. Else step 4.
   4. D ≥ 24 and AI state not 3/19: `roll(100)` < 33 → circle 4 at T
      (no delete); then idle 20 in either case (the idle's neutral mode
      request ends the circle walk at once). End.
   5. Velocity (2, v, 0); escape from T by 15 with think delete; started
      → end. Not C → wander 5; C → `roll(100)` < aip4 → A1, else A2.
2. Param 0 = 0:
   1. Not C → lunge (`0x005DED40(T, 7)`, no velocity request). End.
   2. C from here. AI state 3/19: if L < aip1 [33] and `roll(100)` < 50: param 0 :=
      `roll(5)` + 2; R = stat 74; R ≠ 0 → param 2 := aip5 × R / 8
      (signed, rounding toward 0) and stat 74 := R + param 2; R = 0 →
      param 2 := 0. Velocity (2, v, 0); escape from T by 15 (no delete).
      End. Else if param 1 ≠ 0 and `roll(100)` < 20: circle 3 at T (no
      delete), param 1 := 0, end.
   3. Param 1 ≠ 0 and P(aip3) [55] fails: `roll(100)` < aip2 [20] →
      circle 3 at T (no delete) and param 1 := 0; then idle 15. End.
   4. Param 1 := 1; P(aip4) → A1, else A2, at T.

1.14d-confirmed. Bug kept: step 1.4 starts a circle and then idles in
neutral mode over it.

#### 9.28 SandMaggot (15) `0x005F1800`, alternate `0x005F1750`

Target mode 4 (`ai.md` §2.3). Brackets: sandmaggot1 Normal [35, 35, 2, 75,
120]; `Skill1` MagottUp, `Skill2` MagottDown, `Skill3` MagottLay. AI
params: 0 = state (0–2 above ground; 1 just surfaced or laid, 2 circled;
3 burrowed), 1 = frame before which it does not burrow or surface
again, 2 = eggs laid. "Wait N" = `0x005DE130(N)` (`ai.md` §1.2). S, E as in
§9.19. "Burrow (X)" = `Skill2` in `Sk2mode` at X with (0, 0); wait 30;
param 1 := frame + aip5; state := 3.

1. State < 3, T = 0: if (no S or E > 10), frame > param 1 and `Skill2` ≥
   0 → burrow (no target), end. Otherwise go to step 3.
2. State = 3: unless T ≠ 0, or S with E < 16: wait 20, end. If frame >
   param 1 and `Skill1` ≥ 0: `Skill1` in `Sk1mode` at T with (0, 0);
   wait 25; param 1 := frame + aip5; state := 1; end. Else wait 20, end.
3. Above ground:
   1. Own life percent < 25, `Skill2` ≥ 0, E < 7, frame > param 1 and
      `roll(100)` < 20 → burrow (target T). End.
   2. C and P(aip4) [75] → A1 at T. End.
   3. S, E < 15 and `roll(100)` < aip2 [35] → A2 at S. End.
   4. Draw `lo' % 100` < 20 → circle 6 at T (no delete). End.
   5. Param 2 < aip3 [2] and `roll(100)` < aip1 [35]: state = 2 and
      `Skill3` ≥ 0 → param 2 += 1, state := 1, `Skill3` in `Sk3mode` at
      T, wait 20; else circle 6 at T (no delete), state := 2. End.
   6. Wait 12.

A state above 3 (never written by this function; only a value left in
param 0 by another AI before a re-install, `ai.md` §3.3, could give one) takes
the above-ground steps of step 3 whatever T and S are: steps 1 and 2
only branch on state < 3 and state = 3. 1.14d-confirmed (`0x005F1800`).

Alternate `0x005F1750` (the think while the AI was re-installed over a
running one, `ai.md` §3.3): K = command 14 (`0x0058EFA0(14, 0)`). K's param 4 =
1 and `Skill1` ≥ 0 → `Skill1` in `Sk1mode` at the unit's path target
(`0x00553540`; 0 when that is the unit itself) with (0, 0), wait 30, K's
param 4 := 0. Otherwise re-install the AI for the control's current
state (`0x005B0E00`) keeping AI param 2 across it, then wait 1. No
draws. 1.14d-confirmed (both).

#### 9.29 Scarab (20) `0x005F2540`

Brackets: scarab1 Normal [75, 50, 15, 35, 20]; `Skill1` Jab. AI param
0 = "circled".

1. Current command K (`0x0058EE80`). None: if D < 20, the scarab is its
   own minion owner (`0x0058F0D0`) and P(aip5) [20]: a command of type
   1 (params 1–4 uninitialised stack, unused) goes to all minions
   (`0x0058F730`) and to itself (`0x0058EF40`); K := it. Else step 3.
2. K of type 1: C and `Skill1` ≥ 0 → free the current command,
   `Skill1` in `Sk1mode` at T, end. Else velocity request (2, 100, 0);
   walk to T (flags 0); not started → free the current command. End.
   K of another type: step 3 (K stays).
3. Not C: param 0 ≠ 0 → velocity (2, 0, 4), walk to T with flags 7,
   then draw `lo' % 100` > 10 → param 0 := 0; end. Param 0 = 0 → circle
   0 at T (no delete), param 0 := 1; end.
4. C: P(aip1) [75] fails → idle aip3 [15]. End. `Skill1` ≥ 0 and
   P(aip4) [35] → `Skill1` at T. End. P(aip2) [50] → A1, else A2, at T.

1.14d-confirmed.

#### 9.30 Smith (98) `0x005E3890`, Griswold (90) `0x005E5AC0`

Smith (the Smith, hephasto), no draws:

1. C → A1 at T. End.
2. L = own life percent clamped to 0..100; velocity request (method
   unchanged, speed (100 − L) >> 1, steps 0); walk to T with flags 7.

Griswold:

1. C: draw `lo' % 100` < 80 → A1 at T; else idle 10. End.
2. Not C: draw `lo' % 100` < 50 → walk to T with flags 7; else idle 10.

1.14d-confirmed.

#### 9.31 GoodNpcRanged (60) `0x005E7AC0`

Also the think of special state 5 (`ai.md` §3.2).

1. Anim mode (unit +0x10) not neutral (1) → idle 5. End.
2. Unless the unit's room is in town (`0x0061AB00`): S, E := secondary
   target (`0x005DDC30`). If S and E < 20: `roll(100)` < 30 → for class
   271 (roguehire) `Skill1` in `Sk1mode` at S, for any other class A1
   at S; end. Else `roll(100)` < 30 → circle 4 at S (no delete), else
   idle 10; end.
3. Draw `lo' % 100` < 20 → wander 5; else idle 10.

The "Else" of step 2 belongs to the first `roll(100)` < 30: both rolls
happen only when S exists with E < 20. In town, with no S, or with E ≥
20, the think goes to step 3 (one `lo'` step). A unit with no room
(`0x00620BB0` returns 0) counts as out of town: `0x0061AB00(0)` returns
0. 1.14d-confirmed (`0x005E7AC0`, `0x0061AB00`).

1.14d-confirmed.

#### 9.32 NpcOutOfTown (31) `0x005E7880`

Classes cain1 (146, Tristram) and drehyaiced (527); live monstats has
no other row with AI 31. The quest calls are
seams (`world/quests.md`; D2MOO names), chosen by class: class 527
(drehyaiced) → Act 5 quest 3 functions, every other class (cain1, and
any class given AI 31 by edited data) → Act 1 quest 4 functions:

| Role | cain1 | drehyaiced |
|---|---|---|
| set up portal coordinates (0 = failed) | `0x005944B0` [`ACT1Q4_UpdateCainPortalCoordinates`] | `0x0058A940` [`ACT5Q3_InitializeDrehyaPortalCoordinates`] |
| spawn the town portal | `0x005944F0` [`ACT1Q4_SpawnCainPortalInTown`] | `0x0058A980` |
| spawn the portal out of town (0 = failed) | `0x005943B0` [`ACT1Q4_SpawnCainPortalOutsideTown`] | `0x0058A820` [`ACT5Q3_SpawnDrehyaPortalOutsideTown`] |
| portal coordinates (0 = none) | `0x00594450` [`ACT1Q4_GetCainPortalOutsideTownCoordinates`] | `0x0058A8D0` [`ACT5Q3_GetDrehyaPortalCoordinates`] |

"Leave" = spawn the town portal, set stat 6 (life) to 0
(`0x00627260`), request mode 12 (dead) at the unit's own position.

1. Portal setup (`0x005E77A0`): K = command 3 (`0x0058EFA0(3, 0)`). If
   K exists with params 1, 2 = 0: params 1, 2 := own x + 3, own y + 3;
   set up portal coordinates, and if that returns 0, leave; K's params
   3, 4 := 1, 0; idle 1; end. The param writes and idle 1 also follow a
   leave (the mode 12 request does not end the function); a K with
   params 1, 2 ≠ 0 skips step 1.
2. drehyaiced: `0x0058AA10(game)`.
3. Someone talks to the NPC (`0x00572DC0`) → idle 40. End.
4. K = command 3 (`0x0058EEF0`). Anim mode 12 (dead) → end, nothing
   scheduled.
5. Interaction handler of §9.9 (`0x005E68F0`); its result is ignored.
6. No K: step 1 again, then idle 40. End.
7. K's param 3 ≥ 2: param 3 += 1; portal coordinates (x, y) found: if
   param 3 < 8 and the path distance to (x, y) ≠ 0 → walk to (x, y),
   else leave; end. Not found → idle 20, end.
8. K's param 3 < 2: if the path distance to (param 1, param 2) > 1 and
   param 4 ≤ 5: param 4 += 1; drehyaiced and `0x0058A9F0(game)` ≠ 0 →
   idle 20, param 4 := 0; else walk to (param 1, param 2); end. Else if
   param 3 = 1: spawn the portal out of town; 0 → params 3, 4 := 1,
   idle 20, end; else param 3 := 2. Idle 20.

No draws. 1.14d-confirmed; same as D2MOO.

Act II bodies (PantherJavelin, GreaterMummy, Mummy, PantherWoman, MaggotLarva, SandLeaper, MaggotEgg, PinHead, ClawViper, Vulture, BatDemon, SandMaggotQueen, Duriel, Summoner) and the special-state thinks 10/17, 11, 12: `monsters/ai-bodies-2.md`.

Act III bodies (Mosquito, ThornHulk, ZakarumZealot, ZakarumPriest, FrogDemon, FetishShaman, HighPriest, FetishBlowgun, WillOWisp, Mephisto) with the FrogDemon and FetishShaman alternates: `monsters/ai-bodies-3.md`.

Act IV bodies (VileMother, VileDog, FingerMage, Regurgitator, Megademon, Diablo with its alternate and the boss target pick and score, Izual, DoomKnight, AbyssKnight, OblivionKnight): `monsters/ai-bodies-4.md`.

Act V bodies (Minion, Imp, Succubus, BloodLord, SuccubusWitch, Overseer, ReanimatedHorde, ClawViperEx, DeathMauler, PutridDefiler, Ancient, AncientStatue, FrozenHorror, SiegeBeast, SuicideMinion, BaalMinion, BaalTaunt, BaalToStairs, BaalThrone, BaalCrab, BaalCrabClone, Nihlathak): `monsters/ai-bodies-5.md`.
