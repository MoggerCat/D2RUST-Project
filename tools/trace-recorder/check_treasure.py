"""Check the treasure-class memory dump of dump_tables.py against the
"Real 1.14d" vectors of specs/items/treasure.md (§1.1 layout, §1.6 chest
table, Test vectors).

Usage: py tools/trace-recorder/check_treasure.py traces/raw/<time>-tables
Exit 0 when every vector holds; each failure is printed. --perturb N
flips the group of TC N before checking (M08: must fail).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import json
import os
import struct
import sys

REC, ENT = 0x2C, 0x1C


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("dump")
    ap.add_argument("--perturb", type=int, default=None)
    a = ap.parse_args()
    man = json.load(open(os.path.join(a.dump, "manifest.json")))
    recs = bytearray(open(os.path.join(a.dump, "map-tc_records.bin"), "rb").read())
    ents = open(os.path.join(a.dump, "map-tc_entries.bin"), "rb").read()
    chest = open(os.path.join(a.dump, "map-tc_chest.bin"), "rb").read()
    base = int(man["maps"]["tc_entries"]["tc_base"], 16)
    if a.perturb is not None:
        recs[a.perturb * REC] ^= 1
    n = len(recs) // REC
    tcs, off = [], 0
    for i in range(n):
        r = recs[i * REC:(i + 1) * REC]
        group, level, count, tc_, te, picks, nodrop = struct.unpack_from("<HhiiiIi", r, 0)
        picks = struct.unpack_from("<i", r, 0x10)[0]
        es = []
        for k in range(max(count, 0)):
            sc, se, id_, row, flags = struct.unpack_from("<iiHHB", ents, off + k * ENT)
            es.append((sc, se, id_, row, flags))
        off += max(count, 0) * ENT
        tcs.append(dict(group=group, level=level, count=count, tc=tc_, te=te,
                        picks=picks, nodrop=nodrop, entries=es))
    fails = []

    def want(what, got, exp):
        if got != exp:
            fails.append(f"{what}: got {got!r}, want {exp!r}")

    want("TC count", n, 1013)
    want("entry bytes consumed", off, len(ents))
    allents = [e for t in tcs for e in t["entries"]]
    want("TC entries (flag 4)", sum(1 for e in allents if e[4] & 4), 2742)
    want("unique entries (flag 1)", sum(1 for e in allents if e[4] & 1), 2)
    want("set entries (flag 2)", sum(1 for e in allents if e[4] & 2), 0)
    # §1.5 counts treasureclassex rows only: TC 0 and the 160 automatic
    # TCs (§1.2, §1.3) come first.
    rows = [e for t in tcs[161:] for e in t["entries"]]
    items = [e for e in rows if not e[4] & 7]
    want("treasureclassex item entries", len(items), 660)
    want("item entries with mul", sum(1 for e in items if e[3]), 81)
    want("automatic TCs with entries",
         sum(1 for t in tcs[1:161] if t["count"] > 0) <= 160, True)
    want("TC 0 entries", tcs[0]["count"], 0)
    print(f"automatic TCs 1-160: {sum(len(t['entries']) for t in tcs[1:161])} entries")
    t = tcs[430]
    want("TC 430 group/picks/nodrop", (t["group"], t["picks"], t["nodrop"]), (12, 1, 100))
    want("TC 430 entry ids", [e[2] for e in t["entries"]], [523, 218, 203, 370])
    want("TC 430 starts", [e[0] for e in t["entries"]], [0, 21, 37, 58])
    want("TC 430 totals", (t["tc"], t["te"]), (60, 60))
    ptrs = struct.unpack("<45I", chest)
    idx = [(p - base) // REC if p else None for p in ptrs]
    want("chest table: all found", sum(1 for p in ptrs if p), 45)
    want("chest table: Act 1 Chest A", idx[0], 385)
    for f in fails:
        print("FAIL", f)
    print(f"check_treasure: {n} TCs, {len(allents)} entries, {len(fails)} failures")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
