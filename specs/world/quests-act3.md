# Spec: World — Quests, Act III (Lam Esen through the Guardian, Act III gossip)

- **Status:** draft: every rule below is read from the 1.14d `Game.exe`
  (decompile exports plus disassembly for register arguments and the
  code reached only through callback pointers; addresses inline). No Act
  III quest recording exists yet; nothing is verified against a trace.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act3`
- **Related specs:** `world/quests.md` (owner of the shared machinery:
  flag records §1, quest records §2, game entry §3, events and dispatch
  §4, updater and timers §5, status messages §6, NPC dialog hooks §7,
  act transitions, warps and the Durance warp §8, quest items and
  helpers §9; this spec uses its terms and never restates them);
  `world/quests-act2.md` (same layout and notation, Act II quests);
  `world/quests-act3-2.md` (part 2: §11 implementation clarifications);
  `world/quests.tsv` (records and callbacks: rows 17–24 and 39),
  `world/quest-messages.tsv` (NPC message tables of chains 14–20 and
  39); `world/cube.md` §8 (the `qf2 ` transmute calls §4.8 here);
  `world/npc.md` (chat, §7.5 quest-granted mercenary); `sim/rng.md` §3,
  §5.2; `sim/tick.md` §5 (object events); `sim/unit-order.md` §7 (player
  iteration order); item, object, monster, AI and save specs (not
  written: item creation, object modes, monster spawning, NPC map AI,
  save progression).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 51–66 |
| Inputs | 67–77 |
| Outputs / state changes | 78–88 |
| Rules | 89–90 |
|   1. Conventions | 91–160 |
|   2. Act III records | 161–188 |
|   3. A3Q1 Lam Esen's Tome (chain 15, slot 17) | 189–272 |
|   4. A3Q2 Khalim's Will (chain 16, slot 18) | 273–388 |
|   5. A3Q3 Blade of the Old Religion (chain 17, slot 19) | 389–495 |
|   6. A3Q4 The Golden Bird (chain 18, slot 20) | 496–584 |
|   7. A3Q5 The Blackened Temple (chain 19, slot 21) | 585–681 |
|   8. A3Q6 The Guardian (chain 20, slot 22) | 682–779 |
|   9. Act III gossip and intro records | 780–841 |
|   10. Hooks called from other systems | 842–862 |
|   11. Clarifications (QC-1 … QC-7) | 863–869 |
| Constants & data dependencies | 870–886 |
| Randomness | 887–900 |
| Edge cases & original bugs | 901–948 |
| Test vectors | 949–966 |
| Provenance | 967–993 |
| Open questions | 994–1016 |
<!-- /index -->

## Summary

Act III has six main quests (chains 15–20, flag slots 17–22) and three
gossip/intro records (chains 14, 28, 39). Each main quest is a quest
record (`quests.md` §2.2) whose callbacks react to NPC chat, scroll
messages (C→S 0x31), level changes, kills, item pick-up, player join /
leave, quest objects (Lam Esen's tome, the Gidbinn decoy and altar,
Khalim's chests, the sewer lever, the Compelling Orb, the Hellgate and
Mephisto's bridge) and hooks from monster creation and NPC map AI. They
set player and game flag bits, drop quest items and gold, grant rewards
(stat points, a ring, a mercenary, a Potion of Life), spawn Hratli,
Natalya, the Gidbinn boss and the Dark Wanderer's minions, and report
status with S→C 0x5D. This spec owns those per-quest state machines,
the Act III quest objects' quest-side logic and the hooks other systems
call into Act III quest code.

## Inputs

| Name | Type | Source |
|---|---|---|
| events 0, 2, 3, 4, 5, 8, 9, 10, 11, 13, 14 | quest callbacks | `quests.md` §4 |
| object operate / init / event-7 calls | operate 28, 31, 44, 45, 53, 57–59; init 23, 25, 39, 41–45, 49, 50, 52, 60 | object spec; tables `0x00732D18`, `0x00731BC0` (§1.4) |
| cube transmute producing `qf2 ` | call | `world/cube.md` §8 |
| monster creation / removal hooks; NPC map AI hooks | calls | §10 |
| quest seed | control +0x18 | `quests.md` §2.1 |
| `misc.txt` codes `bbb`, `qey`, `qhr`, `qbr`, `qf1`, `qf2`, `g33`, `j34`, `g34`, `xyz`, `mss`, `box`, `rin` | tables | §3–§8 |

## Outputs / state changes

Player and game quest bits of slots 15–23 and 32; record state, status
and flags bytes; quest extra data; S→C 0x5D, 0x28, 0x27 (text refresh),
0x29, 0x89; items dropped (`bbb`, `qey`, `qhr`, `qbr`, `qf1`, `box`,
`g33`, `j34`, `mss`, gold), given (`g34`, `xyz`, `rin`) or deleted;
monsters spawned (Hratli, Natalya, fetish11, the Compelling Orb monster);
dummy objects spawned around the Dark Wanderer; object modes; stats
added (5 stat points, 20 maximum life); one mercenary hired;
character act progression for Act III (save spec).

## Rules

### 1. Conventions

#### 1.1 Notation

Same notation as `world/quests-act2.md` §1.1: `s.b` = bit b of slot s in
the acting player's record of the game difficulty; "game s.b" = the game
record (set `0x00544720`, test `0x00544760`, slot in EDX, bit pushed);
"state := n" = `0x00544350`; "status n to all" = flags := 0, then
`0x00544300(record, n, 0, F, 1)` (F = the quest's iterate function, §2);
"status n (silent)" = the same call with iterate 0; "for each player
(from X)" = `0x005537D0`; "party of P" = `0x00540510` when P's party id
(`0x00554630`) ≠ −1; "in Act III" = the player's room's level has act 2
(`0x006427F0`); "refresh" = `quests.md` §7.2; "send flags" = 0x28
(`quests.md` §6.6); "add GUID" / "GUID listed" = the record's player
list (record +0x1C; `0x00545200` / `0x005452C0`); event 10 removes the
player from it (`0x00545530`); "quick remove" = `0x00545310` (leaving
Kurast Docks, level 75); "FX b" = `quests.md` §6.5 (`0x00545760`);
"sound n" = `0x00553380` on the player; "completion flag" = for each
player lacking the listed bits: set s.14 and `0x00545920(player, chain,
0)` (`5D <chain> 00 0C 0000`, `quests.md` §6.3). "Holds `code`" =
`0x00558110` finds the item on the player. "Lam Esen done" = game 17.13.

#### 1.2 Message-list selection (event 0)

As `world/quests-act2.md` §1.2: event-0 callbacks add a table state of
the record's table with `0x00543790(npc, table state)`; table states
are `quest-messages.tsv` row states of the chain. Index tables map
record state to table state (§Constants).

#### 1.3 Sequence chain

Root: chain 18 (`quests.md` §3 step 3). "Call seq(c)" = look up chain
c and call that record's sequence function (the guard tests the calling
record's own +0xF0 with `IsBadCodePtr`, then jumps to the target's).

| Chain | Seq fn | Rule |
|---|---|---|
| 18 Golden Bird | `0x005BA7B0` | state ≠ 5 and not-intro → 1; else r1 = seq(16), r2 = seq(17); return r1 or r2 (0 for an absent chain) |
| 16 Khalim | `0x005B8150` | state ≠ 5 and not-intro: state 0 → state := 1; return 1. Else seq(15) |
| 17 Blade | `0x005B9610` | same as chain 16 (state 0 → 1); else seq(15) |
| 15 Lam Esen | `0x005B7CD0` | state ≠ 5 and not-intro → 1 (no state change); else seq(19) |
| 19 Blackened Temple | `0x005BB410` | state ≠ 7 and not-intro → 1; else seq(20) |
| 20 Guardian | `0x005BCC60` | state 0 and not-intro: state := 1, callback 2 := `0x005BC370` (§8.4); return 1 (also when nothing changed) |

So Khalim's Will and the Blade start (state 1) only when the Golden
Bird is finished or switched off; Lam Esen's quest starts only by level
entry (§3.4); the Guardian starts (state 1) when the chain reaches it.

#### 1.4 Act III quest objects

| Object (`objects.txt` row) | Operate fn | Init fn |
|---|---|---|
| 193 `LamTome` | 28 `0x005B7A60` (§3.6) | 23 `0x00544E30` → `0x005B7310` (§3.6) |
| 251 `gidbinn altar` | — | 39 `0x005B9D40` (§5.7) |
| 252 `gidbinn` (decoy) | 31 `0x005B9B40` (§5.6) | 25 `0x005B9AE0` (§5.6) |
| 341 `Dummy` (Mephisto's bridge) | 4 `0x00585F60` (object spec) | 45 `0x005BCB90` (§8.6) |
| 342 `portal` (Hellgate to Act IV) | 46 `0x00584750` (object spec; `quests.md` §8.1) | 44 `0x005BCBF0` (§8.6) |
| 366 `sewer stairs` | 44 `0x005B84E0` (§4.7) | 41 `0x005B8660` (§4.7) |
| 367 `sewer lever` | 45 `0x005B8530` (§4.7) | 42 `0x005B86B0` (§4.7) |
| 368 `darkwanderer` | — | 43 `0x005BD1F0` (§9.2) |
| 378 / 379 `Dummy` (Hratli start / end) | — | 49 `0x005B70B0` / 50 `0x005B7160` (§9.1) |
| 382 `Dummy` (Natalya start) | — | 52 `0x005BCE80` (§8.7) |
| 386 `Dummy` (stairs R) | 50 `0x00582180` (object spec) | 53 `0x005BBB70` (§7.8) |
| 404 `compellingorb` | 53 `0x005BB980` (§7.7) | 60 `0x005BBBA0` (§7.7) |
| 405 / 406 / 407 `chest` (heart / brain / eye) | 57 `0x005B8860`, 59 `0x005B8A20`, 58 `0x005B8940` (§4.6) | — |

Object event 7 (`quests.md` §9.5) reaches record 16 for class 367
(`0x005B85E0`, §4.7) and record 20 for class 341 (`0x005BCAC0`, §8.6);
class 131 is the Dark Wanderer's minion dummy (§9.2).

### 2. Act III records

Callback addresses are in `quests.tsv`; this table adds what the init
functions store beyond it.

| Chain / slot | Quest | Init | State, init_no, seq_id | Extra (bytes, zeroed) | F | Table |
|---|---|---|---|---|---|---|
| 14 / 16 | A3Q0 Hratli gossip | `0x005B7010` | 0, —, — | 0x1C | — | `0x0073EAA0` |
| 15 / 17 | A3Q1 Lam Esen's Tome | `0x005B7E90` | 0, 4, 19 | 0xA0; +0x00 := 1; list +0x1C reset (`0x00545300`) | `0x005B7340` | `0x0073ECF0` |
| 16 / 18 | A3Q2 Khalim's Will | `0x005BDA30` | 0, 0, 15; **status := 1** | 0xB0; list +0x2C reset | `0x005B7F90` | `0x0073F1A0` |
| 17 / 19 | A3Q3 Blade of the Old Religion | `0x005B9850` | 0, 4, 15 | 0x30 | `0x005B8BF0` | `0x0073FB98` |
| 18 / 20 | A3Q4 The Golden Bird | `0x005BAA70` | 0, 0, 16 | 0x1C; +0x01 := 1, +0x0C := 1; record list reset | `0x005B9E70` | `0x007401D0` |
| 19 / 21 | A3Q5 The Blackened Temple | `0x005BBEB0` | 0, 6, 20 | 0x50 | `0x005BAF70` | `0x007408B8` |
| 20 / 22 | A3Q6 The Guardian | `0x005BCDC0` | 0, 6, — | 0x34 | `0x005BC210` | `0x00740EF8` |
| 28 / 32 (reports 16) | A3Q7 Dark Wanderer | `0x005BD300` | 0, —, —; status 0 | 0x14; +0x01 := 1 | — | none |

All are active = 1. F (status iterate) of chains 15, 17, 18, 19, 20
sends 0x5D for the chain to a player when (s.0 clear and s.15 clear) or
s.13 or s.14 (the Act II rule); chain 16's F `0x005B7F90` sends only
when 18.0 and 18.15 are both clear. Status functions: chain 15
`0x005B7D70` (§3.8), 16 `0x005BD850` (§4.9), 17 `0x005B8CC0` (§5.9),
18 `0x005BA170` (§6.9); chains 19, 20 use the default rule
(`quests.md` §6.1, init_no 6); chains 14, 28, 39 return false. Because
chain 16's status byte starts at 1, its status function is consulted
for every 0x40 request from game creation on (`quests.md` §6.2).
Difficulty acts only through the record used, item and monster data and
the Blade's ring level (§5.5).

### 3. A3Q1 Lam Esen's Tome (chain 15, slot 17)

#### 3.1 Extra data

+0x00 reward not yet handed out in this game (1 at init); +0x01 tome
dropped; +0x02 tome active; +0x04 party scratch (§3.8); +0x05 the last
tome holder left; +0x08 tomes in the game; +0x0C tome brought to Alkor
(chat-end pending); +0x10 GUID of the player who brought it (−1 none);
+0x14 tome object GUID; +0x18 tome object mode; +0x1C tome-holder
player list.

#### 3.2 Chat (event 0, `0x005B7400`; active fn `0x005B72B0`)

17.0 set and 17.13 clear → nothing. Holds `bbb ` → table state 3 (any
NPC). Else 17.0 → table state 4 when GUID listed, else nothing. Else
not-intro and state ≤ 3 → index[state] (`0x0073F188`: −1, 0, 1, 2; −1
or > 5 → nothing). Wants to talk: alkor (254) and either (state 1,
17.0 and 17.15 clear) or the player holds `bbb `.

#### 3.3 Alkor's messages (event 11, `0x005B7740`, NPC 254, only without 17.0)

- **549**: state := 2; status 1 to all; for each player `0x005B73A0`
  (players without 17.0 and 17.1 get 17.2 when the state is 2 or 3);
  refresh.
- **564**: refresh. Then: if holds `bbb `: delete it (`quests.md` §9.2),
  +0x0C := 1, +0x10 := player GUID, callback 2 := `0x005B76E0`. If
  +0x00 = 1 (whether or not a tome was handed in): for each player from
  the NPC `0x005B74E0` (in Act III and lacking 17.0 and 17.1: set 17.13,
  17.0, 17.1, send flags); +0x00 := 0; for each player `0x005B7580`
  (17.1 set: add 5 to stat 4 (`statpts`, `0x006272B0(player, 4, 5,
  0)`), send `5D 0F 02 00 0000` (`0x005458E0`), clear 17.1); for each
  player `0x005B75D0` (lacking 17.0 and 17.1: set 17.14, nothing sent).
  Then status 13 (silent); add GUID; if 17.13 and not-intro: game 17.13,
  and if state ≠ 5: state := 5 and call the own seq fn.

#### 3.4 Level changes (event 3, `0x005B7620`)

New level 79 (Lower Kurast), not-intro and state 0 → state := 1 (a
direct write, no status). Old level 75: quick remove; if state = 2 and
the player lacks 17.0 (the bit is tested twice; 17.1 is not tested): if
status ≠ 1, status 1 to all; state := 3; the §3.3 flag iterate
`0x005B73A0` for all.

#### 3.5 Pick-up, drop, leave, join (events 4, 5, 9, 14)

- Pick-up (`0x005B7970`, only active records, `quests.md` §4.3; the
  tome is the only item linked to chain 15): not-intro → status 2 to
  all, state := 4; always add the player to the holder list +0x1C.
- Drop (`0x005B7A00`): remove from +0x1C; not-intro → state := 3,
  status 1 to all.
- Event 9 (`0x005B7B80`, leaving with the tome): count −1; reaching 0
  with not-intro, +0x02 set and state ≤ 3: +0x05 := 1, status 8 to all.
- Event 14 (`0x005B7BC0`): joining player holds `bbb ` → count +1; count
  = 1, not-intro and +0x05 → +0x05 := 0, status 2 to all.

#### 3.6 The tome object (operate 28 `0x005B7A60`, init 23)

Operate: needs chain 15, not-intro and object mode 0. A player with
17.0 → sound 19, return 0. Else drop code `bbb `; one item via
`0x00559A30(game, object, 2, &level, 0, −1, 0)` (normal, not
droppable). Created: object mode := 2; count +1; +0x02 := 1, +0x01 :=
1, +0x18 := 2, +0x14 := object GUID; event-0 and event-11 callbacks
re-stored (same functions); state ≠ 3 → state := 3; status ≠ 1 →
status 1 (silent). Returns 0. Init 23 (`0x00544E30`: chain 15 lookup,
fatal when absent, then `0x005B7310`): intro → mode 2; else mode :=
+0x18.

#### 3.7 Chat end and game start (events 2, 13)

- Chat end (`0x005B76E0`): alkor, +0x0C = 1 and the player's GUID =
  +0x10: sound 67 unless the player's class is 5 or 6 (druid,
  assassin); +0x0C := 0. Callback 2 stays set.
- Event 13 (`0x005B7C10`): 17.0 → game 17.13. Else holds `bbb ` →
  status 2, state 4, count +1, +0x18 := 2, +0x02 := 1, add the player to
  +0x1C. Else 17.3 → state 3, status 1; 17.2 → state 2, status 1.

#### 3.8 Status function (`0x005B7D70`, always true)

15.0 clear → 0. 17.0 → 11 + 2 × (17.13). Holds `bbb ` → 2. Party: +0x04
:= 0, then for each member `0x005B7D40` (holds `bbb ` → +0x04 := 1);
set → 2. Not-intro: state < 4 → 1, but 8 + (game type +0x6A ≠ 3) when
+0x02 = 1 and the count is 0; state ≥ 4 → 12, but 1 when the player is
in Act III and the count ≠ 0. Intro → 0.

### 4. A3Q2 Khalim's Will (chain 16, slot 18)

#### 4.1 Items and bits

Parts: Khalim's Eye `qey`, Heart `qhr`, Brain `qbr`, Flail `qf1`; the
cubed Will `qf2` (`world/cube.md`; all `quest` = 17). Bits: 18.2 Cain
gave the quest; told about 18.3 eye, 18.6 heart, 18.4 brain, 18.5
flail, 18.7 Will.

#### 4.2 Extra data

+0x00 sewer stairs initialised, +0x04 its GUID, +0x08 its mode; +0x01
Cain started (chat-end pending); +0x0C drop count scratch; +0x10 eyes,
+0x14 brains, +0x18 hearts, +0x1C flails dropped (live counts), +0x20
Wills cubed; +0x24 eye, +0x25 brain, +0x26 heart, +0x27 flail dropped
once; +0x2C player list (reset only).

#### 4.3 Chat (event 0, `0x005BD630`; only cain3 245)

1. State ≠ 0 and 18.2 clear → table state 0 (msg 543).
2. Holds `qf2 ` with 18.7 clear and the orb not smashed (chain 19 absent
   or its extra +0x0C = 0) → 5. Holds `qf1 ` with 18.5 clear → 4.
   `qey ` with 18.3 clear → 1. `qhr ` with 18.6 clear → 2. `qbr ` with
   18.4 clear → 3 (first match wins).
3. Else by parts held (n = number of `qf1`, `qey`, `qhr`, `qbr` held):
   n = 0: `qf2 ` held → 11; else 18.2 set → 6; else nothing. n > 0:
   flail → 10; else heart → 8; else eye → 7; else brain → 9.

Wants to talk (`0x005BD500`): NPC 245 and any of: state ≠ 0 with 18.2
clear; a held part whose bit is clear (as step 2; the Will also needs
the orb not smashed, `0x005BBF80`); all four parts held with 18.5 clear.

#### 4.4 Cain's messages (event 11, `0x005B8060`, NPC 245)

| Msg | Effect |
|---|---|
| 543 | state := 2; set 18.2; +0x01 := 1; callback 2 := `0x005B8020` |
| 544 / 545 / 546 / 547 / 548 | set 18.6 / 18.3 / 18.4 / 18.5 / 18.7 |

Chat end (`0x005B8020`): cain3 with +0x01 = 1: status 1 to all (chain
16's F); clear callback 2 (+0x01 stays 1).

#### 4.5 Level change, pick-up, start (events 3, 4, 13; event 10 is a bare `ret`)

- Event 3 (`0x005B7FF0`): new level 77 (Great Marsh), not-intro and
  state < 3 → state := 1.
- Pick-up (`0x005B8210`, active records only): needs 15.0 set and 18.0
  clear. By picked code, a `5D 10 <flags> <v> 0000` to this player only
  (`0x005B81C0`; the record's status byte is not written):

  | Picked | v (first missing part in this order wins; all held → last column) |
  |---|---|
  | `qf1 ` | no eye 1, no brain 2, no heart 4; all held: 18.5 clear 5, else 7 |
  | `qey ` | no brain 2, no flail 3, no heart 4; all held 7 |
  | `qhr ` | no brain 2, no flail 3, no eye 1; all held 7 |
  | `qbr ` | no heart 4, no flail 3, no eye 1; all held 7 |
  | `qf2 ` | status := 7 when it is ≠ 0; nothing sent (edge case 2) |

- Event 13 (`0x005B8470`): 18.0 set → nothing. 18.7 → status 7, state
  2. Else 18.2 → state 2, status 7; then any part count +0x10..+0x1C
  ≠ 0 → status 7, state 2.

#### 4.6 Khalim's chests (operate 57 / 59 / 58)

| Chest (obj) | Fn | Code | Count field, dropped flag |
|---|---|---|---|
| 405 heart | `0x005B8860` | `qhr ` | +0x18, +0x26 |
| 406 brain | `0x005B8A20` | `qbr ` | +0x14, +0x25 |
| 407 eye | `0x005B8940` | `qey ` | +0x10, +0x24 |

Each: passes the quest-chest gate `0x00545850` or returns 0; one
quest-seed step: n = (lo' mod 5) + 5 piles of gold (`0x00585970(game,
object, 'gld ', 2)`) **before** the items; drop code := the part; with
chain 16: +0x0C := 0, for each player (from the operating player) that
holds neither the part nor `qf2 ` (`0x005B8810` / `0x005B87C0` /
`0x005B8770`) +0x0C += 1; that many `0x00559A30(game, object, 2,
&level, 0, −1, 1)` (normal, droppable), each created adds 1 to the
count and sets the flag; then the chest's own treasure `0x00585B90(op,
4)`. Returns 0. (The count includes players anywhere in the game.)

#### 4.7 Sewer lever and stairs

- Stairs init 41 (`0x005B8660`): chain 16: +0x00 := 1, +0x04 := GUID;
  intro → mode 2, else mode := +0x08. Stairs operate 44 (`0x005B84E0`):
  only in mode 2 → `0x0059D9D0` (the stairs' warp, object spec).
- Lever init 42 (`0x005B86B0`): intro → mode 2.
- Lever operate 45 (`0x005B8530`): lever mode 0, chain 16, +0x00 = 1
  and the stairs unit (+0x04) exists: lever mode := 1 with an
  end-animation event (`0x005417D0` type 1 at frame + (`FrameCnt1` >>
  8)); +0x08 := 2; object event 7 on the lever at frame + 30; FX 9.
  Returns 0.
- Lever event 7 (`0x005B85E0`, class 367): if +0x00 = 1: stairs gone →
  +0x08 := 2; else stairs mode := +0x08 and, when +0x08 ≠ 2, +0x08 := 2
  and an end-animation event on the stairs.

#### 4.8 Cubing the Will (`0x005B86E0`, from `world/cube.md` §8)

Reads 18.0 / 18.1 (no effect); chain 16 counts: eyes, hearts, brains,
flails −1 each; Wills cubed +1. No bit changes and nothing sent.

#### 4.9 Status function (`0x005BD850`, always true)

out := 0; 15.0 clear → 0. Else out := 1 when 18.2 or (not-intro and
state ≥ 2). Holds `qf2 ` → 12 when the orb is smashed (chain 19 extra
+0x0C ≠ 0), else 6. n parts held (§4.3): 4 → 18.5 clear ? 7 : 5; n > 0:
no eye 1, else no brain 2, else no heart 4, else (flail ? 7 : 3); n = 0:
18.2 → 1, else (state > 1 ? 1 : 0) (not-intro not tested here). 18.0 is
not tested.

#### 4.10 Completion

Khalim's Will completes outside this record: smashing the Compelling
Orb (§7.7: 18.0, 18.13), Cain's 626 message for players with 18.0
(deletes the parts, §7.3), or the Durance warp (`quests.md` §8.1: 18.0,
18.13, deletes `qey`, `qhr`, `qbr`, `qf1`, `qf2`).

### 5. A3Q3 Blade of the Old Religion (chain 17, slot 19)

#### 5.1 Bits and extra data

Bits: 19.2 started, 19.3 left town, 19.4 Gidbinn dropped, 19.5 holds it,
19.6 handed to Ormus, 19.7 Asheara's mercenary taken, 19.8 Ormus' ring
taken, 19.9 first pick-up sound played. Extra: +0x00 Gidbinn dropped;
+0x01 Hratli started (chat-end pending); +0x02 boss spawned; +0x03 decoy
activated; +0x04 boss spawning; +0x05 spawn timer exists; +0x06 altar
may activate; +0x07 Gidbinn brought (chat-end pending); +0x08 / +0x0C
decoy x, y; +0x10 / +0x14 altar x, y; +0x18 decoy initialised; +0x1C
Gidbinns held in the game; +0x20 last holder left; +0x24 boss GUID;
+0x28 altar GUID; +0x2C altar mode.

#### 5.2 Flag iterate (`0x005B8B20`)

Players without 19.0 and 19.6, chain 17 present: holds `g33 ` → 19.5
(stop). Else Gidbinn dropped (+0x00): set 19.4, and 19.2 if 19.3 is
clear. Else state 2 → 19.3.

#### 5.3 Chat (event 0, `0x005B8EC0`; active fn `0x005B8E10`)

19.0: GUID listed → table state 4, else nothing. Holds `g33 ` with 19.5
set and 19.8 clear → 3. Else 19.6 set, first match: asheara (252) with
19.7 clear → 5; ormus (255) with 19.8 clear → 6; any NPC but asheara
with 19.7 clear → 5; any NPC with 19.8 clear → 6; else nothing. 19.6 clear:
19.5 → 4; state > 3 or holds `g33 ` → nothing; else index[state]
(`0x007401B8`: −1, 0, 1, 2; −1 or > 7 → nothing). Wants to talk (19.0
clear): hratli (253) without `g33 ` and state 1; ormus with `g33 `, or
with 19.6 and 19.8 clear; asheara with 19.6 and 19.7 clear.

#### 5.4 Messages (event 11, `0x005B9240`; nothing when 19.0 is set)

| From | Msg | Effect |
|---|---|---|
| hratli 253 | 571 | state := 2; +0x01 := 1 |
| ormus 255 | 587 | if holds `g33 ` with 19.5 set and 19.8 clear: +0x07 := 1; delete `g33 `; count −1; set 19.6; party members (`0x005B91B0`: chain 17, in Act III, lacking 19.0, 19.6, 19.8, 19.7) get 19.6; for each player `0x005B9130` (lacking 19.0, 19.6, 19.8, 19.7 and not holding `g33 `: set 19.14, nothing sent). Then (always) not-intro → game 19.13; status 13 (silent); not-intro and state ≠ 5 → state := 5 and the own seq fn |
| ormus 255 | 593 | with 19.6 set and 19.8 clear: set 19.8; ring (§5.5); if 19.7: set 19.13, 19.0, add GUID |
| asheara 252 | 589 | with 19.7 clear: set 19.7; quest mercenary `0x00579180(game, player, 252)` (`world/npc.md` §7.5); if 19.8: set 19.13, 19.0, add GUID |

#### 5.5 Ormus' ring

`0x005466B0(game, player, 'rin ', level, 6, 1)` (`quests.md` §9.1):
rare quality, droppable, item level 21 / 35 / 75 for normal / nightmare
/ hell (game difficulty +0x6D).

#### 5.6 The decoy and the boss

- Decoy operate 31 (`0x005B9B40`): object mode 0. A player with 19.0,
  19.7 or 19.8 → sound 19. Else, with chain 17 not-intro: state 0 →
  state := 1; object mode 1 with an end-animation event; +0x03 := 1; no
  timer yet (+0x05) → +0x05 := 1, timer period 7, callback
  `0x005B9A30`. Intro: nothing. Returns 0.
- Decoy init 25 (`0x005B9AE0`): intro → mode 2. Not-intro: +0x18 := 1,
  +0x08 / +0x0C := its position; +0x03 set and +0x02 clear → spawn the
  boss (below) in the object's room.
- Timer (`0x005B9A30`): +0x05 := 0. If +0x18 = 1, +0x03 set, +0x02
  clear, the game has Act III (game +0xC4) and a room covers (+0x08,
  +0x0C) (`0x00619DA0`): scan that room and its adjacent rooms
  (`0x00619790`) in list order for a unit of type 0 (player); found →
  spawn the boss and set callback 8 := `0x005B9980`. Returns 1 in every
  case: one attempt per decoy operation.
- Boss spawn (`0x005B9930`): +0x04 := 1; monster 407 `fetish11` via
  `0x005A43E0(game, room, 0, 407, 1, 0, 0, 1)` (monster spec); created
  → +0x03 := 0, +0x02 := 1, +0x24 := GUID; +0x04 := 0. Monster creation
  links class 407 to chain 17 (§10).
- Kill (event 8, `0x005B9980`): needs +0x02 and not-intro, and the
  victim passes `0x005A0180` or `0x0063E9F0(0, victim)` (unique /
  champion / boss tests, monster spec). Drop code `g33 `; one
  `0x00559A30(game, victim, 2, &level, 0, −1, 0)`. Created: +0x00 := 1,
  the flag iterate for all, callback 8 := null. Not created: +0x02 := 0.

#### 5.7 The altar and Ormus

Altar init 39 (`0x005B9D40`, object 251): +0x28 := GUID; +0x10 / +0x14
:= its position; mode := +0x2C. Chat end (`0x005B8C50`): hratli with
+0x01 → status 2 to all, +0x01 := 0, the flag iterate for all; ormus
with +0x07 → +0x07 := 0, +0x06 := 1. Callback 2 stays set. Ormus' map
AI then reads the altar position (`0x005B9CA0`: only while +0x06) and
activates it (`0x005B9CD0`: +0x06 := 0; altar mode 1 with an
end-animation event at frame + (`FrameCnt1` >> 8) + 1; +0x2C := 2).

#### 5.8 Level change, pick-up, join, leave, start

- Event 3 (`0x005B9090`): new level 78 (Flayer Jungle), not-intro,
  state 0 → state := 1. Old level 75: quick remove; state 2 and the
  player lacks 19.0 and 19.15 → state := 3 and the flag iterate.
- Pick-up (`0x005B9520`, active records): item `g33 ` and not-intro:
  status 4 to all; 19.9 clear → set it and sound 65; state := 4; status
  3 to all. Then (always) the flag iterate for all.
- Event 14 (`0x005B9680`): holds `g33 ` → count +1; +0x20 and count = 1
  → +0x20 := 0. Event 9 (`0x005B96C0`): item `g33 ` → count −1;
  reaching 0 with +0x00, not-intro and state < 5 → +0x20 := 1. Neither
  sends anything.
- Event 13 (`0x005B9700`): holds `g33 ` → count +1. 19.0 → +0x2C := 2,
  stop. Holds `g33 ` → status 4, state 4, flag iterate, stop. 19.6
  clear: 19.5 → state 3, status 2; then 19.4 or 19.3 → state 2, status
  2; else 19.2 → state 3, status 1. 19.6 set: +0x2C := 2, state := 5;
  19.7 clear → status 5; else 19.8 set → set 19.0 and not-intro := 0;
  else status 6.

#### 5.9 Status function (`0x005B8CC0`, always true)

15.0 clear → 0. 19.0 → 11 + 2 × (19.13). Holds `g33 ` → 4. +0x20 → 7 +
(game type = 3). 19.6 set: start 0; 19.8 clear → 6; then 19.7 clear →
5. 19.6 clear: 19.4 → 2 + (+0x00 = 1); 19.3 → 2; 19.2 → 1; else 0.

### 6. A3Q4 The Golden Bird (chain 18, slot 20)

#### 6.1 Bits and extra data

Bits: 20.2 Cain told about the figurine, 20.4 told about the bird, 20.1
bird given to Alkor (reward pending), 20.5 Potion of Life unused, 20.6
first figurine pick-up sound played. Extra: +0x00 bird brought to Alkor
(read and cleared by Alkor's map AI, §10); +0x01 a boss may be chosen
(1 at init); +0x02 a boss is chosen; +0x04 its GUID; +0x08 / +0x09 /
+0x0A / +0x0B chat-end pending for Alkor / Cain's first talk / Cain's
second talk / Meshif; +0x0C figurine still to drop (1 at init); +0x10
figurines plus birds held in the game; +0x14 figurine dropped; +0x15
last holder left; +0x18 bit for the party iterate.

#### 6.2 Choosing the boss (`0x00544E80` → `0x005BAC70`)

Called from special monster creation (`0x005A09E0`, monster spec) for
a monster in Act III when chain 18 is not-intro, its class is not 407
and its monstats flags byte +0x0D has no bit 6: if +0x0C and +0x01 = 1
and (chain 17 absent or its +0x04 = 0, no Gidbinn boss being spawned):
link the unit to chain 18 (`0x005436B0`); callback 8 := `0x005BAB60`;
+0x01 := 0, +0x02 := 1, +0x04 := its GUID. Removal of a monster whose
newest chain link is an active filter-20 record, in Act III
(`0x00544F20` from `0x005421A0`) calls `0x005BACF0`: +0x02 and GUID =
+0x04 → +0x02 := 0, +0x01 := 1 (the next eligible monster is chosen).

#### 6.3 The kill (event 8, `0x005BAB60`)

Needs not-intro, +0x02 and +0x0C. A killing player with 20.0 → nothing.
Drop code `j34 `; one `0x00559A30(game, victim, 2, &level, 0, −1, 0)`.
Not created: +0x01 := 1. Created: callback 8 := null; count +1; +0x0C
:= 0; +0x14 := 1; state 0 → state := 1; status 0 → status 1 to all.
Either way +0x02 := 0.

#### 6.4 Chat (event 0, `0x005B9ED0`; active fn `0x005B9D90`)

20.0: GUID listed → table state 6, else nothing. Alkor (254) with +0x00
= 1 → nothing. 20.1 → 5. Holds `g34 `: cain3 (245) with 20.4 clear → 3;
alkor → 2 (when 20.0 and 20.1 are clear); others → 2. Holds `j34 `:
cain3 or asheara (252) → 0 when 20.2 is clear, 7 when set; others → 1.
Wants to talk (20.0 and 20.1 clear): alkor holding `g34 `; meshif2
(264) holding `j34 `; cain3 holding `j34 ` with 20.2 clear or `g34 `
with 20.4 clear.

#### 6.5 Messages (event 11, `0x005BA320`; nothing when 20.0 is set)

| From | Msg | Effect |
|---|---|---|
| cain3 245 | 527 | set 20.2; state 1 → state := 2, +0x09 := 1, callback 2 := `0x005BA0B0`; refresh; +0x18 := 2; party: members lacking 20.0 and 20.1 get 20.2 (`0x005BA290`) |
| cain3 245 | 531 | set 20.4; +0x0A := 1; refresh; +0x18 := 4; party: 20.4 (callback 2 not set here) |
| meshif2 264 | 529 | with `j34 ` held: callback 2 := `0x005BA0B0`; delete `j34 `; count −1; give `g34 ` (`0x005466B0(game, player, 'g34 ', 0, 2, 1)`); given → count +1, state := 3, +0x0B := 1 |
| alkor 254 | 534 | holds `g34 `, lacks 20.1 and 20.0: delete `g34 `; count −1; not-intro and state ≠ 4 → state := 4; set 20.1; +0x08 := 1; callback 2 := `0x005BA0B0`; +0x18 := 1; party: 20.1 |
| alkor 254 | 538 | with 20.1 (20.0 clear): clear 20.1; set 20.0, 20.13; reset_progress(20) (bits 2–11); set 20.5; not-intro → state := 5; give `xyz ` (`0x005466B0(…, 'xyz ', 0, 2, 1)`); send flags; add GUID. Then (both cases) if 20.13: game 20.13 and the own seq fn |

Chat end (`0x005BA0B0`, never cleared): alkor with +0x08 → status 5 to
all, +0x08 := 0, +0x00 := 1. cain3: +0x09 → status 2 to all, +0x09 :=
0; then +0x0A → status 4 to all, +0x0A := 0. meshif2 with +0x0B →
status 3 to all, +0x0B := 0.

#### 6.6 Potion of Life (item use, owned by the item-use spec)

Using `xyz ` (`0x0055E170`, usable only with 20.5, `0x0055CC90`):
clear 20.5; add 0x1400 (20 << 8) to stat 7 (`maxhp`); send `5D 12 02 00
0000`; consume the potion.

#### 6.7 Pick-up, join, leave (events 4, 14, 9; event 3 `0x005BA0A0`: old level 75 → quick remove)

- Pick-up (`0x005BA6A0`, active records; lacking 20.0 and 20.1): `g34 `
  → state := 3, status 3 to all. `j34 ` → status 1 to all, state := 1;
  20.6 clear → set it and sound 72.
- Event 14 (`0x005BA9E0`): +1 for each of `j34 `, `g34 ` held; count =
  1, not-intro and +0x15 → +0x15 := 0, status (3 when the player holds
  `g34 `, else 1) to all.
- Event 9 (`0x005BA9A0`): count −1; reaching 0 with not-intro, +0x14
  and state < 4 → +0x15 := 1, status 6 to all.

#### 6.8 Game start (event 13, `0x005BA870`)

20.0 or 20.1 → not-intro := 0, +0x0C := 0, +0x01 := 0. Holds `g34 ` →
+0x01, +0x0C := 0; state 3, status 3 (20.4 clear) or 4 (set). Holds
`j34 ` → +0x01, +0x0C := 0; 20.2 clear → state 1, status 1; else state
2, status 2.

#### 6.9 Status function (`0x005BA170`, always true)

15.0 clear → 0. 20.1 → 5. 20.0 → 11 + 2 × (20.13). Holds `g34 ` → 3 +
(20.4). Holds `j34 ` → 1 + (20.2). Not-intro: status byte > 5 → 6 +
(game type ≠ 3); else the status byte. Intro → 0.

### 7. A3Q5 The Blackened Temple (chain 19, slot 21)

#### 7.1 Bits and extra data

Bits: 21.2 talked to Ormus, 21.3 council seen, 21.4 council killed
(Will not yet used). Extra: +0x00 last killed council GUID; +0x04 Ormus
started (chat-end pending); +0x05 council seen; +0x08 starting player
had 17.0; +0x0C Compelling Orb smashed; +0x0D flail dropped; +0x0E cube
dropped; +0x10 six council GUIDs; +0x28 orb monster spawned, +0x2C its
GUID; +0x30 council registered; +0x34 council left to kill; +0x38 orb
hits; +0x3C flails to drop; +0x40 cubes to drop.

"S(a, b)" below = state a when Lam Esen is done (game 17.13), else b.

#### 7.2 Flag iterate (`0x005BADC0`)

Players without 21.0, chain 19 present: state 2 or 3 → 21.2; +0x05 →
21.3.

#### 7.3 Chat and messages (events 0, 11; active fn `0x005BAD60`)

- Event 0 (`0x005BAFD0`): 21.4 set → table state 5 unless the orb is
  smashed. Else 21.0 and not GUID listed → nothing; state > 5 → table
  state 6 when 21.13, else nothing; intro → nothing; else index[state]
  (`0x00740ED8`: −1, 0, 1, 2, 3, 4; −1 or > 7 → nothing).
- Wants to talk (21.0 clear): ormus (255) with state 1; cain3 (245)
  with 21.4 and the orb not smashed.
- Event 11 (`0x005BB210`, nothing with 21.0): ormus msg **594**: state
  := S(2, 3); +0x04 := 1; refresh. cain3 msg **626** with 21.4: 18.0
  set → delete `qey`, `qhr`, `qbr`, `qf1`, `qf2`; 21.13 and state ≠ 7
  → game 21.13, status 13 (silent), state := 7; call the own seq fn;
  set 21.0, clear 21.4, add GUID; refresh.
- Chat end (`0x005BB0C0`, installed by §7.4): ormus with +0x04 = 1:
  status 2 to all; +0x04 := 0; clear callback 2; flag iterate for all.

#### 7.4 Level changes and start (events 3, 13; event 10 = list remove)

- Event 3 (`0x005BB120`): new level 82 (Kurast Causeway), not-intro,
  state 0 → state := 1, callback 2 := `0x005BB0C0`. Old level 75: quick
  remove; state 2 or 3 and the player lacks 21.0 and 21.4: status < 3 →
  status 2 (silent); state := S(4, 5); flag iterate for all.
- Event 13 (`0x005BB480`): 18.0 → +0x0C := 1 (the orb counts as
  smashed). 21.0 or 21.4 → game 21.13, not-intro := 0. Else +0x08 :=
  17.0; 21.3 → status 3, state (+0x08 ? 4 : 5); else 21.2 → status 2,
  state (+0x08 ? 2 : 3).

#### 7.5 The council

Superuniques 26 Ismail Vilehand, 27 Geleb Flamefinger and 29 Toorc
Icefist get a chain-19 link at creation (§10); their kills are forced
(`quests.md` §4.4). The preset path (`0x005A49B0`) also calls
`0x00545B50` → `0x005BB550`: with chain 19 not-intro: +0x05 := 1; if
fewer than 6 are registered and the GUID is new: append it, +0x34 :=
+0x30 := count; then unless (state < 2 and status = 1) or (state ≥ 2
and status = 3): status 3 to all; state := S(4, 5); flag iterate.

#### 7.6 Kills (event 8, `0x005BBC00`)

Victim class 366 (the orb's monster): unit flags |= 0x20000, stop.
Other victims:

1. If the flail or the cube is not dropped yet: +0x3C := 0, +0x40 := 0;
   for each player `0x005BB7B0` (in Act III: lacking 18.0 and holding
   neither `qf1 ` nor `qf2 ` → +0x3C += 1; not holding `box ` → +0x40
   += 1).
2. Flail not dropped: drop code `qf1 `; +0x3C × `0x00559A30(game,
   victim, 7, &level, 0, −1, 0)` (unique quality); any created → +0x0D
   := 1, chain 16 flail count += created, its +0x27 := 1. Else, cube not
   dropped: +0x0E := 1; drop code `box `; +0x40 × quality 2.
3. Not-intro and +0x34 > 0: +0x34 −= 1; reaching 0: +0x00 := victim
   GUID; state := 7 if the orb is smashed, else 6; status 4 to all; for
   each player `0x005BAE20` (in Act III, lacking 21.0 and 21.4, in the
   room of the monster +0x00 or a room adjacent to it: set 21.4 — or
   21.0 when 18.0 is set — and 21.13); for each player from the victim:
   `0x005BB6C0` (21.13 → party members in Act III lacking 21.0, 21.4
   get the same, `0x005BB640`), completion flag `0x005BB710` (lacking
   21.0 and 21.4), `0x005BB770` (21.13 → sound 64); game 21.13.

#### 7.7 The Compelling Orb (object 404)

- Init 60 (`0x005BBBA0`): orb smashed → mode 2. +0x28 = 0 → spawn
  monster 366 `compellingorb` at the object (`0x005B3090`, mode 1);
  created → unit flags |= 0x20000, +0x2C := GUID, +0x28 := 1.
- Operate 53 (`0x005BB980`): object mode 0. The player's weapon
  (`0x0063BEF0` on the inventory) must be `qf2 `, else sound 19. +0x38
  += 1; below 2 → return 0 (the first valid hit does nothing). Then:
  set 18.0, 18.13; delete `qf2 `; 21.4 set → set 21.0 (when clear); the
  orb monster (+0x2C) exists → kill it (`0x005DDFC0`, `0x005DFEE0`,
  monster spec); object mode 1 with an end-animation event; +0x0C := 1;
  FX 10; call chain 19's seq fn; party members (`0x005BB850`): with
  18.0 → delete the five Khalim items; else in Act III → set 18.0,
  18.13, delete the five items unless trading (`0x005678A0`), and 21.4
  set with 21.0 clear → set 21.0. Nothing is sent (no 0x28).
- Stairs R init 53 (`0x005BBB70`, object 386): orb smashed → mode 2.
- Durance warp check (`quests.md` §8.2, `0x005BBFA0`): closed while
  +0x0C = 0, except from Durance of Hate 2.

### 8. A3Q6 The Guardian (chain 20, slot 22)

#### 8.1 Bits and extra data

Bits: 22.2 / 22.3 talked to Ormus (state 2 / 3), 22.4–22.6 and 22.7–22.9
progress (§8.3), 22.11 Mephisto killed, NPC talk pending. Extra: +0x00
status timer; +0x01 Hellgate initialised, +0x04 its GUID; +0x02 bridge
initialised, +0x08 its GUID; +0x03 Ormus started (chat-end pending);
+0x0C Hellgate mode; +0x10 bridge mode; +0x14 soulstones dropped;
+0x18 a soulstone dropped; +0x1C soulstones to drop; +0x20 Natalya
spawned, +0x24 her map AI, +0x2C her GUID, +0x30 map AI applied.

"S(a, b)" as in §7.1.

#### 8.2 Chat and messages (events 0, 11; active fn `0x005BBFE0`)

- Event 0 (`0x005BC270`): 22.11 → table state 5. 22.0 → 6 when GUID
  listed, else nothing. State > 5 → 6 when 22.13, else nothing. Else
  index[state] (`0x00741518`: −1, 0, 1, 2, 3, 4; −1 or > 7 → nothing).
- Wants to talk: ormus (255) with 22.0 clear and state 1; any NPC with
  22.11.
- Event 11 (`0x005BC5A0`, with 22.11 set or 22.0 clear): ormus msg
  **628**: state := S(2, 3); +0x03 := 1; refresh. Msgs **657–663** from
  any NPC: with 22.11: 22.13 → status 13 (silent), state := 7; clear
  22.11; add GUID. Refresh.
- Chat end (`0x005BC370`, installed by the seq fn and §8.4): ormus with
  +0x03 = 1: status 2 to all; +0x03 := 0; clear callback 2.

#### 8.3 Flag iterate (`0x005BC020`)

Players without 22.0 and 22.11: state 2 → 22.2; 3 → 22.3; state 4 with
status 2 / 3 / 4 → 22.4 / 22.5 / 22.6; state 5 with status 2 / 3 / 4 →
22.7 / 22.8 / 22.9.

#### 8.4 Level changes (event 3, `0x005BC3C0`)

1. Old level 75: quick remove; state 2 or 3 and the player lacks 22.0
   and 22.11: status < 3 → status 2 (silent); state := S(4, 5); flag
   iterate for all.
2. New level ≥ 98 (from Ruined Fane on), not-intro, state 0 → state :=
   1, callback 2 := `0x005BC370`.
3. New level 100 (Durance 1): status 0 or 2 → status 3 to all. State 1
   → state := S(4, 5) and flag iterate; other states → flag iterate only
   when status 3 was just sent.
4. New level 102 (Durance 3): status ≠ 4 → status 4 to all; state ∉
   {4, 5} → state := S(4, 5); flag iterate.

#### 8.5 Mephisto's death (event 8, `0x005BC8B0`)

Class 242 kills are forced (`quests.md` §4.4); Mephisto (base 242,
except class 704) gets the chain-20 link at creation (§10). Always:
state := 6; callback 10 := list remove.

1. Not-intro: a killing player without 22.0: stub `0x00545990` (`ret
   4`); without 22.11 → soulstone count +1; then the credit `0x005BC140`
   (set 22.13, 22.0, 22.11; character act progression
   `0x00538680(client, 3, difficulty)`, save spec). For each player from
   the victim: `0x005BC190` (in level 102 without 22.0: without 22.11 →
   credit and count +1), `0x005BC7C0` (22.13 → party members in Act III
   without 22.0 get the credit and count +1, `0x005BC750`), completion
   flag `0x005BC810` (lacking 22.0, 22.11), `0x005BC870` (22.13 → sound
   66). No timer yet → timer period 12 (`0x005BC720`: status ≠ 4 →
   status 4 to all; +0x00 := 0; returns 1), +0x00 := 1.
2. Always: game 22.13; Hellgate (+0x01, GUID +0x04) → mode 1 with an
   end-animation event; +0x0C := 2; drop code `mss `; +0x1C ×
   `0x00559A30(game, victim, 2, &level, 0, −1, 0)`, each created:
   +0x14 += 1, +0x18 := 1; stub `0x005B6930`; FX 11.

#### 8.6 Hellgate and Mephisto's bridge

- Hellgate init 44 (`0x005BCBF0`, object 342): in level 104
  (Outer Steppes, `levels.txt` Act 4 - Mesa 1) → mode 2. Else with chain 20: +0x01 := 1,
  +0x04 := GUID, mode := +0x0C.
- Bridge init 45 (`0x005BCB90`, object 341): +0x02 := 1, +0x08 := GUID,
  mode := +0x10; +0x10 ≠ 2 → object event 7 at frame + 20.
- Bridge event 7 (`0x005BCAC0`): with +0x0C = 2 (Mephisto dead) and the
  bridge mode ≠ 2: the first player in the bridge's room unit list
  closer than 18 (`0x00641530`): mode 0 → +0x10 := 2, mode 1; mode 1 →
  +0x10 := 2, mode 2, free its collision (`0x00623830`). Then (also
  without a player, or before Mephisto dies) event 7 again at frame +
  24; a bridge already in mode 2 stops.
- The Durance warp (`quests.md` §8.1, `0x005BCFD0`) opens both.

#### 8.7 Natalya (init 52, `0x005BCE80`, object 382)

Unless game 22.13: if not (+0x20 and the unit +0x2C exists with class
297): spawn monster 297 `natalya` at the object (`0x005B2F20`, mode 1,
−1; then mode 1, 3); none → +0x20 := 0; else +0x20 := 1, +0x2C := GUID
and, when a map AI is stored (+0x24, with its +4 ≠ 0) and +0x30 = 0,
apply it (`0x0058F000`, `0x00666120`), +0x30 := 1. `0x005BD040` stores
the map AI (+0x24) and applies it the same way (no caller found).

#### 8.8 Game start (event 13, `0x005BCC80`)

22.0, 22.11 or 23.0 → game 22.13, +0x0C := 2. Else the first set of
22.9 → (state 5, status 4), 22.8 → (5, 3), 22.7 → (5, 2), 22.6 → (4,
4), 22.5 → (4, 3), 22.4 → (4, 2), 22.3 → (2, 2), 22.2 → (3, 2).

### 9. Act III gossip and intro records

#### 9.1 A3Q0 Hratli (chain 14, slot 16)

Extra: +0x00 start Hratli spawned, +0x01 end Hratli spawned, +0x02 end
object seen, +0x03 start Hratli present, +0x04 / +0x08 end position,
+0x0C Hratli GUID, +0x10 map AI applied, +0x18 map AI.

- Event 0 (`0x005B6F20`): hratli (253) with 16.0 clear → table state 1
  (msg 466) when the player's class is 1 (sorceress), else 0 (msg 465).
  Wants to talk (`0x005B6FA0`): 253 with 16.0 clear. Status fn: false.
- Event 11 (`0x005B6ED0`): 465 or 466 from 253 → 16.0, game 16.13.
  Event 13 (`0x005B6FD0`): 16.0 → game 16.13.
- Init 49 (`0x005B70B0`, start dummy 378): game 16.13 clear: unless
  +0x00 and Hratli (+0x0C) exists, spawn monster 253 (`0x005B2F20`,
  mode 1, −1); created → +0x0C := GUID, +0x00 := 1.
- Init 50 (`0x005B7160`, end dummy 379): +0x02 := 1, +0x04 / +0x08 :=
  position. Game 16.13 set and +0x01 = 0: Hratli from the start exists
  → +0x03 := 1; else spawn 253 there; created → +0x0C := GUID, +0x01 :=
  1, unit flags |= 0x3000000, map AI applied once as in §8.7.
  `0x005B7230` stores the map AI (no caller found).

#### 9.2 A3Q7 Dark Wanderer (chain 28, table slot 32, reports in slot 16)

Extra: +0x00 wanderer object seen, +0x01 wanderer still to spawn (1 at
init), +0x02 walk target fixed, +0x04 / +0x08 target, +0x0C minions
spawned, +0x0D minion timer exists, +0x10 wanderer GUID.

- Event 13 (`0x005BD2C0`): 32.0 → game 32.13, +0x01 := 0. No other
  callbacks; status and active fns return false.
- Init 43 (`0x005BD1F0`, object 368): with +0x01: target := (x + 7, y);
  spawn monster 368 `darkwanderer` there (room `0x00463740`,
  `0x005B2F20` mode 1, −1); created → +0x01 := 0. Always +0x00 := 1.
- Walk target (`0x005BD0D0`, from the wanderer's AI `0x005EA130`):
  without +0x00 → none. First call: +0x02 := 1; try (X, Y − 20), (X, Y −
  11), (X + 2, Y − 11), (X − 2, Y − 11), (X, Y − 8) with `0x006229F0`
  (mask 0x3C01; nonzero = blocked): the first free one, or (X, Y − 3)
  when all are blocked, becomes the stored target (X, Y = the +0x04 /
  +0x08 values). Later calls return the stored target.
- Minion hook (`0x005BD4A0`, from `0x005EA130`): +0x0C and +0x0D clear →
  +0x0D := 1, +0x10 := wanderer GUID, timer period 2 `0x005BD390`.
- Timer (`0x005BD390`): +0x0C clear: for each player `0x005BD260`
  (without 32.0, in Act III → set 32.0, nothing sent); +0x0C := 1,
  +0x0D := 0. Wanderer exists: one quest-seed step; for i = (lo' & 1)
  … 7: spot = wanderer position + offset[i] (table `0x00741538`: (−3,
  −3), (−3, 0), (−3, 3), (0, −3), (0, 3), (3, −3), (3, 0), (−3, 3));
  free spot (`0x00545340`, size 3, mask 0x3F11, radius 11, limit 100);
  found → object 131 there (`0x00555230`, flags 1, 0, 0). Returns 1.
  Object 131's event 7 spawns monster 301 `vilechild1` in level 76
  (`quests.md` §9.5).

#### 9.3 Act III intro (chain 39)

Event 0 (`0x005B6D30`): special class per NPC: asheara 252 → 0
(amazon), alkor 254 → 2 (necromancer), ormus 255 → 3 (paladin), meshif2
264 → 4 (barbarian), cain3 245 and natalya 297 → 7 (none); hratli and
others → nothing. Intro bit clear (`0x005723C0`) → table state 1 when
the player's class matches, else 0. Event 11 (`0x005B6C60`): 458 from
245, 490/491 from 252, 501/502 from 254, 514/515 from 255, 478/479 from
264, 453 from 297 set the intro bit (`0x00572360`). Wants to talk
(`0x005B6E30`): cain3 with its intro bit clear. Status fn: false.

### 10. Hooks called from other systems

| Caller | Hook | Effect |
|---|---|---|
| monster creation, base-id switch `0x005B1CF0` | `0x005436B0` | base 407 → chain 17; 366 → chain 19 (and flags 0x20000); 242 → chain 20 unless class 704 |
| superunique creation `0x005A4440`, `0x005A49B0` | `0x005436B0` | superuniques 26, 27, 29 → chain 19; `0x005A49B0` then calls §7.5 |
| item creation `0x00555D20` | `0x005436B0` | every item with `quest` ≠ 0 links to chain `quest` − 1 (so pick-up reaches §3.5, §5.8, §6.7) |
| special monster creation `0x005A09E0` | `0x00544E80` | §6.2 |
| monster removal `0x005421A0` | `0x00544F20` | §6.2 |
| NPC map AI `0x005E7130` | `0x005BAD20`, `0x005BAD40` | Golden Bird +0x00 test / clear (Alkor) |
| NPC map AI `0x005E7130` | `0x005B9CA0`, `0x005B9CD0` | §5.7 (Ormus and the altar) |
| Dark Wanderer AI `0x005EA130` | `0x005BD0D0`, `0x005BD4A0` | §9.2 |
| cube (`world/cube.md` §8) | `0x005B86E0` | §4.8 |
| item use | `0x0055E170` | §6.6 |
| Durance warp `0x00546AC0` | `0x005BCFD0` | §8.6 (`quests.md` §8.1) |
| warp check `0x00545B80` | `0x005BBFA0` | §7.7 |
| Khalim chat / active | `0x005BBF80` | chain 19 absent or orb not smashed |

`0x005BBE20` (a status-like function for chain 19) is never stored or
called.

### 11. Clarifications (QC-1 … QC-7)

Moved to `world/quests-act3-2.md` §11 (size split): bit 17.3 never set,
Mephisto's progression step, the `&level` output of `0x00559A30`, the
boss-choice and Gidbinn kill tests, the orb's weapon accessor, the dead
map-AI hooks, and the positions and rooms of §5.6, §5.7, §9.1, §9.2.

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| message-state index tables | chain 15 `0x0073F188`, 17 `0x007401B8`, 19 `0x00740ED8`, 20 `0x00741518` | image (§3–§8) |
| Dark Wanderer minion offsets | 8 pairs (§9.2) | `0x00741538` |
| intro jump table | NPC − 245 → case, 0x35 bytes | `0x005B6CF4`, `0x005B6CD8` |
| levels | 75 Kurast Docks, 76 Spider Forest, 77 Great Marsh, 78 Flayer Jungle, 79 Lower Kurast, 82 Kurast Causeway, 98 Ruined Fane, 100–102 Durance of Hate 1–3, 104 Outer Steppes | `levels.txt` |
| objects | 131, 193, 251, 252, 341, 342, 366–368, 378, 379, 382, 386, 404–407 | `objects.txt` |
| NPCs / monsters | 242 mephisto, 245 cain3, 252 asheara, 253 hratli, 254 alkor, 255 ormus, 264 meshif2, 297 natalya, 301 vilechild1, 366 compellingorb, 368 darkwanderer, 407 fetish11 | `monstats.txt` |
| superuniques | 26 Ismail Vilehand, 27 Geleb Flamefinger, 29 Toorc Icefist | `superuniques.txt` |
| sounds | 19 refused, 64 council, 65 Gidbinn, 66 Mephisto, 67 tome, 72 figurine | §3–§8 |
| FX bytes (0x89) | 9 sewer lever, 10 orb, 11 Mephisto | §4.7, §7.7, §8.5 |
| timer periods (updater ticks) | 7 (boss), 12 (Mephisto), 2 (minions) | §5.6, §8.5, §9.2 |
| object event delays (frames) | 30 lever, 20 / 24 bridge | §4.7, §8.6 |
| rewards | stat 4 +5; stat 7 +0x1400; `rin ` rare at 21 / 35 / 75; mercenary from 252 | §3.3, §6.6, §5.5, §5.4 |

## Randomness

Quest seed (control +0x18) draws, in occurrence order of their events:

| When | Seed | Draws | Decides |
|---|---|---|---|
| each Khalim chest opening | quest | 1 (`lo' mod 5`), before the items | gold piles 5–9 |
| Dark Wanderer minion timer | quest | 1 (`lo' & 1`) | 7 or 8 minion dummies |

