#!/usr/bin/env python3
"""obj_verdicts: collate the `gen-obj-*` runs into ledger rows (our own code).

Spec: specs/tools/scenario-diff.md (channels, verdicts), docs/handoff/ledger
part format: tools/coord/ledger.py.  For every generated object check run by
`suite.py --checks-dir ... --fill-cache` (work dirs traces/raw/suite/<name>/)
it re-runs the state comparator with `--ignore q` (the quest records: d2rs
carries none of the save's, a divergence of the quest rows REC-1625, frame 2
of every check; it would hide everything after it), reads the items and rng
summaries, and prints one verdict per check:

  obj_verdicts.py [--suite DIR] [--out FILE.tsv] [--results FILE.tsv]

--results writes the per-check table (name, state, items, rng, first
divergences, owner); --out writes the ledger part rows of the object areas
(coverage rows `object.<id>-<name>` and the aggregates `object.operate.N.*`,
`object.init.functions`).  Verdict of a check: DIVERGED@frame if any channel
is, else PARTIAL (a state channel never reaches MATCH: fields one side does
not write, specs/tools/state-snapshot.md), else MATCH.
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
REC = os.path.join(ROOT, "tools", "trace-recorder")
LEDGER = os.path.join(ROOT, "docs", "handoff", "fidelity-ledger.tsv")
COLS = ["area", "kind", "group", "source_1.14d", "specs", "spec_status", "checks",
        "last_verdict", "exercised", "provisional", "needs_pc1", "owner", "state",
        "size", "note"]


def sh(argv):
    p = subprocess.run(argv, capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def channel(work, ch):
    """(verdict, frame, text) of one channel of a check's work dir."""
    if ch == "state":
        out = os.path.join(work, "state.ignoreq.json")
        a, b = os.path.join(work, "orig.state.jsonl"), os.path.join(work, "d2rs.state.jsonl")
        if not (os.path.isfile(a) and os.path.isfile(b)):
            return "ERROR", None, "no state recordings"
        sh([sys.executable, os.path.join(REC, "state_diff.py"), a, b, "--ignore", "q",
            "--next", "3", "--json", out])
        path = out
    else:
        path = os.path.join(work, ch + ".summary.json")
    if not os.path.isfile(path):
        return "ERROR", None, f"no {ch} summary"
    d = json.load(open(path))
    first = d.get("first") or {}
    return d["verdict"], first.get("frame"), first.get("text", "")


def route(work, ch):
    path = os.path.join(work, "state.ignoreq.json" if ch == "state" else ch + ".summary.json")
    rc, out = sh([sys.executable, os.path.join(ROOT, "tools", "coord", "route.py"), "--json", path])
    try:
        r = json.loads(out)["routes"][0]
    except Exception:
        return "-"
    if r.get("routed") and r.get("owner"):
        return r["owner"]["branch"]
    return "-"


def check_result(suite, name):
    work = os.path.join(suite, name)
    res = {}
    for ch in ("state", "items", "rng"):
        res[ch] = channel(work, ch)
    bad = [(res[c][1] if res[c][1] is not None else 0, c) for c in res if res[c][0] == "DIVERGED"]
    err = [c for c in res if res[c][0] == "ERROR"]
    if err:
        v = "ERROR"
    elif bad:
        v = f"DIVERGED@{min(bad)[0]}"
    elif any(res[c][0] == "PARTIAL" for c in res):
        v = "PARTIAL"
    else:
        v = "MATCH"
    first = min(bad)[1] if bad else None
    owner = route(work, first) if first else "-"
    return v, res, first, owner


def objects_table(excel):
    with open(os.path.join(excel, "objects.txt"), encoding="latin-1", newline="") as f:
        rows = list(csv.reader(f, delimiter="\t"))
    h = rows[0]
    out = {}
    for r in rows[1:]:
        i = h.index("Id")
        if i < len(r) and r[i].isdigit() and int(r[i]) not in out:
            out[int(r[i])] = {k: (r[h.index(k)] if h.index(k) < len(r) else "")
                              for k in ("Name", "OperateFn", "PopulateFn", "InitFn")}
    return out


def read_ledger():
    rows = {}
    for line in open(LEDGER, encoding="utf-8"):
        if line.startswith("#") or line.startswith("area\t"):
            continue
        c = line.rstrip("\n").split("\t")
        rows[c[0]] = dict(zip(COLS, c))
    return rows


def short(text, n=170):
    t = re.sub(r"\s+", " ", text)
    return t if len(t) <= n else t[:n - 3] + "..."


