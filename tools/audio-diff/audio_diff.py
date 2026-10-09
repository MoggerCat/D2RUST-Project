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

def expected_tail(n_bytes, frames, channels, looped, loop_start):
    """What the device buffer holds after the file's samples, as a function of
    the bytes index (§4 rule 2): zeros, or the loop region again."""
    return None if not looped else (loop_start * channels * 2, frames * channels * 2)


def voice_matches(orig_pcm, d):
    """(equal samples, tail note) for an orig voice stream against a d2rs start."""
    n = d["frames"] * d["channels"] * 2
    head = orig_pcm[:n]
    if len(head) < n:
        # the capture ended before the file did: compare what is there
        return None, f"capture holds {len(head)} of {n} bytes"
    if hashlib.sha256(head).hexdigest() != d["sha256"]:
        return False, None
    tail = orig_pcm[n:]
    if not d["looped"]:
        bad = len(tail) - len(tail.lstrip(b"\0")) if tail.strip(b"\0") else None
        if tail.strip(b"\0"):
            return True, f"non-zero bytes after the samples (first at +{bad})"
        return True, None
    lo = d["loop_start"] * d["channels"] * 2
    region = head[lo:]
    if region and tail:
        want = (region * (len(tail) // len(region) + 1))[:len(tail)]
        if tail != want:
            k = next(i for i in range(len(tail)) if tail[i] != want[i])
            return True, f"loop wrap differs at +{k} after the samples"
    return True, None


def compare(orig_voices, d2rs_recs, orig_mix=None):
    """The summary (§4 rule 4): voices paired by tick and samples, then per pair
    device volume and pan; mixed per tick when both mixes are given."""
    d_starts = [r for r in d2rs_recs if r.get("k") == "start"]
    d_failed = [r for r in d2rs_recs if r.get("k") == "start-failed"]
    by_tick = {}
    for v in orig_voices:
        by_tick.setdefault(v["tick"], []).append(v)
    diffs, matched, used = [], 0, set()
    for d in d_starts:
        cands = [v for v in by_tick.get(d["tick"], []) if v["n"] not in used
                 and v["channels"] == d["channels"]]
        hit = None
        for v in cands:
            eq, note = voice_matches(v["pcm"], d)
            if eq or eq is None:
                hit = (v, note, eq)
                break
        if hit is None:
            other = [v for v in orig_voices if v["n"] not in used
                     and voice_matches(v["pcm"], d)[0]]
            where = (f"; 1.14d starts these samples at T {other[0]['tick']}" if other else
                     "; 1.14d never starts these samples")
            diffs.append({"tick": d["tick"], "what": "voice",
                          "text": f"d2rs starts {d['file']} at T {d['tick']}{where}"})
            continue
        v, note, eq = hit
        used.add(v["n"])
        matched += 1
        if note:
            diffs.append({"tick": d["tick"], "what": "samples" if eq is not None else "partial",
                          "text": f"{d['file']} at T {d['tick']}: {note}"})
        for key in ("dev_vol", "dev_pan"):
            if v[key] != d[key]:
                diffs.append({"tick": d["tick"], "what": key,
                              "text": f"{d['file']} at T {d['tick']}: {key} 1.14d {v[key]}, d2rs {d[key]}"})
    for v in orig_voices:
        if v["n"] not in used:
            nz = len(v["pcm"].rstrip(b"\0"))
            diffs.append({"tick": v["tick"], "what": "voice",
                          "text": f"1.14d starts a voice at T {v['tick']} (buffer {v['buffer']}, "
                                  f"{v['channels']} ch, {nz} bytes before the zero tail, sha256 "
                                  f"{hashlib.sha256(v['pcm'][:nz]).hexdigest()[:16]}) that d2rs "
                                  f"does not start"})
    for r in d_failed:
        diffs.append({"tick": r["tick"], "what": "voice",
                      "text": f"d2rs fails to start {r['file']} at T {r['tick']}"})
    diffs.sort(key=lambda x: (x["tick"], x["what"]))
    out = {"format": SUMMARY_FORMAT, "voices": {"orig": len(orig_voices), "d2rs": len(d_starts),
                                                "matched": matched, "differences": len(diffs),
                                                "first": diffs[0] if diffs else None},
           "mixed": None}
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
          f"{v['differences']} difference(s)")
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
    s, diffs = compare(voices, d2rs, orig_mix)
    s.update(check=c["name"], capture_problems=problems)
    for p in problems:
        print(f"[audio] capture: {p}")
    print_summary(s, diffs)
    out = a.json or r.path("audio.summary.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(dict(s, differences=diffs[:200]), f, indent=1)
    return 0 if s["verdict"] == "MATCH" else 1


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
    d[0]["dev_vol"], d[0]["tick"] = -128, 2
    s, diffs = compare(voices, d)
    assert any(x["text"].endswith("1.14d starts these samples at T 1") for x in diffs), diffs
    assert any("that d2rs does not start" in x["text"] for x in diffs), diffs
    d[0]["tick"] = 1
    om = [{"k": "mix", "tick": 1, "sha256": "x"}, {"k": "mix", "tick": 2, "sha256": "y"}]
    s, _ = compare(voices, d + [{"k": "mix", "tick": 1, "sha256": "x"},
                                {"k": "mix", "tick": 2, "sha256": "z"}], om)
    assert s["mixed"] == {"ticks": 2, "equal": 1, "first_tick": 2} and s["verdict"] == "DIVERGED", s
    # a non-looped voice with sound after its samples
    eq, note = voice_matches(a + b"\1\0", d[0])
    assert eq and "non-zero" in note
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
    p = sub.add_parser("voices")
    p.add_argument("capture")
    p.add_argument("blobs")
    p.add_argument("out_dir")
    p = sub.add_parser("compare")
    p.add_argument("orig_voices")
    p.add_argument("d2rs_dump")
    p.add_argument("--orig-mix")
    p.add_argument("--json")
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
                       load_jsonl(a.orig_mix) if a.orig_mix else None)
    print_summary(s, diffs)
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(dict(s, differences=diffs[:200]), f, indent=1)
    return 0 if s["verdict"] == "MATCH" else 1


if __name__ == "__main__":
    sys.exit(main())
