#!/usr/bin/env python3
"""Link NO-CHECK ledger rows to the generated checks that exist for them.

    python3 tools/coord/link_checks.py [--ledger F] [--checks-dir D] [--out MAP.tsv]
    python3 tools/coord/link_checks.py --selftest

A row is linkable when its state is NO-CHECK and its `checks` cell is `-`.
Rules, first hit wins, every result filtered to checks that exist:
  1. note   a generated check name in the row's note (`gen-obj-175`, `[gen-missile-207]`)
  2. index  traces/checks/gen/INDEX.tsv names the row's area in `ledger_area`
  3. area   object.<n>-*  -> gen-obj-<n>;  missile.<name> -> the INDEX row
            whose `row` cell names the missile (case-insensitive);
            skill.<cls>.* -> gen-skill-<cls>-<id> when the note/area gives an id
Output (--out): `area<TAB>rule<TAB>checks` (comma list), sorted by area.
"""
import argparse
import os
import re
import sys

NOTE_RE = re.compile(r"gen-[a-z0-9]+(?:-[a-z0-9]+)*")


def load_ledger(path):
    rows = []
    hdr = None
    for line in open(path, encoding="utf-8"):
        if line.startswith("#"):
            continue
        cells = line.rstrip("\n").split("\t")
        if hdr is None:
            hdr = cells
            continue
        rows.append(dict(zip(hdr, cells)))
    return rows


def load_index(path):
    """name -> (family, row, ledger_area)"""
    out = {}
    for line in open(path, encoding="utf-8"):
        if line.startswith("#") or line.startswith("name\t"):
            continue
        p = line.rstrip("\n").split("\t")
        if len(p) >= 4:
            out[p[0]] = (p[1], p[2], p[3])
    return out


def existing_checks(checks_dir):
    return {f[:-6] for f in os.listdir(checks_dir) if f.endswith(".check")}


def linkable(row):
    return row.get("state") == "NO-CHECK" and row.get("checks", "-") in ("-", "")


def link_row(row, index, names):
    """-> (rule, [checks]) or None."""
    area = row["area"]
    hit = [c for c in NOTE_RE.findall(row.get("note", "")) if c in names]
    if hit:
        return "note", sorted(set(hit))
    hit = sorted(n for n, (_f, _r, a) in index.items() if a == area and n in names)
    if hit:
        return "index", hit
    m = re.match(r"object\.(\d+)-", area)
    if m and f"gen-obj-{m.group(1)}" in names:
        return "area", [f"gen-obj-{m.group(1)}"]
    m = re.match(r"missile\.(.+)$", area)
    if m:
        want = m.group(1).replace("-", "").lower()
        hit = sorted(
            n for n, (f, r, _a) in index.items()
            if f == "missile" and n in names and want in re.sub(r"[^a-z0-9]", "", r.lower())
        )
        if len(hit) == 1:
            return "area", hit
    return None


def link_all(rows, index, names):
    out = {}
    for r in rows:
        if linkable(r):
            res = link_row(r, index, names)
            if res:
                out[r["area"]] = res
    return out


def selftest():
    rows = [
        {"area": "object.175-boulder", "state": "NO-CHECK", "checks": "-", "note": "x"},
        {"area": "state.itemset6", "state": "NO-CHECK", "checks": "-", "note": "[gen-state-170] 1"},
        {"area": "monster.su.x", "state": "NO-CHECK", "checks": "-", "note": "gen-su-99 missing"},
        {"area": "level.9", "state": "NO-CHECK", "checks": "-", "note": ""},
        {"area": "object.5-a", "state": "EQUAL", "checks": "gen-obj-5", "note": ""},
    ]
    index = {"gen-lvl-9": ("lvl", "levels.txt Id 9", "level.9"),
             "gen-missile-7": ("missile", "Missiles.txt Bone Wall", "-")}
    names = {"gen-obj-175", "gen-state-170", "gen-lvl-9", "gen-missile-7", "gen-obj-5"}
    got = link_all(rows, index, names)
    assert got == {"object.175-boulder": ("area", ["gen-obj-175"]),
                   "state.itemset6": ("note", ["gen-state-170"]),
                   "level.9": ("index", ["gen-lvl-9"])}, got
    assert link_row({"area": "missile.bone-wall", "note": ""}, index, names) == ("area", ["gen-missile-7"])
    print("link_checks selftest ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ledger", default="docs/handoff/fidelity-ledger.tsv")
    ap.add_argument("--checks-dir", default="traces/checks/gen")
    ap.add_argument("--out")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    rows = load_ledger(a.ledger)
    index = load_index(os.path.join(a.checks_dir, "INDEX.tsv"))
    got = link_all(rows, index, existing_checks(a.checks_dir))
    lines = [f"{k}\t{r}\t{','.join(c)}" for k, (r, c) in sorted(got.items())]
    if a.out:
        open(a.out, "w", encoding="utf-8").write("\n".join(lines) + "\n")
    by = {}
    for r, _ in got.values():
        by[r] = by.get(r, 0) + 1
    print(f"linked {len(got)} of {sum(1 for r in rows if linkable(r))} NO-CHECK rows: {by}")


if __name__ == "__main__":
    sys.exit(main())
