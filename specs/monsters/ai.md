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
|   1. Think scheduling | 97–293 |
|   2. Think dispatch `0x005B1740` | 294–433 |
|   3. AI control and AI tables | 434–605 |
|   4. AI parameters | 606–624 |
|   5. Target selection | 625–961 |
|   6. Distances and line tests | 962–977 |
|   7. Tactics helpers | 978–1217 |
|   8. AI commands and minions | 1218–1244 |
|   10. The catalogue `ai-functions.tsv` | 1245–1265 |
| Constants & data dependencies | 1266–1289 |
| Randomness | 1290–1319 |
| Edge cases & original bugs | 1320–1361 |
| Test vectors | 1362–1450 |
| Provenance | 1451–1512 |
| Open questions | 1513–1619 |
|   1. Think scheduling | 97–287 |
|   2. Think dispatch `0x005B1740` | 288–427 |
|   3. AI control and AI tables | 428–599 |
|   4. AI parameters | 600–618 |
|   5. Target selection | 619–826 |
|   6. Distances and line tests | 827–841 |
|   7. Tactics helpers | 842–1081 |
|   8. AI commands and minions | 1082–1108 |
|   10. The catalogue `ai-functions.tsv` | 1109–1129 |
| Constants & data dependencies | 1130–1153 |
| Randomness | 1154–1175 |
| Edge cases & original bugs | 1176–1217 |
| Test vectors | 1218–1306 |
| Provenance | 1307–1367 |
| Open questions | 1368–1471 |
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
think is not rescheduled by the dispatcher. A dead monster's think is **not** dropped here; it has none
because the death clean-up cancels its type-2 events (`sim/units.md`
§4.6 "What keeps a dead monster dead"). The freeze itself schedules
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
  (trappedsoul1) any mode. This branch is tested first and does **not**
  set the anim mode (only the table-1 branch calls `0x00624690(unit,
  1)`); same freeze gate, then the think (`0x005A80C5`–`0x005A80F5`); a
  matching class whose mode does not match falls to the table test;
- every other case requests a mode change to neutral (which schedules
  through §1.3). The request's target is the path's current target unit
  (`0x00553540`: none when it is the unit itself), so a unit without one
  (the Hydras, whose path never had a target unit) makes the request
  point (0, 0) and its path target becomes (0, 0) (§7.5 rule 2;
  `sor-hydra`, frame 42). 1.14d-confirmed (`0x005A8030` at
  `0x005A8100`–`0x005A8140`).

So a monster that walks or runs re-thinks the frame its path ends.
1.14d-confirmed (`0x005A8030`, table `0x0073C6D0` = 00 00 01 00 … 00 01).

The neutral request of the last case (`0x005A8030`, read 2026-10-09)
first, when the unit has state 54, runs `0x005544B0` and stops if the
unit is then dead (`0x005541B0`); it clears the used skill (`0x00620210(unit, 0)`), then builds the
mode-change record {unit, target unit := the path target unit
(`0x00553540`: none when there is none or it is the unit itself), point
(0, 0), mode 1} and calls `0x005A7C20(game, record, 1)`. So a monster
without a path target unit ends with path target point (0, 0) (§7.5
rule 2): the summons that end S1 at the animation end (raven, plague
poppy, vines, cycle of life) show it in 1.14d (REC-1651, settled by this
read).

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
3. **Last client leaves a room: the think is cancelled.** The room
   leave `0x0053A9B0` (`sim/intents-events.md` §7.8 rule 3) removes the
   client (`0x0061A700`), then tests the room's client count
   (`cmp [room + 0x78], 0` at `0x0053AA0A`); when it is 0, every unit of
   the room's unit list (room +0x74, next +0xE8) of type 1 gets
   `0x005738D0(game, unit)` (call at `0x0053AA2A`): delete the unit's
   type-2 events (AI think) and then its type-3 events (stat
   regeneration), any argument (`0x00540E60(2, 0)`, `0x00540E60(3, 0)`).
   Nothing is scheduled in their place, and nothing is drawn. The
   monster has no pending think until a client enters its room again
   (rule 2: +2 if neutral). The same "no client, no think" holds at
   creation (`init.md` §4.1, gate `0x00553160`: `room.clients ≠ 0`, else
   clean-up and no think), so a monster in a room no client sees never
   thinks from a timer.
   - The test is in the room leave only. The think path has none: the
     class handler `0x005A7F80` (freeze gate only, §1.1), the runner
     `0x00541060` (`sim/tick.md` §5.5), the dispatch `0x005B1740` (§2)
     and the Npc AI `0x005E7130` (`ai-bodies.md` §9.9) never read room
     +0x78. A think that is still pending runs normally.
   - Only types 2 and 3 are cancelled. Mode events (types 0 and 1) stay,
     so a monster that is mid-mode when the last client leaves still
     reaches its mode end, and the mode end can schedule or run a think
     (§1.3, §1.4). This is a static reading; no recording has it. An
     idle monster has no mode end to wait for, so it stops thinking.
   - Recorded (`traces/checks/a4-warp-plains-ama.check`, ScnAm4,
     `-seed 1234`): the warp at frame 6 takes the client out of every
     Pandemonium Fortress room. The Fortress NPCs (classes 405, 257,
     246) had think +20 pending from their frame-4 home think
     (`ai-bodies.md` §9.9 step 1). That think is cancelled, so there is
     no frame-24 think and no map-AI draw. Their unit seeds do not
     change from frame 6 to frame 80. With a client in the level, the
     same think runs (`a4-fortress-arrival-ama` matches).
   Provenance: 2026-10-09 (pc1-day3-c, read in `0x0053A9B0`,
   `0x005738D0`, `0x00540E60`, `0x0053A8E0`, `0x005A7F80`,
   `0x00541060`, `0x005B1740`, `0x005E7130`; q-prov-data).

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
54 (D2MOO only warns). It does not test the room's client count either
(room +0x78). A monster in a room with no client has no think to
dispatch, because the room leave cancelled it (§1.5 rule 3); 2026-10-09
(pc1-day3-c, read in `0x005B1740`).

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

