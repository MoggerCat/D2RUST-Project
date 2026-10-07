# Spec: World — Quest system (flags, records, events, updater, Act I)

- **Status:** draft: the shared machinery (flag records, quest records,
  creation, game-entry sequencing, event dispatch, updater, status
  messages, act transitions) is read from the 1.14d `Game.exe` (addresses
  below) and its message bytes match the two hand-played packet
  recordings (`traces/raw/20261006-015956-packets.jsonl`,
  `20261006-022633-packets.jsonl`: 0x5E, 0x28, 0x29, 0x5D, 0x8A, C→S 0x31;
  Test vectors). Act I quests are specified at the level of their
  triggers, flags, rewards, timers and draws; A1Q1 (`quests-act1.md` §10.4) and the
  sequence functions (`quests-act1.md` §10.1) are specified callback by callback from
  the 1.14d disassembly; Acts II–V are catalogued only (`quests.tsv`
  column `spec`).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::quests` (flag record, quest control,
  dispatch, updater); `d2-sim::world::quests::act1` (Act I quests)
- **Related specs:** `sim/tick.md` §3 step 8 (when the updater runs), §5
  (timer events; quest objects use event 7); `sim/intents-events.md` +
  `client-messages.tsv` / `server-messages.tsv` (ids, sizes); `sim/rng.md`
  §5.2 (quest seed derivation), §3 (helpers); `sim/unit-order.md` (player
  iteration order); `world/npc.md` (NPC chat, menus, the 0x27 message,
  imbue, respec, hire; it calls the hooks of §7 and §8.1);
  `world/vendors.md` (npc.txt quest multipliers read §1 flags);
  `world/waypoints.md`; `world/cube.md` (cube outputs that call §8.4);
  item, monster, object and save specs (Phase 3, not written: they own
  item creation, monster spawning, object modes and the save header).
  Machine tables: `world/quests.tsv` (§2.4), `world/quest-messages.tsv`
  (§7.1). `world/quests-act1-rest.md` (A1Q4 gibbet, Cairn stone init,
  A1Q5 chest trap, act progression, party reads).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 56–74 |
| Inputs | 75–86 |
| Outputs / state changes | 87–93 |
| Rules | 94–95 |
|   1. Quest flag records | 96–237 |
|   2. Quest control and quest records | 238–335 |
|   3. Game entry: picking the quest set | 336–368 |
|   4. Events and dispatch | 369–453 |
|   5. Quest updater and timers (tick step 8) | 454–475 |
|   6. Status reporting | 476–579 |
|   7. NPC dialog hooks | 580–612 |
|   8. Act transitions, warps and portals | 613–694 |
|   9. Quest items, rewards and helpers | 695–858 |
|   11. Acts II–V | 859–864 |
| Constants & data dependencies | 865–879 |
| Randomness | 880–906 |
| Edge cases & original bugs | 907–925 |
| Test vectors | 926–957 |
| Provenance | 958–986 |
| Open questions | 987–1034 |
<!-- /index -->

## Summary

Quests are a fixed set of 41 hard-coded quest records per game (37 quests
plus 4 act intros; there is no quest `.txt`). Progress lives in quest
flag records: 42 quest slots × 16 bits = 96 bytes, one record per
difficulty per player (saved) plus one game-wide record (not saved). Game
code reports events (NPC talk, level change, kill, item pick-up, ...) to
the quest records, whose per-quest callbacks change the record's state,
set flags, grant rewards and send status messages. A timer list run every
20 frames (tick step 8) drives delayed quest work. This spec owns the
flag record, the quest records, event dispatch, the updater, the status
messages, act transitions, the quest-side NPC dialog hooks and the Act I
quest state machines.

Split: §10 (Act I quests) moved unchanged to `world/quests-act1.md`
(same section numbers and rule ids; the two files share no id, so a bare
§ reference is unambiguous). Constants, Randomness, Edge cases, Test
vectors, Provenance and Open questions for both files stay here.

## Inputs

| Name | Type | Source |
|---|---|---|
| player quest records | 3 × 96 bytes per player | character save quest section (§1.6) |
| game events | calls from units, NPC, item and level code | §4.4 |
| C→S 0x31 QuestMessage, 0x40 RequestQuestData, 0x58 QuestCompleted, 0x3E ActivateInifussScroll | intents | `client-messages.tsv` |
| `levels.txt` `Quest` (record +0x30) | u8 chain id | §4.6 |
| item records `quest` (+0x12A) | u8 chain id + 1 | §9 |
| game seed | `sim/rng.md` §5.2 | §2.3 |
| frame counter | `sim/tick.md` §2 | §5 |

## Outputs / state changes

Player and game quest flag bits; quest record state bytes; S→C messages
0x27 (text list, via `world/npc.md`), 0x28, 0x29, 0x50, 0x52, 0x5D, 0x5E,
0x61, 0x89, 0x8A, 0x91; quest items created or deleted, gold and gems
dropped, portals created, stats added (requests to the owning specs).

## Rules

### 1. Quest flag records

#### 1.1 Layout

A quest flag record is a 96-byte buffer (`0x0065C430` allocates and
zeroes 0x60 bytes) used as a bit array. Quest slot q (0..41) owns bits
16·q .. 16·q+15, i.e. bytes 2q and 2q+1. Bit n is byte n>>3, mask
1<<(n&7) (LSB first; `0x00410B10` set, `0x00410B30` test, `0x00410B50`
clear, mask table `0x006CE268` = 1,2,4,…). Read as little-endian u16,
slot q's word has bit b at 1<<b.

| Function | 1.14d | Effect |
|---|---|---|
| get(rec, q, b) | `0x0065C310` | test bit 16q+b (asserts rec) |
| set(rec, q, b) | `0x0065C360` | set |
| clear(rec, q, b) | `0x0065C3A0` | clear |
| reset_progress(rec, q) | `0x0065C3E0` | clear bits 2..11 of slot q |
| copy_in(rec, buf, 0x60, normalize) | `0x0065C4D0` | §1.6 |
| copy_out(rec, buf, 0x60) | `0x0065C560` | copy 96 bytes |

Size ≠ 0x60 in copy_in/copy_out is a fatal assert.

#### 1.2 Bits of a slot

| Bit | D2MOO name | Meaning (as used by the Act I code, `quests-act1.md` §10) |
|---|---|---|
| 0 | REWARDGRANTED | quest done and rewarded for this player |
| 1 | REWARDPENDING | goal done, reward not yet collected |
| 2 | STARTED | NPC gave the quest |
| 3 | LEAVETOWN | left town while started |
| 4 | ENTERAREA | entered the quest area |
| 5–11 | CUSTOM1–7 | per quest (e.g. slot 4 bit 10: Cow King killed by this player; bit 11 in the game record: cow portal opened) |
| 12 | UPDATEQUESTLOG | set by C→S 0x58 (§1.7) |
| 13 | PRIMARYGOALDONE | goal done (also set in the game record) |
| 14 | COMPLETEDNOW | goal completed during this game |
| 15 | COMPLETEDBEFORE | completed in an earlier game (§1.6, §3) |

#### 1.3 Slots

Slots 0..40 follow D2MOO's `QUESTSTATEFLAG_*` order; slot 41 is new in
1.14d. `quests.tsv` gives each record's slot (`flag`, `filter`).

| Slots | Content |
|---|---|
| 0–6 | Act I: Warriv gossip, Den of Evil, Sisters' Burial Grounds, Tools of the Trade, Search for Cain, Forgotten Tower, Sisters to the Slaughter |
| 7 | Act I completed (may travel to Act II, §8.1) |
| 8–14 | Act II: Jerhyn gossip, Radament, Horadric Staff, Tainted Sun, Arcane Sanctuary, Summoner, Seven Tombs |
| 15 | Act II completed |
| 16–22 | Act III: Hratli gossip, Lam Esen, Khalim's Will, Blade of the Old Religion, Golden Bird, Blackened Temple, Guardian |
| 23 | Act III completed |
| 24–27 | Act IV: Tyrael gossip, Fallen Angel, Terror's End, Hell's Forge |
| 28 | Act IV completed (expansion, §8.1) |
| 29 | Flavie (A1Q7) |
| 30, 31 | Act II guard gossip (A2Q7, A2Q8) |
| 32 | A3Q7 table slot (its record reports in slot 16, edge case 4) |
| 33 | A4Q4 (Malachai) |
| 34 | unused (no record, no code reference found) |
| 35–40 | Act V: Siege on Harrogath, Rescue on Mount Arreat, Prison of Ice, Betrayal of Harrogath, Rite of Passage, Eve of Destruction |
| 41 | Akara respec reward (`quests-act1.md` §10.3) |

Intro records use slot number 42 (`filter` = 0x2A): beyond the 42 slots
but still inside the 96-byte buffer (bytes 84–85, never saved with
meaning). No 1.14d code reads or writes slot 42 through them (their
status functions return false, §6.1).

#### 1.4 Where records live

- Player: player data (`0x006221A0(player)`) +0x10 + 4·d holds the record
  of difficulty d (0..2); +0x60 + 4·d holds the NPC intro record (§6.7).
  Code always uses d = the game difficulty (game +0x6D, u8). Helper
  `0x00543520(game, player)` returns the current one.
- Game: quest control +0x0C (§2.1); "global" in D2MOO; never saved.
  Get/set through `0x00544760` / `0x00544720` (assert the quest set is
  picked, §3).

#### 1.5 Messages carrying records

| Msg | Size | Layout | Sender |
|---|---|---|---|
| S→C 0x28 QuestInfo | 103 | u8 0x28, u8 unit type, u32 unit GUID, u8 0, 96 bytes player record (current difficulty) | `0x0053D670` |
| S→C 0x29 GameQuestInfo | 97 | u8 0x29, 96 bytes game record | `0x00544520` → `0x0053D700` |

0x28's unit is the NPC being talked to (type 1, its GUID) when sent by the
NPC chat path, and type 6 / GUID 0 when sent by quest code (`0x005455B0`,
§6). Recorded: `28 06 00000000 00 …` at game start; `28 01 06000000 00 …`
when talking to the NPC with GUID 6.

#### 1.6 Loading from a save

The save's quest section holds three 96-byte records (normal, nightmare,
hell) after a 10-byte header starting with `Woo!` and u32 6 (checked in
`0x00532BE0` and `0x0056A370`; the save spec owns the header). Each is
copied with normalize = 1 (`0x0065C4D0`): for every slot q = 0..41, clear
bit 13 and bit 14; then if bit 1 is set, set bit 15. (The loop covers 42
slots; D2MOO 1.10f covers 41.) Bits 0, 1 and 2–12 are kept as saved.
`0x004B6DD0` (client) also copies with normalize = 0.

#### 1.7 C→S 0x58 QuestCompleted

Handler `0x0054C9C0`: size must be 3 (else result 3); quest = u16 @1; if
quest < 0x2A, set bit 12 of slot quest in the player's current record
(result 0), else result 2. Nothing is sent.

#### 1.8 Completed quests and the save

What a "completed" quest is in 1.14d and what the save carries of it
(the save layout is `formats/d2s.md` §4, its load effects
`formats/d2s-load.md`).

1. **The completion test.** The only code that carries "this quest is
   done" from one game to the next is the quest-set pick (§3 step 2a):
   a row's flag slot counts as completed when bit 0 is set
   (`0x005462FD`) or, failing that, bit 15 (`0x0054630F`). Gates that
   need a particular quest test bit 0 of its slot alone: act travel
   (§8.1: slots 7, 15, 23, 28, and 10 / 18 inside the transitions), the
   cow portal (§8.4: slot 26 classic, 40 expansion, and slot 4 bit 10
   against it), and the per-quest code of each act file.
2. **Bit 1 without bit 0** (goal done, reward not collected) is saved as
   it is; the load sets bit 15 beside it (§1.6), so the next game's pick
   treats the quest as completed while the quest code that tests bit 1
   still offers the reward.
3. **What a played completion leaves.** The reward step sets bit 0 and
   clears bit 1 (`quests-act1.md` §10.1 state 5 for the Act I shape).
   Bits 13 and 14 are set during that game; the writer copies the record
   out unchanged (`formats/d2s.md` §4 rule 1) and the next load clears
   them (§1.6). Bit 12 is set only if the client sent C→S 0x58 (§1.7).
   Bits 2–11 keep whatever the player's path set, unless the quest's own
   code clears them (explicit clears, or `reset_progress` `0x0065C3E0`,
   12 call sites, all in Act II–V code). So a played completion has no
   single bit pattern per quest; the act files own which bits each path
   sets.
4. **The smallest record every 1.14d completion reader accepts** is
   bit 0 of the slot (rules 1–2). Two slots are not quests: slot 41 bit 0
   means the Akara respec was used and bit 1 that it is available
   (`quests-act1.md` §10.3, `world/npc.md` §8.2), and slot 4 bit 10 means
   the player killed the Cow King (no more cow portals, §8.4). Slot 34
   is unused (§1.3) and slot 42 lies beyond the saved slots' meaning.
5. Nothing else in the save marks completion: the difficulty unlock and
   the character title use the progression bits of the client save flags
   (`quests-act1-rest.md` §5, header progression in `formats/d2s.md`),
   which only the act-end quest code raises.

### 2. Quest control and quest records

#### 2.1 Quest control (game +0x10F4)

Allocated by `0x00545D80` (0x24 bytes):

| Offset | Field |
|---|---|
| +0x00 | newest quest record (list head, §2.3) |
| +0x04 | executing flag (1 while the updater runs) |
| +0x08 | quest set picked (§3) |
| +0x0C | game quest flag record (§1.4) |
| +0x10 | timer list head (§5) |
| +0x14 | updater tick counter (u32) |
| +0x18 | quest seed (8 bytes, `sim/rng.md` §1) |
| +0x20 | FX byte for 0x89 (§6.5) |

#### 2.2 Quest record (0xF8 bytes)

| Offset | Field (D2MOO name) |
|---|---|
| +0x00 | chain id (`nQuestNo`): lookup key |
| +0x04 | game |
| +0x08 | act (u8) |
| +0x09 | not-intro (`bNotIntro`; 0 = quest treated as done/skipped for this game) |
| +0x0A | active (`bActive`) |
| +0x0B | status shown to clients (`fLastState`, §6.1) |
| +0x0C | state (`fState`) |
| +0x0D | state at which the default status rule switches (`nInitNo`, §6.1) |
| +0x10 | sequence target chain (`nSeqId`) |
| +0x14 | flags byte sent in 0x5D (`dwFlags`; low byte used) |
| +0x18 | per-quest extra data (allocated by the init function) |
| +0x1C | 32 player GUIDs, u16 count at +0x9C (§9.3) |
| +0xA0 | 15 event callbacks (§4.1) |
| +0xDC | NPC message table (§7.1) |
| +0xE0 | flag slot reported in (`nQuestFilter`) |
| +0xE4 | second flag slot (new in 1.14d; only A1Q1 sets it, to 41) |
| +0xE8 | status function (null = default rule) |
| +0xEC | NPC-wants-to-talk function |
| +0xF0 | sequence function |
| +0xF4 | previous (older) record |

D2MOO 1.10f has no +0xE4 field; every later field is 4 bytes lower there.

#### 2.3 Creation (`0x00545D80`, at game creation)

1. For each of the 37 rows of the init table `0x00731510` (24 bytes per
   row: init function, act u8, version u32, no-set-state u8, chain id,
   flag slot; count `0x00731888` = 37), in row order: allocate a zeroed
   0xF8 record; chain id = row chain; game; act = row act; active = 0;
   status = 0; not-intro = 1; GUID count = 0; previous = the record made
   before it; call the row's init function with the record (it sets the
   callbacks, filters, state and extra data; `quests.tsv`).
2. For each of the 4 intro rows of `0x0073188C` (8 bytes: init function,
   act; count `0x007318AC` = 4): chain id = 37 + i; active = 1; not-intro
   = 0; status 0; act; previous; init function.
3. Quest control: newest = the last intro record; executing = 0; picked =
   0; timers = none; tick = 0; quest seed = `init()` then one game-seed
   step and `init_low(lo')` (inline at `0x00545F21`; `sim/rng.md` §5.2,
   the fifth seed derived at game creation); game record = a new zeroed
   96-byte record.
