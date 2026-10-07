# Spec: World — Quests, Act II (Radament through the Seven Tombs, Act II gossip)

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (decompile exports plus disassembly for register arguments; addresses
  inline). No Act II quest recording exists yet; nothing is verified
  against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act2`
- **Related specs:** `world/quests-act2-2.md` (answers to the
  implementation questions QB-1–QB-20, Jerhyn's spawns §2, the orifice
  insert and S→C 0x58 §3); `world/quests.md` (owner of the shared machinery:
  flag records §1, quest records §2, game entry §3, events and dispatch
  §4, updater and timers §5, status messages §6, NPC dialog hooks §7,
  act transitions and warp checks §8, quest items and helpers §9; this
  spec uses its terms and never restates them); `world/quests.tsv`
  (records, callbacks: rows 8–16 and 38), `world/quest-messages.tsv`
  (NPC message tables of chains 7–13, 26, 27, 38); `world/cube.md` §8
  (the `hst ` transmute calls §4.9 here); `world/npc.md` (chat, the
  Meshif act travel that ends Act II, §8.3 there); `sim/rng.md` §3, §5.2,
  §5.4; `sim/tick.md` §5 (object events); `sim/unit-order.md` §7 (player
  iteration order); item, object, monster, AI and DRLG specs (not
  written: item creation, object modes, monster spawning, NPC AI, maze
  building).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–63 |
| Inputs | 64–76 |
| Outputs / state changes | 77–85 |
| Rules | 86–87 |
|   1. Conventions | 88–176 |
|   2. Act II records | 177–196 |
|   3. A2Q1 Radament's Lair (chain 8, slot 9) | 197–280 |
|   4. A2Q2 The Horadric Staff (chain 9, slot 10) | 281–386 |
|   5. A2Q3 Tainted Sun (chain 10, slot 11) | 387–485 |
|   6. A2Q4 Arcane Sanctuary (chain 11, slot 12) | 486–605 |
|   7. A2Q5 The Summoner (chain 12, slot 13) | 606–641 |
|   8. A2Q6 The Seven Tombs (chain 13, slot 14) | 642–816 |
|   9. Act II gossip and intro records | 817–825 |
|   10. Hooks called from other systems | 826–842 |
| Constants & data dependencies | 843–856 |
| Randomness | 857–871 |
| Edge cases & original bugs | 872–905 |
| Test vectors | 906–926 |
| Provenance | 927–949 |
| Open questions | 950–1035 |
<!-- /index -->

## Summary

Act II has six main quests (chains 8–13, flag slots 9–14) and four
gossip/intro records (chains 7, 26, 27, 38). Each main quest is a quest
record (`quests.md` §2.2) whose callbacks react to NPC chat, scroll
messages (C→S 0x31), level changes, kills, item pick-up / drop, player
join / leave and quest objects (chests, the Tainted Sun altar, the
Horazon tome, the staff orifice). They set player and game flag bits,
drop quest items and gold, open portals and doors, change the Act II
light level and report status with S→C 0x5D. This spec owns those
per-quest state machines, the Act II quest objects' quest-side logic,
the true-tomb lookup and the hooks other systems call into Act II quest
code.

## Inputs

| Name | Type | Source |
|---|---|---|
| events 0, 2, 3, 4, 5, 8, 9, 10, 11, 13, 14 | quest callbacks | `quests.md` §4 |
| object operate / init calls | object functions 24, 25, 34, 39–42 / 18–21, 29–33, 37, 38 | object spec; tables `0x00732D18`, `0x00731BC0` (§1.5) |
| C→S 0x44 StaffInOrifice | intent | `client-messages.tsv` |
| cube transmute producing `hst ` | call | `world/cube.md` §8 |
| monster AI hooks (Radament, Summoner, palace guard, Jerhyn, Tyrael) | calls | AI spec (§10) |
| quest seed | control +0x18 | `quests.md` §2.1 |
| DRLG staff / Duriel tomb levels | drlg +0x94 / +0x484 | §8.1 |
| `misc.txt` / `weapons.txt` codes `tr1`, `msf`, `vip`, `box`, `hst`, `ass`; `missiles.txt` row 338 `Range` | tables | §3.7, §4, §8.7 |

## Outputs / state changes

Player and game quest bits of slots 7–14, 30, 31; record state, status
and flags bytes; quest extra data; S→C 0x5D, 0x28, 0x27 (text refresh),
0x29, 0x53, 0x89, 0x50, 0x58; items dropped (`tr1`, `msf`, `box`, `vip`,
`ass`, gold) or deleted (`tr1`, `hst`, `vip`, `msf`); portal, harem
blocker and Duriel's-lair portal objects created; object modes; Act II
light level (environment spec); one skill point (Book of Skill use).

## Rules

### 1. Conventions

#### 1.1 Notation

- `s.b` = bit b of slot s in the acting player's record of the game
  difficulty (`quests.md` §1.4); "game s.b" = the game record.
- "state := n" = `0x00544350(record, n)` (record +0x0C; its file/line
  arguments are debug only). "status" = record +0x0B, "flags" = record
  +0x14 (`quests.md` §2.2).
- "status n to all" = flags := 0, then `0x00544300(record, n, 0, F, 1)`:
  status := n and, for every player in `unit-order.md` §7 order, F
  sends 0x5D for the chain (`quests.md` §6.3) when (s.0 clear and s.15
  clear) or s.13 or s.14 (s = the quest's slot). Every Act II quest has
  its own copy of F with this exact test (§2 column F). "status n
  (silent)" = the same call with iterate = 0: only the status byte
  changes, nothing is sent.
- "for each player" = `0x005537D0(game, 0, from, fn)` (player
  iteration, `unit-order.md` §7). "party of P" = `0x00540510(game,
  party, fn)` when P's party id (`0x00554630`) ≠ −1.
- "in Act II" = the player's room's level has act 1 (`0x006427F0`).
- "refresh" = `quests.md` §7.2 (`0x00545780`): 0x27 then 0x29 to the
  player. "send flags" = `quests.md` §6.6 (0x28). "add GUID" /
  "GUID listed" = the record's player list (`quests.md` §9.3,
  `0x00545200` / `0x005452C0`); event 10 removes the player from it
  (`0x00545530`); "quick remove" = `0x00545310` (leaving Lut Gholein,
  level 40).
- "completion flag" iterate: for each player without s.0 and s.1: set
  s.14 and send `5D <chain> 00 0C 0000` (`0x00545920`, act argument 0).
- "FX b" = `quests.md` §6.5 (`0x00545760`): every player gets 0x28 then
  `89 b`.

#### 1.2 Message-list selection (event 0)

All event-0 callbacks add entries from the record's table
(`quests.md` §7.1, `quest-messages.tsv`) with `0x00543790(npc,
table state)`. "Table state" below means the row index in
`quest-messages.tsv`; "record state" means +0x0C. Index tables that map
record state to table state are read from the image (§Constants).

#### 1.3 Chest pattern (scroll, staff and cube chests, Tainted Sun altar)

A quest chest's operate function: passes `0x00545850(op)` (the shared
quest-chest gate, `quests-act2-2.md` §5.1) or returns 0 (the three chests only; the
altar has its own guards, §5.7, and never calls the gate); sets the
object's drop code once, before the count; counts qualifying players (for each player, starting at the
operating player); creates that many quest items with `0x00559A30(game,
object, quality, &level, 0, −1, droppable)` (item spec; `&level` is an
out parameter, `quests-act2-2.md` §1 item 20); drops the
chest's own treasure with `0x00585B90(op, 4)` (magic, object spec);
then one quest-seed step: n = (lo' mod 5) + 5; n times `0x00585970(game,
object, 'gld ', 2)` (normal-quality gold). The chests return 0.

#### 1.4 Sequence chain

The sequence functions (+0xF0) form a chain started by `quests.md` §3
step 3 (chain 8 is one of the five roots). "Call seq(c)" = look up chain
c and call that record's own sequence function with that record.

| Chain | Seq fn | Rule |
|---|---|---|
| 8 Radament | `0x005991C0` | state ≠ 5 and not-intro → return 1; else call seq(13) (return its value; 0 if absent) |
| 10 Tainted Sun | `0x0059A480` | state ≠ 5 and not-intro → 1; else seq(11) |
| 11 Arcane | `0x0059B500` | if state = 0 and not-intro: state := 1 and seq(13); return 1 |
| 13 Tombs | `0x0059D450` | if state = 0 and not-intro: state := 1; return seq(10) (1 if absent) |

Chains 7, 9, 12 have none. So finishing Radament (or a game where it is
off) moves the Seven Tombs to state 1 (Jerhyn wants to talk), and
finishing Tainted Sun moves Arcane Sanctuary to state 1. Radament's and
Tainted Sun's reward messages call their own sequence function (§3.3,
§5.6).

#### 1.5 Act II quest objects

| Object (objects.txt row) | Operate fn | Init fn |
|---|---|---|
| 121 `jerhyn` (start) / 122 `jerhyn` (palace) | — | 18 `0x005448B0` → `0x0059F380`; 19 `0x005448E0` → `0x0059F440` (§6.10) |
| 149 `taintedsunaltar` | 24 `0x0059A7E0` (§5.7) | 20 `0x00544910` → `0x0059A3F0` (§5.8) |
| 152 `orifice` | 25 `0x0059DC70` (§8.6) | 21 `0x0059DB50` (§8.8) |
| 153 `Door` (Tyrael's door) | — | 38 `0x0059DAD0` (§8.8) |
| 298 `portal` (Arcane Sanctuary ↔ Palace Cellar 3) | 34 `0x005846B0` (object spec; calls §6.9) | 29 `0x0059BA40` (§6.9) |
| 318 `eunuch` (harem blocker) | — | 30 `0x0059B7D0` (§6.8) |
| 354 / 355 / 356 `chest` (cube / scroll / staff) | 39 `0x00599DF0`, 40 `0x00599C10`, 41 `0x00599CF0` (§4.7) | 31 `0x00599EE0`, 32 `0x00599F00`, 33 `0x00599EF0`: bare `ret` |
| 357 `Tome` (Horazon's journal) | 42 `0x0059B970` (§6.7) | — |

Table bases: operate `0x00732D18` (4-byte entries, index = OperateFn),
init `0x00731BC0` (index = InitFn). Init 33 is also the InitFn of rows
360, 362, 375 (no effect). Init 37 (`0x0059DA50`, §8.8) is used by no
1.14d `objects.txt` row.

### 2. Act II records

Callback addresses are in `quests.tsv`; this table adds what the init
functions store beyond it (1.14d `0x0059xxxx` init functions).

| Chain / slot | Quest | Init | State, init_no, seq_id | Extra (bytes, zeroed) | F | Table |
|---|---|---|---|---|---|---|
| 8 / 9 | Radament's Lair | `0x00599340` | 1, 4, 13 | 0x14 | `0x005988B0` | `0x00739218` |
| 9 / 10 | The Horadric Staff | `0x0059EB00` | 0, 0, — ; status := 13 | 0x30 | `0x00599510` / `0x00599570` | `0x00739918` |
| 10 / 11 | Tainted Sun | `0x0059EEA0` | 0, 4, 11 | 0xA0 | `0x00599F50` | `0x0073A188` |
| 11 / 12 | Arcane Sanctuary | `0x0059F2A0` | 0, 5, 13 | 0x48 | `0x0059AD50` | `0x0073A638` |
| 12 / 13 | The Summoner | `0x0059C280` | 0, 2, — | 0x0C (+0x00, +0x01, +0x04, +0x08 cleared) | `0x0059BCB0` | `0x0073AF68` |
| 13 / 14 | The Seven Tombs | `0x0059D5B0` | 0, 4, — | 0x68 | `0x0059C650` | `0x0073B288` |

All are active = 1. Status functions: chain 9 `0x0059E630` (§4.6),
chain 13 `0x0059CA50` (§8.5); the others use the default rule
(`quests.md` §6.1). There is no difficulty branch anywhere in the Act II
quest code; difficulty acts only through the record used and through
item and monster data.

### 3. A2Q1 Radament's Lair (chain 8, slot 9)

#### 3.1 Extra data

+0x00 Radament killed; +0x04 room of the kill; +0x08 Atma started the
quest (chat-end pending); +0x09 "first entry status sent"; +0x0A status
timer exists; +0x0C Book of Skill drop count; +0x10 reward pending from
an earlier game.

#### 3.2 Chat (event 0, `0x00598C40`; active fn `0x00598910`)

Text: 9.1 → table state 3; else GUID listed → 4; else if 9.0 clear,
state ≠ 0 and (state < 4 or 9.13): table state = index[state] (index
`0x007398FC`: −1, 0, 1, 2, 3, 4, 0; −1 or ≥ 9 → nothing). Wants to talk
(0x8A): NPC atma (176) and 9.0 clear and either (not-intro, state 1,
9.15 clear) or 9.1.

#### 3.3 Atma's messages (event 11, `0x00598A70`, NPC 176)

- **304**: extra +0x08 := 1; state := 2; for each player
  (`0x005989E0`): players without 9.0 and 9.1 get 9.2 when the state is
  2, and when it is 3, 9.3 if status = 1 else 9.4. Refresh.
- **334** (only with 9.1): if 9.13: if state ≠ 5: status 13 to all,
  clear callback 2, state := 5, call own seq fn; then if intro: game
  9.13. If 9.13 clear and extra +0x10: call seq(13). Then set 9.0, clear
  9.1, add GUID, refresh.

#### 3.4 Chat end (event 2, `0x00598990`)

NPC 176 and extra +0x08 = 1: status 1 to all; extra +0x08 := 0; clear
callback 2.

#### 3.5 Leaving town (event 3, `0x00599130`)

Old level 40: quick remove; then if state = 2 and the player lacks 9.0
and 9.1: state := 3 and the §3.3 flag iterate (status is still 1 →
9.3).

#### 3.6 Radament's AI hook (`0x00599420`, called by Radament's AI at `0x005F2C47`)

Requires chain 8 with not-intro = 1 and (state < 3 or status < 2) and
the unit in level 49 (Sewers Level 3). Clear callback 2; state < 3 →
state := 3. If status < 2: flags := 0, status := 2 with iterate =
(extra +0x09 was 0; it becomes 1), then the §3.3 flag iterate (→ 9.4).
If status ≥ 2: the flag iterate only when the state was just raised
(no 0x5D); the state already ≥ 3: nothing more. The quest-
chain link hook for monster 229 (`quests.md` §4.6, `0x005991B0`) is a
bare `ret 4`.

#### 3.7 Radament's death (event 8, `0x00599020`)

Only when not-intro: state := 4; game 9.13; clear callback 2; extra +0
:= 1, +4 := the victim's room. For each player from the victim:

1. `0x00598E20`: players whose room is the kill room or one of its
   adjacent rooms (adjacency list of the player's room, `0x00619790`)
   and who lack 9.0 and 9.1: set 9.13, 9.1, 9.5; send flags; then their
   party: members in Act II without 9.0 and 9.1 get 9.13, 9.1, 9.5 and
   send flags (`0x00598D40`).
2. Completion flag (`0x00598DC0`).
3. Book count (`0x00598FA0`, count reset to 0 first): players with 9.5
   that hold no `ass ` (`0x00558110`) add 1; players with 9.13 get sound
   50.

Then, if no timer yet (+0x0A): +0x0A := 1, timer period 12
(`0x00598F70`: if state = 4, status 3 to all; +0x0A := 0; returns 1).
If the count > 0: the victim's drop code := `ass `; count times
`0x00559A30(game, victim, 2, &0, 0, −1, 0)` (normal Book of Skill).

#### 3.8 Book of Skill (item use, owned by the item-use spec)

Using `ass ` (`0x0055E170`, usability test `0x0055CC90`): with 9.5:
clear 9.5, add 1 to stat 5 (new skill points, `0x006272B0`), send `5D 08
02 00 0000` (`0x005458E0`), consume the book; without 9.5: sound 19,
the book stays.

#### 3.9 Game start (event 13, `0x00599230`)

From the first entering player (`quests.md` §3): 9.0: if 9.5 and no `ass
` held, clear 9.0, 9.1, 9.15, 9.5; then game 9.13, not-intro := 0, state
:= 0. Else 9.15: if 9.1, extra +0x10 := 1; state := 0, not-intro := 0.
Else 9.4 → state 3, status 2; 9.3 → state 3, status 1; 9.2 → state 2,
status 1.

### 4. A2Q2 The Horadric Staff (chain 9, slot 10)

#### 4.1 Items and bits

Items: Horadric Scroll `tr1`, Staff of Kings `msf`, Viper Amulet `vip`,
Horadric Cube `box`, Horadric Staff `hst` (all `quest` = 10 = chain 9 +
1). Player bits: 10.3 Cain read the scroll (or any Cain staff talk),
10.4 told about the amulet, 10.5 about the staff, 10.6 about the cube,
10.10 told about the assembled staff, 10.11 staff assembled (cube), 10.9
cleared at every game start and join (§8.11). The row's no_set_state = 1:
§3 of `quests.md` never switches this record off.

#### 4.2 Extra data

+0x14 drop count; +0x18 Staff-of-Kings count, +0x1C Horadric-Staff
count, +0x20 cube count, +0x24 amulet count (items held by players in
the game); +0x28 Staff of Kings dropped, +0x29 cube dropped (chest
opened with a drop); +0x2A staff assembled; +0x2B "missing" already
reported; +0x2C GUID of the assembling player (−1 none).

#### 4.3 Cain's text selector (`0x00599590`)

out := 0xFF. If the player holds `hst `: out := 4 when 10.10 is clear
(result true), else 5 (false). Otherwise, in order (later assignments
win): `box ` held: 10.6 clear → 3, true (stop); else out := 9. `tr1 `:
10.3 clear → 0, true; else 6. `vip `: 10.4 clear → 1, true; else 7.
`msf `: 10.5 clear → 2, true; else 8. Result false.

#### 4.4 Chat (event 0, `0x00599910`; active fn `0x005996C0`)

Only for cain2 (244). 10.1 → table state 4. Else selector; true with out
≠ 0xFF → table state out. Else GUID listed → 5. Else if 10.0 → nothing.
Else if out > 9: 10.3 clear → nothing, set → 6. Else table state out.
Wants to talk: NPC 244 and (10.1 or the selector is true).

#### 4.5 Cain's messages (event 11, `0x005997F0`, NPC 244)

| Msg | Effect |
|---|---|
| 335 | delete `tr1 ` (`quests.md` §9.2); set 10.3 |
| 336 / 337 / 338 | flags := 0; set 10.4 / 10.5 / 10.6; set 10.3 |
| 339 | clear 10.1; set 10.3; add GUID; set 10.10, 10.4, 10.6, 10.5; refresh |

#### 4.6 Status function (`0x0059E630`, always returns true)

1. 7.0 clear (Act I not done) → 0.
2. 10.0 or 10.1: 14.0 clear → 6 − (10.10 ? 1 : 0); 14.0 set → 11 + 2 ×
   (10.13 ? 1 : 0).
3. Else with the held items: `msf`, `box`, `vip` all held → 10.6 ? 3 :
   (10.3 ? 2 : 4). Else `hst` → 6 − (10.10 ? 1 : 0). Else `tr1` and 10.3
   clear → 1. Else start at 0; set 9 if (cube dropped and cube count = 0)
   or (amulet count = 0 and Tainted Sun's altar destroyed, chain 10 extra
   +0x04 via `0x0059AC80`) or (staff dropped and staff count = 0); then
   10.3 → 2 (overrides 9); else any of `vip`, `msf`, `box` held → 4.

#### 4.7 Chests (§1.3 pattern)

| Chest (obj) | Code | Qualifying player | Quality, droppable | Per created item |
|---|---|---|---|---|
| scroll (355), `0x00599C10` | `tr1 ` | lacks 10.0 and 10.3 (`0x00599700`) | 7, 1 | — |
| staff (356), `0x00599CF0` | `msf ` | lacks 10.0, holds no `msf ` and no `hst ` (`0x00599750`) | 7, 1 | identified (flag 0x10, `0x006280D0`); +0x18 += 1; +0x28 := 1 |
| cube (354), `0x00599DF0` | `box ` | holds no `box ` (`0x005997C0`) | 2, 1 | +0x20 += 1; +0x29 := 1 |

#### 4.8 Pick-up and drop (events 4, 5)

- Pick-up (`0x00599A30`; only active records, `quests.md` §4.3): flags
  := 0; by code: `tr1 ` with 10.3 clear → status := 1; `vip ` / `box ` →
  status := 10.3 ? 2 : 6; `msf ` → status := 10.3 ? 2 : 6. Then, for
  these status writes only, 0x5D to this player if F holds (`tr1 ` with
  10.3 set: nothing beyond flags := 0). The status byte is the
  record's, shared by all players (edge case 3).
- Drop (`0x00599B30`): `vip ` clears 10.4, `box ` 10.6, `msf ` 10.5 (each
  only when set).
- Event 8 callback `0x00599A20` is a bare `ret`.

#### 4.9 Staff assembly (from the cube, `0x0059E5C0`)

When a transmute places an `hst ` (`world/cube.md` §8 step 3): +0x2A :=
1, +0x2C := player GUID; staff count −1, amulet count −1, Horadric-Staff
count +1; send flags; then set 10.11 (after the 0x28, so the client sees
10.11 only with the next 0x28); then the Arcane hook `0x0059B660`: chain
11 intro → open the palace (§6.2); else state 0 → state := 1, and status
0 → status 1 to all (chain 11's F). The two tests are independent
(`0x0059B67A`, `0x0059B696`); the status step also clears the record's
flags byte (+0x14 := 0) before the 0x5D iterate. Chain 11 absent →
nothing.

#### 4.10 Joining, starting and leaving (events 9, 13, 14)

- Event 13 (`0x0059E850`) and 14 (`0x0059E970`) first add the player's
  held `msf`, `vip`, `box`, `hst` to the counts.
- Event 13 then, unless 10.0, unless chain 13 is absent, intro or its
  lair is open (chain 13 extra +0x0B), unless the player holds `hst `:
  clears 10.5 if no `msf `, 10.6 if no `box `, 10.4 if no `vip `.
- Event 14 then: if +0x2B and (Horadric-Staff count > 0 or cube,
  amulet and staff counts all ≠ 0) and chain 13 is not-intro with extra
  +0x0E ≠ 1: chain 13 extra +0x10 := 0 (no longer missing).
- Event 9 (`0x0059EA20`, per quest item of a leaving player): decrement
  the count of its code. Then, if +0x2B = 0 and Horadric-Staff count = 0
  and ((cube dropped and cube count = 0) or (amulet count = 0 and the
  altar destroyed) or (staff dropped and staff count = 0)): if chain 13
  is not-intro with extra +0x0E ≠ 1, chain 13 extra +0x10 := 1 and +0x14
  := 8 when game type byte +0x6A = 3, else 9; in every such case +0x2B :=
  1.
- Event 3 (`0x00599A00`): old level 40 → quick remove.

### 5. A2Q3 Tainted Sun (chain 10, slot 11)

#### 5.1 Extra data

+0x01 darken timer exists; +0x02 darkness applied; +0x03 darkness
pending (Act II not loaded); +0x04 altar destroyed; +0x05 altar seen;
+0x06 status timer; +0x08 altar mode (0 = neutral); +0x0C altar GUID;
+0x10 altar room; +0x14 player list (reset at init, `0x00545300`; only
event 10 touches it); +0x98 altar level; +0x9C amulet drop count.

#### 5.2 Darken (`0x0059A350`)

Status 1 to all; state 0 → state := 1. If the game has Act II (game
+0xC0): start the Tainted Sun on it (`0x0061C450`, `quests-act2-2.md` §5.2),
then for each player whose client is in Act II (`0x005382B0`): S→C
0x53 `53 05000000 00000000 01` (`0x0053C900`) and `5D 0A 10 00 0000`;
return 1. Without Act II: +0x03 := 1, return 0. Callers record the
outcome as below.

#### 5.3 Darkening triggers

- Event 3 (`0x0059EE00`): new level 44 or 45 (Lost City, Valley of
  Snakes), state 0, not-intro, no timer yet (`0x0059EDC0`): timer period
  = `between(quest seed, 15, 17)` (`0x004BC500`: max ≤ min → min without
  a step; else roll(max − min) + min, i.e. (lo' & 1) + 15), callback
  `0x0059ED80`; +0x01 := 1. Timer: +0x01 := 0; if +0x02 = 0: darken;
  success → +0x02 := 1 and the flag iterate (§5.4); failure → +0x03 :=
  1. Returns 1.
- Event 3, new level 40 with +0x03 = 1: darken; success → flag iterate,
  +0x03 := 0.
- Event 3, old level 40: quick remove; state 2 → state := 3.
- The three event-3 items above are independent tests run in this
  order (`0x0059EE0E`, `0x0059EE37`, `0x0059EE66`); one level change
  can meet two of them (old level 40 → new 44: the timer start and the
  quick remove).
- Act load (`0x0059AC40(act, n)`, from `0x0053ACB3`): n = 1 and +0x03 =
  1: start the Tainted Sun on that act, +0x03 := 0, +0x02 := 1.
- Altar init (§5.8) and game start (§5.6).

#### 5.4 Flag iterate (`0x0059A4F0`)

Players without 11.0 and 11.1: state ≥ 1 → 11.2; state 2 → 11.3; state 3
→ 11.4.

#### 5.5 Chat (event 0, `0x0059A270`; active fn `0x0059A1C0`)

11.1 → table state 3. Else, unless 11.14: GUID listed → 4; else if state
≠ 0, not-intro, 11.0 clear and state ≤ 3: table state = index[state]
(`0x0073A620`: −1, 0, 1, 2, 3, 0; ≤ 5). Wants to talk: 11.0 clear and
(11.1 with NPC in {176, 175, 198, 199, 177, 202, 244, 210, 200, 201} —
not fara 178 — or NPC drognan 177 with state 1).

#### 5.6 Messages, start and chat end (events 11, 13)

- Event 11 (`0x0059EBE0`): **348** from drognan (177), not-intro: status
  1 → status 2 (silent); state 1 → state := 2 and the flag iterate.
  **362–372** from any NPC: if state ≠ 5, not-intro and 11.13: status 13
  (silent), state := 5, own seq fn. Refresh. If 11.1: set 11.0, clear
  11.1, add GUID; if intro: game 11.13.
- Event 13 (`0x0059A5D0`): 11.0 or 11.1 → game 11.13. Else with 11.2:
  darken (`0x0059A570`); failure → +0x03 := 1 and this player gets `5D 0A
  01 00 0000`; success → +0x02 := 1. Then 11.4 → status 2, state 3;
  11.3 → status 2, state 2; else status 1, state 1.
- Event 10: remove from the record list and the +0x14 list.

#### 5.7 The altar (operate 24, `0x0059A7E0`)

1. If (11.1 or 11.0) and (10.0 or the player holds `vip ` or `hst `):
   sound 19 to the player, return 0.
2. If the object's mode ≠ 0: return 0.
3. Not-intro: +0x10 := altar room, +0x98 := its level; mode := 1; +0x08
   := 2; +0x05 := 1; +0x0C := GUID; +0x04 := 1; if Act II exists: +0x02
   := 0, end the Tainted Sun (`0x0061C4D0`), Act II clients get `53
   02000000 00000000 00` (`0x0059A170`); state := 4; FX 6; amulet count
   := 0 and count (for each player from the player, `0x0059A770`: holds
   neither `vip ` nor `hst ` and lacks 10.0); this player, if 11.0 is
   clear: set 11.1, 11.13, clear 11.14; for each player from the
   object: same level as the altar and neither 11.0 nor 11.15 → set
   11.13, 11.1 (`0x0059A680`); 11.13 → party members in Act II without
   11.0 and 11.1 get 11.13, 11.1 (`0x0059A0B0`); 11.13 → sound 52
   (`0x0059A730`); game 11.13. Drops: level argument := the altar
   room's level id; drop code `vip `; count times quality 7, droppable
   0; each created item identified and counted; chain 9 amulet count +=
   created; `0x00585B90(op, 4)`; gold (§1.3). Completion flag
   (`0x00599FE0`); timer period 10 (`0x0059A700`: state 4 → status 3 to
   all; +0x06 := 0; returns 1).
4. Intro: mode := 1, +0x08 := 2, +0x04 := 1, amulet count and drops as
   in step 3 (level argument = the altar room's level id), magic
   treasure, gold, chain 9 count. No state, flags, FX or timer.

#### 5.8 Altar init (init 20)

`0x00544910`: chain 10 absent → object mode := 2 unless already 2.
Else `0x0059A3F0`: +0x05 := 1, +0x0C := GUID; if not-intro: (state ≤
1: darken (success → +0x02 := 1, failure → +0x03 := 1), state := 3,
flags := 0); then, still inside not-intro, status = 0 → status 2 to all
(never true in 1.14d, `quests-act2-2.md` §1 item 5). Then mode := +0x08. So the
quest reaches state 3 when the altar's room is first populated.

### 6. A2Q4 Arcane Sanctuary (chain 11, slot 12)

#### 6.1 Extra data (offsets used by 1.14d)

+0x04 Drognan started; +0x08 tome room; +0x0C / +0x0D Jerhyn spawned at
start / at the palace; +0x0E palace open; +0x0F / +0x10 guard-moved
flags; +0x11 harem blocker created; +0x15 Jerhyn position stored; +0x16
portal to the Canyon opened; +0x18 / +0x19 guard positions; +0x1A
player near the blocker, +0x1C its GUID; +0x20 / +0x24 blocker x, y;
+0x28 / +0x2C Jerhyn x, y; +0x30 / +0x34 guard x, y; +0x38 blocker GUID;
+0x3C Jerhyn GUID; u16 +0x40 blocker mode; u16 +0x42 / +0x44 portal
mode in the Sanctuary / in Palace Cellar 3; +0x46 blocker was neutral
when opened.

#### 6.2 Opening the palace (`0x0059AEF0`)

+0x0E := 1; if +0x40 = 0: +0x46 := 1; +0x40 := 2; flag iterate (§6.3);
if +0x11 = 1 and the blocker object (+0x38) exists: mode := 2, free its
collision (`0x00623830`).

#### 6.3 Flag iterate (`0x0059ADB0`)

Players without 12.0 and 12.1: state 2 → 12.2; 3 → 12.3; 4 → 12.4 when
status is 2 or 3, 12.5 when status is 4.

#### 6.4 Chat (event 0, `0x0059B1C0`; active fn `0x0059ACA0`)

- act2guard2 (331, Kaelan): palace closed (+0x0E = 0) → table state 7
  (msg 186); open → one quest-seed step, table state 8 + (lo' mod 3)
  (msgs 187–189). This draws on every chat open.
- Others: 12.1 → 4; GUID listed → 5; else if state ≠ 0, not-intro,
  12.0 clear and (state ≤ 4 or 12.13): index[state] (`0x00738D44`: −1,
  0, 1, 2, 3, 4, 0; ≤ 11).
- Wants to talk: 12.0 clear and: 331 with 12.1 clear and either (+0x40 =
  0 and 12.7 clear) or (+0x40 = 2, +0x46 set, 12.8 clear); drognan
  (177) with state 1, or jerhyn (201) with state 2, each with 12.1 and
  12.0 clear.

#### 6.5 Messages (event 11, `0x0059AF50`) and chat end (event 2, `0x0059AE60`)

| From | Msg | Effect |
|---|---|---|
| 331 | 186 | set 12.7 |
| 331 | 187–189 | set 12.8 |
| any other | 397–407 | refresh; if 12.1: clear 12.1, add GUID |
| any other | 396 | if not-intro and status < 5: status 5 (silent). If +0x16 = 0 and the player's room is the tome room (+0x08): free spot from the player's position (`0x00545340`, size 2, mask 0xBE11, radius 8, limit 100); if found and a portal object (class 60) to level 46 (Canyon of the Magi) is created (`0x0056D130`): +0x16 := 1, game 12.13 |
| drognan 177 | 373 | state := 2; +0x04 := 1; open the palace (§6.2); refresh |
| jerhyn 201 | 377 | state := 3; refresh; status 3 to all; flag iterate |

Event 2: drognan with +0x04 = 1: status 2 to all; +0x04 := 0; clear
callback 2; flag iterate.

#### 6.6 Level changes (event 3, `0x0059F0C0`)

- New level 74 (Arcane Sanctuary): state < 4 → state := 4; status < 4
  → status 4 to all, then the flag iterate; status ≥ 4 with the state
  already ≥ 4 → nothing; otherwise the flag iterate.
- New level 50 (Harem Level 1): set 12.8, 12.7.
- Old level 40: the Jerhyn start / palace handling (§6.10); quick
  remove; if the player lacks 12.0, 12.1 and state = 3: state := 4,
  flag iterate.

#### 6.7 Horazon's journal (operate 42, `0x0059B970`)

If the object's mode is 0: mode := 1, end-animation event at frame +
(objects `FrameCnt1` >> 8) (`0x005417D0` type 1). With chain 11: send
the player the scroll text 396 (`0x005456A0`: an S→C 0x27 of type 2;
bytes: Open question 4); +0x08 := the tome's room; only if not-intro
and state ≠ 5 (no grants in intro games): state := 5; for each player: in level 74 without 12.0 and
12.1 (`0x0059B3F0`): set 12.13, 12.1, 12.0, clear bits 2–11
(`0x0065C3E0`), set 12.8, 12.7, and their party members in Act II
without 12.0 and 12.1 get the same six changes (`0x0059B360`); then the
completion flag (`0x0059B320`, 12.14 and `5D 0B 00 0C 0000`) and
`0x0059B940` (reads 12.13, changes nothing). The quest is granted at
once; the 397–407 talk only clears 12.1. The client answers 396 with
C→S 0x31 (§6.5) to open the portal.

#### 6.8 Harem blocker

- Created by `0x0059B710` (object event 7 dispatch `0x005449E0`): if
  +0x0E = 0 and +0x11 = 0: +0x40 := 0; position (x − 2, y − 1) of the
  dispatching unit; create object 318 (`0x00555230`, flags 1, 0, 0);
  failing that at (x − 2, y); on success: unit flags |= 0x3000000, +0x11
  := 1, +0x38 := GUID.
- Init 30 (`0x0059B7D0`): mode := +0x40; +0x38 := GUID; mode 2 → free
  collision.

#### 6.9 Sanctuary portal

- Init 29 (`0x0059BA40`): in level 74 mode := +0x42, in level 54 mode
  := +0x44; if that value was 1 it becomes 2 and an end-animation event
  is set at frame + (`FrameCnt1` >> 8) + 1. Exactly (`0x0059BA6B`–
  `0x0059BAE6`): chain 11 absent → nothing; the object's mode is set to
  the stored value first; only the stored u16 becomes 2 (the object stays
  in mode 1 until its ENDANIM event, `world/objects-2.md` §18.6). Other
  levels: nothing.
- Operate 34 (`0x005846B0`, object spec) calls `0x0059BAF0(level)`:
  level 74 with +0x44 = 0 → +0x44 := 1, +0x42 := 2; level 54 with +0x42
  = 0 → +0x42 := 1, +0x44 := 2.

#### 6.10 Jerhyn and the palace guard (spawn side; AI behaviour: AI spec)

Moved to `quests-act2-2.md` §2 (1.14d re-read for QB-16): init 18
`0x0059F380` spawns the start Jerhyn and is the only writer of +0x3C;
init 19 `0x0059F440` spawns Kaelan (331) at (x + 1, y), schedules the
object event 7 that creates the harem blocker, and calls the palace
spawn `0x0059EF70(record, &point, room)` from the palace-Jerhyn object's
position; event 3 calls it from the harem blocker's position. The
offsets (x + 15 / x − 10, y − 3) apply to that base point; the spawn is
mode 1 twice (spread −1, then 2). `0x0059F510` is a predicate (§10), not
a caller. AI-facing hooks: §10.

#### 6.11 Game start (event 13, `0x0059B530`)

11.0 → palace open (+0x0E := 1, +0x40 := 2). 12.0 or 12.1 → game 12.13,
palace open, stop. Holds `hst ` → status 1, state 1. Then 12.5 → status
4, state 4; else 12.4 → status 3, state 4; else 12.3 → 3, 3; else 12.2
→ 2, 2; else skip the next step. Palace open. Finally 10.0 → palace
open.

### 7. A2Q5 The Summoner (chain 12, slot 13)

#### 7.1 Extra data

+0x00 killed; +0x01 seen; +0x04 kill room; +0x08 timer exists; +0x09
timer phase.

#### 7.2 Rules

- Summoner AI hook (`0x0059C330`, from `0x005F85ED`): not-intro: +0x01 :=
  1; state 0 → state := 1; status < 2 → status 2 to all, then players
  without 13.0 and 13.1 get 13.2 when state = 1 (`0x0059BD10`).
- Chat (event 0, `0x0059BBC0`): 13.1 → table state 1; GUID listed → 2;
  intro or state 0 → nothing; 13.0 without 13.13 → nothing; state > 1
  without 13.13 → nothing; else index[state] (`0x0073B278`: −1, 0, 1, 2;
  ≤ 3). Wants to talk (`0x0059BB40`): 13.1 and NPC in {176, 175, 199,
  177, 202, 244, 210, 201, 200, 178} (not greiz 198).
- Kill (event 8, `0x0059C150`): if not-intro: state := 2; no timer yet
  → +0x08 := 1, timer period 3 (`0x0059BFD0`); +0x09 := 0; +0x00 := 1;
  +0x04 := victim room; for each player from the victim: kill room or
  adjacent (as §3.7) and neither 13.0 nor 13.1 → 13.13, 13.1
  (`0x0059BE70`); 13.13 → party members in Act II without 13.0, 13.1 get
  13.13, 13.1 (`0x0059C0A0` → `0x0059C020`); completion flag
  (`0x0059C0F0`). Always (also intro): FX 7.
- Timer: first run (+0x09 = 0): +0x09 := 1; sound 51 to players with
  13.13 in level 74 (`0x0059BF80`); returns 0. Second run: status 4 to
  all; +0x08 := 0; returns 1.
- Messages 419–429 from any NPC (event 11, `0x0059BD70`): if 13.1: if
  13.13: status 13 to all, game 13.13, state := 3; add GUID; set 13.0,
  clear 13.1. Refresh (always for these messages).
- Event 3 (`0x0059C200`): not-intro, new level ≥ 40 and old level 40 →
  quick remove. Event 13 (`0x0059C220`): neither 13.0 nor 13.15 and
  13.2 → status 2, state 1.
- The quest-chain link hook for monster 250 (`quests.md` §4.6,
  `0x0059C3B0`) is a bare `ret`.

### 8. A2Q6 The Seven Tombs (chain 13, slot 14)

#### 8.1 The true tomb

At DRLG creation for Act II (`0x00642DA0`, DRLG spec): repeat {staff :=
(DRLG-seed step) lo' mod 7; boss := (next step) lo' mod 7} until staff ≠
boss; drlg +0x94 := 66 + staff (tomb holding the orifice), +0x484 := 66
+ boss (tomb holding Duriel's lair entrance; D2MOO names it the boss
tomb). `0x0061AEB0(act)` reads +0x94 (`0x00642230`; 0 without a DRLG).
This spec only reads it: lazily into extra +0x34 (§8.4), in 0x50 (§8.10)
and in the arcane-object list (§8.9).

#### 8.2 Extra data

+0x00 status timer; +0x01 Duriel killed; +0x03 object timer active;
+0x04 orifice seen; +0x05 Tyrael's door seen; +0x06 init-37 object seen;
+0x08 / +0x09 / +0x0A chat-end pending for Jerhyn's start talk / Tyrael
/ Jerhyn's end talk; +0x0B lair entrance open; +0x0C staff already
handed in (10.0 at game start); +0x0D objects need update; +0x0E staff
items removed; +0x0F portal to Lut Gholein opened; +0x10 staff missing,
+0x14 its status (8 or 9, §4.10); +0x18 Tyrael's door mode; +0x20
orifice GUID; +0x28 init-37 object GUID; +0x2C door GUID; +0x34 staff
tomb level; +0x38 arcane list made, u16 +0x3A next index, +0x48 six
object ids (§8.9); +0x3C portal opening; +0x3D completed before; +0x40,
+0x44 Tyrael distance scratch; +0x60 Duriel's room.

#### 8.3 Chat (event 0, `0x0059C3C0`; active fn `0x0059D300`)

- tyrael1 (251): table state 2 only when the door mode +0x18 = 2.
- With 14.13: atma 176 / warriv2 175 / drognan 177 / lysander 202 /
  cain2 244 / fara 178 → table state 6 when 14.6 / 14.7 / 14.8 / 14.9 /
  14.10 / 14.11 is clear (stop).
- Else 14.3 → 3. 14.4 → 5 for meshif1 (210), else 4. GUID listed → 4.
  State 0 → nothing. 14.0 without 14.13 → nothing. State > 3 without
  14.13 → nothing. Table state = index[state] (`0x0073B8A8`: −1, 0, 1,
  2, 3, 4); index 1 for drognan only when game 12.13 is clear; −1 or > 7
  → nothing.
- Wants to talk: jerhyn 201: 14.0 clear and ((state 1, 14.3 and 14.4
  clear) or 14.3); meshif1 210: 14.4; tyrael1 251: not-intro, Duriel
  killed, portal not opened; with 14.13: the six NPCs above while their
  bit is clear.

#### 8.4 Level changes (event 3, `0x0059D1C0`)

Only for new levels 40–74. Old level 40 → quick remove. Intro → stop.
Flag iterate here = `0x0059C6C0` for the moving player only: unless 14.0,
state > 1 → set 14.2.

- Duriel alive: staff tomb := +0x34, else read and store §8.1; none →
  stop. New level = staff tomb: state 2 → stop; state := 2; status ≤ 1 →
  status 2 (silent); flag iterate. New level 46 (Canyon): state 0 →
  state := 2 and flag iterate; status 0 → status 1 to all; stop. Any
  other level falls through to the next item.
- (Duriel dead, or the item above fell through.) New level 73
  (Duriel's Lair) with state ≤ 1: state := 2; status > 1 → stop; status
  2 (silent); flag iterate.

#### 8.5 Status function (`0x0059CA50`, always true)

7.0 clear → 0. +0x10 → +0x14. 14.0 → 0. 14.3 → 5. 14.4 → 6. Intro →
0. 14.5 → status byte. State < 3 → status byte. Else 12.

#### 8.6 Orifice (operate 25 `0x0059DC70`, C→S 0x44)

- Operate, object mode 0: if the player is not busy (`0x00535060` ≠ 1)
  and holds `hst `: interact unit := (2, orifice GUID) (`0x00554120`),
  mode := 1, S→C 0x58 (`0x0053D8D0`, the insert dialog), return 0;
  without `hst `: sound 19, return 1. Mode 1 and the orifice is the
  player's interact unit: reset it (`0x00554190`), mode := 2, return 0.
- C→S 0x44 (`0x0054C380` → `0x005852E0`, object spec; full path and the
  0x58 layout: `quests-act2-2.md` §3): action 3 on the orifice (class
  152) with a cursor item that is not `hst ` → 0x58 result 4. With `hst `
  (no `0x0055EEA0` call for the orifice): reset the interact unit, 0x58
  result 5 with byte 6 = 1; orifice mode := 1 then 2 and `0x0059DD80`
  (§8.7). Action 2: result 1, mode 0. A busy player's operate returns 1
  with no sound.

#### 8.7 Handing in the staff (`0x0059DD80`)

Set 10.0, 10.13; delete `hst `, `vip `, `msf ` (`quests.md` §9.2); party
members in Act II without 10.0 get 10.0, 10.13 and, unless trading
(`0x005678A0`), lose the same three items (`0x0059DBD0`); FX 3; +0x0D :=
1, +0x0E := 1; no object timer yet (+0x03): timer of period (R − 75) /
20 (signed division, R = `Range` of `missiles.txt` row 338
`horadricstaff`; read only when the table has ≥ 339 rows), callback
`0x0059D870`, +0x03 := 1; orifice mode := 1; clear the has-portal flag
(`0x0061AED0`) of the orifice's room and of the room at its position + 3
(y). Chain 13's record is read without a null test (`0x0059DE53`) after
the bits, item deletions, party step and FX: absent → null read (crash);
unreachable in 1.14d (every record exists from game start, `quests.md`
§2). Likewise `missiles.txt` with ≤ 338 rows reads through a null row
pointer (`0x0059DE1C`), not a default; live tables have more rows.

#### 8.8 Lair objects

- Object timer (`0x0059D870`): if +0x0D = 1: (a) init-37 object (+0x06,
  GUID +0x28) in mode 0: mode 1 plus end-animation event when +0x0C = 0,
  else mode 2; (b) orifice (+0x04, GUID +0x20): create object 100
  (Duriel's Lair entrance, `0x00555230`) at (orifice x − 13, y + 3) in
  the room there; created: mode 1 and set has-portal on both rooms when
  +0x0C = 0, else mode 2. Neither (a) nor (b) → return 0 (retry next
  period, +0x03 stays 1). Else +0x0B := 1, +0x0D := 0. Then +0x03 := 0,
  return 1.
- Orifice init 21 (`0x0059DB50`): +0x04 := 1, +0x20 := GUID; if +0x0C,
  no timer and +0x0B = 0: timer period 1 (same callback), +0x03 := 1,
  mode 2; else mode 2 when +0x0B.
- Init 37 (`0x0059DA50`): +0x06 := 1, +0x28 := GUID; mode 2 when intro
  or +0x0B or +0x0D, else 0.
- Tyrael's door init 38 (`0x0059DAD0`, object 153): +0x05 := 1, +0x2C :=
  GUID; mode := 2 when intro, else +0x18.
- Warp check `quests.md` §8.2 (`0x0059DB20`): level 73 is closed while
  not-intro and +0x0B = 0.

#### 8.9 Arcane Sanctuary dummy objects (`0x0059D830` → `0x0059D720`, from `0x0054F439`)

Base list (`0x00738FAC`): 313, 312, 308, 310, 311, 309, 307 for tombs
66–72. First use: copy the six entries whose index ≠ staff tomb − 66, in
order, to +0x48. Each call: index 6 → 0; return entry[index], index +=
1. Staff tomb unknown, or chain 13 absent → 307. A staff tomb outside
66–72 skips no entry: the copy loop runs 7 times and stops copying at
six, so +0x48 holds the first six entries 313, 312, 308, 310, 311, 309
(`0x0059D756`–`0x0059D784`).

#### 8.10 Clue item 0x50 (`0x0059D6A0`, `quests.md` §9.4)

`50 0D00 <u16 staff tomb − 66> …` via `0x0053D7E0`; also stores +0x34.
Its trigger code `trs ` is not an item code in the 1.14d tables
(`misc.txt`), so this path never runs.

#### 8.11 Messages, kill, chat end and start

- Event 11 (`0x0059CB20`):
  - tyrael1 251 msg **302**, not-intro, +0x0F = 0: +0x3C := 1; a portal
    object (class 59) to level 40 at the player's position in the
    player's room (`0x0056D130`); created → state := 4; for each player
    from Tyrael: in level 73 without 14.13, 14.3, 14.4 → set 14.13,
    14.3 and character progression for Act II (`0x00538680(client, 2,
    difficulty)`; save spec) (`0x0059C860`); players with 14.13 → their
    party members (`0x0059C9A0` → `0x0059C920`: chain 13 present, member
    lacks 14.0, 14.3, 14.4 and is in Act II, any level, own 14.13 not
    tested: the same 14.13, 14.3 and progression via `0x0059C810`;
    `quests-act2-2.md` §1 item 17);
    completion flag (`0x0059C9F0`: lacks 14.0, 14.3, 14.4 → 14.14 and
    `5D 0D 00 0C 0000`); +0x0F := 1, +0x09 := 1, callback 2 :=
    `0x0059C760`. +0x3C := 0.
  - jerhyn 201 msg **430**: refresh; state := 2; +0x08 := 1; callback 2
    := `0x0059C760`; call seq(10).
  - jerhyn 201 msg **442** with 14.3: 14.13 → state := 5; +0x0A := 1;
    callback 2 := `0x0059C760`; set 14.4, clear 14.3; refresh.
  - meshif1 210 msg **450** with 14.4: 10.0 → delete `hst `, `vip `,
    `msf `; 14.13 → game 14.13, state := 5, status 13 to all; set 14.0;
    send flags; clear 14.4; add GUID.
  - Any NPC: **444** → 14.9, **445** → 14.6, **446** → 14.7, **447** →
    14.11, **449** → 14.8, **452** → 14.10.
- Duriel's death (event 8, `0x0059D050`): if not-intro: state := 3;
  timer period 8 (`0x0059CEE0`: status ∉ {3, 4, 5} → status 3 to all;
  +0x00 := 0; returns 1); a killing player without 14.0, 14.3, 14.4,
  14.5 gets 14.5 and his party (`0x0059CF20`: member lacks 14.0, 14.3,
  14.4, 14.5 and is in Act II → 14.5), then a call of the stub
  `0x00545990` (`ret 4`). Always: clear callbacks 2 and 8; +0x01 := 1;
  +0x60 := victim room; FX 8; players in level 73 without 14.0, 14.3,
  14.4, 14.5 get 14.5 and their party (`0x0059CFB0`); door (+0x05, GUID
  +0x2C): mode 1 plus end-animation event; +0x18 := 2.
- Chat end (`0x0059C760`): tyrael1 with +0x09: status 4 to all, +0x09 :=
  0, clear callback 2. jerhyn: +0x08 → status 1 to all, +0x08 := 0;
  else +0x0A → status 6 to all, +0x0A := 0; else stop; then clear
  callback 2 and the flag iterate for every player (`0x0059C710`).
- Event 13 (`0x0059D4F0`): clear 10.9; 10.0 → +0x0C := 1, +0x0D := 1;
  14.0 or 14.15 → +0x3D := 1; else 14.2 → status 1, state 2; 14.3 → 5,
  5; 14.4 → status 6, state 5. Event 14 (`0x0059D4D0`): clear 10.9.
- Portal check `quests.md` §8.3 (`0x0059DFD0`): while +0x3C = 1 the
  destination is the Act II spawn location of type 12 in level 40
  (`0x0061B060(act, 40, 12, …, 3)`, `0x0052D0F0`), then a free spot
  (`0x0064E7E0`, size 3, mask 0xBE11, radius 7).

### 9. Act II gossip and intro records

| Record | Rules |
|---|---|
| A2Q0 Jerhyn (chain 7, slot 8) | Event 0 (`0x005986B0`): jerhyn 201 with 8.0 clear → table state 0 (msg 253); cain2 244 with 4.14 and not in the extra GUID list → table state 1 (msg 125) and `0x005940A0` (Act I hook, `quests.md`). Event 11 (`0x00598640`): 253 from 201 → 8.0, game 8.13; 125 from 244 → add GUID (extra list). Event 10: remove. Event 13: 8.0 → game 8.13. Wants to talk: 201 with 8.0 clear. Status fn: false |
| A2Q7 guard (chain 26, slot 30) | Event 0 (`0x0059E140`), NPC act2guard4 (377). 30.0 clear: if 9.0, 9.13, 9.1 or game 9.13 → inline step of the player unit seed, table state (lo' mod 3) + 2; else chain 8 intro → roll(player seed, 3) + 2; else 30.13 clear → roll(player seed, 2); else inline as above. 30.0 set → inline. Event 11 (`0x0059E0E0`, NPC 377 only): msg 59 or 60 → 30.13 and callback 2 := `0x0059E0B0` (inert, `quests-act2-2.md` §1 item 3); any other message → 30.0. Event 8: `ret`. Wants to talk: 377 and ((30.0, 30.13 clear) or (9.13 and 30.0 clear)) |
| A2Q8 guard (chain 27, slot 31) | Event 0 (`0x0059E3F0`), act2guard5 (378): chain 13 not-intro, state < 2, chain 10 absent or intro or state ≥ 4, 14.1 and 14.0 clear → table state 0 (msg 303). Event 11 (`0x0059E3C0`): 303 from 378 → 31.0. Wants to talk: 378, chain 13 not-intro with state < 2, game 11.13, 14.1 and 14.0 clear |
| Act II intro (chain 38) | Event 0 (`0x005984C0`): special class per NPC: meshif1 0 (amazon), drognan 1 (sorceress), elzix 2 (necromancer), fara 3 (paladin), geglash 4 (barbarian); warriv2, greiz, lysander none. If the NPC's intro bit is clear (`0x005723C0`): table state 1 when the player's class matches, else 0. Atma and other NPCs: nothing. Event 11 (`0x005983E0`): messages 190, 203/204, 215, 230/231, 241/242, 263/264, 274, 285/286 from their NPC set the intro bit (`0x00572360`) |

### 10. Hooks called from other systems

| Caller | Hook | Effect |
|---|---|---|
| Radament AI `0x005F2B10` | `0x00599420` | §3.6 |
| Summoner AI `0x005F85C0` | `0x0059C330` | §7.2 |
| palace guard AI `0x005E7130` | `0x0059B6E0` | +0x0F = 1 and +0x10 = 0 → +0x10 := 1, true |
| palace guard AI `0x005E7590` | `0x0059B8B0`, `0x0059B8F0`, `0x0059AEC0` | guard at end position (+0x18, +0x19 clear and +0x40 = 2; true without chain 11); guard target (+0x30, +0x34, y − 4 with +0x19); blocker open (+0x40 = 2) |
| Jerhyn / palace NPC logic `0x0059F580` | `0x0059D7C0`, `0x0059D7E0`, `0x0059B820` | chain 13 not-intro with state < 2 → 0 else 1; not-intro with state 1; a player without 14.0, 14.1 within 30 of the blocker (`0x005DC5C0` < 31): +0x1A := 1, +0x1C := GUID. Exactly: +0x1A := 0 and +0x1C := 0 first (`0x0059F75F`), then the player walk `0x005537D0` (`sim/unit-order.md` §2 r5) with `0x0059B820`, which returns 1 for the first qualifying player (distance unsigned ≤ 30 to the stored blocker point +0x20 / +0x24) and so stops the walk: the **first** such player in walk order is stored |
| `0x0059F510` | `0x0059DFB0` | chain 13 not-intro with state < 4 → 0, else 1 |
| Tyrael AI `0x005E73A0` | `0x0059DF50`, `0x0059C750` | +0x3D → true; +0x0F → true when no living player is within 12 (`0x006416D0`); else false. Exactly (2026-10-07): chain 13 record absent → false; +0x3D ≠ 0 → true; +0x0F ≠ 0 → +0x40 := Tyrael, +0x44 := 0, then for every player without state 7 (`0x005538D0`, callback `0x0059DF30`): d := `0x006416D0(player, Tyrael)` < 12 → +0x44 := 1; result = (+0x44 = 0). `0x006416D0` is the size-adjusted distance of two units (`missiles/missiles.md` §R9.5 item 4) and takes no radius: a host seam bound to it is a distance (`distance_between(a, b)`), and the "< 12" and the living-player loop belong to the caller (a `living_player_within(unit, radius)` seam must be built from both, not bound to `0x006416D0` alone). `0x0059C750` runs the §8.11 flag iterate for all |
| monster class hook `0x005447A0` | `0x0059B6C0`, `0x0059B6D0` | jerhyn / act2guard2: bare `ret` |
| cube (`world/cube.md` §8) | `0x0059E5C0` | §4.9 |
| item use | `0x0055E170` | §3.8 |
| object event 7 `0x005449E0` | `0x0059B710` | §6.8 |
| act load `0x0053AC70` | `0x0059AC40` | §5.3 |

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| message-state index tables | chain 8 `0x007398FC`, 10 `0x0073A620`, 11 `0x00738D44`, 12 `0x0073B278`, 13 `0x0073B8A8` | image (§3–§8) |
| arcane dummy object ids | 313, 312, 308, 310, 311, 309, 307 | `0x00738FAC` |
| levels | 40 Lut Gholein, 44 Lost City, 45 Valley of Snakes, 46 Canyon of the Magi, 49 Sewers 3, 50 Harem 1, 54 Palace Cellar 3, 66–72 Tal Rasha's tombs, 73 Duriel's Lair, 74 Arcane Sanctuary | `levels.txt` |
| objects | 59, 60 portals; 100 lair entrance; 149, 152, 153, 298, 318, 354–357 | `objects.txt` |
| NPCs | 175 warriv2, 176 atma, 177 drognan, 178 fara, 198 greiz, 199 elzix, 200 geglash, 201 jerhyn, 202 lysander, 210 meshif1, 229 radament, 244 cain2, 250 summoner, 251 tyrael1, 331 act2guard2, 377 act2guard4, 378 act2guard5 | `monstats.txt` hcIdx |
| sounds | 19 refused, 50 Radament, 51 Summoner, 52 Tainted Sun | §3.7, §5.7, §7.2 |
| FX bytes (0x89) | 3 orifice, 6 altar, 7 Summoner, 8 Duriel | §1.1 |
| timer periods (updater ticks) | 12, 10, 15–16, 3, 8, 1, (R − 75)/20 | §3.7, §5.3, §5.7, §7.2, §8.7, §8.8, §8.11 |
| `horadricstaff` range R | 440 in 1.14d `missiles.txt` (period 18) | live table |

## Randomness

Quest seed (control +0x18) draws, in occurrence order of their events:

| When | Seed | Draws | Decides |
|---|---|---|---|
| Act II DRLG creation | DRLG seed | 2 per try, until different | staff and Duriel tombs (§8.1) |
| entering level 44/45, Tainted Sun state 0, first time | quest | 1 (`lo' & 1`) | darken delay 15 or 16 |
| each quest chest / altar opening | object TC and item draws (object, item specs), then quest 1, then gold drops' own draws | `lo' mod 5` | gold piles 5–9 |
| Kaelan chat with the palace open | quest | 1 (`lo' mod 3`) | line 187–189 |
| A2Q7 guard chat | player unit seed (+0x20) | 1 (`roll(3)`, `roll(2)` or inline `lo' mod 3`) | line |

