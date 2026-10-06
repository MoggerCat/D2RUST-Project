# Handoff: audio core (`d2_client::audio`, PLAN Phase 6 C10)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/p6-audio`, based on `claude/bold-ptolemy-jvyvxy` at
`978e6c4` (2026-10-06, cloud). Spec: `specs/client/audio.md` (draft,
d2rs-own design) §A2–§A5 and Test vectors.

## State

**Implemented, unverified against 1.14d** (nothing in §A2–§A5 is original
behavior except what the voice log will be compared to; that comparison
needs §B7 traces). The d2rs-own parts are done and tested in CI: trigger
queue, tick scheduler, integer mixer, voice log `d2rs-audio-log 1`
(writer, strict parser, comparison), rodio `Decodable` output edge. 25
unit tests in `crates/d2-client/src/audio/tests.rs`, synthetic samples
only, no game files, no audio device.

Gate run on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client`,
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`, `python3 tools/coverage.py --check`
(`audio.md`: 6 of 10 rules claimed; §A1 r1/r3 and §B are other tasks):
all clean.

Changes outside `crates/d2-client/src/audio/`: one line in
`crates/d2-client/src/lib.rs` (`pub mod audio;`). No dependency, spec,
`main.rs` or other-module change. The output edge uses `bevy::audio`
(rodio 0.22.2 through `bevy_audio 0.19.1`, already in Bevy's default
features), so no Cargo line was needed (M10).

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/audio/mod.rs` | types (`SoundId`, `TriggerSource`, `VoiceParams`, `Trigger`, `CueId`, `Stop`/`StopTarget`, `ParamChange`, `Cue`), `TriggerQueue`, seams `CueSource`, `SoundBank`, `VoicePolicy` (+ `Unlimited`), `AudioEngine` (present / mix_block / log / errors), `AudioError` | §A2, §A3, Edge cases |
| `crates/d2-client/src/audio/mixer.rs` | `Sound` (strict), `Gains` (Q8, strict), seam `GainCurve` (+ `UnityGain`), `Voice` (32.32 phase), `mix` | §A1 (input type), §A4 |
| `crates/d2-client/src/audio/log.rs` | `VoiceEvent`, `VoiceKind`, `VoiceLog::{to_jsonl, parse}`, `header()`, `compare_logs` → `LogMismatch` | §A5 |
| `crates/d2-client/src/audio/output.rs` | `MixerStream` (Bevy `Asset` + `Decodable`), `MixerDecoder` (rodio `Source`, 2 ch, 44,100 Hz), `to_device`, `register(app)` | §A4 Output |
| `crates/d2-client/src/audio/tests.rs` | every §A2–§A5 test vector, golden hashes, strict-parser and perturbation tests | Test vectors |

## Design choices made here (ours; within the spec's latitude)

- **Cue ids.** Every queued cue gets a `CueId` in emission order; a started
  voice keeps its trigger's id, and `Stop`/`ParamChange` name it
  (`StopTarget::All` for area changes). The spec's `Trigger` struct is
  kept field for field; ids live beside it.
- **Ordering.** Cues are released by `present(t)` for `tick <= t`, sorted
  by tick with emission order kept inside a tick (stable sort), and
  applied at the start of the next `mix_block` in that order. Starts,
  stops and param changes share one ordered stream.
- **Ticks.** `present` rejects a tick lower than the previous one
  (`TickBackwards`). A cue arriving for an already presented tick still
  starts (log keeps its own tick) and is reported as `LateCue`.
- **"Nearest-sample" = the frame at `floor(phase)`** (no rounding). This is
  what makes the spec vector "22,050 → 44,100: each sample output twice"
  hold from frame 0.
- **Gain arithmetic.** `Gains { vol, pan_l, pan_r }` are Q8 in `0..=256`
  (checked), `out = (s × vol × pan) >> 16` in i32 (fits exactly, also for
  −32768 at unity), per-block sum with `saturating_add`, clamp to i16.
- **Voice end.** A one-shot voice is removed after its last frame; this is
  *not* logged (it is not a tick-stamped event). A looped voice wraps to
  frame 0 (whole file).
- **Errors** (M07): `Sound::new` and `Gains::new` are strict; a start
  whose sound id has no file, whose file is missing, or whose gains are
  out of range is logged as a `Start` with `error: true` (file `#<id>`
  when the id has no file) and reported in `take_errors()`.
