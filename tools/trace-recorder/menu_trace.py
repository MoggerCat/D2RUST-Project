"""Front-end menu runs (autostart.py --menu) -> a format-1 trace.

Reads the run's log (`stdout.clean.txt` of tools/cloud-game/run.sh, or
the recorder's own stdout), keeps the autostart lines the script logged
(`state`, `wstr`, `close`) as `expected` events in order, and the input
script as `inputs`. With `--band X0 Y0 X1 Y1` it also measures every
`shot` PNG: the mean luma of that rectangle (integer, x1000) and the
number of pixels differing by more than 40 from the shot named by
`--band-ref`, and the counts of strongly red / green pixels (a hovered
label's colour), so a hover text's presence is a tool result, not a look.
The PNGs themselves (Blizzard art) are never written into the trace.

  python3 tools/trace-recorder/menu_trace.py --log RUN/stdout.clean.txt \\
      --script FILE --id frontend-0001 --behavior frontend-menus \\
      --spec specs/ui/frontend-menus.md --notes "..." --out traces/...json \\
      [--shots RUN/shots --band 300 60 500 140 --band-ref base]

Our own code (standard library; Pillow for --band).
"""

import argparse
import json
import os
import re
import sys

TOOL = "trace-recorder menu_trace 0.4.0"
LINE = re.compile(r"^autostart: (state|wstr|WM_CLOSE posted)(.*)$")


def parse_log(text):
    out = []
    for raw in text.splitlines():
        m = LINE.match(raw.strip())
        if not m:
            continue
        kind, rest = m.group(1), m.group(2).strip()
        if kind == "state":
            g = re.match(r"(\S*)\s*at ([\d.]+)s: launcher mode (\S+), player level (\S+)", rest)
            if g:
                out.append({"kind": "state", "data": {
                    "label": g.group(1), "launcher_mode": _num(g.group(3)),
                    "player_level": _num(g.group(4))}})
        elif kind == "wstr":
            g = re.match(r"(\S*)\s*\[(0x[0-9a-f]+)\]\+(0x[0-9a-f]+) = (.*)$", rest)
            if g:
                out.append({"kind": "wstr", "data": {
                    "label": g.group(1), "ptr": g.group(2), "off": g.group(3),
                    "text": json.loads(g.group(4))}})
        else:
            out.append({"kind": "close", "data": {}})
    return out


def _num(s):
    return None if s == "None" else int(s)


def band_stats(shots, box, ref):
    from PIL import Image
    x0, y0, x1, y1 = box

    def px(name):
        im = Image.open(os.path.join(shots, name + ".png")).convert("L").crop((x0, y0, x1, y1))
        return list(im.tobytes())
    base = px(ref) if ref else None
    out = []
    for f in sorted(os.listdir(shots)):
        if not f.endswith(".png"):
            continue
        name = f[:-4]
        p = px(name)
        e = {"shot": name, "mean_luma_x1000": sum(p) * 1000 // len(p)}
        rgb = Image.open(os.path.join(shots, f)).convert("RGB").crop((x0, y0, x1, y1)).tobytes()
        px3 = [rgb[i:i + 3] for i in range(0, len(rgb), 3)]
        e["red_px"] = sum(1 for r, g, b in px3 if r > g + 80 and r > b + 80)
        e["green_px"] = sum(1 for r, g, b in px3 if g > r + 80 and g > b + 80)
        if base is not None:
            e["diff_px_vs_ref"] = sum(1 for a, b in zip(p, base) if abs(a - b) > 40)
        out.append({"kind": "band", "data": e})
    return out


def d2s_fields(paths):
    """Header facts of saves the run wrote (`formats/d2s.md` §2): name,
    status +0x24, town bytes +0xA8..+0xAA, map seed +0xAB."""
    import struct
    out = []
    for p in paths:
        d = open(p, "rb").read()
        out.append({"kind": "d2s", "data": {
            "file": os.path.basename(p), "size": len(d), "status": d[0x24],
            "towns": list(d[0xA8:0xAB]), "map_seed": f"{struct.unpack_from('<I', d, 0xAB)[0]:#010x}"}})
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--log", required=True)
    ap.add_argument("--script", required=True, help="file holding the input script")
    ap.add_argument("--id", required=True)
    ap.add_argument("--behavior", required=True)
    ap.add_argument("--spec", required=True)
    ap.add_argument("--date", required=True)
    ap.add_argument("--notes", default="")
    ap.add_argument("--command", default="", help="the recording command, for the record")
    ap.add_argument("--shots", default=None)
    ap.add_argument("--band", type=int, nargs=4, action="append", default=None,
                    help="a rectangle to measure; may be given more than once")
    ap.add_argument("--band-ref", default=None)
    ap.add_argument("--game-args", default="-w -ns", help="the game arguments of the run")
    ap.add_argument("--d2s", nargs="*", default=[], help="saves the run wrote: header facts")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    script = open(a.script, encoding="utf-8").read().replace("\n", "")
    expected = parse_log(open(a.log, encoding="utf-8", errors="replace").read())
    expected += d2s_fields(a.d2s)
    if a.band:
        for i, box in enumerate(a.band):
            for e in band_stats(a.shots, box, a.band_ref):
                e["data"]["band"] = i
                expected.append(e)
    tr = {
        "format_version": 1, "id": a.id, "game_version": "1.14d", "area": "frontend",
        "behavior": a.behavior, "spec": a.spec,
        "recorded": {"date": a.date, "method": "memory-read", "tool": TOOL + " / autostart 0.2.0",
                     "by": "claude (cloud, Wine)", "command": a.command},
        "setup": {"game_args": a.game_args.split(), "size": [800, 600], "menu_mode": True,
                  "band": a.band, "band_ref": a.band_ref},
        "inputs": [{"tick": 0, "kind": "script", "data": {"text": script}}],
        "expected": [{"tick": 0, **e} for e in expected],
        "compare": {"mode": "exact", "ignore": []},
        "notes": a.notes,
    }
    os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        json.dump(tr, f, indent=1, sort_keys=True, ensure_ascii=False)
        f.write("\n")
    print(f"{a.out}: {len(expected)} events")


if __name__ == "__main__":
    sys.exit(main())