Quest items, the ring, the potion, gold, soulstones and the chest
treasure draw from their own item and object seeds (item, object
specs); spawned monsters from the monster spawn code. No other Act III
quest code draws.

## Edge cases & original bugs

1. The first message 564 to Alkor in a game rewards every player in Act
   III who lacks 17.0 and 17.1, whether or not the sender handed in a
   tome (§3.3); players elsewhere only get 17.14.
2. Picking up `qf2 ` re-tests 15.0 after requiring it, so its 0x5D
   (statuses 5 / 6) is never sent; only the status byte becomes 7
   (§4.5).
3. Khalim's chests drop gold before the parts and the treasure after
   them (Act II chests: items, treasure, gold); same order in D2MOO.
4. The Gidbinn boss timer tries once; with no player near the decoy at
   that moment no boss spawns until the decoy room is populated again
   (init 25, §5.6).
5. The Blade's pick-up sends status 4 then status 3 to all (§5.8).
6. Chat-end callbacks of chains 15, 17 and 18 are never cleared; they
   act only while their pending flags are set.
7. Cain's 531 (Golden Bird) does not install the chat-end callback; the
   status-4 send relies on an earlier install (§6.5).
8. Golden Bird table state 4 (msgs 534–537 rows) is never selected by
   the event-0 code (§6.4).
