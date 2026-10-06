"""Check the adjacency-array rules of specs/drlg/rooms.md (section 6) on
format-1 tick traces (traces/sim/tick/*.json, specs/sim/tick.md).

    py tools/trace-recorder/check_rooms.py traces/sim/tick/*.json
    py tools/trace-recorder/check_rooms.py --selftest

Input: the `room_activate` / `room_deactivate` inputs, the player's
(unit type 0) `room_add` / `room_remove` inputs, and the `lists`
snapshots (`acts[].room`, `acts[].adj`), merged by `data.seq`. A snapshot
is the state at the start of its tick (step `pre`), so it reflects every
event with a smaller seq.

Checks, per snapshot or snapshot window:
  C1  each room's adjacency array contains the room itself, its members
      are active (replayed from the activate/deactivate events and listed
      in the snapshot), no duplicates, symmetric. An `adj` entry "?" is a
      violation (an inactive room in an array).
  C2  per room X, the arrays of "fresh" snapshots are all subsequences of
      one fixed sequence (the union of their consecutive-pair orders is
      acyclic). X is fresh when, replaying the events since the previous
      snapshot, its last perturbation (removal of a room in X's last known
      array, or of a room activated since that snapshot: conservative)
      is followed by a refill (activation of X, or of a room in X's array
      in this snapshot). A room's first array counts after its own
      activation. Non-fresh arrays are counted as excluded, not checked.
  C3  for consecutive snapshots whose window holds exactly one
      room_deactivate of R and no room_activate: every array that held R
      equals the previous array with the first R replaced by the last
      entry, length - 1. Windows with other mixes of events are counted as
      undecidable (arrays are refilled by an activation in between; the
      trace has no event between).
  C4  burst = maximal run of consecutive room_activate events (in the
      stream of room_activate, room_deactivate and the player's room_add /
      room_remove, by seq) in one tick. Room change = the player's
      room_add; the burst's anchor is the room of the player room_add that
      directly precedes the burst in that stream (tick 170 style: remove,
      add, activations), else of the one that directly follows it in the
      same tick (the very first burst: activations, then the placement).
      The anchor room, when it is in the burst, is excluded from the order
      (the start room is built first); every other burst room must be in
      the anchor's array and the burst must visit them in the array's
      order, read from the first snapshot after the burst in which the
      anchor is active and with no room_deactivate in between (a removal
      permutes the array). A burst without an anchor, without such a
      snapshot, or with a deactivation before it, is counted as
      undecidable. The violation is reported at the first activation whose
      array index is smaller than its predecessor's.

Exit 1 on any violation. Our own code; nothing here is derived from
Blizzard code.
"""

import copy
import json
import os
import sys

sys.dont_write_bytecode = True


class Counts(dict):
    def inc(self, key, n=1):
        self[key] = self.get(key, 0) + n


def merged(trace):
    """Relevant events in seq order: (seq, tick, kind, data)."""
    out = []
    for arr in ("inputs", "expected"):
        for e in trace.get(arr, []):
            k = e["kind"]
            d = e["data"]
            if k in ("room_activate", "room_deactivate", "lists"):
                pass
            elif k in ("room_add", "room_remove") and d["unit"][0] == 0:
                k = "p" + k
            else:
                continue
            out.append((d["seq"], e["tick"], k, d))
    out.sort(key=lambda x: x[0])
    return out


def snap_rooms(d):
    """{room: adj list} of a lists snapshot."""
    rooms = {}
    for act in d["acts"]:
        for r in act or []:
            rooms[r["room"]] = list(r["adj"])
    return rooms


def acyclic_add(succ, arr):
    """Add arr's consecutive-pair edges to succ; True if still acyclic."""
    for a, b in zip(arr, arr[1:]):
        succ.setdefault(a, set()).add(b)
    # cycle check from every node of arr (iterative DFS)
    for start in arr:
        seen = set()
        stack = list(succ.get(start, ()))
        while stack:
            n = stack.pop()
            if n == start:
                return False
            if n in seen:
                continue
            seen.add(n)
            stack.extend(succ.get(n, ()))
    return True


