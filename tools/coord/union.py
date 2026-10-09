# usage: union.py FILE...          — resolve every conflict block as HEAD side then theirs side
#        union.py --index FILE...  — spec index conflicts: take the HEAD side of every block, but only
#                                    when both sides are spec index lines (exit 1 and leave the file
#                                    untouched otherwise); then run tools/spec_index.py
# union: lines of theirs already on the HEAD side of the same block are dropped (append-only docs
# where both sides appended the same line); a diff3 base section (|||||||) is dropped.
import sys, re

BLOCK = re.compile(r"^<{7} [^\n]*\n(.*?)(?:^\|{7}[^\n]*\n.*?)?^={7}\n(.*?)^>{7}(?: [^\n]*)?\n", re.S | re.M)


def index_line(l):
    return l == "" or l.startswith("|") or l in ("<!-- index -->", "<!-- /index -->")


def in_index(s, m):
    """The block sits in a spec's <!-- index --> table and holds only its lines."""
    before = s[:m.start()]
    inside = before.rfind("<!-- index -->") > before.rfind("<!-- /index -->") or "<!-- index -->" in m.group(1)
    return inside and all(index_line(l) for g in (1, 2) for l in m.group(g).split("\n"))


def union(m):
    ours, theirs = m.group(1), m.group(2)
    have = {l for l in ours.split("\n") if l.strip()}
    keep = [l for l in theirs.splitlines(True) if not (l.strip() and l.rstrip("\n") in have)]
    return ours + "".join(keep)


def main(argv):
    index = argv[:1] == ["--index"]
    bad = 0
    for p in argv[1:] if index else argv:
        s = open(p, encoding="utf-8").read()
        if index:
            blocks = list(BLOCK.finditer(s))
            if not all(in_index(s, m) for m in blocks):
                print(p, "not an index-only conflict")
                bad = 1
                continue
            s, n = BLOCK.subn(lambda m: m.group(1), s)
        else:
            s, n = BLOCK.subn(union, s)
        open(p, "w", encoding="utf-8").write(s)
        print(p, n)
    return bad


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
