#!/usr/bin/env python3
"""Compares the items of a save written by d2s-tool with the same save after
1.14d loaded it and saved it again (Save and Exit under Wine), item by item
(code, mode and location as the key, the entry bytes as the value), and
writes the facts TSV (format d2s-roundtrip-1) that tools/d2s-tool's
real-data test reads. Our own code; the rows are our own observation.

  python3 tools/cloud-game/d2s_roundtrip.py OURS.d2s GAME.d2s --args "ARGS" \
      --out facts/saves/NAME.tsv

ARGS is the d2s-tool `new` argument string that made OURS (without -o).
Needs D2_GAME_DIR (d2s-tool reads the tables) and target/release/d2s-tool.
"""
import argparse, os, re, subprocess, sys

FORMAT = "d2s-roundtrip-1"
TOOL = "cloud-game d2s_roundtrip 0.1.0"
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))


def items(path):
    out = subprocess.run([os.path.join(REPO, "target", "release", "d2s-tool"), "dump", path, "--items"],
                         capture_output=True, text=True, check=True).stdout
    res, cur = [], None
    for line in out.split("\n"):
        m = re.match(r"  \[(\d+)\] (\S+)\s+mode (\d) (.*?) \|", line)
        if m:
            cur = f"{m.group(2)} mode {m.group(3)} {m.group(4)}"
            continue
        m = re.match(r"      bytes (\w+)", line)
        if m and cur:
            res.append((cur, m.group(1)))
            cur = None
    return res


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("ours"); ap.add_argument("game")
    ap.add_argument("--args", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--note", default="")
    a = ap.parse_args()
    o, g = items(a.ours), items(a.game)
    gd = dict(g)
    rows = []
    for k, v in o:
        w = gd.get(k)
        rows.append((k, v, w or "-", "equal" if w == v else ("missing" if w is None else "differs")))
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write(f"# format: {FORMAT}\n# tool: {TOOL}\n")
        f.write(f"# command: python3 tools/cloud-game/d2s_roundtrip.py <ours> <game> --args \"{a.args}\" --out {os.path.relpath(a.out, REPO)}\n")
        f.write(f"# d2s-tool new {a.args}\n")
        if a.note:
            f.write(f"# run: {a.note}\n")
        f.write("item\tours\tgame\tresult\n")
        for r in rows:
            f.write("\t".join(r) + "\n")
    n = sum(1 for r in rows if r[3] == "equal")
    print(f"wrote {a.out}: {n} of {len(rows)} equal")


if __name__ == "__main__":
    sys.exit(main())
