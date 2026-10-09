#!/usr/bin/env python3
"""Finds, per shrines.txt row, a `-seed` and an objects row for which d2rs'
shrine pick (world/objects.md section 5.1) gives that row, and writes
tools/check-gen/shrine-seeds.tsv (the input of the `shrine` family).

Why: the `poke object` directive creates a shrine object whose shrine row
is picked on the object control's seed, which derives from the game seed
(`-seed N`).  A check for "shrine row n" therefore needs a seed that picks
n.  The row is read from the AssignObject message (S2C 0x51, byte 13,
`interact`) d2rs queues for the poked object (`d2-client state-dump
--packets`).  The d2rs pick is a claim: the generated check compares it
with 1.14d (a different row on 1.14d shows as a divergence of the shrine
object's state at the creation frame, the first thing the check reports).

  The checks run in Lut Gholein (level 40, a town: no monsters; level id
  40 is above every LevelMin of shrines.txt, so the pick never retries).

  objects Parm0 1 -> shrine class 2 -> row 2 (4 remaps to 2)
  objects Parm0 2 -> shrine class 3 -> row 3 (5 remaps to 3)
  objects Parm0 3 -> class 1 (1 in 10: rows 17-22, 16 -> 18) or class 4
                     (rows 1, 6-15): needs a seed search

Usage: find_shrine_seeds.py [--excel DIR] [--max-seeds N] [--jobs N]
Needs target/release/d2-client and d2s-tool (cargo build --release).
"""
import argparse
import csv
import json
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
REACHABLE = [1, 2, 3] + list(range(6, 16)) + list(range(17, 23))


def objects_by_parm0(excel):
    with open(os.path.join(excel, "objects.txt"), encoding="latin-1", newline="") as f:
        rows = list(csv.reader(f, delimiter="\t"))
    h = {n: k for k, n in enumerate(rows[0])}
    out = {}
    for k, r in enumerate(rows[1:]):
        if r[h["InitFn"]] == "1" and r[h["OperateFn"]] == "2" and r[h["Parm0"]] in "123":
            out.setdefault(r[h["Parm0"]], int(r[h["Id"]]))  # first class of each kind
    return out


def pick_for(save, cls, seed, tmp):
    pk = os.path.join(tmp, f"pk-{cls}-{seed}.jsonl")
    out = os.path.join(tmp, f"st-{cls}-{seed}.jsonl")
    cmd = [os.path.join(ROOT, "target", "release", "d2-client"), "state-dump", "--save", save,
           "--seed", str(seed), "--ticks", "34", "--out", out, "--packets", pk,
           "--poke", f"30 object {cls} @x+3 @y"]
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
    if r.returncode != 0:
        raise SystemExit(f"d2-client failed: {r.stderr[-400:]}")
    got = None
    with open(pk) as f:
        for line in f:
            if '"bytes": "51' not in line and '"bytes":"51' not in line:
                continue
            rec = json.loads(line)
            b = bytes.fromhex(rec.get("bytes", ""))
            if len(b) == 14 and b[0] == 0x51 and b[1] == 2 and int.from_bytes(b[6:8], "little") == cls:
                got = b[13]
    for p in (pk, out):
        try:
            os.remove(p)
        except OSError:
            pass
    return got


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--excel")
    ap.add_argument("--max-seeds", type=int, default=400)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--out", default=os.path.join(HERE, "shrine-seeds.tsv"))
    a = ap.parse_args()
    excel = a.excel or os.path.join(os.environ["D2_GAME_DIR"], "extracted", "patch_d2",
                                    "data", "global", "excel")
    objs = objects_by_parm0(excel)
    tmp = tempfile.mkdtemp(prefix="shrine-seeds-")
    save = os.path.join(tmp, "ScnAma.d2s")
    subprocess.run([os.path.join(ROOT, "target", "release", "d2s-tool"), "new", "--name", "ScnAma",
                    "--class", "ama", "--expansion", "--level", "30", "--act", "1", "--quests",
                    "acts=1", "-o", save],
                   check=True, cwd=ROOT)
    found = {}
    for want, parm in ((2, "1"), (3, "2")):
        seed = 1234
        got = pick_for(save, objs[parm], seed, tmp)
        print(f"row {want}: objects {objs[parm]} (Parm0 {parm}) seed {seed} -> {got}")
        if got == want:
            found[want] = (seed, objs[parm], "d2rs: objects Parm0 %s" % parm)
    cls = objs["3"]
    todo = [n for n in REACHABLE if n not in found]
    with ThreadPoolExecutor(a.jobs) as ex:
        seed = 1
        while todo and seed <= a.max_seeds:
            batch = list(range(seed, seed + a.jobs * 4))
            seed += len(batch)
            for s, got in zip(batch, ex.map(lambda s: pick_for(save, cls, s, tmp), batch)):
                if got in todo:
                    found[got] = (s, cls, "d2rs: objects Parm0 3, seed search")
                    todo.remove(got)
                    print(f"row {got}: seed {s}")
    with open(a.out, "w", newline="\n") as f:
        f.write("shrine\tseed\tobject_class\tverified\n")
        for n in sorted(found):
            s, c, how = found[n]
            f.write(f"{n}\t{s}\t{c}\t{how}\n")
    print(f"wrote {len(found)} rows to {a.out}; not found: {todo}")
    return 0 if not todo else 1


if __name__ == "__main__":
    sys.exit(main())
