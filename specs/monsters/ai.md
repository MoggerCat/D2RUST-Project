# Spec: Monsters — AI think

- **Status:** draft: think scheduling, dispatch, AI tables and the
  shared helpers read from the 1.14d `Game.exe` (addresses below; AI
  tables dumped from the file); 37 AI functions read in full; think
  intervals checked against 4,409 recorded type-2 runs in three tick
  recordings (mode-end re-think: 79/79 at aidel; no rule contradicted).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::monsters::ai` (dispatch, helpers, per-AI
  functions); the catalogue `specs/monsters/ai-functions.tsv` is embedded
  with `include_str!`
- **Related specs:** `sim/tick.md` §5 (timer queue, event type 2, the
  monster handler and its freeze gate); `sim/rng.md` §3, §6, §7 (helpers,
  inline draws, unit seed); `sim/units.md` (unit modes, mode-change
  machinery, mode start/end functions, movement and pathing, positions,
  collision; being written on `claude/phase3-units`); skills spec
  (skill execution, `Sk*mode` sequences, damage; `claude/phase3-skills`);
  `monsters/init.md` (AI control allocation and the first AI install at
  creation, monumod flags such as teleport); `monsters/population.md`
  (minion lists, packs, summon-by-table spawns);
  `sim/intents-events.md` (NPC interaction messages);
  `data/fields.tsv` (monstats, monstats2, levels, objects layouts).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–71 |
| Inputs | 72–83 |
| Outputs / state changes | 84–94 |
| Rules | 95–96 |
|   1. Think scheduling | 97–242 |
|   2. Think dispatch `0x005B1740` | 243–372 |
|   3. AI control and AI tables | 373–544 |
|   4. AI parameters | 545–563 |
|   5. Target selection | 564–695 |
|   6. Distances and line tests | 696–710 |
|   7. Tactics helpers | 711–828 |
|   8. AI commands and minions | 829–853 |
|   10. The catalogue `ai-functions.tsv` | 854–874 |
| Constants & data dependencies | 875–898 |
| Randomness | 899–920 |
| Edge cases & original bugs | 921–962 |
| Test vectors | 963–1051 |
| Provenance | 1052–1108 |
| Open questions | 1109–1212 |
<!-- /index -->

## Summary

Every monster thinks through timer events of type 2 (AI think). A think
runs the monster's event handler `0x005B1740`, which builds a small
parameter record, runs three prechecks (stun, doors, minion leash;
target acquisition; boss sound, teleport, special walk) and then calls
the monster's current AI function, chosen once from the 148-entry AI
table by the monstats `AI` column (or from an 18-entry table while an
AI special state is active). An AI function decides with draws from the
monster's own unit seed (`rng.md` §7) and ends in one of three ways: it
starts a mode (attack, skill, walk, run), it schedules the next think
directly ("idle N frames"), or both. A mode that ends in neutral
schedules the next think `aidel` frames later if none is pending; a
walk or run that ends runs the next think at once. Monsters with no
player within 35 tiles re-think every 25 frames. This spec owns the
scheduling, dispatch, the AI control record, the shared helpers and the
per-AI decision rules; mode changes, movement, skills and damage are
owned by the specs linked above.

Split: §9 (per-AI behaviours) moved unchanged to `monsters/ai-bodies.md`
(same section numbers and rule ids; the two files share no id, so a bare
§ reference is unambiguous). Constants, Randomness, Edge cases, Test
vectors, Provenance and Open questions for both files stay here.

## Inputs

| Name | Type | Source |
|---|---|---|
| monster unit | unit type 1 | unit +0x14 = monster data (D2MOO `D2MonsterDataStrc`), +0x20 unit seed, +0x10 anim mode, +0x18 act (u8) |
| AI control | D2MOO `D2AiControlStrc` (0x40 bytes) | monster data +0x28 (§3.1) |
| AI param record | D2MOO `D2AiParamStrc` | monster data +0x2C (velocity request, §7.3) |
| monstats record | 424 bytes | row = unit class; `AI`, `aidel*`, `aidist*`, `aip*`, `Skill1..3`, `Sk*mode`, flags |
| monstats2 record | 308 bytes | row = monstats `MonStatsEx`; `MeleeRng`, mode flags |
| game | | +0x6A nGameType (u8), +0x6D difficulty (u8), +0x74 dwGameType, +0xA8 frame, +0x10F8 target-node lists (§5.2) |
| AI tables | .rdata | `0x0073CA18` (148 × 16 bytes), `0x0073D358` (18 × 16 bytes) |

## Outputs / state changes

- Type-2 timer events scheduled and cancelled (`tick.md` §5.2, §5.4).
- Mode changes requested through the unit mode machinery (`sim/units.md`),
  with a target unit or target coordinates, and path step counts.
- The AI param record's velocity request (§7.3).
- AI control fields: flags, three scratch params, command list, current
  AI function and special state.
- Monster sounds (sound ids 16, 17, 18; owned by `sim/units.md`).
- Draws from the monster's unit seed, in the order each rule gives.

## Rules

### 1. Think scheduling

#### 1.1 The event

A think is a timer event of type 2 on a monster (D2MOO `EVENTTYPE_AITHINK`),
args 0, 0, default handler. The monster class handler `0x005A7F80`
dispatches it to `0x005B1740` through table `0x006E2490` entry 2, but
drops it without running when the monster has state 1 (D2MOO
`STATE_FREEZE`) and is not dead (`0x005541B0`); `tick.md` §5.6. A dropped
think is not rescheduled by the dispatcher. The freeze itself schedules
the next think twice:

1. Freeze apply `0x0057B230` (combat spec owns the length): delete the
   monster's thinks and schedule one at frame + len + 1 (§1.2), len the
   new length (after the monster divisor); the freeze stat list's
   expiry (timer type 12) is set to max(old expiry, frame + len). A
   longer freeze already running makes the len + 1 think land while
   still frozen, so the gate drops it.
2. Freeze end: the freeze stat list's remove callback `0x0057B170`
   (registered with `0x00625CE0` by the apply) does nothing when the
   unit is dead and `0x0063A4A0(unit, 1)` holds; otherwise state 1 off,
   unit refresh (`0x00553570`), and for a monster: delete its thinks and
   schedule one at frame + `aidel` with the column rule of §1.3 rule 1
   (0 → 15; no stun override). 1.14d-confirmed (`0x0057B170`,
   `0x0057B230`); D2MOO `SUNITDMG_RemoveFreezeState` is the same.

When a think is scheduled for a monster that has state 54 (D2MOO
`STATE_UNINTERRUPTABLE`), `0x005544B0(unit, 0)` runs first
(`tick.md` §5.2 rule 4): it clears state 54, handles state 92 for
players only (the branch needs unit +0x80), and for monsters cancels all
the monster's type-2 events. 1.14d-confirmed (`0x005544B0`).

Most schedulers call "delete the unit's type-2 events, then schedule one
at frame + N" (`0x00540E60(2, 0)` then `0x005417D0(2, frame + N)`), so a
monster normally has at most one pending think. Schedules that do not
delete first are marked "add" below.

#### 1.2 Who schedules a think (monsters)

Delays are frames after the current frame (game +0xA8).

| Site (1.14d) | D2MOO counterpart | Delay | When |
|---|---|---|---|
| `0x005DE080` | `AITACTICS_IdleInNeutralMode` | N (0 → 1) | AI "idle N": if the anim mode is not neutral, first a mode change to neutral with the unit itself as target; then delete + schedule |
| `0x005DE0F0` | `AITACTICS_Idle` | N (0 → 1) | delete + schedule, mode unchanged |
| `0x005DE130` | `sub_6FCD0150` | N (0 → 1) | only if no think is pending, or the pending one is due this frame, or it is due at frame + N or later |
| `0x005DE9D0` | `sub_6FCCFC00` | 20 | target modes 4/5 found nothing (§2.3) |
| `0x005A73E0` | `sub_6FC64310` (neutral-mode start) | aidel, or 45 | add; only if the pending think's frame is ≤ the current frame (none pending reads as 0): 45 if the monster has state 21 (D2MOO `STATE_STUNNED`), else `aidel` of the difficulty (§1.3) |
| `0x005A8520` | `sub_6FC64CD0` (knockback-mode end) | 1, or 15/45 | add; if the class has no gethit mode (`0x0046C140(class, 3)` false): +1; else base class 78 (sandleaper1): 45 if stunned else 15; else a mode change to gethit (3), last-hit class 160, no think here |
| `0x00573780` | `MONSTER_UpdateAiCallbackEvent` | 2 | delete all thinks; if the anim mode is neutral (1), schedule +2; if death (0) or dead (12), stop; base classes 110, 118, 136, 247 (vulture1, willowisp1, batdemon1, frogdemon1) also get delete + 2 in other modes |
| `0x00548B00`, `0x0054CA10` | NPC interaction (D2MOO `PlrMsg`) | 1 | units whose monstats `npc` and `interact` bits are both set: stop the path; `0x00548B00` also sets AI param 0 (control +0x14) := 40 (`0x0058EC00(unit, 1, 40)`); delete + 1 |
| `0x0056D940` | AI special-state install for summons | 25 | after installing a special state (`0x005B0E00`) and `0x00573780` |
| `0x005DDD00` | special state 11 install | 1 | |
| `0x005E8140`, `0x005E8340` | special-state AIs 10/17 and 12 | 1 | when the special state ends |
| `0x0057B230` | `SUNITDMG` freeze apply | freeze length + 1 | delete + schedule |
| `0x005A3910` | `MonsterUnique` | 51 | |
| `0x005C07A0`, `0x005C55C0`, `0x005D1350`, `0x005D6020`, `0x005D6E70`, `0x005D8570`, `0x005DC1E0` | skill functions (summons, taunt, etc.) | 25, 15, 13, 1, 20, 1, 20 | owned by the skills spec |

The callers of `0x005417D0` with event type 2 in 1.14d are exactly the
27 sites in `0x00542B40`, `0x00548B00`, `0x0054CA10`, `0x0056D940`,
`0x00573780` (2), `0x0057B230`, `0x005858A0`, `0x005A3910`, `0x005A73E0`
(2), `0x005A8520` (2), `0x005C07A0`, `0x005C55C0`, `0x005D1350`,
`0x005D6020`, `0x005D6E70`, `0x005D8570`, `0x005DC1E0`, `0x005DDD00`,
`0x005DE080`, `0x005DE0F0`, `0x005DE130`, `0x005DE9D0`, `0x005E8140`,
`0x005E8340`; `0x00542B40` and `0x005858A0` schedule type 2 on
objects (object spec). No caller uses `0x00541800` or `0x005416B0` for
type 2.

#### 1.3 Mode-end re-think and `aidel`

When a monster mode ends and the unit returns to neutral, the neutral
mode's start function `0x005A73E0` (mode table `0x006E2260`, mode 1,
first column; `sim/units.md` owns the table) sets combat mode 1 and then
schedules a think as in §1.2. The delay:

1. `aidel` column by difficulty: if game +0x6A and game +0x74 are both 0,
   the Normal column (`aidel`, monstats +79) is used whatever the
   difficulty; otherwise `aidel`, `aidel(N)`, `aidel(H)` (+79, +80, +81)
   by game +0x6D. 1.14d-confirmed (`0x005A73E0`).
2. A value of 0 becomes 15.
3. State 21 (stunned) overrides with 45.

So after an attack, a skill, a gethit or a block the next think comes
`aidel` frames after the mode ended (15 for every Act 1 monster of the
recordings). The AI functions that start such modes do not schedule a
think themselves. A mode whose start fails also falls into
`0x005A73E0` (`0x005A7C20`), so a failed move leaves an `aidel` think
unless the caller deletes it (§7.2, flag 4).

#### 1.4 Inline thinks

Some mode ends run the think handler directly (same freeze gate as the
class handler, no type-2 event): `0x005A8030`, the end function of modes
3–9 and 14, looks up byte table `0x0073C6D0` by the mode that ended:

- modes 2 (walk) and 15 (run) have value 1: the unit's anim mode is set
  to neutral and the think runs at once, unless the monster is frozen and
  alive;
- if the monstats `SplEndGeneric` byte (+422) is nonzero and the base
  class matches, the think also runs inline: base 110 (vulture1) at the
  end of mode 8, 247 (frogdemon1) mode 14, 136 (batdemon1) modes 10/11,
  230/231 (firebeast, iceglobe) any mode, 118 (willowisp1) mode 2, 403
  (trappedsoul1) any mode;
- every other case requests a mode change to neutral (which schedules
  through §1.3).

So a monster that walks or runs re-thinks the frame its path ends.
1.14d-confirmed (`0x005A8030`, table `0x0073C6D0` = 00 00 01 00 … 00 01).

#### 1.5 First think and player arrival

1. Creating and placing a monster (`0x00554850`, called from
   `0x005394A0`, `0x005424F0`, `0x00542B40`) sets its mode to neutral,
   which schedules a think at frame + `aidel` (§1.3), then calls
   `0x00573780`, which deletes it and schedules frame + 2. Recorded: all
   574 think schedules made in the room step (tick step 3) of the three
   recordings are 287 such pairs "+15, cancel, +2", made before the
   unit's `hin` record.
2. When the first client enters a room (room +0x78 < 2,
   `0x0053A8E0`), every monster in that room's unit list gets
   `0x00573780` (+2 if neutral). Recorded: all 61 "step clients"
   schedules are +2.

#### 1.6 AI reset (event type 10)

The monster handler for type 10 (`0x005A7F70`) only calls `0x00573120`,
which clears monster data +0x34 and +0x38 (D2MOO `unk0x34`, `unk0x38`).
It schedules nothing and draws nothing. Which systems schedule type 10
on monsters is owned by the skills spec (D2MOO `SkillNec.cpp`).
1.14d-confirmed (bytes at `0x005A7F70`).

#### 1.7 Resulting think rhythm

Rule summary, with the recorded counts in Test vectors:

| Situation | Next think |
|---|---|
| target mode 1, no target, nearest same-act non-town player at distance d | d ≥ 35: 25; 25 ≤ d < 35: d − 10; d < 25: 10 (§2.3); no player: 25 |
| target modes 4/5, no target | 20, mode unchanged |
| AI chose "idle N" | N |
| AI started an attack/skill/gethit-ending mode | `aidel` (Normal 15) after the mode ends |
| AI started a walk/run | the frame the path ends (inline) |
| AI Idle (index 1, 100) | 200 |
| move start failed, flags without 4 | `aidel` |
| player enters the room, monster neutral | 2 |

### 2. Think dispatch `0x005B1740`

#### 2.1 Tick parameter record

The handler (game in ECX, unit in EDX) builds a 9-dword record on the
stack (D2MOO `D2AiTickParamStrc`) and passes its address to every
precheck and to the AI function:

| Offset | Field | Initial value |
|---|---|---|
| +0x00 | pAiControl | monster data +0x28 (0 for a non-monster) |
| +0x04 | unk0x04 | 0 |
| +0x08 | pTarget | 0 (set by §2.3) |
| +0x0C, +0x10 | unk | 0 |
| +0x14 | nTargetDistance | 0 (set by §2.3) |
| +0x18 | bCombat | 0 (set by §2.3: target in melee range) |
| +0x1C | pMonstatsTxt | monstats row = unit class, if 0 ≤ class < count |
| +0x20 | pMonstats2Txt | monstats2 row = monstats `MonStatsEx` (+24), if in range |

If either record is missing nothing runs. Otherwise:

1. Precheck A `0x005B10E0` (§2.2). Nonzero result: stop.
2. Precheck B `0x005B1650` (§2.3). Nonzero: stop.
3. Precheck C `0x005B13E0` (§2.4). Nonzero: stop.
4. Call the AI control's current function (control +0x04) with (game,
   unit, record). A bad code pointer is a fatal assert.

1.14d-confirmed (`0x005B1740`). The 1.14d handler does not test state
54 (D2MOO only warns).

#### 2.2 Precheck A `0x005B10E0`: stun, doors, leash

In order:

1. **Stun.** State 21: idle 3 (`0x005DE080(3)`), stop.
2. **Doors** (`0x005B0F50`). If the monstats `opendoors` bit is set and
   the unit's path reports a blocked step (path flags bit 0x800,
   `0x00648EB0`): search the adjacent rooms' units with scan 8 (§5.4,
   start distance 9) for a door. If one is found and its objects.txt
   `MonsterOK` (+365) is nonzero: operate it (`0x00584540`, object type
   2, door GUID; object spec), idle 5, stop.
3. **Minion leash** (`0x005B0FF0`, D2MOO `D2GAME_AICORE_MinionLeash`).
   Only when the control's owner GUID (+0x0C) is not −1:
   - if the class cannot walk (`0x0046C140(class, 2)` false), the owner
     cannot be found (`0x00552F60`, owner type +0x10) or is dead: owner
     GUID := −1, continue;
   - d = full-size distance unit→owner (§6);
   - d ≤ 1 and owner type 0 (player): walk away from the owner 19 tiles
     with think delete (`0x005DEFE0`, §7.2); if that fails, wander near
     the owner 19 (`0x005DF530`). Stop.
   - d > 20: velocity request (method 7, speed 0, steps 40); wander near
     the owner 19. Stop.
   - else continue.

No draws except those of the helpers named. 1.14d-confirmed.

#### 2.3 Precheck B `0x005B1650`: target acquisition

Reads the AI table record (§3.2) for the unit and the control's special
state, and switches on its first dword (target mode):

| Mode | Finder | No target found |
|---|---|---|
| 0 | none (target stays 0) | continue |
| 1 | `0x005DE890` | stop (the finder already scheduled) |
| 2 | `0x005DD7F0` | continue with target 0 |
| 4 | `0x005DE9D0` | stop |
| 5 | `0x005DE9D0` | continue |
| other (3) | — | stop |

The finders write target, distance and combat into the record.

`0x005DE890` (D2MOO `sub_6FCCF9D0`), after `0x005DD7F0` found nothing:

1. If AI state is 3 or 19 (§3.1, `0x005DD2B0`) and the class can walk:
   wander 5 (`0x005DE200(5)`, §7.2), return.
2. If the unit's position collides with mask 0x40 (`0x0064D910`, D2MOO
   `COLLIDE_MISSILE`) and the class can walk: wander 5, return.
3. Else idle by the distance `0x005DD7F0` returned (nearest player,
   §5.2): ≥ 35 → 25; ≥ 25 → d − 10; else 10.

`0x005DE9D0` (D2MOO `sub_6FCCFC00`): same collision test → wander 5;
else delete thinks and schedule +20 (no mode change).

1.14d-confirmed (`0x005B1650`, `0x005DE890`, `0x005DE9D0`).

**Target 0 in mode-1 and mode-4 bodies.** Modes 1 and 4 stop the think
when no target is found, so the AI function of a target-mode-1 or -4
record (the active record: special state or base) never runs with T =
0. The "T = 0" branches of such bodies (Fetish's life of T, `ai-bodies.md` §9.21;
BloodRaven's h and raise point, `ai-bodies.md` §9.18; SkeletonBow's walk in radius,
`ai-bodies.md` §9.16; run near T) are unreachable in 1.14d, and several of the helpers
they call read T without a null test (`0x005DF680` run near,
`0x005DE4E0` walk in radius, `0x005DC480` half-size distance read T's
unit type at once): a T = 0 call there is an access violation in
1.14d. An implementation treats them as unreachable (assert), not as a
rule. Null-safe helpers: `0x00621F20` (life percent of 0 → 0, through
the null-safe stat getters `0x00625480` / `0x00625D10`) and escape
`0x005DEFE0` / `0x005DF140` (§7.2).

#### 2.4 Precheck C `0x005B13E0`: boss sound, teleport, special walk

Only when a target was found:

1. **Boss sound** (`0x005B1140`). If the monster is unique (type flag 8,
   `0x005A0180`) or a boss (`0x0063E9F0`) or class 250 (summoner), the
   target distance is < 20, the target is a player, and control flag 0x10
   is clear: play sound 16, set flag 0x10, idle 20, stop. Once per
   monster.
2. **Teleport** (`0x005B11F0`, monsters given control flag 0x20 by a
   monumod; `monsters/init.md`). Not if dead or flag 0x20 clear. Draws:
   1. `lo' % 100` ≥ 40 → continue with 3.
   2. "melee" = monstats `isMelee`, or base class 10, 345 or 557
      (bighead1, councilmember1, baalhighpriest). If life% < 30, or not
      melee and target distance < 10: `roll(100)` < 15 → find a free spot
      (`0x0054DC40`, `monsters/population.md` §8: room seed, up to 20
      x-then-y tries) in the room. If found
      and its room is not in town: if life% < 30, `roll(100)` < 25 and no
      state 52 (D2MOO `STATE_PREVENTHEAL`): add life = level × 256,
      capped at max life. Then use skill 184 (monster teleport) in mode
      4 at the spot (`0x005DEAD0`), stop.
