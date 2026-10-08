# Spec: Flows — One server frame and tick (whole-game order)

- **Status:** draft: a flow spec, the top level of the analysis hierarchy
  (function → module → system → seams → whole-game flows). It owns no
  rule of its own: every step names the owner spec and 1.14d address
  that decides it, and this file only fixes how the owners' steps chain
  into one server frame. Audit of the code against it:
  `docs/handoff/q-tick-flow.md` (2026-10-08).
- **Target version:** 1.14d
- **Crate/module:** `d2-server::host` (frame: drain, tick driver,
  flush); `d2-sim::tick` (the tick steps)
- **Related specs:** `sim/tick.md` (owner of the tick and host
  schedule); `sim/intents-events.md` (drain, dispatch, buffers, flush,
  §7 unit update messages); `sim/unit-order.md` (list orders);
  `sim/units.md` (timer handlers); `monsters/population.md`;
  `drlg/rooms.md`; `flows/client-frame.md` (the client half of the same
  single-player loop pass).

## Summary

One single-player server frame is three phases run from the client loop
pass: drain every queued client message and dispatch it (outside any
tick), run the tick driver (at most one tick), then flush the per-client
buffers only if a tick ran. A tick is a fixed chain: frame counter,
environment, frame-rate stats, room pass (population), the timer-event
queue (all unit behaviour: movement, animation, AI, missiles, regen,
state expiry, in class order missile, player, monster, object, item),
the client pass (per-client unit updates and messages, room switches,
join and act-change completion), room update queues, removal records,
then the periodic steps. This file is the checklist an implementation
of the server frame is audited against.

## Inputs

| Name | Type | Source |
|---|---|---|
| queued client messages | byte messages in three queues | client send path (`sim/intents-events.md` §2.1) |
| game state | units, rooms, acts, clients, timer queue | previous frame |
| host clock | ms, masked to 31 bits | host only (`sim/tick.md` §1 r2, §8) |

## Outputs / state changes

- State changes of the dispatched handlers and of the tick steps.
- S→C messages in per-client buffers, flushed after a tick; direct
  sends (0xAF, 0xB0, 0xB4, …) bypass the buffers
  (`sim/intents-events.md` §3.3 r5).

## Rules

### 1. Server frame (host side)

Owner: `sim/tick.md` §1 r4 (single player, `0x0044EFA0`) and
`sim/intents-events.md` §1 r1–r5.

1. **Drain**: every server queue is drained (`0x0052CFE0`) and each
   game message is dispatched after its gate, size and field checks
   (`sim/intents-events.md` §2.2–§2.4). Queue order
   (`sim/intents-events.md` §2.1 r7): the whole system queue 0 first
   (0x67, 0x69, 0x6B, …: session handlers, §2.5, §8), then the game
   queue 1, then queue 2; arrival order within a queue. No
   tick is running: handlers see the frame counter of the previous tick
   (`sim/tick.md` §1 r5).
2. **Tick driver** (`0x0052FC20`, catch-up 1): at most one tick per
   frame (`sim/tick.md` §1 r2–r3). Due when `now − last ≥ 40` (signed);
   the lag carried forward is capped at one tick.
3. **Flush** (`0x0052FD90(1, 0)`): only if a tick ran in step 2. When no
   tick ran, the buffers keep the messages of the drain (step 1) and
   flush with the next tick (`sim/intents-events.md` §1 r3).
4. Exactly one drain and at most one flush per frame. Other flushes:
   the leave handler's flush of the leaving client (C→S 0x69,
   `flows/save-exit.md` §2 r6) runs inside step 1.
5. The client receive of the same loop pass runs right after step 3
   (`flows/client-frame.md` §1).

d2rs: `Host::frame` (`client/bridge.md` §3 r1, `pump`). The host owns
the clock; `d2-sim` never reads one.

### 2. Tick steps in order

Owner: `sim/tick.md` §3 (`0x0052D870`). The table repeats the order
only; what each step does is the owner's.

| # | Step | Runs | Owner |
|---|---|---|---|
| 0 | frame += 1 | always | `sim/tick.md` §2 r1 |
| 1 | environment (day/night per act 0..4; 0x53 to in-game clients of the act on change) | always | `sim/tick.md` §3 step 1, `render/lighting.md` §9.3 |
| 2 | frame-rate stats | always, host only, no outcome | `sim/tick.md` §8 |
| 3 | room pass: populate newly active rooms | always | `sim/tick.md` §4, `monsters/population.md` §1 |
| 4 | timer events: missile, player, monster, object, item; per class every-tick list then the due bucket | always | `sim/tick.md` §5.5, `sim/units.md` §5–§6 |
| 5 | client pass: per-client update and state 3 / 5 completion | always | `sim/tick.md` §6 |
| 6 | room update queues: free the per-room unit update queues | always | `sim/tick.md` §3 step 6 |
| 7 | removal records: free the per-room removal records | always | `sim/tick.md` §3 step 7 |
| 8 | quests | frame % 20 = 0 | `sim/tick.md` §3 step 8, `world/quests.md` |
| 9 | room deactivation | frame % 12 = 0 | `sim/tick.md` §3 step 9, `drlg/rooms.md` §7–§8 |
| 10 | free inactive rooms | frame % 11 = 0 | `sim/tick.md` §3 step 10 |
| 11 | expired items | frame % 1500 = 0 | `sim/tick.md` §3 step 11 |

