# Handoff: impl-app-gamestart (2026-10-07)

Cloud implementation session. Base `claude/impl-session-flow` @ `691480e`
with `origin/claude/specs-staging-5` @ `2675253` merged in (clean merge,
`7145643`); branch `claude/impl-app-gamestart`. Task: HANDOFF §1 row "3
not implemented" item "a game-creation sequence in the app", per
`docs/handoff/impl-session-flow.md` "What the app (`d2-client`) will
need", `sim/intents-events.md` §8, `client/bridge.md`.

## What landed

1. **The app's game starts through the server session flow**
   (`d2-client/src/app/single_player.rs`). `build` / `build_with` no
   longer call `SimGame::join`, `create_game`, `enter_game` or allocate
   the player. They build the world as before (acts, streamed rooms,
   waypoint object, path provider), then `set_session(SessionFlow::new(
   0x0010_0004, loader))`. There is no client record and no player until the
   client's C→S 0x67 / 0x6B are drained.
   - `create_request()`: the local client's 0x67 (class 1, name
     "Sorceress", Normal, flags = bit 20 | bit 2, locale 0). It passes
     §2.5's stated checks.
   - `Character::{New, Save(Box<D2s>, LoadContext)}`. This is the loader of
     §8.2 rule 2. It allocates the player of the request's class (no room,
     (0, 0), mode 1). For `New` it stages Cold Plains' waypoint on Normal
     (the server tests' staging) and gives `Entry::new(0, char_name)`. For
     `Save` it runs the new `d2_server::adapters::session::load_save` on the
     player and gives the save's act and name. In both cases it sets the
     player fields and returns `Loaded`. What did not apply goes to
     `LocalSeams::log`.
   - `local_player(&Sim)`: the joined player and its GUID. `LocalGame` /
     `Started` no longer carry `player` / `player_guid`, because the player
     exists only after the join.
2. **0x67 from the client** (`app/play.rs`). New `send_create_game(app)`
   sends `create_request()` through the bridge's send path (system queue)
   before the first frame, and `play::run` calls it. 0x6B needed no new
   code: the bridge already answers 0x02 with 0x6B (`client/model.md` §7
   rule 3, `world.outgoing` → `send_outgoing`). The real sequence is:
   frame 1 drains 0x67; frame 2 (tick 1) receives 0x01 0x00 0x02 and
   answers 0x6B; frame 3 drains 0x6B (join), and tick 2's flush carries
   0x59 … 0x03 0x53 … 0x7E and 0x04.
3. **`d2-server`**:
   - `session::load_save(s, player, save, ctx) -> (Entry, LoadReport)` is
     the load part of `enter_game_from_save`, which now calls it (no
     behavior change).
   - `character::LoadError::result()` maps errors to results through
     `formats/d2s.md` §10 r1's table (`GolemSkill`, internal 23 → 10). The
     loader returns it for a save that does not load.
   - `single_player::LOAD_FAILED` (`u32::MAX`) is a d2rs-only diagnostic for
     a failed player allocation. It is documented as not an original code.

## Changed test expectations (each from the new real sequence)

| Test | Was | Now | Why |
|---|---|---|---|
| `app_frame_loop::frame_loop_ticks_the_server_and_feeds_the_world_view` | join + waypoint travel + 0x04 in frame 2 (tick 1); `handled` 19; frames/ticks (2,1), (3,1), (4,2), (304,302); `FrameStats` frame 2 / tick 1 | creation in frame 2 (tick 1, 3 handled), join + 0x04 in frame 3 (tick 2, 15 handled in total), travel in frame 4 (tick 3); `handled` **20**; (4,3), (5,3), (6,4), (306,304); `FrameStats` frame 4 / tick 3 | 0x67 → 0x02 → 0x6B costs one ticking frame, and the waypoint intent is sent after the join, once the player exists. **19 → 20 is not from this change**: the merged base already fails at 20 (checked by running the unmodified test on `7145643`). The extra message is the join's 0x53 that impl-session-flow added after 0x03. |
| `app_frame_loop::game()` | staged the player's interaction at start | stages nothing; the first test stages it after the join | no player before 0x6B |
| `app_client_drlg::the_join_builds_the_client_drlg_in_the_app` | in game after frame 2; player / position read before the first frame | in game after frame 3; player and server position read after the join (through `tests/app_support::SharedLink`) | same sequence |
| `app_client_drlg::the_session_join_on_the_install` (ignored) | two frames | sends 0x67, three frames, then reads the player | same; **queued for a local run** (below) |
| `app_frame_loop::frame_loop_runs_on_the_users_levels` (ignored) | — | sends 0x67, asserts in game with a local player | now exercises the live join |
| `app_single_player::the_build_is_deterministic_and_runs_on_its_thread` | the local client joined and the player had fields at build | session flow set, no client record, no player | no join at build |

New: `app_single_player::the_session_flow_creates_the_game_then_loads_the_character_at_the_join`
and its M08 `a_refused_create_request_starts_nothing`; `d2-server
character::tests::load_errors_map_to_their_results`. Tests that use their
own feeds (`frame_loop_draws_each_tick…`, `…shakes…`, `…frame_store…`,
`app_original_ui`) need no join and are unchanged. The e2e games
(`e2e_*`, `game_wired_host`, `synthetic_game::run`) build their own games
and are untouched (7v (f) still open).

## Not done (blocked, not guessed)

- **`ActionHooks::object_drops` and the hireling tables in the app** (7v
  (c), `impl-world-rest.md` §4 items 9–10). The app's game is
  `SimGame<ActionSim<LocalSeams>, ActionWorld>`, which has no object
  control (`create_objects` is not called; the waypoint object is
  allocated by hand), no `WiredWorld`, and so no `InteractionState` to hold
  `hireling_tables`. There is also no production loader for `DropTables`:
  `ItemTables`, `TreasureClasses`, the item list and `superuniques` from the
  user's files exist only in tests. Installing them needs the 7v (c)
  migration first: `WorldSim` + `WiredWorld`, game creation in the
  `rng.md` §5.2 seed order (regions, objects, NPC, quests), production item
  and treasure loaders (items area: impl-items-rest), and one shared
  unique-bit store (`impl-world-rest.md` §4 item 9).
- **A save from the command line**: `Character::Save` takes a parsed
  `D2s`. `d2s::read` needs a `SaveTables`, and none exists in production
  (items area). There is no `--save` flag yet.
- The §8.3 join messages and the other gaps of `impl-session-flow.md` are
  unchanged.

## Spec gaps (named in code)

| File | Section | Missing |
|---|---|---|
| (client menus, no spec) | — | the client's 0x67 sender: game name, game type, template, arena, bytes 43–44 (zero here; no server rule d2rs runs reads them) |
| `formats/d2s.md` / `intents-events.md` | §8.2 r3 | a new character's player record (`0x00532590`): `Character::New` gives no record (no 0x5F / 0x23) |

## Local run queue (add to HANDOFF §5)

- `D2_GAME_DIR=<install> cargo test -p d2-client --test app_client_drlg -- --ignored the_session_join_on_the_install`:
  expect PASS (the player in a level-1 room at the server's point, ≥ 2
  rooms in sight) through the 0x67 / 0x6B sequence.
- `D2_GAME_DIR=<install> cargo test -p d2-client --test app_frame_loop -- --ignored frame_loop_runs_on_the_users_levels`:
  expect PASS, (101, 100), and in game with a local player.

## Results

`CARGO_INCREMENTAL=0 sh tools/gate.sh`: **GATE: PASS** (every step, d2-client included) on this branch before the push.
