# Handoff: update-pass movement messages — `claude/path-update-pass`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md`; the coordinator
> folds it.

Cloud implementation session, 2026-10-06, medium (METHODS M14). Base:
`claude/tender-meitner-mphas3` at `01dff69` (includes the merged
`wire-path-server`). Repo only, synthetic tables and DRLG, no game files
(M09). Specs: `sim/pathing.md` §10 rules 2–4, §1.3; `sim/tick.md` §6.5.
HANDOFF §2 step 7p, part (1) remainder.

## 1. State

**Callers already present (no code change needed):** the task asked for
the callers of `path::walk::messages::{mode_update, reassign_flag}` in the
server's update pass. `wire-path-server` (merged in the base) already
added them: `tick::client_update` (`tick.md` §6.5) →
`TickHooks::send_unit_update` (`0x0053A5D0`) → `ActionSim` (forwarded by
`WorldSim` through `lent!` and by `QuestTick`) →
`wiring::path::walk::update_messages`, which calls `reassign_flag` (rule
3) then `mode_update` (rule 2) and sends through `Pending::send`, reaching
the clients through `SimGame::tick`'s `take_sent`. So the HANDOFF line
"the update pass (`tick.md` §6 step 5) has no caller of `mode_update`
yet" in step 7p is stale: drop it when folding.

**e2e step 6 with the provider on:** holds in
`crates/d2-client/tests/e2e_walk.rs` (`wire-path-server`: walk, run, the
waypoint to the town, 0x07 then 0x0D exact). `e2e_single_player.rs`
keeps the provider off because its step 4 flies a missile (blocker WP1:
no path points with the provider on); its step-6 comments said the
placement spec was unwritten, now they name WP1 and point to
`e2e_walk.rs` (comment-only change).

**Status:** wired, unverified (M02): no per-tick recording
(`pathing.md` OQ1).

## 2. Tests added (gate: PASS; d2-sim + conformance 1,876, rest 604, d2-client 362)

`crates/d2-server/src/adapters/handlers/walk/tests.rs` (the branches of
the update pass that had no test):

| Test | What |
|---|---|
| `bit_0x800_tells_only_the_other_clients_before_the_mode_update` | flags 2 0x800 + walk: tick 1 client 1 gets 0x15 flag 0, then 0x0F code 1 (rule 3 before rule 2); own client nothing |
| `bit_0x10000_tells_every_client_before_the_mode_update` | 0x10000 \| 0x800: 0x15 flag 1 to both clients, own included; 0x0F to client 1 after it |
| `town_walk_sends_the_walk_code` | walk in the town (room C): mode 6, 0x0F code 1 to the other client; the stop sets 5 |
| `only_players_get_the_movement_messages` | an object with CHANGED, mode 2, 0x10000 \| 0x800, queued: nothing sent |
| `the_update_pass_sends_nothing_without_the_path_provider` | M08: the same queued player with 0x10000 sends 0x15 to both clients with the provider on, nothing off |

d2-server walk tests: 9 → 14.

## 3. Doc fixes asked for

- `d2-server/src/adapters/handlers/world/action.rs` lines 5–6 already read
  "stay stubs on this host; [`super::WiredWorld`] adds them" on the base:
  nothing to change.
- `SimGame::handle`'s doc (`adapters/sim.rs`) already says the
  `pierce_idx` increment is `use_::handle_message`'s: nothing to change.
  `seams.rs` `Intents::handle` says skill messages "still owe the
  `pierce_idx` increment": that is the trait's contract (the dispatcher
  does not do it, the handler must), not a stale TODO; left as is.

## 4. Findings and open questions

1. Not new: rule 2 depends on the client order. Unit flag 0x1 is never
   cleared (`wire-path-server.md` §4 finding 2), and each per-client
   update queues its own player at the end (`tick.md` §6.5). So a client
   updated *after* the walking player's client sees that player queued
   again on every tick, and gets 0x0F every tick while the mode stays
   2/3/6. The tests pass only because client 1 runs before client 0 in
   list order. Settle with `0x00553220` (which flags step 6 clears); the
   answer may change the expected per-tick bytes of every walk test.
2. Rule 4 (monsters 0x67/0x68 through `0x00598220`) has no spec owner
   yet; `update_messages` sends nothing for non-players.

## 5. Local checks to queue

None new. When `pathing.md` OQ1's recording exists, it should cover two
clients in one game. Check that the second client gets 0x0F once per walk
request and not once per tick (finding 1), and whether 0x15 with flag 0
reaches the other client after a 0x800 event.
