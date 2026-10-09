#!/bin/sh
# Capture 1.14d's audio under Wine (specs/tools/audio-diff.md §2): writes an
# ALSA configuration whose default device is the null sink (with
# AUDIO_DIFF_KEEP_MIX=1 a file sink: Wine's own mix, float32 after its
# resampler, ~150 MB per minute, for listening only, never compared), then
# runs record_audio.py through tools/cloud-game/run.sh --python.
#   tools/audio-diff/capture.sh OUT_DIR SECONDS -- RECORD_AUDIO_ARGS...
# Example (town arrival, 300 ticks):
#   tools/audio-diff/capture.sh /tmp/a 150 -- --auto ScnAma --seed 1234 --ticks 300
# Outputs: OUT_DIR/audio.jsonl, OUT_DIR/blobs/ (game samples: never commit),
# OUT_DIR/wine-mix.raw (AUDIO_DIFF_KEEP_MIX=1), run.sh's files in OUT_DIR/run/.
# Our own code.
set -eu
[ $# -ge 3 ] && [ "$3" = "--" ] || { echo "usage: $0 OUT_DIR SECONDS -- ARGS..." >&2; exit 2; }
out=$(mkdir -p "$1" && cd "$1" && pwd); secs=$2; shift 3
here=$(cd "$(dirname "$0")" && pwd); repo=$(cd "$here/../.." && pwd)
rc="$HOME/.asoundrc"
if [ -f "$rc" ] && ! grep -q "audio-diff capture" "$rc"; then
  echo "capture.sh: $rc exists and is not ours; move it away first" >&2; exit 2
fi
if [ "${AUDIO_DIFF_KEEP_MIX:-0}" = 1 ]; then
  sink="pcm.audiodiffcap { type file; slave.pcm \"null\"; file \"$out/wine-mix.raw\"; format \"raw\" }"
else
  sink="pcm.audiodiffcap { type null }"
fi
cat > "$rc" <<EOF
# audio-diff capture (tools/audio-diff/capture.sh): default device = null or file sink
pcm.!default { type plug; slave.pcm "audiodiffcap" }
$sink
EOF
"$repo/tools/cloud-game/run.sh" --python --seconds "$((secs + 30))" --out "$out/run" -- \
  "$here/record_audio.py" --seconds "$secs" --out "$out/audio.jsonl" --blob-dir "$out/blobs" "$@"
