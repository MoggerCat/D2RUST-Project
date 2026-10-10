#!/usr/bin/env python3
"""Census of the generated missile and state checks (q-run-missiles-states).

    python3 tools/check-gen/census.py run [--family missile|state] [--filter GLOB]
        [--batch N] [--workers N] [--no-fill-cache]
    python3 tools/check-gen/census.py post NAME...       # one work dir -> record (no suite run)
    python3 tools/check-gen/census.py report [--ledger F] [--part F] [--md F]

`run` feeds `tools/scenario-diff/suite.py --dir traces/checks/gen` in batches
(the 1.14d recordings land in traces/orig-cache), then reduces each work dir
to one small record `traces/raw/census/<name>.json` and deletes the work dir
(9 MB each). The record compares the two sides again from the check's pokes
on: state (the missile units alone, every unit, and the frames before the
poke), rng and packets, each with `--from` the poke frame, so the warp /
join differences of frames 2-4 (net.s2c.0xaa size, 0x07 map reveal) do not
hide the first difference the poke causes. `report` turns the records into
the ledger part docs/handoff/ledger/q-run-missiles-states.tsv.

Verdict of a row: DIVERGED@F (first frame, over the channels) when any
channel differs from the poke on; PARTIAL when all compared channels are
equal (the state channel never says MATCH: `own` and the headless client are
not compared); NO-CHECK-POSSIBLE when a poke failed on either side.
Python 3 stdlib only. Our own code.
"""
import argparse
import csv
import glob
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
GEN = os.path.join(ROOT, "traces", "checks", "gen")
WORK = os.path.join(ROOT, "traces", "raw", "suite")
OUT = os.path.join(ROOT, "traces", "raw", "census")
TR = os.path.join(ROOT, "tools", "trace-recorder")
STATE_PKTS = {0xA7: "set-state", 0xA8: "set-state-list", 0xA9: "end-state"}


def run_diff(tool, orig, d2rs, extra, tmp):
    """The comparator's diff-summary-1 for the given window; None when a file is missing."""
    if not (os.path.isfile(orig) and os.path.isfile(d2rs)):
        return None
    js = tmp + ".json"
    if os.path.exists(js):
        os.remove(js)
    p = subprocess.run([sys.executable, os.path.join(TR, tool), orig, d2rs, "--json", js] + extra,
                       capture_output=True, text=True, timeout=600)
    if not os.path.isfile(js):
        return {"verdict": "ERROR", "text": (p.stderr or p.stdout)[-200:]}
    d = json.load(open(js))
    first = d.get("first")
    return {"verdict": d["verdict"], "differences": d.get("differences", 0),
            "frame": first["frame"] if first else None,
            "text": first["text"][:240] if first else ""}


def pokes(path):
    out = []
    for line in open(path, encoding="utf-8"):
        if line.startswith('{"k":"poke"'):
            d = json.loads(line)
            out.append({"f": d["f"], "d": d["d"], "r": d["r"], "src": d.get("src", "")})
    return out


def snap_units(path):
    for line in open(path, encoding="utf-8"):
        if line.startswith('{"k":"snap"'):
            d = json.loads(line)
            yield d["f"], d["units"]


def missile_frames(path, cls, lo):
    """(frames with a missile unit of class cls, classes of every missile unit) from frame lo."""
    seen, classes = 0, set()
    for f, units in snap_units(path):
        if f < lo:
            continue
        for u in units:
            if u["ut"] == 3:
                classes.add(u["cl"])
                if u["cl"] == cls:
                    seen += 1
                    break
    return seen, sorted(classes)


def state_packets(path):
    """[(id, state, unit type, unit id)] of the S->C set/end state messages of a packets file."""
    out = []
    for line in open(path, encoding="utf-8"):
        if not line.startswith('{"type":"s2c"') and not line.startswith('{"type": "s2c"'):
            continue
        d = json.loads(line)
        b = bytes.fromhex(d["bytes"])
        if b and b[0] in STATE_PKTS and len(b) >= 7:
            st = b[6] if b[0] != 0xA8 else (b[7] if len(b) > 7 else -1)
            out.append((b[0], st, b[1], int.from_bytes(b[2:6], "little")))
    return out


