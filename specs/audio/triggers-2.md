# Spec: Audio — Sound triggers part 2 (remaining sites, 0x2C senders, frame event 3, ProgSound)

- **Status:** draft: static answers to `audio/triggers.md` open
  questions 2, 6, 10, 12 (part) and 13, and the driver seams (§18 unit
  sound identity and `monsounds` record, §19 request lists, §20 object
  call schedule, §21 inputs), from the 1.14d `Game.exe`
  disassembly and image tables (addresses inline); no rule confirmed by
  a recording yet. Part of `audio/triggers.md`, split off to keep that
  spec under 60 KB; its conventions (§1: request(id, U, d), U, P, C),
  Constants, Randomness and open questions cover this part too.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio::triggers` (same rule functions
  as part 1, `client/audio.md` §A2).
- **Related specs:** `audio/triggers.md` (part 1; §1 conventions, §2
  the 0x2C handler, §3 player events, §4 mode sounds, §5 footsteps, §8
  skill / missile / state sounds, §12 fixed requests), `audio/sound-table.md`
  (request call), `client/model.md` §5, §8, `client/msg-units.md`
  (0xAC umod list), `client/stat-lists.md` §3 (0xA9), `sim/units.md`
  §4 (+0x44 frame, +0x4E event code), `sim/intents-events.md` §7
  (per-client unit update, the 0x2C flush), `monsters/umod-callbacks.md`,
  `monsters/init.md` (type flags), `missiles/bodies.md` (missile data
  accessors), `formats/animdata.md`, `formats/cof.md`, `client/ui.md`
  §B8, `audio/sound-table-2.md` §15 (options menu).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 49–58 |
| Inputs | 59–68 |
| Outputs / state changes | 69–74 |
| Rules | 75–76 |
|   13. Remaining fixed-request conditions (`audio/triggers.md` open question 10) | 77–174 |
|   14. Server senders of S→C 0x2C (`audio/triggers.md` open question 2) | 175–241 |
|   15. Animation event 3 ("sound") (`audio/triggers.md` open question 13) | 242–269 |
|   16. `ProgSound` conditions (`audio/triggers.md` open question 6) | 270–287 |
|   17. Options-menu UI sounds (`audio/triggers.md` open question 12, part) | 288–296 |
|   18. Sound identity of a unit and its `monsounds` record | 297–345 |
|   19. A unit's request list (U +0x78) | 346–392 |
|   20. When object units make their mode sounds | 393–431 |
|   21. What the driver needs per rule (inputs and owners) | 432–460 |
| Constants & data dependencies | 461–470 |
| Randomness | 471–476 |
| Edge cases & original bugs | 477–486 |
| Test vectors | 487–516 |
| Provenance | 517–545 |
| Open questions | 546–549 |
<!-- /index -->

## Summary

This part finishes the trigger catalogue of `audio/triggers.md`: the
conditions of the last fixed request sites of its §12 (corpse
explosions, monster death sounds, leap landing, spider lay), every
server call that queues an S→C 0x2C sound event and what the event
becomes on the client, how the animation event code 3 ("sound") reaches
the sound requests, and the conditions of the four client missile
functions that request `ProgSound`.

## Inputs

| Name | Type | Source |
|---|---|---|
| client unit fields | mode +0x10, frame +0x44 (8.8), event code +0x4E, monster data +0x14 (type flags +0x16, umods +0x1C), path +0x2C | `sim/units.md` §1, `monsters/init.md` |
| client mode requests | code 8 (death) to the monster machine `0x004AFF60` | `client/model.md` §8 |
| S→C 0xA9 EndState | state id | `client/stat-lists.md` §3 r4 |
| server unit fields | event u16 +0x6E, target +0x70, flag 0x400 (+0xC4) | `sim/intents-events.md` §7 |
| `missiles.txt` | `CltParam1`–`3` (+0x58/+0x5C/+0x60), `CltSubMissile1`/`2` (+0x1E/+0x20), `ProgSound` (+0x34) | `data/fields.tsv` |

## Outputs / state changes

Sound requests (`audio/sound-table.md` §5) and, where noted, the client
missiles, overlays and mode sets the same code makes (owned by their
specs; listed only for order).

## Rules

### 13. Remaining fixed-request conditions (`audio/triggers.md` open question 10)

#### 13.1 Corpse explosion 2,458 (`necromancer_corpseexp_1`)

1. **Client umod hooks.** The client monster's umod list (monster data
   +0x1C, 9 bytes, from S→C 0xAC) has hooks by phase in table
   `0x00724E28` (5 dwords per umod: entry `0x00724E28 + 4 × (phase + 5 ×
   umod)`), run by `0x004ADD90(U, arg, phase)` for each of the 9 bytes
   in order, with ECX = U, EDX = the umod, stack = unique (type flag
   0x08 of monster data +0x16; `monsters/init.md`). Phases: 0
   `0x004ADE40` and 3 `0x004ADE70` from the mode machine `0x004AFF60`;
   1 `0x004ADE50` (`0x004B0D81`); 2 `0x004ADE60`, every client update of
   a monster in a mode below 16 (`0x004B1556` in `0x004B13A0`,
   `client/model.md` §5 r2); 4 `0x004ADE80` from the client missile
   create (`0x004CDB2B`, ECX = the missile). Only phase 2 entries
   request sounds.
2. **Umod 9 `fire`, phase 2** (`0x004AD1A0`): when unique ≠ 0, U's mode
   is 0 (DT) and U's frame (+0x44 >> 8, arithmetic; `0x00621810`) is 4:
   with (x, y) = U's position (`0x0045ADF0`, `0x0045AE20`), five client
   missiles 117 `monstercorpseexplode` at (x, y), (x + 1, y + 1), (x +
   1, y − 1), (x − 1, y + 1), (x − 1, y − 1), then five missiles 82
   `fireexplosion` at the same five points in the same order
   (`0x004CDB40`, `0x004CD540`), then request(2,458, U, 0).
3. **Umod 31 `goboom`, phase 2** (`0x004AD0C0`): the same test, five
   missiles 82 at the same five points, then request(2,458, U, 0).
4. **Umod 40 `worms_on_death`, phase 2** (`0x004ADD80` → `0x004ADCE0`):
   no test at all: every update, missile 117 then missile 545 `pain worm
   appear` at U's position (static path +0x0C / +0x10 for types 2, 4, 5;
   else the dynamic path, 0 without one), then request(2,458, U, 0).
   Whether a client monster ever carries umod 40 is the umod owner's
   (the boss-mod cases of `monsters/init.md` §14.3 assign neither 40 nor 41).
5. **State 110 `pregnant`, remove hook.** S→C 0xA9 EndState
   (`0x0045EF60` → `0x004D9F40`) runs the state's `remfunc` (+0x1C)
   from table `0x0072A710` (30 entries); `pregnant` has `remfunc` 10 =
   `0x004D93A0` → `0x004ADCE0`: the two missiles and the request of r4
   on U. No other path reaches `0x004D9F40`.
6. A frame value of 4 can last more than one update when the animation
   speed is below 256 per tick; every such update repeats r2 / r3
   (original behaviour; d2rs reproduces it from the same frame rule).

#### 13.2 Monster death sounds (`0x004AFF60`, code 8)

1. The monster machine's code-8 request (death; its common tail sets
   mode 0 DT through `0x00480E70(U, 0)`, `audio/triggers.md` §4.1)
   branches on U's `BaseId` (monstats +0x02, `0x00463860`; −1 when the
   class is out of range) after its other work (+0xB0, hover, the
   monstats +0x1A4 death spawn):

   <!-- rows -->
   | BaseId (classes) | Before the mode set | After the mode set |
   |---|---|---|
   | 453 `minion1` (453–460, 682–684) | request(s, U, 0), s = [1,308 `minion_death_a1`, 1,311 `_b1`, 1,314 `_c1`, 1,317 `_d1`][d8 & 3] | — |
   | 461 `suicideminion1` (461–468) | overlay 204 `suicide_death` (`0x00464E50(U, 204, 0)`), then request(2,419 `sorceress_fireball_impact_1`, U, 0) | — |
   | 425 `plaguepoppy`, 426 `cycleoflife`, 427 `vinecreature` | client missile 470 `vine beast death` at U's position | request(790 `druidpod_death_1`, U, 0); then flag 0x2 of +0xC4 cleared, `0x00464930(U)`; the common tail is skipped |

   d8 = table `0x00745600` [U's direction byte (path +0x64,
   `0x00620100` / `0x006487F0`; 0 without a path)] = ((d + 4) >> 3) & 7
   for d 0–63 (the 64 directions folded to 8). The mode set runs the
   death mode sound of §4.2 of part 1, so the minion and suicide
   minion sounds come before it and the vine sound after it.
2. Other `BaseId` cases of the same switch (`0x004B0E18` /
   `0x004B0F4C` tables) create overlays only (no request).

#### 13.3 Leap landing 2,517 (`barbarian_leap_land`, `0x004C8970`)

1. `cltdofunc` 43 (Leap, `0x004C8C60`) and 44 (Leap Attack,
   `0x004C8ED0`; table `0x00727BA8`) call the landing check
   `0x004C8970(U, skill, 1)` every client update while the skill's
   flags (`0x006446A0`) have bit 0x100.
2. Not landed (returns 0, no sound): the leap height h = U's client
   motion record +0x0C >> 11 (`0x004DA150`; 0 without a record) is
   non-zero and U's position differs from the skill's target point
   (`0x006444A0`, `0x006444D0`).
3. Landed: `0x004C8890`, `0x004DA1D0(U, 0, 0, 0)`, overlay 80 `dust`
   (`0x00470390(U, 80, 2, 0, …)`, type 2 without offsets: no seed draw,
   `audio/sound-table-2.md` §14.3), then request(f, U, 0) with f = U's
   running footstep (`0x004CAEB0(U, 1)`: part 1 §5 r5 base + material +
   24; no volume is set; id 0 → nothing), then: U not a monster →
   request(2,517, U, 0); a monster with `BaseId` 78 `sandleaper1` →
   `0x004C8750` (no request); any other monster → nothing.

#### 13.4 Spider lay 1,830 (`spider_web_1`, `0x004E2D40`)

1. Called from `0x004807C0(U)`, which the player update runs for
   movement modes (part 1 §5 r1) and skill modes with skill flag bit 0
   (`0x00463390`), and the monster update for mode records of kind 1
   and the skill check (`0x004B13A0`, `0x004AF4C0`); it acts when U has
   state 22 `spiderlay` (`0x00639DF0(U, 22)`).
2. Conditions: U has a path and the path's flag 0x08 (path +0x34,
   `0x006505C0`) is set. With (x, y) = U's position, d8 = table
   `0x00745600`[path +0x65] (`0x00648810`), d16 = [10, 8, 22, 20, 18,
   16, 14, 12][d8], (dx, dy) = the signed bytes at `0x006EA998` /
   `0x006EA978` + d16: the room at (x + dx, y + dy) (`0x00463740`) in
   town (`0x0061AB00`) → nothing. Else client missile 143
   `spidergoolay` with U's skill 173 `SpiderLay` level
   (`0x006439F0`, `0x006442A0`; positions owned by the missile spec),
   then request(1,830, U, 0) whether or not the missile was created.

### 14. Server senders of S→C 0x2C (`audio/triggers.md` open question 2)

1. **Queueing.** Every 0x2C event goes through `0x00553380(unit,
   event, target)`: it stores event at unit u16 +0x6E and target at
   +0x70, marks the unit for update (`0x0064C040`) and sets unit flag
   0x400. A second call before the flush overwrites both: only the
   last event per unit is sent.
2. **Flush.** In the per-client unit update (`sim/intents-events.md`
   §7.1 r2, §7.3: player `0x00580860` at `0x00580917`, monster
   `0x00598220` at `0x00598369`, object `0x00581AD0` at `0x00581B07`)
   flag 0x400 → `0x00571740` → `0x0053D780` (8 bytes: type, GUID,
   event), sent only when target is 0 or that client's player; the
   chest code sends its key sounds at once (`0x00586000`,
   `world/objects.md` §8.1). The client handler requests at receive
   (part 1 §2 r1–r3), so the sound starts at the first sound tick after
   the receive that delivers it; the delays of part 1 §3 r4 still apply.
   PROVISIONAL: the chest's at-once send leaves +0x6E and flag 0x400
   set, so the player's unit update of the same tick sends event 11 a
   second time (because no read function clears flag 0x400 before the
   room clean-up); settled by a recording of a locked chest opened with
   a key (count of S→C 0x2C event 11 in that tick; REC-93).
3. **Call sites by event** (78 calls of `0x00553380`; the event is
   EDX, a constant at each site or `lea` of a register known to be 0
   or 1 there; owner = the spec that owns the calling function):

   <!-- rows -->
   | Event | Client effect (part 1) | Calling functions (owner) |
   |---|---|---|
   | 1 | `item_pickup` | `0x0055CF50`, `0x00563560` (`items/inventory-moves.md`) |
   | 2 | `cursor_level_up` | `0x0055E170`, `0x00570880` (`combat/vitals.md`), `0x00579D60` |
   | 4 | `cursor_convert_item` | `0x00565AB0`, `0x0056C6A0` (`world/cube.md`) |
   | 6 | `cursor_identify_item` | `0x00562590` (`world/vendors.md`) |
   | 7 | `player_townportal_cast` | `0x005BE290` (target 0) |
   | 8 | `player_townportal_enter` | `0x00584870` (`world/objects.md`, target 0) |
   | 9 | `cursor_durability_break` | `0x0055F850` (`skills/bodies.md`) |
   | 10 | `shrine_refill` + volume 120 | `0x00578D30` (`world/npc.md`, target 0) |
   | 11 | `item_key_used` | `0x00585F60` (chest) |
   | 12 | state 68 skill `stsound` | `0x0057CEE0` (`combat/events.md`) |
   | 13 | `object_trap_trigger` | `0x005818B0`, `0x00582510` (`world/objects.md`) |
   | 15 | `rogue_confirm_a_1` (class 271) | `0x0054C430` (`sim/intents-events.md`) |
   | 16 | monster `Taunt` | `0x005B1140` (`monsters/ai.md`), `0x005EF320`, `0x005FC860` (`monsters/ai-bodies-5.md`; target 0) |
   | 17 | monster flee voice | `0x005F02C0` (`monsters/ai-bodies.md`; target 0) |
   | 18 | NPC greeting on P | `0x005E68F0`, `0x005E73A0`, `0x005E7E20` (`monsters/ai*.md`) |
   | 19 | class `impossible` | `0x0054C5D0`, `0x0055E170`, `0x005628C0`, `0x00564D50`, `0x00582610`, `0x00584870` (2), `0x0058D400`, `0x0058E740`, `0x00591AC0` (2), `0x00593480`, `0x00593710`, `0x00593AF0`, `0x0059A7E0`, `0x0059DC70`, `0x005A5E50`, `0x005B5880`, `0x005B5C10` (3), `0x005B7A60`, `0x005B9B40`, `0x005BB980`, `0x005C98F0`; `0x0055C9A0` with EDX from its callers (`0x0055D01A`, `0x0056363A`: 19) |
   | 20 | class `cantuseyet` | `0x00560F00`, `0x00561220`, `0x00563D20` (target 0), `0x00594140` |
   | 22 | class `needkey` | `0x00581D40`, `0x00585F60` |
   | 23 | class `cantcarry` | `0x0055C850`, `0x0057FB70`; `0x0055C9A0` (`0x00563830`: 23) |
   | 24 | class `notintown` | `0x0056D130`, `0x005BE290` |
   | 25–32 | class chat line | `0x0054C070`: C→S 0x3F PlayAudio u16@1 when 25–32 and flag 0x400 not already set (target 0) |
   | 36, 37, 39, 45, 48 | quest lines (part 1 §3 r4) | `0x00591960` (36), `0x00594F10` and `0x00595420` (37; the latter from `0x005957EF`), `0x00593710` (39), `0x00593AF0` (45), `0x00593290` (48, target 0) (`world/quests-act1*.md`) |
   | 65, 72 | quest lines | `0x005B9520` (65), `0x005BA6A0` (72) (`world/quests-act3.md`) |
   | 75 | quest line | `0x005B49C0` (`world/quests-act4.md`) |
   | 83 | quest line | `0x0058DE30` (`world/quests-act5-2.md`) |
   | 85, 86, 87 | hireling item lines | `0x0054D230` (`items/inventory.md`) |
   | 91 | `cursor_level_up_hireling` on P | `0x0057E860` (`world/hirelings.md`) |
   | 92 | stinger re-arm | `0x005AF300` (`missiles/bodies-2.md`, target 0) |
   | 93 | `object_corpse_loot` | `0x0057FB70` |
   | caller's | — | `0x005455F0` (EDX = its second stack argument; its only caller `0x00590387` passes 0, so it never queues) |

   No site queues 3, 5, 14, 21, 84, 90 or any quest event not listed
   (33–35, 38, 40–44, 46, 47, 49–64, 66–71, 73, 74, 76–82): in 1.14d
   these events reach the client sound code only through its own
   callers of `0x004CB9C0` (17 call sites besides the 0x2C handler, e.g.
   the level-entry lines `0x004CC270`, `audio/environment.md` §4;
   the UI functions `0x0048FFE0`–`0x00498A20`; skill start
   `0x004C6140`), never through a 0x2C.

### 15. Animation event 3 ("sound") (`audio/triggers.md` open question 13)

1. The frame advance `0x00623E00` (client player `0x00463480`, monster
   `0x004B1319` / `0x004B1334`; server modes too) clears U +0x4E, then
   for each frame it crosses stores the AnimData event byte of that
   frame if it is 1–4 (`0x006218D0`; values 1 attack, 2 missile, 3
   sound, 4 skill, `formats/cof.md`). The client reads +0x4E in the
   next update, before its own advance.
2. **Generic skill do.** Player update (`0x00463390`): in a mode whose
   movement entry (table `0x00711E00`) is 2 and with a current skill
   (`0x00620250`), unless the skill's flag bit 0 path already ran the
   do this update, U flag +0xC4 bit 0x40 is clear and +0x4E ∈ {1, 2, 3}
   → `0x004C68F0(U)` → `0x004C6680(U, skill, level, 0)`: the skill's
   `cltdofunc` and its `dosound` / `tgtsound` requests (part 1 §8 r2).
   Monster update (`0x004AF4C0`): +0x4E = 4 → the same do; 1, 2 or 3 →
   the do unless it already ran or `0x00451F30(U, 0x40)` is set. So an
   event byte 3 acts as an action frame for the skill do.
3. **Event-3-only functions** (the only client compares of +0x4E with
   3): `cltdofunc` 16 (`0x004F4590`, Jab: `dosound a` / `dosound b`,
   part 1 open question 5) and `cltdofunc` 37 (`0x004C9DB0`; live:
   Charge 107, SerpentCharge 352): a player or monster with +0x4E = 3
   → the attack sound `0x004CB6A0(U, U's mode, 0)` (part 1 §4.3) with
   delay argument 0.
4. No other client code reads the event value: the AnimData info query
   (`0x0066AA80`, `formats/animdata.md` §6) returns only the first
   event frame, and the per-mode attack sounds keep their fixed delays
   (part 1 §4). d2rs: the frame-event stream feeds rules 2–3 only.

### 16. `ProgSound` conditions (`audio/triggers.md` open question 6)

Client missile functions (`pCltDoFunc`, table `0x0072A398`), run every
client update of the missile; "elapsed" = `0x0064A3B0`
(`missiles/bodies.md` accessors); P1–P3 = `CltParam1`–`3`.

<!-- rows -->
| Function | Live missiles | Condition | Unit |
|---|---|---|---|
| 9 `0x004D39C0` | 101 `meteorcenter` (2,438 `sorceress_meteor_impact`), 133, 564 (no `ProgSound`) | (`CltSubMissile1` ≥ 0 and the missile's skill row exists, else only the default `0x004CD390`) elapsed = max(P1, 1) − 2: first the missile's first attached request (unit +0x78 list head, `0x004CA990`) is detached with force (part 1 §1 r3), then `ProgSound` > 0 → request | the missile |
| 29 `0x004D5310` → `0x004CE850` | 307 `andycontrol0` | elapsed > 10, `CltSubMissile1` S > 0, and elapsed = 315 → S's `ProgSound` > 0 (310 `andycolumnfirebase`: 451 `andariel_fire_end`) | the missile's owner (`0x004639D0`) |
| 47 `0x004D5950` | 452 `moltenboulder` (2,416 `sorceress_fireball_1`) | `ProgSound` > 0, a = missile data +0x28 (`0x0064A730`), b = the missile's client motion record +0x40 (`0x004DA320`); a ≠ 0 and a ≠ b → request; then data +0x28 := b | the missile |
| 51 `0x004D5DD0` | 498 `recycler delay` (2,458 `necromancer_corpseexp_1`), 540 (none) | `CltSubMissile2` ≥ 0, elapsed = P2 and the owner exists: owner flag-ex \|= 0x40000, client missile `CltSubMissile2` created (flags 0x2000); created and `ProgSound` > 0 → request | the **new** missile |

All requests have delay 0. Correction to `audio/triggers.md` §8 r3:
`0x004CE850` is not "missile 315 only"; 315 is the elapsed-frame test of
function 29.

### 17. Options-menu UI sounds (`audio/triggers.md` open question 12, part)

The seven options-menu sites of part 1 §11 (ids 1 and 2) are mapped in
`audio/sound-table-2.md` §15 r5: 1 at `0x0047D646` (choice activated),
`0x0047D7E1` (slider dragged to a new stop), `0x0047D8F6` /
`0x0047D971` (down / up), `0x0047DA69` / `0x0047DB59` (left / right
changed the value); 2 at `0x0047D667` (action entry activated). The
other sites of part 1 §11 stay with `client/ui.md` §B8.

### 18. Sound identity of a unit and its `monsounds` record

1. **Identity** (`Unit\UnitSnd.cpp` accessors: type `0x004CA320`,
   class `0x004CA2C0`, mode `0x004CA380`). Start from U's type
   (+0x00), class (+0x04) and mode `0x00621190(U)` (types 0 and 1 with a
   running sequence, +0x30 ≠ 0: the sequence mode +0x40; else +0x10;
   `render/unit-composite.md` §1). **Only when the type is 0** (player)
   the triple then goes through the draw identity substitution
   `0x00645270` (`render/unit-composite.md` §1.1: flag-ex +0xC8 bit 3
   and the first transform state of the list with `gfxtype` 1 or 2).
   Monsters, objects, missiles and items keep their own triple (a
   monster with `dopplezon`, `valkyrie` or `shadowwarrior` is not
   substituted here, unlike its drawing). Live effect: a player with
   state 139 `wolf` sounds as monster class 430 (`wolf`, `monsounds`
   `werewolf`), 140 `bear` as 431 (`werebear`), 176 `monsterset` as
   135, 177 `delerium` as 212, with the player→monster mode map of
   `render/unit-composite.md` §1.1.
2. "Player", "monster", "class", "base class" and the mode tests in
   part 1 §4 (dispatch, hit / death, attack, block, skill voice, dead),
   §5 (footsteps: step modes, `FsCnt`, `Footstep`, the running +24) and
   §6 (idle voices) mean this identity, and so does every `monsounds`
   read (r3). Part 1 §4.1 r1's explicit calls pass m unconverted; m is
   then read in the identity's table (a transformed player's explicit
   m = 3 is the monster GH row). Rules that read the unit directly keep
   the raw values: the player event sounds (part 1 §3: `0x004CB9C0`
   reads U +0x04, class < 7), the swing table (part 1 §4.3 r1, weapon hit
   class `0x00623C20`) and the state tests. The S→C 0x2C handler
   (`0x004CBDE0`, part 1 §2 r2) reads type and class through the
   identity accessors too (class 271 tests, the monster events 16–17).
3. **Record** (`0x004CA410(U)`), with c = the identity class of r1:
   1. No unit, or c ≥ the `monstats` row count → `monsounds` row 0 (the
      empty first data row of the live table: every sound 0). Not
      reachable from live data.
   2. Raw type (+0x00, not the identity) = 1 only: client monster data
      type flag 2 (superunique, +0x16) and `superuniques` row [monster
      data +0x26] exists with `MonSound` (+0x18) > 0 → that `monsounds`
      row. Else type flag 8 (unique) or 0x10 (minion): the `monstats`
      row c's `UMonSound` (+0x16, i16) > 0 → that row (UMonSound ≤ 0
      falls through). Live: superunique `MonSound` is set only for The
      Countess (`countess`), The Smith (`smith`) and The Feature Creep
      (`smithdemon`).
   3. Otherwise `monstats` row c's `MonSound` (+0x14, i16) in 0 … rows
      − 1 → that row; any other value → no record (the rules that need
      one do nothing). The monster data fields come from S→C 0xAC
      (`client/msg-units.md`).
