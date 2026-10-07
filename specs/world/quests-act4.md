# Spec: World — Quests, Act IV (Izual, Hell's Forge, Terror's End, Act IV gossip)

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (disassembly of the whole Act IV quest block `0x005B36A0`–`0x005B6C58`,
  including the callbacks reached only through pointers; addresses
  inline). No Act IV quest recording exists yet; nothing is verified
  against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act4`
- **Related specs:** `world/quests.md` (owner of the shared machinery:
  flag records §1, quest records §2, game entry §3, events and dispatch
  §4, updater and timers §5, status messages §6, NPC dialog hooks §7,
  act transitions §8 (Tyrael's travel to Act V is §8.1), quest items
  and helpers §9; this spec uses its terms and never restates them);
  `world/quests-act3.md` (same layout and notation; Mephisto's soulstone
  drop and the Hellgate, its §8.5–§8.6); `world/quests-act2.md`;
  `world/quests-act5.md` (Act V side of the Act IV → V handoff, written
  in parallel); `world/quests.tsv` (records and callbacks: rows 25–28
  and 30), `world/quest-messages.tsv` (tables of chains 21–24 and 29);
  `world/npc.md` (chat §2, act travel §8.3, Tyrael's resurrection §7);
  `world/waypoints.md` (Harrogath waypoint); `world/cube.md` (no Act IV
  cube recipe touches these quests); `monsters/population.md` §3.1
  (Chaos Sanctum population guard); `monsters/ai-functions.tsv` rows 54
  NpcStationary, 55 Izual; `sim/units.md` §6.4 (object event types 1
  and 7, `fc1`); `sim/unit-events.tsv`; `sim/rng.md` §3, §6;
  `sim/tick.md` §8 (wall clock); `items/generation.md` (item creation),
  `world/objects.md` (object modes), `monsters/init.md` (spawning),
  `formats/d2s.md` §2.3 (save progression).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 53–67 |
| Inputs | 68–78 |
| Outputs / state changes | 79–90 |
| Rules | 91–92 |
|   1. Conventions | 93–159 |
|   2. Act IV records | 160–181 |
|   3. A4Q1 The Fallen Angel (chain 22, slot 25) | 182–302 |
|   4. A4Q3 Hell's Forge (chain 24, slot 27) | 303–450 |
|   5. A4Q2 Terror's End (chain 23, slot 26) | 451–700 |
|   6. Act IV gossip records | 701–727 |
|   7. Multiplayer and party rules | 728–740 |
|   8. Hooks called from other systems | 741–778 |
| Constants & data dependencies | 779–799 |
| Randomness | 800–813 |
| Edge cases & original bugs | 814–872 |
| Test vectors | 873–890 |
| Provenance | 891–928 |
| Open questions | 929–1018 |
<!-- /index -->

## Summary

Act IV has three main quests (chains 22–24, flag slots 25–27) and two
gossip records (chain 21 Tyrael, slot 24; chain 29 Hadriel, slot 33).
The Fallen Angel reacts to Izual's death (a timer spawns Izual's ghost)
and pays 2 skill points at Tyrael. Hell's Forge reacts to Cain's
messages and the Hellforge object (place the soulstone, three hammer
hits) and drops gem sets and, in expansion games, one rune. Terror's End
tracks the five Chaos Sanctum seals and the three seal bosses, clears
the Sanctum and spawns Diablo through a timer, credits players near
Diablo's death, ends a classic game on a wall-clock schedule and, in
expansion games, opens the portal to Harrogath. This spec owns those
state machines, the Act IV quest objects' quest-side logic and the hooks
other systems call into Act IV quest code.

## Inputs

| Name | Type | Source |
|---|---|---|
| events 0, 2, 3, 4, 8, 9, 10, 11, 13, 14 | quest callbacks | `quests.md` §4 |
| object operate / init / event-7 calls | operate 49, 52, 54–56, 73; init 48, 55, 56, 78; event 7 of classes 131 (level 108) and 376 | object tables `0x00732D18`, `0x00731BC0` (§1.4); `quests.md` §9.5 |
| monster and superunique creation hooks; AI hooks | calls | §8 |
| quest seed | control +0x18 | `quests.md` §2.1 |
| `GetTickCount` (import `0x006CC260`) | u32 ms | §5.7 (Open question 2) |
| `misc.txt` / `weapons.txt` codes `mss`, `hfh`; gem and rune codes | tables | §4, §6 |

## Outputs / state changes

Player and game quest bits of slots 24–28 and 33; record state, status
and flags bytes; quest extra data; S→C 0x5D, 0x28, 0x27 (text refresh),
0x29, 0x50, 0x61, 0x89; items dropped (`hfh`, gems, runes), given (`mss`)
or deleted (`mss`, `hfh`); monsters spawned (Izual's ghost, Diablo, the
three seal bosses through dummy objects); monsters killed (the Chaos
Sanctum clear); objects spawned (seal-boss dummies, the Harrogath
portal) and object modes; stat 5 (`newskills`) +2; player warps (classic
end of game, the portal to Harrogath); character act progression (save
spec); game end (classic).

## Rules

### 1. Conventions

#### 1.1 Notation

Same notation as `world/quests-act3.md` §1.1 (`s.b`, "game s.b", "state
:= n", "status n to all", "status n (silent)", "for each player (from
X)", "party of P", "refresh", "send flags", "add GUID", "GUID listed",
"quick remove", "FX b", "sound n", "holds `code`", "drop code"), with
"in Act IV" = the player's room's level has act 3 (`0x006427F0`), and
"completion flag" as there with this chain. The quick remove here
triggers on leaving The Pandemonium Fortress (level 103). "Same or
adjacent room as R" = the player's room is R or R is in the player's
room's adjacent-room list (`0x00619790`). Time "f" = the game frame.

"Radius" of a free-spot search: `0x00545340`'s sixth argument (stack
+0x14) is never read in 1.14d (body `0x00545340`–`0x0054547F`, `ret 0x14`); the ring
search is bounded by the seventh argument (limit) only. Every "radius n"
below is recorded for completeness and has no effect.

#### 1.2 Message-list selection (event 0)

As `world/quests-act2.md` §1.2: event-0 callbacks add a table state of
the record's table with `0x00543790(npc, table state)`; table states are
`quest-messages.tsv` row states of the chain.

#### 1.3 Sequence chain

Root: chain 22 (`quests.md` §3 step 3). "Call seq(c)" as in
`quests-act3.md` §1.3; here the guard tests and jumps to the **target**
record's +0xF0 (`IsBadCodePtr` on it, fatal when bad).

| Chain | Seq fn | Rule |
|---|---|---|
| 22 Fallen Angel | `0x005B38E0` | state ≠ 5 and not-intro → 1; else seq(24) |
| 24 Hell's Forge | `0x005B5F40` | state 0 and not-intro: state := 1, return 1. Else state ≠ 5 and not-intro → 1; else seq(23) |
| 23 Terror's End | `0x005B4530` | state 0 and not-intro: state := 1. Return 1 always |

Chain 22 starts at state 1 (its init). Hell's Forge starts (state 1)
when the Fallen Angel is finished (state 5) or switched off; Terror's
End starts when Hell's Forge is finished or switched off. Entering the
Chaos Sanctum also moves Terror's End (§5.3).

#### 1.4 Act IV quest objects

| Object (`objects.txt` row) | Operate fn | Init fn |
|---|---|---|
| 255 `Dummy` (Diablo start point) | — | 55 `0x005B5590` (§5.5) |
| 376 `Hellforge` | 49 `0x005B5C10` (§4.6) | 48 `0x005B5A20` (§4.6) |
| 392 `Seal` | 54 `0x005B6B70` (§5.4) | 56 `0x005B5620` (`ret`) |
| 393, 395 `Seal` | 52 `0x005B5630` (§5.4) | 56 |
| 394 `Seal` | 55 `0x005B6BC0` (§5.4) | 56 |
| 396 `Seal` | 56 `0x005B6C10` (§5.4) | 56 |
| 566 `Harrogath` (portal to Act V) | 73 `0x005B5880` (§5.9) | 78 `0x005B5840` (§5.9) |

Object event 7 (`quests.md` §9.5) reaches `0x005B5750` for class 131
(the seal-boss dummy) in level 108 (§5.4) and record 24's `0x005B6710`
for class 376 (§4.7). Live `objects.txt` values used: Seal `FrameCnt1`
20; Hellforge `FrameCnt1` 22, `FrameCnt3` 22.

Frame counts are read at each call, not cached: the operate / event
function looks up the object's own `objects.txt` record by its class
(`0x00640E90(class)`, class −1 for a missing unit; Hellforge at
`0x005B5C7D`) and takes the u32 at +0xDC (`FrameCnt1`) or +0xE4
(`FrameCnt3`), stored · 256, shifted right 8 (`0x005B5E2B`,
`0x005B5DC6`; `sim/units.md` §6.4). An implementation reads the column
of the object's row, never a constant.

### 2. Act IV records

Callback addresses are in `quests.tsv`; this table adds what the init
functions store beyond it.

| Chain / slot | Quest | Init | State, init_no, seq_id | Extra (bytes, zeroed) | F | Table |
|---|---|---|---|---|---|---|
| 21 / 24 | A4Q0 Tyrael gossip | `0x005B3780` | 0, —, — | none | — | `0x0073D580` |
| 22 / 25 | A4Q1 The Fallen Angel | `0x005B42B0` | **1**, 4, 24 | 0x1C | `0x005B3940` | `0x0073D708` |
| 23 / 26 | A4Q2 Terror's End | `0x005B54C0` | 0, 4, — | 0x4C | `0x005B4E40` | `0x0073DBB8` |
| 24 / 27 | A4Q3 Hell's Forge | `0x005B6610` | 0, 4, 23 | 0x20 | `0x005B59C0` | `0x0073E140` |
| 29 / 33 | A4Q4 Hadriel (D2MOO "Malachai") | `0x005B6A70` | 0, —, — | none | — | `0x0073E5C0` |

All are active = 1. F (status iterate) of chains 22, 23, 24 sends 0x5D
for the chain to a player when (s.0 clear and s.15 clear) or s.13 or
s.14 (the Act II rule). Chains 22–24 have no status function (default
rule, `quests.md` §6.1, init_no 4); chains 21 and 29 have status
functions that return false (`0x005B3740`, `0x005B69E0`). Act IV has no
intro record (the four intro records are Acts I, II, III, V) and no 0x91
(`quests.md` §6.7). Difficulty acts only through the record used, item
and monster data and the rune table (§4.7).

### 3. A4Q1 The Fallen Angel (chain 22, slot 25)

#### 3.1 Bits and extra data

Bits: 25.2 Tyrael gave the quest, 25.3 left town after it, 25.4 left
town with extra +0x0C set, 25.5 talked to Izual's ghost, 25.1 reward
pending, 25.13 Izual killed for this player. Extra: +0x04 / +0x08
Izual's death position x / y; +0x0C "entered the area" restored by game
start (§3.7); +0x0D ghost timer exists; +0x0F ghost talked (written,
never read); +0x10 ghost to spawn; +0x11 Tyrael started (chat-end
pending); +0x12 ghost talked; +0x14 Izual's unit (at the kill) or the
ghost unit (§3.6); +0x18 a player is near the ghost (scratch).

#### 3.2 Flag iterate (`0x005B3860`)

Players without 25.0, chain 22 present: state 2 → set 25.2; state 3 →
set 25.4 when extra +0x0C ≠ 0, else 25.3.

#### 3.3 Chat (event 0, `0x005B3BB0`; active fn `0x005B37F0`)

npc = the target's class, −1 without a target.

1. Target class 406 (`izualghost`) with 25.5 clear → table state 3
   (msg 675) and stop.
2. 25.1 set → table state 3.
3. Else the **player's** GUID listed → table state 4.
4. Else 25.0 clear, (state < 4 or 25.13 set) and not-intro → index[state]
   (`0x0073DBA0`: −1, 0, 1, 2, 3, 4; −1 or ≥ 6 → nothing).

Wants to talk: not-intro and 25.0 clear, and either tyrael2 (367) with
25.1 set or state 1, or izualghost (406) with 25.5 clear.

#### 3.4 Messages (event 11, `0x005B39A0`)

No bit or state guard beyond the listed ones.

| From | Msg | Effect |
|---|---|---|
| tyrael2 367 | 670 | extra +0x11 := 1; state := 2; flag iterate for all; refresh |
| tyrael2 367 | 676 | only with 25.1 set: if 25.13 is set: (state ≠ 5 → state := 5, call the own seq fn, flags := 0, status 13 (silent)); callback 2 := null; game 25.13. Then (25.13 set or not): set 25.0, clear 25.1, reset_progress(25); add 2 to stat 5 (`newskills`, `0x006272B0(player, 5, 2, 0)`); send `5D 16 02 00 0000` (`0x005458E0`); add GUID; refresh |
| izualghost 406 | 675 | extra +0x12 := 1, +0x0F := 1; set 25.5; status ≠ 4 → flags := 0, status 4 to all |

Chat end (event 2, `0x005B41D0`): tyrael2 with +0x11 = 1 → flags := 0,
status 1 to all; +0x11 := 0; callback 2 := null.

#### 3.5 Izual's death (event 8, `0x005B4020`)

Monster creation links base 256 (`izual`) to chain 22 unless the class
is 706 (`uberizual`) (§8). Izual is not a forced kill (`quests.md`
§4.4), so this runs only while the record is active. In order:

1. Callback 2 := null; state := 4; callback 8 := null (later kills do
   nothing); extra +0x14 := the victim.
2. Not-intro only: a killing player lacking 25.0 and 25.1 gets 25.13,
   25.1 and reset_progress(25). Extra +0x10 := 1; +0x04 / +0x08 := the
   victim's position (`0x00620870`). Then three passes over the players
   (each "for each player from the victim"):
   1. `0x005B3F20`: players in the same or adjacent room as the victim's
      room (extra +0x14), lacking 25.0 and 25.1 → set 25.13, 25.1 (no
      reset_progress). A player without a room is skipped.
   2. `0x005B3DE0`: a player with 25.13 → party of that player:
      `0x005B3D70` (in Act IV, lacking 25.0 and 25.1 → 25.13, 25.1,
      reset_progress).
   3. Completion flag `0x005B3CD0` (lacking 25.0 and 25.1: 25.14 and
      `5D 16 00 0C 0000`).
3. Always (intro games too): +0x0D = 0 → +0x0D := 1, timer period 3
   `0x005B3E60`.

#### 3.6 Izual's ghost

- Timer (`0x005B3E60`): state 4 with status ∉ {4, 13} → flags := 0,
  status 3 to all. If +0x10: +0x10 := 0; the room covering (+0x04,
  +0x08) in the Act IV DRLG (game +0xC8, `0x00619DA0`); found → spawn
  monster 406 there in mode 8 (`0x005B2F20(game, room, x, y, 406, 8, p,
  0)`) with p = −1, then 3, then 5 until one succeeds (monster spec).
  +0x0D := 0; returns 1 (one firing).
- The ghost's AI (NpcStationary `0x005E73A0`, `ai-functions.tsv` row
  54) asks `0x005B43F0(game, ghost)`: chain 22 with +0x12 set →
  +0x18 := 0, +0x14 := the ghost; every player (`0x005538D0`) at
  distance < 5 from it (`0x006416D0`) sets +0x18 := 1; returns
  +0x18 = 0. While it returns true (talked to, nobody within 5) and
  `0x005E7350` agrees (AI spec), the AI removes the ghost
  (`0x005DDFC0`) and calls `0x005B4440`: for each player `0x005B3D30`
  (25.13 set → sound 74).
  - Distance `0x006416D0(a, b)` is the size-adjusted unit distance
    owned by `missiles/missiles.md` §R9.5 item 4 (dx, dy minus half
    sizes, floored at 0, (2·max + min) / 2); it takes two units and no
    radius (callback `0x005B43D0` compares the result with 5).
  - The removal path, read in NpcStationary (`0x005E73A0`): it is
    taken only when `0x005DDF20` finds no interacting player (returns
    none or the NPC itself, `monsters/ai.md` §5.3) and the unit's class
    is 406 (`0x005E74EA`). Then `0x005B43F0` true → `0x005E7350(game,
    ghost)`: when nobody talks to the ghost (interaction list monster
    data +0x30 empty, `0x00572DC0` = 0) it sets the ghost to mode 0
    (death) at (0, 0) (`0x005DDFC0(game, ghost, 0, 0, 0)` =
    `0x005A7E60` request + `0x005A7C20(game, request, 1)`) and returns
    1; else returns 0 and the AI idles 20 (`0x005DE080(…, 20)`). On 1
    the AI sets mode 0 at (0, 0) a second time (`0x005E7521`), then
    `0x005B4440`. The ghost is never removed while someone talks to it.
- Izual's AI (`0x005F89B0`, row 55), on its first think (AI data +0x14
  = 0 → 1), calls `0x005B4390`: chain 22 not-intro with status < 2 →
  flags := 0, status 2 to all.

#### 3.7 Level change, leave, start (events 3, 10, 13)

- Event 3 (`0x005B4140`): old level 103 only: quick remove; state 2 and
  the player lacks 25.0 → state := 3; flag iterate for all; status 0 →
  flags := 0, status 1 (silent). The status test sits inside the
  state-2 / 25.0 test (both branch to the same exit `0x005B41C4`).
- Event 10 (`0x005B3E30`): remove the player's GUID (−1 without a
  player) from the record list.
- Event 13 (`0x005B4220`): 25.0 or 25.15 → nothing. Else the first set
  of 25.4 → (+0x0C := 1, status 2, state 3), 25.3 → (state 3, status 1),
  25.2 → (state 2, status 1).

#### 3.8 Reward summary

2 skill points (stat 5) per player who talks to Tyrael (676) with 25.1;
no item. Credit (25.13 + 25.1) comes from the kill, the room test or the
party pass of §3.5; every other player only gets 25.14.

### 4. A4Q3 Hell's Forge (chain 24, slot 27)

#### 4.1 Items and bits

Mephisto's Soulstone `mss ` (`misc.txt` `quest` 23: linked to chain 22,
which has no item callbacks, so its pick-ups do nothing); Hellforge
Hammer `hfh ` (`weapons.txt` `quest` 25: chain 24). Both have
`questdiffcheck` 1. Bits: 27.2 Cain gave the quest, 27.3 left town after
it, 27.5 Cain gave a new soulstone, 27.1 reward pending (forge smashed),
27.13 smashed for this player.

#### 4.2 Extra data

+0x00 Hellforge mode to restore (u16: 0, 2 soulstone placed, 3 smashed);
+0x02 Cain started (chat-end pending); +0x03 gem drops pending; +0x04
soulstone smashed; +0x08 gem tier (4 → 0); +0x0C hammer hits; +0x10
hammers in the game (written, never read); +0x14 gem sets (players
credited at the smash); +0x1C Cain gave a soulstone.

#### 4.3 Chat (event 0, `0x005B62D0`; active fn `0x005B5EE0`)

1. 27.1 → table state 2. Else player GUID listed → table state 3.
2. Else nothing when state = 0, 27.0 set, or (state ≥ 4 and 27.13
   clear), or not-intro = 0.
3. State 1: holds `mss ` → table state 0 (msg 678); else +0x1C = 0 and
   status < 3 → table state 1 (msg 679).
4. Other states: not holding `mss `, status < 3 and +0x1C = 0 → table
   state 1.

Wants to talk: cain4 (246), 27.0 and 27.1 clear, not-intro, state 1 and
+0x1C = 0. No other Act IV NPC has Hell's Forge text.

#### 4.4 Cain's messages (event 11, `0x005B6100`, NPC 246 only)

| Msg | Effect |
|---|---|
| 678 | +0x02 := 1; state := 2; refresh |
| 679 | give `mss ` (`0x005466B0(game, player, 'mss ', 0, 2, 1)`: default level, normal quality, droppable); given → +0x02 := 1, state < 2 → state := 2, +0x1C := 1, refresh; not given → nothing |
| 680 | only with 27.1: if 27.13 and state ≠ 5: flags := 0, status 13 (silent), state := 5, call the own seq fn. Then set 27.0, clear 27.1 (no reset_progress); add GUID; refresh |

Chat end (event 2, `0x005B6440`): cain4 with +0x02 = 1: status (4 when
+0x1C ≠ 0, else 1) to all, **then** flags := 0; +0x02 := 0; callback 2
:= null (never re-installed); flag iterate for all.

#### 4.5 Flag iterate, pick-up, level change, join, leave, start

- Flag iterate (`0x005B5FC0`): players lacking 27.0 and 27.1, chain 24
  present: state 2 → set 27.5 when +0x1C = 1, else 27.2; state 3 → set
  27.3.
- Pick-up (event 4, `0x005B5A90`, active records; reached by `hfh `):
  the player lacks 27.0, 27.13 and 27.1, not-intro, status ≠ 13 and
  state 0 → state := 1.
- Event 3 (`0x005B6080`): old level 103: quick remove; state 2 and the
  player lacks 27.0 and 27.1 → state := 3; flag iterate for all.
- Event 10 (`0x005B6050`): GUID removed from the record list.
- Event 14 (`0x005B64C0`): holds `hfh ` → +0x10 += 1. Event 9
  (`0x005B64A0`): the item's code (`0x00628590`) is `hfh ` → +0x10 −= 1.
- Event 13 (`0x005B64E0`): holds `hfh ` → +0x10 += 1. Then, unless 27.0
  or 27.15: not holding `mss ` → state := 1 (stop). Holding it: the
  first set of 27.5 → (+0x1C := 1, status 4, state 2), 27.3 → (status 1,
  state 3), 27.2 → (status 1, state 2).

#### 4.6 The Hellforge (object 376)

- Init 48 (`0x005B5A20`): chain 24 not-intro → object mode := +0x00;
  status ∉ {13, 2, 3} → flags := 0, status 2 to all. No record or intro
  → mode 3.
- Operate 49 (`0x005B5C10`), chain 24 required (else returns 0). A
  player with 27.0 or 27.1 → sound 19. Else by object mode:
  - **Mode 0**: holds `mss ` → mode 1 with an end-animation event at f +
    `FrameCnt1` (`0x005417D0` type 1); +0x00 := 2; status 3 to all;
    delete `mss `. Not holding it → sound 19, and not-intro with state 0
    → state := 1.
  - **Mode 2**: the player must hold `hfh `, have an inventory and
    wield an `hfh ` (the inventory's weapon, `0x0063BEF0`, items record
    code +0x80); any test failing → sound 19. Then +0x0C += 1; ≤ 2 → return (three valid hits in the whole
    game, any players). On the third: mode 3; +0x00 := 3; delete `hfh `;
    the operating player (current record) gets 27.13, 27.1,
    reset_progress(27); +0x04 := 1, +0x03 := 1, +0x08 := 4, +0x14 := 1;
    party of the player: `0x005B5B00` (in Act IV, lacking 27.0 and 27.1:
    delete `hfh ` and `mss `, set 27.13, 27.1, reset_progress, +0x14 +=
    1); state := 4; flags := 0, status 13 to all; object event 7 at f +
    `FrameCnt3`; for each player from the player, completion flag
    `0x005B5BB0` (lacking 27.0 and 27.1: 27.14, `5D 18 00 0C 0000`);
    FX 14.
  - Modes 1, 3, 4: nothing.
  Returns 0 in every case.

#### 4.7 Gem and rune drops (event 7 of class 376, `0x005B6710`)

1. +0x04 set → object mode 4.
2. Unless +0x03 and +0x14 > 0: stop. Level slot := 50 (one i32 for
   the whole call, passed as `&level`). The 50 is never used: `&level`
   is an out-parameter of `0x00559A30` (below), overwritten before it
   is read.
3. Round: for i = 0 … +0x14 − 1: table by tier +0x08 (4 → perfect, 3 or
   2 → flawless, 1 → standard, anything else → stop the whole call);
   one quest-seed step, code = table[lo' mod 7]; drop code := code;
   `0x00559A30(game, forge, 2, &level, 0, −1, 0)` (normal quality);
   created → count += 1.

   | Tier | Table | Codes (index 0–6) |
   |---|---|---|
   | 4 | `0x0073E56C` | `gpv gpr gpb gpy gpg gpw skz` |
   | 3, 2 | `0x0073E588` | `gzv glr glb gly glg glw skl` |
   | 1 | `0x0073E5A4` | `gsv gsr gsb gsy gsg gsw sku` |

4. count = 0 → stop (no reschedule; +0x03 stays 1, the tier is kept,
   no further drops). Else tier −= 1; tier > 0 → object event 7 at f +
   20; tier = 0 → +0x03 := 0 and, in an expansion game, one rune:
   one quest-seed step, `roll(11)` (`0x0045C390`, `sim/rng.md` §3) on
   the difficulty's table, drop with the same arguments.

   | Difficulty | Table | Codes (index 0–10) |
   |---|---|---|
   | normal | `0x0073E114` | `r01`–`r11` |
   | nightmare | `0x0073E514` | `r12`–`r22` |
   | hell | `0x0073E540` | `r15`–`r25` |

So each credited player adds one gem per round: rounds give perfect,
flawless, flawless, standard gems, 20 frames apart, the first at f +
`FrameCnt3` after the smash; the gem tier does not depend on difficulty.

Item level of these drops (`0x00559A30(game, unit, quality, &level,
out, −1, 0)`, args read at `0x00559A43`–`0x00559AF8`): before any
read, the function computes a level from the dropping unit and stores
it through `&level`: player (type 0) → stat 12 (`0x006253B0(unit, 12,
0)`); monster (type 1) → stat 12 (`0x00625480(unit, 12, 0)`); any other
unit → the `levels.txt` monster level of the unit's room's level for
the game difficulty (`0x0061DCA0(level, difficulty, game +0x70)`:
`MonLvl1`–`3` at +0x10, or `MonLvl1Ex`–`3Ex` at +0x16 in an expansion
game; difficulty ≥ 3 or a bad level → 1); no unit → 1; any result ≤ 1
→ 1. That value is the item level (request +0x0C, `0x00559C66`). The
drop code at unit +0xB8 (written by the quest before each call) selects
the item (`0x00633680`, fatal when unknown). For the forge (object 376,
River of Flame 107) the gems and the rune get level 27 / 52 / 77
(classic) or 27 / 57 / 85 (expansion) on normal / nightmare / hell
(live `levels.txt`). The item-creation internals are the item spec's.

#### 4.8 Hephasto's hammer (event 8, `0x005B65D0`)

Monster creation links base 409 (`hephasto`) to chain 24 (§8); the kill
is not forced, so the record must be active. Not-intro: drop code `hfh
`; one `0x00559A30(game, victim, 7, &level, 0, −1, 0)` (unique quality;
`level` is an uninitialised stack slot, written by the call before use:
the item level is Hephasto's stat 12, §4.7); created → +0x10 += 1. The
callback stays installed and nothing limits the count.

### 5. A4Q2 Terror's End (chain 23, slot 26)

#### 5.1 Bits and extra data

Bits: 26.2 Tyrael gave the quest, 26.3 / 26.4 progress (§5.3), 26.6 /
26.7 Cain / Tyrael completion talk pending (classic), 26.8 / 26.9
talked to Cain / Tyrael after the kill (expansion), 26.13 Diablo killed
for this player. Extra: +0x00 Tyrael started (chat-end pending); +0x01
timer exists; +0x02 Diablo to spawn; +0x03 end sequence running; +0x04
end the game pending; +0x05 warp pending; +0x06 Diablo start point
initialised, +0x08 its GUID; +0x0C … +0x10 seals 392 … 396 opened; +0x11
Diablo spawned; +0x13 Sanctum cleared; +0x14 Diablo killed; +0x15 save
pass done; +0x18 timer firings before the spawn, then the kill's tick
count; +0x1C players credited (count); +0x20 last player handled by the
end-game warp (written, never read); +0x24 / +0x2C / +0x34 seal-boss
positions (x, y pairs); +0x3C Diablo's room; +0x40 portal mode seen;
+0x44 portal request (never set to 1); +0x45 portal spawned; +0x48
non-Diablo chain-23 kills (the seal bosses).

#### 5.2 Chat and messages (events 0, 11, 2; active fn `0x005B4450`)

Event 0 (`0x005B4790`); npc = the target's class or −1; X = expansion
game (game +0x70):

1. Not X: 26.7 and tyrael2 → table state 2; 26.6 and cain4 → 2; 26.7
   (other NPC): cain4 → 3, others nothing; 26.6: tyrael2 → 3, others
   nothing.
2. 26.0 set: not X → nothing. X: 26.9 clear → tyrael2 table state 4,
   cain4 table state 5; then 26.8 clear → cain4 table state 4, tyrael2
   table state 5 (so with both clear Tyrael lists states 4 and 5, Cain
   5 and 4).
3. 26.0 clear: (state < 4 or 26.13) and not-intro and state ≠ 0 →
   index[state] (`0x0073D56C`: −1, 0, 1, −1, −1; −1 or ≥ 7 →
   nothing).

Wants to talk: tyrael2 with 26.0 and 26.15 clear and state 1; or with
26.0 and (not X: 26.7; X: 26.9 clear). cain4 with 26.0 and (not X:
26.6; X: 26.8 clear).

Event 11 (`0x005B4670`):

| From | Msg | Effect |
|---|---|---|
| tyrael2 367 | 681 | +0x00 := 1; state := 2; flag iterate for all; refresh |
| tyrael2 367 | 684 | clear 26.7 |
| tyrael2 367 | 20000 | X only: set 26.9 (current record); +0x45 = 0 → the portal (§5.9) |
| cain4 246 | 685 | clear 26.6 |
| cain4 246 | 20001 | set 26.8 (any game type) |

Chat end (`0x005B4F90`, never cleared): tyrael2: +0x00 = 1 → status 1
to all, +0x00 := 0; then X with +0x44 set and +0x45 clear → the portal
(dead: +0x44 is never set).

#### 5.3 Flag iterate, level change, start, join

- Flag iterate (`0x005B4560`): players lacking 26.0, chain 23 present:
  state 2 → 26.2; state 3 → 26.3 when status = 1, else 26.4.
- Event 3 (`0x005B4EA0`, no quick remove): old level 103 and state 2:
  the player lacks 26.0 → (state < 3 → state := 3; status < 1 →
  flags := 0, status 1 (silent); flag iterate for all); a player with
  26.0 stops here. New level 108 and not-intro: state < 3 → state := 3;
  status < 1 → status 1 to all; flag iterate for all.
- Event 13 (`0x005B5080`): X, 26.9 set and 28.0 clear → clear 26.9.
  Then unless 26.0 or 26.15: the first set of 26.4 → (status 2, state
  3), 26.3 → (state 3, status 1), 26.2 → (state 2, status 1).
- Event 14 (`0x005B5030`): X, 26.9 set and 28.0 clear → clear 26.9.

The record's state never passes 3: completion lives in the bits and
extra +0x14. With init_no 4 the default status rule reports 12 to a
player with 26.14, else the status byte.

#### 5.4 Seals and seal bosses

- Seal activation (`0x005B5630`, operate 52 and the tail of the boss
  seals; operate args): object mode ≠ 0 → nothing. Mode 1 with an
  end-animation event at f + 2·`FrameCnt1` (`sim/units.md` §6.4);
  chain 23 (absent → stop): seal class 392 … 396 → +0x0C … +0x10 := 1;
  all five set and +0x48 = 3 → the Sanctum clear (§5.5) and the Diablo
  timer start (§5.6). Returns 0. No player test.
- Boss seals: operate 54 (`0x005B6B70`, object 392), 55 (`0x005B6BC0`,
  394), 56 (`0x005B6C10`, 396). Object mode 0 and chain 23 present:
  boss spot := the seal's position + offset, written to its pair; then
  `0x005B6AD0`: the room covering the spot (`0x00463740`); free spot
  (`0x00545340`, size 3, mask 0x3F11, radius r (unused), limit 100; the spot is
  updated in place); found → object 131 there (`0x00555230`, flags 1,
  0, 0); created → `0x0061AED0(room, 0)` and the seal activation above.
  No spot or no object → the seal stays in mode 0 (it can be operated
  again; the spot is recomputed from the seal).
  - `0x0061AED0(room, 0)` sets flag 0x400000 on the room's DRLG room
    (+0x10, flags +0x28; `0x0061BAC0`; with a nonzero second argument
    it clears it). That flag makes the room-removal test fail
    (`0x0061BA30`, `drlg/rooms.md` §8 step 1), so the dummy's room is
    never deactivated; no Act IV code clears it again.
  - The dummy schedules its own first event 7 in its init (object 131
    `InitFn` 59 = `0x0054FE10`, run when it is created): only when its
    mode is 0, mode := 1, then object event 7 at f + 27 and event 1 at
    f + `FrameCnt1` + 1 (Dummy 131 `FrameCnt1` 20). So the boss spawn
    first runs 27 frames after the seal is opened; the retry of a
    failed spawn is f + 10 (below).

  Column r is the unused radius argument (§1.1).

  | Seal | Pair | Offset | r | Boss (data tables +0xAE0 entry) |
  |---|---|---|---|---|
  | 392 | +0x24 | (−12, −52) | 13 | 36 Infector of Souls |
  | 394 | +0x2C | (−39, +33) | 14 | 37 Lord De Seis |
  | 396 | +0x34 | (+32, +16) | 15 | 38 Grand Vizier of Chaos |

- Dummy event 7 (`0x005B5750`, class 131 in level 108): its position
  equals a pair → `0x00545C30(game, dummy, &pair, 2, id)` (`quests-helpers.md` §3) with id = the
  u16 entry 36 / 37 / 38 of the data-tables array at +0xAE0 (+0xB28,
  +0xB2A, +0xB2C; read elsewhere by `0x00586B30`, ≤ 0x41 entries);
  spawn fails → event 7 again at f + 10. No pair → nothing. The array
  is the hcIdx → `superuniques.txt` row map built by the superuniques
  loader `0x006552E0` (`data/loading.md` §8 superuniques row: every hcIdx
  0–65 must occur, first row wins; fill `0x0065560F`–`0x0065565C`, fatal
  check `0x0065565E`). In live data entries 36–38 are rows 36–38
  (the `Expansion` separator is txt line 42, after them).
- Superunique creation (`0x005A4440`, `0x005A49B0`) links hcIdx 36, 37,
  38 to chain 23 (§8); their kills are forced (`quests.md` §4.4).

#### 5.5 Clearing the Sanctum (`0x005B5230`)

Once (+0x13 = 0): +0x13 := 1; FX 12; for each active room of the Act IV
DRLG (`0x0061A180(game +0xC8)`, next +0x7C) of level 108, each unit of
its unit list (+0x74, next read first, +0xE8) that is a monster (type 1),
not class 243, alive (`0x005541B0` = 0) and of alignment 0 (evil,
`0x006259B0`): death through a mode change (`0x005A7E60`, then
`0x005A7C20(game, request, 1)`; monster spec). While +0x13 is set,
level-108 rooms are no longer populated (`monsters/population.md` §3.1
step 6, `0x005B5210`). Runs in intro games too.

#### 5.6 Diablo's spawn

Trigger (from the third seal-boss kill §5.7, the last seal §5.4 or the
start point §5.5 below): all five seals open, +0x48 = 3 exactly, clear
the Sanctum, then if +0x01 = 0: +0x18 := 0, +0x02 := 1, +0x03 := 0,
+0x01 := 1, timer period 1 `0x005B4BE0`.

Start point init 55 (`0x005B5590`, object 255): chain 23 and +0x11 = 0:
+0x06 := 1, +0x08 := its GUID; then the same trigger test.

Timer (`0x005B4BE0`), spawn part (+0x02 set): +0x18 += 1; below 10, or
+0x06 clear → return 0. Else the start-point object (`0x00552F60`, type
2, GUID +0x08) at its x, y and room: `0x005B4B60` spawns monster 243 in
mode 1 (`0x005B2F20`, p = −1, then 5, then 10); created → unit flags |=
0x3000000, +0x11 := 1; +0x02 := 0, +0x01 := 0, return 1 (timer freed).
Failure → return 0 (retried every firing). The three tries use the same
x, y and room (the start point's, read once per firing; the last
`0x005B2F20` argument before the flags is the placement spread,
`monsters/init.md` §2); a start-point GUID that no longer finds an
object (`0x00552F60` null, `0x005B4C22`) also returns 0, so it counts
as a failed spawn and is retried at the next firing.

So Diablo appears at the 10th firing (`quests.md` §5: the 10th firing of
a period-1 timer made at updater tick T is at T + 20, 400 frames) or at
the first firing after the start point initialises.

#### 5.7 Kills (event 8, `0x005B52E0`)

- Victim not class 243: +0x48 += 1; = 3 → the trigger of §5.6.
- Diablo (243; linked at creation unless class 705, forced kill):
  1. Classic: FX 13. +0x14 := 1. The victim's room → +0x3C (no room:
     stop).
  2. A killing player and +0x01 = 0: +0x02 := 0, +0x03 := 1, +0x04 :=
     1, +0x05 := 1, +0x15 := 0, +0x01 := 1, +0x18 := `GetTickCount()`;
     timer period 1 `0x005B4BE0` (both game types).
  3. Not-intro: +0x1C := 0; status 13 to all (flags not cleared); for
     each player from the victim, four passes: `0x005B5140` (same or
     adjacent room as +0x3C, lacking 26.0 and 26.1 → credit),
     `0x005B4DF0` (26.13 → party of the player: `0x005B4DA0`, in Act IV
     and lacking 26.0 → credit), completion flag `0x005B4970` (lacking
     26.0 only: 26.14, `5D 17 00 0C 0000`), `0x005B49C0` (26.13 → `5D
     17 02 00 0000` and sound 75). Then, when the killer lacked 26.0,
     `0x00545990` (a `ret 4` stub) is called +0x1C times.
  The killer has no direct credit: it is credited only by the room or
  party pass.
- Credit (`0x005B4D20`): set 26.13, 26.0, reset_progress(26); classic:
  set 26.6, 26.7 and `0x00538680(client, 4, difficulty)` (save spec);
  +0x1C += 1.

#### 5.8 End of a classic game (timer `0x005B4BE0`, end part)

With +0x02 clear: +0x03 clear → return 1. Else with now =
`GetTickCount()` and t0 = +0x18, checked at each firing (every 2
updater ticks):

1. +0x04 set and now > t0 + 95000: +0x01, +0x03, +0x04 := 0; classic →
   `0x00530590(game, 0)` (host: end the game; see below and `quests-helpers.md` §6). Return 1.
2. Else now > t0 + 90000: +0x05 set → classic: for each player
   `0x005B4A80`; +0x05 := 0. Return 0.
3. Else now > t0 + 75000 and +0x15 = 0: +0x15 := 1; `0x0052E2A0(game)`
   (host save pass, game types 1 and 2 only). Return 0.

`0x005B4A80` per player: end its interaction (`0x005351C0`, `quests-helpers.md` §5); player
data +0x4C := 1; extra +0x20 := player; 26.13 set → level warp to 103
(`0x0053AEC0(game, player, 103, 0)`, `drlg/levels.md`) and `5D 17 01 00
0000`; else 0x50 with byte 0 = 0x50, u16 23 at 1, bytes 3–14 unwritten
stack (Open question 3). Expansion games run the timer to step 1 too but
skip the warp and the game end. `0x005B4A30` (warp and `5D 17 01`) has
no caller.

Host calls. `0x00530590(game, c = 0)` (only caller `0x005B4C9D`) picks
one client as `quests-helpers.md` §6 gives (the last in-game client
passing `0x00539030`, i.e. client +0x3D4 bit 5, else the first in-game
client) and hands it to the host removal `0x005303D0(0x00538590(C), 0)`.
That removal reads the bit once for the picked client: set → after
dropping it, it keeps taking the game's head client (game +0x88,
`0x00539070`) and drops it until the list is empty (whole-game
teardown); clear → only that client is dropped. So the end is a
whole-game teardown exactly when some in-game client has the bit, else
a one-client drop (supersedes both merged readings, "closes the game"
and "drops one client"). The timer then returns 1. `0x0052E2A0(game)` (the
Act V save pass, `quests-act5-2.md` open question 3) saves and uploads
characters: host save / transport code. Both are owned by `d2-server`
(hard rule 6: `d2-sim` has no I/O); `d2-sim` emits a host request (end
game; save pass) at the 1.14d call point, in call order with the
frame's other outputs, and continues as if the call returned.

#### 5.9 The portal to Harrogath (object 566, expansion)

- Spawn (`0x005B45E0`, from msg 20000): spot := Tyrael's position + (5,
  0); free spot near it (`0x00545340`, size 2, mask 0x400, radius 12 (unused),
  limit 100); found → object 566 (`0x00555230`, flags 1, 1, 0); created
  → +0x44 := 0, +0x45 := 1, unit flags |= 0x3000000. At most one per
  game; nothing retries a failed spawn except the next 20000.
- Init 78 (`0x005B5840`): chain 23: +0x40 ≠ 2 → +0x40 := 2, mode 1;
  else mode 2.
- Operate 73 (`0x005B5880`): classic → nothing. A player with neither
  26.0 nor 26.13 → sound 19. Else, in Act IV: if 28.0 is clear: set
  28.0, 28.13; when the client exists and the player is not busy
  (`0x00535060` = 0, `items/inventory.md`): clear its interaction
  (`0x00554190`), player data +0x4C := 1, send `5D 17 02 00 0000`, then
  `61 05`. Then (28.0 set or not): act change to level 109
  (`0x0054B830(game, player, 109, 5)`); send flags; activate the
  Harrogath waypoint for the game difficulty (`0x005B4FF0`: level 109's
  waypoint index (`0x00660E00`) set in player data +0x1C + 4·d
  (`0x00660EC0`), `waypoints.md`). Returns 0.

Tyrael's travel action (`npc.md` §8.3, `quests.md` §8.1) is the other
route to Act V; it requires 26.0. The Act V side: `world/quests-act5.md`.

#### 5.10 Classic-only gates

`0x005B5810(game)`: expansion → 0; classic → 1 when chain 23 is absent
or +0x14 is set. Callers `0x00566E60`, `0x00567620`, `0x00568060` (two
sites; player interaction and trade code, `items/inventory.md`) refuse
when it is 1: after Diablo dies in a classic game those interactions
stop. `0x005B5730` (returns +0x14) has no caller.

### 6. Act IV gossip records

#### 6.1 A4Q0 Tyrael (chain 21, slot 24)

Event 0 (`0x005B36D0`): target tyrael2 (367) with 24.0 clear → table
state 0 (msg 664). Event 11 (`0x005B36A0`): 664 from 367 → set 24.0.
Wants to talk (`0x005B3750`): 367 with 24.0 clear. Status fn false; no
event 13, so 24.0 persists through the save only.

#### 6.2 A4Q4 Hadriel (chain 29, slot 33, NPC 408 `malachai`)

Event 0 (`0x005B6940`): Hadriel with 27.0, 27.1 and 27.13 all clear →
table state 0 (msg 668); else chain 23 absent or Diablo not yet killed
in this game (extra +0x14 = 0) → table state 1 (msg 669). Wants to talk
(`0x005B69F0`): Hadriel and (27.0, 27.1, 27.13 clear, or 26.13 and 26.0
clear). No event 11: nothing is ever set in slot 33. Re-read: the second
branch tests 26.13 (`0x005B6A46`) then 26.0 (`0x005B6A53`) and is true
only when both are clear.

#### 6.3 Halbu, Jamella and the rest

No 1.14d quest record lists halbu (257) or jamella (405), and no Act IV
quest message entry names them (`quest-messages.tsv`); their text is the
NPC's own chat (`npc.md` §2), not quest code. Cain (246) speaks only
through chains 22–24, Tyrael through 21–23, Izual's ghost through 22,
Hadriel through 29.

### 7. Multiplayer and party rules

| Quest | Credit | Others |
|---|---|---|
| Fallen Angel | killer; players in the same or adjacent room as Izual; party members (in Act IV) of anyone credited | 25.14 + `5D 16 00 0C 0000` |
| Hell's Forge | the third hammer hitter; party members in Act IV (their `hfh ` and `mss ` deleted); each credited player adds one gem per round | 27.14 + `5D 18 00 0C 0000` |
| Terror's End | players in the same or adjacent room as Diablo's death room; party members in Act IV of credited players | 26.14 + `5D 17 00 0C 0000` |

Player order is `unit-order.md` §2 r5 ("from" the victim, object or
player). Rewards are per player (Tyrael 676, Cain 680) and need the
player's own 25.1 / 27.1; the Terror's End credit is final at once (no
reward-pending bit). Single player has no party (party id 0xFFFF).

### 8. Hooks called from other systems

| Caller | Hook | Effect |
|---|---|---|
| monster creation, base-id switch `0x005B1CF0` | `0x005436B0` | base 243 → chain 23 unless class 705; 256 → chain 22 unless 706; 409 → chain 24; bases 340–343 (`boneprison1`–`4`): unit flags \|= 0x20000, no link. Bases 243 / 256 also get `0x005A4850(…, 22, 1)` (monster spec) |
| superunique creation `0x005A4440`, `0x005A49B0` | `0x005436B0` | hcIdx 36, 37, 38 → chain 23 (`0x005A49B0` then `0x005A4850(…, 22, 1)`) |
| item creation `0x00555D20` | `0x005436B0` | `hfh ` → chain 24, `mss ` → chain 22 (`quests-act3.md` §10) |
| room population `0x0054EBC0`; inactive-unit restore `0x005424F0` | `0x005B5210` | +0x13 of chain 23 (§5.5); chain 23 absent → 0 (`0x005B5226`) |
| NpcStationary AI `0x005E73A0` | `0x005B43F0`, `0x005B4440` | §3.6 |
| Izual AI `0x005F89B0` | `0x005B4390` | §3.6 |
| object event 7 `0x005449E0` | `0x005B5750`, `0x005B6710` | §5.4, §4.7 |
| interaction / trade `0x00566E60`, `0x00567620`, `0x00568060` | `0x005B5810` | §5.10 |
| Mephisto's death (`quests-act3.md` §8.5) | `0x005B6930` | `ret` stub |

Every "→ chain c" above is the generic link `0x005436B0(game, unit, c)`
(`quests.md` §4: prepend a link to unit +0x74 unless the record is
already linked; chains 4, 8, 12 have special cases, Act IV chains none),
the same for chains 22, 23 and 24. Call sites, all in creation code
(the caller makes the link, not quest code):

| Chain | Site | Function / condition |
|---|---|---|
| 23 | `0x005B1E36` | `0x005B1CF0` BaseId 243, class ≠ 705; umod 22 first (`0x005B1E2B`) |
| 22 | `0x005B1F5C` | `0x005B1CF0` BaseId 256, class ≠ 706; umod 22 first (`0x005B1F51`) |
| 24 | `0x005B1F32` | `0x005B1CF0` BaseId 409 |
| 23 | `0x005A466D` | `0x005A4440`, hcIdx 36–38 (hcIdx − 6 through byte table `0x005A46A0` → case 2 of `0x005A4680`); no umod |
| 23 | `0x005A4C7D` | `0x005A49B0`, hcIdx 36–38; umod 22 after (`0x005A4C8A`) |

`0x005A4850(game, monster, 22, 1)` (`monsters/init.md` §14.1, §19)
marks the monster unique (`0x005A0320`) and appends umod 22
`questcomplete` to the first free byte of its 9-byte umod list (monster
data +0x1C); umod 22 has no init function (entry 22 of `0x0073C008` is
null), so nothing else changes.

`0x005B3130`–`0x005B3690` (monster group spawn helpers, callers in skill
and AI code) sit in the same block but are not quest code (monster
spec).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| message-state index tables | chain 22 `0x0073DBA0` (6 entries), 23 `0x0073D56C` (5) | image (§3.3, §5.2) |
| gem tables | 3 × 7 codes `0x0073E56C`, `0x0073E588`, `0x0073E5A4` | image (§4.7) |
| rune tables | 3 × 11 codes `0x0073E114`, `0x0073E514`, `0x0073E540` | image (§4.7) |
| seal jump table | class − 392 → +0x0C … +0x10 | `0x005B571C` |
| gem tier jump table | index tier − 1: 0 standard, 1 and 2 flawless, 3 perfect | `0x005B691C` |
| levels | 103 The Pandemonium Fortress, 105 Plains of Despair, 107 River of Flame, 108 Chaos Sanctum, 109 Harrogath | `levels.txt` |
| objects | 131, 255, 376, 392–396, 566 | `objects.txt` (§1.4) |
| NPCs / monsters | 243 diablo, 246 cain4, 256 izual, 257 halbu, 340–343 boneprison1–4, 367 tyrael2, 405 jamella, 406 izualghost, 408 malachai (Hadriel), 409 hephasto; 705 uberdiablo, 706 uberizual | `monstats.txt` |
| superuniques | 36 Infector of Souls (megademon3), 37 Lord De Seis (doomknight3), 38 Grand Vizier of Chaos (fingermage3), 41 hephasto | `superuniques.txt` |
| sounds | 19 refused, 74 Izual completion, 75 Diablo completion | §3.6, §5.7 |
| FX bytes (0x89) | 12 Sanctum cleared, 13 Diablo killed (classic), 14 Hellforge smashed | §5.5, §5.7, §4.6 |
| timer periods (updater ticks) | 3 (ghost), 1 (Diablo spawn / end game) | §3.5, §5.6 |
| object event delays (frames) | seal 2·`FrameCnt1`; forge `FrameCnt1`, `FrameCnt3`, then 20; dummy retry 10 | §4.6, §4.7, §5.4 |
| wall-clock delays (ms) | 75000 save, 90000 warp, 95000 game end | §5.8 |
| rewards | stat 5 +2; gems ×(4 rounds × credited players); one rune (expansion) | §3.4, §4.7 |
| item level slot for gems / runes | 50 | §4.7 |

## Randomness

Quest seed (control +0x18) draws, in occurrence order:

| When | Seed | Draws | Decides |
|---|---|---|---|
| each gem round (event 7 of the forge) | quest | +0x14 steps (`lo' mod 7`), each followed by that gem's item drop | gem of the tier |
| after the 4th round, expansion | quest | 1 (`roll(11)`) | rune of the difficulty's table |

