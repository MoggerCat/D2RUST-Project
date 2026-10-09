# Spec: Missiles — server-side creation, flight, collision, hit, expiry

- **Status:** draft: creation, per-tick dispatch, default flight, hit
  handler, pierce and expiry read from the 1.14d `Game.exe` code (addresses
  per rule); function tables dumped from `Game.exe`; lifetimes of all 69
  missiles of `20261006-022304-tick.jsonl` match §R7 (69/69).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::missiles` (creation, dispatch, default
  flight, hit handler); per-function behaviours `d2-sim::missiles::srvdo`,
  `::srvhit`
- **Related specs:** `sim/tick.md` §5.3 (every-tick scheduling), §5.5
  (queue run), §5.6 (missile class handler); `sim/rng.md` §3, §5.3, §6;
  `sim/unit-order.md` §1 (GUIDs), §5–6 (room lists, update queues);
  `sim/units.md` (unit allocation/removal `0x00555230`/`0x00555600`, path
  movement and the collision-mask primitives, hostility test; being
  written); `sim/stats.md`, `sim/stat-lists.md` (stats, state stat lists);
  skills spec (damage setup and formulas, to-hit, block/dodge, damage
  application; `claude/phase3-skills`); `monsters/init.md` (monster
  unique-mod hooks run on missiles); `monsters/ai.md` (monster attacks
  that create missiles); `sim/server-messages.tsv` id `0x73`;
  `data/fields.tsv` (`missiles` rows, record 420 bytes); `srvdo.tsv`,
  `srvhit.tsv` (this folder).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–67 |
| Inputs | 68–79 |
| Outputs / state changes | 80–93 |
| Rules | 94–95 |
|   R1. Data the server keeps per missile | 96–140 |
|   R2. Creation | 141–358 |
|   R3. Per-tick dispatch | 359–388 |
|   R4. Default flight (server-do 1, `0x005B0BC0` → `0x005AE1F0`) | 389–519 |
|   R5. Hit handler (`0x005ADF10`, D2MOO `MISSMODE_SrvDmgHitHandler`) | 520–577 |
|   R6. Damage stage (missile-owned part) | 578–722 |
|   R7. Lifetime and expiry | 723–776 |
|   R8. Pierce | 777–803 |
|   R9. Server-do and server-hit catalogues | 804–1034 |
|   R10. Behaviour of the recorded missiles | 1035–1069 |
|   R11. `missiles.txt` columns and their server use | 1070–1117 |
| Constants & data dependencies | 1118–1144 |
| Randomness | 1145–1183 |
| Edge cases & original bugs | 1184–1210 |
| Test vectors | 1211–1293 |
| Provenance | 1294–1346 |
| Open questions | 1347–1428 |
<!-- /index -->

## Summary

A missile is a server unit of type 3 whose class is a `missiles.txt` row.
It is created by `MISSILES_CreateMissileFromParams` (`0x0059FA30`) from a
parameter record filled by skill, AI or missile code. Unit allocation
gives it a GUID, a unit seed derived from the game seed and an
every-tick type-0 event. Every frame the missile class handler runs the
row's server-do function (`pSrvDoFunc`, table `0x0073C768`, 53 entries);
almost all rows (551 of 684, all four missiles in the recordings) use
function 1, the default flight: move one step, count the frame down,
expire at 0, otherwise test the subtiles crossed this step for units and
missile barriers. Touching a unit or a barrier runs the hit handler
`0x005ADF10`: filters, to-hit roll, the row's server-hit function
(`pSrvHitFunc`, table `0x0073C840`, 71 entries), the damage stage (rolls
on the missile's own seed, then the skills spec's damage application),
and decides whether the missile dies (handler result 2 → removed by
`0x00555600` in the same frame) or flies on (pierce, `CollideKill` 0).

## Inputs

| Name | Type | Source |
|---|---|---|
| missile parameter record | 0x5C bytes (§R2.1) | creating code (skills, AI, missile functions) |
| `missiles.txt` record | 420 bytes, columns of `data/fields.tsv` table `missiles` | data tables |
| owner unit | player or monster | parameter record |
| game seed | `rng.md` §5.2 | one step per missile allocation |
| owner stats | skill_pierce 166, item_pierce 156, pierce_idx 328 (base), state 87 + skill_handofathena 161 | `stats.md` |
| room collision masks | per subtile | `units.md` collision primitive |
| frame counter | game +0xA8 | `tick.md` §2 |

## Outputs / state changes

- A missile unit (type 3) in the room, hash and timer lists, with
  missile data, a stat list (damage stats, pierce_idx 328, tohit 19,
  damage_framerate 327) and a path.
- One every-tick type-0 timer per missile (`tick.md` §5.3).
- Per hit: state 86 (`justhit`) stat list and a type-12 timer on the hit
  unit (rows with `NextHit`), unit event 0 (hit by missile), damage via
  the skills spec, missile seed draws.
- Removal: the missile unit is freed by `0x00555600` (`units.md`) on the
  frame its handler returns 2; the recording shows "hout" in that frame.
- Sub-missiles created by server-do/server-hit functions.
- Message `0x73` to clients only for rows with `ClientSend` (§R2.4).

## Rules

### R1. Data the server keeps per missile

1. **Record.** `missiles.txt` compiles to 684 records of 420 bytes
   (`missiles.bin`: count 684 at offset 0, measured; row id = record
   index, ids 0–683 contiguous). The record is found by
   `class × 0x1A4 + table base`; every lookup first checks
   `0 ≤ class < count` (table count at data-tables +0xB6C) and treats a
   failure as "no record".
2. **Flags dword** (record +0x04): bit n of the `bit` columns in
   `fields.tsv` (LastCollide 0, Explosion 1, Pierce 2, CanSlow 3,
   CanDestroy 4, ClientSend 5, GetHit 6, SoftHit 7, ApplyMastery 8,
   ReturnFire 9, Town 10, SrcTown 11, NoMultiShot 12, NoUniqueMod 13,
   Half2HSrc 14, MissileSkill 15). Tests use the bit-mask table
   `0x006CE268` (entry n = 1 << n, dumped).
3. **Signed reads.** Function indices (`pSrvDoFunc` +0x0C,
   `pSrvHitFunc` +0x0E, `pSrvDmgFunc` +0x10), `Range` +0x96, `LevRange`
   +0x98 and `Accel` +0x9E are read as **signed** 16-bit. A function
   index ≤ 0 means "none". `Vel`, `VelLev`, `MaxVel`, `Activate`,
   `SubLoop/Start/Stop`, `CollideType` and the other byte columns are
   unsigned bytes.
4. **Missile data** (D2MOO `D2MissileDataStrc`, 0x34 bytes, pointer at
   unit +0x14, zeroed at allocation, `0x0064A100`):

| Offset | Field | Notes |
|---|---|---|
| +0x08 | activate frame (i16) | §R2.3 step 13 |
| +0x0A | skill (i16) | setter clamps to 0…0x7FFF (`0x0064A240`) |
| +0x0C | level (i16) | stored unclamped (`0x0064A1E0`) |
| +0x0E | total frames (i16) | |
| +0x10 | current frame = frames left (i16) | counts down (§R4) |
| +0x14 | flags | bit 1 from parameter flag 0x10000 |
| +0x18, +0x1C | last-collided unit type, GUID | GUID −1 = none; used only with `LastCollide` (§R5.1) |
| +0x28, +0x2C | target fields (D2MOO `nTargetX/Y`, a union) | free state for server-do functions |

   The frame setters (`0x0064A2B0` total, `0x0064A330` current,
   `0x0064A5F0` activate) clamp the value to −0x8000…0x7FFF. "Elapsed
   frames" (D2MOO `MISSILE_GetRemainingFrames`, `0x0064A3B0`) is total −
   current; D2MOO's name is misleading.
5. **Unit fields used:** mode (unit +0x10) = the row's `CollideType`
   (set at allocation, §R2.3 step 9); unit seed +0x20 (`rng.md` §5.3);
   unit flags +0xC4: bit 2 (D2MOO `UNITFLAG_CANBEATTACKED`) set by
   `CanDestroy`, bit 3 (`UNITFLAG_ISVALIDTARGET`) and bit 1 cleared;
   path +0x2C; unit size = `Size` (record +0x18A, read by `0x00620510`
   for missiles).

### R2. Creation

#### R2.1 Parameter record (D2MOO `D2MissileStrc`, 0x5C bytes)

| Offset | Field | Use |
|---|---|---|
| +0x00 | flags | below |
| +0x04 | owner | required; player or monster |
| +0x08 | origin unit | start position when flag 1 is clear; damage source (skills spec) |
| +0x0C | target unit | optional |
| +0x10 | missile class | `missiles.txt` row |
| +0x14, +0x18 | x, y | start position with flag 1 |
| +0x1C, +0x20 | target x, y | flag 2: offset from start; flag 0x20: absolute |
| +0x24 | gfx argument | not read by `0x0059FA30` |
| +0x28 | velocity | with flag 4 |
| +0x2C | skill id | stored |
| +0x30 | skill level | stored; used in velocity, range and damage |
| +0x34 | loops | with flag 8 |
| +0x40 | start frame | with flag 0x200 |
| +0x44 | activate frames | with flag 0x800 |
| +0x48 | attack bonus | with flag 0x1000 → stat tohit (19) |
| +0x4C | range | with flag 0x8000 |
| +0x50 | light radius | not read by `0x0059FA30` |
| +0x54, +0x58 | init callback, argument | called once (step 21) |

Flags read by `0x0059FA30`: 1 position given, 2 target relative, 4
velocity given, 8 add loops, 0x10 velocity already fixed point, 0x20
target absolute, 0x200 start frame, 0x400 frames from distance, 0x800
activate given, 0x1000 attack bonus, 0x8000 range given, 0x10000 missile
data flag 2. Other bits are ignored here.

#### R2.2 Entry points

`0x0059FA30` (82 call sites in 82 functions, all in skill, AI and
missile code: e.g. `0x0056D4E0` ×4, `0x005A9370` ×2, `0x005C9EA0` ×3,
`0x005DB410` ×3; full list: every call site of `0x0059FA30`) is the only creator. The
convenience wrappers (D2MOO `D2GAME_CreateMissile_6FD115E0` and the skill
helpers) fill the record and call it; they belong to the skills spec.
Missile-owned helpers: `0x005A9720` (D2MOO
`MISSMODE_CreatePlagueJavelin…HitSubmissiles`) and `0x005A9820` (§R9.3).

**Monster mode missile** `0x005A6D50(ECX game, EDX unit, mode)`
(1.14d-confirmed, asm). Its only caller is the monster attack-family
event 0 `0x005A7670`, in its branch for a monster with no used skill
(`0x005A776E`). That event's tests are `skills/use.md` §5.2 (Open
question 6).
1. Unit null → 0. m := the monstats missile of the mode
   (`0x0063E6B0(unit, mode)`, table in `skills/bodies-3.md` §5.18 step
   3: `MissA1`, `MissA2`, `MissS1`…`MissS4`, `MissC`, `MissSQ`); m < 0
   → 0, and the caller then runs the melee fallback `0x005A5490`.
2. L := `difficultylevels` `MonsterSkillBonus` (record +0x10 of
   `0x00611D30(game +0x6D)`) + 1, i.e. 1 / 4 / 8 on Normal / Nightmare
   / Hell. A unit with unit flag 0x200 (+0xC4 bit 9, a hireling) uses
   its own total stat 12 (level, `0x00625480(unit, 12, 0)`) instead.
3. Create straight: `0x0056ECB0` (`skills/use.md` §5.5) with class m,
   skill 0, level L, offsets 0, aim 0, take ammo 1. When it returns a
   missile, run the quill volley `0x005A6B70`. Return 1, whether or not
   the missile was made.
4. **Quill volley** `0x005A6B70(game, unit)`. Only for a monster whose
   class row has `BaseId` (monstats +0x02) = 63 (`quillrat1`).
   1. Save the path target unit (`0x00553540`). (x, y) := its position,
      or the path target point when there is none. Clear the target
      unit (`0x00648B90(path, 0)`).
   2. Local seed {lo 0x53454953, hi := unit GUID (+0x0C)}
      (`0x00650E30`, `0x00650E40`, `sim/rng.md` §4). This is not the
      unit's seed and not the game's seed, so no game draw is made.
   3. Set sx := 5, sy := 5. Repeat `aip3` times, using the column for
      the difficulty (monstats +0x62 + 2d):
      - Step the seed; low bit 1 → sx := −sx.
      - Step it again; low bit 1 → sy := −sy. The signs carry over
        from one spike to the next.
      - Target point := (x + sx, y + sy) (`0x00648AD0`).
      - Create straight with class m, skill 0, **level 1**, offsets 0,
        aim 0, take ammo 0.
   4. Restore the saved target unit.
   1.14d data: `aip3` is 0 on every difficulty for all eight
   `quillrat1`-based rows, so the volley creates nothing.

#### R2.3 Steps, in order (`0x0059FA30`, 1.14d-confirmed)

1. Fail (return none) unless: owner set, class in `0…count−1`, owner
   type 0 or 1, record found. (1.14d checks the class range and owner
   type first; D2MOO 1.10f checks the owner only.)
2. Start position: flag 1 → (x, y) from the record; else origin
   required (fail without) and its position is used.
3. Room: the room containing the start subtile, searched from the
   owner's room and its adjacent rooms (`0x00463740`); none → fail.
4. Target point: flag 2 → start + (target x, y); else flag 0x20 →
   (target x, y); else the start position.
5. Velocity v: flag 4 clear → `(Vel + (level × VelLev) / 8) << 8`
   (signed division truncating toward zero); flag 4 set → record
   velocity, `<< 8` unless flag 0x10.
6. Slow: if `CanSlow`, the owner has state 87 (`slowmissiles`) and that
   state's stat list exists: `v = stat161 × v / 100` (stat 161
   `skill_handofathena` of that list; signed, truncating).
7. If v ≠ 0: `v = v × 75 / 100` (signed, truncating). Every missile
   flies at 75 % of its table speed.
8. If v ≠ 0: if a target unit is given, is not the owner and stands on
   the owner's subtile → drop the target unit and add (1, 1) to the target
   point (1.14d `0x0059FBBE`–`0x0059FC11`: the target unit's x
   (`0x0045ADF0`, unit in ECX) against the owner's (record +0x04), then
   the y (`0x0045AE20`); both equal → drop; open question 6). With no
   target unit and target point = start → add
   (1, 1). Aim = target unit position or target point; `|aim.x − x| ≥ 100`
   or `|aim.y − y| ≥ 100` → fail. With v = 0 none of this runs.
9. Allocate the unit `0x00555230(x, y, game, room, …, mode = CollideType)`
   (`units.md`). Allocation, in this order: one game-seed step derives
   the missile's unit seed (`rng.md` §5.3), the GUID counter of type 3
   advances (`unit-order.md` §1), then missile init `0x0059F8A0` (called
   at `0x0055542C`): alloc missile data, store GUID, clear unit flags 1
   and 3, cancel all the missile's timers (`0x00540F30`), schedule the
   every-tick type-0 event with args 0, 0 (`0x00541650`; `tick.md`
   §5.3). Allocation failure → fail.
10. Frames: flag 0x8000 → record range; else `Range + level × LevRange`,
    plus, with flag 8 and `SubLoop` ≠ 0, `loops × (SubStop − SubStart)`.
11. Flag 0x200: frames −= start frame; unit +0x44 (animation frame,
    8.8 fixed) = start frame << 8.
12. Total frames = current frame = frames (clamped, §R1.4).
13. Activate frame = frames − (flag 0x800 ? record activate : `Activate`).
14. No path (unit +0x2C) → return none. The unit stays allocated with
    its every-tick event (edge case 4).
15. Path set-up (`units.md` path primitives): velocity 0
    (`0x00648690`); target unit (`0x00648B90`) or target point
    (`0x00648AD0`); footprint mask 0x40 if `Collision` ≠ 0 else 0
    (`0x00648C30`); move-test mask = mask of the `CollideType` entry of
    table `0x0073C720` (§R4.2) (`0x00648CE0`); `CanDestroy` → unit flag
    bit 2 set.
16. If v ≠ 0: path velocity = v, then the path is built toward the
    target (`0x00649970(P, missile, 0)`, D2MOO `D2Common_10142`; the
    third (town-access) argument is the constant 0, `0x0059FDAD`). The missile's path
    has type 4 from its allocation (`sim/path-placement.md` §2.4 calls
    set type `0x00648CF0` with 4), and set type ORs in the type's table
    flags 0x60000 (`sim/pathing.md` §2; `pathtype_flags` row 4 =
    393216): that is where path flag 0x40000 comes from, so
    `0x00649970` takes its first branch to the missile path compute
    (§R4.3), never the walking path functions. That branch
    (`0x006499AE`–`0x006499C2`: `0x00649760` with EAX = path, then
    return) never reads the town-access argument (read 2026-10-09).
17. Last-collided unit := owner (`0x0064A400`; only when `LastCollide`,
    §R5.1). With `LastCollide` the missile never hits its owner first.
18. Path acceleration = `Accel` (signed); maximum velocity =
    `MaxVel << 8`.
19. Flag 0x400: current frame = `(d << 16) / (v << 4)` (unsigned
    division; 0 when `v << 4` is 0), d = distance to the target point
    (`0x006417F0`), at least 1. Total frames are unchanged.
20. Allocate the missile's stat list (`0x00626D40`).
21. Init callback, if any (record +0x54, argument +0x58). Four 1.14d
    callbacks re-seed the new missile (`rng.md` §5.3): `0x005C9290`,
    `0x005CD110` (seed from path target x (+0x10) + a; `skills/bodies-3.md`
    §5.28 owns it), `0x005D40F0`,
    `0x005D4680`; they belong to the skills spec.
22. Skill (clamped ≥ 0), level.
23. Damage setup `0x0059F900` (owner, origin, missile, level),
    `missiles/damage.md`: computes the damage data (`0x0064B860`, which may draw up to
    three `lo' % 100` checks on the owner's or origin's unit seed in
    `0x0064A850`, D2MOO `MISSILE_HasBonusStats`) and writes it as missile
    stats (`0x0064AC60`). The missile only stores the result: stats 21/22
    (min/max damage), 48/49 fire, 50/51 lightning, 52/53 magic, 54/55
    cold + 56 length, 57/58 poison + 59 length + 326 poison count, 60,
    62, 64 drains, 315–317 burning, 66 stun length, 25 damage percent,
    121/122, 180, 141, 103/104/106, 120 (read back in §R6).
