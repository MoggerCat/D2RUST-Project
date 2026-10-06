# Spec: Monsters — AI think

- **Status:** draft: think scheduling, dispatch, AI tables and the
  shared helpers read from the 1.14d `Game.exe` (addresses below; AI
  tables dumped from the file); 17 AI functions read in full; think
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
| Summary | 49–67 |
| Inputs | 68–79 |
| Outputs / state changes | 80–90 |
| Rules | 91–92 |
|   1. Think scheduling | 93–224 |
|   2. Think dispatch `0x005B1740` | 225–339 |
|   3. AI control and AI tables | 340–435 |
|   4. AI parameters | 436–454 |
|   5. Target selection | 455–537 |
|   6. Distances and line tests | 538–550 |
|   7. Tactics helpers | 551–605 |
|   8. AI commands and minions | 606–626 |
|   9. Per-AI behaviours | 627–841 |
|   10. The catalogue `ai-functions.tsv` | 842–862 |
| Constants & data dependencies | 863–884 |
| Randomness | 885–903 |
| Edge cases & original bugs | 904–928 |
| Test vectors | 929–1002 |
| Provenance | 1003–1035 |
| Open questions | 1036–1066 |
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
think is not rescheduled by the dispatcher: the frozen monster's next
think comes from whoever ends the freeze (open question 1).

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
      (`0x0054DC40`; its own draws: open question 4) in the room. If found
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
`dwAiState` read through `0x005734E0`; it is "true" for values 3 and 19.
Who sets it is owned by `sim/units.md` / `monsters/init.md` (open
question 5).

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

D2MOO `sub_6FCF2920`. Runs first. A monster carrying a target override
(D2MOO `sub_6FC61EC0`: 1 player, 2 monster, 4 missile by GUID; 3 =
alignment switch) returns that unit (distance full-size, §6) unless it
is gone, dead, or (when asked) blocked; a failing override is cleared.
Mode 3 draws one raw step (`lo'` bit 0) for neutral-aligned units. Owned
here as D2MOO-derived; 1.14d address confirmed as the first call of
`0x005DD7F0`, details open (question 6).

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
6. No target: combat := 0, distance := M (0x7FFFFFFF when no player
   qualified). Return 0.
7. Target: if alignment ≠ good (2): set control flag 0x08 and vision
   +0x24 := (it was 0). Combat := melee-range test `0x00622C40(unit,
   target, 0)` (`sim/units.md`). Distance := B.

Draws: none here (except §5.1 mode 3). Ties keep the earlier node.
Slot order and node order are the target-node list order (owned by
`sim/units.md` / clients; open question 7). 1.14d-confirmed
(`0x005DD7F0`); the slot meanings are D2MOO's.

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
| `0x005DC640` | `sub_6FCF14D0` | "can reach directly": offset k = table by distance (2 ×3, 3 ×8, 4 ×14, else 3); tests three points (target, and target ± the perpendicular offset) for collision mask 0x1/0x4/0x400 (D2MOO wall, missile barrier, door); fails only if all three collide |
| `0x00622AA0(a, b, 4)` | `UNITS_TestCollisionWithUnit` | blocked line between a and b with mask 4 (`sim/units.md`) |
| `0x00622C40(a, b, 0)` | `UNITS_IsInMeleeRange` | combat flag (`sim/units.md`) |

### 7. Tactics helpers

#### 7.1 Mode requests

