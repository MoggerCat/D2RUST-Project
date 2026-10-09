"""Read the text a 1.14d frame draws from one record_frames.py capture
(frames-raw-3 with draw lists): every glyph draw (`CelDrawColor`, the
glyph draw of `ui/text.md` §3 r1) of the chosen frame, grouped into lines
by pen y and ordered by pen x. Latin fonts look glyphs up by position
(`ui/text.md` §2: record number = the character code), so the cel frame
is the character.

    python3 tools/trace-recorder/panel_text.py RAW.jsonl [--frame SEQ] --out facts/client/ui/NAME.tsv
        [--y-min Y] [--y-max Y] [--x-max X] [--note TEXT]

Columns: y (pen y), x0, x1 (first / last glyph pen x), color (the draw's
color argument), font (the glyph cel file pointer, as an index of first
appearance), text. Measurements only. Format `panel-text-1`;
`--selftest` checks the grouping on a built input.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import json
import os
import sys

TOOL = "trace-recorder panel_text 0.1.0"
FORMAT = "panel-text-1"


def lines(frame, y_min=None, y_max=None, x_max=None):
    files, rows = {}, {}
    for d in frame.get("draws", []):
        cel = d.get("cel")
        if d.get("op") != "CelDrawColor" or not cel:
            continue
        x, y, color = d["a"][0], d["a"][1], d["a"][4] if len(d["a"]) > 4 else None
        if (y_min is not None and y < y_min) or (y_max is not None and y > y_max) \
                or (x_max is not None and x > x_max):
            continue
        font = files.setdefault(cel.get("file"), len(files))
        rows.setdefault((y, color, font), []).append((x, cel.get("frame", 0)))
    out = []
    for (y, color, font), g in sorted(rows.items()):
        g.sort()
        # split a row where two glyph runs are far apart (two labels on one y)
        run = [g[0]]
        for a, b in zip(g, g[1:]):
            if b[0] - a[0] > 24:
                out.append(row(y, color, font, run))
                run = []
            run.append(b)
        out.append(row(y, color, font, run))
    return out


def row(y, color, font, run):
    text = "".join(chr(c) if 32 <= c < 256 else "?" for _, c in run)
    return [y, run[0][0], run[-1][0], color, font, text]


def selftest():
    f = {"draws": [{"op": "CelDrawColor", "a": [20, 10, -1, 5, 0], "cel": {"frame": 66, "file": "a"}},
                   {"op": "CelDrawColor", "a": [10, 10, -1, 5, 0], "cel": {"frame": 65, "file": "a"}},
                   {"op": "CelDrawColor", "a": [90, 10, -1, 5, 0], "cel": {"frame": 67, "file": "a"}},
                   {"op": "CelDraw", "a": [0, 0, 0, 0], "cel": {"frame": 1}},
                   {"op": "CelDrawColor", "a": [10, 30, -1, 5, 1], "cel": {"frame": 49, "file": "b"}}]}
    got = lines(f)
    assert got == [[10, 10, 20, 0, 0, "AB"], [10, 90, 90, 0, 0, "C"], [30, 10, 10, 1, 1, "1"]], got
    assert lines(f, y_min=20) == [[30, 10, 10, 1, 0, "1"]]
    print("selftest ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw", nargs="?")
    ap.add_argument("--frame", type=int, default=None, help="capture seq (default: the last with draws)")
    ap.add_argument("--out")
    ap.add_argument("--y-min", type=int)
    ap.add_argument("--y-max", type=int)
    ap.add_argument("--x-max", type=int)
    ap.add_argument("--note", default="")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    recs = [json.loads(line) for line in open(a.raw, encoding="utf-8")]
    frames = [r for r in recs if r.get("k") == "frame" and r.get("draws")]
    f = next(r for r in frames if r["seq"] == a.frame) if a.frame is not None else frames[-1]
    table = lines(f, a.y_min, a.y_max, a.x_max)
    hdr = recs[0]
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    with open(a.out, "w", encoding="utf-8", newline="\n") as o:
        o.write(f"# format {FORMAT}\n# tool {TOOL}\n")
        o.write(f"# recorder {hdr.get('tool')} ({hdr.get('format')}), Game.exe sha256 "
                f"{hdr.get('game_exe_sha256')}, args {' '.join(hdr.get('args', []))}, frame {f['seq']}\n")
        o.write("# command: python3 tools/trace-recorder/panel_text.py <raw> " + " ".join(sys.argv[2:]) + "\n")
        if a.note:
            o.write(f"# note: {a.note}\n")
        o.write("y\tx0\tx1\tcolor\tfont\ttext\n")
        for r in table:
            o.write("\t".join(str(x) for x in r) + "\n")
    print(f"wrote {a.out}: {len(table)} lines")


if __name__ == "__main__":
    main()
