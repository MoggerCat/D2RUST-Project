# Spec: DRLG — Rooms: creation, rooms-near order, activation, active rooms

- **Status:** draft (no `d2-sim` code yet); every rule read from the 1.14d
  `Game.exe` code. Confirmed on recordings: room seeds and the
  activation-time seed draws (`20261005-232125-rng.jsonl`, 47 rooms of the
  town and 428 DRLG rooms); adjacent-room arrays of 4,975 room snapshots
  (`20261006-015554-tick.jsonl`, `20261006-022304-tick.jsonl`) contain
  their own room, only active rooms of the act, no duplicates, and are
  symmetric; the first activation burst follows the rooms-near order
  (§6). The full order check is queued (Test vectors).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::room` (DRLG rooms, rooms-near,
  status propagation), `d2-sim::drlg::active` (active rooms, adjacency
  arrays, deactivation)
- **Related specs:** `drlg/levels.md` (levels, level seeds, warps,
  coordinate lookups, level freeing); `drlg/preset.md`, `drlg/maze.md`,
  `drlg/outdoor.md` (which rooms a level has, their rects and flags);
  `sim/tick.md` §3 steps 3, 9, 10 and §4 (when rooms are populated,
  deactivated, levels freed); `sim/unit-order.md` §4–§6, §9 (act room
  list, room unit lists; §9 points here for the adjacency order);
  `sim/rng.md` §5.4.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–62 |
| Inputs | 63–71 |
| Outputs / state changes | 72–77 |
| Rules | 78–79 |
|   1. Structures (1.14d layout, for recorders and checks) | 80–128 |
|   2. DRLG room creation and seeds (`0x0066B3E0`) | 129–161 |
|   3. Rooms-near arrays (`0x0066C370`) | 162–205 |
|   4. Status and activation | 206–395 |
|   5. Active room creation (`0x006422A0`, `0x00619890`) | 396–429 |
|   6. Adjacency array order (owner of `unit-order.md` §9) | 430–445 |
|   7. Room clients and the inactivity counter | 446–466 |
|   8. Deactivation (tick step 9) | 467–530 |
|   9. Room tile grid | 531–1105 |
|   10. Collision map from tiles | 1106–1184 |
| Constants & data dependencies | 1185–1199 |
| Randomness | 1200–1217 |
| Edge cases & original bugs | 1218–1235 |
| Test vectors | 1236–1283 |
| Provenance | 1284–1327 |
| Open questions | 1328–1435 |
<!-- /index -->

## Summary

A level consists of DRLG rooms ("RoomEx"): rectangles in tile
coordinates with a seed, flags, a status and a list of nearby rooms. A
DRLG room becomes an **active room** (the server structure units live in)
when a client comes near it: when a client enters a room, the room and
every room in its rooms-near list get status "in sight" and are built
(tiles, collision, active room); rooms two and three steps away get
weaker statuses that only load data. An active room's adjacency array is
its rooms-near list filtered to active rooms, in the rooms-near order: a
bubble sort by position that this spec defines exactly. An active room
whose client count stays 0 for more than 10 deactivation passes, and that
no client still sees, is removed; its DRLG room survives, remembers
whether it was populated, and can be activated again with the same seeds.

## Inputs

| Name | Type | Source |
|---|---|---|
| level rooms | DRLG rooms with rect, type, flags | the level type specs |
| client room changes | old room, new room | player movement and level change (`0x00537B50`) |
| client join / leave | client | `0x00539DA0` (leave) |
| tick | frame | `sim/tick.md` steps 9, 10 |

## Outputs / state changes

DRLG room seeds and statuses; rooms-near arrays; active rooms (act room
list, `unit-order.md` §4) with adjacency arrays, client arrays,
inactivity counters; removal of active rooms.

## Rules

### 1. Structures (1.14d layout, for recorders and checks)

DRLG room (0xEC bytes):

| Offset | Field |
|---|---|
| +0x00 | link list ("orths"; node 0x18 bytes: +0x00 target room, or the level for a cross-level link, +0x04 direction 0..3, +0x08 extra, +0x0C init flag, +0x10 target box (x, y, w, h), +0x14 next) |
| +0x04 | `dwInitSeed` (u32) |
| +0x08, +0x2C | rooms-near array pointer, count |
| +0x0C | status reference counts, u16 × 4 (statuses 0..3) |
| +0x14, +0x18 | room seed (lo, hi) |
| +0x1C, +0xE8 | status list next, previous |
| +0x24 | next room of the level |
| +0x28 | flags (table below) |
| +0x30 | active room (0 = none) |
| +0x34, +0x38, +0x3C, +0x40 | tile x, y, width, height |
| +0x44 | status (u8, 0..4) |
| +0x48 | room type: 1 outdoor-grid room (used by outdoor and maze-style code; D2MOO passes `DRLGTYPE_MAZE`), 2 preset (DS1) room |
| +0x4C | warp links (§3.3) |
| +0x54 | tile grid (`drlg/rooms.md` §9) |
| +0x58 | level |
| +0x5C | preset units |
| +0x60 | other flags (bit 0: was populated) |
| +0x64 | logical-room info: coordinate lists (`drlg/levels.md` §11) |

The link list (+0x00) is written only by maze generation (every caller
of `0x0066B5E0` lies in the maze code `0x00670D47`–`0x00673ACE`;
`0x0066B790` adds cross-level records from the maze and from the outdoor
placer, whose list is the level's outdoor list) and read by the maze code
during generation and by the room free (§2.1, `0x0066B610`, `0x0066B530`);
D2MOO 1.10f has no other reader of `pDrlgOrth`. After generation nothing
known reads a built room's links (the rooms-near arrays of §3 use the
gap rule, warp links are +0x4C): keeping or dropping them is not
observable, except that §2.1 then has nothing to remove.

Room flags (+0x28; D2MOO names): 0x10 << i warp toward vis slot i (i =
0..7); 0x1000–0x8000 sub-shrine rows; 0x10000 waypoint; 0x20000 small
waypoint; 0x40000 automap reveal (client copy); 0x80000 no LOS draw;
0x100000 has tiles/active room built (`HAS_ROOM`); 0x400000 portal
(blocks removal); 0x800000 no population (next to a town); 0x1000000 tile
library loaded; 0x2000000 preset units added.

Active room (0x80 bytes): +0x00 adjacency array, +0x24 its count; +0x04
client array capacity, +0x48 client array, +0x78 client count; +0x08 tile
data; +0x0C inactivity counter; +0x10 DRLG room; +0x2C act; +0x34 flags
(bit 0 populated, bit 1 units active, bit 2 no update, `unit-order.md`);
+0x4C subtile x, y, w, h then tile x, y, w, h; +0x6C active-room seed;
+0x74 first unit; +0x7C next in act list.

### 2. DRLG room creation and seeds (`0x0066B3E0`)

Called by the level type specs, once per room, in their generation
order:

1. Allocate 0xEC bytes, zeroed; type, level; status := 4 (none).
2. **Draw (level seed):** one step; room seed := `init_low(lo')`.
3. **Draw (room seed):** one step; `dwInitSeed` := `lo'`.
4. If the level has flag 0x10 (client copy): room flag 0x40000.
5. Type data: type 1 `0x0067D6D0` (outdoor-grid room data, D2MOO
   `DRLGOUTROOM_AllocDrlgOutdoorRoom`), type 2 `0x00666630` (preset room
   data); other types none. The caller sets rect and flags and
   adds the room to the level's list (each type spec states the order).

So room k of a level (k-th creation) has a seed fixed by the level seed
and k. Rooms are never re-seeded from the level later: activation resets
the room seed to `init_low(dwInitSeed)` (§4.4).

#### 2.1 Room free (`0x0066C100`)

No draws. In order: free the warp links (`0x0066B4F0`); free the
rooms-near array (+0x08, count +0x2C := 0); free the type data (type 1
`0x0067D610`, type 2 `0x006665B0`); free each preset unit (+0x5C); for
each link of the room's list (+0x00, list order) with init flag 1 and
target N: remove from this room's list, then from N's list, the first
init-1 link whose target is the other room (`0x0066B610`); free the
remaining links (`0x0066B530`); unlink the room from its level's list
(level +0x10, next +0x24) and decrement the level's room count (+0x08)
when it was found; free the tile grid (`0x0066F0B0`) and the logical
rooms (`drlg/levels.md` §11); free the room. Links without the init
flag (cross-level links, `drlg/maze.md` §7.1) are dropped from this
room only.

### 3. Rooms-near arrays (`0x0066C370`)

Built once per DRLG room, the first time it is needed (status
propagation, §4; or an active room's creation), and rebuilt after it was
freed (`drlg/levels.md` §9.3):

1. **Candidates** (`0x0066BC20`): every room R of the same level, in level
   list order (including the room itself), with
   `gap_x < 6` and `gap_y < 6`, where for the room A being built
   `gap_x = R.x − A.w − A.x` if `A.x < R.x`, else `A.x − R.w − R.x`
   (same for y with heights). Signed; overlapping or touching rooms have
   gaps ≤ 0, the room itself has negative gaps. Collected in a 30-slot
   array without a bound check.
2. **Sort** (`0x0066BBC0`): bubble sort with n − 1 passes; each pass
   compares positions i = 0..n−2 in order and swaps `a[i]`, `a[i+1]` when
   `a[i].x ≥ a[i+1].x + a[i+1].w` **or** `a[i].y ≥ a[i+1].y + a[i+1].h`
   (signed). The comparison is not a total order: the result depends on
   the input order, so implement exactly this loop.
3. **Warp links** (`0x0066C220`, `0x0066BE80`): if the room has warp flags,
   for each slot i = 0..7 whose flag `0x10 << i` is set, except in level
   133: let V = vis[i] of this level, L := get-or-allocate V, W := warp
   id of slot i (`drlg/levels.md` §7); generate L if it has no rooms.
   Let c = number of slots j < i with vis[j] = V. If W ≠ −1: walk L's
   vis array; at the (c+1)-th slot m with vis[m] = this level, try the
   link (below) once; if it links, done. If not linked (or W = −1), try
   each slot m of L with vis[m] = this level, in order, until one links.
   **Link attempt** for slot m: walk L's rooms (list order) for the first
   room T with flag `0x10 << m`:
   - W ≠ −1: append T to this room's rooms-near array, re-sort the whole
     array (rule 2), prepend a warp-link record {T, enabled 1, lvlwarp
     record of slot i with direction `'b'`} to the room's links (+0x4C);
     linked.
   - W = −1: append and re-sort only if T is within the gap rule of
     rule 1 (both gaps < 6), then continue with the next flagged room of
     L; linked if any was appended.
   Generating L draws from L's level seed (`drlg/levels.md` §5); these
   draws happen inside the near-array build, i.e. inside activation.
4. **Town border** (`0x0066BD50`): if this room's level is not a town and
   any room in its rooms-near array is in a town level, set flag
   0x800000 (no population).

The array mixes levels (warp links) and is sorted by the global tile
coordinates of all levels of the act.

### 4. Status and activation

Statuses: 0 client in room, 1 client in sight, 2 client out of sight,
3 tiles loaded ("untile"), 4 none. A room's status is the lowest status
whose reference count is non-zero (4 if none). The DRLG keeps one
circular doubly-linked list per status 0..3 (heads at drlg +0xA0 +
0xEC·s, `0x0061B7E0`, which also writes s into each head node's status
byte +0x44, §4.6 rule 9); **force status s** (`0x0061B210`): if s differs
from the room's status: unlink the room (if linked), and for s < 4 link
it at the **tail** of list s; status := s.

**Set handlers** (table `0x00744384`, called with the room):

| s | 1.14d | Effect |
|---|---|---|
| 0 | `0x0061B2C0` | force status 0 (unconditional call; force is a no-op when already 0) |
| 1 | `0x0061B2D0` | **build** (§4.4) if the room has no active room and no flag 0x100000; then force status 1 if status > 1 |
| 2 | `0x0061BB10` | if flag 0x1000000 (tiles loaded) and (type +0x48 ≠ 2 or flag 0x2000000, preset units added) and status > 2: force 2, then if count[1] (+0x0E) ≠ 0 run set handler 1 (tail call `0x0061B2D0`); otherwise nothing |
| 3 | `0x0061B320` | load the DT1 files if flag 0x1000000 is clear (`0x0066F240`); if type 2 and flag 0x2000000 clear, add the preset units (`0x00667890`, `drlg/preset.md`); force 3 if status > 3 |

**Unset handlers** (table `0x00744394`): statuses 0, 1 (`0x0061B530`,
`0x0061B540`) and 2 (`0x0061B550`, a jump to the same routine):
recompute (`0x0061B4F0`):
if status ≥ 4 or count[status] = 0, force the lowest s with count[s] ≠
0 (4 if none). Status 3 (`0x0061B560`): if status ≠ 4, recompute; if it
became 4 and the DRLG is a client copy, free the room's tiles
(`0x0066F1A0`). The server keeps tiles until the level is freed or the
active room is removed (§8).

**Propagate(room, s)** (`0x0061B390`): if the room has no rooms-near
array, build it (§3). For each entry N of the array, in order (count and
array re-read every iteration):
1. if s < 3: Propagate(N, s + 1) first (depth first);
2. if N's status ≥ s and N's counts[0..s] are all 0: run set handler s
   on N;
3. N.count[s] += 1.

**Set-and-propagate(room, s)** (`0x0061B490`): Propagate(room, s + 1);
then steps 2–3 of the loop for the room itself with s.

**Unpropagate(room, s)** (`0x0061B5B0`): for each non-null entry N in
order: N.count[s] −= 1; unset handler s on N; if s < 3,
Unpropagate(N, s + 1).

#### 4.1 Client changes room (`0x0061B6F0`, via `0x0061A110`)

Called from `0x00537B50` with (old, new) DRLG rooms (either may be
none): if old ≠ new: first, if new: Set-and-propagate(new, 0); then, if
old and old.count[0] ≠ 0: old.count[0] −= 1, unset handler 0 on old,
Unpropagate(old, 1).

So every room in new's rooms-near array (including new) gets status 1
and is built; rooms two steps away status 2; three steps away status 3.
Builds happen in depth-first order: for each entry N of new's array in
order, the subtree below N is processed before N itself is built.

`0x00537B50` then (server, per client): updates the level activity counts
(`drlg/levels.md` §9.1), and diffs the adjacency arrays of the old and
new active rooms: each room in the new array not in the old gets the
client (`0x0053A8E0` → `0x0061A660`), each room in the old array not in
the new loses it (`0x0053A9B0` → `0x0061A700`).

#### 4.2 Client in sight by coordinates (unused on the server)

`0x0061B640` / `0x0061B690` (D2MOO `DRLGACTIVATE_Set/UnsetClientIsInSight`)
look a room up by coordinates and set/unset status 1 with propagation.
Their only callers `0x0061A070` / `0x0061A0C0` have no direct calls in
1.14d; they run on the client from the receive-table handlers of S→C
0x07 / 0x08 (`client/model.md` §9).

#### 4.3 Stream a room (`0x0061B730`, via `0x0061A140`)

Used when a unit is placed at coordinates (`0x00553720`) and by the spawn
room choice (`drlg/levels.md` §10): load DT1s if needed, add preset units
if type 2 and not yet added, then build (§4.4) if the room has no flag
0x100000. Returns the active room. Does not change any status.

#### 4.4 Build (`0x0061B190`, inlined in `0x0061B730`)

1. If the room has no rooms-near array: build it (§3).
2. `0x0066EE40`: room seed := `init_low(dwInitSeed)`; tile grids
   (`drlg/rooms.md` §9).
3. `0x0066EE70`: map tiles of the room (§9; draws from the room seed).
4. Create the active room (§5).
5. drlg +0x98 += 1 (builds since the last client update), drlg +0x08 +=
   1. Set handler 1 then resets the build timer (§4.6 rule 2; only a
   client reads it).

#### 4.5 Waypoint and spawn rooms

`drlg/levels.md` §10 streams (§4.3) the chosen room; the waypoint search
streams the waypoint room before reading its preset units.

#### 4.6 Client copy only

`0x0061B920` (D2MOO `DRLGACTIVATE_Update`), the **client build timer**.
The server never runs it: on the server only status 1 builds rooms.

1. **When.** Once per client update `0x0044C790` (`client/model.md` §5
   rule 1: single player, a pass where the server ticked, while in
   game), on the client act's DRLG (`0x0061AA90`: act +0x48), after the
   unit update and before the automap reveal (`0x00459020`). Its only
   caller is `0x0044C7E6`.
2. **State** (all in the DRLG, zero at allocation, `drlg/levels.md` §3):
   builds counter B (u8 +0x98; every build §4.4 step 5 and every stream
   §4.3 add 1), timer T (u8 +0x45C), cursor C (+0x460, a DRLG room or
   none). Reset value R = 5 when DRLG flags (+0x8C) bit 0 (client copy)
   is set, else 7 (`0x00642A00`); set handler 1 also sets T := R after
   its build (`0x0061B2F8`).
3. Statistics: drlg +0x08 and +0x468 are copied to two of four globals
   (`0x0096C8B8`/`BC` for a client copy, `0x0096C8C0`/`C4` otherwise);
   nothing reads them for an outcome (d2rs: not modelled).
4. If B > 1: B := 0 and stop (no timer step).
5. Else T := T − 1 (u8 wrap). T ≠ 0 → stop; **B is kept** (it carries
   into the next call).
6. T = 0: T := R. If C is none or C's status byte (+0x44, `0x0061B99E`)
   ≠ 2, C := the first room of the status-2 list (the next pointer of
   the list head, drlg +0x294; the head node itself is at drlg +0x278,
   §4; with an empty list that pointer is the head node). The head node
   reads status 2 (rule 9), so a cursor on the head node is kept.
7. Walk from S = C along the status-2 list (next +0x1C, circular through
   the head node): for each room N: if N is not the head node, has no
   active room (+0x30) and no flag 0x100000, build it (§4.4, which adds
   1 to B). Then N := next; stop when N = S or B ≥ 1. So the walk
   **examines rooms until one build has happened**, and when B was
   already 1 on entry it examines only the first room (building it if
   eligible).
8. C := the room after the last one examined; B := 0.
9. **Cursor on the head node.** `0x0061B7E0` (called once, by the DRLG
   allocation at `0x00642F23`) sets head s's status byte +0x44 := s, so
   the status-2 head reads 2 in rule 6 and stays the cursor (it is
   neither "none" nor "status ≠ 2"). C is the head node after: a walk
   whose last examined room is the list tail (C := tail's next), a walk
   from S = head that builds nothing (it stops back at S), and rule 6 on
   an empty list. A walk from S = head examines the head first (never
   built: rule 7's head test, `0x0061B9C9`), then N := the first room:
   - B = 1 on entry: B ≥ 1 stops the walk at once: **no build this
     call**, C := the first room, B := 0. Resetting C to the first room
     instead would examine (and maybe build) it in this call.
   - B = 0 on entry: the walk goes on from the first room exactly as if
     it had started there, ending on the head (C := head) when nothing
     is built.

   Test vector (synthetic, client copy, R = 5): status-2 list [X, Y],
   both without active room and flag 0x100000; C = head, T = 1. A call
   with B = 1: T → 0, T := 5, C kept, the walk examines the head only →
   no build, C = X, B = 0. The same state with B = 0: X is built (B =
   1), the walk stops at Y → C = Y, B = 0.
10. **The level-free counter** `[0x007A0498]` (below) is a client game
    global. Writers (every reference in the 1.14d export): += 1 at
    `0x0044C806` (the client update); 0 at the game start (`0x0044E200`,
    called by the client game loop `0x0044F360` at `0x0044F4E0` before
    its first update: the memset of `0x007A0480`..`0x007A04FF` at
    `0x0044E20A` and the store at `0x0044E30A`); 0 at the game end
    (`0x0044C890`, memset at `0x0044C904`, called at `0x0044F735`).
    Other readers: getters `0x0044DA90`, `0x0044DB00` (other owners).
    An act change (S→C 0x03), a level load or a new client DRLG does
    not reset it. So it counts client updates from 0 per game; the
    first client level free runs on the 13th update of the game.
11. **Cursor on a freed room.** C has no writer besides rules 6 and 8
    (`0x0061B9AA`, `0x0061B9F4`); freeing a room (§2.1, `drlg/levels.md`
    §9 rule 4) does not clear it. A room is freed only at status 4
    (`drlg/levels.md` §9 rule 3), so it is in no list and its status
    byte was 4 at the free. 1.14d's rule 6 then reads +0x44 of the freed
    block: unchanged → 4 ≠ 2 → C := the first room; reallocated before
    the read → whatever the new owner wrote there (allocator layout,
    not reproducible). d2rs: a cursor whose room was freed reads "not
    status 2" (equivalently: clear C when its room is freed), so the
    next rule 6 sets C := the first room. That matches every case in
    which the freed block's byte +0x44 is not 2 at the read, including
    every case without a reallocation in between. The one divergent
    case is a reallocation that leaves 2 there (for example a new DRLG
    room at the same address that is in the status-2 list: 1.14d then
    walks from that room); it depends on allocator addresses and is not
    modelled (open question 23).

Consequences: on a client, a status-2 room (two rooms-near steps from a
room in sight, §4) is built at most one per call, at the earliest R
calls after the last set-handler-1 build, while at most one other build
happened per call; T starts at 0, so with no set-handler-1 build the
first timed build waits 256 calls. Every build draws from its room seed
(§4.4) and creates an active room (§5) with its active-room seed, so a
client that skips this timer has fewer active rooms and different
unit-creation results (`client/model.md` §2 rule 6, §12 rule 3).

The same client update also runs the level free of `drlg/levels.md`
§9 rule 2 on the client DRLG (`0x0061AA20`) on every 13th call (counter
`[0x007A0498]` += 1 first; free when it is a multiple of 13; reset per
game, rule 10).

### 5. Active room creation (`0x006422A0`, `0x00619890`)

1. Coordinates: subtile rect = tile rect × 5 (`0x00643560`), tile rect
   copied.
2. Only if the tile grid has walls or floors (grid +0x18 or +0x20 ≠ 0);
   else no active room is made.
3. Flags := 4 if room flag 0x40000; else 1 if other flags bit 0 (was
   populated, §8.3); else 0.
4. **Draw (room seed):** one step; the active room seed := `init_low(lo')`
   (`0x00619890` at `0x006198FB`).
5. Allocate 0x80 bytes, zeroed; DRLG room, tile data, coordinates; act.
   **Prepend** to the act's room list and set act +0x54 := 1
   (`unit-order.md` §4, `tick.md` §4). DRLG room +0x30 := it.
6. Adjacency arrays (`0x00619800`): allocate the array with the DRLG
   room's rooms-near count; fill it (§6.1). Then for every entry N ≠ the
   new room, refill N's array from N's DRLG room (§6.1).
7. Collision grid (`0x0064C900`, §10).
8. Act callback (act +0x4C) if set: only the client sets one
   (`0x0061AF60` from `0x00475B40`).
9. The callback is called with ECX = the new active room, after step 7,
   at `0x00619954` (the only call through act +0x4C in `Game.exe`). The
   client registers `0x00475930` in the 0x03 act load (`0x0044E100` →
   `0x00475B40`, right after the act is built, `client/model.md` §7
   rule 4), so it runs for every client active room. Its effect is
   owned by `render/lighting.md` §6.4 (last paragraph): kind-2 light
   records whose radius reaches into the new room drop their cached
   contribution. No DRLG state, no RNG draw, no unit change; the
   automap has its own callbacks (`0x00459150` / `0x004591A0`, drlg
   +0x454). d2rs: the client DRLG reports each new active room to the
   light cache; the server registers nothing.

A populated room that is removed and built again starts with flag bit 0:
`tick.md` §4 then restores its inactive units instead of populating it.

### 6. Adjacency array order (owner of `unit-order.md` §9)

1. **Fill** (`0x0066BD00`): walk the DRLG room's rooms-near array in
   order; append each entry's active room if it has one; pad the rest with
   null; count := number appended. So after a fill the adjacency array is
   the rooms-near order (§3) restricted to active rooms, and contains the
   room itself.
2. Every activation refills the new room's array and the arrays of all
   its active neighbours (§5.6).
3. **Removal** (`0x0061A910`): for each entry N ≠ the removed room R in
   R's array: find R in N's array (first match); replace it by N's last
   entry; count −= 1. N's order is now perturbed until N is refilled by a
   later activation next to it.
4. Consumers walk the array from index 0 to count − 1
   (`unit-order.md` §6.4 and §9).

### 7. Room clients and the inactivity counter

1. Client array (+0x48, count +0x78, capacity +0x04 grown by 4): add
   (`0x0061A660`) appends then bubble-sorts the array by client record
   **address** ascending (`0x0061A5A0`); remove (`0x0061A700`) moves the
   last into the hole and sorts again. Membership changes only at a
   client's room change (§4.1): the client is added to every room of its
   new room's adjacency array as it stands after that change's builds, and
   removed from rooms of the old array missing from the new one. Later
   changes of the array (other clients' activations, removals) do not
   update membership.
2. **Inactivity counter** (`0x0061A790`, room +0x0C; tick open question
   4): each call sets it to 0 if the client count is non-zero, else adds
   1; returns the new value. Its only caller is tick step 9
   (`0x0052D240`), once per active room per pass (every 12 frames).
3. So a room is removed at the first pass where it has had no client for
   11 consecutive passes (the counter reached 11) and the removal test
   passes (§8.1). If the test fails the counter keeps growing; the room is
   removed at the first later pass where the test passes and it still has
   no client.

### 8. Deactivation (tick step 9)

1. **Removal test** (`0x0061A3F0` → `0x0061BA30`; fatal error on a client
   copy or a null room): false if the DRLG room has flag 0x400000
   (portal); else the client-copy check (`0x0066C0B0` returns the
   room's DRLG through level +0x1B4, fatal 0x42B/0x42C/0x42D on a null
   room, level or DRLG; `0x00642A00` on it, fatal 0x2E7 for a client
   copy); false if status ≤ 1 (some client's room is
   this room or next to it); if the level is a town (1, 40, 75, 103, 109)
   or level 120: false if any room of the level has status ≤ 1; else true.
   **Flag 0x400000 setter** (owner): `0x0061AED0(active room, clear)`
   does nothing for a null room, else calls `0x0061BAC0` on the DRLG
   room (active room +0x10): clear = 0 sets DRLG flag 0x400000, clear ≠ 0
   clears it. `0x0061BAC0` has no other caller and holds the only
   immediate `or`/`and` of the bit on DRLG +0x28 in `all.asm`; 31 call
   sites pass 0 or 1 (quest code passes 0 to keep a room, missile bodies
   pass 1; per-site rules live in the quest and missile specs).
2. If true, tick step 9 compresses each unit of the room to inactive
   storage (`0x005433F0`, room list order, next saved first; unit specs)
   and removes the room (`0x0061A910`): unlink from the act list (linear
   search; fatal error if absent), fix neighbours' adjacency arrays
   (§6.3), then `0x0066B4C0`: DRLG room +0x30 := 0, other flags := active
   flags & 1, and if flag 0x100000: free the room's tiles (`0x0066F1A0`,
   §9); then free the active room (`0x0061A840`: rule 4 for every
   remaining unit; collision grid freed `0x0064CA10`; removal records
   freed `0x0061A2C0`; client array and adjacency array freed).
3. The DRLG room keeps its seeds, statuses, rooms-near array and preset
   data; a later build (§4.4) re-derives the same room seed.
4. Units still in a freed room (`0x0061A840`, 1.14d-confirmed): while
   the room's first unit (active room +0x74) is not null, for that unit
   U: if U's flags (unit +0xC4, `sim/units.md` §2) lack 0x400000
   (D2MOO name `UNITFLAG_ISCLIENTUNIT`), U's flags 2 (+0xC8) |= 0x20; then U's flags |= 0x800000; then
   U leaves the room (`0x0064C450`), which unlinks it so the next head
   is the next unit (room list order, `sim/unit-order.md` §5):
   - player, monster, missile (dynamic path): precise and client x, y
     := 0 (the constants at `0x006EB7C8` / `0x006EB7CC` are 0 and never
     written), point count (+0x28) := 0; if the path has a room: previous
     room (+0x20) := room, room-list remove (`0x0064C370`,
     `sim/unit-order.md` §5 rule 3), path flag 0x2 (room changed). The
     path room (+0x1C) itself is not cleared.
   - object, item, tile (static path): if its room (static +0x00) is
     set: room-list remove (`0x0064C370`); static room := null.
   The units are not freed. The only reader of the two bits is the
   client unit update `0x00480810` (`client/model.md` §5 rule 5: C→S
   0x4B, then both bits cleared at `0x0048084D`/`0x00480857`); no server
   code tests them. Unit flag 0x400000 is set only together with
   0x200000 on client-only units (`0x00466437`, `0x004667BF`), so on the
   server every unit left here also gets flags-2 0x20.
5. **Which server units step 2 leaves in the room** (per-unit compress
   `0x005433F0`, called by the tick-step-9 loop at `0x0052D0A8`; type
   dispatch table `0x00543504`):

   | Type | Exit |
   |---|---|
   | 0 player | if `0x00639DF0(unit, 7)` ≠ 0: compressed, flags-2 \|= 0x100, saved (`0x00542E10`), leaves the room (`0x0064C450`), kept; else freed (`0x00555600`) |
   | 1 monster | `0x005431F0`: saved and leaves the room (kept), or saved and freed |
   | 2 object | class 59 or 60: as the kept player exit; else compressed, saved when the keep flag holds, freed |
   | 3 missile | freed |
   | 4 item | saved (`0x00542E10`) only: neither freed nor removed from the room |
   | 5 tile | saved, freed |

   So on the server rule 4 runs for the room's items (and nothing
   else): each gets 0x800000 and flags-2 0x20 and leaves the room.

### 9. Room tile grid

#### 9.1 Summary

A DRLG room (RoomEx) turns its source grids into four lists of **tile
records**: floors, walls (roofs, type 15, are wall records from the wall
layers, §9.5.1 step 6; the client files them by type, `render/draw-order.md`
§3 r2), shadows (type 13), plus
the extra frames of animated tiles. Each grid cell names a tile *key*
(orientation, main index, sub index); the actual DT1 tile is chosen
among the room's loaded tiles with that key, weighted by DT1 rarity,
with one draw from the **DRLG room seed** per choice. Preset rooms take
their grids from the DS1 (`drlg/preset.md`, owner); outdoor rooms get
theirs from the outdoor code (`drlg/outdoor.md`, owner). This section
owns everything from "a grid of packed cells" to "tile records", the
per-room tile library, the rarity choice, cross-room tile linking and
tile animation.

#### 9.2 When it runs (build sequence)

`0x0061B730` (and its twin `0x0061B190`, called from `0x0061B2D0` /
`0x0061B920`) brings a room's tiles up when the room is activated. Order:

1. If room flag `0x1000000` (tile library loaded) is clear: load the
   room's DT1 files (§9.3, `0x0066F240`), which sets the flag.
2. Preset room (room type 2) whose flag `0x2000000` is clear:
   `0x00667890` (preset room init, `drlg/preset.md`).
3. If flag `0x100000` (tiles built) is set: stop. Otherwise:
   a. if the room's near-room list is empty, build it (`0x0066C370`;
      order: §3);
   b. **room seed reset** (`0x0066EE40`): room seed :=
      `init_low(dwInitSeed)` (room +0x14 ← room +0x04, hi = 666;
      rng.md §5.4). `dwInitSeed` is the `lo'` of the single step taken
      at room allocation (`0x0066B461`), so after the reset the seed is
      `{dwInitSeed, 666}` (recorded: every reset's `old.lo` equals
      `new.lo` when nothing drew in between, e.g. seq 6821);
   c. grid init by room type (`0x0066EE40` tail): type 1 (outdoor-grid room)
      → `0x0067D2D0`, type 2 (preset) → `0x006667D0`; other types none.
      These fill the packed grids; outdoor ones draw from the room seed
      (sites in §9.8, owner `drlg/outdoor.md`);
   d. tile fill (`0x0066EE70`): type 1 → `0x0067D710`, type 2 →
      `0x00666AC0`; then flag `0x100000` is set (for any type);
   e. active-room seed: one room-seed step (`0x006422A0`, recorded at
      `0x00642340`, one per built room), owned by the active-room
      section.

Because of 3b the tile draws restart from `dwInitSeed` every time a room
is (re)built; a rebuilt room repeats its draws exactly **if** its
neighbours are in the same built/unbuilt state (§9.6 changes the count).

`0x0066F1A0` (free room tiles, args room, keep): if not keep and the room
has an active room (room +0x30), remove it from the act (`0x0061A910`;
that path frees the collision grid, §10.6); clear room +0x30; free the
logical coord lists (`0x0066C6E0`); if flag `0x100000` is set: clear it,
increment the DRLG freed-room counter (drlg +0x468), free the tile grid
(`0x0066F0B0`: the three record arrays, the link list, every animation
record and its frame array, the grid header), free type data (type 1
`0x0067D680`, type 2 `0x00666610`), clear the two tile-record list heads
(+0x0C, +0x14) of every warp entry on room +0x4C; if keep, set flag
`0x200000`. The DT1 library (flag `0x1000000`) is **not** released.

Callers and the keep argument:

1. `0x0066B4C0(room, client flag, populated)`, called once, from the
   active-room removal `0x0061A910` (§8) with the act's client flag (act
   +0x50) and the active room's flags (+0x34): room +0x30 := 0; room
   "other flags" (+0x60) := populated & 1 (the whole field is
   overwritten); if flag `0x100000` is set, `0x0066F1A0` with **keep =
   1 when the act's client flag is 0** (the server), else 0. Because
   +0x30 is already 0, the "remove from the act" step never runs on this
   path; keep only decides flag `0x200000` (set on the server).
2. `0x0061B560` (status-3 unset handler, §4): keep = 0, client copies
   only.

Type-2 type data free (`0x00666610` → `0x00666520`, no draws): if the
room has preset room data (+0x20) whose preset map has a DS1 file, it
releases the room's grid views over the DS1 layers: for each wall layer
(file wall-layer count) the orientation grid (data +0x60 + 0x14·i) and
the wall grid (+0x10 + 0x14·i), for each floor layer (file floor-layer
count) the floor grid (+0xB0 + 0x14·i), then the shadow grid (+0xD8).
A grid free (`0x0067CD90`) releases the grid's own row-offset array if
it has one and clears the grid. The DS1 data itself and the preset room
data stay; the next build's grid init (§9.2 step 3c) makes new views.
Nothing in the simulation reads these grids between a free and the
next build.

#### 9.3 Tile library of a room

`0x0066F240`. The room has 32 library slots (room +0x68) and a 32-bit
DT1 mask (room +0x50; set at allocation from the lvlprest / lvlmaze /
outdoor source, owner of each). With `T` = the `LvlTypes` row of the
room's level type (level +0x1C0), in this order:

1. For bit `i` = 0..31 of the mask (stop when no bits remain): if set,
   load `T.File(i+1)` into the **first free slot**.
2. Load `DATA\GLOBAL\Tiles\Act1\Outdoors\Blank.dt1`,
   `...\Act1\Barracks\InvisWal.dt1`, `...\Act1\Barracks\Warp.dt1`, each
   into the next free slot.
3. Set room flag `0x1000000`.

A slot load (`0x00604A40`) is fatal (error 0x2A) when all 32 slots are
full. DT1 files are cached process-wide by path (`0x00600710`), so rooms
share library objects. Example (Act 1 town, `LvlTypes` "Act 1 - Town",
lvlprest Dt1Mask 959 = bits 0–5, 7–9): slots = Floor, Objects, Fence
(Town), River, stonewall, trees, Objects (Outdoors), TreeGroups, Bridge,
Blank, InvisWal, Warp.

**Library index.** Each loaded DT1 is indexed by key (orientation,
main index, sub index) = DT1 tile header fields +0x14, +0x18, +0x1C.
Tiles are inserted in file order; the first tile of a key creates the
key's list, every later tile with the same key is **inserted at the head**
(`0x0060CEA0`; a tile already in the list is not added twice). So within
one file a key's tiles are listed in **reverse file order**.

**Lookup** (`0x00604AE0`): walk slots 0..31 in order, append each slot's
list for the key, stop when 40 entries are collected (later matches are
dropped). The result order is: slot order, then reverse file order.

A library entry is the DT1 tile header (`formats/dt1.md`): material flags
at +0x06, orientation +0x14, main +0x18, sub +0x1C, rarity/frame +0x20,
sub-tile flags +0x28 (accessors `0x00604BC0`, `0x00604B60`, `0x00604B90`,
`0x00604C50`, `0x00604C20`, `0x00604CE0`, in that order: `0x00604BC0`
returns the u16 material flags at +0x06, then the u32 fields +0x14,
+0x18, +0x1C, +0x20, and `0x00604CE0` the address of +0x28; each is
fatal on a null entry). The monster population's water-point search
tests material bit 0x2: `0x00604BC0(entry) & 2`
(`0x005B2789`, `monsters/population.md` §9.2).

**Entry identity.** The index stores pointers into the loaded file's
own tile-header array (`0x0060A440`: header `i` at the file's first-tile
pointer (file header +0x110) + 0x60·i, i = 0 … count (+0x10C) − 1,
each passed to the insert `0x0060CFA0`). So an entry, and the DT1 tile
pointer of every tile record built from it (record +0x18, §9.5), is
exactly one **(DT1 file, tile index in file order)**, and every other
header field the client reads comes from that same header:
light direction +0x00, roof height +0x04 (u16), height +0x08 (i32),
the block list (`formats/dt1.md`). Files are cached process-wide by
path, so equal pointers mean equal (file, index). d2rs: the tile choice
(§9.4) returns the entry as (library slot's DT1 path, index); the tile
source's per-tile record carries roof height and height next to the
fields above (`d2_sim::drlg::TileInfo`: `roof_height: u16`, `height:
i32`), and the client keeps (path, index) per record to reach the block
data. No DRLG outcome reads roof height or height (they matter only to
the draw, `render/draw-order.md` §3 rule 2).

#### 9.4 Packed cell and tile choice

**Packed cell** (u32; DS1 wall/floor/shadow cells are already in this
form, `formats/ds1.md` "Cell interpretation"). Bits used by the tile code
(names from D2MOO, use confirmed at the 1.14d addresses given):

| Bits | Name | Used for |
|---|---|---|
| 0 (0x1) | wall | cell has a wall tile (type from the orientation grid) |
| 1 (0x2) | floor | cell has a floor tile |
| 2 (0x4) | linked (LOS) | cell is shared with neighbouring rooms (§9.6) |
| 3 (0x8) | enclosed | record flag 0x4 |
| 7 (0x80) | layer above | record flag 0x1 |
| 8–15 | sub index | key |
| 16 | fill LOS | record flag 0x80 |
| 17 | unwalkable | record flag 0x40 |
| 18–19 | layer index | record layer, link matching |
| 20–25 | main index | key |
| 26 | reveal hidden | record flags 0x20C |
| 27 | shadow | cell has a shadow (type 13) tile |
| 28 | linkage | record flags 0x102 |
| 29 | object wall | record flag 0x800 |
| 31 | hidden | record flag 0x8 |

Key of a cell: main = bits 20–25, sub = bits 8–15; a cell value of 0
means key (0, 0). The tile *type* (orientation) is the cell's entry in
the orientation grid for wall layers, 0 for floors, 13 for shadows.

**Tile choice** (`0x0066D820`, room in ECX, type in EDX, packed cell on
the stack). With `E` = lookup(type, main, sub) (§9.3):

1. If `E` is empty: `E` = lookup(10, 0, 0) (Warp.dt1 has such a tile);
   if that is empty too: fatal (0x73).
2. `total` = sum of the entries' rarity (fatal 0x7C on a null entry).
3. If `total > 0`: **one draw** from the DRLG room seed (room +0x14),
   `r = roll(total)` (rng.md §3: `lo' & (total−1)` when `total` is a
   power of two, site `0x0066D900`; else `lo' % total`, site
   `0x0066D8E3`). If `total ≤ 0`: no draw, `r` = 0.
4. Walk: `n = r + 1`, `i = 0`; while `|E| > 1` and `n > 0`: `n −=
   rarity(E[i])`, `i += 1`. Then if `i > 0`, `i −= 1`. Result `E[i]`.

Consequences: a rarity-0 entry is never chosen when another entry has
rarity > 0; if all rarities are 0 the first entry is taken without a
draw (so every Blank.dt1, InvisWal.dt1 and Warp.dt1 lookup, and any
single rarity-0 tile, costs no draw); a single entry with rarity > 0
still costs a draw. D2MOO's version always calls its roll helper; 1.14d
skips the draw on `total ≤ 0`, which gives the same draws only because
its helper also returns 0 without stepping for `n ≤ 0`.

#### 9.5 Filling one grid

A room's tile fill (`0x00666AC0` preset, `0x0067D710` outdoor) runs a
counting pass (capacity of each record array; draws nothing), allocates
the arrays (`0x0066EEE0`), then a **fill pass**: one call of `0x0066EC10`
per grid, in this order:

| Source | Grids, in order | Orientation grid | FillBlanks |
|---|---|---|---|
| preset (`0x00666AC0`) | each floor layer 0..n−1; each wall layer 0..n−1; the shadow layer; if lvlprest Def is 1 or 108, one extra local grid (zero-filled here, so it adds no tiles; `drlg/preset.md`) | wall layer i's own orientation layer; none for floors/shadow | floor layer 0 only, if lvlprest FillBlanks |
| outdoor (`0x0067D710`) | the outdoor wall grid (with its orientation grid), then the outdoor floor grid | | no |

Then, preset rooms with lvlprest Animate: animation (§9.7). Record array
counts are frozen (capacity := used).

`0x0066EC10` visits cells **row by row (y outer, x inner)** over
`(width + 1) × (height + 1)` cells (the extra row/column is the shared
edge with the next room), minus the last column if KillEdge-X, minus the
last row if KillEdge-Y (preset: KillEdge-X/Y apply only when lvlprest
KillEdge is set and the room's right/bottom edge is the map's edge).
Cell (x, y) is world tile (room X + x, room Y + y). Each cell goes through
`0x0066E9B0` (§9.5.1) with the cell's type (0 if the grid has no
orientation grid).

The preset edge flags (`0x84` OR'd on the border cells of floor layers,
wall layer 0 and the shadow layer, and `i << 18` on layer i) are written
by the preset grid init into the shared DS1 data (`drlg/preset.md`); the
linked bit (0x4) they set drives §9.6.

##### 9.5.1 One cell (`0x0066E9B0`)

With `v` = packed cell, `t` = type, `main`/`sub` its key, "inside" =
(x, y) lies in the room's own rectangle (`0x0066B980`: X ≤ wx < X+W and
Y ≤ wy < Y+H, i.e. not the extra edge row/column):

