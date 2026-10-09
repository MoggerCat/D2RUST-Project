"""audio-diff: 1.14d's audio against d2rs', per voice and mixed
(specs/tools/audio-diff.md).

    python3 tools/audio-diff/audio_diff.py run traces/audio/<name>.check
        [--work DIR] [--reuse] [--orig-only | --d2rs-only] [--json FILE]
    python3 tools/audio-diff/audio_diff.py voices CAPTURE.jsonl BLOBS OUT_DIR
    python3 tools/audio-diff/audio_diff.py compare ORIG_VOICES D2RS_DUMP
        [--orig-mix FILE] [--json FILE]
    python3 tools/audio-diff/audio_diff.py --selftest

`run` takes a scenario-diff check file (specs/tools/scenario-diff.md §2,
`channels audio`): 1.14d under Wine with record_audio.py (capture.sh),
d2rs `play --audio-dump`, then `voices`, `d2-client audio-mix` on the
1.14d voices, and `compare`. Exit code: 0 match, 1 diverged, 3 error.
Standard library only. Our own code.
"""

import argparse
import hashlib
import json
import math
import os
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
VOICES_FORMAT = "audio-voices"
SUMMARY_FORMAT = "audio-diff-summary-1"
RATE = 22050


# --- device curves (§2 rule 6) -------------------------------------------------

def device_db(x, full):
    """trunc(-2000 log10(full / x)) hundredths of a dB (sound-table.md §8.3)."""
    if x <= 0.0001:
        return -10000
    if x >= full - 0.0001:
        return 0
    return int(-2000.0 * math.log10(full / x))


VOL_OF_DB = {}
for _v in range(256):
    VOL_OF_DB.setdefault(device_db(_v, 255.0), _v)
PAN_OF_DB = {0: 128}
for _p in range(128):
    PAN_OF_DB.setdefault(device_db(_p, 127.0), _p)            # right attenuated: pan < 128
    PAN_OF_DB.setdefault(-device_db(_p, 127.0), 255 - _p)     # left attenuated: pan > 128


def vol_of(db):
    """The integer v2 whose device value is db (None: not a curve value)."""
    return VOL_OF_DB.get(db)


def pan_of(db):
    return PAN_OF_DB.get(db)


# --- 1.14d voices from a capture (§2 rule 5) ------------------------------------

