# rc-render-effect: render.effect.* rows, fresh verdicts

## Checks before -> after (EQUAL 0 -> 0 of 80; gate moved)
- The `frame.tsv` `level` difference was already fixed by rc-render-wp-click (waypoint click path; in this branch's base).
  Re-run here: all 20 effect groups (cast + flight scene each = 40 of the 80 scenes) on 1.14d under Wine and d2rs.
- Fresh verdicts (docs/handoff/ledger/rc-render-effect.tsv, 40 rows, all DIVERGED):
  - 36 scenes: first difference `frame.tsv` row 8 `tile_origin_x`, 17-19 % pixels match.
  - 4 scenes (Frozen Armor, Might: right-click casts): first difference `draws.tsv` row 97 `file` (a monster
    shadow `fs/tr/fstrlit...` in 1.14d at row 97 where d2rs draws an object `to/tr/totrlitonhth`), 77-78 %.
- The other 40 scenes (marks f3..f14) were not re-run: the host records 1.14d at about 1.1 ticks/s only for short
  scripts (the full 603-tick script timed out at 0.33 ticks/s with 3 Wine runs in parallel). Their ledger rows keep the older text.

## Cause of the 36 (found in the 1.14d exports, not fixed)
First left-click cast after the waypoint (tick 252, `click 330 190`, spell on the left button): 1.14d walks (WL), d2rs casts at
once. Cause: a left click on the ground without Stand Still is a walk (ui/controls.md §6 r7); the click hovers the unit the
*previous* pass left (cursor still at 207,176 from the waypoint row). 1.14d hovers nothing there (no text label, walks); d2rs's
d2rs-own hover (`bridge/hover.rs`, feet box 24 wide, 96 above) picks the Fallen standing below, so it takes the unit path and casts.
Same at ticks 264 / 277 (1.14d casts when the previous cursor is on a Fallen's frame, walks when not). 1.14d's test
(`0x00467AC0` loop -> `0x00466870` -> `0x00470860`) is the unit's drawn frame rectangle +-16 px, checked against the units
drawn in the last pass, plus priority by unit type; documented in specs/tools/scenario-diff.md §3.
Fix needs the per-pass drawn frame rectangles in the headless play path (today it builds frames only at the dump ticks):
size L. Owner: client input/hover (d2rs `bridge/hover.rs`, `world_view/input_script.rs`); not started.

## Changed
- `tools/sidebyside/build.py`: `SBS_STOP_MARK=<mark>` records only up to that mark and keeps the scenes recorded;
  `SBS_SLOW=<factor>` seconds-per-tick allowance. No game logic changed.

## Open
- Hover = drawn frame rectangle +-16 px (above), size L.
- Frozen Armor / Might: draws row 97 (aura / overlay art order), size S.
- 40 later-mark scenes to re-run once the first difference moves.

## Walk path after tick 260 (measured 2026-10-10, not fixed)
Per-tick `tile_origin` (every 1.14d frame, Fire Ball scene): equal on all ticks to 261 (the whole walk of the first left click),
then at the second click (`click 300 200`, cast on the Fallen under the previous cursor, F = 264, posted at tick 263):
1.14d keeps walking two more steps and TURNS (dir 32 -> 24, origin 7744,78299 -> 7738,78293 -> 7732,78290), starts SC at
f265 facing dir 24/25; d2rs keeps its walk direction (dir 32), stops after one step (7744,78286) and starts SC at tick 264.
The client sends a cast on the hovered unit at once for an `rng` skill (ui/controls.md §6 r9.3, 1.14d `range` counts: loc 0),
the server's `use_on_unit` (skills/use.md §3) uses it now, so the two ticks of walking towards the target and the turn are not
explained by those rules; needs the 1.14d S->C packets of the scene (`record_packets.py` with the scene input) to see whether the
server sends a walk/run-to-target (0x03/0x01/0x02) before the 0x4C skill message. Size M; owner: movement / skill start
(d2-sim `wiring/action`, `bridge/predict.rs`). Row impact: 36 effect rows move from `tile_origin_x` to the next difference
(units' positions / draw rows), they stay DIVERGED either way.
