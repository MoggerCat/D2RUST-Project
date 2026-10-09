# Spec: Tools — audio-diff (1.14d's sound output against d2rs', per voice and mixed)

- **Status:** draft: implemented (`tools/audio-diff/`, `d2-client`
  `app::audio_dump`, `play --audio-dump`, `audio-mix`); the 1.14d capture
  runs under Wine in the cloud (measured 2026-10-09, §2 r1); four checks in
  `traces/audio/` (§5).
- **Target version:** 1.14d (the original side); formats `audio-raw-1`,
  `audio-voices-1`, `d2rs-audio-dump-1`, `audio-diff-summary-1`.
- **Crate/module:** `tools/audio-diff/record_audio.py`, `capture.sh`,
  `audio_diff.py`; `d2-client` `app::audio_dump` (and two accessors in
  `audio::AudioEngine` / `audio::mixer::Voice`).
- **Related specs:** `client/audio.md` (§A3 scheduler, §A4 mixer, §A5 voice
  log, §B), `audio/sound-table.md` (§6.6 channel end, §7 channels, §8.3
  device curves), `formats/wav.md` (§3 samples, OQ1 sample dump),
  `tools/scenario-diff.md` §2 (the check file), `tools/rng-trace.md` (the
  debugger recorders).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–53 |
| Inputs | 54–61 |
| Outputs / state changes | 62–69 |
| Rules | 70–71 |
|   1. What is compared | 72–92 |
|   2. The 1.14d side | 93–159 |
|   3. The d2rs side | 160–186 |
|   4. Comparison | 187–218 |
|   5. Checks and the one-command run | 219–238 |
| Constants & data dependencies | 239–245 |
| Randomness | 246–251 |
| Edge cases & original bugs | 252–263 |
| Test vectors | 264–272 |
| Provenance | 273–281 |
| Open questions | 282–292 |
<!-- /index -->

## Summary

CLAUDE.md rule 10 holds audio to identical decoded samples and identical
trigger ticks. 1.14d plays every sound through 16 DirectSound channel
buffers (`audio/sound-table.md` §7 r7) that it fills itself; what it writes
into them, and when, is the observable output. The recorder hooks the
DirectSound buffer methods of the running 1.14d `Game.exe` (no game
internals besides the tick), keeps every written byte, and stamps every
call with the server frame and the sound tick. d2rs dumps its voices (file,
start tick, decoded-sample digest, device volume and pan) and a
deterministic per-tick mix. The comparison pairs voices by start tick and
samples, then compares device volume and pan exactly, and compares the two
voice sets mixed through one mixer, tick by tick. Wine's own mixed output
(float, resampled) is never compared: the device mix is not 1.14d's
(`client/audio.md` Summary).

## Inputs

| Name | Type | Source |
|---|---|---|
| a check | `.check` file, `channels audio` | `traces/audio/` (§5) |
| the 1.14d install | `Game.exe` (sha256-checked), MPQs | private data repo (`$HOME/game`) |
| d2rs | `d2-client play` | this repository |

## Outputs / state changes

`target/audio-diff/<check>/`: `orig/audio.jsonl` (capture), `orig/blobs/`
and `pcm/` (the game's samples: never committed, CLAUDE.md rule 1),
`orig.voices.jsonl`, `orig.mix.jsonl`, `d2rs.audio.jsonl`,
`audio.summary.json`. Exit code 0 match, 1 diverged, 3 error. Only digests,
sizes, ticks and device integers leave the work folder.

## Rules

### 1. What is compared

1. **Per voice** (the exactness check): for each voice d2rs starts, at
   sound tick `T` with file `F`, 1.14d starts a voice at the same `T` on a
   channel with `F`'s channel count whose written stream begins with
   exactly `F`'s decoded samples (`formats/wav.md` §3: the `data` bytes as
   i16 LE), followed by zero bytes (non-looped) or the loop region again
   (looped, `audio/sound-table.md` §7 r4), as far as the capture reaches;
   and its device volume and pan at `Play` equal d2rs' (§4 r3). Every
   unpaired voice on either side is a difference.
2. **Mixed** (a summary of 1): both voice sets mixed through d2rs' mixer
   (`client/audio.md` §A4) by the same tick stepper (§3 r3) give the same
   i16 output per tick. PROVISIONAL (REC-1362): 1.14d's own mix is
   DirectSound's (Wine's or Windows'), not 1.14d code, so the mixed check
   is defined on the voices, not on a device recording; settled by the
   decision log (`client/audio.md` Summary already excludes the device mix).
