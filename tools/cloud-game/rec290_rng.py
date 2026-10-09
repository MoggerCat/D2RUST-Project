"""REC-290: compare a raw RNG recording (made under Wine) with committed
PC 1 RNG traces (traces/sim/rng/*.json).

    python3 tools/cloud-game/rec290_rng.py RAW.jsonl [TRACE.json ...]

For each trace (default: every traces/sim/rng/sim-*.json) the raw file is
split into chains with convert_rng.py's own rule, and the chain whose
first seed-before equals the trace's setup.seed is converted with
convert_rng.py's own draw_data; then the trace's expected events are
compared with the chain's first events in order, every field including
`site` (same Game.exe). Prints per trace EQUAL (n of n), the first
difference (index, both events), or NO CHAIN (the seed never appears:
the run did not reach that draw, or its seed depends on wall-clock time).
Exit 0 when every trace that has a chain is equal, 1 on any difference.

Our own code.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(REPO, "tools", "trace-recorder"))
import convert_rng  # noqa: E402


def compare(chains, trace_path):
    t = json.load(open(trace_path, encoding="utf-8"))
    seed = (t["setup"]["seed"]["lo"], t["setup"]["seed"]["hi"])
    want = t["expected"]
    cands = [c for c in chains if c["seed"] == seed]
    if not cands:
        return "NO CHAIN", None
    best = None
    for c in cands:
        got = [{"kind": "rng_draw", "tick": 0, "data": convert_rng.draw_data(e)} for e in c["draws"]]
        for i, w in enumerate(want):
            if i >= len(got):
                res = ("SHORT", f"chain ends after {len(got)} of {len(want)} events")
                break
            if got[i] != w:
                res = ("DIFFERENT", f"event {i}: PC 1 {json.dumps(w)} / Wine {json.dumps(got[i])}")
                break
        else:
            return "EQUAL", f"{len(want)} of {len(want)} events (Wine chain has {len(got)})"
        best = best or res
    return best


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    raw = argv[0]
    traces = argv[1:] or sorted(
        os.path.join(REPO, "traces", "sim", "rng", f)
        for f in os.listdir(os.path.join(REPO, "traces", "sim", "rng")) if f.endswith(".json"))
    _, events = convert_rng.load(raw)
    chains = convert_rng.build_chains(events)
    print(f"{raw}: {len(events)} records, {len(chains)} chains")
    bad = 0
    for p in traces:
        verdict, detail = compare(chains, p)
        print(f"{os.path.basename(p)}: {verdict}" + (f": {detail}" if detail else ""))
        bad += verdict in ("DIFFERENT", "SHORT")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