9. The Guardian starts on entering any level ≥ 98 (Ruined Fane,
   Disused Reliquary, the Durance), not only the Durance (§8.4; same in
   D2MOO).
10. At game start 22.3 maps to state 2 and 22.2 to state 3, the reverse
    of the flag iterate (§8.8 vs §8.3; same in D2MOO).
11. Mephisto's death sets state 6 and opens the Hellgate even in an
    intro game (§8.5).
12. The Compelling Orb needs two valid hits (+0x38 ≥ 2); the first does
    nothing (§7.7).
13. A game started by a player with 18.0 treats the orb as smashed
    (§7.4): the Durance opens without a smash.
14. Council flails drop again on every kill until one is created; while
    every player holds a flail or the Will, no cube can drop (§7.6).
15. The Dark Wanderer offset table repeats (−3, 3) and lacks (3, 3)
    (§9.2).
16. Khalim's status function never tests 18.0 (§4.9).
17. `0x005B7230`, `0x005BD040`, `0x005BCF60` (status re-send) and
    `0x005BBE20` have no caller in 1.14d.
18. Bit 17.3 is never set in 1.14d, so the §3.7 event-13 "17.3 → state
    3" branch runs only for a save that already carries the bit (`quests-act3-2.md` §11.1).
19. The Gidbinn boss is a random boss with champions allowed: it can be
    a champion fetish11 instead of a unique one (`quests-act3-2.md` §11.4).
