#!/usr/bin/env python3
"""Replace the rows of docs/handoff/checks-status.md for the checks whose work
dir (traces/raw/suite/<name>/result.json) was just re-run, without regenerating
the whole file (status_md.py rewrites it from one full suite json).

    python3 tools/scenario-diff/status_patch.py NAME_GLOB...

Standard library only. Our own code. Spec: specs/tools/scenario-diff.md §4.
"""
import fnmatch
import json
import os
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
STATUS = os.path.join(REPO, "docs", "handoff", "checks-status.md")
SUITE = os.path.join(REPO, "traces", "raw", "suite")


def rows_of(name):
    d = json.load(open(os.path.join(SUITE, name, "result.json")))
    out = {}
    for ch, r in (d.get("channels") or {}).items():
        s = r["summary"]
        first = (s.get("first") or {}).get("text", "-").replace("|", "/")
        own = "-" if first == "-" else "unrouted"
        out[ch] = f"| {name} | {ch} | {r['verdict']} | {s.get('frames_equal')}/{s.get('frames_compared')} | {first} | {own} |"
    return out


def main():
    pats = sys.argv[1:]
    names = sorted(n for n in os.listdir(SUITE) if any(fnmatch.fnmatch(n, p) for p in pats)
                   and os.path.exists(os.path.join(SUITE, n, "result.json")))
    new = {n: rows_of(n) for n in names}
    lines = open(STATUS).read().split("\n")
    seen, out = set(), []
    for l in lines:
        c = l.split("|")
        if len(c) > 3 and c[1].strip() in new and c[2].strip() in new[c[1].strip()]:
            key = (c[1].strip(), c[2].strip())
            if key not in seen:
                out.append(new[key[0]][key[1]])
                seen.add(key)
            continue
        out.append(l)
    for n in names:
        for ch, row in new[n].items():
            if (n, ch) not in seen:
                out.insert(len(out) - 1 if out[-1] == "" else len(out), row)
    open(STATUS, "w").write("\n".join(out))
    print(len(names), "checks patched")


main()