24. Owner stored as (type, GUID) (`0x00621CE0`, D2MOO
    `UNITS_StoreOwner`); clear unit flag bit 3.
25. Pierce test `0x0059F940` (§R8.1).
26. Flag 0x1000: stat 19 (tohit) = attack bonus.
27. Flag 0x10000: missile data flags |= 2.
28. Monster unique-mod hook `0x005A43B0`: owner is a monster → the
    owner's unique mods run their missile slot (`0x005A4270`, slot 5;
    `monsters/init.md`). Rows with `NoMultiShot` skip the multishot copy
    and rows with `NoUniqueMod` skip the elemental damage mod there
    (D2MOO `MonsterUnique.cpp`; owned by init.md).
29. Stat 327 (`damage_framerate`) = `DamageRate`.
30. Return the missile.

#### R2.4 Client message

`0x0059FEE0` (D2MOO `MISSILES_SyncToClient`) builds message `0x73`
(`server-messages.tsv`) only for rows with `ClientSend` (91 rows) and an
existing owner: class, position, optional path first point, current
frame, owner type and GUID, level, pierce_idx. Its only caller is skill
code `0x00571F90`. No other missile-creation message is sent by
`0x0059FA30`; the client builds its own missiles from skill and attack
messages (`intents-events.md`; open question 5).

Field sources (2026-10-09, REC-414 settled; `0x0059FEE0(client, game,
M, f)` with f = `0x006486C0(path)` = path +0x7C, the velocity, passed by
`0x00571F90` at `0x005721B6`; 32 bytes zeroed first, sent by
`0x0053DA10`):

1. Gates: M is a missile (type 3); its owner U (`0x00552FD0`: missile
   flag-ex +0xC8 bit 0x400 and the stored owner +0x98 found,
   `0x00552F60`) exists; the class's `missiles.txt` row has `ClientSend`
   (row +0x04 & `[0x006CE27C]`). Any gate fails → nothing. The velocity
   is **not** a gate.
2. Bytes 1–4 are never written (0).
3. class u16@5 = M +0x04 (`0x0064A3E0`).
4. u32@7, u32@0xB = the path's **cell** x, y: path +0x02 and +0x06, the
   integer halves of the 16.16 position (`0x0045ADF0` → `0x006488C0`,
   `0x0045AE20` → `0x00648900`, zero-extended words; a missing path → 0).
5. u32@0xF, u32@0x13 = the path's **target point** +0x10, +0x12
   (`0x00648A00`, `0x00648A10`, zero-extended) when f ≠ 0, else 0.
6. u16@0x17 = current frame, missile data +0x10 (`0x0064A380`).
7. owner type u8@0x19 = U +0x00, owner GUID u32@0x1A = U +0x0C.
8. level u8@0x1E = missile data +0x0C cut to a byte (`0x0064A210`).
9. pierce u8@0x1F = total(M, 328, 0) (`0x00625480`) cut to a byte.

```
client_missile(M, f): U := owner(M); if !U or !row(M).clientsend: return
  send 0x73 { class, x>>16, y>>16, f ? (target_x, target_y) : (0, 0),
              frame, U.type, U.guid, level as u8, total(328) as u8 }
```

