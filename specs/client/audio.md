# Spec: Client — Audio (decode, mixer, triggers)

- **Status:** draft; d2rs-own design draft (2026-10-06, architecture
  session). Part (a) is our design; part (b) lists original behavior to
  reproduce, each with an owner spec to be written locally (RE).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio` (scheduler, mixer, voice log;
  plain Rust core), `d2-client::audio::output` (Bevy/rodio stream)
- **Related specs:** `formats/mpq.md` §12 (ADPCM sectors), `data/loading.md`
  §3.4 (`sounds.txt`, `soundenviron.txt` read at runtime),
  `formats/cof.md` (frame event 3 = sound), `sim/intents-events.md`
  (S→C 0x2C `PlaySound`), `sim/tick.md` (tick numbers),
  `client/assets.md` §A5 (sound budget)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 32–43 |
| Inputs | 44–53 |
| Outputs / state changes | 54–57 |
| Rules | 58–59 |
|   A. d2rs design (ours) | 60–186 |
|   B. Original behavior to reproduce (owners) | 187–201 |
| Constants & data dependencies | 202–206 |
| Randomness | 207–211 |
| Edge cases & original bugs | 212–218 |
| Test vectors | 219–231 |
| Provenance | 232–242 |
| Open questions | 243–262 |
<!-- /index -->

## Summary

Sounds are decoded to exact i16 samples from the user's archives, started
by **trigger events** stamped with the tick they belong to, and mixed by
our own integer mixer. Fidelity is measured where the original can be
observed exactly: the **decoded samples** of every sound file and the
**voice log** (which file, at which tick, with which integer volume, pan,
loop and stop parameters). The mixed output stream is ours: it is
deterministic and golden-tested, but it is not compared to the original
(the original hands voices to DirectSound; its mix is not ours to
reproduce).

## Inputs

| Name | Type | Source |
|---|---|---|
| sound files | `.wav` bytes after MPQ decode | `client/assets.md` |
| sound table | `sounds.txt`, `soundenviron.txt` rows | `data/loading.md` §3.4 |
| sim events | sound-bearing events with tick `t` | bridge (snapshots/events, S→C 0x2C, unit modes and frames) |
| UI events | panel/click events at client tick `t` | `client/ui.md` §B8 |
| listener | player position, settings volumes | bridge, `ClientConfig` |

## Outputs / state changes

Voice log entries (§A5), a stereo i16 stream to the device.

## Rules

### A. d2rs design (ours)

#### A1. Decode path

1. `ArchiveSet` reads the `.wav` file; sector decompression (Huffman +
   ADPCM, `mpq.md` §9–§12) is done in `d2-formats` and gives RIFF bytes
   (all 4,992 such files decode to their RIFF size, `mpq.md`
   Observations).
2. A RIFF/WAVE parser (`d2-formats::wav`, owner `formats/wav.md`, to be
   written from a survey of the live files, §B1) gives
   `Sound { rate, channels, samples: Vec<i16> }`.
3. Stored once per file in the sound pool (`client/assets.md` §A5).
   No resampling, no float, at decode.

Exactness check 1 (**decoded samples**): for every live `.wav`, our
`samples` equal the samples 1.14d hands to its sound output for that file
(§B1 measurement), compared as i16 arrays.

#### A2. Triggers

Every sound start is a `Trigger`:

```
Trigger { tick: u32, source: TriggerSource, sound: SoundId,
          params: VoiceParams, cause: CauseTag }
