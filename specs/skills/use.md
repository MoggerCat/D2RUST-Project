# Spec: Skills — Use pipeline (intent → validation → mode → start / do)

- **Status:** draft: read from the 1.14d `Game.exe` disassembly and
  image (addresses below); timer patterns checked against the 2026-10-06
  recordings (`traces/raw/20261006-021854-tick.jsonl`, `…-022304-…`):
  attack request → type-0 do event at start + 6/7, ENDANIM at + 14/15,
  ENDANIM cancelled by the next request in 10 of 13 (melee) and 46 of 59
  (missile session) cases. Per-skill function bodies: `skills/bodies.md`
  for the `functions.tsv` rows marked `spec'd-here`.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use` (message handlers in
  `d2-server` call into it); function tables in `d2-sim::skills::funcs`
- **Related specs:** `sim/intents-events.md` (+ `client-messages.tsv`:
  message ids, sizes, dispatch gate); `sim/tick.md` §5 (timer queue;
  event types 0, 1, 5, 8, 9, 12); `sim/units.md` (modes, animation frames,
  ENDANIM, the run-to-target path); `skills/levels.md` (level, mana cost,
  delay and to-hit formulas, learning); `combat/hit.md`,
  `combat/damage.md` (what do-functions call); `sim/rng.md`. Machine
  table: `skills/functions.tsv` (start and do function tables).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 44–57 |
| Inputs | 58–67 |
| Outputs / state changes | 68–74 |
| Rules | 75–76 |
|   1. Messages | 77–108 |
|   2. `use_at_point(game, unit, skill, x, y)` = `0x00549AD0` | 109–150 |
|   3. `use_on_unit(game, unit, skill, type, guid, run)` = `0x00549BA0` | 151–169 |
|   4. Mode change gates | 170–203 |
|   5. Start and do | 204–324 |
|   6. Cooldown | 325–338 |
|   7. Periodic skills and auras | 339–388 |
|   8. Function tables | 389–408 |
| Constants & data dependencies | 409–429 |
| Randomness | 430–439 |
| Edge cases & original bugs | 440–461 |
| Test vectors | 462–475 |
| Provenance | 476–492 |
| Open questions | 493–533 |
<!-- /index -->

## Summary

A skill message (left/right skill at a point or on a unit) is validated,
picks the left or right skill, checks whether the skill is usable now
(mana, quantity, shape, cooldown), and changes the player's mode to the
skill's animation mode. The mode change runs the skill's **start**
function (`srvstfunc`) in the same tick; it can refuse and cancel the
animation. The animation's action frame (a type-0 timer event) runs the
**do** function (`srvdofunc`) or fires `srvmissile`. Mana is charged at
start or at do; the shared cooldown (`delay`) is a state with a type-12
timer. Auras and periodic skills re-run the do function from type-8
timers. This spec owns that pipeline, the two function tables and their
draw.

## Inputs

| Name | Type | Source |
|---|---|---|
| client messages 0x05–0x11, 0x3B, 0x3C | `client-messages.tsv` | client |
| player unit: left / right / used skill, mode, stats, states, timers | | `sim/units.md` |
| `skills.txt` record (0x23C bytes) | offsets in §Constants | `data/fields.tsv` |
| `states.txt` `srvactivefunc` (+0x36) | | type-5 timers |
| game frame (game +0xA8) | | `sim/tick.md` §2 |

## Outputs / state changes

Mode changes and their timers (types 0, 1); used skill; mana / life /
charges; state 121 (`skilldelay`) and type-12 timers; type-8 / type-9
timers; aura states; server message 0x5A ("can't do that") on a mana
failure; stat 328 (`pierce_idx`) +1 per skill message.

## Rules

### 1. Messages

Handlers (addresses in `client-messages.tsv`):

| Id | Skill | Target | Run allowed |
|---|---|---|---|
| 0x05 | left | point | — |
| 0x06 / 0x07 | left | unit | yes / no (shift) |
| 0x0C | right | point | — |
| 0x0D / 0x0E | right | unit | yes / no (shift) |
| 0x08, 0x09, 0x0A, 0x0F, 0x10, 0x11 | "hold" forms | call 0x05, 0x06, 0x07, 0x0C, 0x0D, 0x0E | |

1. Point validator `0x005496F0`: `sim/intents-events.md` §2.4 rule 3
   (results 3 size, 2 no player data, 1 far; the 0x15 resync is sent
   only on 1, never on 2).
2. Unit validator `0x00549830` → `0x00548F80`: `sim/intents-events.md`
   §2.4 rule 4 (results 3 size ≠ 9, 2 type ≥ 6 or another act, 1 unit
   not found or farther than 50 on either axis, 0 accepted; no resync).
3. No left skill (left messages) or right skill (right messages) → 3.
4. `pierce_idx(328)` = base + 1 (`0x006253B0` / `0x00627260`), before the
   use attempt, whether or not it succeeds.
5. Point: `use_at_point` (§2). Unit: `use_on_unit` (§3) with run allowed
   for 0x06 / 0x0D. Return 0.
6. Hold handlers (`0x00549E80`, `0x00549EE0`, `0x00549F40`, `0x0054A140`,
   `0x0054A1A0`, `0x0054A200`): no left/right skill → 3; else call the
   plain handler and return 0 whatever it returned. The server never
   repeats on its own; the client resends (recording 015956: 0x10 every
   13 frames).

No handler checks `InTown`, `InGame` or `leftskill`; `leftskill` is never
read by server code (client `0x004AA3A0` only).

### 2. `use_at_point(game, unit, skill, x, y)` = `0x00549AD0`

1. Unit null, or mode 0 (death) or 17 (dead) → 2.
2. Dual wield (`0x00549960`): only for skill 0 (Attack), when the unit can
   dual-wield (`0x006235A0`) and both hands (bodylocs 4, 5) hold
   equippable items of type 45 (weapon) that are not type 38: if Attack's
   Param4 ≠ 0 → set it to 0, use Attack; else set it to 5, use skill 5
   (Left Hand Swing). The left hand swings first, then they alternate.
3. `state = use_state(unit, skill)` (`0x00647960`; codes 0 usable, 1 no
   mana, 2 no quantity, 3 disabled, 4 shape restriction, 5 passive, 6
   aura, 7 no level, 8 cooldown). No record → 2.
4. State 1, 2 or 4 and the skill has `AttackNoMana`: use skill 0 instead
   and recompute.
5. Skill mode (skill entry +8, §5.1) = 0 → 2.
6. State 0: `set_mode_xy(game, unit, skill, mode, x, y, reenter = 0)`
   (`0x005809D0`), return 0. State 1: server message 0x5A to the unit's
   own client (`0x00549A60`: client `0x005531C0`, send `0x0053C850`),
   40 bytes: `5A 0E 01` then 37 zero bytes; return 2. Other states: 2.

`use_state(unit, entry)` (`0x00647960`) tests in this order and returns
at the first failure (entry null → fatal assert):

| # | Test | Code |
|---|---|---|
| 1 | entry's skill record missing, or its `InGame` flag clear | 3 |
| 2 | `skill_level(unit, entry, 1)` = 0 (`0x006442A0`) | 7 |
| 3 | `aura` | 6 |
| 4 | `passive` | 5 |
| 5 | quantity and throw `0x00647640`, then item type `0x00643F80` | 2 |
| 6 | mana `can_afford` `0x00647540` (`skills/levels.md` §4) | 1 |
| 7 | shape `0x00644060` | 4 |
| 8 | start stat `0x006440F0` | 1 |
| 9 | charges `0x00647840` | 2 |
| 10 | cooldown `0x006478F0` (§6) returns 0 | 8 |

Else 0. So `InGame` is read on the server, inside `use_state` (no
message handler reads it). The step 4 fallback looks Attack up with
`0x006439B0`(ECX unit, EDX id 0, item GUID −1): the first entry of the
unit's skill list (+0xA8, list +4, next +4) whose record id equals the
id and whose owner item GUID (+0x34) equals the argument; none → null,
and `use_state(null)` is fatal.

### 3. `use_on_unit(game, unit, skill, type, guid, run)` = `0x00549BA0`

1. Dual wield as §2 step 2; `use_state` with the `AttackNoMana` fallback.
2. Any non-zero state → 2 (no message). Skill mode 0 → 2.
3. Without `TargetItem`, `type` must be 0, 1 or 3; else 2.
4. Find the target; if it has state 143 (`attached`) and an owner, target
   the owner.
5. `run = 0` or target not found → `set_mode_target` (`0x00580A70`; a
   missing target changes nothing).
6. Else by `range(skill, unit)` (`0x00645460`: 0 none, 1 h2h, 2 rng, 3
   both, 4 loc; `both` is rng with a bow or crossbow equipped, else h2h;
   a player with range 2 and state mask 0x26 is treated as h2h):
   - h2h in melee range (`0x00622C40`) → use now;
   - h2h out of range, or loc → run (mode 3) to the target and remember
     the skill (`0x00548A50`); it fires on arrival (`sim/units.md`);
   - other → use now.

1.14d `range` counts: none 271, h2h 43, rng 41, both 2, loc 0.

### 4. Mode change gates

`set_mode_xy` / `set_mode_target` with `reenter = 0` first run:

**`can_change_mode` `0x0057EDD0`** (ECX game, EAX unit, EBX new mode):
cursor item held and new mode not 0/17 → no. New mode 0, 1, 5, 17 →
yes. Else by current mode: NU, WL, RN, TN, TW, S2, S4, KB (19) → yes;
DT, GH, BL, DD → no; A1, A2, SC, TH → yes if `frame ≤ E + 5` or the new
mode is GH or BL; KK → `frame ≤ E + 5`; S1 → no for an Amazon (class 0),
else yes; S3 → no for a Druid (5), else yes; SQ → yes if the used skill's
`seqinput` > 0, else `frame ≤ E + 5`. `E` = smallest positive expire
frame of the unit's type-1 timers, 0 if none (`0x005415A0`).

**`interrupt_gate` `0x0057EEC0`:**

1. State 54 or mode 0 / 17 → no. New mode 0 → yes. No used skill or
   record → yes.
2. Same skill object again and its `srvdofunc` is 67 (Charge) or 76
   (Whirlwind) → yes.
3. Used skill without `interrupt`: current mode NU → set NU (reenter 1)
   and yes; else only new modes A1, A2, SC, TH, S1, SQ with `frame ≤ E +
   5`.
4. Used skill with `interrupt`: if state 42 (concentration) is on, draw
   `roll(100)` (unit seed, `0x0045C390`); `r < stat 164` of state 42's
   stat list → blocked. Not blocked and state 15 (concentrate) on →
   blocked. Blocked: current mode NU → set NU, yes; else no.

Then the used skill is set (`0x00620210`) and the mode's start runs
(`0x0057FE90` point, `0x0057FEF0` target; table `0x006E1740`, 8-byte
pairs): set mode (`0x00553570`), clear target, `0x005533D0`, delete the
unit's type-0/1 timers (`0x00553990`), schedule frame events
(`0x005539B0`, §5.2), clear unit flag 0x40 (+0xC4), then **start**
(§5) in the same tick.

### 5. Start and do

#### 5.1 Mode of a skill

Fixed when the skill entry is created (entry +8): players `anim`; the
Assassin's Left Hand Swing 16 (S4); monsters `monanim`; item-charge
skills `anim` if A1/A2/SC/TH/SQ, else SC. 1.14d `anim`: SC 141, SQ 116,
A1 48, none 23, S2 9, S1 6, TH 5, S3 4. `seqtrans`, `seqnum` and
shapeshift mode conversion are animation (`sim/units.md`).

#### 5.2 Frame events

`0x005539B0` schedules a type-0 timer at each animation frame whose
frame code `c` is 1–4 (args `c`, running index for 1, 2, 4; 0 for 3),
then the type-1 ENDANIM timer (`sim/units.md` owns frame data). The
player type-0 handler `0x005811D0` dispatches by mode (table
`0x00732C10`); attack-type modes (7, 8, 10–16, 18) use `0x00580460`:

1. Store arg2 in unit +0x38 bits 8+ (`0x006212C0`).
2. Skill flags bit 0 (moving skills): step the path (`0x00553490`,
   `0x00554CA0`); finished (2) → flags |= 2 and run the do.
3. Otherwise run the do only if unit flag 0x40 is clear and arg1 ∈ {1,
   2}.
4. Return 1, or 2 when the unit died (ENDANIM runs at once).

Monsters: start `0x005A75C0`, per-frame `0x005A7670` (monsters branch;
Open question 6).

#### 5.3 Start `0x0056FAF0` → core `0x0056F640`

1. No used skill → return 0 (no neutral reset).
2. Target checks `0x0056CC60`: no target → pass. Skill id invalid →
   fail. With `TargetableOnly` the target must be hostile (`0x00554200`),
   a pet (`0x005542C0`) or an ally (`0x00554D20`), else fail. A target
   without unit flag 2 is dropped (pass). The corpse rule tests only
   "monster in mode 12": such a target is kept with `TargetCorpse`, else
   dropped; **any other target** (players in mode 0 or 17 included) is
   dropped when the skill has `TargetCorpse`. Dropping clears the unit's
   target (`0x00620C10`) and passes. Failure: player → mode 5
   (neutral), return 0. Skill id invalid after step 3 → return 0.
3. `L = skill_level(unit, entry, 1)`. Item skill with 0 charges → return
   0 (no neutral reset; Edge case 3).
4. Target is an ally and the skill lacks `TargetAlly` → return 0 (no
   reset).
5. No `InTown` and the room is in town → clear the used skill; player →
   neutral; return 0.
6. Core (`noManaCheck = 0`):
   1. Room required.
   2. Mana check `0x0056C160`: non-players pass; item skills need
      charges (+0x38) > 0; else `(mana + max(L − 1, 0) × lvlmana) <<
      manashift ≤ mana(8)`; `srvdofunc` 116 while shapeshifted is free.
   3. `InTown` again.
   4. `lineofsight` (+0x18F) ≠ 0: target position (`0x0056D2C0`); no
      position → the test is skipped (passes). Else by value 1–5 the
      line test `0x00645950` with collision mask 4 / 0x1C09 / 0x180 /
      0x804 / 0x805; failure → 0; value > 5 → 0. (1.14d: only value 4,
      48 skills.)
   5. `srvstfunc` ∉ 0…90 → 0. Null entry → result 1, nothing charged
      (step 7 still runs).
   6. Else `r = srvst[srvstfunc](game, unit, skill, L)` (table
      `0x00732140`; ECX game, EDX unit). `r ≠ 0`, no `usemanaondo` and
      `noManaCheck` = 0 → `consume_mana` (`skills/levels.md` §4; result
      ignored).
   7. `r ≠ 0` and `periodic` → delete type-8 timers with arg1 0.
7. Player and result 0 → neutral (cancels the frame events).

#### 5.4 Do: wrapper `0x0056FC50` → core `0x0056F7F0(game, unit, skill, L, charge = 1, item = 0, aim = 0)`

1. Player or monster (no death test here), used skill without flags
   bit 0 (`0x006446A0`), skill without `InTown`: no room or room in town
   (`0x0061AB00`) → clear used skill, player → neutral, 0. Other unit
   types skip steps 1, 2 and 4.
2. `item = 0`: pass when the used skill has this id and
   `skill_level(unit, used, 1)` > 0 (with bonuses); else the entry
   `0x006439F0`(unit, skill) must exist with `skill_level(…, 1)` > 0,
   else 0.
3. `srvdofunc` ∉ 0…190 → 0.
4. Living unit and `usemanaondo` → mana check (§5.3 6.2); fail → 0.
5. `aim` and `item` and `ItemEffect` (i16 +0x16A) > 1 → do index =
   `ItemEffect`. This index is **not** bounds-checked (the table has
   191 entries): a value > 190 reads past the table.
6. `r = srvdo[index](…)` when the entry is non-null (table
   `0x007322B0`).
7. `srvmissile` (i16 +0x46) ≥ 0 and its `missiles` row exists
   (`0x0046ACE0`): set unit flag 0x40 (later code-1/2 events of this
   animation skip); offset and aim 0 unless `item` and `aim` and the
   target position (`0x0056D2C0`) has both coordinates ≠ 0 (then offset
   = target − unit, aim = 2 × target − unit); create the missile (`lob`
   → `0x0056EE90`, else `0x0056ECB0`; §5.5) with (unit, skill, L,
   offset, aim, 0); `r = 1` whatever the helper returned.
8. `r = 0` → 0.
9. `charge ≠ 0`: living unit: if the start entry is null or
   `usemanaondo` → `consume_mana`; `decquant` → `0x0056C3F0`. `d =
   eval(delay)` (+0x190, `0x00646CA0`); `d > 0`, player, and (the
   skill's `anim` (+0x10) ≠ SQ (18) or unit +0x38 bits 8+ = 0) →
   `set_delay(d)` (§6).
10. Return `r`. The wrapper then calls `schedule_periodic(…, aura = 0)`
    (§7).

So **mana is charged at start** when the skill has a start function and
no `usemanaondo`, **otherwise at do**. 1.14d data: 274 skills have a
do function, 112 a start function, 48 only `srvmissile`, none both
`srvdofunc` and `srvmissile`; `usemanaondo`: Plague Javelin, Blade Fury.

#### 5.5 Skill missile helpers (`0x0056ECB0`, `0x0056EE90`)

(ECX game, EDX missile class; stack unit, skill, L, dx, dy, aim x, aim
y, take ammo). Both fill a zeroed `missiles.md` §R2.1 record: owner :=
unit, class, skill, L; target := (aim x, aim y) when both are ≠ 0, else
the unit's target position (`0x0056D2C0`; none → return null, no
missile). If take ammo ≠ 0 and the unit is a player: `0x0056C3F0` ≤ 0 →
return null. Then create (`0x0059FA30`) and return the missile.

| | `0x0056ECB0` (straight) | `0x0056EE90` (`lob`) |
|---|---|---|
| flags | 0x21 (start given, target absolute) | 0x420 (target absolute, frames from distance) |
| origin unit (+0x08) | none | the unit |
| start x, y | unit position + (dx, dy) | filled the same way, but unused (flag 1 clear: start = origin) |
| target found with a 0 coordinate | return null | used as is |
| attack bonus | monster with stat 19 (`tohit`, total) ≠ 0: +0x48 := it, flag 0x1000 | none |

### 6. Cooldown

`set_delay(game, unit, d)` = `0x0056EF90` (players only, `d ≠ 0`): end
`e = frame + d`. No stat list for state 121 (`skilldelay`): allocate one
(flags 2, expire `e`, owner unit), state 121, remove callback
`0x0056E900`, attach, switch state 121 on. Set the list's expiry to `e`.
Schedule a type-12 timer at `e` (another one per call).

`use_state` returns 8 when a player has state 121 and the requested
skill's `delay` formula is > 0 at its current level (`0x006478F0`). The
cooldown is shared by all skills with a delay. Timer type 14
(REMOVESKILLCOOLDOWN) is never scheduled in 1.14d; expiry is the type-12
handler `0x00580800`.

### 7. Periodic skills and auras

`period(skill, L)` = `0x0056CD50`: `d = eval(perdelay)` (+0x128); `d ≤ 5`
→ 5; expire = `((frame + d − 1) / d) × d + 1` (signed). Timers land on
frames ≡ 1 mod `d`.

`schedule_periodic(game, unit, skill, L, aura)` = `0x0056CDA0` (only for
`periodic` or `aura` skills): aura form → delete type-8 timers with arg
−1, schedule type 8 (−1, 0) at `period`; else delete type 8 with arg
`skill`, schedule type 8 (`skill`, `L`).

Type-8 handler `0x0056FCB0`(game, unit, arg1):
- arg1 = −1: the right skill entry (`0x006201D0`); it exists, its id is
  valid, it is `aura` and the unit is not dead (`0x005541B0`) → L :=
  `skill_level(unit, entry, 1)`, do core (skill, L, 1, 0, 0), then
  `schedule_periodic` in aura form; else delete the unit's type-8
  events with arg −1.
- arg1 = 0 or < −1: nothing (no do, no delete, no reschedule).
- arg1 > 0 (a skill id): id < skills count, record present, `aurastate`
  (+0x80) a valid state, the unit has that state, its list exists
  (`0x006256B0`), the unit owns the skill (`0x006439F0`) and the list's
  base stat 350 = the id → L := the list's base stat 351, do core
  (skill, L, 1, 0, 0), `schedule_periodic` (skill, L, aura 0). Any
  failure → delete type-8 events with arg1. No death test on this
  path.

0x3C SelectSkill → `assign(…)` `0x005701B0`: size 9; skill = low 31 bits
of u32 at +1, left = bit 31, owner GUID = u32 at +5; the skill must exist
for that owner with level (bonus) ≠ 0, else 3. Replacing an `aura` right
skill frees its aura-state list, switches the state off, deletes type-8
(−1) timers. A new `aura` right skill: `immediate` (10 skills: Might,
Resist Fire, Resist Cold, Resist Lightning, Defiance, Blessed Aim,
Concentration, Vigor, Fanaticism, Salvation) → run the do core once now;
else switch on its aura state (stats 350/351). Then schedule in aura
form. Left/right init: `0x00622F10` / `0x00622EA0`.

Aura do (`srvdofunc` 65, `0x005CF010`): body in `skills/bodies.md`
§4.5 (duration, stats on self and targets, `aurafilter` scan, mana and
state 85).

Type-9 handler `0x0056FE40`: `sim/stat-lists.md` §10.3 (skill = arg2,
L = total of stat 151 at layer = skill; do core (…, 1, 1, 0)). Type-5
handler `0x0056D790`: calls `srvdo[states.srvactivefunc]` of the
skill's aura state **directly** (ECX game, EDX unit; skill = arg1, L =
arg2), not through the do core: no charge, item, aim, mana, missile or
delay step (1.14d: hurricane 145, armageddon 146, attached 147).

Passives: learning (0x3B, `skills/levels.md` §6.4) and refreshes call
`0x0056DE40` → `0x00646F20`; stat math belongs to `sim/stats.md`.

### 8. Function tables

`skills/functions.tsv`, columns: `kind` (srvst / srvdo), `index`,
`address` (1.14d, `null` = empty slot), `d2moo_name`, `status`
(`spec'd-here` = body specified in `skills/bodies.md`; `mapped` = 1.14d
table entry identified, body not specified; `null`; `unreferenced` = no
1.14d data uses it), `skills_using` (from `skills.txt` 1.14d), `notes`. Ranges `66-90` and `153-190` are one row
each (all null).

