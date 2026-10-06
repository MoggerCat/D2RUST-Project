# spec-unit-numbering (cloud docs session)

Spec markup and `// Covers:` lines only; no behaviour text or code changed.

## What was done

- `specs/sim/rng.md` §5.2–§7: **not numbered.** These sections are tables
  and prose paragraphs, which the COVERAGE.md grammar cannot address
  (only list items at column 0 are `rN`). Numbering them needs either a
  list rewrite of the tables or a grammar extension for table rows
  (COVERAGE.md §1 "Not counted yet"). Left for a decision.
- Where `py tools/coverage.py` showed one huge unit, the `Edge cases &
  original bugs` bullet lists were converted to numbered lists (`- ` →
  `N. `, text unchanged): fixups, cube, txt-format, runtime-maps,
  callbacks, preset, animdata, sprite-placement, camera, composition,
  map-preview, mpq, bridge, loading, patch-layers.
- Claims narrowed after reading each test:
  - cube `edge_cases`: r1–r7 (the test stops at 7).
  - txt-format `txt_edge_case_data` (game): r1, r3, r5, r8.
  - preset `size_mismatch_fatal_only_at_build`: edge r1;
    `navi_unit`: edge r9.
  - animdata `animdata_edge_cases` (game): r1–r4 (r5, negative counts, is
    not checked).
  - camera: `units_sit_12_rows…` r1; `shake_envelope_and_offsets` r2
    (r3 wall-clock and r4 panel culling not checked).
  - sprite-placement `top_down_cels_clip_as_bottom_up`: r1.
  - loading `loading_edge_cases` (game): r1, r2, r4 (r3 is a pointer).
- Claims left whole because the test checks every item: fixups (10),
  runtime-maps (9), callbacks (11), mpq (4), bridge (3), txt-format unit
  test (8).

## `--summary`, before → after

| | rules | any | verified |
|---|---|---|---|
| before | 3106 | 2839 (91.4%) | 219 (7.1%) |
| after | 3190 | 2896 (90.8%) | 227 (7.1%) |

Dips are real: sections previously claimed whole now show their unchecked
items (camera 100→85.7%, sprite-placement 77.8→58.3%, cube 93.9→82.5%).

## Next

More sections remain one large unit (`py tools/coverage.py --rules`):
`data/field-types.md §6`, `drlg/rooms.md §9.9`, `missiles.md §r11…`,
`drlg/maze.md §3 text` (tables/prose; same grammar issue as rng.md).
The remaining `§edge-cases-original-bugs` claims in `crates/` on other
specs (palette, cof, dcc, ds1, dt1, tbl, font-tbl, calc, audio,
render-pipeline, capture, schema, assets) were not touched.

## Round 2: table-row units (coordinator decision)

- Grammar extension in `tools/coverage.py` + `docs/COVERAGE.md` §1:
  a table directly under a `<!-- rows -->` line gives units
  `§<s> row<N>` (`§<s> t<K> row<N>` for the K-th marked table, K ≥ 2).
  Opt-in per table, so no existing unit or claim id changes; the marker
  line is not rule text; a stray marker is an error. Selftest extended
  (units, covering by the section id, dangling/malformed row claims,
  stray marker).
- `specs/sim/rng.md` §5.2–§7 tables marked: §5.2 row1–2, §5.3 row1–7,
  §5.4 row1–5, §5.5 row1–2, §6 row1–8, §7 row1–13 (39 units).
- Claims: no test claimed anything in §5.2–§7 before. One narrowing/add:
  `setters_and_derive` (sim-0003 #1: `init_low(map seed)` then first step
  = dwStartSeed 4014346869) now also claims `§5.4 row1, §5.4 row2`.
  The rest of §5.2–§7 stays uncovered (it is trace/RE material: needs
  conformance checks, queue in HANDOFF §5).
- Gate: d2-client steps fail here (no wayland system libs for
  `wayland-sys`); all other steps run.