Quest items (`ass`, `tr1`, `msf`, `box`, `vip`) and gold draw from
their own item seeds. No other Act II quest code draws.

## Edge cases & original bugs

1. A player whose Radament reward was granted but who no longer holds
   the Book of Skill while 9.5 is still set has 9.0, 9.1, 9.15, 9.5
   cleared at game start (§3.9): he can earn the book again, but only if
   he creates the game, and the game record is switched off for that
   game anyway (`quests.md` §3).
2. The Book of Skill count (§3.7) counts every player with 9.5 who lacks
   a book, including players far away; all books drop at Radament.
3. Staff-piece pick-ups write the record's status byte (§4.8): the
   value last written by any player is what the default status reads.
4. The staff chest increments the Staff-of-Kings count (+0x18); D2MOO
   1.10f increments the cube count instead and names +0x28 "scroll
   dropped".
5. Kaelan's table state 6 (message 185) is never selected by 1.14d code.
6. Atma has rows in the Act II intro table but the intro record ignores
   her (§9).
7. Tainted Sun's wants-to-talk list omits fara; the Summoner's omits
   greiz (§5.5, §7.2).
8. The altar stores the altar room's level id in the item-level
   variable before the amulet drops (§5.7), but `0x00559A30` overwrites
   it unread (an out parameter, `quests-act2-2.md` §1 item 20).