| 1.14d | D2MOO | Effect |
|---|---|---|
| `0x005DDF90(mode, target)` | `AITACTICS_ChangeModeAndTargetUnit` | mode change with a target unit |
| `0x005DDFC0(mode, x, y)` | `…ChangeModeAndTargetCoordinates` | mode change at coordinates |
| `0x005DE000(skill, target, x, y)` | `AITACTICS_UseSequenceSkill` | skill id in range: mode 14 (sequence), current skill := skill, path step 1, no fallback |
| `0x005DEAD0(mode, skill, target, x, y)` | `AITACTICS_UseSkill` | mode < 16: current skill := skill, unit flag 0x40, path step 1; if the mode change fails: idle 10 |
| `0x005DE190(method, speed, steps)` | `AITACTICS_SetVelocity` | §7.3 |

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
| `0x005DED90`, `0x005DEDE0`, `0x005DEE50` | walk / run (decrepify → walk) / walk + delete thinks on failure, to coordinates | | none |
| `0x005DE200(n)` | `AITACTICS_WalkCloseToUnit` ("wander n") | 2, point near itself, 1, 0 | three or four: (a) step, `lo'` bit 0 = 1 → x offset n, y offset `roll(n)`; bit 0 = 0 → x offset `roll(n)`, y offset n; (b) step, bit 0 = 1 → negate x; (c) step, bit 0 = 1 → negate y. `roll(n)` steps only if n ≥ 1 |
| `0x005DF530(t, n)` | `WanderToTarget` / `WalkToOwner` | same draws, around unit t | as wander |
| `0x005DEFE0(t, n, del)` | `D2GAME_AICORE_Escape` | if n > 5 velocity request steps n; walk to (own x + sign(own x − t.x)·n, own y + sign(own y − t.y)·n), step 1, flags del ? 4 : 0 | none |
| `0x005DF140(t, n, del)` | `sub_6FCD06D0` | same, running | none |
| `0x005DF7D0(t, n, del)` | `sub_6FCD0E80` ("circle n") | one step: low byte of `lo'` < 128 → velocity method 5, else 6, with steps n; then walk toward t, step 1, flags del ? 4 : 0 | one |
| `0x005DE6D0(t, a, b)` → `0x005DE4E0` | `AITACTICS_WalkInRadiusToTarget` | walk to the point that brings the distance to t toward b by at most a | none |

All draws are from the moving unit's seed (unit +0x20). 1.14d-confirmed
for `0x005DEB60`, `0x005DE200`, `0x005DF7D0`, `0x005DEFE0`, `0x005DF140`,
`0x005DED40`; the walk-in-radius geometry is D2MOO's.

#### 7.3 Velocity request

`0x005DE190(unit, method, speed, steps)` asserts −126 ≤ speed ≤ 126,
maps method 1 to 7, and writes into the monster's AI param record
(monster data +0x2C) through `0x005A6260`: each nonzero argument
overwrites its field (+0x18 method, +0x1C speed bonus, +0x20 steps, steps
capped at 77). The record is consumed by the movement code
(`sim/units.md`). 1.14d-confirmed.

### 8. AI commands and minions

A command (D2MOO `D2AiCmdStrc`, 0x1C bytes: next, prev, five i32 params
at +0x08) lives in a ring hung on the control (+0x20 current, +0x24
last). Param 0 is the command type.