4. Game +0x10F4 = the control.

The record list therefore runs newest → oldest: chain 40, 39, 38, 37 (the
intros), then rows 36, 35, …, 0. Every "for each quest record" below
uses this order. Lookup by chain id (`0x00543640`) walks it and returns
the first match (chain ids are unique), asserting id ≥ 0.

#### 2.4 `quests.tsv`

One row per record, in init-table order (`index` 0–36, intros 37–40),
generated from the 1.14d tables above and the init functions:

| Column | Meaning |
|---|---|
| index | row in the init table (intros: 37 + i) |
| chain | chain id (record +0x00) |
| flag | table flag slot (row +0x14; used by §3 and by 0x5E's order) |
| act | act 0–4 |
| version | row +0x08 (100, or 0 for Act IV quests 22–24 and all Act V quests) |
| no_set_state | row +0x0C (1 = skipped by §3 step 2) |
| init | init function |
| filter | slot the record reports in (+0xE0) |
| flag2 | +0xE4 when set |
| init_no, seq_id | +0x0D, +0x10 when set by the init function |
| callbacks | `event:function` for each non-null callback |
| status_fn, active_fn, seq_fn | +0xE8, +0xEC, +0xF0 (0 = null) |
| msgs | NPC message table (+0xDC) |
| name | quest name (D2MOO naming) |
| spec | `specified` (the state machine is written in the act's owner file: Act I `quests-act1.md` §10 and `quests-act1-rest.md`; Act II `quests-act2.md`; Act III `quests-act3.md` and `quests-act3-2.md`; Act IV `quests-act4.md`; Act V `quests-act5.md` and `quests-act5-2.md`) or `catalogued` (row only) |

Values are what the init function stores (`-` = not stored, so 0 from
the zeroed record). Row 40 (Act V intro, init `0x0058EA50`) was read
with `tools/ghidra/disasm.py` (the function is missing from the exports;
Open question 6), and its table `0x00732FF8` (4 states up to chain 31's
table at `0x00733308`, state 3 empty) from `Game.exe`. The `version` column
is never read by the 1.14d quest code found so far.

### 3. Game entry: picking the quest set

`0x00546270(game, player, mode)` runs when a player enters the game
(callers `0x00532590` and `0x00569F80` pass mode 1; `0x005344B0` and
`0x00534520` pass 0):

1. If the set is already picked (control +0x08 ≠ 0): for each record
   with callback 14 (`PLAYERJOINEDGAME`), call it (record, args with the
   player). Go to step 5.
2. Else set picked = 1. If mode = 0: (a) if the global byte `0x0073150C`
   (= 1 in 1.14d, nothing clears it) is set, for each init-table row in
   order with no-set-state = 0: if the player's record has the row's
   flag slot bit 0 or bit 15 set, the row's chain record gets not-intro
   = 0, active = 0, status = 0 (`0x00544410`) and the game record gets
   bit 15 of the row's flag slot; (b) for each record with callback 13
   (`PLAYERSTARTEDGAME`), call it.
3. (Both modes, first entry only) call the sequence functions (+0xF0) of
   chains 1, 8, 18, 22, 31, in this order (each lookup must succeed and
   the pointer must be valid, else fatal).
4. (Every entry, steps 5–8.)
5. Send 0x5E (38 bytes): u8 0x5E, then for each of the 37 init-table rows
   in order the not-intro byte of its chain record (`0x0053D830`).
6. Send 0x28 (unit type 6, GUID 0, current record).
7. Send 0x29 (game record).
8. If chain 1's record exists and `0x00590810` (not-intro ≠ 0 and state
   ≥ 4: Den of Evil cleared in this game) holds, send 0x89 `89 00`.

So the first player to enter decides which quests are "done" for the
whole game: a quest that player has already completed (bit 0 or 15) is
switched off (not intro, inactive) for everybody. Rows with
no-set-state = 1 (the gossip quests, A2Q2 Horadric Staff, A1Q7, A4Q4)
are never switched off.

### 4. Events and dispatch

#### 4.1 Event ids (callback index, record +0xA0 + 4·id)

| Id | D2MOO name | Raised by |
|---|---|---|
| 0 | NPCACTIVATE | NPC chat start (`0x00543D10`, from `0x00572C10`) |
| 1 | — | no raiser found; no 1.14d record sets it |
| 2 | NPCDEACTIVATE | NPC chat end (`0x00543D50`, from `0x00572F20`) |
| 3 | CHANGEDLEVEL | level change (`0x00543B90`, from `0x005380D0`, `0x00584870`) |
| 4 | ITEMPICKEDUP | `0x00543D80` (7 callers: `0x00531390`, `0x00531520`, `0x0055CF50`, `0x005600A0`, `0x00562F30`, `0x00563560`, `0x0057F700`) |
| 5 | ITEMDROPPED | `0x00543DB0` (from `0x00558AA0`) |
| 6 | — | `0x00543DE0` (no caller in the exports) |
| 7 | — | none |
| 8 | MONSTERKILLED | kill parse `0x00543A30` (from `0x0057CCB0`) |
| 9 | PLAYERDROPPEDWITHQUESTITEM | `0x00543BD0` (player leaving, from `0x00539DA0`) |
| 10 | PLAYERLEAVESGAME | same function, after event 9 |
| 11 | SCROLLMESSAGE | C→S 0x31 (`0x0054BA90` → `0x005443B0`) |
| 12 | — | none |
| 13 | PLAYERSTARTEDGAME | §3 step 2b |
| 14 | PLAYERJOINEDGAME | §3 step 1 |

The callback signature is (record, args). Args (7 dwords): game, event,
target unit, player, 0, then two dwords: for event 3 the old and new
level id; for event 11 the NPC class id (u16, 0 when no NPC) and the
message index (u16); for event 0 the text-list handle.

#### 4.2 Dispatch to all records (`0x005438E0(ignore_active, by_act)`)

Return if the game has no quest control. If by_act = 1 and the player
has a room, act = act of the room's level (`0x006427F0`), else act = −1.
For each record (list order, §2.3): call callback[event] if it is not
null and (ignore_active ≠ 0 or active = 1) and (act = −1 or act = record
act). Used by events 0, 2, 3, 10, 11 with: 0 (1, 1), 2 (1, 1), 3 (1, 0),
10 (1, 0), 11 (1, 1). All of them pass ignore_active = 1, so the active
byte gates none of these events.

#### 4.3 Dispatch along a unit's chain (`0x005439A0(force)`)

For each link of the target unit's quest chain (unit +0x74; link = {record,
next}; newest link first): call the record's callback[event] if not null
and (record active = 1 or force ≠ 0). Used by events 4, 5, 6 (force 0)
and 8 (force per §4.5). D2MOO 1.10f passes force = 1 for 4 and 5; 1.14d
passes 0, so pick-up and drop events reach only active records.

