# Spec: World — NPC interaction (talk, menus, heal, identify, mercenaries, services)

- **Status:** draft: every rule is read from the 1.14d `Game.exe` (addresses
  below) and the talk / trade / buy / sell message sequences match the
  hand-played recording `traces/raw/20261006-015956-packets.jsonl`
  (Charsi, Akara, Warriv, Flavie; Test vectors); a hire at Kashya is
  recorded (`traces-raw-buddy/merc1-spawn-packets.jsonl`, Test vectors);
  a heal at Fara is recorded (`a2-npc-fara-heal`, §5); resurrect and
  the service actions have no recording yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::npc`
- **Related specs:** `world/vendors.md` (store inventories, gamble lists,
  prices, buy / sell / repair; it uses the NPC records of §1 and the 0x2A
  message of §9); `world/quests.md` (quest slots and bits, the 0x27 text
  list §7.1, 0x28 / 0x29, act completion §8.1, the respec slot 41 §10.3);
  `world/waypoints.md` (act change and waypoint activation used by §8.3);
  `sim/intents-events.md` §2 (dispatcher, gate, result codes) +
  `client-messages.tsv` / `server-messages.tsv`; `sim/tick.md` §5
  (timer events); `sim/rng.md` §3, §5.2, §7 (seeds and helpers);
  `data/fields.tsv` (`hireling`, `monstats`, `npc`); item creation
  `items/generation.md`, item messages `items/inventory-moves.md`, NPC
  AI `monsters/ai.md` §9.9, monster data `monsters/init.md`, the
  mercenary unit `world/hirelings.md`, stat and skill resets
  `combat/vitals.md` §2.1 / `skills/levels.md` §6.5.
  Machine table: `world/vendors.tsv` (per-NPC roles, shared with
  `vendors.md`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 53–66 |
| Inputs | 67–77 |
| Outputs / state changes | 78–85 |
| Rules | 86–87 |
|   1. NPC control and records | 88–142 |
|   2. Starting an interaction (C→S 0x13) | 143–259 |
|   3. Chat open and close (C→S 0x2F, 0x30) | 260–298 |
|   4. Menu actions (C→S 0x38) | 299–357 |
|   5. Healing on chat open | 358–393 |
|   6. Cain identify (C→S 0x34) | 394–422 |
|   7. Mercenaries | 423–546 |
|   8. NPC services (C→S 0x38, action ∉ {1, 2, 3}) | 547–626 |
|   9. S→C 0x2A NPC transaction (15 bytes) | 627–660 |
|   10. Dead code in 1.14d (no caller, no pointer reference) | 661–672 |
| Constants & data dependencies | 673–685 |
| Randomness | 686–698 |
| Edge cases & original bugs | 699–774 |
| Test vectors | 775–798 |
| Provenance | 799–849 |
| Open questions | 850–908 |
<!-- /index -->

## Summary

Town NPCs are monsters whose `monstats` row has the `interact` flag. Each
game keeps one NPC record per such monster class (§1). A player starts an
interaction with C→S 0x13 when close enough (§2): the NPC stops, the
player is added to the NPC's interaction list and the server sends the
NPC's text list (0x27) and quest state (0x29, 0x28). C→S 0x2F opens the
chat (healers heal, §5), 0x30 closes it (§3). C→S 0x38 picks a menu
action: trade, gamble, hire list, or an NPC-specific service (imbue,
socket, personalize, respec, act travel, §4, §8). Dedicated messages
identify items at Cain (0x34, §6), hire (0x36) and resurrect (0x62) a
mercenary (§7). Results of NPC transactions are reported with S→C 0x2A
(§9). Store contents and prices belong to `vendors.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S 0x13, 0x2F, 0x30, 0x34, 0x36, 0x37, 0x38, 0x62 | intents | `client-messages.tsv` |
| NPC table (43 entries) | `Game.exe` data `0x00731184` | §1.2, `vendors.tsv` |
| `monstats` flags `npc` (bit 8), `interact` (bit 9) of the flag word +0x0C | table | `fields.tsv` |
| `hireling` rows | table | §7 |
| player quest record of the game's difficulty | player data +0x10 + 4·difficulty | `quests.md` §1 |
| NPC-control seed | game +0x1D24 → control +8 | §1.1, `rng.md` §5.2 |

## Outputs / state changes

NPC records (interaction, hire lists); player interact unit; S→C 0x27,
0x28, 0x29 (talk), 0x4F + 0x4E (hire list), 0x2A (transaction result),
0x58 (service result), 0x9B (resurrect), stat messages (heal), sound 10
on the NPC; gold; items identified, created, socketed, personalized;
mercenary units (creation itself: mercenary spec); act changes.

## Rules

### 1. NPC control and records

#### 1.1 Creation (`0x00536070`, at game creation)

1. Allocate 64 records of 0x44 bytes (zeroed) and the control block
   (game +0x1D24): +0x00 record count, +0x04 record array, +0x08 seed,
   +0x10 count (same value).
2. Seed: `init()`, one game-seed step, `init_low(lo')` (`rng.md` §5.2,
   "NPC-control seed").
3. Look up the item ids of `cqv` and `aqv` once (globals `0x00883E9C`,
   `0x00883EA0`; used by `vendors.md`).
4. For every `monstats` row in row order whose flag word has `interact`:
   take the next record; more than 64 → fatal assert. 1.14d: 47 records
   (live `monstats.txt` rows with `interact` = 1).
5. Fill the record from the NPC table (§1.2) when the class is listed;
   traders also get their store data (`vendors.md` §1). Unlisted classes
   (e.g. `drehyaiced` 527, `ancientstatue1`–`3` 537–539) keep act 0,
   trader 0.

Record lookup by class (`0x00535EA0` / `0x00535F10`): first of the 64
slots whose class matches; none → null (`0x00535EA0` asserts on class 0).

Record layout (1.14d offsets):

| Off | Field | Set by |
|---|---|---|
| +0x00 | monstats class | §1.1 |
| +0x04 | NPC inventory (store grid) | §1.1, `vendors.md` §4 |
| +0x08 | per-player gamble lists (node: inventory, player GUID, next) | `vendors.md` §5 |
| +0x0C | has gamble list (u32) | `vendors.md` §1 |
| +0x10 | hire list (0x450 bytes, §7.1) | §7 |
| +0x14 | NPC event list (§10) | `vendors.md` §3.4 |
| +0x18 | per-player vendor-chain list (node: player GUID, gamble-mode byte at +4, next) | `vendors.md` §4 |
| +0x1C | has traded (u32, set at trade open) | `vendors.md` §4 |
| +0x20 | store generated (u8) | `vendors.md` §3 |
| +0x21 | hire list made (u8) | §7.1 |
| +0x22 | act 0–4 (u8) | NPC table |
| +0x23 | trader (u8) | NPC table |
| +0x24, +0x25 | flags set for 9 traders (u8); no reader in 1.14d | `vendors.md` §1 |
| +0x26 | NPC table byte 6 (u8); no reader in 1.14d | NPC table |
| +0x27 | refresh pending (u8) | `vendors.md` §6 |
| +0x28 | store time (`GetTickCount`, u32) | `vendors.md` §6 |
| +0x2C / +0x30 | store item list / count | `vendors.md` §1 |
| +0x34 / +0x38 | permanent item codes / count | `vendors.md` §1 |
| +0x40 | NPC GUID of the last trade | `vendors.md` §4 |

#### 1.2 NPC table (`0x00731184`)

u32 count (43), then 8-byte entries from `0x00731188`: u32 monstats class,
u8 trader, u8 act (0–4), u8 flag (byte 6), u8 0. The full table with each
NPC's roles is `vendors.tsv` (columns `trader`, `act`,
`force_vendor` = byte 6). Cain's act rows: cain5 (265) act 0, cain2 (244) 1, cain3 (245)
2, cain4 (246) 3, cain6 (520) 4; cain1 (146) is listed with act 0, not a
trader.