20. `0x005B7120` (Hratli end position) has no caller; with `quests-act3-2.md` §11.6 the
    Hratli and Natalya map-AI branches are dead code.
21. A Dark Wanderer object whose point (x + 7, y) lies in no room near
    the object spawns no wanderer and keeps trying at each later init
    43 (`quests-act3-2.md` §11.7 rule 4).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| quest seed {12345, 666}, a Khalim chest opened | lo' 22752887 → 7 gold piles before the part | §4.6, `quests-act2.md` vectors |
| quest seed {12345, 666}, minion timer | lo' odd → start index 1 → 7 dummies at offsets 1–7 | §9.2 |
| Ormus 593 on nightmare, 19.6 set, 19.8 clear | `rin ` rare, level 35; 19.8 set | §5.5 |
| Alkor 564, 3 players: A (sender, in Act III), B in Act III, C in Act I; none has 17.0/17.1; +0x00 = 1 | A, B: 17.13, 17.0, +5 stat points, `5D 0F 02 00 0000`; C: 17.14 | §3.3 |
| pick up `qhr `, holding `qey `, `qbr `, no `qf1 ` | `5D 10 <flags> 03 0000` to the picker | §4.5 |
| pick up `qf1 `, holding all parts, 18.5 clear | v = 5 | §4.5 |
| Khalim status: 15.0 set, holds `qf2 `, orb not smashed | 6 | §4.9 |
| Lam Esen not done, Ormus 594 | state 3 (table state 2, msgs 596…) | §7.3 |
| Lam Esen done, Ormus 628 | Guardian state 2 | §8.2 |
| Guardian flag iterate, state 5, status 3 | 22.8 | §8.3 |
| game start with only 22.3 set | state 2, status 2 | §8.8 |
| Potion of Life used with 20.5 | stat 7 += 0x1400; `5D 12 02 00 0000` | §6.6 |
| Blade status, 19.6 set, 19.7 set, 19.8 clear | 6 | §5.9 |

