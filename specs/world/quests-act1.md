# Spec: World — Quests, Act I (§10 of the quest system)

- **Status:** draft: the shared machinery (flag records, quest records,
  creation, game-entry sequencing, event dispatch, updater, status
  messages, act transitions) is read from the 1.14d `Game.exe` (addresses
  below) and its message bytes match the two hand-played packet
  recordings (`traces/raw/20261006-015956-packets.jsonl`,
  `20261006-022633-packets.jsonl`: 0x5E, 0x28, 0x29, 0x5D, 0x8A, C→S 0x31;
  Test vectors). Act I quests are specified at the level of their
  triggers, flags, rewards, timers and draws; A1Q1 (§10.4) and the
  sequence functions (§10.1) are specified callback by callback from
  the 1.14d disassembly; Acts II–V are catalogued only (`quests.tsv`
  column `spec`).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests::act1`
- **Related specs:** `world/quests.md` (owner of the shared machinery §1–§9 and §11, its Constants, Randomness, Edge cases, Test vectors, Provenance and Open questions; this file holds §10 moved out of it unchanged, rule ids kept); `world/quests-act1-rest.md`; `world/quests.tsv`, `world/quest-messages.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 26–32 |
| Rules | 33–34 |
|   10. Act I quests | 35–829 |
<!-- /index -->

## Summary

§10 of `world/quests.md`, split out of that file to keep it readable
(section numbers and rule ids are unchanged, so a reference names
this file and the same §). Status, evidence and open questions are
those of `world/quests.md`.

## Rules

### 10. Act I quests

#### 10.1 Common pattern

Act I quests 1, 2, 5, 6 share a shape (verified per quest below):

| State (+0x0C) | Reached when | Player bits set |
|---|---|---|
| 0 | init (1 for A1Q1) | — |
| 1 | the sequence function of the quest before it in the chain (`seq_id`) | — |
| 2 | the quest NPC's start message (C→S 0x31) | 2 (STARTED), to players without bit 0 or 1 |
| 3 | leaving town / entering the area (event 3) | 3 (LEAVETOWN) or 4 (ENTERAREA) |
| 4 | goal done (kill, event 8) | 13 + 1 (reward pending) for qualifying players; 14 (COMPLETEDNOW) and a 0x5D (`quests.md` §6.3) |
| 5 | the NPC's completion message | 0 (granted), 1 cleared; reward |

Callback 13 (player started game) restores state from the starting
player's bits: bit 4 → state 3, status 2; bit 3 → state 3, status 1;
bit 2 → state 2, status 1 (unless bit 0 or 15 is set). Chain 1:
`0x00590690`, chain 6: `0x00596900` (same steps; the slot is a constant
in each). Event 2 (chat end) after the start message sends status 1 to
every player (`quests.md` §6.3; chain 1: §10.4).

Shorthands used in §10.4–§10.8:

- **broadcast(S, f)**: flags (+0x14) := f, then `0x00544300(record, S,
  0, I, 1)`: status (+0x0B) := S and the quest's status iterate I runs
  for every player. **status(S)** is `0x00544300` with iterate 0:
  status := S, nothing sent.
- **every player F**: `0x005537D0(game, 0, arg, F)` (`sim/unit-order.md`
  §2 r5: hash order, players with state 7 skipped, stops at a call
  returning 1); every Act I iterate function returns 0, so all players
  are visited.
- **add state k**: `quests.md` §7.1 `0x00543790(record, text list, NPC class, k)`.
- R is the event player's current-difficulty record (`quests.md` §1.4).

Sequence functions (record +0xF0) unlock the next quest. Each is called
with its own record (`quests.md` §3 step 3, reward handlers) and runs:

1. Own step (column 3): if it applies, do it and return 1.
2. If state ≠ the pass state and not-intro ≠ 0: return 1.
3. next = lookup(`seq_id`) (`quests.md` §2.3); none → return 0. Fatal if the
   caller's own +0xF0 pointer is invalid (`IsBadCodePtr`); otherwise
   call next's sequence function with next's record and return its
   result.

| Chain | Function | Own step | Pass state | `seq_id` |
|---|---|---|---|---|
| 1 | `0x00590620` | none | 5 | 2 |
| 2 | `0x005910F0` | state 0 and not-intro = 1 → state := 1 | 5 | 4 |
| 3 | `0x00591E40` | state 0 and not-intro = 1 → state := 1 | 5 | 6 |
| 4 | `0x00593D70` | state 0 and not-intro = 1 → state := 1 (plus a debug log) | 6 | 3 |
| 5 | `0x00595240` | state < 2 and not-intro = 1 → nothing (return 1) | 5 | 3 |
| 6 | `0x005968E0` | state 0 and not-intro = 1 → timer (record, `0x00596580`, period 20) (§10.8); in every case return 1, never passes on | — | 37 (unused) |

So the unlock order from Den of Evil is 1 → 2 → 4 → 3 → 6: a quest
already done (state 5; chain 4: 6) or switched off (`quests.md` §3) passes the call
on, a quest at state 0 opens, any other quest stops the walk. Set-state
`0x00544350` and `0x00544070` only add a debug log when the global
`0x008846DC` ≠ 0; they change nothing else.

#### 10.2 Act I table

| Quest | Chain / slot | NPC (class) | Start msg | Goal | Reward msg | Reward | Timer |
|---|---|---|---|---|---|---|---|
| Warriv gossip | 0 / 0 | warriv1 (155) | 0 or 1 | — | — | sets 0.0 | — |
| Den of Evil | 1 / 1 (+41) | akara (148) | 64 | clear level 8 | 76 | +1 skill point (stat 5); slot 41 bits 13, 1 | 8 |
| Sisters' Burial Grounds | 2 / 2 | kashya (150) | 81 | kill Blood Raven | 92 | mercenary from Kashya (`0x00579180(150)`) | 15 |
| Tools of the Trade | 3 / 3 | charsi (154) | 146 | bring the Horadric Malus | 163 | imbue (NPC menu, `0x00591790` grants) | — |
| Search for Cain | 4 / 4 | akara (148), cain5 (265) | 97 | free Cain | 118 | ring (§10.6) | 1 (D2MOO) |
| Forgotten Tower | 5 / 5 | (tower tome, msg 127) | 127 | kill the Countess | — (granted on kill) | — | 7 |
| Sisters to the Slaughter | 6 / 6 | cain5 (265), warriv1 (155) | 166 | kill Andariel | 183 (Warriv) | gems (§10.8) | 1 |

Message numbers are the C→S 0x31 message index; all confirmed as
compare constants in the 1.14d scroll callbacks (`quests.tsv` callback
11).

#### 10.3 A1Q0 Warriv gossip, A1Q7 Flavie, the respec record, Act I intro

- A1Q0 (`0x0058FB80`): event 0 for warriv1 when slot 0 bit 0 is clear
  adds state 1 messages for a paladin, else state 0; event 11 with
  message 0 or 1 from warriv1 sets slot 0 bit 0. Active fn: warriv1 and
  slot 0 bit 0 clear. Status fn returns false.