Item drops (hammer, gems, runes, the given soulstone) draw from their
own seeds (item spec); spawned monsters and objects from their spawn
code; the free-spot searches draw nothing (`quests.md` §9.1). No other
Act IV quest code draws.

## Edge cases & original bugs

1. Scroll messages are not state-guarded: a 670 from Tyrael (C→S 0x31)
   sets Fallen Angel state 2 whatever the state, a 681 sets Terror's End
   state 2 (§3.4, §5.2).
2. Izual's kill callback clears itself: only the first linked kill
   counts; with the record inactive (switched off by the first player,
   `quests.md` §3) no ghost spawns at all (§3.5).
3. The ghost spawns only in not-intro games, but the ghost timer is
   made in every game (§3.5).
4. Event 0 tests the player's GUID against the record list (§3.3 step
   3); D2MOO 1.10f tests the NPC's.
5. Hell's Forge counts three hits per game, not per player (§4.6).
6. A gem round in which no gem is created ends all further drops,
   including the rune (§4.7).
7. Cain's chat-end callback of chain 24 is removed after the first use
   and sends its status before clearing the flags byte (§4.4).
8. Hephasto drops a hammer on every linked kill (§4.8).
9. The Terror's End killer is credited only through the room or party
   pass (§5.7); a killer standing two rooms away gets 26.14 only.
