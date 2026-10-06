# Handoff: lighting, shading and blend modes in code — `claude/impl-lighting-blend`

Cloud implementation session, 2026-10-06. Base `claude/specs-staging`
(`5844674`). Repo only, synthetic fixtures, no game files. Implements
`specs/render/lighting.md` (+ `env-periods.tsv`, `wall-light-points.tsv`,
new, all of it) and the sections of `specs/render/shading.md` and
`specs/render/blend-modes.md` added since `impl-shading-blend`, and wires
lighting into the world view. Everything is **unverified** against 1.14d
(no capture compared; METHODS M02).

## What changed

| Spec rule | Code | Tests (`// Covers:`) |
|---|---|---|
| lighting §1–§4, §6, §7, §12 r3 | `rules/lighting/{map,records,contribute}.rs`: `LightMap` (48 × 48, origin, clamped `read`, `bytes`, `digest` SHA-256), `fill_ambient` (`AmbientScene`, `NearRoom`), `fill_blocks`, `build` (§2 order); `LightList` / `LightRecord` / `LightKind` (create, set radius / target / color, remove, die, per-frame update `frame`, `room_leaving`), trait `LightWorld` (owner lookup, sub-tile, room, local player, blockers, cell → room); `oct`, `plain`, `shadowed` (64 × 64 B / S planes, ring order), `cached` | `rules/lighting/core_tests.rs` (25): every §6 / §7 synthetic vector |
| lighting §3.1, §5, §8, §9, §10 | `rules/lighting/{quality,environment,overrides,sources}.rs`: `Quality`, `LightOptions`, `DrawRateMeter`; `Environment` (create, `update`, `set_from_server` 0x53, intensity with `PI_F`, `sin`, f32 reload, color), `PeriodTables` (strict TSV), `EclipseTrigger`, `room_ambient`; `Overrides` (Den, 107/108, darkness event, 0x89 dispatcher), `sine_table`, `den_light_points` (`Seed::roll`); `sources::*` → `LightRequest` per source row, `missile_flicker`, `umod3_light` | `rules/lighting/env_tests.rs` (32): every §9.3 / §9.4 / §10 vector |
| lighting §11 | `rules/lighting/draws.rs`: `unit_light`, `light_byte`, `WallPoints` (strict TSV, `wall_points()`), `wall_light_words`, `wall_block_corners`, `wall_block_shades` → `BlockShade`s, `LightGrid`, `floor_light_grid`, `roof_light_grid` | `rules/lighting/draws_tests.rs` (7) |
| lighting §13, §11 r1; blend-modes §3; shading §3, §5, §10 (wiring) | `rules/lighting/view.rs`: `FrameLight` (act `ShadeTables` + frame `LightMap`), `LookFeed` (unit sub-tile, `ComponentLook`), `layer_mode` (COF bytes 3 / 4), `component_ops`, `LitRules` (a `ViewRules` wrapper answering `shade` / `blend`); `world_view::feed`: `ViewFeed::light` (default `None`) → `FeedLight`, `build_frame` builds through `LitRules` when the feed states a light (`ModelFeed` forwards it) | `rules/lighting/view_tests.rs` (3), `world_view::tests::build_frame_lights_units_through_the_feeds_light` |
| shading §4 r4, lighting §11 r2 (per-block tile light) | `rules::BlockShade`, `ViewSource::tile_blocks` (default empty; `OrderedSource`, `ModelFeed` forward), `OriginalView::tile_draws`: one `TileDraw` per block, clipped to the block, gradient moved to the block's screen position; culled blocks draw nothing | `rules::tests::tile_blocks_draw_one_item_per_block` |
| shading §4 floors r4 (iso floor gradient, answered) | `ShadingError::IsometricFloorGradient` removed; `floor_block_chain(tables, light, x, y)` (no `rle` argument, no `Result`): RLE and iso blocks share the 15-row gradient; `GradientKind::RleFloor` doc | `shading_blend_tests` (the iso assertion now expects the gradient: the spec answered OQ1, not a weakening) + the spec's iso vector |
| shading §6 r2, r3, r6, r7 | `rules/shading.rs` (appended): `ShiftInputs`, `shift_trans_lvl`, `shift_unique` (`Seed::init_low` + `step`), `shift_utrans`, `monster_shift_index`; `ShiftMapInputs`, `ShiftMap`, `shift_map` (fatal 0x160 → `ShiftError`); `palshift_map`, `rand_transforms_map`, `blood_map`, `green_blood_switch` (data as caller slices, never embedded) | `rules/shading_shift_tests.rs` (8): both spec vectors |
| blend-modes §5 r3, r4 | `rules/blend.rs`: `unit_shadow_skipped`, `ObjectShadow`, `ShadowMotion`, `composite_shadow_position`, `single_cel_shadow_position`, `unit_shadow_position` (camera `cx_u`, `cy_u`, `shiftX`; perspective → `BlendError::Perspective`) | `rules/blend_gdi_tests.rs` |
| blend-modes §8 r1, r2 (GDI line, rectangle) | `rules/blend.rs`: `gdi_line_pixels`, `gdi_line`, `gdi_mode_value` (`k` table), `gdi_rectangle`, `GdiDraw` → `DrawItem` (image of 1s, chain maps 1 → color; `k` = 1 is `[Z]` + `IndexTableSrcRow(T)` = `T[0][d]`, `k` = 2 is `IndexTable(T)`): no new GPU op | `blend_gdi_tests.rs` (8); harness case `gdi-lines-rects` (CPU = emulate = GPU on llvmpipe) |