3. **Special walk** (`MonSpcWalk`). If the monstats `isMelee` bit is set,
   the class can walk, the unit has a room, the levels.txt `MonSpcWalk`
   (+47) of its level is nonzero and smaller than the target distance,
   and the direct-line test `0x005DC640` to the target fails: scan 11
   (§5.4) for an alternative target. None: wander 4, stop. Else the
   alternative replaces target and distance; continue.

1.14d-confirmed (`0x005B13E0`, `0x005B1140`, `0x005B11F0`).

### 3. AI control and AI tables

#### 3.1 AI control record (monster data +0x28)

D2MOO `D2AiControlStrc`, 0x40 bytes:

| Offset | Field | Use in 1.14d |
|---|---|---|
| +0x00 | nAiSpecialState | 0 = none; selects the special-state table (§3.3) |
| +0x04 | pAiParamFn | current AI function; called by the dispatcher |
| +0x08 | nAiFlags (u16) | 0x08 target seen (§5.2), 0x10 boss sound done, 0x20 may teleport, 0x40 force a line-of-sight test on the next search |
| +0x0C, +0x10 | dwOwnerGUID, dwOwnerType | leash owner (§2.2); −1 = none |
| +0x14, +0x18, +0x1C | dwAiParam[0..2] | scratch per AI; cleared on install (§3.3); set by `0x0058EC00(unit, 1..3, v)` |
| +0x20, +0x24 | pCurrentCmd, pLastCmd | command ring (§8) |
| +0x28 | pGame | nonzero = minion bookkeeping active |
| +0x2C, +0x30 | dwOwnerGUIDEx, dwOwnerTypeEx | minion owner (pack leader); `0x0058F0D0` returns it |
| +0x34 | pMinionList | GUID list of this leader's minions |
| +0x38 | pMapAi | preset path nodes (town NPCs) |
| +0x3C | nMinionSpawnClassId | spawner AIs |