10. Diablo spawns only when +0x48 is exactly 3 (§5.6): a fourth
    non-Diablo chain-23 kill before the trigger would block him (only
    the three seal bosses are linked in 1.14d data).
11. The Sanctum clear and Diablo's spawn ignore not-intro; Diablo's
    death sets +0x14, FX 13 and the end timer in intro games too (§5.5,
    §5.7).
12. The classic end of game runs on `GetTickCount`, not on frames
    (§5.8); the 0x50 sent to uncredited players carries 12 unwritten
    bytes.
13. Terror's End's state never passes 3 (§5.3).
14. The expansion portal's chat-end path is dead (+0x44 never set,
    §5.2); the portal accepts 26.13 without 26.0, Tyrael's travel needs
    26.0 (§5.9).
15. Joining or starting a game with 26.9 and no 28.0 clears 26.9, so
    Tyrael offers the portal again (§5.3).
16. The level slot of the gem and rune drops is set to 50 once per
    event and reused for every drop of that event (D2MOO resets it per
    gem) (§4.7, Open question 5). Answered (Open question 4): the slot
    is an out-parameter, overwritten by every `0x00559A30` call with the
    forge's area level before it is read, so neither the 50 nor the
    reuse changes any outcome.
17. Hadriel's text tests game state (+0x14) but his wish to talk tests
    the player's bits (§6.2).