9. Tainted Sun reaches state 3 on the altar room's first population
   (§5.8), not on entering a level.
10. The orifice timer reads `missiles.txt` row 338 without a range check
    beyond the row count test; a table with fewer than 339 rows makes it
    read a null row (crash).
11. `0x0059D6A0` (true-tomb 0x50) is unreachable: no `trs ` item exists.
12. Duriel's death clears callbacks 2 and 8 even in an intro game; a
    later Duriel spawn raises nothing.
13. `0x00545990` (D2MOO `D2Game_10034_Return(61)`) is a `ret 4` stub.
14. The Summoner's party iterate sets A2Q5 bits in 1.14d (`0x0059C020`);
    D2MOO 1.10f calls the A2Q4 iterate there.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Radament chat, record state 3, 9.0/9.1 clear, 9.13 clear, not GUID listed | table state 2 (msgs 315–324; Atma 317) | §3.2, index `0x007398FC` |
| Radament chat, state 4, 9.13 clear | nothing | §3.2 |
| Radament killed; 3 players: A in the kill room, B adjacent, C in town (party of A); all lack 9.0/9.1, none holds `ass ` | A, B, C get 9.13, 9.1, 9.5; 3 books drop; A, B, C get sound 50 | §3.7 |
| Cain selector: holds `box ` (10.6 set), `vip ` (10.4 clear) | out 9 then 1 → true → table state 1 | §4.3 |
| Cain selector: holds `tr1 ` only, 10.3 set | out 6, false → chat: 10.3 set → table state 6 | §4.3, §4.4 |
| staff status, nothing held, 10.0/10.1 clear, 7.0 set, cube dropped, cube count 0, 10.3 clear | 9 | §4.6 |
| same with 10.3 set | 2 | §4.6 |
| quest seed {12345, 666}, chest opened | lo' 22752887 → 7 gold piles | §1.3; seed values as `npc.md` test vectors |
| quest seed {12345, 666}, darken timer | lo' & 1 = 1 → period 16 (first fires 17 updater ticks later) | §5.3, `quests.md` §5 |
| quest seed {12345, 666}, Kaelan with palace open | lo' mod 3 = 2 → table state 10 (msg 189) | §6.4 |
| DRLG seed {12345, 666} | lo' mod 7: 3, 3 (retry), 3, 6 → staff tomb 69, Duriel tomb 72; 0x50 tomb field 3 | §8.1 |
| staff tomb 69, five arcane calls | 313, 312, 308, 311, 309 (index 3 = 310 skipped); 7th call wraps to 313 | §8.9 |
| live `missiles.txt` row 338 range 440 | orifice timer period 18 | §8.7, game-file test (`#[ignore]`) |
| altar opened with Act II loaded | Act II clients get `53 02000000 00000000 00`; every player `89 06` after a 0x28 | §5.7 |
| darken with Act II loaded | Act II clients get `53 05000000 00000000 01`, `5d 0a 10 00 0000` | §5.2 |
| A2Q6 status: 7.0 set, staff missing, game type 3 | 8 | §8.5, §4.10 |

