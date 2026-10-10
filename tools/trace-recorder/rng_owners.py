"""Owner post-pass for 1.14d RNG recordings made with `record_rng.py
--frames` (specs/tools/rng-trace.md §2, §4): gives every `draw` and
`seed_set` record an `owner`: "game", "unit T:G" or "other:<addr>"
("other:inline" for an inline draw whose seed is not known).

    python3 rng_owners.py FILE.jsonl      # rewrite FILE with owners
    python3 rng_owners.py --selftest

Rules (rng-trace.md §2):
  1. A helper draw or seed setter names its seed's address: game +0xD0 is
     "game"; an address whose unit (address - 0x20) the recorder read as a
     server unit T:G, with T:G in some tick's unit list, is "unit T:G".
  2. Every tick record holds the game seed and every server unit's seed at
     the tick's entry. Forward: from those values, a draw whose seed-before
     equals one owner's current value is that owner's; its seed-after
     becomes the owner's value. Seed setters of a known owner set it.
  3. Backward: from the values at the next tick, a draw still open whose
     seed-after equals an owner's value is that owner's; its seed-before
     becomes the value.
  4. A draw that does not step (roll with n < 1) links nothing.
  5. An inline draw in the DRLG code (sim/rng.md §5.4, §7 DRLG rows:
     0x642000-0x643000, 0x66B000-0x682000 without the
     charged-bolt path compute 0x67A240-0x67A390) is never a game or unit draw:
     owner "other:drlg". With `-seed N` the DRLG seed starts at {N, 666},
     the game seed's own start, and replays its states.
  6. The backward pass gives a draw to an owner only when that owner's
     forward chain did not already reach its value at the next tick.
  7. A forward chain that began at an address-hinted draw (rule 1) of an
     owner with no known value yet (a unit created this frame) is not a
     chain from a known point: rule 6 does not stop the backward pass for
     it, which then takes the draws before that one (the inline steps a
     new monster's seed takes before its first helper draw).

Standard library only; our own code.
"""

import json
import os
import sys

G_SEED = 0xD0
# 0x67A240-0x67A390 is the charged-bolt path compute (FUN_0067a240, REC-3120): its
# inline draws step the missile's unit seed, so it is not skipped.
DRLG_SITES = ((0x642000, 0x643000), (0x66B000, 0x67A240), (0x67A390, 0x682000))


def drlg_site(r):
    if r.get("via") != "inline" or "site" not in r:
        return False
    a = int(r["site"], 16)
    return any(lo <= a < hi for lo, hi in DRLG_SITES)


def _key(v):
    return None if v is None or None in v else (v[0], v[1])