- A1Q7 (chain 25, `0x00596C40`; callbacks 0 = `0x00596A90`, 8 = a bare
  `ret`): event 0 for navi (class 266, Flavie): if the game record slot 1
  bit 13 is clear, chain 1's record is not-intro, and the player has none
  of slot 1 bits 0, 13, 1: state = roll(player unit seed, 2) (states 0–1);
  else state = (step of the player unit seed, lo' mod 3) + 2 (states
  2–4); add the state's messages. Active fn: navi, chain 1 not-intro,
  game slot 1 bit 13 clear, player lacks slot 1 bits 0, 13, 1.
- Chain 30 (table row 7, flag slot 41) uses the same init function, so
  it is a second Flavie record with filter 29 and the same table; its
  only distinct role is the `quests.md` §3 step 2a test on slot 41. Both records
  handle event 0 (edge case 2).
- Slot 41 (respec): Den of Evil's reward message also sets slot 41 bits
  13 and 1 (`flag2`). The NPC respec action (Akara menu action 0x94,
  `0x00579D60`, owned by `world/npc.md`) uses `0x0058FD20` (set 41.13,
  41.1; used in Hell for a player with slot 1 bit 0 and neither 41.1 nor
  41.0) and `0x0058FD50` (after the respec: set 41.0, clear 41.1; if
  41.15 is clear, chain 30's record gets active := 0; lookup failure
  is fatal).
- Status functions of chains 25 / 30 (`0x00596BB0`) and 37
  (`0x0058F9B0`, which also writes out := 0) return false: nothing is
  reported. Flavie's active fn is `0x00596BC0` (as above; chain 1's
  record is looked up without a null check).
- Act I intro (chain 37, `0x0058FA20`): per-NPC first-talk text, kept in
  the NPC intro record (`quests.md` §6.7; set `0x00572360`, test `0x005723C0`, both
  by NPC class through the class table `0x00732738`).

  | NPC (class) | Special-text player class | Event 11 messages that set the intro bit |
  |---|---|---|
  | gheed (147) | 2 necromancer | 45, 46 |
  | akara (148) | 1 sorceress | 11, 12 |
  | kashya (150) | 0 amazon | 24, 25 |
  | charsi (154) | 4 barbarian | 36, 37 |

  1. **Event 0** `0x0058F8F0`: only the four NPCs above (jump table on
     class − 147). If the player's intro bit for the NPC is set: nothing.
     Else add state 1 if the player's class (unit +4; −1 when no player)
     is the NPC's special class, else state 0.
  2. **Event 11** `0x0058F870`: NPC and message in the table → set the
     player's intro bit for that NPC. Nothing else (no 0x27 refresh).
  3. **Active** `0x0058F9C0`: true iff the player's record lacks 1.0,
     the NPC is akara (148) and the player's akara intro bit is clear.

#### 10.4 A1Q1 Den of Evil (chain 1)

Init `0x00590720`: callbacks per `quests.tsv`; active 1, state 1,
init_no 4, seq_id 2, filter 1, flag2 41, status fn none (default rule,
`quests.md` §6.1), active fn `0x005905B0`, seq fn `0x00590620`; extra = 0x8C zeroed
bytes from the game pool, GUID list emptied (`0x00545300`).

| Extra | Type | Field |
|---|---|---|
| +0x00 | 32 × u32, u16 count at +0x80 | killers: players who killed a level-8 monster (`quests.md` §9.3 list) |
| +0x84 | u8 | cleared (written, never read) |
| +0x85 | u8 | entered (written, never read) |
| +0x86 | u8 | talked: message 64 given, chat end not yet handled |
| +0x87 | u8 | timer pending |
| +0x88 | i32 | monsters left; `0x005901E0` returns it (0x50, 0x5D) |

Iterate functions (game, player, arg; all return 0; slot 1 is a
constant in each):

