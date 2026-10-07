# Spec: Monsters — AI think bodies, Act III

- **Status:** draft: the 10 AI functions used by Act III monsters that
  were still unread, their two alternate functions and the scan
  callbacks they use, read from the 1.14d `Game.exe` disassembly
  (addresses per section; register arguments checked in `all.asm`). No
  recording covers these bodies yet (Open questions 1–2).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (per-AI functions beside the
  `ai.md` §9 bodies)
- **Related specs:** `monsters/ai.md` (owner of scheduling §1, dispatch
  §2, AI control and tables §3, aip reads §4, target search §5,
  distances §6, tactics helpers §7, commands §8, the conventions of §9);
  `monsters/ai-bodies-2.md` (Act II bodies; owner of the "wait N" and
  "mode m at (x, y)" helpers, the Vulture land / take-off helpers
  §11 and the special-state thinks §16); `monsters/ai-functions.tsv`
  (catalogue; rows flipped to `spec'd-here` link here);
  `sim/path-placement.md` (positions, collision point test);
  `sim/stat-lists.md` (stat lists, states); `skills/levels.md` (skill
  level); `world/quests.md` (quest flags); skills spec (skill checks).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–67 |
| Inputs | 68–77 |
| Outputs / state changes | 78–85 |
| Rules | 86–87 |
|   1. Scope and order | 88–115 |
|   2. Mosquito (24) `0x005F3730` | 116–138 |
|   3. ThornHulk (27) `0x005F4850` | 139–161 |
|   4. ZakarumZealot (48) `0x005F6E60` | 162–196 |
|   5. ZakarumPriest (49) `0x005F72D0` | 197–234 |
|   6. FrogDemon (52) `0x005F8260`, alternate `0x005F81D0` | 235–276 |
|   7. FetishShaman (65) `0x005F9A80`, alternate `0x005F9950` | 277–315 |
|   8. HighPriest (85) `0x005E0490` | 316–351 |
|   9. FetishBlowgun (96) `0x005E1250` | 352–382 |
|   10. WillOWisp (25) `0x005F39B0` | 383–458 |
|   11. Mephisto (50) `0x005F78B0` | 459–514 |
| Constants & data dependencies | 515–532 |
| Randomness | 533–543 |
| Edge cases & original bugs | 544–563 |
| Test vectors | 564–582 |
| Provenance | 583–608 |
| Open questions | 609–622 |
<!-- /index -->

## Summary

Act III levels (levels.txt `Act` = 2: `mon1`–`mon25`, `nmon*`, `umon*`),
the Act III superuniques (superuniques.txt rows 21–31: Web Mage the
Burning, Witch Doctor Endugu, Stormtree, Sarina the Battlemaid, Icehawk
Riftwing and the six council members) and Mephisto, plus the minions
and spawns those rows name (monstats `minion1`, `minion2`, `spawn`),
give 65 monstats rows. Their AIs not yet specified are written here, in
descending order of how many of those rows use them (§1). Every body
follows the `ai.md` §9 conventions: T, D, C from the dispatch record,
P(aipN) one unit-seed step tested `lo' % 100 < aipN`, "draw" = one
unit-seed step read as `lo' % 100` (inline or `roll(100)` `0x0045C390`,
same value), draws listed in order, "idle N" = `0x005DE080(N)`, "wait
N" = `0x005DE0F0(N)` (`ai-bodies-2.md` Summary), A1 / A2 / SC / S1 =
mode request 4 / 5 / 7 / 8 (`0x005DDF90(mode, unit)`). "Skill k at U"
= `0x005DEAD0(Skk mode, Skill k, U, 0, 0)` (`ai.md` §7.1). Positions
are a unit's path position (`0x0045ADF0` x, `0x0045AE20` y). Frame =
game +0xA8; "frame > p" is a signed compare. Bracketed values are the
Normal values of the named live row.

## Inputs

| Name | Type | Source |
|---|---|---|
| tick record | `ai.md` §2.1 | control (+0x00), T (+0x08), D (+0x14), C (+0x18), monstats (+0x1C) |
| AI control | `ai.md` §3.1 | state (+0x00), params 0–2 at +0x14, +0x18, +0x1C |
| monstats | 424 bytes | `aip1`..`aip8` (+86 + 6(N − 1) + 2·difficulty, signed), `Skill1`..`Skill8` (+0x170..+0x17E), `Sk1mode`..`Sk8mode` (bytes +0x180..+0x187), `Velocity` (+0x32), `Run` (+0x34), `MissS1` (+0x3E), `BaseId` (+2) |
| missiles | 420 bytes | `Range` (+0x96, read signed) |
| game | | +0x1C memory pool, +0x6D difficulty, +0xA8 frame |

## Outputs / state changes

Mode requests, think schedules, AI control params, velocity requests,
AI commands copied to minions (FetishShaman), states 12 (FetishShaman),
unit flags and path pattern (FrogDemon, through the Vulture helpers),
AI params of other wisps and a stat list on the target player
(WillOWisp), unit-seed draws in the order given.

## Rules

### 1. Scope and order

Rows counted by script over the live `patch_d2` monstats.txt,
levels.txt and superuniques.txt (Provenance; the same script gives the
70 Act II rows of `ai-bodies-2.md`). Unread AI index, Act III rows:

| AI (index) | Act III rows | Rows | § |
|---|---|---|---|
| Mosquito (24) | 3 | mosquito1, 2, 4 | §2 |
| ThornHulk (27) | 3 | thornhulk1–3 | §3 |
| ZakarumZealot (48) | 3 | zealot1–3 | §4 |
| ZakarumPriest (49) | 3 | cantor1–3 | §5 |
| FrogDemon (52) | 3 | frogdemon1–3 | §6 |
| FetishShaman (65) | 3 | fetishshaman2–4 | §7 |
| HighPriest (85) | 3 | councilmember1–3 | §8 |
| FetishBlowgun (96) | 3 | fetishblow2–4 | §9 |
| WillOWisp (25) | 1 | willowisp1 | §10 |
| Mephisto (50) | 1 | mephisto | §11 |

Ties are listed by index. The other Act III rows use AIs already
specified: Fetish (6 rows), Arach, Baboon, BatDemon, Vampire (3 each),
Mummy, Vulture (2 each) and 14 single-row AIs (`ai.md` §9,
`ai-bodies-2.md`). 63 of the 65 rows have the monstats `switchai` bit,
so the special states 10, 11 and 12 apply (`ai-bodies-2.md` §16). None
of the bodies below installs a special state: the two alternates (§6,
§7) re-install the control's current state. Act III town NPCs are not
level monster rows and are not counted.

### 2. Mosquito (24) `0x005F3730`

Brackets: mosquito1 [–, –, 40, 40, 5]; `Skill1` Mosquito
(`seq_mosquitoskill`). AI params: 0 = state s (0 approach, 1 hover, 2
retreat), 1 = counter n.

1. C:
   1. Draw < aip3 [40], or n = 0 (the draw is made first, always): n +=
      1; `Skill1` ≥ 0 and draw < aip4 [40] → `Skill1` at T; else A1 at
      T. End.
   2. Draw > 20 → idle 15. End. Else s := 2, n := 0, go on.
2. s = 0: velocity request (method 13, speed 100, steps 0); walk to T
   with flags 0 (`0x005DEC80`). End.
3. s = 1: velocity (0, 50, 0); wander 4 (`0x005DE200`); n += 1; n > aip5
   [5] → s, n := 0. End.
4. s = 2: velocity (2, 100, 0); escape from T by 10 with think delete
   (`0x005DEFE0(T, 10, 1)`), result ignored; s := 1; n := 0. End.
5. Any other s: s, n := 0; idle 10.

So nothing in the AI moves a mosquito from s = 0 to s = 1 except a
retreat; an approaching mosquito keeps approaching until it is in
melee. 1.14d-confirmed; same as D2MOO.

### 3. ThornHulk (27) `0x005F4850`

Brackets: thornhulk1 [80, 15, 10, 30, 5, 3]; `Skill1` MonFrenzy. AI
params: 0 = frenzy swings left f, 1 = frenzy cooldown c.

**Frenzy swing**: `0x005DEAD0(5, Skill1, T, 0, 0)` (mode 5 fixed, not
`Sk1mode`; `Skill1` is not tested for ≥ 0); F := the earliest positive
expiry frame of the unit's type-1 timers (`0x005415A0(unit, 1)`, 0 when
none; `skills/use.md`); F > frame → wait F − frame + aip5 [5] (so the
next think comes aip5 frames after that timer).

1. Not C, or T = 0: f := 0; lunge to T (`0x005DED40(T, 7)`: velocity
   method 13, walk to T, flags 7). End.
2. f ≥ 1: frenzy swing; f −= 1; f < 1 → c := 3. End.
3. Draw ≥ aip1 [80] → draw < aip3 [10] → circle 4 at T (`0x005DF7D0(T,
   4, 0)`, one more draw); else idle 15. End.
4. c < 1 and draw < aip4 [30] (drawn only when c < 1) → frenzy swing; f
   := aip6 [3]. End.
5. c −= 1; draw < aip2 [15] → A2 at T; else A1 at T.

c keeps falling below 0 until the next frenzy sets it to 3 again.
1.14d-confirmed.

### 4. ZakarumZealot (48) `0x005F6E60`

Brackets: zealot1 [65, 50, 35, 50]. AI params: 0 = "attacked last
think", 1 = flee cooldown. L = own life percent (`0x00621F20`). v = the
run speed bonus: `Run` × 100 / `Velocity` − 100 (signed, truncating),
0 when ≤ 0 or `Velocity` ≤ 0, capped at 120 (`ai-bodies-2.md` §16 uses
the same formula).

1. Quest flight (T ≠ 0). Q := T; T a monster → its owner record
   (`0x0058F090(T, &GUID, &type)`); owner type 0 → Q := the player with
   that GUID (`0x00552F60(game, 0, GUID)`, may be 0); any other owner
   type keeps Q = T. If Q is a player, the act of the unit's level
   (`0x006427F0(0x0061A1B0(room))`, `drlg/levels.md`) is 2 (Act III),
   and Q's quest record of the difficulty (player data +0x10 + 4 ×
   difficulty, `0x006221A0`) has quest 21 (A3Q5 The Blackened Temple)
   flag 0 set (`0x0065C310(record, 21, 0)` ≠ 0; `world/quests.md`) →
   velocity (0, v, 0); run away from T by 8 with think delete
   (`0x005DF140(T, 8, 1)`); not started → wander 6. End.