def assign(records):
    """Adds `owner` to every draw / seed_set of `records` (dicts in file
    order). Returns {owner kind: count}."""
    ticks = [r for r in records if r.get("type") == "tick"]
    gaddr = int(ticks[0]["game"], 16) + G_SEED if ticks else None
    units_seen = set()
    for t in ticks:
        for u in t.get("units", []):
            units_seen.add(f"{u[0]}:{u[1]}")
    body = [r for r in records if r.get("type") in ("draw", "seed_set", "tick")]
    fixed = {}
    for i, r in enumerate(body):
        if r["type"] == "tick" or "seed" not in r:
            continue
        addr = int(r["seed"], 16)
        if gaddr is not None and addr == gaddr:
            fixed[i] = "game"
        elif r.get("unit") in units_seen:
            fixed[i] = "unit " + r["unit"]

    def scan(t):
        cur = {"game": tuple(t["gseed"])}
        for ut, g, lo, hi in t.get("units", []):
            cur[f"unit {ut}:{g}"] = (lo, hi)
        return cur

    def pick(cur, value):
        c = sorted(o for o, v in cur.items() if v == value)
        return c[0] if c else None

    owner = dict(fixed)
    drlg = {i for i, r in enumerate(body) if r["type"] == "draw" and drlg_site(r)}
    # forward (rule 2); `explained[i]`: owners whose chain reached the tick at i
    cur = {}
    explained = {}
    loose = set()  # owners whose chain this frame began at a hinted draw (rule 7)
    for i, r in enumerate(body):
        if r["type"] == "tick":
            nxt = scan(r)
            explained[i] = {o for o, v in nxt.items() if cur.get(o) == v and o not in loose}
            cur = nxt
            loose = set()
            continue
        if i in drlg:
            continue
        if r["type"] == "seed_set":
            o = owner.get(i)
            if o:
                cur[o] = tuple(r["new"])
            continue
        b, a = _key(r.get("before")), _key(r.get("after"))
        if b is None or a is None or a == b:
            continue
        o = owner.get(i) or pick(cur, b)
        if o:
            if o not in cur:
                loose.add(o)
            owner[i] = o
            cur[o] = a
    # backward (rules 3, 6)
    cur = {}
    for i in range(len(body) - 1, -1, -1):
        r = body[i]
        if r["type"] == "tick":
            cur = {o: v for o, v in scan(r).items() if o not in explained.get(i, ())}
            continue
        if i in drlg:
            continue
        if r["type"] == "seed_set":
            new, old = _key(r.get("new")), _key(r.get("old"))
            o = owner.get(i)
            if o is None and new is not None:
                o = pick(cur, new)
                if o:
                    owner[i] = o
            if o and cur.get(o) == new:
                cur[o] = old
            continue
        b, a = _key(r.get("before")), _key(r.get("after"))
        if b is None or a is None or a == b:
            continue
        o = owner.get(i)
        if o is None:
            o = pick(cur, a)
            if o:
                owner[i] = o
        if o and cur.get(o) == a:
            cur[o] = b
    counts = {}
    for i, r in enumerate(body):
        if r["type"] == "tick":
            continue
        o = owner.get(i)
        if o is None:
            o = f"other:{r['seed']}" if "seed" in r else (
                "other:drlg" if i in drlg else "other:inline")
        r["owner"] = o
        if r["type"] == "draw":
            k = o.split(":")[0].split(" ")[0]
            counts[k] = counts.get(k, 0) + 1
    return counts


def assign_file(path):
    """Rewrites `path` with owners (atomic replace). Returns the counts."""
    with open(path, encoding="utf-8") as f:
        recs = [json.loads(line) for line in f if line.strip()]
    counts = assign(recs)
    if recs and recs[0].get("type") == "header":
        recs[0]["owners"] = True
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8", newline="\n") as f:
        for r in recs:
            f.write(json.dumps(r, separators=(",", ":")) + "\n")
    os.replace(tmp, path)
    return counts


# --- self-test -----------------------------------------------------------------

K = 0x6AC690C5
M32 = 0xFFFFFFFF


def step(s):
    v = s[0] * K + s[1]
    return (v & M32, v >> 32)


def synthetic():
    """A small recording: game at 0x1000 (seed at 0x10D0), unit 1:5 seed at
    0x2020, an item-data seed at 0x3004 (other)."""
    g0, u0, x0 = (1234, 666), (77, 666), (5, 666)
    recs = [{"type": "header", "format": "rng-raw-1"}]
    # frame 0: game creation: helper on the game seed before the game is known
    g1 = step(g0)
    recs.append({"type": "draw", "via": "inline", "op": "step", "before": list(g0),
                 "after": list(g1), "frame": 0})
    recs.append({"type": "seed_set", "op": "init_low", "seed": "0x2020", "old": [1, 666],
                 "new": list(u0), "frame": 0, "unit": "1:5"})
    g2 = step(g1)
    recs.append({"type": "tick", "f": 1, "game": "0x1000", "gseed": list(g1),
                 "units": [[1, 5, u0[0], u0[1]]]})
    recs.append({"type": "draw", "via": "helper", "op": "roll", "seed": "0x10d0",
                 "before": list(g1), "after": list(g2), "frame": 1})
    u1 = step(u0)
    recs.append({"type": "draw", "via": "inline", "op": "step", "before": list(u0),
                 "after": list(u1), "frame": 1})
    x1 = step(x0)
    recs.append({"type": "draw", "via": "helper", "op": "roll", "seed": "0x3004",
                 "before": list(x0), "after": list(x1), "frame": 1})
    # inline on a seed written inline mid-frame (no setter): found backward
    v0 = (99, 666)
    v1 = step(v0)
    recs.append({"type": "draw", "via": "inline", "op": "step", "before": list(v0),
                 "after": list(v1), "frame": 1})
    recs.append({"type": "tick", "f": 2, "game": "0x1000", "gseed": list(g2),
                 "units": [[1, 5, u1[0], u1[1]], [4, 9, v1[0], v1[1]]]})
    recs.append({"type": "footer"})
    return recs


