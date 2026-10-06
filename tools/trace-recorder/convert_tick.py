"""Convert a tick recording (record_tick.py, format tick-raw-1) into a
format-1 trace (traces/FORMAT.md): area "sim", behavior "tick".

    py tools/trace-recorder/convert_tick.py RAW.jsonl [--id sim-0006] [--note "..."]
    py tools/trace-recorder/convert_tick.py --check traces/sim/tick/*.json
    py tools/trace-recorder/convert_tick.py --check TRACE --perturb-run N   (must fail)
    py tools/trace-recorder/convert_tick.py --check TRACE --perturb-lists N (must fail)
    py tools/trace-recorder/convert_tick.py --selftest

The trace keeps everything a timer-queue and unit-list implementation
needs to replay the recording without game files: every list operation
and timer schedule/cancel as `inputs`, every timer run and list snapshot
as `expected`, all interleaved by a global `seq`. Event kinds and fields
are documented in specs/sim/tick.md (Test vectors, "Trace sim/tick").
Pointers are replaced by stable ids: units by [type, GUID], timers by
their schedule number, rooms by their activation number ("R<n>"), acts by
index, clients by join number.

Strict (METHODS M07): the raw file must pass check_tick.py first, and
every record must fit the rules below, or nothing is written. Step 6's
queue clears are not stored: the converter checks that each tick's clears
are exactly what tick.md §3 step 6 gives (acts with the pending-update
flag, act order, every active room in list order) and the checker
regenerates them. `anim` records (record_tick.py 0.2.0, units.md §4) are
skipped: the units harness reads them from the raw file.

--check rebuilds a tick-raw-1 record stream from a trace, runs the
check_tick.py model on it (it must predict every run and reproduce every
snapshot), and converts the rebuilt stream again, which must give the
same trace. The perturbations swap two timer runs or reverse one room's
unit list; the check must then fail at that event (METHODS M08).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import json
import os
import re
import sys

sys.dont_write_bytecode = True
import check_tick  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
OUT_DIR = os.path.join(REPO, "traces", "sim", "tick")
TOOL = "trace-recorder convert_tick 0.1.0"
CLASS_OF_TYPE = {0: 0, 1: 1, 3: 2, 2: 3, 4: 4}  # tick.md §5.1
LIST = {"d": "due", "i": "every"}
STEPS = ["env", "rooms", "events", "clients", "updq", "dels", "quests", "deact", "inactive",
         "items", "end"]


class ConvertError(Exception):
    pass


def periodic_steps(f):
    """Step markers of frame f in recording order (tick.md §3; record_tick.py STEPS)."""
    out = ["env", "rooms", "events", "clients", "updq", "dels"]
    if check_tick.crem(f, 20) == 0:
        out.append("quests")
    if check_tick.crem(f, 12) == 0:
        out.append("deact")
    if check_tick.crem(f, 11) == 0:
        out.append("inactive")
    return out + ["items", "end"]


# --- raw -> trace -------------------------------------------------------------


def convert(records, source="", note=""):
    """Returns (setup, inputs, expected, header) from tick-raw-1 records."""
    if not records or records[0].get("format") != "tick-raw-1":
        raise ConvertError("not a tick-raw-1 recording")
    m = check_tick.check(records, quiet=True)
    if m.errors:
        raise ConvertError(f"check_tick.py reports {len(m.errors)} errors, first: {m.errors[0]}")
    ends = [i for i, r in enumerate(records) if r["k"] == "step" and r["s"] == "end"]
    if not ends:
        raise ConvertError("no complete tick")
    records = records[:ends[-1] + 1]  # drop a tick cut by the time limit
    game = next((r for r in records if r["k"] == "game"), None)
    if game is None or game.get("frame_before") != 0:
        raise ConvertError("the recording must start at the game's first tick (frame_before 0)")

    act_index = {}
    for r in records:
        if r["k"] == "snap":
            for i, a in enumerate(r["acts"]):
                if a:
                    if act_index.setdefault(a["a"], i) != i:
                        raise ConvertError(f"act {a['a']} at two indexes")

    inputs, expected = [], []
    seq = 0
    frame, step = 1, "pre"  # records before a tick belong to it (step "pre")
    timers = {}         # pointer -> timer id
    sched = {}          # timer id -> what a run of it must show
    n_timers = 0
    rooms = {}          # pointer -> room id (active rooms only)
    room_act = {}       # room id -> act index
    act_rooms = {}      # act index -> [room id], head first
    n_rooms = 0
    clients = {}        # pointer -> client id
    pending = set()     # acts with the pending-update flag (unit-order.md §6.2)
    clears = []

    def emit(dest, kind, **data):
        nonlocal seq
        seq += 1
        data["seq"] = seq
        data["step"] = step
        dest.append({"tick": frame, "kind": kind, "data": data})

    def room_id(ptr, i):
        if ptr not in rooms:
            raise ConvertError(f"record {i}: room {ptr} is not an active room")
        return rooms[ptr]

    def act_of(ptr, i):
        if ptr not in act_index:
            raise ConvertError(f"record {i}: act {ptr} never appears in a snapshot")
        return act_index[ptr]

    def unit(r):
        return [r["ut"], r["g"]]

    def end_updq(i):
        want = [room for a in sorted(pending) for room in act_rooms.get(a, [])]
        if clears != want:
            raise ConvertError(f"record {i}: frame {frame}: step 6 cleared {clears}, "
                               f"tick.md §3 step 6 gives {want}")
        pending.clear()

    for i, r in enumerate(records):
        k = r["k"]
        if k in ("header", "game", "footer"):
            continue
        if k == "anim":
            # record_tick.py 0.2.0: mode animation schedules (units.md §4), read
            # by the units harness from the raw file; not part of a sim/tick
            # trace (tick.md, Test vectors), so not converted.
            continue
        if k == "tick":
            frame, step = r["f"], "pre"
            continue
        if k == "step":
            if step == "updq":
                end_updq(i)
            step = r["s"]
            if step not in STEPS:
                raise ConvertError(f"record {i}: unknown step {step}")
            if step == "end":
                step = "pre"  # anything after the tick belongs before the next one
                frame += 1
            clears = []
            continue
        if k == "set":
            if r["c"] is None or r.get("req") is None:
                raise ConvertError(f"record {i}: schedule without a server unit or request")
            if r["c"] != CLASS_OF_TYPE.get(r["ut"]):
                raise ConvertError(f"record {i}: class {r['c']} for unit type {r['ut']}")
            if (r["l"] == "i") != (r["req"] == -1):
                raise ConvertError(f"record {i}: list {r['l']} with requested expire {r['req']}")
            n_timers += 1
            timers[r["tm"]] = n_timers
            sched[n_timers] = (r["c"], r["l"], r["ty"], unit(r), r["x"], r["a1"], r["a2"])
            d = dict(timer=n_timers, type=r["ty"], unit=unit(r), req=r["req"], expire=r["x"],
                     a1=r["a1"], a2=r["a2"])
            if r["cb"] != "0x0":
                d["cb"] = r["cb"]
            emit(inputs, "timer_set", **d)
        elif k == "cancel":
            if r["tm"] not in timers:
                raise ConvertError(f"record {i}: cancel of an unknown timer")
            emit(inputs, "timer_cancel", timer=timers[r["tm"]], deferred=r["deferred"])
        elif k == "ex":
            if r["tm"] not in timers or r["ut"] is None:
                raise ConvertError(f"record {i}: run of an unknown timer or unit")
            t = timers[r["tm"]]
            got = (r["c"], r["l"], r["ty"], unit(r), r["x"], r["a1"], r["a2"])
            if got != sched[t]:
                raise ConvertError(f"record {i}: timer {t} runs as {got}, scheduled as {sched[t]}")
            emit(expected, "timer_run", timer=t)
        elif k == "hin":
            emit(inputs, "hash_add", unit=unit(r), class_id=r["cl"])
        elif k == "hout":
            emit(inputs, "hash_remove", unit=unit(r))
        elif k == "rin":
            emit(inputs, "room_add", unit=unit(r), room=room_id(r["r"], i))
        elif k == "rout":
            emit(inputs, "room_remove", unit=unit(r))
        elif k == "qin":
            room = room_id(r["r"], i)
            pending.add(room_act[room])
            emit(inputs, "queue_add", unit=unit(r), room=room)
        elif k == "qout":
            emit(inputs, "queue_remove", unit=unit(r))
        elif k == "qclear":
            room = room_id(r["r"], i)
            if step == "updq":
                clears.append(room)
            else:
                emit(inputs, "queue_clear", room=room)
        elif k == "ract":
            if r["r"] in rooms:
                raise ConvertError(f"record {i}: room {r['r']} activated twice")
            n_rooms += 1
            room, act = f"R{n_rooms}", act_of(r["a"], i)
            rooms[r["r"]] = room
            room_act[room] = act
            act_rooms.setdefault(act, []).insert(0, room)
            emit(inputs, "room_activate", room=room, act=act)
        elif k == "rdeact":
            room, act = room_id(r["r"], i), act_of(r["a"], i)
            act_rooms[act].remove(room)
            del rooms[r["r"]]
            emit(inputs, "room_deactivate", room=room, act=act)
        elif k == "snap":
            if step != "pre" or r["f"] != frame:
                raise ConvertError(f"record {i}: snapshot outside the start of frame {r['f']}")
            emit(expected, "lists", **snapshot(r, i, rooms, act_index, clients))
        else:
            raise ConvertError(f"record {i}: unknown record kind {k}")

    setup = {"start_frame": 0, "frames": frame - 1, "game_type": "single player", "source": source,
             "game_args": records[0].get("args", []), "snap_every": records[0].get("snap_every")}
    return setup, inputs, expected, records[0]


def snapshot(r, i, rooms, act_index, clients):
    acts = [None] * 5
    for a in r["acts"]:
        if a is None:
            continue
        rl = []
        for room in a["rooms"]:
            if room["r"] not in rooms:
                raise ConvertError(f"record {i}: snapshot room {room['r']} is not active")
            rl.append({"room": rooms[room["r"]], "units": room["u"], "queue": room["q"],
                       "adj": [rooms.get(x, "?") for x in room["adj"]]})
        acts[act_index[a["a"]]] = rl
    for c in r["clients"]:
        clients.setdefault(c, len(clients) + 1)
    hashes = {t: r["hash"][t] for t in sorted(r["hash"])}
    return {"hash": hashes, "tiles": r["tiles"], "acts": acts,
            "clients": [clients[c] for c in r["clients"]]}


def make_trace(tid, setup, inputs, expected, header, note):
    return {
        "format_version": 1, "id": tid, "game_version": "1.14d", "area": "sim",
        "behavior": "tick", "spec": "specs/sim/tick.md",
        "recorded": {"date": header.get("date", ""), "method": "debugger",
                     "tool": f"{header.get('tool', '')}; {TOOL}", "by": "d2rs trace-recorder"},
        "setup": setup, "inputs": inputs, "expected": expected,
        "compare": {"mode": "exact", "ignore": []},
        "notes": note or "Timer queue and unit lists of a hand-played 1.14d single-player "
                         "game; see specs/sim/tick.md Test vectors.",
    }


def dumps(trace):
    """Sorted keys; inputs and expected one event per line (compact)."""
    lines = ["{"]
    keys = sorted(trace)
    for n, k in enumerate(keys):
        comma = "," if n < len(keys) - 1 else ""
        v = trace[k]
        if k in ("inputs", "expected"):
            lines.append(f'  "{k}": [')
            for j, e in enumerate(v):
                c = "," if j < len(v) - 1 else ""
                lines.append("    " + json.dumps(e, sort_keys=True, separators=(",", ":")) + c)
            lines.append("  ]" + comma)
        else:
            lines.append(f'  "{k}": ' + json.dumps(v, sort_keys=True, separators=(",", ": ")) + comma)
    lines.append("}")
    return "\n".join(lines) + "\n"


# --- trace -> raw (the checker) ---------------------------------------------------


def rebuild(trace):
    """A tick-raw-1 record stream equivalent to the trace, plus for each
    record the index of the trace event it came from (('i'|'e', n) or None)."""
    events = [("i", n, e) for n, e in enumerate(trace["inputs"])]
    events += [("e", n, e) for n, e in enumerate(trace["expected"])]
    events.sort(key=lambda x: x[2]["data"]["seq"])
    seqs = [x[2]["data"]["seq"] for x in events]
    if seqs != list(range(1, len(seqs) + 1)):
        raise ConvertError("seq is not 1..N over inputs and expected")
    out = [{"k": "header", "format": "tick-raw-1"}, {"k": "game", "g": "G", "frame_before": 0}]
    src = [None, None]
    timers = {}
    act_rooms, room_act, pending = {}, {}, set()
    frames = trace["setup"]["frames"]
    by_tick = {}
    for ev in events:
        by_tick.setdefault(ev[2]["tick"], []).append(ev)
    if any(t < 1 or t > frames + 1 for t in by_tick):
        raise ConvertError("event tick outside 1..frames+1")

    def put(rec, ev=None):
        out.append(rec)
        src.append(None if ev is None else (ev[0], ev[1]))

    def raw_of(ev):
        e, d = ev[2], ev[2]["data"]
        k = e["kind"]
        u = d.get("unit")
        if k == "timer_set":
            t = d["timer"]
            timers[t] = d
            return {"k": "set", "tm": f"T{t}", "l": "i" if d["req"] == -1 else "d", "ty": d["type"],
                    "x": d["expire"], "req": d["req"], "ut": u[0], "g": u[1],
                    "c": CLASS_OF_TYPE.get(u[0]), "a1": d["a1"], "a2": d["a2"],
                    "cb": d.get("cb", "0x0")}
        if k == "timer_cancel":
            return {"k": "cancel", "tm": f"T{d['timer']}", "deferred": d["deferred"]}
        if k == "timer_run":
            t = timers[d["timer"]]
            v = t["unit"]
            return {"k": "ex", "c": CLASS_OF_TYPE.get(v[0]), "l": "i" if t["req"] == -1 else "d",
                    "tm": f"T{d['timer']}", "ty": t["type"], "x": t["expire"], "ut": v[0],
                    "g": v[1], "cl": None, "a1": t["a1"], "a2": t["a2"], "cb": "0x0"}
        if k == "hash_add":
            return {"k": "hin", "u": f"U{u[0]}:{u[1]}", "ut": u[0], "g": u[1], "cl": d["class_id"]}
        if k == "hash_remove":
            return {"k": "hout", "u": f"U{u[0]}:{u[1]}", "ut": u[0], "g": u[1]}
        if k in ("room_add", "queue_add"):
            if k == "queue_add":
                pending.add(room_act[d["room"]])
            return {"k": "rin" if k == "room_add" else "qin", "u": f"U{u[0]}:{u[1]}", "ut": u[0],
                    "g": u[1], "r": d["room"]}
        if k in ("room_remove", "queue_remove"):
            return {"k": "rout" if k == "room_remove" else "qout", "u": f"U{u[0]}:{u[1]}",
                    "ut": u[0], "g": u[1]}
        if k == "queue_clear":
            return {"k": "qclear", "r": d["room"]}
        if k == "room_activate":
            act_rooms.setdefault(d["act"], []).insert(0, d["room"])
            room_act[d["room"]] = d["act"]
            return {"k": "ract", "r": d["room"], "a": f"A{d['act']}"}
        if k == "room_deactivate":
            act_rooms[d["act"]].remove(d["room"])
            return {"k": "rdeact", "r": d["room"], "a": f"A{d['act']}"}
        if k == "lists":
            acts = []
            for a, rl in enumerate(d["acts"]):
                acts.append(None if rl is None else {"a": f"A{a}", "rooms": [
                    {"r": x["room"], "u": x["units"], "q": x["queue"], "adj": x["adj"]}
                    for x in rl]})
            return {"k": "snap", "f": e["tick"], "hash": d["hash"], "tiles": d["tiles"],
                    "acts": acts, "clients": [f"C{c}" for c in d["clients"]]}
        raise ConvertError(f"unknown event kind {k}")

    for f in range(1, frames + 2):
        evs = by_tick.get(f, [])
        groups = {}
        for ev in evs:
            groups.setdefault(ev[2]["data"]["step"], []).append(ev)
        order = ["pre"] + (periodic_steps(f)[:-1] if f <= frames else [])
        unknown = set(groups) - set(order)
        if unknown:
            raise ConvertError(f"tick {f}: events in steps {sorted(unknown)} that do not run")
        rebuilt = [ev for s in order for ev in groups.get(s, [])]
        if [ev[2]["data"]["seq"] for ev in rebuilt] != [ev[2]["data"]["seq"] for ev in evs]:
            raise ConvertError(f"tick {f}: step labels disagree with seq order")
        for ev in groups.get("pre", []):
            put(raw_of(ev), ev)
        if f > frames:
            break
        put({"k": "tick", "f": f})
        for s in order[1:]:
            put({"k": "step", "s": s, "f": f})
            for ev in groups.get(s, []):
                put(raw_of(ev), ev)
            if s == "updq":
                for a in sorted(pending):
                    for room in act_rooms.get(a, []):
                        put({"k": "qclear", "r": room})
                pending.clear()
        put({"k": "step", "s": "end", "f": f})
    return out, src


def check_trace(trace, quiet=False):
    """Returns a list of errors, each naming the trace event it points to."""
    try:
        records, src = rebuild(trace)
    except ConvertError as e:
        return [str(e)]
    m = check_tick.check(records, quiet=True)
    errors = []
    for e in m.errors:
        i = int(e.split(":")[0].split()[1])
        j = i
        while j >= 0 and src[j] is None:
            j -= 1
        where = f"{'inputs' if src[j][0] == 'i' else 'expected'}[{src[j][1]}]" if j >= 0 else "?"
        errors.append(f"{where}: {e.split(':', 1)[1].strip()}")
    if not errors:
        try:
            setup, inputs, expected, _ = convert(records)
        except ConvertError as e:
            return [f"round trip: {e}"]
        if inputs != trace["inputs"] or expected != trace["expected"]:
            for name, a, b in (("inputs", inputs, trace["inputs"]),
                               ("expected", expected, trace["expected"])):
                for n, (x, y) in enumerate(zip(a, b)):
                    if x != y:
                        errors.append(f"{name}[{n}]: round trip gives {x}, trace has {y}")
                        break
                else:
                    if len(a) != len(b):
                        errors.append(f"{name}: round trip gives {len(a)} events, trace {len(b)}")
    if not quiet:
        runs = sum(1 for e in trace["expected"] if e["kind"] == "timer_run")
        snaps = len(trace["expected"]) - runs
        print(f"{trace['id']}: {trace['setup']['frames']} ticks, {len(trace['inputs'])} inputs, "
              f"{runs} timer runs, {snaps} snapshots: {len(errors)} errors")
        for e in errors[:20]:
            print("  " + e)
    return errors


def perturb_run(trace, n):
    """Swap the timer runs of the n-th adjacent pair (same tick, nothing between them)."""
    ex = trace["expected"]
    pairs = [j for j in range(len(ex) - 1)
             if ex[j]["kind"] == ex[j + 1]["kind"] == "timer_run"
             and ex[j]["tick"] == ex[j + 1]["tick"]
             and ex[j + 1]["data"]["seq"] == ex[j]["data"]["seq"] + 1
             and ex[j]["data"]["timer"] != ex[j + 1]["data"]["timer"]]
    if n >= len(pairs):
        raise SystemExit(f"only {len(pairs)} swappable run pairs")
    j = pairs[n]
    a, b = ex[j]["data"], ex[j + 1]["data"]
    ex[j]["data"], ex[j + 1]["data"] = dict(b, seq=a["seq"]), dict(a, seq=b["seq"])
    return f"expected[{j}]"


def perturb_lists(trace, n):
    """Reverse one room's unit list in the n-th snapshot (or the next one that has one)."""
    snaps = [j for j, e in enumerate(trace["expected"]) if e["kind"] == "lists"]
    for j in snaps[n:]:
        for rl in trace["expected"][j]["data"]["acts"]:
            for room in rl or []:
                if len(room["units"]) >= 2 and room["units"] != room["units"][::-1]:
                    room["units"].reverse()
                    return f"expected[{j}]"
    raise SystemExit(f"no snapshot with a reversible room list at or after #{n}")