1. `t` is 10 or 11 and `main ≥ 8`: nothing.
2. `t` = 0 and key (30, 0) or (30, 1): set the hidden bit in `v`.
3. If hidden:
   - `t` 8 or 9 (door) and the level is not 111, 112 or 117: add the
     door's preset unit (`0x0066D9E0`, below) and stop.
   - `t` 10 or 11 (exit, `main < 8` here): add the warp unit
     (`0x0066E1C0`, `sim/path-placement.md` §12.1), then the floor warp tiles (`0x0066E360`, below), and
     stop.
4. If the linked bit (0x4) is set (§9.6), at most one of, then stop:
   - floor bit: if key (30, 0/1), clear bit 7; linked tile of type 0;
   - else wall bit: linked tile of type `t`;
   - else shadow bit and not hidden: linked tile of type 13.
   If none applies, fall through.
5. Floor: if the floor bit is set: choose (0, v) → floor record. Else if
   FillBlanks and inside: choose type 0 with key (30, 1) in level 74
   (Arcane Sanctuary) or (30, 0) elsewhere → floor record whose packed
   value is `v` with bit 7 cleared and hidden set.
6. Wall: if the wall bit is set: choose (`t`, v) → wall record of type
   `t` (`0x0066DC50`); for `t` 10/11 outside level 133, the wall warp
   tiles (`0x0066E260`, below).
