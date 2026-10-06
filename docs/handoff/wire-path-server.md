# Handoff: walk / run handlers on the server (`d2-server`) — `claude/wire-path-server`

> Not yet folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file is the detailed record until a docs session folds it.

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `60a4998` (host merge + `d2_sim::path` + `wiring::path` from
`wire-path-sim`). Repo only, synthetic tables and DRLG, no game files
(M09). Specs: `sim/pathing.md` §1.1, §9, §10; `sim/path-placement.md`
§10–§12; `world/waypoints.md` §7; `sim/intents-events.md` §2.4.
Parallel session: `wire-inventory-server` (its handler module and e2e
extensions; none of its files touched here).

## 1. State

**Wired, unverified** (M02): no rule is added; the server routes C→S
0x01–0x04 to the path provider of `wire-path-sim`, and the update pass
sends the player movement messages of `pathing.md` §10 rules 2–3.
Per-tick positions have no recording yet (`pathing.md` OQ1).

- **Handlers** (`crates/d2-server/src/adapters/handlers/walk.rs`, new):
  `WALK_IDS` (0x01 Walk, 0x02 WalkToUnit, 0x03 Run, 0x04 RunToUnit:
  form, mode, owner spec), `handle` (after the dispatcher's gate, size
  and point / unit parse; reads x, y or type, GUID; result 0 in every
  case, §1.1), `run` (on `ActionEvents`: `ActionSim::with` →
  `wiring::path::walk::walk_message`; `None` while
  `ActionHooks::paths` is off), `enable_paths` (turns the provider on;
  call before any allocation). Without the provider the ids keep the
  stub (recorded in `SimGame::unhandled`), so every existing host and
  test is unchanged.
- **Routing:** new `WorldHost::walk` (default `None`), implemented by
  `ActionWorld` (`walk::run`) and `WiredWorld` (delegates to its
  `action`). `SimGame::handle` tries walk after skills, before the
  stub.
- **Update pass** (`pathing.md` §10 rules 3 then 2): new
  `d2_sim::wiring::path::walk::update_messages`, called from
  `ActionSim`'s `TickHooks::send_unit_update` (`0x0053A5D0`, per
  client and queued unit; `WorldSim` forwards it) when the path
  provider is on: S→C 0x15 from flags 2 (0x10000 → flag 1; 0x800 for
  other clients → flag 0), then, for a player with unit flag 0x1 in
  modes 2 / 3 / 6, 0x0F or 0x10 to every client but its own. Sent
  through `Pending::send`, reaching the clients by `SimGame::tick`'s
  `take_sent`.
- **0x0D** is the placement's / waypoint arrival's (`waypoints.md` §7
  r7, already wired): with the provider on, the same-act waypoint warp
  places the player in the destination's spawn room, so the e2e
  step-6 condition holds and the client gets 0x07 then 0x0D.
- **0x96** has no sender spec (`pathing.md` §10 rule 5, OQ6): never
  sent.
- **Builders:** the bytes are built in `d2-sim` by
  `path::walk::messages` (d2-sim does not depend on `d2-proto`); every
  test states the expected bytes with `d2-proto`'s typed builders
  (`PlayerMove`, `PlayerToTarget`, `PlayerStop`, `ReassignPlayer`,
  `MapReveal`), so a layout difference fails.

Tests:

| File | Test | What |
|---|---|---|
| `d2-server/src/adapters/handlers/walk/tests.rs` | `walk_to_point_moves_the_player_and_tells_the_other_client` | 0x01 through `SimGame::handle` + ticks, two clients: vector M1 translated ((26, 10) → (31, 10), x = 0x1AE000 + k·0x6000, tick 14 at 0x1F8000, neutral); tick 1: client 1 gets 0x0F code 1, target (31, 10), cell (26, 10); own client nothing |
| | `run_to_point_drains_stamina_and_sends_the_run_code` | 0x03: mode 3, same positions (run list not wired), 0x0F code 0x17, stamina − 14 · 40 |
| | `walk_and_run_to_a_unit_send_0x10` | 0x02 / 0x04 to an object: target unit set, M1 positions, 0x10 code 0 / 0x18 with target type 2 and GUID |
| | `walking_into_room_b_changes_the_players_room` | (37, 10) → (43, 10): 16 ticks, room A → B, nothing sent with one client |
| | `refused_requests_still_return_0` | missing target unit, mode DT: result 0, nothing moves, nothing sent |
| | `without_the_path_provider_the_ids_stay_stubs` | M08 for the routing |
| | `waypoint_to_the_town_places_the_player_and_sends_0x0d` | 0x49 to level 1: player in room C (spawn room), mode 5, flags 2 0x10000; handler sends exactly 0x07 (0, 16, 1) then 0x0D (x + 3, y + 3); no 0x15 after (finding 1) |
| | `a_warp_within_the_level_sends_0x15_in_the_next_update_pass` | 0x49 to Cold Plains itself: 0x07 only (arrival room test fails, `wire-path-sim.md` §6); tick 1: 0x15 flag 1 at the cell; again with the next walk (finding 2) |
| | `walk_ids_match_client_messages_tsv` | `WALK_IDS` against `client-messages.tsv` (name, `handler`/`sim`, layout of the form) and the dispatcher's point / unit parse; perturbations: stubbed row, moved layout, swapped form |
| `d2-client/tests/e2e_walk.rs` | `walk_across_the_level_and_take_the_waypoint` | over the bridge: walk A → B (62 frames, +0x6000 each, room change), run back (56 frames, stamina), walk to the waypoint object (unit form, 11 frames), 0x49 to the town: client receives 0x07 + 0x0D exact bytes, `unowned` {0x07: 1, 0x0D: 1}; no message while walking; no errors |
| | `same_run_same_transcript` | two runs equal |

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-server/src/adapters/handlers/walk.rs` | `WALK_IDS`, `Form`, `form`, `WalkCall`, `WalkResult`, `enable_paths`, `run`, `handle` | `pathing.md` §1.1, §10 |
| `crates/d2-sim/src/wiring/path/walk.rs` | + `update_messages` (0x15, 0x0F / 0x10 of the update pass) | `pathing.md` §10 r2, r3 |
| `crates/d2-sim/src/wiring/action/dispatch.rs` | + `TickHooks::send_unit_update` on `ActionSim` | `tick.md` §6.5 |
| `crates/d2-client/tests/e2e_walk.rs` | the walk / waypoint e2e above | |

## 3. Signature changes and edits outside the owned files

- `handlers::world::WorldHost<D>`: new provided method `walk(&mut self,
  game, events, call: WalkCall) -> Option<WalkResult>` (default `None`);
  `ActionWorld` and `WiredWorld` implement it.
- `handlers/mod.rs`: `pub mod walk;`. `adapters/sim.rs`: one dispatch
  line in `SimGame::handle`.
- `d2-sim`: `wiring::path::walk::update_messages` (new, pub);
  `ActionSim` gains a `send_unit_update` body (behaviour change only
  with the path provider on; off, it returns at once).

## 4. Findings and questions

1. **No 0x15 after a waypoint warp to another level** (vs. recording R3
   of `path-placement.md`: 0x15 the next tick). The placement queues the
   player in the destination room's update queue; tick 1's per-client
   update walks the queues of the *client's* room adjacency (still the
   old room) before the room switch (`tick.md` §6.5 order), and step 6
   clears the destination queue. As specified, nothing sends 0x15.
   Settle (spec / Ghidra): whether the placement (`0x00554EA0` rule 6,
   which reads the player's client) or the level change switches the
   client's room before the update walk, or `0x0053A5D0` also covers
   the client's own player outside the adjacency.
2. **Flags 2 0x10000 / 0x800 and unit flag 0x1 are never cleared**:
   step 6's `0x00553220` "clears per-unit flags" (`items/inventory.md`
   §6.3 r4) without naming them. So any later queueing of the player
   (e.g. the next walk's mode set) sends 0x15 again, and a later
   queueing in a walk mode sends 0x0F again. Test
   `a_warp_within_the_level_sends_0x15_in_the_next_update_pass` records
   it (`TODO(spec)` at `update_messages`). Settle: Ghidra
   `0x00553220`.
3. The dispatcher's range checks (`point_state`, `unit_target`) read
   `SimGame`'s staged `UnitFacts`, not the path record; with the
   provider on they go stale after a walk (the e2e re-stages them from
   the path before each request). Fix (adapter): answer the player's and
   targets' positions from `ActionHooks::path_position` when the
   provider is on (needs `&D` access in `Intents::point_state`).
4. Run velocity equals walk velocity (`attach_run_stats`, `pathing.md`
   §8.2, still a `WalkUnits` default; `wire-path-sim.md` §5).
5. A same-level waypoint travel with two rooms does not send 0x0D: the
   arrival's spawn search returns another room than the placement used
   (`wire-path-sim.md` §6, second question).

## 5. Local checks to queue

None new runnable now. When `pathing.md` OQ1's per-tick recording
exists, replay its walks through `walk/tests.rs` (same start, target,
collision) and compare per-tick precise positions and the S→C bytes
the other client receives; R3's next-tick 0x15 settles finding 1.

## 6. Gate

`sh tools/gate.sh` — results in the commit message.
