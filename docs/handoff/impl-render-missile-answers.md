# Handoff: PC 1's render and missile answers in code — `claude/impl-render-missile-answers`

Cloud implementation session, 2026-10-07. Base `claude/specs-staging-2`
(`b222388`). Repo only, synthetic fixtures, no game files (M09). Wrapped
up early on the coordinator's budget cut. Everything here is still
**unverified** against 1.14d. The checks are the ones the source notes
already queued.

## Done

1. **Missile bodies, Q1–Q6** (`specs/missiles/*`, `d2-sim::missiles`):
   - Q1: the ring velocity flag 0x10 already matched. The `TODO` is now a
     plain comment.
   - Q2: `elem_roll` now returns the rolled amount (physical, fire,
     lightning, magic or cold; poison and burn × 25; else 0). Server-hit 4
     reads `EType` from the row. The fire head heals by the fire roll.
   - Q3: `full_record` sets bypass flags 103 / 104 / 106 → hit flags
     0x100 / 0x200 / 0x400.
   - Q4: `tyrael` already refreshed the room after the test. It now has
     a comment.
   - Q5: in `spawn_for_level`, no room is now fatal and the dead
     `c = −1` test is gone. The floor drop with no room gives no gold.
   - Q6: new `bodies_ext2::unit_find` + `FindFilter` (bodies-2 §44: the
     rooms, the found order and the default filter). The seam
     `corpse_units` is replaced by `room_subtiles`. In the wired `View`,
     `room_subtiles` reads the DRLG rect and `room_seed` reads the
     active-room seed (+0x6C), through the new `DrlgWorld::active_seed_mut`.
2. **Lighting / blend** (`d2-client::rules`):
   - 0x53 eclipse: the setter now follows the spec. The type comes from
     the table of the flag. The first intensity uses the previous flag,
     then the flag is set. With the flag set, the period reset runs
     (`Environment::period_reset`), then the intensity again and the
     color. `EnvError::EclipsePending` is removed.
   - Wall direction 0: `draws::WallPass` reuses the previous record's
     words in the pass. As the first wall of a pass, direction 0 is
     still `WallDirection0` (undefined in the original, so fatal).
   - GDI lines: 45° lines are x-major. `LineMajorAxisTie` is removed.
     The rectangle with x1 < x0 stays fatal; only its message changed.
3. **Draw order 2, W1–W7** (`rules::draw_order::weather`):
   - `snow_lock_hold` is removed (`[0x007A8A20]` is always 0).
   - Phase entry: fatal 0x12A when p > 3. A locked entry with p ≠ 2
     keeps the stored phase, and r3 ramps with this call's p.
   - With rain off, the intensity is 0 and the target is kept.
   - The snow wind goal is rolled.
   - Shape values are exact integer floors. The snow spawn adds the
     extra raw step, size, bounce 1 and `snow_color` (alpha by period,
     S / S′ by video mode; new `UpdateInput::video_mode`).
   - `ColorTables::build(palette)` builds the five ramps.
   - Splash threshold: `k = ⌊target·1000/256⌋` (`splash_threshold`,
     `FloorContext::k`). `last_s` advances whatever `k` is.
   - Pass 9: snow lines come from `SNOW_LINES`. Falling drops use the
     sine-table vector, cut at the ground. A cut with v = 0 is
     `DropDivideByZero`: the original divides by zero.
   - Still open (spec OQ3): the particle move `0x004732C0`
     (`WeatherError::Open`).
4. **Pal column**: `LevelRow::pal` is new, filled from Levels `Pal`
   (+0x02). The 0x15 room change switches `palette_act` to the new level's
   `Pal`, not `Act`. `app::palette` already took any index (n outside
   1…5 → act 1), so only its docs changed. The mask path: the code
   already reads `specs/tools/scenario-masks.tsv`, so nothing changed.

## Tests corrected because the spec now says otherwise (not weakened)

- `missiles::tests::r9::elem_roll_and_len_by_etype`: the return value is
  now 30, not `EType` 0.