- srvst `0x00732140`: 91 slots (bound < 0x5B), 64 non-null (1–29,
  31–65).
- srvdo `0x007322B0`: 191 slots (bound < 0xBF), 152 non-null (1–152).
- There is no separate `srvprgfunc` table: the progressive finisher
  `0x005D5220` indexes the srvdo table with `srvprgfunc1–3`.
- srvdo 145–147 are `states.srvactivefunc` targets; 36 and 151 are
  `ItemEffect` targets.

Contracts: start (ECX game, EDX unit, stack skill, level) → 0 refuses.
Do: same plus the core's arguments; return 0 = nothing happened.

## Constants & data dependencies

`skills` offsets: srvstfunc +0x2C, srvdofunc +0x2E, srvprgfunc1–3
+0x30/+0x32/+0x34, srvmissile +0x46, anim +0x10, range +0x14, seqinput
+0x16, aurastate +0x80, perdelay +0x128, weapsel +0x168, ItemEffect
+0x16A, startmana +0x184, minmana +0x186, manashift +0x188, mana +0x18A,
lvlmana +0x18C, lineofsight +0x18F, delay +0x190; flag dwords +4/+8
(bits from `fields.tsv`: decquant 0, lob 1, aura 5, periodic 6, InTown
8, InGame 10, durability 18, TargetableOnly 20, TargetCorpse 24,
TargetAlly 26, TargetItem 27, AttackNoMana 28, interrupt 31,
usemanaondo 37). Mask table `0x006CE268`.

