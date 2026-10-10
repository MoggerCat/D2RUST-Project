# rc-pc1-wine hand-back (2026-10-10)

Task: the 175 NO-CHECK rows marked Windows-only / needs 1.14d facts -> settle with Wine or Ghidra what can be.

## Result (merged ledger, NO-CHECK / EQUAL / DIVERGED / NOT-APPLICABLE)
- Before (integ-r23 at start of batch 1): NO-CHECK 260, EQUAL 3455, DIVERGED 649, NOT-APPLICABLE 99.
- After batch 2: NO-CHECK 248, EQUAL 3457, DIVERGED 654, NOT-APPLICABLE 105 (the rest moved with other sessions' merges).

## What changed
- New channel `cstate` (`tools/scenario-diff/cstate_diff.py`, `scenario_diff.py`, specs/tools/state-snapshot.md §3 r5,
  scenario-diff.md rule 19): `d2-client state-dump --client-out` writes the client model's unit sets S and C
  (`app/client_state.rs`), 17 `traces/checks/*-cs.check`. 1.14d side = `traces/pc1/client-state/*.cstate.jsonl`;
  re-recorded under Wine (`D2_CSTATE_RECORD=1`): equal to PC 1's in all fields but the local player's client seed.
- Client model fixes found by it: set C units are removed with their freed room (`ClientWorld::refresh_active_rooms`),
  client preset objects get the animation set-up (frame count, speed), player / monster creation runs the animation
  set-up, a monster mode set to the current mode no longer restarts the animation.
- Ledger: rc-pc1-wine.tsv (new rows) + rc-pc1-wine-override.tsv (7 `system.client` rows re-measured DIVERGED that
  rc-gen-nc-sys had EQUAL from the server-state login check; delete the override file to restore their EQUAL).
  NOT-APPLICABLE: object.preset.574-579 (no DS1 names those classes; `game_world_data` ignored test, fresh run).
  DIVERGED: system.formats.d2s-appearance.4/5 (new `draws-equip-*` checks: the play preview gives the local player no
  equipment components). EQUAL: item.affix.automagic / charm (24 gen-itemq checks, items channel, all MATCH fresh).
  checks-status.md refreshed for the 24 gen-itemq checks (they were stale DIVERGED); `ledger.py --fix` synced
  rc-gen-wine168 / rc-run-5 to it (mechanical).
- pc1-data.md Step 4: ranked list of the truly Windows-only rows (audio 10, cinematic video hook 1).

## Open (sizes)
- cstate divergences, each its own cause (M): local player stat words on saves above level 1 (1.14d client keeps level-1
  values for the whole run, d2rs shows the save's); hireling hp at creation (1.14d 32768, d2rs 64512); walking server
  monsters (1.14d copies the server path at the client update end `0x00465070`, d2rs predicts, REC-2416); items (type 4)
  are not dumped from the client item data (cl, m, iq, ... : M); tx / ty / act / own / q not modelled (gaps, PARTIAL at best).
- Equipped items on the local player's paper doll in `play` (draws-equip-*): L.
- Rows left NO-CHECK that are cloud work (not Windows): frontend 54 (C009), items / cube / drop / inventory / vendor / skills 56
  (new check families), format-0 legacy item rows (need a version-0x47 save), d2s-load variants (golem, hotkeys, runeword
  mismatch, refused load: d2s-tool + state-dump packets).
- Audio rows stay PC 1 (rc-audio-fmt-div).
- tests: `app::state_dump::tests::options_parse` was red on integ at the first merge, green after the next.