def post(name):
    w = os.path.join(WORK, name)
    chk = open(os.path.join(GEN, name + ".check"), encoding="utf-8").read()
    fam = re.search(r"family (\w+)", chk).group(1)
    area = re.search(r"# Ledger area: (\S+)\.", chk).group(1)
    poke_re = r"at (\d+) poke (missile|state) "
    pf = [int(m.group(1)) for m in re.finditer(poke_re, chk)]
    f0 = min(pf)  # the first poke the row is about
    rec = {"name": name, "family": fam, "area": area, "poke_frame": f0}
    tmp = os.path.join(w, "census")
    o, d = (lambda s, e: os.path.join(w, f"{s}.{e}.jsonl"))("orig", "state"), \
           (lambda s, e: os.path.join(w, f"{s}.{e}.jsonl"))("d2rs", "state")
    rec["pokes_orig"] = pokes(o) if os.path.isfile(o) else []
    rec["pokes_d2rs"] = pokes(d) if os.path.isfile(d) else []
    ign = ["--ignore", "q", "--next", "0"]
    rec["state_types3"] = run_diff("state_diff.py", o, d, ign + ["--types", "3", "--from", str(f0)], tmp)
    rec["state_all"] = run_diff("state_diff.py", o, d, ign + ["--from", str(f0)], tmp)
    rec["state_before"] = run_diff("state_diff.py", o, d, ign + ["--to", str(f0 - 1)], tmp)
    po = os.path.join(w, "orig.rng.jsonl"), os.path.join(w, "d2rs.rng.jsonl")
    rec["rng"] = run_diff("rng_diff.py", po[0], po[1], ["--from", str(f0), "--next", "0"], tmp)
    pp = os.path.join(w, "orig.packets.jsonl"), os.path.join(w, "d2rs.packets.jsonl")
    rec["packets"] = run_diff("packets_diff.py", pp[0], pp[1], ["--from", str(f0), "--next", "0"], tmp)
    if fam == "missile":
        mid = int(name.rsplit("-", 1)[1])
        rec["missile"] = mid
        if os.path.isfile(o):
            rec["seen_orig"], rec["classes_orig"] = missile_frames(o, mid, f0)
        if os.path.isfile(d):
            rec["seen_d2rs"], rec["classes_d2rs"] = missile_frames(d, mid, f0)
    else:
        sid = int(name.rsplit("-", 1)[1])
        rec["state"] = sid
        for side, p in (("orig", pp[0]), ("d2rs", pp[1])):
            if os.path.isfile(p):
                rec["pkts_" + side] = [x for x in state_packets(p) if x[1] == sid]
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump(rec, f, sort_keys=True)
    return rec


def cmd_run(a):
    pat = a.filter or "*"
    names = sorted(os.path.basename(p)[:-6] for p in glob.glob(os.path.join(GEN, f"gen-{a.family}-*.check"))
                   if re.fullmatch(pat.replace("*", ".*"), os.path.basename(p)[:-6]))
    names = [n for n in names if a.force or not os.path.isfile(os.path.join(OUT, n + ".json"))]
    print(f"census: {len(names)} checks to run")
    for i in range(0, len(names), a.batch):
        chunk = names[i:i + a.batch]
        cmd = [sys.executable, os.path.join(ROOT, "tools", "scenario-diff", "suite.py"), "--dir", GEN,
               "--filter", ",".join(chunk), "--no-playthrough", "--workers", str(a.workers),
               "--orig-cache"] + ([] if a.no_fill_cache else ["--fill-cache"])
        if i:
            cmd.append("--no-build")
        subprocess.run(cmd, cwd=ROOT, check=False)
        for n in chunk:
            try:
                post(n)
            except Exception as e:  # keep the batch going; the report lists the missing ones
                print(f"census: {n}: {type(e).__name__}: {e}", file=sys.stderr)
            shutil.rmtree(os.path.join(WORK, n), ignore_errors=True)
        print(f"census: {min(i + a.batch, len(names))}/{len(names)}", flush=True)
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--family", choices=["missile", "state"], required=True)
    r.add_argument("--filter")
    r.add_argument("--batch", type=int, default=24)
    r.add_argument("--workers", type=int, default=3)
    r.add_argument("--no-fill-cache", action="store_true")
    r.add_argument("--force", action="store_true")
    p = sub.add_parser("post")
    p.add_argument("names", nargs="+")
    rp = sub.add_parser("report")
    rp.add_argument("--ledger", default=os.path.join(ROOT, "docs", "handoff", "fidelity-ledger.tsv"))
    rp.add_argument("--part", default=os.path.join(ROOT, "docs", "handoff", "ledger",
                                                   "q-run-missiles-states.tsv"))
    rp.add_argument("--md")
    a = ap.parse_args(argv)
    if a.cmd == "run":
        return cmd_run(a)
    if a.cmd == "post":
        for n in a.names:
            post(n)
        return 0
    import census_report
    return census_report.report(a)


if __name__ == "__main__":
    sys.exit(main())