## Provenance

- Read from the 1.14d `Game.exe` exports (`re/exports/funcs`,
  `functions.tsv`, `all.asm`) and `tools/ghidra/disasm.py` (register
  arguments: status values of `0x00544300`, states of `0x00544350`,
  chains of `0x00543640`, timer callbacks and periods of `0x00543F10`,
  FX bytes of `0x00545760`, sound ids of `0x00553380`, object classes of
  `0x00555230` / `0x0056D130`); raw image bytes for the index tables,
  the arcane list and the object operate / init tables. Assert strings
  `d:\diablo2\…\Quests\a2q0.cpp` … `a2q8.cpp` identify the files. Live
  1.14d `levels.txt`, `objects.txt`, `monstats.txt`, `misc.txt`,
  `weapons.txt`, `missiles.txt` (extracted patch_d2 tables) gave the row
  names.
- D2MOO 1.10f `D2Game/src/QUESTS/ACT2/A2Q0–A2Q8.cpp`, `Quests.cpp`,
  `ObjMode.cpp`, `DrlgDrlg.cpp` gave names and structure; each rule
  above was matched to the 1.14d function named next to it. Differences
  found: edge cases 4, 14; Radament's link hook is a stub; the 1.14d
  Summoner kill-room test matches rooms correctly; chest init functions
  are empty; Duriel's `0x00545990` call is a stub; the Tainted Sun timer
  uses `0x004BC500`; the 1.14d altar init sets state 3; Jerhyn's 430
  calls chain 10's own sequence function.