- **Voice log.** JSON lines, header
  `{"format":"d2rs-audio-log","version":1}`, fixed key order
  `tick, kind, file, vol, pan, looped, error, cause`, no whitespace,
  escapes only `\"`, `\\`, `\u00xx` (< U+0020). The parser accepts
  exactly that (no leading zeros, no `-0`, no extra keys, final newline
  required). `error` is added to the spec's record (its Edge cases ask
  for the flag). `compare_logs` compares
  `(tick, kind, file, vol, pan, looped)` only and reports per-record
  field lists plus a length mismatch; perturbation test per field (M08).
  The spec's format section lives in `audio.md` §A5; if the field list
  grows, bump the version (M20).
- **Output edge.** `MixerStream` wraps `Arc<Mutex<AudioEngine>>`; the
  decoder mixes a block whenever its buffer is empty and yields
  `s / 32768.0`. A poisoned lock ends the stream (no stale playback).
- **Golden hashes.** FNV-1a 64 over the LE bytes of 8 mixed blocks of a
  scripted set (rates 11,025 / 22,050 / 32,000 / 44,100, mono/stereo,
  one-shot/looped, a test gain curve, a param change, a stop-all) and over
  the resulting log text. Determinism only, not fidelity (§A4).

## Seams (TODO hooks; nothing original invented)

| Hook | Neutral behavior now | Owner spec |
|---|---|---|
| `CueSource` (the narrow trait the bridge implements: rule functions turn snapshots/events into cues and push them in emission order) | nothing implements it yet | `audio/triggers.md` (§B2, §B6), `audio/environment.md` (§B4, §B5) |
| `SoundBank` (id → canonical path + decoded `Sound`) | test fakes only; real one needs C1 paths, the sound pool (`assets.md` §A5) and the WAV parser | `audio/sound-table.md` (§B3), `formats/wav.md` (§B1) |
| `GainCurve` | `UnityGain`: unity both sides for every vol/pan | `audio/sound-table.md` (§B3, §B8) |
| `VoicePolicy` | `Unlimited`: admit all; `Reject` logs nothing, `Steal` logs a `Stop` (cause `steal:<cause>`) then the `Start` | `audio/sound-table.md` (§B3) |
| Variant RNG | not implemented; no `d2-client → d2-sim` dependency added | `audio/sound-table.md` (§B3) |
| Stopping a voice that is not playing | logs nothing | `audio/sound-table.md` / §B7 trace |

Not wired yet (needs other modules; out of this task's files):
`d2-client --audio-log FILE` in `main.rs` (call `engine.log().to_jsonl()`
and write it), `output::register(&mut app)` plus spawning
`AudioPlayer(assets.add(MixerStream::new(engine)))` in `app.rs`, and the
bridge calling `pump`/`present` once per presented tick.

## Open questions

1. Tick domains: UI triggers carry the client tick, sim triggers the sim
   tick; the scheduler assumes both count on the one presented-tick
   counter. Confirm with `client/bridge.md` / `ui.md` §B8 when wiring.
2. Should a natural one-shot end appear in the voice log? The original's
   §B7 trace decides; today it does not.
3. Spec open questions 2 (per-voice rate) and 3 (device rate) unchanged.
4. Spec test vector "one voice, full volume, centered, constant 1000"
   waits for the §B3 tables; `one_voice_constant_1000_at_unity_placeholder`
   pins only the placeholder curve.

## Checks to queue (local, after the §B owner specs exist)

- §B1: decoded-samples check per live `.wav` (needs `formats/wav.md` and
  the debugger dump) — not runnable yet.
- §B7: record the original's sound calls twice on a static scene
  (stability), then compare our voice log with `compare_logs` on a
  replayed recording — not runnable yet.
- Smoke (local, has an audio device): once wired in `app.rs`, `d2-client
  view` plays a scripted `Cue` set; expected: audible, no underrun
  errors. Not an exactness check.
