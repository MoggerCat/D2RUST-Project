#!/usr/bin/env python3
"""Quest/NPC-relevant S->C messages of a check's two packet recordings, side by side.
    python3 tools/chk-act2/qflags.py traces/raw/suite/<check>        (ids: --ids 0x5d,0x28,...)
A record's frame is the next 'flush' record's frame. Prints the first difference of the
sequence (id + bytes) and both lists. Our own code."""
import json, sys
IDS = {0x5D: "QuestStatus", 0x28: "NpcDialogStart/QuestFlags", 0x27: "NpcInfo", 0x8A: "NpcWantsInteract",
       0x62: "NpcDialogEnd", 0x29: "NpcDialogMore", 0x2A: "NpcTransaction", 0x58: "QuestItemUse", 0x4F: "HireList", 0x4E: "HireList2",
       0x63: "WaypointMenu", 0x91: "NpcIntro", 0x1E: "SetStatWord", 0x95: "LifeManaUpdate2"}
def load(p, ids):
    out, pend = [], []
    for l in open(p):
        l = l.strip()
        if not l: continue
        r = json.loads(l)
        if r.get("type") == "s2c":
            b = bytes.fromhex(r["bytes"])
            if b and b[0] in ids: pend.append((b[0], r["bytes"]))
        elif r.get("type") == "flush" and r.get("frame") is not None:
            out += [(r["frame"],) + x for x in pend]; pend = []
    return out
def main():
    d = sys.argv[1]; ids = IDS
    if "--ids" in sys.argv: ids = {int(x, 0): "" for x in sys.argv[sys.argv.index("--ids") + 1].split(",")}
    a, b = load(d + "/orig.packets.jsonl", ids), load(d + "/d2rs.packets.jsonl", ids)
    print(f"1.14d {len(a)} records, d2rs {len(b)} records")
    for i in range(max(len(a), len(b))):
        x = a[i] if i < len(a) else None; y = b[i] if i < len(b) else None
        same = x is not None and y is not None and x[1:] == y[1:]
        tag = "  " if same and x[0] == y[0] else ("~ " if same else "! ")
        f = lambda t: "-" if t is None else f"f{t[0]} {IDS.get(t[1], hex(t[1]))} {t[2]}"
        print(tag + f(x) + ("" if same else "   ||   " + f(y)))
main()
