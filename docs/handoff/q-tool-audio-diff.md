# q-tool-audio-diff — hand-back (2026-10-09)

Session q-tool-audio-diff (fidelity plan, `fidelity-gaps.md` §4 `audio-diff`).
Branch `claude/q-tool-audio-diff`. REC-1360..1363 used (1364..1369 free).

## Done

- **Feasibility (step 1): yes, in the cloud under Wine.** 1.14d runs with
  sound (no `-ns`) on Wine 9.0's `winealsa` with an ALSA file → null sink.
  The recorder follows the game's static `DSOUND.dll` import (IAT
  `0x006CC088` = DirectSoundCreate) to the IDirectSound and buffer
  vtables and logs every buffer write with its bytes, every
  Play/Stop/SetVolume/SetPan/SetCurrentPosition, and the sound requests,
  each with server frame, sound tick `T` and client update `C`. Hooks are
  race-free (INT3 kept, thread resumed in a trampoline: 262 Lock = 262
  Unlock); the town music voice equals `town1.wav`'s `data` byte for byte
  over 3.3 MB. No PC 1 capture is needed for the tool itself (REC-1363
  asks PC 1 for one Windows-vs-Wine comparison).
- **Tool** (`tools/audio-diff/`, spec `specs/tools/audio-diff.md`):
  `record_audio.py`, `capture.sh`, `audio_diff.py` (`run`, `voices`,
  `compare`, selftests); d2rs side `d2-client play --audio-dump FILE
  --audio-ticks N` (deterministic tick stepper, no device; voices with
  sample digests and device volume / pan, per-tick mix digests, decoded
  samples in `FILE.pcm/`) and `d2-client audio-mix` (a voice list through
  d2rs' mixer, for the mixed check). Small core additions:
  `AudioEngine::record_starts` / `take_started`, `Voice::sound`.
- **Measured facts:** 1.14d's 16 channel buffers are rings (4 × 256 KiB
  stereo + 12 × 128 KiB mono, `sound-table.md` §7 r7); the start sequence
  and the device curves (`trunc(−2000·log10(full/x))`, all 110 captured
  volume values; REC-1361).
- **Four checks** (`traces/audio/`), all run on both sides:

| Check | 1.14d voices | d2rs voices | differences | mixed equal ticks | first difference |
|---|---|---|---|---|---|
| audio-town-ambience-ama | 23 | 23 | 31 (tick 9, voice 6, variant 8, dev_vol 7, dev_pan 1) | 0/251 | T 0: town1.wav, wilderness day 2.wav, fire4.wav start at T 1 in d2rs |
| audio-walk-town-ama | 32 | 35 | 71 | 0/121 | same; then the player's first footstep T 8 (1.14d) vs T 89 (d2rs) |
| audio-monster-hit-ama | 35 | 26 | 57 | 0/201 | same; Fallen warcry1 T 29 (1.14d) vs warcry2 T 30 (d2rs); impact / get-hit sounds missing |
| audio-cast-frost-nova-sor | 13 | 7 | 14 | 0/101 | same; coldcast.wav T 18 / 58 and novaice.wav T 25 / 65 missing in d2rs |

- **Decoded samples equal** for every file both sides played (no
  `samples` difference): 16 files (town1 / wild music over the captured
  part, footsteps MedDirt1–4, rain2, wilderness day 2, fire4, Fallen
  warcry1 / warcry3 / roar6, one hand thrust01/03/05/06): settles
  `formats/wav.md` OQ1 for these files.
- Ledger part `docs/handoff/ledger/q-tool-audio-diff.tsv`: 10
  `system.audio.*` / `system.client.audio` rows set to DIVERGED with the
  first difference (validated with `ledger.py`, 0 format errors; the
  check names sit in the note because `ledger.py` only resolves
  `traces/checks/`).

## Routed (audio has no owner in `tools/coord/owners.tsv`: to the coordinator)

In the order to fix (the first one shifts every later comparison):

1. **Sound tick time base:** every d2rs voice of the game's first sound
   tick starts at T 1, 1.14d's at T 0 (`sound-table.md` §6.1: the tick runs
   with T, then T += 1). All four checks.
2. 1.14d plays `cursor\windowopen.wav` at T 0 (likely the one request
   with no unit at that tick, id 6 from `0x0049E58A`; not yet tied by a
   hook); d2rs does not. d2rs starts `object\riverloop.wav` (vol 0)
   at T 1; 1.14d never does.
3. Rain (`environment.md` §6): `rain2.wav` at T 4 in 1.14d, T 14–16 in d2rs.
4. Skill sounds (`triggers.md` §8): no `coldcast.wav` / `novaice.wav` in
   d2rs.
5. Level-entry lines (`environment.md` §4): no
   `<class>_act1_entry_wilderness.wav` at T 64 in d2rs; event cues (§7):
   no `birdie*.wav` at T 75.
6. Combat: impact (`combat\impact\sword2.wav`, `sword5.wav`) and player
   get-hit (`combat\player\amazon\hard6.wav`) sounds missing; weapon
   thrust sounds at other ticks; device volume / pan of paired voices
   differ (e.g. warcry3 T 41 pan 71 vs 13).
7. Player footsteps (`triggers.md` §5): first at T 8 in 1.14d, T 89 in d2rs.

Variant picks (footsteps, warcries) differ as `client/audio.md` §A3
expects without the seed input; they are reported as `variant`, not
hidden.

## Open

- REC-1360 (voice start tick = `T` at `SetCurrentPosition`), REC-1361
  (f64 device curve), REC-1362 (mixed check through d2rs' mixer: needs a
  decision-log entry), REC-1363 (Windows vs Wine: `pc1-data.md` Step 4
  item).
- Seeded variant choices: §A3's seed input (roll hook of
  `local-buddy-q-rec.md` entry 74) is not wired into audio-diff yet.
- The checks live in `traces/audio/` so the scenario-diff suite does not
  run them; an `audio` channel in `scenario_diff.py` (owner
  q-tool-state-diff) would let `suite.py` run them.

## Repro

```sh
# once: setup per docs/handoff/q-tool-audio-diff-task.md (game in $HOME/game, Wine, saves)
export D2_GAME_DIR=$HOME/game
cargo build --release -p d2-client -p d2s-tool
export D2RS_BIN_DIR=$PWD/target/release
python3 tools/audio-diff/audio_diff.py run traces/audio/audio-town-ambience-ama.check   # ~8 min
python3 tools/audio-diff/record_audio.py --selftest && python3 tools/audio-diff/audio_diff.py --selftest
cargo nextest run -p d2-client audio_dump
```
