"""Convert a raw packets recording (record_packets.py) to a join trace.

    python3 tools/trace-recorder/convert_join.py RAW.jsonl OUT.json --id sim-0530 [--frames 2]

Keeps the server->client messages of the first FRAMES server frames (the
join: game creation 0x01/0x00/0x02, the loader and join sequence of
`specs/flows/game-join.md`, the first tick): one `s2c_message` event per
message with its id, its size and, for the ids whose bytes are positions and
seeds (0x03, 0x07, 0x0B, 0x15), the bytes in hex. Our own observation of
1.14d; no table or asset is copied. Format: traces/FORMAT.md, version 1.
"""
import argparse
import json
import sys

KEEP_BYTES = {0x03, 0x07, 0x0B, 0x15}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("raw")
    ap.add_argument("out")
    ap.add_argument("--id", required=True)
    ap.add_argument("--frames", type=int, default=2)
    ap.add_argument("--command", default="")
    a = ap.parse_args()
    events = [json.loads(l) for l in open(a.raw)]
    head = events[0]
    exp = []
    for e in events:
        if e.get("type") != "s2c":
            continue
        fr = e.get("frame")
        if fr is not None and fr > a.frames:
            break
        raw = bytes.fromhex(e["bytes"])
        d = {"id": raw[0], "size": len(raw)}
        if raw[0] in KEEP_BYTES:
            d["bytes"] = e["bytes"]
        exp.append({"tick": 0 if fr is None else fr, "kind": "s2c_message", "data": d})
    trace = {
        "format_version": 1,
        "id": a.id,
        "game_version": "1.14d",
        "area": "sim",
        "behavior": "join",
        "spec": "specs/flows/game-join.md",
        "recorded": {
            "date": head["date"],
            "method": "debugger",
            "tool": head["tool"],
            "by": "tools/cloud-game (Game.exe 1.14d under Wine)",
        },
        "setup": {
            "seed": {"lo": 1234, "hi": 666},
            "difficulty": "normal",
            "expansion": True,
            "args": head["args"],
            "game_exe_sha256": head["game_exe_sha256"],
        },
        "inputs": [],
        "expected": exp,
        "compare": {"mode": "exact"},
        "notes": "Server->client messages of the join of an expansion sorceress "
        "loaded from a save (d2s-tool new, level 1, ScnSor), tick = server frame "
        "(0 before the first). Frame 1: the loader's and join messages; frame 2: "
        "the first tick. Command: " + a.command,
    }
    with open(a.out, "w") as f:
        json.dump(trace, f, indent=2, sort_keys=True)
        f.write("\n")
    print(f"{a.out}: {len(exp)} messages", file=sys.stderr)


if __name__ == "__main__":
    main()
