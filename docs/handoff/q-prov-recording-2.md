# Handoff: client recording points under Wine — `claude/q-prov-recording-2`

Cloud session, 2026-10-09. Task: settle the `docs/handoff/provisional-index.tsv`
rows with `settle_kind` = `recording` whose system starts with `d2-client`
(102 rows), by recording 1.14d under Wine (`tools/cloud-game/`). REC block
570–579 (used: REC-570, REC-576). Client missiles (REC-450/451) were left to
q-fix-client-missiles-rest.

## Done

| Point | Result | Evidence |
|---|---|---|
| REC-286 (1) | settled: `0x007A520C` / `0x007A5208` / `0x007A5214` are 0 at the first tick and first draw | `traces/client/model/client-0001.json` |
| REC-94 | settled: the client tests `0x00623660` every frame of the walk and sends C→S 0x13 on the first 1; server range always 1 for it | `facts/objects/objanim-a1-town.tsv` |
| REC-440 | fixed: 0x51 init and the 0x0E code 3 handler both call `0x00624390`, which draws for client and server objects; `Cx::set_mode` (mode change) and `Cx::reinit` run `anim_setup`; test `anim_setup_matches_the_1_14d_torch_draws` | same |
| REC-45 (part) | settled: a cycling mode wraps by one subtraction of `FrameCnt` | same |
| REC-268 (1) | settled: Shift spends 70 points as 0x1F, 0x1F, 0x05 | HANDOFF REC-268 |
| REC-281 (3g) | fixed: the item's own durability, quantity, sockets go into its list (`app::items::stream_props`) | test `the_items_own_values_are_in_its_list` |
| REC-184 | fixed: `TRACK_FRAMES` 21, `BTN_HALF` [32, 35, 32], `BTN_H` 39 | `traces/frontend/frontend-options/frontend-0006.json` |
| REC-181 name filter | fixed: letters only, max 15, `-`/`_` refused at caret 0 and when one is present | `traces/frontend/frontend-menus/frontend-0001.json` |
| REC-291 window close | fixed: WM_CLOSE in a game saves the `.d2s` and ends the program (`play::run` → `PlayEnd`) | `traces/frontend/frontend-options/frontend-0004.json` |
| REC-576 | weather move counter (no live particle → no move, F stays 0) settled; chat filter settled from the spec | `facts/client/weather/a1-town-rain-start.tsv` |

Measured, fixes queued (`docs/handoff/build-queue.tsv`; several already
implemented on staging): `q-fix-client-path-exact` (the 1.14d client path
equals the server's 16.16 position at every tick and frame: 0 differences
over 1,721 ticks), `q-fix-arrival-walkout` (no walk to the 0x0D target after a
waypoint 0x15), `q-fix-client-anim-rate` (REC-430), `q-fix-skill-motion`
(REC-275), `q-fix-char-damage-block` (REC-269), `q-fix-client-item-oq3`,
`q-fix-create-hitbox`, `q-fix-create-name-caret`, `q-fix-controls-btn-width`,
`q-fix-new-char-seed` (REC-291: fresh map seed per new character).

## Not settleable in the cloud

REC-186 / REC-223 (Bink videos do not play under Wine), REC-222 (the loading
screen lasts under a second), REC-185 (draw-side only), sound table ST-4 /
ST-7 (`-ns`, no audio device), REC-268 (2) y offset (no window manager: needs
a native Windows run).

## Open

- REC-570: cave-warp and Town Portal arrival walk-outs (`record_walk.py`).
- REC-97, REC-108 rest: need a scripted fight (`autostart` has no fight loop).
- REC-236 (loading draw calls), REC-126 (hardcore death), REC-441 (no town COF
  with a differing weapon class), REC-158 grip 2, hireling 0x81 (d2s-tool
  cannot write a mercenary).
- REC-45: `FrameCnt` 0, end of a non-cycling mode 1, Baal's portal (Act V save).
- REC-440: a 0x0E code 3 into a different mode (one draw or two).
- PC 1 / Ghidra: `docs/handoff/pc1-data.md` Step 4, the interact range
  formula `0x00623660` (REC-94).

## Act II playthrough

Assigned by the coordinator after the recordings; not started in this
session. State reported by the coordinator: `tools/playthrough/playthrough.py
traces/playthrough/act2.play` at 12/15, next blocker Radament (missile pokes
in Sewers 3 never appear after a `pos` sweep; check whether the player's room
goes stale after `pos`).

## Tools added

`tools/trace-recorder/`: `record_walk.py` + `convert_walk.py` (walk-raw-1;
the writer now closes the JSON object and re-parses its output: the first
two traces were written without the closing brace, fixed),
`record_objanim.py` + `objanim_facts.py`, `record_anim.py` + `anim_facts.py`,
`panel_text.py`, `menu_trace.py`; `autostart.py` 0.3.0 ops `char`, `state`,
`close`, `wstr` and the BitBlt screenshot fallback.

## Repro

```sh
sh tools/cloud-setup.sh; tools/cloud-game/setup_winpy.sh; tools/cloud-game/fetch.sh
export D2_GAME_DIR=$HOME/game; tools/cloud-game/prepare_saves.sh
WINEPREFIX=$HOME/.wine-d2 D2_DISPLAY=:99 tools/cloud-game/run.sh --python --seconds 300 -- \
  tools/trace-recorder/record_walk.py --game "$D2_GAME_DIR/Game.exe" --auto ScnAma --seed 1234 --input "..."
```

Exact commands are in each trace's `recorded.command` and each facts file's
header. Disk note: the d2-client integration test binaries are about 200 MB
each; on a small disk run the gate in batches (`--test` groups, deleting the
executables between batches).