#### 4.4 Kill parse (`0x00543A30(game, victim, killer)`)

1. Return if the victim has no quest chain.
2. Game type byte +0x6A = 3 (D2MOO `nGameType`) and no killer: killer =
   the first client's player (`0x00539070`, `0x00537860`).
3. Player argument: the killer if it is a player; if it is a monster
   whose owner (`0x0058F090`) is a player, that player; else none.
4. force = 1 if the victim's superunique row (`superuniques.txt` hcIdx
   field) is one of 26 Ismail Vilehand, 27 Geleb Flamefinger, 29 Toorc
   Icefist, 36 Infector of Souls, 37 Lord De Seis, 38 Grand Vizier of
   Chaos, 39 The Cow King, 42 Siege Boss, 43–45 Ancient Barbarians 1–3,
   60 Nihlathak Boss; or its class id is 242 mephisto, 243 diablo,
   391 hellbovine, 544 baalcrab. Else 0.
5. Dispatch event 8 along the victim's chain (§4.3) with force.

#### 4.5 Player leaving (`0x00543BD0`)

For each item in the player's inventory (inventory order) whose item
record `quest` (+0x12A) is ≠ 0: record = lookup(chain = quest − 1); call
its callback 9 if not null (target = the item). Then dispatch event 10 to
all records (§4.2, (1, 0)).

