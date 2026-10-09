#!/usr/bin/env python3
"""Runs the d2rs side of generated checks and reports whether they ran.

  d2rs_smoke.py [--sample N] [--family F]... [CHECK_NAME...]

For each check: `scenario_diff.py traces/checks/gen/<name>.check --d2rs-only`,
then the poke records of d2rs.state.jsonl: every poke (and `send`) must be
`ok`, and the run must reach the check's last tick.  Prints one line per
check and a summary; exit 1 when one did not run.  With --sample N it takes
N evenly spaced checks of each family from traces/checks/gen/INDEX.tsv.
Needs D2_GAME_DIR and `cargo build --release -p d2-client -p d2s-tool`.
"""
import argparse
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
GEN = os.path.join(ROOT, "traces", "checks", "gen")


def index():
    rows = []
    with open(os.path.join(GEN, "INDEX.tsv"), encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or line.startswith("name\t"):
                continue
            c = line.rstrip("\n").split("\t")
            rows.append((c[0], c[1]))
    return rows


def run_one(name):
    text = open(os.path.join(GEN, name + ".check"), encoding="utf-8").read()
    ticks = int(re.search(r"^ticks (\d+)", text, re.M).group(1))
    r = subprocess.run([sys.executable, os.path.join(ROOT, "tools", "scenario-diff",
                        "scenario_diff.py"), os.path.join(GEN, name + ".check"), "--d2rs-only",
                        "--channels", "state"], capture_output=True, text=True, cwd=ROOT)
    path = os.path.join(ROOT, "traces", "raw", "check-" + name, "d2rs.state.jsonl")
    if r.returncode != 0 or not os.path.isfile(path):
        return False, "run failed: " + (r.stderr or r.stdout)[-200:].replace("\n", " ")
    bad, last, n = [], 0, 0
    with open(path, encoding="utf-8") as f:
        for line in f:
            x = json.loads(line)
            if x.get("k") == "poke":
                n += 1
                if x["r"] != "ok":
                    bad.append(f"{x['d']} {x['r']} ({x.get('src')})")
            elif x.get("k") == "snap":
                last = x["f"]
    if bad:
        return False, "pokes not ok: " + "; ".join(bad)
    if last < ticks:
        return False, f"stopped at frame {last} of {ticks}"
    return True, f"{n} pokes ok, {last} frames"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*")
    ap.add_argument("--sample", type=int, default=0)
    ap.add_argument("--family", action="append")
    a = ap.parse_args()
    names = list(a.names)
    if a.sample:
        fams = {}
        for n, f in index():
            fams.setdefault(f, []).append(n)
        for f, ns in fams.items():
            if a.family and f not in a.family:
                continue
            k = min(a.sample, len(ns))
            names += [ns[i * len(ns) // k] for i in range(k)]
    if not names:
        ap.error("no checks")
    failed = 0
    for n in names:
        ok, msg = run_one(n)
        failed += not ok
        print(("ok   " if ok else "FAIL ") + n + ": " + msg, flush=True)
    print(f"d2rs_smoke: {len(names) - failed}/{len(names)} ran")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