| Id | Function | Effect on one player P |
|---|---|---|
| I1 | `0x0058FBE0` | if P has neither 1.0 nor 1.15, or has 1.13 or 1.14: 0x5D for chain 1 (`0x00544190`, `quests.md` §6.3) |
| I2 | `0x0058FC90` | if P has neither 1.0 nor 1.1: chain 1 state 2 → set 1.2; state 3 → set 1.3 if status = 1, else 1.4; other states nothing |
| I3 | `0x00590190` | if P has 1.13 and P's party id ≠ 0xFFFF (`0x00554630`: a player unit with +0x80 ≠ 0 whose GUID is in a party of the list at game +0x1D2C, else 0xFFFF): for each member of that party (list order; GUID lookup `0x00552F60`, missing ones skipped) run `0x00590120`: if the member has neither 1.0 nor 1.1 and a room whose level is ≠ 0 and in Act I (`0x006427F0` = 0): set 1.13, then 1.1 (no 0x28) |
| I4 | `0x00590080` | if P has neither 1.0 nor 1.1: set 1.14; `5D 01 00 0C 0000` to P (`0x00545920` with act 0: sent unless P's room has level 0); 0x28 to P (`quests.md` §6.6) |
| I5 | `0x005900E0` | if P has 1.13: sound event 35 on P, target P (`0x00553380`) |

Chain 1's broadcast iterate is I1.

**Event 0** `0x0058FF90` (NPC text; target = the NPC, class = unit +4,
−1 when none):

1. If R has 1.1: add state 3. End.
2. If the player's GUID (−1 when no player) is in the record's GUID list
   (`0x005452C0`): add state 4. End.
3. End if R has 1.0, or state ≥ 4 and R lacks 1.13, or not-intro = 0.
4. m = `0x00736CD0`[state] (−1, 0, 1, 2, 3, 4 for states 0–5); m = −1
   (or ≥ 8) → end; else add state m.

**Event 2** `0x0058FC40`: if the target exists, its class is 148 (akara)
and talked = 1: broadcast(1, 0); talked := 0; callback 2 := null.

**Event 3** `0x00590470` (old level a, new level b):

1. b = 8: end if not-intro = 0. changed := (state is 1 or 2); if so
   state := 3. entered := 1. If status < 2: broadcast(2, 0), callback 2
   := null, then every player I2. Else if changed: every player I2.
2. b ≠ 8 and a = 1 (leaving the Rogue Encampment): if the record's GUID
   list is not empty, remove the player's GUID (`0x00545310`). Then, if
   state = 2 and R has neither 1.0 nor 1.1: state := 3; every player
   I2; if status ≠ 1: broadcast(1, 0), callback 2 := null.
3. Otherwise nothing.

In step 2, I2 runs before the status changes: a player who leaves town
while the status is still 0 (start message given, chat not yet ended)
gets 1.4, not 1.3 (bug kept).

**Event 8** `0x00590260` (a monster with a chain-1 link dies, `quests.md` §4.4;
victim = target, killer = the player argument):

1. End if not-intro = 0.
2. M = monster region of level 8 (`0x00547BB0(game +0xF0, 8)`; none →
   fatal). left := M spawned (+0x2CC) − M killed (+0x2D0)
   (`monsters/population.md` §2.2).
3. If there is a killer and its GUID is not in the killers list: add it.
4. P = populated-room count of level 8 (`0x0061ABF0(act DRLG of level 8,
   8)`), V = M rooms visited (+0x04).
5. If P ≤ V and killed = spawned (cleared):
   1. cleared := 1; callback 2 := null; state := 4; callback 8 := null.
   2. Game record: set 1.13 (`0x00544720`).
   3. Killers list: `0x005455F0(game, list, 1, sound 0)` (`quests.md` §9.3: set 1.13,
      1.1 for each killer lacking 1.0 and 1.1).
   4. Every player I3, then every player I4, then every player I5 (arg =
      the victim, unused).
   5. `0x00545760(game, 0)`: every player gets 0x28 then `89 00` (`quests.md` §6.5).
   6. If timer pending = 0: timer pending := 1; timer (record,
      `0x00590230`, period 8) (`quests.md` §5).
6. Else, if P ≤ V and left ≤ 5 (signed): broadcast(4, 0x20); callback
   2 := null.
7. Else, if status = 4 and left > 5: broadcast(4, 0x20).

Steps 6 and 7 repeat on every qualifying kill (one 0x5D per kill and
player). Timer `0x00590230`: if state = 4, broadcast(5, 0); timer
pending := 0; return 1 (runs once, 9 updater ticks after the clearing
kill).

**Event 10** `0x005901F0`: remove the player's GUID (−1 when none) from
the record's GUID list, then from the killers list (`0x00545240`).

**Event 11** `0x0058FDD0` (NPC class a, message b): only class 148.

1. b = 64: talked := 1; state := 2 (no guard on the current state);
   every player I2; refresh the text (`quests.md` §7.2, with the target NPC). End.
2. b = 76 and R has 1.1:
   1. If R has 1.13: if state ≠ 5: state := 5, run the sequence function
      (§10.1), flags := 0, status(13). Then (state 5 or not) callback 2
      := null.
   2. Set 1.0, clear 1.1, set 41.13 and 41.1 (`flag2`), clear bits 2–11
      of slot 1 (`0x0065C3E0`), stat 5 (new skill points) += 1 on the
      player (`0x006272B0(player, 5, 1, 0)`), add the player's GUID to
      the record's GUID list, refresh the text.
3. Other messages: nothing.

**Event 13** `0x00590690`: §10.1 restore with slot 1.

**Active** `0x005905B0` (`quests.md` §6.4): true iff the NPC class is 148, R lacks
1.0, and (R has 1.1, or not-intro = 1 and state = 1).

**Sequence** `0x00590620`: §10.1.

No draws. 0x50 and 0x5D carry `left` while the status ≠ 0 (`quests.md` §6.2,
`quests.md` §6.3); `quests.md` §3 step 8 sends `89 00` to later joiners once state ≥ 4.

#### 10.5 A1Q2 Sisters' Burial Grounds (chain 2) and A1Q3 Tools of the Trade (chain 3)

**A1Q2.** Init `0x00591210`: callbacks per `quests.tsv`; active 1, state
0, init_no 4, seq_id 4, filter 2, status fn none, active fn
`0x00591080`, seq fn `0x005910F0`; extra 0x0C bytes: +0 u8 killed, +1 u8,
+2 u8 (both set on the kill), +3 u8 talked, +4 u32 (set to 1 on the
kill; not initialized, never read by chain 2), +8 u32 victim GUID; init
zeroes +0..+3 and +8.

| Id | Function | Effect on one player P (slot 2, chain 2) |
|---|---|---|
| J1 | `0x00590830` | broadcast iterate: as I1 (§10.4) for slot 2, 0x5D for chain 2 |
| J2 | `0x00590890` | as I2 for slot 2 and chain 2's state / status |
| J3 | `0x00590C40` | chain 2's record (lookup fatal if absent); end unless extra +2 ≠ 0; V = the monster with GUID extra +8 (`0x00552F60`, type 1), fatal if V is missing; end if P has no room; near := P's room is V's room, or V's room (none when V has no room) is in P's room's room list (`0x00619790`, `drlg/rooms.md` §10.4); if P has neither 2.0 nor 2.1 and near: set 2.13, then 2.1 |
| J4 | `0x00590D60` | party member: as `0x00590120` (§10.4 I3) for slot 2 |
| J5 | `0x00590DD0` | if P has neither 2.0 nor 2.1: set 2.14; `5D 02 00 0C 0000` (`0x00545920`, act 0); no 0x28 |
| J6 | `0x00590E30` | if P has 2.13: sound event 34 on P, target P |
| J7 | `0x00590E70` | if P has 2.13 and a party: J4 for each member (as I3) |

1. **Event 0** `0x00590B10`: R has 2.1 → add state 3; else player GUID
   in the record's list → add state 4; else if state ≠ 0, R lacks 2.0
   and state < 4: m = `0x00737180`[state] (−1, 0, 1, 2); m ≠ −1 → add
   state m. No not-intro test.
2. **Event 2** `0x00590920`: target class 150 (kashya) and talked = 1:
   broadcast(1, 0); talked := 0; callback 2 := null; every player J2.
3. **Event 3** `0x00590FA0` (old a, new b): if b = 17 (Burial Grounds,
   1.14d constant) and not-intro = 1: changed := (state < 3, so also
   from state 0); if so state := 3, flags := 0. If status ≤ 1:
   status 2 through J1 (flags as they are), then every player J2. Else
   if changed: every player J2. Otherwise (b ≠ 17 or not-intro ≠ 1),
   if a = 1: remove the player's GUID from a non-empty record list
   (`0x00545310`); if state = 2 and R lacks 2.0 and 2.1: state := 3,
   every player J2 (no status message).
4. **Event 8** `0x00590EC0` (any monster with a chain-2 link; the killer
   is not read): end if not-intro = 0. state := 4; extra +1 := 1, +2 :=
   1, +8 := victim GUID (−1 if none). Every player J3, then J7, then J5,
   then J6 (arg = the victim). Timer (record, `0x00590BF0`, period 15).
   callback 2 := null; killed := 1; extra +4 := 1; game record 2.13.
   Callback 8 stays: a second linked death repeats all of it.
5. Timer `0x00590BF0`: broadcast(3, 0); return 1 (runs once).
6. **Event 10** `0x00590C10`: remove the player's GUID from the record's
   list.
7. **Event 11** `0x00590980` (only class 150): message 81: talked := 1;
   state := 2 (no guard); every player J2; refresh text. Message 92 with
   R 2.1: if R has 2.13 and state ≠ 5: flags := 0, status(13), state :=
   5, run the sequence function (§10.1). Then (always) set 2.0, clear
   2.1, 0x28 to the player, add its GUID to the record's list, Kashya's
   mercenary (`world/npc.md` §7.5, `0x00579180(game, player, 150)`),
   refresh text. Bits 2–11 stay; callback 2 is not cleared.
8. **Event 13** `0x00591180`: §10.1 restore, slot 2.
9. **Active** `0x00591080`: as A1Q1's (§10.4) with class 150 and slot 2.

**A1Q3.** Init `0x00591F70`: callbacks per `quests.tsv`; active 1, state
0, init_no 5, seq_id 6, filter 3, status fn `0x00591D30`, active fn
`0x00591C30`, seq fn `0x00591E40`; extra 0xA4 zeroed bytes, GUID list at
+0x14 emptied.

| Extra | Type | Field |
|---|---|---|
| +0x01 | u8 | Malus object known (set by the object init and the drop) |
| +0x02 | u8 | talked: message 146 given, chat end not yet handled |
| +0x03 | u8 | rewarded: message 163 finished the quest, chat end not yet handled |
| +0x04 | u32 | Malus object GUID |
| +0x08 | u8 | cleared by `0x005918D0` |
| +0x14 | GUID list (count u16 +0x94) | players who brought the Malus (message 163) |
| +0x98 | i32 | Malus object mode to restore: 2 = the Malus was taken |
| +0x9C | i32 | Malus items in the game (players carrying `hdm `) |
| +0xA0 | u8 | scratch for the party test of the status fn |
| +0xA1 | u8 | a player started the game carrying `hdm ` |

"has hdm" = the player has an item with code `hdm ` (`0x00558110`);
"level" = the player's base stat 12 (`0x006253B0`, layer 0).

| Id | Function | Effect on one player P (slot 3) |
|---|---|---|
| K1 | `0x005912E0` | broadcast iterate: as I1 for slot 3, 0x5D for chain 3 |
| K2 | `0x00591340` | if P has neither 3.0 nor 3.1: chain 3 state 2 → set 3.2; state 3 or 4 → set 3.3 |
| K3 | `0x00591430` | party member: if it has neither 3.0 nor 3.1 and level ≥ 8: set 3.13, 3.1 |
| K4 | `0x00591CD0` | true if P has hdm; else, with a party, extra +0xA0 := 0, each member with hdm sets it to 1 (`0x00591CA0`), true iff +0xA0 ≠ 0; no party → false |

1. **Object init** (Malus object; pointer at `0x00731BFC` →
   `0x00544950`): if chain 3's record exists (`0x00592090`): Malus known
   := 1, GUID := the object's; if not-intro = 0, extra +0x98 := 2; object
   mode := extra +0x98 (`0x00624690`). Without the record: object mode
   := 2 unless it is 2 already.
2. **Operate** `0x00591AC0` (pointer at `0x00732D6C`; args game, object,
   player; returns 0):
   1. No chain 3 record → end. mode = the object's mode (+0x10; 0 when
      no object).
   2. not-intro = 0: object mode := 2; sound event 19 on the player. End.
   3. mode ≠ 0, or R has 3.0 or 3.1 → end.
   4. Level < 8: sound event 19 on the player. End (nothing drops).
   5. Object drop code (+0xB8) := `hdm `; drop at the object
      (`0x00559A30(game, object, 2, &out, 0, −1, 0)`, items spec); failed
      → end.
   6. Object mode := 2; Malus known := 1; +0x98 := 2; +0x9C += 1; GUID
      := the object's.
   7. If state ≠ 4: state := 4; every player K2.
   8. If status ≠ 1: flags := 0; status(1) (nothing sent).
