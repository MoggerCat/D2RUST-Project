# q-tick-flow: whole-game flow specs and code audit (2026-10-08)

Branch `claude/q-tick-flow` (from `staging` at `aefd2b82`). Scope: the
top level of the analysis hierarchy (function → module → system →
seams → whole-game flows). No game files used.

## 1. What was written

`specs/flows/` (authored, every step cites its owner spec and the 1.14d
address the owner gives; no new `Game.exe` reading):

| Spec | Flow | Unspecified (open questions) |
|---|---|---|
| `flows/server-tick.md` | one server frame (drain → tick driver → flush if ticked) and the tick's 12 steps; the timer-queue class order; the client pass | none of its own; note: the drain runs the whole system queue before the game queue (`intents-events.md` §2.1 r7) |
| `flows/client-frame.md` | one client loop pass (skip, pause, server frame, receive, update pass, fallback, draw, input/send); prediction / correction / interpolation; d2rs Bevy mapping | OQ1 where 1.14d handles input relative to the draw; OQ2 the d2rs order of world view vs input systems |
| `flows/game-join.md` | 0x67 → 0x01/0x00/0x02 → 0x6B → load, player, act, entry → first tick 0x04 + join sequence → first drawn frame | OQ1 is an act prebuilt before 0x6B equal in effect |
| `flows/act-change.md` | act change (`0x0053ACC0`, state 5, 0x04 from the client pass), same-act waypoint, warp tile, portal (off-tick population) | OQ1 recorded order across the flush (owner OQ); OQ2 client view caches on 0x03 |
| `flows/save-exit.md` | Save and Exit → C→S 0x69 → server save, 0x05, 0x06, 0xB0, flush, removal; the 8192-frame save; load | OQ1 which client code sends 0x69 on the Esc-menu path |

## 2. Findings (code against the flows)

Paths under `crates/`. "Flow step" names the flow spec rule.

### Server frame and tick (`flows/server-tick.md`)

`Host::frame` (`d2-server/src/host.rs:218-233`) matches §1: one clock
read, drain every frame (system queue first), at most one tick
(`TickDriver::poll`, catch-up capped at one tick), flush only after a
tick. `d2_sim::tick::tick` (`d2-sim/src/tick/mod.rs:165-199`) runs the
§2 steps in order; `TimerClass::RUN_ORDER` (`d2-sim/src/tick/timer.rs:45`)
is missile, player, monster, object, item (§3). The client pass
(`tick/mod.rs:284-349`) has the §4 r2 order. Findings:

| # | Where | Flow step | Kind | Effect |
|---|---|---|---|---|
| T1 | `d2-sim/src/tick/mod.rs:282-284` (comment "the 8192-frame save … host-only"); no save in `d2-server` | server-tick §4 r4, save-exit §3 | missing — **fixed** (q-fix-flow-save; bit 1 clear still open, REC-291) | no character save every 8192 frames; `tick.md` §6 r3 was corrected 2026-10-08 (runs in single player), the code still follows the old reading. Also no inventory flag bit 1 clear that the save does |
| T2 | `d2-server/src/adapters/handlers/world/wired.rs:945-947` (`arrivals`, `item_arrivals`: `npc_approach.rs:32`, `item_approach.rs:52`) | server-tick §1 r1, §2 r2 | out of order | d2rs-own approach arrivals replay C→S 0x13 / 0x16 at the start of `run_tick`, before frame += 1: outside the drain and outside step 4, so their messages and draws come before the tick's environment and room pass |
| T3 | `wired.rs:958-971` (`after_tick`: quest events, `player_deaths`, corpses, pet deaths, approaches, `hireling_calls` incl. NPC act changes `hireling_host.rs:52-69`, pet follows, hirelings) | server-tick §2 r2 | out of order | unit work 1.14d does inside step 4 (timer events) or inside a C→S handler runs after step 11, after the client pass: its unit update messages miss this tick's client pass and go out a tick late; deaths and NPC act changes happen after the room bookkeeping |
| T4 | `d2-server/src/adapters/sim.rs:586-587` (`update_pass`, `vitals_sync` after the tick); `tick/mod.rs:114` (`client_update_messages` default, empty) | server-tick §4 r2 | out of order | stat messages (0x1D / 0x1E, 0x95) and the bit-21 inventory refresh are sent after the whole tick, not in the per-client update between the unit updates and the room switch; at the join's first tick 1.14d sends them before 0x04 (`intents-events.md` §8.3), d2rs after |
| T5 | `tick/mod.rs:95`, `:99` (`refresh_inventory`, `join_sequence`: default empty, no override in `d2-sim/src`) | server-tick §4 r3, game-join §3 r2 | missing | after 0x04 no 0x48, 0x5B, 0x65, 0x8D, no 0x5A "joined" |
| T6 | `d2-sim/src/wiring/action/dispatch.rs:440-458` (`client_level_change`, TODO) | server-tick §4 r2 | missing | a level change never fires quest event 3 CHANGEDLEVEL or the town-leave refresh `0x00537340` before the room switch |
| T7 | `tick/mod.rs:122` (`arena_sync`), step 6 `clear_arena_flag`, step 11 `delete_inactive_items` / `expire_inactive_unit_items` (defaults, empty) | server-tick §2 steps 6, 11; §4 r2 | missing | inactive items never expire (every 1500 frames); arena sync none (no single-player effect found) |

