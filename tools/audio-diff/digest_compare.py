#!/usr/bin/env python3
"""digest_compare: a 1.14d voice list recorded on Windows (digests only, no
sample blobs) against a d2rs audio dump (specs/tools/audio-diff.md §4).

    python3 tools/audio-diff/digest_compare.py ORIG.voices.jsonl D2RS.audio.jsonl SFX_DIR

A 1.14d voice is named by the sound file whose `data` chunk, zero-padded to
the buffer size, has the voice's digest (SFX_DIR: the sound files extracted
from the user's archives into a work folder; never committed). Each d2rs
start pairs with the 1.14d voice of the same tick and file; volume and pan
compare too. Streamed loops (music, ambience, rain) show as unknown: their capture holds the
stream buffer at the last write, not a file prefix. Exit 0 only when every voice pairs with equal volume and pan.
Our own code.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from audio_diff import wav_data  # noqa: E402


def index(sfx):
    out = {}
    for root, _, files in os.walk(sfx):
        for f in files:
            if f.lower().endswith(".wav"):
                with open(os.path.join(root, f), "rb") as fh:
                    d = wav_data(fh.read())
                if d:
                    out.setdefault(len(d), []).append((os.path.join(root, f), d))
    return out


def name_of(v, idx):
    n = v["bytes"]
    for size, lst in idx.items():
        for path, d in lst:
            # whole file zero-padded to the buffer, or the first n bytes of a longer file
            # (a capture holds a loop's first buffer only)
            part = d + b"\0" * (n - size) if size <= n else d[:n]
            if size > 0 and hashlib.sha256(part).hexdigest() == v["sha256"]:
                return os.path.basename(path).lower()
    return None


def main():
    orig_path, d2rs_path, sfx = sys.argv[1:4]
    idx = index(sfx)
    orig = [json.loads(l) for l in open(orig_path)][1:]
    for v in orig:
        v["file"] = name_of(v, idx)
    starts = [r for r in map(json.loads, open(d2rs_path)) if r.get("k") == "start"]
    used, diffs = set(), []
    for d in starts:
        base = d["file"].replace("/", "\\").rsplit("\\", 1)[-1].lower()
        hit = next((i for i, v in enumerate(orig) if i not in used and v["tick"] == d["tick"]
                    and v["file"] == base and v["vol"] == d["vol"] and v["pan"] == d["pan"]), None)
        if hit is None:
            diffs.append((d["tick"], f"d2rs starts {base} vol {d['vol']} pan {d['pan']}; 1.14d has no such voice"))
        else:
            used.add(hit)
    for i, v in enumerate(orig):
        if i not in used:
            diffs.append((v["tick"], f"1.14d starts {v['file'] or '(unknown sha ' + v['sha256'][:8] + ')'} "
                                     f"vol {v['vol']} pan {v['pan']}; d2rs has no such voice"))
    diffs.sort()
    print(f"[digest] 1.14d {len(orig)} voices, d2rs {len(starts)}, paired {len(used)}, {len(diffs)} difference(s)")
    for t, m in diffs[:40]:
        print(f"  T {t}: {m}")
    sys.exit(1 if diffs else 0)


if __name__ == "__main__":
    main()