```

- `tick` is the simulation tick the cause belongs to (from the bridge's
  tick stamp), or the client tick for UI causes. Triggers are produced by
  plain-Rust rule functions from snapshots and events, one function per
  cause class (§B2–§B6). The audio module never inspects Bevy state.
- `SoundId` and `VoiceParams` (integer volume 0..=255 or as §B3 says,
  integer pan, loop flag, priority, group) come from the §B3 rules over
  `sounds.txt`. Floats are not used.
- Random choices among sound variants use a client RNG
  (`d2_sim::rng::Seed` algorithm, its own seed and draw order per §B3),
  never the sim's seeds and never a thread RNG. This adds a
  `d2-client → d2-sim` dependency for the RNG type only (allowed by
  `tools/depcheck`); no sim state is touched.
- Triggers for one tick are processed in the order the §B rules emit
  them; the scheduler keeps that order (stable).

#### A3. Scheduler and clock

- The audio clock is slaved to the presentation tick: when the client
  presents tick `t`, all triggers with `tick ≤ t` not yet started are
  started at the next mixer block.
- Voice limits, stealing and per-sound repeat suppression are §B3; the
  scheduler exposes a `VoicePolicy` hook for them.
- Stops (unit death, area change, loop end) are also tick-stamped
  events and are logged.
- **One-shot end tick (d2rs model).** A non-looped voice started at
  tick `t0` ends at the first presented tick `t` with
  `(t − t0) × 40 ms ≥ duration` (duration = sample count / file rate, in
  integer ms; compared as `(t − t0) × 40 × rate ≥ samples × 1000`). 1.14d
  is not tick-exact here: a 50 ms wall-clock service thread
  (`0x00516250`, a `WaitForSingleObject` loop with timeout 0x32, started
  through the pointer at `0x00516558`) notices the end, and the next
  sound-tick upkeep (`0x004DF890`, 16 slots of 0x20 bytes from
  `0x007C8A80`) sees it (owner: `audio/sound-table.md` §6.6). Voice-log
  `Stop` ticks of one-shots are therefore excluded from the §A5
  comparison until `audio/sound-table.md` OQ12 fixes a conformance rule.
- **Seeded choices (conformance input).** 1.14d draws sound variants,
  NPC greetings and unit sound timers from the local player's client
  unit seed, which the draw phase also steps once per drawn frame (frames
  are dropped under load) and the cursor steps on `GetTickCount` time
  (`audio/sound-table-2.md` §14.3, §14.4; `sim/rng.md` §7). So the seed
  at a sound draw cannot be replayed from the tick sequence. The
  conformance check of variant / greeting / timer choices therefore
  takes, for each sound draw, the seed recorded before it in the 1.14d
  run (the roll hook of `docs/handoff/local-buddy-q-rec.md` entry 74)
  as input, sets the d2rs sound RNG to it, and compares the chosen value
  (and the resulting `VoiceEvent` fields), not the seed sequence. Runs
  without that input exclude from §A5 the `file` of variant-chosen
  starts and the ticks of seed-timed starts.

#### A4. Mixer

- Fixed output rate `R` = 44,100 Hz stereo, block 512 frames (ours;
  changeable). Each voice resamples from its file rate with an integer
  phase accumulator (32.32 fixed point), nearest-sample (no
  interpolation) unless §B3 shows the original asks for a different
  playback rate per voice.
- Gain: `out = (s × vol × pan_l) >> shift` in i32, summed per block in
  i32, saturated to i16. Pan and volume curves are tables from §B3, not
  formulas we invent.
- `GainCurve(v, pan, occ)`: the occlusion `occ` is an input next to
  volume `v` (0..=255) and pan. 1.14d's device gain is
  `trunc((1 − occ) × trunc(v × G / 255)) / 255` with `G` = 255 in game
  (global at `0x0072F9B0`; `0x005157B0` divides by 255 with the
  0x80808081 reciprocal, then applies `1 − occ` from voice +0x3C to
  every voice except one with flag 0x4 at +0x34 while `0x00513B90`
  returns 2, the EAX case: `0x005157DF`–`0x005157F0`). `occ` moves
  toward its target in steps of 0.05 (owner: `audio/sound-table.md`
  §6.4, §8.3 rule 3). The product is x87 arithmetic on the f32 `1 −
  occ` and is truncated (`0x00682FD0`), so an integer rewrite in
  hundredths is not equal by construction ((1 − 0.05f) × 200 truncates
  to 189, not 190). The occlusion state is an f32 whose exact step rule and reachable
  values are `audio/sound-table.md` §6.4 r4 (answers open question 4):
  `GainCurve` keeps that f32 state and computes `floor((1 − occ) × v1)`
  exactly (both factors and the product are exact in f64 for v1 ≤ 255),
  not a table; 492 of the 23 × 256 (state, v1) cells of the one-unit
  states differ from an integer hundredths rewrite.
- The mixer is pure: `mix(voices, block) -> [i16; 1024]`; golden tests
  hash the output of scripted voice sets (determinism, not fidelity).
- Output: one custom `rodio::Source` registered through Bevy's
  `Decodable` (checked in pinned `bevy_audio 0.19.1`: the decoder item is
  `rodio::Sample`, an f32). i16 → f32 is `s / 32768.0`, exact in f32;
  device conversion and resampling after that are outside the
  exactness boundary.

#### A5. Voice log (exactness check 2)

Every start and stop appends a record:

```
VoiceEvent { tick: u32, kind: Start | Stop | Param, file: CanonicalPath,
             vol: i32, pan: i32, looped: bool, cause: CauseTag }