def load_jsonl(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def build_voices(recs, read_blob):
    """Voices of an audio-raw-1 capture. A voice starts at a SetCurrentPosition(0)
    on a channel buffer once a write or a Play followed the previous start; its
    samples are the writes until the next start, in order. Returns (voices,
    problems)."""
    bufs = {}
    for r in recs:
        if r["k"] == "create" and "b" in r and r.get("fmt"):
            bufs[r["b"]] = {"ch": r["fmt"]["ch"], "bytes": r["bytes"], "rate": r["fmt"]["rate"]}
    voices, cur, problems = [], {}, []
    dev = {b: {"vol": 0, "pan": 0} for b in bufs}   # a buffer keeps its volume and pan (DS default 0)
    for r in recs:
        b = r.get("b")
        if r["k"] not in ("setpos", "write", "play", "volume", "pan", "stop") or b not in bufs:
            continue
        if r["k"] in ("volume", "pan"):
            dev[b]["vol" if r["k"] == "volume" else "pan"] = r["v"]
        v = cur.get(b)
        if r["k"] == "setpos" and r["v"] == 0 and (v is None or v["used"]):
            if r.get("f") is None:      # front end: not part of the game
                cur.pop(b, None)
                continue
            v = {"b": b, "channels": bufs[b]["ch"], "f": r["f"], "T": r["T"], "C": r["C"],
                 "used": False, "played": None, "vol": None, "pan": None, "params": [],
                 "stop": None, "chunks": [], "gaps": 0}
            cur[b] = v
            voices.append(v)
            continue
        if v is None:
            continue
        if r["k"] == "write":
            v["used"] = True
            v["chunks"].append((r["off"], r["len"], r["sha256"]))
        elif r["k"] == "play":
            v["used"] = True
            if v["played"] is None:
                v["played"] = r["T"]
                v["vol"], v["pan"] = dev[b]["vol"], dev[b]["pan"]
        elif r["k"] in ("volume", "pan"):
            key = "vol" if r["k"] == "volume" else "pan"
            if v["played"] is not None and v["stop"] is None:
                v["params"].append((r["T"], key, r["v"]))
        elif r["k"] == "stop" and v["played"] is not None and v["stop"] is None:
            v["stop"] = r["T"]
    out = []
    for n, v in enumerate(voices):
        v["chunks"], v["gaps"] = ring_order(v["chunks"], bufs[v["b"]]["bytes"])
        if not v["chunks"]:
            problems.append(f"voice {n} (buffer {v['b']}, T {v['T']}): no samples written")
            continue
        if v["gaps"]:
            problems.append(f"voice {n} (buffer {v['b']}, T {v['T']}): {v['gaps']} write gap(s)")
        pcm = b"".join(read_blob(h) for h in v["chunks"])
        if v["played"] is None:
            v["vol"], v["pan"] = dev[v["b"]]["vol"], dev[v["b"]]["pan"]
        vol, pan = vol_of(v["vol"]), pan_of(v["pan"])
        if vol is None or pan is None:
            problems.append(f"voice {n}: device volume {v['vol']} / pan {v['pan']} not on the curve")
        params, state = [], [vol, pan]
        for tick, key, db in v["params"]:
            val = vol_of(db) if key == "vol" else pan_of(db)
            if val is None:
                problems.append(f"voice {n}: {key} {db} at T {tick} not on the curve")
                continue
            state[0 if key == "vol" else 1] = val
            if params and params[-1][0] == tick:
                params[-1] = [tick, state[0], state[1]]
            else:
                params.append([tick, state[0], state[1]])
        out.append({"n": n, "buffer": v["b"], "tick": v["T"], "f": v["f"], "C": v["C"],
                    "play": v["played"], "channels": v["channels"], "dev_vol": v["vol"],
                    "dev_pan": v["pan"], "vol": vol, "pan": pan, "params": params,
                    "stop": v["stop"], "pcm": pcm})
    return out, problems


def ring_order(chunks, size):
    """The writes of one voice in ring order (§2 r5): from the first, each next
    write is the earliest recorded one starting where the last ended (two
    threads can unlock regions of one buffer out of order); a write found
    nowhere in that chain is taken next in record order and counted as a gap.
    Returns (digests in order, gaps)."""
    left = list(chunks)
    out, gaps, want = [], 0, None
    while left:
        i = next((k for k, c in enumerate(left) if want is not None and c[0] == want), None)
        if i is None:
            i = 0
            if want is not None:
                gaps += 1
        off, n, h = left.pop(i)
        out.append(h)
        want = None if off is None else (off + n) % size
    return out, gaps


def write_voices(voices, out_dir):
    """OUT_DIR/orig.voices.jsonl (audio-voices-1) and OUT_DIR/pcm/<n>.pcm (the
    game's samples: never commit)."""
    os.makedirs(os.path.join(out_dir, "pcm"), exist_ok=True)
    path = os.path.join(out_dir, "orig.voices.jsonl")
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps({"format": VOICES_FORMAT, "version": 1}) + "\n")
        for v in voices:
            name = os.path.join("pcm", f"{v['n']}.pcm")
            with open(os.path.join(out_dir, name), "wb") as p:
                p.write(v["pcm"])
            rec = {k: v[k] for k in ("tick", "channels", "vol", "pan", "params", "stop", "buffer",
                                     "f", "C", "play", "dev_vol", "dev_pan")}
            rec.update(label=f"orig#{v['n']}", pcm=name.replace(os.sep, "/"), bytes=len(v["pcm"]),
                       sha256=hashlib.sha256(v["pcm"]).hexdigest())
            f.write(json.dumps(rec) + "\n")
    return path


