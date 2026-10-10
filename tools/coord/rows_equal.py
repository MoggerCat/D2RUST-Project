#!/usr/bin/env python3
"""Write ledger-part rows (state EQUAL) for rows settled by other rows' checks.

    python3 tools/coord/rows_equal.py PART_NAME < spec.tsv
spec.tsv lines: `area<TAB>checks(comma list)<TAB>note[<TAB>EQUAL|DIVERGED@frame[<TAB>owner]]`. The row is copied from the
merged ledger and set to EQUAL / MATCH-or-PARTIAL as given in the note's prefix
`[PARTIAL]` (default PARTIAL). Appends to docs/handoff/ledger/PART_NAME.tsv.
Our own code.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import link_checks as L


def main():
    part = f"docs/handoff/ledger/{sys.argv[1]}.tsv"
    rows = {r["area"]: r for r in L.load_ledger("docs/handoff/fidelity-ledger.tsv")}
    old = open(part, encoding="utf-8").read().split("\n") if os.path.exists(part) else None
    out = old[:-1] if old else open("docs/handoff/ledger/rc-link-2.tsv", encoding="utf-8").read().split("\n")[:2]
    have = {l.split("\t")[0] for l in out[2:]}
    hdr = out[1].split("\t")
    for line in sys.stdin:
        a, checks, note, *rest = line.rstrip("\n").split("\t")
        state = rest[0] if rest else "EQUAL"
        if a in have:
            continue
        r = dict(rows[a])
        if state == "EQUAL":
            r.update(checks=checks, last_verdict="PARTIAL", exercised="yes", state="EQUAL", size="-", note=note)
        else:
            r.update(checks=checks, last_verdict=state, exercised="yes", state="DIVERGED", size="M", note=note,
                     owner=rest[1] if len(rest) > 1 else "-")
        out.append("\t".join(r[c] for c in hdr))
    open(part, "w", encoding="utf-8").write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