2. AI state 3/19 (`0x005DD2B0`):
   1. Param 1 = 0 and L < aip3 [35]: param 0 := 0; velocity (0, v, 0);
      run away from T by 8 with delete; started → end; else param 1 :=
      5.
   2. The unit's own position collides with mask 0x40
      (`0x0064CB30(room, x, y, 0x40)` ≠ 0, `sim/path-placement.md`): C
      → circle 4 at T (no delete); else walk to T with flags 7. End.
3. Param 1 ≠ 0 → param 1 −= 1.
4. Not C: param 0 := 0; draw < aip4 [50] → velocity (0, v, 0), run to T
   (`0x005DED20`); else walk to T with flags 7. End.
5. Param 0 ≠ 0 and draw ≥ aip1 [65] → param 0 := 0; draw < 80 → idle
   10, else circle 4 at T. End.
6. Param 0 := 1; draw < aip2 [50] → A2, else A1, at T.

Step 5's draw is made only when param 0 ≠ 0. 1.14d-confirmed; same as
D2MOO.

### 5. ZakarumPriest (49) `0x005F72D0`

Brackets: cantor1 [25, 5, 50, 25, 120, 36]; `Skill1` ZakarumHeal,
`Skill2` ZakarumLightning, `Skill3` MonTeleport, `Skill4` MonBlizzard.
AI params: 0 = teleport cooldown frame, 1 = blizzard cooldown frame, 2
= lightning cooldown frame. L = own life percent, read first.

1. C, or AI state 3/19 (tested only when C = 0):
   1. `Skill3` ≥ 0, L < 33 and frame > param 0: param 0 := frame + 4 ×
      aip5 [480]. Point (x, y) := T + k × (T − own position), per axis,
      with k = 4 when C, else 1 (a shift by 2 or 0). Skill check
      `0x005FD470(Skill3, 0, x, y)` ≠ 0 → `0x005DEAD0(Sk3mode, Skill3,
      0, x, y)` (the teleport past the target). End. The cooldown is
      set even when the check fails.
   2. C and P(aip1) [25] → A1 at T. End.
2. Heal scan: scan 1 (`ai.md` §5.4), callback `0x005F7240`, arg {best
   0, count 0, 0x7FFFFFFF, max aip6² [1296]}. A unit U is taken when it
   is a monster other than the scanner, with the scanner's alignment
   pairing (`0x00650D70`, `ai-bodies-2.md` §3), not good (`0x006259B0`
   ≠ 2), not in mode 0 or 12 (`0x0063EA40`), whose `BaseId`
   (`0x00463860`, checked against the monstats count by `0x00463900`)
   is 235 (zealot1) or 238 (cantor1), with squared distance
   (`0x005B0BD0`) ≤ max and life percent ≤ 60: count += 1, best := U.
   The third arg word never changes, so the **last** such unit in scan
   order wins (bug kept).
3. `Skill1` ≥ 0 and a best unit: draw < 25 → `Skill1` at it. End.
4. Clear line (`0x005DD290(unit, T)`: `0x00622AA0(unit, T, 4)` = 0):
   P(aip4) [25] → (a) `Skill4` ≥ 0, frame > param 1 and draw < aip2
   [5] → `Skill4` at T, param 1 := frame + aip5 [120], end; (b) `Skill2`
   ≥ 0, frame > param 2 and draw < aip3 [50] → `Skill2` at T, param 2
   := frame + 20, end.
5. Blocked line: P(aip1) [25], `Skill4` ≥ 0, frame > param 1 and draw <
   aip2 → as 4(a). End.
6. Draw < 30 → circle 4 at T (no delete); else idle 20.

Draws in 4(a), 4(b) and 5 follow their tests left to right; a failing
test stops the line. 1.14d-confirmed; same as D2MOO.

### 6. FrogDemon (52) `0x005F8260`, alternate `0x005F81D0`

Target mode 5 (`ai.md` §2.3: the finder `0x005DE9D0`; no target →
think goes on with T = 0). Brackets: frogdemon1 [65, 20, 50, 50, 20,
12, 15, 9]; `Skill1` Submerge (`seq_froghidden`), `Skill2` Emerge
(`S1`). AI params: 1 = submerged think counter n, 2 = state s (0
start, 1 submerged, 2 surfaced). "Sink" = Vulture take-off
`0x005F2EB0` (unit flags +0xC4 &= ~0x0E, clear cell bits 0x1000 /
0x100, pattern 5, move mask 0, **wait 12**); "surface" = Vulture land
`0x005F2FC0` (`ai-bodies-2.md` §11; returns 1 on success, after
wait 12) followed by `Skill2` in `Sk2mode` at the unit itself and s :=
2.

1. s = 0:
   1. `Skill1` ≥ 0 and D > 12 → `Skill1` at T; wait 8; s := 1; sink
      (its wait 12 replaces the wait 8). End.
   2. `Skill2` ≥ 0 and land succeeds → surface. End.
   3. The own position with collision mask 0xC01 is clear
      (`0x0064CB30(room, x, y, 0xC01)` = 0) → s := 2. Idle 12.
