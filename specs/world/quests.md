# Spec: World — Quest system (flags, records, events, updater, Act I)

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
| Summary | 57–70 |
| Inputs | 71–82 |
| Outputs / state changes | 83–89 |
| Rules | 90–91 |
|   1. Quest flag records | 92–194 |
|   2. Quest control and quest records | 195–290 |
|   3. Game entry: picking the quest set | 291–323 |
|   4. Events and dispatch | 324–408 |
|   5. Quest updater and timers (tick step 8) | 409–430 |
|   6. Status reporting | 431–522 |
|   7. NPC dialog hooks | 523–555 |
|   8. Act transitions, warps and portals | 556–637 |
|   9. Quest items, rewards and helpers | 638–703 |
|   10. Act I quests | 704–1498 |
|   11. Acts II–V | 1499–1504 |
| Constants & data dependencies | 1505–1519 |
| Randomness | 1520–1538 |
| Edge cases & original bugs | 1539–1557 |
| Test vectors | 1558–1589 |
| Provenance | 1590–1611 |
| Open questions | 1612–1644 |
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

| Bit | D2MOO name | Meaning (as used by the Act I code, §10) |
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
| 41 | Akara respec reward (§10.3) |

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
| spec | `specified` (this spec, §10) or `catalogued` (row only) |

Values are what the init function stores (`-` = not stored, so 0 from
the zeroed record). Row 40 (Act V intro, init `0x0058EA50`) is not
disassembled in the exports (`?`; Open question 6). The `version` column
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
uses b = 0 (§10.4).

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
collision mask 0x400, radius 4, limit 100); if found and a portal object
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
(`0x00545340`, size 1, mask 0x3E01, radius 5, limit 100) and return it;
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
Inifuss scroll) → `0x00593CB0` (sends the Cairn stone order, §10.6); code
`trs ` → `0x0059D6A0` (A2Q6 true tomb): send 0x50 to the player:
u8 0x50, u16 13, i16 = true tomb level − 66 (`0x0061AEB0` on the Act II
DRLG, game +0xC0), or 0 when that is 0; a nonzero level is also stored
at chain 13's extra +0x34. Bytes 5–14 are never written (stack, as in
§10.6).

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
| 0x10C (268) | Wirt's body, record 4 | `0x00594630` (§10.6) |
| 0x155 (341) | record 20 | `0x005BCAC0` |
| 0x16F (367) | record 16 | `0x005B85E0` |
| 0x173 (371) | Countess chest, record 5 | `0x005956C0` (§10.7) |
| 0x178 (376) | record 24 | `0x005B6710` |
| 0x1CB / 0x1CC / 0x1CD (459–461) | — | `0x0058B940` / `0x0058A500` / `0x00589540` |
| 0x1DA–0x1DC (474–476) | record 35 | `0x0058C0E0` |

Classes 0x173–0x1DC use the jump tables `0x00544DA0` / `0x00544DBC` (index =
class − 0x173). The Act II–V functions are catalogued only (§11).

### 10. Act I quests

#### 10.1 Common pattern

Act I quests 1, 2, 5, 6 share a shape (verified per quest below):

| State (+0x0C) | Reached when | Player bits set |
|---|---|---|
| 0 | init (1 for A1Q1) | — |
| 1 | the sequence function of the quest before it in the chain (`seq_id`) | — |
| 2 | the quest NPC's start message (C→S 0x31) | 2 (STARTED), to players without bit 0 or 1 |
| 3 | leaving town / entering the area (event 3) | 3 (LEAVETOWN) or 4 (ENTERAREA) |
| 4 | goal done (kill, event 8) | 13 + 1 (reward pending) for qualifying players; 14 (COMPLETEDNOW) and a 0x5D (§6.3) |
| 5 | the NPC's completion message | 0 (granted), 1 cleared; reward |

Callback 13 (player started game) restores state from the starting
player's bits: bit 4 → state 3, status 2; bit 3 → state 3, status 1;
bit 2 → state 2, status 1 (unless bit 0 or 15 is set). Chain 1:
`0x00590690`, chain 6: `0x00596900` (same steps; the slot is a constant
in each). Event 2 (chat end) after the start message sends status 1 to
every player (§6.3; chain 1: §10.4).