3. **Event 0** `0x005916A0`: end if R has 3.0 but not 3.13. If the
   player has hdm: if level ≥ 8 and R lacks 3.0, add state 3. Else: end
   if R has 3.0 or state is 0 or 4; m = `0x00737630`[state] (−1, 0, 1,
   2, 3, 4); m ≠ −1 → add state m.
4. **Event 2** `0x005913C0` (target class 154, charsi): if talked = 1:
   every player K2, broadcast(1, 0), talked := 0. Else if rewarded = 1:
   broadcast(13, 0), rewarded := 0.
5. **Event 3** `0x00591810`: not-intro = 0 → callback 3 := null, end. If
   a = 1, state = 2 and R lacks 3.0 and 3.1: if status ≠ 1,
   broadcast(1, 0); state := 3; every player K2; callback 3 := null.
6. **Event 4** `0x00591960` (item picked up, along the item's chain, so
   only while the record is active, `quests.md` §4.3): if R lacks 3.6: set 3.6,
   sound event 36 on the player. Then broadcast(2, 0).
7. **Event 6** `0x00591A90` (never raised in 1.14d, `quests.md` §4.1): if
   not-intro ≠ 0: +0x9C −= 1; if it reaches 0 and +0x98 = 2: reset
   (`0x005918D0`, below).
8. **Event 9** `0x00591A20` (a player leaves with the Malus): if
   not-intro ≠ 0 and state ≠ 5: +0x9C −= 1; if it reaches 0, Malus
   known ≠ 0 and +0x98 = 2: broadcast(3, 0).
9. **Event 10** `0x00591A60`: remove the player's GUID from the +0x14
   list.
10. **Event 11** `0x00591490` (only class 154):
    1. Message 146: state := 2 (no guard); talked := 1; refresh text.
    2. Message 163: end if R has 3.0 (no refresh). Add the player's GUID
       to the +0x14 list. Without hdm: refresh text, end. With hdm: set
       3.13, 3.1; 0x28 to the player; delete its `hdm ` (`quests.md` §9.2); +0x9C
       −= 1; with a party, K3 for each member. Then, if not-intro ≠ 0:
       if state = 4: state := 5, rewarded := 1, game record 3.13, run
       the sequence function (§10.1); else if +0xA1 ≠ 0 and the
       `seq_id` record (6) exists: run that record's sequence function
       (§10.8). Refresh text.
    3. Other messages: nothing.
11. **Event 13** `0x00591ED0`: if the player has hdm: +0x9C += 1, +0xA1
    := 1. Then unless R has 3.0 or 3.15: 3.2 → state 2, status 1; else
    3.3 → state 3, status 1.
12. **Event 14** `0x005919D0` (player joins): if the player has hdm,
    not-intro ≠ 0 and state ≠ 5: +0x9C += 1; if it is now 1, Malus
    known ≠ 0 and +0x98 = 2: status := 2 (nothing sent).
13. **Status** `0x00591D30` (writes out, always returns 1): out := 0; R has 3.1 → 10. Else K4 true → 2 if R lacks
    3.0, else 0. Else not-intro = 0 → 0. Else R has 3.13 → 13; R has
    3.14 → 12; state < 5 → the status byte; else game record 3.13 →
    (level ≥ 8 ? 12 : 4); else 0.
14. **Active** `0x00591C30`: true iff class 154, R lacks 3.0, and either
    (state = 1 and R lacks 3.1) or (level ≥ 8 and the player has hdm).
15. **Reset** `0x005918D0` (from event 6 only): returns 0 if not-intro
    = 0. +0x98 := 0; end (0) if Malus known = 0. state := 3; flags := 1;
    status 1 through K1; +0x08 := 0. The object with the stored GUID
    (`0x00552F60` type 2) of class 108: mode := 0, return 1; else Malus
    known := 0, return 1.
16. **Imbue granted** `0x00591790` (from the NPC imbue, `world/npc.md`):
    set 3.0, clear 3.1; if R lacks 3.15: chain 3's record (fatal if
    absent) gets active := 0.
17. **Sequence** `0x00591E40`: §10.1.

No draws in A1Q2 or A1Q3 (the Malus drop draws from the item seeds,
items spec). The Malus needs level 8 in 1.14d (D2MOO 1.10f drops it at
any level).

#### 10.6 A1Q4 The Search for Cain (chain 4)

Init `0x005971B0`: callbacks per `quests.tsv`; active 1, state 0,
init_no 6, seq_id 3, filter 4, status fn none, active fn `0x00592FB0`,
seq fn `0x00593D70`; extra 0x1BC zeroed bytes, GUID lists at +0xB4 and
+0x138 emptied. Extra fields read or written by the functions below:

| Extra | Type | Field |
|---|---|---|
| +0x00 | 5 × u16 | Cairn stone order (17–21); +0x0C u16 cleared with it |
| +0x28, +0x49 | u32, u8 | GUID of the object of class 61 linked to chain 4 (`quests.md` §4.6, `0x00592F80`), and "linked" := 1 |
| +0x30, +0x47 | u32, u8 | GUID of an object of class 30 (Inifuss tree; D2MOO name), and "known" |
| +0x34, +0x48 | u32, u8 | GUID of an object of class 26 (Cain's gibbet), and "known" |
| +0x38 | u32 | GUID of the `bkd ` made by message 112 |
| +0x46 | u8 | Cain-removal timer pending |
| +0x4A | u8 | stone order computed |
| +0x4B–+0x4F | u8 | progress flags (+0x4E, +0x4B: scroll deciphered; +0x4F: tested by events 0, 6, 9) |
| +0x50 | u8 | Cain gone from Tristram (`0x00596CA0`) |
| +0x51, +0x52 | u8 | Cain spawned in the Rogue Encampment; Cain still to spawn there |
| +0x53 | u8 | talked: message 97 given, chat end not yet handled |
| +0x54, +0x58 | i32 | 3 when the gibbet is open; progress marker (1 or 0) |
| +0x61 | u8 | the Tristram Cain was removed |
| +0x63 | u8 | the reward message must set game 4.13 |
| +0x64 | u8 | scroll deciphered, chat end not yet handled |
| +0x68 | u32 | GUID of the spawned town Cain |
| +0x6C, +0x70 | u32, u8 | GUID of the town-Cain marker object (class 385; `quests-act1-rest.md` §3), and "known" |
| +0x78 | u8 | set with the scroll states |
| +0x7C | i32 | `bks ` / `bkd ` items in the game |
| +0x93 | u8 | set at chat end with class 146 |
| +0xB4 | GUID list | players credited when Cain reached Act II without them |
| +0x138 | GUID list | players who heard Cain's message 123 or 126 |

| Id | Function | Effect on one player P (slot 4) |
|---|---|---|
| L1 | `0x005920D0` | broadcast iterate: as I1 for slot 4, 0x5D for chain 4 |
| L2 | `0x00592130` | if P has neither 4.0 nor 4.1: state 2 → set 4.2; state 3–5 → set 4.3 |
| L3 | `0x00592B10` | if P has neither 4.0 nor 4.1: set 4.14; add P's GUID (−1 when none) to the +0xB4 list |
| L4 | `0x00593130` | if P has neither 4.0 nor 4.1 and P's room's level is 38 (Tristram): set 4.13, 4.1; 0x28 to P; with a party, each member: if it has neither 4.0 nor 4.1 and its room's level is ≠ 0 and in Act I, set 4.13, 4.1 and send it 0x28 (`0x005930B0`) |
| L5 | `0x005931C0` | if P has neither 4.0 nor 4.1: set 4.14; `5D 04 00 0C 0000` (act 0) |

1. **Event 0** `0x00592580` (NPC class c, −1 when none):
   1. c = 146: add state 9 with NPC class 146; continue.
   2. If +0x4F = 1 and the player has `bkd `: delete it (`quests.md` §9.2), +0x7C −=
      1.
   3. c = 265 (cain5), the player is not in the +0x138 list and R has
      4.13: add state 5. End.
   4. R has 4.1: add state 7 if c = 265 and the player is in the +0x138
      list, else state 5. End.
   5. Player in the +0xB4 list: add state 6. End.
   6. Player in the record's list: if c = 265 and not in the +0x138
      list, add state 5; else R has 4.14 → state 8; else R has 4.0 →
      state 7; else nothing. End.
   7. End if R has 4.14, state = 0, R has 4.0 or R has 4.15. The player
      has `bks ` → add state 3. Else state 4 → add state 2. Else m =
      `0x00737648`[state] for states 1–5 (0, 1, 2, 3, 4); add state m.
2. **Event 2** `0x005921B0`: c = 148 (akara): if talked = 1:
   broadcast(1, 0), talked := 0, every player L2. Then, if +0x64 = 1:
   broadcast(3, 0), every player L2, +0x64 := 0. c = 146: +0x93 := 1.
3. **Event 3** `0x00596DE0` (old a, new b):
   1. b = 38, +0x51 = 0, +0x50 = 0 and state ≥ 6: state := 5; flags :=
      0; status(4); every player L2.
   2. a = 1: remove the player's GUID from a non-empty record list and
      from the +0xB4 list. If R has 4.0 or 4.1: end. If state = 2: state
      := 3.
   3. b = 1: if +0x70 ≠ 0, the object with GUID +0x6C exists, +0x52 = 1
      and +0x51 = 0: spawn the town Cain beside it (step 15). End.
   4. b = 40 (Lut Gholein): if R lacks 4.0 and 4.1, +0x50 = 0 and state
      < 6: Cain cleanup (step 14, arguments 0, 1); state := 7; broadcast(5,
      0); game record 4.13; every player L3; +0x58 := 1.
4. **Event 4** `0x00592E20` (item picked up; active records only): if
   not-intro ≠ 0, every player L2.
5. **Event 6** `0x00592E60` (never raised, `quests.md` §4.1): if +0x4F ≠ 1 and state
   ≤ 5: state := 3; tree reset (step 13).
6. **Event 8** `0x00593E70` (the Cow King's death; dispatched with force,
   `quests.md` §4.4):
   1. With a killer player: end if R has 4.10; end if the game is
      expansion (+0x70) and R lacks 40.0, or classic and R lacks 26.0.
      Set R 4.10.
   2. Every player whose room's level is 39 (Moo Moo Farm,
      `0x00593E30`): set 4.10.
   3. `0x00545990` (a `ret 4` stub).
   4. Victim drop code (+0xB8) := `vps `; 8 drops at the victim
      (`0x00559A30(game, victim, 0, &out, 0, −1, 0)`).
7. **Event 9** `0x00592C80` (a player leaves with a quest item; target =
   the item): if not-intro ≠ 0 and state ≠ 6: if its code (`0x00628590`)
   is `bkd ` or `bks `, +0x7C −= 1. Then, if +0x7C = 0, state ≤ 5, +0x4F
   ≠ 1 and +0x47 = 1: state := 3; tree reset (step 13).
8. **Event 10** `0x00592CF0`: if R has 4.1 and 4.0 and the record list
   is not empty, remove the player's GUID from it (`0x00545530`); remove
   it from the +0xB4 and +0x138 lists.
9. **Event 11** `0x00592250` (class a, message b):
   1. Akara, 97: talked := 1; state := 2 (no guard); refresh text.
   2. Akara, 112, the player has `bks `: delete it; create `bkd ` (`quests.md` §9.1:
      level 0, quality 2, droppable). Created: +0x64, +0x4B, +0x4E := 1;
      +0x38 := its GUID; state := 5; flags := 0; status(3). Not created:
      +0x7C −= 1. Without `bks `: nothing.
   3. Akara, 118, R has 4.1: set 4.0, clear 4.1; add the GUID to the
      record list; 0x28; ring `rin ` (`quests.md` §9.1, droppable): Normal level 7
      quality 4, Nightmare 30 / 6, Hell 60 / 6; `5D 04 02 00 0000`
      (`0x005458E0`); refresh text. If R has 4.13: status := 13; state
      := 6 unless it is 6; if the game record lacks 4.13, set it and run
      the sequence function. Then, if +0x63 = 1, set game 4.13.
   4. cain5, 125: add the GUID to the record list, remove it from the
      +0xB4 list, refresh text. cain5, 123 or 126: add the GUID to the
      +0x138 list, refresh text.
