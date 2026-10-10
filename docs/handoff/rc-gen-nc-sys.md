# rc-gen-nc-sys hand-back (REC-3880..3899 unused)

Scope: NO-CHECK ledger rows of system.render (27), system.client (16 + 36 model/msg rows), system.ui (11) and coverage.check-gen (8).
Claim C-gen-nc-sys (not frontend = C009, not the cstate dump = C-pc1-client). NO-CHECK in this scope 152 -> 102.

## Result (part docs/handoff/ledger/rc-gen-nc-sys.tsv, 50 rows)
- EQUAL +31: 30 `system.client.model/msg-*/stat-lists` rows whose gen-sysc-client checks were re-run fresh (orig-cache) and
  qualify under REC-2055/2056 (`tools/coord/promote.py`: every channel MATCH, state PARTIAL only for the d2rs client gap),
  and `coverage.check-gen.umod` (42 gen-umod checks run for the first time, all promoted).
- Checked, DIVERGED +19: 7 coverage.check-gen family rows (ai, boss, lvl, shrine, skill, su, wp: check glob `gen-<fam>-*`),
  render rows composition.1/2/6, lighting.12/13, shading.10 (gen-render-town-dawn/night), blend-modes.8 (gen-ui-automap, gen-ui-hud),
  shading.5 (ui-draws-hover-npc-ama), ui panels.2/3/5 (new `gen-ui-conflict`), item-tips.9 (new `gen-ui-setitem`).
- check-gen: `UI_RENDER_ROWS` lets a UI scene carry system.render rows; new UI scenarios `conflict` and `setitem`;
  render scenes carry the composition/lighting/shading answer rows. `--selftest` passes (5318 checks); FAMILIES/FAMILY_FN/resolve_area unchanged.
- checks-status.md rows of 64 gen-sysc-client, 15 gen-ui, 7 gen-render, 42 gen-umod checks refreshed.

## Open (sizes)
- Every gen-ui / gen-render draws check (61 draws checks in all) first diverges at the pass-9 rain DrawLine column x (C002; 31 checks).
  Not cheap: rc-draw-row173 traced it to an extra footstep-variant roll on the client seed (sound 2768 at T22 vs T27); the cloud Wine
  capture is not a valid reference for it (pc1-data.md Step 4 [rc-audio-fmt-div]). Released my C002 claim. L, needs the Windows recording.
- 54 `system.ui.frontend-*` rows: C009 (rc-c009-scenes), frontend channel only does the paper dolls. L.
- 21 `system.client.*` rows with no scenario: d2rs-internal seams (bridge 1/3/7/8/9/10, assets/audio/render-pipeline/ui section A),
  chat/hire/quest-special/event text (msg-ui 4/6/7/15/19: no poke or input triggers the S->C message), hireling (model 14/16/17/18,
  msg-stats-items.4), msg-units.3 (second player). Proposal for the owner: NOT-APPLICABLE for the d2rs-internal ones.
- 9 `system.render.map-preview.*` (d2rs dev tool, no 1.14d counterpart): proposal NOT-APPLICABLE. 9 `system.render.capture.*`: queued in pc1-data.md Step 4 [rc-gen-nc-sys].
- system.ui menus.3/4 (hire list, shop), item-tips.11, automap.7/12, panels.15/16 (tables): need NPC menu clicks / second player; M each.

## Incident
My merge f551d6373 emptied docs/handoff/checks-status.md on integ-r23 for ~15 min (a conflict loop with `statusres.py`); restored in the next
commit from 0c3f080f7 with my rows re-applied (2020 lines). If another session merged in between, re-run `statusres.py` on its branch.