18. `0x005B4A30`, `0x005B5730` and the stub `0x00545990` have no effect
    or no caller in 1.14d.
19. The ghost's removal sets mode 0 (death) twice in one AI call: once
    inside `0x005E7350`, once right after it (§3.6). An implementation
    issues both mode requests.
20. Each seal-boss dummy pins its room active for the rest of the game
    (room flag 0x400000, §5.4).
21. The gem and rune item levels follow the forge's area level and the
    game type, not the quest: an expansion hell forge drops level-85
    gems and rune (§4.7).
22. The 12 unwritten bytes of the uncredited 0x50 (§5.8) are never read
    by the client: `0x0045E370` copies the 15 bytes and `0x004B9210`
    switches on the u16 at 1 only (23 → case 4, `0x004B924B`, which
    calls client code with no payload). Trace comparisons mask bytes
    3–14 of that message; `d2-sim` writes zeros there.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| quest seed {12345, 666}, one credited player, expansion, normal, all drops created, no other quest-seed use | lo' 22752887 → `gpy`; 2337785264 → `gly`; 1617882871 → `gly`; 4008788125 → `sku`; rune lo' 3081094168 mod 11 = 9 → `r10` | §4.7; seed steps as `npc.md` vectors |
| same, two credited players | lo' mod 7 per step: 3, 3 / 3, 6 / 5, 3 / 6, 1 → (`gpy`, `gpy`), (`gly`, `skl`), (`glw`, `gly`), (`sku`, `gsr`); rune step 9 lo' 2190598022 mod 11 = 8 → `r09` | §4.7 |
| same seed, hell | rune index 9 → `r24` | §4.7 |
| Tyrael 676 with 25.1 and 25.13, state 4 | state 5; status 13 (silent); 25.0 set, 25.1 clear; stat 5 += 2; `5D 16 02 00 0000`; Hell's Forge state 0 → 1 | §3.4, §1.3 |
| Izual killed by A; B in an adjacent room; C in B's party, elsewhere in Act IV; D in no party, in Act I; none has 25.0 / 25.1 | A, B, C: 25.13 + 25.1 (A and C also reset_progress); D: 25.14 and `5D 16 00 0C 0000` | §3.5 |
| Hellforge, mode 2, three valid hits by two players | 3rd hit: mode 3, credit for the 3rd hitter (+ party) | §4.6 |
| seals 392–396 opened, then the 3 seal bosses killed | 3rd kill: FX 12, evil level-108 monsters killed, timer; Diablo at the 10th firing (T + 20 updater ticks) | §5.6 |
| seal 392 at (1000, 1000), free | dummy 131 at (988, 948); its event 7 spawns superunique 36 | §5.4 |
| Diablo killed, classic, killer in his room | killer credited (26.13, 26.0, 26.6, 26.7), status 13, sound 75, FX 13; 75 s later save pass, 90 s warp to 103, 95 s game end | §5.7, §5.8 |
| expansion, Tyrael 20000 | 26.9 set; portal 566 at Tyrael + (5, 0) or the nearest free spot | §5.9 |
| forge gem / rune drops, forge in level 107; classic normal / nightmare / hell; expansion normal / nightmare / hell | item level 27 / 52 / 77; 27 / 57 / 85 | §4.7 (live `levels.txt`) |
| seal 392 operated at frame f, dummy created | dummy mode 1; its event 7 at f + 27 (boss spawn), event 1 at f + 21; spawn failure → next try f + 37 | §5.4 |
| `0x006416D0`: units of size 2 and 2 at (0, 0) and (6, 3) | dx 4, dy 1 → (8 + 1) / 2 = 4 (< 5: "near the ghost") | §3.6, `missiles.md` §R9.5 |