#### 4.6 Unit quest chains

- `0x00545CD0(unit, room, debug)` (callers `0x00574250`, `0x005CC960`:
  monster creation): if the room's level row has `Quest` ≠ 0, add a link
  for chain `Quest` to the unit. In 1.14d only `levels.txt` rows 8 (Den
  of Evil, chain 1) and 74 (Arcane Sanctuary, chain 11) have a value.
- `0x005436B0(game, unit, chain)` adds a link: returns 0 if the chain
  has no record. Special cases before adding: chain 4 with an object of
  class 61 → `0x00592F80`; chain 8 with monster 229 (radament) →
  `0x005991B0`; chain 12 with monster 250 (summoner) → `0x0059C3B0`.
  The last two are `ret 4` stubs in 1.14d (they change nothing); the
  link is added after the special case in all three.
  `0x005435C0` then returns 0 if the unit's chain already holds the
  record (scan stops at a null record), else prepends a new link and
  returns 1.
- Quest code also attaches links directly (bosses, quest objects; per
  quest).

### 5. Quest updater and timers (tick step 8)

Runs from `sim/tick.md` §3 step 8 (`0x00543E10`, frame % 20 = 0):

1. Return if no quest control. tick += 1 (u32). If tick = 0xFFFFFFFF,
   every timer's due value becomes 0xFFFFFFFF − due.
2. executing = 1.
3. Walk the timer list from its head, keeping the previous kept timer:
   next = timer.next; if timer.due < tick (unsigned): call
   callback(game, record); if it returns 1, unlink and free the timer
   (the previous kept timer stays the same); else timer.due = tick +
   timer.period. Continue with next.
4. executing = 0.

Creating a timer (`0x00543F10(record, callback, period)`, fatal if called
while executing): prepend {callback, record, due = tick + period,
period}. So a timer made at updater tick T first fires at tick T +
period + 1 and then every period + 1 ticks until its callback returns
1; one updater tick = 20 frames. Timers made outside the updater during
frame f fire at the updater run of tick T + period + 1, where T is the
tick count at frame f.

### 6. Status reporting

#### 6.1 Status values

A record's status byte (+0x0B) is what the quest log shows (the client
owns the meaning; values used: 0 none, 1–11 progress steps, 12 completed
now, 13 completed). Default rule `0x00543F90(record, out, player record)`
used when status_fn is null (q = filter, s = state +0x0C, n = +0x0D,
L = status +0x0B; "done" = bit 13, "now" = bit 14):

1. If s < n: if now → 12; else if chain = 4 and L = 6 and not done → 12;
   else L.
2. Else (s ≥ n): if done → L (asserts q ≤ 40); else if chain = 4 and now
   → (12 if s = 6 else L); else if chain = 10 → (4 if L = 4 else 12);
   else 12.

A status function (+0xE8) returns true and writes the status, or false
(nothing reported).

#### 6.2 C→S 0x40 → S→C 0x28, 0x50, 0x52

Handler `0x0054C0C0` (size must be 1) calls `0x00546040(game, player)`:

1. Send 0x28 (type 6, GUID 0, current record).
2. list[41] = 0. For each record (list order) with status ≠ 0: assert
   chain ≤ 40; status from status_fn if set (written to list[filter] only
   if it returns 1), else the default rule into list[filter].
3. 0x50 (15 bytes: u8 0x50, u16 1, u16 Den of Evil monsters left, i16
   staff tomb, u16 barbarians left, 6 zero bytes) is sent if list[1] ≠ 0
   (then monsters left = `0x005901E0` of chain 1's record), or list[36] ≠
   0 (barbarians = `0x00588C50` of chain 32's record: 5 for each of
   extra bytes +0x86, +0x87, +0x88 that is 0, plus extra +0xA4 − +0xAC −
   +0xA8 (i32), floored at 0), or the player
   record has slot 12 bit 0 or 13 and the game has an Act II (game
   +0xC0) (then staff tomb = level id of the true tomb `0x0061AEB0` −
   66). Fields not computed stay 0.
4. Send 0x52 (42 bytes): u8 0x52, list[0..40].

#### 6.3 S→C 0x5D (one quest's status)

`0x00544190(game, player, chain)`: find the record; return unless the
player's room's act = record act. Message (6 bytes, `0x0053D710`): u8
0x5D, u8 chain, u8 record flags (+0x14), u8 status (status_fn, or the
default rule), u16 extra = Den of Evil monsters left for filter 1,
barbarians left for filter 36, else 0. Quest code sends it per player
through `0x00544300(record, status, unit, iterate_fn, iterate)`: status
byte = status; if iterate = 1 (iterate_fn null → fatal), call
iterate_fn(game, player, unit) for every player through `0x005537D0`
(`unit-order.md` §2 r5 order), and the per-quest iterate functions send
0x5D to players whose flags qualify. Two fixed forms: `0x005458E0` sends
`5D chain 02 00 0000`; `0x00545920(player, chain, act)` sends `5D chain
00 0C 0000` to the player when it has no room, or its room's level is
≠ 0 and in an act ≥ act.

#### 6.4 S→C 0x8A NpcWantsInteract

`0x00544590(game, player, npc)` (caller `0x005DDE80`, NPC AI): only for
a monster whose monstats flags byte +0xD has bit 1 (interact); asserts
the set is picked. For each record (list order) of the client's current
act whose active_fn (+0xEC) is valid: if active_fn(record, npc class,
player, player record, npc) returns true, send `8A 01 <npc GUID u32>`
(6 bytes) and stop. Recorded every 20 frames or so while near the NPC
(`8a 01 07000000` from frame 24 on in `022633`).

#### 6.5 S→C 0x89 UniqueEvent

`0x00545760(game, b)` stores b at control +0x20 and, for every player,
sends 0x28 (type 6) then `89 b` (`0x005456F0`). Den of Evil completion
uses b = 0 (`quests-act1.md` §10.4).

#### 6.6 Updating a player's flags

`0x005455B0(game, player)` sends 0x28 (type 6, GUID 0, current record).
Quest code calls it after changing a player's bits.

#### 6.7 NPC intro record and S→C 0x91

Each player has an NPC intro record per difficulty (player data +0x60 +
4·d): whether the player has heard an NPC's first-talk text (set
`0x00572420`, test `0x00572470`, by NPC class id). Act lists (§Constants)
hold 6, 11, 7, 0 and 7 NPCs for acts I–V.

- `0x00544FA0(act)` / `0x00544F60(list, n)` set the intro bit of every
  NPC of the act's list (act transitions, §8.1).
- `0x00545100(game, player, act)` (caller `0x00537340`): for acts 0, 1,
  2, 4 build 0x91 (26 bytes, `0x0053E060`): u8 0x91, u8 (D2MOO: act),
  then 12 u16 slots preset to 0xFFFF; for each NPC of the act's list in
  order whose intro bit is set, write its class id into the **next**
  slot (`0x00545090`: introduced NPCs are packed at the front; D2MOO
  1.10f writes slot i for list entry i). Sent only if at least one is
  set. Act 3 (IV) sends nothing.