def check_trace(trace):
    """Returns (counts, violations); a violation is (check, seq, text)."""
    ev = merged(trace)
    c = Counts()
    viol = []
    active = set()
    prev = None  # (seq, rooms) of the previous snapshot
    window = []  # room events since the previous snapshot
    succ = {}  # C2: room -> {member -> successor set}
    perturbed = {}  # C2: room -> bool (carried across snapshots)
    snaps = []  # (index in ev, seq, rooms)

    for i, (seq, tick, k, d) in enumerate(ev):
        if k == "lists":
            rooms = snap_rooms(d)
            snaps.append((i, seq, rooms))
            c.inc("snapshots")
            # ---- C1
            if set(rooms) != active:
                viol.append(("C1", seq, "listed rooms differ from replayed active set: "
                             "%s" % sorted(set(rooms) ^ active)))
            for x, adj in rooms.items():
                c.inc("C1 arrays")
                if x not in adj:
                    viol.append(("C1", seq, "%s: own room not in its array %s" % (x, adj)))
                if len(set(adj)) != len(adj):
                    viol.append(("C1", seq, "%s: duplicate in %s" % (x, adj)))
                for y in adj:
                    if y == "?" or y not in active or y not in rooms:
                        viol.append(("C1", seq, "%s: member %s is not active" % (x, y)))
                    elif x not in rooms[y]:
                        viol.append(("C1", seq, "%s lists %s but %s does not list %s"
                                     % (x, y, y, x)))
            # ---- C2
            acts_since = []
            for x, adj in rooms.items():
                pb = perturbed.get(x, True)
                last = prev[1].get(x, []) if prev else []
                seen_act = []
                for (wk, wd) in window:
                    r = wd["room"]
                    if wk == "room_activate":
                        seen_act.append(r)
                        if r == x or r in adj:
                            pb = False
                    elif r in last or r in seen_act:
                        pb = True
                perturbed[x] = pb
                if pb:
                    c.inc("C2 arrays excluded (not refilled since a removal)")
                    continue
                c.inc("C2 arrays checked")
                sx = succ.setdefault(x, {})
                if not acyclic_add(sx, adj):
                    viol.append(("C2", seq, "%s: array %s contradicts the order of earlier "
                                 "arrays" % (x, adj)))
                    # drop this array's edges so later snapshots report only new conflicts
                    for a, b in zip(adj, adj[1:]):
                        sx[a].discard(b)
            # ---- C3
            if prev is not None:
                deacts = [wd["room"] for (wk, wd) in window if wk == "room_deactivate"]
                nacts = sum(1 for (wk, _) in window if wk == "room_activate")
                if len(deacts) == 1 and nacts == 0:
                    c.inc("C3 pairs decided")
                    R = deacts[0]
                    for x, old in prev[1].items():
                        if R not in old or x == R:
                            continue
                        c.inc("C3 arrays checked")
                        want = list(old)
                        j = want.index(R)
                        want[j] = want[-1]
                        want.pop()
                        if x not in rooms:
                            viol.append(("C3", seq, "%s vanished" % x))
                        elif rooms[x] != want:
                            viol.append(("C3", seq, "%s: %s after removing %s from %s, "
                                         "expected %s" % (x, rooms[x], R, old, want)))
                elif deacts:
                    c.inc("C3 pairs undecidable (removals mixed with activations or several)")
            prev = (seq, rooms)
            window = []
        elif k == "room_activate":
            active.add(d["room"])
            window.append((k, d))
        elif k == "room_deactivate":
            active.discard(d["room"])
            window.append((k, d))

    # ---- C4
    rev = [(i, e) for i, e in enumerate(ev) if e[2] != "lists"]
    bursts = []
    cur = None
    for pos, (i, e) in enumerate(rev):
        if e[2] == "room_activate":
            if cur is not None and cur["tick"] == e[1] and cur["last"] == pos - 1:
                cur["evs"].append(e)
                cur["last"] = pos
            else:
                cur = {"tick": e[1], "first": pos, "last": pos, "evs": [e]}
                bursts.append(cur)
    for b in bursts:
        c.inc("C4 bursts")
        anchor = None
        before = rev[b["first"] - 1][1] if b["first"] > 0 else None
        after = rev[b["last"] + 1][1] if b["last"] + 1 < len(rev) else None
        if before and before[2] == "proom_add" and before[1] == b["tick"]:
            anchor = before[3]["room"]
        elif before and before[2] == "proom_remove" and b["first"] > 1 and \
                rev[b["first"] - 2][1][2] == "proom_add" and rev[b["first"] - 2][1][1] == b["tick"]:
            anchor = rev[b["first"] - 2][1][3]["room"]
        elif after and after[2] == "proom_add" and after[1] == b["tick"]:
            anchor = after[3]["room"]
        if anchor is None:
            c.inc("C4 undecidable (no player room_add next to the burst)")
            continue
        endseq = b["evs"][-1][0]
        got = None
        for (si, sseq, rooms) in snaps:
            if sseq > endseq and anchor in rooms:
                got = (sseq, rooms[anchor])
                break
        if got is None:
            c.inc("C4 undecidable (anchor not in any later snapshot)")
            continue
        if any(e[2] == "room_deactivate" and endseq < e[0] < got[0] for _, e in rev):
            c.inc("C4 undecidable (removal between the burst and the snapshot)")
            continue
        c.inc("C4 bursts checked")
        arr = got[1]
        lastidx = -1
        for e in b["evs"]:
            r = e[3]["room"]
            if r == anchor:
                continue
            if r not in arr:
                viol.append(("C4", e[0], "burst room %s not in anchor %s's array %s"
                             % (r, anchor, arr)))
                continue
            idx = arr.index(r)
            c.inc("C4 activations checked")
            if idx < lastidx:
                viol.append(("C4", e[0], "%s activated after a room later in anchor %s's "
                             "array %s" % (r, anchor, arr)))
            lastidx = max(lastidx, idx)
    return c, viol


