"""Convert a raw RNG recording into traces in the traces/FORMAT.md format.

    py tools/trace-recorder/convert_rng.py RAW.jsonl --list
    py tools/trace-recorder/convert_rng.py RAW.jsonl --chain 3 --chain 7 [--limit 200]

A raw recording interleaves many seeds. The converter splits it into
chains: a chain is a run of draws where each draw's seed-before is the
previous draw's seed-after (helpers and inline steps alike). Each exported
chain becomes one trace (area "sim", behavior "rng"): setup.seed is the
chain's first seed-before, expected[] holds one "rng_draw" event per draw
with the operation, its arguments, the value and the seed-after.

rng_draw data fields (documented in specs/sim/rng.md, Test vectors):
  op     "step" | "roll" | "roll_range" | "mask" | "mask_range"
  n      range argument (roll, roll_range, mask, mask_range)
  min    lower bound (roll_range, mask_range)
  value  returned value (step: the new low word)
  state  {"lo", "hi"} seed after the operation
  site   1.14d call site (informational; listed in compare.ignore)
"""

import argparse
import datetime
import json
import os
import re
import sys

import d2rng

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
OUT_DIR = os.path.join(REPO, "traces", "sim", "rng")


def load(path):
    events = [json.loads(line) for line in open(path, encoding="utf-8")]
    return events[0], [e for e in events if e["type"] in ("draw", "seed_set")]


def build_chains(events):
    chains, open_by_state = [], {}
    for e in events:
        if e["type"] != "draw" or e.get("after") is None:
            continue
        before, after = tuple(e["before"]), tuple(e["after"])
        idx = open_by_state.pop(before, None)
        if idx is None:
            idx = len(chains)
            chains.append({"seed": before, "draws": [], "ptrs": set()})
        c = chains[idx]
        c["draws"].append(e)
        if "seed" in e:
            c["ptrs"].add(e["seed"])
        open_by_state[after] = idx
    return chains


def draw_data(e):
    d = {"op": e["op"], "value": e["ret"],
         "state": {"lo": e["after"][0], "hi": e["after"][1]}, "site": e["site"]}
    for k in ("n", "min"):
        if k in e:
            d[k] = e[k]
    return d


def next_id(out_dir):
    used = [int(m.group(1)) for f in os.listdir(out_dir)
            if (m := re.fullmatch(r"sim-(\d+)\.json", f))] if os.path.isdir(out_dir) else []
    return max(used, default=0) + 1


def dumps(trace):
    """Sorted keys, two-space indent, one event per line."""
    lines = ["{"]
    keys = sorted(trace)
    for i, k in enumerate(keys):
        comma = "," if i < len(keys) - 1 else ""
        v = trace[k]
        if k in ("inputs", "expected") and v:
            lines.append(f'  "{k}": [')
            for j, ev in enumerate(v):
                c2 = "," if j < len(v) - 1 else ""
                lines.append("    " + json.dumps(ev, sort_keys=True) + c2)
            lines.append("  ]" + comma)
        else:
            body = json.dumps(v, indent=2, sort_keys=True).replace("\n", "\n  ")
            lines.append(f'  "{k}": {body}{comma}')
    lines.append("}")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser(description="raw RNG recording -> traces/FORMAT.md")
    ap.add_argument("raw")
    ap.add_argument("--list", action="store_true", help="list chains and exit")
    ap.add_argument("--chain", type=int, action="append", default=[], help="chain index to export")
    ap.add_argument("--limit", type=int, default=0, help="keep at most N draws per trace")
    ap.add_argument("--min-len", type=int, default=20, help="--list: hide shorter chains")
    ap.add_argument("--out-dir", default=OUT_DIR)
    ap.add_argument("--by", default="tools/trace-recorder (automated)")
    ap.add_argument("--note", default="", help="text for the trace's notes field")
    a = ap.parse_args()

    header, events = load(a.raw)
    chains = build_chains(events)
    if a.list or not a.chain:
        print(f"{len(chains)} chains; showing those with >= {a.min_len} draws")
        for i, c in enumerate(chains):
            if len(c["draws"]) < a.min_len:
                continue
            ops = {}
            for e in c["draws"]:
                ops[e["op"]] = ops.get(e["op"], 0) + 1
            sites = sorted({e["site"] for e in c["draws"]})
            print(f"  [{i}] {len(c['draws'])} draws, seed {c['seed']}, ptr {sorted(c['ptrs'])}, "
                  f"ops {ops}, {len(sites)} sites {sites[:4]}{'...' if len(sites) > 4 else ''}")
        return 0

    os.makedirs(a.out_dir, exist_ok=True)
    n = next_id(a.out_dir)
    for ci in a.chain:
        c = chains[ci]
        draws = c["draws"][:a.limit] if a.limit else c["draws"]
        # self-check before writing: the chain must replay under d2rng
        lo, hi = c["seed"]
        for e in draws:
            val, lo, hi, _ = d2rng.apply(e["op"], lo, hi, n=e.get("n"), lo_bound=e.get("min"))
            if [lo, hi] != list(e["after"]) or val != e["ret"]:
                print(f"chain {ci}: draw seq {e['seq']} does not replay; not exported")
                return 1
        tid = f"sim-{n:04d}"
        n += 1
        trace = {
            "format_version": 1,
            "id": tid,
            "game_version": "1.14d",
            "area": "sim",
            "behavior": "rng",
            "spec": "specs/sim/rng.md",
            "recorded": {"date": header.get("date", datetime.date.today().isoformat()),
                         "method": "debugger", "tool": header["tool"], "by": a.by},
            "setup": {"seed": {"lo": c["seed"][0], "hi": c["seed"][1]}},
            "inputs": [],
            "expected": [{"tick": 0, "kind": "rng_draw", "data": draw_data(e)} for e in draws],
            "compare": {"mode": "exact", "ignore": ["site"]},
            "notes": (a.note + " " if a.note else "") +
                     f"Chain {ci} of raw recording {os.path.basename(a.raw)} "
                     f"(Game.exe {' '.join(header.get('args', []))}); "
                     f"{len(draws)} of {len(c['draws'])} draws; seed address(es) "
                     f"{', '.join(sorted(c['ptrs'])) or 'unknown (inline only)'}. "
                     "Untimed: tick is 0 for every event.",
        }
        path = os.path.join(a.out_dir, tid + ".json")
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(dumps(trace))
        print(f"wrote {os.path.relpath(path, REPO)} ({len(draws)} draws)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