`0x005DE9D0` (D2MOO `sub_6FCCFC00`): runs `0x005DD7F0` first (a target
found → return it); else the same collision test (mask 0x40 at the
unit's position, static path +0x0C / +0x10 for path types 2, 4, 5,
else the dynamic x / y) **and** the class can walk (has mode 2 `WL`,
`0x0046C140(class, 2)`, the same test as `0x005DE890`) → wander 5;
otherwise (no collision, or no walk mode) delete thinks (`0x00540E60`
type 2) and schedule a type-2 think at frame + 20 (`0x005417D0`; no
mode change), return 0. No draw happens in this function besides the
wander's own (§7.2).

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
2. Line-of-sight test flag T, vision record V := none, seen value
   S := 0: if control flag 0x40 is set, T = 1 and the flag is cleared.
   Else if the room's "LOS draw" test `0x0061AA40` is false: T = (flag
   0x08 clear); for a monster (type 1) with monster data, V := monster
   data +0x50 (loaded whatever T is); if T = 1 and V ≠ none, S := V
   +0x24 and T := (S = 0). Else T = 0. V is the monster's DRLG
   coordinate record (§5.2.1).
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
7. Target: if alignment ≠ good (2): set control flag 0x08, and if V ≠
   none, V +0x24 := (S = 0). Combat := melee-range test `0x00622C40(unit,
   target, 0)` (`sim/units.md`). Distance := B.

Implemented (`monsters/ai/target.rs`, `wiring/action/ai.rs`): the
record is loaded on the LOS-draw-false path whatever T is; S is its
+0x24 only when flag 0x08 is clear, else 0. Step 7 writes +0x24 :=
(S = 0) only when the record was loaded, so a find with the token at 1
consumes it. Settled by `a1-warp-tower-cellar-ama` (frame 45, fallen3
leader 1:10) and the unit test `vision_token_toggles_only_when_loaded`
(the token write). PROVISIONAL (REC-1698), unchanged: the record
is kept by identity (act, rect, index) in
`wiring/worldgen/population_init.rs`.
##### 5.2.1 The vision record (monster data +0x50)