4. Unknown monsters and players without a transform state thus have no
   monster identity at all; a transformed player with no `MonSound` row
   gets no record (nothing), not the player rules.

### 19. A unit's request list (U +0x78)

1. **Contents.** One node per handle, newest at the head: every request
   made on U (new or merged, `audio/sound-table.md` §5 r2, r6) pushes its
   handle (`0x004CA8A0`). A unit attached twice to one compound request
   holds the handle twice.
2. **Lifetime.** A node leaves the list only (a) when the request is
   freed (`0x004B94E0`: for each unit of the request's unit list, the
   first node of that unit's list holding the handle is removed,
   `0x004CA8D0`), or (b) by a detach (`0x004BA790(h, U, force)`), which
   first removes the first node holding h from U's list **whether or not
   the request still exists**, then removes one U from the request's
   unit list and applies part 1 §1 r3. A request freed: the ended state
   (2) is freed in the next update unless it restarts
   (`audio/sound-table.md` §6.3 r1), a due one-shot without a channel
   in the same update (§6.3 r4). So the list holds waiting, playing and
   ended-not-yet-freed requests; ended ones count in every walk below
   until the next sound update frees them.
3. **Walks** (all newest first, reading the next node before acting):
   `0x004CA900(U, s)` "a request of U in s's group": B = s's group base
   (+0x60 of s's row), n = B's `Group Size` (after the group pass); the
   first handle whose request's current id (`0x004B9CA0`; a freed
   handle reads 0) lies in B … B + n − 1 (signed compares). False while
   the channels are not set up (`0x004DF880`). `0x004CA990(U)`: the
   head handle (part 2 §16 function 9). Detach walks: `0x004CA9C0(U, f)`
   every handle; `0x004CAA10(U)` every handle whose current id's row has
   `Loop` (part 1 §7 r4, no force); `0x004CB190(N)` (part 1 §10 r1, r2)
   and `0x004CB2C0`, `0x004CB220` (part 1 §4.6, §4.3 r2.2).