10. **Event 13** `0x00597030`:
    1. R has 4.0: +0x52 := 1; game 4.13; +0x54 := 3; +0x4C, +0x4D := 1.
    2. Else R has 4.15: +0x52 := 1; +0x63 := 1; +0x54 := 3; +0x4C,
       +0x4D := 1.
    3. Else R has 4.4: status 4, state 5; +0x4C, +0x4D, +0x4E, +0x78 :=
       1; +0x58 := 1. Else 4.3 → state 3, status 1; else 4.2 → state 2,
       status 1.
    4. Always: +0x7C += (has `bkd `) + (has `bks `). Has `bkd `: state 5,
       status 3, +0x4E, +0x78 := 1, +0x58 := 1. Else has `bks `: +0x78 :=
       1, state 4, status 2, +0x58 := 1.
11. **Event 14** `0x00592B90`: +0x7C += (has `bkd `) + (has `bks `).
12. **Active** `0x00592FB0`: akara: R has 4.1, or R lacks 4.0 and (state
    1, or state 4 with `bks `, or state 6 with R 4.13). cain5: the player
    is in the +0xB4 list, or not in the +0x138 list and R has 4.13.
    Others false.
13. **Tree reset** `0x00592BD0`: if +0x47 = 1: the object with GUID +0x30
    of class 30 gets mode 0; broadcast(1, 0); state := 3; +0x58 := 0;
    every player L2. If +0x47 ≠ 1: broadcast(1, 0); state := 3; +0x58 :=
    0.
14. **Cain cleanup** `0x00596CA0(record, timer, remove)`:
    1. If +0x48 = 1 and the object with GUID +0x34 is class 26: mode :=
       3 unless it is 3 (then +0x54 is left). Otherwise and after
       setting it: +0x54 := 3.
    2. If +0x47 = 1 and the object with GUID +0x30 exists: mode := 1.
    3. timer ≠ 0: if +0x46 = 0, +0x46 := 1 and timer (record, `0x00593260`,
       period 1). timer = 0 and remove ≠ 0: every monster `0x005928C0`.
    4. remove ≠ 0: if +0x61 = 0, drop act 0's stored preset of monster
       146 (`0x00543140(game, 1, 146, 0)`); +0x50 := 1; if +0x51 = 0,
       +0x52 := 1. remove = 0: +0x50 := 1.
    Timer `0x00593260`: every monster `0x005928C0`; +0x46 := 0; return 1.
    `0x005928C0` (stops the walk at the first class-146 monster): if
    an NPC chat is open with it (`0x00572DC0`), send `5D 04 01 00 0000`
    to its chat clients (`0x00573180` with `0x00592880`); else request
    its removal mode (`0x005A7E60(monster, 0, buf)`, `0x005A7C20(game,
    buf, 1)`, `monsters/init.md`) and +0x61 := 1. Returns 1 for class
    146, else 0.
15. **Town Cain spawn** `0x00592960(game, x, y)` in room R0: find a
    point: for i = 0 through 20 (21 points) test (x + i, y + i) inside R0's tile rectangle
    (`0x00619730`, excluding the last row and column); found → that
    point; not found → (y, y + 21) (bug kept). Free spot
    (`0x00545340`, args 2, 0x100, 1, 100; the third, a radius, is never read by `0x00545340`); none → (x, y, R0). Spawn
    cain5 (`0x005B2F20(game, room, x, y, 265, mode 1, r 5, 0)`). On
    failure up to 20 retries, each moving the point by (+1, +1), taking
    its room (`0x00463740`; none → (x, y, R0)), a free spot (args 2,
    0x100, 2 (unused), 100) and r 10; then one last try at (x, y, R0) with r 15.
    Spawned: unit +0xC4 |= 0x3000000; +0x51 := 1; +0x52 := 0; +0x68 :=
    its GUID.
16. **Act change** `0x00597310(game, player)` (`quests.md` §8.1): if R lacks 4.0
    and 4.1, +0x50 = 0 and state < 6: Cain cleanup (0, 1); state := 7;
    broadcast(5, 0); game record 4.13; every player L3.
17. **add_link** `0x00592F80` (`quests.md` §4.6): +0x49 := 1; +0x28 := the object's
    GUID (−1 when none).
18. **Sequence** `0x00593D70`: §10.1.

- Cairn stone order (`0x00592E90`, run once: +0x4A guards it in
  `0x00593CB0`; also run when a stone is operated, `0x00593710`):
  order[0..4] = 0; i = 0; while i < 5: step the quest seed, k = lo' mod
  5; if order[k] = 0: order[k] = 17 + i, i += 1.
- Stone order message `0x00593CB0(game, player)` (`quests.md` §9.4): compute the
  order if +0x4A = 0 (+0x4A := 1); send 0x50 (15 bytes, `0x0053D7E0`):
  u8 0x50, u16 4, then order[k] − 17 as u16 for k = 0..4 (each must be
  < 5, else fatal); bytes 13–14 are never written (stack bytes; `quests-act1-rest.md` §7, Open
  question 5).