1.14d `skills.txt` (357 rows): InGame all; interrupt 335; InTown 63;
aura 26; passive 26; AttackNoMana 41; TargetableOnly 45; immediate 10;
periodic 2 (Thunder Storm, Blade Shield). `delay`: Poison Javelin 15,
Plague Javelin 100, Immolation Arrow 25, Valkyrie 150, Fire Wall 35,
Meteor 30, Blizzard 45, Hydra 40, Frozen Orb 25, Fist of the Heavens 25,
Firestorm 15, Molten Boulder 50, Eruption 50, Volcano 100, Armageddon
150, Hurricane 150, Shock Field 15, Blade Sentinel 50, Shadow
Warrior/Master 150, Dragon Flight 25, Wearwolf/Wearbear 25.

## Randomness

The pipeline draws once, in `interrupt_gate` (§4 step 4): `roll(100)` on
the acting unit's seed, only when a mode change with `reenter = 0` is
requested by a living, interruptible unit whose used skill has
`interrupt` and which has state 42 (and the request is not a Charge /
Whirlwind repeat). Start and do functions, missiles and combat draw on
their own (`combat/*`, `skills/levels.md`). The unit-seed reseeder
`0x00645390` has no caller.

## Edge cases & original bugs

1. Hold handlers return 0 even when the use fails.
2. `pierce_idx` increments on every skill message, failed ones included.
3. Start refusals "item skill with 0 charges" and "ally without
   `TargetAlly`" return 0 without the neutral reset; the animation and its
   do timer still run (Open question 3).