### 2. Starting an interaction (C→S 0x13)

Handler `0x0054AA90`: size 9 else 3; unit type (u32 @1) > 5 → 2; then
`0x00548B00(GUID u32 @5, type)`. For type 1 (monster):

1. Monster missing, or unit distance (`0x00641530`, unit spec) > 50 →
   result 1, nothing else.
2. If the monster's `monstats` row has both `npc` and `interact`: clear
   its path (`0x00648730`), set its AI parameter 0 (AI control +0x14)
   := 40 (`0x0058EC00(npc, 1, 0x28)`, asm `0x00548D41`; the field and
   its readers: `monsters/ai.md` §3), cancel its AI-think events (type 2) and schedule one
   at frame + 1 (`tick.md` §5.2–5.4). Exact (`0x00548D4F`–`0x00548D70`):
   cancel `0x00540E60(game, NPC, type 2, 0)`; schedule
   `0x005417D0(game, NPC, type 2, game frame (+0xA8) + 1, args 0, 0)`. This happens for every distance
   ≤ 50. What the NPC AI then does with param 0 = 40: `monsters/ai.md`
   §9.9 (after "Interaction", "Effect of param 0 := 40").
3. Distance 9..50: result 0, no interaction. Distance 7..8: approach
   (`0x00548A50`), result 0:
   1. Run request to the NPC: `0x00580A70(no skill, mode 3, type 1,
      GUID, 0)` (`sim/pathing.md` §1.2; it clears player data +0x150
      and +0x154; a stamina-less run becomes a walk, §1.5). Its result
      is not read.
   2. Then, whether or not the mode started, the queued interaction
      (`0x00641F20` → `0x00460780`, player units with player data
      only): player data +0x150 := 1, +0x154 := −1 (0x13 passes flag
      0; a caller passing flag ≠ 0 stores −2), +0x158 := unit type,
      +0x15C := GUID.
   3. The run stops by the arrival check (`sim/pathing.md` §9.5 rule
      3) at unit distance ≤ the stop distance, which is 0 for a player
      path: the allocation writes +0x93 := 0 (`0x00649D00`) and its
      setter `0x00649070` (+0x93 := v − 1 for v in 1..19, else 0) has
      no caller on the player request path (callers: client code and
      monster AI / quest functions only). So the player stops at the
      first step where the unit distance is 0, short of the NPC's own
      sub-tile for any NPC size ≥ 2 (Akara: monstats2 `SizeX` 2).
   4. On the stop (step result 2 in `0x00580C20`): +0x150 ≠ 0 and
      +0x154 < 0 → neutral start (`0x0057F020`), then `0x00548B00`
      runs again with (player, +0x158, +0x15C, flag = (+0x154 = −2)):
      this 0x13 handling from rule 1, now normally at distance ≤ 6;
      then +0x150 := 0. This is the server-side "talk on arrival";
      the client sends no second 0x13.
4. Distance ≤ 6: if the player is free (`0x00535060` returns 0: no
   interact unit, no cursor item, player data +0x4C = 0): clear the
   player's path and start (`0x00573020` → `0x00572C10`).

Start (`0x00572C10`, through the void wrapper `0x00573020`, which
drops its result: the 0x13 result of rule 4 is 0 on every path, and
the results 1 below are internal only):

1. Requires: the player has no interact unit; NPC mode ≠ 0 (death) and ≠
   12 (dead); `0x00535060` ≠ 1; the NPC's class has the monstats
   `interact` flag (`0x00457490(class, 9)`, asm `0x00572C5D`–`0x00572C65`:
   bit 9 of the flag word, `data/fields.tsv` `interact`; clear → result 1);
   for cain1 (146) `0x00594610` false (Tristram Cain, `quests.md`). A
   player already in the NPC's list → result 1.
2. Prepend a node {player, state 0, next} to the NPC's interaction list
   (monster data +0x30 → interaction block; its first field is the list
   head). States: 0 talking, 1 chatting, 2 trading. The block pointer
   comes from `0x00535E80` (monster with data → data +0x30, else 0) and
   the start reads its first field before the "already in the list"
   test of rule 1, so a null block faults; every monster gets a block
   at init (`0x00572BA0`, `monsters/init.md` §5 step 2), so it is never
   null here.
3. `0x00576770`: hire list to the player if the NPC sells mercenaries
   (§7.2), with "first" = the list was empty.
4. Player interact unit := (type 1, NPC GUID) (`0x00554120`).
5. Text list: new list, quest event 0 for player and NPC, send S→C 0x27
   (40 bytes: 0x27, u8 1, u32 NPC GUID, 34 bytes of list built by
   `0x00661480`; contents `quests.md` §7.1), free it; send 0x29
   (`0x00544520`) and 0x28 (`0x0053D670`, type 1, NPC GUID; `quests.md`
   §1.5).

Recorded order in one frame: 0x27, 0x29, 0x28 (frames 746, 798, 1464,
1720, 1751, 3570).

Entry order of the 0x27 list (recorded 2026-10-09,
`items-vendor-akara-buy` frame 15, Akara): `(0, 64), (0, 11)` where the
quest dispatch adds 11, then 64, i.e. newest first. Settled from the
binary (REC-1401; 1.14d-confirmed `0x006612F0`, `0x00661480`, read
2026-10-10, PC 1 today):

- Text list: u16 count at +0x04, head pointer at +0x08. Node (file
  `Text.cpp` line 90): u16 string id at +0x00, u32 kind at +0x04, next
  at +0x08.
- Add `0x006612F0(list, u16 string, u32 kind)`: new node, next := the
  old head, head := node, count + 1. A **prepend**; no draw, no sort,
  no duplicate test.
- Writer `0x00661480(list, out)`: zero 34 bytes; byte 0 := the low byte
  of the count; walk from the **head** along next, entry k (from 0):
  byte 2 + 4k := low byte of kind, u16 at 4 + 4k := string id. Bytes 1
  and 3 + 4k stay 0. Entries 0..7 fit (the last u16 ends at byte 33); a
  ninth node is fatal (assert line 248), so 8 entries are written
  without error and the count byte is not clamped. A null list or out
  is fatal (lines 237, 238).

So the 0x27 list is always newest-added first, whichever records or
table rows added the entries; the dispatch order is `quests.md` §7.1.

**Call forms** (2026-10-09, static asm; used by `tools/poke.md` §4
rule 10). All run on the game thread and return with the stack popped
by the callee.