2. s = 1: `Skill2` ≥ 0 and (D < aip8 [9] and land succeeds, or D < 20
   and n > 64 and land succeeds — the second land is tried only when
   the first test fails) → surface. End. Otherwise sink; wait 24; n +=
   1; s := 1. End.
3. Any other s: T = 0 → wait 32. End.
   1. C: draw < aip2 [20] → A2 at T; draw < aip1 [65] → A1 at T; draw
      < aip3 [50] → circle 3 at T (no delete); else wait aip7 [15]. End.
   2. Not C, D < aip6 [12]: draw < aip5 [20] → A2 at T; draw < aip4
      [50] → circle 4 at T; else wait aip7. End.
   3. Not C, D ≥ aip6: draw < aip4 → circle 3 at T; else walk to T
      with 4 steps (`0x005DEF80(T, 4)`).

Each draw of a line is made only when the earlier ones of that line
failed. A failed land still sets unit flags 0x0E (Vulture edge case).

**Alternate** `0x005F81D0` (the think after a re-install over the
running AI, `ai.md` §3.3): the control's special state is not 10 or
11, `Skill1` ≥ 0 and s ≥ 2 → `Skill1` at T, s := 1, sink. Otherwise
re-install the AI for the control's current state (`0x005B0E00`; the
full reset of `ai.md` §3.3 step 4) and wait 1. No draws.

1.14d-confirmed (both).

### 7. FetishShaman (65) `0x005F9A80`, alternate `0x005F9950`

Brackets: fetishshaman2 [40, 0, 15, 66, 50]; `Skill1` FetishInferno
(`A1`), `Skill2` FetishAura, `Skill3` Resurrect2 (`seq_fetishres`).
Minions fetish and fetishblow rows of the same rank.

1. R := level of `Skill1` on the unit: skill record `0x006439B0(unit,
   Skill1, owner −1)`, level `0x006442A0(unit, record, 1)`
   (`skills/levels.md`, with bonus); `Skill1` < 0, no record or level <
   1 → R := 1.
2. `Skill1` ≥ 0, D < R and the unit lacks state 12 (`inferno`,
   `0x00639DF0`): copy the command {type 1, T's GUID (T +0x0C), T's
   unit type} to the minions (`0x0058F730`, `ai.md` §8; T = 0 gives
   {1, −1, 6}); `Skill1` at T. End.
3. State 12 set → state 12 off (`0x00639DB0(unit, 12, 0)`).
4. Fetish scan: scan 1, callback `0x005F99A0`, arg {corpse 0, corpse
   distance 0x7FFFFFFF, max aip5² [2500], alive 0, alive distance
   0x7FFFFFFF, alive count 0, H = aip2 [0]}. A unit U counts when it is
   a monster other than the scanner, has unit flag 0x2 (+0xC4), has no
   `udead` state (`0x0063A770`), (H ≠ 0 or U's minion owner
   (`0x0058F0D0`) is the scanner), its `BaseId` is 141 (fetish1) or —
   when H ≠ 1 — 396 (fetishblow1), its alignment is 0 (`0x006259B0`),
   (H ≥ 3 or U is neither unique nor champion: `0x005A0180(U, 0x0C)` =
   0), and its squared distance d ≤ max. Then U in mode 12 → nearest
   corpse (d strictly smaller replaces); else alive count += 1 and
   nearest alive. Only the corpse is used.
5. A corpse: P(aip1) [40] and the skill check `0x005FD470(Skill3,
   corpse, 0, 0)` ≠ 0 (checked only after the draw passes) → copy the
   command {14, corpse GUID, 1} to the minions; corpse d ≤ aip3 [15]
   (squared d against an **unsquared** aip3) → sequence skill `Skill3`
   on the corpse (`0x005DE000(Skill3, corpse, 0, 0)`); else wander near
   the corpse 10 (`0x005DF530(corpse, 10)`). End.
6. Draw < aip4 [66] → circle 4 at T (no delete); else idle 10.

The minions read commands 1 and 14 in their own AIs (Fetish `ai.md`
§9.21, FetishBlowgun §9). **Alternate** `0x005F9950`: state 12 set →
off; re-install the AI for the control's current state (`0x005B0E00`);
idle 1. No draws. 1.14d-confirmed (both); same as D2MOO.

### 8. HighPriest (85) `0x005E0490`

Brackets: councilmember1 [75, 25, 125, 40, 70, 8, 15, 30]; `Skill1`
Hydra, `Skill2` ZakarumHeal; `MissS1` the S1 missile. AI params: 0 =
engaged e, 1 = spell cooldown frame.

