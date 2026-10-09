"""Convert a record_walk.py raw file (walk-raw-1) into a committed trace
(traces/FORMAT.md, area "client", behavior "model"): the camera globals
before the first drawn frame, and the local player's client and server
path states at each server tick where either changed, with the per-tick
and per-frame comparison counts.

  python3 tools/trace-recorder/convert_walk.py RAW.jsonl --id client-0001
      --setup '{"character": "ScnAma", ...}' --notes TEXT --out traces/client/model/client-0001.json

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import json

TOOL = "trace-recorder convert_walk 0.1.0"


def state(rec):
    """[x 16.16, y 16.16, mode, target x, target y, index, count] -> data."""
    if rec is None:
        return None
    return {"x": rec[0], "y": rec[1], "mode": rec[2], "target": [rec[3], rec[4]], "index": rec[5],
            "count": rec[6]}


def convert(lines):
    header, out, prev = None, [], None
    stats = {"ticks": 0, "frames": 0, "tick_pos_diff": 0, "frame_pos_diff": 0, "same_unit": 0}
    for line in lines:
        r = json.loads(line)
        k = r.get("k")
        if k == "header":
            header = r
        elif k == "cam0":
            out.append({"tick": r["f"], "kind": "camera_init",
                        "data": {"at": r["at"], "unit_origin": r["origin"], "shift_x": r["shift_x"],
                                 "open_mode": r["open_mode"], "draws": r["draw"],
                                 "client_updates": r["client_update"], "player": r["player"]}})
        elif k == "units":
            if r["client"][0] != "0x0" and r["client"] == r["server"]:
                stats["same_unit"] += 1
        elif k in ("t", "fr") and r["c"] and r["s"]:
            c, s = r["c"], r["s"]
            diff = c[:2] != s[:2]
            stats["ticks" if k == "t" else "frames"] += 1
            stats["tick_pos_diff" if k == "t" else "frame_pos_diff"] += diff
            if k == "t":
                key = (tuple(c), tuple(s))
                if key != prev:
                    prev = key
                    out.append({"tick": r["f"], "kind": "walk_state",
                                "data": {"client": state(c), "server": state(s)}})
    return header, out, stats


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("raw")
    ap.add_argument("--id", required=True)
    ap.add_argument("--setup", default="{}")
    ap.add_argument("--notes", default="")
    ap.add_argument("--command", default="")
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    with open(a.raw, encoding="utf-8") as f:
        header, expected, stats = convert(f)
    trace = {
        "format_version": 1, "id": a.id, "game_version": "1.14d", "area": "client",
        "behavior": "model", "spec": "specs/client/model.md",
        "recorded": {"date": header["date"], "method": "debugger",
                     "tool": f"{header['tool']} ({header['format']}); {TOOL}", "by": "q-prov-recording-2",
                     "game_exe_sha256": header["game_exe_sha256"], "args": header["args"],
                     "command": a.command},
        "setup": json.loads(a.setup), "inputs": [], "expected": expected,
        "compare": {"mode": "exact", "ignore": []},
        "summary": stats, "notes": a.notes,
    }
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("{\n")
        keys = sorted(trace)
        for i, k in enumerate(keys):
            end = "," if i + 1 < len(keys) else ""
            if k == "expected":
                f.write('  "expected": [\n')
                for j, e in enumerate(trace[k]):
                    f.write("    " + json.dumps(e, sort_keys=True, separators=(",", ":"))
                            + ("," if j + 1 < len(trace[k]) else "") + "\n")
                f.write("  ]" + end + "\n")
            else:
                f.write(f"  {json.dumps(k)}: {json.dumps(trace[k], sort_keys=True)}{end}\n")
        f.write("}\n")
    with open(a.out, encoding="utf-8") as f:
        json.load(f)                  # the written trace must parse
    print(f"wrote {a.out}: {len(expected)} events, {stats}")


if __name__ == "__main__":
    main()