## Provenance

- Read from the 1.14d `Game.exe` with `tools/ghidra/disasm.py` (`at`
  over the whole block `0x005B3130`–`0x005B6C58`, because most callbacks
  are reached only through pointers and are absent from
  `re/exports/functions.tsv`; `xref` for callers) and `re/exports/all.asm`
  (monster and superunique link switches, data-table reads). Register
  arguments read from the disassembly: states of `0x00544350`, statuses
  and iterate flags of `0x00544300`, chains of `0x00543640` /
  `0x005436B0`, flag slots and bits, timer periods, FX bytes, sound ids,
  0x5D bytes, free-spot and spawn arguments. Raw image bytes: the index,
  gem, rune and jump tables, the object operate / init tables, the
  switch byte tables `0x005B20A4` / `0x005B2060`, `0x005A46A0` /
  `0x005A4680`, `0x005A4EA4` / `0x005A4E7C`.
- Live 1.14d tables (extracted `patch_d2`): `objects.txt` (rows,
  OperateFn / InitFn, FrameCnt1 / 3), `monstats.txt` (hcIdx, BaseId,
  AI), `superuniques.txt` (hcIdx 36–38, 41), `levels.txt` (103–109),
  `misc.txt` / `weapons.txt` (`quest`, `questdiffcheck` of `mss`,
  `hfh`), `itemstatcost.txt` (stat 5 `newskills`).