1. e = 0:
   1. C: P(aip1) [75] → e := 1, A1 at T; else escape from T by 6 with
      think delete (`0x005DEFE0(T, 6, 1)`). End.
   2. `Skill2` ≥ 0, frame > param 1 and draw < aip2 [25]: heal scan
      (scan 1, callback `0x005E0430`, arg {best 0, best life 75, max
      2500}): U is taken when it is a monster not in mode 0 or 12, with
      the scanner's alignment pairing, alignment 0, squared distance ≤
      2500 and life percent ≤ 75 and strictly below the best so far
      (best := U, best life := its percent; lowest wins, ties keep the
      earlier). Found → param 1 := frame + aip3 [125]; `Skill2` at it.
      End.
   3. `Skill1` ≥ 0, frame > param 1, D < aip8 [30] and draw < aip4
      [40]: k := `0x00472210(seed, 4)` (one step, `lo' & 3`); point :=
      T's position + offset k of {(−5, −5), (5, −5), (5, 5), (−5, 5)}
      (table `0x006E337C`); `0x005DEAD0(Sk1mode, Skill1, T, x, y)`;
      param 1 := frame + 100. End.
   4. aip5 [70] > 0, `MissS1` > 0, the missile row exists
      (`0x0046ACE0`), D < its `Range` − 2 and draw < aip5 → S1 at T.
      End.
   5. Draw < 80: D ≤ aip8 → circle 3 at T (no delete); else walk to T
      with 6 steps (`0x005DEF80(T, 6)`). End. Draw ≥ 80 → go on.
2. e := 1.
3. C: draw < aip7 [15] → S1 at T. End. Draw < aip6 [8] → escape from T
   by 6 with delete; e := 0. End. Draw < 90 → A1 at T; else idle 10.
4. Not C: D < 6 and draw < aip7 → S1 at T. End. Draw < aip6 → e := 0;
   idle 10. End. Draw < 70 → walk to T with flags 7; else wander 12.

Each test of a line draws only when the tests before it in that line
hold. 1.14d-confirmed; same as D2MOO.

### 9. FetishBlowgun (96) `0x005E1250`

Brackets: fetishblow2 [20, 30]. AI params: 0 = state s (0 shoot, 1
reposition, 2 retreat), 1 = shot counter n.

1. Current command K (`0x0058EE80`). The unit it names is
   `0x00552F60(game, type K param 2, GUID K param 1)`.
   1. K type 1 and that unit exists → s, n := 0; A1 at it; free K
      (`0x0058ED10`). End.
   2. K type 14 and it exists → s, n := 0; velocity (13, 50, 0); walk
      to it with flags 0; free K. End.
   3. Any other K: free it, go on.
2. Not C: D > aip1 [20] → velocity (0, 50, 0); wander near T 6
   (`0x005DF530(T, 6)`). End. D ≥ 6 → step 4.
3. (C, or D < 6) draw < aip2 [30] → s := 2.
4. S := secondary target (`0x005DDC30`, second argument 0; its distance
   is not used).
5. s = 0: n += 1; draw `lo' % 3` + 3 < n → s := 1, n := 0. S → A1 at
   S; else circle 4 at T (no delete). End.
6. s = 1: s, n := 0; velocity (0, 50, 0); circle 6 at T with delete
   (`0x005DF7D0(T, 6, 1)`); not started → idle 10. End.
7. s = 2:
   1. D ≤ 12: velocity (2, 50, 0); escape from T by 14 with delete;
      not started → A1 at S (`0x005DDF90(4, S)`, also when S = 0), s,
      n := 0. End.
   2. D > 12: draw < 20 and circle 4 at T with delete started → end.
      Otherwise s, n := 0; idle 10. End.
8. Any other s: idle 10.

1.14d-confirmed.

### 10. WillOWisp (25) `0x005F39B0`

Brackets: willowisp1 [40, 70, 50]; `Skill1` Chain Lightning (`SC`). AI
params: 0 = state s (0 idle, 1 moving, 2 may cast, 3 may strike, 4
gather, 5 baptism, 6–12 ritual shapes), 1 = counter n, 2 = slot / ritual
cooldown frame r (one field, both uses). "Approach P" (P a point): path
distance unit→P (`0x005DC5C0`, unsigned) > 3 → draw < 34 → wander 4,
else mode 2 at P with no path step (`0x005DE490(x, y, 2)`), end;
otherwise "near". m67 := frame % 67, m337 := frame % 337 (signed).

0. Ritual trigger: T ≠ 0, frame > r, squared distance unit→T
   (`0x005B0BD0`) < 1024, s < 4, and `roll(1000)` (`0x0045C390`, `lo'
   % 1000`) ≤ difficulty + 2 → s := 4. The draw is made only when the
   first four hold.
1. s = 4 (gather): unit find around the own position, size 32, flags
   0x583, callback `0x005F3960` (a monster not in mode 0 or 12 with
   `BaseId` 118 willowisp1, the finder itself included), over the
   neighbouring rooms (`0x0065A950` init, `0x0065AC70` collect,
   `0x0065AA00` free; Open question 3). Exactly 4 found → each found
   unit i (i = 0..3 in found order): its AI param 0 := 6, param 1 := 0,
   param 2 := i + 1 (`0x0058EC00(unit, 1..3, v)`), idle 337 − m337 on
   it; then idle 337 − m337 on the finder. Exactly 5 → the same with
   param 0 := 5. Any other count → s := 2; idle 8.