4. A failed `consume_mana` is ignored: the skill still executes (mana
   spent between start and do, or the `minmana` clamp).
5. `can_afford` ignores `minmana`, consume applies it (Teleport level ≥
   25 at < 1 mana: Open question 4).
6. Blood mana (state 114): `use_state` compares life, the start/do check
   `0x0056C160` compares mana.
7. A1/A2/SC/TH/KK/SQ animations can be replaced while their ENDANIM
   timer is pending (`frame ≤ E + 5`); the recordings show attacks
   chained one frame before ENDANIM.
8. In SQ animations the delay is set only on the first event.
9. With unit flag 0x40, only the first missile event per animation fires
   `srvmissile`.
10. Unit-target messages refuse objects (type 2) unless `TargetItem`.
11. Server accepts any skill on the left button (`leftskill` unread).

## Test vectors

| Case (1.14d data) | Expected |
|---|---|
| Fire Bolt L1 (5, 0, shift 7, minmana 1, no start/do, srvmissile) | check 640; charged at do: 640; missile; flag 0x40 |
| Fire Ball L10 | (10 + 9) << 7 = 2,432 at do |
| Teleport L1 / L20 / L25 / L30 | check 6,144 / 1,280 / 0 / −1,280; consume 6,144 / 1,280 / 256 / 256 |
| Blade Fury L1 (8, +1, shift 5, startmana 6, usemanaondo) | request needs ≥ 256 and startmana 1,536; start charges 0; each do checks and charges 256 |
| Multiple Shot L10 (srvst 4; 4, +1, shift 8) | 3,328 charged at start |
| Meteor (delay 30), do at frame F | state 121 until F + 30, type-12 timer at F + 30; any delay skill → state 8 meanwhile |
| Might (perdelay 50, immediate) assigned at 1234 | do now; type 8 at 1251, then 1301, … ; assigned at 1250 → 1251 |
| Dual wield, three Attack requests | Left Hand Swing, Attack, Left Hand Swing |
| Recording 021854: request at frame 886 | type 0 (1, 0) expiring 892, type 1 expiring 900; do at 892; ENDANIM cancelled at 899 by the next request |

