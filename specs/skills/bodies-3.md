# Spec: Skills — Start and do function bodies, batch 4 (monster slots)

- **Status:** draft: every body and helper below read from the 1.14d
  `Game.exe` disassembly (`py tools/ghidra/disasm.py fn|at`; several
  entries have no Ghidra function, `use.md` Open question 1); tables
  read from the image; use counts from the 1.14d `patch_d2`
  `skills.txt`. D2MOO 1.10f names only (no body taken from it). No
  recording covers these bodies yet (Open questions).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use_::bodies` (same module as
  `bodies.md`; `functions.tsv` rows whose note says `bodies-3.md` or
  `bodies-4.md`)
- **Related specs:** `skills/bodies.md` (conventions §1, helpers §2,
  §6), `skills/bodies-2.md` (helpers §2), `skills/bodies-4.md` (the
  rest of this batch); `skills/use.md` (§5.3 start core, §5.4 do core);
  `skills/levels.md`; `combat/hit.md` §4; `combat/damage.md`;
  `missiles/missiles.md` §R2; `monsters/init.md`; `monsters/ai.md`;
  `sim/pets.md`; `sim/path-placement.md`; `sim/pathing.md`;
  `items/inventory.md`; `skills/functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–52 |
| Inputs | 53–57 |
| Outputs / state changes | 58–63 |
| Rules | 64–65 |
|   1. Conventions | 66–93 |
|   2. Implementation questions answered | 94–188 |
|   3. Shared helpers, batch 4 | 189–340 |
|   4. Bodies used by several monster skills | 341–529 |
|   5. Bodies used by one monster skill | 530–713 |
| Constants & data dependencies | 714–748 |
| Randomness | 749–762 |
| Edge cases & original bugs | 763–787 |
| Test vectors | 788–802 |
| Provenance | 803–821 |
| Open questions | 822–832 |
<!-- /index -->

## Summary

Batch 4 of the skill function bodies: the `functions.tsv` slots still
`mapped` after batch 3, ordered by how many live monster rows of
`skills.txt` (`charclass` empty) use them (Constants, "Batch 4 order").
They are the monster skill bodies (Inferno channels, teleports, summon
nests, eggs, jumps, heals, resurrections), the generic Throw /
Unsummon / scroll bodies, and the table slots no data row names.
This file also answers the implementation questions of
`docs/handoff/impl-skill-slots-2.md` (§2). Conventions and helpers of
`bodies.md` / `bodies-2.md` are linked, not restated.

## Inputs

As `bodies.md` (game, unit, skill, level L, `skills.txt` record R, target
T, frame F, difficulty).

## Outputs / state changes

As `bodies.md`: combat records, missiles, monsters, unit fields, state
bits, timers, items, used-entry params and flags, and the value the start
/ do core reads.

## Rules

### 1. Conventions

`bodies.md` §1 and `bodies-2.md` §1 apply unchanged (calling convention
ECX game, EDX unit, stack skill, L, `ret 8`; R, T, `eval(c)`, E and its
params / flags, positions, "zeroed record", "rewind"). Also:

- "Record" = a zeroed 0x5C missile parameter record (`missiles.md`
  §R2.1); fields not named stay 0. "Create" = `0x0059FA30(game, record)`.
  A record that does not set the skill field (+0x2C) leaves it 0.
- "Room at (x, y)" = `0x00463740(the unit's room (0x00620BB0), x, y)`.
- "Pattern stamp / clear (x, y, p, mask)" = `0x0064EA90` / `0x0064EC10`
  (`sim/path-placement.md` §5.1) on the named room. "The unit's
  pattern" = `0x00649180(unit)`.
- "Footprint mask := k" = `0x00648C30(path, k)`; "move-test mask := k" =
  `0x00648CE0(path, k)`; "path type := t" = `0x00648CF0(path, t)`
  (`sim/pathing.md` §2); "compute the path" = `0x00649970(path, unit,
  0)`; P = the unit's path (+0x2C).
- "Delete type-k timers" = `0x00540E60(game, unit, k, 0)`; "type-k timer
  at f" = `0x005417D0(game, unit, k, f, 0, 0)` (`sim/tick.md` §5).
- "Mode request m" = `0x005A7E60(U, m, &req)` then `0x005A7C20(game,
  &req, flag)` (`monsters/ai.md`); flag 1 unless stated.
- "Start core" = `0x0056F640(game, unit, skill, L, 0)` (`use.md` §5.3
  step 6, mana check included).
- "Free T's combat records" = `0x0057C9F0(game, unit, T)`
  (`combat/damage.md` §5.1).
- Every "Hit:" clause below is a nested list; a step outside it runs on
  a miss too.

### 2. Implementation questions answered

Answers to `docs/handoff/impl-skill-slots-2.md` Open questions (numbers
kept). Rules owned by `bodies.md` / `bodies-2.md` are corrected here and
those files point to this section.

1. Answered (rule owner `sim/pets.md` §8, not edited here): the add
   record of `0x00575D90` is {+0 pet GUID, +4 owner GUID, +8 class u16,
   +0xC pet type} and `0x0053CB30` writes record +0 at message byte 5 and
   +4 at byte 9: S→C 0x7A carries the **pet** GUID at +5 and the
   **owner** GUID at +9, as `sim/server-messages.tsv`. `pets.md` §8 and
   its test vector have them swapped.
2. Answered (owner `sim/pets.md` §6–§7, not edited here): Remove with
   kill ≠ 0 of a GUID whose unit exists broadcasts 0x7A three times
   (unlink, the dismiss it calls, Remove step 4); twice when the unit is
   gone or kill = 0. The add record's +0x10 / +0x14 are zeroed and never
   set by `0x00575D90`, so an add always sends 0x7A (0x81 comes only from
   the hireling path). A GUID in two lists is not prevented by the code.
3. Answered: `0x0063EA70` writes `mode` only when it returns a valid
   class from a column (`summon` with a valid R, or the monster `spawn`
   path); R invalid or `summon` outside 0…monstats count − 1 → −1 and
   nothing written. `0x0056E620` may then return the AI-control spawn
   class (`bodies.md` §6.1 step 2) with `mode` (and x, y) **unwritten**:
   the caller's variable keeps its value. Every caller in `bodies.md`,
   `bodies-2.md` and this file passes an uninitialised local (Bone
   Prison `0x005C5BC0`: [ebp−0xC]), so the fallback path gives an
   undefined mode; implement it as "keep the caller's value" and give
   callers 0 (no 1.14d row reaches it: every summon skill has a valid
   `summon`, every caller tests R first).
