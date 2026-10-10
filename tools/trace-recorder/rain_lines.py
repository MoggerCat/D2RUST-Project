#!/usr/bin/env python3
"""Rain lines of several record_frames.py runs, side by side.

Reads frames-raw-3 recordings made with `--every 1 --draws-every 1` and writes
one TSV (format rain-lines-1): for every drawn frame of every run, the client
seed before and after the frame, the number of seed steps between them, and
each pass-9 particle line (DrawLine called from 0x0047368E rain / 0x00473585
snow; specs/tools/facts-render.md §6 r5, specs/render/draw-order-2.md §11.7).
The header says which runs are equal frame by frame and where the others part.

    py tools/trace-recorder/rain_lines.py --out traces/pc1/rain-lines-a1-town.tsv \
        --frames 28 run1=a.jsonl run2=b.jsonl
"""
import argparse
import json
import sys

VERSION = "0.2.0"
FORMAT = "rain-lines-1"
SITES = {"0x47368e": "rain", "0x473585": "snow"}
MUL = 0x6AC690C5


def steps(start, end, limit=4000):
    lo, hi = start
    for n in range(limit + 1):
        if [lo, hi] == list(end):
            return n
        v = lo * MUL + hi
        lo, hi = v & 0xFFFFFFFF, (v >> 32) & 0xFFFFFFFF
    return None


def load(path, frames):
    header, out = None, []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            rec = json.loads(line)
            if rec.get("k") == "header":
                header = rec
            if rec.get("k") != "frame" or rec.get("draws") is None:
                continue
            lines = [(SITES[d["at"]],) + tuple(d["a"]) for d in rec["draws"]
                     if d.get("op") == "DrawLine" and d.get("at") in SITES]
            out.append({
                "seq": rec["seq"], "f": rec["f"], "update": rec["client_update"],
                "start": rec["seed_start"], "end": rec["seed_end"],
                "level": rec["level"]["level_id"], "lines": lines,
                "cursor": rec["cursor"]["state"],
            })
            if len(out) >= frames:
                break
    return header, out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("runs", nargs="+", metavar="NAME=FILE")
    ap.add_argument("--out", required=True)
    ap.add_argument("--frames", type=int, default=28)
    ap.add_argument("--note", default="")
    args = ap.parse_args()

    runs = []
    for spec in args.runs:
        name, _, path = spec.partition("=")
        header, frames = load(path, args.frames)
        if not frames:
            sys.exit(f"{path}: no frame with a draw log")
        runs.append((name, header, frames))

    n = min(len(fr) for _, _, fr in runs)
    key = lambda fr: [(x["start"], x["end"], x["lines"]) for x in fr[:n]]
    groups = []
    for name, _, fr in runs:
        for g in groups:
            if key(g[1]) == key(fr):
                g[0].append(f"{name}@{fr[0]['update']}")
                break
        else:
            groups.append(([f"{name}@{fr[0]['update']}"], fr))

    with open(args.out, "w", encoding="utf-8", newline="\n") as out:
        h0 = runs[0][1] or {}
        out.write(f"# format {FORMAT}\n")
        out.write(f"# tool trace-recorder rain_lines {VERSION}\n")
        out.write(f"# recorder {h0.get('tool')} ({h0.get('format')}), Game.exe sha256 "
                  f"{h0.get('game_exe_sha256')}, args {' '.join(h0.get('args', []))}\n")
        out.write("# command: py tools/trace-recorder/rain_lines.py --frames "
                  f"{args.frames} --out {args.out} " + " ".join(r[0] + "=<frames-raw-3>" for r in runs) + "\n")
        if args.note:
            out.write(f"# note: {args.note}\n")
        out.write(f"# level {runs[0][2][0]['level']}; {len(runs)} runs, first {n} drawn frames compared "
                  "(seed before, seed after, every particle line)\n")
        for names, fr in groups:
            out.write(f"# equal group (run@client update of its first drawn frame): {' '.join(names)}\n")
        for names, fr in groups[1:]:
            base = groups[0][1]
            for a, b in zip(base[:n], fr[:n]):
                if (a["start"], a["end"], a["lines"]) != (b["start"], b["end"], b["lines"]):
                    out.write(f"# {names[0]} parts from {groups[0][0][0]} at drawn frame {b['seq']} "
                              f"(client update {b['update']} against {a['update']}): seed steps "
                              f"{steps(b['start'], b['end'])} against {steps(a['start'], a['end'])}, "
                              f"cursor state {b['cursor']} against {a['cursor']}\n")
                    break
        out.write("# i = -: the frame drew no particle line. steps = raw steps of the player's client seed "
                  "inside the frame; gap = 1 when the seed changed between the last frame's end and this start.\n")
        out.write("run\tseq\tf\tclient_update\tseed_start\tseed_end\tsteps\tgap\ti\tkind\tx0\ty0\tx1\ty1\tcolor\talpha\n")
        for names, fr in groups:
            prev = None
            for x in fr[:n]:
                st = steps(x["start"], x["end"])
                gap = int(prev is not None and prev != x["start"])
                prev = x["end"]
                head = [names[0].split('@')[0], x["seq"], x["f"], x["update"],
                        "%08X:%08X" % tuple(x["start"]), "%08X:%08X" % tuple(x["end"]),
                        "?" if st is None else st, gap, x["cursor"]]
                rows = x["lines"] or [None]
                for i, ln in enumerate(rows):
                    tail = ["-"] * 8 if ln is None else [i, *ln]
                    out.write("\t".join(str(c) for c in head + tail) + "\n")
    print(f"wrote {args.out}: {len(runs)} runs, {len(groups)} distinct, {n} frames")


if __name__ == "__main__":
    main()