V is a DRLG coordinate record (`D2RoomCoordListStrc`, the record r of
`population.md` §3.2; layout `drlg/levels.md` §11.1, where +0x24 is
otherwise never written), shared by every monster whose record it is.
It is set once at creation (`population.md` §9.6 step 3) and cleared
to none on the monster's first room change (`sim/pathing.md` §9.8);
`0x00552D60` and `0x00554670` are its only writers. Steps 2 and 7 make
+0x24 a "someone here has seen a target" token. A monster whose flag
0x08 is clear tests line of sight while the token is 0, and if it
finds a target the token becomes 1. While the token is 1 it searches
**without** the collision test (T = 0) once; a find then writes 0 (it
consumes the token). A monster whose flag 0x08 is already set rewrites
1 on every find. Nothing is written without a target, with alignment
2, on the flag-0x40 path, or when the LOS-draw test is true (V stays
none there). This explains `a1-warp-tower-cellar-ama`. A fallen3 party
minion finds the player at frame 43 and sets the token. At frame 45
its pack leader (1:10) has flag 0x08 clear and the same record (the
leader holds the pack's rect r, the minion the record at its own point,
`population.md` §9.6). It skips the collision test, finds the player at
distance 32 and resets the token to 0.
1.14d-confirmed (`0x005DD9B6`–`0x005DDA0F`, `0x005DDBE6`–`0x005DDC08`,
`0x00552D60`, `0x00554670`, read 2026-10-09, PC 1 late).

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
| `0x005DD510` | `sub_6FCF27B0` | alternative-target choice `(unit; &main, &main d, alt, alt d)` → 1 = take the alternative, 0 = keep main (main / main d may be rewritten). Order (1.14d-confirmed `0x005DD510`–`0x005DD601`, read 2026-10-09, PC 1 night; settles REC-1271): (1) the unit is a player → 0; (2) no alternative → 0; (3) **no main target → 1, whatever the alternative's distance**; (4) alt d > 5 (signed) → 0; (5) trial path toward main on the unit's path record (settings saved, target := main, path type 2, `0x00649970(path, unit, 0)`, settings restored); the path has points (`0x00648780` ≠ 0) → 0; (6) scan 7 (§5.4) with context {0, 0x7FFFFFFF, main}: found and its d < 20 → main := it, main d := d, return 0; else → 1. No draws of its own, but scan 7 runs the filter `0x005DC970`, so its state-146 draw (scan 6 step 1.5 below) can happen here too; no life test (an hp-0 unit is judged like any other). Scan 7 callback `0x005DCC60`, context {best, best d, excluded = main}: C = main → skip (before the filter, so main draws nothing here); filter; d := `0x005DC380(C, unit)` > 48 → skip; class t (step 3 below) < 2 → skip; d ≥ best d → skip; line `0x00622AA0(C, unit, 4)` blocked → skip; else best := (C, d); always returns 0 (1.14d-confirmed `0x005DCC60`–`0x005DCCBC`, PC 1 late) |
| `0x005DDC30` | `sub_6FCF2CC0` | secondary target search: forced target, else scan 6 + `0x005DD510`; returns target S, distance E (0x7FFFFFFF if none), melee flag M. In full below |
| `0x005DDF20` | `sub_6FCCFD70` | nearest interacting player within 15 for NPCs (scan 2, callback D2MOO `sub_6FCCFDE0`); "close" when distance < 4; returns the NPC itself when none. Callback `0x005DDE80(game, npc, C, ctx)` (1.14d-confirmed, settles REC-500): C not a player → skip; d := full-size distance `0x005DC380(npc, C)` (the NPC's size subtracted, §6); d > 15 → skip (so ≤ 15 is in); NPC without monstats flag `interact` (byte +0xD & 2, mask `0x006CE26C` = 2, flag bit 9) → take C; with it → take C only if the quest active-cycler test `0x00544590(game, C, npc)` holds and d < best. Taking C writes (C, d) and **returns C, which stops the scan** (§5.4), so the first qualifying player in scan-1 order wins and there are no ties |

**`0x005DDC30(game, unit; &E, &M)`, in full** (1.14d-confirmed
`0x005DDC30`–`0x005DDCF5` and its 29 call sites in `all.asm`, read
2026-10-09, PC 1 late). Register call (ECX game, EDX unit), stack
pointers &E then &M, `ret 8`; each pointer may be null and is tested
before the write. The "second argument 0" of the `ai-bodies*.md` files
is a null &M: the caller does not want the melee flag.

1. Forced target (§5.1) with (a = 0, s = 1). Accepted → S, E := its
   unit and distance; go to step 4. This S skips the 49 window, the
   threat classes and `0x005DD510`, and may be a missile.
2. Else scan 6 (§5.4; mode 0: the unit's room `0x00620BB0`, none →
   nothing found; that room's near-room list `0x00619790` in list
   order, own room included; per room its unit list, head room +0x74,
   next unit +0xE8, every unit type) with the callback `0x005DCBD0`
   below, context {main 0, 0x7FFFFFFF, alt 0, 0x7FFFFFFF}.
3. `0x005DD510(unit; &main, &main d, alt, alt d)` (table above): 1 → S,
   E := alt, alt d; 0 → S, E := main, main d (main as its scan 7 may
   have replaced it).
4. S = 0 → E := 0x7FFFFFFF, M := 0, return 0. Else M := melee-range
   test `0x00622C40(unit, S, 0)` (`sim/units.md`), E as found, return S.

What it scans: every player and monster of the near rooms that the
filter keeps (not dead, not in town, unit flag 4, hostile by
`0x00554200`): players, hirelings, pets and summons, monsters, of any
act. It reads no target-node list (unlike §5.2 step 5), so there is no
55 limit, no act test and no `aidist`. Range: full-size distance with
C's size subtracted ≤ 48. Priority: class t ≥ 2 (every player, t = 14,
and monsters by `nThreat`) fills main; t 0 / 1 fills alt, which wins
only through `0x005DD510` (no main, or a main it rejects). Nearest
wins; ties keep the earlier unit in room-list, then unit-list order.

Draws: none on the unit's own seed except §5.1 k = 3 (one raw step,
confused units only). The filter's state-146 draw (step 1.5 below) is
on the candidate player's own seed, once per such player in scan 6 and
again in `0x005DD510`'s scan 7. The callers' own tests draw after the
call returns. Side effects: a failed §5.1 clears the override (k, g :=
0); §5.1 k = 3 swaps and restores the alignment; `0x005DD510` step 5
makes a trial path and restores the path settings.