- The record holds two bit arrays by NPC class (class table
  `0x00732738`; the save's NPC fields A and B, `formats/d2s.md` §6):
  record +0x04, the "introduced" bits above (`0x00572420` set,
  `0x00572470` test); record +0x00, the "first-talk text heard" bits of
  the intro records (chains 37–40): set `0x00572360` (callers
  `0x0058E9D4`, `0x0058EA25`, `0x0058F8C2`, `0x00598464`, `0x005B6CCF`:
  the intros' event 11 callbacks), test `0x005723C0` (callers
  `0x00586BC8`, `0x00586C62`, `0x005876B8`, `0x0058F94F`, `0x0058F9FC`,
  `0x00598532`, `0x005B6D9B`, `0x005B6E42`: their event 0 and active
  functions). The bits each intro sets are in its act file
  (`quests-act1.md` §10.3, `quests-act2.md`, `quests-act3-2.md`,
  `quests-act5-2.md` §9).

### 7. NPC dialog hooks

#### 7.1 NPC message tables (`quest-messages.tsv`)

A record's table (+0xDC) is an array of states, 0xC4 bytes each: 16
entries of {i32 NPC class id, i16 message index, u16 pad, i32 menu} then
an i32 count at +0xC0. `quest-messages.tsv` lists every used entry of
every 1.14d table (columns: table address, catalogue `index` list,
chain, state, slot, npc, string = message index, menu). States without
entries are omitted; table extents are bounded by the next table's
address and are consistent (no state has a count > 16, every NPC id is a
valid monstats row).

`0x00543790(record, text list, npc, state)`: for each entry of the
state with that NPC, add (message index, menu with 1 mapped to 0) to the
text list (`0x006612F0`). The NPC chat code (`world/npc.md`) builds the
list, raises event 0 (§4.1) with it, and sends it as the 0x27 message.

#### 7.2 Quest-triggered NPC text refresh

`0x00545780(game, player, npc)` (called by most quest scroll-message
callbacks after a state change): new text list; raise event 0 for the
player and NPC (dispatch (1, 1)); send it as 0x27 (`0x00661480` builds
the entries); free the list; send 0x29. Recorded after C→S
`31 10000000 4000` (message 64 to the NPC with GUID 0x10): S→C 0x27
then 0x29 in the same frame (1729).

#### 7.3 C→S 0x31 QuestMessage

Handler `0x0054BA90` (size must be 9, else result 3): `0x005443B0(npc
GUID = u32 @1, message = u16 @5)`: if GUID ≠ −1 look up the monster;
args: player, NPC class (0 if none), message; dispatch event 11 (1, 1).

### 8. Act transitions, warps and portals

#### 8.1 Act completion (`0x005467E0(game, player, npc)`)

Called by the NPC travel action (`world/npc.md`, from `0x00579D60`)
before the act change; only for a monster NPC:

| NPC (class) | Condition | Sets (player record) | Then |
|---|---|---|---|
| warriv1 (155) | slot 7 bit 0 clear | 7.0, 7.13 | 0x28; if player data +0x4C ≠ 1: set it to 1 and send 0x61 (`0x0053D940`); set intro flags (`0x00544FA0`) |
| warriv1 (155) | always, after the above | — | if slot 6 bits 13 and 0 are set and chain 6's record is not-intro: call its callback 3; then `0x00597310` (A1Q4 act-change hook) |
| meshif1 (210) | slot 15 bit 0 clear | if slot 10 bit 0 clear: 10.0, 10.13 and delete items `msf ` and `vip ` (§9.2); then 15.0, 15.13 | intro flags; 0x28; +0x4C / 0x61 |
| tyrael2 (367) | slot 28 bit 0 clear and game is expansion (game +0x70) | 28.0, 28.13 | intro flags; 0x28; if +0x4C ≠ 1: set, send 0x5D then 0x61 |

0x61 is 2 bytes, `61 <byte>`. Intro flags are `0x00544FA0(game,
player, act)` (act in CL, jump table `0x00545078` over the §6.7 lists).
1.14d values (read at the call sites; same as D2MOO 1.10f):

| Transition | Intro-flag act | 0x61 byte | Order of sends |
|---|---|---|---|
| warriv1 | 0 (Act I list) | 2 | 0x28, 0x61, then intro flags |
| meshif1 | 1 (Act II list) | 3 | intro flags, 0x28, 0x61 |
| tyrael2 | 1 (Act II list, sic) | 5 | intro flags, 0x28, `5D 17 02 00 0000`, 0x61 |
| Durance (`0x00546AC0`) | 2 (Act III list) | 4 | intro flags, 0x28, 0x61 |

0x5D and 0x61 go only when player data +0x4C ≠ 1 (it is then set to
1).

`0x00546AC0(game, player, level)` (object warp, from `0x00584750`):
level 102 (Durance of Hate 3): if slot 23 bit 0 clear: if slot 18 bit 0
clear: set 18.0, 18.13 and delete items `qey`, `qhr`, `qbr`, `qf1`,
`qf2`; then 23.0, 23.13; intro flags; 0x28 (1.14d sends it, D2MOO
1.10f does not); +0x4C / 0x61. Other levels: `0x005BCFD0`: chain 20's
record (A3Q6; none → nothing): extra +0x0C := 2, +0x10 := 2; if extra
+1 ≠ 0, the object with GUID +4 gets mode 1; if extra +2 ≠ 0, the
object with GUID +8 gets mode 2 (the Act III record fields are
otherwise uncatalogued, §11).

#### 8.2 Level warp check (`0x00545B80(game, player, from, to)`)

Called by the warp code (`0x005550B0`); nonzero = closed:

| Target level | Check |
|---|---|
| 73 Duriel's Lair | `0x0059DB20` |
| 100 Durance of Hate 1 | `0x005BBFA0` |
| 118, 128 | only when coming from 120: `0x0058D090` (else open) |
| 132 Worldstone Chamber | `0x0058E640` |
| other | open |

The checks read the quest record found by chain id (`0x00543640`, §2.3)
and its extra data X (record +0x18):

| Check | Chain id | Closed when |
|---|---|---|
| `0x0059DB20` | 13 | record found, not-intro (+0x09) ≠ 0 and X +0x0B = 0 |
| `0x005BBFA0` | 19 | the player's room is not in level 101, the record is found and X +0x0C = 0 (in level 101: open) |
| `0x0058D090` | 35 | not-intro ≠ 0 and X +0x00 = 0 (no null check on the record) |
| `0x0058E640` | 36 | record not found, or X +0x86 ≠ 1 |

(Dispatch: `to` − 73 indexes byte table `0x00545BE8` into jump table
`0x00545BD4`; the extra-data fields belong to each quest's section.)

#### 8.3 Portal check

`0x00545830(level, …)`: level 73 → `0x0059DFD0` (A2Q6 tomb), else 0
(caller `0x0056CF40`).

#### 8.4 Cow portal and Pandemonium portals

`0x00594140(game, player)`: fails (player sound, `0x00553380`) if the
game record has slot 4 bit 11, or the player record slot 4 bit 10 (killed
the Cow King); or (classic game) the player lacks slot 26 bit 0, or
(expansion) slot 40 bit 0; or the player is not in level 1 (Rogue
Encampment). Else free spot near the player (`0x00545340`, size 3,
collision mask 0x400, radius 4 (unused: `0x00545340` never reads this sixth argument, `[ebp+0x14]`; the search runs to the limit), limit 100); if found and a portal object
of class 60 to level 39 is created (`0x0056D130`), set game slot 4 bit 11
and return 1. Its only route is the cube output-kind table `0x006E11C8`
through `jmp` thunks (`world/cube.md` §9). The Pandemonium portal functions
`0x00594270` and `0x00594280` are `xor eax, eax; ret` stubs in 1.14d:
they create nothing.

### 9. Quest items, rewards and helpers

#### 9.1 Creating a reward item (`0x005466B0`)

(game, player, code, level, quality, droppable): look up the item code
(return none if absent); level = the player-based default
(`0x00558200`) unless level ≠ 0; ask item creation (`0x00559CE0`,
count 1, the given quality) for the item (items spec); if it has max
durability > 0 set durability to it; inventory page 0; try to place it in
the inventory (`0x00560200`); on success identify it unless identified
and return it; else if droppable: drop it at a free spot near the player
(`0x00545340`, size 1, mask 0x3E01, radius 5 (unused: `0x00545340` never reads this sixth argument, `[ebp+0x14]`; the search runs to the limit), limit 100) and return it;
else free it and return none.

#### 9.2 Deleting a quest item

`0x00544160(game, player, code)` finds the player's item with the code
(`0x00558110`) and removes it (`0x005440A0`: by item mode: stored →
update client and remove; equipped → unequip path; on cursor → remove).

#### 9.3 Player GUID lists

Records and quest extra data keep lists of up to 32 player GUIDs with a
u16 count (+0x80 of the list): add (`0x00545200`, ignores duplicates and
full lists), remove by swap with the last (`0x00545240`), test
(`0x00545290`; record list `0x005452C0`). `0x005455F0(game, list, slot,
sound)`: for each GUID in list order whose player exists and has neither
bit 0 nor bit 1 of the slot: set bits 13 and 1; if sound ≠ 0 attach the
sound (`0x00553380`).

#### 9.4 Reading a clue item (C→S 0x3E)

Handler `0x0054BF60` (size 5): item GUID u32 @1; the item must exist,
be the player's and in the same act; `0x00544840`: code `bkd ` (deciphered
Inifuss scroll) → `0x00593CB0` (sends the Cairn stone order, `quests-act1.md` §10.6); code
`trs ` → `0x0059D6A0` (A2Q6 true tomb): send 0x50 to the player:
u8 0x50, u16 13, i16 = true tomb level − 66 (`0x0061AEB0` on the Act II
DRLG, game +0xC0), or 0 when that is 0; a nonzero level is also stored
at chain 13's extra +0x34. Bytes 5–14 are never written (stack, as in
`quests-act1.md` §10.6).