def synthetic():
    """check_tick.py's hand-built recording, completed with what that model
    does not need: run arguments, frame_before, and the update-queue inserts
    moved to just before the frame-4 snapshot so that tick 4's step 6 clears
    them (R2 then R1, the act's list order)."""
    base = check_tick.synthetic()
    queue = [r for r in base if r["k"] == "qin"]
    out, queued = [], False
    for r in base:
        if r["k"] in ("qin", "qclear"):
            continue
        if r["k"] == "snap":
            out += queue
            queued = True
        if r["k"] == "ex":
            r = dict(r, a1=0, a2=0)
        if r["k"] == "game":
            r = dict(r, frame_before=0)
        out.append(r)
        if r["k"] == "step" and r["s"] == "updq" and queued:
            out += [{"k": "qclear", "r": "R2"}, {"k": "qclear", "r": "R1"}]
            queued = False
    return out


def synthetic_v02(raw):
    """`raw` as record_tick.py 0.2.0 writes it: `set` with its optional
    `site`, `cl` and `m`, and an `anim` record after each schedule (the
    fields of README.md "Version 0.2.0", values made up)."""
    out = []
    for r in raw:
        if r["k"] == "set":
            r = dict(r, site="0x5539c7", cl=0, m=1)
        out.append(r)
        if r["k"] == "set":
            out.append({"k": "anim", "fn": "0x5539b0", "f": 1, "ut": r["ut"], "g": r["g"],
                        "cl": 0, "m": 1, "seq": False, "cur": 0, "fc": 8, "sp": 256, "b": 0,
                        "ad": "M0NUHTH", "ad_frames": 8, "ad_speed": 256, "ev": [[4, 1]]})
    return out


