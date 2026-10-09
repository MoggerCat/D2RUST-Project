"""Turn record_objanim.py recordings (objanim-raw-1) into a small facts TSV:
every animation set-up on an object (who called it, seed before / after,
speed), every generic-step wrap or mode change, and every interact range
test with the C->S 0x13 object case. Measurements only (seeds, numbers,
positions, return addresses); runs anywhere (no game, no Windows).

  python3 tools/trace-recorder/objanim_facts.py RAW.jsonl[@kind,..] [RAW2.jsonl ...] --out FILE.tsv
  python3 tools/trace-recorder/objanim_facts.py --selftest

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import json
import sys

TOOL = "trace-recorder objanim_facts 0.1.0"
COLS = ["run", "k", "f", "srv", "cl", "g", "m0", "m1", "sp0", "sp1", "cur", "fc",
        "seed0", "seed1", "drew", "eax", "pos_obj", "pos_player", "caller"]


def rows_of(run, lines):
    out = []
    for line in lines:
        r = json.loads(line)
        k = r.get("k")
        row = dict.fromkeys(COLS, "-")
        row.update(run=run, k=k, f=r.get("f"))
        if k in ("anim", "setmode"):
            b, a = r["before"], r["after"]
            row.update(srv=int(a["srv"]), cl=a["cl"], g=a["g"], m0=b["m"], m1=a["m"],
                       sp0=b["sp"], sp1=a["sp"], cur=a["cur"], fc=a["fc"],
                       seed0="%d,%d" % tuple(b["seed"]), seed1="%d,%d" % tuple(a["seed"]),
                       drew=int(r["drew"]), caller=r["stack"][0])
        elif k in ("init51", "mode0e"):
            u = r["unit"]
            row.update(srv=int(u["srv"]), cl=u["cl"], g=u["g"], m1=u["m"], sp1=u["sp"],
                       cur=u["cur"], fc=u["fc"], seed1="%d,%d" % tuple(u["seed"]),
                       caller=r["stack"][0])
        elif k == "gstep":
            u, last = r["unit"], r["last"]
            if last is None:
                continue
            row.update(srv=int(u["srv"]), cl=u["cl"], g=u["g"], m0=last[0], m1=u["m"],
                       sp1=u["sp"], cur="%d->%d" % (last[1], u["cur"]), fc=u["fc"])
        elif k in ("range", "op13"):
            objs = [u for u in r["units"] if u["t"] == 2]
            pls = [u for u in r["units"] if u["t"] == 0]
            if objs:
                row.update(cl=objs[0]["cl"], g=objs[0]["g"], pos_obj="%d,%d" % tuple(objs[0]["pos"]))
            if pls:
                p = pls[0]["pos"]
                row.update(srv=int(pls[0]["srv"]), pos_player="%d.%05d,%d.%05d" % (
                    p[0], p[2] * 100000 // 65536, p[1], p[3] * 100000 // 65536))
            if k == "range":
                row.update(eax=r["eax"])
            row.update(caller=r["stack"][0])
        else:
            continue
        out.append(row)
    return out


def write(rows, cmd, f):
    f.write(f"# facts v1; tool: {TOOL}; command: {cmd}; game: 1.14d\n")
    f.write("\t".join(COLS) + "\n")
    for row in rows:
        f.write("\t".join(str(row[c]) for c in COLS) + "\n")


def selftest():
    import io
    lines = [json.dumps({"k": "anim", "f": 2, "before": {"srv": False, "cl": 37, "g": 1, "m": 2,
                         "sp": 0, "cur": 0, "fc": 5120, "seed": [1, 666]},
                         "after": {"srv": False, "cl": 37, "g": 1, "m": 2, "sp": 191, "cur": 0,
                                   "fc": 5120, "seed": [5, 7]}, "drew": True,
                         "stack": ["0x4bc7e6"]}),
             json.dumps({"k": "range", "f": 9, "eax": 1, "stack": ["0x480ce3"], "units": [
                 {"t": 2, "cl": 119, "g": 10, "pos": [4899, 4209]},
                 {"t": 0, "cl": 0, "g": 1, "srv": False, "pos": [4895, 4212, 32768, 0]}]})]
    rows = rows_of("r", lines)
    buf = io.StringIO()
    write(rows, "x", buf)
    text = buf.getvalue().splitlines()
    assert text[2].split("\t")[COLS.index("sp1")] == "191", text
    assert text[3].split("\t")[COLS.index("pos_player")] == "4895.50000,4212.00000", text
    assert text[3].split("\t")[COLS.index("eax")] == "1"
    print("selftest ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw", nargs="*")
    ap.add_argument("--out")
    ap.add_argument("--cmd", default="", help="the recorder command(s), for the header")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    rows = []
    for i, spec in enumerate(a.raw):
        p, _, kinds = spec.partition("@")
        with open(p, encoding="utf-8") as f:
            got = rows_of(f"r{i + 1}", f)
        rows += [r for r in got if not kinds or r["k"] in kinds.split(",")]
    cmd = (a.cmd + "; " if a.cmd else "") + "python3 tools/trace-recorder/objanim_facts.py " + \
        " ".join(f"r{i + 1}.jsonl" + ("@" + x.partition("@")[2] if "@" in x else "")
                 for i, x in enumerate(a.raw))
    with (open(a.out, "w", encoding="utf-8", newline="\n") if a.out else sys.stdout) as f:
        write(rows, cmd, f)


if __name__ == "__main__":
    main()
