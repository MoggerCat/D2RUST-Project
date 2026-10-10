# q-chk-frontend: front-end screens vs 1.14d (REC-1550..1554)

Tool: `tools/frontend-sbs/frontend_sbs.py` (X screenshots of 1.14d under Wine on :99 and of
`d2-client play` on :98 with lavapipe, same xdotool script, 800x600 crops, per-screen diff and
a 1.14d | d2rs | diff page). Pages (Blizzard art) are in the PRIVATE repo:
`reports/side-by-side/2026-10-09-q-chk-frontend/`.

Repro: `sh tools/cloud-setup.sh`, assemble the install to `$HOME/game` (+ symlink `/root/game`),
`tools/cloud-game/setup_winpy.sh`, `prepare_saves.sh`, `cargo build --release -p d2-client`,
`python3 tools/frontend-sbs/frontend_sbs.py /tmp/out` (OUT outside the repo). Note: if
`~/.wine-d2` was created before `wine32:i386` was installed, `syswow64` is empty and Game.exe fails
with "could not load kernel32.dll c0000135": delete the prefix and run `wineboot -i`.

## Result (no screen is exact; first difference per screen)

| Screen | First difference | REC |
|---|---|---|
| main menu | Open Battle.net button label ("GATEWAY: ..." in 1.14d) blank in d2rs | 1554 |
| character select | paper dolls missing in slot figures | 1552 |
| char select after Esc from create | wrong (blue) palette in d2rs | 1551 |
| create (7 classes) | "EXPANSION CHARACTER" label beside the check box missing | 1553 |
| credits, cinematics | layout equal by eye; brightness differs | 1550 |
| every screen | per-pixel brightness/palette offset (e.g. 96 vs 104, 28 vs 20); q-fix-ui-blend may fix | 1550 |

## Open
- Input is wall-clock, not frame-anchored: snow/fire animation differs, so a zero diff is not
  reachable yet; needs tick-anchored input (`specs/tools/scenario-diff.md` §2 r4) before a "pass".
- Not covered: options tree, controls, difficulty popup, trademark, per-class hover text, loading.
- Differences judged by eye apart from the pixel counts; none of the screens is settled EQUAL.
- Ledger rows: `docs/handoff/ledger/q-chk-frontend.tsv` (6, DIVERGED; no `.check` file yet).
- Owner of fixes: front-end menus (coordinator, owners.tsv).