4. Answered: `bodies.md` §6.5 step 2 is as read: `itemevent2` is
   registered only inside the `itemevent1` > 0 and "no handler found"
   branch (`0x005C457F`). Step 9: the clamps (3L, ≥ 1, ≤ owner level)
   apply only when the given ilvl is 0 (`0x005C497C`–`0x005C49B3`); a
   non-zero ilvl is passed unchanged.
5. Answered: `bodies.md` §6.15 has no lower clamp (`0x005C4183` tests
   only lvl > 10). A negative stat 351 indexes 9·|lvl| bytes before the
   table `0x00741940` (other image data). Unreachable with 1.14d data
   (the `skel_mastery` list holds the mastery level ≥ 1, or the default
   lvl 1); implement as a fatal assertion.
6. Answered: `0x006538A0` flag 8 (`monsters/init.md` §8.1) without
   `noRatio`: TH = pct(monlvl `TH` / `L-TH` for d, monstats `A1TH` for
   d, 100); flag 0x10 uses `A2TH`, flag 0x20 `S1TH` with the same monlvl
   column (`0x00653A3E`, `0x00653AD0`); `noRatio` → the monstats value.
   The `bodies-2.md` §2.1 reading is right.
7. Answered, per body (all from the asm):
   - `bodies-2.md` §2.12 `claw_hit`, `bodies.md` §8.11 (Fend, Zeal,
     Fury), `bodies-2.md` §4.3 (srvst 16 Poison Dagger): `roll_elemental`
     runs **inside** the `EType` ≠ 0 clause (skipped without an
     element); `fill` and the after-roll charges of `claw_hit` run only
     on a hit.
   - `bodies-2.md` §2.19 Leap Attack strike: on a hit, conversion inside
     the `EType` clause, then `roll_elemental` **always** (outside the
     clause), `start_combat`, `apply_melee`, overlay and stun removal;
     a miss does none of them and returns 1.
   - `bodies-2.md` §2.26 Blade Shield callback: `HitFlags`, the
     `SrcDam` fill, `start_combat` and `apply_melee` all run only on a
     hit.
   - `bodies-2.md` §3.1 (Jab): `roll_elemental` outside the clause, as
     written.
8. Answered: `bodies-2.md` §2.16: step 2 / step 1 add the step × 4 to a
   byte index compared signed with 60 / 64; a negative step never ends
   the loop in the original (it walks backwards through memory until it
   faults). No 1.14d caller passes one; implement as a fatal assertion.
   §6.8: k outside 0…2 sets no hit class (the zeroed record keeps 0) and
   E param 1 := (k + 1) rem 3 (C remainder, so a negative k can stay
   negative). §7.2 step 8: the rewind runs **only** when K2 exists
   (`0x005DBC10`–`0x005DBC33`); the body returns 1 either way. §7.10:
   `pettype` ≥ count → pt := 0, as read.
9. Answered: `bodies.md` §6.2 step 4 calls `0x0057CCB0(ECX game, EDX the
   linked unit, stack owner, 1)` (`0x0056D8AE`): the owner is passed as
   the killer, last argument 1 (`combat/damage.md` §7.2).
10. Answered: `0x005701B0(unit, EDX side, skill, owner)`: EDX ≠ 0 sets
    the **left** skill (`0x00643BC0`, skill list +0x08, read by
    `0x00620190`), EDX = 0 the **right** skill (`0x00643C50`, list +0x0C,
    `0x006201D0`; `items/inventory.md` §5 agrees). So `bodies.md` §6.5
    step 6 (EDX 0) sets the right skill, as written, and `bodies.md`
    §2.16 is corrected: `0x005801E0` saves the left skill (list +0x08)
    into the first pair and the right into the second, and the restore
    `0x00580280` re-selects the **left** first (EDX 1), then the right
    (EDX 0).
11. Answered (not a spec question): the providers listed are host
    wiring; the bodies' seams are fully specified by the owning specs
    named in each `Pending` default.
12. Answered: both callbacks are specified: jitter `0x005C9290`
    (`bodies-2.md` §2.3) and damage percent `0x005DB6A0` (`bodies.md`
    §8.19: adds the argument to the missile's `damagepercent(25)`);
    routing them is `missiles.md` §R2.3 step 21 wiring.
13. Also answered (`bodies-2.md` Open question 11): `0x00643C50(U,
    skill, owner)` looks up U's skill entry with that record and owner
    item GUID (+0x34) in list order (`0x00620150`, next +0x04) and, when
    found, makes it the right skill (skill list +0x0C); not found →
    nothing (an invalid skill id is a fatal assertion). So Revive's
    `0x00643C50(T, 0, −1)` sets T's right skill back to its own Attack
    entry.

### 3. Shared helpers, batch 4

#### 3.1 Spawn-column summon class

`spawn_class(unit, skill, L, &mode, &x, &y)` = `0x0056E620` with EDX = 1
(callers pass null for unused outputs): through `0x0063EA70` with its
flag ≠ 0:

1. The unit is a monster with a monstats record: c = `spawn` (+0x20,
   i16); x := unit x + `spawnx` (+0x22, i8), y := unit y + `spawny`
   (+0x23, i8); mode := `spawnmode` (+0x24, i8), outside 0…15 → 1
   (`monsters/population.md` §14). A monster class without a record →
   −1.
2. Any other unit: the skill path of `bodies.md` §6.1 step 1, except x
   := unit x, y := unit y when asked.
3. Then `bodies.md` §6.1 step 2 (range check, AI-control fallback; §2
   answer 3 for the unwritten outputs).

#### 3.2 Inferno channel helpers

- **End** `0x005CC3B0` (unit in ESI, game in EDI): d = monstats2
  `InfernoLen` (+0x108) of a monster's class (monstats +0x18 → monstats2
  record); 11 for a non-monster or a missing record. Delete type-0
  timers; type-1 timer at F + d. State 12 is **not** turned off (unlike
  `bodies.md` §6.17 step 7).
- **Frames** `0x005CC2E0(skill, unit, L)` (EDX skill, EBX the missile M;
  `ret 8`): R invalid → 0; M none or M's class (+0x04) without a missiles
  record → 0. n = `eval(calc1)`; n ≤ 0 → n = M's missiles `Param2` (+0x3C)
  + L − 1. n < 2 → 1; n > 255 → 255. M's path step counts := n
  (`0x00648E70`); M's animation speed (+0x4C) := clamp(((missiles
  `AnimLen` (+0x178) << 8) / n), 0, 0x7FFF) (`0x00621780`); M's total
  frames := n, frames left := n (`0x0064A2B0`, `0x0064A330`). Return 1.