4. **Skill voices** (`0x004CB190(N)`, answers the code's reading of part
   1 §10 r1): only when N's identity type (§18) is 1 and N has a record
   (§18 r3): every handle whose request's current id has a group base
   (`0x00481890`) **equal to** one of the raw `Skill1` … `Skill4` cells
   (+0x44, +0x48, +0x4C, +0x50) is detached with force. The cells are
   compared as given, not reduced to their group bases; a freed handle
   has base 0 and matches a 0 cell, and its detach only removes the
   node (r2 b).
5. **Unit free** (`0x00465870`, `client/model.md` §2 rule 5): before
   the per-type frees, every handle of U is detached without force
   (`0x004CA9C0(U, 0)`: loops lose U and stop when U was their last
   unit; one-shots keep playing at their last position), then U's
   sample locks are released (`0x004CC160(U, −1)`). The per-type
   branches call `0x004CA9C0(U, 1)` again; the list is already empty,
   so that call does nothing.
6. d2rs: `SoundSystem::unit_requests` must return U's list in this
   order with ended-not-yet-freed requests included, and a detach of a
   freed handle must still drop it from the unit's list.

### 20. When object units make their mode sounds

1. **Per client update** (`0x00480810` → object update `0x004BDFF0`,
   `client/model.md` §5 rule 2): first the object light update
   (`0x004BCBB0`, `render/lighting.md` §8); then with `ClientFn` =
   `objects.txt` +0x1B4 of U's class (`data/fields.tsv` objects 150):
   `ClientFn` ≤ 3 → the mode sound call (part 1 §7, `0x004CB460`);
   `ClientFn` ≥ 4 → the client object function `0x004BDEE0` first
   (table `0x007277F0`, 19 entries by `ClientFn`; ≥ 19 fatal `0x546`),
   then the mode sound call only if that function returned non-zero.
