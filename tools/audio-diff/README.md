# audio-diff: 1.14d's sound output against d2rs', per voice and mixed

Spec: `specs/tools/audio-diff.md`. Our own code.

| File | Does |
|---|---|
| `record_audio.py` | Windows Python debugger (on `trace-recorder/record_tick.py`): hooks 1.14d's DirectSound buffers through the static `DSOUND.dll` import and logs every buffer write (bytes as blobs), `Play`, `Stop`, `SetVolume`, `SetPan`, `SetCurrentPosition`, and the sound requests, each with the server frame `f`, sound tick `T`, client update `C` (`audio-raw-1`); `--poke` / `--send` as the other recorders; `--selftest` |
| `capture.sh OUT SECONDS -- ARGS` | Cloud: an ALSA file sink as Wine's audio device, then `record_audio.py` through `tools/cloud-game/run.sh --python` |
| `audio_diff.py` | `run CHECK` (both sides, then compare), `voices` (1.14d capture → voice list), `compare` (per voice and mixed; `--sounds DIR` names voices d2rs never plays); `--selftest` |

d2rs side: `d2-client play … --audio-dump FILE --audio-ticks N` (no audio
device; voices, sample digests, per-tick mix digests; decoded samples in
`FILE.pcm/`) and `d2-client audio-mix VOICES OUT --last-tick N` (a voice
list through d2rs' mixer).

```sh
export D2_GAME_DIR=$HOME/game
python3 tools/audio-diff/audio_diff.py run traces/audio/audio-town-ambience-ama.check
# naming every 1.14d voice: extract the sound files named by sounds.txt into a work folder
python3 tools/audio-diff/audio_diff.py compare target/audio-diff/<check>/orig.voices.jsonl \
    target/audio-diff/<check>/d2rs.audio.jsonl --orig-mix target/audio-diff/<check>/orig.mix.jsonl \
    --sounds /path/to/extracted-sfx
```

Captured blobs, `pcm/` and `FILE.pcm/` hold the game's samples: they stay
in `target/` (gitignored) and never go into the repository (CLAUDE.md
rule 1).
