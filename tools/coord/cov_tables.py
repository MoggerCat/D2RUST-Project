#!/usr/bin/env python3
"""Check the `cov.*` coverage rows against the 1.14d data tables.

OWNER DECISION (docs/PLAN.md decisions log): a coverage row is checked by
comparing the value d2rs uses with the table value for the same record, no
written spec needed. d2rs' value = the record `d2-data` compiles from the
table's `.txt` (and loads); the table value = the shipped 1.14d `.bin`
record. `data-tool cov-records` compares them field by field (every byte
of the record); a row is EQUAL only when its record is identical. The
key of each row (`Id`/first column) is checked against the ledger name.

    python3 tools/coord/cov_tables.py --records R.tsv --excel DIR \
        [--ledger docs/handoff/fidelity-ledger.tsv] [--out PART.tsv]
    python3 tools/coord/cov_tables.py --selftest

R.tsv = `data-tool cov-records`; DIR = `data-tool excel-dir` output.
Rows already DIVERGED stay as they are (a behaviour check owns them); the
table verdict is reported beside them. Rows with no table (quest flags and
slots are d2rs-internal) stay as they are.
"""
import argparse
import re
import sys
from pathlib import Path

COLS = ["area", "kind", "group", "source_1.14d", "specs", "spec_status", "checks",
        "last_verdict", "exercised", "provisional", "needs_pc1", "owner", "state",
        "size", "note"]
TABLES = {"monstats": "monstats", "levels": "levels", "missiles": "missiles",
          "skills": "skills", "states": "states", "objects": "objects"}
CHECK = "tools/coord/cov_tables.py"
SRC = re.compile(r"^(monstats|levels|missiles|skills|states|objects)\.txt (id|row) (\d+) \((.*)\)$", re.I)
NPC = re.compile(r"^npc class id (\d+) \((.*)\)$")
ITEM = re.compile(r"^armor/weapons/misc\.txt row (\d+) \((.*)\)$")
AI = re.compile(r"^monstats\.txt AI column row (.*) \((.*)\)$")
NEVER = re.compile(r"^(monstats AI|monstats\.txt|Levels\.txt|Missiles\.txt|Skills\.txt|States\.txt|Objects\.txt|Items|npc class|quest flag) entries reachable")
NEVER_TABLES = {"monstats AI": ["monai"], "monstats.txt": ["monstats"], "levels.txt": ["levels"],
                "missiles.txt": ["missiles"], "skills.txt": ["skills"], "states.txt": ["states"],
                "objects.txt": ["objects"], "items": ["weapons", "armor", "misc"],
                "npc class": ["monstats"]}


def read_txt(path):
    rows = [l.rstrip("\r").split("\t") for l in path.read_text("latin-1").split("\n")]
    rows = [r for r in rows if any(c for c in r)]
    # the "Expansion" separator row is not a record (field-types.md; the compiler skips it)
    rows = [r for r in rows[:1]] + [r for r in rows[1:] if r[0].lower() != "expansion"]
    return rows[0], rows[1:]


def parse_records(text):
    """-> ({table: (records_txt, records_bin)}, {table: {record: [labels]}})"""
    info, diffs = {}, {}
    for line in text.splitlines():
        f = line.split("\t")
        if f[0] == "table":
            info[f[1]] = (int(f[2]), int(f[3]))
        elif f[0] == "diff":
            labs = sorted(set(x for x in f[3].split("|")))
            diffs.setdefault(f[1], {})[int(f[2])] = labs
    return info, diffs


def locate(source, excel):
    """source_1.14d cell -> (table, record index or None, key name, id) or
    ('whole', [tables]) / None (no table)."""
    s = source.strip()
    m = SRC.match(s)
    if m:
        t, kind, n, name = m.group(1).lower(), m.group(2), int(m.group(3)), m.group(4)
        hdr, rows = excel(t)
        if kind == "row":
            return t, n, name
        if t == "monstats":  # Id is the name column; the number is the row (hcIdx)
            return t, n, name
        idc = [h.lower() for h in hdr].index("id")
        for i, r in enumerate(rows):
            if idc < len(r) and r[idc] == str(n):
                return t, i, name
        return t, None, name
    m = NPC.match(s)
    if m:
        return "monstats", int(m.group(1)), m.group(2)
    m = ITEM.match(s)
    if m:
        name = m.group(2)
        for t in ("weapons", "armor", "misc"):
            hdr, rows = excel(t)
            hit = [i for i, r in enumerate(rows) if r and r[0] == name]
            if hit:
                return t, hit[0], name
        return "weapons", None, name
    m = AI.match(s)
    if m:
        hdr, rows = excel("monai")
        hit = [i for i, r in enumerate(rows) if r and r[0].lower() == m.group(1).lower()]
        return "monai", (hit[0] if hit else None), m.group(2)
    m = NEVER.match(s)
    if m:
        k = m.group(1).lower() if m.group(1) != "monstats AI" else "monstats AI"
        k = {"monstats ai": "monstats AI"}.get(k, k)
        tabs = NEVER_TABLES.get(k) or NEVER_TABLES.get(m.group(1).lower())
        return ("whole", tabs) if tabs else None
    return None