2. **Client functions that call it themselves**: `ClientFn` 3
   (`0x004BD7C0`: calls it, returns 1, so twice per update; no live
   class), 4, 5, 6 (`0x004BD860`, `0x004BD900`, `0x004BD9C0`: call it,
   return 0, so once; live classes 110 `drinker`, 112 `gesturer`,
   114 `turner`). Their mode
   changes run on `GetTickCount` time (owner: `world/objects-client.md`
   §25–§26). `ClientFn` 18 (`0x004BDD50`; live:
   class 568 `Keeper`): when `GetTickCount` > U+0xD4 (unsigned), one step
   of U's own seed (+0x20/+0x24, `sim/rng.md` §2) r1; r1 mod 100 < 10
   (low dword, unsigned) → request(2,505 `barbarian_grunt_small_1`, U);
   a second step r2; U+0xD4 := `GetTickCount` + (r2 mod 60) × 1,000.
   Wall-clock driven: not tick-exact in 1.14d.
3. **Mode change** (S→C 0x0E code 3, `0x004BCF60`, `client/model.md`
   §8 rule 5): the mode sound call runs inside the mode change
   (`0x004BD062`), so a mode change and the next update's call can both
   request; the second sees U+0x74 = the mode and stops at part 1 §7 r3.
4. **Client-only objects** (the C object set, walked by `0x00463CC0`):
   after their update (r1) the walk runs `0x004BDEE0` once more for
   type-2 units, for every `ClientFn` (case 0 returns 1 at once), so
   `ClientFn` 3–6 objects of that set make their call a second time.
   In 1.14d only client-only (C) objects reach this second call site and no
   live C object has `ClientFn` 3–6 (C objects are classes 40–42, 65, 478;
   `world/objects-client.md` §25 r4), so the second call happens only for
   `clientsmoke` (`ClientFn` 9, no sound call of its own).
