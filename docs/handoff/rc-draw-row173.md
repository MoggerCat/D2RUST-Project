# rc-draw-row173 hand-back (REC-2710..2719 unused)

Base: specs-staging-7 + integ-r23. Draws checks: 21 gen (`gen-ui-*`, `gen-render-*`, `gen-fmt-draws-town`) + `draws-town-arrival-ama`. EQUAL draws 0 -> 0 (the 2 packets checks walkclick/beltuse: MATCH, unchanged). Ledger part: 187 rows (185 DIVERGED, 2 EQUAL).

## Cause and fix (row 173)

1.14d's CelDraw at row ~173 (`0x0046E539`) is overlay 72 `npcalert` (NPCSpeechBalloon) on Warriv, drawn by `0x0046E300` in the host's front call. 0x8A's kind-3 create (`0x004B3380`) was recorded by the UI and never reached the renderer. Now the UI queues ordered `OverlayCall`s (also the 0x28 remove, msg-ui §16 r4.1). `world_view::missiles` runs them through the `render/overlay.md` engine (`world_view/overlay.rs`, which nothing used before) before the update's advance, and draws each record with the host's unit tag. Captured timing is in the spec: create, then 1 advance in the same update, rate 144. In all 13 town checks the row is now equal (dir 47, frame, x/y). Test: `a_ui_overlay_call_creates_advances_draws_on_its_host_and_removes`.

## Next causes (by ledger rows)

- 139 rows, all town checks (row 174-177, "1.14d unit vs d2rs CelDraw"). Two parts, size S-M: (a) object 2:6 (class 78) gets a unit-draw row with no cel in 1.14d (§5 r17 "unit draw alone"); d2rs emits none. (b) `ItemTag::Unit` carries the GUID only, so monster 1:3 and object 2:3 share one run in `facts/export.rs::unit_row` (no unit row before the rb object cel).
- 14 blood-moor, 12 firebolt (row 98 CelDrawShadow file: player shadow `amlglitnu1` component), 11 frozen (row 108 shadow dir 46 vs 0), 3 den-of-evil (shadow frame), 3 wp (FloorTileDraw frame row 1), 2 kurast-rain (x), 1 stash (shadow frame).

## Notes

- `HostEnv::roll` returns 0. The UI's creates (kind 3, a = b = 0) never roll. A future create site that rolls needs the local player's seed.
- The overlay's cel direction is file direction 0 (`npcalert` has one direction). A multi-direction overlay needs the host's dir64.
- Tests: `cargo nextest run -p d2-client --lib` passed 2315/2315. The integration test binaries were not built (disk).
