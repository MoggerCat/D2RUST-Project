# rc-c009-scenes: C009 ui.frontend (6 rows) -> a real check

Checks before/after: ui.frontend rows had no check (state DIVERGED by hand, sidebyside); now all six
link `ui-frontend-screens` (channel `frontend`, `traces/checks/ui-frontend-screens.check`). EQUAL 0 -> 0
(rows stay DIVERGED, now with a measured first divergence); stable-pixel differences per screen:
main 26500 -> 918, cinematics 49732 -> 372, credits 46388 -> 374, charselect 33651 -> 2916.

## What changed
- `tools/frontend-sbs/frontend_sbs.py`: `--script screens` (per screen a shot series: 6 on 1.14d, 40 on
  d2rs), crop offsets measured (1.14d window origin y = 97), d2rs waits 6 s (`FE_WAIT_OURS`) so the
  trademark screen is still up when the script starts (at 12 s d2rs had already left it: one screen ahead).
- `tools/frontend-sbs/screens_check.py`: per-pixel median per side; compared where stable on both sides at
  tolerance 0; animated pixels counted, not judged. `frontend_channel.py` picks it for `ui-frontend-screens*`.
- Fix 1 (`front_end/mod.rs image_tiles`): last tile row of a multi-row image ends on the control's bottom row
  (y = 599), not 600. Rows 512..599 of every 800x600 background were one row low. PROVISIONAL other sizes.
- Fix 2 (`screens/cinematics.rs`): heading centred (flag 2) and colour 4 (0x004FD060).
- Spec: frontend-menus.md §F2.11, scenario-diff.md §3 rule 17. Ledger part `ledger/rc-c009-scenes.tsv`.

## Open (first divergences after this, by size)
1. Mouse cursor: 1.14d draws it into the frame on every front-end screen, d2rs does not (372 px of the
   cinematics/credits remainder; part of every other screen). Needs the front-end cursor cel/frame rule (S-M).
2. `GATEWAY: <name>` label: 549 px, glyph rows 1 px apart in the lower rows (S).
3. create screens 377..16264 px: class sprite animation phase and fire (wall-clock; M, needs a phase-locked capture).
4. charselect slot figures (paper dolls, `ui-charselect-dolls`).
5. NOT done: C009 render.scene (8), system.audio (2+), system.replay (2) and C019 ui.scene (11 rows): untouched.
   Not mine: `app::state_dump::tests::options_parse` fails on integ-r23 (frame 2; key i).