Coverage after this branch (`python3 tools/coverage.py`): `blend-modes.md`
20 / 20 rule units, `shading.md` 29 / 32 (unclaimed: §6 text, §6 r5 —
`ui/text.md`'s, §9 — one palette), `lighting.md` 81 / 90 (unclaimed: §1 r3,
§2 r4 — `q` is the caller's input to `build`, §3.1 text, §8 r4
`cursecenter` has code (`cursecenter_light`) but no test claim, §10 text,
§12 r1, r2, r4, r5 — capture statements). `--check`: 0 errors.

## Not wired (open)

1. **No feed states a light yet.** `ViewFeed::light` defaults to `None`;
   the client model holds no light records, room ambients, collision, act
   environment or per-unit look inputs (fade, ghostly, hover, items,
   remaps). Whoever owns the client model (`impl-client-staging`) builds a
   `FrameLight` per drawn frame (`LightMap::build` with `LightList` fed by
   `sources::*` from the unit mirror, `Environment::update` per client
   update, `Overrides`) and implements `LookFeed`. Likewise
   `ViewSource::tile_blocks` / `ViewFeed::tile_art` need the DT1 block grid,
   wall direction and fade state (`draws::wall_block_shades` is the helper
   for walls; floors need the block's (gx, gy), not in `BlockRect`).
2. Unit shadows (`blend-modes.md` §5) have position and pixel rules but no
   draw-order item yet (`draw-order.md` open question 3 still refuses
   them in `draw_order::source::resolve`).
3. Pending, spec silent (each returns an explicit error, no guess):
   - lighting §9.2 r2: the eclipse branch of the 0x53 setter calls
     `0x0061BDF0`, undescribed → `EnvError::EclipsePending`.
   - lighting §11 r2 / edge 7: wall direction 0 (uninitialized stack) →
     `DrawLightError::WallDirection0`; directions > 9 and block columns
     needing point 6 → errors.
   - blend-modes §8 r1: a line with |Δx| = |Δy| > 0 — which axis is
     major is unstated and changes pixels → `BlendError::LineMajorAxisTie`
     (45° weather lines and Arcane stars cannot draw yet). §8 r2: x1 < x0
     after the clamp → `RectangleColumnsReversed`.
   - lighting §6.4 room leave with an owner without a room →
     `LightError::OwnerWithoutRoom`.
4. Readings chosen where the spec is loose (listed for the spec session):
   - §8 r1: the literal step rule often stops short of (x1, y1) ((0,0)→(5,2)
     ends at (5,1)); implemented literally.
   - §8 r2: "empty" (x0 = x1) is tested before the fatal y1 < y0.
   - blend §5 r4: objects (type 2) take r3 by type dispatch, so r4's object
     Draw / BlocksLight branch is unreachable that way; is `0x004DB180` a
     type ≥ 3 test?
   - lighting §7.4 r1: the cached grid's ray centres use the grid's own
     sub-tiles (owner-centred); only the owner = light-cell case is tested.
   - lighting §9.2 r2: the setter's intensity step uses the player room's
     act and level; §9.2 r3 pending flag `[0x007A060E]` is left set after
     the act load; §9.3 r4 `ticks / speed` as a double division; §10 r2
     "resets flag and counter" = 0 and −1; §8 monster level-8 radius 3 is
     applied before "none when 0"; §11 r3 / r4 floor and roof reads use the
     clamping read; §3 r1 with no local player still needs an origin.
   - shading §6: `RandTransforms.dat` read as 30 headerless 256-byte maps,
     `GreenBlood.dat` as its first 256 bytes; palshift offsets are from the
     class base; `0x004791B0` (OQ7) and `0x00410A80` (lighting §10 r4 id 13)
     are caller inputs; missile `LocalBlood` is a bool.
5. §7.1 r6 note: a white light on black gives R, G, B = 254, not 255
   (`T[i] = 65536 / i` truncates); the code follows the rule.

## Gate

- `cargo test -p d2-client --lib`: 493 passed, **47 failed, all one cause
  on the base, not this branch**: `Table(Mismatch([NoHandler { id: 122 },
  NoHandler { id: 129 }]))` — the S→C dispatch table (`bridge-dispatch.tsv`
  from the merged `client/msg-*` specs) owns ids 122 and 129 with no handler
  (bridge / `impl-client-staging`'s). The same cause fails the integration
  targets `app_frame_loop` (5), `e2e_*` (12), `mutants_client` (2),
  `prop_bridge` (4), `prop_client_bridge` (1). Every other target passes.
  This branch changes no file under `specs/`, `bridge/` or `ui/`.
- `rules::`, `scene::`, `gpu_compositor::`, `world_view::` lib tests: all
  pass (232 in the filtered run), GPU harness (`--include-ignored`) on
  llvmpipe incl. the new `gdi-lines-rects` case: 0 differing bytes.
- `cargo clippy -p d2-client --all-targets -- -D warnings`: clean.
  `cargo fmt --check`: clean. `python3 tools/coverage.py --check`: 0
  errors. `python3 tools/spec_index.py --check`: clean.
- Cloud setup: Bevy needs `apt-get install pkg-config libasound2-dev
  libudev-dev libwayland-dev libxkbcommon-dev` (as CI), plus
  `mesa-vulkan-drivers` for the ignored GPU tests.

## Local checks (queued in `docs/HANDOFF.md` §5 C65–C68)

- `env-periods.tsv` = `Game.exe` `0x007443F0`, `0x00744438`, `0x00744480`
  (18 × 12 bytes: start, type, color `0x00BBGGRR`); `wall-light-points.tsv`
  = `0x0072A9E8` / `0x0072ABC8` (9 directions × 6 points × 2).
- `mpq-tool extract` sizes: `Data\Global\Monsters\RandTransforms.dat` 7,680
  bytes, `GreenBlood.dat` 256, a class's `palshift.dat` 2,048.
- Environment intensity per tick over one day vs `Environment::update`
  (lighting OQ7); a light-map digest in the capture key (§12 r3, OQ9).
- Captures: a weather line / Arcane star vs `gdi_line_pixels` (settles the
  tie and end-pixel questions); the player's shadow on a flat floor vs
  `unit_shadow_position` (blend-modes OQ1); the earlier `impl-shading-blend`
  captures (orientation, wall fade, lit floor at night) once a feed states
  a light.
