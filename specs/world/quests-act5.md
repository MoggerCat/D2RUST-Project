# Spec: World — Quests, Act V part 1 (Siege, Rescue on Mount Arreat, Prison of Ice)

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (decompile exports plus disassembly for register arguments and the
  callbacks reached only through pointers; addresses inline). No Act V
  quest recording exists yet; nothing is verified against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act5`
- **Related specs:** `world/quests.md` (owner of the shared machinery:
  flag records §1, quest records §2, game entry §3, events and dispatch
  §4, updater and timers §5, status messages §6, NPC dialog hooks §7,
  act transitions and warp checks §8, quest items and helpers §9; this
  spec uses its terms and never restates them); `world/quests-act5-2.md`
  (part 2: Betrayal of Harrogath, Rite of Passage, Eve of Destruction,
  the Act V intro record, game completion, the hook table for all of
  Act V); `world/quests-act3.md` and `world/quests-act2.md` (same layout
  and notation); `world/quests-act4.md` (Act IV, Diablo's death and the
  tyrael2 travel that opens Act V, `quests.md` §8.1); `world/quests.tsv`
  (records and callbacks: rows 31–36, 40), `world/quest-messages.tsv`
  (NPC message tables of chains 31–36; the intro's table is in part 2
  §9); `world/npc.md` §8.1 (Larzuk's socket service gated by 35.1,
  calling §3.9 here), §7 (Qual-Kehk's
  hire list gated by 36.0); `world/hirelings.md` (the act 5 hireling rows
  Qual-Kehk sells); `sim/rng.md` §3; `sim/tick.md` §5 (object events);
  `sim/unit-order.md` §7 (player iteration order); item, object, monster,
  AI and save specs (not written: item creation, object modes, monster
  spawning, NPC map AI, the barbarian prisoner and Anya AI that call the
  hooks of part 2 §10, save progression).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–64 |
| Inputs | 65–75 |
| Outputs / state changes | 76–84 |
| Rules | 85–86 |
|   1. Conventions | 87–169 |
|   2. Act V records | 170–197 |
|   3. A5Q1 Siege on Harrogath (chain 31, slot 35) | 198–284 |
|   4. A5Q2 Rescue on Mount Arreat (chain 32, slot 36) | 285–409 |
|   5. A5Q3 Prison of Ice (chain 33, slot 37) | 410–644 |
| Constants & data dependencies | 645–662 |
| Randomness | 663–672 |
| Edge cases & original bugs | 673–707 |
| Test vectors | 708–722 |
| Provenance | 723–747 |
| Open questions | 748–776 |
<!-- /index -->

## Summary

Act V (expansion content) has six main quests (chains 31–36, flag slots
35–40) and one intro record (chain 40, slot 42). This part covers the
first three: Siege on Harrogath (Shenk), Rescue on Mount Arreat (the
caged barbarians) and Prison of Ice (Anya). Each is a quest record
(`quests.md` §2.2) whose callbacks react to NPC chat, scroll messages
(C→S 0x31), level changes, kills, player join / leave, quest objects
(Larzuk's start dummy, the barbarian cages and their rescue portals,
Anya's frozen statue and her portals, the town dummies of Anya and
Nihlathak) and hooks from monster creation and the prisoner / Anya AI.
They set player and game flag bits, give items (runes, Malah's thawing
potion, the Scroll of Resistance, Anya's class item), spawn the
barbarians, Larzuk, Anya and Nihlathak, and report status with S→C 0x5D.

## Inputs

| Name | Type | Source |
|---|---|---|
| events 0, 2, 3, 8, 10, 11, 13, 14 | quest callbacks | `quests.md` §4 |
| object init / operate / event-7 calls | init 62, 66–71, 74; operate 67 | object spec; tables `0x00731BC0`, `0x00732D18` (§1.4) |
| monster creation hooks; prisoner / Anya AI hooks | calls | part 2 §10 |
| item use of `tr2 ` | call | item-use spec (§5.7) |
| quest seed | control +0x18 | `quests.md` §2.1 |
| `misc.txt` codes `r07`, `r08`, `r09`, `ice `, `tr2 `; `weapons.txt` / `armor.txt` codes of Anya's class items (§5.7) | tables | §4, §5 |

## Outputs / state changes

Player and game quest bits of slots 35–38; record state, status and
flags bytes; quest extra data; S→C 0x5D, 0x28, 0x27 (text refresh), 0x29,
0x89; items given (`r07`–`r09`, `ice `, `tr2 `, a rare class item) or
deleted (`ice `); monsters spawned (larzuk, act5pow, drehya, drehyaiced,
nihlathak) or removed; objects spawned (rescue portals 189, the frozen
Anya object 558); object modes; a resistance stat list on the player.

## Rules

### 1. Conventions

#### 1.1 Notation

As `world/quests-act3.md` §1.1, with these Act V bindings: "in Act V" =
the player's room's level has act 4 (`0x006427F0`); "quick remove" =
`0x00545310` (leaving Harrogath, level 109); "status n to all" = flags :=
0, then `0x00544300(record, n, 0, F, 1)` (F = the quest's iterate
function, §2); "status n (silent)" = same with iterate 0; "give `code`" =
`0x005466B0(game, player, code, 0, 2, 1)` (`quests.md` §9.1: default
level, quality 2, droppable) unless stated; "delete `code`" = `quests.md`
§9.2; "holds `code`" = `0x00558110`; "completion flag" = for each player
(from the unit named) lacking the listed bits: set s.14 and
`0x00545920(player, chain, 0)` (`5D <chain> 00 0C 0000`); "sound n" =
`0x00553380`; "FX b" = `quests.md` §6.5; "refresh" = `quests.md` §7.2;
"send flags" = `quests.md` §6.6. "S5D(c, f, v)" = a 6-byte S→C 0x5D
`5D c f 00 v` (u16 v, normally 0) sent to that player only
(`0x005531C0` client of the player, `0x0053D710`); its status byte is 0
and the record's status byte is not touched. "Critical spawn of class C at
(x, y)" = `0x005459A0(game, x, y, room, 1, C)` (monster spec). "Kill in
place" = the unit's interaction is ended, it is put in mode 12 (dead) and
removed (monster spec; `0x005A7E60`, `0x005A7C20`, `0x0061A270`,
`0x00623830`, `0x0064C370`). "Drop inactive node of C" =
`0x00543140(C, 4)` (the act-V inactive-unit record of class C is
deleted; unit spec).

"Radius" of a free-spot search: `0x00545340`'s sixth argument (stack
+0x14) is never read in 1.14d (body `0x00545340`–`0x0054547F`, `ret 0x14`); the ring
search is bounded by the seventh argument (limit) only. Every "radius n"
below is recorded for completeness and has no effect.

#### 1.2 Message-list selection (event 0)

As `world/quests-act2.md` §1.2: event-0 callbacks add a table state of the
record's table with `0x00543790(npc, table state)`; table states are
`quest-messages.tsv` row states of the chain. Index tables map record
state to table state (§Constants).

#### 1.3 Sequence chain

Root: chain 31 (`quests.md` §3 step 3). "Call seq(c)" = look up chain c
and call that record's sequence function (+0xF0 checked with
`IsBadCodePtr`, fatal when bad).

| Chain | Seq fn | Rule |
|---|---|---|
| 31 Siege | `0x00587560` | state ≠ 5 and not-intro → 1 (no state change); else seq(32) (0 for an absent chain) |
| 32 Rescue | `0x005883E0` | state ≠ 5 and not-intro: state 0 → state := 1 (direct write); return 1. Else seq(33) |
| 33 Prison of Ice | `0x00589160` | state < 5 and not-intro: state 0 → 1; return 1. Else seq(34) |
| 34 Nihlathak | `0x0058B140` | part 2 §6.10 |
| 35 Ancients | `0x0058CD30` | part 2 §7.10 |
| 36 Baal | `0x0058E390` | part 2 §8.9 |

So Rescue on Mount Arreat starts (state 1) only when the Siege is
finished in this game or switched off; the Siege itself starts from the
Malah intro (part 2 §9) or game start (§3.7).

#### 1.4 Act V quest objects (objects.txt row, init / operate number)

| Object | Init fn | Operate fn |
|---|---|---|
| 473 `cagedwussie1` | 62 `0x005886A0` (§4.7) | — |
| 474 / 475 / 476 Ancient Statue 3 / 1 / 2 | 63 `0x0058D150`, 64 `0x0058D190`, 65 `0x0058D110` (part 2 §7.8) | 62 `0x0058D1E0`, 63 `0x0058D200`, 64 `0x0058D220` (sound 19 only) |
| 459 dummy (Anya in town) | 66 `0x0058EAC0` (§5.9) | — |
| 460 dummy (Anya outside town) | 67 `0x0058A5B0` (§5.6) | — |
| 461 dummy (Nihlathak in town) | 68 `0x0058A610` (§5.9) | — |
| 462 dummy (Nihlathak in his temple) | 69 `0x0058A6C0` (§5.9) | — |
| 542 / 543 Dummy (Larzuk start) | 70 `0x00587830` (`ret`) / 71 `0x00587840` (§3.8) | — |
| 546 `ancientsaltar` | 72 `0x0058D240` | 65 `0x0058D310` (part 2 §7.8) |
| 547 To The Worldstone Keep Level 1 | 73 `0x0058D280` | 66 `0x0058D400` (part 2 §7.8) |
| 558 `fana` (frozen Anya) | 74 `0x0058AA50` (§5.6) | 67 `0x0058ABC0` (§5.6) |
| 561 Dummy (invisible Ancient) | — | 69 `0x0058D5E0` (part 2 §7.8) |
| 563 / 569 Worldstone Chamber / Throne of Destruction portal | 75 `0x0058E670` | 70 `0x0058E6A0` (part 2 §8.8) |
| 564 (summit door) | 76 `0x0058D640` | 71 `0x0058D6A0` (part 2 §7.8) |
| 565 `strlastcinematic` (last portal) | 77 `0x0058E710` | 72 `0x0058E740` (part 2 §8.8) |
| 567 Zoo | 79 `0x0058E830` (part 2 §8.8) | — |

Object event 7 (`quests.md` §9.5) reaches Act V code for class 189 (the
rescue / Anya portals: level 109 or ≥ 113 → chain 33 `0x0058A730` §5.8,
other levels → chain 32 `0x00588CA0` §4.8), 459 (`0x0058B940`, part 2
§6.7), 460 (`0x0058A500`, §5.6), 461 (`0x00589540`, §5.9) and 474–476
(chain 35 `0x0058C0E0`, part 2 §7.6).

### 2. Act V records

Callback addresses are in `quests.tsv`; this table adds what the init
functions store beyond it.

| Chain / slot | Quest | Init | State, init_no, seq_id | Extra (bytes, zeroed) | F | Table |
|---|---|---|---|---|---|---|
| 31 / 35 | A5Q1 Siege on Harrogath | `0x005876E0` | 0, 4, 32 | 0x18 | `0x00586C80` | `0x00733308` |
| 32 / 36 | A5Q2 Rescue on Mount Arreat | `0x00588510` | 0, 4, 33 | 0x160; list +0x00 reset | `0x005879D0` | `0x00733940` |
| 33 / 37 | A5Q3 Prison of Ice | `0x0058A410` | 0, 5, 34 | 0x114; list +0x00 reset | `0x00588EB0` | `0x00734320` |
| 34 / 38 | A5Q4 Betrayal of Harrogath | `0x0058BB20` | 0, 4, 35 | 0x90; list +0x00 reset | `0x0058AEA0` | `0x00734AC8` |
| 35 / 39 | A5Q5 Rite of Passage | `0x0058CE30` | 0, 4, 36 | 0x64 | `0x0058BE20` | `0x00735100` |
| 36 / 40 | A5Q6 Eve of Destruction | `0x0058E500` | 0, 4, 37; +0x94 := 1, +0x9C := 1 | 0xA8; list +0x00 reset | `0x0058D770` | `0x00735800` |
| 40 / 42 | Act V intro | `0x0058EA50` | 0, —, — | none | — | `0x00732FF8` |

All are active = 1. Every F sends 0x5D for its chain to a player when
(s.0 clear and s.15 clear) or s.13 or s.14 (the Act II rule). Status
functions: chain 31 `0x00586CE0` (§3.10), 33 `0x0058A300` (§5.11), 34
`0x0058AF00` (part 2 §6.9); chains 32, 35, 36 use the default rule
(`quests.md` §6.1); the intro's returns false. The init table version
of all Act V rows is 0 (`quests.md` §2.4). The records exist in classic
games too, and their event-13 and sequence functions run in every game
(`quests.md` §3); events dispatched by act (`quests.md` §4.2) need a
player in Act V, which needs an expansion game (`quests.md` §8.1,
tyrael2 requires game +0x70). Difficulty acts through
the record used, item and monster data and the explicit tiers of §5.7
and part 2 §7.7, §8.5.

### 3. A5Q1 Siege on Harrogath (chain 31, slot 35)

#### 3.1 Bits and extra data

Bits: 35.2 started, 35.3 left town, 35.4 left town after status 2 (Shenk
seen), 35.1 Shenk killed (socket pending), 35.5 told Larzuk (socket
offered). Extra: +0x00 Larzuk map AI; +0x04 kill room; +0x08 Larzuk
started (chat-end pending); +0x10 Larzuk GUID; +0x15 Larzuk spawned;
+0x16 reward talk (chat-end pending); +0x17 map AI applied.

#### 3.2 Flag iterate (`0x00586DE0`)

Players lacking 35.0 and 35.1, chain 31 present: state 2 → 35.2; state 3
→ 35.4 when status is 2, else 35.3.

#### 3.3 Chat (event 0, `0x00586FF0`; active fn `0x005874E0`)

35.1 set: 35.5 clear → table state 3, else nothing. 35.1 clear: 35.0 set
→ nothing; else (state < 4 or 35.13) and not-intro → index[state]
(`0x00733928`: −1, 0, 1, 2, 3, 4; −1 or ≥ 8 → nothing). Wants to talk:
larzuk (511) and either 35.1 set with 35.5 clear, or not-intro, state 1,
35.0 and 35.1 clear.

#### 3.4 Larzuk's messages (event 11, `0x00586E70`, NPC 511 only)

- **20077**: +0x08 := 1; state := 2; flag iterate for all; refresh.
- **20090** (only with 35.1 set): +0x16 := 1; reset_progress(35) (bits
  2–11); set 35.5; 35.13 set and state ≠ 5 → state := 5, the own seq fn,
  status 13 to all. Add GUID. No refresh.

Chat end (`0x00586D80`, never cleared): larzuk: +0x08 = 1 → status 1 to
all, +0x08 := 0; +0x16 = 1 → status 4 to all, +0x16 := 0.

#### 3.5 Level changes (event 3, `0x005873E0`)

New level 110–112 (Bloody Foothills, Frigid Highlands, Arreat Plateau),
not-intro and state 1 or 2 → state := 3, flag iterate for all. Otherwise,
old level 109: quick remove; state 2 and the player lacks 35.0 and 35.1 →
state := 3; status 0 → status 1 to all; flag iterate for all.

#### 3.6 Shenk's death (event 8, `0x00587330`)

Reached only through the chain-31 link of superunique 42 (Siege Boss,
part 2 §10; its kills are forced, `quests.md` §4.4); the victim is not
re-tested. Needs not-intro and a victim room: +0x04 := room; for each
player `0x00587100` (lacking 35.0, in the kill room or one adjacent to it
(`0x00619790`) → set 35.1, 35.13); for each player from the victim:
`0x00587240` (35.13 → party members `0x005871C0`: lacking 35.0, 35.1,
35.13 and in Act V → 35.1, 35.13), completion flag `0x00587290` (lacking
35.0, 35.1, 35.13), `0x005872F0` (35.13 → sound 80). Game 35.13; FX 15;
status < 3 → status 3 to all.

#### 3.7 Game start, leave (events 13, 10)

- Event 13 (`0x005875F0`): 35.0, 35.15 or 35.1 → status byte := 5 (state
  untouched). Else 35.4 → status 2, state 3; 35.3 → status 1, state 3;
  35.2 → status 1, state 2; else the player's Malah intro bit (513,
  `0x005723C0`) set, state 0 and not-intro → state := 1.
- Event 10 (`0x005870D0`): remove the player's GUID from the record list.

#### 3.8 Larzuk and the siege boss

- Init 71 (`0x00587840`, object 543): chain 31 present and +0x15 = 0:
  free spot near the object (`0x00545340`, size 2, mask 0x100, radius 16 (unused),
  limit 100); found → spawn monster 511 there (`0x005B2F20`, mode 1, 5,
  0); created → +0x15 := 1, +0x10 := GUID, unit flags |= 0x3000000, and a
  stored map AI (+0x00 with its +4 ≠ 0) is applied once (`0x0058F000`,
  `0x00666120`, +0x17 := 1). `0x00587950` (map-AI store, from
  `0x00545CB3`) copies the map AI to +0x00 and applies it the same way
  when Larzuk exists. Init 70 (object 542) is a bare `ret`.
- Shenk activated (`0x00587900`, from the AI at `0x005E27D0`): chain 31
  not-intro, status < 2 and the unit's superunique id (`0x005A01E0`) = 42
  → status 2 to all.

#### 3.9 Socket reward (`0x005877C0`, from `world/npc.md` §8.1)

After Larzuk sockets an item for a player with 35.1: set 35.0, clear
35.1; 35.15 clear and no chain-31 record → fatal assert. Nothing is sent
here (the socket handler sends its own messages).

#### 3.10 Status function (`0x00586CE0`, always true)

out := 0. 35.1 or 35.13 → 3, or 4 when 35.5. Else intro → 0; 35.0 → 0;
35.14 → 12; state ≥ 5 → 0; else the status byte.

`0x005875D0` (state 0 and not-intro → state 1) has no caller in 1.14d.

### 4. A5Q2 Rescue on Mount Arreat (chain 32, slot 36)

#### 4.1 Bits and extra data

Bits: 36.2 started, 36.3 left town, 36.4 rescued a group (this game,
cleared at start and join), 36.1 rescue done (reward pending), 36.5 /
36.6 / 36.7 15 / 14 / 12–13 barbarians freed. Extra: +0x00 GUID list;
+0x84 Qual-Kehk started (chat-end pending); +0x86..+0x88 cage group 0–2
spawned; +0x8C + 8g / +0x90 + 8g group g cage position; +0xA4 barbarians
spawned; +0xA8 killed; +0xAC freed; +0xB0 + 20g + 4n GUID of barbarian n
of group g; +0xEC + 4g group portal GUID; +0xF8 + g portal spawned; +0xFC
+ 4g portal close counter; +0x108 + g group accounted for; +0x10B + g
portal may close; +0x10E + g group portal made; +0x114 + 4g group
counter; +0x120 freed-barbarian GUIDs (+0x15C count).

#### 4.2 Flag iterate (`0x00587A90`)

Players lacking 36.0 and 36.1: state 2 → 36.2; state 3 → 36.3.

#### 4.3 Chat (event 0, `0x00587D30`; active fn `0x00588340`)

36.1 set → table state 3. Else: GUID listed → 4; 36.0 → nothing; state >
3 with 36.13 clear → nothing; intro → nothing; idx = index[state]
(`0x00734270`: −1, 0, 1, 2, 3, 4); idx 2 with NPC qual-kehk (515) and
36.4 set → table state 5 (msg 20104); else idx −1 or > 11 → nothing;
else idx. Wants to talk: qual-kehk with state 1 lacking 36.0 and 36.1,
or with 36.1 set (other states); act5pow (534) lacking 36.0 and 36.1.

#### 4.4 Qual-Kehk's messages (event 11, `0x00587B00`, NPC 515 only)

- **20096**: +0x84 := 1; state := 2; flag iterate for all; refresh.
- **20110** (only with 36.1 set; else nothing at all): 36.13 set → state
  ≠ 5 → state := 5, the own seq fn, status 13 (silent); then callback 2
  := null. Runes: n = 2 when 36.6, else 1 when 36.7, else 3; give the
  first n of `r07`, `r08`, `r09` (Tal, Ral, Ort) in that order. Any given
  → set 36.0, clear 36.1, reset_progress(36), add GUID, S5D(32, 0x02, 0).
  Refresh.

Chat end (`0x00587A30`): qual-kehk with +0x84 = 1 and killed < 5 →
status 1 to all, +0x84 := 0, callback 2 := null.

#### 4.5 Level changes (event 3, `0x00588200`)

New level 111 or 112 (Frigid Highlands, Arreat Plateau), not-intro and
killed < 5: state 1 or 2 → state := 3 (b); status 0 → status 1 to all and
callback 2 := null, then flag iterate; status ≠ 0 → flag iterate only
when b. Otherwise, old level 109: quick remove; state 2 and the player
lacks 36.0 and 36.1 → state := 3, flag iterate; then status 0, not-intro
and killed < 5 → status 1 to all, callback 2 := null.

#### 4.6 Kills (event 8, `0x00588040`)

Reached by the prison doors (base id 434 `prisondoor`, part 2 §10) and
the spawned barbarians (§4.7). Needs not-intro.

1. Victim class 434: clear the door room's portal flag (`0x0061AED0`,
   0); for each room adjacent to it, each unit of its list: a monster of
   class 534 in neither mode 12 nor 0 and closer than 15 to the door
   (`0x006416D0`) → its GUID appended to +0x120, freed += 1.
2. Other victims: listed in +0x120 → its group counter += 1 (≥ 5 → group
   accounted for); stop. Else killed += 1.
3. Then (both): killed > 4 and status ≠ 12 → status 12 to all (failed),
   +0x84 := 0, completion flag `0x00587F60` from the killer (lacking 36.0,
   36.1, 36.13). Victim in a group → its counter += 1 (> 4 → accounted).

#### 4.7 The cages and the barbarians

- Init 62 (`0x005886A0`, object 473): not-intro: state < 2 → state := 2;
  status 0 and killed < 5 → status 1 to all. Group g = first of 0..2
  without +0x86 + g (none → stop); a cage whose position equals a stored
  earlier group's → stop. Store the position; spawn (`0x00588600`): up to
  25 tries of monster 534 at the cage (`0x005B2F20`, mode 1, 5, 0), each
  created: unit flags |= 0x3000000, GUID → +0xB0 + 20g + 4n, link to
  chain 32 (`0x005436B0`); stop after 5. spawned += count; +0x86 + g :=
  1; +0x10E + g := 0.
- Rescue (`0x005888D0`, from the prisoner AI at `0x005EE533`, args
  player P and barbarian B): B's group g (none → stop); +0x10E + g set →
  stop. Target := group position + (12, 0). Look through the rooms
  adjacent to P's room for a dead (mode 12) class-434 monster closer
  than 15 to B: none → stop; found → target := its position + (2, 0),
  its room. +0x10E + g := 1. Object 189 at the target (`0x00555230`, type
  2, flags 1, 1, 0); failing, at x − 2; failing and a room exists, at a
  free spot (`0x00545340`, size 2, mask 0x8000, radius 17 (unused), limit 100).
  Created → +0xF8 + g := 1, +0xEC + 4g := GUID, object event 7 at frame +
  25, clear its room's portal flag.
- Completion check (same call, P's flags): spawned = killed + freed and
  all three groups spawned: freed < 12 → completion flag from P; stop.
  Else status 3 to all; P lacking 36.0 and 36.1 → set 36.13, 36.1 and
  36.5 (freed = 15), 36.6 (freed = 14) or 36.7 (any other freed,
  `0x00588B96`–`0x00588BA8`: equality tests only); then from P: party
  `0x005887A0` (36.13 → members `0x00587E60`: lacking 36.0, 36.1 and in
  Act V → 36.13, 36.1 and the same freed bit), completion flag
  `0x00587F60`, `0x005887F0` (36.13 → sound 81). Not all accounted: set
  36.4 on P; killed < 5 → flags byte := 0x20, status 2 to all (the
  flags are set to 0x20, not cleared, before the call, `0x00588C2A`);
  then +0x84 := 0 whatever killed is (`0x00588C33`).

#### 4.8 Rescue portals (class 189 event 7, `0x00588CA0`)

The portal's GUID must be a group's (+0xEC + 4g), else nothing. Mode 1 →
mode 2. Mode 2: group accounted for → close counter += 1; counter > 5 →
may close; may close → mode 3. Mode 3 → mode 4. Then (any mode) object
event 7 again at frame + 25.

#### 4.9 Game start, join, leave (events 13, 14, 10)

- Event 13 (`0x00588470`): clear 36.4. 36.0, 36.15 or 36.1 → nothing.
  36.3 → state 3, status 1; 36.2 → state 2, status 1.
- Event 14 (`0x00588450`): clear 36.4 of the joining player.
- Event 10 (`0x00587F20`): remove the GUID from the record list and the
  extra list +0x00.

#### 4.10 Barbarian AI hooks

| Hook | From | Effect |
|---|---|---|
| `0x00588830` | `0x005EE3F6` | not-intro and B's group counter ≠ 0 → 1 |
| `0x00588880` | `0x005EE423` | not-intro: B's group counter += 1; > 4 → accounted |
| `0x00588D60` | `0x005EE3DB` | B's group portal spawned and the unit exists → returns it |
| `0x00588DD0` | `0x005EE562` | not-intro, status ≠ 5, killed < 5 → status 5 to all |
| `0x00588E10` | `0x005EE525` | a dead class-434 monster in a room adjacent to the player's (or, without a player: game type 3's first client's player, else B) → 1 |

The barbarians-left count of 0x50 and 0x5D (`0x00588C50`) is owned by
`quests.md` §6.2–§6.3.

### 5. A5Q3 Prison of Ice (chain 33, slot 37)

#### 5.1 Bits and extra data

Bits: 37.2 started, 37.3 left town, 37.1 Anya freed (rewards pending),
37.6 (read by part 2 §6.8), 37.7 Scroll of Resistance used, 37.8 scroll
given, 37.9 Anya's item given, 37.10 Anya's item taken (once per
difficulty). Extra (main fields): +0x00 GUID list; +0x84 Anya 0 frozen,
1 thawed, 2 back in town; +0x88 Nihlathak gone from town; +0x8C thaw
step; +0x90 thawed monster spawned, +0x98 its GUID; +0x91 Anya in town,
+0x94 her GUID; +0x92 Nihlathak boss spawned, +0xA0 its GUID; +0x93
Nihlathak in town, +0x9C his GUID; +0xA4 frozen object spawned, +0xA8 its
GUID; +0xAC Malah started (chat-end pending); +0xAD outside portal may
close; +0xAE town portal may close; +0xB1 / +0xC0 outside portal spawned
/ GUID, +0xC4 / +0xC8 its position; +0xB2 / +0xB4 town portal spawned /
GUID; +0xD8 thawing potions in the game; +0xDC town portal close counter;
+0xE1 town dummy seen, +0xE4 its GUID, +0xE8 / +0xEC its position; +0xF0
/ +0xF4 thaw position; +0xF8 outside dummy GUID; +0xFC frozen object
GUID; +0x100 thaw timer exists; +0x101 potion given (chat-end pending);
+0x102 scroll given again this game; +0x104 / +0x108 Anya / Nihlathak
map AI, +0x10C / +0x10D applied; +0x110 Nihlathak town dummy GUID.

#### 5.2 Flag iterate (`0x00588F10`)

Players lacking 37.0 and 37.1: state 2 → 37.2; state 3 → 37.3.

#### 5.3 Chat (event 0, `0x00589B00`; active fn `0x00589F10`)

1. drehyaiced (527): state 6 → kill it in place; nothing is added.
2. malah (513) with state 4: no `ice ` held and +0xD8 = 0 → table state
   3; stop. Malah otherwise: the table-5 test needs 37.8 both set and
   clear and never passes (edge case 3).
3. 37.1 clear: GUID listed → 6; 37.0 → nothing; state > 4 with 37.13
   clear → nothing; intro → nothing; index[state] (`0x00734304`: −1, 0,
   1, 2, 3, 4, 5; −1 or > 9 → nothing).
4. 37.1 set: drehya (512) with 37.9 set or malah with 37.8 set → nothing;
   else table state 5.

Wants to talk: malah with 37.1 set and 37.8 clear, or (lacking 37.0,
37.1) state 1, or state 4 with +0xD8 = 0; drehyaiced with not-intro,
state ≤ 4 and no `ice ` held; drehya with 37.1 set and 37.9 clear.

#### 5.4 Messages (event 11, `0x00589580`)

| From | Msg | Effect |
|---|---|---|
| malah 513 | 20116 | +0xAC := 1; state := 2; flag iterate for all; refresh |
| malah | 20127 | no `ice ` held and +0xD8 = 0: give `ice `; given → +0xD8 += 1, S5D(33, 0x01, 0), +0x101 := 1 |
| malah | 20132 | §5.7 (scroll) |
| drehya 512 | 20136 | §5.7 (Anya's item) |
| any NPC (also none) | 20131 | not-intro: state ≤ 3 → state := 4 and town cleanup (§5.9); status < 3 → status 3 to all (the text the frozen statue sends, §5.6) |

Chat end (`0x00588F80`, never cleared): malah: +0xAC → status 1 to all,
+0xAC := 0; not-intro, +0x101 set and status < 4 → status 4 to all,
+0x101 := 0.

#### 5.5 Level changes (event 3, `0x00589D80`)

1. New level 112 (Arreat Plateau): not-intro and state 0 → state := 1
   and town cleanup (§5.9); otherwise nothing for this step. New level 113 or 114 (Crystalline Passage,
   Frozen River) with not-intro: state ≤ 2 → state := 3 (b); status 0 →
   status 1 to all, flag iterate; status ≠ 0 → flag iterate when b; then
   town cleanup.
2. Old level 109: quick remove; state 2 and the player lacks 37.0 and
   37.1 → state := 3; status 0 → status 1 to all; flag iterate.
3. New level 121–124 (Nihlathak's Temple to Halls of Vaught), not-intro
   and state < 5: completion flag `0x00589AA0` from the event's unit
   (lacking 37.0, 37.1, 37.13); state := 6 (direct write); +0x84 := 2;
   +0x88 := 1 when 0. The prison quest is lost for players who did not
   free Anya.

#### 5.6 Frozen Anya and the thaw

- Dummy 460 init 67 (`0x0058A5B0`): not-intro: +0xF8 := GUID; +0xA4 = 0
  → object event 7 at frame + 25. Its event 7 (`0x0058A500`): not-intro
  and +0xA4 = 0: object 558 at the dummy (`0x00555230`, type 2, flags 1,
  0, 0); created → unit flags |= 0x3000000, +0xA4 := 1, +0xA8 := GUID;
  else event 7 again at frame + 25.
- Object 558 init 74 (`0x0058AA50`): +0xFC := GUID; not-intro and status
  < 2 → status 2 to all.
- Operate 67 (`0x0058ABC0`): no `ice ` held → scroll message 20131 to the
  player (`0x005456A0`); status 1 → status 3 to all. Holding `ice ` and
  lacking 37.0 and 37.1: +0xD8 −= 1; delete `ice `; state := 5; +0x84 :=
  1; set 37.13, 37.1; status 5 to all; FX 16; from the player: party
  `0x00589A50` (37.13 → members `0x00589000`: lacking 37.0, 37.1, in Act
  V → 37.13, 37.1), completion flag `0x00589AA0`; +0x100 = 0 → +0x100 :=
  1, +0xF0 / +0xF4 := object position, timer period 1 `0x0058AAB0`,
  clear the object room's portal flag. A holder with 37.0 or 37.1 gets
  nothing. Returns 0.
- Thaw timer (`0x0058AAB0`): +0x90 set → 0. Step 0: object 558 → mode 2,
  collision freed; step := 1; returns 0. Step 1: step := 2; object
  exists → critical spawn of 527 at (+0xF0, +0xF4) in its room; created
  → the object leaves its room, the stored Anya map AI is applied once,
  +0x98 := GUID, +0x90 := 1, the own seq fn, +0x100 := 0, returns 1.
  Every other case returns 0 and the timer stays (edge case 5).

#### 5.7 Rewards

- **Scroll, msg 20132 (malah)**: with 37.1 set and 37.0 clear: give `tr2
  ` (none → stop); set 37.8; S5D(33, 0x02, 0); 37.9 clear → not-intro
  and status < 6 → status 6 to all; 37.9 set → clear 37.1, set 37.0.
  Then +0x84 = 1: +0xE3 := 0; for each monster (`0x005537D0` type 1)
  `0x005890B0` (class 527: interacting → its interaction ends with
  `0x00589070`, which sends S5D(33, 0x20, 0); else killed in place; +0xE3
  := 1; returns 1); +0xE3 still 0 → drop inactive node of 527; +0xAD :=
  1; +0x84 := 2; Anya to town (§5.9). Send flags.
  Exact form of `0x005890B0` (re-read 2026-10-07): the first class-527
  monster stops the walk (returns 1). If someone talks to it
  (`0x00572DC0`), `0x00573180(game, 527, 0x00589070, 0)` runs and +0xE3
  is **not** set; else it is killed in place (mode 12 request) and
  +0xE3 := 1. `0x00573180` walks the NPC's interaction list (monster
  data +0x30) in list order; for each entry's player: S→C 0x62 with
  (1, the NPC's GUID) to that player (`0x0053D6D0`), the player's
  interaction cleared (`0x00554190`, unconditional with last argument
  0), then the callback for that player: `0x00589070` sends S5D(33,
  0x20, 0) to it. Each entry is freed and the list emptied. So every
  player chatting with iced Anya gets S5D(33, 0x20, 0) (the rewarded
  player only if it is one of them); iced Anya stays in the world, and
  the drop-inactive step still runs (Edge case 9). Without 37.1 (or with
  37.0): 37.0 or 37.15 set, +0x102 = 0, 37.8 set, 37.7 clear and no `tr2
  ` held → give `tr2 `; given → +0x102 := 1.
- **Anya's item, msg 20136 (drehya)**: needs 37.1 set, 37.0 clear and
  37.10 clear. Tier (L = the player's level, stat 12): the exceptional
  list `0x00735EFC` when the difficulty is 1 and (game type +0x6A = 3 or
  L > 45); the elite list `0x00735F34` when the difficulty is 2 and (game
  type 3 or L > 65); else the normal list `0x00735F6C` (also on nightmare
  and hell below those levels). Each tier lists per class (amazon,
  sorceress, necromancer, paladin, barbarian, druid, assassin) 5 item
  codes (assassin 7): normal `am1`–`am5`, `ob1`–`ob5`, `ne1`–`ne5`,
  `pa1`–`pa5`, `ba1`–`ba5`, `dr1`–`dr5`, `ktr wrb axf ces clw btl skr`;
  exceptional the 6–a codes (`am6`…`ama`, …, `9ar 9wb 9xf 9cs 9lw 9tw
  9qr`); elite the b–f codes (`amb`…`amf`, …, `7ar 7wb 7xf 7cs 7lw 7tw
  7qr`). One quest-seed roll(count) picks the code (`0x0045C390`); it is
  given at level `0x00558200(player, 0)`, quality 6 (rare), droppable.
  Given → S5D(33, 0x10, v) with v = the item's `items.txt` drop sound
  (record +0x124, when > 0, else 0); set 37.9; 37.8 set → clear 37.1,
  set 37.0; set 37.10.
- **Using the scroll** (item use `0x0055E170`, owned by the item-use
  spec): needs 37.8 set and 37.7 clear; set 37.7; apply `0x0058A0A0`;
  `5D 21 02 00 0000` (`0x005458E0`); consume. `0x0058A0A0` (also at
  player load, `0x00539A1B`, expansion games only): v = 10 × (number of
  the player's three difficulty records with 37.7); v ≠ 0 → a new stat
  list on the player (`0x006251F0`, attached `0x00626E10`) with stats 39,
  41, 43, 45 (fire, lightning, cold, poison resist) := v, and the four
  stats sent (`0x00548520`).

#### 5.8 Anya's portals (class 189 event 7 in levels 109 or ≥ 113, `0x0058A730`)

Mode 1 → mode 2. Mode 2: in Harrogath (room level 109) close counter
+0xDC += 1, > 5 → +0xAE := 1, +0xAE set → mode 3; elsewhere +0xAD set →
mode 3. Mode 3 → mode 4. Then object event 7 again at frame + 25. Anya
AI hooks (part 2 §10) make the outside portal (`0x0058A820`: +0xB1 set →
1; +0x84 ≠ 0: no room at (+0xC4, +0xC8) → both += 3, returns 0; else
object 189 there (flags 1, 1, 0) → +0xB1 := 1, +0xC0 := GUID, 1).

Anya AI hooks (iced Anya 527, NpcOutOfTown `0x005E7880`, whose use of
the results is `monsters/ai-bodies.md` §9.32; each looks up chain 33
and does nothing / returns 0 without it):

| Hook | Called from | Effect | Returns |
|---|---|---|---|
| `0x0058A940(game, unit)` | `0x005E7806` (portal setup) | +0xC4 / +0xC8 := the unit's position; +0xE0 := 1 | 1 (0 without the record) |
| `0x0058A980(game)` | `0x005E780D`, `0x005E7951` (leave) | +0xAD := 1, +0x84 := 2, Anya to town (§5.9) | — |
| `0x0058A820(game, unit)` | `0x005E7A49` (portal out of town) | above | 1 made or already made; 0 else |
| `0x0058A8D0(game, unit, &xy)` | `0x005E794A` (portal coordinates) | +0x84 = 0 → nothing; +0xB1 clear → xy := the unit's position; set → xy := (+0xC4, +0xC8) | 1 when +0x84 ≠ 0, else 0 |
| `0x0058A9F0(game)` | `0x005E79F9` | — | +0x84 = 0 (still frozen) |
| `0x0058AA10(game)` | `0x005E78B9` (every think) | not-intro and status < 2 → status 2 to all | — |
| `0x0058A7D0(game, &xy)` | no caller | not-intro, +0xB2 set and +0xAE clear → xy := (+0xB8, +0xBC) | 1 / 0 |
| `0x0058A9B0(game)`, `0x0058A9D0(game)` | no caller | +0xAE := 1; returns +0xAF ≠ 0 | — |

Map-AI stores (`0x00545C90`, from preset object placement `0x00555910`,
with the preset's path data; object class 459 → `0x0058AD80`, 461 →
`0x0058AE10`, 543 → `0x00587950`): chain 33 and a non-null path:
+0x104 (Anya) or +0x108 (Nihlathak) := a copy of the path
(`0x006660B0`, game pool); then, if that NPC is in town (+0x91 / +0x93),
its unit (GUID +0x94 / +0x9C, type 1) exists, the copy has +4 ≠ 0 and
+0x10C / +0x10D is clear: apply it (`0x00666120(copy, 0x0058F000(unit))`)
and set +0x10C / +0x10D. The spawn sites of §5.9 apply the stored copy
the same way ("map AI applied once").

#### 5.9 Town NPCs and cleanup

- Dummy 459 init 66 (`0x0058EAC0`): chain 33: +0xE4 := GUID, +0xE1 := 1,
  +0xE8 / +0xEC := position; +0x84 = 2 → the own seq fn and, +0x91 = 0,
  critical spawn of 512 there → +0x94 := GUID, +0x91 := 1, map AI applied
  once. Then chain 34 extra +0x87 set → object event 7 at frame + 12
  (part 2 §6.7).
- Anya to town (`0x005891D0`): +0xE1 = 1 and the dummy +0xE4 exists;
  +0x91 = 0 → critical spawn of 512 at the dummy (`0x005459A0` with the
  dummy's room) → +0x94, +0x91 := 1, map AI applied once; object 189 at
  her position → +0xB2 := 1, +0xB4 := GUID; then the own seq fn.
  Exact nesting (`0x0058926C`, `0x0058928F`): the portal and the seq fn
  run only after a successful spawn in this call (+0x91 already set or
  a failed spawn → return). The portal: the room covering her position
  (`0x00463740` from her room), object 189 type 2 there with flags (1,
  1, 0) (`0x00589313`, same order as the outside portal); no room or no
  object → no portal, the seq fn still runs.
- Dummy 461 init 68 (`0x0058A610`): +0x110 := GUID; +0x88 = 0 and +0x93 =
  0 → critical spawn of 514 → +0x9C, +0x93 := 1, map AI applied once.
- Dummy 462 init 69 (`0x0058A6C0`): +0x88 = 1 and +0x92 = 0 → preset
  spawn (`0x0054E600`) of the monster of superunique 60 (Nihlathak Boss,
  datatables +0xB58) at the dummy → +0xA0, +0x92 := 1.
- Town cleanup (`0x005893E0`): +0x91 set and +0x84 ≠ 2: Anya gone → drop
  inactive node of 512; else her interaction ends and she leaves the
  room (no death); +0x91 := 0. +0x93 set: Nihlathak gone → drop inactive
  node of 514; else killed in town (`0x00589340`: interaction ended,
  path freed, AI event 2 deleted, stat 6 := 0, mode 12, refresh, unit
  flags |= 1) and, the dummy +0x110 existing, its event 7 at frame + 1
  (which kills him again if he is back, `0x00589540`); +0x93 := 0. +0x88
  := 1 when 0.
- Dummy 461 event 7 (`0x00589540`): chain 33; "back" = a monster unit
  with GUID +0x9C still exists (`0x00552F60(game, 1, +0x9C)`); then
  `0x00589340` runs on it again (interaction ended, path freed, AI event
  2 deleted, stat 6 := 0, mode 12, refresh, unit flags |= 1). No other
  test (not +0x93, not his mode): a corpse still in the unit list one
  frame later is killed again. +0x9C is never cleared.

#### 5.10 Game start, join, leave (events 13, 14, 10)

- Event 13 (`0x0058A110`): holds `ice ` → +0xD8 := 1. 37.0 or 37.15 →
  +0x84 := 2, +0x88 := 1; stop. Intro → stop. 37.3 → state 3, status 1;
  else 37.2 → state 2, status 1. Then +0xD8 ≠ 0 → state 3, status 4.
- Event 14 (`0x0058A1E0`): holds `ice ` → +0xD8 += 1. Unless (not-intro
  and state < 5): chain 34 present and not-intro and the player lacks
  37.1, 37.6 and 37.0 → set 38.14.
- Event 10 (`0x00589D20`): remove the GUID from both lists; leaving with
  `ice ` → +0xD8 −= 1.

#### 5.11 Status function (`0x0058A300`, always true)

out := 0. 37.0 → 0. 37.1 or 37.13: 37.8 set and 37.9 clear → 6; 37.8
clear → 5; else 13. Else intro → 0; holds `ice ` → 4 (the party test
never adds, edge case 4); 37.14 → 12; state > 4 → 0; else the status
byte.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| message-state index tables | chain 31 `0x00733928`, 32 `0x00734270`, 33 `0x00734304` | image |
| rune codes | `r07`, `r08`, `r09` | `0x00732FEC` |
| Anya item tiers | normal / nightmare / hell pointer tables (7 × {list, count}) | `0x00735F6C`, `0x00735EFC`, `0x00735F34` |
| levels | 109 Harrogath, 110 Bloody Foothills, 111 Frigid Highlands, 112 Arreat Plateau, 113 Crystalline Passage, 114 Frozen River, 121–124 Nihlathak's Temple … Halls of Vaught | `levels.txt` |
| NPCs / monsters | 434 prisondoor, 511 larzuk, 512 drehya, 513 malah, 514 nihlathak, 515 qual-kehk, 527 drehyaiced, 534 act5pow | `monstats.txt` |
| superuniques | 42 Siege Boss (overseer1), 60 Nihlathak Boss | `superuniques.txt` |
| objects | 189, 459–462, 473, 542, 543, 558 | `objects.txt` |
| sounds | 19 refused, 80 Shenk, 81 barbarians | §3.6, §4.7 |
| FX bytes (0x89) | 15 Shenk, 16 Anya freed | §3.6, §5.6 |
| timer period | 1 (thaw) | §5.6 |
| object event delays (frames) | 25 portals and dummy 460, 12 dummy 459, 1 dummy 461 | §4.7–§5.9 |
| thresholds | 5 barbarians killed fails; 12 freed completes; 15 / 14 / 12–13 → 3 / 2 / 1 runes | §4.6, §4.7, §4.4 |
| level gates | Anya tier: > 45 (nightmare), > 65 (hell) | §5.7 |

## Randomness

| When | Seed | Draws | Decides |
|---|---|---|---|
| Anya's item (msg 20136) | quest | 1 (roll(count), count 5 or 7) | item code within the class list |

Runes, potions, scrolls and the rare item's own properties draw from item
creation (item spec); spawned monsters from the monster spawn code. No
other Act V part-1 quest code draws.

## Edge cases & original bugs

1. Shenk's kill callback does not check the victim (§3.6): only the
   chain-31 link restricts it.
2. Rescue fails for the whole game at the fifth dead barbarian (status
   12); freed barbarians that die later count only toward their group
   (§4.6).
3. Malah's event 0 tests 37.8 set and then clear for the scroll re-offer
   (table state 5): the branch is dead in 1.14d (D2MOO tests once).
4. The status function's party potion check (`0x0058A2A0`, members via
   `0x0058A270`) looks up chain 3 (A1Q3) instead of 33 and writes its
   flag at A1Q3 extra +0xE2, past that record's 0xA4-byte block; the
   value read back is always 0, so status 4 never comes from a party
   member. Same bug in D2MOO 1.10f. Implementations reproduce the
   result, not the stray write.
5. The thaw timer returns 0 forever when the frozen object or the spawn
   is missing at step 1 (§5.6): the timer entry stays in the list.
6. Entering Nihlathak's Temple or the Halls before freeing Anya sets the
   prison state to 6 and moves Anya to town (§5.5): Prison of Ice cannot
   be finished afterwards in that game.
7. `0x00587830` (init 70) and `0x005875D0` do nothing reachable; the
   preset-boss hook `0x00545B50` jumps to the `ret` stub `0x0058CFD0` for
   levels ≥ 108 (`quests-act3.md` §7.5 for the Act III branch).
8. The scroll's resistance stat list is added on each use and at load,
   with the sum over all difficulties (§5.7); whether the earlier list is
   replaced is open (Open question 3).
9. The scroll reward with iced Anya in a chat ends the chat for every
   chatting player (0x62 and S5D(33, 0x20, 0) each) but neither kills
   her nor sets +0xE3, so the drop-inactive step runs and iced Anya
   stays where she was while Anya also appears in town (§5.7).
10. Nihlathak's second town kill (`0x00589540`, frame + 1) tests only
    that his GUID still names a monster (§5.9).
11. Anya's town portal and the Prison of Ice seq fn are skipped when
    Anya is already in town or her spawn fails (§5.9).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Larzuk 20090, player 35.1 set, 35.13 set, state 3 | reset bits 2–11; 35.5 set; state 5; status 13 to qualifying players; GUID added | §3.4 |
| Siege status fn: 35.1 set, 35.5 clear | 3 | §3.10 |
| Qual-Kehk 20110, 36.1 + 36.7 | one rune `r07`; 36.0 set, 36.1 clear; `5D 20 02 00 0000` | §4.4 |
| Qual-Kehk 20110, 36.1 + 36.5 | `r07`, `r08`, `r09` | §4.4 |
| 3 groups spawned (15), killed 2, freed 13, last rescue | spawned = 15 = 2 + 13; freed ≥ 12 → status 3; P gets 36.13, 36.1, 36.7 | §4.7 |
| killed reaches 5 | status 12 to all; completion flag | §4.6 |
| Anya item, hell, level 70, assassin, quest-seed roll(7) = 3 | `7cs` rare, level from `0x00558200` | §5.7 |
| Anya item, nightmare, level 40, open game type ≠ 3, barbarian, roll(5) = 0 | `ba1` (normal list) | §5.7 |
| scroll used after normal and nightmare scrolls (37.7 in two records) | resist stats 39/41/43/45 = 20 | §5.7 |
| Prison status fn, 37.1 set, 37.8 set, 37.9 clear | 6 | §5.11 |

## Provenance

- Read from the 1.14d `Game.exe` exports (`re/exports/funcs`,
  `functions.tsv`, `all.asm`) and `tools/ghidra/disasm.py` (`fn`, `at`,
  `xref`: register arguments of `0x00544350`, `0x00544300`,
  `0x00543640`, `0x005436B0`, `0x00544720`, FX bytes, sounds, the 0x5D
  bytes built inline; callbacks reachable only through pointers between
  exported functions), raw image bytes for the index tables, the rune and
  Anya item tables, the object init / operate tables and the link jump
  tables. Assert strings `d:\diablo2\…\Quests\a5q1.cpp` … `a5q3.cpp`
  identify the files. Live 1.14d `levels.txt`, `objects.txt`,
  `monstats.txt`, `superuniques.txt` (extracted patch_d2 tables) gave the
  row names.
- D2MOO 1.10f `D2Game/src/QUESTS/ACT5/A5Q1–A5Q3.cpp` gave names and
  structure; each rule was matched to the 1.14d function named next to
  it. Differences: 1.14d's dead Malah re-offer test (edge case 3); the
  same chain-3 party bug; no victim test in Shenk's kill; the 20169 /
  Ancients details are in part 2.
- No packet or RNG recording of Act V exists yet.
- 2026-10-07 answers (QE-1–QE-4, QE-8b/c): `disasm.py at` on
  `0x00589540`, `0x00589340`, `0x005893E0`, `0x005890B0`, `0x00589070`,
  `0x00573180`, `0x005891D0`, `0x0058A7D0`–`0x0058AA42`, `0x0058AD80`,
  `0x0058AE10`, `0x00545C90`, `0x005E77A0`, `0x005E7880`, `0x00588B30`–
  `0x00588C40`; `xref` for the hook pointers.

## Open questions

1. Status meanings per Act V quest (client quest log): settle with
   `quests.md` open question 1.
2. `0x00558200(player, 0)`, the item level of Anya's rare item: item
   spec.
3. Does a second `0x0058A0A0` stat list (scroll used again in a later
   difficulty, or at load) stack with or replace the first? Settle with
   the stat-list spec (owner ids of `0x006251F0`) and a recording.
4. Prisoner AI and Anya AI callers (`0x005EE3DB`…`0x005EE562`,
   `0x005E7806`…`0x005E7A49`) and what they do with the returned values:
   AI spec (another owner). **Answered** (2026-10-07) for Anya: every
   hook's effect and return value is in §5.8 (table, and the map-AI
   stores `0x0058AD80` / `0x0058AE10`); the AI's use of the returns is
   `monsters/ai-bodies.md` §9.32 (NpcOutOfTown). The prisoner hooks are
   §4.10.
5. Siege Boss state 118 set at creation (part 2 §10): states spec.
6. `quests.tsv` column `spec` still says `catalogued` for rows 31–36;
   switch it to `specified` (with a link to these files) once
   `quests.md` §2.4 documents owner files per act. Row 40's `?` cells
   and the intro message rows: part 2 open question 8. **Answered**
   (2026-10-07): rows 31–36 switched to `specified` (quests-fixups
   CODE-TABLE commit; every address of these rows is named in
   `quests-act5.md` / `-2`); row 40 and the intro rows were done by
   quests-core (part 2 open question 8).
7. Record a full Act V run (packets + RNG, `docs/HANDOFF.md` §5): Shenk,
   the rescue portals, the rune reward, Anya's thaw, the scroll and
   Anya's item draw.