# Covers: specs/tools/rng-trace.md §2 r1, §2 r2, §2 r3, §2 r5, §2 r6, §2 r7, §4 r4
def selftest():
    recs = synthetic()
    counts = assign(recs)
    got = [r.get("owner") for r in recs if r["type"] in ("draw", "seed_set")]
    want = ["game", "unit 1:5", "game", "unit 1:5", "other:0x3004", "unit 4:9"]
    assert got == want, got
    assert counts == {"game": 2, "unit": 2, "other": 1}, counts
    # a unit hint that no tick lists is not trusted
    recs = synthetic()
    recs[2]["unit"] = "3:77"
    assign(recs)
    assert recs[2]["owner"] == "unit 1:5", recs[2]  # found backward instead
    # rule 5: a DRLG inline draw replaying the game's states is not the game's;
    # rule 6: nor is a copy after the game's chain already reached the tick
    recs = synthetic()
    t1 = next(i for i, r in enumerate(recs) if r["type"] == "tick")
    g1, g2 = recs[t1]["gseed"], recs[t1 + 1]["after"]
    recs.insert(t1 + 1, {"type": "draw", "via": "inline", "op": "step", "site": "0x676165",
                         "before": list(g1), "after": list(g2), "frame": 1})
    recs.insert(t1 + 3, {"type": "draw", "via": "inline", "op": "step", "site": "0x600000",
                         "before": list(g1), "after": list(g2), "frame": 1})
    assign(recs)
    assert recs[t1 + 1]["owner"] == "other:drlg", recs[t1 + 1]
    assert recs[t1 + 2]["owner"] == "game", recs[t1 + 2]
    assert recs[t1 + 3]["owner"] == "other:inline", recs[t1 + 3]
    # rule 7: a unit created mid-frame (its init hint read before the GUID is
    # written: "1:0", not listed); an inline step on the new seed, then a
    # hinted helper; the backward pass still takes the inline step
    recs = synthetic()
    t2 = [i for i, r in enumerate(recs) if r["type"] == "tick"][1]
    n0 = (4242, 666)
    n1 = step(n0)
    n2 = step(n1)
    recs[t2:t2] = [
        {"type": "seed_set", "op": "init_low", "seed": "0x4020", "old": [1, 666],
         "new": list(n0), "frame": 1, "unit": "1:0"},
        {"type": "draw", "via": "inline", "op": "step", "site": "0x573a8e",
         "before": list(n0), "after": list(n1), "frame": 1},
        {"type": "draw", "via": "helper", "op": "roll", "seed": "0x4020", "unit": "1:9",
         "before": list(n1), "after": list(n2), "frame": 1}]
    recs[t2 + 3]["units"].append([1, 9, n2[0], n2[1]])
    assign(recs)
    got = [r["owner"] for r in recs[t2:t2 + 3]]
    assert got == ["unit 1:9", "unit 1:9", "unit 1:9"], got
    print("rng_owners selftest: 4 checks passed")
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        sys.exit(selftest())
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    print(assign_file(sys.argv[1]))