- **Animation** `0x005CC440` (ECX R, EDX unit): a missing unit or a
  non-monster: frame event index (unit +0x38 bits 8+, `0x006212C0`) := 8,
  frame count (+0x48) := 0x500. A monster without a monstats2 record:
  nothing. Else with a = monstats2 `InfernoAnim` (+0x109): R `monanim`
  (+0x11) = 14 (SQ) → frame event index := a − 1, frame count := (a + 1)
  << 8; otherwise current frame (+0x44) := a << 8.

#### 3.3 Throw `0x0056F460`, `0x0056F550`

Both bodies (srvdo 3, srvdo 5) differ only in step 2.2:

1. Inventory (unit +0x60) none → 0.
2. I := none. Weapon pick `0x0063C9B0(inventory, &I, &loc, &inuse)`:
   the item at body location 4, else at 5 (inventory +0x0C → +0x10 /
   +0x14), that is usable (`0x0062A4E0`) and of item type 45 (`weap`):
   I := it, loc := 4 / 5, inuse := 1 when it is the weapon in use
   (`0x0063BEF0`), else 0; returns 1. Neither → I := none, 0. When it
   returns 1:
   1. (step 2.2) srvdo 3: inuse = 0; srvdo 5: inuse ≠ 0 → I := the item
      at the other location (loc 5 → 4, else 5; `0x0063BDE0`).
   2. I's item type is not `throwable` (`0x0062BA80`) and I's
      `item_throwable(125)` (`0x00625500`) = 0 → return 0.
3. I none → 0.
4. Unit flags |= 0x40.
5. m = I's `missiletype` (`0x006288A0`, not range-checked here). I of
   item type 38 (missile potion) → lob `skill_missile` (`0x0056EE90`),
   else straight (`0x0056ECB0`), with (m, unit, skill, L, 0, 0, 0, 0,
   quant 1) (`bodies.md` §2.4).
6. Post-throw `0x0056C600(unit, M, I)` (M = the created missile; any of
   the three null → nothing): M `tohit(19)` := its value + throw mastery
   to-hit `0x00645720(unit, I, 0, 0, 0)`; M `damagepercent(25)` += throw
   mastery damage `0x00645720(unit, I, 0, 1, 0)` (`levels.md` §3.5:
   stats 345 / 346 for a throwing item whose used skill has range 2).
7. Return 1.

#### 3.4 Monster swing `0x005CDDD0`

`mon_swing(game, unit, T, skill, L)` (ECX game, EDX unit; `ret 0xC`):