## Provenance

- Read from the 1.14d `Game.exe` exports (`re/exports/funcs`,
  `functions.tsv`, `all.asm`) and `tools/ghidra/disasm.py` (register
  arguments: states of `0x00544350`, statuses of `0x00544300`, chains of
  `0x00543640` / `0x005436B0`, game-flag slots of `0x00544720` /
  `0x00544760`, timer periods and callbacks of `0x00543F10`, FX bytes,
  sound ids, the 0x5D status bytes of `0x005B81C0`; callbacks reachable
  only through pointers, disassembled with `disasm.py at`); raw image
  bytes for the index tables, the intro jump table, the minion offsets,
  the object operate / init tables and the monster / superunique link
  switches. Assert strings `d:\diablo2\…\Quests\a3q0.cpp` … `a3q7.cpp`
  identify the files. Live 1.14d `levels.txt`, `objects.txt`,
  `monstats.txt`, `superuniques.txt`, `misc.txt` (extracted patch_d2
  tables) gave the row names and item codes.
- D2MOO 1.10f `D2Game/src/QUESTS/ACT3/A3Q0–A3Q7.cpp` and headers gave
  names and structure; each rule above was matched to the 1.14d
  function named next to it. Checked to agree: the chest gold order, the
  orb's two hits, the ≥ Ruined Fane start, the 22.2 / 22.3 start order,
  the global Lam Esen test in A3Q5 / A3Q6.