- No packet or RNG recording of Act II exists yet.

## Open questions

1. Status meanings 1–13 per Act II quest (client quest log): settle with
   `quests.md` open question 1. **Answered** (2026-10-07): `world/quests-status.md`: the client (0x52 `0x0045CC00` → `0x004A40D0` stores the list; row build `0x004A1950`, tables `0x00723F30` and the per-quest status tables) maps each status to a description string id, a replay speech id and an icon state (§4, §5); Act II tables §8; Seven Tombs rows 5–7 and the tomb symbol §4 rule 2, §5 rule 6.
2. `0x00545850` (quest-chest gate) and `0x00585B90` (chest treasure):
   their exact tests and draws belong to the object spec; until written,
   record one chest opening (packets + RNG). **Answered** (2026-10-07):
   the gate is `quests-act2-2.md` §5.1 (mode test, `Mode1` mode 1 + ENDANIM
   at f + `FrameCnt1` >> 8 or mode 2, flag 0x2 cleared; no draw); the
   chest treasure is `items/treasure.md` §4. A chest recording would
   still confirm the order (rec item, not blocking).
3. Act II light change (`0x0061C450` / `0x0061C4D0`) effect and 0x53
   field names: environment spec; record a Tainted Sun start.
   **Answered** (2026-10-07): the effect on the server act environment
   record is `quests-act2-2.md` §5.2 (start: speed 4, index 0, eclipse,
   ticks 1,200; end: speed 128, index 2, ticks 0, eclipse off); the 0x53
   fields are `render/lighting.md` §9.2 r2 (u32 period index, u32 ticks,
   u8 eclipse). A recording would still confirm the client view.
