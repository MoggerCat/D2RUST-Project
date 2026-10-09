# Spec: Tools — frame time and memory

- **Status:** implemented; d2rs-own tool (no 1.14d behaviour). Evidence:
  `docs/handoff/perf.md`.
- **Target version:** none (d2rs performance)
- **Crate/module:** `tools/perf/perf.py`, `d2-client::app::perf`,
  `d2-server::perf`
- **Related specs:** `sim/tick.md` (25 ticks/s), `client/bridge.md`

## Summary

Measures, per scene, the server tick time, the client frame split
(pump / update / post) and the render-world frame time, plus the peak RSS,
and checks them against the budget: 25 server ticks per second and no
frame over its budget.

## Inputs

| Name | Type | Source |
|---|---|---|
| `D2_PERF_OUT` | file path | environment of `d2-client` |
| scenes | `--poke` warps | `tools/perf/perf.py` |

## Outputs / state changes

A `perf 1` JSON per run (`peak_rss_kib`, `ticks` as [frame, drain µs,
tick µs, flush µs], `frames` as [interval, pre, update, post] µs,
`render` µs, `bridge` µs: headless `state-dump` bridge frames that ran a
tick), and `docs/handoff/perf.md`.

## Rules

1. Timing is on only when `D2_PERF_OUT` is set; the wall clock is read in
   `d2-server::host::Host::frame` and in the Bevy app, never in d2-sim.
2. Budgets: a server tick (tick + flush) ≤ 40 ms (1000 / 25); a client
   frame interval ≤ 40 ms (the game's 25 frames per second). A scene
   misses the budget when its max exceeds it; p95 over budget is a
   sustained miss.
3. Scenes: each act's town and one busy outdoor level, reached by the
   `warp` poke at frame 4 from a new character; headless (`state-dump`,
   server only) and windowed (`play` under Xvfb, lavapipe) where possible.

## Constants & data dependencies

Level ids from Levels.txt (towns 1, 40, 75, 103, 109).

## Randomness

None added; the scenes use fixed seeds.

## Edge cases & original bugs

Under Xvfb the GPU is Mesa lavapipe (CPU): render times are an upper
bound, not a GPU measurement.

## Test vectors

| Input / seed | Expected output | Source (trace id) |
|---|---|---|
| `perf.py --selftest` | percentiles of a known list | selftest |

## Provenance

d2rs-own design (2026-10-09, `q-tool-coverage-perf`).

## Open questions

None.
