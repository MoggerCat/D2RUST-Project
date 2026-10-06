# Trace format

- **Format version:** 1
- **Reader:** `crates/conformance` (`TRACE_FORMAT_VERSION`)
- **Writer:** `tools/trace-recorder` (Phase 4)

A trace is one recorded observation of the original game (1.14d): the
starting conditions, the inputs, and what the game produced. Conformance
tests replay the same starting conditions and inputs through `d2-sim` and
compare the results.

Traces hold only our own observations: seeds, numbers, IDs and row
references. They never contain Blizzard assets, strings or table copies.
Raw debugger dumps go in `traces/raw/` (gitignored). Only normalized traces
in this format are committed.

## Files and layout

```
traces/
  FORMAT.md
  <area>/<behavior>/<trace-id>.json     e.g. traces/rng/sequence/rng-0001.json
  raw/                                  gitignored
```

- One trace per file, UTF-8 JSON, LF line endings.
- `trace-id` is `<area>-<4+ digit number>`, unique within the repository.
  Specs cite traces by this ID in their test vectors.
- Pretty-print with sorted keys so diffs stay readable.

## Top-level object

| Field | Type | Required | Meaning |
|---|---|---|---|
| `format_version` | integer | yes | Always `1` for this version of the document. |
| `id` | string | yes | Trace ID, matches the filename. |
| `game_version` | string | yes | `"1.14d"`. Traces from other versions are rejected. |
| `area` | string | yes | Area, e.g. `"rng"`, `"items"`, `"drlg"`, `"combat"`. |
| `behavior` | string | yes | Behavior within the area, matching the spec's file name. |
| `spec` | string | yes | Path of the spec this trace tests, e.g. `"specs/rng/sequence.md"`. |
| `recorded` | object | yes | Provenance, see below. |
| `setup` | object | yes | Starting conditions. Schema depends on `behavior`. |
| `inputs` | array | yes | Ordered inputs (may be empty), see below. |
| `expected` | array | yes | Ordered observed outputs, see below. |
| `compare` | object | no | Comparison options, see below. |
| `notes` | string | no | Free text. |

### `recorded`

| Field | Type | Meaning |
|---|---|---|
| `date` | string | ISO 8601 date, e.g. `"2026-10-05"`. |
| `method` | string | `"debugger"`, `"memory-read"`, `"in-game-observation"` or `"save-diff"`. |
| `tool` | string | Recorder name and version, e.g. `"trace-recorder 0.1.0"`. |
| `by` | string | Who recorded it. |

### `setup`

Behavior-specific starting state. Common fields:

| Field | Type | Meaning |
|---|---|---|
| `seed` | object | RNG seed as `{ "lo": u32, "hi": u32 }`, the two 32-bit halves of the D2 seed. |
| `difficulty` | string | `"normal"`, `"nightmare"` or `"hell"`. |
| `expansion` | bool | LoD game (`true`) or classic. |
| `players` | integer | Player count setting, if it matters. |

Anything else needed to reproduce the start (character level, monster
class ID, item level, treasure class name, area ID) goes here with D2MOO /
community field names.

### `inputs[]` and `expected[]`

Both are arrays of event objects, ordered by time:

| Field | Type | Meaning |
|---|---|---|
| `tick` | integer | Game tick (25 per second) relative to the start of the trace. Use `0` for untimed one-shot behaviors. |
| `kind` | string | Event kind, e.g. `"rng_draw"`, `"item_dropped"`, `"damage"`, `"intent"`. |
| `data` | object | Kind-specific payload. |

Integers that can exceed 2^53 (64-bit seeds, raw flags) are written as
strings with a `0x` hex prefix so JSON readers don't lose precision.

### `compare`

| Field | Type | Default | Meaning |
|---|---|---|---|
| `mode` | string | `"exact"` | `"exact"`: `expected` must match in order. `"set"`: same events, any order within one tick. |
| `ignore` | array of strings | `[]` | Paths within `data` to skip (e.g. a server-assigned unit ID). |