4. Bytes of the 0x27 type-2 scroll text sent by `0x005456A0` (tome
   message 396). **Answered** (2026-10-07): `quests-act2-2.md` §5.4 (type
   2, object GUID, count 1, kind 0, string id u16@10; bytes 7, 9, 12–39
   unwritten, `sim/intents-events.md` §3.5).
5. ~~Chain 38's intro storage~~ Answered: they act on field A (record
   +0) of the player's NPC record, `quests.md` §6.7's pair on field B
   (record +4); `quests-act2-2.md` §1 item 15 (`0x00572360`,
   `0x00572420`).
6. ~~Altar level-id argument~~ Answered: an out parameter, overwritten
   at `0x00559AF8` (`quests-act2-2.md` §1 item 20).
7. ~~`0x00538680(client, 2, difficulty)`~~ Answered:
   `quests-act1-rest.md` §5 (client +0x0A bits 8–12, step 2).
8. Init 37 (`0x0059DA50`) has no `objects.txt` user in 1.14d; confirm no
   preset spawns an object through it. **Answered** (2026-10-07): the init table
   `0x00731BC0` is read only by `0x0054F5D0` (`0x0054F6A7`,
   `0x0054F6E6`), indexed by the `objects.txt` record's `InitFn`
   (+0x1B1); entry 37 (`0x00731C54`) is the only pointer to `0x0059DA50`
   and no rel32 call reaches it (`disasm.py xref`); no live 1.14d row
   (d2data, d2exp, patch_d2) has `InitFn` 37. So no object runs it.
