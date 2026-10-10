#!/usr/bin/env python3
"""Merge the rows of a suite.py --md table into docs/handoff/checks-status.md.

    python3 tools/coord/status_merge.py SUITE.md [--note TEXT]
Rows `| check | channel | ticks | equal | match | verdict | first |` become
`| check | channel | verdict | equal/ticks | first | owner |`, replacing an existing
(check, channel) row or appended after the last table row. Our own code.
"""
import sys

STATUS = "docs/handoff/checks-status.md"


def main():
    src = sys.argv[1]
    new = {}
    for l in open(src, encoding="utf-8"):
        p = [x.strip() for x in l.strip().strip("|").split("|")]
        if len(p) == 7 and p[0].startswith("gen-") or (len(p) == 7 and p[1] in ("state", "packets", "rng", "items", "draws", "save", "frontend", "sounds", "cstate")):
            chk, ch, ticks, eq, _m, v, first = p
            owner = "-" if v in ("MATCH", "PARTIAL") else "unrouted"
            new[(chk, ch)] = f"| {chk} | {ch} | {v} | {eq}/{ticks} | {first} | {owner} |"
    lines = open(STATUS, encoding="utf-8").read().split("\n")
    out, done, last = [], set(), 0
    for i, l in enumerate(lines):
        p = [x.strip() for x in l.strip().strip("|").split("|")]
        if l.startswith("| ") and len(p) == 6 and (p[0], p[1]) in new:
            out.append(new[(p[0], p[1])]); done.add((p[0], p[1]))
        else:
            out.append(l)
        if l.startswith("| ") and len(p) == 6:
            last = len(out)
    rest = [new[k] for k in sorted(new) if k not in done]
    out[last:last] = rest
    open(STATUS, "w", encoding="utf-8").write("\n".join(out))
    print(f"replaced {len(done)}, appended {len(rest)}")


if __name__ == "__main__":
    main()