7. Shadow: if the shadow bit is set: choose (13, v) → shadow record
   (`0x0066DF40`).

So a cell costs, in order: [floor or blank] [wall] [corner half]
[shadow] choices, each a draw when its total rarity is > 0.

**Wall records** (`0x0066DC50`): append a record of type `t`; set its
flags (below); if `t` = 3, **choose (4, v)** and append a second record
of type 4 at the same position (the other half of the corner), with the
same flag rules. Draw order for an orientation-3 cell: type 3, then 4.

**Record flags** (`0x0066DB20`; shadows `0x0066DF40` use the same rules
except the layer and type rules):

| Condition | Record flag |
|---|---|
| type ≠ 13 | `(layer + 1) << 14` (layer = cell bits 18–19) |
| type 14 (tree) | 0x4 |
| type 8, 9, 10, 11 | 0x2 |
| cell bit 7 | 0x1 |
| cell bit 28 | 0x102 |
| cell bit 17 | 0x40 |
| cell bit 16 | 0x80 |
| cell bit 3 | 0x4 |
| cell bit 31 | set 0x8, else clear 0x8 |
| cell bit 26 | 0x20C (sets 0x8 too) |
| cell bit 29 | 0x800 |
| cell bit 2 | 0x2000 |
| DT1 material bit 0 | 0x4 |
| DT1 material bit 2 | 0x800 |

