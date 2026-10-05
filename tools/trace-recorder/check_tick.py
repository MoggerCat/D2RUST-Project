"""Check a tick recording (record_tick.py, format tick-raw-1) against the
rules of specs/sim/tick.md and specs/sim/unit-order.md.

A model of the rules replays the recording in order:

  * timer queue (tick.md §5): every recorded schedule and cancel updates
    the model's buckets and every-tick lists; during each tick's event step
    the model walks the lists exactly as §5.5 says and must predict every
    recorded timer run, in order, and leave none due unrun. It also checks
    the expire clamp (§5.2 rule 3) and that a due timer is freed after it
    runs;
  * tick steps (§3): frame + 1 per tick, step order, periodic steps;
  * lists (unit-order.md §2, §4–§6): every recorded insert and removal is
    applied by the spec's rule, and every snapshot must equal the model.

Exit status 1 on any mismatch. --perturb-ex N swaps the N-th pair of adjacent run records,
--perturb-snap N reverses one list in snapshot N: both must make the
check fail at that spot (METHODS M08). --selftest runs a synthetic
recording built from the specs' test vectors, plain and perturbed.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import json
import sys

ORDER = [(2, "i"), (2, "d"), (0, "i"), (0, "d"), (1, "i"), (1, "d"),
         (3, "i"), (3, "d"), (4, "i"), (4, "d")]  # tick.md §5.5
STEP_ORDER = ["env", "rooms", "events", "clients", "updq", "dels"]
HASH_TYPES = (0, 1, 2, 3, 4)


def crem(a, n):
    """C remainder (sign of the dividend), as idiv gives (tick.md §2.2)."""
    r = abs(a) % n
    return -r if a < 0 else r


class Model:
    def __init__(self):
        self.errors = []
        self.frame = None
        self.ticks = 0
        self.first_frame = None
        # timers
        self.timers = {}                      # tm -> dict
        self.inf = {c: [] for c in range(5)}  # head first
        self.bucket = {(c, b): [] for c in range(5) for b in range(-63, 64)}
        self.in_events = False
        self.k = -1
        self.cursor = None
        self.executing = None
        self.runs = 0
        # steps
        self.steps = []
        # lists
        self.hash = {t: {} for t in HASH_TYPES}  # type -> bucket -> [guid desc]
        self.tiles = []
        self.unit_ids = {}                       # unit ptr -> (type, guid)
        self.room_units = {}                     # room -> [unit ptr]
        self.unit_room = {}
        self.queue = {}                          # room -> [unit ptr]
        self.unit_queue = {}
        self.act_rooms = {}                      # act -> [room]
        self.snaps = 0

    def err(self, i, msg):
        self.errors.append(f"record {i}: {msg}")

    # --- timers -----------------------------------------------------------

    def list_of(self, tm):
        t = self.timers[tm]
        return self.inf[t["c"]] if t["l"] == "i" else self.bucket[(t["c"], crem(t["x"], 64))]

    def next_of(self, tm):
        lst = self.list_of(tm)
        j = lst.index(tm)
        return lst[j + 1] if j + 1 < len(lst) else None

    def current_list(self):
        c, l = ORDER[self.k]
        return self.inf[c] if l == "i" else self.bucket[(c, crem(self.frame, 64))]

    def advance(self):
        """Next timer the queue run visits and executes (tick.md §5.5)."""
        while True:
            if self.cursor is None:
                self.k += 1
                if self.k >= len(ORDER):
                    return None
                lst = self.current_list()
                self.cursor = lst[0] if lst else None
                continue
            t = self.cursor
            self.cursor = self.next_of(t)
            if ORDER[self.k][1] == "d" and self.timers[t]["x"] != self.frame:
                continue
            return t

    def finish_run(self, i):
        t = self.executing
        self.executing = None
        if t is None:
            return
        info = self.ran[t]
        if info["l"] == "d" and t in self.timers:
            self.err(i, f"due timer {t} still queued after its run (tick.md §5.5 rule 3)")
        if info["l"] == "i" and info.get("deferred") and t in self.timers:
            self.err(i, f"every-tick timer {t} cancelled during its run but not freed")

    def on_set(self, i, r):
        if r["c"] is None:
            self.err(i, f"timer {r['tm']} scheduled for a unit outside the server lists")
            return
        if r["tm"] in self.timers:
            self.err(i, f"timer {r['tm']} scheduled while already queued")
        if r["l"] == "d" and r.get("req") is not None and self.frame is not None:
            want = r["req"] if r["req"] > self.frame else self.frame + 1
            if r["x"] != want:
                self.err(i, f"expire {r['x']}, rule gives {want} (requested {r['req']}, "
                            f"frame {self.frame}; tick.md §5.2 rule 3)")
        self.timers[r["tm"]] = {"c": r["c"], "l": r["l"], "x": r["x"]}
        if r["l"] == "i":
            self.inf[r["c"]].insert(0, r["tm"])
        else:
            self.bucket[(r["c"], crem(r["x"], 64))].append(r["tm"])

    def on_cancel(self, i, r):
        tm = r["tm"]
        if tm not in self.timers:
            self.err(i, f"cancel of unknown timer {tm}")
            return
        if r["deferred"]:
            if tm != self.executing:
                self.err(i, f"timer {tm} marked executing but the model runs {self.executing}")
            self.ran[tm]["deferred"] = True
            return
        if tm == self.executing and self.ran[tm]["l"] == "i" and not self.ran[tm].get("deferred"):
            self.err(i, f"every-tick timer {tm} freed after its run without a cancel")
        if self.in_events and self.cursor == tm:
            self.cursor = self.next_of(tm)
        self.list_of(tm).remove(tm)
        del self.timers[tm]

    def on_ex(self, i, r):
        if not self.in_events:
            self.err(i, "timer run outside the event step")
            return
        self.finish_run(i)
        want = self.advance()
        if want != r["tm"]:
            c, l = ORDER[self.k] if self.k < len(ORDER) else (None, None)
            self.err(i, f"frame {self.frame}: ran timer {r['tm']} (class {r['c']}, {r['l']}, "
                        f"type {r['ty']}, unit {r['ut']}:{r['g']}), model expects {want} "
                        f"(class {c}, {l})")
            # resynchronise on the recorded timer so later errors stay meaningful
            if r["tm"] in self.timers:
                self.k = ORDER.index((r["c"], r["l"]))
                self.cursor = self.next_of(r["tm"])
        if (r["c"], r["l"]) != (ORDER[self.k] if self.k < len(ORDER) else None):
            self.err(i, f"run record class/list {(r['c'], r['l'])} differs from the model's list")
        if r["l"] == "d" and r["x"] != self.frame:
            self.err(i, f"due timer with expire {r['x']} ran at frame {self.frame}")
        self.executing = r["tm"]
        self.ran[r["tm"]] = {"l": r["l"]}
        self.runs += 1

    def end_events(self, i):
        self.finish_run(i)
        left = self.advance()
        if left is not None:
            t = self.timers[left]
            self.err(i, f"frame {self.frame}: timer {left} (class {t['c']}, {t['l']}, expire "
                        f"{t['x']}) should have run")
        self.in_events = False

    # --- ticks and steps ----------------------------------------------------

    def on_tick(self, i, r):
        self.check_steps(i)
        if self.frame is not None and r["f"] != self.frame + 1:
            self.err(i, f"frame {r['f']} follows frame {self.frame}")
        if self.first_frame is None:
            self.first_frame = r["f"]
        self.frame = r["f"]
        self.ticks += 1
        self.steps = []

    def check_steps(self, i):
        if self.frame is None or not self.steps:
            return
        f = self.frame
        want = list(STEP_ORDER)
        if crem(f, 20) == 0:
            want.append("quests")
        if crem(f, 12) == 0:
            want.append("deact")
        got = [s for s in self.steps if s != "inactive"]
        n_inactive = len(self.steps) - len(got)
        want += ["items", "end"]
        if got != want:
            self.err(i, f"frame {f}: steps {got}, rule gives {want} (tick.md §3)")
        if (crem(f, 11) == 0) != (n_inactive > 0):
            self.err(i, f"frame {f}: {n_inactive} inactive-room steps (tick.md §3 step 10)")

    def on_step(self, i, r):
        s = r["s"]
        if s == "events":
            self.in_events = True
            self.k, self.cursor, self.executing, self.ran = -1, None, None, {}
        elif s == "clients":
            self.end_events(i)
        self.steps.append(s)

    # --- lists -------------------------------------------------------------

    def ident(self, r):
        self.unit_ids[r["u"]] = (r["ut"], r["g"])

    def on_hin(self, i, r):
        t, g = r["ut"], r["g"]
        if t == 5:
            lst = self.tiles
        else:
            lst = self.hash[t].setdefault(g & 0x7F, [])
        if g in lst:
            self.err(i, f"duplicate GUID {t}:{g}")
        j = 0
        while j < len(lst) and lst[j] > g:  # unit-order.md §2.1
            j += 1
        lst.insert(j, g)

    def on_hout(self, i, r):
        t, g = r["ut"], r["g"]
        lst = self.tiles if t == 5 else self.hash[t].get(g & 0x7F, [])
        if g not in lst:
            self.err(i, f"hash removal of absent unit {t}:{g}")
        else:
            lst.remove(g)

    def on_rin(self, i, r):
        self.ident(r)
        u, room = r["u"], r["r"]
        if u in self.unit_room:
            self.err(i, f"unit {r['ut']}:{r['g']} added to room {room} while in {self.unit_room[u]}")
        self.room_units.setdefault(room, []).insert(0, u)  # unit-order.md §5.2
        self.unit_room[u] = room

    def on_rout(self, i, r):
        room = self.unit_room.pop(r["u"], None)
        if room is not None:
            self.room_units[room].remove(r["u"])

    def on_qin(self, i, r):
        self.ident(r)
        u, room = r["u"], r["r"]
        if u in self.unit_queue:
            self.err(i, f"unit {r['ut']}:{r['g']} queued twice (unit-order.md §6.2)")
            return
        self.queue.setdefault(room, []).insert(0, u)
        self.unit_queue[u] = room

    def on_qout(self, i, r):
        room = self.unit_queue.pop(r["u"], None)
        if room is not None:
            self.queue[room].remove(r["u"])

    def on_qclear(self, i, r):
        for u in self.queue.get(r["r"], []):
            self.unit_queue.pop(u, None)
        self.queue[r["r"]] = []

    def on_ract(self, i, r):
        self.act_rooms.setdefault(r["a"], []).insert(0, r["r"])  # unit-order.md §4.2

    def on_rdeact(self, i, r):
        rooms = self.act_rooms.get(r["a"], [])
        if r["r"] in rooms:
            rooms.remove(r["r"])
        else:
            self.err(i, f"deactivation of unknown room {r['r']}")

    def on_snap(self, i, r):
        self.snaps += 1
        f = r["f"]
        for t in HASH_TYPES:
            got = {b: g for b, g in r["hash"].get(str(t), [])}
            want = {b: g for b, g in self.hash[t].items() if g}
            if got != want:
                bad = sorted(set(got) ^ set(want) | {b for b in got if got.get(b) != want.get(b)})
                self.err(i, f"snapshot frame {f}: type {t} hash buckets {bad[:4]} differ: "
                            f"recorded {[got.get(b) for b in bad[:2]]}, model {[want.get(b) for b in bad[:2]]}")
        if r["tiles"] != self.tiles:
            self.err(i, f"snapshot frame {f}: tile list {r['tiles']} != model {self.tiles}")
        ids = lambda us: [list(self.unit_ids.get(u, ("?", u))) for u in us]  # noqa: E731
        for act in r["acts"]:
            if act is None:
                continue
            rooms = [room["r"] for room in act["rooms"]]
            if rooms != self.act_rooms.get(act["a"], []):
                self.err(i, f"snapshot frame {f}: act {act['a']} rooms {rooms} != model "
                            f"{self.act_rooms.get(act['a'], [])}")
            for room in act["rooms"]:
                want_u = ids(self.room_units.get(room["r"], []))
                if room["u"] != want_u:
                    self.err(i, f"snapshot frame {f}: room {room['r']} units {room['u']} != "
                                f"model {want_u}")
                want_q = ids(self.queue.get(room["r"], []))
                if room["q"] != want_q:
                    self.err(i, f"snapshot frame {f}: room {room['r']} update queue {room['q']} "
                                f"!= model {want_q}")


def check(records, out=sys.stdout, quiet=False):
    m = Model()
    m.ran = {}
    handlers = {"tick": m.on_tick, "step": m.on_step, "ex": m.on_ex, "set": m.on_set,
                "cancel": m.on_cancel, "hin": m.on_hin, "hout": m.on_hout, "rin": m.on_rin,
                "rout": m.on_rout, "qin": m.on_qin, "qout": m.on_qout, "qclear": m.on_qclear,
                "ract": m.on_ract, "rdeact": m.on_rdeact, "snap": m.on_snap}
    last_complete = None
    for i, r in enumerate(records):
        h = handlers.get(r["k"])
        if h:
            h(i, r)
        if r["k"] == "step" and r["s"] == "end":
            last_complete = i
    # a recording cut inside a tick: drop errors past the last complete tick
    if last_complete is not None and records and records[-1].get("k") != "step":
        cut = [e for e in m.errors if int(e.split(":")[0].split()[1]) > last_complete]
        m.errors = [e for e in m.errors if e not in cut]
    if not quiet:
        print(f"ticks {m.ticks} (first frame {m.first_frame}), timer runs {m.runs}, "
              f"snapshots {m.snaps}, errors {len(m.errors)}", file=out)
        for e in m.errors[:50]:
            print("  " + e, file=out)
    return m


def load(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def perturb_ex(records, n):
    """Swap the n-th pair of consecutive run records (within one tick) that
    name different timers."""
    idx = [i for i, r in enumerate(records) if r["k"] == "ex"]
    pairs = [(a, b) for a, b in zip(idx, idx[1:])
             if records[a]["tm"] != records[b]["tm"]
             and not any(records[j]["k"] == "tick" for j in range(a, b))]
    if n >= len(pairs):
        raise SystemExit(f"only {len(pairs)} swappable run pairs")
    a, b = pairs[n]
    records[a], records[b] = records[b], records[a]
    return a


def perturb_snap(records, n):
    idx = [i for i, r in enumerate(records) if r["k"] == "snap"]
    for i in idx[n:]:
        for act in records[i]["acts"]:
            for room in act["rooms"] if act else []:
                if len(room["u"]) >= 2 and room["u"] != room["u"][::-1]:
                    room["u"].reverse()
                    return i
    raise SystemExit(f"no snapshot with a reversible room list at or after #{n}")


def synthetic():
    """A recording built by hand from the test vectors of tick.md §Test vectors
    and unit-order.md §Test vectors (not from any model code)."""
    def steps(f, events):
        out = [{"k": "tick", "f": f}, {"k": "step", "s": "env"}, {"k": "step", "s": "rooms"}]
        out += [{"k": "step", "s": "events"}] + events + [{"k": "step", "s": "clients"}]
        out += [{"k": "step", "s": s} for s in ("updq", "dels", "items", "end")]
        return out

    def tset(tm, c, l, x, req, ut, g):
        return {"k": "set", "tm": tm, "c": c, "l": l, "x": x, "req": req, "ty": 0, "ut": ut,
                "g": g, "a1": 0, "a2": 0, "cb": "0x0"}

    def ex(tm, c, l, x, ut, g):
        return {"k": "ex", "tm": tm, "c": c, "l": l, "x": x, "ty": 0, "ut": ut, "g": g}

    def cancel(tm, deferred=False):
        return {"k": "cancel", "tm": tm, "deferred": deferred}

    rec = [{"k": "header", "format": "tick-raw-1"}, {"k": "game", "g": "0x1"}]
    rec += [{"k": "ract", "r": "R1", "a": "A0"}, {"k": "ract", "r": "R2", "a": "A0"}]
    for g in (129, 1, 257):  # monsters in hash bucket 1, inserted out of order
        rec.append({"k": "hin", "u": f"M{g}", "ut": 1, "g": g, "cl": 0})
    for g in (1, 129, 257):  # room list: add 1, 129, 257 -> [257, 129, 1]
        rec.append({"k": "rin", "u": f"M{g}", "ut": 1, "g": g, "r": "R1"})
    rec += [{"k": "qclear", "r": "R1"}]
    rec += [{"k": "qin", "u": "M1", "ut": 1, "g": 1, "r": "R1"},
            {"k": "qin", "u": "M129", "ut": 1, "g": 129, "r": "R1"}]
    # tick 1: schedules during the room step, then the every-tick missile runs
    t1 = steps(1, [ex("S2", 2, "i", -1, 3, 2)])
    t1[3:3] = [tset("A", 1, "d", 3, 3, 1, 1), tset("B", 1, "d", 67, 67, 1, 129),
               tset("C", 1, "d", 3, 3, 1, 257), tset("S2", 2, "i", -1, -1, 3, 2),
               tset("P1", 0, "d", 3, 3, 0, 1), tset("S1", 2, "d", 3, 3, 3, 1)]
    rec += t1
    # tick 2: S3 scheduled after S2 runs first (every-tick lists run newest first)
    t2 = steps(2, [ex("S3", 2, "i", -1, 3, 3), ex("S2", 2, "i", -1, 3, 2)])
    t2[3:3] = [tset("S3", 2, "i", -1, -1, 3, 3)]
    rec += t2
    # tick 3: S3, S2 (every tick; S4 scheduled during S2 waits for tick 4), then the due
    # S1, P1, A; A schedules M2 for "now" (-> 4) and cancels C, which never runs
    rec += steps(3, [ex("S3", 2, "i", -1, 3, 3), ex("S2", 2, "i", -1, 3, 2),
                     tset("S4", 2, "i", -1, -1, 3, 4), ex("S1", 2, "d", 3, 3, 1), cancel("S1"),
                     ex("P1", 0, "d", 3, 0, 1), cancel("P1"), ex("A", 1, "d", 3, 1, 1),
                     tset("M2", 1, "d", 4, 3, 1, 1), cancel("C"), cancel("A")])
    rec.append({"k": "snap", "f": 4, "tiles": [], "clients": [],
                "hash": {"1": [[1, [257, 129, 1]]]},
                "acts": [{"a": "A0", "rooms": [
                    {"r": "R2", "u": [], "q": [], "adj": []},
                    {"r": "R1", "u": [[1, 257], [1, 129], [1, 1]], "q": [[1, 129], [1, 1]],
                     "adj": []}]}, None, None, None, None]})
    rec += steps(4, [ex("S4", 2, "i", -1, 3, 4), ex("S3", 2, "i", -1, 3, 3),
                     ex("S2", 2, "i", -1, 3, 2), ex("M2", 1, "d", 4, 1, 1), cancel("M2")])
    return rec


def selftest():
    ok = True
    base = synthetic()
    m = check(base, quiet=True)
    if m.errors:
        print("selftest: the synthetic recording fails:", *m.errors, sep="\n  ")
        ok = False
    for n in range(3):
        rec = json.loads(json.dumps(base))
        at = perturb_ex(rec, n)
        m = check(rec, quiet=True)
        first = int(m.errors[0].split(":")[0].split()[1]) if m.errors else None
        if first != at:
            print(f"selftest: run swap at record {at} reported at {first}: {m.errors[:2]}")
            ok = False
    rec = json.loads(json.dumps(base))
    at = perturb_snap(rec, 0)
    m = check(rec, quiet=True)
    if not any(e.startswith(f"record {at}:") and "units" in e for e in m.errors):
        print(f"selftest: snapshot perturbation at record {at} not reported: {m.errors[:2]}")
        ok = False
    print("selftest:", "pass" if ok else "FAIL")
    return ok


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw", nargs="?", help="traces/raw/<time>-tick.jsonl")
    ap.add_argument("--perturb-ex", type=int, help="swap the N-th pair of adjacent run records of one tick (must fail)")
    ap.add_argument("--perturb-snap", type=int, help="reverse a room list in snapshot N (must fail)")
    ap.add_argument("--selftest", action="store_true", help="check the checker on a synthetic recording")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(0 if selftest() else 1)
    if not a.raw:
        ap.error("a recording or --selftest is required")
    records = load(a.raw)
    if records[0].get("format") != "tick-raw-1":
        sys.exit(f"{a.raw}: not a tick-raw-1 recording")
    if a.perturb_ex is not None:
        print(f"perturbed: run records swapped at record {perturb_ex(records, a.perturb_ex)}")
    if a.perturb_snap is not None:
        print(f"perturbed: room list reversed in record {perturb_snap(records, a.perturb_snap)}")
    m = check(records)
    sys.exit(1 if m.errors else 0)


if __name__ == "__main__":
    main()