Shorthands used in §10.4–§10.8:

- **broadcast(S, f)**: flags (+0x14) := f, then `0x00544300(record, S,
  0, I, 1)`: status (+0x0B) := S and the quest's status iterate I runs
  for every player. **status(S)** is `0x00544300` with iterate 0:
  status := S, nothing sent.
- **every player F**: `0x005537D0(game, 0, arg, F)` (`sim/unit-order.md`
  §2 r5: hash order, players with state 7 skipped, stops at a call
  returning 1); every Act I iterate function returns 0, so all players
  are visited.
- **add state k**: §7.1 `0x00543790(record, text list, NPC class, k)`.
- R is the event player's current-difficulty record (§1.4).

Sequence functions (record +0xF0) unlock the next quest. Each is called
with its own record (§3 step 3, reward handlers) and runs:

1. Own step (column 3): if it applies, do it and return 1.
2. If state ≠ the pass state and not-intro ≠ 0: return 1.
3. next = lookup(`seq_id`) (§2.3); none → return 0. Fatal if the
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
already done (state 5; chain 4: 6) or switched off (§3) passes the call
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
  only distinct role is the §3 step 2a test on slot 41. Both records
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
  the NPC intro record (§6.7; set `0x00572360`, test `0x005723C0`, both
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
§6.1), active fn `0x005905B0`, seq fn `0x00590620`; extra = 0x8C zeroed
bytes from the game pool, GUID list emptied (`0x00545300`).

| Extra | Type | Field |
|---|---|---|
| +0x00 | 32 × u32, u16 count at +0x80 | killers: players who killed a level-8 monster (§9.3 list) |
| +0x84 | u8 | cleared (written, never read) |
| +0x85 | u8 | entered (written, never read) |
| +0x86 | u8 | talked: message 64 given, chat end not yet handled |
| +0x87 | u8 | timer pending |
| +0x88 | i32 | monsters left; `0x005901E0` returns it (0x50, 0x5D) |

Iterate functions (game, player, arg; all return 0; slot 1 is a
constant in each):