Use of the result: `0x005DDC30` never writes the AI control's target,
state or vision flags. S is a local of the calling think, used as the
target of that call's skill or attack. Callers (29 sites, 26 thinks):
with a non-null &M (12 sites): NecroPet ranged `0x005E4AC0`, Hireable
`0x005E52D0`, BloodRaven `0x005E6320`, GoodNpcRanged `0x005E7AC0`,
TownRogue `0x005E7D60` and its never-run special state `0x005E7DC0`
(§3.3), Navi `0x005E7E20`, AssassinSentry `0x005EA3D0`, DeathSentry
`0x005EA980`, Vines `0x005EC6C0`, Bighead `0x005EFF50` (two sites).
With &M null (17 sites): PantherJavelin `0x005E1080`, FetishBlowgun
`0x005E1250`, Succubus `0x005E1E00`, SuccubusWitch `0x005E2120` (two),
Imp `0x005E2FF0`, FallenShaman `0x005F1440`, SandMaggot `0x005F1800`,
GreaterMummy `0x005F2B10`, Vampire `0x005F4A70`, CorruptArcher
`0x005F5A20`, SkeletonBow `0x005F6070` (two), Summoner `0x005F85C0`,
TentacleHead `0x005F9270`, SkeletonMage `0x005F96C0`, OblivionKnight
`0x005FAF00`. A caller that keeps a distance variable across steps
(e.g. SuccubusWitch E, `ai-bodies-5.md` §6) sees it overwritten by the
call, with 0x7FFFFFFF when nothing was found.

```
s, e = forced_target(unit, a=0, s=1)
if !s:
    ctx = scan6(unit)                       # main (t>=2) / alt (t<2), d <= 48
    take_alt = alt_choice(unit, &ctx.main, &ctx.main_d, ctx.alt, ctx.alt_d)
    s, e = take_alt ? (ctx.alt, ctx.alt_d) : (ctx.main, ctx.main_d)
if !s: return (0, 0x7FFFFFFF, 0)
return (s, e, melee_range(unit, s, 0))
```

**Scan 6 callback `0x005DCBD0`** (the search window of `0x005DDC30`;
2026-10-09, pc1-data Step 4 item 3). Context {main, main d, alt, alt d},
started {0, 0x7FFFFFFF, 0, 0x7FFFFFFF}. For each candidate C:

1. Filter `0x005DC970(game, scanner, C)` → 1 = keep; tests in this
   order, the first failure skips C (1.14d-confirmed
   `0x005DC970`–`0x005DCA1D`, read 2026-10-09, PC 1 night; settles
   REC-1270):
   1. scanner present and a player or monster (type 0 / 1); C the same;
   2. C not dead, then the scanner not dead (`0x005541B0`: flag-ex bit
      +0xC6 & 1, mode 0, or mode 12 for a monster / 17 for a player; no
      life test, so an hp-0 unit in a living mode passes);
   3. C's room not in town (`0x00620BB0`, `0x0061AB00`);
   4. C has unit flag 4 (`0x00451F30(C, 4)`, +0xC4 & 4). A monster gets
      it at creation only when its monstats2 `isAtt` is set (`0x00573CB0`
      at `0x00574141`–`0x0057414E`); a class without `isAtt` (e.g.
      `cow`, the traps) is never a scan 6 candidate;
   5. only when **C** has state 146 `invis` (`0x00639DF0(C, 0x92)`; the
      scanner's states are not read): a player C draws `roll(100)` on
      C's own unit seed (+0x20, `0x0045C390`); draw < 80 and the scanner
      not in melee range of C (`0x00622C40(scanner, C, 0)`) → skip (draw
      ≥ 80, or in range → go on). A monster C (no draw): in melee range
      `0x00622C40(scanner, C, 0)` → skip. Without state 146 nothing in
      this step runs: no melee test, no draw, no distance;
   6. hostility `0x00554200(game, scanner, C)` (`combat/hit.md` §7.1)
      = 0 → skip.

   No step reads a distance except the state-146 melee test, so for a
   C without state 146 the filter cannot tell distance 1 from 2.
2. d := `0x005DC380(C, scanner)`: the full-size distance with **C's**
   size subtracted (§6; a = C). d ≥ 49 (`cmp d, 0x30; jg`) → skip.
3. Class t := 14 for a player, the monstats byte +0x4E (`nThreat`) for
   a monster (`0x005DC920`). t ≥ 2 competes for main, else for alt; d
   not below that slot's distance (signed) → skip (ties keep the
   earlier candidate in scan order).
4. Line test `0x00622AA0(C, scanner, 4)` (a = C: C's room, C's end
   pulled first, `render/draw-order-2.md` §15.1) blocked → skip; else
   the slot := (C, d). The callback always returns 0 (whole scan).

Against the q-fix-ass-traps recordings (Lightning Sentry, `ai-bodies-6.md`
§14): the hp-0 poked `cow` 6 away was never a candidate (no `isAtt` →
no unit flag 4, step 1.4), so `0x005DD510`'s order is not what refused
it (with no main and a candidate alternative, step 3 takes it at any
distance). The Fallen (`isAtt` 1, `threat` 10 → main) refused at
distance 1 but taken at 2 is **not** explained by steps 1–3: the only
distance-dependent test left in the scan is step 4's line, whose ends
are pulled by the Fallen's size 2 (the sentry's size is 0), so at a
diagonal 1 the Fallen's end lands on the far side of the sentry and the
cells tested differ from those at 2. Whether that cell carries
collision bit 4, or the refusal lies after the scan (the think's
`0x005DEAD0` / the skill), needs the recorded dx, dy and the collision
grid at both points (open, REC-1270 follow-up).