1. R invalid or T none → 0.
2. Path target := T (`0x00620C10`).
3. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)` (`combat/hit.md` §4).
4. Hit:
   - result |= `ResultFlags`; hit flags |= `HitFlags`; `HitClass` ≠ 0 →
     hit class := it;
   - enhanced damage % (+0x0C) := `eval(calc2)`;
   - `roll_elemental(unit, record, skill, L)`.
5. `start_combat(game, unit, T, record, 128)`; `apply_melee(game, unit,
   T)`.
6. E exists → E param 1 := 1 on a hit, else 0. Return 1.

Differs from `bodies-2.md` §2.24 Swing: `calc2` instead of `calc1`, no
`EType` conversion, `start_combat` always with 128.

#### 3.5 Next action event `0x005CDD30`

`next_event(unit)` (ECX unit): returns the first non-zero event byte
after the current position of the running animation, else 0.

1. With a sequence (`0x006633B0`): n = sequence frame count
   (`0x006633D0`) >> 8; c = frame event index (unit +0x38 >> 8). For i
   = c + 1 … n − 1: e = the sequence event at frame **c** (`0x006634C0(unit
   +0x30, c << 8, &e)`; the frame argument is never advanced, Edge case
   3); e ≠ 0 → return e. Return 0.
2. Without: n = frame count (+0x48) >> 8, c = current frame (+0x44) >> 8;
   n > the AnimData record's frame count (unit +0x50 → +0x08, unsigned) →
   0. For i = c + 1 … n − 1: byte (record +0x10 + i) ≠ 0 → return it.
   Return 0.

#### 3.6 Revive in place `0x005CC960`

`revive(game, T)` (ECX game, EDX T; return value unread):

1. T's room none → stop.
2. T `hitpoints(6)` := T's `maxhp(7)` (raw stat getter `0x00625480`).
3. T flags (+0xC4) |= 0xE.
4. T a monster: flags |= 0x4020000 (no experience, no drop); AI think
   reset `0x00573780(game, T)` (`monsters/ai.md` §2); evil-killed
   counter −1 `0x00547E90(game +0xF0, T)` (`monsters/population.md`
   §16 counters rule 3); quest chain link `0x00545CD0(game, T, room, 0)`
   (`world/quests.md` §4.6); footprint mask := 0x100; path reset
   `0x00649CA0(T)`; pattern stamp (T's position, T's pattern, 0x100) on
   T's room.
5. Other T: footprint mask := 0x80; path reset; pattern stamp (T's
   position, T's pattern, 0x80).

No mode change and no owner change here: callers request the mode.

#### 3.7 Resurrect mode `0x005FD7B0`

`resurrect_mode(U, f)` (ECX U, EDX f): f = 0 → 1. U none, no monstats or
monstats2 record → 1. k = monstats2 `ResurrectSkill` (+0x10C, u16) in
1…skills count − 1 → U's used skill := U's entry of k (`0x006439F0`,
`0x00620210`). Return `ResurrectMode` (+0x10B) when < 16, else 1.

#### 3.8 Direction tables

- `dir64(unit, x, y)` = `0x00621DC0`: the 64-step direction from the
  unit's position to (x, y) (`0x0064FDC0`, `sim/pathing.md` §8.3).
- `dir8(d)` = table `0x00745600`: ((d + 4) >> 3) & 7 for d in 0…63.
- Offset pair e (`0x0063E7E0(e, &dx, &dy)`): dx = table `0x006EA998`
  [e], dy = `0x006EA978`[e] (signed bytes; 32 entries, read from the
  image):

  | e | 0–7 | 8–23 | 24–31 |
  |---|---|---|---|
  | dx | 0, −1, −1, −1, 0, 1, 1, 1 | 0, −1, −2, −2, −2, −2, −2, −1, 0, 1, 2, 2, 2, 2, 2, 1 | 0, −3, −3, −3, 0, 3, 3, 3 |
  | dy | −1, −1, 0, 1, 1, 1, 0, −1 | −2, −2, −2, −1, 0, 1, 2, 2, 2, 2, 2, 1, 0, −1, −2, −2 | −3, −3, 0, 3, 3, 3, 0, −3 |

#### 3.9 Dead-body footprint `0x00649F70`

`0x00649F70(unit, f)` with f = 1 (the only use here): P exists → remove
the footprint (`0x00649560(unit, 1)`, `sim/path-placement.md` §5.2);
stamp a 3 × 3 box with mask 0x8000 at P's position (`0x0064DE30(P room,
P x (+0x02), P y (+0x06), 3, 3, 0x8000)`); snap P to its sub-tile
centre (`0x00650590`). (f = 0 with a monster: remove, pattern := 5,
footprint mask := 0x8000, add, snap; `sim/path-placement.md` §5.3
rule 3.)

### 4. Bodies used by several monster skills

#### 4.1 srvst 53 MonInferno start `0x005CC240`

Skills: FetishInferno, DiabLight, mon inferno sentry, Baal Inferno,
MegademonInferno, Horror Arctic Blast.

1. R invalid → 0. E none → 0.
2. E param 1 := F + max(`eval(calc2)`, 1).
3. The unit lacks state 12 (`inferno`) → state 12 on. Return 1.

No state list is made (unlike `bodies.md` §6.16).

#### 4.2 srvdo 95 MonInferno `0x005CC4E0`

Skills: FetishInferno, mon inferno sentry, Baal Inferno,
MegademonInferno, Horror Arctic Blast (DiabLight uses srvdo 152).

1. R invalid → 0. E none → 0. m = `srvmissilea` (+0x48); not 0 ≤ m <
   missiles count → 0.
2. Record: flags 0x25 (position given, velocity given, target absolute),
   owner the unit, class m, position the unit's (`0x0045ADF0`,
   `0x0045AE20`), level L (+0x30; the skill field stays 0), velocity
   (+0x28) := missiles `Vel` (+0x9A) + trunc(missiles `VelLev` (+0x9B) ×
   L / 8).
3. Target position (`bodies.md` §2.4) into the record target; failure →
   channel end (§3.2); return 0.
4. Create; none → channel end; return 0.
5. Frames (§3.2) on the new missile with (skill, unit, L); animation
   (§3.2) with R.
6. F < E param 1 and the unit has state 12: n = max(`eval(calc3)`, 1);
   delete type-1 and type-0 timers; type-0 timer at F + n. Return 1.
7. Else channel end. Return 1.

Each do makes one missile and either re-arms the next action event (F +
`calc3`) while the start's deadline (F + `calc2`) is ahead, or ends the
animation after `InfernoLen` frames.

#### 4.3 srvdo 98 MonTeleport `0x005CCC80`

Skills: MonTeleport, Teleport 2, Baal Teleport, Baal Clone Teleport.

1. (x, y) = P's target point (`0x00648A00`, `0x00648A10`). Skill, L and
   R are not read.
2. Room = room at (x, y). None: the unit's minion owner O (`0x0058F0D0`)
   exists → room := O's room (`0x00620BB0`).
3. `0x00554EA0(game, unit, room, x, y, 0, 0)` (`sim/path-placement.md`
   §10) = 0 → 0.
4. Queue the unit for update (`0x0064C040`); unit flags 2 (+0xC8) |=
   0x10000 (teleported, `sim/pathing.md` §10 rule 3). Return 1.

#### 4.4 srvdo 113 Scroll / Book `0x005BF3D0`

Skills: Scroll of Identify, Book of Identify, Scroll of Townportal, Book
of Townportal. The Ghidra function ends at `0x005BF3E9`; the body runs
to `0x005BF508`.

1. Inventory (unit +0x60) none → 0.
2. For each inventory node in list order (first `0x0063B2C0`, next
   `0x0063DFA0`), I = its item (`0x0063DFD0`):
   - I of item type 18 (`book`): its books row (item data +0x3E spell
     index, `0x00627F80(I, 0)` → `0x006374B0`, `items/inventory.md` §5)
     exists, `bookskill` (+0x0C) = skill and I's `quantity(70)` > 0 →
     use I (step 3).
   - Else I of item type 22 (`scro`): its books row's `scrollskill`
     (+0x08) = skill → use I (no quantity test).
   - Else next node.
3. Use I by its node kind (`0x0063E020`): 1 → `0x0055E170(game, unit, I
   GUID, I x, I y, &out)` (`items/inventory.md` §7.11); 2 → `0x00562390(
   game, unit, I GUID, I x, I y, &out, 0)` (§7.17); return 1. Any other
   kind → return 0 (the walk stops).
4. No match → 0.

The skill is consumed through the same item-use path as a client use
request; `out` is not read.

#### 4.5 srvdo 110 Hireable / Rogue missile `0x005CE1B0`

Skills: MissileSkill1, HireableMissile, RogueMissile.

1. R invalid → 0. T none → 0.
2. Unit flags |= 0x40.
3. m = `bow_missile(unit, &L)` (`bodies.md` §2.3; L becomes the arrow
   stat for 27 / 41).
4. m ∉ {0 (arrow), 31 (bolt)}: m without a missiles record (also m = −1)
   → 0; else straight `skill_missile(game, m, unit, skill, L, 0, 0, 0, 0,
   quant 0)`; return 1.
5. m ∈ {0, 31}: m := `srvmissilea`; not 0 ≤ m < count or no record → 0.
   R `lob` (flags +0x04 bit 1) → lob `skill_missile`, else straight,
   same arguments. Return 1.

#### 4.6 srvst 49 Nest, EvilHutSpawner `0x005CBD10`

1. R invalid → 0. E none → 0.
2. c = `spawn_class(unit, skill, L, &mode, &x, &y)` (§3.1); c < 0 → 0.
3. E params 1…4 := c, x, y, mode (`0x00644560`, `0x006445A0`,
   `0x006445E0`, `0x00644620`).
4. Pattern stamp (x, y, pattern 1, 0x100) on the room at (x, y)
   (reserves the spot).
5. `set_uninterruptable(unit, 1)` (`bodies-2.md` §2.8).
6. Delete type-1 timers. Return 1.

#### 4.7 srvdo 91 Nest, EvilHutSpawner `0x005CBE00`

1. R invalid → 0. E none → 0.
2. Delete type-1 timers; `set_uninterruptable(unit, 0)`.
3. c, x, y, mode := E params 1…4.
4. Room at (x, y) none, x = 0 or y = 0 → 0.
5. Pattern clear (x, y, 1, 0x100) on that room.
6. m = `0x005B2F20(game, room, x, y, c, mode, spread 1, flags 0)`
   (`monsters/init.md` §1); none → 0.
7. m flags |= 0x4020000 (no experience, no drop).
8. `sumoverlay` (+0xE6) in 0…overlay count − 1 → overlay on m
   (`0x00621E40(m, it, 0)`). Return 1.

#### 4.8 srvst 63 CorpseCycler, VineCycler `0x005D2A10`

1. Unit none or not a monster → 0. R invalid → 0. m = `srvmissilea`; not
   1 ≤ m < missiles count → 0 (m = 0 refused).
2. O = the unit's minion owner (`0x0058F0D0`); none → 0.
3. T none → 0. T has state 118 (`corpse_noselect`) → 0.
4. State 118 on for T; queue T for update.
5. Record: flags 0 (start at the origin), owner O, origin T, class m,
   skill, L. Create (result not read).
6. `msg_a3(unit, T, skill, 1, 0, 0, 0)` (`bodies-2.md` §2.21). Return 1.

The missile belongs to the summoner's owner and starts on the corpse.

#### 4.9 srvdo 85 UnHolyBolt, ShamanFire `0x005CB0C0`

1. R invalid → 0. m0 = `srvmissilea`; not 0 ≤ m0 < count → 0.
2. Unit flags |= 0x40.
3. m = m0 + the chain position of the unit's class (monstats +0x4B,
   `0x006510C0(class, 0, &v)`, `data/fixups.md`; 0 for an invalid
   class). m < 0 or ≥ count → 0.
4. Straight `skill_missile(game, m, unit, skill, L, 0, 0, 0, 0, quant
   0)`. Return 1.

Each member of a monster family (`NextInClass` chain) fires the next
missile row: fallenshaman1 `srvmissilea`, fallenshaman2 the row after,
and so on.

#### 4.10 srvdo 96 ZakarumHeal, Bestow `0x005CC840`

1. R invalid → 0. T none → 0.
2. Unit flags |= 0x40.
3. hi = `eval(calc2)`, negative → 0, ≥ 100 → 100. lo = `eval(calc1)`,
   negative → 0, then min(lo, hi).
4. p = lo + `roll(hi − lo)` (unit seed, `0x0045C3E0`; no draw when hi =
   lo).
5. M = T's maximum life (`0x00625D10`); v = pct(M, p, 100) + T's
   `hitpoints(6)`. T stat 6 := min(max(v, 1), M) (`0x00627260`; when
   max(v, 1) ≥ M the result is M). Return 1.

T is the skill target (the healed ally), not the caster.

#### 4.11 srvdo 97 Resurrect, Resurrect2 `0x005CCB10`

1. R invalid → 0. T none → 0.
2. Unit flags |= 0x40.
3. T must be a monster in mode 0 or 12 (`0x0063EA40`), else 0; T with a
   state of the `hide` group (data tables +0xD4, `0x0063A320`) → 0.
4. T's room none → 0. T's pattern collides at T's position
   (`0x0064D910(room, x, y, pattern, mask)`, mask 0x1C09 for a player T,
   0x3C01 otherwise) → 0.
5. The unit's path target unit := T (`0x00648B90(P, T)`).
6. `revive(game, T)` (§3.6).
7. T a monster: m = `resurrect_mode(T, 1)` (§3.7); mode request m
   (`0x005A7E60(T, m, &req)`); `resurrect_mode(T, 1)` again (sets the
   used skill again; result unread); `0x005A7C20(game, &req, m ≠ 14)`.
8. `srvoverlay` (+0x4E) in 0…overlay count − 1 → overlay on T. Return 1.

T keeps its owner, alignment and level; life is restored to `maxhp`.

#### 4.12 srvst 64 MonFrenzy `0x005CDF00`

Return 1 when T exists, else 0. Skill and L are not read. (The
`functions.tsv` name joins it with srvst 25; srvst 25 is `0x005D6330`.)

#### 4.13 srvdo 109 MonFrenzy, BloodLordFrenzy `0x005CDF10`

1. E none → 0. T none → 0. (R is not tested here.)
2. Free T's combat records.
3. Unit flags |= 0x40.
4. `next_event(unit)` (§3.5) ≠ 0 → start core (§1).
5. `mon_swing(game, unit, T, skill, L)` (§3.4).
6. E param 1 ≠ 0 (the swing hit) → Frenzy charge `0x005D8C70(game,
   unit, skill, L)` (`bodies-2.md` §2.24). Return 1.

### 5. Bodies used by one monster skill

Ordered by the using skill's `Id` (Constants, "Batch 4 order"), start
before do.

#### 5.1 srvdo 3 Throw `0x0056F460`

Throw (§3.3, step 2.2 with inuse = 0: when the picked weapon is not
the one in use, the other hand's item is thrown).

#### 5.2 srvdo 4 Unsummon `0x0056CC20`

No Ghidra function at this entry. E none → 0. Pet remove
`0x005750E0(game, unit, E param 1, kill 1)` (`sim/pets.md` §6; param 1
= the GUID stored by srvst 3, `bodies.md` §3.3). Return 1.

#### 5.3 srvdo 5 Left Hand Throw `0x0056F550`

Throw (§3.3, step 2.2 with inuse ≠ 0).

#### 5.4 srvst 42 Fire Hit `0x005CAE40`

1. R invalid → 0. T none → 0.
2. The unit is a player → return srvst 32 (`bodies.md` §3.8) with
   (game, unit, skill, L).
3. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
   skill, L), 0)`.