1. Steps run in this order every tick; due periodic steps run 8 → 11
   (`sim/tick.md` §3 r1). Acts are visited 0..4 by index in every
   per-act step.
2. **Where the task's named phases sit**: intents are applied in the
   drain (§1 r1), never inside the tick; unit movement, animation, AI
   (monster event 2), missiles (every-tick event 0 of class missile) and
   regeneration all run inside step 4 (`sim/tick.md` §5.7); there is no
   separate movement, AI, collision or missile step (`sim/tick.md` §3
   r2). Rooms: step 3 (population), steps 6, 7, 9, 10 (bookkeeping).
   Messages: produced by the drain handlers and by steps 1, 4, 5;
   flushed after the tick (§1 r3).
3. Step 6 runs **after** step 5: the client pass reads the room update
   queues that units joined during step 4 and turns them into unit
   update messages (`sim/intents-events.md` §7), then step 6 empties
   them. Step 7 likewise frees the removal records the client pass
   turned into removal messages. Swapping 5 with 6 or 7 loses those
   messages.
4. A room activated in step 4 (a player walks into a new room) is
   populated by step 3 of the **next** tick (`sim/tick.md` §4), unless an
   off-tick population ran inside its caller
   (`monsters/population.md` §1 r2).

### 3. Timer queue within step 4

Owner: `sim/tick.md` §5.5.

1. bucket := frame % 64 (signed).
2. For class in missile (2), player (0), monster (1), object (3), item
   (4): the class's every-tick list from its head (newest first), then
   bucket `bucket` of the class from its head (oldest first), running
   only timers whose expire = frame.
3. An event scheduled during the run never runs in the same tick
   (`sim/tick.md` §5.5 consequence 1). Classes never interleave.

### 4. Client pass within step 5

Owner: `sim/tick.md` §6 r4–r6.

1. Clients in client-list order (`sim/unit-order.md` §7), next saved
   before the body.
2. Every client in state 3, 4 or 5 runs the per-client update
   (`0x005380D0`) in this order: removal messages, unit update messages
   of the adjacent rooms' update queues, the player's stat-change
   messages, the conditional inventory refresh (flag-ex bit 21), client
   counter += 1, the level change (quest event 3 CHANGEDLEVEL, then the
   town-leave refresh) when the player's room's level differs from the
   client room's, the room switch, arena sync, queue the player for
   update (`sim/tick.md` §6 r5).
3. Then, state 3 (joining): if the client's room is ready
   (`sim/tick.md` §6 r6) → S→C 0x04, state 4, inventory refresh, the join
   sequence (0x5B, 0x65, 0x8D, 0x5A to all) (`flows/game-join.md` §3).
   State 5 (changing act): room ready → S→C 0x04, state 4, inventory
   refresh (`flows/act-change.md` §1 r4).
4. frame % 8192 = 0 and no heartbeat drop: every client with a player
   has its character saved (`0x0052CA10` → `0x00532400`), before the
   per-client loop (`sim/tick.md` §6 r3; `flows/save-exit.md` §3).

## Constants & data dependencies

Tick length 40 ms; periods 11, 12, 20, 64 (bucket), 1500, 8192
(`sim/tick.md` §7).

## Randomness

No draw of its own. Draw order follows the step order above: room pass
draws (room seed, game seed) before every timer-event draw of the same
tick; per-class draws in class order (`sim/rng.md`, `sim/tick.md`
Randomness).

## Edge cases & original bugs

1. A frame with no tick still drains: handlers run and queue messages
   that wait in the buffers until the next tick's flush
   (`sim/intents-events.md` §1 r3).
2. Two client messages sent in the same client frame are drained in the
   same server frame, before one tick: in send order within one queue,
   but every system message (e.g. 0x69 leave) before every game message
   of that frame, whatever the send order.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| frame 660 | steps 8, 9, 10 all run, in that order | `sim/tick.md` §3 r1 |
| C→S 0x01 (walk) sent in client frame k | dispatched in the drain of frame k+1, before its tick | `sim/intents-events.md` §1 r2 |
| server frame with no tick due | no flush; buffers keep the drain's messages | `sim/intents-events.md` §1 r3 |

## Provenance

Compiled 2026-10-08 (q-tick-flow) from the owner specs cited in each
step; no new reading of `Game.exe`. Every address is the owner's.

## Open questions

None of its own; the owners' open questions apply.