def populate_rows(ledger, objs, lv):
    """object.populate.N.* from the level runs: the classes of PopulateFn N present in a level on
    one side only are population divergences (the pick is rolled on the level's seeds)."""
    out = []
    for r in ledger.values():
        m = re.fullmatch(r"object\.populate\.(\d+)\..*", r["area"])
        if not m:
            continue
        fn = m.group(1)
        mine = {c for c, o in objs.items() if o["PopulateFn"] == fn}
        seen, bad = set(), []
        for lvl, (o1, o2, v, first) in sorted(lv.items(), key=lambda kv: int(kv[0].split("-")[-1])):
            for c in sorted(set(o1) | set(o2)):
                if c in mine:
                    seen.add(c)
                    if (c in o1) != (c in o2):
                        bad.append((lvl, c, "1.14d" if c in o1 else "d2rs"))
        row = dict(r)
        row.update(kind="entity", group="world", checks="-", exercised="yes", needs_pc1="n")
        if not seen:
            row["note"] = "no level of the 136 warp runs holds an object of this PopulateFn; " + r["note"]
            out.append(row)
            continue
        if bad:
            row.update(last_verdict="DIVERGED", state="DIVERGED", size="M" if len(bad) > 3 else "S",
                       owner="-")
            row["note"] = (f"gen-lvl-* state runs: {len(bad)} class/level pairs present on one side only, "
                           f"first {bad[0][1]} in {bad[0][0]} only on {bad[0][2]}; {len(seen)} classes seen")
        else:
            row.update(last_verdict="PARTIAL", state="NO-CHECK", size="S", owner="-")
            row["note"] = (f"gen-lvl-* state runs (traces/checks/gen): {len(seen)} classes of this "
                           "PopulateFn present identically in every level, equal where compared "
                           "(34 levels diverge on the game seed from frame 20, D1)")
        out.append(row)
    return out


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("--suite", default=os.path.join(ROOT, "traces", "raw", "suite"))
    ap.add_argument("--excel", default=os.path.join(os.environ.get("D2_GAME_DIR", os.path.expanduser("~/game")),
                                                    "extracted", "patch_d2", "data", "global", "excel"))
    ap.add_argument("--levels", help="JSON {gen-lvl-N: [classes 1.14d, classes d2rs, state verdict "
                    "(types 2, q ignored), first]} from the state-only level runs")
    ap.add_argument("--out")
    ap.add_argument("--results")
    a = ap.parse_args(argv)
    ledger = read_ledger()
    objs = objects_table(a.excel)
    results = {}
    for area, row in sorted(ledger.items()):
        m = re.fullmatch(r"object\.(\d+)-.*", area)
        if not m:
            continue
        name = f"gen-obj-{m.group(1)}"
        if not os.path.isdir(os.path.join(a.suite, name)):
            continue
        results[int(m.group(1))] = (name,) + check_result(a.suite, name)
    if a.results:
        with open(a.results, "w", encoding="utf-8") as f:
            f.write("#obj-verdicts 1\nobject\tcheck\tverdict\tstate\titems\trng\towner\n")
            for oid, (name, v, res, first, owner) in sorted(results.items()):
                f.write("\t".join([str(oid), name, v] + [
                    f"{res[c][0]}@{res[c][1]}: {short(res[c][2], 120)}" if res[c][0] == "DIVERGED"
                    else res[c][0] for c in ("state", "items", "rng")] + [owner]) + "\n")
    if not a.out:
        return 0
    out = []
    byfn = {}
    for oid, (name, v, res, first, owner) in sorted(results.items()):
        area = next(k for k in ledger if k.startswith(f"object.{oid}-"))
        row = dict(ledger[area])
        # ledger.py only knows traces/checks/*.check: the generated check is named in the note
        row.update(kind="entity", group="world", checks="-", last_verdict=v, exercised="yes",
                   needs_pc1="n", owner=owner)
        if v.startswith("DIVERGED"):
            row["state"], row["size"] = "DIVERGED", "S"
            row["note"] = f"{first}: {short(res[first][2])} (state compared with q ignored, REC-1625)"
        elif v == "PARTIAL":
            row["state"], row["size"] = "NO-CHECK", "S"
            row["note"] = ("state, items and rng equal where compared (state PARTIAL: fields one "
                           "side does not write; q ignored, REC-1625)")
        else:
            row["state"], row["size"] = "EQUAL", "-"
            row["note"] = "state, items and rng MATCH"
        o = objs.get(oid, {})
        row["note"] = (f"{name} (traces/checks/gen), OperateFn {o.get('OperateFn')} InitFn "
                       f"{o.get('InitFn')}: " + row["note"])
        out.append(row)
        for key in (("operate", o.get("OperateFn")), ("init", o.get("InitFn"))):
            byfn.setdefault(key, []).append((oid, name, v, owner))
    # aggregates
    for r in list(ledger.values()):
        m = re.fullmatch(r"object\.operate\.(\d+)\.(.*)", r["area"])
        if m:
            members = byfn.get(("operate", m.group(1)), [])
        elif r["area"] == "object.init.functions":
            members = [x for k, v in byfn.items() if k[0] == "init" for x in v]
        else:
            continue
        if not members:
            continue
        row = dict(r)
        div = [x for x in members if x[2].startswith("DIVERGED")]
        row.update(kind="entity", group="world", exercised="yes", needs_pc1="n")
        row["checks"] = "-"
        names = sorted({x[1] for x in members}, key=lambda n: int(n.split("-")[-1]))
        lst = ", ".join(names) if len(names) <= 6 else f"{names[0]} .. {names[-1]} ({len(names)})"
        row["note_prefix"] = f"checks {lst} (traces/checks/gen): "
        frames = [int(x[2].split("@")[1]) for x in div]
        row["last_verdict"] = (f"DIVERGED@{min(frames)}" if div else
                               "PARTIAL" if any(x[2] == "PARTIAL" for x in members) else "MATCH")
        owners = sorted({x[3] for x in div if x[3] != "-"})
        row["owner"] = owners[0] if len(owners) == 1 else "-"
        if div:
            row["state"], row["size"] = "DIVERGED", "S" if len(div) < 5 else "M"
            row["note"] = (f"{len(div)} of {len(members)} object rows diverge (first: object "
                           f"{div[0][0]}); the rest equal where compared")
        else:
            row["state"], row["size"] = "NO-CHECK", "S"
            row["note"] = f"all {len(members)} object rows equal where compared (state PARTIAL)"
        row["note"] = row.pop("note_prefix") + row["note"]
        out.append(row)
    if a.levels:
        out += populate_rows(ledger, objs, json.load(open(a.levels)))
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("#ledger 1\n" + "\t".join(COLS) + "\n")
        for r in out:
            f.write("\t".join(r[c] for c in COLS) + "\n")
    print(f"obj_verdicts: {len(results)} checks, {len(out)} ledger rows")
    return 0


if __name__ == "__main__":
    sys.exit(main())