def verdict(loc, info, diffs, excel):
    """-> (EQUAL|DIVERGED|UNKNOWN, cause text)"""
    if loc is None:
        return "UNKNOWN", "no data table behind this row"
    if loc[0] == "whole":
        bad = []
        for t in loc[1]:
            if t not in info:
                return "UNKNOWN", f"table {t} not in the cross-check"
            nt, nb = info[t]
            if nt != nb:
                bad.append(f"{t} records {nt} vs bin {nb}")
            for r, labs in diffs.get(t, {}).items():
                bad.append(f"{t}[{r}] {';'.join(labs)}")
        return ("EQUAL", f"all records of {', '.join(loc[1])} identical") if not bad else ("DIVERGED", "; ".join(bad))
    t, rec, name = loc
    if rec is None:
        return "UNKNOWN", f"{t}: record for the key not found"
    if t not in info:
        return "UNKNOWN", f"table {t} not in the cross-check"
    hdr, rows = excel(t)
    if rec >= len(rows) or rec >= info[t][1]:
        return "UNKNOWN", f"{t} record {rec} beyond the table"
    key = rows[rec][0] if t != "monstats" else rows[rec][0]
    if key.lower() != name.lower():
        return "UNKNOWN", f"{t} record {rec} is '{key}', ledger says '{name}'"
    if rec in diffs.get(t, {}):
        return "DIVERGED", f"{t} record {rec} ({key}): " + ";".join(diffs[t][rec])
    return "EQUAL", f"{t} record {rec} ({key}) identical in all {len(rows[rec])} columns' bytes"


def run(records, exceldir, ledger, out):
    info, diffs = parse_records(Path(records).read_text())
    cache = {}

    def excel(t):
        if t not in cache:
            cache[t] = read_txt(Path(exceldir) / f"{t}.txt")
        return cache[t]

    lines = Path(ledger).read_text().split("\n")
    hdr = next(l for l in lines if l.startswith("area\t")).split("\t")
    outrows, tally, kept, unk = [], {}, [], {}
    for l in lines[2:]:
        c = l.split("\t")
        if len(c) < 15 or not c[0].startswith("cov."):
            continue
        r = dict(zip(hdr, c))
        v, cause = verdict(locate(r["source_1.14d"], excel), info, diffs, excel)
        key = (r["state"], v)
        tally[key] = tally.get(key, 0) + 1
        if r["state"] == "DIVERGED":
            kept.append((r["area"], v, cause))
            continue
        if v == "UNKNOWN":
            unk.setdefault(re.sub(r"\d+", "N", cause)[:80], []).append(r["area"])
            continue
        r["group"] = "cov-tables"  # not `coverage`: those rows only set `exercised` in the merge
        r["checks"], r["last_verdict"] = "-", "MATCH" if v == "EQUAL" else "DIVERGED"
        r["state"], r["size"] = v, "-" if v == "EQUAL" else ("S" if v == "DIVERGED" else "M")
        r["note"] = f"cov-tables ({CHECK}): {cause} (d2-data compiled txt vs shipped .bin, `data-tool cov-records`)"
        outrows.append("\t".join(r[h] for h in COLS))
    if out:
        Path(out).write_text("#ledger 1\n" + "\t".join(COLS) + "\n" + "\n".join(outrows) + "\n")
    for c, l in unk.items():
        print(f"unresolved {len(l)} x {c}: {l[:3]}")
    return tally, kept, outrows


def selftest():
    ex = {"monstats": (["Id"], [["a"], ["b"]]), "levels": (["Name", "Id"], [["x", "0"], ["y", "5"]])}
    e = lambda t: ex[t]
    assert locate("monstats.txt row 1 (b)", e) == ("monstats", 1, "b")
    assert locate("Levels.txt id 5 (y)", e) == ("levels", 1, "y")
    assert locate("npc class id 0 (a)", e) == ("monstats", 0, "a")
    assert locate("quest slot (specs) row 3 (Q)", e) is None
    assert locate("Items entries reachable but in no check or soak run", e) == ("whole", ["weapons", "armor", "misc"])
    info = {"monstats": (2, 2), "levels": (2, 2)}
    d = {"monstats": {1: ["~x"]}}
    assert verdict(("monstats", 0, "a"), info, d, e)[0] == "EQUAL"
    assert verdict(("monstats", 1, "b"), info, d, e)[0] == "DIVERGED"
    assert verdict(("monstats", 1, "zz"), info, d, e)[0] == "UNKNOWN"
    assert verdict(("whole", ["monstats"]), info, d, e)[0] == "DIVERGED"
    assert verdict(("whole", ["levels"]), info, d, e)[0] == "EQUAL"
    assert verdict(None, info, d, e)[0] == "UNKNOWN"
    print("cov_tables selftest ok")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--records")
    ap.add_argument("--excel")
    ap.add_argument("--ledger", default="docs/handoff/fidelity-ledger.tsv")
    ap.add_argument("--out")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    tally, kept, rows = run(a.records, a.excel, a.ledger, a.out)
    for (st, v), n in sorted(tally.items()):
        print(f"ledger {st:9} table {v:9} {n}")
    causes = {}
    for a_, v, cause in kept:
        if v != "EQUAL":
            causes.setdefault(re.sub(r"\d+", "N", cause)[:70], []).append(a_)
    for c, l in causes.items():
        print(f"kept DIVERGED rows with table verdict not EQUAL: {len(l)} x {c}: {l[:3]}")
    print(f"{len(rows)} part rows")


if __name__ == "__main__":
    sys.exit(main())