"AI state" (`0x005DD2B0`, D2MOO `sub_6FCF2E70`) is the monster data
`dwAiState` (+0x54) read through `0x005734E0` (a non-monster reads 1);
it is "true" for values 3 and 19, i.e. "hit recently". Its only setter
is `0x005734C0(unit, v)`, called from two places:

1. Damage reaction `0x0057CEE0` (`combat/damage.md` §7.1), monster
   defender: v := 19 when a hit starts no reaction mode: a block that
   is not played (result flag 0x4000, class 243 diablo, 333
   diabloclone or 705 uberdiablo, or a class without mode 6), a get-hit
   that §6.2 there refuses, or a soft hit (flag 0x4000 without
   get-hit).
2. Monster mode set `0x005A7C20` (`sim/units.md`), when the unit's
   current mode m (unit +0x10, the mode being left) is not 1:
   `0x005A68E0`: old state s ≥ 16 → s − 16 (so 19 becomes 3); s = 13
   and m = 3 → stays 13; else s := m.

So the state is the last non-neutral mode the monster left (3 after a
get-hit), and 19 marks a reaction-less hit until the next mode change
turns it into 3. 1.14d-confirmed (`0x005734C0`, `0x005A68E0`,
`0x0057CEE0` sites `0x0057D083` / `0x0057D119`).

**Control getter** `0x00541860(ECX unit)` (no stack argument): the AI
control (monster data +0x28) when the unit exists, is a monster (type
1) and has monster data (unit +0x14); else 0. No side effects, no
draws. Callers (6): the raise `0x005C07A0` (`combat/events.md`
function 31, §3.3 "Re-install on a fresh monster"), Hydra
`0x005CA910`, imp release `0x005D19D0`, `0x005DCA70`, `0x005E1720`,
BaalThrone `0x005EF320`. 1.14d-confirmed (`0x00541860`–`0x00541876`,
`all.asm` call sites).

#### 3.2 AI tables

Both tables are arrays of 16-byte records (D2MOO `D2AiTableStrc`):

| Offset | Field | Meaning |
|---|---|---|
| +0x00 | target mode | §2.3 |
| +0x04 | init function | called once when the AI is installed (§3.3), or 0 |
| +0x08 | think function | becomes control +0x04 |
| +0x0C | alternate function | used when the AI is re-installed while running (§3.3), or 0 |

- AI table `0x0073CA18`, 148 records, indexed by the monstats `AI` value
  (link16 at +30 into monai.txt). An index outside 0..147 uses record 0.
- Special-state table `0x0073D358` (= `0x0073CA18` + 148 × 16), 18 records.

`monai.txt` is a compile-only lookup (`data/tables.tsv`): its 148 rows
give the names of the 148 indices and nothing else. Dumped from
`Game.exe` .rdata; every record is in `ai-functions.tsv`. Target modes
used: 0 (none) 26 records, 1 (needs target) 104, 2 (optional) 16, 4 1
(SandMaggot), 5 1 (FrogDemon); counts from the dump.

Special-state records (index: target mode, init, think, alternate; D2MOO
name):

| # | D2MOO | Mode | Init | Think | Alt |
|---|---|---|---|---|---|
| 0 | none | 0 | – | – (→ 0x005B0CD0) | – |
| 1 | Idle | 0 | – | `0x005B0CD0` | – |
| 2 | SpecialState02 | 1 | – | `0x005B14E0` | – |
| 3 | SpecialState03 | 1 | `0x005E5730` | `0x005E5870` | – |
| 4 | Hireable | 0 | – | `0x005E52D0` | `0x005E5280` |
| 5 | GoodNpcRanged | 0 | – | `0x005E7AC0` | – |
| 6 | SpecialState06 | 0 | – | `0x005E7C10` | – |
| 7 | NecroPet | 0 | – | `0x005E4CF0` | – |
| 8 | TownRogue | 1 | – | `0x005E7DC0` | – |
| 9 | SpecialState09 | 1 | – | `0x005E7F80` | – |
| 10 | SpecialState10_17 (D2MOO dim vision) | 1 | – | `0x005E8020` | – |
| 11 | SpecialState11 (terror) | 1 | `0x005E80E0` | `0x005E8140` | – |
| 12 | SpecialState12 (taunt) | 0 | – | `0x005E8340` | – |
| 13 | SpecialState13 | 1 | – | `0x005E5C50` | – |
| 14 | SpecialState14 | 1 | – | `0x005E2610` | – |
| 15 | SuicideMinion | 1 | – | `0x005E1D30` | – |
| 16 | SpecialState16 | 1 | `0x005E2CD0` | `0x005E2D80` | – |
| 17 | SpecialState10_17 | 2 | – | `0x005E8020` | – |

The table lookup `0x005B15D0` uses the special-state record when the
state is nonzero, except that states 10, 11 and 12 apply only to classes
whose monstats `switchai` bit is set (`0x00623470`); otherwise the AI
table record. 1.14d-confirmed (`0x005B15D0`, dump).

#### 3.3 Installing an AI `0x005B0E00`

D2MOO `AITHINK_ExecuteAiFn(game, unit, control, state)`:

1. Fatal assert if the unit has state 54.
2. Nothing if any argument is missing, state ≥ 18 or the unit is not a
   monster. A class ≥ the monstats count forces state 1.
3. If the control already has a function, the record for its current
   state has that function as think, and the record's alternate is
   nonzero: control state := new state, control function := alternate.
   Nothing else (no reset, no init).
4. Else: AI params 0–2 := 0; free all commands (`0x0058EDE0`); take the
   record for the new state; if it has an init function, call it with a
   fresh tick record (control, monstats, monstats2; nothing if either
   record is missing); control state := state; control function := the
   record's think, or `0x005B0CD0` (Idle) if 0.

Callers: monster creation (`0x005A49B0`, `monsters/init.md`), skills
and special states (32 sites; `0x005B14E0`, `0x005E8140`, … switch back
to state 0). 1.14d-confirmed.

**When an alternate runs.** Step 3 looks up the record of the
control's *current* state through `0x005B15D0` (so states 10–12 of a
class without `switchai` read the base record) and ignores the new
state's value. So any install over a running base AI whose record has
an alternate (BatDemon 29, FrogDemon 52, FetishShaman 65, Diablo 51,
Hireable 61, BaalCrab 135, BaalCrabClone 140, UberBaal 145,
UberDiablo 147) makes the alternate the function for **one**
think; the alternate then re-installs the control's state (now the new
one), which is step 4 because the function is no longer the record's
think. Installers that reach a running monster: curse AI `0x005C34B0`
(10 / 11 / 12 / 0 by skill state, `ai-bodies-2.md` §16), terror
`0x005DDD00` (11), `0x005D6520` (10), suicide minion `0x005D1E10`
(15), imp possess `0x005D18E0` (16), and the special-state thinks'
own switch back to 0. Monster creation, the class reinit
(`0x00574250`) and the inactive restore (`0x005424F0`) install on a
fresh control (function 0), so never step 3.

**Installed special states in 1.14d.** Literal states pushed: 0, 5, 6
(Hireable `0x005E52D0`), 10 (`0x005D6520`), 11 (`0x005DDD00`), 13
(`0x005A49B0`, Countess), 15 (`0x005D1E10`), 16 (`0x005D18E0`), and
10 / 11 / 12 through `0x005C34B0`. The summon spawn `0x0056D940`
passes its request's state, and all 15 callers set it to 0. The other
sites pass the control's own state. So special-state records 1, 2, 3, 4,
7, 8, 9, 14 and 17 are never selected in 1.14d: the thinks only they
hold (`0x005B14E0`, `0x005E5870`, `0x005E7DC0`, `0x005E7F80`,
`0x005E2610`) never run; those shared with base AIs (Hireable 61,
GoodNpcRanged 60, NecroPet 67) run from the base table.
1.14d-confirmed (all 33 call sites, `all.asm`; `0x00570530`, a state-0
re-install, has no caller and no pointer to it).

**Re-install on a fresh monster (raise).** The reanimate raise
`0x005C07A0` (`combat/events.md` function 31) spawns N through the
monster allocator, whose type init already ran the first install
(state 0, `monsters/init.md` §5 step 5), then calls this function
again at `0x005C08A0` with (ECX game, EDX N, control = `0x00541860
(N)`, state 0). With the rules above, on N's fresh control:

1. N's class AI record has an alternate (the 9 AIs listed under "When
   an alternate runs") and the first install left the record's think
   as control function: step 3. The control's state stays 0 and its
   function becomes the alternate, so N's first think runs the
   alternate, which re-installs state 0 through step 4 (params
   cleared, init run, think installed).
