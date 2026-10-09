# Spec: Tools — Autoplay bot (plays d2rs through the real client input)

- **Status:** draft: the host protocol (§1, §2) is implemented by
  `d2-client autoplay-host` (`crates/d2-client/src/app/autoplay_host.rs`)
  and driven by `tools/autoplay/autoplay.py` (§3, §4). d2rs only: no
  1.14d side. A reached milestone means "a player could get there in
  d2rs", never a fidelity check (CLAUDE.md rule 10).
- **Target version:** 1.14d (the quest steps it plays); the protocol and
  the bot are d2rs-own.
- **Crate/module:** `d2-client::app::autoplay_host`; `tools/autoplay/`
  (Python stdlib).
- **Related specs:** `tools/playthrough.md` (milestones and the
  `playthrough-result-1` keys reused in §4), `tools/state-snapshot.md`
  (the `snap` object of `state`), `world/quests.md` §1.9 (the quest
  steps and the done bits), `ui/controls.md` §6–§7 (the world clicks the
  pointer events become), `ui/menus.md` (NPC menus), `ui/panels.md` §13
  (waypoint menu).

## Summary

The honest "playable start to finish" test. A bot plays a d2rs game from
a save the way a player would: it moves the mouse, clicks and presses
keys; nothing else. No pokes, no warps, no stat edits, no C→S message
written by hand. It reads the game state each step to decide (where it
is, where the next quest target is, what is hostile and near), and
reports where a real player would get stuck: the frame, the level, the
position, its last 20 actions and the state.

The game side is the `play` client, headless (`autoplay-host`): Bevy's
`MinimalPlugins` with the app `d2-client play` builds
(`play::add_live_client`), the in-process server thread, and the bot's
input put into the UI queue where the window's input goes. A click
therefore passes the panels, the hover pick and the world-click
dispatcher (`ui/controls.md` §6), exactly as a mouse click.

## Inputs

| Name | Type | Source |
|---|---|---|
| install | `D2_GAME_DIR` / `--game-dir` | the user's 1.14d files |
| character | `.d2s` (`--save`) or `--new CLASS NAME` | `d2s-tool new`, a checkpoint save |
| plan | the act's objective list (§3) | `tools/autoplay/acts.py` |

## Outputs / state changes

- The host reads the game only. The `state` and `map` reads run on the
  server thread between two passes and change nothing (no RNG draw, no
  list change): running them or not gives the same game.
- The bot writes a work dir (`target/autoplay/<act>-<time>/`): a
  `run.jsonl` of every command and reply summary, the stuck report
  (`stuck.json`: frame, level, position, the last 20 actions, the state)
  and a result file `autoplay-result-1` (§4).

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
   controls), then its typed character.

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
   `ground` (ground items: GUID, code, cell, gold).
2. `map`: the levels the DRLG of the player's act has allocated, each
   with id, DRLG type, rect and warp-room centres (sub-tiles), and its
   rooms: id, rect (sub-tiles), near list, warp links (target room,
   enabled, lvlwarp row). The bot uses it as a player's knowledge of the
   map (where the exits are), never to move: it still walks there.

### 3. The bot loop (`tools/autoplay/autoplay.py`)

1. **Objective**: the act's next quest step (`world/quests.md` §1.9 for
   the done bits) gives a target: a level to reach, a unit to kill, an
   NPC to talk to, a waypoint to take. The route to a level is a path
   over the `map` rooms (near lists and warp links) from the player's
   room; the next room on it gives the next point to walk to.
2. **Walk**: a left click on a frame point about 8 sub-tiles ahead along
   the route (the client's own pathing walks there); a warp tile is
   clicked on its unit.
3. **Fight**: the nearest hostile monster within a radius is attacked by
   a left click on it (or the class's main skill with a right click); a
   belt potion key is pressed under 40 % life or mana; potions and gold
   on the ground near the player are clicked.
4. **Town**: the quest NPC is clicked, its menu row `Talk` chosen, the
   dialog skipped with clicks; a waypoint is clicked and its menu row
   chosen.
5. **Stuck**: no progress toward the objective (route distance not
   shrinking) for N seconds of game time, a death loop (three deaths
   without progress), the host exiting or panicking. The report holds
   the frame, the level, the position, the last 20 actions and the
   state.

### 4. Result `autoplay-result-1`

Per act: `act`, the milestones of `tools/playthrough` reached in order
(`reached`, `total`), `ticks` (game time taken), `deaths`, and the first
stuck point (`stuck`: milestone, frame, level, position, reason) or
`null`.

## Constants & data dependencies

Ids of the act plans: levels.txt rows, monstats rows, the quest slots of
`world/quests.md` §1.9 (the same ids `traces/playthrough/act1.play`
names).

## Randomness

None in the host. The bot is deterministic for a given save and seed
(its choices read only the state).

## Edge cases & original bugs

None (a d2rs tool).

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| `parse_command` lines | `autoplay_host` unit test | this spec §1 r2 |
| `autoplay.py --selftest` | the route, projection and stuck logic on fixed inputs | this spec §3 |

## Provenance

d2rs-own tool. The input path is the one `play` uses
(`world_view::present::ui_input`, `script_input`); the headless app is
the one the `play_smoke` test runs.

## Open questions

1. The hover pick headless reads the event position (`ui/controls.md`
   §6, decision D1 of the preview): a click on a unit picks it as the
   window does only while the preview's pick is on.