PROVISIONAL (REC-1270): rule 1's "monster C in melee range → skip" holds
for every monster C, not only one with state 146 (as d2rs reads it: C in
the scanner's melee range, `0x00622C40` step 3 without the line, d ≤ 0 or
d ≤ `MeleeRng` + 1, is skipped). 1.14d measured (`ass-lightning-sentry-kill`
and its probes, Fallen packs, Lightning Sentry `MeleeRng` 0): of three
Fallen the one at distance 1 (dx 1, dy 0) is never the trap's target
while one at distance 2–3 is, in three placements; the trap fires at it
once the nearer one has moved off. Settled by: a run that puts a monster
of another class on the trap (a state-146 test) or reads `0x005DC970`
(PC 1 item "[q-fix-ass-traps] 0x005DC970 melee-range rule").
PROVISIONAL (REC-1271): a main-less scan (only `nThreat` < 2 candidates,
e.g. a cow) takes the alternative at once in d2rs; the spec's
`0x005DD510` row also refuses an alternative farther than 5, and the
order of those two rules is unread (an idle cow poked next to a trap was
not shot by 1.14d in a one-off run on 2026-10-09, check file not kept; the cow
had hp 0, so the dead test may be the cause instead). Not applied.
PROVISIONAL (REC-1642): the scan 5 callback `0x005DCA70` (§5.2 step 4,
the not-evil search) skips a candidate without unit flag 4 (+0xC4,
monstats2 `isAtt`, `monsters/init.md`) like rule 1 here (because
1.14d's pets never take the poked cow, class 179, `isAtt` 0, neutral
alignment, four sub-tiles away: a Clay Golem and a Valkyrie in
`nec-clay-golem.check` / `ama-valkyrie.check` follow and wander for
50 frames, q-fix-skills-4cls 2026-10-09); settled by reading
`0x005DCA70`.

PROVISIONAL (REC-1695): `0x005DDC30` as d2rs runs it (`d2-sim`
`wiring/action/ai_scan.rs`): the forced target (§5.1 with a = 0, s = 1)
first, else scan 6 over every unit of the scanner's near-room list (own
room included, list order, §5.4 mode 0) with the callback rules above,
then `0x005DD510` on the scan's main M and alternative A: a player
scanner or no A → M; no M → A; A's distance > 5 → M; else the trial path
and scan 7 (not read in detail; the host answers, keeping M by default).
Returned: the pick, its distance (0x7FFFFFFF without one) and the
melee-range test `0x00622C40(unit, pick)`. In rule 1 the `roll(100)` on a
state-146 player's seed is drawn before the melee-range test (the order
is not read). Settled by: the PC 1 read of `0x005DDC30` / `0x005DC970`
(the REC-1270 item) and `milestone-baal-throne` frame 50 (SuccubusWitch's
`Skill5` draw at `0x5e242e` follows a found S).

So `0x005DDC30` sees targets closer than 49; each caller applies its
own distance gate (the Hireable think: E < 25, `ai-bodies-6.md` §7
step 8, `0x005E55A3` `cmp [E], 0x19; jae`). The hireling's effective
engage range is therefore full-size distance < 25 — neither the 20 of
REC-100 nor the 35 of REC-279 (35 is the not-evil §5.2 step 4 scan 5
radius, a different search).
```
best = scan6(unit, d < 49); best = alt_choice(best); if best && dist(best) < 25 { hireling_attack(best) }
```

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
| `0x005DC640(a, b)` | `sub_6FCF14D0` | "can reach directly": d = the full-size distance `0x005DC380(a, b)`; offset k = table by d (d 0–2: 2, 3–10: 3, 11–24: 4, ≥ 25: 3); sx = sign(a.x − b.x)·k, sy = sign(a.y − b.y)·k (0 on an equal axis), positions as `0x006488C0` / `0x00648900` (static path +0x0C / +0x10 for missiles, items, tiles). Probes, in order, stopping at the first clear one: the line from a to (b.x, b.y), to (b.x − sy, b.y + sx), to (b.x + sy, b.y − sx), each `0x006229F0(a, x, y, 0x805)`. Returns 1 when a probe is clear, 0 when all three are blocked. No draws |
| `0x006229F0(a, x, y, mask)` | - | a without an active room (`0x00620BB0`): 0 (clear); else `0x00622920` (the end pull and line test of `render/draw-order-2.md` §15.1 rules 2–6) from a's position and size to the point (x, y) read as an end of size 2, in a's room; nonzero = blocked |
| `0x00622AA0(a, b, 4)` | `UNITS_TestCollisionWithUnit` | blocked line between a and b with mask 4 (`render/draw-order-2.md` §15–16) |
| `0x00622C40(a, b, 0)` | `UNITS_IsInMeleeRange` | combat flag (`sim/units.md`) |

### 7. Tactics helpers

#### 7.1 Mode requests