| Function | Registers | Stack from [ESP+4] | `ret` | EAX | Proof |
|---|---|---|---|---|---|
| handler `0x0054AA90` (every C→S handler, `sim/intents-events.md` §2.3) | ECX game, EDX player | message (id byte first), size | 8 | handler code 0..3 | `0x0054AA93` reads [EBP+0xC] = size, `0x0054AAA6` [EBP+8] = message; `0x0054AA98` keeps EDX; `ret 8` at `0x0054AACB` |
| interact `0x00548B00` | ECX player, EDX unit type (0..5, `ja` past 5 → 1) | GUID, flag (0x13 passes 0; ≠ 0 stores −2 as the queued action, rule 3.2), game | 0xC | 0 done or walking, 1 missing / distance > 50, 3 object mode ≥ 8 or object vanished | `0x0054AABE`–`0x0054AAC4` (push game, 0, GUID; ECX := player; EDX = type from @1); `0x00548B0A` EBX := ECX; `0x00548B12` jump table `0x00548E8C` on EDX; [EBP+0x10] = game at `0x00548B1C`; `ret 0xC` at `0x00548DDC` |
| start wrapper `0x00573020` | ECX game, EDX player | NPC unit, interaction block (rule 2 of the start: the u32 at monster data +0x30), 0 | 0xC | not set (the wrapper drops `0x00572C10`'s EAX) | call site `0x00548DC0`–`0x00548DCF` (push 0, push block from `0x00535E80`, push NPC; EDX := player, ECX := game) |
| start `0x00572C10` | EAX NPC unit, EBX player | game, interaction block, the wrapper's third word | 0xC | 0 (1 only for an NPC without `interact`) | `0x00573023`–`0x00573032` (wrapper moves stack → EAX, EBX := EDX, pushes ECX); `0x00572C1B` EDI := EAX; `ret 0xC` at `0x00572C78`, `0x00572D92` |

The type 1 case runs at `0x00548CE8`; NPC lookup is `0x00552F60` with
ECX = game, EDX = 1, [ESP+4] = GUID (`0x00548CE8`–`0x00548CF6`). The
distance in rule 3 is the value saved at `0x00548D0F`; the ≤ 6 branch
is `0x00548D78`, the free test `0x00548DAC`, the path stop
`0x00548DBB`. Calling the start wrapper directly skips rules 1–3 (no
distance test, the NPC's path and AI param 0 untouched), so the NPC
keeps its own AI and may walk off while the node exists.

### 3. Chat open and close (C→S 0x2F, 0x30)

Both handlers (`0x0054B930`, `0x0054B9F0`): size 9 else 3; the NPC is
the unit with GUID u32 @5 (bytes 1–4 are not read), looked up in the
**monster** list only (`0x00552F60` with EDX = 1, `0x0054B954`,
`0x0054BA12`): a GUID of another unit type is "missing"; missing → 1; not a
monster with an interaction list → 3; NPC in another act than the player
(`0x00548A80`) → 2.

- **0x2F**: player position within **10** subtiles of the NPC on both
  axes (`0x00548EF0`, threshold EBX := 0xA at `0x0054B9AD`; NPC x / y
  from `0x0045ADF0` / `0x0045AE20`, player from its own path; r2
  2026-10-09, earlier text said 50, which is the 0x38 range) else its
  result (1); then `0x00572E60`: the player's
  node in state 0 goes to state 1 and the heal hook runs (§5). Other
  states, or no node for the player: nothing, result 0.
- **0x30** (`0x00572F20`, no distance test): quest event dispatch
  `0x00543D50` (`quests.md` §4.2), then the player's node:
  - state ≥ 1: unlink and free it; if the player's interact type is 1,
    reset the interact unit (`0x00554190`: GUID −1, type 6, flag 0); if
    the list is now empty: drop the player's gamble list at this NPC
    (`0x00537190`, `vendors.md` §5.4);
  - state 0: unlink, reset as above, free; the gamble list is kept.

**Call forms** (2026-10-09, static asm). Handlers: ECX game, EDX player,
[ESP+4] message, [ESP+8] size, `ret 8` (`0x0054B934` size, `0x0054B94C`
message, `0x0054B939` / `0x0054B9F9` EDX kept, `ret 8` at `0x0054B9E4`,
`0x0054BA8C`); EAX 0 done, 1 NPC missing or (0x2F) out of range,
2 other act, 3 bad size / no interaction block. The inner functions
take ECX game, EDX player, [ESP+4] NPC unit, [ESP+8] interaction block
(the u32 at monster data +0x30, read at `0x0054B9CA` / `0x0054BA62`),
`ret 8`, no result: `0x00572E60` (state 0 → 1 at `0x00572E85`, then the
heal hook `0x00578E70(NPC)` with ECX / EDX unchanged) and `0x00572F20`
(quest dispatch `0x00543D50(NPC)` first at `0x00572F31`). Calling the
inner functions skips the act and (0x2F) distance tests and needs a
non-null block (a null block faults at `0x00572E66` / `0x00572F39`).
Neither reads anything the client sets: the S→C replies go to the
player's own client.

### 4. Menu actions (C→S 0x38)

Handler `0x0054BCA0`: size 13 else 3; unit check `0x00548F80`
(`intents-events.md` §2.4 rule 4) must return 0; then `0x00579D60(action
u32 @1, NPC GUID u32 @5, item GUID u32 @9)`. Nothing happens unless the
NPC exists, has `interact` and an interaction list. single := the list
has exactly 1 node. No 0x2A is sent by this handler.

| Action | NPCs (class) | Does |
|---|---|---|
| 1 trade | gheed 147, akara 148, charsi 154, drognan 177, fara 178, elzix 199, lysander 202, asheara 252, hratli 253, alkor 254, ormus 255, halbu 257, jamella 405, larzuk 511, drehya 512, malah 513 | node state 1 → 2 (`0x00572EA0`); trade open `0x00579430(npc, single, 0)` (`vendors.md` §4) |
| 2 gamble | gheed 147, elzix 199, alkor 254, jamella 405, drehya 512, nihlathak 514 | state 1 → 2; `0x00579430(npc, single, 1)` |
| 3 hire list | any (only merc sellers send, §7.2) | `0x00576770(npc, first = node count < 2)` |
| other values | charsi 154 | imbue (§8.1) |
| other | larzuk 511 | socket (§8.1) |
| other | drehya 512 | personalize (§8.1) |
| other | akara 148 | respec (§8.2) |
| other | warriv1 155, warriv2 175, meshif1 210, meshif2 264, tyrael2 367, cain6 520 | act travel (§8.3) |

Any other NPC / action pair: nothing. Nihlathak has no trade action
although a store is cached for him (`vendors.md` §1).

**Call forms** (2026-10-09, static asm). Handler `0x0054BCA0`: as §3
(ECX game, EDX player, [ESP+4] message, [ESP+8] size, `ret 8` at
`0x0054BCF9`). Its unit check `0x00548F80` takes EAX = NPC GUID, EDX =
unit type 1, ECX = game, EDI = player, [ESP+4] = range 0x32
(`0x0054BCCF`–`0x0054BCD8`), `ret 4`: 1 when no such monster, 2 when it
is in another act, else the per-axis test `0x00548EF0` with range 50
(0 within |dx|, |dy| ≤ 50, else 1). The handler returns that value when
non-zero, else the action's result. Action `0x00579D60`: ECX game, EDX
player, [ESP+4] action, [ESP+8] NPC GUID, [ESP+12] item GUID
(`0x0054BCE1`–`0x0054BCEF`; `0x00579D79` ESI := player, `0x00579D81`
EDI := game, NPC looked up as a monster at `0x00579D83`), `ret 0xC`
(`0x00579F8E`); EAX 0, or 3 when an item service's item check
`0x00578610` fails (`0x00579F7B`). Trade and gamble (actions 1, 2):
`0x00572EA0` (ECX NPC, EDX player, no stack) moves the player's node
1 → 2 only when it is in state 1 (`0x00572ECC`), but the trade open
`0x00579430` (ECX game, EDX player, stack NPC, single, gamble flag;
`0x00579E86`–`0x00579E94`) follows with no test of its result, so it
runs with the node in any state or with no node for the player (what
it needs: `vendors.md` §4). The client's order is 0x13, 0x2F, then
0x38 (node in state 1).

Client senders (confirmed 2026-10-07 in the 1.14d client code, relay
from `ui/menus.md` §2 rule 2): the NPC menu builder `0x004B4830` sends
C→S 0x38 action 3 (13 bytes, `0x004786D0`: 0x38, u32 EDX @1, u32 first
stack argument @5, u32 second @9) at `0x004B48E8` when the NPC class
(`[0x007C0D2D]`) is 252 (asheara), 198 (greiz), 515 (qual-kehk) or 150
(kashya) (compares `0x004B48AD`–`0x004B48C7`): action 3 @1, NPC GUID
(`[0x007C0D25]`) @5, the local player's GUID @9, or 0xFFFFFFFF when
there is no local player unit (`0x00463DD0` null, `0x004B48D2`). The
server ignores @9 for action 3 (`0x00576770` takes only the NPC and
"first"). The menu box itself is `ui/menus.md` §2. The hire request
(§7.3) is sent by `0x004B1E80` (`ui/menus.md` §3) through `0x004786A0`
(9 bytes: 0x36, u32 NPC GUID `[0x007C0C64]` @1, u32 @5): the merc name
id is a u16 from the list entry (`movzx` at `0x004B1E94`) widened to
u32, so bytes 7–8 are 0; the server reads only the u16 @5 (`movzx
esi, word ptr [eax + 5]` at `0x0054BBE6`).

### 5. Healing on chat open

`0x00578E70(npc)`: global `0x0088CAC4` := NPC GUID (−1 for none; read
only by dead code, §10). Healers: akara 148, atma 176, fara 178, ormus
255, jamella 405, malah 513 → `0x00578D30(player, npc)`, which does
nothing unless the player's interact unit is that NPC. Then, in order:

1. Life (stat 6) < max life (`0x00625D10`): set it to max and send the
   stat (`0x00548520` → SetStat 0x1D–0x1F, stat spec).
2. Same for mana (stat 8, max `0x00625D60`) and stamina (stat 10, max
   `0x00625DB0`). Comparisons are unsigned.
3. Remove the stat list of state 2 (poison) and of state 1 (freeze)
   when present.
4. `0x00578C20`: for each state id 0 … states count − 1 that the player
   has, whose curable mask is set (`0x0063A460`) and that has a stat
   list: remove it.
5. Pets (`0x00574DE0` with `0x00578CA0`, pet iteration order of the
   player spec): life to max (no message), curable states (step 4),
   poison, freeze. Exact (`0x00578CA0`): life only when total stat 6 <
   max life `0x00625D10` (a **signed** compare, unlike the player's),
   then base stat 6 := max (`0x00627260`) and "changed"; each of the
   other three counts as a change when it removes something.
6. If anything changed: sound 10 attached to the NPC (`0x00553380(npc, 10, 0)`: target none, every client; asm `0x00578E4F`,
   delivered by the unit spec).

Heal is free and happens on every 0x2F that moves the node from state 0
to 1, i.e. once per interaction.

Recorded (2026-10-09, `a2-npc-fara-heal`, 1.14d under Wine, life poked
to 20 of 50): the 0x2F's handling sends `1e 06 00 32` (step 1, SetStat
0x1E, stat 6 := 12800) at once, before the client's C→S 0x31 of the
same drain; the next tick's client pass sends the sound `2c 01 0a000000
0a00` (step 6, sound 10 on Fara, type 1 GUID 10); the 0x95 of the
vitals sync follows in that tick's flush (`combat/vitals.md` §5.1).
d2rs equal on every byte and phase (q-fix-npc-interact).

### 6. Cain identify (C→S 0x34)

Handler `0x0054BBA0` (size 5 else 3) → `0x00578460(npc GUID u32 @1)`:

1. NPC missing or not the player's interact unit → 0x2A code 9.
2. Class not cain2 244, cain3 245, cain4 246, cain5 265, cain6 520 →
   return, **no message** (cain1 146 included).
3. n = unidentified items (`0x0062A530`): items without flag 0x10 that
   are in a grid page with inventory page 0 (backpack) or 3 (cube), or
   equipped (node page 3). n = 0 → 0x2A code 9.
4. Unless quest slot 4 (Search for Cain) bit 0 or bit 1 is set: pay
   100·n (`vendors.md` §9.1); not enough → 0x2A code 12.
5. Identify (`0x00562590`, item spec) every item of step 3 without flag
   0x10, inventory order; stash (page 4) and belt are skipped.
6. One 0x2A code 3, flag 0, GUID −1.

Client caption (owner `ui/menus.md` §2.3 r3; the menu build `0x004B4830`):
the client counts n with the same `0x0062A530` (`0x004B4B86`) on its own
item model and tests the same two bits on its copy of the player's quest
record, `[0x007C0D43]` (written only by S→C 0x28, which arrives with the
dialog before the menu is built, `client/msg-ui.md` §16 r7):
`0x0065C310(record, 4, 0)` at `0x004B4B61`, then `(record, 4, 1)` at
`0x004B4B74`. Both clear → `NPCIdentify2` (4021, `0x004B4BA2`) + `100 ×
n` (`0x004B4BCB`); either set → 4020 `NPCIdentify1` as is (`0x004B4BFE`).
So the caption shows a cost exactly when step 4 charges one, and the
client needs no other quest state for it.

C→S 0x37 (identify the item just gambled) is `vendors.md` §5.5.

### 7. Mercenaries

Sellers: kashya 150 (Act I), greiz 198 (II), asheara 252 (III),
qual-kehk 515 (V). Resurrection also at tyrael2 367 (IV).

#### 7.1 Hire list (`0x00576070(record)`)

Runs when record +0x21 = 0: set +0x21 := 1; if no list yet (+0x10 = 0):

1. Allocate 0x450 bytes (69 slots of 16: u16 name id, u32 seed @4, u32
   hired @8, u32 offered @0xC), zeroed.
2. Hireling row = first `hireling` row with seller = NPC class,
   difficulty column = 1 (Normal, for every game difficulty) and version
   = 100 in expansion games, 0 otherwise (`0x00575FF0` → `0x006564D0`);
   none → fatal. first, last = its name ids (+0x114, +0x116; ids from
   `fixups.md` §7); n = last − first + 1.
3. Slot i (i = 0 … n−1): name = first + i; seed = lo' of one step of the
   NPC-control seed; hired = offered = 0.
4. 10 times: s = roll(NPC-control seed, n); from slot s, probe upward
   (wrapping to 0) for a slot neither offered nor hired; mark it offered.
   If the probe returns to s, stop at once (no more draws).

The list is made at the first of: a 0x13 start with a seller (§2 step
3), a 0x38 action 3, a trade open (`vendors.md` §4; only these four
classes), or a hire that empties the offer (§7.3 step 8).

#### 7.2 Sending the list (`0x00576770(npc, first)`)

Only for kashya, greiz, qual-kehk, asheara: S→C 0x4F (1 byte); make the
list if needed (§7.1); then for each slot 0 … n−1 that is offered and not
hired: S→C 0x4E (7 bytes: 0x4E, u16 name id, u32 slot seed). The client
derives each offer's stats and price from name and seed with the same
routine as §7.3 step 5.

#### 7.3 Hire (C→S 0x36)

Handler `0x0054BBD0` (size 9 else 3) → `0x00577FE0(npc GUID u32 @1,
name u16 @5)`; NPC missing or not the interact unit → 0x2A code 9. Then
`0x005770E0`:

1. lvl = player level, capped in Normal by the NPC's act (12, 20, 28,
   36, 45; `0x00576890`).
2. qual-kehk: quest slot 36 (Rescue on Mount Arreat) bit 0 clear → 0x2A
   code 11. kashya with lvl < 8: slot 2 (Sisters' Burial Grounds) bit 0
   clear → code 11. (D2MOO names the first check `QUEST_A5Q6_BAAL`, 36 of
   another enumeration; 1.14d reads quest slot 36.)
3. No record → code 9. Name outside first … last of the §7.1 row →
   code 9. Slot (name − first): name differs → 9; hired ≠ 0 → 9. An
   offered = 0 slot is accepted.
   Exact order (`0x005770E0`): step 1's cap reads the record through
   its own lookup (`0x00576890`, `vendors.md` §1 rule 6: no record → no
   cap), then step 2's gates, then the record lookup of step 3. The
   §7.1 row is looked up again here (`0x00575FF0`, Normal row of the
   NPC class and the game's version): none → first = last = 0 and
   hireling class 0 without an assert (only §7.1's caller asserts), so
   a name ≠ 0 answers code 9 and name 0 reads slot 0 of the record's
   hire list (a null list faults). The row's +8 (hireling monster
   class) is the class step 7 creates. All four sellers have records
   and Normal rows in 1.14d, so only the name tests are reachable.
4. act = `0x00663750(name)` (act of the hireling row whose name range
   holds it, minus 1).
5. Hire init `0x006637F0(seed = slot seed, act, difficulty)`; fails (no
   rows) → return without a message:
   - local seed := `init()`, `init_low(slot seed)`;
   - candidates: the first row with act + 1, difficulty + 1 and the
     game's version, then every later row with the same act, difficulty,
     version and the same `level` as that first row (`0x00656580`);
   - row = candidates[roll(local, count)];
   - one step of the local seed: L = (lo' mod 5) + player level − 5,
     at least 2 ("player level" = the player's total stat 12,
     `0x00625480`, read inside `0x006637F0` from EDX = the player:
     uncapped, not step 1's lvl; edge case 12);
   - price = gold · (100 + 15·(L − row level)) / 100 (signed), at least
     the row's `gold`. Other outputs (life, damage, skills…) belong to
     the mercenary spec.
6. Pay the price (`vendors.md` §9.1); not enough → 0x2A code 12.
7. Create the mercenary unit near the NPC, else near the player
   (`0x005B23C0(class, 1, 4, 0)` twice; spot and draw order:
   `hirelings.md` §3.1.1); fails → code
   15 (gold already taken).
8. Slot hired := 1; mercenary init `0x00573270`; resend the
   list (§7.2, first = node count < 2); 0x2A code 5, flag 0, GUID =
   mercenary; then `0x00577010`: if no slot is offered-and-not-hired,
   free the list, clear +0x21 and make a new list (§7.1 draws).

#### 7.4 Resurrect (C→S 0x62)

Handler `0x0054BC00`: expansion game and size 5, else 3; `0x00579C00
(npc GUID u32 @1)`:

1. NPC missing or not the interact unit → 0x2A code 9; class not kashya,
   greiz, asheara, tyrael2, qual-kehk → code 9.
2. Dead hireling of the player (`0x00574EC0(7, 1)`) missing → code 9.
   `(7, 1)` returns the first hireling node dead or alive; 1.14d does
   not test the dead bit (edge case 11). d2rs: a living node is
   answered like a missing one (code 9, nothing changed;
   `world/hirelings.md` edge case 5).
3. cost = min((L·L / 2)·15, 50000), L = mercenary level (stat 12),
   signed division, unsigned cap (`0x006637B0`). Pay → else code 12.
4. Clear unit flag 0x10000, mode 1, life := max, revive `0x00579AA0`
   (mercenary spec); S→C 0x9B (u16 0xFFFF @1, u32 0 @3); 0x2A code 5,
   flag 0, GUID = mercenary.

#### 7.5 Quest-granted mercenary (`0x00579180`)

Called by quest code (`0x00590980`, `0x005B9240`; `quests.md`). Needs
the NPC record and its hire list. Skipped when the player already has a
hireling: expansion `0x00574EC0(7, 1)` non-null (then only the refill
check runs), classic `0x00574EC0(7, 0)` non-null (nothing). Else: the
hireling row for the game's **difficulty** (unlike §7.1); the first slot
offered and not hired becomes hired; S→C 0x50 (`0x0053D7E0`; u16 2 @1,
u16 name @3; `quests.md` §6.2); mercenary created near the player
(spawn modes 4, then 6, then 12; all fail → stop, no refill check) and
initialised; then the §7.3 step 8 refill check.
Exact (`0x00579180`): no record or no hire list (+0x10 = 0) → nothing.
No `hireling` row for the game's version, the seller and the game's
difficulty → fatal assert (line 0xFA2). The walk covers slots 0 … n_d
− 1, n_d = last − first + 1 of that row (the list itself was built from
the Normal row, §7.1); the first slot with hired ≠ 1 and offered = 1 is
taken: hired := 1, then 0x50 with that slot's name, then the creation
(spawn modes 4, 6, 12; all fail → stop, the slot stays hired, no
refill check). No such slot → no 0x50, no unit, and the refill check
`0x00577010` runs.

### 8. NPC services (C→S 0x38, action ∉ {1, 2, 3})

#### 8.1 Imbue, socket, personalize

Common: the item GUID (u32 @9) must be the player's cursor item
(`0x00578610`), else return silently. Result S→C 0x58 (7 bytes via
`0x0053D8D0`: 0x58, u32 NPC GUID, u8 result @5, byte 6 not written):
6 done, 7 refused. "Refuse" below = 0x58 result 7 and the item is put
back (`0x00563C00`).

| NPC | Quest gate (slot.bit) | Item must be (predicate) |
|---|---|---|
| charsi 154 | 3.1 (Tools of the Trade, reward pending) | `0x0062C590`: not gold, no flag 0x1000, `bitfield1` bit 0, not a throwable unless unit flag bit 25, not a quest item except `leg`, no socketed items, not socketed (0x800), quality 1–3 |
| larzuk 511 | 35.1 (Siege on Harrogath) | `0x0062C770`: not gold, no 0x1000, quest only `leg`, not broken (0x100), no socketed items, not socketed, max sockets > 0, stat 194 = 0 |
| drehya 512 | 38.1 (Betrayal of Harrogath) | `0x0062C6A0`: not gold, no 0x1000, type ≠ 7, 5, 6 (ear, quivers), not broken, not personalized (0x1000000), no socketed items, `Nameable` set |

Gate bit clear or predicate false → refuse.

- **Imbue**: fill a drop request from the input (`0x00558270`); flags
  |= 0x20 and (ethereal input ? 4 : 2); item format := game +0x78; keep
  the personalized name; remove the input from the cursor (fail →
  refuse); quality 6 (rare), item level = player's base level (stat 12,
  at least 1, `0x00558200`) + 4 if > 5; create (`0x00558D90`, item
  spec); null → 0x58 result 7 (input lost). Else repair (`0x005761C0`,
  `vendors.md` §8.2; all three services pass no player, so the repair
  sends no 0x3E), `0x0055FE00`, inventory page 0, name restored,
  place in the inventory or drop at a free spot near the player; quest
  reward hook `0x00591790` (`quests.md`); result 6.
- **Socket**: duplicate the input into the player (`0x0055A2A0`, `vendors.md` §7.3) and
  remove the input from the cursor (`0x0055EEA0`); either fails →
  refuse. Order (`0x0057A319`–`0x0057A36B`): the duplicate first (null
  → refuse, the input untouched); then the removal; a failed removal
  refuses through the same path (0x58 result 7, `0x00563C00` on the
  input) and leaves the duplicate as `0x0055A2A0` made it: a unit in
  no room and no inventory, never freed (a leak; d2rs frees it, which
  is not observable). Flag 0x800; s = max sockets (`0x0062BC20`); quality 4: s :=
  roll(item seed of the duplicate, min(s, 2)) + 1 (`rng.md` §7); quality
  5–9: s := 1 if s > 0; other qualities keep s; add s sockets
  (`0x0062BCB0`); repair, `0x0055FE00`, page 0, place or drop; hook
  `0x005877C0`; result 6.
- **Personalize**: duplicate as above; a failed duplicate sends result
  7 and drops the cursor item (`0x00563C00`) but does not stop (edge
  case 6). Remove the input from the cursor (`0x0055EEA0`; fail →
  refuse); repair, page 0, place or drop;
  flag 0x1000000; name := player name; hook `0x0058BC00`; result 6.
  The failed-duplicate path is settled in edge case 13.

#### 8.2 Akara respec

Hell only (difficulty 2): if slot 1 bit 0 is set and slot 41 bits 1 and
0 are clear → `0x0058FD20` (sets 41.13, 41.1). Then any difficulty: if
slot 41 bit 1 is set → reset skills (`0x00570360`, `skills/levels.md`
§6.5) then stats (`0x00570C80`, `combat/vitals.md` §2.1), in that order
(`0x0057A242`, `0x0057A24B`); sound event 2 on the player
(`0x00553380(player, 2, player)`); `0x0058FD50` (41.0 set, 41.1
cleared; `quests-act1.md` §10.3). (Earlier text swapped the two
addresses.)

#### 8.3 Act travel

| NPC | Action | Condition | Calls |
|---|---|---|---|
| warriv1 155 | any ∉ 1–3 | slot 6 bit 0 | act change to level 40 (`0x0054B830(40, 0)`), `0x005467E0(npc, 40, 1)` (`quests.md` §8.1), activate level 40's waypoint (`0x00660E00`, `0x00660EC0`; `waypoints.md`) |
| warriv2 175 | any ∉ 1–3 | none | `0x0054B830(1, 5)` |
| meshif1 210 | 0 | slot 14 bit 0 | level 75 (`0x0054B830(75, 0)`), `0x005467E0(npc, 75, 40)`, waypoint 75 |
| meshif2 264 | 0 | none | `0x0054B830(40, 5)` |
| tyrael2 367 | 0 | expansion and slot 26 bit 0 | level 109 (`0x0054B830(109, 0)`), `0x005467E0(npc, 109, 103)`, waypoint 109 |
| cain6 520 | 0 | none | `0x0054B830(103, 5)` |

The three calls of a row run in the order listed (`0x0057A67A`,
`0x0057A688`, `0x0057A696`/`0x0057A6B4` for warriv1; the same for
meshif1 and tyrael2): the act change first, then act completion
(`quests.md` §8.1), then the waypoint. Act change `0x0054B830(game,
player, level, arg)`: level 0 → warp to the town of the player's act
(`0x0061AB70`) with arg; level in the player's act → a town-portal
object (class 59) at the player's (x − 5, y) leading to it
(`0x0056D130`); else the warp `0x0053AEC0(level, arg)`
(`waypoints.md` §7 rule 5, the act change §11; the argument is the spawn tile index). Every §8.3 destination is in another act,
so only the warp runs.

### 9. S→C 0x2A NPC transaction (15 bytes)

Builder `0x0053D740(code, gold, GUID, kind)`:

| Off | Size | Value |
|---|---|---|
| 0 | 1 | 0x2A |
| 1 | 1 | kind |
| 2 | 1 | result code |
| 3 | 4 | **not written**: stack contents of the builder's frame |
| 7 | 4 | GUID (item, mercenary) or −1 |
| 11 | 4 | player gold (stat 14) after the transaction |

| Code | Kind | Meaning (sender) |
|---|---|---|
| 0 | 4 | bought, GUID = new item (`vendors.md` §7) |
| 0 | 5 | bought into a stack or tome, GUID = that item |
| 1 | 3 | sold, GUID = the sold item |
| 2 | 1 | repaired (one or all) |
| 3 | 0 | identified (§6) |
| 5 | 0 | mercenary hired or resurrected, GUID = mercenary |
| 6 | 0 | healed (dead code, §10) |
| 7 | 0 | buy refused: item missing, not offered, cursor busy |
| 9 | 0 | refused: NPC not the interact unit, wrong NPC, invalid item or name |
| 10 | 0 | no room for the bought item |
| 11 | 0 | sell: item not the player's; hire: quest gate |
| 12 | 0 | not enough gold |
| 14 | 0 | nothing to heal (dead code) |
| 15 | 0 | mercenary could not be placed |

Recorded: `2a 03 01 05a4f619 07000000 f4010000` (sell, frame 1157),
`2a 04 00 056cf619 36000000 bc010000` (buy, frame 1279): bytes 3–6
differ between messages.

### 10. Dead code in 1.14d (no caller, no pointer reference)

- `0x00578ED0` purchase heal (D2MOO `D2GAME_NPC_PurchaseHeal`): cost =
  (player level · (missing life >> 8 + missing mana >> 8)) >> 2, at
  least 1 (`0x00622DE0`); codes 14, 12, 9, 6.
- `0x00579090` store regeneration of the last healer (global
  `0x0088CAC4`).
- `0x005368F0` / `0x005367B0` NPC event processing; `0x00579030` /
  `0x00576C90` random cache pick (draws from the NPC-control seed).
  Level-up (`0x00570880`) still pushes events (`0x00536850`,
  `vendors.md` §3.4); they are freed with the record data, never run.

## Constants & data dependencies

- NPC table `0x00731184` (§1.2, `vendors.tsv`); healer, Cain, seller,
  resurrector, trade, gamble and service class lists are code constants
  (§4–§8, `vendors.tsv` columns).
- Normal-difficulty level cap per NPC act: 12, 20, 28, 36, 45
  (`0x00576890`).
- Distances: 50 (start), 6 (talk), 7–8 (approach), 50 subtiles (0x2F).
- `hireling`: seller +0x14, difficulty +0x10 (1-based), act +0x0C
  (1-based), version +0x00, level +0x1C, gold +0x18, name ids
  +0x114/+0x116 (`fields.tsv`).
- Costs: identify 100 per item; resurrection min((L²/2)·15, 50000).

## Randomness

All draws use `rng.md` §3 semantics.

| When | Seed | Draws, in order |
|---|---|---|
| hire list (§7.1) | NPC-control | n steps (slot seeds), then up to 10 × roll(n) (fewer when the probe wraps) |
| hire / client offer (§7.3) | local, `{slot seed, 666}` | roll(candidate rows), one step (level) |
| socket (§8.1) | item seed of the duplicate | roll(min(max sockets, 2)) for quality 4 only |
| imbue (§8.1) | item spec | item creation |

Talk, chat, heal, identify, resurrect and act travel draw nothing.

## Edge cases & original bugs

Reproduced by default.

1. 0x2A bytes 3–6 are uninitialised stack (§9). Exact-match comparison
   (`intents-events.md` §6) must mask them; d2rs writes 0.
2. The hire list always uses the Normal `hireling` row's names (§7.1)
   while the price uses the game's difficulty rows (§7.3).
3. A hire slot that was never offered can be hired by a crafted 0x36
   (only "hired" is checked).
4. Gold is taken before the mercenary is placed; placement failure
   (code 15) keeps the gold.
5. Cain with no unidentified items answers code 9; non-Cain NPCs get no
   answer to 0x34 at all.
6. Personalize with a failed duplicate (`0x00579D60`: duplicate
   call `0x0057A533` returns null, removal call `0x0057A56E`): S→C 0x58 result 7,
   then the cursor drop `0x00563C00` (`items/inventory-moves.md` §9.1),
   then **no stop**: `0x0055EEA0(game, player, input)` removes the
   input only if it is still the player's cursor item (else returns 0
   and changes nothing).
   - Drop found a spot (the normal case): the input is on the ground,
     the cursor is empty, `0x0055EEA0` returns 0 → refuse path: a
     second S→C 0x58 result 7 and a second `0x00563C00`, which does
     nothing (no cursor item). Net: two 0x58 result 7, the item on the
     ground (0x9C action 2), no flag or name change. Reproduced.
   - Drop found no spot: the input is still on the cursor,
     `0x0055EEA0` removes and frees it, and 1.14d continues with the
     null duplicate (repair, placement with GUID −1, flag 0x1000000 and
     the name set on a null item); the outcome is undefined and is
     **not** reproduced. d2rs policy: reproduce up to and including
     the removal (one 0x58 result 7, the input removed from the cursor
     and freed as `0x0055EEA0` does: the input is lost), then stop: no
     further message, no unit created. Also edge case 13.
7. 0x38 trade / gamble does not require that the player is in the
   NPC's interaction list (state change is skipped if absent).
8. Healing triggers only on the 0 → 1 chat transition; a second 0x2F in
   the same interaction does not heal.
9. Nihlathak (514) owns a store but no trade action; he can still be
   sold to while gambling (`vendors.md` §7.2).
10. 0x58 byte 6 is not written (stack), like 0x2A bytes 3–6.
11. §7.4 step 2 uses `0x00574EC0(7, 1)`, which returns the first
    hireling node whether dead or alive: a crafted 0x62 with a living
    hireling is charged and reaches the revive (`world/hirelings.md`
    §9, edge case 5). Settled 2026-10-07 (`docs/handoff/gaps-night-specs.md`
    GN1): the revive (`0x00579AA0`) then frees that same unit and keeps
    using the freed record (`world/hirelings.md` §9 rule 3), so 1.14d's
    outcome is undefined and is **not** reproduced. d2rs refuses: when
    the node `(7, 1)` returns is living (bit 0 clear), S→C 0x2A code 9
    as in step 2, before the cost (no gold taken, no message other than
    the 0x2A, no unit, node or flag change). Both specs state this one
    policy.
12. The §7.3 step 1 cap (12, 20, 28, 36, 45) only feeds the Kashya
    `lvl < 8` gate; the offer level and price use the uncapped player
    level (`world/hirelings.md` §2).
13. Personalize with a failed duplicate (settles edge case 6 and open
    question 4; `0x00579D60` drehya branch, decompiled export and
    `0x0057A2DA`/`0x0057A32F`): duplicate `0x0055A2A0` null → 0x58
    result 7 and `0x00563C00` (drop the cursor item, `items/inventory-
    moves.md` §9.1). Then `0x0055EEA0(player, input)` compares the
    player's cursor item with the input:
    - the drop found a free spot (the usual case): the input lies on the
      ground (mode 3, 0x9C action 2), cursor = none ≠ input → 0 → the
      common refusal `0x0057A32F`: a **second** 0x58 result 7 and
      `0x00563C00` again (cursor empty: nothing). End: two 0x58 result 7,
      the item on the ground next to the player, not personalized;
    - no free spot: the item stays on the cursor, `0x0055EEA0` takes it
      off the cursor (returns 1) and the code continues with the null
      duplicate (repair `0x005761C0`, placement `0x00560200` with GUID
      −1, …): not followed further; outcome not defined by this spec.
    d2rs: reproduces the first case exactly; for the second, edge case 6
    (removal call `0x0057A56E` read: `0x0055EEA0` removes and frees the
    input, which is lost) gives the d2rs policy: one 0x58 result 7, the
    input removed and freed, then stop. Other PC 2 session read: "stops
    after the first 0x58 result 7 (item stays on the cursor)" — to
    reconcile (staging-6 merge).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| C→S `13 01000000 06000000` (Charsi, GUID 6, close) | S→C 0x27 `27 01 06000000 01000000 25 00…`, 0x29, 0x28 in that order, same frame | `20261006-015956-packets`, frame 746 |
| C→S `2f 01000000 06000000` after 0x13 | no S→C message (Charsi is not a healer) | same, frame 747 |
| C→S `38 01000000 06000000 00000000` | no 0x2A; 43 S→C 0x9C action 11 (store) | same, frame 898–899 |
| C→S `30 01000000 10000000` (Akara) | no message | same, frame 1958 |
| 0x2A sell, kind 3, code 1, GUID 7, gold 500 | `2a 03 01 ?? ?? ?? ?? 07 00 00 00 f4 01 00 00` (?? masked) | frame 1157 |
| hire list, NPC seed {12345, 666}, n = 41 | slot seeds 22752887, 2337785264, 1617882871, …; offered slots in order 24, 29, 19, 10, 8, 11, 27, 5, 22, 28; seed after {1296536796, 747986489} | synthetic (§7.1) |
| same seed, n = 3 | seeds 22752887, 2337785264, 1617882871; offered 1, 2, 0; 4th roll wraps → stop; seed after {4247383538, 1001282318} | synthetic |
| hire init, slot seed 22752887, 2 candidate rows, player level 10 | row 1, L = 6 | synthetic (§7.3 step 5) |
| price: row gold 100, row level 3, L = 5 / 7 / 9 | 130 / 160 / 190 | synthetic |
| resurrect L = 10 / 30 / 82 | 750 / 6750 / 50000 | synthetic (§7.4) |
| crafted 0x62 at Kashya, hireling living | 0x2A code 9; no gold taken, no 0x9B (d2rs policy, edge case 11) | synthetic (§7.4) |
| C→S `13 01000000 03000000` (Kashya, GUID 3), player level 8 | S→C 0x4F, ten 0x4E (name ids 0x0D56, 0x0D5A, 0x0D5B, 0x0D60, 0x0D66, 0x0D67, 0x0D68, 0x0D6F, 0x0D70, 0x0D76; u16 name, u32 slot seed), then 0x27, 0x29, 0x28, all in the input phase (§2 steps 3, 5) | `merc1-spawn-packets`, frame 969 |
| C→S `36 03000000 680d` (hire 0x0D68), gold 296 | S→C 0x81 (`world/hirelings.md` Test vectors), 0x27 (level speech), 0x4F + nine 0x4E (0x0D68 gone), 0x2A `2a 00 05 ?? ?? ?? ?? 0d000000 88000000` (code 5, GUID 13, 136 = gold left), all in the input phase; S→C 0x1D gold 136 next frame; price 160 | same, frames 1730–1731 (§7.3 steps 5–8; Kashya at level ≥ 8 needs no quest, step 2) |
| identify, 3 unidentified, slot 4 bits 0, 1 clear | pay 300; 0x2A code 3 | synthetic (§6) |
| client hire request for name 0x0D68 at Kashya (GUID 3) | C→S 9 bytes `36 03000000 680d0000` (u16 id widened to u32, §4 client senders) | `merc1-spawn-packets`, frame 1730 (`client_out`) |
| Warriv (class 155, GUID 7) at (4870, 4231), player running, at (4876, 4218) after its frame-287 step; Npc think, interaction step 7 (`monsters/ai-bodies.md` §9.9), d ≥ 5 → walk in radius of the player with (3, 2) | mode 2, path target (4871, **4228**); no unit-seed draw (seed unchanged across the think). Geometry: `monsters/ai.md` §7.2 row `0x005DE6D0` (the owner): k = 3, n = 6 + 13 = 19, (kx, ky) = (0, 2) → fix-up (1, 3). With the player read at its pre-step (4876, 4219) the same rule gives (1, 2) → 4229, so the think reads the player's current position | `town-ama-10k` frame 287 (`docs/handoff/q-tool-replay-diff.md` finding 6; d2rs gives 4229 via the old rounding geometry). This is not the town wander (`0x005DE200`, which draws 3–4 times) |

Game-file test (`#[ignore]`): with live `monstats.txt`, §1.1 yields 47
records in row order; the 43 table entries (`vendors.tsv`) attach.

## Provenance

- 1.14d code (read from `re/exports`): records `0x00536070`, lookups
  `0x00535EA0`/`0x00535F10`; 0x13 `0x0054AA90` → `0x00548B00` →
  `0x00572C10`; 0x2F/0x30 `0x0054B930`/`0x0054B9F0` → `0x00572E60`/
  `0x00572F20`; 0x38 `0x0054BCA0` → `0x00579D60`; heal `0x00578E70`,
  `0x00578D30`, `0x00578C20`, `0x00578CA0`; Cain `0x00578460`,
  `0x0062A530`; hire `0x00576070`, `0x00576770`, `0x00577FE0`,
  `0x005770E0`, `0x00577010`, `0x006637F0`, `0x00663750`, `0x00656580`,
  `0x006564D0`; resurrect `0x00579C00`, `0x006637B0`; quest merc
  `0x00579180`; 0x2A `0x0053D740` (stack layout read from the
  instructions); 0x4E `0x0053D7B0`; 0x9B `0x0053E0E0`. The NPC table and
  the switch targets were read from the `Game.exe` image (data at
  `0x00731184`, jump tables `0x00536454`/`0x00536488`).
- Approach (§2 rule 3): `0x00548A50` (asm: pushes skill 0, mode 3,
  type, GUID, 0 to `0x00580A70`, then `0x00641F20(player, action,
  type, GUID)`), `0x00460780` (writes player data +0x150..+0x15C), the
  arrival branch of `0x00580C20` (asm `0x00580D94`–`0x00580E88`); stop
  distance: every write of path +0x93 in `all.asm` (`0x00649D00`,
  `0x00649070`, `0x005893E0`, `0x005921B0`, client `0x00465070`) and
  every `call 0x649070` site (26: client `0x00466360`, `0x004AFF60`,
  `0x004C8750`; the rest in `0x005C07A0`–`0x005F5D50`, AI / quest
  code).
- Dead code: no call and no 32-bit pointer to `0x00578ED0`,
  `0x00579090`, `0x005368F0`, `0x005367B0` in `Game.exe`.
- D2MOO 1.10f `SUnitNpc.cpp` / `SUnitProxy.cpp` were used as a map
  (names, structure); every rule above was re-read in 1.14d. Differences:
  D2MOO sends the identify 0x2A per item inside the loop (1.14d once
  after it); D2MOO's trade-open class switch is inverted
  (`vendors.md` §4); D2MOO names the Qual-Kehk gate `QUEST_A5Q6_BAAL`;
  1.14d adds the Hell respec branch (§8.2) and 0x2A codes are confirmed
  by recording.
- Recording `20261006-015956-packets.jsonl`: GUID → class from S→C 0xAC
  (`ac 06000000 9a00` Charsi, `ac 10000000 9400` Akara, `ac 0b000000
  9b00` Warriv, `ac 26000000 0a01` Flavie).
- Third hirelings pass (2026-10-07, open questions 1, 2, 4, 5; §4
  client senders): disassembly of `0x00572C10` (`0x00572C5D`–
  `0x00572C65`), `0x00548B00` (`0x00548D38`–`0x00548D4A`),
  `0x0058EC00`, `0x00579C00` (`0x00579CF5`–`0x00579D54`), `0x0053E0E0`,
  `0x00579D60` (drehya branch), `0x00563C00`, `0x0055EEA0`, client
  `0x004B4830` (`0x004B48A8`–`0x004B48E8`), `0x004786D0`,
  `0x004B1E80`, `0x004786A0`, server `0x0054BBD0`; `data/fields.tsv`
  `monstats` flag bits; D2MOO `AiGeneral.h` (AI control +0x14 =
  `dwAiParam[0]`) as a map.
- Call forms (§2–§4, 2026-10-09, for `tools/poke.md` §4 rule 10): the
  register and stack use of `0x0054AA90`, `0x00548B00`, `0x00573020`,
  `0x00572C10`, `0x0054B930`, `0x0054B9F0`, `0x00572E60`, `0x00572F20`,
  `0x0054BCA0`, `0x00548F80`, `0x00548EF0`, `0x00579D60`,
  `0x00572EA0`, read from `all.asm` at the addresses cited in each
  table; the 0x2F range 10 at `0x0054B9AD` corrects §3 (was 50).

## Open questions

1. Answered: `0x00457490(class, bit)` tests bit `bit` of the monstats
   flag word (record 0x1A8 bytes, flags at +0x0C); the start check
   passes bit 9 = `interact` (asm `0x00572C60`: `mov ecx, [edi + 4]`;
   `mov edx, 9`; bit 15 is `killable`, `world/hirelings.md` §8 rule 1).
   §2 Start rule 1. So the start refuses (result 1) an NPC whose row
   lacks `interact`.
2. Answered: `0x0058EC00(unit, slot 1..3, v)` writes AI parameter
   slot−1 (AI control, monster data +0x28, +0x14 / +0x18 / +0x1C; other
   slots and non-monsters: nothing); the start passes slot 1 (EDX = 1,
   `0x00548D43`), v = 40 (`0x00548D41`–`0x00548D4A`). Its effect on each
   NPC AI is owned by `monsters/ai.md` §3 (field table, `dwAiParam[0]`)
   and §9.9. §2 rule 2.
3. Answered (scoped): no instruction reads record bytes +0x24, +0x25 or
   +0x26 in any of the 202 functions that reach an NPC record (callers
   of the lookups `0x00535EA0` / `0x00535F10`, every user of game
   +0x1D24, their callers and those functions' direct callees; `all.asm`
   scan of byte accesses at those offsets). Not proven for code that
   receives a record pointer deeper than that. The writer `0x00536070` stores them
   through a pointer at record +0x14 (`[esi+0x10]`, `[esi+0x11]`). d2rs
   stores them; they have no gameplay effect.
4. Answered: edge case 6 (`0x00579D60` duplicate `0x0057A533`, removal
   `0x0057A56E`; `0x0055EEA0` removes only the current cursor item) and
   edge case 13 (two 0x58 result 7, the item dropped next to the
   player).
5. ~~0x9B bytes on a resurrect (`9b ffff 00000000`).~~ **Answered**
   (bytes) 2026-10-07 from the binary: builder `0x0053E0E0` writes 7
   bytes: 0x9B, u16 DX @1, u32 stack argument @3; the resurrect call at
   `0x00579D0F` passes DX = 0xFFFF (`0x00579D08`) and 0 (`push 0` at
   `0x00579CF5`) to the player's client (`0x005531C0`): `9b ffff
   00000000`, after the revive `0x00579AA0` and before the 0x2A code 5.
   A resurrect recording would only confirm the order: recording list
   `docs/handoff/pc2-rec-npc-vendors.md` R-NV-1.
6. ~~Resurrect / heal / Cain / services message order.~~ Hire: answered
   by recording 2026-10-07 (Test vectors, frames 969–1731: 0x81, 0x27,
   list, 0x2A code 5 in the input phase, as §7.3 steps 7–8 and
   `world/hirelings.md` §3.2). The rest needs recording (2026-10-07; the
   binary fixes each order, a packets recording confirms it): R-NV-1
   (resurrect), R-NV-2 (heal, Cain identify), R-NV-3 (imbue, socket,
   personalize, respec). Expected: (a) resurrect at the seller with a
   dead hireling: the S→C messages of the C→S 0x62 input phase in the
   order of `world/hirelings.md` §9 rules 2–9 (0x81, the follow's 0xAC
   when the corpse is out of the client's rooms, `9b ffff 00000000`,
   0x2A code 5 with the merc GUID and the gold left) and the stats batch
   in the next tick; (b) heal: chat open (C→S 0x2F) at Akara with lost
   life / mana: the §5 messages in the 0x2F input phase, and none on a
   second 0x2F (edge case 8); (c) Cain identify (C→S 0x34) with 3
   unidentified items: one 0x2A code 3 after all items (§6), gold
   message; with none: 0x2A code 9; (d) services: imbue / socket /
   personalize (0x38 at Charsi / Larzuk / Anya with the quest bit): 0x58
   result 6, the item's 0x9C messages, quest messages; act travel
   (0x38 at Warriv): the act-change messages of §8.3. Each: message ids,
   phase (input vs tick) and order.
7. Answered: `sim/pathing.md` §9.2 rule 6 now states the queued
   interaction (`0x00460780` writes +0x150 := 1; arrival branch
   `0x00580D94`–`0x00580E88`), matching §2 rule 3.4. A confirming
   recording is R-NV-4.
