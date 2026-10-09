"""Message facts from a record_packets.py raw file (packets-raw-1).

Writes the messages of a game start, in recorded order, as a TSV under
facts/join/: every client->server message the client sent (`client_out`)
and every server->client message queued for a client (`s2c`), from the
start of the recording through server frame --frames (default 2: the
join and the first game frame, `sim/intents-events.md` §8.2); with --from
F only the messages of frames > F (a later window, e.g. item moves after
the join; --frames 0 = to the end), and --skip ID,.. leaves out S→C ids
(e.g. the ping and unit-update traffic of a town). One row per
message: n, frame (empty before the first tick), dir, id, size, caller
(the queueing call site, s2c only) and the message bytes in hex.

The header comment lines carry the format version, this tool, the command
that made the file, and the raw file's header (Game.exe sha256, game
arguments). Our own code; the rows are our own observation of 1.14d.

  python3 tools/trace-recorder/facts_join.py RAW --name NAME [--frames N]
          [--from F] [--skip ID,..] [--out-dir DIR] [--note TEXT]
"""

import argparse
import json
import os
import sys

TOOL = "trace-recorder facts_join 0.2.0"
FORMAT = "join-facts-1"


def rows(path, frames, start=-1, skip=()):
    header, out = None, []
    with open(path, encoding="utf-8") as f:
        for line in f:
            r = json.loads(line)
            t = r.get("type")
            if t == "header":
                header = r
                continue
            fr = r.get("frame")
            if frames and fr is not None and fr > frames:
                break
            if start >= 0 and (fr is None or fr <= start):
                continue
            if t == "s2c":
                d, caller = "s2c", r.get("caller", "")
            elif t == "client_out":
                d, caller = "c2s", ""
            else:
                continue
            b = r.get("bytes", "")
            if d == "s2c" and b[:2] in skip:
                continue
            out.append((len(out), "" if fr is None else fr, d, b[:2], r.get("size"), caller, b))
    if header is None or header.get("format") != "packets-raw-1":
        raise SystemExit(f"{path}: not a packets-raw-1 file")
    return header, out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw")
    ap.add_argument("--name", required=True, help="facts/join/<name>.tsv")
    ap.add_argument("--frames", type=int, default=2, help="last frame (0 = to the end)")
    ap.add_argument("--from", dest="start", type=int, default=-1,
                    help="only frames > F (default: from the start)")
    ap.add_argument("--skip", default="", help="S->C ids to leave out, hex, comma-separated")
    ap.add_argument("--note", default="", help="one line describing the run (character, input)")
    ap.add_argument("--out-dir", default=None)
    a = ap.parse_args()
    here = os.path.dirname(os.path.abspath(__file__))
    out_dir = a.out_dir or os.path.normpath(os.path.join(here, "..", "..", "facts", "join"))
    skip = tuple(x.strip().lower().zfill(2) for x in a.skip.split(",") if x.strip())
    header, rs = rows(a.raw, a.frames, a.start, skip)
    os.makedirs(out_dir, exist_ok=True)
    out = os.path.join(out_dir, a.name + ".tsv")
    cmd = "python3 tools/trace-recorder/facts_join.py <raw> --name " + a.name
    if a.frames != 2:
        cmd += f" --frames {a.frames}"
    if a.start >= 0:
        cmd += f" --from {a.start}"
    if skip:
        cmd += " --skip " + ",".join(skip)
    if a.out_dir:
        cmd += " --out-dir " + os.path.relpath(out_dir, os.path.join(here, "..", ".."))
    with open(out, "w", encoding="utf-8", newline="\n") as o:
        o.write(f"# format: {FORMAT}\n# tool: {TOOL}\n# command: {cmd}\n")
        o.write(f"# recorder: {header.get('tool')} ({header.get('format')}), "
                f"Game.exe sha256 {header.get('game_exe_sha256')}\n")
        o.write(f"# game args: {' '.join(header.get('args', []))}\n")
        if a.note:
            o.write(f"# run: {a.note}\n")
        o.write("n\tframe\tdir\tid\tsize\tcaller\tbytes\n")
        for r in rs:
            o.write("\t".join(str(x) for x in r) + "\n")
    print(f"wrote {out}: {len(rs)} messages")


if __name__ == "__main__":
    sys.exit(main())
