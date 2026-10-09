"""Turn one record_anim.py recording (anim-raw-1) into a small facts table:
one row per client update record of the chosen units (and their footstep
and leap-start records), in recording order.

    python3 tools/trace-recorder/anim_facts.py RAW.jsonl --out facts/client/anim/NAME.tsv
        [--me] [--unit T:GUID ...] [--from C] [--to C] [--note TEXT]

Columns: k (upd / fs / leap), C (client update counter [0x7A0498]), tick
(last server tick), t, c, g (type, class, GUID), m (mode), f (+0x44), F
(+0x48), s (+0x4C, i16), e (+0x4E), st (+0x84), px, py (path +0x00 /
+0x04, 16.16 subtiles), mfl, mn (motion record flags, ticks left; `-`
when none), ox, oy, oz (motion offsets). Measurements only (no game data).
Format `anim-facts-1`; `--selftest` checks the converter on a built input.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import json
import os
import sys

TOOL = "trace-recorder anim_facts 0.1.0"
FORMAT = "anim-facts-1"
COLS = ["k", "C", "tick", "t", "c", "g", "m", "f", "F", "s", "e", "st", "px", "py",
        "mfl", "mn", "ox", "oy", "oz"]


def rows(records, me=False, units=(), lo=None, hi=None):
    tick, out = None, []
    for r in records:
        k = r.get("k")
        if k == "tick":
            tick = r["f"]
            continue
        if k not in ("upd", "fs", "leap"):
            continue
        u = r.get("u") if k == "leap" else r
        if not u:
            continue
        keep = (me and u.get("me")) or (u["t"], u["g"]) in units
        if k == "fs" and not keep:
            # fs records carry no `me`: match the player by its key
            keep = (0, u["g"]) in units or (me and u["t"] == 0)
        c = r.get("C", u.get("C"))
        if not keep or (lo is not None and c < lo) or (hi is not None and c > hi):
            continue
        mo = u.get("mo")
        p = u.get("p") or ["-", "-"]
        out.append([k, c, tick, u["t"], u["c"], u["g"], u["m"], u["f"], u["F"], u["s"], u["e"],
                    u["st"], p[0], p[1], mo["fl"] if mo else "-", mo["n"] if mo else "-",
                    *(mo["o"] if mo else ["-", "-", "-"])])
    return out


def write(path, header, cmd, note, table):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(f"# format {FORMAT}\n# tool {TOOL}\n")
        f.write(f"# recorder {header.get('tool')} ({header.get('format')}), Game.exe sha256 "
                f"{header.get('game_exe_sha256')}, args {' '.join(header.get('args', []))}\n")
        f.write(f"# command: {cmd}\n")
        if note:
            f.write(f"# note: {note}\n")
        f.write("\t".join(COLS) + "\n")
        for r in table:
            f.write("\t".join(str(x) for x in r) + "\n")


def selftest():
    recs = [{"k": "header"}, {"k": "tick", "f": 7},
            {"k": "upd", "t": 0, "c": 4, "g": 1, "m": 18, "f": 256, "F": 3584, "s": 256, "e": 0,
             "st": 0, "C": 9, "p": [1, 2], "me": 1,
             "mo": {"fl": 2, "n": 13, "o": [0, 0, -20]}},
            {"k": "upd", "t": 1, "c": 5, "g": 9, "m": 2, "f": 0, "F": 2048, "s": 96, "e": 0,
             "st": 0, "C": 9, "p": [3, 4]},
            {"k": "fs", "t": 0, "c": 4, "g": 1, "m": 3, "f": 0, "F": 2048, "s": 1, "e": 0,
             "st": 0, "C": 10}]
    got = rows(recs, me=True)
    assert [r[0] for r in got] == ["upd", "fs"], got
    assert got[0][1:3] == [9, 7] and got[0][-3:] == [0, 0, -20] and got[0][14:16] == [2, 13]
    assert rows(recs, units={(1, 9)})[0][5] == 9
    assert rows(recs, me=True, lo=10)[0][0] == "fs"
    print("selftest ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw", nargs="?")
    ap.add_argument("--out")
    ap.add_argument("--me", action="store_true", help="the local player")
    ap.add_argument("--unit", action="append", default=[], help="T:GUID (repeatable)")
    ap.add_argument("--from", dest="lo", type=int)
    ap.add_argument("--to", dest="hi", type=int)
    ap.add_argument("--note", default="")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    recs = [json.loads(line) for line in open(a.raw, encoding="utf-8")]
    units = {tuple(int(x) for x in u.split(":")) for u in a.unit}
    table = rows(recs, a.me, units, a.lo, a.hi)
    cmd = "python3 tools/trace-recorder/anim_facts.py <raw> " + " ".join(
        a for a in sys.argv[2:] if not a.startswith("<"))
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    write(a.out, recs[0], cmd, a.note, table)
    print(f"wrote {a.out}: {len(table)} rows")


if __name__ == "__main__":
    main()