- D2MOO 1.10f `D2Game/src/QUESTS/ACT4/A4Q0–A4Q4.cpp` gave names and
  structure only; each rule above was matched to the 1.14d function
  named next to it. Agreeing: the seal offsets and radii, the 10-firing
  spawn wait, the 75 / 90 / 95 s schedule, the gem tiers and rune
  tables, the 2 skill points. 1.14d differences: the player-GUID test of
  §3.3, the expansion messages 20000 / 20001 and bits 26.8 / 26.9, the
  level slot of §4.7.
- No packet or RNG recording of Act IV exists yet.
- 2026-10-07 answers to the implementation's questions (QD-*): read
  with `disasm.py fn` / `at` on `0x00559A30`, `0x0061DCA0`,
  `0x005A4850`, `0x005E73A0`, `0x005E7350`, `0x005DDFC0`, `0x00572DC0`,
  `0x006416D0`, `0x0061AED0`, `0x0061BAC0`, `0x0054FE10`, `0x005B4A80`,
  `0x0045E370`, `0x004B9210`, `0x00538680`, `0x005B4D20`, `0x005B4B60`,
  `0x005B4BE0`, `0x005B4140`, `0x005B69F0`, `0x005B5C10`, `0x005436B0`,
  `0x005435C0`; `all.asm` for the link sites in `0x005B1CF0`,
  `0x005A4440`, `0x005A49B0` and the loader `0x006552E0`; raw bytes of
  `0x0073C008`, `0x00731BC0`, `0x005A46A0`, `0x004B9284`; live
  `levels.txt`, `objects.txt`, `monumod.txt`, `superuniques.txt`.

