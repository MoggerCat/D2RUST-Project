# Handoff: the client world model and its S→C handlers — `claude/impl-client-model`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it (§1 state, §2 step 7q RW2, §3 code map, §5 queue, §7 questions).

Cloud implementation session, 2026-10-06, task class: implementation from
new specs, high (the client's view of the world). Base:
`claude/tender-meitner-mphas3` at `b435f5a`. Repo only: no `game/`, no
`re/`, no recording (M09). Specs read: `specs/client/model.md`,
`msg-units.md`, `msg-stats-items.md`, `bridge.md`, `bridge-dispatch.tsv`
(no spec edited). Parallel sessions (`impl-ui-text`, `impl-draw-order`,
`impl-unit-composite`, `render-followups-impl`) own the render code; this
session touched `d2-client::bridge`, one new `world_view` file, the app's
feed choice and the tests whose expectations the newly owned ids change.

## 1. State

**Implemented, unverified** (M02): every handler follows its spec rule and
is pinned by the spec's vectors (recorded bytes where the spec gives
them); no executable check against 1.14d runs yet (queued in §5).

1. **Model** (`bridge/world.rs`, `model.md` §1–§2): `ClientWorld` holds
   the §1 rule 1 fields (`units` = set S, `local_player`, `difficulty`,
   `expansion`, `ladder`, `game_flags`, `act`, `in_game`, `unloaded`,
   `exit_requested`, `rooms_in_sight`, `outgoing`) plus `use_cursor`
   (`msg-stats-items.md` §3 rule 2.3) and the bridge counters.
   `ClientUnit` holds the §1 rule 2 fields (`class`, `mode`, `position`,
   `server_point`, `stats`, `seed`, `queue`, `last_mode_request`, `kind`
   = per-kind data of players, monsters, objects, items).
   `ClientWorld::add` replaces an existing key (§2 rule 4), `remove`
   drops the queue and clears the local player (§2 rule 5);
   `update_order` is §5 rule 4. `ClientUnit::new(key)` replaces the
   struct literal everywhere (other sessions' tests: one-line edits).
2. **Queue and update pass** (`receive.rs`, `update.rs`, `dispatch.rs`,
   `model.md` §4–§5): a handler is `Handle::General` (applied at receive)
   or `Handle::Unit` (queued on the addressed unit at receive, dropped
   when the unit is not in the model, applied by the update pass). The
   dispatch check (§6 rule 5) gained `Mismatch::Kind`: the kind must
   follow the receive table's `client_unit_handler` column. `Bridge::frame`
   runs the update pass after the receive when the pump ticked and
   `in_game`, then sends `outgoing` (0x6B, 0x5F) through the send path.
   `ReceiveLog` / `FrameReport` gained `queued`, `dropped`, `drained`
   (and `answered` on the frame report).
3. **Handlers** (`bridge/msg/`, registered in `msg::HANDLERS`, 51 ids,
   exactly the owned rows of `bridge-dispatch.tsv`): `session.rs`
   (0x00–0x08, 0x0B), `units.rs` (0x0A, 0x0C–0x10, 0x15, 0x18, 0x4C,
   0x4D, 0x51, 0x59, 0x67–0x72, 0x95, 0x96, 0xAC), `stats_items.rs`
   (0x19–0x20, 0x3F, 0x42, 0x47, 0x48, 0x9C, 0x9D). Ids `d2_proto::s2c`
   parses are matched on `d2_proto::s2c::Message`; the others are read
   from their spec layouts. Position check `check.rs` (§6), bit reader
   `bits.rs` (§10).
4. **ViewFeed** (`world_view/model_feed.rs`, RW2): `ModelFeed<F = NoFeed>`
   answers `player` (the local player's cell centre as a 16.16 position,
   `model.md` §3 rule 3) and `unit_position` (moving kinds: cell centre;
   objects / items / tiles: their cell; an item with no cell is an error);
   every other hook goes to the wrapped feed. The app (`app/play.rs`) now
   uses `ModelFeed::<NoFeed>`. With today's server stream the window stays
   black: the server sends no 0x59 / 0x0B (§4 F1), so there is no local
   player and no camera.

Tests (all `// Covers:`): `bridge/msg/tests_model.rs`,
`tests_units.rs`, `tests_stats_items.rs` (every recorded vector of the
three specs, plus synthetic ones: the 0xAC bit stream with components,
flags, umods, value, 31-bit value and stat list; every §4 row's record
layout; the §6 check branches), `bits.rs`, `world_view/model_feed.rs`,
`bridge/tests.rs::update_pass_runs_on_ticked_in_game_frames_and_answers_are_sent`.
E2E: `tests/e2e_full_loop.rs` (the local server's S→C stream drives the
model: after pick-up and placement the model holds exactly the one item
unit with the last 0x9C record; the end state asserts the log: 6 applied,
0x0D dropped, 0x07 rejected, 0x27–0x2A unowned), and the same for
`e2e_single_player.rs` (three item units, 31 applied), `e2e_walk.rs`,
`app_frame_loop.rs`, `local_tests.rs` (their expectations changed from
"every id unowned" to the new accounting; nothing weakened: each now
also asserts the dropped / rejected sets). `py tools/coverage.py`:
`model.md` 47 of 51, `msg-units.md` 28 of 30, `msg-stats-items.md` 13 of
13 (uncovered: model §2 r7, §8 r2; msg-units §3 r3, §5 r4: all Phase 6 or
client DRLG, nothing to model yet).

Gate (`sh tools/gate.sh`, this branch): every step passes except "test
d2-sim + conformance", which fails on the base `b435f5a` too (no file of
`d2-sim`, `conformance` or `specs/` is changed here):
`d2-sim monsters::ai::tests::implemented_matches_catalogue` and
`monsters::ai::tests::rules::d2moo_only_act1_ais_are_stubs` — the AI
catalogue TSV marks thinks 4, 5, 8, 10, 11, 15, 20, 26, 28, 30–33, 37, 43,
59, 60, 64, 90, 98 `spec'd-here` while `IMPLEMENTED` has none of them (a
monster-AI spec landed ahead of its implementation). Owner: the
monster-AI implementation session. "test d2-client" (all of the above)
passes, clippy and fmt pass.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/bridge/world.rs` | `ClientWorld`, `ClientUnit`, kind data, add / remove, drain order, `ModelInputs` (tables, visibility seam) | `client/model.md` §1, §2, §5 r4 |
| `crates/d2-client/src/bridge/update.rs` | the update pass (queue drains) | `client/model.md` §4 r5–r6, §5 |
| `crates/d2-client/src/bridge/check.rs` | position check `0x004804E0` | `client/model.md` §6 |
| `crates/d2-client/src/bridge/bits.rs` | bit reader | `client/model.md` §10 |
| `crates/d2-client/src/bridge/msg/` | the 51 handlers, `HANDLERS` | `client/model.md`, `msg-units.md`, `msg-stats-items.md` |
| `crates/d2-client/src/world_view/model_feed.rs` | `ModelFeed`: local player and unit positions for the camera | `client/model.md` §3 r3, `render/camera.md` §2–§3 |

## 3. Decisions (d2rs, within the specs)

- **Inputs are not model fields.** The tables 0xAC reads (`monstats` row
  existence + `monstats2` component choice counts, `itemstatcost` send
  bits) and the visibility predicate `0x004DBF20` are `ModelInputs`
  (`Bridge::set_tables`, `Bridge::set_visibility`), handed to handlers in
  `Message::inputs`. The app supplies none yet (§4 F3).
- **Unknown inputs fail loudly** (M07): the check's rule 6 without a
  visibility predicate is a handler error (`TODO(spec: …)`), recorded as a
  rejection; it is reached only when the stated point differs from the
  unit's cell on both axes within tolerance.
- **Seeds**: `ClientUnit::seed` is `None` when the unit was created at a
  point other than (0, 0) (the client room seed is not in the model,
  model OQ5); at (0, 0) the spec's values ({1, 666}, players one step →
  {0x6AC6935F, 0}, monsters {0, 666}).
- **0x59 always steps the new player's seed**: the "unless already the
  local player" test compares the new record with the local player
  pointer, which at that moment is still the old record (Randomness
  rule 2 read literally).
- **Mode requests on missiles are not stored** (`model.md` §8 rule 1:
  the missile branch does nothing).
- **0x04 "local player has a room"** is modelled as "the local player is
  placed" (a unit created at (0, 0) has no room, §2 rule 6).
- **total(s) = base** for 0x19–0x1B: the model holds layer-0 base values
  only (edge case 1's bonus drift cannot appear yet).
- **Outgoing answers** are sent at the end of the bridge frame (1.14d
  sends during the receive; both reach the next pump's drain).

## 4. Findings (for the coordinator)

- **F1 (blocks a camera in the app)** — the server never sends the join
  stream the client model is built from (0x01, 0x00, 0x02, 0x59, 0x0B,
  0x03, 0x07 ×n, 0x15, monsters, objects, 0x04: `model.md` Provenance).
  So the e2e streams leave `local_player` none, `ModelFeed::player`
  answers `None`, and the window stays black; 0x0D for the player is
  dropped and 0x07 is rejected (fatal 0x58A: no act). Owner: the server
  session / join spec (`sim/intents-events.md`, the session handler).
- **F2** — 0x9D needs the local player and its owner unit (§2 rule 3),
  so with F1 every 0x9D changes nothing (e2e_single_player: 4).
- **F3** — `ModelInputs::tables` is empty in the app: 0xAC creates no
  monster until a spec names the `monstats2` columns that hold the
  component choice counts in the 1.14d `.bin` (`TODO(spec)` in
  `world.rs`), and the loader fills them from `d2-data`.
- **F4** — the visibility predicate (model OQ7) is a render seam; until
  `impl-unit-composite` / render specs give it, `set_visibility(None)`
  makes rule 6 of the check an error when reached.

## 5. Local run queue entries (for `docs/HANDOFF.md` §5)

1. **Replay a recorded join into the model** (needs `traces/raw/
   20261006-022633-packets.jsonl`, local): feed every S→C message of the
   recording through `Bridge::receive_chunk` (+ `update_pass` per frame
   where the server ticked) with the live tables, and compare after frame
   2 against the recorder's dump of the client unit table and the local
   player's stat list (`msg-stats-items.md` OQ1; recorder addition). Look
   for: no rejection; unit set, positions and stats equal.
2. **Visibility seam** (model OQ7): once `0x004DBF20` is specified,
   replay the 0x68 / 0x6B / 0x6C / 0x95 / 0x96 checks of recording A and
   compare the corrections against the recorded C→S 0x5F (none recorded).

## 6. Open questions raised here

- `client/model.md` OQ5 (client DRLG) also decides room-of-point (§2
  rule 7, fatal 0x13C), the unit seeds at a point, 0x15's fatals
  0x168 / 0x538 and the nearest-free-point fallback (msg-units §3 rule
  4.5): all `TODO(spec)` in code.
- The local player's hireling GUID (msg-units §1.2 rule 2, §2 rule 2) is
  not in the model: 0xAC never re-initialises and 0x0A always removes.
- 0x51 for unit types 0, 3, 4, 5 (msg-units §1.3 rule 2): the kinds'
  inits are named but not stated; the handler refuses them
  (`TODO(spec)`).
- `PlayerData::cursor_item` is cleared by 0x42 but set by no rule until
  the item stream spec (msg-stats-items OQ3).
