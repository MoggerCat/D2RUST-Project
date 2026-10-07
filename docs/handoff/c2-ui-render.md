# Handoff: c2-ui-render (coverage session, specs/ui + specs/render)

Branch `claude/c2-ui-render`. Test results: see "Verification" at the end.

## Counts (uncovered rules, ui + render files; skill_desc.rs left to another session)

Before: 73 uncovered rules (capture 2, draw-order 4, lighting 2, map-preview 2,
overlay 6, sprite-placement 2, unit-composite 7, automap 1, control-panel 1,
controls 6, inventory 5, panels-2 17, panels-3 2, panels 15, text 1).
After: see `python3 tools/coverage.py` (ui/render rows); the list of rules
left is at the end.

## Code added (all from the spec, no spec changed)

- `ui/panels/npc.rs`: record lookup (panels-2 §14 r7), Resurrect insert/remove
  (`resurrect_edit`, §14 r2), talk-end decision (§14 r8), Cain count reset
  (§14 r10), the 0x2F/0x30/0x31 byte builders (§14 r9). Tests in
  `ui/panels/npc/tests_c2ui.rs`.
- `ui/panels/shop.rs`: shop button records per NPC (§14 r11), start page and
  tab setter (§14 r12). Tests in `ui/panels/shop/tests_c2ui.rs`.
- `ui/panels/inventory.rs`: `in_panel_area` (§18 r2). Test in
  `ui/panels/tests_c2ui.rs` (also the waypoint close hover, panels §13 r4).
- `world_view/overlay.rs`: `overlay_light` (overlay §2 r9), `overlay_draws`
  (unit-composite §5 r4), `kind6_clock_ms` (overlay §3 r7). Tests in
  `world_view/overlay_c2ui_tests.rs`.
- `rules/unit_composite.rs`: `dir_source` (unit-composite §3 r1).
- Claims added to existing tests: controls edge cases r1–r5, belt hit area
  (panels-2 §18 r3), shop panel / messages (§14 r4, r5).
- `ui/tests_c2ui.rs`: inventory edge cases r1 and r3.

No existing code had to be changed to match a spec.

## Notes

- panels-2 §14 r4 is claimed by the shop art/tab/button tests; the store-grid
  draw is still not in `ShopPanel::draw` (module Open note), so r4 is only
  as complete as that.
- panels-2 §14 r3 (menu box draw), §14 r13 (shop mouse) are not implemented
  in this session.

## Exempt candidates

(one per line: spec TAB rule TAB reason)
specs/render/capture.md	§1	recorder configuration of the original game; checkable only in the game process
specs/render/capture.md	§2	recorder hooks and addresses in the original game; not behaviour of d2rs
specs/render/draw-order.md	§5 r2	'not in the reference renderer' (perspective pre-test): no behaviour
specs/render/draw-order.md	§edge-cases-original-bugs	aggregate list restating rules covered individually
specs/render/lighting.md	§1 r3	timing narration (rebuilt per drawn frame); verified by trace only
specs/render/lighting.md	§2 r4	pointer to §5 quality
specs/render/map-preview.md	§edge-cases-original-bugs text	list of Phase 6 simplifications (not original behaviour)
specs/render/map-preview.md	§edge-cases-original-bugs r2	simplification note: shadows not drawn
specs/render/overlay.md	§5	table of create call sites owned by other specs
specs/render/overlay.md	§edge-cases-original-bugs r5	light created before the graphics-record check: ordering inside the caller, fatal-error path
specs/render/overlay.md	§2 r10	graphics record null is a fatal error path; list push is covered by create
specs/render/sprite-placement.md	§6	index-0 transparency is exact only against live game data counts (game files)
specs/render/sprite-placement.md	§edge-cases-original-bugs r2	dead branch (L = R = 0 never happens)
specs/render/unit-composite.md	§1 text	narration
specs/render/unit-composite.md	§3 text	narration
specs/render/unit-composite.md	§8 text	narration
specs/render/unit-composite.md	§edge-cases-original-bugs	aggregate list restating rules covered individually
specs/ui/automap.md	§edge-cases-original-bugs text	aggregate list restating rules covered individually
specs/ui/control-panel.md	§edge-cases-original-bugs	aggregate list restating rules covered individually
specs/ui/inventory.md	§edge-cases-original-bugs r2	sockets only on hovered item: drawn by the GPU side, pixel check
specs/ui/panels-2.md	§edge-cases-original-bugs	aggregate list restating rules covered individually
specs/ui/panels-3.md	§edge-cases-original-bugs	aggregate list restating rules covered individually
specs/ui/panels.md	§6 r3	pointer to Open questions 1
specs/ui/panels.md	§9 r6	draw-order narration of calls in other rules
specs/ui/panels.md	§9 r7	pointer to §15 and items spec
specs/ui/panels.md	§9 r8	pointer to panels-2 §18
specs/ui/panels.md	§10 r7	pointer to Open questions 4
specs/ui/panels.md	§10 r9	pointer to panels-2 §19
specs/ui/panels.md	§11 r6	pointer to panels-2 §21
specs/ui/panels.md	§13 r8	pointer to menus.md §1
specs/ui/text.md	§edge-cases-original-bugs	aggregate list restating rules covered individually