## Open questions

1. Status meanings per Act IV quest (client quest log): settle with
   `quests.md` open question 1. **Answered** (2026-10-07): `world/quests-status.md`: the client (0x52 `0x0045CC00` → `0x004A40D0` stores the list; row build `0x004A1950`, tables `0x00723F30` and the per-quest status tables) maps each status to a description string id, a replay speech id and an icon state (§4, §5); Act IV tables §10; Fallen Angel and Hell's Forge exceptions §6.
2. ~~The classic end-of-game schedule reads `GetTickCount` (§5.8), an
   outcome driven by wall-clock time, against `sim/tick.md` §8.~~
   Recording R-PQ-12 (`docs/handoff/pc2-rec-pc2-quests.md`). Chosen
   until settled: `d2-sim` uses elapsed game time = 40 ms × frames since
   the kill; confirm with a recording (warp and end frames after a
   classic Diablo kill) and record the exception in `sim/tick.md`.
   **Needs recording** (re-read 2026-10-07: the three tests at
   `0x005B4C79` (+95000), `0x005B4CB0` (+90000), `0x005B4CEC` (+75000)
   are unsigned "t0 + delay < now" with `GetTickCount` through the
   import `0x006CC260`; there is no frame-based path, so only a
   recording can show the frame offsets). The recording must show, for
   one classic Diablo kill with game frames logged: the kill frame
   (0x89 FX 13 and `5D 17 02` to credited players), the frame of the
   `5D 17 01` / level warp to 103 of each credited player, the frame of
   the uncredited player's 0x50 (`50 17 00`), and the frame the server
   ends the game; ideally two runs (one on an idle machine, one under
   load) to show whether the offsets drift from 2250 / 2375 frames (90 /
   95 s at 40 ms) with wall-clock time. Settles the `sim/tick.md`
   exception wording.
