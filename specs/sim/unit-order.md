# Spec: Simulation — Unit ordering

- **Status:** conformance-passing: `cargo test -p conformance --test
  tick_replay` reproduces all 446 list snapshots of
  `traces/sim/tick/sim-0006..0008` in `d2-sim` with 0 mismatches
  (2026-10-06; the client list §7 has one client and no recorded joins,
  the adjacent-room arrays §9 are not compared). Rules read from the 1.14d
  `Game.exe` code; §2, §4, §5, §6 confirmed on the running game:
  `check_tick.py` reproduces all 197 list snapshots of
  `traces/raw/20261006-015554-tick.jsonl` (4,902 ticks, hand-played)
  from 238 hash inserts, 191 removals, 82 room activations, 69
  deactivations, 267 room inserts and 5,662 queue inserts, 0 mismatches.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::units::lists` (unit identity, game unit
  lists, room lists, update queues); `d2-sim::tick` iterates them
- **Related specs:** `sim/tick.md` (which step iterates which list, and
  the timer queue, whose order is owned there, §5); `sim/rng.md` (seeds
  derived per allocation: allocation order decides draws);
  `sim/intents-events.md` (messages whose order follows these lists);
  DRLG spec (Phase 3, not written: room creation, the adjacent-room
  array).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–61 |
| Inputs | 62–70 |
| Outputs / state changes | 71–75 |
| Rules | 76–77 |
|   1. Unit identity and GUIDs | 78–99 |
|   2. Game unit hash lists | 100–138 |
|   3. Unit placement and removal (list bookkeeping) | 139–150 |
|   4. Act room lists (active rooms) | 151–161 |
|   5. Room unit lists | 162–225 |
|   6. Room update queues | 226–245 |
|   7. Client list | 246–254 |
|   8. Unit timer lists | 255–262 |
|   9. Adjacent-room arrays (dependency) | 263–271 |
|   10. Iteration and modification | 272–288 |
| Constants & data dependencies | 289–299 |
| Randomness | 300–306 |
| Edge cases & original bugs | 307–317 |
| Test vectors | 318–351 |
| Provenance | 352–369 |
| Open questions | 370–414 |
<!-- /index -->

## Summary

Every outcome that loops over units depends on the order of the list it
loops over, and every RNG draw made inside such a loop inherits that
order. The server keeps these lists: per-game unit hash lists (5 unit
classes × 128 buckets, each bucket sorted by GUID, highest first), one
tile list, per-act active-room lists, per-room unit lists, per-room
update queues, the client list, and per-unit timer lists. All are
singly or doubly linked lists with fixed insertion rules: the hash
buckets are kept sorted; every other list inserts at the head, so it
iterates newest first. GUIDs come from one counter per unit type. No
server list is ever re-sorted. The timer queue, which decides the order
units act in during a tick, is owned by `tick.md` §5.

## Inputs

| Name | Type | Source |
|---|---|---|
| unit creation / removal | unit type, class, optional fixed GUID | the system creating the unit |
| room activation / deactivation | active room | DRLG streaming; `tick.md` §3 step 9 |
| room change | unit, new room | unit movement (path code) |
| client join / leave | client record | `intents-events.md` |

## Outputs / state changes

The lists below, and the GUID counters. Iteration order is the output
every consumer depends on.

## Rules

### 1. Unit identity and GUIDs

1. A unit is identified by (unit type, GUID). Types: 0 player, 1
   monster, 2 object, 3 missile, 4 item, 5 tile (warp). GUID at unit
   +0x0C, type at +0x00, class id at +0x04.
2. Counters: one u32 per type at game +0x90 + 4·type (6 counters), set to
   0 at game creation (`0x00530930`, writes +0x90…+0xA4).
3. Allocation `0x00552EE0(game, type)`: `next = counter[type] + 1`; if
   `next == 0xFFFFFFFF` then `next = 1`; `counter[type] = next`; return
   `next`. Type ≥ 6 is a fatal error. First GUID of each type is 1; 0
   and 0xFFFFFFFF are never handed out by the counter.
4. Called only from unit allocation `0x00555230`, once per unit, after
   the unit's seed is derived (`rng.md` §5.3) and before the type's init.
   Exception: a monster allocated with flag 2 takes the GUID passed by
   the caller and the counter does not move (D2MOO: restoring a unit
   with its old GUID).
5. Players also draw a GUID from counter 0; the unit seed derivation
   (`rng.md` §5.3) is skipped for type 0 at this point (the player load
   path sets it).
6. Counters never decrease and GUIDs are not reused while the counter
   has not wrapped (2^32 − 2 units of one type).

### 2. Game unit hash lists

Layout: game +0x1120 + `offset(type)` + 4·(GUID & 0x7F), offsets from
table `0x006E10E0`:

| Type | List index (D2MOO) | Offset | Address |
|---|---|---|---|
| player (0) | 0 | 0x000 | game +0x1120 |
| monster (1) | 1 | 0x200 | game +0x1320 |
| object (2) | 2 | 0x400 | game +0x1520 |
| item (4) | 3 | 0x600 | game +0x1720 |
| missile (3) | 4 | 0x800 | game +0x1920 |
| tile (5) | — | — | single list at game +0x1B20 |

128 buckets per type, cleared at game creation; next link at unit
+0xE4.

1. **Insert** (`0x00553060`, called only from `SUNIT_Add` `0x00554850`):
   walk the bucket from its head while the walked unit's GUID is
   **greater** than the new GUID; insert before the first unit whose GUID
   is ≤ the new one (equal GUIDs are a fatal error). A bucket is
   therefore always sorted by GUID, **descending**. The tile list uses
   the same rule.
2. **Remove** (`0x005530F0`, from `0x00555580`): unlink in place; order
   of the rest is unchanged.
3. **Lookup** (`0x00552F60(game, type, GUID)`): walk the bucket, first
   unit with that GUID.
4. **Iterate all units of a type**: buckets 0..127 in index order, each
   from its head. Since a bucket holds GUIDs ≡ b (mod 128) in descending
   order, a full iteration visits e.g. GUIDs 256, 128 (bucket 0), then
   257, 129, 1 (bucket 1), … — neither creation order nor GUID order.
5. Helpers: `0x005537D0(game, type, arg, fn)` for players (+0x1120) or
   monsters (+0x1320) only (other types: nothing), calls `fn(game, unit,
   arg)` for each unit **without state 7** (D2MOO `STATE_PLAYERBODY`) and
   stops at the first call returning 1 (returns that unit);
   `0x005538D0(game, fn, arg)` does the same over players without the
   early stop. Both read the next link after the call (a callback must not
   remove the unit it is given).

### 3. Unit placement and removal (list bookkeeping)

1. `SUNIT_Add` `0x00554850` (allocation with the "add" flag, unit load
   and restore paths): place the unit in its room (room list insert, §5),
   then hash insert (§2.1), then queue it for update (§6).
2. Unit removal `0x00555600` → `0x00555580`: room list unlink (§5) and
   update-queue unlink, then hash unlink (§2.2); the type's free routine
   then cancels all the unit's timers (`0x00540EE0`, from the player,
   monster, object, missile and item free routines; `tick.md` §5.4) and
   the unit is freed. Removal is immediate, also during the timer run
   (`tick.md` §5.5 consequence 3).

### 4. Act room lists (active rooms)

1. Head at act +0x10, next at room +0x7C. Acts are `game +0xBC + 4·act`
   (act 0..4).
2. **Activation** (`0x00619890`): the new active room is **prepended**;
   the act's pending-room flag (act +0x54) is set to 1 (`tick.md` §4).
   The room's seed is set here (`rng.md` §5.4).
3. **Deactivation** (`0x0061A910`): unlinked in place.
4. Iteration (every tick step that loops over rooms, `tick.md` §3):
   from the head, i.e. **newest-activated first**.

### 5. Room unit lists

1. Head at room +0x74, next at unit +0xE8 (D2MOO `pUnitFirst`,
   `pRoomNext`).
2. **Insert** (`0x0064C2C0`, also through `0x0064C350`): the unit is
   **prepended**. Then it is queued for update (§6). Players and good-
   aligned monsters also raise the room's allied count (`0x00619EE0`):
   type 0, or type 1 with alignment `0x006259B0` = 2 (`0x0064C321`–
   `0x0064C339`); missiles, objects, items and tiles never do.
3. **Remove** (`0x0064C370`): unlink in place (linear search); remove from
   the update queue (`0x0064C1B0`, clears unit flag 0x2000 at +0xC4);
   allied count lowered for players and good-aligned monsters.
4. **Room change**: path code `0x0064FAD0` removes the unit from the old
   room, prepends it to the new room (`0x0064C350`) and queues it for
   update. A unit that walks into a room is therefore at its head.
5. Iteration: from the head (newest arrival first). Server code never
   re-sorts a room list: the sort by Y (`0x0064C0C0`) is called only from
   `0x00619EA0`, whose only caller is client code (`0x004DDA32`, draw
   order).
6. **Client room lists.** The client's units live in the same lists of
   its own active rooms (client DRLG, `client/model.md` §12 rule 1),
   through the same insert (`0x0064C350`, prepend, rule 2) and remove
   (`0x0064C370`, rule 3); the client has no other link from a unit to
   a room. Client call sites (1.14d):

   | When | Insert / remove |
   |---|---|
   | player, monster, missile creation with a room: dynamic path set-up `0x00649D00` (from `0x00460BF0`, `0x004AE8D0` at `0x004AEA63`, `0x004CD0A0`) | insert into the creation room (`0x00649EA4`), only when the room is not none |
   | object creation `0x004BC720` | insert (`0x004BC7D4`) |
   | tile creation (type 5 in `0x00465FD0`) | insert (`0x00466152`) |
   | item mode set `0x004C1910` to mode 3 (ground) or 5 (dropping) | unit leaves its room first (`0x0064C450`), then insert (`0x004C19B4`, `0x004C197C`); other modes only leave |
   | movement and placement of a dynamic-path unit (0x15 teleport, path steps, `sim/pathing.md` §9.6 rules 8–9) | room recache `0x0064FAD0`: remove from the old room, insert into the new one when it is not none (rule 4) |
   | unit free `0x00465870` (`client/model.md` §2 rule 5) | remove, while the client act exists: per-kind frees `0x00460D50`, `0x004AED30`, `0x004BCA50`, `0x004CD170`; items `0x004C1A70` only when the unit is in the room's list (`0x0064C260`); tiles at `0x00465968` |
   | client room free `0x0061A840` (`drlg/rooms.md` §8.2 rule 4) | each unit leaves the room (`0x0064C450`) |

   The recache runs on every path step that changes the unit's cell, the
   client's walking server monsters included: a town NPC that walks
   across a room edge is in the new room's list (capture `gen-ui-hud`
   2026-10-10: Gheed 1:4 at sub-tile x 4841, tile 968, is drawn from
   the room of tiles 968–975; the room of tiles 960–967 fails the room
   test of `render/draw-order.md` §3 r1 and is not walked).
   A unit created at (0, 0) has no room and is in no list. 0x59 / 0xAC /
   0x51 create in the room of their point (`client/model.md` §2 rule 7),
   so a new unit is at the head of its room's list.
7. **Y sort** (`0x0064C0C0`, through `0x00619EA0(room)`, which then
   returns the head): in place, ascending by the unit's y
   (`0x006206B0`: static path +0x08 for types 2, 4, 5, else the dynamic
   path y, `client/model.md` §6 rule 6), signed compare; repeated
   adjacent-swap passes swapping only when the earlier unit's y is
   strictly greater, until a pass makes no swap. The result equals a
   **stable** sort by y: units of equal y keep their list order. The
   draw runs it on each room before walking its units
   (`render/draw-order.md` §3 rule 4; skipped when "skip units" is set,
   `0x004DDA27`), so the sorted order persists and later inserts
   prepend into it.
8. **Readers.** The only reader of a client room's list head besides
   rules 2, 3 and the room free is `0x00619EA0` (the head is reached
   only through `0x00619F60`, whose callers are `0x0064C0C0`,
   `0x0064C260`, `0x0064C2C0`, `0x0064C370`); so the client's list order
   is observable only through rule 7, where the pre-sort order decides
   ties of equal y. d2rs: the bridge keeps per active room an ordered
   list of `UnitKey`s with rules 2, 3, 6 and 7; units whose position is
   (0, 0) or whose room is none are not in any list.

### 6. Room update queues

1. Head at room +0x1C, next at unit +0xE0; membership flag unit +0xC4
   bit 0x2000 (D2MOO `UNITFLAG_ISLINKREFRESHMSG`).
2. **Queue** (`0x0064C040`, "refresh unit"; 56 callers: any change the
   clients must see): only if the unit has a room, the room's flag bit 2
   (room +0x34 & 4) is clear, and the unit is not queued yet; then the
   unit is **prepended**, the flag set, and the act's pending-update flag
   (act +0x00) set to 1 (`0x00619F90(room, 1)`).
3. A unit queued twice in a tick keeps its first position.
4. **Consumers**: each client's per-client update (`tick.md` §6, step 5)
   walks, for each room in the client room's adjacent-room array (§9),
   that room's queue from the head and sends the units' update messages
   (`0x0053A620` → `0x0053A5D0`); then step 6 (`tick.md` §3) walks every
   active room's queue (act list order) calling `0x00553220` on each
   unit and clears it (`0x0064C160`: unlinks every unit, clears flag
   0x2000).
5. So within one room, update messages go out **most recently queued
   first**.

### 7. Client list

1. Head at game +0x88, next at client +0x4A8.
2. A joining client is **prepended** (`0x00539A30`); leaving unlinks
   (`0x00539DA0`).
3. Iterated from the head (newest client first) by the client pass and
   the environment step (`tick.md` §3, §6), with the next link saved
   before each body.

### 8. Unit timer lists

Head at unit (`0x00553980` get, `0x00553970` set), links at timer
+0x24 / +0x28. New timers are prepended (`0x00540DF0`). Used only to
find and cancel a unit's own timers (`tick.md` §5.4); the order of
execution is the queue's, never this list's. The cancel helpers walk it
from the head, saving the next link first.

### 9. Adjacent-room arrays (dependency)

Each active room holds an array of adjacent active rooms (pointer at
room +0x00, count at room +0x24; includes the room itself). Client
updates (§6.4), and per D2MOO most nearby-unit searches (targets,
collision neighbours), walk rooms in this array's order, then each
room's unit list. The array is built by the DRLG when rooms are
activated; its order is owned by the DRLG spec (open question 2).

### 10. Iteration and modification

| List | Body may remove the current unit? | How 1.14d iterates |
|---|---|---|
| hash bucket (`0x005537D0`, `0x005538D0`) | no (next read after the call) | head → next |
| act room list, tick steps 3, 6, 7 | not expected | head → next (read after the body) |
| act room list, tick step 9 | yes | next saved before the body |
| room unit list, tick step 9 | yes | next saved before the body |
| room update queue, client update (`0x0053A5D0`) | yes | next saved before the body |
| client list | yes | next saved before the body |
| timer lists | yes | cursor (`tick.md` §5.5) |

A unit added to a head-insert list during an iteration of that list is
not visited by that iteration (it lands behind the walker). A unit added
to a hash bucket during an iteration is visited only if its bucket comes
later and its GUID sorts after the walker's position.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| hash buckets per class | 128 (`GUID & 0x7F`) | §2 |
| hash list offsets | table `0x006E10E0`: 0x1120, 0x1320, 0x1520, 0x1920, 0x1720 (+ tile 0x1B20) | §2 |
| GUID wrap | 0xFFFFFFFF → 1 | §1.3 |
| excluded state | 7 (player body) | §2.5 |

No `.txt` data.

## Randomness

None drawn here. Allocation order decides each unit's seed (`rng.md`
§5.3: one game-seed step per non-player unit, a second for items, in
allocation order) and its GUID; list order decides the order of draws
made while iterating.

## Edge cases & original bugs

1. Hash iteration order (§2.4) mixes GUIDs; reproduce it exactly where a
   system iterates a hash list.
2. GUID wrap (§1.3) can produce duplicates with live units after 2^32 − 2
   allocations of one type; the insert then hits the fatal error. Not
   reachable in normal play.
3. `0x005537D0` returns nothing for types other than player and monster.
4. The update queue silently ignores units in rooms with flag bit 2 and
   units without a room (§6.2).

## Test vectors

Synthetic (from the rules; CI-safe):

| Input | Expected | Source |
|---|---|---|
| fresh game; allocate monster, monster, item, monster | GUIDs 1, 2, 1 (item), 3 | §1.3 |
| monster counter 0xFFFFFFFE; allocate | GUID 1 | §1.3 |
| insert monster GUIDs 1, 129, 257 into bucket 1 (any order) | bucket 1 = [257, 129, 1] | §2.1 |
| players GUIDs 1, 2, 128, 129; iterate | 128, 129, 1, 2 (buckets 0, 1, 1, 2: 128; 129, 1; 2) | §2.4 |
| room: add A, B, C | room list [C, B, A] | §5.2 |
| room [C, B, A]; B walks out and back in | [B, C, A] | §5.4 |
| activate rooms R1, R2, R3 in act 0 | act list [R3, R2, R1] | §4.2 |
| queue A, B, A in one tick | update queue [B, A] | §6.2–6.3 |
| clients join X then Y | client list [Y, X] | §7.2 |

Recorded (`record_tick.py`, 2026-10-06; see `tick.md` Test vectors):
`20261006-015554-tick.jsonl`, 197 snapshots, all equal to the model;
`check_tick.py --perturb-snap N` is reported at the changed snapshot.
GUIDs in the same recording: the player is 1; every GUID above a
type's previous maximum is exactly maximum + 1 (204 cases, §1.3); the
34 other insertions reuse GUIDs of units removed earlier (restored
units keep their GUID, §1.4). The client list (§7) and the
adjacent-room arrays (§9) are snapshotted but not yet checked.

Comparison (exact): at chosen ticks, every list of
§2, §4, §5, §6, §7 written as the sequence of (unit type, GUID) (rooms by
activation sequence number) equals the same lists in `d2-sim` after the
same inputs. Until `d2-sim` has units, `tools/trace-recorder/
check_tick.py` replays every recorded insert and removal through the
rules above and must reproduce each recorded snapshot exactly; the same
replay runs on the committed traces `traces/sim/tick/sim-0006`–`0008`
(`tick.md`, Test vectors, "Trace sim/tick").

## Provenance

- **1.14d `Game.exe`**: all addresses read from the disassembly
  (capstone over `re/exports/functions.tsv`) with the Ghidra decompile
  as a guide; table `0x006E10E0` read from the file image; callers found
  by scanning `.text` for rel32 calls (`0x00553060` has one caller,
  `0x00552EE0` one, `0x0064C0C0` one).
- **D2MOO** (1.10f) `D2Game/src/UNIT/SUnit.cpp` (`SUNIT_AllocUnitData`,
  `SUNIT_GetServerUnit`, `SUNIT_RemoveUnit`, `SUNIT_IterateUnitsOfType`),
  `D2Common/src/Units/UnitRoom.cpp`, `include/GAME/Game.h`
  (`GAME_RemapUnitTypeToListIndex`, `pUnitList[5][128]` at +0x1120):
  same structures and rules; every rule was re-read in 1.14d. D2MOO
  documents the remap (missile 4, item 3) the same way; 1.14d's room
  offsets differ from D2MOO's names only by field placement given above.
- **Recorded**: hooks on the list primitives (`record_tick.py`; client-
  side callers of the shared room code filtered by the server-unit flag
  and the act); snapshots read the lists directly from game memory.

## Open questions

1. Client list (§7) with more than one client: needs a hosted game.
   Out of Phase 0–6 scope (multiplayer, Phase 7).
2. Adjacent-room array order (§9): owned by the DRLG spec; record it in
   the same trace (room +0x00 / +0x24) to fix it (the snapshots already hold the arrays).
   *Answered* in the owner: `drlg/rooms.md` §6 (fill `0x0066BD00` =
   rooms-near order restricted to active rooms, refilled at each
   neighbouring activation; removal `0x0061A910` swaps the last entry
   into the hole). Its trace confirmation is `drlg/rooms.md` OQ 3.
3. ~~Inactive-unit storage (compress `0x005433F0`, restore `0x00542B40`):
   order in which restored units re-enter the room list and whether they
   keep their GUIDs (observed: GUIDs are reused after removal, §1.4).
   Room-lifecycle spec. Which units are freed, stored or kept by the
   compress: answered in `sim/units.md` §3.3; the restore stays open
   there (OQ8).~~ → PC 2 recording list.
4. Which systems iterate hash lists rather than rooms (inventory of
   `0x005537D0` / `0x005538D0` callers by system), for the unit specs.
   *Answered* (static, 1.14d `disasm.py xref`, call sites mapped to
   `functions.tsv` and to the specs citing each function):
   - `0x005537D0` (search with early stop): 180 call sites, all in quest
     code (`0x00544300`–`0x005BD390`: 83 functions plus 29 sites in
     code Ghidra left without a function, all inside the quest ranges;
     owners `world/quests*.md`, a few shared with `items/treasure.md`,
     `monsters/ai-bodies-*.md`, `world/npc.md`). Type argument: 0
     (players) at every site read except `0x00589790` (type 1,
     monsters, in `0x00589580`, act 5) and `0x00596D33` (register).
   - `0x005538D0` (players, no stop): 34 sites in 28 functions:
     game / client code `0x00535730`–`0x0053DF80` (13 sites: client
     add / remove notices, `world/objects.md`, `world/vendors.md`,
     `sim/tick.md`), inventory and cube (`0x00557FD0`, `0x00558B90`, 2
     unattached sites; `items/inventory.md`), intents (`0x0055B620`,
     `0x0055B790`), `0x005678F0`, vitals (`0x00570880`), pets and
     hirelings (`0x00574450`–`0x00575E90`, 8 functions;
     `sim/pets.md`, `world/hirelings.md`), and quest / AI helpers
     `0x0059DF50`, `0x005A5A30`, `0x005A5FF0`, `0x005B43F0`.
   Tick step 11 reaches `0x005538D0` through `0x00558B90`
   (`sim/tick.md` §3). Inline walks of the buckets (+0x1120 + 4·i without a helper) are not in
   this inventory.
5. *Answered:* client room unit lists (`impl-client-drlg` §3 Q6): §5
   rules 6–8 (same list code on the client DRLG's rooms, the client
   call sites, the draw's stable Y sort). Open inside it: a client
   recording that dumps one room's list (active room +0x74, unit
   +0xE8) before and after a drawn frame confirms rule 7's tie order.
