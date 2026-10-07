# Spec: Simulation — Game tick

- **Status:** conformance-passing: `cargo test -p conformance --test
  tick_replay` replays `traces/sim/tick/sim-0006..0008` (11,105 ticks,
  48,316 timer runs, 446 list snapshots, 65,754 recorded inputs) through
  `d2_sim::tick::tick` with 0 mismatches (2026-10-06; host schedule §1:
  `d2-server` unit tests only). Every rule read from the
  1.14d `Game.exe` code and confirmed on the running game: `check_tick.py`
  replays `traces/raw/20261006-015554-tick.jsonl` (4,902 ticks of a
  hand-played single-player game, 16,704 timer runs, 7,674 schedules,
  9,645 cancels, 197 list snapshots) with 0 mismatches.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::tick` (tick steps, timer queue); the host
  schedule (§1) lives in `d2-server`
- **Related specs:** `sim/intents-events.md` (what the messages drained
  before a tick and flushed after it contain); `sim/unit-order.md` (the
  unit, room and client lists this spec iterates, GUIDs); `sim/rng.md`
  (the generator; draws happen inside the steps below, in this order);
  `sim/units.md` (who schedules each timer event and what it does;
  AI, skill, missile, object and item internals in their later specs).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 45–58 |
| Inputs | 59–66 |
| Outputs / state changes | 67–72 |
| Rules | 73–74 |
|   1. Tick rate and host schedule | 75–128 |
|   2. Frame counter | 129–142 |
|   3. Tick steps in order | 143–173 |
|   4. Room pass (step 3) | 174–209 |
|   5. Timer events (step 4) | 210–376 |
|   6. Client pass (step 5) | 377–434 |
|   7. Periodic steps, summary | 435–444 |
|   8. Wall-clock and host-only parts | 445–457 |
| Constants & data dependencies | 458–471 |
| Randomness | 472–479 |
| Edge cases & original bugs | 480–511 |
| Test vectors | 512–600 |
| Provenance | 601–629 |
| Open questions | 630–677 |
<!-- /index -->

## Summary

The server simulation advances in ticks ("frames") of 40 ms, 25 per
second. The host loop drains queued client messages, runs at most one
tick when 40 ms have passed, then flushes the server's messages. A tick
increments the game's frame counter and runs nine fixed steps in a fixed
order, plus five periodic steps keyed on the frame counter. Almost all
unit behaviour (movement, animation, AI, missiles, regeneration, state
expiry) runs inside one step: the timer-event queue, which executes events
by unit class (missiles, players, monsters, objects, items), each class
first its every-tick events, then the events due this frame. No step
reads wall-clock time for an outcome; wall-clock time only decides *when*
a tick runs (§1) and drives host-only bookkeeping (§8).

## Inputs

| Name | Type | Source |
|---|---|---|
| game state | units, rooms, acts, clients, timer queue | previous tick |
| client messages | byte messages | drained before the tick (§1; contents: `intents-events.md`) |
| tick trigger | — | the host schedule (§1); d2rs: the caller decides, never a clock inside `d2-sim` |

## Outputs / state changes

- Frame counter + 1; every state change made by the steps of §3.
- Server→client messages queued during the tick, flushed by the host
  after it (`intents-events.md`).

## Rules

### 1. Tick rate and host schedule

1. Tick length = `1000 / rate` ms, integer division, with rate = 25:
   **40 ms**. A static initializer (`0x006CAF10`, from the CRT
   initializer table at `0x006CC6F8`) stores 40 into `0x00883D60` from the
   rate global `0x00731014` (initial 25). The only other writer,
   `0x0052DF80` (set rate), has no callers.
2. Tick driver `0x0052FC20(catch_up)`: `now = timeGetTime() & 0x7FFFFFFF`;
   `last` (`0x00883D58`) is set to `now` on first use. If
   `now − last < 40` (signed) it runs nothing and returns 0. Otherwise
   `excess = now − last − 40`; with `catch_up ≠ 0` and `excess ≥ 40`,
   `excess = 40`; `last = now − excess`. It then runs one tick
   (`0x0052D870`) for every live game (slot table `0x00882D38`, 1024
   slots; 0 and −1 are empty), each under that game's lock, in slot
   order. Return value: a QueryPerformanceCounter delta, or 0 when no
   game ran. Exact arithmetic (`0x0052FC46`–`0x0052FC83`): only `now`
   is masked; `last` is never masked but is only ever written from the
   masked `now` (`now` on first use, `now − excess` after a tick), so it
   stays in 0 .. 2^31 − 1. First use is the test `last == 0` (no
   separate flag). `now − last` is a wrapping 32-bit subtraction compared
   **signed** with 40 (`jge`); the catch-up clamp compares `excess` with
   40 unsigned (`jb`), which is the same since `excess ≥ 0` there. Wrap
   behaviour: edge case 7.
3. Consequence of rule 2: one call runs at most one tick per game; after
   a stall the next call is due at once, so the host catches up by at
   most one extra tick, then drops the rest of the lag. No tick is ever
   run twice in one call and none is skipped inside the simulation: the
   frame counter always advances by exactly 1 per tick.
4. Single player (game type global `0x007A0610` = 0) runs the server
   inline in the client frame function `0x0044EFA0`: drain the server's
   incoming message queues (`0x0052CFE0`), call the tick driver with
   `catch_up = 1`, and only if a tick ran, flush the outgoing messages
   (`0x0052FD90(1, 0)`). With game type 1 the flush runs every frame
   (`0x0052FD90(0, 0)`). In single player the flush at `0x0044F162` is
   the only one per frame, and the client's receive step `0x0044C6E0`
   runs right after it in the same client frame (`intents-events.md`).
   The server-thread loop `0x0044CF20` (drain, tick, flush, sleep) serves
   hosted games (game types 6 and 8); `0x0052D870`'s other caller
   (`0x00564603`) is a game-server worker path single player does not
   use.
5. Messages that arrive between two ticks are therefore all processed,
   in arrival order, before the next tick's first step; a message never
   lands in the middle of a tick (single player). Their handlers run
   with the frame counter of the previous tick.
6. Per-game load ratio (game +0x1DBC): the driver writes 1024 when global
   `0x0073100C` ≠ 0, which it always is in 1.14d (initial 1, no writer).
   The ratio is read only by the game-info query `0x0052ED10` (where the
   same field is reported as uptime) and never affects an outcome.

d2rs: `d2-sim` exposes `tick(&mut Game)`; the server host owns the
40 ms schedule of rules 2–5 and the message drain/flush around each
tick. Determinism: the same messages applied before the same frame give
the same result regardless of wall-clock timing.

### 2. Frame counter

1. `game +0xA8` (D2MOO `dwGameFrame`), signed 32-bit. The tick's first
   action is `+= 1`, so the first tick of a game runs as frame 1
   (the counter starts at 0: the game record comes from a zeroed
   allocation, game creation `0x00530930` does not write it, and the
   recordings read 0 before the first tick).
2. Every periodic test (`% 20`, `% 12`, `% 11`, `% 1500`, `% 8192`) and
   the timer bucket (`% 64`) use **signed** remainder (`idiv`, or the
   equivalent `and 0x8000003F` sign fix-up). For a counter past
   2^31 − 1 the remainders turn negative; unreachable in practice
   (2^31 ticks ≈ 994 days), reproduce anyway.
3. Timer expiry frames are absolute frame numbers (§5).

### 3. Tick steps in order

`0x0052D870(game)` (D2MOO 1.10f `GAME_UpdateProgress`, 1.14d address
confirmed):

| # | Step | 1.14d | When | Does |
|---|---|---|---|---|
| 0 | frame | `0x0052D870` | always | frame += 1; then debug trap switch on game +0x1DC8 (1, 2: write to an invalid address; 3: fatal error; 4: huge allocation), never set in normal play |
| 1 | environment | `0x0052D7B0` | always | per act 0..4: advance the act's day/night cycle (`0x0061C040`); when the cycle index changes, every client (client-list order) gets its player's items refreshed (`0x0055FDE0`) and, if in-game (state 4) in that act, message 0x53 |
| 2 | frame-rate stats | `0x0052D720` | always | wall-clock statistics only (§8) |
| 3 | rooms | `0x0052D160` | always | room pass (§4) |
| 4 | **timer events** | `0x005414D0` | always | the timer-event queue (§5): all unit behaviour |
| 5 | clients | `0x0052D440` | always | client pass (§6) |
| 6 | room update queues | `0x0053B000` | always | per act 0..4 with pending updates (act +0x00 ≠ 0): for each active room (act room-list order), for each unit in the room's update queue (queue order, `sim/unit-order.md` §6) call `0x00553220`, then clear the queue (`0x0064C160`); reset the act flag. Then `0x0053FAE0` (clears an arena flag bit) |
| 7 | removal records | `0x0053A820` | always | per act 0..4 with pending room deletions (act +0x58 ≠ 0): for each active room (act room-list order) free its unit-removal records (`0x0061A2C0`; D2MOO `LEVEL_FreeDrlgDeletes`, the records step 5 turned into removal messages); reset the flag |
| 8 | quests | `0x00543E10` | frame % 20 == 0 | quest updater |
| 9 | room deactivation | `0x0052D240` | frame % 12 == 0 | per act 0..4, each active room (next saved before the body): if the room's inactivity counter (`0x0061A790`, `drlg/rooms.md` §7) > 10 and the act allows it (`0x0061A3F0`, `drlg/rooms.md` §8), compress every unit in the room to inactive storage (`0x005433F0`, room-list order, next saved first) and remove the room (`0x0061A910`) |
| 10 | free inactive rooms | `0x0061AA20` | frame % 11 == 0 | per act 0..4 (D2MOO `DUNGEON_UpdateAndFreeInactiveRooms`) |
| 11 | expired items | `0x0052D310` | frame % 1500 == 0 | per act 0..4: delete inactive items (`0x00558B90`), then expired inactive-unit item nodes (`0x00542AC0`) |

Rules:

1. Steps run exactly in this order every tick; the periodic steps 8–11
   run in this order when several are due (e.g. frame 660: % 12 and
   % 11 and % 20).
2. There is no separate movement, collision, AI or state-expiry step.
   Those run as timer events in step 4 (§5.7) or as consequences of
   them; collision is tested inside unit movement.
3. Acts are always visited 0..4 by index; an act slot that is null is
   skipped.

### 4. Room pass (step 3)

For each act 0..4 whose pending-room flag (act +0x54) is set (written 1
when a room is activated, `0x00619890`): for each active room in the
act's room list (`sim/unit-order.md` §4: newest-activated first, next at
room +0x7C):

1. `0x0054F060(game, room)`: ambient spawns (draws from the active room
   seed, `rng.md` §7). Runs for every room in the list, populated or not.
2. If the room's populated bit (room +0x34 bit 0) is clear: preset units
   (`0x005559A0`), restore inactive units (`0x00542B40`), object
   population (`0x00552610`), monster population (`0x0054EC90`, draws from
   the game seed), set bit 0, then mark the room's units active
   (`0x0061A510(room, 1)`, room +0x34 bit 1).
3. Else, if bit 1 is clear: restore inactive units (`0x00542B40`), then
   mark (`0x0061A510(room, 1)`). If bit 1 is set: nothing.
4. After the act's list: clear act +0x54.

A room activated during a tick (e.g. by a player moving in step 4) is
populated at the start of the **next** tick's step 3, unless one of the
off-tick paths below populates it at once.

5. **Off-tick population** `0x0052D0F0(game, room)`: the same body as
   rules 1–3 for one room (ambient spawns `0x0054F060` first, then the
   bit-0 / bit-1 branches), without the act flag test and without
   clearing act +0x54. Its callers and triggers (portal destinations,
   portal objects, the A2Q6 arrival) are owned by
   `monsters/population.md` §1 r2; they run inside whichever step runs
   the caller (timer events, message handling before the tick), so that
   room's population draws happen there, not in step 3. The room stays
   in the act list with act +0x54 set, so the next step 3 visits it
   again: one more `0x0054F060` (one more room-seed draw), then nothing
   (bits 0 and 1 already set). Since activation
prepends (`unit-order.md` §4), rooms activated together are populated
newest first, and their game-seed draws happen in that order.

### 5. Timer events (step 4)

#### 5.1 Queue structure

The queue is a record at game +0xB8 (D2MOO `D2EventTimerQueueStrc`):

| Offset | Field |
|---|---|
| +0x000 | current bucket index (frame % 64) |
| +0x004 + 0x100·c + 4·b | head of bucket b (0..63) of class c |
| +0x504 + 0x100·c + 4·b | tail of bucket b of class c |
| +0xA04 + 4·c | head of class c's every-tick list ("infinite") |
| +0xA18 | iteration cursor (next timer to visit) |
| +0xA1C | first timer slab (free list at slab +0x7080) |

Class c from the unit type (table `0x006E0B9C`): player 0, monster 1,
missile 2, object 3, item 4 (unit type 5, tile: −1, never used). The
table has 6 entries; a timer with no unit has type 6, which reads past
the table (edge case 3).

Timer record (D2MOO `D2EventTimerStrc`; 1.14d offsets):

| Offset | Field |
|---|---|
| +0x00 | event type (u8) |
| +0x02 | flags (u16): 1 executing, 2 free, 4 every-tick, 8 delete after execution |
| +0x04 | expire frame (i32; −1 = every tick) |
| +0x08 | unit |
| +0x0C | unit GUID (−1 without a unit) |
| +0x10 | unit type (6 without a unit) |
| +0x14, +0x18 | event arguments (D2MOO `dwEventCustomId`, `dwEventCustomParam`) |
| +0x1C, +0x20 | next / previous in its bucket or every-tick list |
| +0x24, +0x28 | next / previous in the unit's own timer list |
| +0x2C | callback (null = the class's default handler, §5.6) |

#### 5.2 Scheduling a timed event

`0x005416B0` (type, expire, game, unit, callback, arg1, arg2); public
form `0x005417D0` (no callback) and `0x00541800` (with callback):

1. Event type ≥ 15: nothing happens.
2. expire = −1: handled as an every-tick event (§5.3) through
   `0x005415E0` with the same type, unit and arguments but a **null
   callback** (`0x005416D5` pushes 0): the caller's callback is lost and
   the class default handler (§5.6) runs instead. Rule 4's state-54
   check is skipped on this path. (`units.md` §5 r4 relies on this rule.)
3. If expire ≤ current frame: expire = frame + 1.
4. If the unit is a monster (type 1), the event is AI-think (2) and the
   monster has state 54 (D2MOO `STATE_UNINTERRUPTABLE`): `0x005544B0
   (unit, 0)` runs first (monster spec).
5. A timer is taken from the slab free list and linked at the **head**
   of the unit's own timer list.
6. It is appended at the **tail** of bucket `expire % 64` of the unit's
   class; if the bucket was empty it also becomes the head.

So timers in one bucket keep insertion order, and a bucket holds timers
of different expiry frames that share `expire % 64`.

#### 5.3 Scheduling an every-tick event

`0x005415E0` (wrappers `0x00541650`, `0x00541680`): type ≥ 15 ignored;
flags |= 4, expire = −1; the timer is linked at the head of the unit's
timer list and **prepended** to its class's every-tick list. The only
callers in 1.14d (all through `0x00541650`):

| Caller | Event | Before scheduling |
|---|---|---|
| `0x00553F00` (D2MOO `sub_6FCBD3A0`: start per-tick mode updates of a player or monster) | type 0, args 0, 0 | cancels the unit's type-0 events (`0x00540E60(type 0, any arg)`) |
| `0x0059F8A0` (missile setup) | type 0, args 0, 0 | cancels all the missile's timers (`0x00540F30`) |
| `0x005D18E0` (imp possess helper: acts only when the first unit is monster class 492 `imp1`; only caller `0x005D1BCD` in srvdo 129 Imp Teleport `0x005D1AB0`, `skills/bodies-4.md` §3.24 "Possess"; not a druid skill as D2MOO's file placement suggested) | type 5, args skill id, level | cancels the unit's type-5 events with that skill id |

#### 5.4 Cancelling

`0x00540CD0` (timer):

1. If the timer is executing (flag 1): set flag 8 and return; the runner
   frees it after the callback.
2. If it is the iteration cursor's target, the cursor moves to its next.
3. Unlink from its bucket or every-tick list (fixing head and tail),
   and from the unit's timer list.
4. Push it on the slab free list; flags = 2.

Helpers: delete a unit's events of one type (and optional argument), of
one type and callback, or all of them; the unit-removal path cancels all
of a unit's timers. Cancelling never runs the event.

#### 5.5 Running the queue

`0x005414D0(game)`:

1. bucket = frame % 64 (signed, §2.2), stored at queue +0x000.
2. For each class in the order **missile (2), player (0), monster (1),
   object (3), item (4)**:
   1. every-tick list of the class, from its head (newest scheduled
      first);
   2. bucket `bucket` of the class, from its head (oldest scheduled
      first).
3. Visiting a list: cursor = first timer. Loop while a timer is
   current: cursor = its next; then
   - every-tick list: always run it;
   - bucket: run it only if its expire == frame; otherwise leave it.
   Running: flags |= 1; call the callback, or the class default handler
   (§5.6) with (game, unit, type, arg1, arg2); flags &= ~1. A bucket
   timer is then always freed (§5.4). An every-tick timer is freed only
   if flag 8 was set during the call.
   Continue with the cursor (not with the old timer's next field).
4. Runners: missile `0x00541240`, player `0x00540F60`, monster
   `0x00541060`, object `0x00541160`, item `0x00541320`; each takes
   (list head, due-only flag).

Consequences (all follow from the rules; reproduce them):

1. An event scheduled during the queue run never runs in the same tick:
   timed events get expire ≥ frame + 1; an expire of frame + 64·k lands in
   the bucket being visited, after the cursor, and is skipped by the
   expire test; every-tick events are prepended behind the cursor.
2. A timer cancelled before the cursor reaches it never runs this tick;
   cancelling the cursor's target advances the cursor.
3. A unit freed during the run (e.g. a missile whose handler returns 2,
   `0x00555600`) has its timers cancelled; its executing timer is freed
   after the callback returns.
4. Within a class, the order of due events is the order they were
   scheduled, across all units of that class; the order of every-tick
   events is the reverse of scheduling. Classes never interleave.

#### 5.6 Default handlers per class

| Class | Handler | Dispatch |
|---|---|---|
| missile | `0x005ADCC0` → `0x005ADBB0` | ignores the event type: runs the missile's `missiles.txt` server-do function (index from the record, table `0x0073C768`) unless the missile's room tests fail; result 2 removes the missile (`0x00555600`) |
| player | `0x00581220` | type ≤ 14: table `0x006E1810` (15 entries, null = nothing) |
| monster | `0x005A7F80` | type ≤ 14: table `0x006E2490`; for types 0, 1, 2, 6, 7, 9, 10, 11, 13, 14 the event is dropped when the monster has state 1 (D2MOO `STATE_FREEZE`) and `0x005541B0` is false; types 3, 4, 5, 8, 12 always dispatch |
| object | `0x00586AD0` | table `0x006E19B0` by type (no null check) |
| item | `0x00562DA0` | table `0x006E117C` by type (no null check) |

The monster gate (`0x005A7F80`; jump table `0x005A7FE0` indexed by byte
table `0x005A7FE8` for types 3–12) only skips the call: the runner
(`0x00541060`) then treats the timer as run. A gated **due** timer
(e.g. an AI think, type 2) is freed like any run bucket timer
(`0x005410CF`) and nothing reschedules it: neither the dispatcher nor the
runner schedules anything (who re-thinks after a freeze:
`monsters/ai.md` §1.1). A gated **every-tick** timer (type 0) stays in
its list and is simply skipped each tick while the gate holds, then
dispatches again.

Event type names (D2MOO, 1.10f): 0 MODECHANGE, 1 ENDANIM, 2 AITHINK,
3 STATREGEN, 4 TRAP, 5 ACTIVESTATE, 6 FREEHOVER, 7 MONUMOD / QUESTFN,
8 PERIODICSKILLS, 9 PERIODICSTATS, 10 AIRESET, 11 DELAYEDPORTAL,
12 REMOVESTATE, 13 UPDATETRADE, 14 REMOVESKILLCOOLDOWN. The handler of
each (class, type) and what it does: `units.md` §5–§6 and
`unit-handlers.tsv`.

#### 5.7 Where the usual "update" work happens

Owned by `units.md`; confirmed on the recordings by `check_units.py`
(rules U1–U10):

| Work | Mechanism | Owner |
|---|---|---|
| walking, running, knockback (players, monsters) | every-tick event 0 | `units.md` §4.4–§4.6 |
| animation action frames, mode end | timed events 0 and event 1 from the animation | `units.md` §4.2 |
| missile flight, collision, hits, expiry | the missile's every-tick event 0 | `units.md` §6.3 |
| monster AI | event 2 | `units.md` §4.6, §6.2 |
| life regeneration | event 3 (players every frame) | `units.md` §6.1, §6.2 |
| object and item timers | events 0–11 (objects), 3 and 12 (items) | `units.md` §6.4, §6.5 |
| state expiry, periodic skills and stats | events 5, 8, 9, 12 | `sim/stat-lists.md` (scheduling: `units.md` §6) |

### 6. Client pass (step 5)

`0x0052D440(game)`:

1. Asserts the game's arena record (`0x0053FCA0`).
2. Heartbeat (`0x0052D350`): host-only, wall-clock (§8).
3. If frame % 8192 == 0 and no client was dropped by the heartbeat: for
   each client, save its character (`0x00532400`) and report it to the
   host callbacks (`0x0052CA10`). Host-only (§8).
4. For each client in client-list order (`unit-order.md` §7; next saved
   before the body), by client state (client +0x04):
   - 3 (joining): per-client update (`0x005380D0`); when the client's room
     is ready (`0x0061A460`): message 4, state = 4, inventory refresh
     (`0x0055DF00`), then the join sequence (`0x0052C410`,
     `0x0055B620`, host callback, player-join message 0x5A to all).
   - 4 (in game): per-client update `0x005380D0`.
   - 5 (changing act): per-client update; when the room is ready:
     message 4, state = 4, inventory refresh.
5. Per-client update `0x005380D0`: removal messages for units deleted in
   the adjacent rooms (`0x0053A770`); unit update messages for the
   adjacent rooms' update queues (`0x0053A620`, `unit-order.md` §6);
   player stat-change messages (`0x006258D0`); when the player's flag-ex
   +0xC8 bit 21 is set, inventory refresh (`0x0055DF00`) then
   `0x0055F4F0` (both skipped otherwise, `0x0053812F`–`0x00538152`;
   corrected 2026-10-08, `0x0055F4F0` read as unconditional before);
   client counter +0x1B0 += 1;
   if the player's room (`0x00620BB0`) differs from the client's: when
   the two rooms' level ids (`0x0061A1B0`) differ, from := the client
   room's level, to := the player room's level, quest event 3
   CHANGEDLEVEL `0x00543B90`(game, from, to, player) (`world/quests.md` §4.1)
   then the town-leave refresh `0x00537340`(game, player, from, to)
   (`world/vendors.md` §6 rule 1); then, levels equal or not, the room
   switch (`0x00537B50`, new room) (`0x0053815E`–`0x0053819A`; the
   level-change calls and their argument order confirmed 2026-10-08,
   impl-pc1-s5). This
   step has no act logic: an act change happens only through the warp
   (`0x0053AEC0`, `world/waypoints.md`), which calls `0x00537340` and
   the act change `0x0053ACC0` itself; arena sync (`0x0053FC20`); queue the player for
   update (`0x0064C040`).
6. **Room ready** `0x0061A460(R)`, R = the client's room (client
   +0x1B4, written by the room switch, `intents-events.md` §7.8; null R
   is fatal assert 0x3EF): ready iff R's adjacency count (active room
   +0x24) equals its DRLG room's rooms-near count (DRLG room +0x2C,
   `0x0066BCF0`; `drlg/rooms.md` §1) **and** every entry of R's
   adjacency array (+0x00, array order) has the populated bit (active
   room +0x34 bit 0, set by §4 rule 2). A count of 0 is ready. So a
   client is ready once every neighbour of its room exists as an active
   room and has been populated. At a single-player join the client
   enters state 3 inside the C→S 0x6B handling (`intents-events.md`
   §8.2), the join activates the town rooms, step 3 of the **next** tick
   populates them, and step 5 of that same tick sends 0x04 (rule 4, after
   the per-client update's unit messages): recorded frame 2 of both
   recordings (`-022633` seq 219, after the units' 0xAC / 0x51 / 0x0E).
   Rule 4's message 4 is `0x0053B320(client, 4)` (1 byte; at
   `0x0052D514` for state 3, `0x0052D5A2` for state 5).

Message contents: `intents-events.md`.

### 7. Periodic steps, summary

| Period (frames) | Seconds | Step |
|---|---|---|
| 11 | 0.44 | free inactive rooms |
| 12 | 0.48 | room deactivation |
| 20 | 0.8 | quests |
| 1500 | 60 | expired items |
| 8192 | 327.68 | character save (host) |

### 8. Wall-clock and host-only parts

These read the clock or touch the host, never feed an outcome, and stay
out of `d2-sim`:

| Part | 1.14d | Rule |
|---|---|---|
| tick schedule | `0x0052FC20` | §1 |
| frame-rate stats | `0x0052D720` | every ≥ 1000 ms of `GetTickCount`: game +0xAC = frames·1000/elapsed, global `0x00731010`; frame count +0xB0; peak memory over 10 s |
| heartbeat | `0x0052D350` | only when global `0x00730FFC` ≠ 0 (initial 1) and host callbacks `0x00883D50` exist: drop a client silent > 45 s, or hardcore and silent > 10 s with > 10 pings (save first) |
| character save | `0x0052D440`, `0x0052CA10` | frame % 8192 == 0 (§6.3) |
| game info | `0x0052ED10` | reads the load-ratio field as uptime |

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| ticks per second | 25 | `0x00731014` |
| tick length | 40 ms | `0x00883D60` = 1000 / 25 |
| timer buckets per class | 64 | `frame % 64` |
| timer classes | 5 (missile, player, monster, object, item run order) | §5.5 |
| event types | 15 (0–14) | ≥ 15 ignored |
| periods | 11, 12, 20, 1500, 8192 frames | §7 |
| room deactivation threshold | counter > 10 | step 9 |

Data read by the tick itself: none (handlers read their tables).

## Randomness

The tick draws nothing itself. Draw order = step order: step 3 (ambient
spawns per room: active room seed; room population: game seed, object
seeds), then step 4 (event handlers: unit seeds, item seeds, missile
seeds), then step 5 (join, level change), then periodic steps. Within a
step the order is the list order this spec and `unit-order.md` define.

## Edge cases & original bugs

1. Signed frame remainders (§2.2).
2. Bucket aliasing: a timer for frame f + 64·k sits in the same bucket as
   one for f and is skipped until its frame (§5.5). Order inside the
   bucket is still scheduling order.
3. A timer without a unit (unit type 6) indexes the 6-entry class table
   at `0x006E0B9C` out of bounds (reads the next 4 bytes of `.rdata`,
   1095195694): its bucket address is garbage. No 1.14d caller passing a
   null unit was found; open question 2.
4. An every-tick event scheduled inside the queue run is not visited this
   tick (prepended behind the cursor); one scheduled in steps 1–3 runs in
   this tick's step 4.
5. An event "for this frame" scheduled during step 4 is moved to
   frame + 1 (§5.2 rule 3); it never runs in the same tick.
6. Catch-up is limited to one extra tick (§1.3): the simulation runs
   slower than real time under load instead of skipping frames.
7. 31-bit wrap of the tick clock (§1.2, `0x0052FC20`): `now` =
   `timeGetTime() & 0x7FFFFFFF` returns to 0 every 2^31 ms (≈ 24.86
   days of Windows uptime). After the wrap `now − last` is negative (both
   operands are in 0 .. 2^31 − 1, so the signed difference never
   overflows) and no tick runs until `now ≥ last + 40` again. If `last`
   ended in 2^31 − 40 .. 2^31 − 1 (the usual case when the driver is
   called every frame: every pre-wrap tick that was possible ran), that
   never happens and the games stop ticking for good; otherwise they stall
   for ≈ 24.86 days until `now` climbs back past `last + 40`. Reproduced
   (the host is not `d2-sim`; `d2-server` keeps the literal rule).
8. `last == 0` is the first-use test (§1.2): if the masked clock reads
   exactly 0 on first use, `last` stays 0 and the next call initialises
   it again. After a tick `last ≥ 40` (it only grows by ≥ 40 from a
   non-negative value), so the re-initialisation cannot happen later.

## Test vectors

Synthetic (from the rules; CI-safe):

| Input | Expected | Source |
|---|---|---|
| rate 25 | tick length 40 ms | §1.1 |
| driver: last = 1000, now = 1039 | no tick, last stays 1000 | §1.2 |
| driver: last = 1000, now = 1040, catch-up 1 | tick, last = 1040 | §1.2 |
| driver: last = 1000, now = 1150, catch-up 1 | tick, excess 110 → 40, last = 1110 (next call at ≥ 1150 ticks again) | §1.2 |
| driver: last = 1000, now = 1150, catch-up 0 | tick, last = 1040 | §1.2 |
| driver: last = 0 (first use), now = 5000 | last = 5000; 0 < 40: no tick | §1.2 |
| driver: last = 2147483620, now = 5 (clock wrapped) | 5 − 2147483620 = −2147483615 < 40: no tick, last unchanged; no later `now` < 2^31 ticks | edge case 7 |
| driver: last = 2147483000, now = 100 (wrapped) | no tick; ticks again first at now = 2147483040 | edge case 7 |
| frame 0 → tick | frame 1; bucket 1 | §2, §5.5 |
| frames 660 | steps 8 (660 % 20 = 0), 9 (% 12 = 0), 10 (% 11 = 0) run; 11 not | §3 |
| frame 1500 | step 11 and step 8 (1500 % 20 = 0) run; 9 (1500 % 12 = 0) runs; 10 (1500 % 11 = 4) not | §3 |
| frame −1 (wrapped) | bucket −1 % 64 = −1 (signed) | §2.2 |
| schedule at frame 10: A(expire 12), B(expire 76), C(expire 12), all monsters | bucket 12 = [A, B, C]; frame 12 runs A, C; frame 76 runs B | §5.2, §5.5 |
| schedule at frame 10: expire 5 | expire 11 | §5.2 rule 3 |
| every-tick events scheduled X then Y (one class) | run order Y, X each tick | §5.3 |
| frame 12, due: player P1, monster M1, missile S1; every-tick: missile S2 | order S2, S1, P1, M1 | §5.5 |
| during M1's event at frame 12, schedule M2 with expire 12 | M2 expire 13, runs at frame 13 | §5.2, §5.5 |
| during M1's event, cancel M3 (due 12, later in bucket) | M3 never runs | §5.4 |
| event type 15 | not scheduled | §5.2 |

Recorded (`record_tick.py`, 2026-10-06, `Game.exe -w -ns`, existing
character, Normal, Act 1 town and field, played by hand; raw files
gitignored):

| Recording | Ticks | Runs (every-tick / due) | Checked |
|---|---|---|---|
| `20261006-015554-tick.jsonl` | frames 1–4902 | 10,020 / 6,684 (players, monsters, a few missiles, objects, items) | `check_tick.py`: 0 errors; step order and every periodic step (incl. frames 1500, 3000, 4500); 197 snapshots |
| `20261006-022304-tick.jsonl` (combat with missiles) | frames 1–4632 | 15,154 / 9,740 (incl. 1,580 missile runs) | `check_tick.py`: 0 errors; 186 snapshots |
| `20261006-021854-tick.jsonl` (melee) | frames 1–1572 | 4,413 / 2,305 | 0 errors; 63 snapshots |
| smoke run (15 s, not kept) | frames 1–185 | 401 | 0 errors |

`check_tick.py --perturb-ex N` (swapped runs at frames 4, 242, 2665) and
`--perturb-snap N` are each reported at exactly the changed record (M08);
`--selftest` checks the checker on a hand-built recording of this
table's vectors and fails when a rule in the checker is mutated.
Coverage gaps: no unit-less timer, no hosted game.

Comparison (exact): for each tick of a recording,
the frame number, the step markers, and the ordered list of executed
timers `(class, list: every-tick | due, event type, unit type, unit GUID,
expire, arg1, arg2)` equal the ones `d2-sim` produces from the same start
state and the same messages. Until `d2-sim` has units, the recorded run
is checked against a model of §5 (`tools/trace-recorder/check_tick.py`:
it replays every recorded schedule and cancel in order and must predict
every recorded execution and nothing else).

**Trace sim/tick (format 1).** `traces/sim/tick/sim-0006`, `-0007`,
`-0008` are the three recordings above (frames 1–4901, 1–1572,
1–4632; a tick cut by the time limit is dropped), written by
`tools/trace-recorder/convert_tick.py`. CI runs `convert_tick.py --check
traces/sim/tick/*.json`: it rebuilds a recording from each trace, replays
it through the `check_tick.py` model (0 errors on all three) and converts
it back to the same trace; `--perturb-run N` and `--perturb-lists N` are
reported at exactly the changed event (M08). `setup`: `start_frame` 0,
`frames` (complete ticks), `source`, `game_args`, `snap_every`. Every
event has `tick` = the frame it belongs to and `data.seq` (one count
1..N over `inputs` and `expected` together: the recorded order) and
`data.step`: `pre` (before step 1: message handling, and the snapshot)
or the §3 step it happened in (`env`, `rooms`, `events`, `clients`,
`updq`, `dels`, `quests`, `deact`, `inactive`, `items`). Ids: unit =
`[type, GUID]`; timer = its schedule number (1, 2, … in `timer_set`
order); room = `R<n>`, n = activation number; act = index 0–4; client =
join number.

| Kind | Array | `data` |
|---|---|---|
| `timer_set` | inputs | `timer`, `type`, `unit`, `req` (requested expire; −1 = every-tick, §5.3), `expire` (after §5.2 rule 3), `a1`, `a2`; `cb` (1.14d callback address, informational) only when not null |
| `timer_cancel` | inputs | `timer`, `deferred` (cancelled while executing, §5.4 rule 1) |
| `timer_run` | expected | `timer`; class, list, type, unit, expire and args are those of its `timer_set` (the converter checks every recorded run against them) |
| `hash_add`, `hash_remove` | inputs | `unit` (+ `class_id` on add); `unit-order.md` §2 |
| `room_add`, `room_remove` | inputs | `unit` (+ `room` on add); §5 |
| `queue_add`, `queue_remove` | inputs | `unit` (+ `room` on add); §6 |
| `queue_clear` | inputs | `room`: a clear outside step 6 (none recorded). Step 6's clears are not stored: they are exactly §3 step 6 (acts with the pending-update flag, set by `queue_add`, in act order; every active room in list order), which held in all 11,105 recorded ticks |
| `room_activate`, `room_deactivate` | inputs | `room`, `act`; §4 |
| `lists` | expected | state at the start of the tick (step `pre`), at frame 1 and every `snap_every` frames: `hash` {type: [[bucket, [GUID…]]]}, `tiles` [GUID], `acts` (5 × null or [{`room`, `units`, `queue`, `adj`}], `adj` = room ids, `?` for an inactive one), `clients` [id] |

Replay (timer queue and lists without unit behaviour): walk both arrays
merged by `seq`; apply each input; at the `events` step, drive the §5.5
iterator: before each `timer_run`, apply the inputs with a smaller `seq`,
then the iterator's next timer must be that run's; after the tick's last
`events` input the iterator must be exhausted. Each `lists` event must
equal the implementation's lists (`unit-order.md`, Test vectors).

## Provenance

- **1.14d `Game.exe`** (SHA-256 `631066c1…adaaf`), image base `0x400000`.
  Every address in this spec was read from the disassembly (capstone over
  the Ghidra function list `re/exports/functions.tsv`) and the Ghidra
  decompile; register arguments were taken from the disassembly because
  the decompile drops them. Data tables (`0x006E0B9C`, `0x006E10E0`,
  `0x006E1810`, `0x006E2490`) and initial values (`0x00731004` = −1,
  `0x0073100C` = 1, `0x00731010` = 25, `0x00731014` = 25,
  `0x00730FFC` = 1) were read from the file image. Writers of the
  globals were found by scanning `.text` for every rel32 call and every
  4-byte reference to the address.
- **D2MOO** (1.10f) `D2Game/src/GAME/Game.cpp` (`GAME_UpdateProgress`,
  `GAME_UpdateGamesProgress`, `D2GAME_UpdateAllClients`), `Event.cpp`,
  `Level.cpp`, `Clients.cpp`: same step order, same timer queue, same
  class order and list disciplines. Each rule above was re-read in the
  1.14d code; differences: 1.14d calls `0x0055F4F0` in the per-client
  update (no 1.10f counterpart named), and the 1.14d dispatcher tables
  are at the addresses given. D2MOO's comment "1.14d: Game.0x52D870" for
  `GAME_UpdateProgress` is correct.
- **Recorded**: `tools/trace-recorder/record_tick.py` hooks the tick
  entry, the eleven step call sites, the ten timer-run sites, schedule
  (entry and after allocation), cancel, and the list primitives of
  `unit-order.md`; `check_tick.py` confirms §2, §3 and §5 (Test vectors).
- §6 rule 6 (server-join session, 2026-10-07): `0x0061A460`,
  `0x0066BCF0`, the client pass `0x0052D440` (`0x0052D503`–`0x0052D520`,
  `0x0052D565`–`0x0052D5AE`); 0x04 position checked on
  `20261006-022633-packets.jsonl` seq 219 and `-015956` frame 2.

## Open questions

1. Hosted-game recording: confirm §5 and the client pass with more than
   one client (missiles confirmed: 1,580 runs, 2026-10-06).
2. Timers without a unit (edge case 3): does any 1.14d path schedule one?
   Search callers of `0x005417D0` / `0x00541800` passing unit 0. None in
   the recordings.
   *Partly answered* (static): `0x005416B0` has only the two wrappers as
   callers; the unit is EDX at the wrapper call. Of the 252 call sites in
   `all.asm` (251 of `0x005417D0`, `0x00555046` of `0x00541800`) none
   loads a constant 0 into EDX. The one site where the unit register may
   be null, `0x005A3EDA` in `0x005A3E70` (type 7, frame + 4, taken when
   its unit argument is null), is umod 33's mode-1 callback (table
   `0x0073C0B8` entry 199, `monsters/init.md` §22), which the dispatcher
   only calls with the monster itself. Open: a register-held unit that
   is null at run time at one of the other sites (a breakpoint on
   `0x005416B0` with EDX = 0 over a long run settles it).
   Static narrowing (2026-10-07, the 266 sites `disasm.py xref` finds
   for both wrappers, 0x60 bytes before each call): 70 load EDX from a
   register that the preceding code dereferences or tests; 65 load it
   from memory (mostly a record's +4 unit field); 126 from a register
   with no such local evidence; 5 set EDX before the window. No local
   read proves the other 196 non-null, so the breakpoint stays the
   check (PC 2 list, `docs/HANDOFF.md` §7).
3. Settled for the observed combinations by `units.md` (U1–U10 on the
   three recordings); per-site confirmation needs a `record_tick.py`
   0.2.0 recording (`units.md` open question 1).
4. Answered: the inactivity counter (`0x0061A790`) is owned by
   `drlg/rooms.md` §7 r2 (0 while the room has a client, else +1 per
   step-9 pass; its only caller is step 9).
5. Does the character save every 8192 frames write the `.d2s` in single
   player (host callbacks `0x00883D50` absent)? Observe a save timestamp.
   *Answered* (static): yes. `0x0052D440` runs `0x0052CA10` on frame
   % 8192 = 0 (no heartbeat drop); it calls the save `0x00532400` for
   every client whatever the host callbacks (only the report after it
   needs `[0x00883D50]`). `0x00532400` branches on game +0x6A: 1 or 2 →
   `0x00532340` (save sent through the client code); otherwise, with no
   host callbacks → `0x00532240`, which builds the save (`0x00569AD0`,
   0x2000-byte buffer) and writes it with `fopen(path, "wb")` /
   `fwrite` / `fclose`, path `"%s%s.d2s"` (save directory + name;
   failure logs "Unable to open player save file %s"); with host
   callbacks → `0x00531EB0`. Single player is game type 3 with no host
   callbacks, so the file is written. `0x00532240` and `0x00531EB0`
   return without writing while `[0x007310CC]` = 0 (`-nosave`,
   `tools/original-hooks.md` §5.1; initial 1).
6. *Answered* (static): `0x0055F4F0` is an empty function (`ret 4`, 3
   bytes; 8 callers). It does nothing in 1.14d.