#### 9.5 Quest functions called from object events

Object timer event 7 (QUESTFN, `sim/tick.md` §5.6) dispatches through
`0x005449E0(game, object)` by object class. "Record c" means: look up
chain c's record (no record → nothing) and call the function with
(record, object); other cases pass (game, object). Other classes do
nothing.

| Class | Case | Function |
|---|---|---|
| 0x1A (26) | Cain's gibbet | `0x00593290` (`quests-act1-rest.md` §1.2) |
| 0x7A (122) | record 11 | `0x0059B710` |
| 0x83 (131) | needs a room; mode 1 → mode 2; room level 76 → `0x005B23C0(game, object, 301, 1, −1, 0)`; level 108 → `0x005B5750` | — |
| 0xBD (189) | room level in Act I → record 4, `0x005942C0`; level 109 or ≥ 113 → record 33, `0x0058A730`; other → record 32, `0x00588CA0` | — |
| 0x10C (268) | Wirt's body, record 4 | `0x00594630` (`quests-act1.md` §10.6) |
| 0x155 (341) | record 20 | `0x005BCAC0` |
| 0x16F (367) | record 16 | `0x005B85E0` |
| 0x173 (371) | Countess chest, record 5 | `0x005956C0` (`quests-act1.md` §10.7) |
| 0x178 (376) | record 24 | `0x005B6710` |
| 0x1CB / 0x1CC / 0x1CD (459–461) | — | `0x0058B940` / `0x0058A500` / `0x00589540` |
| 0x1DA–0x1DC (474–476) | record 35 | `0x0058C0E0` |

Classes 0x173–0x1DC use the jump tables `0x00544DA0` / `0x00544DBC` (index =
class − 0x173). The Act II–V functions are catalogued only (§11).

#### 9.6 Quest-owned object init and operate functions