def report(path, c, viol):
    print("%s: %s" % (path, ", ".join("%s %d" % (k, c[k]) for k in sorted(c))))
    for (chk, seq, text) in viol[:20]:
        print("  VIOLATION %s at seq %d: %s" % (chk, seq, text))
    if len(viol) > 20:
        print("  ... %d more" % (len(viol) - 20))
    per = {}
    for v in viol:
        per[v[0]] = per.get(v[0], 0) + 1
    print("  violations: " + (", ".join("%s %d" % kv for kv in sorted(per.items())) or "none"))


# ---------------------------------------------------------------- selftest

def synthetic():
    """rooms.md synthetic vector: adjacency [R, X, Y, Z], remove X gives [R, Z, Y].

    R1..R4 form one clique with array [R1, R2, R3, R4]; the player is placed
    in R1; R2 is removed; the player moves to R4 and R5, R6 are activated.
    Returns (trace, marks) with the seq of the events the perturbations hit.
    """
    seq = [0]
    inputs, expected = [], []
    marks = {}

    def add(arr, kind, tick, **data):
        seq[0] += 1
        data["seq"] = seq[0]
        data["step"] = "pre"
        arr.append({"kind": kind, "tick": tick, "data": data})
        return seq[0]

    def snap(tick, arrays):
        acts = [[{"room": r, "units": [], "queue": [], "adj": list(a)}
                 for r, a in arrays.items()], None, None, None, None]
        return add(expected, "lists", tick, acts=acts, hash={}, tiles=[], clients=[1])

    marks["snap0"] = snap(1, {}) if False else None
    snap(1, {})
    for r in ("R1", "R2", "R3", "R4"):
        add(inputs, "room_activate", 2, room=r, act=0)
    add(inputs, "room_add", 2, room="R1", unit=[0, 1])
    marks["snap25"] = snap(25, {r: ["R1", "R2", "R3", "R4"] for r in ("R1", "R2", "R3", "R4")})
    add(inputs, "room_deactivate", 30, room="R2", act=0)
    marks["snap50"] = snap(50, {"R1": ["R1", "R4", "R3"], "R3": ["R1", "R4", "R3"],
                                "R4": ["R1", "R4", "R3"]})
    add(inputs, "room_remove", 60, unit=[0, 1])
    add(inputs, "room_add", 60, room="R4", unit=[0, 1])
    marks["act5"] = add(inputs, "room_activate", 60, room="R5", act=0)
    marks["act6"] = add(inputs, "room_activate", 60, room="R6", act=0)
    full = ["R1", "R3", "R4", "R5", "R6"]
    marks["snap75"] = snap(75, {r: full for r in full})
    return {"inputs": inputs, "expected": expected}, marks


