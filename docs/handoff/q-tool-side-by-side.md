# Handoff: side-by-side viewer — `claude/q-tool-side-by-side`

Cloud tools session, 2026-10-09, branched from `claude/specs-staging-7`
(`af5b1de`). No REC ids used (block 970–979 free).

## What it is

`tools/sidebyside/build.py --out DIR` (README beside it) renders every
recorded scene on 1.14d (Wine) and d2rs (Xvfb + lavapipe) at the same
server ticks and writes one self-contained page, `DIR/side-by-side.html`:
an index with a pass / differs badge, the scene frame's match % and the
first differing tick per scene; per scene the first frame, the first
differing frame, fixed ticks and the scene frame with a slider, a toggle
and a diff overlay, and `facts-compare`'s first difference.

Scenes: the groups of `tools/cloud-game/scene_defs.py` (a1 idle, a2–a5
towns, the eight walks and runs, the seven panels, the Cold Plains fight)
and the arrival scene of `traces/checks/draws-town-arrival-ama.check`
(group `arrival`). Not included: `npc` (its `goto` steps have no d2rs
form) and the non-draws checks of `traces/checks/` (state / rng / packets:
no frame to show; `tools/scenario-diff/suite.py` reports those).

## Rule 1

The frames are game art. The tool writes only under `--out` and refuses a
folder inside the repository; `record_frames.py --img-dir` puts the 1.14d
PNGs there too. Pages go to the private data repo,
`reports/side-by-side/<date>/` (command and tool version in its
`README.md`).

## Code

- `d2-client play --at-tick N,M,...` dumps several ticks in one run into
  `DIR/tick-<N>`; `--dump-image` adds `frame.png` (the composed index
  frame, palettized). `specs/tools/facts-render.md` §5 r19; tests
  `facts::tests::dump_image_keeps_indices_and_palette`,
  `facts::tests::dump_dirs_per_tick`, `main` `dump_draws_takes_a_dir_and_a_tick`.
- `tools/trace-recorder/record_frames.py --img-dir DIR`.

## Notes

- Input: both sides take the group script in the shared frame-anchored
  form (`frame F; click X Y`, `scenario-diff.md` §2 r4), mark ticks =
  2 + the `waitticks` before them (q-scenes-compare's schedule: walk
  clicks at 22 + 14k, scenes at 32 + 14k). The first run used
  autostart's plain steps: each click waits 0.65 s of wall clock, so under
  Wine with a PNG per frame the marks drifted 1–3 ticks per click (walk
  w_s at 49 instead of 46) and the two sides got different input ticks.
  d2rs gets the steps acting in a frame up to the scene tick T; a paused
  scene (Esc menu) the steps before its mark.
- The page compares the frames of its own recording (framed input), so a
  scene's tick and first difference can differ from `facts/render/`
  (recorded with plain steps).