| 1.14d | D2MOO | Effect |
|---|---|---|
| `0x005DDF90(mode, target)` | `AITACTICS_ChangeModeAndTargetUnit` | mode change with a target unit; request flag 1; the path step count is not set; no skill is set, so the builder's clear leaves the used skill entry (`0x00620250`) **none** for the whole mode (a plain A1 / A2 has no skill entry: every reader of the used entry takes its "none" branch, e.g. `skills/bodies.md` §-rules "no used skill entry → 0", `sim/units.md` attack weapon → D; Attack (skill 0) is **not** substituted; 2026-10-09 read of `0x005DDF90` → `0x005A7E60` → `0x00620210(U, 0)` → `0x00643990`: list +0x10 := 0) |
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
(AI param record +0x18 method, +0x1C speed, +0x20 steps, §7.3)
replaces them field by field and is cleared; then path type 100 = no
path (the path is not touched; counter index 0), 101 = path type 13,
any other value is the path type computed (`0x005A6290`; a failed
compute of types 2, 7, 9 or 13 retries with type 15). Exact steps:
§7.5 rules 4–5. Flag ≠ 0: when the unit's mode after the change equals the
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
| `0x005DE6D0(t, a, b)` → `0x005DE4E0` | `AITACTICS_WalkInRadiusToTarget` | walk to the point that brings the distance to t toward b by at most a. `0x005DE4E0(game, U, t, mode 2, a, b)`, 1.14d-confirmed (settles REC-501): d := full-size distance `0x005DC380(U, t)`; s := −1 if d < b else +1; k := min(\|d − b\|, a). ax = \|t.x − U.x\|, ay = \|t.y − U.y\|, n := max(ax + ay, k); if n > 0: kx := ax·k / n, ky := ay·k / n (truncated), then **while kx + ky < k: kx += 1, ky += 1** (both, so the sum may pass k). Point = (U.x + sign(t.x − U.x)·kx·s, U.y + sign(t.y − U.y)·ky·s) (sign 0 on an equal axis). No early exit: k = 0, or t on U's position, gives U's own position and the request is still made. Request: `0x005A7E60(U, 2, rec)`, path step count 1 (`0x00649070(path, 1)`), coordinates into the record, `0x005A7C20(game, rec, 1)`; the result is not read. Warriv's recorded arrival walks (`-seed 1234`, player at (4873, 4228)): (4866, 4235)→(4868, 4233) with (3, 2), →(4869, 4232) with (2, 2), →(4870, 4231) with (1, 2). Second recorded case (`town-ama-10k` frame 287, re-read 2026-10-09 PC 1 late, decompile `0x005DE4E0`): Warriv at (4870, 4231), player at (4876, 4218), (3, 2) → (4871, 4228) (n = 19, (0, 2) fixed up to (1, 3)); a "round to nearest of Δ·k / dist" reading gives (4871, 4229), so this vector separates the two (`world/npc.md` Test vectors). A target equal to U's own position: §7.5 rule 8 (no points, neutral at once, think at f + `aidel`; settles REC-665) | none |
| `0x005DF680(t, n)` | `AITACTICS_RunCloseToTargetUnit` ("run near t n") | 15 (2 with the velocity reset if state 60), point near t, 1, no flags; returns the mode-change result | as wander, around t, n as a byte |
| `0x005DEF30(x, y)` | `WalkToTargetCoordinatesNoSteps` | 2, coordinates, 0, no flags; returns the mode-change result | none |

All draws are from the moving unit's seed (unit +0x20). 1.14d-confirmed
for `0x005DEB60`, `0x005DE200`, `0x005DF7D0`, `0x005DEFE0`, `0x005DF140`,
`0x005DED40`, `0x005DF680`, `0x005DEF30`, `0x005DED90`, `0x005DEDE0`,
`0x005DEE50`, `0x005DE4E0`.
```
k = min(|d - b|, a); kx, ky = ax*k/n, ay*k/n; while kx+ky < k { kx += 1; ky += 1 }; p = U + sign(t - U)*(kx, ky)*s
```

#### 7.3 Velocity request

`0x005DE190(unit, method, speed, steps)` asserts −126 ≤ speed ≤ 126,
maps method 1 to 7, and writes into the monster's AI param record
(monster data +0x2C) through `0x005A6260`: each nonzero argument
overwrites its field (+0x18 method, +0x1C speed bonus, +0x20 steps, steps
capped at 77). The record is consumed by the next non-GH mode request
(§7.5 rule 4.1). 1.14d-confirmed.

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

#### 7.5 Path target and re-path budget on a mode request

Every AI mode request of §7.1–§7.2 (and idle `0x005DE080`, when the unit
is not already neutral) builds a mode-change record and calls the
monster mode set `0x005A7C20(game, record, flag)` (`sim/units.md` §4.6);
the path fields are set there, not in the AI functions:

1. Unit not a monster (type 1) → nothing. Requested mode 3 (GH) → no
   path change (the budget keeps its value).
2. Any other mode, before the mode's start function runs (so also when
   the start then fails and neutral runs instead): record target unit
   (+0x08) set → path target := that unit (`0x00648B90`); else path
   target := record (x +0x0C, y +0x10), target unit none (`0x00648AD0`)
   (`sim/pathing.md` §1.2). Idle's record targets the monster itself.
