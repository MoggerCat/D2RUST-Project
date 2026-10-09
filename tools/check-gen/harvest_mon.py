#!/usr/bin/env python3
"""Harvest the suite results of the `mon` family into a ledger part.

    python3 tools/check-gen/harvest_mon.py RESULT.json... [--ledger docs/handoff/fidelity-ledger.tsv]
        [--out docs/handoff/ledger/q-run-monsters.tsv] [--causes docs/handoff/q-run-monsters-causes.tsv]

RESULT.json: `suite.py --json` outputs of runs over traces/checks/gen (checks gen-mon-<hcIdx>).
One ledger row per `monster.<id>` area (copied from the fidelity ledger, columns
checks / last_verdict / state / size / owner / note filled). Verdict per row:
EQUAL = state and rng channels equal on all frames (state PARTIAL with 0 differences
only because the fields `q` and `seed` are ignored, see check_gen.py fam_mon);
DIVERGED@<frame> = the first of the two channels' first divergence; PARTIAL otherwise.
Our own code. Standard library only.
"""

import argparse
import csv
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
COLS = ["area", "kind", "group", "source_1.14d", "specs", "spec_status", "checks", "last_verdict",
        "exercised", "provisional", "needs_pc1", "owner", "state", "size", "note"]


def cause_of(channel, text):
    """A short normalised label of a first-divergence text (the clustering key)."""
    if not text:
        return ""
    m = re.search(r"(monster|player|item|missile|object) (\d+:\d+) class (-?\d+)?,? ?field (\w+)", text)
    if m:
        return f"state {m.group(1)} field {m.group(4)}"
    m = re.search(r"unit (\d+):\d+, draw #(\d+), field (\w+): (.*)", text)
    if m:
        site = re.sub(r"\s+", " ", m.group(4))
        site = re.sub(r"crates/\S+", "d2rs-site", site)
        return f"rng unit-type {m.group(1)} draw #{m.group(2)} field {m.group(3)}: {site[:60]}"
    return channel + " " + re.sub(r"\d+", "N", text)[:70]


def route_owner(text, fallback):
    """The owner branch tools/coord/route.py names for a first-divergence text, else fallback."""
    p = subprocess.run([sys.executable, os.path.join(ROOT, "tools", "coord", "route.py"), "-"],
                       input="FIRST DIVERGENCE: " + text + "\n", capture_output=True, text=True)
    m = re.search(r"owner:\s+(.*)", p.stdout)
    if m:
        br = re.search(r"claude/[\w./-]+", m.group(1))
        if br:
            return br.group(0)
    return fallback


def verdict_of(rec):
    ch = rec["channels"]
    parts = []
    for name in ("state", "rng"):
        c = ch.get(name)
        if not c:
            continue
        v, s = c["verdict"], c.get("summary") or {}
        first = s.get("first") or {}
        parts.append((name, v, first.get("frame"), first.get("text", ""), s.get("differences")))
    if rec.get("error") or any(v == "ERROR" for _, v, *_ in parts):
        return "ERROR", None, parts
    bad = [p for p in parts if p[1] == "DIVERGED"]
    if bad:
        frame = min(p[2] for p in bad if p[2] is not None)
        return f"DIVERGED@{frame}", frame, parts
    # state PARTIAL with no differences = equal under the ignored q/seed fields
    if all(p[1] in ("MATCH", "PARTIAL") and not p[4] for p in parts):
        return "EQUAL", None, parts
    return "PARTIAL", None, parts


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("results", nargs="+")
    ap.add_argument("--ledger", default=os.path.join(ROOT, "docs", "handoff", "fidelity-ledger.tsv"))
    ap.add_argument("--out", default=os.path.join(ROOT, "docs", "handoff", "ledger", "q-run-monsters.tsv"))
    ap.add_argument("--causes", default=os.path.join(ROOT, "docs", "handoff", "q-run-monsters-causes.tsv"))
    ap.add_argument("--owner", default="claude/coord-resume-3",
                    help="owner column of a DIVERGED row (route.py names it per cause)")
    a = ap.parse_args(argv)
    recs = {}
    for p in a.results:
        for r in json.load(open(p))["checks"]:
            recs[r["name"]] = r  # a later file overrides an earlier one
    with open(a.ledger, encoding="utf-8") as f:
        lines = [l for l in f if not l.startswith("#")]
    rows = {r["area"]: r for r in csv.DictReader(lines, delimiter="\t")}
    by_hc = {}
    for r in rows.values():
        m = re.search(r"\(hcIdx (\d+)\)$", r["source_1.14d"])
        if r["area"].startswith("monster.") and r["source_1.14d"].startswith("monstats.txt") and m:
            by_hc[int(m.group(1))] = r
    out, causes, owners = [], {}, {}
    counts = {}
    for hc, row in sorted(by_hc.items()):
        rec = recs.get(f"gen-mon-{hc}")
        if rec is None:
            continue
        verdict, frame, parts = verdict_of(rec)
        row = dict(row)
        row["checks"] = f"gen-mon-{hc}"
        row["group"] = "monsters"  # an entity row, so the coverage row of the same area only adds `exercised`
        row["last_verdict"] = "MATCH" if verdict == "EQUAL" else verdict  # ledger.py's vocabulary
        counts[verdict.split("@")[0]] = counts.get(verdict.split("@")[0], 0) + 1
        if verdict == "EQUAL":
            row["state"], row["size"], row["owner"] = "EQUAL", "-", "-"
            row["note"] = ("spawn + 150 ticks: state and rng channels equal on all frames "
                           "(fields q, seed ignored: harness level)")
        elif verdict == "ERROR":
            row["state"], row["size"] = "UNKNOWN", "S"
            row["note"] = "check run failed: " + str(rec.get("error"))[:100]
        else:
            first = min((p for p in parts if p[1] == "DIVERGED" and p[2] is not None),
                        key=lambda p: (p[2], p[0] != "state"))
            key = cause_of(first[0], first[3])
            causes.setdefault(key, []).append((hc, first[3]))
            row["state"], row["size"] = "DIVERGED", "M"
            if key not in owners:
                owners[key] = route_owner(first[3], a.owner)
            row["owner"] = owners[key]
            row["note"] = f"{first[0]} {first[3]}"[:200]
            row["_cause"] = key
        out.append(row)
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("#ledger 1\n" + "\t".join(COLS) + "\n")
        for r in out:
            f.write("\t".join(str(r.get(c, "-")).replace("\t", " ") for c in COLS) + "\n")
    with open(a.causes, "w", encoding="utf-8", newline="\n") as f:
        f.write("#q-run-monsters causes 1\ncause\trows\texample\tclasses\n")
        for k, v in sorted(causes.items(), key=lambda kv: -len(kv[1])):
            f.write(f"{k}\t{len(v)}\t{v[0][1][:120]}\t{' '.join(str(h) for h, _ in v[:12])}\n")
    print(f"harvest: {len(out)} rows: {counts}; {len(causes)} first-divergence causes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
