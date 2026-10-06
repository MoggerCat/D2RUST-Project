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

## Scenario raw recordings (`scenario-raw-0`, provisional)

Written by `tools/trace-recorder/run_scenario.py` to
`traces/raw/<time>-scenario.jsonl` (gitignored). Provisional: replaced by
the scenario format of `specs/tools/scenario.md` when that lands. Format
name `scenario-raw-0`, `format_version` 0; any change in the meaning of a
record bumps it.

JSON lines, UTF-8, LF. First line `{"type":"header",...}`: `format`,
`format_version`, `tool`, `date`, `game_exe_sha256`, `scenario` (repo
path), `scenario_sha256`, `scenario_version`, `seed`, `init_seed`,
`save`, `class`, `difficulty`, `ticks`, `streams`, `steps`, `args`. Last line
`{"type":"footer",...}`: `records`, `records_sha256` (SHA-256 of the
record lines in order), `counts`, `notes`, `ticks`, `injected`.

Records (keys sorted; each has `seq`; no wall-clock time, no thread ids,
so two runs of one scenario compare byte for byte):

| `type` | Stream | Fields |
|---|---|---|
| `tick` (and `tick_end` in `packets`) | always | server tick of the recorded game; `frame` = game +0xA8 after the tick's increment; scenario ticks are these frame numbers |
| `seed_override` | always | `which` (`time` at 0x52C2BB, `init` at 0x52C2E3), `old`, `new` |
| `inject` | always | `tick`, `c2s` (hex), `result` (send return, 1 = queued), `via` |
| `units` | `units` | `tick`, `units`: server units sorted by (`t`, `id`): `id`, `t` type, `cl` class, `m` mode, `x`, `y` (sub-tile), `life`, `mana` (raw full-array values, 1/256 points; null without a stat list) |
| `c2s`, `c2s_sys`, `dispatch`, `result`, `s2c`, `net`, `client_send`, `client_out`, `drain`, `flush`, `tick_end` | `packets` | as `packets-raw-1` (`record_packets.py`) |
| `draw`, `seed_set` | `rng` | as `rng-raw-1` (`record_rng.py`) |

`units` at `tick` N is the state after tick N completed.

## Scenario traces

A scenario trace is what one runner (the original 1.14d or d2rs)
produced for one scenario script (`specs/tools/scenario.md`). Unlike
the traces above it is a JSON-lines file, written by both sides and
never committed: `traces/raw/<name>.<side>.trace.jsonl`.

- **Format:** `scenario-trace`, **version** 1 (header fields `format`,
  `version`). A reader rejects another format or version.
- **Reader / comparator:** `conformance::scenario::{trace, compare}`;
  **writers:** `tools/scenario-run` (d2rs), `run_scenario.py` (original,
  `docs/handoff/scenario-harness.md`).
- One JSON object per line, LF, keys sorted, no spaces (the output of
  `serde_json::to_string` on sorted maps; Python `json.dumps(o,
  sort_keys=True, separators=(",", ":"))`). Byte strings are lower-case
  hex without separators. Integers are JSON numbers (all fit in 2^53).
- Line 1 is the header; then the records in tick order; the last line
  is the `end` record. Within a tick, records keep the order of the
  table below (`c2s`, `spawn`, `s2c`, `rng`, `draw`, `unit`, `stats`); within a
  kind, the order the rules give.

Header (`k` = `"header"`):

| Field | Type | Meaning |
|---|---|---|
| `format`, `version` | string, int | `"scenario-trace"`, `1` |
| `game_version` | string | `"1.14d"` |
| `side` | string | `"original"` or `"d2rs"` |
| `tool` | string | writer name and version, e.g. `"scenario-run 0.1.0"` |
| `data` | string | what the game ran on: `"1.14d"` (original), `"live"` or `"synthetic"` (d2rs) |
| `scenario` | string | the script's `name` |
| `scenario_sha256` | string | SHA-256 (hex) of the script's canonical text (`scenario.md` §2 rule 6) |
| `seed`, `init`, `end` | int | the script's `seed`, `init` and `end` |
| `streams` | array of strings | streams this trace holds, sorted: `c2s` always; `s2c`, `rng`, `rng-draws`, `units`, `stats`, `frames` as produced |
| `gaps` | array of strings | what this runner could not apply or record (`scenario.md` §4); empty when none |

Records (`t` = tick, `scenario.md` §4 rule 2):

| `k` | Fields | Written |
|---|---|---|
| `c2s` | `t`, `i` (step index in the tick, from 0), `b` (bytes) **or** `unresolved` (the reference text) | every step (§4 rule 4) |
| `spawn` | `t`, `i`, and one of `guid` (the unit the first call returned), `failed` (`true`: none) or `unresolved` | every spawn step (§3.1 rule 3); part of the `c2s` stream |
| `s2c` | `t`, `c` (client id), `b` | every message queued for the client (§4 rule 5) |
| `rng` | `t`, `before`, `after`: `[lo, hi]` of the game seed | every tick when `rng` is recorded (§4 rule 6) |
| `draw` | `t`, `n` (draw index in the tick), `before`, `after`, `site` (caller: a 1.14d address `"0x…"` or a d2rs label; never compared) | each game-seed draw, `rng-draws` only |
| `unit` | `t`, `type`, `guid`, `class`, `mode`, `x`, `y`, `life`, `mana` | snapshot ticks, `units` (§4 rule 7) |
| `stats` | `t`, `type`, `guid`, `base`: `[[stat, layer, value], …]` | snapshot ticks, `stats` (§4 rule 8) |
| `end` | `t` = the script's `end` | last line |

What is compared and what is masked: `specs/tools/scenario.md` §5–§6
(the masks are `specs/tools/scenario-masks.tsv`, each citing the spec
that states the bytes are unwritten or clock values, as
`specs/sim/intents-events.md` §6 rule 3 requires).

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
| 1 | 2026-10-06 | §Scenario raw recordings (`scenario-raw-0`, provisional; no bump to the trace format). |

| 1 | 2026-10-06 | Added §Scenario traces: a separate JSON-lines kind with its own version (`scenario-trace` 1); format-1 traces unchanged. |