2. s ≥ 6 (ritual shape s, slot r):
   1. r > 4 → s := 2; idle 8. End.
   2. Approach T + A[s][r]. Near: m67 ≠ 0 → idle 67 − m67. End.
   3. n < 3: B[s][r] = (0, 0) → n += 1, idle 67. Else SC at T +
      B[s][r] (`0x005DDFC0(7, x, y)`), n += 1. End.
   4. n ≥ 3: s += 1; n := 0; s > 12 → s := 2, r := frame + 1800. Idle
      337 − m337. Then, when T is a player that is not dead
      (`0x005541B0`): stat list `0x006251F0(pool, 2, frame +
      1,728,000, T type, T GUID)`; allocated → stat 80
      `item_magicbonus` := 50 × (difficulty + 1) (`0x00627150(list, 80,
      v, 0)`), expiry frame + 1,728,000 (`0x006260B0`), a type-12 timer
      on T at that frame (`0x005417D0`), attach to T (`0x00626E10(T,
      list, 1)`) (`sim/stat-lists.md`). This runs after **every**
      completed shape, not only the last.
3. s = 5 (baptism, slot r):
   1. r > 5 → s := 2; idle 8. End.
   2. Approach T + C[r]. Near: m67 ≠ 0 → idle 67 − m67. End.
   3. n ≥ 3 → s := 2, r := frame + 1800, idle 337 − m337. End.
   4. E[r] = (0, 0) → n += 1, idle 337 − m337. Else SC at T + E[r], n
      += 1. End.
4. s = 1: n ≤ 0 and C → S1 with no target (`0x005DDF90(8, 0)`), s :=
   3. End. n ≤ 0, not C and draw < aip1 [40] → S1 with no target, s :=
   2. End. Otherwise n −= 1; s := 1; draw < aip3 [50] → walk to T with
   flags 0; else wander 6.
5. Any other s (0, 2, 3):
   1. Not C: s = 2 or draw < aip1 (drawn only when s ≠ 2) → SC at T; s
      := 0. End.
   2. C: s = 3 or draw < aip2 [70] (drawn only when s ≠ 3) → A1 at T; s
      := 0. End.
   3. s := 1; draw < aip3 → walk to T with flags 0; else wander 4; n :=
      3.

Offset tables (x, y), built on the stack each think. Ritual stand
points A[s][slot 1..4] and ritual cast points B[s][slot 1..4]:

| s | A slot 1–4 | B slot 1–4 |
|---|---|---|
| 6 | (−10, −3) (1, −5) (6, 0) (4, 10) | (1, −5) (6, 0) (4, 10) (−10, −3) |
| 7 | (−9, −2) (−6, −11) (−2, −5) (6, 1) | (1, 8) (−9, −2) (−5, 4) (1, 8) |
| 8 | (0, 13) (−12, 0) (7, 5) (−3, −11) | (−12, 0) (7, 5) (−3, −11) (11, −2) |
| 9 | (−15, −3) (−6, −3) (−4, −5) (−6, −13) | (7, 9) (−15, −3) (−6, −13) (9, 7) |
| 10 | (−12, −2) (−6, 2) (−5, 5) (1, 11) | (−5, −7) (−5, −7) (9, 0) (−12, −2) |
| 11 | (−13, −6) (−8, −11) (−7, 0) (1, 8) | (−8, −11) (−2, −6) (−2, −6) (−13, −6) |
| 12 | (−5, −8) (−8, 1) (1, −6) (6, 9) | (−8, 1) (1, −6) (6, 9) (0, 0) |

Baptism stand points C[slot 1..5] = (−5, −5) (3, −5) (6, 3) (3, 6)
(−5, 3); cast points E[slot 1..5] = (6, 3) (3, 6) (−5, 3) (−5, −5)
(3, −5). Slot 0 entries are not initialised (unreachable: slots start
at 1).

1.14d-confirmed (all tables read from the stack stores); same as D2MOO
(including the buff after every shape).

### 11. Mephisto (50) `0x005F78B0`

Brackets: mephisto [15, 25, 25]; skills `Skill1` PrimeLightning,
`Skill2` PrimeBolt, `Skill3` PrimePoisonNova, `Skill4`
MephistoMissile, `Skill5` MephFrostNova, `Skill6` Blizzard (`Sk6mode`
+0x185), all `A2`. AI params: 0 = volley count v, 1 = volley step c, 2
= state s. L = own life percent; K := (100 − L) / 5 (signed,
truncating), 0 when negative. Every exit writes the final s to param
2.

**Pick** `0x005F77C0(game, unit, monstats, T)`: scan 1, callback
`0x005F7700`. A unit U counts when it is not the scanner, not dead
(`0x005541B0(U)`), has unit flag 0x4 (+0xC4), passes the hostility
test `0x00554200(game, unit, U)` (`combat/hit.md`) and has squared
distance d ≤ 1024. The callback keeps the lowest life U (stat 6 >> 8,
strictly lower replaces), and a "nearest player" that it records only
when the **scanner** is a player — never for Mephisto (bug kept). Lowest
found and draw < aip2 [25] → it; else the nearest-player branch
(never taken, no draw) and T.

1. C: draw > aip1 [15] → s := 3. Else draw → v := `lo' % 3` + 3; s :=
   2.
2. Not C: D > 20 → s := 4. Else s ≠ 0: s ≤ 4 → keep s; s > 4 (unsigned)
   → **roam**. Else (s = 0): L ≤ 20, D < 5 and draw < 40 → s := 1;
   else draw < K + 50 → draw → v := `lo' % 3` + 3, s := 2; else
   **roam**.
