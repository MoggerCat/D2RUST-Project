# Spec: Tools — Autoplay host and real-input smoke routes

- **Status:** draft: `d2-client autoplay-host`
  (`crates/d2-client/src/app/autoplay_host.rs`) implements §1–§2; the
  route replayer `tools/autoplay/replay.py` and the routes in
  `tools/autoplay/routes/` implement §3. d2rs only: no 1.14d side. A
  passing route means "these clicks still do this in d2rs", never a
  fidelity check (CLAUDE.md rule 10). The autoplay bot that recorded
  the routes was dropped (2026-10-09); its findings are in
  `docs/handoff/q-tool-autoplay.md`.
- **Target version:** 1.14d (the NPCs, levels and waypoints the routes
  visit); the protocol and the route format are d2rs-own.
- **Crate/module:** `d2-client::app::autoplay_host`; `tools/autoplay/`
  (Python stdlib).
- **Related specs:** `tools/state-snapshot.md` (the `snap` object of
  `state`), `ui/controls.md` §6–§7 (the world clicks the pointer events
  become), `ui/menus.md` (NPC menus), `ui/panels.md` §13 (waypoint menu).

## Summary

A real-input smoke test. `autoplay-host` runs the `play` client
headless (Bevy's `MinimalPlugins` with the app `d2-client play` builds,
`play::add_live_client`, as the `play_smoke` test runs it) and takes
mouse and key events on stdin, put into the UI queue where the window's
input goes: a click passes the panels, the hover pick and the
world-click dispatcher exactly as a mouse click. Nothing else changes
the game: no pokes, no warps, no hand-written C→S message.

A route is a fixed list of those events with state checks between
them, recorded once from a run that reached its milestones (town NPC
talks, waypoint menus, walks between levels, waypoint travel). The
same build, save and seed give the same game, so replaying the route
gives the same clicks on the same things; a check that stops holding
is a real-input regression.

## Inputs

| Name | Type | Source |
|---|---|---|
| install | `D2_GAME_DIR` / `--game-dir` | the user's 1.14d files |
| character | `.d2s` (`--save`) or `--new CLASS NAME` | `d2s-tool new` (the route's `save` line) |
| route | text, §3 | `tools/autoplay/routes/*.route` |

## Outputs / state changes

- The host reads the game only: the `state` and `map` reads run on the
  server thread between two passes and change nothing (no RNG draw, no
  list change); running them or not gives the same game.
- The replayer prints, per route, the milestones whose checks held and
  the first failing check (line, milestone, why); exit 0 all held, 1 a
  check failed or the host died, 3 error.

## Rules

### 1. Host protocol `autoplay-1` (stdin / stdout lines)

1. `d2-client autoplay-host (--save FILE | --new CLASS NAME) [--seed N]
   [--difficulty D] [--game-dir DIR]` builds the game as `play` does,
   joins (passes until the server has the local player) and prints
   `{"k":"hello","format":"autoplay-1","passes":n}`. Diagnostics go to
   stderr only.
2. Commands, one per line (blank and `#` lines skipped):
   `step N` (N client passes; the host clock advances 40 ms after each,
   one server tick), `move X Y`, `press L|R X Y`, `release L|R X Y`
   (800 × 600 frame pixels), `key K` (a key as `input_script::vk_code`
   names it), `state`, `map`, `quit`.
3. Each command but `quit` prints one JSON line: `{"k":"ok",...}`, the
   read's object, or `{"k":"error","error":...}` for a line that does not
   parse. The game advances only on `step`.
4. Pointer events: a position different from the last reported cursor
   is first sent as a cursor move (the window path's `CursorMoved`),
   then the press or release; the click view reads that cursor. A key is
   delivered as `play --input`'s `key` step: its bound action in the
   UI's key mode (the `original` controls preset, not the user's saved
   controls), then its typed character; the key is also held in the raw
   key state for one pass (the window's input plugin keeps it; the death
   and hardcore screens read it).

### 2. Reads

1. `state`: `{"k":"state","passes","server_ticks","local":<player
   GUID>,"client":{...},"snap":<state-1 snap>}`. `snap` is
   `tools/state-snapshot.md` §1 r2 (with `q`). `client`: `player` (the
   model's local player GUID, cell, mode), `open` (the UI states 0–0x2F
   that are on), `npc_menu` (the NPC menu's GUID, class, `talking`, and
   per row its string id, option kind and centre point), `topics` (the
   talk topic box: texts, centre points, cancel point), `dialog_lines`
   (the dialog panel's line count while it is up), `waypoint_rows`
   (level, known, current of the open menu's tab), `belt` (slot, code),
   `ground` (ground items: GUID, code, cell, gold), `camera` (the drawn
   frame's anchor: the local player position the screen and the hover
   pick use, 16.16 and whole sub-tiles; clicks aim from it, not from the
   server position), `sent` (C→S message ids sent so far, with counts).
2. `map`: the levels the DRLG of the player's act has allocated, each
   with id, DRLG type, rect and warp-room centres (sub-tiles), and its
   rooms: id, rect (sub-tiles), near list, warp links (target room,
   enabled, lvlwarp row), and each active room's collision grid (`#`
   blocked for a walking player, `D` door, `.` free) and the levels' vis
   arrays. A driver may read it as a player's knowledge of the map,
   never to move: movement is still its clicks.

### 3. Routes `autoplay-route 1`

1. Line 1 is `autoplay-route 1`. `#` lines are comments;
   `# milestone NAME: TEXT` names the milestone the following checks
   belong to.
2. `host ARGS`: extra `autoplay-host` options (`--new CLASS NAME`,
   `--seed N`). `save ARGS`: the character, `d2s-tool new ARGS -o FILE`,
   passed as `--save FILE` (fixed `--time` and `--map-seed`, so the save
   is the same on every run).
3. `step N`, `move X Y`, `press L|R X Y`, `release L|R X Y`, `key K`
   are sent to the host as they are (§1 r2).
4. `check level N`: the local player's `lv` is N. `check sent ID >= N`:
   the client has sent C→S message ID (hex) at least N times (`state`
   `client.sent`; 0x2F counts NPC menus opened, 0x30 their closes).
5. The replayer makes no decision: it sends every line in order and
   stops at the first check that does not hold.

## Constants & data dependencies

None: a route holds frame pixels and levels.txt ids only.

## Randomness

None in the host or the replayer. The game's own draws are fixed by
the save and the seed, which is what makes a route replayable.

## Edge cases & original bugs

None (a d2rs tool).

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| `parse_command` lines | `autoplay_host` unit test | this spec §1 r2 |
| `replay.py --selftest` | the route parser and the checks on fixed inputs; every committed route parses | this spec §3 |
| `replay.py --all` (1.14d install) | act1 7, act2 20, act3 12, act4 6, act5 12 milestones held (2026-10-09) | this spec §3 |

## Provenance

d2rs-own tool. The input path is the one `play` uses
(`world_view::present::ui_input`, `script_input`); the headless app is
the one the `play_smoke` test runs.

## Open questions

1. The hover pick headless reads the event position (`ui/controls.md`
   §6, decision D1 of the preview): a click on a unit picks it as the
   window does only while the preview's pick is on.