3. Then the re-path budget (path +0x94) := 20 (`0x006490E0(path, 20)`),
   whatever the mode, moving or not.

So each AI request refills the budget to 20, and a monster re-paths
(`sim/pathing.md` §9.10) until it has advanced 20 points since its last
request. `0x006490E0` has no other caller and no other code writes path
+0x94 except the drain `0x00649140`; the path allocation zeroes it
(`sim/path-placement.md` §2.4), so a monster that has made no request
yet does not re-path. 1.14d-confirmed (`0x005A7C20` at `0x005A7CC9`–
`0x005A7CFF`; xref of `0x006490E0`; `byte [r + 0x94]` stores in
`all.asm`).

Rules 4–7 (1.14d-read 2026-10-08, gaps MV4–MV7 of
`docs/handoff/impl-path-motion.md`; `0x005A7D1F`–`0x005A7D2F`,
`0x005A63F0`, `0x005A6290`):

4. **Movement set-up** `0x005A63F0`, after rule 3, for every mode but
   GH. Inputs: P = the AI param record (monster data +0x2C), t = record
   byte +0x15, v = record dword +0x18 (speed), n = record byte +0x1C
   (steps).
   1. Velocity request: P +0x18 ≠ 0 → t := P +0x18 (the method replaces
      the path type, also a 100 or 101 byte); P +0x1C ≠ 0 → v := P
      +0x1C; P byte +0x20 ≠ 0 → n := it. Then P +0x18, +0x1C, +0x20 :=
      0. So any non-GH mode request consumes the request, also one whose
      start then fails.
   2. t = 100 → c := 0, v := 0; nothing is written to the path (type,
      points and step counts keep their values).
   3. Else n = 0 → n := 5; t = 101 → t := 13; c := compute(t, n) (rule
      5).
   4. P +0x14 := 10 when t ≠ 100, c ≠ 1 and the path has a target unit
      (`0x00648BF0`), else −1. P +0x10 := v.
   5. Counters: game +0x1D70 + 4·c += 1; c ≠ 0 → game +0x1DB4 += 1 (no
      reader in this spec).
5. **Compute** `0x005A6290(t)` (U, P, n as in rule 4): step counts :=
   n (`0x00648E70`, `sim/pathing.md` §13.1 rule 1). Target cache: when
   (path target unit, target x, target y) (`0x00648BF0`, `0x00648A00`,
   `0x00648A10`) differ from P (+0x04, +0x08, +0x0C), store them there
   and P bytes +0x00, +0x01 := 0. Path type := t through **set type**
   `0x00648CF0` (`sim/pathing.md` §2: the flag, previous-type and saved
   velocity rules apply), then compute `0x00649970(path, U, 0)` (town
   access 0). Point count ≠ 0 → U queued for update (`0x0064C040`), U
   flags (+0xC4) |= 1, P bytes +0x00, +0x01 := 0, c := t. Point count
   0 and t ∈ {2, 7, 9, 13} → U queued, flags |= 1, set type 15,
   compute again (town access 0), c := 15 whatever it finds. Other t →
   c := t (no points).
6. **Callers without a target** (MV6). Every caller of `0x005A7C20`
   goes through rule 2; nothing keeps the old target. A record with
   +0x08 = 0 sets the target point (+0x0C, +0x10), which the builder
   `0x005A7E60` and the inline records leave at (0, 0) unless the
   caller writes them, and clears the target unit. Mode 3 requests
   (reaction GH, KB event 1 `sim/units.md` §4.6 rule 14) change no path
   field (rule 1). A non-moving mode never computes (byte 100, rule
   4.2), so a (0, 0) target matters only for a moving request or a
   pending velocity request (rule 4.1).
7. **Path step count and stop path** (MV7): the "path step count" of
   §7.1–§7.2 is the stop distance, path +0x93 (`0x00649070`,
   `sim/pathing.md` §9.5 rule 3). "Stop the path" (§1.2 table, the NPC
   interaction at `0x00548B95`, AI bodies) is `0x00648730`:
   `sim/pathing.md` §13.1 rule 3.
