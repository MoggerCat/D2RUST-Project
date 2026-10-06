# Handoff: audio implementation — `claude/impl-audio`

Cloud implementation session, 2026-10-06, medium effort. Base
`claude/specs-staging` at `5844674` (the merges of `spec-formats-wav`,
`spec-audio-buddy`). Repo only, no game files. Every result below is
**implemented, unverified** (METHODS M02): the specs are drafts and no
1.14d sound trace or game-file run exists yet.

## 1. What was built

| Spec | Code | Tests |
|---|---|---|
| `formats/wav.md` §1–§4 | `d2_formats::wav` (`Wav::parse`, `frames`, `is_playable`); `d2_client::audio::pool::D2Wav` (the real `WavDecoder`: parse + the §4 sound-start check, tag 1 / 16-bit / 22,050 Hz / 1–2 ch, else failed to load) | 10 unit (every synthetic vector incl. the pad-free walk misalignment), 1 in `pool`; 2 `#[ignore]` in `d2-formats/tests/wav_game.rs` |
| `audio/sound-table.md` §1–§10, §12 | `d2_data::sounds` (field lists of `0x00481950`, `SoundRow`, `SoundEnvironRow`, `compile_sounds` / `compile_soundenviron` through the existing `compile_table`, record sizes 142 / 88); `d2_client::audio::sound_table::{table, volume, system}` (`SoundTableData`: group pass, block count, song range, path rule §3, variant pick and history; `SoundSystem`: 200-slot request pool, compound, priority +80 byte wrap, fades / stops, sound tick with bubble-sort order, 16 channels with stealing and resume offset, ducks, f32 falloff / pan as the spec writes it then `ftol`; `SoundWorld` seam; `SoundCtx` implements the call traits; `DeviceGain` is the `GainCurve`) | 46 + 1 `#[ignore]` (`sound_table::tests::game`); d2-data `sounds` 3 |
| `audio/triggers.md` §1–§12 + `object-sounds.tsv`, `npc-speech.tsv` | `d2_client::audio::triggers::{events, modes, movement, objects, skills, npc, ui, tables}`: one rule function per cause class; `UnitSound` / `Globals` state; the TSVs parsed strictly with check + perturbation tests (M05, M08) | 30 |
| `audio/environment.md` §1–§8 | `d2_client::audio::environment`: `Environment::tick` (ambience, rain, event cues, then music), `Music` (songs, resume blocks, stingers), `EntryLines`, `SamplePins` | 26 |

Seam: `audio::calls::SoundCalls` (request, volume set, fade, detach, group
stops, speaking, sound tick, `roll` on the client seed; helpers `uniform`,
`jitter`) with two extensions declared by their users,
`triggers::TriggerSound` and `environment::EnvCalls`; all three are
implemented by `SoundCtx` (`sound_system.with(&mut world)`). Rule sets are
tested against recording fakes; the sound system end to end against
`AudioEngine` + `DeviceGain`.

How the parts connect: `SoundSystem::run_tick(world, &mut TriggerQueue)`
pushes every channel start, stop and sent volume / pan change as a `Cue`
stamped with the sound tick T. The engine must then use `Unlimited` (the
sound system owns channels and stealing) and present ticks in the
sound-tick domain.

## 2. Not done (next steps)

1. **App wiring.** `app/sound.rs` still runs `NoSoundTable` / `NoWavDecoder`
   / `NoCues`. Wiring is: `SoundPaths` as its `SoundTable`, `D2Wav` as the
   decoder, `SoundSystem` driven once per sound tick with a `SoundWorld`
   from the bridge, `Environment::tick` before `run_tick`, and the
   trigger functions fed by bridge events (S→C 0x2C, mode changes,
   footsteps, UI). None of the bridge feeds exist yet.
2. The real-data `#[ignore]` tests that need the sound table's loader and
   were not written: environment (50 soundenviron rows, songs in
   4,657–4,684, `find` lines per class) and triggers (all TSV ids resolve
   in `sounds.txt`, class-record ids). Write them on top of
   `SoundTableData::from_txt`.
3. Mixer modes 1–2, start offset and loop start in the core mixer (kept on
   the channel, not played).

## 3. Open points (each a `TODO(spec: …)` in code, neutral behavior)

Sound table (`sound-table.md`):
- ST1 §2: EAX column names not in the spec (spec says 22 read columns;
  named + 12 EAX = 21); not bound.
