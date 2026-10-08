# q-seam-audit: contract checks at the seams (M23)

Branch `claude/q-seam-audit` (from staging `aefd2b82`). The findings came
from reading both sides of each boundary; none was found or confirmed on
the real install. Several were confirmed by a contract test that failed
and was then removed, because the fix is more than one line (task rule:
no failing test is left in). Every finding has a `q-fix-seam-*` row in
`docs/handoff/build-queue.tsv`; the row holds the file:line of both
sides, the fix and the test to add.

## Boundaries and their contract specs

| Boundary | Contract spec | Contract tests (all pass, no game files) |
|---|---|---|
| client model ↔ prediction ↔ server movement | `specs/seams/movement-prediction.md` | `d2-client` lib `bridge::seam_movement_tests` (7) |
| world ↔ screen (projection, camera, pick) | `specs/seams/world-screen.md` | `d2-client/tests/seam_world_screen.rs` (5) |
| server ↔ proto ↔ client messages | `specs/seams/messages.md` | `d2-client/tests/seam_messages.rs` (5) |
| bridge ↔ Bevy app (view, UI, audio, timing) | `specs/seams/bridge-app.md` | `d2-client/tests/seam_bridge_app.rs` (2) |
| d2-sim ↔ d2-server wiring | `specs/seams/sim-server.md` | `d2-server/tests/seam_sim_server.rs` (2) |
| DRLG room / level coordinates (sim, server, client) | `specs/seams/drlg-coords.md` | `d2-client/tests/seam_drlg_coords.rs` (2) |
| item and inventory grids (sim, wire, client UI) | `specs/seams/item-grids.md` | `d2-client/tests/seam_item_grids.rs` (5) |

What already agrees, according to these tests:
- The projection maths: draw then pick gives back the same subtile, and every pixel of a subtile diamond picks it.
- Cell ↔ 16.16 conversions, the walk intents' layouts and the base-case walk / run tick distance.
- About 35 server message builders read field by field through the generated layouts. The join sequence goes through the client's receive without a refusal.
- Client rooms equal the server's after the join and across the town / Blood Moor border.
- Page ids, wire position fields, grid records, belt slots and body locations.

One fix was made in place (one line): `crates/d2-client/src/app/sound.rs`
`audio_frame` now runs in `PostUpdate`. It used to run unordered in
`Update` beside `world_view_frame`, so a UI sound could start one sound
tick late, and which frame it started on could change from run to run.
`seam_bridge_app` counts the systems, so the old placement fails the test.

## Findings, most visible first

Rendering and movement first. "Seen as" is the failure on the real install.