9. `quests.tsv` column `spec` still says `catalogued` for Act II rows;
   switch it to `specified` (with a link to this file) once
   `quests.md` §2.4 documents a second owner file. **Answered** (2026-10-07):
   §2.4 names the owner per act; rows 8–16 and 38 switched to
   `specified` (quests-fixups CODE-TABLE commit); the row addresses part
   1 does not name are in `quests-act2-2.md` §4.
10. Record a full Act II run (packets + RNG, `docs/HANDOFF.md` §5) to
    confirm message order: chat-end status, kill timers, Tyrael's
    portal, Meshif's completion.
11. QB-1 (test vector 1 messages): Answered, typo fixed (315–324);
    `quests-act2-2.md` §1 item 1.
12. QB-2 (event-10 bodies of chains 7 / 8, `0x005987B0` / `0x00598980`):
    Answered, §1 item 2.
13. QB-3 (NPC class in chains 26 / 27 event 11): Answered, both test it;
    chain 26 installs `0x0059E0B0`; §1 item 3.
14. QB-4 (altar gate): Answered, no `0x00545850` call; §1 item 4.
15. QB-5 (altar init status 0 → 2): Answered, unreachable in 1.14d
    (`0x0059A43C`); §1 item 5.
16. QB-6 (game-start status / state): Answered, byte stores only; §1
    item 6.