### Join (`flows/game-join.md`)

`SessionFlow::create` (`d2-server/src/adapters/session_flow.rs:328-366`)
queues 0x01, 0x00, state 1, 0x02: matches §1. `enter_game`
(`d2-server/src/adapters/session.rs:343-492`) follows §8.2 r3.2–r6 in
order (0x0B, 0x5F, stats, items, 0x7B, 0x23 × 2, stats, 0x95, 0x03,
0x53, state 2, entry 0x07 …, 0x15, 0x7E, state 3). The two stat runs
are rule 3.8, not a doubling.

| # | Where | Flow step | Kind | Effect |
|---|---|---|---|---|
| J1 | `session.rs:60-63` (module docs: "no spec gives them") | game-join §2 r2 | missing | the loader's 0x22 / 0x21 per item, 0x5E, 0x28, 0x29 are not sent; `intents-events.md` §8.2 r3.1 (c)–(e) now gives their order and callers |
| J2 | `session_flow.rs:36`, `:131` | game-join §2 r1 | missing | a refused load removes the client without the direct 0xB4: the client waits forever instead of showing the error |
| J3 | T5 | game-join §3 r2 | missing | no 0x5A joined, 0x5B, 0x65, 0x8D |

### Act change and travel (`flows/act-change.md`)

| # | Where | Flow step | Kind | Effect |
|---|---|---|---|---|
| A1 | `d2-sim/src/units/lists.rs:279` (`CHANGING_ACT` never written; read only at `tick/mod.rs:293`) | act-change §1 r2, r4 | missing | the client never enters state 5; the tick's state-5 branch (room ready → 0x04, inventory refresh) is dead code |
| A2 | `d2-sim/src/wiring/path/act_change.rs:149` (0x04 sent directly) | act-change §1 r4 | doubled source / out of order | 0x04 sent inside the act change instead of by the next tick's client pass when the new room is ready; the client may get 0x04 before the new room's neighbours exist |
| A3 | `act_change.rs:78-135` | act-change §1 r3 | out of order + missing | code: switch to none → leave room → 0x05 → 0x03 → placement (0x07 adds, 0x15 / 0x0D) → unit act; spec (`waypoints.md` §11): leave O → enter R → switch to none → 0x05 → client act → unit act → 0x03, 0x53 → switch to R → update / 0x15 → pets. 0x53 missing; arena test, act build, classic pet drop, disguise check, town-leave refresh, pets follow missing; the code's spawn uses the same-act placement `0x00554EA0` path, which `0x0053ACC0` does not call (no MapReveal 0x07 of its own) |
| A4 | `act_change.rs:101-135` | act-change §1 r3 | doubled | the player is re-added (0x59, part B, 0x0B) and the vitals caches reset: not in `waypoints.md` §11 (d2rs-own, REC-278); with §11's order the player stays the local player |
| A5 | `hireling_host.rs:52-69` (NPC / quest act changes queued by `request_act_change`, `wiring/action/units.rs:383`) | act-change §1, server-tick §1 r1 | out of order | NPC travel's act change runs after the tick (T3), not inside the C→S handler of the NPC menu, so it lands one tick after the request |

### Client frame (`flows/client-frame.md`)

`Bridge::frame` (`d2-client/src/bridge/mod.rs:169-212`) matches §1
r3–r5: pump once, receive every chunk (system list first), update pass
only when ticked and `in_game`, outgoing answers last. `bridge_frame`
→ `mirror_units` are chained in `PreUpdate`
(`d2-client/src/bridge/mirror.rs:53-58`) (§3 r1). Findings:

| # | Where | Flow step | Kind | Effect |
|---|---|---|---|---|
| C1 | `bridge/mod.rs:202-206` | client-frame §1 r6 | missing — **fixed** | the skill fallback (`client/model.md` §17 r4) never ran: a hand on a skill whose level dropped to 0 kept it. Now `bridge/skill_fallback.rs`, last step of a tick frame while `in_game`; tests `skill_fallback::tests::*`, `bridge::tests::skill_fallback_runs_only_in_a_tick_frame` |
| C2 | `bridge/mod.rs:169-212` (no pause test); `world_view/present.rs:612` (draws only on a new server tick) | client-frame §1 r2 | missing | no paused pass: with the Esc menu (UI state 9) open the server keeps ticking; a paused pass would also not draw (the world view needs a new tick) |
| C3 | `world_view/present.rs:612`, input in `world_view_frame` (`:630`, `:736-776`) | client-frame §1 r8 | unspecified order | UI and world clicks are handled only in frames with a new server tick (events wait in `ui.queue`); whether 1.14d handles input every pass or only in drawn passes is client-frame OQ1 |
| C4 | `world_view/present.rs:270-276` (`deliver_outputs`), `world_view/walk.rs:104-111` (`preview_walk_frame`), `world_view/monster_walk.rs:25-31` (`monster_walk_frame`) | client-frame §3 r1 | ambiguous order | three `PreUpdate` systems with `ResMut<BridgeResource>` and no order between them; `deliver_outputs` can send C→S 0x31, the walk systems change the predicted position: the order of sends within a frame is up to Bevy's executor |
| C5 | `app/death.rs:79-93`, `app/hardcore.rs:100`, `world_view/present.rs:537-545`; `app/hardcore.rs:57` vs `app/death.rs:80` | client-frame §1 r8 | ambiguous order | Esc on a dead player is read by three unordered systems (0x41, hardcore exit, Esc menu); `save_on_death` reads `DeathScreen` written by the unordered `death_screen` (one-frame lag possible) |
| C6 | `app/visibility.rs:25-40`; camera written at `world_view/present.rs:804` | client-frame §1 r5, r7 | out of order | the visibility predicate the position check reads (`client/model.md` §6 r6) uses the previous frame's camera; 1.14d's update pass reads the camera of the last draw too, so the effect depends on 1.14d's camera write point (`render/camera.md` §3); listed for the camera owner |
| C7 | `app/play.rs:108` and `:530` (`add_visibility`), `:104` and `:521` (`GameAudio`), `world_view/present.rs:269` and `app/ui.rs:154` (`UiSounds`) | client-frame §3 | doubled | registered twice; the later one overwrites; no behaviour effect found |
| C8 | `bridge/msg/session.rs:109-113`, `bridge/msg/ui_quest.rs:59-63` | save-exit §4 r1 | missing reader — **fixed** (q-fix-flow-save: `app/save.rs` `end_of_game`) | the model's `exit_requested` (0x06, 0x50 code 23) is never read by the app |

### Save and exit (`flows/save-exit.md`)

| # | Where | Flow step | Kind | Effect |
|---|---|---|---|---|
| E1 | `d2-client/src/app/save.rs:415-418` (`request_save_and_exit`: `AppExit` only), called at `world_view/present.rs:655-657` | save-exit §1 r2 | missing — **fixed** (q-fix-flow-save) | Save and Exit sends no C→S 0x69: no leave handshake, the server never runs §2 |
| E2 | `d2-server/src/adapters/session_flow.rs:387-391` (`SessionFault::NotSaved`) | save-exit §2 r2 | missing — **fixed** (q-fix-flow-save) | the leave records "not saved" instead of writing the character before 0x05 |
| E3 | `d2-client/src/app/play.rs:550-555` → `app/save.rs:351` | save-exit §2 r2, §3 | moved — **fixed** (q-fix-flow-save) | the `.d2s` is written by the client app after `app.run()` returns, reading the server's live state through the link; correct bytes possibly, but not at the spec's point (before 0x05) and never periodically (T1) |

## 3. Fixed here

C1 (skill fallback), with three unit tests and one frame test (it fails
before the change: the hand stayed on 36). No other code change: every
other finding is a seam between systems or touches several modules.

## 4. Queued (`docs/handoff/build-queue.tsv`)

| Row | Findings |
|---|---|
| `q-fix-flow-save` | T1, E1, E2, E3, C8 |
| `q-fix-flow-act-change` | A1–A5 |
| `q-fix-flow-client-pass` | T4, T5 / J3, T6, T7 |
| `q-fix-flow-after-tick` | T2, T3 |
| `q-fix-flow-join-load` | J1, J2 |
| `q-fix-flow-client-order` | C2, C4, C5, C7 (C3, C6 wait on client-frame OQ1 and the camera owner) |

## 5. Gate

See the commit message of the push (fmt, clippy on d2-sim, d2-server,
d2-client, test-fixtures, nextest on those crates, coverage --check,
spec_index --check).