| # | Row | Visibility | Seen as |
|---|---|---|---|
| 1 | `q-fix-seam-check-own-position` | high | Rubber band. The position check compares the server's 0x96 cell with the stale model cell and mode, not with the client's own path cell. Once a walk has gone more than 3 sub-tiles, every 0x96 snaps the prediction, or sends 0x5F and the server walks the player back. |
| 2 | `q-fix-seam-vitals-delta` | high | Rubber band. The dx/dy sign of 0x18 / 0x95 / 0x96 is mirrored: the server writes X − target and the client rebuilds X + dx. A client that is ahead on its path is judged far and corrected. The owner specs disagree (`combat/vitals.md` §5.4 vs `client/msg-units.md` §5 r3), so the sign needs Ghidra or a recording (local). Do it with #1. |
| 3 | `q-fix-seam-monster-velocity` | high | Monsters crawl, then jump. The client reads the 0x67 / 0x68 velocity field (stat 67, a percent) as a path velocity, about 15× too slow. The motion goal is also wrong for most path types. |
| 4 | `q-fix-seam-local-pos` | high | Two local-player positions in one frame. The draw and the click use the prediction. Hover, labels, corpse clicks, overhead text, the automap and the near-room centre use the model cell. They drift 16 px per subtile off the sprites while walking, and the highlighted unit can differ from the one the click hits. |
| 5 | `q-fix-seam-click-distance` | high | The interaction distance is measured from the model cell. After a walk, an NPC / object click detours, or sends a 0x13 from out of range that the server refuses (the same class as q-smoke-town break 5). Same root as #4. |
| 6 | `q-fix-seam-audio-pending` | high | Crash. A sound needing an input that isn't wired yet returns `DriverError::Pending`. It becomes a Bevy error, and Bevy 0.19.1 panics on it by default. The first unit sound on the real install would trigger it; synthetic runs have no driver. |
| 7 | `q-fix-seam-pause` | high | The Esc menu doesn't pause (`client/bridge.md` §8 r5). If only the bridge paused, the UI (which runs only on server-tick frames) could never close the menu. |
| 8 | `q-fix-seam-anim-frame` | med | Every object mode start (chest, door) draws one wrong frame, and torches flicker once per loop: animation frame 0 is read as "no frame". |
| 9 | `q-fix-seam-beltable` | high | Scrolls never go to the belt from the client. The client uses a fixed list of item codes, not the itemtypes `beltable` column the sim reads. |
| 10 | `q-fix-seam-store-grid` | med-high | The shop repacks store items and ignores the server's (x, y). The layout differs from the original, and on a full page some items are neither drawn nor buyable. |
| 11 | `q-fix-seam-grid-facts` | med | The client never sends stack (0x21), scroll-to-tome (0x29), item-to-cube (0x2A) or Ctrl-click sell (0x33), because the facts are hard-coded off. The cube grid passes inventory mode 0 instead of 0x0E. |
| 12 | `q-fix-seam-click-sounds` | med | World-click refusal sounds ("can't use that in town") and interact outputs are dropped. |
| 13 | `q-fix-seam-sound-listener` | med | The audio listener stays at the model cell while the drawn player walks. Same root as #4. |
| 14 | `q-fix-seam-predict-velocity-stats` | med | With faster run/walk, Burst of Speed or slows, the prediction drifts from the server and is snapped. It ignores stats 67 and 96 beyond the run bonus. |
| 15 | `q-fix-seam-stamina-scale` | low | For about 6 ticks before exhaustion, the client predicts a walk while the server still runs. The client tests stamina after the 8.8 shift. |
| 16 | `q-fix-seam-pick-anchor` | low | Objects and ground items are picked 8 rows below where they are drawn. |
| 17 | `q-fix-seam-shake-camera` | low, latent | Screen shake reaches only the main camera. Ground items, missiles and clicks would be off by up to 25 px once shakes start. |
| 18 | `q-fix-seam-tick-order` | low | A tick's messages leave grouped by outbox (action, then inventory, then quest / NPC / vendor), not in production order (`sim/intents-events.md` §1 r3). |
| 19 | `q-fix-seam-staged-act` | low | Test hosts only: after a cross-act warp, caller-staged unit facts keep the old act, so unit messages to new-act targets are refused. |
| 20 | `q-fix-seam-npc-dialog-capture` | low | The NPC menu reads the level / unidentified count / expansion at delivery, not at receive. |
| 21 | `q-fix-seam-room-order` | low | The draw writes the room order back to the model. `bridge.md` §1 r1 doesn't list that write, and it is skipped on frames whose image isn't built. |
| 22 | `q-fix-seam-act-cache` | low | Tile flags and fades are keyed by DRLG room slot and may carry into the new act. Unconfirmed: it depends on message timing. |
| 23 | `q-fix-seam-present-offset` | low | The presented image can sit half a physical pixel off the click mapping with an odd leftover width or height. |
| 24 | `q-fix-seam-stash-fallback` | low | The stash fallback rectangle (used only without `inventory.bin`) maps the bottom pixel row to row 8, outside the grid. |

Suggested grouping for the build loop:
- #1 + #2 + #14 + #15: the position check and prediction.
- #4 + #5 + #13 + #16: one per-frame local position and camera.
- #9 + #11: client item-table facts.

#6 and #7 are the cheapest high ones.

## Note on an existing row

`q-fix-set-skill-fatal`: the server's own join sequence is accepted
whole once the client model has one `skills` row for skill 0. With
`ModelInputs::default()`, the two 0x23 are refused with `Fatal(0x668)`.
That points the row at the client's table wiring (`ClientTables.skills`
not bound), not at a bad skill id from the server
(`seam_messages::the_server_join_sequence_builds_the_client_model`).

## Open questions for spec sessions

1. The dx/dy sign of the vitals messages (#2): Ghidra 0x00548760 /
   0x0045DB20, or a recorded walk.
2. The original's screen → world conversion for a click (0x0045AFF0,
   named in `ui/controls.md` §6 r2) is not specified. d2rs inverts the
   unit origin (`specs/seams/world-screen.md` OQ1).
