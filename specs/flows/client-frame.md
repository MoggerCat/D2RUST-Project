# Spec: Flows — One client frame (single-player loop pass)

- **Status:** draft: a flow spec (top level of the analysis hierarchy).
  It owns no rule: each step names its owner spec and 1.14d address, and
  this file fixes how they chain into one client loop pass. Audit:
  `docs/handoff/q-tick-flow.md` (2026-10-08).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::bridge` (`bridge_frame`, mirror),
  `d2-client::app::play` (Bevy schedule), `d2-client::world_view`,
  render and input systems
- **Related specs:** `client/bridge.md` §1–§8 (d2rs frame design);
  `client/model.md` §4–§6, §17 r4 (receive, update pass, position
  check, runs per loop pass); `render/camera.md` §9 (draw schedule, no
  interpolation, client path step); `render/composition.md` §3 (draw
  cycle); `sim/intents-events.md` §1 (loop order); `flows/server-tick.md`.

## Summary

One pass of the 1.14d client loop (`0x0044EFA0`, single player) runs the
server frame inline, receives what the server flushed, updates the
client's units (each unit's own per-frame step, then its queued unit
messages), draws once if the server ticked, and handles input, which
queues C→S messages for the next pass's drain. There is no separate
prediction or interpolation step: between server messages the client
advances its own units in the update pass, and a position that drifts
too far is corrected by the position check. d2rs maps the pass onto one
Bevy frame: `bridge_frame` (pump, receive, update pass), then the
mirror, world view and render, with input systems sending through the
bridge.

## Inputs

| Name | Type | Source |
|---|---|---|
| S→C chunks | bytes | server flush of this pass (`flows/server-tick.md` §1 r3) |
| user input | keys, mouse | OS events |
| UI state | open panels, pause | UI layer (`client/model.md` §17) |

## Outputs / state changes

- Client model changes (unit table, local player, act).
- One presented frame in a drawn pass.
- C→S messages queued during the pass, drained by the next pass's server
  frame.

## Rules

### 1. Loop pass order (1.14d)

Owner: `client/model.md` §17 r4 ("Runs per loop pass"),
`sim/intents-events.md` §1 r1.

1. **Skip**: `0x004F6070` ≠ 0 → the pass returns at once (no receive,
   no draw).
2. **Paused** (UI state 9 or 11, local player with a room): draw
   (`0x0044F017`) and return: no server frame, no receive, no update
   pass. d2rs: the skill fallback runs once (`client/bridge.md` §8 r5).
3. **Server frame**: drain, tick driver, flush if a tick ran
   (`flows/server-tick.md` §1).
4. **Receive** (`0x0044C6E0`): the system list, then the game list
   (`sim/intents-events.md` §3.4 r1); each message's general handler
   runs at once; a message with a unit handler is appended to its
   unit's queue instead (`client/model.md` §4 r1). Runs every pass, tick
   or not.
5. **Client update pass** (`0x0044C790`): only in a pass whose server
   frame ran a tick, and only while `in_game` (`client/model.md` §5 r1).
   Per unit in the order of `client/model.md` §5 r3: the per-type update
   (the unit's own step: mode machine, client path step of
   `render/camera.md` §9 table), then the unit's queued messages
   (`client/model.md` §4 r5). A unit-handler message therefore takes
   effect after every general handler of the same receive.
6. **Skill fallback** (d2rs, last step of `bridge_frame`): only in a
   pass whose pump ran a tick, while `in_game` (`client/bridge.md` §8
   r5, `client/model.md` §17 r4).
7. **Draw** (`0x0044C990`, the drawn pass): when the skip counter is 0,
   `in_game`, and the local player has a room (`client/model.md` §17 r4
   r3); outside the act-load hold and the lag case, exactly the passes
   whose server frame ticked (`render/camera.md` §9). Inside: camera and
   shake, `StartDraw` / partial clear, world, UI, cursor, overlays, the
   act-load black frame, `EndScene` / present (`render/composition.md`
   §3).
8. **Input and send**: the rest of the pass handles input; requests
   become C→S messages sent at once to the server queues
   (`client/bridge.md` §4 r4) and drained by the **next** pass's server
   frame (`sim/intents-events.md` §1 r2). Messages the client sends on
   its own also go out during the pass that caused them: 0x6B on 0x02
   (receive), 0x5F on a failed position check of the local player
   (receive or update pass, `client/model.md` §6 r8), 0x4B
   (update pass, `client/model.md` §5 r5).

### 2. Prediction, correction, interpolation

1. No prediction of outcomes: the client never decides one
   (`CLAUDE.md` rule 7; `client/bridge.md` §1 r1).
2. Client-side movement is the client's own per-unit step in the update
   pass (§1 r5): the path step of 0x400 per step, step counts per unit
   type in `render/camera.md` §9; it runs only in a tick pass.
3. Correction: position messages run the position check
   (`client/model.md` §6): within tolerance nothing; else the local
   player is not moved and C→S 0x5F is sent with its own position; other
   units are re-placed.
4. No interpolation: a drawn frame shows the integer state at draw time
   (`render/camera.md` §9). Passes without a tick do not draw.

### 3. d2rs mapping (one Bevy frame)

Owner: `client/bridge.md` §7 r1, §8 r1–r4.

1. `PreUpdate`: `bridge_frame` (pump → receive and dispatch every chunk
   → update pass if a tick ran and `in_game` → skill fallback), then
   `mirror_units`, in that order.
2. Input and UI systems run later in the frame and send through the
   bridge resource; the bytes reach the server queues at once, the
   next frame's pump drains them.
3. World view and render read the model after the mirror of the same
   frame; one present per drawn frame.
4. `pump` at most once per frame; no catch-up loop (`client/bridge.md`
   §8 r2).

## Constants & data dependencies

Client tick 40 ms (`[0x0070EF1C]`); position tolerances
(`client/model.md` §6 r4).

## Randomness

The client update pass draws only from the client's own seeds where an
owner spec says so (`client/model.md` Randomness); none here.

## Edge cases & original bugs

1. A receive in a pass without a tick still dispatches general handlers;
   unit-handler messages wait in their queues until the next tick pass's
   update pass.
2. A message for a unit added later in the same receive is dropped
   (`client/model.md` §4 r6).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| pass with a tick, `in_game` | receive, then update pass, then draw | `client/model.md` §17 r4 |
| pass without a tick | receive only; no update pass, no draw | `client/model.md` §5 r1, §17 r4 |
| paused pass | draw only, no receive | `client/model.md` §17 r4 r2 |

## Provenance

Compiled 2026-10-08 (q-tick-flow) from the cited owner specs; no new
reading of `Game.exe`.

## Open questions

1. Unspecified: where in the 1.14d loop pass `0x0044EFA0` the input
   handling (window messages, UI mouse and key handlers that build C→S
   messages) runs relative to the draw (`0x0044F28B`): before it, after
   it, or in a separate message pump. `sim/intents-events.md` §1 r1
   says only "rest of the client frame". It decides whether a click in
   pass k is drawn (cursor, panel) in pass k or k+1. Settled by a static
   read of `0x0044EFA0` after the draw site.
2. Unspecified: the d2rs order between the world view / render systems
   and the input systems inside one Bevy frame (`client/bridge.md` §7 r1
   fixes only `PreUpdate` for the bridge). Follows question 1.
