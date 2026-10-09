"""Compare two packets recordings item by item IGNORING the creation frame
(q-chk-items-drops): separates "item made one frame late" from "different
item". Prints one line per item pair: = equal, otherwise the first field.

    python3 tools/scenario-diff/items_content.py orig.packets.jsonl d2rs.packets.jsonl
Our own code."""
import os, sys
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import items_diff as I


def load(p):
    return I.items_of(I.packets_diff.load(p)[1])


def main():
    a, _ = load(sys.argv[1]); b, _ = load(sys.argv[2])
    bad = 0
    for n in range(max(len(a), len(b))):
        x = a[n] if n < len(a) else None; y = b[n] if n < len(b) else None
        if not x or not y:
            print("#%d missing on %s" % (n, "d2rs" if not y else "1.14d")); bad += 1; continue
        y = dict(y, frame=x["frame"])
        d = I.compare_item(x, y)
        fr = "" if x["frame"] == b[n]["frame"] else " (frame %d vs %d)" % (x["frame"], b[n]["frame"])
        print("#%d %s %s%s" % (n, I.describe(x), "=" if not d else "DIFF %s" % (d,), fr))
        bad += bool(d)
    print("content: %d items, %d differ" % (max(len(a), len(b)), bad)); sys.exit(1 if bad else 0)


main()