2. Otherwise step 4 a second time: AI params 0–2 := 0, commands freed,
   the record's init function (if any) runs again with its draws
   (`monsters/init.md` OQ2 lists the init functions that draw), then
   function := the record's think (or Idle).

The leash, owner and node steps of the raise follow this install.
1.14d-confirmed (`0x005C0888`–`0x005C08A5`, §3.3 steps 3–4).

### 4. AI parameters

| Column (fields.tsv) | Offset | Read as | By |
|---|---|---|---|
| `AI` | +30 | link16 → monai index | §3.2 |
| `aidel`, `aidel(N)`, `aidel(H)` | +79..+81 | u8 frames | §1.3 (game-type gate); some AIs read `aidel` by difficulty directly (SandRaider `0x005F0700`, Izual `0x005F89B0`, UberIzual `0x005F8C80`) |
| `aidist`, `aidist(N)`, `aidist(H)` | +82..+84 | u8 tiles, 0 → 35 | §5.2 (by game +0x6D, no gate) |
| `aip1`..`aip8`, each ×3 | +86 + 6·(n−1) + 2·d | **signed** 16-bit | AI functions |

An AI function reads `aipN` for difficulty d = game +0x6D (no game-type
gate) at monstats +86 + 6(N − 1) + 2d. The usual test "P(aipN)" is one
step of the unit seed with `lo' % 100` (unsigned, `rng.md` §6) compared
**signed** with the aip value: `lo' % 100 < aipN`. aipN ≤ 0 never
passes, ≥ 100 always passes, but the draw happens either way. Where a
function uses the `roll(100)` helper (`0x0045C390`, unit seed) the result
is the same value. Several AIs pass an aip value to a helper as a byte
(wander/escape distances), as noted per AI. The monai.txt `*aip`
headers are comments, not data.

### 5. Target selection

#### 5.1 Forced targets `0x005DD610`

