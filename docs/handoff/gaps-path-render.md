# Gap tests: path-placement §1–§6, pathing, camera, sprite-placement

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: branch `claude/gaps-path-render`, based on
`claude/tender-meitner-mphas3` at `002b244`, then merged with its later
head (main `5cc2cfb` + render-wire, incl. `wire-path-sim`'s reshaped
`path::walk`); the tests were ported to that API with every assertion
and claim kept. Cloud, repo only, synthetic
grids (M09). Unit-tier tests only, so no rule became "verified"
(CLAUDE.md rule 10). Each claim names only what its test asserts.
`path-placement.md` §7–§12 belong to `gaps-new-specs` and were not
touched.

## Coverage (any tier, units covered / units, `coverage.py --summary`)

| Spec | Before | After |
|---|---|---|
| specs/sim/path-placement.md | 77/84 | 77/84 |
| specs/sim/pathing.md | 106/125 (84.8%) | 114/125 (91.2%) |
| specs/render/camera.md | 11/11 | 11/11 |
| specs/render/sprite-placement.md | 7/9 | 7/9 |
| repository total (any) | 2819/3106 (90.8%) | 2827/3106 (91.0%) |

"Before" is the merged base without this branch's file (the base itself
gained path-placement §2.5 and pathing §9.1 from `wire-path-sim`).

## Tests

One new file, `crates/d2-sim/src/path/walk/tests/gaps.rs` (8 tests), and
one line `mod gaps;` in `walk/tests/mod.rs`. It reuses the existing fakes
(`tests/fake.rs`) plus a small `Recorder` unit side that records the
`PathInfo` the unit-side path functions receive.

| Test | Claims (`pathing.md`) |
|---|---|
| `path_functions_receive_the_path_info_record` | §3 text (every field of the record; slack 1 for a point, 2 for an item target) |
| `toward_with_a_direction_offset_runs_the_circling_function` | §5.2 text (types 5, 6, 12 with offsets +2/−2/−4 call `0x00679B30`; index and count := 0; type 2 does not) |
| `astar_target_room_check_needs_a_free_probe_cell` | §7 r1 |
| `velocity_setter_marks_a_change_and_sets_the_max` | §8.1 r3 |
| `unit_distance_table_and_formula` | §9.5 text (`dist8_unit` rows, size 3 and size < 2 adjustments, the 2·max + min branch) |
| `one_step_clears_collided_and_only_a_monster_with_flag_0x10_repaths` | §9.6 r1 |
| `arrival_passes_for_circling_types_past_the_last_point` | §9.5 r1 |
| `room_recache_can_leave_a_non_missile_without_a_room` | edge case 11 |

M08 by hand: dropping the §7 r1 check, the §9.5 r1 shortcut or the §9.6
r1 `collided := 0` each fails exactly its test.

## Code fixes

None. No test showed the code deviating from a rule it states
unambiguously.

## Not claimed, with reasons

- **path-placement §2.5**: claimed by `wire-path-sim` on the merged base.
  **§edge-cases text**: "Reproduced by default." only.
- **pathing §1.3 text**: the mode table is asserted by
  `tests/mod.rs::mode_check_rules`, but E (smallest positive type-1
  timer expire, `0x005415A0`) is the seam `first_type1_expire`, which
  no code implements yet; claiming the text would overstate.
- **pathing §8.1 text, §8.3 text**: callers of `0x00623F50` (integration)
  and the `tan` table's "x² + y² ≈ 4096²" (not an exact rule; the rows are
  checked by `path_tables.py`).
- **pathing §9.2 r5, §10 r4, §10 r5** (§9.1 now claimed by
  `wire-path-sim`): host-only history, owners elsewhere (as `impl-walk.md`).
- **pathing §9.2 r2**: the state-13 call is a seam; whether the step
  continues after it is `impl-walk.md` question 5, so a test could only
  assert half the rule.
- **pathing §9.6 r2**: unreachable through the public API: §9.4 rule 2.2
  skips one step when the velocity vector is (0, 0), and `one_step` is
  private. Spec question below.
- **pathing edge cases r1, r2**: an A* run where a cheaper open node's
  missing re-sort changes the result, and a propagation chain over 200,
  need hand-derived A* traces; not built here. Edge text: "Reproduced by
  default."
- **sprite-placement §1, §6**: driver path and field offsets (decoders,
  `dc6.md`) and transparency through the encodings (open question 1, the
  queued game-file count), as `render-camera-placement.md` said.
- **camera.md**: already 11/11.

## Spec questions

1. **pathing §9.6 r2 vs §9.4 r2.2:** movement goes to reset when the
   velocity vector is (0, 0), so one step's own "velocity vector 0"
   branch has no caller in this spec. Is `0x00650660` called from
   elsewhere (missiles, knockback), or is the branch dead in 1.14d?

## Gate

`sh tools/gate.sh` (all steps, nextest), 2026-10-06 on this branch after the base merge: GATE: PASS (d2-sim + conformance 1556 passed, rest 567, d2-client 360; coverage 3,869 claims, 0 errors); `cargo check --workspace --all-targets --keep-going` clean.