- `tests::ext::fire_head_heals_its_owner`: heals by the fire roll
  (100 + 40), not by 1.
- `tests::ext::radament_redemption`: runs on the real unit find instead
  of the removed fake seam.
- `lighting::env_tests::server_setter`: the `EclipsePending` assertion is
  removed. The new test is `server_setter_eclipse_resets_the_period`,
  the spec vector.
- `draws_tests::wall_direction_0_*`: reuse semantics.
- `blend_gdi_tests`: the tie error is now the spec's 45° vectors.
- `weather_tests`:
  - `rain_off_clears_*`: the target is kept and the intensity is 0.
  - `first_rain_update_*`: the shape values are exact.
  - The snow spawn test is replaced by spawn vectors.
  - `floor_context_gates`: the new `k` gate.
  - The falling drop draws now.
  - The lock test no longer sets `snow_lock_hold`.

New tests:
- `unit_find_*`
- `full_record_carries_crit_and_bypass_flags`
- `splash_threshold_vectors`
- `snow_color_*`
- `color_tables_from_the_ramps`
- `falling_drops_and_snow_lines`
- `room_change_reads_levels_pal_not_act`

## Gate (this head)

| Check | Result |
|---|---|
| `cargo test -p d2-sim --lib missiles` | 184 passed, 0 failed |
| `cargo test -p d2-client --lib` | 62 failed before the last test fix: one was this branch's (`floor_context_gates`, fixed and re-run green); the other **61 are red on the base** (S→C dispatch-table `Mismatch(NoHandler {33, 34, 35, 44, 83, 93, 99, 119, 148})`: 58 `bridge::` tests, `ui::tests` ×2, `app::palette::tests::the_presented_palette_follows_the_palette_act`). Owner: impl-client-msgs-3. Not touched. |
| `cargo clippy -p d2-sim -p d2-client --all-targets -D warnings` | clean |
| `cargo fmt --all --check` | clean |
| `coverage.py --check` | 8,409 claims, 0 errors |
| `spec_index.py --check` | clean |

**Not run** (budget cut): the full `CARGO_INCREMENTAL=0 cargo test
--workspace`.

## Left

- Run the full workspace test.
- The unit find's flags 0x200 / 0x800 rely on an empty coordinate list
  and a null extra test, as `0x0056DCC0` builds the record. A caller with
  its own filter (`ai-bodies-3` OQ3) needs a callback parameter.
- The particle move `0x004732C0` is open (draw-order-2 OQ3).
- Feed wiring is unchanged:
  - `Weather` still needs `video_mode` and a palette-built
    `ColorTables` from the feed.
  - `WallPass` has no caller yet: the view does not draw wall blocks lit.

## Follow-up after the merge into `claude/specs-staging-2`

The coordinator removed the `EnvError::EclipsePending` arm in
`bridge/msg/lighting.rs`. After that, two `impl-client-msgs-3` tests
failed because they still asserted the old "eclipse pending" rejection.
Spec `lighting.md` §9.2 r2 now defines the eclipse branch: the period
reset `0x0061BDF0`, then intensity and color. So the tests were wrong, not
the code, and I corrected the tests to the spec (no assertion loosened):

- `bridge::msg::tests_outputs::quest_status_eclipse`: 0x5D quest 10 with
  a client act, and the pending eclipse at the act-2 load. Both now
  assert that nothing is rejected, plus the spec's state: index 5, type 2,
  ticks 240 × 128 = 30,720, eclipse on.
- `bridge::msg::tests_outputs::darkness_needs_the_players_act`: 0x53
  index 5 with eclipse 1 is applied (same state), so 2 rejections
  (bad index, negative ticks) instead of 3.

The Levels `Pal` reading is not involved in either test. The handler code
is unchanged.

Gate: `CARGO_INCREMENTAL=0 cargo test -p d2-client --no-fail-fast`. Lib:
884 passed, 1 failed (`bridge::local_tests::unknown_and_unowned_ids`,
owned by another session). Every other target is green. fmt is clean and
`coverage.py --check` reports 0 errors.
