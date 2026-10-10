#!/usr/bin/env python3
"""Map an rng draw site or first-difference address to its 1.14d function.

  site2fn.py <0xADDR> [--depth N]
  site2fn.py --selftest

Prints: the containing function (entry, name, offset), its callers up to
depth 2 (indented), and the spec files in specs/ that cite the function
entry or the address itself. Reads the private re/ like lookup.py.
"""
import os
import re
import signal
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lookup  # noqa: E402

REPO = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../.."))


def spec_cites(specs_dir, addrs):
    pats = [re.compile(r"(?i)\b0x0*%x\b" % a) for a in addrs]
    out = {}
    for dp, _, fns in os.walk(specs_dir):
        for fn in sorted(fns):
            if not fn.endswith(".md"):
                continue
            p = os.path.join(dp, fn)
            with open(p, encoding="utf-8", errors="replace") as f:
                for i, ln in enumerate(f, 1):
                    for a, pat in zip(addrs, pats):
                        if pat.search(ln):
                            out.setdefault(os.path.relpath(p, specs_dir), []).append((i, a))
    return out


def tree(r, fn, depth, indent, seen, out):
    if depth == 0:
        return
    for c in sorted(r.callers(fn)):
        out.append("  " * indent + lookup.line(r, c, "\tcaller") if c in r.fns else "  " * indent + lookup.fmt(c))
        if c not in seen:
            seen.add(c)
            tree(r, c, depth - 1, indent + 1, seen, out)


def site2fn(r, addr, depth, specs_dir):
    e = r.containing(addr)
    if e is None:
        return ["no function contains " + lookup.fmt(addr)]
    out = [lookup.line(r, e, "\t+0x%x" % (addr - e))]
    tree(r, e, depth, 1, {e}, out)
    for rel, hits in sorted(spec_cites(specs_dir, [e, addr]).items()):
        out.append("spec\tspecs/%s\t%s" % (rel, " ".join("L%d" % i for i, _ in hits[:6])))
    return out


def main(argv):
    if "--selftest" in argv:
        return selftest()
    depth = 2
    args = []
    it = iter(argv)
    for a in it:
        if a == "--depth":
            depth = int(next(it))
        else:
            args.append(a)
    addr = lookup.parse_addr(args[0]) if args else None
    if addr is None:
        print(__doc__)
        return 2
    for ln in site2fn(lookup.Re(lookup.re_root()), addr, depth, os.path.join(REPO, "specs")):
        print(ln)
    return 0


def selftest():
    with tempfile.TemporaryDirectory() as d:
        os.makedirs(os.path.join(d, "re"))
        lookup.make_fixture(os.path.join(d, "re"))
        sp = os.path.join(d, "specs", "x")
        os.makedirs(sp)
        with open(os.path.join(sp, "a.md"), "w") as f:
            f.write("text\nmatches 0x00401000 here\nnot 0x4010000\n")
        out = site2fn(lookup.Re(os.path.join(d, "re")), 0x40101A, 2, os.path.join(d, "specs"))
        assert out[0].startswith("0x00401000\tRNG_Roll\t32\t+0x1a"), out
        assert "AI_Think" in out[1] and "Main" in out[2], out
        assert out[-1] == "spec\tspecs/x/a.md\tL2", out
    print("site2fn selftest ok")
    return 0


if __name__ == "__main__":
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)
    sys.exit(main(sys.argv[1:]))