`world/object-functions.tsv` names this file the owner of seven object
functions. Each was re-read in the 1.14d `Game.exe` (2026-10-07,
`tools/ghidra/disasm.py`); where another quest file already states the
body, this table links it and records only what the re-read added.
Init functions take the init record {game +0x00, object +0x04, room
+0x08, control +0x0C, `objects.txt` record +0x10, x +0x14, y +0x18}
(`world/objects.md` §3 rule 6; built at `0x0054F6D0`–`0x0054F6F6`, the
control being `0x00546FA0`'s game +0x10F0); operate functions take the
operate record {game +0x00, object +0x04, operator +0x08, …, class
+0x10}. All live rows below have `Sync` = 1, so none of their mode sets
draws (`world/objects.md` §4).

| Function | Class (1.14d `objects.txt`) | Owner of the body | Re-read |
|---|---|---|---|
| init 7 `0x00544990` | 26 gibbet | `quests-act1-rest.md` §9 item 8 | confirmed: record walk from game +0x10F4 (id +0x00, next +0xF4), tail jump to `0x00594060` |
| init 9 `0x00593FC0` | 30 Inifuss tree | `quests-act1-rest.md` §9 item 9 | confirmed (`0x00543640(game, 4)`) |
| init 46 `0x005506D0` | 369 trapped soul placeholder | rules 1–6 below | new |
| init 59 `0x0054FE10` | 131 Dummy "vile dog afterglow" | rule 7 below (use: `quests-act4.md` §5.4) | confirmed |
| init 61 `0x00594290` | 189 Dummy "cain portal" | `quests-act1-rest.md` §9 item 10 | confirmed: no mode test, mode := 1 always, event 7 at f + 25 |
| operate 33 `0x00583E70` | 268 Wirt's body | `quests-act1-rest.md` §9 item 11 | confirmed; the "object exists" test only guards the mode read: a null object goes on and writes the drop code through null (fatal; operate always passes an object) |
| operate 43 `0x00584D00` | 100 Duriel's Lair portal, 377 guild portal | rule 8 below | new |

**Init 46, trapped-soul placeholder (`0x005506D0`).** Every draw is on
the object-control seed (game +0x10F0, +0x00; the function reaches it
both as `0x00546FA0`'s result and as the init record's control, the same
pointer). `roll` is `sim/rng.md` §3 (`n` < 1: 0, no draw).

1. The object exists and its mode ≠ 0 → nothing (population allocates
   it in mode 0, `world/object-population.md`).
2. (x0, y0, w, h) := the room's sub-tile box (`0x00619730`). Budget c :=
   (((w · h) >> 7) · 30) >> 8 (i32, arithmetic shifts). c ≤ 0 → nothing.
3. Up to 12 tries; a try starts only while c > 0 (tested before the
   first try and after each one). One try:
   1. px := x0 + roll(w − 4); then py := y0 + roll(h − 4).
   2. Fit A (`0x00550220`, sx = sy = 3: `world/object-population.md`
      §6, Fit A row) on (px, py) with the room's box and room. No fit →
      the try ends.
   3. class := 403 (`trappedsoul1`) + roll(2) (1 → 404 `trappedsoul2`).
      Spawn `0x005B2F20(game, room, x, y, class, mode 1, spread −1,
      flags 0)` at the **placeholder's own point** (init record x, y),
      not at (px, py). Created → object mode := 2 (`0x00624690`) and
      c −= 1.
   4. k := 1, walking := yes. Then repeat:
      1. n := k >> 1; n > 0 → roll(n); a nonzero result ends the try.
         (k = 2 and 3 give roll(1): one draw, always 0.)
      2. walking = no → the try ends.
      3. Walk: walking := no; m := 3 · max(c, 4); up to m steps, each:
         d := roll(8); px += (roll(5) + 5) · DX[d] · 2; py += (roll(5) +
         5) · DY[d] · 2 (draws in that order: roll(8), x's roll(5), y's
         roll(5)); then Fit A on (px, py); a fit → walking := yes, stop
         stepping. Failed steps still move the point: the walk is
         cumulative from the try's (px, py) and never resets in a try.
      4. No fit in m steps → back to 4.1 (so k ≥ 2 still draws its
         roll(k >> 1) before the try ends).
      5. Fit: class := 403 + roll(2); k += 1; spawn as in 3.3 at (px,
         py); created → object mode := 2, c −= 1. Back to 4.1.
   5. c is not tested inside rule 3.4, so it can end below 0; the
      outer test then stops the tries.
4. DX = (−1, 0, 1, −1, 1, −1, 0, 1), DY = (−1, −1, −1, 0, 0, 1, 1, 1)
   for d = 0…7 (i32 tables `0x00731B7C`, `0x00731B9C`, read from the
   image). One step moves 10–18 sub-tiles on each nonzero axis.
5. Fit A compares the low 16 bits of px / py with the box (a walk that
   goes negative never fits); its two collision boxes take the full
   values.
6. Each spawn draws as `monsters/init.md` §1–§4 and the placement of
   `monsters/population.md` §9.3 say (their own seeds; they do not touch
   the control seed). The object ends in mode 2 when at least one trapped
   soul was created, else stays in mode 0. No message, timer or event.

**Rule 7, init 59 (`0x0054FE10`).** Object null or mode 0: mode := 1
(`0x00624690`), object event 7 at frame (game +0xA8) + 27, then object
event 1 at frame + (record +0xDC >> 8) + 1 (`FrameCnt1`; Dummy 131: 20,
so f + 21) (`0x005417D0`, both with the two trailing arguments 0).
Mode ≠ 0 → nothing. No draw. Event 7 then runs the class-131 row of
§9.5.

**Rule 8, operate 43 (`0x00584D00`), Duriel's Lair / guild portal.**
Returns 0 in every non-fatal case. No draw of its own.

1. Operator null or not a player (type ≠ 0) → fatal (assert, line
   0xFB3).
2. Portal flags (object data +0x05, `0x006222C0` / `0x00622300`) |= 5.
3. Destination level L: class 100 → 73 (Duriel's Lair); class 377 → the
   object's `InteractType` (data +0x04); other classes → 0.
4. a := act of L (`0x006427F0`, `drlg/levels.md` §6 rule 3); a ≥ 5 →
   fatal (0xFCB).
5. Room R := `0x0061B060(act record game +0xBC + 4·a, L, tile index 0,
   &x, &y, operator size 0x00620510)` (`sim/path-placement.md` §11; it
   may draw on the level seed); none → fatal (0xFCD).
6. Free point from R at (x, y): `0x0064E7B0(R, &pt, operator size, mask
   0x1C09, fallback 0)` (`sim/path-placement.md` §7). None → return 0
   (the flags of step 2 stay set; the player is not moved).
7. Found room R' → place the operator: `0x00554EA0(game, operator, R',
   pt.x, pt.y, 0, 0)` (`sim/path-placement.md` §10); failure → fatal
   (0xFD9).

### 11. Acts II–V

Catalogued in `quests.tsv` (records, callbacks, tables) and `quest-messages.tsv`.
Hooks other specs rely on are specified above (§8.1–§8.4, §9.4, §9.5).
Their state machines are not yet specified (Open question 8).

## Constants & data dependencies

| Item | Value | Source |
|---|---|---|
| quest records | 37 table rows + 4 intros | `0x00731888`, `0x007318AC` |
| flag record | 96 bytes, 42 slots | `0x0065C430`, `0x0065C4D0` |
| quest record size | 0xF8 | `0x00545D80` |
| updater period | 20 frames | `sim/tick.md` §3 |
| intro NPC lists | Act I 148,154,150,147,155,265; Act II 175,176,177,178,202,200,210,201,198,199,244; Act III 245,252,253,254,255,264,297; Act V 520,521,511,512,513,514,515 | `0x007318B0`, `0x007318C8`, `0x007318F4`, `0x00731910` |
| sequence switch | byte `0x0073150C` = 1 | §3 |

Tables read: `levels.txt` `Quest`; item `quest` (`questdiffcheck` is
read by item code, not here); `monstats` interact flag; `superuniques`
hcIdx.

## Randomness

In order of occurrence; no other quest draws exist in Act I.

| When | Seed | Draws | Decides |
|---|---|---|---|
| game creation (§2.3) | game seed | 1 step | quest seed `{lo', 666}` |
| Flavie chat (`quests-act1.md` §10.3), per handling record | player unit seed (+0x20) | 1 (`roll(2)` or lo' mod 3) | which line |
| Cairn stone order (`quests-act1.md` §10.6), once | quest seed | ≥ 5 steps (one per try, collisions retried) | stone order |
| Wirt's body, first event 7 | quest seed | 1 (lo' mod 10) | gold piles 10–19 |
| Andariel kill (`quests-act1.md` §10.8) | quest seed | 3, each followed by an item drop's own draws | gem codes |

Item drops, monster and missile spawns draw from their own seeds (items,
monster specs). Quest-seed sites outside Act I (for later specs):
`0x00589580`, `0x0058DF20`, `0x0058E830`, `0x00599C10`, `0x00599CF0`,
`0x00599DF0`, `0x0059A7E0`, `0x0059B1C0`, `0x0059EDC0`, `0x005B6710`,
`0x005B8860`, `0x005B8940`, `0x005B8A20`, `0x005BD390`. A2Q7 gossip
(`0x0059E140`) draws from the player unit seed like Flavie.

Object functions of §9.6: init 46 (trapped-soul placeholder, at the
object's creation) draws on the object-control seed (game +0x10F0), in
§9.6 rule 3's order: per try roll(w − 4), roll(h − 4), then roll(2) per
spawn, roll(k >> 1) per cluster round (k ≥ 2), and roll(8), roll(5),
roll(5) per walk step. Inits 7, 9, 59, 61 and operates 33, 43 draw
nothing themselves (33's drop and 43's spawn-point search draw in the
item and path-placement specs).

## Edge cases & original bugs

1. The first player to enter decides the switched-off quests for the
   whole game (§3).
2. Two records (chains 25 and 30) handle Flavie's event 0 with the same
   table: two message-list additions and two player-seed draws per chat
   (code reading; Open question 2).
3. 1.14d passes force = 0 for pick-up/drop events (§4.3): an inactive
   quest ignores them.
4. A3Q7's record reports in slot 16 (its init stores 0x10) while its
   table slot is 32; D2MOO 1.10f has the same 16.
5. The default status rule asserts filter ≤ 40 but intro records use 42;
   they never reach it (status functions return false).
6. Timer due values are compared unsigned; the wrap at tick 2^32 − 1
   rewrites them as 0xFFFFFFFF − due.
7. The Malus needs character level 8 in 1.14d (`quests-act1.md` §10.5).
8. 0x50's unused fields are 0; the client reads all three counters from
   one message.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| set(slot 1, bit 2) on a zero record | byte 2 = 0x04, others 0 | §1.1 |
| set bits 2, 3 of slot 1 | bytes 2..3 = `0c 00` | recorded 0x28 at frame 3570 (`015956`) |
| copy_in normalize: slot 5 word 0x6002 (bits 1, 13, 14) | 0x8002 (13, 14 cleared; 15 set from bit 1) | §1.6 |
| copy_in normalize: word 0x4001 | 0x0001 | §1.6 |
| timer period 8 created at tick 100 | fires at ticks 109, 118, … until it returns 1 | §5 |
| default rule: chain 10, s ≥ n, not done, L = 4 | 4; L = 3 → 12 | §6.1 |
| default rule: chain 1, s < n, now clear, L = 1 | 1 | §6.1 |
| fresh game, first entry | `5e` + 37 × `01`; `28 06 00000000 00` + record; `29` + 96 zero bytes | recorded frame 1, both recordings |
| player record slot 0 bit 0 set (talked to Warriv before) | game record stays all zero (row 0 has no_set_state = 1) | recorded frame 1, `015956` |
| C→S `31 10000000 4000` (Akara, msg 64) then chat end `30 01 10000000` | S→C 0x27, 0x29 at frame 1729; `5d 01 00 01 0000` at frame 1744; next 0x28 has slot 1 = `04 00` | `015956` frames 1729–1751 |
| Cairn order, quest-seed lo' mod 5 = 2, 2, 0, 4, 1, 3 (6 steps; the second 2 is a collision) | order = [18, 20, 17, 21, 19]; 0x50 values 1, 3, 0, 4, 2 | `quests-act1.md` §10.6 |
| Andariel, lo' values 9, 3, 13 | chipped[2] `gcb `, chipped[3] `gcy `, normal[6] `sku ` | `quests-act1.md` §10.8 |
| A1Q1 event 0 with Akara: state 1, not-intro 1, R slot 1 = 0 | message state 0: Akara 64 | `quests-act1.md` §10.4 |
| A1Q1 event 0: R has 1.1 (any state) | message state 3: Akara 76 | `quests-act1.md` §10.4 |
| A1Q1 event 0: state 4, R lacks 1.0, 1.1, 1.13, GUID not in list | nothing added | `quests-act1.md` §10.4 |
| A1Q1 event 3 a = 1, b = 2: state 2, status 0, one player with slot 1 = 0x0004 | state 3; player slot 1 = 0x0014 (bit 4, bug kept); status 1, `5d 01 00 01 0000` | `quests-act1.md` §10.4 |
| A1Q1 event 8: P = 10, V = 10, spawned 40, killed 37, state 3 | left = 3; `5d 01 20 04 0300` per qualifying player (default rule: s < n, L = 4); callback 2 null | §6.1, `quests-act1.md` §10.4 |
| A1Q1 event 8: P = 10, V = 10, spawned = killed = 40 | state 4; game slot 1 bit 13; killers get 13, 1; others get 14 + `5d 01 00 0c 0000` + 0x28; `89 00`; timer 8 | `quests-act1.md` §10.4 |
| A1Q1 timer made at updater tick 100, state still 4 | tick 109: broadcast(5, 0), timer removed | §5, `quests-act1.md` §10.4 |
| A1Q1 msg 76 with R slot 1 = 0x2002 (13, 1), state 4, chain 2 state 0 not-intro 1 | chain 1 state 5, status 13; chain 2 state 1; R slot 1 = 0x2001; slot 41 = 0x2002; stat 5 + 1 | `quests-act1.md` §10.4, §10.1 |
| Sequence from chain 1 (state 5), chain 2 state 5, chain 4 state 0 | chain 4 state 1; walk stops (returns 1) | `quests-act1.md` §10.1 |
| A1Q2 event 3 a = 1, b = 17: state 0, not-intro 1, status 0 | state 3, flags 0, status 2: `5d 02 00 02 0000` to qualifying players; then J2 (state 3, status 2 → 2.4 for players lacking 2.0, 2.1) | `quests-act1.md` §10.5 |
| A1Q3 status fn, R slot 3 = 0x0002 | returns 1, out 10 | `quests-act1.md` §10.5 |
| A1Q3 Malus operate, level 7, not-intro 1, R slot 3 = 0 | sound event 19 on the player; no drop; state unchanged | `quests-act1.md` §10.5 |
| A1Q6 Andariel killed, timer made at updater tick T | firings at T + 2, T + 4, …; the 9th (counter 10) opens the portals (O7), the 11th (counter 12, tick T + 22) sends status 3 and removes the timer | §5, `quests-act1.md` §10.8 |
| A1Q4 stone 0x50 with order [18, 20, 17, 21, 19] | `50 0400 0100 0300 0000 0400 0200` + 2 unwritten bytes | `quests-act1.md` §10.6 |
| Cow King killed by a classic-game player lacking 26.0 | nothing (no bits, no `vps ` drops) | `quests-act1.md` §10.6 |

## Provenance

- Read from the 1.14d `Game.exe` exports (`re/exports/funcs`,
  `all.asm`) and raw bytes of `game/Game.exe` (init tables, NPC lists,
  message tables, gem lists, stub bytes at `0x00594270`/`0x00594280`):
  addresses inline. Assert strings `.\QUESTS\QUESTS.CPP`,
  `.\QuestRecord\QuestRecord.cpp` and `d:\diablo2\…\Quests\a1q*.cpp`
  identify the files.
- D2MOO 1.10f (`D2Game/src/QUESTS/Quests.cpp`, `ACT1/*.cpp`,
  `D2Common/src/D2QuestRecord.cpp`) gave names and structure; every rule
  above was matched to the 1.14d function named next to it. Differences
  found: 37 table rows (new chain 30 / slot 41), record 0xF8 with +0xE4,
  42-slot normalize loop, force 0 for events 4/5, Malus level gate,
  0x28 on the Act III transition, 1.14d special cases in §4.6, the
  respec slot, the cow portal's search limit 100, stubbed Pandemonium
  portals.
- `quests.tsv` and `quest-messages.tsv` were generated from the init
  table and init functions by a script over the exports (spec session
  scratch; regenerate the same way).
- Recordings: `015956-packets` and `022633-packets` (messages and frames
  cited in Test vectors).
- §9.6 (2026-10-07, quests-fixups): `0x00544990`, `0x00594060`,
  `0x00593FC0`, `0x005506D0` (with `0x00550220`, `0x00619730`,
  `0x00546FA0`, `0x005B2F20`, tables `0x00731B7C` / `0x00731B9C` read
  from the image), `0x0054FE10`, `0x00594290`, `0x00583E70`,
  `0x00584D00` (with `0x006222C0`, `0x00622300`, `0x006427F0`,
  `0x0061B060`, `0x0064E7B0`, `0x00554EA0`), init record layout at
  `0x0054F6D0`–`0x0054F6F6`; classes from live 1.14d `objects.txt`.

## Open questions

1. Status byte meanings 1–11 per quest (client quest-log text): settle
   from the client's quest log code (`0x0045CC00` 0x52 handler).
2. Does Flavie's chat really draw twice from the player seed and list
   two lines? Settle with an RNG + packets recording of one Flavie chat.
3. (Settled: the cow portal is reached through the cube's thunk table,
   `world/cube.md` §9.) A recording of a cow-portal transmute would still
   confirm its draws (`world/cube.md` open question 4).
4. Which game-entry path (mode 0 or 1, §3) single player takes: record a
   game start with a breakpoint on `0x00546270`.
5. (Answered: `quests-act1-rest.md` §7: bytes 13–14 are confirmed never
   written by `0x00593CB0`; d2rs writes 0 and exact-match comparison
   masks them.)
6. (Answered 2026-10-07: `0x0058EA50` sets event 0 `0x00586B50`, event
   11 `0x0058E990`, table `0x00732FF8`, active 1, state 0, status 0,
   extra none, filter 42, status fn `0x00586C40`, active fn
   `0x00586C50`; nothing else (`0x0058EA50`–`0x0058EAB4`). `quests.tsv`
   row 40 and the table's 15 rows in `quest-messages.tsv` hold it; the
   behaviour is `quests-act5-2.md` §9.)
7. (Answered: `quests-act1-rest.md` §6 states what the quest code reads
   of the party list at game +0x1D2C (`0x00554630`, `0x00540710`,
   `0x00540510`); future owner `world/party.md`. The Act I iterate tests
   are in `quests-act1.md` §10.4–§10.8 (A1Q2's J3 and J7 in `quests-act1.md` §10.5).)
8. Acts II–V state machines (later spec).
9. Event 1, 6, 7, 12 raisers: none found; confirm no indirect calls.
10. (Settled: §8.1 table, read from the call sites.) A recording of
    each act change would still confirm the 0x61 bytes.
11. (Answered: `quests-act1-rest.md` §1 gibbet operate `0x00593480` and
    quest function `0x00593290`; §2 stone init `0x005935E0`, portal
    timer `0x00592D50` and the stone value (= the object's class); §3
    the +0x6C/+0x70 marker object (init `0x005940E0`). `0x005944F0`
    stays open there, its Open question 1.)
12. (Answered: `quests-act1-rest.md` §4; a Countess-kill recording would
    still confirm it, its Open question 3.)
13. (Answered: `quests-act1-rest.md` §5: it raises the character
    progression in the client save flags; future owner the save spec.)
14. Which bits a played completion of each quest leaves (§1.8 rule 3):
    Needs recording: a 1.14d character that completed every quest on
    Normal (expansion), saved after the last one and loaded once; dump
    the quest section (`d2s-tool dump`) and list, per slot, the bits
    set, against a second save of the same character after another
    game (bits 13 / 14 must be gone).
15. Quest-owned object functions with no stating spec
    (`object-functions.tsv` rows owned by this file: inits 7, 9, 46, 59,
    61, operates 33, 43): **Answered** (2026-10-07): §9.6 (inits 46, 59
    and operate 43 stated there; the others confirmed and linked).
