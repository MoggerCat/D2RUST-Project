# rc-warriv-draw hand-back (REC-2320..2324 unused)

Base: specs-staging-7 + integ-r19. `suite.py --filter '*arrival*'`.

| check | before | after |
|---|---|---|
| rng / packets-town-arrival-ama | MATCH 33/33, 40/40 | same |
| a1 / a4 / a5-harrogath / a5-town-bar arrival (state) | all frames equal, PARTIAL | same |
| draws-town-arrival-ama | DIVERGED row 172 (66.3%) | DIVERGED row 173 (66.7%) |

Equal ticks 273/274 -> 273/274 (the draws check still has 0/1 equal ticks).

## Cause and fix (row 172)

Not the position track: Warriv's walk track and rest point were right (cell
(4870, 4231), rest 376 short of the centre). At tick 72 the server sends a
code-7 request; 1.14d's monster machine takes the neutral fallback F
(`0x004AE1D0`), whose path stop (`0x00480490` -> `0x00650590`) sets the
path's precise position to the centre of its cell (`(p & 0xFFFF0000) +
0x8000` per axis). d2rs only changed the mode, so Warriv kept the rest
point and drew 1 px left/up. `bridge/modes.rs::neutral_fallback` now does
the path stop (precise only, when a track set one). Spec: `client/model.md`
§19 r4 (F). Test: `the_neutral_fallback_stops_the_path_at_the_cell_centre`.
Row 172 now 304/292, equal to 1.14d.

## Open

- draws row 173: 1.14d has a CelDraw where d2rs has a `unit` row, and the row
  counts differ (1.14d 261, d2rs 267 on this tick). Size M; the unit-pass
  alignment, not the position.
- Neutral fallback path stop is done for server monsters only; set C
  critters (`critter_path.rs`) stop through their own path. Size S.
- Tests: `cargo test -p d2-client --lib bridge::modes` 13/13; full d2-client
  suite and nextest not run in this session (build time).