d2rs: `units::messages::client_missile` (the field roles are the client
reader's, `client/msg-units.md` §7 r6) from `View::missile_add` when a
missile is added to a room's client (`intents-events.md` §7.2 part A).
d2rs differs on three points (q-fix `qfix-a4a-0x73-fields`): it skips the
message on velocity 0, sends the 16.16 position and sends `points[0]`.

### R3. Per-tick dispatch

`0x005ADCC0` (missile class default handler, `tick.md` §5.6) passes
(game, missile) to `0x005ADBB0`; the event type and arguments are
ignored. 1.14d-confirmed:

1. Missile null, class out of range, or `pSrvDoFunc` ≤ 0 or ≥ 53
   (count at `0x0073C71C`) → return, nothing happens (rows with
   `pSrvDoFunc` 0, 53 rows such as blood1, never move, count down or
   expire on the server).
2. Room = the missile's room; if none, a lookup with a null start room
   (`0x00463740(0, x, y)`) is made, which always yields none (dead
   fallback).
3. Owner = `SUNIT_GetOwner` (`0x00552FD0`; none when the stored owner
   no longer exists).
4. `SrcTown`: owner exists, has a room and the room is in town
   (`0x0061AB00`) → remove.
5. **1.14d addition** (not in D2MOO 1.10f): owner exists, is a
   **player**, has a room and the room is in town → remove, whatever the
   row's flags. Evaluated after step 4 when step 4 did not remove; skipped
   when `SrcTown` is set and there is no owner.
6. Not `Town` and the missile's room (step 2) is in town → remove. A
   missile without a room is never "in town".
7. Call server-do function `pSrvDoFunc` (table `0x0073C768`, ECX game,
   EDX missile; no null check, so indices 4 and 38–52 would crash; no
   live row uses them). Result 2 → remove; any other result keeps it.
8. "Remove" = `0x00555600` (`units.md`): frees the unit and cancels its
   timers; the executing timer is freed after the callback (`tick.md`
   §5.5 consequence 3).

### R4. Default flight (server-do 1, `0x005B0BC0` → `0x005AE1F0`)

Server-do 1 is a jump to `0x005AE1F0` (D2MOO
`MISSMODE_HandleMissileCollision`), which most other server-do functions
also call last. Steps per run, 1.14d-confirmed:

1. No record → expiry hit (`0x005ADF10(missile, unit none, a4 = 1)`,
   §R5) and return its result.
2. **Move.** If the path velocity ≠ 0: unit step `0x00554CA0`
   (`units.md`). It returns 2 when the path step `0x00650840` reports no
   movement (its conditions — path points exhausted, velocity or step
   vector 0, … — are `units.md`'s) → expiry hit, return its result.
3. **Count down.** current frame −= 1; if the new value < 1 → expiry hit,
   return its result.
4. Mode (= `CollideType`) 0 → return 1 (no collision at all).
5. No room or no path → return 2 (removed without a hit).
6. Collision word under the missile = the room's collision mask at the
   path position with the missile's size, all bits
   (`0x00648EB0`, D2MOO `D2Common_10201`: recomputed at the current
   position when the path velocity is 0, otherwise the word cached in the
   path by the last step, path +0x54; `units.md`). Bit 0 or
   bit 2 set → return 2 (removed **without** the hit handler, so no
   server-hit function and no explosion).
7. Mode 6 → return 1. Current frame > activate frame → return 1 (not yet
   active: collision starts on run k ≥ `Activate`, R7.3). Collision word
   0 → return 1.
8. Mode entry of table `0x0073C720` has no unit callback (modes 4 and 6)
   → return 2.
9. For each subtile the path crossed this step (path saved-step list,
   `0x00648F40`, in path order): collision mask there with the missile's
   size against the mode's mask (`0x0064D9B0`). If non-zero: look for a
   unit on that subtile accepted by the mode's callback (`0x00641CB0`,
   D2MOO `D2Common_10407`, `sim/path-placement.md` §4 rule 6 with
   r = the missile's size, so a size outside 1..3 finds no unit);
   found → hit handler
   (unit, a4 = 0), return its result. Not found and mask bit 2 (missile
   barrier) set → expiry-style hit (none, a4 = 1), return 2.
10. No subtile hit → return 1.

#### R4.1 Movement in fixed point

The path code is `units.md`'s; what the missile fields mean there
(1.14d `0x006502D0`, called each step with base 0x400):

1. Velocity (path +0x7C) starts at v (§R2.3), max (path +0x84) =
   `MaxVel << 8`, acceleration (path +0x88) = `Accel`.
2. With acceleration ≠ 0, a counter (+0x8C) increments each step; at 5
   it resets and velocity += acceleration; velocity > max → velocity =
   max and acceleration = 0; velocity < 0 → 0. (D2MOO 1.13c stops at
   `≥ max`; same velocities.)
3. Step vector = `((velocity × 0x400) >> 6) × dir >> 12` per axis
   (arithmetic shifts), dir = the path direction vector (path +0x6A,
   +0x6E).
4. `MaxVel` caps only acceleration; a missile created faster than
   `MaxVel` keeps its speed until it accelerates (edge case 6).

#### R4.2 Collide types (missile modes)

Table `0x0073C720`, 9 entries of (unit callback, mask), dumped from
`Game.exe`; callbacks 1.14d-read (open question 2, answered 2026-10-08):

| Mode | Callback | Mask | Units accepted | Rows |
|---|---|---|---|---|
| 0 | none | 0 | none (R4 step 4) | 301 |
| 1 | `0x005A87F0` | 0x84 | players, and good-aligned monsters | 6 |
| 2 | `0x005A87B0` | 0x104 | monsters | 0 |
| 3 | `0x005A8850` | 0x184 | players and monsters | 339 |
| 4 | none | 0 | none (R4 step 8: removed when on a non-zero collision word) | 0 |
| 5 | `0x005A87B0` | 0x104 | monsters | 0 |
| 6 | none | 0x4 | none (R4 step 7) | 24 |
| 7 | `0x005A8890` | 0x40 | missiles with `CanDestroy` | 0 |
| 8 | `0x005A8850` | 0x185 | players and monsters; mask adds bit 0 | 14 |

Mask bits (`units.md` collision): 0x1 wall, 0x4 missile barrier, 0x40
missile/unit footprint, 0x80 player, 0x100 monster. Shared unit filter
(`0x005A8730`, 1.14d-confirmed): unit flags bits 3 and 2 both set; not
(`NextHit` and unit has state 86); not the last-collided unit; and, if
the missile has an owner, the owner may attack the unit
(`0x00554200`, D2MOO `sub_6FCBD900`, `units.md`) or `CollideFriend` ≠ 0.

Callbacks (ECX unit, EDX the collide context, whose +0x0C must be
non-null; result 1 = accept, else 0): `0x005A87B0` unit type 1 →
filter; `0x005A87F0` type 0 → filter, type 1 with alignment
`0x006259B0` = 2 → filter, no unit or other types → 0; `0x005A8850`
type 0 or 1 → filter; `0x005A8890` (no context test, no filter) type 3
whose class is valid and has `CanDestroy` (flags bit 4, mask table
`0x006CE278` = 0x10) → 1. A null unit → 0 in all four. A `CollideType` ≥ 9
would read past the table; no live row has one, and d2rs treats it as
mode 0 (design choice).

#### R4.3 Missile path compute (`0x00649760`, 1.14d-confirmed)

`0x00649970` (`sim/pathing.md` §3 step 1) hands a path with flag
0x40000 here before any of its own steps: no collided-mask reset, no
footprint removal, no room or town checks. By path type (+0x3C):

| Type | Function | Result |
|---|---|---|
| 4 (every missile created by §R2.3) | `0x006492F0`, below | 0 → path flag 0x20 cleared, result 0; else flag 0x20 set, result = point count (+0x28) |
| 10 | `0x0067A240` (charged-bolt zigzag, `sim/pathing.md` §11.2; open question 12) | flag 0x20 set, result = point count |
| 14 | `0x0067A140` (blessed-hammer spiral, `sim/pathing.md` §11.3; open question 12) | flag 0x20 set, result = point count |
| any other | fatal assert | — |

Type 4, straight line to the target (`0x006492F0`):

1. Current point index (+0x24) := 0.
2. With a target unit (+0x58): target (+0x10, +0x12) := that unit's
   position (`sim/path-placement.md` §2.1 getter: static x/y for types
   2, 4, 5; dynamic sub-tile for 0, 1, 3; (0, 0) without a path).
   Without one the target point set at creation (§R2.3 step 15) stays.
3. |target x − sub-tile x| > 99 or |target y − sub-tile y| > 99
   (unsigned 16-bit words, difference taken as a signed int) → result
   0. Target x = 0 or target y = 0 → result 0.
4. Point[0] (+0x9C) := target; count (+0x28) := 1; +0x38 := 0.
5. Velocity and direction toward point[0] (`0x0064FE40`,
   `sim/pathing.md` §8.4). The direction is set once here: the flight
   step (§R4 step 2, `sim/pathing.md` §9.4) skips the arrival check and
   the per-point re-aim for missiles, so the missile keeps this
   direction past the target point until its frames run out (§R7) or it
   hits.
6. Room-exit flag: path flag 0x1 := 1 when the path has no room (+0x1C)
   or the target lies outside that room's sub-tile rectangle (active
   room +0x4C x, +0x50 y, +0x54 w, +0x58 h, `drlg/rooms.md` §1; inside
   means x ≤ tx < x + w and y ≤ ty < y + h). Only set here, never
   cleared.
7. Result = count (1).

The limit is ±99 here, not the ±100 of the walking compute
(`sim/pathing.md` §3 step 5); §R2.3 step 8 already refuses aims 100 or
more away, so the two agree for a missile aimed at a point.

### R5. Hit handler (`0x005ADF10`, D2MOO `MISSMODE_SrvDmgHitHandler`)

Arguments: game, missile, unit (may be none), a4 (1 = expiry or
barrier). Locals: result `c`, pierce word `p`. 1.14d-confirmed, in
order:

1. Owner = `SUNIT_GetOwner`. `c = 2` (deal direct damage), or 0 with
   `Explosion`. `p = 3`. A missile with no record (class out of range) reads
   flags at address 4 (`0x005ADF61`, a null read in 1.14d); unreachable,
   since creation fails without a record (§R2.3 step 1), so an
   implementation asserts the record.
2. With a unit:
   1. `NextHit` and the unit has state 86 → return 1.
   2. Unit is the last-collided unit (`LastCollide` only) → return 1.
   3. Last-collided unit := this unit (`LastCollide` only).
   4. Mode filter (`0x005ADA40`): mode 0 → return 1; mode 1 and unit not
      a player → return 1; mode 2 and unit not a monster → return 1.
      Other modes accept any unit. (Mode-1 rows therefore find
      good-aligned monsters in R4 step 9 and then ignore them here.)
   5. If an owner exists: owner may not attack the unit (`0x00554200`)
      and `CollideFriend` = 0 → return 1. Then, if `Pierce` and the
      owner has skill_pierce (166) or item_pierce (156): `p` =
      `0x005ADA80` (§R8.2).
3. `CollideKill` → `c |= 1`.
4. Without a unit and a4 = 0 and not `AlwaysExplode` → go to step 8.
5. With a unit and `ToHit`: hit test `0x0057D9B0(attacker = owner,
   defender = unit, tohit = missile stat 19, missile = 1)` (skills
   spec; the chance is clamped to 5…95 and one `lo' % 100` is drawn on
   the owner's unit seed, `0x0057DB22`–`0x0057DB5A`; a missing owner
   returns "miss" without a draw; ECX = owner, EDX = unit at
   `0x005AE06B`–`0x005AE06D`, so the owner's seed is drawn even when the
   owner is dead). Recorded 2026-10-09 (`check-combat-arrow-quillrat`
   f46): the quill rat's `spike1` reaches the player, one rat-seed step
   {4094205064, 1004892389} → `lo'` 1069704589, 89 ≥ chance → miss,
   removed, player life unchanged. On a miss: apply `justhit` (step 6.1), unit event 0, then if
   `AlwaysExplode` and `pSrvHitFunc` in 1…70: call it, result bit 2 →
   return 1. Return 2: **a missed to-hit missile is always removed**,
   whatever `CollideKill` and pierce say.
6. `p & 2` (always true: `p` is 2 or 3):
   1. `justhit` (`0x005ADB00`): with `NextHit` and a unit, a stat list
      expiring at frame + `NextDelay` with state 86 is attached to the
      unit, state 86 is turned on, and a type-12 (remove state) timed
      event is scheduled on the unit for that frame (`0x005417D0`,
      `tick.md` §5.2).
   2. Unit event 0 (hit by missile, events.txt row 0; `0x005C0C30`,
      called also when the unit is none).
   3. `pSrvHitFunc` in 1…70 (count `0x0073C83C`): `c` = its result
      (ECX game, EDX missile, stack unit; no null check). `c & 4` →
      return 1.
7. With a unit and `c & 2`: damage stage §R6.
8. Exit: if `(c & 1 or a4 = 1) and (p & 1)`: clear collision bit 0x40
   under the missile with its size (`0x0064EBA0`) and return 2; else
   return 1.

Server-hit result bits (convention of the functions in `srvhit.tsv`):
1 = the missile dies (subject to pierce), 2 = run the direct damage
stage, 4 = ignore this contact (keep flying, no exit test).

### R6. Damage stage (missile-owned part)

#### R6.1 Order (1.14d `0x005ADF10` step 7)

1. Fill the damage record `0x005A89A0` (missile, unit) — §R6.2.
2. `pSrvDmgFunc` in 1…30 (table `0x0073C960`, count 31 at
   `0x0073C95C`, entries 1–14 non-null, 15–30 null, no null check):
   adjusts the record (`srvdmg` functions: fire/magic/cold arrows, ice
   arrow, fire wall, ice blast, blessed hammer, …; D2MOO names; rules
   in §R6.3).
3. `0x005ADCD0`: result flags (`0x005AD730`, below), then damage
   application (skills spec), then, for a monster that is not a
   hireling, armor −= missile stat 120 (`item_damagetargetac`, clamped
   at ≥ 0 after adding).

`0x005AD730` (D2MOO `MISSMODE_SetDamageFlags`), missile-owned rules,
1.14d-confirmed where marked: no owner → nothing. Result flag "hit";
unless the unit has state 54 (`uninterruptable`): `GetHit` → get-hit
flag, else `SoftHit` → soft-hit flag; `KnockBack` > 0 → `roll(100)` on
the **missile's** seed (`0x0045C390`), < `KnockBack` → knockback flag
(1.14d `0x005AD7CB`). Then block/dodge (`0x0057DFB0`, skills spec);
hit class from `HitClass`; missile data flags 1, 2 → hit flags; pierce
percent = stat 327; events and damage execution (skills spec).
Every result-flag write here ORs into the u16 at R +0x04 (the only
clears are the block/dodge ones of the skills spec, bit 0), so §R6.3
functions 3 and 14 keep their bits. The hit class merges: R +0x60 :=
`HitClass` | (R +0x60 & 0xF0), and R byte +0x64 := 1 when either has a
bit in 0xF0 (`0x005AD863`–`0x005AD87A`); §R6.3 functions 7 and 9's 0x60
therefore survive as `HitClass` | 0x60 with +0x64 = 1. Confirmed
2026-10-08 (impl-pc1-s5): OR; hit class corrected (not an overwrite).

Full order of `0x005AD730(game, missile, unit, R)` (1.14d-read
2026-10-09; owner O = `0x00552FD0`):

1. R +0x04 |= 1; get-hit / soft-hit; knockback `roll(100)` on the
   missile seed (`0x005AD7DF`).
2. Block/dodge `0x0057DFB0` (ECX game, EDX O; stack unit, avoid 1,
   block = physical (R +0x08) ≠ 0) at `0x005AD806`: draws only on the
   **unit's** seed (`combat/hit.md` §6).
3. Hit-class merge (above).
4. **Monster critical hit** `0x005A5560` (ECX O, EDX unit, stack R) at
   `0x005AD884` (`combat/damage.md` §3.1 step 13): O a monster with
   `monstats.Crit` ≠ 0 → one inline `lo' % 100` on the **owner's** seed
   whether or not the hit survived block/dodge; `r < Crit` doubles the
   damage fields. Not reached on a to-hit miss (§R5 step 5).
5. Missile data flags 1, 2 → hit flags 0x20, 0x80; R +0x54 := stat 327.
6. R +0x04 has 1 → `apply(game, O, unit, missile 1, R)` (`0x0057C6C0`
   at `0x005AD8C6`, `combat/damage.md` §5.2); then the reaction
   `0x0057CEE0(game, O, unit, R)` at `0x005AD8D3` (§7.1), always.

Recorded (`check-combat-arrow-kill`, quill rat `spike1` on the player,
f46): rat seed {1069704589, 1707661690} → two steps: to-hit `lo' % 100`
= 39 (hit), crit `lo' % 100` = 76 (≥ `Crit`, no double); player life
12800 → 12415; player seed unchanged (no shield, no get-hit mask draw).

#### R6.2 Damage rolls (`0x005A89A0`, 1.14d-confirmed)

Each element uses the missile's own unit seed (unit +0x20), in this
order, as `min + roll(max − min)` after reading max then min:

| # | Element | Stats (min, max) | Mastery stat |
|---|---|---|---|
| 1 | physical | 21, 22 | none (inline) |
| 2 | fire | 48, 49 | 329 |
| 3 | magic | 52, 53 | 357 |
| 4 | lightning | 50, 51 | 330 |
| 5 | cold | 54, 55 | 331 |
| 6 | poison | 57, 58 | 332 |
| 7 | burning | 316, 317 | 329 |

Per element (`0x005A8910`; every stat, the mastery included, is the
missile's own total `0x00625480(missile, s, 0)`): max ≤ 0 → 0, no draw; min ≤ 0 → 0, no
draw; if min > max swap; mastery m ≠ 0 → `min += pct(min, m)`, `max +=
pct(max, m)`; result `min + roll(max − min)` — `roll(0)` does not step
(`rng.md` §3), so equal bounds draw nothing. `pct(a, b) = a × b / 100`
with the overflow-safe evaluation of `0x00483360` (exact 64-bit product
when `a > 0x100000` or `b > 0x10000` and the shortcut does not apply).
Note `roll(n)` gives 0…n−1: the maximum is never reached (an original
rule; damages are 8.8 fixed point).

After the rolls (no draws): cold length 56, poison length 59 divided by
poison count 326 when > 1, drains 62/60/64, burn length 315, stun 66;
with a unit: damage percent 25 (+121 demon, +122 undead, +180 by monster
type), floored at −90, `phys += phys × pct / 100` (32-bit `imul`,
wrapping, then signed ÷ 100 truncating, `0x005A8BE8`); deadly strike 141 ≠ 0
→ crit flag and phys × 2; 103/104/106 bypass flags.

The record is the 0x70-byte damage record itself (`combat/damage.md`
§1; `0x005A89A0` zeroes all 0x70 bytes first), so an area helper takes
it unchanged. Fields written: physical +0x08, fire +0x10, magic +0x20,
lightning +0x1C, cold +0x24, cold length +0x30 (stat 56), poison +0x28,
poison length +0x2C (stat 59, ÷ stat 326 when > 1), mana leech +0x3C
(62), life leech +0x38 (60), stamina leech +0x40 (64), burn +0x14,
burn length +0x18 (315), stun length +0x44 (66). Deadly strike: result
flags (u16 +0x04) |= 0x2000 and +0x08 × 2. Bypass: stat 103 → hit flags
(+0x00) |= 0x100, 104 → 0x200, 106 → 0x400 (`combat/damage.md` §1:
undead / demons / beasts). Nothing else is set (freeze length +0x34
stays 0).

#### R6.3 Server-damage functions (`pSrvDmgFunc`, 1.14d-confirmed 2026-10-08)

Table `0x0073C960` (§R6.1 step 2). Each is called as (ECX game, EDX
missile M, stack unit U, record R) and returns nothing. "Row" = M's
`missiles.txt` record; "row valid" = class in range and record found
(§R1 rule 1); functions that test it do nothing without it. Shared
helpers:

- `add_elem(M, R, a)` = `0x005A9270`: adds a to R's field for the
  row's `EType` (`0x0064B0C0`): 0 physical +0x08, 1 fire +0x10, 2
  lightning +0x1C, 3 magic +0x20, 4 cold +0x24, 5 poison +0x28, 6 / 7 /
  8 life / mana / stamina leech +0x38 / +0x3C / +0x40, 9 stun +0x44,
  11 burn +0x14; 12 **sets** cold +0x24 := a; 10 and > 12 nothing.
  M none → nothing.
- `clear_elems(M, R)` = `0x005A91C0`: hit flags &= ~0x700 (bypass
  bits); physical, fire, burn, lightning, magic, cold, poison, the three
  leeches and stun := 0; then by `EType`: 4 or 12 → poison length +0x2C
  and burn length +0x18 := 0 (cold / freeze lengths kept); 5 → cold
  length +0x30, freeze +0x34, burn length := 0 (poison length kept); 11
  → cold length, poison length, freeze := 0 (burn length kept); any
  other → all four lengths and freeze := 0. M none → nothing.
- `c` = missiles evaluator `0x0064B7C0(M, owner, DmgCalc1 (+0x90), −1,
  −1)` (owner `0x00552FD0`); `pct` = `0x00483360` (`combat/damage.md`
  §0). `elem_roll` = `0x005A8C70` (§R5 helpers).

| # | Address | Rows (1.14d) | Rule |
|---|---|---|---|
| 1 | `0x005AD050` | firearrow, magicarrow, coldarrow | row valid; c > 0: c := min(c, 100); f := min(pct(R phys, c, 100), R phys); R phys −= f; `add_elem(M, R, f)` |
| 2 | `0x005AD1B0` | icearrow, royalstrikechaosice | row valid: R freeze +0x34 := pct(M's total stat 56 `coldlength` (`0x00625480(M, 56, 0)`), max(`dParam1`, 0), 100); R cold length +0x30 := 0 |
| 3 | `0x005AD220` | blaze, firewall, vampirefirewall, immolationfire, meteorfire, vampiremeteorfire | row valid: one step of M's unit seed (unit +0x20; seed := lo × 0x6AC690C5 + hi, inline, no `0x0045C390`); (new lo & 0x7F) < `dParam1` → R result flags |= 0x4000 (soft hit) |
| 4 | `0x005AD290` | iceblast | no checks: R freeze +0x34 := R cold length +0x30; cold length := 0 |
| 5 | `0x005AD2F0` | blessedhammer | row valid: e := `elem_roll(game, M, U, R)`; `dParam1` > 0 and U undead (`0x0063E990`) → `add_elem(M, R, pct(e, dParam1, 100))`; then `dParam2` > 0 and U demon (`0x0063E940`) → `add_elem(M, R, pct(e, dParam2, 100))` |
| 6 | `0x005AD3A0` | none (no live row) | owner O of M; O, O flags +0xC4 bit 0x200 clear, U present → U flags +0xC4 |= 0x20000 |
| 7 | `0x005AD3E0` | warcry, shockwave | row valid: R stun +0x44 := `dParam1` if > 0, else `0x004E6CA0(skill, level)` (M data +0x0A / +0x0C, `0x0064A280` / `0x0064A210`; `missiles/bodies-2.md` §38 step 4 formula); R hit class +0x60 := 0x60 (kept as `HitClass` \| 0x60, §R6.1) |
| 8 | `0x005AD450` | erruption crack 1, 2 | row valid, U present, `ProgOverlay` (+0x36, i16) > 0 → overlay on U (`0x00621E40(U, ProgOverlay, 0)`) |
| 9 | `0x005AD4A0` | twister | row valid: k := M's skill; R stun := `dParam1` if > 0; else k's skills row (`0 ≤ k <` count) `Param2` (+0x14C); else 0. R hit class := 0x60 (kept as `HitClass` \| 0x60, §R6.1) |
| 10 | `0x005AD2B0` | bladesoficecubes | row valid: R freeze +0x34 := R cold length +0x30 (cold length kept) |
| 11 | `0x005AD530` | rabiescontagion | O := owner; O or U none → R poison +0x28, poison length +0x2C := 0. Else t := M data +0x28 (`0x0064A730`) − game frame (game +0xA8); 10 ≤ t ≤ `elem_len(O, k, L, 1)` (`0x00644F20`, `skills/levels.md` §3.2; k, L = M's skill, level) → R poison length := t; else poison and its length := 0 |
| 12 | `0x005AD0F0` | lightningjavelin | row valid; c > 0: c := min(c, 100); f := min(pct(R phys, c, 100), R phys); e := `elem_roll(game, M, U, R)`; `clear_elems(M, R)`; `add_elem(M, R, f + e)` |
| 13 | `0x005AD5C0` | blessedhammerex | U present and a monster (type 1): O := owner; O → R phys += R phys × O's total stat 25 (`0x00625480(O, 25, 0)`) / 100 (signed, truncating); then function 5 (O or not). Other U: nothing |
| 14 | `0x005AD630` | moltenboulder | M type 3 and row valid, U present. p by U: player → `dParam2` when `dParam1` ≥ 1, else none; monster (class flags `0x004638A0(class, 10)` small, `(class, 11)` large): `dParam1` < 1 → `dParam2` if small, else none; = 1 → small 2 × `dParam2`, large none, other `dParam2`; > 1 → small 3 × `dParam2`, large `dParam2`, other 2 × `dParam2`; other types none. p > 0: `roll(100)` on M's seed (`0x0045C390`) < p → R result flags |= 8 (knockback) |

Draws: function 3 one inline seed step and function 14 one `roll(100)`,
both on the missile's seed; 5, 12 and 13 make `elem_roll`'s draws.
Entries 15–30 are null and never reached by live data (§R6.1).

### R7. Lifetime and expiry

1. The every-tick event is prepended to the missile class list. A
   missile created in frame F during the timer-queue run (unit events,
   including other missiles' functions) first runs in frame F + 1
   (`tick.md` §5.5 consequence 1), then every frame. One created before
   the queue run of F (tick steps 1–3, `tick.md` §3) already runs in F;
   one created by a client message between ticks runs first in the next
   tick, like one from the queue run. Observed: first run in F + 1 for
   all 70 recorded missiles.
   **Creation before the queue run** (1.14d, static; Open question 11).
   In the call graph of `Game.exe` (direct calls, jump and function
   tables), every path from the message drain `0x0052CFE0` or tick
   steps 1–3 (`0x0052D7B0`, `0x0052D720`, `0x0052D160`) to the create
   `0x0059FA30` that avoids the queue run `0x005414D0` passes through
   the skill do core `0x0056F7F0` or the umod dispatcher `0x005A4270`.
   - Do core there: the Iron Golem re-summon at join (`0x005394A0`,
     `srvdofunc` 57 `0x005C5250`) and an `immediate` aura chosen by
     C→S 0x3C or re-selected by the item refresh (`srvdofunc` 65
     `0x005CF010`). Neither has `srvmissile` or a do function that
     creates a missile.
   - Dispatcher there: modes 0 and 1 run at a monster mode set
     `0x005A7C20`. The only mode-0 or mode-1 callback that creates a
     missile is umod 23 poisonhit's (`0x005A3490`, the
     `queenpoisoncloud` cross burst, `monsters/umod-callbacks.md`
     §16). A spawn or restore sets the mode before the umods are
     copied (`0x005424F0`: creation `0x005B30E0` / `0x005A4440` /
     `0x005A46E0` first, umod copy `0x005428D3` after). The mode set on
     an existing monster in step 3 is the re-placement of a kept pet
     (`0x00542B40` → `0x00554A30` → `0x005735A0`, the current mode set
     again). The other mode-set route found, a think scheduled on a unit
     with state 54 (`0x005416B0` → `0x005544B0` → kill or reaction), is
     taken only by players (`monsters/ai.md` §1, the state-54 note).
     No pet carries umod 23 in 1.14d data (`sumumod` 32, 33, 42;
     hirelings 19).
   So with 1.14d data no missile is created before the queue run, and
   every missile first runs in the frame after its creation. A modded
   umod-23 pet would create its clouds in step 3, and they would run in
   that frame's step 4.
2. With frames N = `Range + level × LevRange` (§R2.3 step 10, no flags)
   and N ≥ 1, run k leaves N − k frames; the expiry hit happens on run
   k = N (or run 1 when N ≤ 1). Unless the server-hit function returns
   bit 4, the expiry hit returns 2 and the missile is removed in frame
   F + N. **Lifetime in runs = N**, independent of velocity.
3. Collision (R4 steps 6–9) is tested on run k only when N − k ≤
   N − `Activate`, i.e. k ≥ `Activate`.
4. Earlier removal: movement stopped (R4 step 2), a blocking collision
   word under the missile (step 6, no hit), a mode-4 contact (step 8), a
   barrier on a crossed subtile (step 9), a unit hit that ends in exit
   (R5 step 8: `CollideKill` or a missed to-hit), the room tests of §R3.
5. At expiry the row's server-hit function runs with no unit (e.g. a
   fireball explodes at the end of its range); the direct damage stage
   does not.

### R8. Pierce

#### R8.1 Pierce test at creation (`0x0059F940`, 1.14d-confirmed)

Runs only when the missile is type 3, its owner is a player or monster
and the row has `Pierce` (60 rows):

1. P = owner's stat 166 (`skill_pierce`, `0x00625480`) + stat 156
   (`item_pierce`, `0x00625500`). P = 0 → nothing (stat 328 not set).
2. A local seed (not stored anywhere) = `init_low(owner's base stat
   328 pierce_idx)` (`0x006253B0`, `0x00650E40`): `{counter, 666}`. The
   owner's base pierce_idx is a counter incremented by attack/skill
   messages (D2MOO `PlrMsg.cpp`, `MonsterMsg.cpp`; skills spec).
3. n = 0; up to 4 times: draw `lo' % 100` (inline step); if ≥ P stop;
   else n += 1. The compare is signed (`0x0059F9FB` `jge`): a negative P
   stops at the first draw (one step, n = 0).
4. Missile stat 328 (`pierce_idx`) = n: the number of units it may pass
   through.

#### R8.2 Pierce at a hit (`0x005ADA80`)

With `Pierce` and the missile's stat 328 > 0: stat 328 −= 1 and the
pierce word is 2 (bit 0 clear: R5 step 8 keeps the missile even with
`CollideKill`); else 3. Only reached when the owner still has a pierce
stat (R5 step 2.5), so a missile whose owner lost its pierce stat stops
piercing.

### R9. Server-do and server-hit catalogues

#### R9.1 Tables (dumped from `Game.exe`, 1.14d-confirmed)

| Table | Address | Count (address) | Non-null | Dispatcher |
|---|---|---|---|---|
| server-do | `0x0073C768` | 53 (`0x0073C71C`) | 1–3, 5–37 | `0x005ADBB0` (R3) |
| server-hit | `0x0073C840` | 71 (`0x0073C83C`) | 1–29, 31–33, 35–40, 43–45, 47–48, 50–59 | `0x005ADF10` (R5), `0x005AD9D0` |
| server-damage | `0x0073C960` | 31 (`0x0073C95C`) | 1–14 | `0x005ADF10` (R6.1) |

The null pattern equals D2MOO 1.10f's tables entry for entry; 1.14d
entries 23 and 24 of server-do share `0x005AF790`, as in D2MOO.
`0x005AD9D0` (other caller of the server-hit table) runs the row's
server-hit function with no unit when its flag argument is non-zero,
then removes the missile (`0x00555600`). It is dead code in 1.14d: no
call, jump or stored pointer reaches it (open question 10); d2rs does
not implement it.

#### R9.2 TSV columns (`srvdo.tsv`, `srvhit.tsv`)

One row per table index (53 and 71 rows), tab separated, header row:

| Column | Meaning |
|---|---|
| `index` | table index = the `missiles.txt` column value |
| `addr_114d` | function address read from the table in `Game.exe`; `-` for null |
| `d2moo_name` | D2MOO 1.10f function name (`MissMode.cpp`) |
| `rows` | number of live `missiles.txt` rows with this value (numeric cells only) |
| `examples` | up to 4 rows as `name(id)` |
| `params` | missile columns the function reads (`Param1-5`, `SrvCalc1`, `SubMissile1-3` for server-do; `sHitPar1-3`, `SHitCalc1`, `HitSubMissile1-4` for server-hit), from D2MOO; "skill" = skills.txt calcs |
| `rng` | draws the function makes itself |
| `behaviour` | one line |
| `status` | `spec'd-here` (rules in this spec), `summarized` (1.14d entry or helper read, behaviour from D2MOO), `D2MOO-only` (1.14d address from the table, behaviour unverified), `unread` |

#### R9.3 Seeded sub-missile helper (`0x005A9820`, D2MOO `MISSMODE_CreateMissileWithCollisionCheck`, 1.14d-confirmed)

Arguments: missile, range r, interval i, sub-missile class, collision
mask. Used by server-do 8, 10, 17, 25.

1. i = 0, or elapsed frames (`0x0064A3B0`) not a multiple of i (signed
   remainder) → nothing.
2. No owner or no room → nothing.
3. **Re-seed the missile's own unit seed**: `init_low(x + elapsed)`.
4. dx = `roll(2(r − 1))` − (r − 1); dy = `roll(2(r − 1))` − (r − 1), both
   on the missile's seed, in that order.
5. If the room's collision mask at (x + dx, y + dy) & mask ≠ 0
   (`0x0064CB30`) → nothing; else create the sub-missile (flags 3: given
   position, relative target 0, so target = position; owner, skill, level
   of the parent).

#### R9.4 Missile-seed re-initialisations (`rng.md` §5.3 list)

| Site | Function | Seed value |
|---|---|---|
| `0x005A9820` | §R9.3 | missile x + elapsed frames |
| `0x005ACDF0` | server-hit 58 (Baal taunt lightning) | missile x |
| `0x005AFB80` | server-do 28 (volcano) | missile data +0x28; the new low word is stored back there |
| `0x005B04A0` | server-do 34 (Baal taunt control) | path first point x (path +0x0C) |
| `0x005B0640` | server-do 35 (royal strike chaos ice) | missile data +0x28; low word stored back |
| `0x005AC040` | init callback of server-hit 38 / 45 sub-missiles (`bodies.md` §19) | path target x (path +0x10) |
| `0x005D5BF0` | target scatter (server-hit 40, `bodies-2.md` §48; also a skill caller) | target x |
| `0x005C9290`, `0x005CD110`, `0x005D40F0`, `0x005D4680` | init callbacks (§R2.3 step 21) | path geometry; skills spec |

Every re-seed sets `{value, 666}` on unit +0x20. Missiles never
re-seeded draw from the seed derived at allocation.

#### R9.5 Server-do bodies (1.14d-confirmed)

Called by §R3 step 7 (ECX game, EDX missile). "Flight" = §R4
(`0x005AE1F0`); its result is returned. Position = the missile's path
x / y. Owner = `0x00552FD0` (none when gone). Level and skill = missile
data +0x0C / +0x0A (`0x0064A210`, `0x0064A280`); frames left = data
+0x10 (`0x0064A380`).

1. **2 Poison Javelin, poison traps** `0x005AE400`:
   1. Missile and record exist and `SubMissile1` (+0x18, i16) ≥ 0: v =
      missiles evaluator `0x0064B7C0(missile, owner, SrvCalc1 (+0x80),
      class, level)` (`data/calc-expressions.md`), evaluated every run;
      `sub_at_step(game, missile, SubMissile1, range 0, loops v)`.
   2. Return flight.

   `sub_at_step(game, missile, class, range, loops)` = `0x005A9720`:
   nothing unless the path's new-step flag is set (path +0x34 bit 3,
   `0x006505C0`) and the missile has an owner. Zeroed parameter record
   (§R2.1): flags 5 (position given, velocity given = 0: the
   sub-missile does not move); range > 0 → flags |= 0x8000, range field
   = range; loops > 0 → flags |= 8. Owner; origin = the missile; class;
   x, y = the missile's position; skill and level = the missile's;
   loops field = 2 × level − 2 (whatever `loops` was: the argument only
   switches flag 8). Create (`0x0059FA30`). 1.14d rows: poisonjav
   (`SrvCalc1` "0": no loops), viper_poisjav (3); the two trap rows have
   no `SubMissile1`.
2. **3 Poison cloud, Blizzard, Thunder Storm, Hand of God**
   `0x005AE480`: path present and path velocity (+0x7C) ≠ 0 → flight.
   Else OR 0x40 into the one collision cell under the missile
   (`0x0064CB90(room, x, y, 0x40)`: the room containing (x, y) searched
   from the missile's room, its collision grid; no size, no marker), then
   flight.
3. **5 Fire Wall, Immolation fire, Meteor fire, Molten Boulder path**
   `0x005AE520`: no record → flight. f = animation frame (unit +0x44 >>
   8, arithmetic). Stamp 0x40 with the missile's size at its position
   (`0x0064EA00`, `sim/path-placement.md` §5.1). S = `SubStart`
   (+0x181), E = `SubStop` (+0x182), unsigned bytes:
   - f = S − 1: animation frame := (S − 1 + `roll(E − S)`) << 8 — one
     draw on the missile's seed (`0x0045C3E0`; none when E − S < 1);
   - else frames left = S: frame := (S − 3) << 8;
   - else frames left < S: frame := max(f − 2, 0) << 8.
   Return flight.
4. **7 Guided Arrow, Bone Spirit** `0x005AE780`:
   1. Missile or record missing → 2. Owner none or dead (`0x005541B0`)
      → 2. The missile's room in town (`0x0061AB00`) → 2.
   2. Homing = data +0x28 bit 0 (srvdo 10 sets +0x28 := 1 with a
      target unit, 2 with a point, `0x005DB7FE`). Homing: refresh the
      missile's path target and take T = its target (`skills/bodies.md`
      §2.1); T dead, or the owner may not attack T (`0x00554200(game,
      owner, T)`) → T none. n = `Param1`, ≤ 0 → 5. T and frames left mod
      n = 0 (signed): d = `0x006416D0(missile, T)`; 4 ≤ d ≤ 24 → rebuild
      the path (`0x00649970(path, missile, 0)`, `sim/pathing.md` §3).
   3. Return flight.

   `0x006416D0(a, b)`: dx = |bx − ax|, dy = |by − ay|; each minus (a
   size / 2 + b size / 2) (signed halves, `0x00620510`), floored at 0;
   d = (2 × max(dx, dy) + min(dx, dy)) / 2, truncating.

5. **8 MonBlizzCenter** `0x005AE8A0`: record missing → the code
   reads `SubMissile1` through a null record (fatal; class ids are
   always valid). Else q = level / max(`Param3`, 1) (signed); range =
   `Param1` + max(q, 2); interval = max(`Param2` − q, 3); §R9.3 helper
   (missile, range, interval, `SubMissile1`, mask 5) — `SubMissile1` < 0
   is passed on and the creation fails; return flight. (The missing
   record cannot happen: a missile's class is the valid row it was
   created from, §R2; an implementation may assert.)
6. **10 BlizzardCenter** `0x005AEA60`: record missing → as 5. Missile
   skill k invalid → return 2 (removed). range = `eval(owner, k.calc1,
   k, level)`, interval = `eval(owner, k.calc2, k, level)` (calc1
   first; owner may be none); §R9.3 helper (missile, range, interval,
   `SubMissile1`, mask 5); return flight.
7. **25 EruptionCenter** `0x005AF880`: record missing or `SubMissile1`
   = 0 → 2; k invalid → 2; then as 6 with mask 0x45.

The §R9.3 helper takes ESI = missile, EAX = interval and stack (game,
range, class, mask).

#### R9.6 Server-hit bodies (1.14d-confirmed)

Called by §R5 step 6.3 (ECX game, EDX missile, stack unit or none);
result bits per §R5. Helpers:

- `elem_roll(game, missile, unit, record)` = `0x005A8C70`: rolls only
  the row's `EType` (+0xE4) element with the §R6.2 per-element helper
  `0x005A8910` (missile seed) and returns it: 0 physical (21/22, no
  mastery) → +0x08; with a unit, p = missile stat 25 (+ stat 121 if the
  unit is a demon `0x0063E940`, + 122 if undead `0x0063E990`), at least
  −90, physical += physical × p / 100 (signed); then stat 141 ≠ 0 →
  physical × 2 and result |= 0x2000 (no draw); 1 fire (48/49, mastery
  329) → +0x10; 2 lightning (50/51, 330) → +0x1C; 3 magic (52/53, 357)
  → +0x20; 4 cold (54/55, 331) → +0x24, cold length +0x30 = stat 56; 5
  poison (57/58, 332) → +0x28, length +0x2C = stat 59, divided by stat
  326 when > 1; 6, 7, 8 → life / mana / stamina leech = stat 60 / 62 /
  64 (no roll); 9 → stun length +0x44 = stat 66; 10 and > 12 → nothing;
  11 burn (316/317, 329) → +0x14, burn length +0x18 = stat 315; 12
  freeze (54/55, 331) → +0x24, freeze length +0x34 = stat 56. Unlike
  §R6.2 there is no `damage_vs_montype` term. **Return value** (a
  rolled amount, 8.8 like the record; not the `EType`): 0 → the final
  physical (after the percent and the ×2); 1 / 2 / 3 / 4 / 12 → the
  fire / lightning / magic / cold / cold (freeze) roll; 5 → poison × 25;
  11 → burn × 25; 6–10 and > 12 → 0 (6 still stores the life leech).
- `elem_len(record, len)` = `0x005A8F20` by `EType`: 4 cold length, 5
  poison length, 9 stun length, 11 burn length, 12 freeze length :=
  len; others nothing.
- `area_damage(game, owner, x, y, r, record, f)` = `0x0056BAD0`: f = 0
  → 0x8583; owner none → return 1; else `scan_unit(game, owner, x, y,
  r, f, cb, record, noaura 0)` (`skills/bodies.md` §2.12) and return 1.
  Per unit U the callback copies the record and runs `0x0056B9C0(game,
  owner, U, copy)`: a missile attacker becomes its owner; result flags
  0 → nothing; b = `block_or_dodge(game, attacker, U, avoid 1, block 0)`
  (`combat/hit.md` §6.1): 2 → result |= 0x100, 4 → 0x80, 0x10 → 0x8000,
  1 → 0x10, b ≠ 0 → clear 1; hit and U without state 54 → |= 4;
  monster critical hit `0x005A5560` (`combat/damage.md` §3.1 step 13);
  hit → `apply(game, attacker, U, missile 1, copy)` (§5.2); reaction
  `0x0057CEE0` (§7.1). b = 8 (evade) and any other bit outside 1, 2,
  4, 0x10 adds no result bit; only the "b ≠ 0 → clear 1" applies
  (asm of `0x0056B9C0`).

Bodies:

1. **1 Fireball, Exploding / Freezing Arrow explosion** `0x005A9A70`:
   missile, record or owner missing → 1. r = `sHitPar1`; r ≤ 0: missile
   skill k invalid → return 1 (no explosion); else r = max(`eval(owner,
   k.calc1, k, level)`, 1). Zeroed record; `elem_roll(game, missile,
   unit, record)`; hit flags |= `HitFlags` (+0xA8); result |=
   `ResultFlags` (+0xAC); `area_damage(game, owner, x, y, r, record,
   0)` around the missile. Return 1.
2. **4 Exploding Arrow, Freezing Arrow, Royal Strike meteor center**
   `0x005B07A0`: missile or record missing → 1. Zeroed record: flags 0
   (start at the origin, target = start), owner (none → every creation
   fails), origin = the missile, skill and level of the missile. For i =
   1…4: `HitSubMissile_i` (+0x24 + 2(i − 1)) > 0 → create that class;
   created and `sHitPar1` > 0 → hit handler `0x005ADF10(game, new
   missile, unit, a4 = 1)` (§R5; result ignored). Return 3.
3. **12 Chain Lightning, Lightning Strike** `0x005AA730`: missile or
   record missing → 1. Owner none or no unit → 3. n = data +0x28
   (bounces left, `0x0064A730`); n ≤ 1 → 3. r = `sHitPar1`, or (≤ 0)
   max(`eval(owner, k.aurarangecalc, k, level)`, 1), k = the missile's
   skill (no record → 3). U = `next_unit(game, owner, x, y, r, 0x88583,
   unit GUID)`; U none or U = the unit → 3. Record: flags 0x21; owner;
   target unit U; class = this missile's; start = the missile's
   position; target point = U's position; skill, level of the missile.
   Created → its data +0x28 := n − 1 (`0x0064A710`). Return 3.

   `next_unit(game, source, x, y, r, f, g)` = `0x0056BD10`: `scan_unit`
   with f | 0xA783 (here 0x8A783: players and monsters, flag tests 0x80
   and 0x400, not in town, line of sight 0x200, no town rooms 0x2000,
   hostile 0x8000, no `justhit` 0x80000). The callback keeps, comparing
   GUIDs unsigned, the unit with the smallest GUID > g and the unit with
   the smallest GUID ≤ g (a later equal one replaces it); returns the
   first, else the second (the hit unit itself when nothing else
   qualifies). No draws: the chain order is GUID order with wrap-around.
4. **13 Glacial Spike, Hell Meteor down** `0x005AA8B0`: missile,
   record, or the missile skill k's record missing → 1. r = `sHitPar1`,
   or max(`eval(owner, k.aurarangecalc)`, 1); len = `sHitPar2` when >
   0, else `eval(owner, k.auralencalc)` (unclamped), both at the
   missile level (same > 0 test as r; asm of `0x005AA8B0`).
   Zeroed record; `elem_roll`; len > 0 → `elem_len(record, len)`; flags
   from the row as in 1; `area_damage(…, 0)`, which always returns 1, so
   the result is always 1 (the code maps a 0 to 3).

Server-do 17, 28, 34, 35 and server-hit 58: `missiles/bodies.md` §1–§5;
every other body: `missiles/bodies.md` §6–§30 and
`missiles/bodies-2.md` §31–§62.

### R10. Behaviour of the recorded missiles

All four use server-do 1, server-hit 0, server-damage 0, `CollideType`
3, `CollideKill` 1, `LastCollide` 1, `Size` 1, `Activate` 0, `Accel` 0,
`LevRange` 0 (`missiles.bin` measured):

| Row | Missile | Owner | Vel / VelLev / MaxVel | Range | Flags dword | ToHit | AlwaysExplode | Pierce | Explosion | Damage columns |
|---|---|---|---|---|---|---|---|---|---|---|
| 7 | spike1 | quill rat (monstats 63) | 10 / 8 / 10 | 40 | 0x249 | 1 | 0 | 0 | 0 | Min/MaxDamage 1/2 |
| 22 | shafire1 | fallen shaman (58) | 8 / 0 / 8 | 40 | 0x24D | 0 | 0 | 1 | 0 | fire EMin/EMax 1/4 |
| 58 | firebolt | player (skill Fire Bolt) | 20 / 0 / 20 | 50 | 0x249 | 0 | 0 | 0 | 0 | from the skill |
| 120 | rogue1 | rogue NPC/hireling | 24 / 0 / 24 | 40 | 0x24D | 1 | 1 | 1 | 0 | Min/MaxDamage 1/1 |

Flags 0x249 = LastCollide, CanSlow, GetHit, ReturnFire; 0x24D adds
Pierce. Consequences of the rules:

1. **Arrow/bolt path** (rows 0 arrow, 1 javelin, 120 rogue1 and every
   row with server-do 1 and `ToHit`): flies straight at 75 % of `Vel`,
   ignores its owner first (`LastCollide`), touches the first player or
   monster the owner may attack, rolls to-hit; miss → removed (R5 step
   5; with `AlwaysExplode` the server-hit function would run first —
   none here); hit → direct damage (c = 3), removed unless pierce count
   remains (R8). Expires after `Range` runs.
2. **spike1**: v = `((10 + level × 8 / 8) << 8) × 75 / 100`. The quill
   rat's A2 has no used skill, so the monster mode missile (§R2.2) makes
   it with level = `MonsterSkillBonus` + 1: v = 2112 on Normal (level
   1), 2688 on Nightmare (4), 3456 on Hell (8). To-hit as 1; no pierce.
3. **shafire1**: no to-hit test (always hits a valid unit); `Pierce`
   set but monster owners have no pierce stats (R8.1 P = 0) so it never
   pierces; `ExplosionMissile` (shamanexp) is client-side only.
4. **firebolt**: like shafire1 without `Pierce`; `ExplosionMissile`
   fireexplode is not read by the server.
5. Speeds (synthetic, from R2.3): firebolt v = 3840, rogue1 4608,
   shafire1 1536, spike1 level 0 1920.

### R11. `missiles.txt` columns and their server use

Server columns (owned here unless noted):

| Column | Server use |
|---|---|
| `pSrvDoFunc`, `pSrvHitFunc`, `pSrvDmgFunc` | §R3, §R5, §R6.1 |
| `Param1`–`Param5`, `SrvCalc1`, `sHitPar1`–`sHitPar3`, `SHitCalc1`, `dParam1`–`2`, `DmgCalc1` | per function (`srvdo.tsv`, `srvhit.tsv`; calcs: `data/calc-expressions.md`) |
| `Vel`, `VelLev`, `MaxVel`, `Accel` | §R2.3 steps 5–7, 16, 18; §R4.1 |
| `Range`, `LevRange`, `SubLoop`, `SubStart`, `SubStop`, `Activate` | §R2.3 steps 10–13; SubStart/SubStop also server-do 5 |
| `CollideType`, `Collision`, `CollideKill`, `CollideFriend`, `LastCollide`, `NextHit`, `NextDelay`, `Size` | §R1.5, §R2.3, §R4, §R5 |
| `ToHit`, `AlwaysExplode`, `Explosion`, `Pierce`, `CanDestroy`, `CanSlow` | §R5, §R8, §R2.3 |
| `Town`, `SrcTown` | §R3 |
| `ClientSend` | §R2.4 |
| `GetHit`, `SoftHit`, `KnockBack`, `HitClass`, `HitFlags`, `ResultFlags`, `ProgOverlay` | §R6.1 and some functions |
| `SubMissile1`–`3`, `HitSubMissile1`–`4` | per function |
| `DamageRate` | §R2.3 step 29 |
| `NoMultiShot`, `NoUniqueMod` | `monsters/init.md` (unique-mod missile hooks) |
| `MissileSkill`, `Skill`, `SrcDamage`, `SrcMissDmg`, `Half2HSrc`, `ApplyMastery`, `Holy`, `HitShift`, `MinDamage`…`MaxLevDam5`, `DmgSymPerCalc`, `EType`, `EMin`…`MaxELev5`, `EDmgSymPerCalc`, `ELen`, `ELevLen1`–`3`, `ReturnFire` | damage setup and skill code: skills spec |

Not read by the server (client only, or unused): `pCltDoFunc`,
`pCltHitFunc`, `CltCalc1`, `CltParam1`–`5`, `CHitCalc1`,
`cHitPar1`–`3`, `CltSubMissile1`–`3`, `CltHitSubMissile1`–`4`,
`TravelSound`, `HitSound`, `ProgSound`, `CelFile`, `animrate`,
`AnimLen`, `AnimSpeed`, `RandStart`, `LoopAnim`, `xoffset`, `yoffset`,
`zoffset`, `Light`, `Flicker`, `Red`, `Green`, `Blue`, `Trans`,
`NumDirections`, `LocalBlood`, `ClientCol`, `CltSrcTown`,
`ExplosionMissile`, `InitSteps`, `Qty`, `SpecialSetup`. Evidence
(1.14d, full xref, Open question 7): every `Game.exe` access with
displacement +0x135 (`InitSteps`, u8), +0x18E (`Qty`, u8) or +0x190
(`SpecialSetup`, u32) was classed by its base pointer, and every
function that obtains a missile record (inline `class × 0x1A4` + data
tables +0xB64, 214 functions, plus the 30 getters such as `0x0046ACE0`
and their callers, 430 functions) was searched for those offsets and
for word reads at +0x16 (`ExplosionMissile`). Readers: `InitSteps` —
client `0x004CD540`, `0x004D30C0`, `0x004D38D0`, `0x004D4590`, and
the unit accessor `0x006297A0`, which has no caller (no rel32 call or
jump and no absolute pointer to it in the file); `ExplosionMissile` —
client `0x004D2D70` only; `Qty`, `SpecialSetup` — none (the other
+0x18E / +0x190 sites index skills records, 0x23C, or other structs).
No D2MOO server read either. Client readers: `missiles/client.md` §C11.

`ProgSound` is read only by client missile functions (`pCltDoFunc`
table `0x0072A398`) 9, 29, 47 and 51; their conditions are owned by
`audio/triggers-2.md` §16. Function 29 (`0x004D5310` → `0x004CE850`)
tests the elapsed-frame argument against 315 (`cmp` with 0x13B at
`0x004CE8AE`), not the missile id 315.

## Constants & data dependencies

| Constant | Value | Source |
|---|---|---|
| record size | 420 (0x1A4) | `0x005ADBD9` |
| speed factor | 75 / 100 | `0x0059FA30` |
| target distance limit | < 100 subtiles per axis | `0x0059FA30` |
| VelLev divisor | 8 (signed) | `0x0059FA30` |
| pierce tries | 4 | `0x0059F940` |
| acceleration period | 5 steps | `0x006502D0` |
| step base | 0x400, shifts 6 and 12 | `0x00554CA0`, `0x006502D0` |
| frame field range | −0x8000…0x7FFF | `0x0064A2B0` |
| missile data size | 0x34 | `0x0064A100` |
| tables | `0x0073C720` (modes), `0x0073C768`, `0x0073C840`, `0x0073C960`, bit masks `0x006CE268` | dumped |
| stats | 19, 21, 22, 25, 48–60, 62, 64, 66, 103, 104, 106, 120–122, 141, 156, 161, 166, 180, 315–317, 326–332, 357 | `itemstatcost.txt` ids |
| states | 54 uninterruptable, 86 justhit, 87 slowmissiles | `states.txt` rows |
| unit event | 0 (hit by missile) | `0x005ADF10` |
| timer | type 12 (remove state) on the hit unit | `0x005ADB00` |

Live data quirks:

1. `royalstrikechainlightning` (row 568) has `pSrvHitFunc` = `*12`; the
   compiled record holds 0xFDB4 (negative as i16), so the row has no
   server-hit function (R1.3). Measured in `missiles.bin`.
2. 53 rows have `pSrvDoFunc` 0 (client-only effects); created on the
   server they would never act (R3.1).

## Randomness

Draws in order, per event:

**Creation** (`0x0059FA30`):

1. Unit allocation: one **game-seed** step derives the missile's unit
   seed (`rng.md` §5.3).
2. Damage setup (skills spec): up to three `lo' % 100` on the owner's
   (or origin's) unit seed (`0x0064A850`), plus any random calc terms.
3. Init callback (if any): re-seeds the missile's seed (§R9.4).
4. Pierce test: up to 4 `lo' % 100` on a **local seed**
   `{owner base stat 328, 666}` (no effect on any other seed).
5. Unique-mod hook (`monsters/init.md`): monster owners only.

**Per tick** (server-do 1): no draw unless a hit happens. Other
server-do functions: see `srvdo.tsv` `rng` and §R9.3–R9.5 (server-do 5:
one `roll(SubStop − SubStart)` on the missile seed at frame SubStart − 1).

**Hit on a unit** (`0x005ADF10`):

1. To-hit (`ToHit` rows): one `lo' % 100` on the **owner's** unit seed
   (chance clamped to 5…95 first, `0x0057DB22`; the formula and any
   early exits before the draw are the skills spec's).
2. Server-hit function draws (`srvhit.tsv`).
3. Damage rolls, **missile's** unit seed, R6.2 order (0–7 draws).
4. Server-damage function draws (skills spec).
5. Knockback `roll(100)` on the **missile's** seed when `KnockBack` > 0
   and the unit lacks state 54.
6. Block/dodge (unit's seed), then the monster critical hit: one
   `lo' % 100` on the **owner's** seed when the owner is a monster with
   `Crit` ≠ 0 (§R6.1 order step 4), then damage execution (skills
   spec). A monster `ToHit` missile on a player without block / dodge
   / get-hit draws therefore steps the owner's seed once on a miss and
   twice on a hit (to-hit + crit), the damage itself on the missile's
   seed.

**Expiry / barrier**: only the server-hit function's draws.

## Edge cases & original bugs

1. **Player missiles die when the player is in town** (1.14d R3.5), even
   rows with `Town`; D2MOO 1.10f lacks the rule.
2. **Missed to-hit removes the missile** regardless of pierce and
   `CollideKill` (R5.5).
3. **Blocking collision word under the missile removes it without a
   hit** (R4.6): no server-hit function, no explosion, no damage.
4. Creation that fails at "no path" (R2.3 step 14) leaves an allocated
   missile with an active every-tick event; it then behaves per R3/R4.
5. A missile with no room passes the town tests (R3.2, R3.6) and is
   removed by R4 step 5 on its first run that reaches step 5, that is
   the first run with `CollideType` ≠ 0 that survives steps 1–3.
   `Activate` does not delay it: the activate-frame test is step 7,
   after step 5.
6. `MaxVel` below the creation speed is not a cap (R4.1.4).
7. Pierce is all-or-nothing per owner counter value: the first draw
   decides whether any pierce happens; with counter 0 the draws are 66,
   18, 35, 30 (test vector), so P ≤ 66 never pierces and P ≥ 67 pierces
   4 times.
8. `roll(max − min)` never yields `max` (R6.2).
9. Mode-1 rows ignore good-aligned monsters after finding them (R5.2.4).
10. Indices 4 and 38–52 of server-do, null server-hit entries and
    server-damage 15–30 would crash if a row used them (no null checks).
11. The flag-0x400 frame formula divides by `v << 4` unsigned; negative
    v gives a huge divisor.

## Test vectors

Synthetic (CI-safe):

| Input | Expected | Rule |
|---|---|---|
| Vel 20, VelLev 0, level 5, no flags | v = 3840 | R2.3.5–7 |
| Vel 10, VelLev 8, level 3 | (13 << 8) × 75 / 100 = 2496 | R2.3.5–7 |
| Monster mode missile, quillrat1 A2, Hell, no flag 0x200 | `spike1` at level 7 + 1 = 8; skill 0; v = 3456 | R2.2, R10 |
| Monster mode missile, hireling (flag 0x200) with stat 12 = 30, mode with `MissA1` ≥ 0 | level 30 | R2.2 |
| Quill volley, `aip3` = 0 (1.14d data) | no extra missile, no seed step | R2.2 |
| Vel 10, VelLev 7, level 3 | 21 / 8 = 2 → (12 << 8) × 75 / 100 = 2304 | R2.3.5 |
| Vel 24, CanSlow, owner state 87 with stat 161 = 50 | 6144 × 50 / 100 = 3072 → 2304 | R2.3.6–7 |
| Range 40, LevRange 2, level 3 | N = 46 runs, removed in frame F + 46 | R7.2 |
| Range 50, Activate 3 | collision tested on runs 3…49 | R7.3 |
| target point = start, v ≠ 0 | target point += (1, 1) | R2.3.8 |
| aim 100 subtiles away on x | creation fails | R2.3.8 |
| velocity 1000, MaxVel 4 (1024), Accel 10 | velocities 1000 ×4 steps, 1010 at step 5, 1020 at 10, 1024 (accel 0) at 15 | R4.1 |
| pierce P = 67, owner base 328 = 0 | draws 66, 18, 35, 30 → pierce_idx 4 | R8.1 |
| P = 66, counter 0 | first draw 66 ≥ 66 → pierce_idx 0 | R8.1 |
| P = 52, counter 1 | draws 51, 31, 12, 93 → 3 | R8.1 |
| P = 80, counter 5 | draws 99 → 0 | R8.1 |
| R9.3 helper: x = 5000, elapsed 8, r = 4 | seed {5008, 666}; `lo'` 3429896298, 4050930106 → dx = 0 − 3 = −3, dy = 4 − 3 = 1 | R9.3 |
| damage min 0x100, max 0x100 | 0x100, no draw | R6.2 |
| min 0x300, max 0x100 | swapped: 0x100 + roll(0x200) | R6.2 |
| type-4 path at sub-tile (100, 100), room sub-tile rect (80, 80, 40, 40), target point (110, 104) | index 0, count 1, point[0] (110, 104), flag 0x20 set, flag 0x1 clear, result 1 | R4.3 |
| same, target point (199, 100) | Δx 99: count 1, result 1; flag 0x1 set (199 ≥ 120) | R4.3 |
| same, target point (200, 100) | Δx 100 > 99: result 0, flag 0x20 cleared, index 0 | R4.3 |
| same, target unit with a null path (position (0, 0)) | Δ 100 > 99: result 0 | R4.3 |
| new missile path: flags 0, set type 4 (`0x00648CF0`) | flags 0x60000 (0x40000 missile branch, 0x20000 saved steps) | R2.3.16 |

Real (1.14d data and recordings):

| Input | Expected | Source |
|---|---|---|
| `missiles.bin` | 684 records × 420 bytes, count 684 | measured |
| server-do table | 53 entries; non-null 1–3, 5–37; 1 = `0x005B0BC0` | `Game.exe` dump |
| server-hit table | 71 entries; null 0, 30, 34, 41, 42, 46, 49, 60–70 | `Game.exe` dump |
| row 568 `pSrvHitFunc` | 0xFDB4 in the record | `missiles.bin` |
| usage | server-do 1: 551 rows; 0: 53; server-hit 0: 591 | `missiles.txt` |

**Recorded lifetimes** (`traces/raw/20261006-022304-tick.jsonl`,
recording-confirmed). For all 69 missiles: the every-tick "set" is in the
"hin" frame F, the runs ("ex") are exactly frames F+1 … F+n with no gap,
and "hout" is in frame F+n (removed by the handler during its n-th run).
`20261006-015554-tick.jsonl` has one more (spike1, 4 runs); `…021854`
none.

| Class | Missiles | Range + LevRange × lvl | Runs n per missile (GUID@hin frame:n) |
|---|---|---|---|
| 7 spike1 | 1 | 40 | 29@3471:11 |
| 22 shafire1 | 1 | 40 | 61@4430:40 |
| 58 firebolt | 56 | 50 | 18 × 50; 47, 37, 33, 28, 25, 21 ×3, 18, 17, 14, 12 ×2, 11 ×4, 10 ×2, 7 ×2, 6 ×4, 5, 4 ×3, 3 ×2, 2 ×3, 1 ×4 |
| 120 rogue1 | 11 | 40 | 58@4303:40; others 20 ×2, 19, 17 ×3, 14, 9, 8, 7 |

Firebolt GUID list: 1@677:50 2@713:50 3@754:50 4@788:10 5@818:50
6@837:1 7@861:6 8@890:10 9@924:25 10@946:4 11@969:50 12@1010:3
13@1377:21 14@1414:50 15@1443:2 16@1463:33 17@1743:14 18@2122:12
19@2149:7 20@2166:50 21@2183:5 22@2202:1 23@2219:1 24@2236:2 25@2255:50
26@2961:21 27@2986:17 28@3468:28 30@3487:4 31@3512:50 32@3539:3
33@3565:21 34@3594:7 35@3618:4 36@3635:6 37@3659:50 38@3688:50
39@3717:6 40@3730:50 41@3761:18 42@3788:12 43@3804:11 44@3822:50
45@3835:50 46@3848:50 47@3861:50 48@3875:50 49@3889:47 50@3902:37
51@3917:50 52@3931:1 53@3946:2 59@4352:11 60@4367:11 63@4448:6
65@4493:11. Rogue1: 54@4183:17 55@4213:17 56@4243:17 57@4273:14
58@4303:40 62@4444:7 64@4474:19 66@4504:9 67@4534:8 68@4564:20
69@4594:20.

Reading:

1. Every missile that lived its full time ran exactly `Range` runs
   (firebolt 50 ×18, shafire1 40, rogue1 40): R7.2 with `LevRange` 0, so
   independent of level and speed. No run exceeded `Range`.
2. Shorter lives are early removals (R7.4): for these rows a unit hit
   with `CollideKill` 1, a missed to-hit (spike1, rogue1), the path
   stopping at a wall, or a blocking collision word. The recording has
   no positions or hit events, so the cause of each is not separated
   (open question 1).
3. The rogue1 shots are 30 frames apart (4183, 4213, …): one shot per
   rogue attack cycle (`monsters/ai.md`).
4. GUIDs are one counter for all missile classes (1–69, with 29 spike1
   and 54–69 interleaved), as `unit-order.md` §1.

## Provenance

- Creation `0x0059FA30` (decompile and asm), missile init `0x0059F8A0`
  (rel32 call at `0x0055542C` inside unit allocation, found by a byte
  scan of `.text`; the region is missing from the disassembly export),
  missile free `0x0059F8E0` (called at `0x0055570F`), pierce test
  `0x0059F940`, sync `0x0059FEE0`, damage setup `0x0059F900`, unique hook
  `0x005A43B0`.
- Dispatch `0x005ADCC0`/`0x005ADBB0`; default flight `0x005AE1F0`;
  server-do 1 (`0x005B0BC0`) is a 5-byte tail jump to `0x005AE1F0`;
  hit handler `0x005ADF10` with helpers `0x005ADA40` (mode filter),
  `0x005ADA80` (pierce), `0x005ADB00` (justhit), `0x005ADCD0`,
  `0x005AD730`; damage fill `0x005A89A0`, roll helper `0x005A8910`,
  percentage `0x00483360`; unit filter `0x005A8730`; sub-missile helper
  `0x005A9820`; missile data accessors `0x0064A100`–`0x0064A780`; path
  velocity `0x006502D0`, unit step `0x00554CA0`, `0x00650840`.
- Tables dumped from `game/Game.exe` (SHA-256 631066c1…adaaf) by parsing
  the PE section headers: `0x0073C71C`/`0x0073C768`,
  `0x0073C83C`/`0x0073C840`, `0x0073C95C`/`0x0073C960`, `0x0073C720`,
  `0x006CE268`.
- Data: `game/extracted/patch_d2/data/global/excel/missiles.txt` and
  `missiles.bin` (measured with a local script); `itemstatcost.txt`,
  `states.txt` ids.
- D2MOO 1.10f (`MISSILES/Missiles.cpp`, `MISSILES/MissMode.cpp`,
  `Units/Missile.cpp`, `MonsterUnique.cpp`, `PathMisc.cpp`): names, the
  parameter record layout, collide-type callbacks, per-function
  behaviours of `srvdo.tsv`/`srvhit.tsv` rows marked D2MOO-only. 1.14d
  differences found: owner/class validation in creation; the player-
  in-town removal (R3.5); the dead room fallback (R3.2); D2MOO's
  `STAT_ITEM_PIERCE`/`STAT_SKILL_PIERCE` names are swapped against the
  1.14d ids (0xA6 = 166 `skill_pierce`, 0x9C = 156 `item_pierce`);
  acceleration clamp `>` vs `≥`; to-hit/knockback draw sites.
- Recording: `traces/raw/20261006-022304-tick.jsonl` (69 missiles,
  1,580 runs), `20261006-015554-tick.jsonl` (1 missile), analysed with a
  scratch script pairing "hin"/"ex"/"hout" per GUID.
- Missile path compute (R4.3): `0x00649970` tests path flag 0x40000
  first and returns `0x00649760`'s result; `0x00649760` switches on
  path +0x3C (4 → `0x006492F0`, 10 → `0x0067A240`, 14 → `0x0067A140`,
  else assert 0x207); `0x006492F0` read in full. Flag 0x40000's
  source: `0x00649D00` (missile: masks 0, then `0x00648CF0(path, 4)`)
  and `0x00648CF0` (flags := flags & 0xFFF800FF | table
  `0x006EB690`[t]; row 4 = 0x60000).
- Monster mode missile and quill volley (R2.2), read 2026-10-08 from
  the asm: `0x005A6D50` (`0x005A6D6B`–`0x005A6DB7`: mode missile,
  difficulty record +0x10 + 1, flag 0x200 → stat 12), its call site
  `0x005A776E` in `0x005A7670`; `0x005A6B70` (`BaseId` test 0x3F, local
  seed `0x005A6C70`–`0x005A6C8D`, loop `0x005A6CB7`–`0x005A6D3C`).
- Server use of `InitSteps` / `Qty` / `SpecialSetup` /
  `ExplosionMissile` (R11) and creation outside the queue run (R7.1):
  call-graph and displacement scans of `Game.exe` and `all.asm`
  (2026-10-08). The edges were direct calls, jump and function tables
  (read from the image), and pushed function addresses.

## Open questions

1. Which early removals in the recording are unit hits, misses, walls or
   collision words? Settle: record per missile run the R4 exit path
   (hook `0x005AE1F0` returns and `0x005ADF10` arguments) in a combat
   recording.
2. Answered (2026-10-08), §R4.2 "Callbacks": disassembled from
   `Game.exe`; they match D2MOO (mode 7 tests `CanDestroy` without the
   shared filter).
3. Answered (2026-10-08), R4.1 rule 3: yes to both. The direction
   vector comes from the 128-row `tan` table with x² + y² ≈ 4096²
   (`sim/pathing.md` §8.3, `0x0064FC60`); the position is two u32 16.16
   sub-tile values (`sim/path-placement.md` §1 rule 2), and the step
   vector is 16.16 per tick (`sim/pathing.md` §9.4 rule 2.1,
   `0x006502D0`). Owners: those two specs.
4. Damage application, to-hit formula, block/dodge and the server-damage
   functions' formulas are the skills spec's; their draw counts complete
   the hit draw order here.
5. Does the server send anything for missiles without `ClientSend`
   (e.g. through the room update queues "qin"/"qout" seen for missiles
   in the recording)? Settle: a packets recording during a firebolt cast
   compared with `0x73`/`0x4C`/`0x4D` counts.
6. Answered (2026-10-08), R2.3 step 8: asm read of
   `0x0059FBBE`–`0x0059FC11` confirms the owner (record +0x04): x
   compared first, then y, via `0x0045ADF0` / `0x0045AE20` with the unit
   in ECX; a target unit equal to the owner is skipped before the
   compare. As D2MOO.
7. Answered (2026-10-08), "Not read by the server" paragraph after the
   field-owner table: none. Full xref of +0x135, +0x18E, +0x190 (every
   site in `Game.exe`, classed by base) and +0x16 (every function that
   obtains a missile record): only client readers (`InitSteps`
   `0x004CD540`, `0x004D30C0`, `0x004D38D0`, `0x004D4590`;
   `ExplosionMissile` `0x004D2D70`) and the uncalled accessor
   `0x006297A0`; `Qty` and `SpecialSetup` have no reader.
8. Answered (2026-10-08), R9.2 `status`: none is left. Every row of
   `srvdo.tsv` (53) and `srvhit.tsv` (71) is `spec'd-here`; the bodies
   are §R9.5–R9.6 (server-do 2, 3, 5, 7, server-hit 1, 4, 12, 13) and
   `missiles/bodies.md` §6 onward with `missiles/bodies-2.md` (the rest,
   1.14d-read). No status change was needed.
9. Answered (2026-10-08), §R2.2 "Monster mode missile" and §R10 item 2:
   a monster attack with no used skill creates its monstats mode
   missile at level `MonsterSkillBonus`(d) + 1 (1 / 4 / 8; hirelings
   use their level, stat 12) (`0x005A6D50`, 1.14d asm). Also read: the
   quill volley `0x005A6B70` (`BaseId` 63, `aip3` extra spikes on a
   GUID-seeded local seed; 0 in 1.14d data). A recording with positions
   would still confirm the speed (PC 2 list).
10. Answered (2026-10-08), R9.1: nobody. A search of `Game.exe` 1.14d
    for the absolute value `0x005AD9D0` (all sections) and for E8 / E9
    / 0F 8x rel32 branches to it in `.text` finds nothing; the export
    lists 0 callers. Dead code; not implemented.
11. Answered statically (2026-10-08), §R7 rule 1 "Creation before the
    queue run": not with 1.14d data. Paths outside the queue run reach
    the create only through the do core (Iron Golem at join, immediate
    auras; no missile) or the umod-23 mode-0 callback at a mode set on
    a kept pet in step 3 (no 1.14d pet has umod 23). A message-time
    missile would run first in the next tick anyway. The recording
    check (hook `0x0059FA30` with the tick step) is still on the PC 2
    list as confirmation.
12. Answered (2026-10-08), R4.3 table: the computes are owned by
    `sim/pathing.md` §11.2 (type 10) and §11.3 (type 14, the x87 part
    as integer math; its precision-control question is pathing Open
    question 9). Seed: `0x00649760` passes EDX = unit + 0x20 with the
    unit asserted equal to the path's unit (`0x0064998A`), so the draws
    are on the **missile's own seed**, which the setter re-seeded just
    before. Server setters of type 10 (`0x00648CF0`): init callbacks
    `0x005C9290` (`skills/bodies-2.md` §2.3), `0x005CD110`
    (`skills/bodies-3.md`), `0x005D40F0`, `0x005D4680`
    (`skills/bodies-4.md`) and `0x005AC040` (`bodies.md` §19); type 14:
    srvdo 73 Blessed Hammer `0x005D0040` (`skills/bodies-2b.md` §6.9).
    The other setters (`0x004C95E0`, `0x004D62B0`, `0x004E1C80`,
    `0x004E2ED0`, `0x004F2AC0`, `0x004F2BE0`) are client-side mirrors
    of these callbacks (e.g. `0x004E1C80` = `0x005CD110`'s shape);
    owners: the client specs.
13. *Answered* (`impl-missile-bodies-2` Q2): `elem_roll` returns the
    rolled amount of its element, not the `EType` (§R9.6 return list;
    `0x005A8C70`).
14. *Answered* (`impl-missile-bodies-2` Q3): the full roll writes the
    0x70-byte damage record directly: crit → result flags 0x2000,
    bypass 103 / 104 / 106 → hit flags 0x100 / 0x200 / 0x400 (§R6.2;
    `0x005A89A0`).
15. Does the server's 75 % speed step have the same `v > 0x100000` branch as the client's (`missiles/client.md` §C2 r7)? Settle: read the server speed step (R-section for missile speed) in the asm.