Record flag 0x8 (hidden) means "not drawn" (render) but the record still
exists and still counts for collision (§B). Door records (type 8/9) in a
room also get their preset unit (`0x0066D9E0`, before the flags; skipped
if the record already has flag 0x20). The flag rules routine
(`0x0066DB20`) makes that call itself, first, for every record of type 8
or 9 (`0x0066DB2A`–`0x0066DB3E`), so the tile code sets flag 0x20
through it: `0x0066D9E0` ORs 0x20 into record +0x14 on the outcomes of
`drlg/preset.md` §11 (unit added, or `roll(3)` gave 0). The record's
flags were assigned at creation (`0x0066DC50` writes +0x14 before
calling `0x0066DB20`) and every later rule only ORs (only 0x8 is
cleared), so 0x20 stays; a re-run of the flag rules on a door record
(§9.6 step 3) finds 0x20 and skips the unit and its draw. Record fields (0x30 bytes): screen
x/y of the tile (`0x00643310` on (wx, wy+1); y + 40), position relative
to the room, flags, chosen DT1 entry, type, next-in-chain, RGB = 0xFF.

**Door preset units** (`0x0066D9E0`, also from step 3): tables, lookup
and placement are `drlg/preset.md` §11 (tables transcribed in
`drlg/preset-tables.tsv`, table `door`). **RNG:** object ids 91–92 only:
`roll(3)` on the room seed (`0x0066DAC1`), the unit is placed unless the
result is 0. The draw belongs here because it interleaves with the tile
draws: it comes before the record's flag rules (wall records) and, for a
hidden door cell (step 3, no record yet), instead of any tile choice.