`"exact"` is the default and the goal. Anything looser needs a reason in
`notes`.

## Render captures

Frames captured from 1.14d (`specs/render/capture.md`) are traces with
`area` `"render"`, `behavior` the capture case's spec name (e.g.
`"camera"`), `recorded.method` `"debugger"`. The images show Blizzard art
and are never committed: they stay in `game/captures/` and the trace holds
their hashes and the state they were drawn from.

- Files: `traces/render/<behavior>/render-NNNN.json`.
- `setup` (required fields):

| Field | Type | Meaning |
|---|---|---|
| `capture_format` | integer | Version of this section's payload; `2` (`1`: traces taken from `frames-raw-1`, state without cursor, level, seeds, light, weather, draws). A reader rejects an unknown value. |
| `case` | string | Capture case id, e.g. `"stability-0001"` (`capture.md` §8). |
| `video_type` | integer | `1` (GDI); other values are rejected. |
| `size` | `[w, h]` | `[800, 600]`. |
| `raw` | string | Name of the raw file the frames were taken from (`frames-raw-2` for `capture_format` 2). |
| `images` | string | Directory under `game/captures/` holding the PNGs (local only). |

- `expected[]`: one event per kept frame, `kind` `"frame"`, `tick` = the
  capture's server tick relative to the first kept frame, `data`:

| Field | Type | Meaning |
|---|---|---|
| `seq` | integer | The recorder's capture sequence number (`capture.md` §4; the draw counter is not unique). |
| `draw` | integer | In-game draw counter `[0x7A0494]`. |
| `index_sha256` | string | SHA-256 of the W × H index bytes, row-major, top row first. |
| `palette_sha256` | string | SHA-256 of the 768 palette bytes R, G, B. |
| `image` | string | PNG file name in `setup.images` (8-bit palettized, indices unchanged). |
| `state` | object | The recorded state of `capture.md` §3 (`player`, `tile_origin`, `unit_origin`, `view_rect`, `open_mode`, `shift_x`, `shake`, `clear_counter`; from `capture_format` 2 also `client_update`, `level`, `cursor`, `seed_start`, `seed_end`, `light`, `weather`). |
| `draws` | array | Optional: the frame's draw log of `capture.md` §3.5, in call order. |

- `compare.mode` is `"exact"`: the CPU reference's index frame and palette
  must hash to the recorded values (`capture.md` §6).

## Example

```json
{
  "area": "rng",
  "behavior": "sequence",
  "expected": [
    { "data": { "max": 100, "value": 0 }, "kind": "rng_draw", "tick": 0 }
  ],
  "format_version": 1,
  "game_version": "1.14d",
  "id": "rng-0001",
  "inputs": [],
  "recorded": {
    "by": "example",
    "date": "2026-10-05",
    "method": "debugger",
    "tool": "trace-recorder 0.1.0"
  },
  "setup": { "seed": { "hi": 666, "lo": 1 } },
  "spec": "specs/rng/sequence.md"
}
```

(Values are placeholders and do not show real game output.)

## Versioning

- Any change that would make an old trace read wrong bumps
  `format_version`. Add a migration in `crates/conformance`, and describe
  the change in the changelog below.
- Adding a new optional field or a new `kind` doesn't need a bump.
- The harness rejects traces with a `format_version` it doesn't know.

## Changelog

| Version | Date | Change |
|---|---|---|
| 1 | 2026-10-05 | Initial format. |
| 1 | 2026-10-06 | Added §Render captures (new area and kind, no bump; its payload carries `capture_format` 1). |
| 1 | 2026-10-06 | §Render captures payload `capture_format` 2 (raw `frames-raw-2`, `record_frames.py` 0.2.0): `seq`, cursor, level, seeds, light, weather and the optional draw log; images named by `seq`. Readers keep accepting 1. |
