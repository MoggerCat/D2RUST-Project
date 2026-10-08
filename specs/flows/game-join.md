# Spec: Flows — Game join (single player, create → first drawn frame)

- **Status:** draft: a flow spec. It owns no rule: each step names its
  owner spec and 1.14d address; this file fixes the chain across server
  and client. Audit: `docs/handoff/q-tick-flow.md` (2026-10-08).
- **Target version:** 1.14d
- **Crate/module:** `d2-server` session flow (create, join, enter);
  `d2-sim::tick` client pass (state 3); `d2-client::bridge::msg::session`
- **Related specs:** `sim/intents-events.md` §8 (server session
  sequence, owner of the message order); `client/model.md` §7, §11
  (client side); `sim/tick.md` §4, §6 (first tick); `sim/path-placement.md`
  §11, §13 (game entry); `formats/d2s-load.md`, `formats/d2s.md` §9
  (load); `render/composition.md` §3 r4, `render/camera.md` §9 (first
  frames); `flows/server-tick.md`, `flows/client-frame.md`.

## Summary

The client sends C→S 0x67; the server creates the game and answers
0x01, 0x00, 0x02; the client answers 0x02 with 0x6B; the server loads
the character, sends the player and its state, builds the act (0x03,
0x53), enters the player into the world (0x07 …, 0x15, 0x7E) and puts
the client in state 3; the next tick populates the town rooms, sends
their units and, the room being ready, 0x04 and the join sequence. The
client is in game on 0x04; its first drawn frame follows.

## Inputs

| Name | Type | Source |
|---|---|---|
| C→S 0x67 | 46 bytes | client game start (`client/model.md` §7 r9) |
| character save | `.d2s` or the 335-byte stub | save directory (`formats/d2s.md` §9) |

## Outputs / state changes

Game record, acts, client record (state 1 → 2 → 3 → 4), player unit;
the client model's flags, act, unit table; `in_game` true.

## Rules

### 1. Create (C→S 0x67), server frame F1

1. Client: 0x67 is sent (system queue) before the first loop pass that
   pumps (`client/model.md` §7 r9).
2. Server, drain of F1 (`sim/intents-events.md` §8.1): game, acts, seeds,
   arena, client and party records; queue S→C **0x01**, **0x00**
   (state 1), act byte, **0x02**.
3. These are buffered: they reach the client with the first flush,
   i.e. after the first tick (`flows/server-tick.md` §1 r3).
4. Client receive of that pass: 0x01 sets the flags; 0x00 nothing; 0x02
   sends C→S **0x6B** (`client/model.md` §7 r1–r3), drained by the next
   server frame.

### 2. Join (C→S 0x6B), drain of a later server frame F2

Owner: `sim/intents-events.md` §8.2; every message in this order:

1. Load (`0x00539760`); refusal → direct **0xB4**, client removed, stop
   (`client/model.md` §7 r8: `JoinRefused`).
2. Loader messages: **0x59**, **0xAA**, **0x76** (the player's add
   messages), then 0x94, 0x22 / 0x21 per item, 0x23 (mouse skill),
   0x5E, 0x28, 0x29 (§8.2 r3.1; new character: stub branch, one 0x23
   from `StartSkill`).
3. **0x0B**, **0x5F**, stats (0x1D / 0x1E), items (0x9C, 0x9D) when
   applicable, **0x7B** per set hot-key slot, **0x23** hand 1 then hand
   0, stats again, **0x95** and the gold / experience messages
   (§8.2 r3.2–r3.10).
4. Act: build if missing, **0x03** then **0x53**; client state 2
   (§8.2 r4).
5. Game entry (`0x005394A0`, `sim/path-placement.md` §11): **0x07** for
   the spawn room, the room switch (0x07 for every adjacent room; no
   unit adds: the rooms are not populated yet), placement, **0x15**,
   **0x7E**, followers (§8.2 r5).
6. Client state 3 (§8.2 r6).
7. All of it is buffered and flushed after F2's tick together with that
   tick's own messages (drain output first).

### 3. First tick after the join (tick of F2 or later)

Owner: `sim/intents-events.md` §8.3, `sim/tick.md` §4, §6 r4, r6.

1. Step 3 (room pass) populates the town rooms activated by the join.
2. Step 5, client in state 3: per-client update (unit adds of the new
   rooms: monsters 0xAC …, objects 0x51, 0x0E; the player's updates;
   stats; the inventory refresh 0x48); room ready → **0x04**, state 4,
   inventory refresh (0x48), join sequence: **0x5B**, **0x65**,
   **0x8D**, host callback (none in single player), **0x5A** code 2 to
   every client in state 4, the joiner included.
3. If the room is not ready this tick (a neighbour not yet populated),
   0x04 waits for a later tick; state stays 3.

### 4. Client side

Owner: `client/model.md` §7, §11.

1. 0x59: the player unit at (0, 0), no room. 0x0B: the local player.
   0x03: the client act is (re)built (`client/model.md` §7 r4) and the
   act-load black-frame counter set (`render/composition.md` §3 r4).
   0x07: rooms in sight. 0x15: the player placed, its room and level
   known (`client/model.md` §11 r3).
2. 0x04: `in_game` := true; the local player must have a room (fatal
   0x527 otherwise) (`client/model.md` §7 r5). Must come after 0x15,
   which the server order guarantees (§2 r5 before §3 r2).
3. The first update pass runs in the first tick pass with `in_game`
   (`flows/client-frame.md` §1 r5); the first draw is in a drawn pass
   with `in_game` and a room; the first in-game frame after the act load
   is black (`render/composition.md` §3 r4).

## Constants & data dependencies

Arena flags 0x00100004 (default); spawn tile index 0 of the act's town
(`sim/path-placement.md` §11).

## Randomness

Seed derivations at create (`sim/rng.md` §5.2); spawn and placement
draws in the join; population draws in the first tick
(`sim/tick.md` §4). Order as above.

## Edge cases & original bugs

1. A brand-new character sends no 0x7B (`sim/intents-events.md` §8.2
   r3.6) and still sends 0x5F and two 0x23 (§8.2 r7).
2. A failed load sends only 0xB4; nothing follows it.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| single-player join, `-022633` | seq 5–7 `01 …`, `00`, `02`; seq 102–155 load, 0x0B … 0x03, 0x53, 0x07 × 10, 0x15, 0x7E; frame 2: units, 0x04, 0x48, 0x5B, 0x65, 0x8D, 0x5A | `sim/intents-events.md` §8 |

## Provenance

Compiled 2026-10-08 (q-tick-flow) from the cited owner specs.

## Open questions

1. Unspecified for d2rs: the act built at join (`0x0052C210` builds the
   client's act only when its slot is empty). Whether building an act in
   advance (before 0x6B) is equal in effect depends on the act build's
   draws (`drlg/levels.md` §2); not stated by the owner.