4. Hit:
   - result |= `ResultFlags`; hit flags |= `HitFlags`; `HitClass` ≠ 0 →
     hit class := it;
   - `mode_damage(unit, 8)` (`bodies-2.md` §2.1: the S1 damage row).
5. `start_combat(game, unit, T, record, 128)`. Return 1.

#### 5.5 srvdo 83 Fire Hit `0x005CAF30`

1. T none → 0.
2. `apply_melee(game, unit, T)`; free T's combat records.
3. Return the start core's result (§1): the second swing of the
   animation runs the whole start again (srvst 42 included).

#### 5.6 srvst 43 MaggotEgg `0x005CAF80`

No Ghidra function at this entry. Unit flags (+0xC4) &= ~0xE;
dead-body footprint (§3.9). Return 1. Skill and L are not read.

#### 5.7 srvdo 84 MaggotEgg `0x005CAFA0`

1. R invalid → 0.
2. c = `spawn_class(unit, skill, L, &mode, null, null)` (§3.1); c < 0 →
   0. E none → 0.
3. Unit +0x3C (sequence speed, `sim/units.md` §3) := 0.
4. n = `eval(calc1)`; n ≤ 0 → step 6.
5. Spawn near the unit (`0x005B23C0(game, unit, class, mode, spread,
   flags)`, `monsters/init.md` §1):
   1. k = 0. m = spawn (c, mode, −1, 0); m → m flags |= 0x4000000 (no
      experience), k = 1.
   2. s = 1, tries = 0. While k < n, s < 5 and tries < 2n: m = spawn (c,
      **mode 8**, spread s, 0); m → flags |= 0x4000000, k += 1; else s +=
      1; tries += 1.