# --- comparison (§4) --------------------------------------------------------------

def stream_matches(orig, pcm, looped, loop_start, channels):
    """(equal, note) of a 1.14d voice stream against a file's decoded samples
    (§1 r1): the overlap equal byte for byte (False and the first differing
    sample otherwise), then the tail after the samples: zeros, or the loop
    region again. A capture shorter than the file is `partial`."""
    n = len(pcm)
    k = min(n, len(orig))
    if orig[:k] != pcm[:k]:
        i = next(j for j in range(k) if orig[j] != pcm[j])
        return False, f"samples differ from sample {i // 2} (frame {i // 2 // channels})"
    if len(orig) < n:
        return True, f"partial: the capture holds {len(orig)} of {n} bytes (equal so far)"
    tail = orig[n:]
    if not looped:
        if tail.strip(b"\0"):
            j = len(tail) - len(tail.lstrip(b"\0"))
            return True, f"non-zero bytes after the samples (first at +{j})"
        return True, None
    region = pcm[loop_start * channels * 2:]
    if region and tail:
        want = (region * (len(tail) // len(region) + 1))[:len(tail)]
        if tail != want:
            j = next(i for i in range(len(tail)) if tail[i] != want[i])
            return True, f"loop wrap differs at +{j} after the samples"
    return True, None


def family(file):
    """A sound file's variant family: its name without trailing digits
    (`MedDirt3.wav` -> `meddirt`), for telling a variant pick apart."""
    base = file.replace("/", "\\").rsplit("\\", 1)[-1].lower()
    stem = base[:-4] if base.endswith(".wav") else base
    return stem.rstrip("0123456789 ")


def name_voices(orig_voices, sounds):
    """Each 1.14d voice's file: the first sound whose samples its stream
    starts with (§4 r2; the overlap byte for byte, at least 64 bytes or the
    whole file). sounds: [(file, channels, pcm, looped, loop_start)]."""
    for v in orig_voices:
        v["file"] = None
        for file, ch, pcm, looped, ls in sounds:
            k = min(len(pcm), len(v["pcm"]))
            if ch == v["channels"] and (k >= 64 or k == len(pcm)) and v["pcm"][:k] == pcm[:k]:
                v["file"] = file
                break


def wav_data(b):
    """The `data` bytes of a WAV (formats/wav.md §2 walk, no pad bytes), or None."""
    if len(b) < 12 or b[:4] != b"RIFF" or b[8:12] != b"WAVE":
        return None
    i, rem = 12, len(b) - 12
    while rem >= 8:
        cid, n = b[i:i + 4], int.from_bytes(b[i + 4:i + 8], "little")
        rem -= 8
        if cid == b"data":
            return b[i + 8:i + 8 + min(n, rem)]
        if rem < n:
            return None
        rem -= n
        i += 8 + n
    return None


def sound_index(d):
    """[(name, channels, data)] of every .wav under d (files extracted from the
    user's archives into a work folder, `mpq-tool extract-names`), for naming
    1.14d voices d2rs never plays (§4 r2)."""
    out = []
    for root, _, files in os.walk(d):
        for f in sorted(files):
            if not f.lower().endswith(".wav"):
                continue
            path = os.path.join(root, f)
            with open(path, "rb") as fh:
                b = fh.read()
            data = wav_data(b)
            if data is None or len(b) < 24:
                continue
            ch = int.from_bytes(b[22:24], "little") if b[12:16] == b"fmt " else 1
            rel = os.path.relpath(path, d).replace(os.sep, "\\")
            out.append((rel, ch, data, False, 0))
    return out


def compare(orig_voices, d2rs_recs, orig_mix=None, pcm_of=None, extra_sounds=()):
    """The summary (§4 r4). pcm_of(sha256) gives d2rs' decoded samples (the
    dump's `.pcm` folder); without it the samples compare by digest only.
    extra_sounds (sound_index) name 1.14d voices no d2rs sound matches."""
    d_starts = [r for r in d2rs_recs if r.get("k") == "start"]
    d_failed = [r for r in d2rs_recs if r.get("k") == "start-failed"]

    def pcm(d):
        return pcm_of(d["sha256"]) if pcm_of else None

    def same(v, d):
        p = pcm(d)
        if p is not None:
            return stream_matches(v["pcm"], p, d["looped"], d["loop_start"], d["channels"])
        n = d["frames"] * d["channels"] * 2
        if len(v["pcm"]) < n:
            return None, f"partial: the capture holds {len(v['pcm'])} of {n} bytes (no d2rs samples to compare)"
        return hashlib.sha256(v["pcm"][:n]).hexdigest() == d["sha256"], None

    sounds, seen = [], set()
    for d in d_starts:
        p = pcm(d)
        if p is not None and d["sha256"] not in seen:
            seen.add(d["sha256"])
            sounds.append((d["file"], d["channels"], p, d["looped"], d["loop_start"]))
    name_voices(orig_voices, sounds)
    if extra_sounds:
        rest = [v for v in orig_voices if not v["file"]]
        name_voices(rest, list(extra_sounds))
        for v in rest:
            if v["file"]:
                v["file"] = "(not played by d2rs) " + v["file"]
    diffs, matched, used = [], 0, set()

    def free(cond):
        return [v for v in orig_voices if v["n"] not in used and cond(v)]

    partial = []

    def check_pair(v, d, note):
        if note and note.startswith("partial"):
            partial.append(f"{d['file']} at T {d['tick']}: {note}")
        elif note:
            diffs.append({"tick": d["tick"], "what": "samples",
                          "text": f"{d['file']} at T {d['tick']}: {note}"})
        for key in ("dev_vol", "dev_pan"):
            if v[key] != d[key]:
                diffs.append({"tick": d["tick"], "what": key,
                              "text": f"{d['file']} at T {d['tick']}: {key} 1.14d {v[key]}, d2rs {d[key]}"})

    for d in d_starts:
        at = free(lambda v: v["tick"] == d["tick"] and v["channels"] == d["channels"])
        hit = next(((v, r) for v in at for r in [same(v, d)] if r[0] is not False), None)
        if hit:
            v, (eq, note) = hit
            used.add(v["n"])
            matched += 1
            check_pair(v, d, note)
            continue
        fam = [v for v in at if v["file"] and family(v["file"]) == family(d["file"])]
        if fam:
            v = fam[0]
            used.add(v["n"])
            diffs.append({"tick": d["tick"], "what": "variant",
                          "text": f"T {d['tick']}: 1.14d plays {v['file']}, d2rs {d['file']} "
                                  f"(variant pick, client/audio.md §A3 seeded choices)"})
            continue
        named = free(lambda v: v["file"] == d["file"])
        if named:
            v = min(named, key=lambda v: abs(v["tick"] - d["tick"]))
            used.add(v["n"])
            diffs.append({"tick": min(v["tick"], d["tick"]), "what": "tick",
                          "text": f"{d['file']}: 1.14d starts it at T {v['tick']}, d2rs at T {d['tick']}"})
            check_pair(v, d, same(v, d)[1])
            continue
        unnamed = [v for v in at if not v["file"]]
        if len(unnamed) == 1 and pcm(d) is not None:
            v = unnamed[0]
            used.add(v["n"])
            diffs.append({"tick": d["tick"], "what": "samples",
                          "text": f"{d['file']} at T {d['tick']}: "
                                  + stream_matches(v["pcm"], pcm(d), d["looped"], d["loop_start"],
                                                   d["channels"])[1]})
            continue
        diffs.append({"tick": d["tick"], "what": "voice",
                      "text": f"d2rs starts {d['file']} at T {d['tick']}; 1.14d starts no voice with "
                              f"these samples"})
    for v in orig_voices:
        if v["n"] not in used:
            nz = len(v["pcm"].rstrip(b"\0"))
            what = v["file"] or (f"an unknown sound ({v['channels']} ch, {nz} bytes before the zero "
                                 f"tail, sha256 {hashlib.sha256(v['pcm'][:nz]).hexdigest()[:16]})")
            diffs.append({"tick": v["tick"], "what": "voice",
                          "text": f"1.14d starts {what} at T {v['tick']} (buffer {v['buffer']}); "
                                  f"d2rs starts nothing like it"})
    for r in d_failed:
        diffs.append({"tick": r["tick"], "what": "voice",
                      "text": f"d2rs fails to start {r['file']} at T {r['tick']}"})
    diffs.sort(key=lambda x: (x["tick"], x["what"]))
    out = {"format": SUMMARY_FORMAT, "voices": {"orig": len(orig_voices), "d2rs": len(d_starts),
                                                "matched": matched, "differences": len(diffs),
                                                "first": diffs[0] if diffs else None,
                                                "by_kind": {}, "partial": partial},
           "mixed": None}
    for d in diffs:
        out["voices"]["by_kind"][d["what"]] = out["voices"]["by_kind"].get(d["what"], 0) + 1
    if orig_mix is not None:
        om = {r["tick"]: r for r in orig_mix if r.get("k") == "mix"}
        dm = {r["tick"]: r for r in d2rs_recs if r.get("k") == "mix"}
        ticks = sorted(set(om) & set(dm))
        first = next((t for t in ticks if om[t]["sha256"] != dm[t]["sha256"]), None)
        out["mixed"] = {"ticks": len(ticks), "equal": sum(om[t]["sha256"] == dm[t]["sha256"]
                                                          for t in ticks), "first_tick": first}
    mix_ok = out["mixed"] is None or out["mixed"]["first_tick"] is None
    out["verdict"] = "MATCH" if not diffs and mix_ok and d_starts else (
        "EMPTY" if not d_starts and not orig_voices else "DIVERGED")
    return out, diffs


def print_summary(s, diffs, limit=20):
    v = s["voices"]
    print(f"[audio] voices: 1.14d {v['orig']}, d2rs {v['d2rs']}, paired {v['matched']}, "
          f"{v['differences']} difference(s) {v['by_kind']}")
    for d in diffs[:limit]:
        print(f"  T {d['tick']}: {d['what']}: {d['text']}")
    if len(diffs) > limit:
        print(f"  ... {len(diffs) - limit} more")
    if s["mixed"]:
        m = s["mixed"]
        print(f"[audio] mixed: {m['equal']}/{m['ticks']} ticks equal"
              + (f", first difference at T {m['first_tick']}" if m["first_tick"] is not None else ""))
    print(f"[audio] {s['verdict']}")


# --- run a check (§5) ---------------------------------------------------------------

def run_check(a):
    sys.path.insert(0, os.path.join(REPO, "tools", "scenario-diff"))
    import scenario_diff as sd  # noqa: E402
    sd.CHANNELS = sd.CHANNELS + ("audio",)
    with open(a.check, encoding="utf-8") as f:
        c = sd.parse(f.read())
    if c["channels"] != ["audio"]:
        raise SystemExit(f"{a.check}: an audio check has `channels audio` only")
    work = a.work or os.path.join(REPO, "target", "audio-diff", c["name"])
    os.makedirs(work, exist_ok=True)
    r = sd.Runner(c, work, reuse=a.reuse)
    r.use_variant()
    save = r.build_save()
    cap = r.path("orig")
    sides = {"orig", "d2rs"} - ({"d2rs"} if a.orig_only else set()) - ({"orig"} if a.d2rs_only else set())
    if "orig" in sides and not (a.reuse and os.path.exists(os.path.join(cap, "audio.jsonl"))):
        game = os.path.join(r.game_dir, "Game.exe")
        args = ["--game", game, "--ticks", str(c["ticks"]), "--auto", c["char"], "--seed",
                str(c["seed"])] + r.poke_args() + r.send_args()
        if r.orig_input():
            args += ["--input", r.orig_input()]
        r.sh([os.path.join(HERE, "capture.sh"), cap, str(c["seconds"]), "--"] + args,
             timeout=c["seconds"] + 240, check=False)
    dump = r.path("d2rs.audio.jsonl")
    if "d2rs" in sides and not (a.reuse and os.path.exists(dump)):
        exe = r.d2_client_bin()
        args = [exe, "play"] + r.d2rs_common(save, play=True) + [
            "--audio-dump", dump, "--audio-ticks", str(c["ticks"])]
        if r.d2rs_input():
            args += ["--input", r.d2rs_input()]
        with r.display() as env:
            r.sh(args, timeout=r.draws_timeout + 30 * c["ticks"], check=False, env=env)
    if sides != {"orig", "d2rs"}:
        return 0
    recs = load_jsonl(os.path.join(cap, "audio.jsonl"))
    voices, problems = build_voices(recs, blob_reader(os.path.join(cap, "blobs")))
    vpath = write_voices(voices, work)
    last = max([v["tick"] for v in voices] + [0]) + 200
    d2rs = load_jsonl(dump)
    d_last = max([x["tick"] for x in d2rs if x.get("k") == "mix"] + [0])
    exe = r.d2_client_bin()
    mix_path = r.path("orig.mix.jsonl")
    r.sh([exe, "audio-mix", vpath, mix_path, "--last-tick", str(min(last, d_last) or last)],
         check=False)
    orig_mix = load_jsonl(mix_path) if os.path.exists(mix_path) else None
    s, diffs = compare(voices, d2rs, orig_mix, pcm_reader(dump + ".pcm"),
                       sound_index(a.sounds) if a.sounds else ())
    s.update(check=c["name"], capture_problems=problems)
    for p in problems:
        print(f"[audio] capture: {p}")
    print_summary(s, diffs)
    out = a.json or r.path("audio.summary.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(dict(s, differences=diffs[:200]), f, indent=1)
    return 0 if s["verdict"] == "MATCH" else 1


def pcm_reader(d):
    """d2rs' decoded samples by digest (the dump's `.pcm` folder), or None."""
    if not os.path.isdir(d):
        return None

    def read(h):
        p = os.path.join(d, h + ".pcm")
        if not os.path.exists(p):
            return None
        with open(p, "rb") as f:
            return f.read()
    return read


def blob_reader(d):
    def read(h):
        with open(os.path.join(d, h + ".pcm"), "rb") as f:
            return f.read()
    return read


# --- selftest -----------------------------------------------------------------------

def selftest():
    import struct
    assert device_db(255, 255.0) == 0 and device_db(0, 255.0) == -10000
    assert vol_of(-128) is not None and vol_of(-127) is None
    assert pan_of(1377) > 128 and pan_of(-595) < 128 and pan_of(0) == 128
    a = struct.pack("<4h", 1, 2, 3, 4)
    blobs = {"A": a + b"\0" * 8, "B": b"\x05\x00" * 8}
    recs = [
        {"k": "create", "b": 1, "bytes": 16, "fmt": {"ch": 1, "rate": RATE}},
        {"k": "setpos", "b": 1, "v": 0, "f": None, "T": 0, "C": 0},    # front end: ignored
        {"k": "setpos", "b": 1, "v": 0, "f": 3, "T": 1, "C": 2},
        {"k": "pan", "b": 1, "v": 0, "f": 3, "T": 1, "C": 2},
        {"k": "volume", "b": 1, "v": 0, "f": 3, "T": 1, "C": 2},
        {"k": "write", "b": 1, "off": 0, "len": 16, "sha256": "A", "f": 3, "T": 1, "C": 2},
        {"k": "pan", "b": 1, "v": 1377, "f": 3, "T": 1, "C": 2},
        {"k": "volume", "b": 1, "v": -128, "f": 3, "T": 1, "C": 2},
        {"k": "play", "b": 1, "flags": 1, "f": 3, "T": 1, "C": 2},
        {"k": "volume", "b": 1, "v": -885, "f": 5, "T": 3, "C": 4},
        {"k": "stop", "b": 1, "f": 6, "T": 4, "C": 5},
        {"k": "setpos", "b": 1, "v": 0, "f": 9, "T": 7, "C": 8},
        {"k": "write", "b": 1, "off": 0, "len": 16, "sha256": "B", "f": 9, "T": 7, "C": 8},
        {"k": "play", "b": 1, "flags": 1, "f": 9, "T": 7, "C": 8},
    ]
    voices, problems = build_voices(recs, blobs.__getitem__)
    assert problems == [] and len(voices) == 2, (problems, voices)
    v = voices[0]
    assert (v["tick"], v["dev_vol"], v["dev_pan"], v["vol"], v["pan"]) == (1, -128, 1377, vol_of(-128),
                                                                            pan_of(1377)), v
    assert v["params"] == [[3, vol_of(-885), pan_of(1377)]] and v["stop"] == 4, v
    sha = hashlib.sha256(a).hexdigest()
    d = [{"k": "start", "tick": 1, "file": "x.wav", "channels": 1, "frames": 4, "sha256": sha,
          "looped": False, "loop_start": 0, "dev_vol": -128, "dev_pan": 1377},
         {"k": "start", "tick": 7, "file": "y.wav", "channels": 1, "frames": 8,
          "sha256": hashlib.sha256(blobs["B"]).hexdigest(), "looped": True, "loop_start": 0,
          "dev_vol": -885, "dev_pan": 1377}]   # the buffer keeps the last voice's values
    s, diffs = compare(voices, d)
    assert s["verdict"] == "MATCH" and s["voices"]["matched"] == 2, (s, diffs)
    d[0]["dev_vol"] = -129
    s, diffs = compare(voices, d)
    assert s["verdict"] == "DIVERGED" and diffs[0]["what"] == "dev_vol", diffs
    store = {d[0]["sha256"]: a, d[1]["sha256"]: blobs["B"]}
    pcm_of = store.get
    d[0]["dev_vol"], d[0]["tick"] = -128, 2
    s, diffs = compare(voices, d, pcm_of=pcm_of)
    assert [x["what"] for x in diffs] == ["tick"], diffs
    assert diffs[0]["text"] == "x.wav: 1.14d starts it at T 1, d2rs at T 2", diffs
    d[0]["tick"] = 1
    om = [{"k": "mix", "tick": 1, "sha256": "x"}, {"k": "mix", "tick": 2, "sha256": "y"}]
    s, _ = compare(voices, d + [{"k": "mix", "tick": 1, "sha256": "x"},
                                {"k": "mix", "tick": 2, "sha256": "z"}], om, pcm_of)
    assert s["mixed"] == {"ticks": 2, "equal": 1, "first_tick": 2} and s["verdict"] == "DIVERGED", s
    # a variant pick: 1.14d plays step1 (known to the run), d2rs step2 at the same tick
    s1, s2 = struct.pack("<2h", 7, 7), struct.pack("<2h", 9, 9)
    v1 = [{"n": 0, "tick": 3, "channels": 1, "buffer": 2, "pcm": s1 + b"\0" * 4, "dev_vol": 0,
           "dev_pan": 0}]
    dd = [{"k": "start", "tick": 3, "file": "fx\\step2.wav", "channels": 1, "frames": 2,
           "sha256": "s2", "looped": False, "loop_start": 0, "dev_vol": 0, "dev_pan": 0},
          {"k": "start", "tick": 9, "file": "fx\\step1.wav", "channels": 1, "frames": 2,
           "sha256": "s1", "looped": False, "loop_start": 0, "dev_vol": 0, "dev_pan": 0}]
    s, diffs = compare(v1, dd, pcm_of={"s1": s1, "s2": s2}.get)
    assert [x["what"] for x in diffs] == ["variant", "voice"], diffs
    assert "1.14d plays fx\\step1.wav, d2rs fx\\step2.wav" in diffs[0]["text"], diffs
    # a decode difference: one unnamed voice at the tick
    s, diffs = compare(v1, [dict(dd[0], sha256="s3")], pcm_of={"s3": struct.pack("<2h", 7, 8)}.get)
    assert diffs[0]["what"] == "samples" and "sample 1" in diffs[0]["text"], diffs
    # a capture shorter than the file: compared as far as it goes
    eq, note = stream_matches(a[:4], a, False, 0, 1)
    assert eq and note.startswith("partial"), note
    assert stream_matches(a[:2] + b"\x09\0", a, False, 0, 1)[0] is False
    # a non-looped voice with sound after its samples; a loop wrap
    eq, note = stream_matches(a + b"\1\0", a, False, 0, 1)
    assert eq and "non-zero" in note
    assert stream_matches(a + a[2:6], a, True, 1, 1) == (True, None)
    w = b"RIFF\0\0\0\0WAVEfmt " + (16).to_bytes(4, "little") + bytes(16) + b"data" + \
        (4).to_bytes(4, "little") + a[:4]
    assert wav_data(w) == a[:4] and wav_data(b"RIFX") is None
    assert family("DATA\\GLOBAL\\SFX\\ambient\\footstep\\MedDirt4.wav") == "meddirt"
    # a gap in the ring writes
    recs2 = recs[:6] + [{"k": "write", "b": 1, "off": 8, "len": 8, "sha256": "A", "f": 3, "T": 1, "C": 2}]
    _, problems = build_voices(recs2, blobs.__getitem__)
    assert problems and "gap" in problems[0], problems
    assert ring_order([(0, 4, "a"), (8, 4, "c"), (4, 4, "b"), (12, 4, "d")], 16) == (
        ["a", "b", "c", "d"], 0)
    assert ring_order([(0, 4, "a"), (8, 4, "c")], 16) == (["a", "c"], 1)
    print("audio_diff selftest: OK")


def main():
    if "--selftest" in sys.argv[1:]:
        selftest()
        return 0
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("run")
    p.add_argument("check")
    p.add_argument("--work")
    p.add_argument("--reuse", action="store_true")
    g = p.add_mutually_exclusive_group()
    g.add_argument("--orig-only", action="store_true")
    g.add_argument("--d2rs-only", action="store_true")
    p.add_argument("--json")
    p.add_argument("--sounds", help="folder of .wav files extracted from the archives (naming)")
    p = sub.add_parser("voices")
    p.add_argument("capture")
    p.add_argument("blobs")
    p.add_argument("out_dir")
    p = sub.add_parser("compare")
    p.add_argument("orig_voices")
    p.add_argument("d2rs_dump")
    p.add_argument("--orig-mix")
    p.add_argument("--json")
    p.add_argument("--sounds", help="folder of .wav files extracted from the archives (naming)")
    a = ap.parse_args()
    if a.cmd == "run":
        return run_check(a)
    if a.cmd == "voices":
        voices, problems = build_voices(load_jsonl(a.capture), blob_reader(a.blobs))
        path = write_voices(voices, a.out_dir)
        for p in problems:
            print("capture:", p)
        print(f"{len(voices)} voices -> {path}")
        return 0
    base = os.path.dirname(os.path.abspath(a.orig_voices))
    voices = []
    for i, r in enumerate(load_jsonl(a.orig_voices)):
        if i == 0:
            continue
        with open(os.path.join(base, r["pcm"]), "rb") as f:
            r["pcm"] = f.read()
        r["n"] = i - 1
        voices.append(r)
    s, diffs = compare(voices, load_jsonl(a.d2rs_dump),
                       load_jsonl(a.orig_mix) if a.orig_mix else None,
                       pcm_reader(a.d2rs_dump + ".pcm"),
                       sound_index(a.sounds) if a.sounds else ())
    print_summary(s, diffs)
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(dict(s, differences=diffs[:200]), f, indent=1)
    return 0 if s["verdict"] == "MATCH" else 1


if __name__ == "__main__":
    sys.exit(main())