| 1.14d | D2MOO | Effect |
|---|---|---|
| `0x0058EE80` | `GetCurrentAiCommandFromUnit` | current command or 0 |
| `0x0058ED10` | `FreeCurrentAiCommand` | unlink and free the current one; current := its next |
| `0x0058EDE0` | `FreeAllAiCommands` | |
| `0x0058EF40` | `CopyAiCommand` | new command with the given params, inserted before current, becomes current |
| `0x0058EFA0` | `SetCurrentAiCommand(type, set)` | find by type (search starts at current's next), create it with params (type, 0, 0, 0, 0) if absent |
| `0x0058F730` | `AllocCommandsForMinions` | copy the command to every minion of this unit's minion owner (control +0x2C/+0x30), in minion-list order |
| `0x0058F0D0` | `GetMinionOwner` | minion owner unit, or 0 |

Command types used by Act 1 AIs: 1 = "attack now" (Fallen, FallenShaman
minions), 10 = home position (NPCs, BloodRaven: params 1, 2 = x, y), 4,
5, 7 = NPC map actions. QuillRat reads params 1, 2 as a unit type and
GUID. 1.14d-confirmed for the functions listed.

### 9. Per-AI behaviours

Conventions: T = the dispatch target, D = its distance, C = combat flag,
P(aipN) = one unit-seed step tested `lo' % 100 < aipN` (§4), "pct(k)" =
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
start (§1.3), since the fallen deleted its thinks in step 1.

#### 9.5 Brute (7) `0x005EFB80`

1. C: P(aip3) [100] → P(aip4) [45] → A1 else A2. Else a second P(aip3):
   pass → circle 4 at T (one more draw, §7.2); fail → idle 15.
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

#### 9.9 Npc (32) `0x005E7130` (summarized)

Target mode 0. Order (1.14d): `0x005E6800` (home command: if command
10 has no position yet, store the current position and idle 20; the
first think of every town NPC); class cases 0xC9 (jerhyn: idle 40/20/50
by the palace quest state), 0xFE, 0xFF, 0x109, 0x200 (quest movers;
quest specs); then three handlers, each of which may end the think:
`0x005E68F0` (D2MOO `sub_6FCE5EE0`: interaction; while someone interacts
or AI param 0 > 0, stay or return home and idle 8, counting param 0
down, which `0x00548B00` set to 40; otherwise a player within 15 at
distance ≤ 2 or ≥ 24 gets a greeting, sound 18 at most once per 60
thinks, idle 20, and a player in between makes the NPC walk around him),
`0x005E6AE0` (D2MOO `sub_6FCE69A0`: command 4/5/7 path actions with
their own idles 8–50), `0x005E7080` (D2MOO `AITHINK_ExecuteMapAiAction`:
pct(66) passes → `roll(node count)` picks a preset path node, whose
action runs); if all three decline: idle 8. Only the top-level order is
1.14d-confirmed; the handler details are D2MOO's. Recorded: idle 8 and
10 dominate (Test vectors); the source of 10 is open (question 8).

#### 9.10 Navi (58) `0x005E7E20`, TownRogue (62) `0x005E7D60`

Navi (Flavie, target mode 0):

1. Interacting (`0x00572DC0`) → idle 10. End.
2. P = nearest player within 15 (`0x005DDF20`). If P is a player, not
   busy (`0x00535060`), `roll(3)` ≠ 0, and P is closer than 4: AI param 1
   = 0 → param 1 := 60, sound 18 to P, idle 20; else clamp param 1 at 0
   and count it down, idle 20. End. (`roll(3)` is drawn only when P is a
   non-busy player.)
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

Summaries in `ai-functions.tsv` for Bighead (4), BloodHawk (5),
CorruptRogue (10), SkeletonBow (37), FoulCrowNest (43), BloodRaven
(59). Their 1.14d functions are listed but not yet compared.

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
| `monstats_rows` | count of live monstats rows using the index, then up to three "row Id" pairs |
| `aip_meaning` | the monai.txt `*aipN` comment headers (hints, not data) |
| `summary` | one-line behaviour, `-` when unread |
| `status` | `spec'd-here` (full rules in §9, 1.14d read), `summarized` (top-level order read in 1.14d), `D2MOO-only` (summary from D2MOO 1.10f, 1.14d not compared), `unread` |

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
| monstats | `AI` +30, `aidel*` +79, `aidist*` +82, `aip*` +86, `isMelee`, `opendoors`, `npc`, `interact`, `switchai` bits of the flags at +12, `Skill1..3` +368/+370/+372, `Sk*mode` bytes +0x180.., `SplEndGeneric` +422, `MonStatsEx` +24, `BaseId` +2 | `data/fields.tsv` |
| monstats2 | `MeleeRng` +14 | `data/fields.tsv` |
| levels | `MonSpcWalk` +47 | `data/fields.tsv` |
| objects | `MonsterOK` +365 | `data/fields.tsv` |

## Randomness

All AI draws use the thinking monster's unit seed (unit +0x20, `rng.md`
§5.3, §7) with the step of `rng.md` §2; forms: `lo' % 100` (inline or
`roll(100)` via `0x0045C390`), `roll(n)` (`0x0045C3E0` in wander),
`lo'` bit 0 (wander, forced-target alignment), low byte < 128 (circle),
`lo' % 20` (Fallen sound), `roll(3)` (Navi). Per think, the order is:

1. Precheck C teleport (§2.4): 1–3 draws, only for flag-0x20 monsters
   with a target.
2. Target acquisition: none (forced-target mode 3: 1).
3. Target mode 1 with no target: wander 5 (3–4 draws) or none.
4. The AI function's draws in the order of its §9 entry, including the
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

## Test vectors

Synthetic (CI-safe; seeds are `{lo, hi}` of the unit seed before the
think; draws per `rng.md` §2):

| Seed | Raw `lo'` of the first four steps | `lo' % 100` |
|---|---|---|
| {1, 666} | 1791398751, 791599131, 671516612, 3064641593 | 51, 31, 12, 93 |
| {12345, 666} | 22752887, 2337785264, 1617882871, 4008788125 | 87, 64, 71, 25 |
| {3735928559, 666} | 3119800453, 2769797046, 1466979120, 4097238796 | 53, 46, 20, 96 |
| {4014346870, 666} | 2928842600, 1513463342, 1084273713, 4002178480 | 0, 42, 13, 80 |

| Case (Normal aip of §9.1; unit at (100, 100)) | {1,666} | {12345,666} | {3735928559,666} | {4014346870,666} |
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
| fallen1 | 2,280 | 25 ×1763, 10 ×301, 15 ×10, 5 ×1 | 15 ×33 | 25 ×18, 10 ×118, 15 ×4, 5 ×6 | idle 10 / 5 rules of §9.4 |
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
  `0x0058EC00`–`0x0058F730` (commands), and the AI functions of §9.
- Tables dumped from the file's .rdata/.data with a PE-section parser:
  AI and special-state tables (`0x0073CA18`, 166 records), monster
  event table `0x006E2490`, mode table `0x006E2260`, inline-think bytes
  `0x0073C6D0`, scan table `0x006E3300`; bytes at `0x005A7F70`,
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
- Recordings: `traces/raw/20261006-015554-tick.jsonl`,
  `-021854-tick.jsonl`, `-022304-tick.jsonl` (`tick-raw-1`); counts by
  a script over `hin`, `set`, `ex`, `cancel` records (Test vectors).

## Open questions

1. What reschedules a frozen monster's think when the freeze ends in
   1.14d (D2MOO `SUNITDMG_RemoveFreezeState` has no 1.14d `aidel` copy;
   only `0x005A73E0` and three AIs read `aidel`)? Settle: find the state
   1 remove callback and record a freeze (cold damage) on a monster.
2. Game +0x6A and +0x74 values in single player (decides whether Nightmare
   and Hell `aidel` columns are ever used): record a Nightmare game and
   compare mode-end delays with `aidel(N)` (14 for zombie1).
3. The `0x005A8520` knockback-end branches have no recorded instance:
   record a knockback (e.g. a player skill with knockback on fallen and
   on a sand leaper) and check +1 / +15 / gethit.
4. `0x0054DC40` (teleport spot search): which seed it draws from and in
   what order (`monsters/population.md` owns spot search?).
5. Who sets monster data `dwAiState` (+0x54) to 3 or 19, read by
   `0x005734E0`.
6. Forced targets `0x005DD610`: full 1.14d read (taunt/attract/confuse
   records, the mode-3 draw).
7. Target-node list slots 8 and 9 contents and order (game +0x10F8): read
   the 1.14d target-node functions (D2MOO `Targets.cpp`).
8. Which Npc-AI path produces the dominant 10-frame idle (map-AI actions
   `0x005E7080` or `0x005E6AE0`)? Settle: read those two functions.
9. The 2 recorded fallen1 type-1 runs with no schedule and later
   activity: likely a death end followed by a shaman resurrect; check
   with a recording that hooks mode changes.
10. 119 AI functions are unread and 11 are D2MOO-only
    (`ai-functions.tsv` status column).
11. Which message handlers reach `0x00548B00` / `0x0054CA10` (NPC
    interaction start/stop, +1 think): settle with their callers and a
    recording that logs client messages next to timer schedules.