- Wirt's body (object class 268, event 7): the first time, piles =
  roll_range(quest seed, 10, 10) (one step, lo' mod 10 + 10) and the
  object's drop code becomes `gld `; each run with piles > 0 drops one
  gold item (`0x00559A30`, normal quality); if a drop succeeded, piles −=
  1 and, if still > 0, schedule event 7 at frame + 10.
- **Tree operate** `0x00593AF0` (operate pointer `0x00732D48`; args
  game, object, player; returns 0): chain 4's record (fatal if absent).
  not-intro = 0 → object mode 1, end. End if state ≥ 6, the object's
  mode ≠ 0, or R has 4.0 or 4.1. The player has `bkd ` or `bks ` → sound
  event 19, end. Else: sound event 45 on the player; object drop code :=
  `bks `; state := 4; drop at the object (`0x00559A30(game, object, 2,
  &out, 0, −1, 0)`). Dropped: callback 9 := `0x00592C80`; broadcast(2,
  0); +0x4B := 1; +0x38 := the scroll's GUID; +0x7C += 1; +0x58 := 1;
  +0x78 := 1; object mode := 1. Always then: +0x47 := 1; +0x30 := the
  object's GUID.
- **Stone operate** `0x00593710` (operate pointer `0x00732D3C`; args
  game, object, player, …, stone value u16 at args +0x10; returns 0):
  1. Chain 4's record (fatal if absent); compute the order if +0x4A = 0.
  2. R has 4.0 or 4.1: sound event 19. End.
  3. No `bkd `: if the touch counter u16 +0x2C mod 64 = 0 and R lacks
     4.3 and 4.4, sound event 39; +0x2C += 1. End.
  4. End if not-intro = 0 or state ≥ 6. P := the linked class-61 object
     (+0x28, when +0x49 = 1). If +0x4E = 0 and P exists: state 0 with
     not-intro 1 → state := 1. End if +0x4F = 1.
  5. state ≠ 5: state 0 with not-intro 1 → state := 1; state := 5.
  6. k := +0x0C. End unless the stone value = order[k]. Store the
     object's GUID at +0x10 + 4k; +0x0C += 1. End if the object's mode ≠
     0.
  7. n = +0x0C ≤ 4: object mode := 1; P mode := n + 1. End.
  8. n = 5: object mode := 1; P mode := 6; +0x4F := 1; delete the
     player's `bkd `; +0x7C −= 1. Position: the stored stone whose order
     value is 21 if it is an object of class 21, else the first class-21
     object in the rooms of the operated object's room list. Found: a
     missile of class 288 (cairnstones, owner the player, skill 0,
     level 1; it opens the Tristram portal, `missiles/bodies.md`) at
     (x + 6, y − 3) (`0x0056EDE0`, `quests-act1-rest.md` §4.1), its room
     refreshed (`0x0061AED0`). If status < 4: broadcast(4, 0); with a
     party, members lacking 4.0 and 4.1 in an Act I level get 4.4
     (`0x005936B0`); set R 4.4. Finally `0x00545760(game, 1)` (0x28 and
     `89 01` to every player).
  Stone values come from the object (args +0x10: the object's class,
  `quests-act1-rest.md` §2.1), 17–21 as in the
  order.
- The gibbet operate `0x00593480` (pointer `0x00732D40`), the gibbet
  quest function `0x00593290` (`quests.md` §9.5 class 26), the stone init
  `0x005935E0` (pointer `0x00731BD8`), the Tristram-portal timer
  `0x00592D50` and the town-Cain marker init `0x005940E0` are in
  `quests-act1-rest.md` §1–§3.

#### 10.7 A1Q5 The Forgotten Tower (chain 5)

Init `0x00595920`: callbacks per `quests.tsv`; active 1, status 0, state
0, init_no 4, seq_id 3, filter 5, status fn none, active fn `0x005952C0`,
seq fn `0x00595240`; extra 0x120 zeroed bytes, list C emptied.

| Extra | Type | Field |
|---|---|---|
| +0x00 | 12 × u32, u16 count +0x30 | list A: players who reported the kill to a town NPC |
| +0x34 | 12 × u32, u16 count +0x64 | list B: players credited in Tower Cellar 5 (appended only while count < 12) |
| +0x68 | u32 list, u16 count +0x88 | chest object GUIDs for the trap step (filled outside the callbacks below) |
| +0x8C | `quests.md` §9.3 GUID list | list C: players in Tower Cellar 5 at the kill |
| +0x110, +0x114 | i32 × 2 | the Countess's death position |
| +0x118, +0x119 | u8 | killed; trap spawned |
| +0x11A | u8 | the next town report finishes the quest |
| +0x11B | u8 | the tome was read before any status |
| +0x11C | u8 | cleared by the timer |

Removing from A or B swaps the last entry into the hole (`0x00594740`
with 0 = A, else B, returns whether found).

| Id | Function | Effect on one player P (slot 5) |
|---|---|---|
| M1 | `0x00594830` | broadcast iterate: 0x5D for chain 5 unless P has 5.0 and neither 5.13 nor 5.14 (no 5.15 test, unlike I1) |
| M2 | `0x00594890` | if P has neither 5.0 nor 5.1: state 2 → set 5.2; state 3 → set 5.3, 5.4, 5.5, 5.6 for status 1, 2, 3, 4 (jump table `0x0059494C`); else nothing |
| M3 | `0x00594DD0` | if P has neither 5.0 nor 5.1 and P's room's level is 25 (Tower Cellar 5): add P's GUID to list C; set 5.0, 5.13 |
| M4 | `0x00594F10` | if P lacks 5.0 and has a room: level 25 → set 5.13, 5.0, clear 5.1, sound event 37 on P, append P's GUID to list B; other levels (any act) → set 5.14 |
| M5 | `0x005953D0` | if P has 5.13 and a party: each member lacking 5.0 whose room's level is ≠ 0 and in Act I gets 5.13, 5.0 (`0x00595370`) |
| M6 | `0x00595320` | if P lacks 5.0: set 5.14; `5D 05 00 0C 0000` (act 0) |

1. **Event 0** `0x00594C50`: end if not-intro = 0, or R has 5.0 but not
   5.13. If state ≥ 4, end unless the player is in list B or list A.
   Then: in list B → add state 2. Else, R has 5.0 and 5.13 → add state 3
   if in list A, else nothing. Else m = `0x007382AC`[state] (−1, −1, 0,
   1, 2, 3); m ≠ −1 → add state m.
2. **Event 3** `0x00595010` (old a, new b):
   1. not-intro ≠ 0 and b = 20 (Forgotten Tower): state 0 → state := 2,
      broadcast(3, 0), every player M2. Else state ≤ 3 and status = 1 →
      broadcast(4, 0), every player M2. End.
   2. not-intro ≠ 0 and b = 25: if state < 4 and status ≠ 2: state := 3
      (unless 3), broadcast(2, 0), every player M2. End.
   3. Otherwise, a = 1: state 2 and R lacks 5.0 → state := 3 (nothing
      sent). State 5 and not-intro ≠ 0 → remove the player from list A;
      if it was there and lists A and B are both empty, active := 0.
3. **Event 8** `0x00595710` (the Countess's death; victim = target):
   1. Store the victim's position at +0x110 (`0x00620870`).
   2. If not-intro ≠ 0: callback 2 := null; state := 5; +0x11A := 1;
      every player M4; game record 5.13; list B empty → active := 0,
      else callback 10 := `0x00594BB0`. +0x118 := 1; callback 8 :=
      null; game record 5.13; every player M3; `0x00595420(game, list
      C, 5, 37)` (each listed player lacking 5.0 gets 5.13, 5.0 and
      sound event 37); every player M5; every player M6; timer (record,
      `0x005954C0`, period 7).
   3. Trap step `0x005954F0` (both cases); if no trap was spawned
      (+0x119 = 0), event 7 on the victim at frame + 10 (`0x005417D0`).
4. Timer `0x005954C0`: if state = 5, broadcast(13, 0); +0x11C := 0;
   return 1.
5. **Event 10** `0x00594BB0`: remove the player's GUID from list C,
   list B and list A.
6. **Event 11** `0x00594960` (class c, message m):
   1. m = 127 (the tome) and not-intro ≠ 0: changed := 0. If +0x11B =
      1: status < 1 → broadcast(1, 0), changed; status = 3 →
      broadcast(2, 0), changed, and state := 3 if state < 3. Then state
      < 2 → state := 2 and every player M2; else if changed, every
      player M2.
   2. c ∈ {154, 150, 265, 155, 148, 147} and m ∈ 140–145: if R has
      5.13 and +0x11A ≠ 0: +0x11A := 0, state := 5, run the sequence
      function (§10.1). Then, if the player is in list B: remove it and,
      if list A has fewer than 12, append it to list A.
7. **Event 13** `0x00595860`: unless R has 5.0 or 5.15: 5.4 → state 3,
   status 1; else 5.6 → state 3, status 4; else 5.5 → state 2, status 3;
   else 5.3 → state 3, status 1; else 5.2 → state 2, status 1.
8. **Active** `0x005952C0`: the player is in list B and the NPC class is
   neither 155 nor 147.
9. **Tome operate** `0x00594E70` (object operate pointer `0x00732D30`;
   args game, object, player, …; returns 0): if there is no object or its
   mode is 0: mode := 1 and an object event 1 is scheduled at frame +
   (anim length >> 8) (`0x00640E90`, `0x005417D0`). Then, if chain 5's
   record exists with not-intro ≠ 0: `0x005456A0(player, object, 127)`
   (opens message 127); if state ≤ 1: state := 2 and, if status < 1,
   +0x11B := 1.
10. **Chest** (object class 0x173): init `0x00595A50` (object init
    pointer `0x00731C7C`; args game, object): chain 5's record must
    exist (else nothing); add the object's GUID (−1 when none) to the
    +0x68 list unless present or the list holds 8; trap step; if killed
    and no trap spawned yet, object event 7 at frame + 10. Its event 7
    (`quests.md` §9.5) runs `0x005956C0`: trap step, then the same reschedule.
    Object init `0x00595A00` (pointer `0x00731BD0`, object class not
    traced): chain 5's record must exist (else fatal); not-intro = 0 →
    object mode 3.
11. **Sequence** `0x00595240`: §10.1.

No quest-seed draws. The trap step `0x005954F0` (one trap-firebolt
monster 326, then a towerchestspawner missile 332 per chest) is
`quests-act1-rest.md` §4.
`0x00595160` and `0x00594DB0` have no direct caller in the exports.

#### 10.8 A1Q6 Sisters to the Slaughter (chain 6)

Init `0x00596990`: callbacks per `quests.tsv` (event 10 is added at the
kill); active 1, status 0, state 0, init_no 4, seq_id 37 (unused),
filter 6, status fn none, active fn `0x005967F0`, seq fn `0x005968E0`;
extra 0x198 bytes: three `quests.md` §9.3 GUID lists at +0x00 (Cain), +0x84 (Akara),
+0x108 (Kashya), emptied; +0x18C u32 victim GUID, +0x190 u16, +0x192 u16
timer counter, +0x194 u8 killed, +0x195 u8 talked, all 0.

| Id | Function | Effect on one player P (slot 6) |
|---|---|---|
| O1 | `0x00595B20` | broadcast iterate: as I1 for slot 6, 0x5D for chain 6 |
| O2 | `0x00595BD0` | as I2 for slot 6 and chain 6's state / status |
| O3 | `0x00596260` | if P has neither 6.0 nor 6.15 and P's room's level is 37 (Catacombs 4): add P's GUID to the three lists; credit P (below) |
| O4 | `0x00596440` | if P has 6.13 and a party: each member with neither 6.0 nor 6.1 whose room's level is ≠ 0 and in Act I is added to the three lists and credited (`0x00596320`) |
| O5 | `0x005963E0` | if P has neither 6.0 nor 6.1: set 6.14; `5D 06 00 0C 0000` (act 0) |
| O6 | `0x00596170` | if P has 6.1 and 6.13: sound event 33 on P |
| O7 | `0x00596490` (arg = the victim) | if P's room's level is 37: a portal object of class 59 to level 1 at P's position, owner P (`0x0056D130(game, P, room, x, y, 1, 0, 59, 0)`); returns 1 (stops the walk) |

Credit `0x00596210(game, P)`: set 6.13, 6.1 in P's record; then
`0x00538680(P's client, 1, difficulty)` (character progression,
`quests-act1-rest.md` §5).

1. **Event 0** `0x00595E20` (NPC class c):
   1. c = 265 and the player is in the Cain list, c = 148 and in the
      Akara list, or c = 150 and in the Kashya list: add state 3 (with
      that NPC). End.
   2. R has 6.1: add state 4 if c is 265, 148 or 150, else state 3. End.
   3. Player in the record's GUID list: add state 4. End.
   4. state = 1, c = 265 and R lacks 6.0: add state 0. End.
   5. End if state = 0, R has 6.0, or state ≥ 4 and R lacks 6.13. m =
      `0x007382C4`[state] (−1, 0, 1, 2, 3, 4); add state m.
2. **Event 2** `0x00595B80`: target class 265 and talked = 1:
   broadcast(1, 0); talked := 0; callback 2 := null.
3. **Event 3** `0x00596010` (old a, new b):
   1. b in 34–37 (Catacombs 1–4) and not-intro ≠ 0: if state < 3,
      state := 3 and changed := 1; else the state is kept and changed :=
      0 (`quests-act1-rest.md` §8 item 6). b = 37: status < 2 → broadcast(2, 0) and every
      player O2; else if changed, every player O2. b ≠ 37: status 0 →
      flags := 0, status(1) (nothing sent), every player O2; else if
      changed, every player O2. End.
   2. Else state = 4 and b = 40: state := 5. End.
   3. Else a = 1: remove the player's GUID from a non-empty record list
      (`0x00545310`); if state = 2 and R lacks 6.0 and 6.1: state := 3,
      every player O2.
4. **Event 8** `0x005965A0` (Andariel's death; victim = target):
   1. callback 2 := null.
   2. not-intro ≠ 0: if there is a killer player lacking 6.0 and 6.1:
      credit it; then twice {step the quest seed; victim drop code
      (+0xB8) := chipped[lo' mod 7]; drop (`0x00559A30(game, victim, 2,
      &out, 0, −1, 0)`)}, then once with normal[lo' mod 7]; drop code
      := 0. With or without a killer: every player O3, then O4, then O5.
   3. +0x18C := the victim's GUID (−1 when none); +0x192 := 1.
   4. not-intro ≠ 0: timer (record, `0x00596500`, period 1); every
      player O6.
   5. killed := 1; state := 4; callback 10 := `0x005961C0`; callback 8 :=
      null.
   chipped = `gcv gcr gcb gcy gcg gcw skc` (`0x007361DC`), normal = `gsv
   gsr gsb gsy gsg gsw sku` (`0x00736444`).
5. Timer `0x00596500` (every 2 updater ticks): +0x192 += 1. At 10: every
   player O7 with the monster of GUID +0x18C as argument; return 0. At
   12: unless status is 3 or 13, broadcast(3, 0); return 1. Otherwise
   return 0.
6. **Event 10** `0x005961C0` (after the kill): `0x00545530` (§10.6 step
   8); remove the player's GUID from the three lists.
7. **Event 11** `0x00595C60` (class c, message m):
   1. Cain 166: state := 2 (no guard); talked := 1; every player O2;
      refresh text. Cain 184: remove the player from the Cain list.
   2. Warriv 183: refresh text first. Then, if R has 6.1: if R has 6.13,
      flags := 0, status(13), state := 5, game record 6.13. Clear 6.1,
      set 6.0, add the player's GUID to the record's list, 0x28.
   3. Akara 179: remove from the Akara list. Kashya 181: remove from the
      Kashya list.
8. **Event 13** `0x00596900`: §10.1 restore, slot 6.
9. **Active** `0x005967F0`: Cain: in the Cain list, or R lacks 6.0 and
   6.1 and state = 1. Warriv: R lacks 6.0 and has 6.1. Akara / Kashya:
   in their list. Others false.
10. **Sequence** `0x005968E0` (§10.1): its timer `0x00596580` (period
    20) sets state := 1 if it is 0 and returns 1.
11. `0x005968C0` (no direct caller found): returns killed (+0x194) when
    not-intro ≠ 0, else 1.

The Warriv travel hook (`quests.md` §8.1, `0x005467E0`) calls this record's event 3
with args (game, 3, target and the fifth dword unset, player, a = 1, b =
40): step 3.2, so a state-4 record goes to 5. `0x00597310` runs after
it whether or not chain 6's record exists.