def selftest():
    ok = True
    raw = synthetic()
    setup, inputs, expected, header = convert(raw)
    trace = make_trace("sim-9999", setup, inputs, expected, header, "selftest")
    # a 0.2.0 recording converts to the same trace (anim skipped, CH2)
    v02 = synthetic_v02(raw)
    if not any(r["k"] == "anim" for r in v02):
        print("selftest: the 0.2.0 synthetic recording has no anim record")
        ok = False
    try:
        if convert(v02)[:3] != (setup, inputs, expected):
            print("selftest: a 0.2.0 recording converts to another trace")
            ok = False
    except ConvertError as e:
        print("selftest: a 0.2.0 recording is refused:", e)
        ok = False
    # any other unknown record kind is still refused (M07)
    odd = json.loads(json.dumps(v02))
    odd.insert(next(i for i, r in enumerate(odd) if r["k"] == "anim"), {"k": "anym"})
    try:
        convert(odd)
        print("selftest: an unknown record kind was accepted")
        ok = False
    except ConvertError:
        pass
    if check_trace(trace, quiet=True):
        print("selftest: the converted synthetic recording fails:", check_trace(trace, True))
        ok = False
    for name, fn in (("run swap", perturb_run), ("list reversal", perturb_lists)):
        for n in range(2):
            t = json.loads(json.dumps(trace))
            try:
                at = fn(t, n)
            except SystemExit:
                break
            errs = check_trace(t, quiet=True)
            if not errs or not errs[0].startswith(at + ":"):
                print(f"selftest: {name} at {at} reported as {errs[:1]}")
                ok = False
    # a broken raw input is refused (M07)
    bad = json.loads(json.dumps(raw))
    del bad[next(i for i, r in enumerate(bad) if r["k"] == "qclear" and r["r"] == "R2")]
    try:
        convert(bad)
        print("selftest: a step-6 clear the rule does not give was accepted")
        ok = False
    except ConvertError:
        pass
    print("selftest:", "pass" if ok else "FAIL")
    return ok


