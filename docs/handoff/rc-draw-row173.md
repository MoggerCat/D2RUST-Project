# rc-draw-row173 hand-back (REC-2710..2719, REC-3250..3269 unused)

Base: specs-staging-7 + integ-r23. 22 draws checks (21 gen + `draws-town-arrival-ama`). EQUAL draws 0 -> 0 (packets walkclick/beltuse MATCH, unchanged). Town checks: first difference row 172-175 -> 195-198 (rain). Ledger part: 187 rows (185 DIVERGED, 2 EQUAL).

## Fixed (round 1: row 173; round 2: rows 174-190; round 3: rows 188-194)

- Row 173: overlay 72 `npcalert` on Warriv (`0x004B3380` create, `0x0046E300` front call). The UI queues `OverlayCall`s (0x8A create, 0x28 remove); `world_view::missiles` runs them through `world_view/overlay.rs` and draws them in the host's run. Spec: `render/overlay.md`.
- Row 176: a listed unit without a pose (object 78 `invisible town sound`, no COF) is the unit call alone. New hook `ViewRules::unit_listed_key`. Spec: `draw-order.md` §5 r4.
- Row 177: the export started runs by tag (GUID only), so monster 1:3 and object 2:3 merged. A run is now one draw slot. Spec: `tools/facts-render.md` §5 r1.
- Row 180: objects with `Draw` (+0x150) 0 (385 `cain start position`): `0x00471EC0` returns before the cel, but the shadow still draws (row 112). New hook `unit_draws_body`; `UnitLooks::object_no_draw`. Spec: `unit-composite.md` §8.
- Rows 188-191: the missing unit was Gheed 1:4, not torch 2:5. The monster walk track (`bridge/motion.rs`) never ran the room recache (`0x0064FAD0`, `sim/pathing.md` §9.6 r9), so Gheed stayed in the room of tiles 960-967, which fails the draw order's room test. Spec: `sim/unit-order.md` §5 r6.

- Rain (pass 9): the cursor steps the shared client seed after the weather update; the rain colors use the environment record's day period. `play --frame-schedule` (written by `scenario_diff.py` from the capture) replays 1.14d's drawn ticks and host clock (`tools/scenario-diff.md` §3 r7 step 5; settles REC-510 and REC-2440 for checks). Overlay calls are stamped with their delivery tick for the catch-up. On `draws-town-arrival-ama` the seed equals the capture's through tick 25, and the rain lines are equal on ticks 4-24.

- Round 4: cursor jump (`ui/panels.md` §4 r3: move at (x', y − 3), mouse starts at (320, 240); REC-3470 provisional), `play --no-sound` from the capture's `-ns`, host pointer ignored in check runs (`tools/scenario-diff.md` §3 r7 step 6). The ama client seed equals the capture's through tick 73, and the rain rows match.

## Next causes (by ledger rows)

- 26 rows: draw rows all equal, frame `index_sha256` differs (pixels: rc-sysrender-div).
- Town/ui checks at rows ~244-284 (DrawBox / CelDrawColor / menubutton): the mini panel starts open in d2rs (no registry) and closed in 1.14d (`Diablo II\Mini Panel` on the recording host, `ui/control-panel.md` §9 r9). Needs a decision: the host registry as a check input.
- 5 ui-draws checks with scripted input still differ at the rain rows (left/right-skill-pick, minipanel, cube, inv-item): not traced yet.
- 14 blood-moor, 12 firebolt, 11 frozen, 3 den, 3 wp, 2 kurast, 1 stash: rc-sysrender-div. Item-tip scout: rc-c011-gameseed.

Tests: `cargo nextest run -p d2-client --lib` 2330/2330; clippy clean. Integration test binaries were not built (disk).