8. **Zero-length walk** (settles REC-665; 1.14d-read 2026-10-09,
   `0x005DE4E0`, `0x005A7C20`, `0x005A6290`, `0x00649970`, `0x005A7520`,
   `0x005A73E0`). A mode-2 request whose point is U's own cell (§7.2
   walk-in-radius with k = 0 or t on U: the point is built from
   `0x006488C0` / `0x00648900`, the same sub-tile the compute starts
   from) runs rules 2–5 in full: target := the point, target unit none,
   budget := 20, the velocity request consumed, step counts := n (5
   unless the request gave steps), stop distance := 0 (path step count
   1). Compute type 13: start = target → no path function, index :=
   count := 0, flag 0x20 := 0 (`sim/pathing.md` §3 steps 5, 11–12).
   Type 13 retries: U queued, U flags |= 1, set type 15, compute again
   (same exit), c := 15. So the path is left at **type 15** (flags of
   type 15) with 0 points; P +0x14 := −1 (no target unit); counters
   game +0x1D70 + 4·15 and +0x1DB4 each += 1. The WL start
   `0x005A7520` sees 0 points and returns 0 (`sim/units.md` §4.6 rule
   5): current skill := none (`0x00620210(U, 0)`), neutral start
   `0x005A73E0`: set mode 1 (a monster already in mode 1 is not
   re-queued by the set itself), and when U has no type-2 timer
   expiring after f, think at f + `aidel` (f + 45 with state 21,
   §1.3). Mode 1 has no schedule flag, so nothing else is scheduled;
   the unit never enters walk and no event 0 / mode end (§1.4) runs.
   Message: the compute's flags |= 1 makes the update pass send the
   neutral mode message to each client of U's rooms: `0x00597E20`
   with mode 1 (current skill already none, so not the skill branch)
   takes the mode-1 case (`0x00598067`–`0x005980B0`) and sends **S→C
   0x6D** (`0x0053BB70`: GUID, U's cell x, y, life byte
   `0x005A5650(U)`), then stat 328 += 1, and returns before any 0x67 /
   0x68 (`sim/intents-events.md` §7.4 rule 5; 1.14d-read 2026-10-09,
   corrects the earlier "0x67 code 7"). Draws: none
   (the point, the request, both computes and the neutral start draw
   nothing). Next think: f + `aidel` (15 for the Act 1 classes of the
   recordings) unless the AI body schedules or deletes thinks itself.
   ```
   point == U.cell: type 13 -> 0 pts -> type 15 -> 0 pts; WL start 0 -> NU; think f + aidel; 0x6D, stat 328 += 1
   ```
9. **Mode damage** (1.14d-read 2026-10-09, `0x005A7D34`–`0x005A7D39`):
   after rule 4 (`0x005A63F0` returns at `0x005A7D34`), still inside
   the m ≠ 3 branch, `0x005A4F50(U, m)` with m = the requested mode
   (record +0x00, kept at [ebp − 4]) rewrites U's base `tohit`,
   `mindamage`, `maxdamage` and element stats for m
   (`skills/bodies-2.md` §2.1), then umod mode 0 (`0x005A4350` at
   `0x005A7D42`) and the start function. Every non-GH request (also
   one whose start then fails, and the creation mode) runs it; a
   monster's melee to-hit and damage are those of its last such
   request (`combat/damage.md` §10, checked by a 1.14d recording).

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
| `0x0058F0D0` | `GetMinionOwner` | minion owner unit, or 0: 0 when control +0x28 is 0; else the unit of type +0x30, GUID +0x2C (`0x00552F60`) |
| `0x0058F030(game, unit, GUID, type, a, b)` | — (set owner data) | monsters only: b ≠ 0 → `0x005DD230(control, 2, 1)`; a ≠ 0 → `0x005DD230(control, 1, 1)`; then control +0x2C := GUID, +0x30 := type, +0x28 := game (this is what makes `0x0058F0D0` answer). Party leaders get their own GUID (`population.md` §10.2.1), so `0x0058F0D0(leader)` = leader (`ai-bodies.md` §9.4 rule 6) |
| `0x0058F100(game, leader, minion)` | — (add minion) | new 8-byte node {minion GUID (+0x0C, −1 for none), next}, pushed at the **head** of control +0x34, so the list runs newest first |

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
draw nothing. Apart from the forced-target mode-3 draw (item 2), the
searches of §5 draw nothing on the thinker's seed; the other draw inside
them is the scan 6 filter's `roll(100)` on a
**candidate player's** seed when that player has state 146 (§5.3 rule
1.5). A body's search call (e.g. `0x005DDC30`) therefore decides
whether the body's next draw happens (its distance gate), without
drawing itself: FallenShaman and Fallen site-by-site lists are in
`ai-bodies.md` §9.6 item 8 and §9.4 item 8 (1.14d-confirmed, read
2026-10-09, PC 1 late). Skill draws after a mode request belong to the skills
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

- 2026-10-10 (rc-mon-spawn-think, Ghidra exports read in the cloud): `0x005DC640`, `0x006229F0`, `0x00622920` (register arguments from `all.asm`: ECX a.x, EDX 2 = the point's size, EAX x; stack a.y, size(a), y, room, mask); §6 rows. Recorded: `traces/checks/gen/gen-mon-436.check` frame 31 (ReanimatedHorde's step-3 roll needs the direct line clear).
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
- 2026-10-09 (pc1-day3-c, read in `0x0053A9B0`, `0x005738D0`,
  `0x00540E60`, `0x005B1740`, `0x005E7130`): §1.5 rule 3. When the last
  client leaves a room, the think is cancelled. Evidence:
  `traces/checks/a4-warp-plains-ama.check`.

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
   clears the preset's). An object preset's path goes instead to
   `0x00545C90` (objects 459, 461, 543 only), which hands a copy to the
   Act V quest NPC it spawned (Anya, Nihlathak, Larzuk:
   `world/quests-act5.md` §3.8 "Where Larzuk's map AI comes from", §5.8);
   Act III's equivalents are dead (`world/quests-act3-2.md` §11.6). The idle-10 source is settled in
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
