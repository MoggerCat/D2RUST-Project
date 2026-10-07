# cov-ui (coverage session, UI batch 9)

Sub-session notes: `cov-ui-quests-overlay.md`, `cov-ui-menus.md` (merged here).
Gate: tools/gate.sh skipped by coordinator order; ran fmt, clippy -D warnings,
cargo test (d2-client, d2-data), coverage.py --check (0 errors).

## Rules covered (claimable / covered now; before was 0 unless noted)
| spec | covered / claimable |
|---|---|
| ui/text.md | 54/56 (was 38 base) — §1, §2, §4 r5, §11, §14 (wformat), §15 r4-r6 (edit_box) |
| ui/inventory.md | 42/47 (was 5) — ui/inv_grid.rs, ui/gold.rs |
| ui/controls.md | 69/86 (was 0) — controls/original.rs (§1-§5, §7, §B4) |
| ui/panels.md | 84/100 — char_details (§8 r11), skilltree/stash/cube |
| ui/panels-2.md | 35/52 (was 4) — §17, §19, §20, §21 |
| ui/panels-3.md | 34/36 (was 0) — §23 cursor, §24-§28 |
| ui/messages, menus, control-panel | 50/50, 26/26, 49/50 (sub-session) |
| world/quests-status.md | 64/64 (sub-session) |
| render/overlay.md | 26/32 (sub-session) |

## Code fixes found by tests/spec
- character.rs `release`: add-button walk stops at first hit, no clearing without points (panels-2 §17 r1-r2); one existing test pre-state changed (statpts 1).
- stash_cube.rs: cube-gone close sends two 0x4F 0x17 (panels-2 §20 r4); existing test updated.
- skilltree.rs: mouse down/up follow panels-2 §19 (draw flag, no-points 0x3C path, click sounds, close pressed first); full close-offset table (§10 r6) replaces amazon-only; tests updated; `SkillEntry.passive` added.
- character panel now draws level, experience, next level (§8 r7/r11).
- waypoint latch fix (menus §1.5) from the menus sub-session.

## Left, with reason
- controls §6 r1-r11 (world click pipeline): Opus session (impl-c-client). edge-case sections: narration.
- inventory §3 r1 (item list from the world model), §6 r1 (slot rects need inventory.bin records).
- panels-2 §14 (shop tabs/mouse/Cain reset), §18 r1-r3, §22 r2/r4; panels.md §6 r3, §7 r4, §9 r1/r6-r8, §10 r7/r9, §11 r1/r6, §12 r1/r3/r5, §13 r4/r8; panels-3 §24 r5: not reached in the time box (code exists for several; need tests only).
- overlay §2 r9/r10, §3 r1/r7, §5, edge r5: seams owned by impl-c-client.

## Notes
- New modules are not wired into the Bevy layer/bridge: ui/{edit_box,wformat,inv_grid,gold,cursor}, panels/{char_details,char_inputs,skill_inputs,waypoint_rows,scroll,stash_input}, controls/original.
- PROVISIONAL points: edit_box scroll window (REC-60), waypoint tab>4, upper_latin1 table, item_graphic_visible; sub-sessions list theirs.
- Queued local check: `controls::original::tests::defaults_equal_game_exe_table` (#[ignore], D2_GAME_DIR, §B4 r2) — claimed but not yet run.
