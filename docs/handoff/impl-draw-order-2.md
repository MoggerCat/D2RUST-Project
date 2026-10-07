# impl-draw-order-2 — `render/draw-order-2.md` and the draw-order answers in d2-client

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Scope: branch `claude/impl-draw-order-2`, from `claude/specs-staging` @
5844674. Cloud, repo only (no `game/`, `re/`, `../refs/`). Parallel
sessions own lighting / blend and bridge / model code; this one kept to
`rules::draw_order`, the world view feed hook, and (by task) the answered
unit-composite and ui/text items.

`specs/render/map-preview.md` already had code (`d2-client::map`, status
implemented, GPU = CPU verified); nothing to do there.

## What changed

All under `crates/d2-client/src/rules/draw_order/` unless noted. Every
module names its spec; every test carries a `// Covers:` claim.

- **`mod.rs` — `draw-order.md` answers.**
  - A7 / §8 group mode: `Logical` (record +0x10, coordinate record x0, y0,
    index), `TileRecord::logical`, `NearRooms::player_logical`,
    `fade_near_group`. Group mode is the default (1.14d `[0x0072A968]` =
    1): only walls of lvlprest `Logicals` presets have a coordinate record,
    so only they fade. The geometric branch is kept behind
    `LevelFacts::fade_geometric` (replaces `fade_group_mode`, which used to
    fail the frame).
  - DO3 / §2 pool overflow: grid flags set only when the entry is filed;
    a dropped wall gets no fade target; cell flag 4 still set before the
    pool test.
  - DO5 / §8 clock arithmetic: unsigned `end ≤ now`, wrapping 32-bit
    signed ramp product, C division, low byte added mod 256 (wrap vector
    → 0x3B).
  - DO4 / §6 r6: flag 0x20000 is no longer set by the order;
    `sets_drawn_flag` (walls / lower walls: a block passes the wall block
    test; floors / roofs: whole-tile test; shadow tiles never) and
    `mark_drawn`, applied by `source::ordered_source` after the draws.
  - §6 r7: `AutomapReveal` (countdown, `d = (2·max + min) / 2 ≥ 0x50`,
    floors then walls of the player level's rooms) and `reveal_room`
    (the `AutoMap` preset path); `Room::level` added. The automap itself
    (`0x00457CF0`, room objects) belongs to the automap spec.
  - DO1: `tile_of(−160, 0) = (−2, 1)` asserted whole.
  - Errors for pass 1 (levels 74 / 120) and edge floors now name
    `draw-order-2.md` §12 / §14 and what is missing (see Gaps).
- **`sight.rs`** (§15, §16): `line_test` (0x0064E260, error term from 0,
  room switching through `d2_sim::path::collision::find_room`),
  `sight_line`, `sight_hidden`. Reuses d2-sim's `CollisionRooms`,
  `CollisionGrid` and `bits::VISIBLE`; d2-sim's own `line_clear` /
  `line_blocked` seams are still unimplemented and could take this code.
  `UnitFacts::sight_hidden` is filled by the feed from it (`None` still
  fails a sight-tested unit: no collision rooms are lent yet).
- **`weather.rs`** (§11): pools, update, rain cycle, top-up / wind /
  lightning timer, water floors (`FloorContext`, `water_floor`, spawns),
  `pass4` / `pass9` as data, act load, level presets, thunder as an event.
- **`background.rs`** (§12, §13): `Stars`, `Summit`, `Backgrounds::pass1`
  (pass 1 items in build order), `pass8_items` (empty; `PASS8_RUNS =
  false`).
- **`edges.rs`** (§14): `edges_active`, `DrawnExtents`, strips,
  `edge_floors` (sets 0x20000, widens extents), `edge_key`.
- **`source.rs`**: `resolve_drawn` → `DrawEffects { drawn, water }`;
  `WeatherFrame` (weather, floor context, player seed, update count,
  `Mud`) and `floor_pass`: one `roll_range(0, 1000)` per drawn water
  floor, in draw order, at its handed (X, Y). `unwired_passes` fails a
  frame with live pools or lightning (passes 4 / 9 have no art path yet).
- **`world_view/feed.rs`**: `ViewFeed::weather_frame` hook (default
  `None`; a drawn water floor without it fails the frame).
- **`rules/unit_composite.rs`** (UC2 error `ArmTypeIndex`; UC3 asserted;
  UC4 case-insensitive `OYTRlitTNhth`; UC5: mode override tables,
  Barbarian dual-wield class, linked-unit inventory, act II skeleton
  tables, follow branch) and **`ui/text.rs`** (UT1 no trailing empty
  line from the NUL; UT2–UT4 already right, now tested; UT5 no-color
  variant removed as dead code). Handoff docs `impl-unit-composite.md`
  §4 and `impl-ui-text.md` record the answers.

Tests changed because the spec answered against them (not weakened):
`roofs_draw_by_layer_mask` (0x20000 comes after the draw),
`wall_fade_runs_through_the_frame` (now sets the geometric branch),
`unresolved_items_fail_the_frame` (water floors go to the weather state),
`player_armor_classes` (UC2 error), two wrap vectors (UT1), the no-color
assertion (UT5).

## Gate

- `cargo test -p d2-client`: lib 471 passed, 47 failed, 7 ignored. The
  failing set is **identical** to the base 5844674 (checked by running
  both trees, failing lists diffed): 45 `bridge::` tests (protocol
  version / `Table(Mismatch(NoHandler …))`), 2 `ui::tests` intent tests,
  and the integration binaries that drive the bridge (`app_frame_loop`,
  `single_player_end_to_end`, `vendor_end_to_end`, determinism runs …).
  Owner: the bridge / model session; not touched here.
- `cargo clippy -p d2-client --all-targets -- -D warnings`: clean.
- `cargo fmt --all --check`, `py tools/coverage.py --check` (5,326
  claims, 0 errors), `py tools/spec_index.py --check`: pass.
- Coverage (unit tier): `draw-order-2.md` 43 / 44, `draw-order.md` 19 /
  25, `unit-composite.md` 33 / 45, `ui/text.md` 36 / 45.
- Build note: the client needs `pkg-config libasound2-dev libudev-dev
  libwayland-dev libxkbcommon-dev` (as CI installs). Worktrees sharing one
  `CARGO_TARGET_DIR` overwrite each other's `d2_client` test binary:
  `touch crates/*/src/lib.rs` before trusting a run.

## Gaps (strict errors, never skipped)

- Pass 1: `background` exists; the order still fails levels 74 / 120 —
  the feed must lend the recorded time seeds, the star tick and the
  palette, and the view needs a line / cel art path.
- Passes 4 / 9: data exists; the view has no overlay-cel / line /
  rectangle path, so live pools or lightning fail the frame.
- Edge floors: the order fails `DrawEdges` at open mode 0 — no resolution
  mode in the client yet, and the act edge record is spec OQ2.
- Sight: no collision rooms are lent to the feed yet (`sight_hidden` stays
  `None` → error for sight-tested units).
- Unit shadows: `blend-modes.md` §5 answers them; wiring belongs to the
  blend session (`resolve` still fails a `UnitShadow`).
- UC1 (`composite::check` rejects duplicate layer records; §5.1 r5 says
  first match) — not in this task's list, left for a follow-up.
- `ui/text.rs` vertical window still `Unspecified` though §9 now details
  it.

## Questions for the spec owner

`draw-order-2.md`:
- W1 `[0x007A8A20]` (read by §11.3 with the snow lock): no writer in the
  spec.
- W2 snow spawn draw list ("one more step" than what? does `0x00472FB0`
  draw?). Snow spawns return `Unspecified`.
- W3 values of the snow line table `0x006D6E78`.
- W4 the grey / tinted ramps of the colour tables (taken as an input);
  alpha 0x7F only for day period 0?; landed drop colour = +0x24?
- W5 intensity = target / 256 makes int(intensity) 0 for every rain peak
  (32–255), so splashes could never spawn — but run 2 saw splash ripples
  in the Rogue Encampment. Re-check the scale or the `r < int` test.
- W6 §11.7 r3: with lightning on but ≤ 9 fps (no flash), do particles
  draw? Implemented: yes.
- W7 initial phase / countdown / length and `last_s` / `last_b` (taken as
  0, `.bss`).
- §12: the two seed globals are 4 bytes apart — `init_low(time_value)`
  (hi 666)?; initial star tick `last`; which palette `nearest` searches;
  build order of the extra summit frame (x0 − 256) and mountains vs
  clouds.
- §14: extents before any floor is drawn and when they reset; does a
  drawn edge floor widen by its strip sub-tile?; `draw-order.md` §10 has
  no row for edge-floor keys (`edge_key`: pass 3, major 2 × rooms).
- §16: a room whose rectangle holds the cell but whose grid has no cell
  there (returns `NoGridCell`); the stop cell written back when r1 blocks
  (room null / start in no room; `Blocked { at: None }`); unit size
  signedness in `0x00622AA0`.

`unit-composite.md`: does body armour / the "no inventory" check read the
linked unit's inventory (Decoy)?; §8 r4 "nothing happens while K is in
DT/DD" — following missiles only?

## Local checks queued (HANDOFF §5 A "Draw order 2", S9-A1 (1)–(3))

- `weather-0001` (spec Test vectors): Rogue Encampment in rain, 2 frames
  with the recorded player seed, pass 4 / 9 pixels; also count the
  player-seed draws per frame against the drawn water floors.
- A `Logicals` preset walk (`draw-order.md` OQ6: Crypt / Mausoleum,
  levels 18, 19): fade on pixels in group mode.
- Arcane Sanctuary / Arreat Summit captures with the background seeds
  recorded (`[0x00712C4C]`, `[0x00712C50]`).
