"""Replay the environment records of a record_sound.py --env recording through
the rules of specs/render/lighting.md §9.3-§9.4 (period tables from
specs/render/env-periods.tsv) and report every client update whose recorded
period index, ticks, intensity or color differs from the prediction.

Each `env_update` record holds the environment after the update; the
prediction for record k starts from the recorded state after record k-1 (so
one wrong step is reported once, not carried on). `env_set` (S->C 0x53)
records reset the base to their recorded state. Exit 1 on any mismatch.

  py check_env.py <file>-sound.jsonl [--theta int|float] [--selftest]

Our own code, written from the spec; nothing here is derived from Blizzard code.
"""

import argparse
import json
import math
import os
import struct
import sys

SPEED = 128
PI_F = 3.1415927410125732
ACT_STARTS = (40, 75, 103, 109)


def load_tables():
    here = os.path.dirname(os.path.abspath(__file__))
    path = os.path.join(here, "..", "..", "specs", "render", "env-periods.tsv")
    tabs = {}
    with open(path, encoding="utf-8") as f:
        next(f)
        for line in f:
            t, p, start, typ, r, g, b = line.split("\t")
            tabs.setdefault(t, []).append((int(start), int(typ), (int(r), int(g), int(b))))
    return tabs


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def act_of(level):
    return sum(1 for s in ACT_STARTS if s <= level)


def step(tabs, st, level, theta_mode="float"):
    """One client update (§9.3 r1-r4, §9.4, §9.2 r1 level 120). st: idx, type, ticks, I,
    rgb, eclipse. Returns the new state (a copy)."""
    st = dict(st)
    a = act_of(level)
    normal = tabs["normal"]
    if st["eclipse"] == 0:
        st["ticks"] += 1
        if a == 3:
            st["ticks"] += 15
        elif normal[st["idx"]][1] == 2:
            st["ticks"] += 1
            if a == 2:
                st["ticks"] += 8
    else:
        st["ticks"] += 1
    if st["ticks"] >= SPEED * 360:
        st["ticks"] = 0
    tab = normal if st["eclipse"] == 0 else (tabs["act4"] if a == 3 else tabs["eclipse"])
    nxt = (st["idx"] + 1) % 6
    if tab[nxt][0] * SPEED < st["ticks"]:
        st["idx"] = nxt
        src = normal if st["eclipse"] == 0 else tabs["eclipse"]
        st["type"] = src[nxt][1]
        st["ticks"] = src[nxt][0] * SPEED
    # intensity
    if a == 3:
        target = {103: 128, 104: 64, 105: 56, 106: 48}.get(level, 16)
        if st["I"] < target:
            st["I"] += 1
        elif st["I"] > target:
            st["I"] -= 1
        if st["I"] == 0:
            st["I"] = target
    elif st["eclipse"]:
        if st["I"] > 32:
            st["I"] -= 8
        if st["I"] < 32:
            st["I"] = 32
    elif level == 120:
        st["I"] = 200
    else:
        q = (st["ticks"] // SPEED) if theta_mode == "int" else (st["ticks"] / SPEED)
        th = (q / 180.0) * PI_F
        s = math.sin(th) if st["ticks"] < SPEED * 180 else 0.5 * math.sin(th)
        s = f32(s)
        v = math.trunc(s * 128 + 128 + 0.5)
        cap = 170 if a == 4 else 255
        st["I"] = max(0, min(cap, v))
    # color (§9.4)
    ctab = tabs["act4"] if a == 3 else (tabs["eclipse"] if st["eclipse"] else normal)
    c, n = ctab[st["idx"]], ctab[(st["idx"] + 1) % 6]
    den = (n[0] - c[0]) * SPEED
    t = (st["ticks"] - c[0] * SPEED) / den if den else 0.0
    st["rgb"] = [(c[2][k] + math.trunc((n[2][k] - c[2][k]) * t + 0.5)) % 256 for k in range(3)]
    if level == 120:
        st["rgb"] = [245, 240, 255]
    return st


KEYS = ("idx", "ticks", "I", "rgb")


def check(recs, tabs, theta_mode):
    base, n, bad = None, 0, []
    stats = {"updates": 0, "set": 0, "periods": set(), "I_min": None, "I_max": None}
    for r in recs:
        if r.get("type") == "env_set" and r.get("env"):
            base = r["env"]
            stats["set"] += 1
            continue
        if r.get("type") != "env_update" or not r.get("env"):
            continue
        e = r["env"]
        stats["updates"] += 1
        stats["periods"].add(e["idx"])
        stats["I_min"] = e["I"] if stats["I_min"] is None else min(stats["I_min"], e["I"])
        stats["I_max"] = e["I"] if stats["I_max"] is None else max(stats["I_max"], e["I"])
        if base is not None:
            want = step(tabs, base, r.get("L") or 0, theta_mode)
            n += 1
            diff = [k for k in KEYS if want[k] != e[k]]
            if diff:
                bad.append({"C": r.get("C"), "seq": r.get("seq"), "L": r.get("L"), "diff": diff,
                            "got": {k: e[k] for k in KEYS}, "want": {k: want[k] for k in KEYS}})
        base = e
    stats["periods"] = sorted(stats["periods"])
    return n, bad, stats


def selftest(tabs):
    st = {"idx": 2, "type": 0, "ticks": 0, "I": 128, "rgb": [255, 255, 255], "eclipse": 0}
    s1 = step(tabs, st, 1)
    assert s1["ticks"] == 1 and s1["I"] == 128 and s1["rgb"] == [255, 255, 255], s1
    # a full day from creation in act 1 town: every period is visited, back to index 2
    seen, s = set(), st
    for _ in range(60000):
        s = step(tabs, s, 1)
        seen.add(s["idx"])
    assert seen == set(range(6)), seen
    # period 1 lasts one update (§9.4 consequences)
    s = dict(st, idx=1, type=3, ticks=340 * SPEED)
    assert step(tabs, s, 1)["idx"] == 2
    # a perturbed record must be reported
    a = dict(st)
    b = step(tabs, a, 1)
    rec = [{"type": "env_update", "L": 1, "env": a}, {"type": "env_update", "L": 1, "env": dict(b, I=b["I"] + 1)}]
    assert len(check(rec, tabs, "float")[1]) == 1
    print("selftest ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("file", nargs="?")
    ap.add_argument("--theta", default="float", choices=["float", "int"],
                    help="ticks / speed as a double (default) or truncated first")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--show", type=int, default=5)
    a = ap.parse_args()
    tabs = load_tables()
    if a.selftest or not a.file:
        selftest(tabs)
        if not a.file:
            return
    recs = [json.loads(line) for line in open(a.file, encoding="utf-8")]
    n, bad, stats = check(recs, tabs, a.theta)
    print(f"env updates {stats['updates']}, 0x53 sets {stats['set']}, predicted {n}, "
          f"periods seen {stats['periods']}, I {stats['I_min']}..{stats['I_max']}")
    for b in bad[:a.show]:
        print("mismatch", json.dumps(b))
    print(f"mismatches {len(bad)}")
    print("OK" if not bad and n else "FAIL")
    sys.exit(0 if not bad and n else 1)


if __name__ == "__main__":
    main()
