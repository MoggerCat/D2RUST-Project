#!/usr/bin/env python3
"""cstate_diff: compare the client unit sets of 1.14d and d2rs (the `cstate`
channel, specs/tools/scenario-diff.md §3, specs/tools/state-snapshot.md §3 r5).

    python3 cstate_diff.py ORIG.cstate.jsonl D2RS.cstate.jsonl [--json FILE] [state_diff options]

Both files are state-1 with `set` S (the units the server announced; paired by
type and GUID) and C (client-only units; their GUIDs are the client's own counter,
so they are paired by type, class and cell, in that order). The comparison is
state_diff.py's, on the fields both headers list; the local player's client seed
`s` is not compared (it follows the wall clock, state-snapshot.md §3 r5).

Our own code. Standard library only.
"""
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "trace-recorder"))
import state_diff  # noqa: E402

C_BASE = 0x40000000


def normalize(path, out):
    """Write `path` to `out` with set C units re-keyed by rank and the local
    player's seed dropped."""
    with open(path, encoding="utf-8") as f, open(out, "w", encoding="utf-8") as o:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            if rec.get("k") == "snap":
                units = rec.get("units", [])
                cs = sorted((u for u in units if u.get("set") == "C"),
                            key=lambda u: (u.get("ut"), u.get("cl"), u.get("x"), u.get("y")))
                for i, u in enumerate(cs):
                    u["g"] = C_BASE + i
                for u in units:
                    if u.get("set", "S") == "S" and u.get("ut") == 0:
                        u.pop("s", None)
                rec["units"] = units
            o.write(json.dumps(rec, separators=(",", ":")) + "\n")


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "--selftest":
        return selftest()
    if len(argv) < 2:
        print("usage: cstate_diff.py ORIG D2RS [--json FILE] [state_diff options]", file=sys.stderr)
        return 3
    orig, d2rs, rest = argv[0], argv[1], argv[2:]
    with tempfile.TemporaryDirectory() as td:
        a, b = os.path.join(td, "a.jsonl"), os.path.join(td, "b.jsonl")
        try:
            normalize(orig, a)
            normalize(d2rs, b)
        except (OSError, ValueError) as e:
            print(f"error: {e}", file=sys.stderr)
            return 3
        code = state_diff.main([a, b] + rest)
    js = None
    if "--json" in rest:
        js = rest[rest.index("--json") + 1]
    if js and os.path.exists(js):
        with open(js, encoding="utf-8") as f:
            d = json.load(f)
        d["channel"] = "cstate"
        d["tool"] = "cstate_diff.py"
        with open(js, "w", encoding="utf-8") as f:
            json.dump(d, f, indent=1)
    return code


def selftest():
    hdr = {"k": "header", "format": "state-1", "side": "x", "fields": ["ut", "g", "cl", "m", "x", "y", "s"], "gaps": []}

    def w(path, units):
        with open(path, "w", encoding="utf-8") as f:
            f.write(json.dumps(hdr) + "\n")
            f.write(json.dumps({"k": "snap", "f": 1, "units": units}) + "\n")
    with tempfile.TemporaryDirectory() as td:
        o, d = os.path.join(td, "o"), os.path.join(td, "d")
        s0 = {"set": "S", "ut": 0, "g": 1, "cl": 1, "m": 1, "x": 5, "y": 6, "s": [1, 2]}
        c1 = {"set": "C", "ut": 1, "g": 2, "cl": 149, "m": 1, "x": 9, "y": 9, "s": [3, 4]}
        w(o, [s0, c1])
        w(d, [dict(s0, s=[7, 8]), dict(c1, g=77)])
        assert main([o, d]) == 0          # seed of the player ignored, C paired by class and cell
        w(d, [dict(s0, s=[7, 8])])
        assert main([o, d]) == 1          # set C missing in d2rs
    print("cstate_diff selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
