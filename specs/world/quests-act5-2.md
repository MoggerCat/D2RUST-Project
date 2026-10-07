# Spec: World — Quests, Act V part 2 (Nihlathak, Ancients, Baal, intro, completion)

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (decompile exports plus disassembly for register arguments and the
  callbacks reached only through pointers; addresses inline). No Act V
  quest recording exists yet; nothing is verified against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act5`
- **Related specs:** `world/quests-act5.md` (part 1: conventions §1,
  records §2, A5Q1–A5Q3; its notation is used here unchanged);
  `world/quests.md` (shared machinery; §8.1 the act transitions, §8.2 the
  warp checks of chains 35 and 36, §8.4 the cow portal's 40.0 gate);
  `world/quests-act4.md` (Diablo's death and the Act IV → V travel);
  `world/quests.tsv` rows 34–36, 40; `world/quest-messages.tsv` chains
  34–36; `world/npc.md` §8.1 (Anya's personalize service gated by
  38.1, calling §6.11 here); `world/waypoints.md` (the waypoint record
  read in §6.8); `sim/rng.md` §3; `monsters/init.md`, `monsters/ai.md`
  and `monsters/ai-bodies*.md` (spawning, Baal's throne waves, the
  Ancients' AI), `missiles/missiles.md` (missiles 541 / 625),
  `sim/stats.md` (level-up), `formats/d2s.md` §2.3 (progression),
  `world/objects.md` (object modes).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 44–54 |
| Inputs | 55–64 |
| Outputs / state changes | 65–73 |
| Rules | 74–75 |
|   6. A5Q4 Betrayal of Harrogath (chain 34, slot 38) | 76–178 |
|   7. A5Q5 Rite of Passage (chain 35, slot 39) | 179–358 |
|   8. A5Q6 Eve of Destruction (chain 36, slot 40) | 359–488 |
|   9. Act V intro (chain 40, slot 42) | 489–515 |
|   10. Hooks called from other systems | 516–538 |
|   11. NPC services and game completion | 539–552 |
| Constants & data dependencies | 553–572 |
| Randomness | 573–584 |
| Edge cases & original bugs | 585–620 |
| Test vectors | 621–635 |
| Provenance | 636–658 |
| Open questions | 659–732 |
<!-- /index -->

## Summary

This part covers the last three Act V quests: Betrayal of Harrogath
(Nihlathak, chain 34, slot 38), Rite of Passage (the Ancients, chain 35,
slot 39) and Eve of Destruction (Baal, chain 36, slot 40); the Act V intro
record (chain 40); the hooks other systems call into Act V quest code;
and the game-completion bits. The quests give the personalize reward
(through Anya), an experience reward with a level gate, gold piles, the
Worldstone Chamber and the last portal, the zoo monster id (S→C 0x50) and
character progression for Act V.

## Inputs

| Name | Type | Source |
|---|---|---|
| events 0, 2, 3, 8, 10, 11, 13, 14 | quest callbacks | `quests.md` §4 |
| object init / operate / event-7 calls | init 63–65, 72, 73, 75–77, 79; operate 62–66, 69–72 | part 1 §1.4 |
| monster creation hooks; Nihlathak, Anya, Ancients and Baal AI hooks; town portal and player death hooks | calls | §10 |
| quest seed | control +0x18 | `quests.md` §2.1 |
| player waypoint record | player data +0x1C + 4·d | `world/waypoints.md` |

## Outputs / state changes

Player and game quest bits of slots 38–40; record state, status and flags
bytes; quest extra data; S→C 0x5D, 0x28, 0x27, 0x29, 0x89, 0x50, 0x61;
experience and levels added; gold piles dropped; monsters spawned (the
three Ancients, tyrael3) or removed; objects spawned (the Nihlathak
portal, object 561, the last portal 565); object modes; warps; character
progression for Act V (save spec).

## Rules

### 6. A5Q4 Betrayal of Harrogath (chain 34, slot 38)

#### 6.1 Bits and extra data

Bits: 38.2 started, 38.3 left town, 38.1 Nihlathak killed (personalize
pending), 38.4 told Anya (personalize offered). The kill credit needs
Prison of Ice done or pending (37.0 or 37.1). Extra: +0x00 GUID list;
+0x85 status timer exists; +0x86 Anya started (chat-end pending); +0x87
temple portal wanted; +0x88 temple portal made; +0x89 Anya to open a
portal (no Halls of Pain waypoint); +0x8C kill room.

#### 6.2 Flag iterate (`0x0058B0B0`)

Players with 37.0 or 37.1 and lacking 38.0 and 38.1: state 2 → 38.2;
state 3 → 38.3.

#### 6.3 Chat (event 0, `0x0058B2C0`; active fn `0x0058B870`)

Needs 37.0 or 37.1, else nothing. drehya (512) with state 1: lacking 38.1
and 38.0 → table state 0; else nothing. Other cases: 38.1 set → table
state 3 (38.4 clear) or 4 (38.4 set); GUID listed → 4; 38.0 → nothing;
(state < 4 or 38.13) and not-intro → index[state] (`0x007350E8`: −1, 0,
1, 2, 3, 4; −1 or ≥ 8 → nothing). Wants to talk: drehya, and in state 1
(37.0 or 37.1) with 38.0 and 38.1 clear; in other states 38.13 set and
38.4 clear.

#### 6.4 Anya's messages (event 11, `0x0058B1C0`, NPC 512 only)

- **20137**: not-intro → state := 2; +0x86 := 1; +0x87 := 1; flag
  iterate for all.
- **20148**: the current status byte is sent again to all (flags := 0);
  set 38.4; then 38.13 set, or intro with 38.1 set → state := 5 and the
  own seq fn.

Chat end (`0x0058B050`, never cleared): drehya: +0x86 = 1 → status 1 to
all, +0x86 := 0; +0x87 set and +0x88 clear → the temple portal (§6.7).

#### 6.5 Level changes, leave (events 3, 10)

- Event 3 (`0x0058BAB0`): old level 109 and state 2 → state := 3 (no
  player-bit test), flag iterate for all. New level 121 (Nihlathak's
  Temple) and status 1 → status 2 to all; +0x86 := 0.
- Event 10 (`0x0058B460`): remove the GUID from the record list and the
  extra list.

#### 6.6 Nihlathak's death (event 8, `0x0058B7A0`)

Reached through the chain-34 links of superunique 60 and base id 526
(§10); kills of superunique 60 are forced (`quests.md` §4.4). Needs
not-intro and a victim room: +0x8C := room; for each player `0x0058B4A0`
(lacking 38.14 and 38.0, in the kill room or adjacent, with 37.0 or
37.1, lacking 38.1 → set 38.1, 38.13); for each player from the victim:
`0x0058B680` (38.13 → party members `0x0058B5B0`: lacking 38.0, 38.14,
38.1, 38.13, in Act V, with 37.0 or 37.1 → 38.1, 38.13), completion flag
`0x0058B6D0` (lacking 38.0, 38.1, 38.13), `0x0058B730` (38.13 → sound
82). Game 38.13; FX 17; state := 4; +0x85 = 0 → +0x85 := 1, timer period
8 `0x0058B770` (status ≠ 4 → status 4 to all; +0x85 := 0; returns 1).

#### 6.7 The temple portal

`0x0058AF90` (unit U = Anya or dummy 459): +0x88 = 0 → portal object 60
to level 121 at (U.x + 10, U.y + 5) in U's room (`0x0056D130`, 0, 0x3C,
1); made → +0x88 := 1, +0x87 := 0, returns 1; else 0. Callers: chat end
(§6.4, U = Anya); dummy 459 event 7 (`0x0058B940`: +0x87 set → try; not
made → event 7 again at frame + 12; part 1 §5.9 schedules the first one);
Anya's AI (`0x0058BC80`, from `0x005E72B3`: +0x89 set and U in Act V →
the same portal at (U.x + 10, U.y + 5), made → +0x89 := 0).
`0x0058B900` (an event-7 rescheduler for +0x87) has no caller.

#### 6.8 Game start (event 13, `0x0058B990`)

Intro: state := 5 (direct write); the player's Halls of Pain (level 123)
waypoint (`0x00660E00`, `0x00660E50` on player data +0x1C + 4·d) is not
active → +0x89 := 1. Not-intro and 38.1 clear: 38.3 → +0x87 := 1, status
2 (to all, flags kept), state := 3; else 38.2 → +0x87 := 1, status 1,
state := 2. Both statuses are sent to all with the flags byte kept
(`0x0058BA3A`, `0x0058BA86`: `0x00544300` with iterate 1, no flags
write).

Status notation (re-read 2026-10-07): `0x00544300(record, n, arg, F,
iterate)` writes the status byte (+0x0B) and, with iterate 1, calls F
for every player; it never touches the flags byte (+0x14). "Status n"
without "to all" in an event-13 rule of Act V is a plain status-byte
write (e.g. `0x00587665`); "status n to all" clears the flags first
unless a rule says "flags kept"; the one Act V "(silent)" call (part 1
§4.4, 20110, `0x00587C1E`) clears the flags first too.

#### 6.9 Status function (`0x0058AF00`, always true)

out := 0. 38.0 → 0. 38.1 or 38.13 → 4 + (38.4). Else intro → 0; 38.14 →
12; state > 3 → 0; else the status byte.

#### 6.10 Sequence function (`0x0058B140`)

State < 4 and not-intro: state 0 → state := 1; return 1. Else seq(35).

#### 6.11 Personalize reward (`0x0058BC00`, from `world/npc.md` §8.1)

After Anya personalizes an item for a player with 38.1: set 38.0, clear
38.1 (38.15 is read, nothing done with it). Nothing is sent here.
`0x0058BC40` (from the Nihlathak AI at `0x005EE5F9`): not-intro and
status < 3 → status 3 to all.

### 7. A5Q5 Rite of Passage (chain 35, slot 39)

#### 7.1 Bits, gate and extra data

Gate G (player level, stat 12, ≥ 20 × (difficulty + 1): 20 / 40 / 60).
Bits: 39.2 started, 39.3 left town, 39.4 heard the invisible Ancient
(msg 20169), 39.5–39.9 heard the post-quest line of larzuk, cain6,
drehya, malah, qual-kehk (msgs 20167, 20165, 20166, 20168, 20164). There
is no reward-pending bit: the reward is given on the kill (39.0, 39.13).
Extra: +0x00 Ancients defeated; +0x01 Qual-Kehk started (chat-end
pending); +0x04 statue timer exists; +0x0C town portals open in level
120; +0x10 altar used; +0x11 fight armed; +0x14 / +0x18 / +0x1C GUIDs of
statues 476 / 474 / 475; +0x20 three Ancient GUIDs; +0x2C three
"Ancient spawned" bytes; +0x30 / +0x34 / +0x38 stored modes of statues
475 / 474 / 476; +0x3C / +0x3D / +0x3E statue respawn wanted for 476 /
474 / 475; +0x40 Ancients spawned, +0x44 alive; +0x48 fight started;
+0x4A completed before (door 547 open); +0x4C altar mode, +0x50 altar
GUID; +0x54 living-player count; +0x58 summit door seen, +0x5C its GUID,
+0x60 its mode.

#### 7.2 Flag iterate (`0x0058C210`)

Players lacking 39.0 and passing G, chain 35 present: state 2 → 39.2;
state 3 → 39.3.

#### 7.3 Chat (event 0, `0x0058C440`; active fn `0x0058CCF0`)

Needs not-intro. 39.0 clear: ancientstatue1–3 (537–539) with +0x10 = 0 →
table state 4 (msg 20002); then index[state] (`0x007357E4`: −1, 0, 1, 2,
3, 4, 0; −1 or > 8 → nothing). 39.0 set: by NPC the bit b = 39.5 larzuk
(511), 39.7 drehya (512), 39.8 malah (513), 39.9 qual-kehk (515), 39.6
cain6 (520); other NPCs nothing; b clear → table state 5 (first time,
menu 0); b set and 39.13 → 3. Wants to talk: qual-kehk with state 1 and
39.0 clear.

#### 7.4 Messages (event 11, `0x0058C290`)

| Msg | Effect |
|---|---|
| 20153 (qual-kehk only) | not-intro and state 1: state := 2; status ≠ 1 → status 1 to all; flag iterate; +0x01 := 1 |
| 20002 (any) | not-intro, state < 4 and status ≠ 3 → status 3 to all. Then, with no town portal open (+0x0C ≤ 0) and all three stored statue modes 0: arm the statues (§7.6); armed → +0x10 := 1, +0x11 := 1 |
| 20169 | set 39.4 |
| 20167 / 20165 / 20166 / 20168 / 20164 from larzuk / cain6 / drehya / malah / qual-kehk | set 39.5 / 39.6 / 39.7 / 39.8 / 39.9 (current record, `0x00543520`) |

Chat end (`0x0058BE80`): qual-kehk with +0x01 = 1 → status 1 to all, +0x01
:= 0.

#### 7.5 Level changes, game start, leave (events 3, 13, 10)

- Event 3 (`0x0058CBA0`): old level 109: quick remove; intro → stop;
  state 2 and the player lacks 39.0 → state := 3, status ≠ 1 → status 1
  to all, flag iterate. New level 120 (Arreat Summit): status < 2 and
  state > 2 → status 2 to all (b); state < 3 → state := 3 and flag
  iterate; else b → flag iterate. Then not defeated, door seen, door
  exists and door mode +0x60 = 0 → +0x60 := 2, door mode 1.
- Event 13 (`0x0058CDA0`): 39.0 or 39.15 → +0x4A := 1. Else 39.3 →
  status 1, state 3; 39.2 → status 1, state 2.
- Event 10 (`0x0058C750`): remove the GUID from the record list.

#### 7.6 The statues and the fight

- Arm (`0x0058BF40`): all three statue GUIDs must resolve (else nothing,
  returns 0); clear each statue room's portal flag; each statue mode 3,
  collision freed, object event 7 at frame + 20, stored mode 3; spawned
  := 0, alive := 0; returns 1.
- Statue event 7 (`0x0058C0E0`, classes 474–476): only when the statue
  is not in mode 4 and spawned < 3. 474 → superunique 45 (index 1), 475 →
  43 (index 0), 476 → 44 (index 2) (ids from datatables +0xB36..+0xB3A).
  A town portal open in level 120 (+0x0C > 0) → reset (below), stop.
  Spawn (`0x00545C30(game, statue, &position, 2, superunique)`: the
  preset class M + superunique (`0x00659B80(2, ·)`, M = monstats
  count), the room holding the position (`0x00620BB0` of the statue,
  then `0x00463740(room, x, y)`; none → no spawn, 0), then
  `0x0054E600(game, room, class, x, y, mode 1)`, the preset superunique
  path of `monsters/population.md` §11.2 / §11.4; `0x00545C30`–`0x00545C87`;
  also `quests-helpers.md` §3): none →
  event 7 again at frame + 10. Made → statue mode 4 (stored), Ancient
  slot [spawned] := 1 with its GUID, spawned += 1, alive += 1, +0x48 :=
  1.
- Kill (event 8, `0x0058C9A0`; Ancients reach it through their
  superunique link, §10; forced): needs +0x11, +0x10 and no town portal
  open. +0x04 = 0 → +0x04 := 1, timer period 2 `0x0058BD50`. Victim 540
  → +0x3E (statue 475), 541 → +0x3C (476), 542 → +0x3D (474); other
  victims stop. Missile 541 flies from the victim to its statue
  (`0x0058C8D0`, flags 0x420, level 1; `quests-helpers.md` §4.1; row 541 `ancient death center`:
server-do 1, the default flight of `missiles/missiles.md` R4). FX 18. alive −= 1;
  reaching 0: +0x00 := 1 (defeated); then with not-intro: a killing
  player lacking 39.0 and passing G → set 39.0, 39.13, experience reward
  (§7.7). For each player from the victim: `0x0058C7E0` (in level 120,
  passing G, lacking 39.0 → 39.13, 39.0, reward; its party members
  `0x0058C6D0` lacking 39.0, passing G, in Act V → 39.0, 39.13, reward),
  completion flag `0x0058C780` (lacking 39.0, 39.13). state := 5; object
  561 (the invisible Ancient) at the victim (type 2, flags 1, 0, 0);
  chain 36's seq fn; status ≠ 13 → status 13 to all. Nesting confirmed
  (`0x0058CA47`): everything after "+0x00 := 1" — the killer reward, both
  player passes, state 5, object 561, the chain-36 seq fn and status 13 —
  is skipped in an intro record.
- Statue timer (`0x0058BD50`, period 2): no town portal open and armed:
  each statue with its respawn byte set: exists → mode 1 with an
  end-animation event at frame + (`FrameCnt1` >> 8), byte := 0, stored
  mode 2; missing → retry (returns 0). Done (or portals / not armed) →
  +0x04 := 0, returns 1.
- Reset (`0x0058C000`): each spawned Ancient is removed (`0x0058BEC0`:
  state 54 → `0x005544B0`; in a room → mode 12, removed from the room,
  collision freed) and counted out (spawned, alive −1), or, gone, its
  inactive node (class 540 + slot) dropped; spawned bytes cleared;
  +0x48 := 0; respawn bytes 0, statues mode 0, stored modes 0; altar mode
  0 (+0x4C := 0).
- Town portal opened in level 120 (`0x0058CF00`, from `0x005BE389`,
  `0x005BE393`): not defeated → +0x0C += 1, +0x11 := 0, +0x10 := 0,
  reset. Closed (`0x0058CF50`, from `0x0053548D`, `0x005354D2`,
  `0x00584C1B`): +0x0C −= 1; reaching 0 with +0x10 → +0x11 := 1.
- Player died (`0x0058D560`, from `0x00535141`, `0x0053515A`,
  `0x0053572C`, `0x005359FC`): in level 120, not-intro, no portal open,
  not defeated, armed: count players in level 120 in neither mode 0
  (death) nor 0x11 (dead) (`0x0058D510`); none → +0x11 := 0, +0x10 := 0,
  reset.

#### 7.7 Experience reward (`0x0058C5C0`)

Send flags first. A = 1,400,000 (normal), 20,000,000 (nightmare),
40,000,000 (hell). L = level (stat 12), M = the class's maximum level
(`0x00611830`); L ≥ M → nothing. A := min(A, T(L + 1) − T(L)) (T =
`0x00611800`, experience thresholds), so at most one level's span. Loop
while A ≠ 0 and L < M: gap = stat 30 (next experience) − stat 13
(experience); A < gap → stat 13 += A, A := 0; else stat 13 := stat 30,
stat 29 := gap, level up (`0x00570880`), A −= gap, L re-read.
Every write is a set of the base stat (`0x00627260(player, stat, value,
0)`: stat 13 := stat 30, stat 29 := gap, or stat 13 := stat 13 + A);
stats 13 / 30 are read with the base getter `0x006253B0`, the level with
`0x00625480`; "A < gap" is unsigned (`0x0058C673`).

#### 7.8 Altar, doors and the invisible Ancient

- Altar init 72 (`0x0058D240`, object 546): +0x50 := GUID; mode := +0x4C.
- Altar operate 65 (`0x0058D310`): only in object mode 0. Intro: town
  portals open → close them (`0x0058D2C0`: each player's town portal
  (`0x005353F0`) in level 120 is closed, `0x00535430`, `quests-helpers.md` §7); scroll message
  20002 to the player. Not-intro and state < 4: the same; state < 2 →
  state := 2; status ≠ 3 → status 3 (the flags byte is not cleared).
  Not-intro with state ≥ 4: neither. Then altar mode 1, +0x4C := 2.
  The status 3 is sent to all (`0x0058D3CA`, iterate 1, flags kept).
  Returns 0.
- Statue operates 62–64: sound 19, return 0. Statue inits 63–65 store
  the GUID and set the stored mode for the class (`0x0058D0C0`).
- Door 547 init 73 (`0x0058D280`): +0x4A → mode 2, else 0. Operate 66
  (`0x0058D400`): the player lacks 39.0 and 39.1 → sound 19. Else (not
  defeated and (not-intro or +0x48)) → sound 19. Else mode 0 → mode 1
  with an end-animation event, room portal flag 0; mode 2 → warp
  (`0x0059D9D0`), room portal flag 1. Returns 0.
- Summit door 564 init 76 (`0x0058D640`): +0x58 := 1, +0x5C := GUID, mode
  := +0x60; not defeated and +0x60 ≠ 2 → +0x60 := 2, mode 1. Operate 71
  (`0x0058D6A0`): mode 0: not defeated → mode 1, +0x60 := 2; defeated →
  warp. Mode 1: not defeated → sound 19, mode 2; defeated → mode 0,
  +0x60 := 0. Mode 2: not defeated → sound 19 twice, mode 2; defeated →
  (fight started → mode 0, +0x60 := 0) then mode 0, +0x60 := 0. Other
  modes: nothing. Returns 0. The warp (mode 0, defeated; `0x0058D758`)
  is `0x0059D9D0`, the same as door 547's: for each unit of the door's
  room's unit list (+0x74, next +0xE8) of type 5 (warp tile),
  `0x005550B0(game, player, tile)` (`sim/path-placement.md` §12.2).
- Object 561 operate 69 (`0x0058D5E0`): 39.0 set and 39.4 clear → scroll
  message 20169.
  The client sends the C→S 0x13 that triggers operate 69 by itself
  (`ClientFn` 13: distance < 25, quest bit 39.0 set and 39.4 clear;
  `world/objects-client.md` §26.13).
- The warp check `0x0058D090` (`quests.md` §8.2) reads +0x00: leaving
  the summit for levels 118 or 128 is closed until the Ancients are
  defeated (in a not-intro record).

#### 7.9 Hooks with no caller

`0x0058CFB0` (portal count; called from `0x005EEAB1`), `0x0058CF90`
(armed = 0; from the Ancients' AI `0x005EEB83`, `0x005EEDB7`,
`0x005EF027`) are used; `0x0058CFE0`, `0x0058D000`, `0x0058D030` have no
caller; `0x0058CFD0` is a `ret` stub (part 1 edge case 7).

#### 7.10 Sequence function (`0x0058CD30`)

State ≠ 5 and not-intro: state 0 → 1; return 1. Else seq(36).

### 8. A5Q6 Eve of Destruction (chain 36, slot 40)

#### 8.1 Bits and extra data

Bits: 40.2 started, 40.3 left town, 40.4–40.9 heard the post-Baal line
of larzuk, cain6, malah, tyrael3, qual-kehk, drehya, 40.10 Baal done in
an earlier game or the last portal taken (game finished). Extra: +0x00
GUID list; +0x86 Worldstone Chamber open; +0x88 players credited; +0x8C
kill room; +0x90 quest started by the sequence; +0x94 Throne / Chamber
portal mode (1 at init); +0x98 last portal made; +0x9C last portal mode
(1 at init); +0xA0 zoo chosen, +0xA4 zoo monster id.

#### 8.2 Flag iterate (`0x0058D8B0`)

Players lacking 40.0 and 40.1, in Act V: state 2 → 40.2; state 3 →
40.3.

#### 8.3 Chat (event 0, `0x0058D9F0`; active fn `0x0058E2B0`)

1. ancientstatue1–3 (537–539): chain 35's fight started (+0x48), alive
   0, and (state 1 with +0x90 = 0, or state 2 with +0x90 set) → table
   state 3 (which has no statue entries; nothing is added).
2. 40.0 clear: state ≤ 3 → index[state] (`0x00735EE4`: −1, −1, −1, 0):
   only state 3 adds table state 0.
3. 40.0 set, by NPC: larzuk 40.4 clear → 2, else 40.13 → 3; drehya 40.9
   set → 3 when 40.13, else 2; malah 40.6, qual-kehk 40.8 as larzuk;
   cain6: 40.10 set → nothing; 40.5 clear → 4; else 40.13 → 5; tyrael3
   (521): 40.13 → 2. Table states 2 / 4 have menu 0 (first time), 3 / 5
   menu 2.

Wants to talk: the statues in the case of step 1 (state 1, +0x90 = 0);
else with 40.0 set: larzuk 40.4, drehya 40.9, malah 40.6, qual-kehk 40.8,
tyrael3 40.7 clear; cain6 with 40.10 clear and 40.5 clear.

#### 8.4 Messages, chat end (events 11, 2)

- Event 11 (`0x0058D940`, any NPC): 20178 / 20177 / 20179 / 20175 /
  20180 / 20176 → set 40.4 / 40.5 / 40.6 / 40.7 / 40.8 / 40.9 (jump
  table `0x0058D9D0`).
- Event 2 (`0x0058D870`): tyrael3 and +0x98 = 0: for each player
  `0x0058D7D0` (the first one in level 132: object 565 at a free spot
  near (x + 5, y), `0x00545340` size 5, mask 0x400, radius 18 (unused); returns 1
  and the walk stops); +0x98 := 1. Exact form of `0x0058D7D0`: needs the
  player's room with level 132; spot := player position + (5, 0),
  searched from the player's room with limit 100 (`0x0058D80A`); a spot
  found → object 565 (type 2, flags 1, 1, 0) there; it returns 1 for the
  first player in level 132 whether or not a spot or object was made, so
  +0x98 is set and nothing retries.

#### 8.5 Baal's death (event 8, `0x0058DF20`)

Baal (base 544, not class 709) gets the chain-36 link at creation (§10);
class 544 kills are forced. FX 19 (always).

1. Not-intro: +0x8C := victim room (none → the whole callback returns,
   step 2 included, `0x0058DF5C` → `0x0058E110`); status 4 to all (flags
   kept: no flags write, `0x0058DF72`). With a
   killing player: b = the killer lacks 40.0; +0x88 := 0; for each player
   from the victim `0x0058DEA0` (lacking 40.0 and 40.1, in level 132 →
   credit `0x0058DD30`: set 40.13, 40.0, character progression
   `0x00538680(client, 5, difficulty)`, +0x88 += 1), `0x0058DD90` (40.13
   → party members `0x0058DC60`: lacking 40.0, 40.1, in Act V → 40.0,
   40.13, progression; not counted), completion flag `0x0058DDE0`
   (lacking 40.0), `0x0058DE30` (40.13 → S5D(36, 0x02, 0), sound 83). b →
   +0x88 times: stub `0x00545990` (`ret 4`), gold pile of min + roll(max
   − min) at the victim (`0x0055B030`; min = 6000·d + 1500, max = 6000·d
   + 3000 capped at 0xFFFF, d = difficulty; `0x004BC500` on the quest
   seed). Then `0x0052E2A0(game)` (Open question 3). state := 5.
2. Always: missile 625 at the victim (`0x0056EDE0`, `quests-helpers.md` §4.2; row `baalfx
   control`: server-do 36, server-hit 57, `missiles/bodies-2.md`, which
   call Tyrael's spawn `0x0058E920` below); made
   → its room's portal flag cleared.

#### 8.6 Level changes (event 3, `0x0058E190`)

1. New level 109: send the zoo id to the player (`0x0058E120`: S→C 0x50,
   u8 0x50, u16 36, u16 zoo id at bytes 3–4; the rest of the 15 bytes is
   not written, as `quests.md` §9.4).
2. Old level 109: quick remove; state 2 and the player lacks 40.0 and
   40.1 → state := 3; status ≠ 1 → status 1 to all; flag iterate.
3. New level ≥ 131 (Throne of Destruction, the Chamber and the
   Pandemonium levels 133–136): not-intro and status < 2 → status 2 to
   all; flag iterate (always).

#### 8.7 Game start, join, leave (events 13, 14, 10)

- Event 13 (`0x0058E430`): 40.0 or 40.15 → set 40.10, character
  progression (5). Else 40.1 → nothing; 40.3 → status 1, state 3; 40.2 →
  +0x90 := 1, status 1, state 3.
- Event 14 (`0x0058E3F0`): 40.0 → set 40.10.
- Event 10 (`0x0058DCF0`): remove the GUID from both lists.

#### 8.8 Portals and the zoo

- Init 75 (`0x0058E670`, objects 563, 569): mode := +0x94; +0x94 := 2.
  Operate 70 (`0x0058E6A0`): the player in level 131 → warp to 132 entry
  11 (`0x0053AEC0`) only when +0x86 = 1; in any other level → warp to
  131 entry 0. Returns 0.
- Init 77 (`0x0058E710`, object 565): mode := +0x9C; +0x9C := 2. Operate
  72 (`0x0058E740`): +0x9C = 2: 40.13 clear → sound 19; set → warp to 109
  entry 0, `0x0052E2A0(game)`; the player's client exists and the player
  is not busy (`0x00535060`) → interaction reset (`0x00554190`), player
  data +0x4C := 1, S→C `61 07` (`0x0053D940`), set 40.10.
- Init 79 (`0x0058E830`, object 567): +0xA0 = 0: up to 10 tries of id =
  1 + roll(N − 1) on the quest seed (`0x004BC500`, N = monstats rows,
  datatables +0xA80); an id whose monstats flags byte +0x0E has bit 6
  set ends the tries. No hit → id 0. +0xA0 := 1; send 0x50 to every
  player (`0x0058E180`).
- Worldstone Chamber: `0x0058E600` (from the BaalToStairs AI, AI 138,
  `0x005EF620` at `0x005EF67B`: the crab is within aip1 of the portal
  object 563, `monsters/ai-bodies-5.md` §19): +0x86 := 1; not-intro and status < 3 → status 3 to all.
  The warp check `0x0058E640` (`quests.md` §8.2) is open only with +0x86
  = 1.
- Tyrael (`0x0058E920`, from `0x005AD952`, `0x005AD9AB`, `0x005B0A7F`):
  free spot near (x − 5, y − 5) of the given unit (size 5, mask 0x400,
  radius 19 (unused)) → monster 521 there (`0x005B2F20`, mode 1, 4, 0x42).
  Exact form: `0x0058E920(game, room, unit)`; spot := unit position −
  (5, 5), searched from the given room with limit 100 (`0x0058E940`);
  spot found → `0x005B2F20(game, found room, x, y, 521, mode 1, spread
  4, flags 0x42)`; flags 0x42 = 0x02 skip normal mods + 0x40 skip party
  minions (`monsters/init.md` §2). No spot → nothing. Caller
  `0x005AD952` passes the caller unit's room.

#### 8.9 Sequence function (`0x0058E390`)

Looks up chain 36 itself: not-intro and state 0 → +0x90 := 1, state := 2
(direct), status 1 to all, flag iterate. Returns 1. So Eve of
Destruction starts when the Ancients die (§7.6) or when the chain reaches
it at game start.

### 9. Act V intro (chain 40, slot 42)

Init `0x0058EA50`: event 0 `0x00586B50`, event 11 `0x0058E990`, table
`0x00732FF8`, active 1, state 0, status 0, no extra, filter 42, status
fn `0x00586C40` (false), active fn `0x00586C50`. The table (4 states of
0xC4 bytes up to chain 31's table; state 3 empty; every menu 0) is not
yet in `quest-messages.tsv` (Open question 8):

| State | drehya 512 | malah 513 | nihlathak 514 | qual-kehk 515 | cain6 520 |
|---|---|---|---|---|---|
| 0 | 20014 | 20037 | 20053 | 20065 | 20003 |
| 1 | 20014 | 20039 | 20054 | 20067 | 20003 |
| 2 | 20014 | 20038 | 20055 | 20066 | 20003 |

(slots 0–4 in the column order shown).

- Event 0: intro bit of the NPC clear (`0x005723C0`) → table state by the
  player's class: malah 513: barbarian (4) → 1, sorceress (1) → 2;
  nihlathak 514: assassin (6) → 1, necromancer (2) → 2; qual-kehk 515:
  druid (5) → 1, paladin (3) → 2; otherwise 0. drehya 512 and cain6 520
  always 0. Other NPCs nothing.
- Event 11 (jump on NPC − 512): 20014 from drehya, 20003 from cain6,
  20053–20055 from nihlathak, 20065–20067 from qual-kehk set the intro
  bit (`0x00572360`); 20037–20039 from malah set it and, when chain 31
  is not-intro with state 0, set its state to 1 (the Siege starts).
- Active fn: malah with its intro bit clear.

### 10. Hooks called from other systems

| Caller | Hook | Effect |
|---|---|---|
| superunique creation `0x005A4440` (jump `0x005A46A0` / `0x005A4680` on hcIdx − 6) | `0x005436B0` | 42 Siege Boss → unit state 118, chain 31; 43–45 Ancients → chain 35; 60 Nihlathak Boss → chain 34 (6 → chain 5, Act I) |
| preset superunique `0x005A49B0` (jump `0x005A4EA4` / `0x005A4E7C` on hcIdx − 6) | `0x005436B0` | 42 → minions, chain 31, `0x00545B50`, state 118; 43–45 → chain 35, `0x00545B50`; 60 → chain 34. `0x00545B50` jumps to the `ret` stub `0x0058CFD0` for levels ≥ 108 |
| monster creation, base-id switch `0x005B1CF0` | `0x005436B0` | 434 prisondoor → 32; 526 nihlathakboss → 34; 544 baalcrab → 36 unless class 709 (uberbaal) |
| barbarian spawn (part 1 §4.7) | `0x005436B0` | act5pow → 32 |
| prisoner AI `0x005EE3DB`–`0x005EE562` | part 1 §4.10 | rescue, counters, portal, status 5 |
| Shenk AI `0x005E27D0` | `0x00587900` | part 1 §3.8 |
| Anya AI `0x005E7806`–`0x005E7A49` | `0x0058A7D0`, `0x0058A820`, `0x0058A8D0`, `0x0058A940`, `0x0058A980`, `0x0058A9F0`, `0x0058AA10`; `0x0058BC80` | part 1 §5.8 portal coordinates and spawn, +0x84 tests, return to town (+0xAD := 1, +0x84 := 2, part 1 §5.9), status 2 (< 2); §6.7 |
| Nihlathak AI `0x005EE5F9` | `0x0058BC40` | §6.11 |
| Ancients' AI | `0x0058CF90`, `0x0058CFB0` | §7.9 |
| BaalToStairs AI 138 `0x005EF67B` (`monsters/ai-bodies-5.md` §19) | `0x0058E600` | §8.8 |
| map-AI store `0x00545CB3` / `0x00545CB9` / `0x00545CBF` | `0x00587950`, `0x0058AE10`, `0x0058AD80` | Larzuk, Nihlathak, Anya map AI (part 1 §3.8, §5.9) |
| town portal open / close, player death | `0x0058CF00`, `0x0058CF50`, `0x0058D560` | §7.6 |
| item use `tr2 `; player load `0x00539A1B` | `0x0058A0A0` | part 1 §5.7 |
| socket / personalize (`world/npc.md` §8.1) | `0x005877C0`, `0x0058BC00` | part 1 §3.9, §6.11 |
| object event 7 (`quests.md` §9.5) | part 1 §1.4 | classes 189, 459–461, 474–476 |
| warp checks (`quests.md` §8.2) | `0x0058D090`, `0x0058E640` | §7.8, §8.8 |
| missile 625 `baalfx control` bodies `0x005AD952`, `0x005AD9AB` (server-hit 57), `0x005B0A7F` (server-do 36), `missiles/bodies-2.md` §60 | `0x0058E920` | §8.8 |
| baalfx control missile (625) server-hit 57 `0x005AD970` (sites `0x005AD952`, `0x005AD9AB`) and server-do 36 `0x005B0A40` (site `0x005B0A7F`), each after the not-intro test `0x005444B0(game, 36)` (`quests.md` §2.3) | `0x0058E920` | §8.8; `missiles/bodies-2.md` |

### 11. NPC services and game completion

- Gated NPC services (owned by `world/npc.md`): Larzuk sockets while
  35.1 is set (§8.1 there, then part 1 §3.9); Anya personalizes while
  38.1 is set (then §6.11); Qual-Kehk's hire list needs 36.0 (§7 there;
  rows in `world/hirelings.md` §1).
- Game completion: slot 40 bit 0 (Baal killed with credit, §8.5) is the
  completion mark; its owner bits are set with character progression
  `0x00538680(client, 5, difficulty)` (save spec: Act V completion and
  the next difficulty). 40.10 marks the ending seen (last portal) or a
  completion from an earlier game. The expansion cow portal needs 40.0
  (`quests.md` §8.4). No act-completed slot exists for Act V; the Act IV
  → V travel sets 28.0 (`quests.md` §8.1).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| message-state index tables | chain 34 `0x007350E8`, 35 `0x007357E4`, 36 `0x00735EE4` | image |
| intro jump table | NPC − 512 → case, 9 entries | `0x0058EA2C` |
| A5Q5 NPC jump table | NPC − 511 → case, 10 entries | `0x0058C414` |
| A5Q6 message jump table | msg − 20175 → case, 6 entries | `0x0058D9D0` |
| levels | 118 Ancients' Way, 120 Arreat Summit, 121 Nihlathak's Temple, 123 Halls of Pain, 128 Worldstone Keep 1, 131 Throne of Destruction, 132 Worldstone Chamber, 133–136 Pandemonium | `levels.txt` |
| NPCs / monsters | 511 larzuk, 512 drehya, 513 malah, 514 nihlathak, 515 qual-kehk, 520 cain6, 521 tyrael3, 526 nihlathakboss, 537–539 ancientstatue1–3, 540–542 ancientbarb1–3, 544 baalcrab, 709 uberbaal | `monstats.txt` |
| superuniques | 43–45 Ancient Barbarian 1–3, 60 Nihlathak Boss | `superuniques.txt` |
| objects | 60 (temple portal), 546, 547, 561, 563–565, 567, 569 | `objects.txt` |
| sounds | 19 refused, 82 Nihlathak, 83 Baal | §6.6, §8.5 |
| FX bytes | 17 Nihlathak, 18 each Ancient, 19 Baal | §6.6, §7.6, §8.5 |
| timer periods | 8 (Nihlathak status), 2 (statues) | §6.6, §7.6 |
| object event delays | 12 dummy 459, 20 statues armed, 10 spawn retry | §6.7, §7.6 |
| Ancients reward | 1,400,000 / 20,000,000 / 40,000,000, capped at one level; gate 20 / 40 / 60 | §7.7 |
| Baal gold | [6000·d + 1500, 6000·d + 3000) per credited player | §8.5 |
| missiles | 541 (Ancient to statue), 625 (Baal) | §7.6, §8.5 |

## Randomness

Quest seed (control +0x18) draws, in occurrence order of their events:

| When | Seed | Draws | Decides |
|---|---|---|---|
| zoo object init (once per game) | quest | 1–10 (`1 + roll(N − 1)` each, stop on a zoo-flagged id) | zoo monster id |
| Baal's death, killer lacking 40.0 | quest | one `roll(max − min)` per credited player, each followed by that gold pile's own drop | pile sizes |

Spawned Ancients, Tyrael and the missiles draw from their own code
(monster, missile specs). No other Act V part-2 quest code draws.

## Edge cases & original bugs

1. Betrayal of Harrogath credits only players with Prison of Ice done or
   pending (37.0 or 37.1), in the kill room or adjacent, or their party
   members in Act V (§6.6).
2. Anya's 20148 re-broadcasts the current status byte instead of a new
   value (§6.4).
3. Leaving Harrogath in state 2 moves Betrayal to state 3 without testing
   the player's bits (§6.5).
4. Rite of Passage party members anywhere in Act V get the reward, not
   only those on the summit (§7.6); the killer is rewarded first even if
   outside level 120.
5. Opening a town portal on the summit, or every player there dying,
   removes the Ancients and resets the statues (§7.6); the altar must be
   used again.
6. The altar's status-3 send in the not-intro path keeps the flags byte
   (§7.8).
7. Eve of Destruction's statue chat (§8.3 step 1) selects a table state
   with no statue entries: nothing is added.
8. Baal's credits and gold need a killing player (kill parse,
   `quests.md` §4.4); the gold count includes only players in the
   Chamber, not party members credited elsewhere (§8.5).
9. Entering any level ≥ 131, including the Pandemonium levels, sends
   Baal status 2 (§8.6).
10. `0x0058B900`, `0x0058CFE0`, `0x0058D000`, `0x0058D030` have no
    caller in 1.14d.
11. Baal killed with no victim room in a not-intro game: FX 19 only, no
    credit and no missile 625 (§8.5); in an intro game the missile is
    still made.
12. Baal's status 4 and Betrayal's event-13 statuses keep the flags byte
    (§8.5, §6.8).
13. The last portal is tried once: the first player in level 132 ends
    the walk and sets +0x98 even when no free spot was found (§8.4).
14. The free-spot "radius" arguments of §8.4 / §8.8 are unused (part 1
    §1.1).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Nihlathak status fn, 38.1 + 38.4 | 5 | §6.9 |
| Nihlathak killed, player P in the next room with 37.0, lacking 38.* | P: 38.1, 38.13; sound 82; game 38.13; FX 17; timer 8 | §6.6 |
| intro game start, no Halls of Pain waypoint | chain 34 state 5; +0x89 = 1 | §6.8 |
| Ancients reward, normal, level 25, T(26) − T(25) = 40,000 (synthetic) | A = 40,000 → exactly one level | §7.7 |
| Ancients reward, hell, level 60, A capped at T(61) − T(60) | one level at most | §7.7 |
| level 39 player in nightmare kills the last Ancient | no reward (gate 40) | §7.1 |
| Baal, hell (d = 2), 2 credited players, quest-seed rolls r1, r2 | piles 13500 + r1 mod 1500, 13500 + r2 mod 1500 | §8.5 |
| msg 20179 | 40.6 set | §8.4 |
| malah 20038 with chain 31 not-intro, state 0 | malah intro bit; chain 31 state 1 | §9 |
| intro event 0, nihlathak, assassin, intro bit clear | table state 1 (msg 20054) | §9 |

## Provenance

- Read from the 1.14d `Game.exe` exports and `tools/ghidra/disasm.py`
  (`fn`, `at`, `xref`), as part 1; raw image bytes for the intro and
  jump tables, the index tables and the link jump tables. Assert
  strings `d:\diablo2\…\Quests\a5q4.cpp` … `a5q6.cpp` identify the files.
  Live 1.14d `levels.txt`, `objects.txt`, `monstats.txt`,
  `superuniques.txt` gave row names.
- D2MOO 1.10f `D2Game/src/QUESTS/ACT5/A5Q4–A5Q6.cpp` and `A5Intro.cpp`
  gave names and structure; each rule was matched to the 1.14d function
  named next to it. Checked to agree: the Prison-of-Ice gate on
  Nihlathak's credit, the 20 / 40 / 60 gate, the experience amounts and
  one-level cap, the Baal gold range, the zoo draw. 1.14d additions:
  39.4–39.9 set by the post-quest messages (D2MOO returns), the 20169
  bit, 40.4–40.9 via a jump table.
- No packet or RNG recording of Act V exists yet.
- 2026-10-07 answers (QE-5–QE-8): `disasm.py at` on `0x0058D7D0`,
  `0x0058E920`, `0x0058D6A0`, `0x0059D9D0`, `0x0058B990`, `0x0058C5C0`,
  `0x0058C9A0`, `0x0058DF20`–`0x0058E116`, `0x0058EA50`, `0x00544300`;
  raw dump of the table `0x00732FF8`; a scan of every `0x00544300` call
  in `0x00586B50`–`0x0058EB00` for the iterate argument and a preceding
  flags write.

## Open questions

1. Status meanings per quest (client quest log): `quests.md` open
   question 1. **Answered** (2026-10-07): `world/quests-status.md`: the client (0x52 `0x0045CC00` → `0x004A40D0` stores the list; row build `0x004A1950`, tables `0x00723F30` and the per-quest status tables) maps each status to a description string id, a replay speech id and an icon state (§4, §5); Act V tables §11 (Rite of Passage, Eve of Destruction).
2. `0x00538680(client, 5, difficulty)`: which save progression field
   (save spec; with `quests-act3.md` open question 5). **Answered**
   (2026-10-07): `quests-act1-rest.md` §5 (client +0x0A bits 8–12 raised
   to 5 · difficulty + 5; never lowered).
3. `0x0052E2A0(game)`, called after Baal's credits and by the last
   portal: owner and effect (game / save spec). **Answered** (2026-10-07,
   effect): only in game types 1 and 2 (game +0x6A; any other type →
   nothing): for each client of the game's client list (game +0x88, next
   +0x4A8) with a player (`0x00537860(client, 0)`): save its character
   (`0x00532400(game, player, 0x00538830(client, 0))`, the save of
   `sim/tick.md` §3 step 3), then, while the client has a pending save
   download (client +0x3D4 bit 0x10 and buffer +0x17C, `0x00538FC0`),
   call the 0xB3 DownloadSave sender `0x0052E110` (`sim/intents-events.md`
   §3.2 step 5). No game state changes and no draw: it is host save /
   transport code (owner: the host side with `sim/tick.md` §8); `d2-sim`
   only raises it as a host call. Owner decided (2026-10-07):
   `d2-server` (`quests-helpers.md` §6; `quests-act4.md` §5.8 "Host calls").
4. Monstats flags byte +0x0E bit 6 (zoo eligibility): name the column
   (monster spec). **Answered** (2026-10-07): the `zoo` column: `data/fields.tsv`
   lays it out as `bit(22)` of the flags dword at offset 12, i.e. byte
   +0x0E mask 0x40; `0x0058E8B8` reads that byte and ANDs it with the
   mask table entry `0x006CE280` (= 0x40, read from the image).
5. ~~The Baal throne AI condition that calls `0x0058E600` and the callers
   of `0x0058E920`: AI / monster specs.~~ **Answered** (2026-10-07): the
   caller of `0x0058E600` is BaalToStairs (AI 138, `0x005EF620`), not the
   throne: when the crab is closer than aip1 to the Worldstone Chamber
   portal object 563 (`monsters/ai-bodies-5.md` §19 step 2). `0x0058E920`
   is called only by missile 625's (`baalfx control`) bodies
   (`missiles/bodies-2.md` §60: server-do 36 at frames left ≤ 100,
   server-hit 57 with no unit; §10 hook table; `0x005AD970`,
   `0x005B0A40`), gated by `0x005444B0(game, 36)`.
6. ~~`0x00545C30(position, 2, superunique)` (Ancient spawn) and missile
   541 / 625 parameters: monster and missile specs.~~ **Answered**
   (2026-10-07): §7.6 statue event 7 (`0x00545C30`–`0x00545C87`: preset
   superunique spawn through `monsters/population.md` §11.4) and
   `quests-helpers.md` §3 (class = id + monstats rows, room lookup at the
   point, `0x0054E600` mode 1); missile 541 = `ancient death center`
   (server-do 1; `quests-helpers.md` §4.1: flags 0x420, owner = origin =
   the victim, target = the statue position, level 1, data +0x28 :=
   statue GUID), 625 = `baalfx control` (server-do 36, server-hit 57;
   `quests-helpers.md` §4.2: `0x0056EDE0` skill 0, level 1 at the
   victim's position); their behaviour is the missile specs' (§7.6, §8.5).
7. ~~Record an Act V run: Nihlathak's kill and portal, the Ancients (with
   a town portal reset), Baal's gold draws and the zoo id 0x50.~~ Needs
   recording: R-PQ-13 (= HANDOFF §5 S9-A3 Act V).
8. `quests.tsv` row 40 still has `?` cells; its values are §9's (filter
   42, callbacks `0:0x00586B50 11:0x0058E990`, status fn `0x00586C40`,
   active fn `0x00586C50`, msgs `0x00732FF8`). The 15 table rows of §9
   were not appended to `quest-messages.tsv`: `d2-sim`
   `world::quests::tests::tables_parse_and_check` asserts 779 rows and
   that every message table is referenced by a row, so the rows, the row
   40 cells and that test must change together in one implementation
   commit (rows: `0x00732FF8 40 40 <state> <slot> <npc> <string> 0`).
   **Answered** (2026-10-07, TSV change handed to the TSV owner): init
   `0x0058EA50` re-read (callbacks +0xA0 / +0xCC, table +0xDC, filter
   +0xE0 = 42, status fn +0xE8, active fn +0xEC, no seq fn, active 1)
   and the table dumped (states 0–2: 5 entries each as §9, menus 0;
   state 3 count 0; state 4 = chain 31's table `0x00733308`). The exact
   row-40 cells and 15 rows are in `docs/handoff/pc2-spec-quests-act4-5.md`
   "Cross-file requests" for PC 2 quests-core (`quests.tsv`,
   `quest-messages.tsv` and the 779 → 794 count change together).
   Done (2026-10-07): quests-core commits `b242ee7` (row 40) and
   `39dabf1` (15 rows, right after the header, before the first
   `0x00733308` row); re-checked by quests-fixups against the image:
   all 15 values equal `0x00732FF8`'s entries (4 states of 0xC4 bytes up
   to `0x00733308`, counts 5, 5, 5, 0). Row order matters only within one
   table: `d2-sim` filters by (table, state, NPC) and keeps file order,
   so slot order inside a table is what must hold, not the table's place
   in the file.