def swap_seq(trace, a, b):
    ev = {e["data"]["seq"]: e for e in trace["inputs"] + trace["expected"]}
    ev[a]["data"]["seq"], ev[b]["data"]["seq"] = b, a


def selftest():
    ok = True
    base, m = synthetic()
    c, v = check_trace(base)
    if v or c.get("C3 arrays checked") != 3 or c.get("C4 bursts checked") != 2 \
            or c.get("C2 arrays excluded (not refilled since a removal)") != 3:
        print("selftest FAIL: synthetic trace: %s %s" % (dict(c), v))
        ok = False

    def snap_of(t, seq):
        return next(e for e in t["expected"] if e["data"]["seq"] == seq)

    def adj_of(t, seq, room):
        return next(r for r in snap_of(t, seq)["data"]["acts"][0] if r["room"] == room)

    cases = []
    t = copy.deepcopy(base)
    adj_of(t, m["snap25"], "R2")["adj"].remove("R2")
    cases.append(("C1", m["snap25"], t))
    t = copy.deepcopy(base)
    a = adj_of(t, m["snap75"], "R1")["adj"]
    a[1], a[2] = a[2], a[1]
    cases.append(("C2", m["snap75"], t))
    t = copy.deepcopy(base)
    a = adj_of(t, m["snap50"], "R3")["adj"]
    a[1], a[2] = a[2], a[1]
    cases.append(("C3", m["snap50"], t))
    t = copy.deepcopy(base)
    swap_seq(t, m["act5"], m["act6"])  # now R6 is activated first; R5 second
    cases.append(("C4", m["act5"] - 0, t))
    for chk, where, t in cases:
        # the C4 case: the event hit is the one that moved, R5 at the later seq
        if chk == "C4":
            where = m["act6"]
        c, v = check_trace(t)
        first = min(v, key=lambda x: (x[1], x[0])) if v else None
        if not first or first[0] != chk or first[1] != where or len(v) != 1:
            print("selftest FAIL: perturbation %s expected exactly one violation at seq %d, "
                  "got %s" % (chk, where, v))
            ok = False
        else:
            print("selftest: perturbation %s reported at seq %d" % (chk, where))
    print("selftest %s" % ("ok" if ok else "FAILED"))
    return ok


def main(argv):
    if argv == ["--selftest"]:
        return 0 if selftest() else 1
    if not argv or any(a.startswith("-") for a in argv):
        print(__doc__)
        return 2
    bad = 0
    total = Counts()
    for p in argv:
        with open(p, encoding="utf-8") as f:
            trace = json.load(f)
        c, v = check_trace(trace)
        report(os.path.basename(p), c, v)
        bad += len(v)
        for k, n in c.items():
            total.inc(k, n)
    print("total: " + ", ".join("%s %d" % (k, total[k]) for k in sorted(total)))
    print("violations: %d" % bad)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