5. The mode sound call itself (part 1 §7) decides by U+0x70 / U+0x74
   whether anything is requested; the distance test of part 1 §7 r2 uses
   `0x006416D0` (the units' path distance, `sim/units.md`), not the
   sound distance.

### 21. What the driver needs per rule (inputs and owners)

Every value the trigger rules read, with the 1.14d client field and
the spec that owns its value. The client model must hold these for the
rules to run (wiring: Cross-file requests in
`docs/handoff/pc2-s4-audio.md`).

<!-- rows -->
| Input | 1.14d client field | Read by | Owner of the value |
|---|---|---|---|
| unit type, class, mode, sequence | +0x00, +0x04, +0x10, +0x30 / +0x40 | §18 r1 | `client/model.md` §2, §8; `render/unit-composite.md` §1 |
| transform state, flag-ex bit 3 | state bits, +0xC8 | §18 r1 | `client/stat-lists.md` §3; `render/unit-composite.md` §1.1 |
| monster type flags, superunique row | monster data +0x16, +0x26 | §18 r3 | `client/msg-units.md` (0xAC) |
| frame, frame count, speed, frame event | +0x44, +0x48, +0x4C (i16), +0x4E | part 1 §1 r9, §5; §13, §15 | `sim/units.md` §4; `render/unit-composite.md` |
| hit class of the last hit | +0xB0 | part 1 §4.2, §4.4 | the mode machines (part 1 open question 3; `client/model.md` §8) |
| weapon hit class | `0x00623C20(U)` | part 1 §4.3, §8 r1 | `items/inventory.md` (equipped weapon), `data/fields.tsv` `HitClass` |
| unit sound fields | +0x70, +0x74, +0x78, +0x7C, +0x80, +0x84, +0x88 | part 1 §1 r6; §19 | this spec (client-side only, zero at creation) |
| client position | `0x00620900` pixel point | `audio/sound-table.md` §8.1 r1; part 1 §6 r1.5 | `client/model.md` §8 rule 6 |
| floor material | room, floor tile list, DT1 flags | part 1 §5 r8 | `drlg/rooms.md`, `formats/dt1.md` |
| states of U | state bits | part 1 §4.2 r1, r5, §8 r4 | `client/stat-lists.md` |
| level of U, town test | room → level, `0x0061AB00` | part 1 §6 r1.4; `audio/environment.md` §1 | `client/model.md` §11 |
| day phase | `[0x007A0634]` act +0x04 record +0x00 | part 1 §10 r1; `audio/environment.md` §1 r3 | `render/lighting.md` §9 |
| weather active, intensity | `0x00473C40`, `[0x007A89A0]` | `audio/environment.md` §5–§6; part 1 §12 | `render/draw-order-2.md` §11 |
| client quest state | `[0x007C0D43]` record | `audio/environment.md` §4 r2; part 1 open question 8 | `world/quests.md` §1, `world/quests-status.md` |
| NPC interaction state | `[0x007C0D25]`, `[0x007C0D29]`, menu flags | part 1 §10, open question 8 | `ui/menus.md` |
| `ClientFn`, object seed, U+0xD4 | objects +0x1B4; +0x20/+0x24; +0xD4 | §20 | `world/objects.md`, `world/objects-client.md` §25–§26 |
| NPC greeting records | `npc-greetings.tsv` | part 1 §10 r1, r5 | this directory |
| `monsounds`, `monstats`, `monstats2`, `superuniques`, `skills`, `missiles`, `states`, item rows, `objects`, `levels`, `soundenviron` | tables | all | `data/fields.tsv` |

## Constants & data dependencies

Tables read from the image: umod hooks `0x00724E28` (5 per umod), state
remove hooks `0x0072A710` (30), direction fold `0x00745600` (64 bytes),
direction offsets `0x006EA998` / `0x006EA978` (32 signed bytes each),
`cltdofunc` `0x00727BA8`, missile `pCltDoFunc` `0x0072A398`. Live rows:
`monumod` 9, 31, 40; `states` 22, 110; `missiles` 82, 117, 143, 307,
310, 452, 470, 498, 545; `monstats` `BaseId` 425–427, 453, 461; overlays
80, 204; `skills` 43, 44, 173.

## Randomness

None of §13–§17 draws. The overlays created here are type 2 without
offsets (no draw, `audio/sound-table-2.md` §14.3); the variant draws of
the requested sounds are `audio/sound-table.md` §4.

## Edge cases & original bugs

1. Only the last 0x2C event set on a unit before its flush is sent
   (§14 r1); two events in one server tick on one unit lose the first.
2. The umod 40 phase-2 hook has no mode or frame test (§13.1 r4).
3. Frame 4 held over several updates repeats the fire / goboom
   explosion and its sound (§13.1 r6).
4. Spider lay requests 1,830 even when its missile was not created
   (§13.4 r2).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| unique monster with umods [9], mode 0, frame 0x400 | 10 missiles (117 ×5, 82 ×5), then 2,458 on U | §13.1 r2 |
| same, unique flag clear (a minion) | nothing | §13.1 r2 |
| same, frame 0x3FF | nothing | §13.1 r2 |
| 0xA9 EndState for state 110 on a unit | missiles 117, 545, then 2,458 on it | §13.1 r5 |
| code-8 request for class 456 `minion4`, direction byte 20 | d8 = 3 → 1,317 on U, then mode 0 | §13.2 r1 |
| same, direction byte 34 | d8 = 4 → 1,308 | §13.2 r1 |
| code-8 request for class 426 | missile 470, mode 0 (DT sound), then 790 | §13.2 r1 |
| barbarian leap, h = 0 at the update | `dust`, run footstep, 2,517 | §13.3 r3 |
| sandleaper1 leap lands | `dust`, run footstep, no 2,517 | §13.3 r3 |
| two `0x00553380` calls on one player in one tick (19, then 1) | client receives event 1 only | §14 r1 |
| C→S 0x3F with u16@1 = 33 | no 0x2C (outside 25–32) | §14 r3 |
| Charge, +0x4E = 3 | attack sound of U's mode, delay argument 0 | §15 r3 |
| `meteorcenter`, P1 59, elapsed 57 | first attached request detached, then 2,438 on the missile | §16 |
| `andycontrol0`, elapsed 315 | 451 `andariel_fire_end` on its owner | §16 |
| `moltenboulder`, data +0x28 = 0 | no request; +0x28 := the motion value | §16 |
| Druid with state 139 `wolf` (flag-ex bit 3), player mode 2 WL | identity type 1, class 430, mode WL; footsteps from `monsounds` `werewolf` (`FsCnt`, `Footstep`) | §18 r1, r2 |
| same Druid, player event 21 | class `needmana` of class 5 (druid, 3,425) | §18 r2 |
| superunique with `superuniques` `MonSound` = `countess` | that row, not the `monstats` row | §18 r3 |
| minion (type flag 0x10) of a class with `UMonSound` = 0 | the `monstats` `MonSound` row | §18 r3 |
| U list [h3, h2, h1]; h2's request freed | [h3, h1] | §19 r2 |
| detach(h9, U) with h9 already freed, U list [h9, h4] | [h4]; nothing else | §19 r2 |
| `0x004CB190`, `Skill1` = 1,200 (a group base), U list holds a request of id 1,202 in that group | detached with force | §19 r4 |
| object with `ClientFn` 0, per update | one mode sound call | §20 r1 |
| object with `ClientFn` 4 | client function, one call inside it, none after | §20 r1, r2 |
| `Keeper` (568), U's seed step low dword 1,234,567,807 | 1,234,567,807 mod 100 = 7 < 10 → request(2,505, U) | §20 r2 |

## Provenance

1.14d `Game.exe`: `tools/ghidra/disasm.py` of `0x004AD0C0`,
`0x004AD1A0`, `0x004ADCE0`, `0x004ADD80`, `0x004ADD90`,
`0x004ADE40`–`0x004ADE80`, `0x004B13A0`, `0x004AFF60` (code-8 branch
`0x004B053E`–`0x004B0947`, class switches `0x004B0E50` / `0x004B0F74`),
`0x004D9F40`, `0x004C8970`, `0x004C8C60`, `0x004C8ED0`, `0x004CAEB0`,
`0x004807C0`, `0x004E2D40`, `0x00553380`, `0x00623E00`, `0x006218D0`,
`0x00463390`, `0x004AF4C0`, `0x004C68F0`, `0x004C9DB0`, `0x004D39C0`,
`0x004D5310`, `0x004CE850`, `0x004D5950`, `0x004D5DD0`, `0x004CA990`;
the 82 `0x00553380` sites and their EDX values by our scratch script
over `re/exports/all.asm` (each computed value read by hand); jump and
pointer tables read with `pefile`; live rows from P `monumod.txt`,
`states.txt`, `missiles.txt`, `monstats.txt` (by `hcIdx`), `overlay.txt`
(Expansion row removed, `data/loading.md` d2-data policy 5),
`sounds.txt`, `skills.txt`. D2MOO not used.
Fifth pass (2026-10-07, §18–§21): `0x004CA2C0`, `0x004CA320`,
`0x004CA380`, `0x00645270`, `0x004CA410` (`0x004AE3A0`, `0x004AE360`,
`0x004AE380` → `0x004AC7E0` with type-flag masks 2, 8, 0x10;
`0x004AC7C0`, `0x006556E0`, `0x00451F80`, `0x00656FC0`, `0x00656F90`),
`0x004CC5B0`, `0x004CAF60`; lists `0x004B94E0`, `0x004BA790`,
`0x004CA8A0`, `0x004CA8D0`, `0x004CA900`, `0x004CA990`, `0x004CA9C0`,
`0x004CAA10`, `0x004CB190`, `0x00516950`, `0x00516980`; unit free
`0x00465870`; objects `0x004BDFF0`, `0x004BDEE0` (table `0x007277F0`
read with `pefile`), `0x004BD7C0`–`0x004BD9C0`, `0x004BDD50`,
`0x004BCF60`, `0x00463CC0`. Live rows from P `objects.txt` (`ClientFn`
by `Id`; the `Expansion` row skipped), `monstats.txt` (by `hcIdx`),
`superuniques.txt`, `monsounds.txt`.

## Open questions

Kept in `audio/triggers.md` (numbers there).