6. Kill the unit: `0x0057CCB0(game, unit, unit, 1)` (`combat/damage.md`
   §7.2). Return 1.

#### 5.8 srvst 44 MagottUp `0x005CB170`

1. (x, y) = the unit's position (`bodies.md` §1).
2. `0x00554EA0(game, unit, the unit's room, x, y, 0, 0)` = 0 → 0.
3. Unit flags |= 0xE. Pattern stamp (the unit's position after the
   move, pattern 1, mask 0x1000) on the unit's room. Return 1.

#### 5.9 srvst 45 MagottDown `0x005CB270`

No Ghidra function at this entry. Unit flags &= ~0xE; pattern clear
(the unit's position, pattern 5, mask 0x1000) on its room. Return 1.

#### 5.10 srvdo 86 MagottDown `0x005CB300`

1. R invalid → 0.
2. Unit flags |= 0x40.
3. `0x00621810(unit, &cur, &count)`: cur = current frame (+0x44) >> 8;
   cur ≤ 0 → unit +0x3C := 0.
4. p = `eval(calc1)`; p > 0: h = `hitpoints(6)`, M = maximum life
   (`0x00625D10`); stat 6 := min(h + pct(h, p, 100), M). Return 1.

The heal is p % of the **current** life.

#### 5.11 srvdo 87 MagottLay `0x005CB3C0`

1. R invalid → 0.
2. c = `spawn_class(unit, skill, L, &mode, null, null)`; c < 0 → 0.
3. T none → 0.
4. k = `dir8(dir64(unit, T x, T y))` (§3.8); e = table `0x006E3138`[k] =
   (10, 8, 22, 20, 18, 16, 14, 12)[k]; (x, y) = the unit's position +
   (dx[e], dy[e]): k 0 (−2, −2), 1 (0, −2), 2 (2, −2), 3 (2, 0), 4 (2,
   2), 5 (0, 2), 6 (−2, 2), 7 (−2, 0).
5. Unit action frame (+0x4E) := 0.
6. m = `0x005B2F20(game, the unit's room, x, y, c, mode, spread −1,
   flags 0)`; none → 0. m flags |= 0x4000000. Return 1.

#### 5.12 srvdo 88 AndrialSpray `0x005CB580`

The start is srvst 46 (`bodies.md` §3.10, stores T's position).

1. R invalid → 0. m = `srvmissilea`; not 0 ≤ m < count → 0. E none → 0.
2. (x, y) = (E param 1, E param 2); either 0 → target position
   (`bodies.md` §2.4); failure → 0.
3. k = `dir8(dir64(unit, x, y))`. (px, py) = the unit's position +
   pair `0x006E3188`[k] (e = 29, 28, 27, 26, 25, 24, 31, 30): k 0 (3, 3),
   1 (0, 3), 2 (−3, 3), 3 (−3, 0), 4 (−3, −3), 5 (0, −3), 6 (3, −3), 7
   (3, 0).
4. f = clamp(current frame (+0x44 >> 8) − 4, 0, 8). e = table
   `0x006E3140`[9k + f]; e ≠ 99 → (px, py) += (dx[e], dy[e]). Sweep
   offsets by k (f = 4 is 99, no change):

   | k | f = 0, 1, 2, 3 | f = 5, 6, 7, 8 |
   |---|---|---|
   | 0 | (−3, 3), (−2, 2), (−1, 2), (−1, 1) | (1, −1), (2, −1), (2, −2), (3, −3) |
   | 1 | (−3, 0), (−2, 0), (−2, 1), (−1, 0) | (1, 0), (2, 1), (2, 0), (3, 0) |
   | 2 | (−3, −3), (−2, −2), (−2, −1), (−1, −1) | (1, 1), (1, 2), (2, 2), (3, 3) |
   | 3 | (0, −3), (0, −2), (−1, −2), (0, −1) | (0, 1), (−1, 2), (0, 2), (0, 3) |
   | 4 | (3, −3), (2, −2), (1, −2), (1, −1) | (−1, 1), (−2, 1), (−2, 2), (−3, 3) |
   | 5 | (3, 0), (2, 0), (1, −1), (1, 0) | (−1, 0), (−1, −1), (−2, 0), (−3, 0) |
   | 6 | (3, 3), (2, 2), (2, 1), (1, 1) | (−1, −1), (−1, −2), (−2, −2), (−3, −3) |
   | 7 | (0, 3), (0, 2), (1, 2), (0, 1) | (0, −1), (1, −2), (0, −2), (0, −3) |

5. Record: flags 0x20 (target absolute; starts at the origin), owner =
   origin = the unit, class m, target (px, py), level L (skill field 0).
   Create (result not read). Return 1.

#### 5.13 srvst 47 Jump `0x005CB730`

1. R invalid → 0. E none → 0. P none → 0. The unit has state 1
   (`freeze`) → 0.
2. T = target.
   - T none: (x, y) = P's target point (`0x00648A00`, `0x00648A10`); E
     param 4 := −1.
   - T: (x, y) = (2·Tx − ux, 2·Ty − uy) (the point mirrored past T).
3. Room at (x, y) none → 0.
4. T present:
   1. Zeroed record; result = `melee_result(game, unit, T, to_hit(unit,
      skill, L), 0)`.
   2. Hit: result |= `ResultFlags`; hit flags |= `HitFlags`; `HitClass` ≠
      0 → hit class := it; enhanced damage % := `eval(calc1)`;
      `roll_elemental`.
   3. `start_combat(game, unit, T, record, 128)` (also on a miss).
   4. E param 3 := T type; E param 4 := T GUID.
5. Pattern stamp (x, y, the unit's pattern, 0x100) on that room.
6. `set_uninterruptable(unit, 1)`.
7. E param 1 := x, param 2 := y; E flags := 0x80 (launch). Return 1.

#### 5.14 srvdo 89 Jump `0x005CB940`

1. E none → 0. P none → 0.
2. Delete type-1 timers.
3. Action frame (+0x4E) = 2: action frame := 0; K = the unit of (E param
   3, E param 4) (`0x00552F60`); K → `apply_melee(game, unit, K)`. Return
   1.
4. Else, with g = E flags (`0x006446A0`), (x, y) = E params 1, 2:
   1. g & 0x100 (in flight):
      - The unit stands at (x, y): E flags := 0; `set_uninterruptable(
        unit, 0)`; pattern clear (x, y, the unit's pattern, 0x100) on the
        unit's room; footprint mask := 0x100; move-test mask := 0x3C01;
        path type := 101 (0x65, past the 18-row type table: Edge case 1).
        b = the unit's `BaseId` (`0x00463860`, `0x00463900`); b ≠ 78
        (`sandleaper1`) → return 1. b = 78: K = the unit of (E param 3,
        E param 4); none → return 1. E flags := 1; frame event index :=
        12; frame count (+0x48) := (count & ~0xFF) + 0x100; snap P
        (`0x00650590`); P step counts := 5 (`0x00648E70`);
        `0x00649070(P, 1)`; P target unit := none (`0x00648B90`); P
        target point := (3x − 2·Kx, 3y − 2·Ky) (`0x00648AD0`); path type
        := 8; `0x00648E40(P, 5)`; compute the path. Return 1.
      - Not there yet: frame event index := 8 for b = 78, else 0; frame
        count := (count & ~0xFF) + 0x100. Return 1.
   2. Else g & 0x80 (launch): E flags := 0x101; move-test mask := 0;
      footprint mask := 0; P target point := (x, y); path type := 9
      (leap); P velocity := walk velocity `0x0056E5B0(unit)` (`0x00648690`;
      player: charstats `WalkVelocity` (+0x40) << 8; monster: monstats
      `Velocity` (+0x32) << 8; other or no record: 0x600); compute the
      path. Return 1.
   3. Else: E flags := 0. Return 1.

The jump differs from Leap (`bodies-2.md` §2.13): the hit is rolled at
the start and applied at action frame 2; the sandleaper hop aims from
K's position, not the unit's.

## Constants & data dependencies

| Item | Value | Where |
|---|---|---|
| skills columns | `monanim` +0x11, `srvoverlay` +0x4E, `srvmissilea` +0x48, `summon` +0xBC, `sumoverlay` +0xE6, `calc1`–`calc4` +0x138–+0x144, `ResultFlags` +0x12E, `HitFlags` +0x130, `HitClass` +0x134; flags +0x04 bit 1 `lob` | `fields.tsv` |
| monstats columns | `BaseId` +0x02, `spawn` +0x20, `spawnx` +0x22, `spawny` +0x23, `spawnmode` +0x24, `Velocity` +0x32, chain position +0x4B (`data/fixups.md`) | |
| monstats2 columns | `InfernoLen` +0x108, `InfernoAnim` +0x109, `ResurrectMode` +0x10B, `ResurrectSkill` +0x10C | |
| missiles columns | `Param2` +0x3C, `Vel` +0x9A, `VelLev` +0x9B, `AnimLen` +0x178 | |
| books columns | `scrollskill` +0x08, `bookskill` +0x0C | |
| charstats | `WalkVelocity` +0x40 | |
| item types | 18 book, 22 scroll, 38 missile potion, 45 weapon | |
| state ids | 1 freeze, 12 inferno, 118 corpse_noselect | |
| direction tables | `0x00745600` (64 → 8), dx `0x006EA998`, dy `0x006EA978` (32 entries), MagottLay `0x006E3138`, AndrialSpray `0x006E3188` (start) and `0x006E3140` (8 × 9 sweep, 99 = none) | §3.8, §5.11, §5.12 |
| default `InfernoLen` | 11 (non-monster or no monstats2 row) | §3.2 |
| walk velocity default | 0x600 | §5.14 |

Batch 4 order: the `functions.tsv` rows still `mapped` after batch 3,
by the number of 1.14d `patch_d2` `skills.txt` rows with an empty
`charclass` naming the slot in `srvstfunc` / `srvdofunc`, descending;
ties by the lowest such skill `Id`, start before do; slots no row
names last, by kind and index.

| Count | Slots (skills) |
|---|---|
| 6 | st 53 (FetishInferno, DiabLight, mon inferno sentry, Baal Inferno, MegademonInferno, Horror Arctic Blast) |
| 5 | do 95 (the same without DiabLight) |
| 4 | do 98 (MonTeleport …), do 113 (scrolls and books) |
| 3 | do 110 (MissileSkill1, HireableMissile, RogueMissile) |
| 2 | st 49, do 91 (Nest, EvilHutSpawner), st 63 (CorpseCycler, VineCycler), do 85 (UnHolyBolt, ShamanFire), do 96 (ZakarumHeal, Bestow), do 97 (Resurrect, Resurrect2), st 64 / do 109 (MonFrenzy; do 109 also BloodLordFrenzy) |
| 1 | do 3 (Throw 2), do 4 (Unsummon 3), do 5 (Left Hand Throw 4), st 42 + do 83 (Fire Hit 156), st 43 + do 84 (MaggotEgg 159), st 44 (MagottUp 161), st 45 + do 86 (MagottDown 162), do 87 (MagottLay 163), do 88 (AndrialSpray 164), st 47 + do 89 (Jump 165), st 48 + do 90 (Swarm Move 166), st 50 + do 92 (Quick Strike 168), do 93 (GargoyleTrap 172), st 51 + do 94 (Submerge 176), do 111 (FetishAura 177), st 52 (Emerge 180), do 99 (PrimePoisonNova 192), do 152 (DiabLight 193), do 100 (DiabCold 194), do 101 (FingerMageSpider 196), do 102 (DiabWall 197), st 54 + do 103 (DiabRun 198), do 104 (DiabPrison 199), do 105 (DesertTurret 203), do 106 (ArcaneTower 204), st 55 + do 107 (Mosquito 206), do 112 (MonCurseCast 212), do 108 (RegurgitatorEat 214), do 125 (Wake Of Destruction Sentry 281), st 59 + do 126 (Imp Inferno 282), do 141 (Baal Corpse Explode 285), st 60 + do 127 (Suck Blood 289), do 128 (Cry Help 290), st 61 (Self-resurrect 293), do 130 (Vine Attack 294), do 131 (Overseer Whip 295), do 132 (Imp Fire Missile 299), do 133 (Impregnate 300), do 134 (Siege Beast Stomp 301), st 62 + do 135 (MinionSpawner 302), do 136 (DeathMaul 308), do 137 (fenris rage 314), do 140 (Baal Tentacle 315), do 139 (Baal Cold Missiles 318), do 129 (Imp Teleport 330), do 148 (DoomKnightMissile 335), do 149 (NecromageMissile 338) |
| 0 | do 36–41, 143 (progressive functions), 145–147 (state functions), 151 (item effect) |

83 slots: 13 of count ≥ 2, 59 of count 1, 11 of count 0. Bodies past
§5.14 are in `bodies-4.md`.

## Randomness

No body draws except where a step says so; callees draw in the order the
steps call them (`bodies.md` Randomness).

| Where | Seed | Draw |
|---|---|---|
| §3.4 `mon_swing` | unit | `melee_result`, then `roll_elemental` on a hit, then `start_combat` |
| §4.10 | unit | `roll(hi − lo)` when hi > lo |
| §4.6, §4.7, §5.7, §5.11 | game / new monster | monster creation (`monsters/init.md`) |
| §5.4 | unit | `melee_result`, then `start_combat` (and `mode_damage` element draws, `bodies-2.md` §2.1, between them) |
| §5.13 | unit | `melee_result`, `roll_elemental` on a hit, `start_combat` |
| every missile created | game | `missiles.md` §R2.3 step 9 |

## Edge cases & original bugs

1. Jump's landing sets path type 101, past the 18-row type table of
   `sim/pathing.md` §2; the image bytes there give flags 0 and direction
   offset 0 (`0x006EB690` / `0x006EB648` + 4·101 read 0). The path is not
   computed with it here (a later AI path type replaces it).
2. srvdo 95 and srvdo 88 leave the missile's skill field 0 (Attack); the
   level is set. Damage formulas of those missiles that read the skill
   see skill 0.
3. `next_event` (§3.5) with a sequence reads the event byte of the
   current frame on every loop step: it returns that byte or 0, never a
   later frame's.
4. Resurrect requests the mode with flag (m ≠ 14) and calls
   `resurrect_mode` twice (§4.11 step 7).
5. MonFrenzy reruns the start core (mana check and srvst 64) when a
   later action event remains, before its swing (§4.13 step 4); Fire
   Hit's do returns the start core's result (§5.5).
6. MaggotEgg spawns its extra eggs in mode 8 whatever `spawnmode` says
   and kills the caster (§5.7).
7. srvst 63 refuses `srvmissilea` = 0; srvdo 85 accepts 0 and adds the
   family chain position (§4.8, §4.9).
8. srvdo 113 stops at the first matching scroll / book; a match in a
   node kind other than 1 or 2 returns 0 without looking further
   (§4.4).

## Test vectors

| Case (1.14d data unless synthetic) | Expected |
|---|---|
| srvdo 95 velocity, `Vel` 10, `VelLev` 3, L = 5 (synthetic) | 10 + trunc(15 / 8) = 11 |
| Inferno frames, `calc1` 0, `Param2` 4, L = 3, `AnimLen` 16 (synthetic) | n = 6; anim speed (16 << 8) / 6 = 682 |
| srvdo 96, hi 60, lo 80 (synthetic) | lo = 60, no draw, p = 60 |
| srvdo 96, M = 1000, life 0, p 0 (synthetic) | stat 6 = 1 |
| srvdo 85, fallenshaman2 (chain position 1) | m = `srvmissilea` + 1 |
| `dir8` of 0, 3, 4, 60, 63 | 0, 0, 1, 0, 0 |
| MagottLay, direction 20 | k = 3, egg at (+2, 0) |
| AndrialSpray, k = 1, current frames 3, 8, 9, 12 | target − unit position = (−3, 3), (0, 3), (1, 3), (3, 3) |
| Jump from (10, 10) at T (14, 12) | (x, y) = (18, 14) |
| Sandleaper hop, (x, y) = (20, 20), K at (18, 19) | target (24, 22) |

## Provenance

- 1.14d disassembly (`py tools/ghidra/disasm.py fn|at`) of every listed
  address; data-only entries (no Ghidra function) read with `at`:
  `0x0056CC20`, `0x005CAF80`, `0x005CB270`, `0x005BF3D0` (the Ghidra
  function is cut at `0x005BF3E9`).
- Tables read from the image: `0x00745600`, `0x006EA998`, `0x006EA978`,
  `0x006E3138`, `0x006E3140`, `0x006E3188`, `0x006EB690`, `0x006EB648`.
- Column offsets from `data/fields.tsv`; the monstats +0x4A / +0x4B
  chain bytes from `data/fixups.md`; use counts from the 1.14d
  `patch_d2` `skills.txt`.
- §2 answers: `0x0063EA70`, `0x0056E620`, `0x005C4470`, `0x005C4180`,
  `0x006538A0`, `0x005D6200`, `0x005DBC60`, `0x005DA660`, `0x005D7C40`,
  `0x005A9370`, `0x005CFE10`, `0x005DBA40`, `0x005C5BC0`, `0x0056D840`,
  `0x005701B0`, `0x00643BC0`, `0x00643C50`, `0x005801E0`, `0x00580280`,
  `0x00575D90`, `0x00574410`, `0x00574930`, `0x0053CB30`.
- D2MOO 1.10f consulted for names only (`D2MonStatsTxt` +0x4A / +0x4B,
  `UNITFLAGEX_TELEPORTED`).

## Open questions

1. Recording: a Fetish Shaman Inferno and a Baal Inferno: E param 1,
   timers and missile frames per do (§4.1, §4.2).
2. Recording: a Greater Mummy resurrecting: T's mode, used skill and
   life after §4.11.
3. Recording: a Sand Leaper jump and hop: E flags, path type and target
   per do (§5.13, §5.14).
4. Recording: a Maggot Queen / Sand Maggot egg cast: egg count and
   modes (§5.7).
