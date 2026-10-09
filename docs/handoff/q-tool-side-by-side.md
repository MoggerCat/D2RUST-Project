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
- Coordinator items 16 (audio) and 17 (input feel): `d2-client play
  --sound-log FILE` (every `SoundSystem::request` call, `facts-render.md`
  §5 r20, test `sound_table::tests::request_log_keeps_every_call`) and
  `record_frames.py --sounds` (1.14d `0x004B9A00` entry, bytes
  `55 8B EC 83 EC 18`, `audio/triggers.md` Checks "request log"); the page
  shows both sequences around the first difference and the input-to-mode
  latency per step.

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

## Runs

| Date | Page (private repo) | Public commit | Result |
|---|---|---|---|
| 2026-10-09 | `reports/side-by-side/2026-10-09/side-by-side.html` (27 MB, 32 scenes) | `8dd2c592` (merged with `claude/specs-staging-7`) | 0 pass. Scene-frame match: panels 93.7–99.9 %, towns a1–a5 94.9–99.1 %, walk-n 98.9 %, arrival 98.6 %; walk-s…nw 20.9–26.9 %, runs 17.7–18.7 %, Cold Plains 13.6–14.5 % (camera off: the walk / run drift and the waypoint walk of q-scenes-compare) |

Findings of the first run (all on framed input, d2rs at `8dd2c592`):
- 1.14d's first captured frame (tick 3, seq 1) is one flat colour (the
  capture runs before the first world draw); d2rs's tick 3 already shows
  the town, so every scene's first differing tick is 3. From tick 4 the
  idle town is ~99.1 % equal per frame.
- a1-walk-s: both sides walk north identically through tick 35 (tile
  origin equal every drawn tick); after the click at 36 (frame 37) 1.14d
  turns back south-west (x 9930 → 9926 by tick 47) while d2rs keeps
  drifting east (9931 → 9933): the re-targeted walk's direction, not a
  render difference. Every later walk / run scene inherits the offset.
| 2026-10-09 (2) | `reports/side-by-side/2026-10-09-2/side-by-side.html` (27 MB, 32 scenes, sounds + input feel) | `d2146986` | 0 pass. Pixels: walks s–e now 98.7–99.4 % (staging's walk fixes); walk-w 32 %, se / nw 23 %, runs 18–19 %, Cold Plains 14 %. Sounds: every scene differs at call 0 (order): 1.14d makes its id-0 unit calls (return `0x4d9bcc`) before the object calls of tick 2, d2rs after them; 1.14d requests id 6 (no unit) at tick 2 where d2rs requests 70 and 4673. Cold Plains: 1.14d 221–614 calls vs d2rs 108–182. Input feel: the first town click (tick 22) shows d2rs's player in WL at tick 22 and TW from 24; 1.14d stays TN until TW at 24 |