```

- `d2-client --audio-log FILE` writes it as JSON lines with a header
  `{ "format": "d2rs-audio-log", "version": 1 }` (M20).
- The original's counterpart is a trace of the sound output calls with
  the tick number (§B7). Comparison: identical sequence of
  `(tick, kind, file, vol, pan, looped)` for a replayed recording.
  `cause` is ours (debugging) and not compared.

### B. Original behavior to reproduce (owners)

Owners (2026-10-08): B1 `formats/wav.md` §5; B3, B8 `audio/sound-table.md` (+ `audio/sound-table-2.md` §16–§17); B2, B6 `audio/triggers.md`, `audio/triggers-2.md`; B4, B5 `audio/environment.md`; B7 the Checks tables of those specs.

| # | Behavior | Owner spec | Measure | Comparison |
|---|---|---|---|---|
| B1 | WAV subset in 1.14d archives (format tags, bit depths, rates, channels, chunk order) and the samples 1.14d passes to its sound output | `formats/wav.md` (written; sample dump queued, its OQ1) | survey of every live `.wav` (`mpq-tool formats` extension); debugger dump of buffers at the sound output | identical i16 samples per file |
| B2 | Which sim events make sounds: S→C 0x2C fields (`server-messages.tsv`, status partial), unit mode changes, COF frame event 3, missiles, skills (`skills.txt` sound columns), monsters (`monsounds`), items (drop/use sounds), objects | `audio/triggers.md`, `audio/triggers-2.md` (all causes; COF event 3 runs the skill do, `triggers-2.md` §15) | packet + sound-call trace on a recorded game | identical (tick, file) sequence |
| B3 | `sounds.txt` semantics: volume, pan from listener distance, falloff, priority, groups and variants (and their RNG), loop, repeat suppression, voice limit and stealing | `audio/sound-table.md`, `audio/sound-table-2.md` | sound path; traces with known positions | identical (vol, pan, looped) per voice event |
| B4 | Environment and ambient sound: `soundenviron.txt` by level, day/night, random ambient cues | `audio/environment.md` §1, §5–§8 | traces while walking between areas | identical voice log |
| B5 | Music: which track per level, transitions, loop | `audio/environment.md` §2–§4 (front end §9) | traces | identical (tick, file) |
| B6 | UI and speech sounds (NPC dialog, quests, item pickup, panel clicks) | `audio/triggers.md` §9–§11, `triggers-2.md` §17 | traces | identical voice log |
| B7 | Recording the original's sound calls with tick numbers | `tools/trace-recorder` (`record_sound.py`, on `origin/claude/local-buddy-q-rec-2026-10-07`) + `traces/FORMAT.md` | debugger hooks on the sound-output entry points | a static scene recorded twice gives identical logs (stability first) |
| B8 | Volume settings (sound/music sliders) to integer volume | `audio/sound-table.md` §9, §8.2; slider → 0–100 `sound-table-2.md` §15 | traces at known slider settings | identical vol |

## Constants & data dependencies

Ours: `R` = 44,100 Hz, block 512 frames, log format version 1. Data:
`sounds.txt`, `soundenviron.txt` (runtime, `data/loading.md` §3.4).

## Randomness

Variant choices and ambient cues use a client-side seeded RNG (§A2); its
seeding and draw order are §B3/§B4. No other randomness.

## Edge cases & original bugs

To be listed by the §B owners. Design rule: a trigger naming a missing
file is an error logged with its cause; the voice log records it as a
`Start` with `file` and an error flag, so a log comparison still shows
it.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| ADPCM mono vectors | as `mpq.md` Test vectors (already pass) | `mpq.md` §12 |
| two triggers in one tick | started in emission order | §A2 |
| trigger at tick 10, presented tick 9 then 10 | starts in the block after tick 10 is presented | §A3 |
| mixer: one voice, full volume, centered, constant 1000 | output per §B3 tables (vector added with B3) | §A4 |
| mixer: two voices summing past 32767 | saturates to 32767 | §A4 |
| file rate 22,050 → 44,100 | each sample output twice (nearest) | §A4 |
| scripted voice set | stable output hash across runs and platforms | §A4 golden |
| audio log header | `{"format":"d2rs-audio-log","version":1}` | §A5 |

## Provenance

Design decided 2026-10-06 (architecture session) from `mpq.md` (ADPCM,
observations), `data/loading.md` §3.4, `cof.md` events and the S→C
message table. Bevy audio interface checked in the pinned registry
source. Original behaviour is owned by the §B specs; the two facts
below are stated only as inputs to our design. §A3 end-tick and §A4 occlusion inputs (PC 2 request, spec-audio):
1.14d asm of `0x00516250`, `0x004DF890`, `0x005157B0`. §A3 seeded-choice input (PC 2 request,
spec-audio pass 2): `0x00470390` (`0x004703E1`, `0x004704DD`, `0x004705B6`, `0x004705D7`) re-read;
the seed-user list is `audio/sound-table-2.md` §14. No original behavior is stated here.

## Open questions

1. ~~§B1–§B8~~: every row has its owner spec (§B table); their own
   open questions and the recordings queued in `docs/HANDOFF.md` §7
   (PC 2 recording list) remain.
2. *Answered* (static): no per-voice rate change. No code in the sound
   driver (`0x00513000`–`0x00517FFF`) or the Storm stream player
   (`0x00413000`–`0x0041CFFF`) calls the buffer's `SetFrequency` (vtable
   +0x44 of the DirectSound buffer; the only +0x44 slot call there,
   `0x005161E7`, is on the 3D-buffer interface at voice +0x0C); a
   buffer plays at its file's rate (`formats/wav.md`). The mixer needs
   no rate field.
3. ~~Whether 44,100 Hz suits all devices~~: a d2rs decision, not a
   fidelity question (no effect on either exactness check); decided
   when the output is first used.
4. *Answered* (`0x004BA333`–`0x004BA398`, constants `0x006DA6B0`,
   `0x006DA690`; enumeration by a scratch script over the step rule at
   24, 53 and 64-bit precision): §A4 "Occlusion state". No fixed
   table: mean targets add states; keep the f32 state.
