# rc-draw-row173 hand-back (REC-2710..2719 unused)

Base: specs-staging-7 + integ-r23. 22 draws checks (21 gen + `draws-town-arrival-ama`). EQUAL draws 0 -> 0 (packets walkclick/beltuse MATCH, unchanged). Town checks: first difference row 172-175 -> 188-197. Ledger part: 187 rows (185 DIVERGED, 2 EQUAL).

## Fixed (round 1: row 173; round 2: rows 174-190)

- Row 173: overlay 72 `npcalert` on Warriv (`0x004B3380` create, `0x0046E300` front call). The UI queues `OverlayCall`s (0x8A create, 0x28 remove); `world_view::missiles` runs them through `world_view/overlay.rs` and draws them in the host's run. Spec: `render/overlay.md`.
- Row 176: a listed unit without a pose (object 78 `invisible town sound`, no COF) is the unit call alone. New hook `ViewRules::unit_listed_key`. Spec: `draw-order.md` §5 r4.
- Row 177: the export started runs by tag (GUID only), so monster 1:3 and object 2:3 merged. A run is now one draw slot. Spec: `tools/facts-render.md` §5 r1.
- Row 180: objects with `Draw` (+0x150) 0 (385 `cain start position`): `0x00471EC0` returns before the cel, but the shadow still draws (row 112). New hook `unit_draws_body`; `UnitLooks::object_no_draw`. Spec: `unit-composite.md` §8.

## Next causes (by ledger rows)

- 122 rows (11 town checks, row 188/191): torch 2:5 (class 37, mode 2, client 9568,73184, cell (9,27) of 34) has a unit row in 1.14d with no cel. d2rs does not list it, so it is missing from its room's unit list or near rooms (the grid cell is fine). Next step: dump d2rs `Lists` / room units for tick 57. Size M.
- 17 rows (town-dawn/night, row 197): DrawLine x 734 vs 242/237. Size ?.
- 14 blood-moor, 12 firebolt (row 98 player shadow file), 11 frozen (row 108 shadow dir), 3 den, 3 wp, 2 kurast, 1 stash.

Tests: `cargo nextest run -p d2-client --lib` 2317/2317; clippy clean. Integration test binaries were not built (disk).
