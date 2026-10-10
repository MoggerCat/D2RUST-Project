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

## Cause of the 36 (found, not fixed)
First left-click cast after the waypoint (tick 252, `click 330 190`): 1.14d's player goes to mode WL (walk) after 1-2 ticks, d2rs
casts at once (SC / A1 / S2, 0-1 ticks). The next click (264) casts in 1.14d. Right-click groups (Frozen Armor, Might)
cast on the first click in 1.14d too. So the player ends the scene at another position (camera origin 7729,78285 vs
7760,78336 for Fire Ball). Likely input state after the waypoint menu click (172) or left-skill state on the new level;
needs the 1.14d input path (Ghidra) to settle. Owner: claude/q-tool-state-diff (input) / client; size M.

## Changed
- `tools/sidebyside/build.py`: `SBS_STOP_MARK=<mark>` records only up to that mark and keeps the scenes recorded;
  `SBS_SLOW=<factor>` seconds-per-tick allowance. No game logic changed.

## Open
- Walk-vs-cast on the first left click (above), size M.
- Frozen Armor / Might: draws row 97 (aura / overlay art order), size S.
- 40 later-mark scenes to re-run once the first difference moves.