3. Run the case of s:
   - 0: s := 4; idle 5.
   - 1: s := 0; velocity (0, 50, 0); escape from T by 8 with think
     delete; started → end. C → `Skill3` at T, end. Not C → case 2
     (with s = 0).
   - 2: v −= 1; v = 0 → s := 0. c = 0 → c := 2; circle 3 at T with
     delete (`0x005DF7D0(T, 3, 1)`); not started → wander 12; end.
     Else c += 1; X := pick; n := the number of leading `Skill1`..
     `Skill8` ≥ 0 (stop at the first negative; Mephisto 6). n = 0 →
     wander 6. Else q := 100 / n [16]; draw r; then the first that
     holds: difficulty > 0 and (X blocked from the unit,
     `0x00622AA0(unit, X, 4)` ≠ 0, or D > 30) → `Skill6` at X; D < 15,
     r < q and difficulty > 0 → `Skill5` at X; r < 2q → `Skill4` at X;
     r < 3q → `Skill2` at X; else `Skill1` at X. Then draw < 50 − K →
     c := 0. End.
   - 3: s := 0; draw < K + 80: draw < 80 − K → A1 at T, else `Skill3`
     at T. Else velocity (0, 50, 0); circle 3 at T with delete; not
     started → wander 12.
   - 4: s := 0; velocity (0, 50, 0); wander near T 6 (`0x005DF530`)
     started → end. Else delete the unit's thinks (`0x00540E60(2,
     0)`); walk in radius of T (`0x005DE6D0(T, 12, 6)`); not started →
     wander 12.
4. **Roam**: s := 0; draw ≥ 65 → idle 10. Else draw < 65 or D > 5 →
   circle 4 at T (no delete); else velocity (0, 0, steps 4), walk to T
   with flags 7.

Mephisto on Normal never casts Blizzard or Frost Nova here. The skill
draw r of case 2 is made before the difficulty tests. 1.14d-confirmed;
same as D2MOO.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| zealot quest | act 2 (Act III), quest 21 flag 0, flee 8 | `0x005F6E60` |
| zealot hurt cooldown | 5 thinks; collision mask 0x40 | `0x005F6E60` |
| priest heal scan | `BaseId` 235 / 238, life ≤ 60 %, radius aip6, last wins | `0x005F7240` |
| priest teleport | life < 33 %, cooldown 4 × aip5, point T + k(T − U), k 4 / 1 | `0x005F72D0` |
| priest cooldowns | blizzard aip5, lightning 20 | `0x005F72D0` |
| frog | submerge D > 12; emerge D < aip8, or D < 20 after 64 thinks; collision mask 0xC01; waits 8, 12, 24, 32 | `0x005F8260` |
| shaman | inferno range = `Skill1` level; corpse `BaseId` 141 / 396; unique / champion mask 0x0C | `0x005F9A80`, `0x005F99A0` |
| high priest | heal ≤ 75 %, squared radius 2500; hydra offsets ±5 (`0x006E337C`); hydra cooldown 100 | `0x005E0490`, `0x005E0430` |
| blowgun | retreat D ≤ 12 by 14; burst `lo' % 3` + 3 shots | `0x005E1250` |
| wisp | trigger `roll(1000)` ≤ difficulty + 2, squared 1024; find size 32 flags 0x583; beats 67 / 337; cooldown 1800; buff stat 80 = 50 × (d + 1) for 1,728,000 frames, timer type 12 | `0x005F39B0` |
| mephisto | K = (100 − L)/5; pick squared 1024; skill share 100 / n | `0x005F78B0`, `0x005F77C0`, `0x005F7700` |
| thorn hulk | frenzy mode 5; cooldown 3 | `0x005F4850` |
| monstats | `aip1..8`, `Skill1..8` +0x170..+0x17E, `Sk1..8mode` +0x180..+0x187, `Velocity` +0x32, `Run` +0x34, `MissS1` +0x3E | `data/fields.tsv` |

## Randomness

All draws are on the thinking monster's unit seed (`rng.md` §7), in the
order of each section; a test that is not reached draws nothing. Forms:
`lo' % 100` (P(…), "draw", `roll(100)` `0x0045C390`), `roll(1000)`
(WillOWisp trigger), `lo' % 3` (FetishBlowgun, Mephisto), `mask(4)`
(`0x00472210`, HighPriest), plus the helpers' draws (`ai.md` §7.2:
wander 3–4, circle 1) and the alternates' none. The scan and find
callbacks draw nothing. Skill checks (`0x005FD470`) and the dispatcher's
finders have their own draws, owned elsewhere.

## Edge cases & original bugs

1. ZakarumPriest §5 step 2: the heal scan keeps the last qualifying
   unit, not the most hurt one.
2. ZakarumPriest §5 step 1.1: the teleport cooldown is spent even when
   the skill check fails.
3. FetishShaman §7 step 5 and PantherJavelin (`ai-bodies-2.md` §2):
   squared distance against an unsquared aip.
4. FetishBlowgun §9 step 7.1: a failed retreat attacks S even when
   there is no secondary target (mode request with target 0).
