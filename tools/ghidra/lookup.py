#!/usr/bin/env python3
"""Find 1.14d Game.exe functions in the private repo's re/ exports.

  lookup.py <name-part>          functions whose name contains it
  lookup.py <0xADDR>             containing function of any address
  lookup.py --callers <q>        callers of the function q (name or address)
  lookup.py --callees <q>        callees of the function q
  lookup.py --str <text>         strings containing text and the functions using them
  lookup.py --selftest           no private files needed

One line per hit, tab separated. Reads $D2_RE or ../d2rust-private-repo/re
(exports-typed/ preferred, exports/ fallback; exports/names.tsv overrides
names). Read-only; prints addresses and names only, no decompiled text.
"""
import bisect
import os
import sys
import signal
import tempfile


def re_root():
    p = os.environ.get("D2_RE")
    if p:
        return p
    here = os.path.dirname(os.path.abspath(__file__))
    for c in ("../../../d2rust-private-repo/re", "/home/user/d2rust-private-repo/re"):
        q = os.path.normpath(os.path.join(here, c))
        if os.path.isdir(q):
            return q
    return os.path.normpath(os.path.join(here, "../../../d2rust-private-repo/re"))


def rows(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        next(f, None)
        for line in f:
            line = line.rstrip("\n")
            if line:
                yield line.split("\t")


def parse_addr(s):
    s = s.strip()
    try:
        return int(s, 16) if s.lower().startswith("0x") else None
    except ValueError:
        return None


def fmt(a):
    return "0x%08X" % a


class Re:
    def __init__(self, root):
        self.root = root
        base = os.path.join(root, "exports-typed")
        if not os.path.isfile(os.path.join(base, "functions.tsv")):
            base = os.path.join(root, "exports")
        self.base = base
        self.fns = {}  # entry -> (name, size)
        for r in rows(os.path.join(base, "functions.tsv")):
            self.fns[int(r[0], 16)] = (r[1], int(r[2] or 0))
        names = os.path.join(root, "exports", "names.tsv")
        if os.path.isfile(names):
            for r in rows(names):
                a = parse_addr(r[0])
                if a in self.fns:
                    self.fns[a] = (r[1], self.fns[a][1])
        self.entries = sorted(self.fns)
        self._calls = None

    def containing(self, addr):
        i = bisect.bisect_right(self.entries, addr) - 1
        if i < 0:
            return None
        e = self.entries[i]
        size = self.fns[e][1]
        return e if addr < e + max(size, 1) else None

    def name(self, a):
        return self.fns.get(a, ("?", 0))[0]

    def calls(self):
        if self._calls is None:
            self._calls = list(rows(os.path.join(self.base, "index", "calls.tsv")))
        return self._calls

    def callers(self, fn):
        out = {}
        for r in self.calls():
            if parse_addr(r[4]) == fn and parse_addr(r[1]) is not None:
                out.setdefault(int(r[1], 16), []).append(int(r[0], 16))
        return out  # caller entry -> call sites

    def callees(self, fn):
        out = {}
        for r in self.calls():
            if parse_addr(r[1]) == fn and parse_addr(r[4]) is not None:
                out.setdefault(int(r[4], 16), []).append(int(r[0], 16))
        return out

    def resolve(self, q):
        a = parse_addr(q)
        if a is not None:
            return self.containing(a)
        ql = q.lower()
        for e in self.entries:
            if self.fns[e][0].lower() == ql:
                return e
        return None

    def strings(self, text):
        t = text.lower()
        hits = {}
        for r in rows(os.path.join(self.base, "index", "strings.tsv")):
            if t in r[2].lower():
                hits[int(r[0], 16)] = r[2]
        users = {}
        for r in rows(os.path.join(self.base, "index", "datarefs.tsv")):
            a = parse_addr(r[3])
            if a in hits:
                users.setdefault(a, set()).add(int(r[1], 16))
        return hits, users


def line(re_, e, extra=""):
    return "%s\t%s\t%d%s" % (fmt(e), re_.name(e), re_.fns[e][1], extra)


def main(argv):
    if "--selftest" in argv:
        return selftest()
    args = [a for a in argv if not a.startswith("--")]
    mode = next((a for a in argv if a in ("--callers", "--callees", "--str")), None)
    if not args:
        print(__doc__)
        return 2
    r = Re(re_root())
    q = " ".join(args) if mode == "--str" else args[0]
    if mode == "--str":
        hits, users = r.strings(q)
        for a in sorted(hits):
            fs = sorted(users.get(a, ()))
            print("%s\t%r\t%s" % (fmt(a), hits[a], " ".join(fmt(f) + ":" + r.name(f) for f in fs) or "-"))
        return 0
    if mode:
        e = r.resolve(q)
        if e is None:
            print("no such function: " + q, file=sys.stderr)
            return 1
        m = r.callers(e) if mode == "--callers" else r.callees(e)
        for k in sorted(m):
            print(line(r, k, "\tsites " + " ".join(fmt(s) for s in sorted(m[k]))) if k in r.fns else fmt(k))
        return 0
    a = parse_addr(q)
    if a is not None:
        e = r.containing(a)
        if e is None:
            print("no function contains " + q, file=sys.stderr)
            return 1
        print(line(r, e, "\t+0x%x" % (a - e)))
        for k in sorted(r.callers(e)):
            print(line(r, k, "\tcaller"))
        for k in sorted(r.callees(e)):
            print(line(r, k, "\tcallee") if k in r.fns else fmt(k) + "\tcallee")
        return 0
    ql = q.lower()
    n = 0
    for e in r.entries:
        if ql in r.fns[e][0].lower():
            print(line(r, e))
            n += 1
    return 0 if n else 1


def make_fixture(root):
    ex = os.path.join(root, "exports")
    os.makedirs(os.path.join(ex, "index"))
    with open(os.path.join(ex, "functions.tsv"), "w") as f:
        f.write("entry\tname\tsize\tcallers\n0x00401000\tFUN_00401000\t32\t1\n"
                "0x00401100\tAI_Think\t64\t1\n0x00401200\tMain\t100\t0\n")
    with open(os.path.join(ex, "names.tsv"), "w") as f:
        f.write("address\tname\tsource\tevidence\n0x00401000\tRNG_Roll\ts\te\n")
    with open(os.path.join(ex, "index", "calls.tsv"), "w") as f:
        f.write("site\tfrom_fn\tfrom_name\tto_addr\tto_fn\tto_name\n"
                "0x00401120\t0x00401100\tx\t0x00401000\t0x00401000\tx\n"
                "0x00401210\t0x00401200\tx\t0x00401100\t0x00401100\tx\n")
    with open(os.path.join(ex, "index", "strings.tsv"), "w") as f:
        f.write("addr\tlength\tvalue\n0x00500000\t5\thello\n")
    with open(os.path.join(ex, "index", "datarefs.tsv"), "w") as f:
        f.write("site\tfrom_fn\tref_type\tto_addr\tto_label\tto_type\tto_value\n"
                "0x00401230\t0x00401200\tDATA\t0x00500000\tx\tx\t\n")


def selftest():
    with tempfile.TemporaryDirectory() as d:
        make_fixture(d)
        r = Re(d)
        assert r.name(0x401000) == "RNG_Roll"
        assert r.containing(0x40101F) == 0x401000
        assert r.containing(0x401020) is None
        assert r.containing(0x401130) == 0x401100
        assert list(r.callers(0x401000)) == [0x401100]
        assert list(r.callees(0x401200)) == [0x401100]
        assert r.resolve("ai_think") == 0x401100 and r.resolve("0x401210") == 0x401200
        h, u = r.strings("ELL")
        assert h == {0x500000: "hello"} and u[0x500000] == {0x401200}
    print("lookup selftest ok")
    return 0


if __name__ == "__main__":
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    sys.exit(main(sys.argv[1:]))
