"""Check a raw RNG recording against the rule in specs/sim/rng.md.

    py tools/trace-recorder/check_rng.py traces/raw/<file>.jsonl

Recomputes every recorded event from its own seed-before with d2rng.py:
  * helper draws: the new seed and the returned value;
  * inline draws: the new seed (the recorder derives the old hi word from
    lo and lo', so the real test is the new hi word: carry and high half
    of the product);
  * chains: a draw's seed-before must equal the seed-after of the previous
    draw on the same seed, or the value a setter wrote. For helper draws
    the seed is known by address; inline draws are linked by state. A
    draw whose seed-before matches a known seed-after on the low word but
    not on the high word is a mismatch.
Exit status 1 on any mismatch.
"""

import collections
import json
import sys

import d2rng


def main(path):
    events = [json.loads(line) for line in open(path, encoding="utf-8")]
    header = events[0]
    draws = [e for e in events if e["type"] in ("draw", "seed_set")]
    bad = []
    stats = collections.Counter()
    by_ptr = {}            # seed address -> last known state
    after_by_lo = {}       # lo -> set of hi seen as a seed-after (or set value)
    sites = collections.Counter()

    def known(state):
        his = after_by_lo.get(state[0])
        if his is None:
            return None
        return state[1] in his

    def remember(state):
        after_by_lo.setdefault(state[0], set()).add(state[1])

    for e in draws:
        if e["type"] == "seed_set":
            stats["seed_set"] += 1
            by_ptr[e["seed"]] = tuple(e["new"])
            remember(tuple(e["new"]))
            continue
        if e.get("after") is None:
            stats["inline_unresolved"] += 1
            continue
        before, after = tuple(e["before"]), tuple(e["after"])
        sites[(e["via"], e["site"])] += 1
        # 1. the step / helper result
        if e["via"] == "helper":
            val, lo2, hi2, drew = d2rng.apply(e["op"], *before, n=e.get("n"),
                                              lo_bound=e.get("min"))
            stats["helper_draws" if drew else "helper_nodraw"] += 1
            if (lo2, hi2) != after or val != e["ret"]:
                bad.append((e["seq"], "helper result", e, (val, lo2, hi2)))
        else:
            lo2, hi2 = d2rng.step(*before)
            stats["inline_draws"] += 1
            if (lo2, hi2) != after:
                bad.append((e["seq"], "inline step", e, (lo2, hi2)))
        # 2. continuity
        link = known(before)
        if e["via"] == "helper":
            prev = by_ptr.get(e["seed"])
            if prev == before:
                stats["chain_by_address"] += 1
            elif link:
                stats["chain_by_state"] += 1
            else:
                stats["chain_unexplained"] += 1
                if prev is not None and prev[0] == before[0]:
                    bad.append((e["seq"], "hi word differs from last state", e, prev))
            by_ptr[e["seed"]] = after
        else:
            if link is True:
                stats["chain_by_state"] += 1
            elif link is False:
                bad.append((e["seq"], "lo matches a known state, hi differs", e,
                            sorted(after_by_lo[before[0]])))
            else:
                stats["chain_unexplained"] += 1
        remember(after)

    print(f"{path}")
    print(f"  recorded {header.get('date')} by {header.get('tool')}, args {header.get('args')}")
    for k in ("helper_draws", "helper_nodraw", "inline_draws", "inline_unresolved",
              "seed_set", "chain_by_address", "chain_by_state", "chain_unexplained"):
        print(f"  {k:20s} {stats[k]}")
    print(f"  distinct draw sites  {len(sites)}")
    if bad:
        print(f"MISMATCHES: {len(bad)}")
        for seq, what, e, extra in bad[:20]:
            print(f"  seq {seq}: {what}: {e} expected {extra}")
        return 1
    print("OK: every recorded draw follows specs/sim/rng.md")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    sys.exit(main(sys.argv[1]))