5. FrogDemon §6 step 1.1: the wait 8 is replaced by the sink's wait 12.
6. Mephisto §11: the pick's second branch is dead (nearest player only
   recorded for a player scanner); case 0 is unreachable from the
   dispatch of step 2 (s = 0 never reaches the case table) but kept.
7. WillOWisp §10: AI param 2 is both the slot and the cooldown frame; a
   wisp joining a formation loses its cooldown. The magic-find buff is
   posted after every completed shape.
8. ThornHulk §3: the frenzy uses mode 5 and `Skill1` without the usual
   `Skill1` ≥ 0 test.

## Test vectors

Synthetic (CI-safe), draws given as the `lo' % 100` values:

| AI, input | Draws | Result |
|---|---|---|
| Mosquito, C = 1, n = 0, mosquito1 Normal | 90 (≥ 40, but n = 0), 10 (< 40) | n := 1, `Skill1` at T |
| Mosquito, C = 1, n = 2 | 50 (≥ 40), 15 (≤ 20) | s := 2, n := 0; velocity (2, 100, 0), escape by 10, s := 1 |
| ThornHulk, C = 1, f = 0, c = 0, thornhulk1 | 50 (< 80), 20 (< 30) | frenzy swing, f := 3 |
| ThornHulk, C = 1, f = 0, c = 2 | 50, 10 (< 15) | c := 1, A2 at T |
| ZakarumZealot, no quest, AI state 0, C = 1, param 0 = 1, zealot1 | 70 (≥ 65), 85 (≥ 80) | param 0 := 0, circle 4 at T |
| ZakarumPriest, C = 0, AI state 0, no heal target, clear line, cantor1, frame 100, params 0 | 20 (< 25), 3 (< 5) | MonBlizzard at T, param 1 := 220 |
| FrogDemon, s = 2, C = 1, frogdemon1 | 30 (≥ 20), 70 (≥ 65), 60 (≥ 50) | wait 15 |
| HighPriest, e = 0, C = 0, D = 40, frame > param 1, `MissS1` `Range` ≤ 42, councilmember1 | 30 (≥ 25: no heal scan), no hydra draw (D ≥ 30), no S1 draw, 50 (< 80) | walk to T with 6 steps |
| FetishBlowgun, s = 0, n = 3, C = 1, S present, fetishblow2 | 50 (≥ 30), then `lo' % 3` = 0 | n := 4 > 3 → s := 1, n := 0; A1 at S |
| Mephisto, C = 1, L = 100, mephisto | 50 (> 15) | s := 3; then draw 70 (< 80), draw 85 (≥ 80) → `Skill3` at T; param 2 := 0 |

Game-file vectors: Open question 1.

## Provenance

- 1.14d `Game.exe`: `0x005F3730`, `0x005F4850`, `0x005415A0`,
  `0x005F6E60`, `0x0058F090`, `0x00552F60`, `0x006427F0`,
  `0x006221A0`, `0x0065C310`, `0x0064CB30`, `0x005F72D0`,
  `0x005F7240`, `0x00463860`, `0x00463900`, `0x005DD290`,
  `0x005F8260`, `0x005F81D0`, `0x005F2EB0`, `0x005F2FC0` (entry only),
  `0x005F9A80`, `0x005F9950`, `0x005F99A0`, `0x006439B0`,
  `0x006442A0` (entry only), `0x005A0180`, `0x005E0490`, `0x005E0430`,
  `0x00472210`, `0x0046ACE0`, `0x005E1250`, `0x005F39B0`,
  `0x005F3960`, `0x005DE490`, `0x005F78B0` (with its jump table
  `0x005F7EF0`, five cases), `0x005F77C0`, `0x005F7700`. Ghidra
  decompile read first, every call's register and stack arguments
  checked in the disassembly (`tools/ghidra/disasm.py`); the HighPriest
  table `0x006E337C` and the Mephisto jump table read from the file;
  the WillOWisp tables reconstructed from the function's stack stores
  (register values tracked by a throwaway script).
- Live data (`patch_d2`): monstats.txt (`AI`, `aip*`, `Skill1`..`8`,
  `switchai`, `minion1/2`, `spawn`, `BaseId`), levels.txt (Act 2 rows,
  `mon*`, `nmon*`, `umon*`), superuniques.txt (rows 21–31) — counted
  by a throwaway script (scratch, not committed); quest 21 = A3Q5 from
  `world/quests.tsv`.
- D2MOO 1.10f `AiThink.cpp`: names, state and param meanings; compared
  for every body here: same rules as read in 1.14d (WillOWisp tables
  and Mephisto cases included).

## Open questions

1. No recording of any Act III AI: record a Spider Forest / Flayer
   Jungle / Travincal / Durance run (tick recorder) and compare think
   schedules and draws with §2–§11.
2. FrogDemon §6: confirm the submerged footprint (Vulture helpers) and
   the emerge timing with a recording that logs modes and collision.
3. Answered (2026-10-07): the unit find `0x0065A950` / `0x0065AC70`
   (rooms, found order, filter) is owned by `monsters/umod-callbacks.md`
   §3.1.
4. Who calls the FrogDemon and FetishShaman alternates (`ai.md` §3.3
   re-install while running): the skill or event paths that re-install
   AIs 52 and 65.