def next_id():
    used = []
    for d in (OUT_DIR, os.path.join(REPO, "traces", "sim", "rng")):
        if os.path.isdir(d):
            used += [int(m.group(1)) for f in os.listdir(d)
                     if (m := re.fullmatch(r"sim-(\d+)\.json", f))]
    return f"sim-{max(used, default=0) + 1:04d}"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("files", nargs="*")
    ap.add_argument("--id", help="trace id (default: next free sim-NNNN)")
    ap.add_argument("--note", default="")
    ap.add_argument("--check", action="store_true", help="check traces instead of converting")
    ap.add_argument("--perturb-run", type=int, help="with --check: swap the N-th run pair")
    ap.add_argument("--perturb-lists", type=int, help="with --check: reverse a list in snapshot N")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(0 if selftest() else 1)
    if not a.files:
        ap.error("files required")
    if a.check:
        bad = 0
        for p in a.files:
            trace = json.load(open(p, encoding="utf-8"))
            if a.perturb_run is not None:
                print("perturbed:", perturb_run(trace, a.perturb_run))
            if a.perturb_lists is not None:
                print("perturbed:", perturb_lists(trace, a.perturb_lists))
            bad += bool(check_trace(trace))
        sys.exit(1 if bad else 0)
    for p in a.files:
        records = check_tick.load(p)
        try:
            setup, inputs, expected, header = convert(records, os.path.basename(p))
        except ConvertError as e:
            sys.exit(f"{p}: {e}")
        tid = a.id or next_id()
        trace = make_trace(tid, setup, inputs, expected, header, a.note)
        errs = check_trace(trace, quiet=True)
        if errs:
            sys.exit(f"{p}: the converted trace fails its own check: {errs[0]}")
        os.makedirs(OUT_DIR, exist_ok=True)
        out = os.path.join(OUT_DIR, tid + ".json")
        with open(out, "w", encoding="utf-8", newline="\n") as f:
            f.write(dumps(trace))
        print(f"wrote {out}: {setup['frames']} ticks, {len(inputs)} inputs, "
              f"{len(expected)} expected, {os.path.getsize(out)} bytes")
        a.id = None


if __name__ == "__main__":
    main()