**Warp tiles.** A warp entry is a record of the room's warp-link list
(+0x4C, §3.3; prepend order): +0x04 next, +0x0C and +0x14 two tile-record
chains, +0x10 its lvlwarp record. **Finding the entry for a cell:** W :=
warp id of slot (cell main index, bits 20–25) of the room's level
(`0x0066AF30`, `drlg/levels.md` §7 rule 4); the first entry in list order
whose lvlwarp record's `Id` (record +0x00) equals W. Then
`0x0066E160(t)`: if the entry's record `Direction` (+0x2C) is neither
'b' nor the cell's letter (t = 11 → 'r', else 'l'), the entry's record
is replaced by the lvlwarp row for (`Id`, that letter) (`0x0061F310`,
`drlg/levels.md` §7 rule 4). LitVersion is lvlwarp +0x24, Tiles +0x28.

- Floor warp tiles (`0x0066E360`; wx, wy, v, t): room flag `0x800000`
  first; no entry → nothing more. Else re-resolve as above; if
  LitVersion ≠ 0: for k = 0..3 (`0x006EF554`: (dx, dy) = (0,0), (1,0),
  (0,1), (1,1)) one floor record at (wx − 1 + dx, wy − 1 + dy) with
  packed value v_k = (cell sub << 20) | ((k | 4) << 8), tile = choose
  (0, v_k) — **4 choices, each a draw** when its total rarity is > 0 —
  record flags from v_k, then 0x8 (hidden); each prepended to the entry's
  +0x14 chain. Then every floor record of the room so far (array order)
  whose tile has main = cell sub and sub < 4 is prepended to the entry's
  +0x0C chain (a record met by two exit cells is re-chained; its +0x20 is
  overwritten).