## Provenance

- 1.14d disassembly; tables read from the image: srvst `0x00732140`,
  srvdo `0x007322B0`, mode starts `0x006E1740`, player per-mode handlers
  `0x00732C10`. Function identification: same null pattern as D2MOO's
  tables, 1.14d addresses in D2MOO file order (exceptions: srvst 18
  Attract is a `return 1` stub; srvdo 121 Rabies out of order); 24 table
  targets lack a Ghidra function (Open question 1).
- D2MOO 1.10f `PlrMsg.cpp`, `PlrModes.cpp`, `Skills.cpp`, `D2Skills.cpp`
  for names. 1.14d differences: hold handlers simplified; type 14 unused
  (cooldown via state 121 + type 12); consume charges at level 1; aura
  state functions 145–147 named "unused" in D2MOO.
- Recordings `20261006-021854-tick.jsonl`, `20261006-022304-tick.jsonl`,
  `20261006-015956-packets.jsonl` (timer sequences, hold-message
  spacing); event types 8 and 14 never ran (no aura or periodic skill
  used).

## Open questions

1. Answered: the 24 table targets missing from the Ghidra export
   (0x0056CAB0, 0x0056CBA0, 0x0056CC20, 0x005C3070, 0x005C31C0,
   0x005C3260, 0x005C3270, 0x005C3350, 0x005C4CD0, 0x005CAF80,
   0x005CB270, 0x005CB4D0, 0x005CBBF0, 0x005CC1D0, 0x005CC220,
   0x005CDF00, 0x005D0180, 0x005D1BF0, 0x005D2B20, 0x005D32F0,
   0x005D6330, 0x005D80C0, 0x005D8760, 0x005DA8B0) were read from the
   raw disassembly (`tools/ghidra/disasm.py at`, first checked on
   0x0056CAB0 and 0x005DA8B0); every one is `spec'd-here` in
   `functions.tsv` (open question 10), so no re-export is needed.
