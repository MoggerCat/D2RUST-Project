# Spec: Simulation — Unit ordering

- **Status:** draft (implemented in `d2-sim`; synthetic vectors pass, trace replay pending); rules read from the 1.14d
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
| Summary | 44–57 |
| Inputs | 58–66 |
| Outputs / state changes | 67–71 |
| Rules | 72–73 |
|   1. Unit identity and GUIDs | 74–95 |
|   2. Game unit hash lists | 96–134 |
|   3. Unit placement and removal (list bookkeeping) | 135–146 |
|   4. Act room lists (active rooms) | 147–157 |
|   5. Room unit lists | 158–175 |
|   6. Room update queues | 176–195 |
|   7. Client list | 196–204 |
|   8. Unit timer lists | 205–212 |
|   9. Adjacent-room arrays (dependency) | 213–221 |
|   10. Iteration and modification | 222–238 |
| Constants & data dependencies | 239–249 |
| Randomness | 250–256 |
| Edge cases & original bugs | 257–267 |
| Test vectors | 268–299 |
| Provenance | 300–317 |
| Open questions | 318–329 |
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
   aligned monsters also raise the room's allied count (`0x00619EE0`).
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
rules above and must reproduce each recorded snapshot exactly.

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
2. Adjacent-room array order (§9): owned by the DRLG spec; record it in
   the same trace (room +0x00 / +0x24) to fix it (the snapshots already hold the arrays).
3. Inactive-unit storage (compress `0x005433F0`, restore `0x00542B40`):
   order in which restored units re-enter the room list and whether they
   keep their GUIDs (observed: GUIDs are reused after removal, §1.4).
   Room-lifecycle spec.
4. Which systems iterate hash lists rather than rooms (inventory of
   `0x005537D0` / `0x005538D0` callers by system), for the unit specs.