- Wall warp tiles (`0x0066E260`; record R, v, t): no entry → fatal
  0x2B1. If the cell's sub byte (v bits 8–15) is 0 or 4: the warp unit
  first (`sim/path-placement.md` §12.1); if it adds none, stop here.
  Else (and after an added unit): re-resolve as above; R is prepended to
  the entry's +0x0C chain; if LitVersion ≠ 0: v' = v | (Tiles << 8),
  **one choice** (t, v') → a new wall record of type t at R's position
  (§9.5 wall records, prepended to the entry's +0x14 chain), flag 0x8.

Warps themselves: `drlg/levels.md`.

#### 9.6 Tiles shared between rooms (linked cells)

A cell with the linked bit lies on a room border and may already have a
tile record in a neighbour. `0x0066E940` (args: type, packed v, wx, wy):

1. **Find** (`0x0066E580`): for each room in this room's near list
   (room +0x08, count +0x2C), in list order, skipping itself: the
   neighbour must have a tile grid and contain (wx, wy) or have it on its
   border (`0x0066B9D0`); search its link lists of the same kind (floor
   list for type 0, the non-floor list otherwise) for a record at (wx,
   wy) whose type is not 4, which is either a shadow or the cell has no
   shadow bit, and whose layer matches (record has no layer bits, or
   record layer − 1 = cell bits 18–19). First match wins. Order inside a room:
   a room has at most one link node per kind (floor, non-floor), found
   or created on first use and **prepended** to the room's node list
   (tile grid +0x00; node {+0 floor flag, +4 chain, +8 next}); every
   record added to a chain is **prepended** (record +0x20 := old head),
   so a chain lists records newest first, a type-3 record's type-4 half
   before it.
2. **Not found** (`0x0066E620`): exits (10/11) outside the room are
   skipped; otherwise get or create this room's link list of that kind
   (head insertion, 12-byte node), choose (type, v) **on this room's
   seed**, prepend the record to the node's chain (floor / shadow / wall
   record as in §9.5.1, incl. the type-4 corner half, and the wall warp
   tiles for 10/11).
3. **Found** (`0x0066E740`, existing record R in neighbour N):
   - R has flag 0x1 (layer above): if R is a door (type 8/9), re-run the
     flag rules on R with v; stop.
   - **Arguments of both flag-rule re-runs of this step** (this one,
     `0x0066E769`–`0x0066E776`, and the last bullet, `0x0066E91E`–
     `0x0066E92E`): room = **this room** (the one whose grid is being
     filled, not N), position = the cell (wx, wy), packed value = this
     room's v, record = R. So for a door R without flag 0x20 the door
     unit (`drlg/preset.md` §11) is looked up in this room's level with
     this v and right = (R's type = 9), placed at ((wx − this room's
     x)·5 + dx, (wy − this room's y)·5 + dy) and range-checked against
     this room's size, added to this room's preset-unit list, and its
     `roll(3)` (ids 91–92) draws on this room's seed; flag 0x20 goes on
     R (in N's chain).
   - Merged type m, starting from m = t (the new cell's type):
     1. v has bit 7: m = t (no table, no edge test).
     2. Else t is a door (8/9) and wx = this room's x or wy = this
        room's y (top or left edge): m = t, skip 3–4.
     3. Else if t is not a door and R is a door (8/9) and wx = N's x or
        wy = N's y: **stop** (R unchanged, no flag rules).
     4. Else the class of t (`drlg/wall-remap.tsv`, read from
        `0x006EF620` / `0x006EF578`): `table` with R's type ≤ 7 → m =
        table value for R's type; `table` with R's type > 7 → **stop**;
        `keep` (t 0, 4, 10–12, 14–19) → m = t; `stop` (t 8, 9 off the
        edge, 13) → **stop**.
     Earlier text read the classes inverted (−1 is `keep`, −2 is `stop`).
   - Corner handling: R type 3 and m ≠ 3: the record **after R in its
     chain** (R +0x20) gets flag 0x8 and N's collision is updated for it
     (§10.5). Because the type-4 half is prepended after its type-3
     record (step 1), R +0x20 is the record chained into N's list just
     before R, not R's own half (D2MOO: same); if R is the oldest record
     of its chain (+0x20 null) 1.14d writes through a null pointer
     (open question 12).
     R type ≠ 3 and m = 3: R gets flags 0xC008 (layer 3,
     hidden), collision update, then **choose (3, v) on this room's
     seed** and add a new type-3 wall record (plus its type-4 half, a
     second draw) to this room's non-floor link list.
   - If m differs from R's type, or R is a floor showing
     key (30, 0): **choose (merged type, v) on N's seed** (the
     neighbour's room seed, `0x0066E8F7` passes N), set R's type to m and
     tile; if the tile changed and N has an active room, update its
     collision (§10.5).
   - Re-run the flag rules on R with v (flags are OR'd; only bit 0x8 can
     be cleared).

So the number of draws a room makes depends on which neighbours already
have tiles: shared cells already built cost no draw on this room's seed
unless the type changes or the old tile is a blank floor, and such a
re-choice draws from the **neighbour's** seed. Recorded effect: §9.9.

#### 9.7 Animated tiles

Preset rooms with lvlprest Animate only.

- Counting pass (`0x0066D290`, per floor layer, wall layer and the
  shadow layer): for a cell with any of bits 0, 1, 27 whose key lookup
  is non-empty and whose first entry has DT1 material bit 0x100
  (animated): add `count − 1` to the floor (bit 1), wall (bit 0) or
  shadow capacity; set room flag `0x8000000`. No draws.
- After the fill (`0x0066D700` → `0x0066D440` for walls, floors,
  shadows, in that order): for each existing record whose chosen tile
  has material 0x100: look up all tiles of the record's type and the
  key of the cell it came from (frames `F`, up to 40); allocate an
  animation entry {frame records[|F|], frame count, position 0, speed,
  next} at the head of the room's animation list; speed = lvlprest
  AnimSpeed, 80 when 0. The DT1 rarity field is the **frame number**:
  the record's tile is replaced by the entry with rarity 0 (frame 0);
  for k = 1..|F|−1 a new record (floor / shadow / wall of the same type)
  is appended at the same position with the entry whose rarity = k and
  flag 0x8 (hidden). A missing frame number is fatal (0xB6 for frame 0,
  0xCB for others; D2MOO warns and uses the first entry instead).
- **Frame record flags.** The packed value v used for the key and for
  the frame records is the cell of the record's own layer: grid index =
  (record flag bits 14–16) − 1 in the walls / floors / shadows grid
  array being processed (`0x0066D4C4`). Frame records k ≥ 1 are added
  with no chain (+0x20 = 0) and get the full flag rules of §9.5 from v
  (layer, cell bits, DT1 material of the frame entry), then 0x8. A wall
  frame of type 3 also gets its type-4 corner half (one more **choice
  and draw** on the room seed; that half is not hidden and not in the
  frame array). Frame 0 keeps the original record and its flags; only
  its tile changes. Shadow records have no layer bits, so their index is
  −1: the grid slot before the shadow grid (preset room data +0xC4,
  floor layer 1) is read (open question 13).
- The rarity choice in §9.4 still ran first for the cell (total = 0 +
  1 + … + (|F|−1), a draw when |F| > 1); its result is overwritten.
- Tick (`0x0066D410` over the near rooms → `0x0066D3B0`, rooms with flag
  `0x8000000` and a tile grid): per entry, hide frame `pos >> 8`, `pos =
  (pos + speed) mod (|F| · 256)`, show frame `pos >> 8` (clear 0x8). No
  RNG. `0x0066D750` copies the position of the first animated near room
  of one room to every entry of another room's near rooms (keeps
  neighbours in phase).

#### 9.8 RNG sites of the tile code

| Site | Function | Seed | Decides |
|---|---|---|---|
| `0x0066D8E3` | `0x0066D820` tile choice | DRLG room seed (room +0x14), after the reset | `lo' % total`, weighted tile |
| `0x0066D900` | `0x0066D820` | same | `lo' & (total−1)` (power-of-two total) |
| `0x0066DAC1` | `0x0066D9E0` door units | DRLG room seed | `roll(3)`, objects 91–92 placed unless 0; not hit in the Act 1 recording |
| `0x0066F78D`, `0x0066F7B8`, `0x0066F7E9`, `0x0066F8DB`, `0x0066F905` | `0x0066F690` | **level seed** (level +0x1C4/+0x1C8), not the room seed | lvlsub substitution (DrlgTileSub): Fisher–Yates shuffle of candidate positions, then a `roll(Trials…)`-style pick; owner `drlg/outdoor.md` |
| `0x0066F9C9` | `0x0066F990` | level seed (`roll(n)`, +0x1C4) | lvlsub start index; owner `drlg/outdoor.md` |
| `0x0067000D`, `0x00670061` | `0x0066FF50` | DRLG room seed | DrlgTileSub; not hit in Act 1 |
| `0x006701DB`, `0x006702A4`, `0x006702C3`, `0x006702EB`, `0x00670408`, `0x0067044D` | `0x00670170` | DRLG room seed after the reset (outdoor grid init, step A.2 3c) | DrlgTileSub per-room substitution; owner `drlg/outdoor.md` |
| `0x006706D7` | `0x006706A0` (from outdoor room alloc `0x0067D540`) | DRLG room seed **before** the reset (R@ label) | sub-theme pick; owner `drlg/outdoor.md` |

The labelling script's attributions are correct: `0x0066F690` loads
`[level + 0x1C4]` (esi = the level), so it draws from the level seed (`rng.md` §7 lists
it so); `0x0066D820` and `0x00670170`
use room +0x14/+0x18. Of these, only `0x0066D820` and `0x0066D9E0` are in
the RoomTile object; `0x0066F1A0`–`0x006704E0`+ is DrlgTileSub
(D2MOO order: RoomTile then TileSub).

#### 9.9 Test vectors (recording 20261005-232125-rng)

The seed of this run chose `TownW1.ds1` for the Act 1 town (found by
simulation: only TownW1 room 15 reproduces the first room's counts). The
town is 56 × 40 tiles = 35 preset rooms of 8 × 8, allocated **row-major**
(room k at DS1 origin (8·(k mod 7), 8·⌊k/7⌋); alloc seq 2456 + 3k on the
server, 13405 + 3k on the client).

**First room** (server, room k = 15, origin (8, 16), seed address
0x5411114, `dwInitSeed` 3635379357): reset at seq 6821 (`0x0066EE49`,
`new = {3635379357, 666}`), then 72 tile draws (57 at `0x0066D8E3`, 15 at
`0x0066D900`) seq 6822–6893, then the active-room step (`0x00642340`).
No neighbour had tiles, so every linked cell went to "not found".
Simulating §9.3–§9.5 (floor layer, wall layers 0–1, shadow layer, 9 × 9
cells each, FillBlanks on the floor layer) gives the same 72 choices with
the same site for each draw (power-of-two total ↔ `0x0066D900`). First
draws:

| # | seq | cell (x,y) | type, key | total (entries) | site | lo' | r | entry |
|---|---|---|---|---|---|---|---|---|
| 1 | 6822 | row 0, linked floor | 0, (0,0) | 12 (12) | 8E3 | 3633395563 | 7 | Floor.dt1 #39 |
| 2 | 6823 | row 0 | 0, (0,0) | 12 | 8E3 | 3691942195 | 7 | Floor.dt1 #39 |
| 3 | 6824 | row 0 | 0, (0,18) | 2 (2) | 900 | 1319667706 | 0 | Floor.dt1 #143 |
| 4 | 6825 | row 0 | 0, (0,21) | 3 (3) | 8E3 | 1599208247 | 2 | Floor.dt1 #61 |
| 5 | 6826 | | 0, (0,0) | 12 | 8E3 | 2864581749 | 9 | Floor.dt1 #37 |
| 10 | 6831 | | 0, (0,0) | 12 | 8E3 | 1835282004 | 0 | Floor.dt1 #133 |
| 11 | 6832 | first interior (unlinked) floor | 0, (0,23) | 3 | 8E3 | 1329633184 | 1 | Floor.dt1 #99 |
| 12 | 6833 | | 0, (0,19) | 2 | 900 | 90112679 | 1 | Floor.dt1 #62 |

(All rarities 1 here, so entry = list index r; the "entry" column assumes
the reverse-file-order lists of §9.3, which the counts do not test.)

**Per-room counts** (`0x0066D8E3` / `0x0066D900` draws attributed to the
room's seed by state chaining, which includes re-choices made on its
seed while a later neighbour was built). "standalone" = simulation with
no neighbour built. Server built 9 rooms (order 15, 7, 14, 8, 9, 21, 16,
22, 23); the client built all 35 in reverse allocation order (34 → 0).

| k | origin | standalone | server (order: mod/mask) | client (order: mod/mask) |
|---|---|---|---|---|
| 0 | (0,0) | 83/6 | – | 35: 65/6 |
| 1 | (8,0) | 81/0 | – | 34: 64/0 |
| 2 | (16,0) | 81/0 | – | 33: 64/0 |
| 3 | (24,0) | 54/27 | – | 32: 48/16 |
| 4 | (32,0) | 54/27 | – | 31: 40/24 |
| 5 | (40,0) | 81/0 | – | 30: 64/0 |
| 6 | (48,0) | 81/0 | – | 29: 72/0 |
| 7 | (0,8) | 68/11 | 2: 67/11 | 28: 58/8 |
| 8 | (8,8) | 66/11 | 4: 52/9 | 27: 52/10 |
| 9 | (16,8) | 64/12 | 5: 57/12 | 26: 51/8 |
| 10 | (24,8) | 41/38 | – | 25: 38/24 |
| 11 | (32,8) | 54/27 | – | 24: 40/24 |
| 12 | (40,8) | 81/0 | – | 23: 64/0 |
| 13 | (48,8) | 81/0 | – | 22: 72/0 |
| 14 | (0,16) | 46/28 | 3: 35/25 | 21: 35/22 |
| 15 | (8,16) | 57/15 | 1: 57/15 | 20: 45/11 |
| 16 | (16,16) | 64/14 | 7: 51/11 | 19: 51/9 |
| 17 | (24,16) | 37/44 | – | 18: 33/32 |
| 18 | (32,16) | 54/27 | – | 17: 40/24 |
| 19 | (40,16) | 81/0 | – | 16: 64/0 |
| 20 | (48,16) | 81/0 | – | 15: 72/0 |
| 21 | (0,24) | 59/22 | 6: 55/17 | 14: 44/21 |
| 22 | (8,24) | 62/12 | 8: 49/9 | 13: 49/9 |
| 23 | (16,24) | 61/23 | 9: 48/18 | 12: 49/18 |
| 24 | (24,24) | 35/46 | – | 11: 30/34 |
| 25 | (32,24) | 54/27 | – | 10: 40/24 |
| 26 | (40,24) | 81/0 | – | 9: 64/0 |
| 27 | (48,24) | 81/0 | – | 8: 72/0 |
| 28 | (0,32) | 72/1 | – | 7: 64/1 |
| 29 | (8,32) | 69/1 | – | 6: 62/0 |
| 30 | (16,32) | 71/1 | – | 5: 63/1 |
| 31 | (24,32) | 40/36 | – | 4: 40/27 |
| 32 | (32,32) | 54/27 | – | 3: 45/27 |
| 33 | (40,32) | 81/0 | – | 2: 72/0 |
| 34 | (48,32) | 81/0 | – | 1: 81/0 |

Checks: the client's first room (k 34) and the server's first room
(k 15) equal their standalone values; client k 33 and k 32 are 9 lower
than standalone (one shared 9-cell column already built by k 34). A full
simulation with §9.6 must reproduce every cell of this table; it is the
conformance vector for linking.

Other levels in the recording (L2, L3; rooms with resets at seq 32718,
32973, 33382) also draw at the TileSub sites (counts per room for room
0x560F014: 6 at `0x006706D7` before the reset, then 30/11 tile draws and
10/115/115 at `0x006701DB`/`0x00670408`/`0x0067044D`); their order
relative to the tile draws belongs to `drlg/outdoor.md`.

#### 9.10 Edge cases

- `map-preview.md` OQ3 (8 unflagged invisible collision tiles in
  TownN1): the tile build does **not** hide them. Their cells are
  `0x00500081` in wall layer 0 (orientation 1, key (5, 0); no bit 31, no
  bit 26), the only tile with that key is River.dt1 #28 (rarity 0, no
  draw), and §9.5.1 sets record flag 0x8 only from bits 31 / 26, frame
  hiding, warp tiles or corner merging. So in game these records are
  visible-flagged; why they are not seen is a client draw-path question
  (OQ 3).
- Hidden cells still get records (except hidden doors/exits handled in
  step 3), and hidden records count for collision (§B).
- 40-entry cap per key; a key spread over many files can drop tiles.
- Exits (10/11) with main ≥ 8 are dropped entirely (no record, no draw).

### 10. Collision map from tiles

#### 10.1 Summary

Each active room has a collision grid: one u16 mask per sub-tile (5 × 5
sub-tiles per tile). At active-room creation it is built from the tile
records of every room in the active room's room list: each record ORs
its DT1 sub-tile flag bytes into its 5 × 5 block and adds three bits from
its record flags. Units then set and clear higher bits at run time
(owners: movement/units specs).

#### 10.2 Where and when

`0x00619890` (active room creation, Dungeon.cpp) calls `0x0064C900`
(alloc + build) after `0x00619800`, before the act's room callback.
`0x0064CA10` frees it (from `0x0061A840`, active room removal). Module
anchor: `.\COLLISN\Collisn.cpp` (0x006EB730) in `0x0064C900`/`0x0064CA10`.

#### 10.3 Grid layout

`0x0064C900`: get the room's coordinates (`0x00619730`: sub-tile X, Y,
W, H, tile X, Y, W, H; sub-tile = tile × 5, `0x00643560`); allocate a
header (those 8 values + mask pointer, 0x24 bytes) plus `2·(W·H + 2W +
0x12)` bytes; zero `W·H` masks; mask index = (sy − Y)·W + (sx − X), row
stride W (sub-tiles).

#### 10.4 Build

For each room R in the active room's room list (`0x00619790`: list at
active room +0x00, count +0x24; it includes the room itself, §6.1), with R's collision grid header giving R's tile
origin (so every listed room must already have a grid), for R's floor
records, then wall records (roofs included), then shadow records (`0x00619660`,
`0x006196A0`, `0x006196E0`), each record (`0x0064C790`):

1. Sub-tile origin `(ox, oy)` = 5 · (R tile origin + record position).
2. Skip unless `(ox, oy)` is inside this room's sub-tile rectangle
   (`0x00619DF0`). Since origins are multiples of 5 and rooms are whole
   tiles, the block is then fully inside; each tile belongs to exactly
   one room's grid.
3. OR the DT1 sub-tile flag bytes into the block (`0x0064C4C0`): mask at
   (ox + c, oy + r) |= flags[5·(4 − r) + c], r, c = 0..4. The DT1 rows
   are stored **bottom row first** (settles `formats/dt1.md` OQ 2 for
   collision).
4. Record flags → mask bits over the whole 5 × 5 block (`0x0064C700`):
   record 0x2 → 0x10, record 0x40 → 0x1, record 0x80 → 0x4.

No record is skipped for being hidden (flag 0x8): hidden walls (TownN1
river banks, InvisWal.dt1, Blank.dt1 fill floors with flags 0x05),
corner halves and animation frames all add their flags. The result is
order-independent (OR only).

#### 10.5 Updates when a tile changes

`0x0064C860` (from linking, §9.6; args: active room, record, new tile or
none): if the room has a grid, find the active room containing the
record's sub-tile origin among the room and its list (`0x00463740`), and
in that room's grid clear the old tile's DT1 bytes (AND NOT,
`0x0064C580`), then OR the new tile's bytes if a new tile is given. The
record-flag bits of B.4 step 4 are not touched.

#### 10.6 Collision bits

| Bit | D2MOO name | Set by |
|---|---|---|
| 0x0001 | WALL | DT1 sub-tile bit 0; record flag 0x40 (cell bit 17, unwalkable) |
| 0x0002 | VISIBLE | DT1 sub-tile bit 1 |
| 0x0004 | MISSILE_BARRIER | DT1 sub-tile bit 2; record flag 0x80 (cell bit 16, fill LOS) |
| 0x0008 | NOPLAYER | DT1 sub-tile bit 3 |
| 0x0010 | PRESET | DT1 sub-tile bit 4; record flag 0x2 (door/exit types 8–11, cell bit 28) |
| 0x0020 | BLANK | DT1 sub-tile bit 5 |
| 0x0040 | MISSILE | DT1 sub-tile bit 6 (units at run time) |
| 0x0080 | PLAYER | DT1 sub-tile bit 7 (units at run time) |
| 0x0100–0x8000 | MONSTER, ITEM, OBJECT, DOOR, NO_PATH, PET, 0x4000, CORPSE | units at run time, never from tiles |

DT1 sub-tile bytes seen in the version-7 Act 1 DT1s of d2data.mpq (measured): 0, 1, 2,
3, 4, 5, 7, 0x20, 0x21, 0x25. Bit meanings beyond "OR'd into the mask"
are the consumers' rules (movement, missiles, line of sight); the names
are D2MOO's.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| near gap | < 6 tiles on both axes | §3.1, §3.3 |
| near candidates | 30 slots, unchecked | §3.1 |
| statuses | 0..3 + 4 none; list stride 0xEC at drlg +0xA0 | §4 |
| propagation depth | statuses 1, 2, 3 (three rings) | §4 |
| removal threshold | counter > 10 (11 passes of 12 frames) | §7 |
| no-removal levels | towns 1, 40, 75, 103, 109 and level 120 (all rooms) | §8.1 |
| warp-link exception | level 133 | §3.3 |
| client copy build timer | 5 (client), 7 (server value, unused) | §4.6 |
| wall remap | `drlg/wall-remap.tsv` (tables `0x006EF620`, `0x006EF578`) | §9.6 |
| floor warp offsets | `0x006EF554`: (0,0), (1,0), (0,1), (1,1) | §9.5 |

## Randomness

1. Room creation (§2), per room in creation order: one level-seed step,
   one room-seed step.
2. Build (§4.4), per room in build order (§4.1 depth-first): rooms-near
   build may generate other levels (their level and room seeds,
   `drlg/levels.md` §5); room seed reset; tile draws on the room seed
   (§9); one room-seed step for the active-room seed (§5.4).
3. Status 2 and 3 handlers: no draws from this spec (DT1 loading; preset
   units: `drlg/preset.md`).
4. Removal and freeing: no draws.

Recorded (town, server, `20261005-232125-rng.jsonl`): room seed reset at
seq 6821 (`0x0066EE49`, {3635379357, 666} = that room's `dwInitSeed`),
70 tile draws (`0x0066D8E3`, `0x0066D900`), then seq 6894 one step at
`0x00642340` (`lo'` 3128655592) and seq 6895 the active-room seed
`init_low(3128655592)` at `0x006198FB`.

## Edge cases & original bugs

1. The near sort is not a total order (§3.2); arrays of two rooms can
   list shared neighbours in different relative orders (observed:
   `0x4f83f00` lists `e80` before `e00`, `0x4f83b00` the reverse).
2. Removal perturbs neighbours' adjacency arrays (§6.3); iteration order
   then differs from the rooms-near order until the next activation next
   to them.
3. More than 30 near candidates overflow the stack array (§3.1); not seen
   in 1.14d data (open question 5).
4. Client arrays are sorted by memory address (§7.1); a reimplementation
   needs a stable stand-in (open question 4).
5. The inactivity counter is only advanced by tick step 9; rooms keep
   their value while a portal or a town blocks removal.
6. Levels reached only through warp links are generated inside the
   build of the linking room (§3.3), so their draws interleave with room
   builds.

## Test vectors

Synthetic (rules; CI-safe):

| Input | Expected | Source |
|---|---|---|
| level seed {4014346870, 666}; create 2 rooms | seeds {2928842600, 666} and {1513463342, 666}; `dwInitSeed` 4134077858 for the first | §2 (= recorded seq 2455–2460) |
| A = (0,0,8,8); R = (8,0,8,8), (14,0,8,8), (13,13,4,4) | gaps (0,−8) near; (6,−8) not; (5,5) near | §3.1 |
| list [A=(10,0,8,8), B=(0,0,8,8)] | sorted [B, A] (A.x ≥ B.x + B.w) | §3.2 |
| list [A=(0,10,8,8), B=(10,0,8,8), C=(0,0,8,8)] | pass 1: (A,B) swap (A.y ≥ 8), (A,C) swap → [B,C,A]; pass 2: (B,C) swap, (B,A) swap → [C,A,B] | §3.2 |
| list [A=(0,0,8,8), B=(8,8,8,8), C=(8,0,8,8)] | [A,C,B] | §3.2 |
| adjacency [R, X, Y, Z]; remove X | [R, Z, Y] | §6.3 |
| counter 0, no client, 11 passes, test true | removed on pass 11 | §7.3 |

Recorded:

| Recording | Observation | Rule |
|---|---|---|
| `20261005-232125-rng.jsonl` seq 2454–2591 | 47 town rooms: each one level-seed step (`0x0066B42E`), `init_low` (`0x0066B457`), one room-seed step (`0x0066B461`) | §2 |
| same, seq 6821–6895 | first build: reset to `dwInitSeed`, tile draws, active-room seed | §4.4, §5 |
| `20261006-022304-tick.jsonl`, frames 1–50 | 9 activations: the start room, then 8 rooms in the order they appear in the start room's adjacency array (snapshot frame 50: `b80, c00, c80, d00, d80, [b00], e00, e80, f00`) | §4.1, §6.1 |
| both tick recordings, all snapshots | 4,975 room snapshots: own room present, members active, no duplicates, symmetric | §6 |

Checked on the three traces (`py tools/trace-recorder/check_rooms.py
traces/sim/tick/*.json`; `--selftest` runs the rooms.md vectors above
plus one perturbation per check, each reported at exactly the changed
event; 446 snapshots, 0 violations). Replay of the `room_activate` /
`room_deactivate` inputs and the player's `room_add` events against the
`lists` snapshots:

| Check | Property | Result |
|---|---|---|
| C1 | own room present, members active and listed, no duplicates, symmetric | 5,808 arrays, 0 violations |
| C2 | per room, the arrays seen after a refill (activation of the room or of a neighbour) and before the next removal next to it are subsequences of one fixed sequence (union of their orders acyclic) | 4,668 arrays checked, 0 violations; 1,140 excluded as perturbed by a removal and not yet refilled |
| C3 | across a snapshot pair with exactly one removal of R and no activation, arrays that held R = previous with R replaced by the last entry, length −1 | 4 pairs, 5 arrays, 0 violations; 56 pairs undecidable (several removals, or activations in the same window, which refill the arrays) |
| C4 | burst = consecutive `room_activate` events of one tick; anchor = room of the player `room_add` directly before it (else directly after, for the first placement); the anchor itself is excluded (built first); the other rooms are activated in the anchor's array order read from the first later snapshot with no removal in between | 69 of 95 bursts checked (205 activations), 0 violations; 26 undecidable (19 without an adjacent player `room_add`, 7 with a removal before the next snapshot) |

C3 is thin (5 arrays): the traces hold few isolated removals, and the
snapshots are 25 frames apart, so most removal windows also contain
activations. The §6.3 swap-with-last rule is otherwise exercised only by
the synthetic vector. Rooms-near order itself (§3) and §7.3 are not
checkable from the trace. Full check (needs a
recorder extension): record per active room the DRLG room (+0x10), its
tile rect (+0x34..+0x40), level id, rooms-near array (+0x08/+0x2C),
status (+0x44) and counts (+0x0C), the room's client count (+0x78) and
counter (+0x0C), and per level all DRLG rooms in list order; then §3 and
§6 are recomputed exactly and §7.3 is checked against each `rdeact` frame.

## Provenance

- **1.14d `Game.exe`**: addresses from `re/exports/all.asm` and the Ghidra
  decompile; register arguments, call order (set before unset in §4.1,
  depth-first recursion in §4) and comparison signs from the
  disassembly (`0x0061B390`, `0x0066BBC0`, `0x0061A110`); handler tables
  `0x00744384`, `0x00744394` read from the file image; struct sizes from
  allocation calls; callers by scanning direct calls.
- **D2MOO** (1.10f) `DrlgActivate.cpp`, `DrlgDrlgRoom.cpp`
  (`DRLGROOM_AllocRoomEx`, `sub_6FD77BB0` = 1.14d `0x0066C370` as
  D2MOO notes, `DRLGROOM_SortRoomListByPosition`,
  `DRLGROOM_ReorderNearRoomList`, `sub_6FD77F00`), `D2Dungeon.cpp`
  (`DUNGEON_AllocRoom`, `DUNGEON_RemoveRoomFromAct`, client arrays,
  `D2Common_10081`): same rules. 1.14d differences: the warp-link
  exception is level 133 (D2MOO's `LEVEL_PANDEMONIUMRUN1` branch); the
  removal test calls `0x0066C0B0` first; the server never calls the client
  update (§4.6); handler tables at the addresses above.
- **Tiles and collision (§9, §10)**: 1.14d: `0x0066D820` (tile choice; draw sites read room +0x14/+0x18), `0x0066E9B0` (cell), `0x0066EC10` (grid walk), `0x0066DC50`/`0x0066DB20`/ `0x0066DDE0`/`0x0066DF40` (records, flags), `0x0066E580`/`0x0066E4C0`/ `0x0066E620`/`0x0066E740`/`0x0066E940` (linking; tables `0x006EF620`, `0x006EF578` read with rd.py), `0x0066E1C0`/`0x0066E260`/`0x0066E360` (warps), `0x0066D9E0` (doors), `0x0066D290`/`0x0066D440`/`0x0066D700`/ `0x0066D3B0`/`0x0066D410`/`0x0066D750` (anim), `0x0066EE40`/`0x0066EE70`/ `0x0066EEA0`/`0x0066EEE0`/`0x0066F050`/`0x0066F0B0`/`0x0066F1A0`/ `0x0066F240` (lifecycle, library), `0x0061B190`/`0x0061B730` (build sequence), `0x00604A40`/`0x00604AE0`/`0x0060D040`/`0x0060CFA0`/ `0x0060CEA0`/`0x0060CF00`/`0x0060A440` (DT1 library), `0x0064C4C0`/ `0x0064C580`/`0x0064C700`/`0x0064C790`/`0x0064C860`/`0x0064C900`/ `0x0064CA10` (collision). D2MOO `DrlgRoomTile.cpp`, `DrlgDrlgAnim.cpp`, `D2Collision.cpp` used as a map; 1.14d differences noted: per-cell function split from the loop, no draw on zero total rarity, fatal missing animation frames, level-133 exclusion of wall warp tiles, all three linked cases through one find-or-add routine. Measurements: `LvlTypes`, `LvlPrest`, `Levels` (patch_d2), Act 1 DT1s and Town DS1s from d2data.mpq. Recording `20261005-232125-rng.jsonl`: per-room counts, first-room vector reproduced by simulation (scratchpad `dt1/simtown.py`).
- **Recorded**: §Test vectors.
- **Client build timer (§4.6)**: disassembly of `0x0061B920` (u8 +0x98
  and +0x45C compares, the `sete`-based reset 5 / 7, list head
  `+0x278`, cursor `+0x460`), `0x00642A00` (DRLG flags bit 0),
  `0x0061B2D0` (reset after build), caller `0x0044C790` (single call
  at `0x0044C7E6`; every-13th free at `0x0044C7F0`–`0x0044C817`); the
  only writers of +0x45C / +0x460 in `all.asm` are these two functions.
- **Act callback (§5 rule 9)**: `0x00619954` (`call edi` on act +0x4C;
  no other indirect call through +0x4C in `all.asm`), `0x0061AF60`,
  `0x00475B40` (pushes `0x00475930`; single caller `0x0044E1B6` in the
  0x03 act load `0x0044E100`).
- **Entry identity (§9.3)**: `0x0060A440` (stride 0x60 over +0x110,
  count +0x10C) → `0x0060CFA0` (stores the header pointer);
  `0x0060D040` (lookup copies the stored pointers).
- **Units left after compression (§8 rules 4–5)**: `all.asm` scan for
  tests of unit +0xC4 bit 23 / +0xC8 bit 5 (`shr 0x17`, `0xC6` & 0x80,
  `shr 5`) and writers of +0xC4 0x400000 (`or 0x600000` only);
  `0x005433F0` disassembly with its table `0x00543504` read from the file
  image.
- **Flag 0x400000 setter (§8 rule 1)**: asm of `0x0061AED0` (null test, active room +0x10, `ret 8`) and `0x0061BAC0` (`edx` = clear; `and 0xFFBFFFFF` / `or 0x400000` on +0x28); `disasm.py xref`: `0x0061BAC0` has the single caller `0x0061AEE0`, `0x0061AED0` has 31 call sites. Requested by PC 2 (`world/quests-act1-rest.md` §9 item 12 links here).
- **Room free, units left (§8.2 rule 4)**: asm of `0x0061A840`
  (`0x0061A851`–`0x0061A87F` loop), `0x0064C450` (unit leaves room),
  `0x0064FC20` (dynamic path reset), `0x0064C370` (room-list remove);
  `0x006EB7C8` / `0x006EB7CC` read from `Game.exe` (both 0, only
  readers in `all.asm`). Flag name from D2MOO `Units.h` (hint only).

## Open questions

1. *Answered* (static, `disasm.py at`): set handler 0 forces status 0;
   set handler 2 tests tiles-loaded, preset-units-added and status > 2,
   forces 2 and runs set handler 1 when count[1] ≠ 0; unset handler 2
   is the recompute shared with unset 0 and 1 (§4 table). D2MOO's
   behaviour holds; handler 0 has no "status > 0" test of its own.
2. *Answered* (static): `0x0066C0B0` is the room → DRLG getter (via the
   level, three null-pointer fatal errors); the removal test passes its
   result to `0x00642A00` and raises fatal 0x2E7 for a client copy. It
   decides nothing else (§8 rule 1).
3. Run the queued checks C1–C4 and the full adjacency check (Test
   vectors).
4. Client array order (§7.1): which server code iterates a room's client
   array (message fan-out?) and whether address order can change an
   outcome; with one client (single player) it cannot.
   *Answered* (static, `all.asm`: every function reading both +0x48 and
   +0x78 of one register, then each read): besides add / remove / sort
   (`0x0061A660`, `0x0061A700`, `0x0061A5A0`) and their null-entry check
   `0x0061A550` (fatal 0x453), two readers: the membership test
   `0x005387F0` (is client C in unit U's room; order-free) and the
   room-change messages `0x00554670` (`sim/pathing.md` §9.8), which
   merge-walks the old and new rooms' arrays and relies on both being
   sorted by address. Each client gets either the removal or the add
   messages, so the address order changes only the order in which
   different clients' queues are written, never what one client
   receives. `0x0061A7E0` (array subset test) has no caller. No outcome
   depends on the order.
5. Maximum near-candidate count over all 1.14d levels (§3.1, 30 slots):
   measure from generated layouts once the type specs are implemented.
6. Unit-order open question 3 (inactive-unit compress/restore order) is
   unit-side (`0x005433F0`, `0x00542B40`; D2MOO `SUnitInactive.cpp`): it
   belongs to the unit or monster spec, not here.
7. Equal-key tile order within one DT1 (reverse file order from
   `0x0060CEA0`, §9.3) is not tested by the draw counts: record the entry
   returned by `0x0066D820` for seq 6822–6835 of the RNG recording.
8. Full linked-cell simulation of the 35 town rooms against the §9.9
   table (implementation check, after §3 and §9.6 are implemented).
9. `map-preview.md` OQ 3 (8 visible-flagged invisible tiles in
   `townN1.ds1`): the tile build does not hide them (§9.10); the cause is
   in the client draw path (Phase 6).
10. *Answered:* the door tables are transcribed in
    `drlg/preset-tables.tsv` (table `door`), rules in `drlg/preset.md`
    §11.
11. *Answered* (static): `0x0064C900` allocates the header, copies the
    8 coordinates, stores it at active room +0x20 (`0x0061A040`, call at
    `0x0064C95A`) and zeroes the masks before the room-list loop; each
    listed room's header is read through `0x0061A010` (active room
    +0x20). The new room's own header therefore exists when the loop
    reaches it; every other listed room is an active room, whose grid
    was made at its own creation (§10.2).
12. *Answered* (static, `all.asm` bit-test scan and `0x005433F0`
    exits): §8 rule 4 (readers: client `0x00480810` only; server units
    never have 0x400000) and rule 5 (items are the units compression
    leaves in the room). Open inside it: whether `0x005421A0` (the item
    save) unlinks the item some other way; a server memory read of a
    freed room's +0x74 after an item was dropped there settles it.
13. Merge corner case (§9.6 step 3): can R be a type-3 record with no
    successor in its chain (R +0x20 null) when the merged type is not 3?
    1.14d then writes through a null pointer. A dump of every link chain
    of the five acts' built rooms settles whether it occurs.
14. Animated shadow records (§9.7) read the grid before the shadow grid
    (floor layer 1). Does any 1.14d DS1 with lvlprest `Animate` have a
    shadow cell whose tile has material 0x100? Scan the Animate rows'
    DS1 files with their DT1s.
15. *Answered:* the client build timer (`impl-client-drlg` §3 Q2):
    §4.6 rules 1–8. Open inside it: a client recording that logs B, T,
    C and each timed build per client update (memory read of the client
    DRLG +0x98, +0x45C, +0x460) confirms the order on live data; the
    recording that built all 35 town rooms is consistent with it.
16. *Answered:* the act room callback (`impl-client-drlg` §3 Q4): §5
    rule 9 (`0x00475930`, the light-cache invalidation of
    `render/lighting.md` §6.4; not automap). Also settled: the "act's
    unset callback `[0x00744398]`" of `client/model.md` §9 rule 2 is
    entry 1 of the unset-handler table at `0x00744394` (§4, unset
    handler 1), not an act field.
17. *Answered:* DT1 roof height and height for the client draw
    (`impl-client-drlg` §3 Q7, `render/draw-order.md` OQ 12): §9.3
    "Entry identity". Test vector (synthetic): a DT1 with 3 tiles whose
    tile 2 has key (1, 0, 0), roof height 0, height −80, loaded into
    slot 0 → the lookup for (1, 0, 0) returns (slot 0's path, index 2)
    and the record reads roof height 0, height −80 from it.
18. *Answered* (`impl-drlg-act3-5` Q11): a built room's link list has no
    reader after maze generation other than the room free (§1, after
    the room flags table); dropping it changes nothing observable.
19. *Answered* (`impl-drlg-act3-5` Q12, door flag): the tile code sets
    record flag 0x20 through `0x0066DB20` → `0x0066D9E0` (§9.5.1, wall
    records); the door-unit call returns nothing, the flag is written
    on the record.
20. *Answered (2026-10-08)* (`fix-drlg-answers` Q2): the flag-rule
    re-runs of §9.6 step 3 pass this room (not N), the cell (wx, wy)
    and this room's v; a door R without flag 0x20 gets its unit and
    draw in this room (§9.6 step 3, "Arguments" bullet; `0x0066E740`,
    `0x0066DB20`, `0x0066D9E0`).
21. *Answered (2026-10-08)* (`impl-client-drlg-2` Q1): yes,
    `0x0061B7E0` writes status s into head s, so the status-2 head
    reads 2 and stays the cursor; with B = 1 on entry the walk then
    builds nothing that call (§4.6 rules 6, 9).
22. *Answered (2026-10-08)* (`impl-client-drlg-2` Q2): `[0x007A0498]`
    is zeroed at every game start and end (`0x0044E200`, `0x0044C890`),
    never by 0x03 or a level load (§4.6 rule 10).
23. *Answered (2026-10-08)* (`impl-client-drlg-2` Q3): a freed cursor
    room reads "not status 2" in d2rs; this equals 1.14d unless the
    freed block is reallocated with byte +0x44 = 2 before the read
    (§4.6 rule 11). Open inside it: a client recording that logs the
    cursor +0x460 and the byte at its +0x44 at each timed step across
    a level free (PC 2 recording list) shows whether reuse happens.