17. QB-7 (§5.6 / §6.7 scope): Answered (`0x0059ED63`, `0x0059B9E4`);
    §1 item 7.
18. QB-8 (Summoner +0x09, 13.2 iterate): Answered (`0x0059C1A9`,
    `0x0059C391`); §1 item 8.
19. QB-9 (Tyrael chat): Answered (`0x0059C448`); §1 item 9.
20. QB-10 (msgs 442 / 430): Answered (`0x0059CCFA`, `0x0059CC54`); §1
    item 10.
21. QB-11 (0x44 non-orifice, 0x58 layout): Answered (`0x005852E0`,
    `0x0053D8D0`); §1 item 11, §3.
22. QB-12 (Duriel kill party credit): Answered (`0x0059CF20`); §1 item
    12.
23. QB-13 (`tr1 ` pick-up with 10.3): Answered, no 0x5D (`0x00599AA9`);
    §1 item 13.
24. QB-14 (counts per code): Answered, one per code (`0x00558110`); §1
    item 14.
25. QB-15 (= open question 5): Answered; §1 item 15.
26. QB-16 (Jerhyn's palace spawn): Answered (`0x0059EF70`, `0x0059F380`,
    `0x0059F440`, `0x0059F0C0`); `quests-act2-2.md` §2.
27. QB-17 (Tyrael's party tail): Answered (`0x0059C920`, `0x0059C810`);
    §1 item 17.
28. QB-18 (bodies `0x005940A0`, `0x005985C0`, status fns of 7 / 26 / 27
    / 38): Answered; §1 item 18.
29. QB-19 (§3.6 status ≥ 2 with state < 3; busy orifice): Answered
    (`0x005994C8`, `0x0059DCFC`); §1 item 19.
30. QB-20 (chest drop code, item level): Answered (`0x00599D08`,
    `0x00559AF8`); §1 item 20.
31. Byte 6 of the orifice's S→C 0x58 (results 0, 1, 4) is a stale stack
    byte: Needs recording (`quests-act2-2.md` open question 1).