2. Answered: srvdo slot 121 holds `0x005C8AD0`, and Rabies (id 238) is
   the only 1.14d `skills.txt` row with `srvdofunc` 121; its body is
   `skills/bodies-2.md` §6.14. The 1.14d body does not follow D2MOO's
   SrvDo121 (used-skill param check, `0x005C8980`, `apply_melee`
   `0x0057D4F0`, `elem_len` `0x00644F20`, `0x005C7DB0`, `0x005C7C20`).
3. Recording: hook `0x0056FAF0` entry/return and `0x0056F7F0` entry; cast
   a non-`TargetAlly` skill on a party member: does the do still run after
   start returned 0?
4. Recording: hook `0x0056BFE0` (ECX game, EDX unit, return); Teleport at
   level ≥ 25 with < 1 mana: is the cast free?
5. Injection or recording: two 0x06 messages two frames apart: does the
   second restart A1?
6. Monster per-frame `0x005A7670` tests unit +0x4E = 1 or mode 14, not
   the event argument: hook it, log arg1/arg2 and +0x4E (monsters branch).
7. Frame codes 1–4 of `0x005539B0`: log arg1 per type-0 timer for a
   multi-hit animation (Strafe, Zeal) (`sim/units.md`).
8. Answered from `traces/raw/20261006-022304-tick.jsonl`: a shrine. In
   the client-message phase after frame 1715 the object GUID 34
   (`objects.txt` class 2, `Shrine`) gets timers type 6 at 2015 and
   type 5 at 7716, and the player a type-12 (state expiry) timer at
   4115 = 1715 + 2400, the `shrines.txt` duration of Armor, Combat,
   Skill and Recharge Boost. Which of the four is not recorded; not a
   skill rule.
9. Answered: the 40-byte layout sent by the skill path is in §2 step 6
   (`5A 0E 01`, rest zero); its field names belong to
   `sim/server-messages.tsv` (row 0x5A, layout empty).
10. Answered: every `functions.tsv` row is `spec'd-here` (bodies in
    `skills/bodies.md`, `bodies-2.md`, `bodies-3.md`, `bodies-4.md`),
    except the null slots and the 3 `unreferenced` rows.