3. Not compared: the ticks of the refill writes after the first (they
   follow the device's play cursor, wall-clock), the `Stop` ticks of
   one-shots (the 50 ms service thread, `client/audio.md` §A3), front-end
   sounds (no server frame).

### 2. The 1.14d side

1. **Device.** Under Wine the game needs an audio device; `capture.sh`
   writes `~/.asoundrc` with a default ALSA device `type null` (with
   `AUDIO_DIFF_KEEP_MIX=1` `type file` → `null`; Wine 9.0 `winealsa.drv`,
   `libasound2:i386`) and runs the recorder
   through `tools/cloud-game/run.sh --python`. The game runs without
   `-ns`. Measured 2026-10-09: `DirectSoundCreate` succeeds, 28 buffers are
   created, the game plays.
2. **Hooks** (`record_audio.py`, on `record_tick.TickRecorder`). The 1.14d
   `Game.exe` imports `DSOUND.dll` statically: IAT slot `0x006CC088` is
   ordinal 1 (`DirectSoundCreate`), `0x006CC084` ordinal 2. At the entry
   point the slot is read and `DirectSoundCreate` hooked; at its return
   `*ppDS` gives the `IDirectSound` vtable: `CreateSoundBuffer` (slot 3)
   and `DuplicateSoundBuffer` (5) are hooked; at their return `*ppBuf` and
   the `DSBUFFERDESC` (flags, bytes, `WAVEFORMATEX`) make a `create`
   record and the first secondary buffer's vtable gives `Release` 2,
   `Lock` 11, `Play` 12, `SetCurrentPosition` 13, `SetVolume` 15,
   `SetPan` 16, `SetFrequency` 17, `Stop` 18, `Unlock` 19. The sound
   request entry `0x004B9A00` is logged as `record_frames.py --sounds`
   does.
3. **Records** (`audio-raw-1`, JSON lines after `record_tick`'s header):
   `create` (`b` = buffer number in creation order, `flags`, `bytes`,
   `fmt`), `write` (at `Unlock`: `b`, `off` = the matching `Lock` offset,
   `n1`, `n2`, `len`, `sha256` of the bytes of both regions in order; the
   bytes in `<blob-dir>/<sha256>.pcm`), `play` (`flags`), `stop`,
   `volume` / `pan` (`v`, signed hundredths of a dB), `setpos`, `freq`,
   `release`, `request`. Every record carries `f` (server frame, `null`
   before the game ticks), `T` (sound tick `[0x007BC9BC]`), `C` (client
   updates `[0x007A0498]`); method records carry the thread id.
4. **No lost calls.** The stream thread and the main thread both enter
   `Lock` / `Unlock`. Lifting an INT3 for a single step lets another thread
   pass it unseen (measured: 10 of 101 music writes lost); suspending the
   other threads during the step deadlocks under Wine. So each hook keeps
   its INT3 and resumes the thread in a trampoline (the relocated first
   instruction, decoded by `trace-recorder/x86emu.py`, and a `jmp` back);
   only an undecodable first instruction is single-stepped (noted).
   Check: `lock` count = `unlock` count, and the ring offsets of one voice's
   writes are contiguous (§2 r5 counts gaps).
5. **Voices** (`audio_diff.py voices`). 1.14d's 16 channel buffers are
   created once (4 × 256 KiB stereo, 12 × 128 KiB mono, 22,050 Hz 16-bit:
   `audio/sound-table.md` §7 r7) and used as rings. A voice starts at a
   `SetCurrentPosition(0)` on a channel buffer once a write or a `Play`
   followed the previous start; its tick is that call's `T`. Its stream is
   every write until the next start, in order. Its device volume and pan
   are the buffer's at its first `Play` (a DirectSound buffer keeps them
   from the previous voice when the start sends none); later `SetVolume` /
   `SetPan` before its `Stop` are parameter changes. PROVISIONAL
   (REC-1360): the start tick is the `T` at `SetCurrentPosition` (the main
   thread's start sequence `SetCurrentPosition(0)`, `SetPan(0)`,
   `SetVolume(0)`, first fill, `SetPan`, `SetVolume`, `Play(LOOPING)`,
   all at one `T` in every measured start); settled by a capture with the
   channel start `0x004E01B0` hooked next to these.
6. **Device curves.** Every captured `SetVolume` value is
   `trunc(−2000 × log10(255 / v))` for an integer `v` (0 → −10,000, 255 →
   0) and every `SetPan` value `±trunc(−2000 × log10(127 / p))`
   (`audio/sound-table.md` §8.3: negative attenuates the right side, pan
   < 128; positive the left, pan > 128); rounding misses 55 (flooring more)
   of 110 measured volume values. So each device integer maps back to one
   `v` (0–255) and one pan (0–255) (`audio_diff.py` `vol_of`, `pan_of`).
   PROVISIONAL (REC-1361): the conversion is f64 with truncation; settled
   by more captures (any device value off the f64 curve) or a reading of
   `0x005165F0`'s x87 sequence.
7. With `AUDIO_DIFF_KEEP_MIX=1`, Wine's mixed output lands in
   `wine-mix.raw` (float32 at the device rate, after Wine's resampler,
   about 150 MB a minute): for listening only.

### 3. The d2rs side

1. `d2-client play … --audio-dump FILE --audio-ticks N` runs the play mode
   with the original sound layer and **no audio device**: the dump drives
   the audio engine; the app exits once the server tick reaches `N`.
2. **Format** `d2rs-audio-dump-1`: a header
   `{"format":"d2rs-audio-dump","version":1,"rate":44100,"block":512,"frames_per_tick":1764}`,
   then per record one JSON object: `start` (`tick` = the trigger's sound
   tick, `server_tick`, `file`, `vol`, `pan`, `looped`, `occ`,
   `loop_start`, `channels`, `frames`, `sha256` of the decoded samples as
   i16 LE, `dev_vol`, `dev_pan` = §2 r6's curves applied to
   `device_occluded(vol, occ)` and `pan`), `stop` / `param` /
   `start-failed` (the voice log records, `client/audio.md` §A5), `mix`
   (`tick`, `frames`, `sha256` of that tick's mixed i16 LE output). Each
   started sound's decoded samples are also written once, as
   `FILE.pcm/<sha256>.pcm` (i16 LE; the user's game data, kept in the work
   folder like the 1.14d blobs).
3. **Tick stepper** (`app::audio_dump::Stepper`): for each sound tick
   `t` from the last one stepped + 1 to the presented tick, present `t`
   (`client/audio.md` §A3) and mix whole 512-frame blocks while the frames
   mixed stay ≤ (t − t₀ + 1) × 1,764 (44,100 Hz × 40 ms; t₀ = the first
   tick stepped). Frame pacing therefore does not change the dump.
4. `d2-client audio-mix VOICES OUT --last-tick N` mixes an
   `audio-voices-1` list (§4 r1) with the same stepper and the device gain
   (`DeviceGain`, occlusion 0: the list's volumes are already after it)
   and writes `mix` records.

### 4. Comparison

1. `audio-voices-1` (`audio_diff.py voices`): header
   `{"format":"audio-voices","version":1}`, one voice per line: `tick`,
   `label`, `channels`, `pcm` (path of the stream, relative to the list),
   `vol`, `pan` (§2 r6 integers), `params` ([tick, vol, pan] after the
   start), `stop` (tick or null), and for diagnosis `buffer`, `f`, `C`,
   `play`, `dev_vol`, `dev_pan`, `bytes`, `sha256`.
2. **Naming.** Each 1.14d voice is named by the first sound d2rs decoded
   in the same run whose samples its stream matches (§1 r1, byte for
   byte over the overlap); a 1.14d voice no d2rs sound matches stays
   unnamed (reported by digest and length).
3. **Pairing**, d2rs starts in dump order, each against the unpaired
   1.14d voices: (a) same `T`, same channel count, matching stream →
   paired; then `dev_vol` and `dev_pan` must be equal and the tail rule
   of §1 r1 hold (a capture shorter than the file is compared as far
   as it goes and listed as `partial`, not a difference); (b) same `T`, a 1.14d voice of the same variant
   family (file name without trailing digits) → `variant` (the seeded
   pick, `client/audio.md` §A3); (c) the same file at another `T` →
   `tick` (both ticks given); (d) one unnamed 1.14d voice at that `T` →
   `samples` (the first differing sample); (e) else `voice` (d2rs starts
   a sound 1.14d does not). Every 1.14d voice left is a `voice`
   difference (1.14d starts a sound d2rs does not); d2rs' failed starts
   too.
4. **Differences** are listed in tick order with a count per kind.
   `mixed`: the number of ticks both mixes cover, how many are equal, the
   first unequal tick.
5. **Verdict:** `MATCH` when there is no difference, every mix tick is
   equal and d2rs started at least one voice; `EMPTY` when neither side
   started a voice; else `DIVERGED`. `audio-diff-summary-1` holds the
   counts, the first difference and up to 200 differences.

### 5. Checks and the one-command run

1. `python3 tools/audio-diff/audio_diff.py run traces/audio/<name>.check`
   reads the check with `scenario_diff.parse` (`channels audio` only),
   builds the save, runs both sides, `voices`, `audio-mix` (up to the last
   tick both cover) and `compare`. `--orig-only` / `--d2rs-only` run one
   side, `--reuse` keeps outputs.
2. The four authored checks:

| Check | Scenario | Exercises |
|---|---|---|
| `audio-town-ambience-ama` | Rogue Encampment arrival, idle, 250 ticks | town music (stream), ambience bed and its fade-in, NPC footsteps, rain |
| `audio-walk-town-ama` | `walk-town-ama`'s click, 120 ticks | the player's footsteps (`audio/triggers.md` §5) |
| `audio-cast-frost-nova-sor` | `sor-frost-nova-twice` (empty Blood Moor), 100 ticks | cast and missile sounds (§8) |
| `audio-monster-hit-ama` | `combat-fallen-hits-player`, 200 ticks | monster attack, hit, get-hit sounds |

3. These checks live in `traces/audio/`, not `traces/checks/`, so the
   scenario-diff suite (whose channels do not include `audio`) does not
   pick them up.

## Constants & data dependencies

IAT slot `0x006CC088` (DirectSoundCreate), sound tick `0x007BC9BC`,
client updates `0x007A0498`, sound request `0x004B9A00` (prologue
`55 8B EC 83 EC 18`), the tick hook of `record_tick.py`; COM vtable slots of
`dsound.h`; 44,100 Hz, 512-frame blocks, 40 ms ticks (d2rs mixer).

## Randomness

None in the tools. The sound layer's seeded choices (variants, greetings,
timers) follow the client seed (`client/audio.md` §A3 "Seeded choices"): a
variant difference shows as a `voice` difference at that tick.

## Edge cases & original bugs

- A voice shorter than one ring keeps the rest of the ring at zero; a
  sound longer than the ring is refilled in halves (64 KiB mono, 128 KiB
  stereo) or 32 KiB chunks (stream songs).
- `Play` is always called with `DSBPLAY_LOOPING` (the ring); a sound's
  own looping shows only in the stream (§1 r1).
- A voice whose start sends no volume or pan keeps the buffer's previous
  values (DirectSound state), which §2 r5 reproduces.
- The front end's sounds (title music) have no server frame and are
  skipped.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| town arrival ScnAma seed 1234, 250 ticks | 28 `create`, `lock` = `unlock` = 262, the town music voice's 101 writes = `music\act1\town1.wav` `data` bytes 0–3,309,567 | capture 2026-10-09 (cloud, Wine 9.0) |
| measured device values | volume −128, −885, −415, −1187, −754, −2460, −2168, −78; pan 1377, −595, −492, −348, −252 are curve values (§2 r6) | same capture; `audio_dump.rs` test |
| d2rs stepper: start at tick 5, ticks 3..6 | mix frames 1536, 1536, 2048, 1536 | `audio_dump.rs` test |
| `record_audio.py --selftest`, `audio_diff.py --selftest` | hook chain, voice building, pairing, mixed first tick | selftests |

## Provenance

Measured on the 1.14d `Game.exe` under Wine 9.0 in the cloud
(2026-10-09): the import table (DSOUND ordinals 1, 2), the buffer
layout, the start sequence and the device values come from the
capture; the sample equality from `mpq-tool extract` of `town1.wav`.
DirectSound method order is `dsound.h`'s. No decompiled code was read
for this spec.

## Open questions

1. Windows vs Wine (REC-1363): the API calls 1.14d makes do not depend on
   the DirectSound implementation except the refill timing; settled by
   one `record_audio.py` run on PC 1 (`docs/handoff/pc1-data.md` Step 4)
   compared with the cloud capture (`audio_diff.py voices` on both: same
   voices, ticks, samples and device integers).
2. Stream voices (music) start on the stream thread: their `Play` `T` may
   lag the main thread's by one; the start tick uses the main-thread
   `SetCurrentPosition` (REC-1360).
