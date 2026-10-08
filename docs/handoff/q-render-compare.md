# q-render-compare: run a scene compare end to end

Branch `claude/q-render-compare` (2026-10-08). Format and rules:
`specs/tools/facts-render.md`. Code: `crates/d2-client/src/facts/`
(`mod.rs` format, `export.rs` d2rs side, `compare.rs` first difference);
CLI in `crates/d2-client/src/main.rs`.

## State

- Done: the spec; the exporter (`play --dump-draws DIR [--at-tick N]`);
  `facts-compare` (exit 0 match, 1 diverged, 2 partial, 3 error); format
  tests on hand-made TSVs (`cargo nextest run -p d2-client -E
  'test(facts::)'`); `facts/requests.tsv` (empty, header only);
  `pc1-data.md` step 3 points PC 1 at the spec.
- Not done: no 1.14d scene exists in this format yet (PC 1 step 3). No
  compare has run on real facts: everything here is unverified (M02).
  The exporter has not run on the real install in this session (cloud,
  no window); first run is queued below.

## End to end, one scene

1. PC 1 records and converts the scene: `facts/render/scenes/<scene>/`
   with `draws.tsv`, `frame.tsv`, and `facts/render/sprites.tsv` (on
   `claude/local-pc1-facts`). Its `frame.tsv` names `tick`, `level`, the
   character and seed are in the header's `command`.
2. Fetch that branch's `facts/` (`git fetch origin claude/local-pc1-facts`
   then `git checkout origin/claude/local-pc1-facts -- facts`).
3. Run d2rs on the same start (same character class, seed and input;
   the private data repo as `D2_GAME_DIR`):
   `cargo run -p d2-client -- play --new <class> <name> --seed <N>
   --dump-draws game/facts-d2rs/<scene> --at-tick <tick>`.
   The app exits after writing the three files (it needs a window:
   local, or cloud with a virtual display).
4. Compare: `cargo run -p d2-client -- facts-compare
   facts/render/scenes/<scene> game/facts-d2rs/<scene>`.
   - exit 0 MATCH: record the result (scene, both commits) in the
     scene's spec status and HANDOFF.
   - exit 1 DIVERGED: the report names the stage (frame inputs, sprites,
     draws, frame outputs), the row, the column and both full rows. Fix
     the cause (it is the earliest), re-run, repeat.
   - exit 2 PARTIAL: no difference among measured cells; the counts say
     which columns are `?`. Those are the REC items below.
   - exit 3: a file is missing or malformed; the message says which line.
5. While a known gap blocks the first rows (e.g. tick numbering, the
   provisional op names), `--ignore tick,op` compares the rest; say so
   wherever the result is reported.

## Provisional points (REC)

- **REC-295** d2rs wrapper names (`facts-render.md` §5 r3): tile items by
  pass and blend, cel items `CelDraw` / `CelDrawShadow`. Settled by the
  first compared scene with units and UI (the 1.14d `op` column).
- **REC-296** Tile X, Y (§5 r5): the handed position (floors / roofs:
  block origin + 80 − view.left). Settled by the first compared scene
  with floor tiles (row x, y of a `FloorTileDraw`).
- **REC-297** Cel `mode`, `light`, `pal` and tile `light` are `?` on the
  d2rs side (§5 r7): d2rs keeps a shade chain and blend op, not the call's
  arguments. Needed: the world view to carry the original call's draw
  mode, light and palette choice next to each item.
- **REC-298** Tick alignment (OQ 1): bridge `server_ticks` vs 1.14d `f`.
  Settled by the first compared scene (`frame.tsv` `tick`).
- **REC-299** 1.14d cel sizes and offsets (OQ 4): `sprites.tsv` needs the
  decoded cel header; the converter reads the cel file or the recorder
  hooks the decode.

## Local run queue item

- **q-render-compare**: with game files and a window: `cargo run -p
  d2-client -- play --new sorceress Test --seed 1 --dump-draws
  game/facts-d2rs/smoke --at-tick 40`; look for: exit 0 and three files;
  then `cargo run -p d2-client -- facts-compare game/facts-d2rs/smoke
  game/facts-d2rs/smoke` prints PARTIAL (d2rs's own `?` cells), never an
  error.