D2MOO `sub_6FCF2920` (EAX game, ESI unit; stack: a, s, &target,
&distance). Callers: the main search §5.2 step 3 with (a = its
line-of-sight flag T, s = 0), and `0x005DDC30` (§5.3) with (a = 0, s =
1). Runs first. The override is monster data
kind k (+0x38) and GUID g (+0x34). Setter `0x00573090(unit, k, g)`:
monsters whose monstats `switchai` bit is set, k < 5; callers: terror
special-state init `0x005E80E0` (k by the type of its path target
unit: player 1, monster 2, missile 4; g its GUID), skill callback
`0x005C3B30` (k 1 or 2 by the record's byte +0x10, g from +0x14) and
the confuse skill `0x005C3DE0` (k 3, g 0, after putting the unit in
target-node list 9). Clear `0x00573120` (k, g := 0): event type 10
(§1.6), `0x005C5653`, the terror think's end `0x005E8217`, and every
failure below.

1. Not a monster, or k = 0 → return 0.
2. k = 1 / 2 / 4: U := the unit of type 0 / 1 / 3 with GUID g
   (`0x00552F60`); none → clear, 0. d := full-size distance U→unit
   (`0x005DC380`, §6). s ≠ 0 and the collision test `0x00622AA0(U,
   unit, 4)` hits → clear, 0.
3. k = 3: A := the unit's alignment (`0x006259B0`); r := one raw step
   of the unit seed, `& 1` (`0x00472210(seed, 2)`). Temporary
   alignment (`0x005543B0(unit, value, 1)`): A = 1 → r ? 2 : 0; A = 0
   and r → 2; A = 2 and r → 0; else unchanged. Then scan 5 (s = 0;
   context {best 0, d 0x7FFFFFFF, a, 35, coordinate index of the
   unit's position `0x0061B130`, 0, 0x7FFFFFFF}) or scan 6 (s ≠ 0;
   context {0, 0x7FFFFFFF, 0, 0x7FFFFFFF}) (§5.4); U := best, d := its
   distance; restore alignment A (`0x005543B0(unit, A, 1)`).
4. Accept U when it is a player or monster that is not dead
   (`0x005541B0`), or a missile (type 3): target := U, distance := d,
   return 1. Anything else (none, object, item, tile, dead) → clear, 0.

From the main search the accepted U then takes §5.2 step 7 like any
target (control flag 0x08 and vision +0x24 unless the unit is good,
combat := melee-range test, distance := d).

The k = 3 draw is the only one, and it is made on every think while
the override holds. 1.14d-confirmed (`0x005DD610`, jump table
`0x005DD7D8`, `0x00573090`, `0x00573120`, `0x005730E0`,
`0x00573100`).

#### 5.2 Main search `0x005DD7F0`

D2MOO `sub_6FCF2110`. Returns target, distance, combat:

1. No room: nothing.
2. Line-of-sight test flag T: if control flag 0x40 is set, T = 1 and the
   flag is cleared. Else if the room's "LOS draw" test `0x0061AA40` is
   false: T = (flag 0x08 clear); for monsters with a vision record
   (monster data +0x50) and T = 1, T = (vision +0x24 == 0). Else T = 0.
3. Forced target (§5.1) → use it.
4. Alignment (`0x006259B0`) not evil (0): scan 5 (§5.4) within 35 with
   T; choose between its best and alternative via `0x005DD510` (§5.3).
5. Evil: best distance B = monstats `aidist` of the difficulty, 0 → 35.
   Walk the game's target-node lists (game +0x10F8, 10 heads; D2MOO
   `pTargetNodes`; slots 0–7 hold one player each followed by that
   player's attached units):
   1. For each slot 0..7 with a head: the head must be a player (fatal
      assert). If the player is in the same act (unit +0x18), has a room
      and that room is not in town: d = no-size distance (§6) from the
      player to the monster; M := min(M, d). If d < 55: a dead player
      counts as d = 0x7FFFFFFF; then the head and each following node of
      the slot (with its own d) is chosen if d < B and (T = 0 or the
      collision test `0x00622AA0(monster, node, 4)` is clear); B := d.
   2. Slot 8: every node in the same act with d < B and the same test is
      chosen.
   3. Slot 9: the nearest same-act node passing the test becomes an
      alternative; `0x005DD510` decides whether it replaces the choice.
      Its d is the no-size distance `0x005DC530`; no 55 or B limit
      applies, a node replaces the alternative only when strictly
      nearer (ties keep the earlier node), and the collision test runs
      only when T ≠ 0. Accepted: target := the alternative and B := its
      d, so step 7 reports the alternative's own distance
      (1.14d-confirmed, `0x005DDA85`–`0x005DDB1D`).
6. No target: combat := 0, distance := M (0x7FFFFFFF when no player
   qualified). Return 0.
7. Target: if alignment ≠ good (2): set control flag 0x08 and vision
   +0x24 := (it was 0). Combat := melee-range test `0x00622C40(unit,
   target, 0)` (`sim/units.md`). Distance := B.

Draws: none here (except §5.1 k = 3). Ties keep the earlier node.

**Target-node lists** (game +0x10F8 + 4·slot; node 0x10 bytes {unit,
a, next, prev}; unit +0xD0 = its slot, 11 = none):

| Slot | Inserted by | Holds |
|---|---|---|
| 0–7 | `0x005B1880` (player join: the first empty slot, only while game +0x8C ≤ 8) | the player, as head |
| 0–7 | `0x005B1900` (`skills/bodies.md` §6.3) | the player's attached units, each inserted right after the head (newest first) |
| 8 | `0x005B1990` from the rogue2 wanderers (`population.md` §12, `0x0054EF50`), NpcBarb `0x005EDC50`, Wussie `0x005EE3C0`, inactive restore `0x005424F0` | good NPCs that evil monsters attack (step 5.2) |
| 9 | `0x005B1990` from Bone Wall `0x005C58B0`, Bone Prison `0x005C5BC0`, the bone wall maker missile `0x005AEDA0`, skill `0x005C3BCC`, confuse `0x005C3DE0`, inactive restore | alternative targets (step 5.3) |

`0x005B1990` prepends, so slots 8 and 9 are walked newest first.
1.14d-confirmed (`0x005DD7F0`, `0x005B1880`, `0x005B1900`,
`0x005B1990` and its 9 call sites).

#### 5.3 Secondary helpers

| 1.14d | D2MOO | Does |
|---|---|---|
| `0x005DD510` | `sub_6FCF27B0` | alternative-target choice: never for players or without an alternative; take it if no main target; refuse if the alternative is farther than 5; else trial path toward the main target, keep main if a path exists; else scan 7 for something closer than 20 |
| `0x005DDC30` | `sub_6FCF2CC0` | target search for "good" shooters and secondary picks: forced target, else scan 6 + `0x005DD510`; returns target, distance (0x7FFFFFFF if none), melee flag |
| `0x005DDF20` | `sub_6FCCFD70` | nearest interacting player within 15 for NPCs (scan 2, callback D2MOO `sub_6FCCFDE0`); "close" when distance < 4; returns the NPC itself when none |

#### 5.4 Room scans `0x005DD0B0`

D2MOO `sub_6FCF1E80(game, unit, arg, callback, scan id)`. Table
`0x006E3300`, 13 × 8 bytes (scan mode, callback):

| Id | Mode | Callback | Id | Mode | Callback |
|---|---|---|---|---|---|
| 0 | 0 | caller's | 7 | 0 | `0x005DCC60` |
| 1 | 0 | caller's | 8 | 0 | `0x005DCD50` (doors) |
| 2 | 1 | caller's | 9 | 3 | `0x005DCDA0` (dead minions) |
| 3 | 0 | `0x005DC870` | 10 | 3 | caller's |
| 4 | 0 | `0x005DC8E0` | 11 | 2 | `0x005DCCC0` |
| 5 | 2 | `0x005DCA70` | 12 | 0 | `0x005DCE10` |
| 6 | 0 | `0x005DCBD0` | | | |

Modes: 0 every unit of every adjacent room (`0x005DCE60`); 1 every
client's player in the adjacent rooms (`0x005DCEE0`); 2 units of
adjacent rooms that are not in town and have clients, only for a living
player or monster scanner (`0x005DCF70`); 3 the minions of the
scanner's minion owner (`0x005DD050`). "Adjacent rooms" is the active
room's near-room list, own room included (`sim/unit-order.md` §4,
DRLG). A scan stops at the first callback that returns a unit; most
callbacks record a best candidate in `arg` and return 0. 1.14d-confirmed
(table dump, `0x005DD0B0`).

### 6. Distances and line tests

All distances are in tiles (subtile coordinates of `sim/units.md`):

| 1.14d | D2MOO | Value |
|---|---|---|
| `0x005DC530` | `AIUTIL_GetDistanceToCoordinates_NoUnitSize` | dx = \|ux − x\|, dy = \|uy − y\|; (2·max + min) / 2, truncated |
| `0x005DC380` | `…_FullUnitSize(a, b)` | dx, dy as above from a to b's position, each minus a's size (`0x00620510`) and **clamped at 0** (D2MOO uses the absolute value: 1.14d differs); then the same formula |
| `0x005DC5C0` | `AIUTIL_GetDistanceToCoordinates` | same formula on the path position |
| `0x00621F20(u)`, wrapper `0x005DD280` | `UNITS_GetCurrentLifePercentage` | life percent: (stat 6 life >> 8) × 100 / (max life (`0x00625D10`) >> 8), signed, truncating; 0 when max life >> 8 is 0 |
| `0x005DC480(u, x, y)` | `…_HalfUnitSize` | dx = \|ux − x\|, dy = \|uy − y\| (u's position), each minus (size(u) / 2 + 1) (`0x00620510`, unsigned halving) and clamped at 0; then (2·max + min) / 2 |
| `0x005DC640` | `sub_6FCF14D0` | "can reach directly": offset k = table by distance (2 ×3, 3 ×8, 4 ×14, else 3); tests three points (target, and target ± the perpendicular offset) for collision mask 0x1/0x4/0x400 (D2MOO wall, missile barrier, door); fails only if all three collide |
| `0x00622AA0(a, b, 4)` | `UNITS_TestCollisionWithUnit` | blocked line between a and b with mask 4 (`render/draw-order-2.md` §15–16) |
| `0x00622C40(a, b, 0)` | `UNITS_IsInMeleeRange` | combat flag (`sim/units.md`) |

### 7. Tactics helpers

#### 7.1 Mode requests

| 1.14d | D2MOO | Effect |
|---|---|---|
| `0x005DDF90(mode, target)` | `AITACTICS_ChangeModeAndTargetUnit` | mode change with a target unit; request flag 1; the path step count is not set |
| `0x005DDFC0(mode, x, y)` | `…ChangeModeAndTargetCoordinates` | mode change at coordinates; request flag 1; the path step count is not set (like `0x005DE490`) |
| `0x005DE000(skill, target, x, y)` | `AITACTICS_UseSequenceSkill` | skill id in range: mode 14 (sequence), current skill := skill, path step 1, no fallback |
| `0x005DEAD0(mode, skill, target, x, y)` | `AITACTICS_UseSkill` | mode < 16: current skill := the unit's skill entry with that id and owner −1 (`0x006439B0`, 0 when none; `0x00620210`), unit flag 0x40, path step 1; request with target unit, point (x, y) and flag 0; returns 1 when the mode change succeeds; else idle 10 and returns 0. Mode ≥ 16: nothing, returns 0 |
| `0x005DE190(method, speed, steps)` | `AITACTICS_SetVelocity` | §7.3 |

**Mode request record** (built by `0x005A7E60(unit, mode, &req)`,
consumed by `0x005A7C20(game, &req, flag)`; 0x20 bytes on the caller's
stack): +0x00 mode, +0x04 unit, +0x08 target unit, +0x0C x, +0x10 y,
+0x14 dword whose byte +0x15 is the path type, +0x18, +0x1C. The
builder clears the current skill (`0x00620210(unit, 0)`), zeroes
+0x08..+0x1C, and sets byte +0x15 := 101 when the mode is a moving mode
of the monster mode table (`0x005A6B10`: table `0x006E23D0`, 12-byte
rows by mode, first word nonzero; or second word 0, third nonzero and
the class's monstats2 mode bit set: byte +0x104 + mode / 8, bit mode %
8, through `0x00451FE0`), else 100. The request holds a
target unit **and** a point at once; for every mode except 3 the mode
set uses the unit when +0x08 ≠ 0 (path target unit, `0x00648B90`) and
the point (+0x0C, +0x10) only when +0x08 = 0 (`0x00648AD0`). So
`0x005DEAD0(mode, skill, T, x, y)` with T ≠ 0 aims at T; the point is
stored and ignored. Byte +0x15 reaches the movement set-up
`0x005A63F0` with +0x18 and byte +0x1C: a pending velocity request
(AI param record +0x18 method, +0x1C speed, §7.3) replaces them and is
cleared; then path type 100 = no path (path type 0, nothing computed),
101 = path type 13, any other value is the path type computed
(`0x005A6290`; a failed compute of types 2, 7, 9 or 13 retries with
type 15). Flag ≠ 0: when the unit's mode after the change equals the
requested mode, the current skill is cleared again. 1.14d-confirmed
(`0x005A7E60`, `0x005A7C20`, `0x005A63F0`, `0x005A6290`,
`0x005A6B10`).

Mode numbers: 1 neutral, 2 walk, 4 attack1 (A1), 5 attack2 (A2), 8
skill1 (S1), 9 skill2 (S2), 14 sequence, 15 run (`sim/units.md`).
Skill modes come from monstats `Sk1mode`..`Sk3mode` (+384..+386 as read
by 1.14d: bytes +0x180..+0x182) with skills `Skill1..3` (+368, +370,
+372); a skill id < 0 means "no skill" and skips the test (and its draw).

#### 7.2 Movement requests and their draws

`0x005DEB60` (D2MOO `AITACTICS_MoveToTarget(target, mode, x, y, step,
flags)`): if the mode is run (15) and the unit has state 60 (D2MOO
`STATE_DECREPIFY`), reset the velocity request and walk instead; set
the path step count; request the mode toward the target unit, or the
coordinates when the target is 0. On success return 1. On failure:
flag 4 → delete thinks; flag 1 → set control flag 0x40; flag 2 → draw
`lo' % 100`: < 70 → wander 4, else idle 10, and return 1. Else return 0.

| 1.14d | D2MOO | Mode, target, step, flags | Draws before the move |
|---|---|---|---|
| `0x005DEC80(t, f)` | `WalkToTargetUnitWithFlags` | 2, unit, 1, f | none |
| `0x005DED00(t, f)` / `0x005DED20(t)` | `RunToTargetUnit(WithFlags)` | 15, unit, 1, f / 0 | none |
| `0x005DED40(t, f)` | `sub_6FCD0410` | velocity method 13 first; then 2, unit, 1, f | none |
| `0x005DEF80(t, n)` / `0x005DEFB0(t, n)` | `Walk/RunToTargetUnitWithSteps` | 2 / 15, unit, n (0 → 1), 0 | none |
| `0x005DED90`, `0x005DEDE0`, `0x005DEE50` | walk / run (decrepify → walk) / walk + delete thinks on failure, to coordinates | 2 / 15 (state 60: velocity reset, 2) / 2, coordinates, path step count 1 (`0x00649070(path, 1)`), no flags | none |
| `0x005DE200(n)` | `AITACTICS_WalkCloseToUnit` ("wander n") | 2, point near itself, 1, 0 | three or four: (a) step, `lo'` bit 0 = 1 → x offset n, y offset `roll(n)`; bit 0 = 0 → x offset `roll(n)`, y offset n; (b) step, bit 0 = 1 → negate x; (c) step, bit 0 = 1 → negate y. `roll(n)` steps only if n ≥ 1 |
| `0x005DF530(t, n)` | `WanderToTarget` / `WalkToOwner` | same draws, around unit t | as wander |
| `0x005DEFE0(t, n, del)` | `D2GAME_AICORE_Escape` | unit or t = 0 → return 0, nothing done; if n > 5 velocity request steps n; walk to (own x + sign(own x − t.x)·n, own y + sign(own y − t.y)·n), step 1, flags del ? 4 : 0 | none |
| `0x005DF140(t, n, del)` | `sub_6FCD06D0` | same, running | none |
| `0x005DF7D0(t, n, del)` | `sub_6FCD0E80` ("circle n") | one step: low byte of `lo'` < 128 → velocity method 5, else 6, with steps n; then walk toward t, step 1, flags del ? 4 : 0 | one |
| `0x005DE6D0(t, a, b)` → `0x005DE4E0` | `AITACTICS_WalkInRadiusToTarget` | walk to the point that brings the distance to t toward b by at most a | none |
| `0x005DF680(t, n)` | `AITACTICS_RunCloseToTargetUnit` ("run near t n") | 15 (2 with the velocity reset if state 60), point near t, 1, no flags; returns the mode-change result | as wander, around t, n as a byte |
| `0x005DEF30(x, y)` | `WalkToTargetCoordinatesNoSteps` | 2, coordinates, 0, no flags; returns the mode-change result | none |

All draws are from the moving unit's seed (unit +0x20). 1.14d-confirmed
for `0x005DEB60`, `0x005DE200`, `0x005DF7D0`, `0x005DEFE0`, `0x005DF140`,
`0x005DED40`, `0x005DF680`, `0x005DEF30`, `0x005DED90`, `0x005DEDE0`,
`0x005DEE50`; the walk-in-radius geometry is
D2MOO's.

#### 7.3 Velocity request

`0x005DE190(unit, method, speed, steps)` asserts −126 ≤ speed ≤ 126,
maps method 1 to 7, and writes into the monster's AI param record
(monster data +0x2C) through `0x005A6260`: each nonzero argument
overwrites its field (+0x18 method, +0x1C speed bonus, +0x20 steps, steps
capped at 77). The record is consumed by the movement code
(`sim/units.md`). 1.14d-confirmed.

#### 7.4 Monster skill check `0x005FD470`

`0x005FD470(game, unit, skill, T, x, y)` (ECX game, EDX unit, four
stack words) → 1 when the skill may be used now, else 0. No draws.
Positions are the units' path positions (`0x0045ADF0` / `0x0045AE20`);
"pattern free at (x, y) in room R" = `0x0064D910(R, x, y, pattern of
the unit (`0x00649180`), 0x3C01)` = 0; "line clear" =
`0x00645910(ux, uy, x, y, R, mask)` ≠ 0 (`sim/path-placement.md`).

1. The skills row is missing (id < 0 or ≥ the count) or the skill is
   167 → 0.
2. The row's `TgtPlaceCheck` bit (flag bit 32: byte +8 bit 0) set: T
   ≠ 0, T has unit flag 0x2 (+0xC4) and T's pattern is free at T's
   position in T's room (pattern and mask 0x3C01 of T) → 1; else 0.
3. Skill 164 → T ≠ 0.
4. `srvdofunc` (+0x2E) 77 or 78: T ≠ 0 and the unit not in mode 0 or 12
   (`0x0063EA40`); P := 2 × T's position − the unit's position (the
   point mirrored through T); free-point search
   `0x0064E7B0(unit's room, &P, unit size, 0x3C01, 0)`
   (`sim/path-placement.md`) gives room R (none → 0); R not in town
   (`0x0061AB00`); the unit's pattern free at the **original** P in R;
   line clear from the unit to P in R with mask 0xC01 → 1; else 0.
5. Skill 199 → `0x005B34C0(game, room of T, 0, 0, T, 0x154, 1)` (the
   DiabPrison placement test, skills spec).
6. Skill 184 (monster teleport): unit ≠ 0; R := room at (x, y) from the
   unit's room (`0x00463740`), not in town; the unit's pattern free at
   (x, y) in R; line clear from the unit to (x, y) with mask 0xC01.
7. `srvdofunc` 67: unit ≠ 0; R as in 6, not in town; line clear with
   mask 0xC01 (no pattern test).
8. Skill 203: as 7 with mask 0x805.
9. Any other skill → 1.

1.14d-confirmed (`0x005FD470`, register use in the disassembly).

### 8. AI commands and minions

A command (D2MOO `D2AiCmdStrc`, 0x1C bytes: next, prev, five i32 params
at +0x08) lives in a ring hung on the control (+0x20 current, +0x24
last). Param 0 is the command type.

| 1.14d | D2MOO | Effect |
|---|---|---|
| `0x0058EE80` | `GetCurrentAiCommandFromUnit` | current command or 0 |
| `0x0058ED10` | `FreeCurrentAiCommand` | unlink and free the current one; current := its next |
| `0x0058EDE0` | `FreeAllAiCommands` | |
| `0x0058EF40` | `CopyAiCommand` | new command (`0x0058EC90`) with the given five params; becomes current |
| `0x0058EC90` | — (allocator) | zeroed 0x1C bytes from the game's pool. Empty ring (last = 0): the only node, next = prev = itself, current = last = it. Else linked between current's prev and current (it is current's new prev); if last = current, last := it |
| `0x0058EEF0(type, set)` | `GetAiCommandFromParam` | no current (ring empty) → 0. Else the first command of that type in the order current's next, its next, …, current (current is tested last; a one-node ring tests only it); set ≠ 0 makes it current; 0 if none |
| `0x0058EFA0` | `SetCurrentAiCommand(type, set)` | find by type (`0x0058EEF0`), create it with params (type, 0, 0, 0, 0) if absent (it becomes current), then return `0x0058EEF0(type, set)` |
| `0x0058F730` | `AllocCommandsForMinions` | copy the command to every minion of this unit's minion owner (control +0x2C/+0x30), in minion-list order |
| `0x0058F0D0` | `GetMinionOwner` | minion owner unit, or 0 |

Command types used by Act 1 AIs: 1 = "attack now" (Fallen, FallenShaman
minions), 10 = home position (NPCs, BloodRaven: params 1, 2 = x, y), 4
(walk to), 5 (wander) and 7 (mode action) = NPC actions (`ai-bodies.md` §9.9). QuillRat reads params 1, 2 as a unit type and
GUID. `0x0058ED10` keeps current and last both 0 or both set (freeing
the only node clears both; freeing last moves last to its next).
1.14d-confirmed for the functions listed.

### 10. The catalogue `ai-functions.tsv`

One row per AI table index (148 rows), tab-separated, header row:

| Column | Meaning |
|---|---|
| `index` | monai index = AI table record number = monstats `AI` value |
| `monai` | row name in the live monai.txt |
| `think_1_14d` | think function (record +0x08) |
| `init_1_14d` | init function (record +0x04), `-` if 0 |
| `alt_1_14d` | alternate function (record +0x0C), `-` if 0 |
| `target_mode` | record +0x00 (§2.3) |
| `d2moo_name` | D2MOO 1.10f function name at the same index |
| `monstats_rows` | count of live monstats rows using the index, then up to three "row Id" pairs; row = record index (hcIdx) after the `Expansion` separator line is dropped (`data/txt-format.md` §5), so file line − 2 up to druidbear (409) and file line − 3 from wakeofdestruction (410) on |
| `aip_meaning` | the monai.txt `*aipN` comment headers (hints, not data) |
| `summary` | one-line behaviour, `-` when unread |
| `status` | `spec'd-here` (full rules in `ai-bodies.md` §9, 1.14d read), `summarized` (top-level order read in 1.14d), `D2MOO-only` (summary from D2MOO 1.10f, 1.14d not compared), `unread` |

Columns 1–6 are dumped from `Game.exe` (`0x0073CA18`) and monai.txt and
can be checked mechanically.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| AI table | `0x0073CA18`, 148 × 16 | `0x005B15D0` bound 0x94, dump |
| special-state table | `0x0073D358`, 18 × 16 | `0x005B15D0`, `0x005B0E00` bound 0x12 |
| scan table | `0x006E3300`, 13 × 8 | `0x005DD0B0` |
| inline-think modes | `0x0073C6D0`, 16 bytes, modes 2 and 15 | `0x005A8030` |
| idle-by-distance | 25 / d − 10 / 10 at 35 / 25 | `0x005DE890` |
| no-target delay (modes 4/5) | 20 | `0x005DE9D0` |
| default target radius | 35 (aidist 0) | `0x005DD7F0` |
| player distance cut | 55 | `0x005DD7F0` |
| stun idle | 3; stun re-think 45 | `0x005B10E0`, `0x005A73E0` |
| aidel 0 → 15 | | `0x005A73E0` |
| teleport | 40 % gate, 15 % move, 25 % heal, skill 184 mode 4 | `0x005B11F0` |
| leash | ≤ 1 push away 19, > 20 return 19 (steps 40) | `0x005B0FF0` |
| boss sound | distance < 20, sound 16, idle 20 | `0x005B1140` |
| Npc map actions | `0x00741D74`, 7 pointers (0 and 6 null) | `0x005E7080` |
| Npc command counter G | u32 `0x0088CADC`, zero-initialised, process-wide | `0x005E6AE0` |
| monstats | `AI` +30, `aidel*` +79, `aidist*` +82, `aip*` +86, `isMelee`, `opendoors`, `npc`, `interact`, `switchai` bits of the flags at +12, `Skill1..3` +368/+370/+372, `Sk*mode` bytes +0x180.., `SplEndGeneric` +422, `MonStatsEx` +24, `BaseId` +2 | `data/fields.tsv` |
| monstats2 | `MeleeRng` +14 | `data/fields.tsv` |
| levels | `MonSpcWalk` +47 | `data/fields.tsv` |
| objects | `MonsterOK` +365 | `data/fields.tsv` |

## Randomness

All AI draws use the thinking monster's unit seed (unit +0x20, `rng.md`
§5.3, §7) with the step of `rng.md` §2; forms: `lo' % 100` (inline or
`roll(100)` via `0x0045C390`), `roll(n)` (`0x0045C3E0` in wander),
`lo'` bit 0 (wander, forced-target alignment), low byte < 128 (circle),
`lo' % 20` (Fallen sound), `roll(3)` (Navi), `lo' % 10` (FoulCrowNest),
`roll(15)`, `roll(L)` and `lo'` bit 0 via `0x00472210(seed, 2)`
(BloodRaven), `roll(count)` (Npc map AI). The Npc command counter G
(`ai-bodies.md` §9.9) is not a draw. Per think, the order is:

1. Precheck C teleport (§2.4): 1–3 draws, only for flag-0x20 monsters
   with a target.
2. Target acquisition: none (forced-target mode 3: 1).
3. Target mode 1 with no target: wander 5 (3–4 draws) or none.
4. The AI function's draws in the order of its `ai-bodies.md` §9 entry, including the
   helpers' draws (§7.2) at the point the helper is called.

Precheck A, the scheduling rules of §1 and the helpers of §6 and §8
draw nothing. Skill draws after a mode request belong to the skills
spec and happen later, when the mode's action frame runs.

## Edge cases & original bugs

1. A frozen living monster's thinks are dropped (§1.1); nothing in the
   dispatcher reschedules them.
2. The `aidel` difficulty column is used only when game +0x6A or +0x74
   is nonzero (§1.3); otherwise Normal's value applies at every
   difficulty. `aip` and `aidist` are read by difficulty with no gate.
3. aip values are signed: a negative aip never passes but still draws.
4. Full-size distance clamps each axis at 0 in 1.14d (D2MOO takes the
   absolute value).
5. Fallen deletes its own think first and may end without scheduling;
   its next think then comes only from `aidel` of a failed mode start, a
   room entry (+2), or a mode end.
6. Brute tests aip3 twice (the "circle" chance is the attack chance).
7. CorruptArcher with no secondary target circles toward the dispatch
   target, which may be 0 (the move then fails and `aidel` applies).
8. In §5.2 the dead-player rule applies only to the slot's head; the
   player's attached units are still candidates.
9. Teleport heal adds `level × 256` (fixed-point life) only when life <
   30 % and a second draw passes; the first two draws happen even when no
   spot is found.
10. A think scheduled for a monster with state 54 cancels every pending
    think of that monster first (§1.1), even the one being scheduled
    over.
11. Npc (`ai-bodies.md` §9.9): while a player talks to the NPC and AI param 0 ≤ 0, the
    interaction handler returns 1 without scheduling; the next think
    comes from an NPC message (+1) or a mode end. AI param 1 is both the
    message's x and the greeting countdown.
12. Npc command 4 alternates velocity method 5 / 7 by a process-wide
    counter G that never resets between games; its value depends on all
    NPC walks since the process started.
13. BloodRaven keeps the halved target distance after a failed "run
    near" (step 3) and uses it for the 5 / 12 tests of steps 5 and 6.
14. FoulCrowNest ends by requesting death mode on itself with unit flag
    0x20000 (no drop) once it has made aip3 summons.
15. Vampire uses `Skill1`, `Skill4` (and `Skill2`, `Skill3` under its
    flags) without testing the skill id for < 0; a row without them
    requests a skill mode with skill −1 (`0x005DEAD0`).
16. Secondary searches (`0x005DDC30`) report distance 0x7FFFFFFF when
    they find nothing; SkeletonMage and SkeletonBow then take their
    "far" branch.

## Test vectors

Synthetic (CI-safe; seeds are `{lo, hi}` of the unit seed before the
think; draws per `rng.md` §2):

| Seed | Raw `lo'` of the first four steps | `lo' % 100` |
|---|---|---|
| {1, 666} | 1791398751, 791599131, 671516612, 3064641593 | 51, 31, 12, 93 |
| {12345, 666} | 22752887, 2337785264, 1617882871, 4008788125 | 87, 64, 71, 25 |
| {3735928559, 666} | 3119800453, 2769797046, 1466979120, 4097238796 | 53, 46, 20, 96 |
| {4014346870, 666} | 2928842600, 1513463342, 1084273713, 4002178480 | 0, 42, 13, 80 |

| Case (Normal aip of `ai-bodies.md` §9.1; unit at (100, 100)) | {1,666} | {12345,666} | {3735928559,666} | {4014346870,666} |
|---|---|---|---|---|
| wander 3 from (100,100): end point; seed after | (103,100); {3064641593, 280084454} | (97,98); {4008788125, 674806599} | (103,100); {4097238796, 611865796} | (98,103); {4002178480, 452242291} |
| Zombie, C | A2 (51) | A2 (87) | A2 (53) | A1 (0) |
| Zombie, not C, D = 5, AI state 0, level ≠ 17 | 51 ≥ 30: wander 3 → (97,98) | wander 3 → (99,103) | wander 3 → (100,103) | run to T |
| Zombie, not C, D = 12 | no draw: wander 3 → (103,100) | (97,98) | (103,100) | (98,103) |
| Fallen, C, no command, param 0 = 0 | 51 ≥ 50; 31 ≥ 30 → idle 10 | 87; 64 → idle 10 | 53; 46 → idle 10 | 0 < 50; 42 ≥ 20 → A2 |
| Brute, C | 51 < 100; 31 < 45 → A1 | 87; 64 → A2 | 53; 46 → A2 | 0; 42 → A1 |
| QuillRat, not C, D = 5, AI state 0, no command | 51 ≥ 35 → escape 2 | escape 2 | escape 2 | 0 → A2 |
| circle: low byte, method | 95 → 5 | 119 → 5 | 133 → 6 | 104 → 5 |
| CorruptLancer, not C, D = 10 | 51 < 60; 31 ≥ 0 → walk 3 steps | 87 → idle 9 | 53; 46 → walk 3 steps | 0; 42 → walk 3 steps |
| CorruptRogue [60, 15, 75, 100, 20], C, D = 10 | 51 < 75 → A1 | 87 → idle 15 | 53 → A1 | 0 → A1 |
| CorruptRogue, not C, D = 10 | 51 < 60; 31 ≥ 20 → walk flags 7 | 87 → idle 15 | 53; 46 → walk flags 7 | 0; 42 → walk flags 7 |
| SkeletonBow [75, 15, 50, 5, 6], AI state 0, S at E = 10 | 51 < 75 → A1 at S | 87; 64 ≥ 20 → idle 15 | 53 → A1 at S | 0 → A1 at S |
| SkeletonBow, no S | 51 ≥ 50 → idle 20 | 87 → idle 20 | 53 → idle 20 | 0 → walk in radius (5, 6) |
| FoulCrowNest, D ≤ 20, summon not due: idle | 21 | 27 | 23 | 20 |
| BloodRaven, D = 8, at home (steps 1–3 draw nothing), not C, param 0 = 0 → 3, param 1 = 0 | 51 ≥ 3: no raise | no raise | no raise | 0 < 3: L = 7; raise at (T.x − 7, T.y − 6); param 1 = 1 |
| SkeletonMage [`ai-bodies.md` §9.19], S at E = 12 | 51 ≥ 30; 31 < 35 → A1 at S | 87; 64; step 2: 71 ≥ 30; 25 ≥ 20 → idle 5 | 53; 46; step 2: 20 < 30 → walk to T, 9 steps | 0 < 30 → walk to S, 9 steps |
| Arach [`ai-bodies.md` §9.20], state 0, not C, AI state 0, params 1, 2 = 0 | 51 ≥ 15; 31 ≥ 20 → idle 15 | 87; 64 → idle 15 | 53; 46 → idle 15 | 0 < 15 → lunge |
| Fetish [`ai-bodies.md` §9.21], state 2, D = 15, param 1 = 0 | 51 → idle 10 | 87 → idle 10 | 53 → idle 10 | 0 < 20 → circle 4 (low byte 46 → method 5) |
| Vampire [`ai-bodies.md` §9.22], state 0, not C, D = 10, L ≥ 33, no S, param 2 = 0 | 51 ≥ 40; 31 < 50 → circle 4 | 87; 64 → idle 10 | 53; 46 → circle 4 | 0 < 40; no F2/F4; no S → walk to T flags 7 |
| Bighead [`ai-bodies.md` §9.23], hurt, D = 10, no S | 51 ≥ 40 → idle 10 | 87 → idle 10 | 53 → idle 10 | 0 < 40 → circle 3 (low byte 46 → method 5) |
| BloodHawk [`ai-bodies.md` §9.24], not C, D = 10, param 0 = 0 | 51 ≥ 30; 31 < 90 → speed −50, wander 4 | 87; 64 → wander 4 | 53; 46 → wander 4 | 0 < 30 → charge: speed 100, steps 10, walk to T |
| HellMeteor [`ai-bodies.md` §9.25], unit at (100, 100) | 51 ≥ 50 → idle 50 | 87 → idle 50 | 53 → idle 50 | 0 < 50: roll(20) 2, 13 → `Skill1` at (92, 103) |
| Scarab [`ai-bodies.md` §9.29], C, D = 25, no command | 51 < 75; 31 < 35 → `Skill1` at T | 87 → idle 15 | 53; 46; 20 < 50 → A1 | 0; 42; 13 → A1 |
| Npc after steps 1–4 (no class case, no interaction, no command), map AI with 3 nodes | 51 < 66; node 0 | 87 ≥ 66 → 0 (then idle 8) | 53; node 0 | 0; node 2 |

Scheduling (no draws):

| Input | Expected next think |
|---|---|
| mode ends to neutral, no pending think, Normal, aidel 15, not stunned | frame + 15 |
| same, aidel 0 | frame + 15 |
| same, stunned (state 21) | frame + 45 |
| same, a think pending at frame + 7 | unchanged |
| mode 1, no target, nearest player d = 40 / 30 / 20 / none | +25 / +20 / +10 / +25 |
| mode 4, no target, no collision | +20 |
| idle 0 | +1 |
| `0x005DE130(10)` with a pending think at frame + 4 | unchanged (4 < 10); at frame + 12: replaced by +10 |

Recorded (tick recordings `20261006-015554`, `-021854`, `-022304`;
Normal Act 1; "own ty2" = schedules made while the unit's own type-2
event ran; "ty1" = during its type-1 end-of-mode event; "ty0" = during
its type-0 events, i.e. inline thinks at a path end, §1.4):

| Class | Type-2 runs | own ty2 delays (count) | ty1 | ty0 | Rule |
|---|---|---|---|---|---|
| zombie1 | 551 | 25 ×502, 15 ×3 | 15 ×26 | 25 ×11 | no player within 35 → 25; failed move → aidel |
| fallen1 | 2,280 | 25 ×1763, 10 ×301, 15 ×10, 5 ×1 | 15 ×33 | 25 ×18, 10 ×118, 15 ×4, 5 ×6 | idle 10 / 5 rules of `ai-bodies.md` §9.4 |
| brute1 | 19 | 25 ×19 | – | – | |
| fallenshaman1 | 24 | 25 ×19 | 15 ×1 | 15 ×4 | |
| quillrat1 | 641 | 25 ×608 | 15 ×5 | 25 ×21, 15 ×2 | |
| cr_lancer1 | 108 | 25 ×103, 9 ×2 | 15 ×1 | 9 ×1 | idle aip3 = 9 |
| cow | 24 | 200 ×24 | – | – | Idle |
| rogue1 | 57 | 200 ×57 | – | – | Idle |
| gheed | 150 | 8 ×47, 10 ×92, 20 ×3 | – | 8 ×1, 10 ×7 | Npc |
| akara | 106 | 8 ×60, 10 ×39, 20 ×2 | – | 8 ×1, 10 ×4 | Npc |
| kashya | 155 | 8 ×14, 10 ×130, 20 ×3 | – | 10 ×8 | Npc |
| charsi | 124 | 8 ×8, 10 ×105, 20 ×3 | 15 ×1 | 10 ×7 | Npc |
| warriv1 | 149 | 8 ×3, 10 ×127, 15 ×5, 20 ×3, 25 ×5 | 15 ×1 | 10 ×9 | Npc |
| navi | 21 | 10 ×4, 20 ×4, 50 ×2 | 15 ×11 | – | Navi; A1 arrows end → aidel |

Other recorded checks:

- Type-2 runs per recording: 1,368 (`015554`), 654 (`021854`), 2,387
  (`022304`); total 4,409.
- Mode-end rule (§1.3): 137 monster type-1 runs, none with a think
  pending; 79 scheduled exactly frame + 15; the other 58 scheduled
  nothing, and 56 of those units never ran another type-0 or type-2
  event (deaths: death-mode end goes to dead, not neutral). 2 fallen1
  without a schedule ran events later (open question 9).
- Creation pairs +15/+2 (§1.5): 287 pairs in the room step; client room
  entry +2: 61 schedules in the client step, all +2; 12 schedules made
  between ticks (recorder step "end": message handling) for gheed, akara
  and navi, all +1, consistent with the NPC interaction rule of §1.2
  (open question 11).

## Provenance

- 1.14d `Game.exe` (SHA-256 631066c1…adaaf): functions read from the
  Ghidra exports (`re/exports`, decompile and `all.asm` with register
  arguments checked in the assembly): `0x005B1740`, `0x005B10E0`,
  `0x005B0F50`, `0x005B0FF0`, `0x005B1650`, `0x005B13E0`, `0x005B1140`,
  `0x005B11F0`, `0x005B15D0`, `0x005B0E00`, `0x005B14E0`, `0x005A7F80`,
  `0x005A73E0`, `0x005A8520`, `0x005A8030`, `0x005A7C20`, `0x00573780`,
  `0x0053A8E0`, `0x00554850`, `0x005544B0`, `0x00573120`,
  `0x005DD7F0`, `0x005DE890`, `0x005DE9D0`, `0x005DD0B0`, `0x005DC380`,
  `0x005DC530`, `0x005DE080`–`0x005DF7D0` (helpers of §7),
  `0x0058EC00`–`0x0058F730` (commands), and the AI functions of `ai-bodies.md` §9,
  with their helpers: Npc `0x005E6800`, `0x005E6860`, `0x005E68F0`,
  `0x005E6AE0`, `0x005E7080`, map actions `0x005E6DE0`–`0x005E6FC0`;
  `0x005DC480`, `0x005DF680`, `0x005DEF30`, `0x0058EEF0`,
  `0x00472210`, `0x005FD350`, `0x00573930`, `0x0054CA10`,
  `0x00621F20`, `0x005B0BD0`, `0x0063EA40`, `0x00553540`,
  `0x00621E40`, `0x00639DB0`, `0x005DDC30`; for the implementation
  questions AI1–AI10 (2026-10-06): `0x005E77A0` (class test
  `cmp [unit+4], 0x20F` = 527), `0x005E7880`, `0x005E7AC0`,
  `0x0061AB00`, `0x00620BB0`, `0x0058EC90`, `0x0058ED10`,
  `0x0058EF40`, `0x0058EFA0`, `0x005DED90`, `0x005DEDE0`,
  `0x005DEE50`, `0x005DE4E0`, `0x005DE6D0`, `0x005F1800`,
  `0x00625480`, `0x00625D10`; G's references by a scan of `all.asm`;
  for the 2026-10-07 answers: `0x0057B170`, `0x0057B230`,
  `0x005734C0`, `0x005A68E0`, `0x0057CEE0` (AI-state sites),
  `0x005DD610`, `0x00573090`, `0x00573120`, `0x005E80E0`,
  `0x005B1880`, `0x005B1900`, `0x005B1990` (9 call sites),
  `0x0058F000`, `0x00666120`, `0x00665950` (DS1 path section),
  `0x006660B0`, `0x00667510`, and the 33 `0x005B0E00` call sites
  with the state each passes (15 `0x0056D940` callers checked for
  the request's state field).
- Tables dumped from the file's .rdata/.data with a PE-section parser:
  AI and special-state tables (`0x0073CA18`, 166 records), monster
  event table `0x006E2490`, mode table `0x006E2260`, inline-think bytes
  `0x0073C6D0`, scan table `0x006E3300`, map-action table `0x00741D74`
  (7 pointers); bytes at `0x005A7F70`,
  `0x005B0CC0`, `0x005E7F50` (functions Ghidra did not create).
- Callers of `0x005417D0` with event type 2: found by scanning all.asm
  for the type push before each call (27 sites).
- D2MOO (1.10f) `AiThink.cpp`, `AiTactics.cpp`, `AiUtil.cpp`,
  `AiGeneral.cpp`, `MonsterMode.cpp`, `Monster.cpp`, `SUnitDmg.cpp`:
  names, struct layouts and the meaning of the scan table and
  target-node slots. Every rule marked 1.14d-confirmed was compared
  with the 1.14d code; differences found: full-size distance clamps
  instead of taking the absolute value (§6); the 1.14d AI table has 148
  entries (D2MOO 1.10: 144) and several think functions are separate
  copies (indices 1/100, 12/19); special states 10–12 gate on
  `switchai`.
- Live data: monstats.txt, monai.txt, levels.txt (`patch_d2` layer).
  `ai-functions.tsv` `monstats_rows` regenerated 2026-10-06 from
  `patch_d2` monstats.txt (734 records = `monstats.bin` count; all 148
  counts unchanged, 51 rows' pairs corrected by −1 from record 410).
- Recordings: `traces/raw/20261006-015554-tick.jsonl`,
  `-021854-tick.jsonl`, `-022304-tick.jsonl` (`tick-raw-1`); counts by
  a script over `hin`, `set`, `ex`, `cancel` records (Test vectors).

## Open questions

1. Answered (2026-10-07): the freeze stat list's remove callback
   `0x0057B170` deletes the thinks and schedules frame + `aidel` (§1.1
   item 2); it reads `aidel` itself (monstats +0x4F), so the earlier
   "only `0x005A73E0` reads `aidel`" was incomplete. A freeze recording
   remains a check (PC 2 list), not a question.
2. Answered (2026-10-07): game +0x6A is the game type, 3 on every tick
   of the single-player packet recordings (`sim/intents-events.md`
   §Provenance, `-015956`); +0x74 is the ladder flag (game creation
   flags bit 21, `0x00530D59`). +0x6A ≠ 0 in single player, so §1.3
   rule 1 takes `aidel(N)` / `aidel(H)` by difficulty there. Confirm
   with a Nightmare recording (PC 2 list).
3. The `0x005A8520` knockback-end branches have no recorded instance:
   record a knockback (e.g. a player skill with knockback on fallen and
   on a sand leaper) and check +1 / +15 / gethit.
4. Answered (2026-10-07): `monsters/population.md` §8 owns it: the
   room seed, up to 20 tries of x := `roll(w)` + left then y := `roll(h)`
   + top, a test-only placement probe per try.
5. Answered (2026-10-07): §3.1 (the damage reaction sets 19; the
   monster mode set records the mode left, 19 → 3).
6. Answered (2026-10-07): §5.1 read in full on 1.14d.
7. Answered (2026-10-07): §5.2 "Target-node lists" (slot 8: rogue2
   wanderers, act 5 barbarians; slot 9: bone walls / prisons, confused
   units; both newest first; `population.md` OQ7 gives `0x005B1990`).
8. Answered (2026-10-07) for the builder: the DS1 loader makes the
   path of a preset unit from the DS1 path section (`drlg/preset.md`
   §5 step 10: points in file order, action 1 below DS1 version 15),
   the unit filter copy offsets every point with the preset
   (`drlg/preset.md` §7, `0x00667510`), and the preset spawn
   `0x00555910` (`population.md` §11.1) moves the pointer to control
   +0x38 (`0x0058F000` gives &control +0x38; `0x00666120` moves and
   clears the preset's). Quest code swaps it later
   (`world/quests-act3.md`, `0x00587950`). The idle-10 source is settled in
   `ai-bodies.md` §9.9 (command 4 delay). Left: confirm with a town recording that logs command 4 next to thinks.
9. The 2 recorded fallen1 type-1 runs with no schedule and later
   activity: likely a death end followed by a shaman resurrect; check
   with a recording that hooks mode changes.
10. Answered (see 16): no AI function is `unread` or `D2MOO-only`.
11. `0x0054CA10` is C→S 0x59 and `0x00548B00` is reached from C→S 0x13
    (`world/npc.md` §2); still open: confirm the 12 recorded "+1"
    schedules between ticks with a recording that logs client messages
    next to timer schedules.
12. Answered: G (`0x0088CADC`) has exactly two references in the
    binary (the read and the `add 1` in `0x005E6AE0`) and no reset, so
    it counts from 0 at process start across every game of the process.
    `d2-sim` takes G from its host as a counter that outlives games
    (the local server process owns it, one per server process, never
    saved); a new game does not reset it. Conformance traces start from
    a fresh process (G = 0), or are the first game after one.
13. Answered (`docs/handoff/impl-ai-acts2-5.md` reading 2):
    `0x005DEAD0(mode, skill, T, x, y)` with both a unit and a point.
    The mode request holds both (§7.1 "Mode request record"); the mode
    set aims at T whenever T ≠ 0 and uses the point only when T = 0.
    Summoner (`ai-bodies-2.md` §15), Izual (`ai-bodies-4.md` §8) and
    the HighPriest hydra (`ai-bodies-3.md` §8 step 1.3) all pass T ≠ 0,
    so all three aim at T; the hydra's offset point is stored and not
    used by the mode set.
14. Answered (reading 18): `0x005DDFC0` (mode at a point) and
    `0x005DDF90` (mode at a unit) do not set the path step count; both
    request with flag 1 (§7.1).
15. Answered (reading 19), owners of the AI seams that `d2-sim` routes
    through `Pending::ai_*`: skill check `0x005FD470` → §7.4 here; skill
    entries, levels, add `0x0056DEB0`, right skill `0x005701B0`, assign
    `0x00647280` → `skills/bodies.md` / `skills/use.md`; patterns,
    placement `0x00554EA0`, stamps `0x0064EA90`, free points
    `0x0064E7B0`, cell bits → `sim/path-placement.md`; path target,
    compute, path type → `sim/pathing.md`; free spot `0x0054DC40`,
    spawn `0x005B2F20`, spawn class `0x0054DA60`, player-count record
    `0x00573930` → `monsters/init.md`, `monsters/population.md`; kill
    `0x0057CCB0` → `combat/damage.md` §7.2; stat lists `0x006251F0`,
    curse flag `0x00625760` → `sim/stat-lists.md`; quest seams →
    `world/quests.md` and its act files. The last four (2026-10-07): the
    reinit-as-class `0x00574370(game, unit, class, mode)` →
    `monsters/init.md` (it rebuilds the monster data of an existing
    unit; callers are the death action of `sim/intents-events.md`, a
    skill class change `skills/bodies-4.md` and BaalThrone
    `ai-bodies-5.md` §20, so it is not AI-only); the unit find
    `0x0065A950` / `0x0065AC70` → `monsters/umod-callbacks.md` §3.1; the
    client preload `0x00571C00` (only caller BaalThrone) → its rule is in
    `ai-bodies-5.md` §20 step 5 (a pending event record on the unit),
    the record list and the send → `sim/intents-events.md` §7.9; the
    spawn info `0x0063EFA0` → `ai-bodies-2.md` §13.1 (every key).
16. Answered (open question 10): no `unread` row is left in
    `ai-functions.tsv`; the last 55 bodies are `ai-bodies-6.md` and
    `ai-bodies-7.md` (the Uber Mephisto, Diablo and Baal thinks are
    empty in 1.14d: `ai-bodies-7.md` §26 and its open question 2).
17. Answered (2026-10-07), the implementation questions of
    `docs/handoff/impl-ai-act1.md` §4 and `gaps-combat-ai.md`, all
    settled in the text: AI1 drehyaiced is class 527 (`ai-bodies.md` §9.32,
    `0x005E77A0` compares 0x20F; the catalogue row pairs were corrected,
    Provenance); AI2 and AI3 `ai-bodies.md` §9.31 (the "Else" belongs to the first
    30 % roll; no room counts as out of town); AI4 and AI5 `ai-bodies.md` §9.32 (the
    param writes and idle 1 follow a leave; any class other than 527
    with AI 31 takes cain1's Act 1 functions, it does not do nothing);
    AI6 §8 (`0x0058EFA0` creates through `0x0058EF40`, which makes the
    new command current; `0x0058EEF0` with no current returns 0); AI7
    §7.2 table (coordinate walk / run: path step count 1); AI8 OQ12 (G
    is per server process, not per game or per AI store: the code's
    per-store G matches only the first game of a process); AI9 `ai-bodies.md` §9.26
    (states through `0x00639DB0`); AI10 `ai-bodies.md` §9.28 (a state above 3 takes the
    above-ground steps; the function never writes one). Forced-target combat and flags: §5.1 end
    (§5.2 step 7 applies).