- No packet or RNG recording of Act III exists yet.
- corrected: level id vs Levels.txt (Act IV writer finding). Init 44 tests
  the room's level id (`0x0061A1B0`) against 104; the `levels.txt` Id
  column is that runtime id (row 0 Null, no offset), so 104 is Outer
  Steppes (Act 4 - Mesa 1), not Pandemonium Fortress (103).
- §11 clarifications: provenance in `world/quests-act3-2.md`.

## Open questions

1. Status meanings per Act III quest (client quest log): settle with
   `quests.md` open question 1.
2. Answered (`quests-act3-2.md` §11.3): `&level` is an output, written at `0x00559AF8`
   before any read; the item level is the source monster's stat 12 or
   the object's area level.
3. Answered (`quests-act3-2.md` §11.4): mask 0x40 at `0x006CE280` on byte +0x0D = flag
   word bit 14, the `flying` column.
4. Answered (`quests-act3-2.md` §11.4): type flags & 0x0E (superunique, champion,
   unique) or monstats `boss`; the spawn is the random boss of
   `monsters/init.md` §16.1 with champion allowed = 1.
5. Answered (`quests-act3-2.md` §11.2): `quests-act1-rest.md` §5, n = (4 or 5)·difficulty
   + 3 into client +0x0A bits 8–12 (`0x005BC182`).
6. Answered (`quests-act3-2.md` §11.5): the weapon in use (inventory +0x1C GUID) at body
   location 5, else 4, of item type `weap`.
7. Answered (cross-file request to PC 2 quests-core,
   `docs/handoff/pc2-spec-quests-act3.md`): `quests.tsv` rows 17–24 and
   39, column `spec` := `specified`; owner of those rows is this file.
8. Record a full Act III run (packets + RNG, `docs/HANDOFF.md` §5):
   chest drops, the Alkor reward broadcast, council kills, the orb, the
   Hellgate and Natalya's spawn.