- ST2 §7 r3: channel kinds `0x004E0050` gives in mode 0 unknown; all 16
  slots accept any request (`ChannelKind::Any`).
- ST3 Which record after a variant pick: the variant's row for sample,
  Stream, Async Only, Loop, blocks, Stereo; the requested row otherwise.
- ST4 Channel end and saved play position on steal come from the device in
  1.14d; modeled as elapsed ticks × 40 ms at the file rate; a natural end
  pushes no stop cue.
- ST5 §5 r2: a merged compound call attaches no unit.
- ST6 §6.4 r1 option flag `0x007A061C`: meaning / default unknown; off.
- ST7 §10: async latency one tick (OQ 10), preload phase tick 0 then every
  25; no eviction.
- ST8 No local player when rolling: returns 0, no seed step.
- ST9 §6.3 reading order (loop restarted by r1 may start the same pass; r5
  only for requests playing before r3); new requests appended to the
  active list end.
- ST10 f32 math (§8) uses Rust `log2` / `powf`; may differ from x87 by one
  ulp at a truncation boundary. The voice-log trace decides.
- ST11 `play_position` in bytes (environment OQ 2); `set_position` writes
  no distance²; `unit_requests` in active-list order.

Triggers (`triggers.md`):
- TR1 §7 r1: 120 of 573 object classes have no TSV row: no call.
- TR2 §7 r4: cairn ids of class 61 (table `0x00728338`) not given: loop step
  skipped.
- TR3 §10 r1: NPC class → greeting record table (35 classes, 28 records)
  not in the spec as data: the caller supplies the record; mode 2
  last/tick update after the attempts.
- TR4 §4.3 r2.2: multi-unit request detached with force, else fade to 0
  over 6.
- TR5 §5 r4: signedness of `f ± s` unstated: u32 wrapping.
- TR6 Request with id 0 where the spec has no guard is still made (logged
  by the entry hook in 1.14d).
- TR7 Spec issue: Test vectors' "115 distinct records" counts addresses;
  the TSV has 106 distinct contents. `npc-speech.tsv` key 506 is
  duplicated (first wins).
- Spec OQs 4, 5, 6, 8, 9, 11 left to the callers (see module docs).

Environment (`environment.md`):
- EN-A §5 r2: second exception of the 72–201 stop passed as 0; "when
  raining" read as this tick's weather flag.
- EN-B §6 r3: rain handle gone → volume 0 → rain stops.
- EN-C §3 r1, k ≠ 0 on a song without that block: resume −1 → offset
  0xFFFFFFFF, as written.
- EN-D T / C differences done unsigned wrapping (only §7 r3 says so).
- EN-E §4 r1 order of last-checked := L vs the flag check.
- Day phase (`render/lighting.md`) and weather inputs are plain
  parameters until those specs exist.

## 4. Checks queued (`docs/HANDOFF.md` §5 C72–C75)

- C72 `cargo test -p d2-formats --test wav_game -- --ignored`: the 8 spec
  files (archive, channels, frames, sum, first 8, CRC) and every
  `sounds.txt` file parses playable: 4,508 / 4,434 mono / 74 stereo.
- C73 `cargo test -p d2-client --lib sound_table::tests::game -- --ignored`:
  4,699 records, song range 4,657–4,684, the spec's path / group / block
  rows, 4,508 resolve, 157 `none.wav`, 34 missing, 698 openers, 7 nested.
- C74 (player) `record_sound.py` hooks of `triggers.md` / `environment.md`
  "Checks": request log `(C, id, unit, delay, flags, offset)`, volume
  sets and roll order identical to the rule functions on the same inputs.
- C75 Dump of the DirectSound buffers (`wav.md` OQ 1–2): decoded samples
  equal to ours.

## 5. Base state found

`cargo test -p d2-client --lib` on the base `5844674` and on this branch:
47 failures, all outside audio (bridge, `ui::tests` ×2): the bridge
dispatch table has rows for S→C ids 122 and 129 without handlers
(`Mismatch([NoHandler { id: 122 }, NoHandler { id: 129 }])`). Not this
task's; left alone. Everything else green: d2-formats + d2-data 548 pass,
0 fail, 65 ignored; d2-client 511 pass (132 audio), 8 ignored; clippy
`-D warnings`, fmt, `coverage.py --check` (5,594 claims, 0 errors),
`spec_index.py --check`.

Environment note: d2-client needs `pkg-config libasound2-dev libudev-dev
libwayland-dev libxkbcommon-dev` (as CI installs) to build in a cloud
container.