3. Bytes 3–14 of the 0x50 sent to uncredited players (§5.8) and its
   client meaning: record one classic Diablo kill with an uncredited
   player. **Answered** (2026-10-07): the bytes are the caller-frame
   slots `ebp−0x11`…`ebp−0x06` of `0x005B4A80`, which only writes byte 0
   (`0x005B4B33`) and the u16 at 1 (`0x005B4B37`), so they hold whatever
   an earlier call left on the stack (not reproducible). The client
   handler `0x0045E370` → `0x004B9210` reads only the u16 at 1 (id 23 →
   case 4 at `0x004B924B`), so they have no client effect. Rule: Edge
   case 22 (d2-sim zeros, traces mask them). The client action of case
   23 is the client spec's.
4. `0x00559A30`'s `&level` argument (with `quests-act3.md` open question
   2): whether the item code reads 50 (gems, runes) or the uninitialised
   slot (hammer), and whether it writes back (Edge case 16).
   **Answered** (2026-10-07): neither. `0x00559A30` writes the level
   through the pointer before reading it (`0x00559AF8`) and uses that
   value (`0x00559C66`); the level comes from the dropping unit (§4.7).
   So `&level` is an output; a `drop_item_at` without it is exact when
   it derives the level from the dropping unit as §4.7 says.
5. `0x005A4850(…, 22, 1)` on Diablo, Izual and the seal bosses: what it
   sets (monster spec). **Answered** (2026-10-07): unique mark plus umod
   22 `questcomplete` in the umod list, no init function (§8, owner
   `monsters/init.md` §14.1 / §19; `0x005A4850`, table `0x0073C008`).
6. `0x005E7350` (the ghost's removal condition) and the remove call
   `0x005DDFC0` arguments: AI spec (`ai-functions.tsv` row 54 is
   unread). **Answered** (2026-10-07): §3.6 (`0x005E7350`: nobody talking
   → mode 0 at (0, 0), return 1; `0x005DDFC0(game, unit, mode, x, y)`;
   called with (0, 0, 0) twice). The rest of NpcStationary is the AI
   spec's (row 54).
7. `0x0061AED0(room, 0)` after a seal-boss dummy is created, and the
   dummy's own event-7 scheduling (object init 59): object spec.
   **Answered** (2026-10-07): §5.4 (room flag 0x400000 set, blocks room
   removal, `drlg/rooms.md` §8; init 59 `0x0054FE10`: event 7 at f + 27,
   event 1 at f + `FrameCnt1` + 1).
8. The data-tables u16 array at +0xAE0 (entries 36–38): confirm the
   loader fills it by `superuniques.txt` hcIdx (data spec).
   **Answered** (2026-10-07): yes, `0x006552E0` (§5.4; owner
   `data/loading.md` §8).
9. `0x00538680(client, 4, difficulty)` in the classic credit: save
   progression field (with `quests-act3.md` open question 5).
   **Answered** (2026-10-07): the rule is `quests-act1-rest.md` §5
   (client +0x0A bits 8–12 raised to 4 · difficulty + 4 for a classic
   character; never lowered). The call is classic-only (`0x005B4D3F`
   skips it in expansion games) and sits after 26.6 / 26.7 (`0x005B4D77`).
10. ~~`0x00530590` (game end) and `0x0052E2A0` (save pass) are host
    code: decide whether `d2-server` or `d2-sim` owns them.~~
    **Answered** (2026-10-07): `d2-server` owns both; `d2-sim` raises
    each as a host request at the 1.14d call point, in order, and goes
    on as if it returned, changing no state (rule: §5.8 "Host calls";
    also `quests-helpers.md` §6; the save pass's effect is
    `quests-act5-2.md` open question 3).
11. `quests-act3.md` §8.6 says level 104 is The Pandemonium Fortress;
    live `levels.txt` has 103 = The Pandemonium Fortress, 104 = Outer
    Steppes (`0x005BCBF0` compares 104). Settle in that spec. **Answered**
    (2026-10-07): `quests-act3.md` §8.6 already says Outer Steppes
    (`levels.txt` Act 4 - Mesa 1); re-read `cmp eax, 0x68` at
    `0x005BCC0B`.
12. `quests.tsv` column `spec` still says `catalogued` for rows 25–28
    and 30; switch it to `specified` (with a link to this file) once
    `quests.md` §2.4 documents owner files per act. No TSV rows were
    missing for Act IV. **Answered** (2026-10-07): rows 25–28 and 30 switched to
    `specified` (quests-fixups CODE-TABLE commit; every address of these
    rows is named in this file).
13. ~~Record a full Act IV run (packets + RNG, `docs/HANDOFF.md` §5):
    Izual and the ghost, the Hellforge drops, the seals and seal
    bosses, Diablo's spawn and death in classic and expansion, the
    portal.~~ Needs recording: R-PQ-11 (= HANDOFF §5 S9-A3 Act IV).
