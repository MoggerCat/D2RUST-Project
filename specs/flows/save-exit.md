# Spec: Flows — Save and exit, periodic save, load

- **Status:** draft: a flow spec. It owns no rule: each step names its
  owner spec and 1.14d address. Audit: `docs/handoff/q-tick-flow.md`
  (2026-10-08).
- **Target version:** 1.14d
- **Crate/module:** `d2-client` Esc menu and exit; `d2-server` leave
  handler and character save; `d2-sim::tick` client pass (save period)
- **Related specs:** `ui/frontend-options.md` §O3 (Save and Exit Game);
  `client/msg-ui.md` §3 code 23 (the exit send); `sim/intents-events.md`
  §2.5 r2 (C→S 0x69 leave); `sim/tick.md` §6 r3, open question 5
  (periodic save, the save writer); `formats/d2s.md` (save bytes);
  `client/model.md` §7 r6, r7, r10; `ui/frontend-menus.md` §F1.3
  (where the client goes after the exit); `flows/game-join.md` (load).

## Summary

The character is written by the **server** (`0x00532400` →
`0x00532240`, `fopen` / `fwrite` of `<save dir><name>.d2s`) in two
places: every 8192 frames from the tick's client pass, and in the leave
handler of C→S 0x69, for every client with a player, before the leave
messages. Save and Exit on the client posts the game exit; C→S 0x69 is
sent by the exit send `0x00477EE0`; the server saves, answers 0x05,
0x06 and a direct 0xB0, flushes the leaving client and removes it. The
client ends the game on 0x05 / 0x06 and returns to character select.
Loading is the join's load step (`flows/game-join.md` §2 r1).

## Inputs

| Name | Type | Source |
|---|---|---|
| Save and Exit Game | UI action | Esc menu (`ui/frontend-options.md` §O3) |
| frame counter | i32 | `sim/tick.md` §2 |

## Outputs / state changes

The `.d2s` file; the client record removed; client `in_game` false,
`exit_requested` true.

## Rules

### 1. Client: Save and Exit Game

1. `ui/frontend-options.md` §O3: `[0x0070EE8C]` := 0, post `WM_CLOSE`,
   `[0x007A0674]` := 1, `0x0047E200(1)`.
2. C→S **0x69** through `0x00477EE0` (system queue, no duplicate
   filter; `client/msg-ui.md` §3 code 23; `sim/intents-events.md` §2.1
   r2), `exit_requested` := 1.
3. The client keeps running loop passes until the server's answer:
   0x69 is drained by the next server frame (`flows/server-tick.md` §1
   r1).
4. Unspecified: the exact chain from `WM_CLOSE` / `0x0047E200(1)` to the
   0x69 send is split across §O3 and `client/msg-ui.md` code 23 (that
   row is the S→C 0x50 code 23 handler); which of the two sends 0x69
   on the Esc-menu path is not stated by the owners.

### 2. Server: leave (C→S 0x69, in the drain)

Owner: `sim/intents-events.md` §2.5 r2, in this order:

1. Only when the client record exists and its state is 4.
2. Single player (game type 3): `0x0052CA10(game)`: every client with a
   player has its character **saved** (`0x00532400` → `0x00532240`,
   writes the file; `sim/tick.md` OQ5 answer).
3. S→C **0x05**; S→C **0x06**; direct **0xB0**.
4. Flush the leaving client's buffers now (`0x0052E320`): the only
   flush outside the post-tick flush (`flows/server-tick.md` §1 r4).
5. 0x5A code 3 built; the client removed (`0x00539DA0`, 0x5C to
   remaining clients in state 4), then 0x5A to the remaining clients
   (none in single player).

### 3. Server: periodic save

1. Tick step 5, frame % 8192 = 0, no heartbeat drop: `0x0052CA10`, every
   client with a player saved, before the per-client loop
   (`sim/tick.md` §6 r3). Runs in single player.
2. The save clears the inventory flag bit 1 (`items/inventory-moves.md`;
   `items/inventory.md`), so a save has a model effect besides the file.

### 4. Client: end of game

1. 0x05: `in_game` := false, `unloaded` := true (`client/model.md` §7
   r6). 0x06: `exit_requested` := true (§7 r7). 0xB0: `connected` := 0
   (§7 r10).
2. Then the front end: the main menu (recorded 2026-10-09, REC-200:
   `ui/frontend-menus.md` §F1.3 last row; d2rs still opens character
   select → `q-fix-real-exit-target`).

### 5. Load

The next game's join loads the written file (`flows/game-join.md` §2
r1; `formats/d2s-load.md`).

## Constants & data dependencies

Save period 8192 frames (327.68 s); path `"%s%s.d2s"`.

## Randomness

None.

## Edge cases & original bugs

1. `-nosave` (`[0x007310CC]` = 0) suppresses the file write only
   (`sim/tick.md` OQ5).
2. A leave from a client not in state 4 (e.g. during the act change,
   state 5) is ignored: no save, no answer (§2 r1).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| C→S 0x69, client state 4 | file written, then 0x05, 0x06, 0xB0, flush, client removed | `sim/intents-events.md` §2.5 r2 |
| frame 8192 | every player saved in step 5 | `sim/tick.md` §6 r3 |

## Provenance

Compiled 2026-10-08 (q-tick-flow) from the cited owner specs.

## Open questions

1. §1 r4: which client code sends C→S 0x69 on the Esc-menu Save and
   Exit path (`0x0047F2D0` → `0x0047E200(1)` / `WM_CLOSE` → ?). Settled
   by a static read of `0x0047E200` and the `WM_CLOSE` handler.