| Id | Function | Effect on one player P |
|---|---|---|
| I1 | `0x0058FBE0` | if P has neither 1.0 nor 1.15, or has 1.13 or 1.14: 0x5D for chain 1 (`0x00544190`, §6.3) |
| I2 | `0x0058FC90` | if P has neither 1.0 nor 1.1: chain 1 state 2 → set 1.2; state 3 → set 1.3 if status = 1, else 1.4; other states nothing |
| I3 | `0x00590190` | if P has 1.13 and P's party id ≠ 0xFFFF (`0x00554630`: a player unit with +0x80 ≠ 0 whose GUID is in a party of the list at game +0x1D2C, else 0xFFFF): for each member of that party (list order; GUID lookup `0x00552F60`, missing ones skipped) run `0x00590120`: if the member has neither 1.0 nor 1.1 and a room whose level is ≠ 0 and in Act I (`0x006427F0` = 0): set 1.13, then 1.1 (no 0x28) |
| I4 | `0x00590080` | if P has neither 1.0 nor 1.1: set 1.14; `5D 01 00 0C 0000` to P (`0x00545920` with act 0: sent unless P's room has level 0); 0x28 to P (§6.6) |
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

**Event 8** `0x00590260` (a monster with a chain-1 link dies, §4.4;
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
   3. Killers list: `0x005455F0(game, list, 1, sound 0)` (§9.3: set 1.13,
      1.1 for each killer lacking 1.0 and 1.1).
   4. Every player I3, then every player I4, then every player I5 (arg =
      the victim, unused).
   5. `0x00545760(game, 0)`: every player gets 0x28 then `89 00` (§6.5).
   6. If timer pending = 0: timer pending := 1; timer (record,
      `0x00590230`, period 8) (§5).
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
   every player I2; refresh the text (§7.2, with the target NPC). End.
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

**Active** `0x005905B0` (§6.4): true iff the NPC class is 148, R lacks
1.0, and (R has 1.1, or not-intro = 1 and state = 1).

**Sequence** `0x00590620`: §10.1.

No draws. 0x50 and 0x5D carry `left` while the status ≠ 0 (§6.2,
§6.3); §3 step 8 sends `89 00` to later joiners once state ≥ 4.

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
   only while the record is active, §4.3): if R lacks 3.6: set 3.6,
   sound event 36 on the player. Then broadcast(2, 0).
7. **Event 6** `0x00591A90` (never raised in 1.14d, §4.1): if
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
       3.13, 3.1; 0x28 to the player; delete its `hdm ` (§9.2); +0x9C
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
| +0x28, +0x49 | u32, u8 | GUID of the object of class 61 linked to chain 4 (§4.6, `0x00592F80`), and "linked" := 1 |
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
   2. If +0x4F = 1 and the player has `bkd `: delete it (§9.2), +0x7C −=
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
5. **Event 6** `0x00592E60` (never raised, §4.1): if +0x4F ≠ 1 and state
   ≤ 5: state := 3; tree reset (step 13).
6. **Event 8** `0x00593E70` (the Cow King's death; dispatched with force,
   §4.4):
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
   2. Akara, 112, the player has `bks `: delete it; create `bkd ` (§9.1:
      level 0, quality 2, droppable). Created: +0x64, +0x4B, +0x4E := 1;
      +0x38 := its GUID; state := 5; flags := 0; status(3). Not created:
      +0x7C −= 1. Without `bks `: nothing.
   3. Akara, 118, R has 4.1: set 4.0, clear 4.1; add the GUID to the
      record list; 0x28; ring `rin ` (§9.1, droppable): Normal level 7
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
    (`0x00545340`, args 2, 0x100, 1, 100); none → (x, y, R0). Spawn
    cain5 (`0x005B2F20(game, room, x, y, 265, mode 1, r 5, 0)`). On
    failure up to 20 retries, each moving the point by (+1, +1), taking
    its room (`0x00463740`; none → (x, y, R0)), a free spot (args 2,
    0x100, 2, 100) and r 10; then one last try at (x, y, R0) with r 15.
    Spawned: unit +0xC4 |= 0x3000000; +0x51 := 1; +0x52 := 0; +0x68 :=
    its GUID.
16. **Act change** `0x00597310(game, player)` (§8.1): if R lacks 4.0
    and 4.1, +0x50 = 0 and state < 6: Cain cleanup (0, 1); state := 7;
    broadcast(5, 0); game record 4.13; every player L3.
17. **add_link** `0x00592F80` (§4.6): +0x49 := 1; +0x28 := the object's
    GUID (−1 when none).
18. **Sequence** `0x00593D70`: §10.1.

- Cairn stone order (`0x00592E90`, run once: +0x4A guards it in
  `0x00593CB0`; also run when a stone is operated, `0x00593710`):
  order[0..4] = 0; i = 0; while i < 5: step the quest seed, k = lo' mod
  5; if order[k] = 0: order[k] = 17 + i, i += 1.
- Stone order message `0x00593CB0(game, player)` (§9.4): compute the
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
  quest function `0x00593290` (§9.5 class 26), the stone init
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
| +0x8C | §9.3 GUID list | list C: players in Tower Cellar 5 at the kill |
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
    (§9.5) runs `0x005956C0`: trap step, then the same reschedule.
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
extra 0x198 bytes: three §9.3 GUID lists at +0x00 (Cain), +0x84 (Akara),
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

The Warriv travel hook (§8.1, `0x005467E0`) calls this record's event 3
with args (game, 3, target and the fifth dword unset, player, a = 1, b =
40): step 3.2, so a state-4 record goes to 5. `0x00597310` runs after
it whether or not chain 6's record exists.

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
| Flavie chat (§10.3), per handling record | player unit seed (+0x20) | 1 (`roll(2)` or lo' mod 3) | which line |
| Cairn stone order (§10.6), once | quest seed | ≥ 5 steps (one per try, collisions retried) | stone order |
| Wirt's body, first event 7 | quest seed | 1 (lo' mod 10) | gold piles 10–19 |
| Andariel kill (§10.8) | quest seed | 3, each followed by an item drop's own draws | gem codes |

Item drops, monster and missile spawns draw from their own seeds (items,
monster specs). Quest-seed sites outside Act I (for later specs):
`0x00589580`, `0x0058DF20`, `0x0058E830`, `0x00599C10`, `0x00599CF0`,
`0x00599DF0`, `0x0059A7E0`, `0x0059B1C0`, `0x0059EDC0`, `0x005B6710`,
`0x005B8860`, `0x005B8940`, `0x005B8A20`, `0x005BD390`. A2Q7 gossip
(`0x0059E140`) draws from the player unit seed like Flavie.

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
7. The Malus needs character level 8 in 1.14d (§10.5).
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
| Cairn order, quest-seed lo' mod 5 = 2, 2, 0, 4, 1, 3 (6 steps; the second 2 is a collision) | order = [18, 20, 17, 21, 19]; 0x50 values 1, 3, 0, 4, 2 | §10.6 |
| Andariel, lo' values 9, 3, 13 | chipped[2] `gcb `, chipped[3] `gcy `, normal[6] `sku ` | §10.8 |
| A1Q1 event 0 with Akara: state 1, not-intro 1, R slot 1 = 0 | message state 0: Akara 64 | §10.4 |
| A1Q1 event 0: R has 1.1 (any state) | message state 3: Akara 76 | §10.4 |
| A1Q1 event 0: state 4, R lacks 1.0, 1.1, 1.13, GUID not in list | nothing added | §10.4 |
| A1Q1 event 3 a = 1, b = 2: state 2, status 0, one player with slot 1 = 0x0004 | state 3; player slot 1 = 0x0014 (bit 4, bug kept); status 1, `5d 01 00 01 0000` | §10.4 |
| A1Q1 event 8: P = 10, V = 10, spawned 40, killed 37, state 3 | left = 3; `5d 01 20 04 0300` per qualifying player (default rule: s < n, L = 4); callback 2 null | §10.4, §6.1 |
| A1Q1 event 8: P = 10, V = 10, spawned = killed = 40 | state 4; game slot 1 bit 13; killers get 13, 1; others get 14 + `5d 01 00 0c 0000` + 0x28; `89 00`; timer 8 | §10.4 |
| A1Q1 timer made at updater tick 100, state still 4 | tick 109: broadcast(5, 0), timer removed | §10.4, §5 |
| A1Q1 msg 76 with R slot 1 = 0x2002 (13, 1), state 4, chain 2 state 0 not-intro 1 | chain 1 state 5, status 13; chain 2 state 1; R slot 1 = 0x2001; slot 41 = 0x2002; stat 5 + 1 | §10.4, §10.1 |
| Sequence from chain 1 (state 5), chain 2 state 5, chain 4 state 0 | chain 4 state 1; walk stops (returns 1) | §10.1 |
| A1Q2 event 3 a = 1, b = 17: state 0, not-intro 1, status 0 | state 3, flags 0, status 2: `5d 02 00 02 0000` to qualifying players; then J2 (state 3, status 2 → 2.4 for players lacking 2.0, 2.1) | §10.5 |
| A1Q3 status fn, R slot 3 = 0x0002 | returns 1, out 10 | §10.5 |
| A1Q3 Malus operate, level 7, not-intro 1, R slot 3 = 0 | sound event 19 on the player; no drop; state unchanged | §10.5 |
| A1Q6 Andariel killed, timer made at updater tick T | firings at T + 2, T + 4, …; the 9th (counter 10) opens the portals (O7), the 11th (counter 12, tick T + 22) sends status 3 and removes the timer | §10.8, §5 |
| A1Q4 stone 0x50 with order [18, 20, 17, 21, 19] | `50 0400 0100 0300 0000 0400 0200` + 2 unwritten bytes | §10.6 |
| Cow King killed by a classic-game player lacking 26.0 | nothing (no bits, no `vps ` drops) | §10.6 |

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
6. Act V intro init `0x0058EA50` (not disassembled): callbacks and table.
7. (Answered: `quests-act1-rest.md` §6 states what the quest code reads
   of the party list at game +0x1D2C (`0x00554630`, `0x00540710`,
   `0x00540510`); future owner `world/party.md`. The Act I iterate tests
   are in §10.4–§10.8 (A1Q2's J3 and J7 in §10.5).)
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
