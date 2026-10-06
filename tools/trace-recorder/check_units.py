"""Check the per-kind timer-event rules of specs/sim/units.md on a tick
recording (record_tick.py, format tick-raw-1).

Rules checked (units.md §6, Test vectors; rule ids U1-U11 are the spec's):

  U1  every scheduled or run event (unit kind, event type, list) is one a
      1.14d scheduler can produce (specs/sim/unit-events.tsv), and object
      and item events have a handler (specs/sim/unit-handlers.tsv);
      with a `site` field (record_tick 0.2.0) the exact TSV row must match:
      type, kind, list, callback, constant arguments
  U2  callbacks: null, except type 14 with 0x554570
  U3  every-tick type-0 events carry arguments 0, 0
  U4  mode schedules of players and monsters: cancel first (no live type-0
      or type-1 timer of the unit), action events then one ENDANIM, action
      event arguments and ordering; with an `anim` record the whole list is
      recomputed from the unit's animation fields (units.md §4.2)
  U5  player STATREGEN: expire frame + 1, rescheduled first thing in every run
  U6  monster STATREGEN: expire frame + 1, arguments 0
  U7  player event 11: frame + 250 at join, frame + 30 from every run
  U8  object ENDANIM: frame + FrameCnt1 (+1, or twice; per site)
  U9  object event 2: frame + Parm0 + 1
  U10 a monster's AI think scheduled during its own ENDANIM run:
      frame + aidel (Normal column; 15 when 0) or frame + 45
  U11 with `site`: every TSV expire rule that is a formula of the frame and
      table data (f+N, f+fc1..., f+parm..., f+aidel) holds exactly

Tables (monstats, objects) come from a dump_tables.py directory (default:
the newest traces/raw/*-tables). Exit status 1 on any mismatch.
--perturb N adds 1 to the requested expire of the N-th exactly checked
schedule; the check must then fail at exactly that record (METHODS M08).
--selftest runs a hand-built recording, plain and with every such
perturbation.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import glob
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
EVENTS_TSV = os.path.join(REPO, "specs", "sim", "unit-events.tsv")
HANDLERS_TSV = os.path.join(REPO, "specs", "sim", "unit-handlers.tsv")
KIND = {0: "player", 1: "monster", 2: "object", 3: "missile", 4: "item"}
API_LIST = {"timed": "d", "timed_cb": "d", "every": "i"}
COOLDOWN_CB = 0x554570
MODE_FUNCS = {0x5539B0, 0x553B10, 0x553C70, 0x553DC0, 0x553F00}  # units.md §4.2, §4.4


def load_tsv(path):
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    head = lines[0].split("\t")
    return [dict(zip(head, ln.split("\t"))) for ln in lines[1:] if ln]


class Tables:
    """monstats aidel and objects FrameCnt1 / Parm0 / Parm1 by class id."""

    def __init__(self, mon=None, obj=None):
        self.mon, self.obj = mon or {}, obj or {}

    @classmethod
    def from_dump(cls, d):
        man = json.load(open(os.path.join(d, "manifest.json"), encoding="utf-8"))["tables"]
        t = cls()
        ms = man["monstats"]["record_size"]
        raw = open(os.path.join(d, "monstats.bin"), "rb").read()
        for c in range(len(raw) // ms):
            t.mon[c] = {"aidel": raw[c * ms + 79]}           # aidel, Normal (fields.tsv)
        os_ = man["objects"]["record_size"]
        raw = open(os.path.join(d, "objects.bin"), "rb").read()
        for c in range(len(raw) // os_):
            fc1, = struct.unpack_from("<I", raw, c * os_ + 0xDC)  # FrameCnt1 (<< 8)
            p0, p1 = struct.unpack_from("<II", raw, c * os_ + 0x178)  # Parm0, Parm1
            t.obj[c] = {"fc1": fc1 >> 8, "parm0": p0, "parm1": p1}
        return t


def anim_schedule(r):
    """The (type, expire, a1, a2) list a mode schedule produces (units.md §4.2),
    or None when it cannot be computed from the record (sequence animations,
    event indexes outside the logged array)."""
    if r["seq"]:
        return None
    f, fc, sp = r["f"], r["fc"], r["sp"]
    ev = {i: b for i, b in r.get("ev", [])}
    fn = r["fn"]
    if fn == "0x553c70" and r["arg"] <= 0:
        return []
    if sp == 0:
        return [(1, f + 1, 0, 0)]

    def event(i):
        if 0 <= i < 144:
            return ev.get(i, 0)
        if i == -1:
            return (r.get("ad_speed", 0) >> 24) & 0xFF   # byte before the event array
        raise ValueError(i)

    out, n = [], f
    if fn == "0x5539b0":
        idx, param, af = r["b"], 0, (r["b"] << 8) + sp
        while af < fc:
            n += 1
            top = af >> 8
            while idx <= top and idx < 144:
                e = event(idx)
                if e in (1, 2, 4):
                    out.append((0, n, e, param))
                    param += 1
                elif e == 3:
                    out.append((0, n, 3, 0))
                idx += 1
            af += sp
    else:
        cur = r["cur"] >> 8
        if fn == "0x553b10":
            d = (100 - r["arg"]) * (f - cur)
            calc = abs(d) // 100 * (1 if d >= 0 else -1)
        elif fn == "0x553c70":
            calc = f - cur - r["arg"]
        else:
            calc = r["arg"]
        idx, af = calc - 1, (calc << 8) + sp
        while af < fc:
            n += 1
            while idx <= af >> 8:
                try:
                    e = event(idx)
                except ValueError:
                    return None
                if 1 <= e <= 4:
                    out.append((0, n, e, 0))
                idx += 1
            af += sp
    if n == f:
        n += 1
    out.append((1, n + 1, 0, 0))
    return out


class Checker:
    def __init__(self, tables, events=None, handlers=None):
        self.t = tables
        rows = events if events is not None else load_tsv(EVENTS_TSV)
        self.by_site = {}
        self.combos = set()
        for row in rows:
            self.by_site.setdefault(int(row["site"], 16), []).append(row)
            for c in row["classes"].split(","):
                self.combos.add((c, int(row["type"]), API_LIST[row["api"]]))
        hrows = handlers if handlers is not None else load_tsv(HANDLERS_TSV)
        self.handler = {(h["class"], int(h["type"])): h["handler"] for h in hrows}
        self.errors = []          # (record index, message)
        self.exact = []           # record indexes of schedules checked exactly
        self.counts = {}
        self.frame = None
        self.cls = {}             # (ut, g) -> class id
        self.live = {}            # tm -> (ut, g, ty, l)
        self.run = None           # the executing run: dict
        self.group = None         # current mode-schedule group
        self.anim = None          # last anim record (index, record)
        self.joined = set()

    def err(self, i, rule, msg):
        self.errors.append((i, f"record {i}: {rule} {msg}"))

    def count(self, k):
        self.counts[k] = self.counts.get(k, 0) + 1

    # --- group (U4) ---------------------------------------------------------

    def close_group(self):
        g, self.group = self.group, None
        if not g:
            return
        sets = g["sets"]
        types = [r["ty"] for _, r in sets]
        i_end = sets[-1][0]
        if types.count(1) != 1 or types[-1] != 1:
            self.err(i_end, "U4", f"mode schedule of {g['u']} ends without exactly one "
                                  f"ENDANIM: types {types}")
            return
        f = g["f"]
        end_x = sets[-1][1]["req"]
        acts = sets[:-1]
        a2s = [r["a2"] for _, r in acts if r["a1"] in (1, 2, 4)]
        numbered = a2s == list(range(len(a2s)))
        last = f
        for i, r in acts:
            if not 1 <= r["a1"] <= 4:
                self.err(i, "U4", f"action event argument {r['a1']} outside 1..4")
            if r["req"] < last or r["req"] >= end_x:
                self.err(i, "U4", f"action event at {r['req']}: not in order before the "
                                  f"ENDANIM at {end_x}")
            last = r["req"]
            if r["a1"] == 3 and r["a2"] != 0 or not numbered and r["a2"] != 0:
                self.err(i, "U4", f"action event numbering a2={r['a2']} (units.md §4.2)")
        if acts and end_x < f + 2 or not acts and end_x < f + 1:
            self.err(i_end, "U4", f"ENDANIM at {end_x} too early for frame {f}")
        self.count("U4 groups")
        if g.get("anim") is not None:
            ai, ar = g["anim"]
            want = anim_schedule(ar)
            if want is None:
                self.count("U4 anim not computable")
                return
            got = [(r["ty"], r["req"], r["a1"], r["a2"]) for _, r in sets]
            if got != want:
                k = next((j for j, (a, b) in enumerate(zip(got, want)) if a != b),
                         min(len(got), len(want)))
                at = sets[min(k, len(sets) - 1)][0]
                self.err(at, "U4", f"mode schedule {got}, animation gives {want} (anim record {ai})")
            else:
                self.count("U4 anim exact")
                self.exact.extend(i for i, _ in sets)

    def site_fn(self, site):
        rows = self.by_site.get(int(site, 16))
        return int(rows[0]["function"], 16) if rows else None

    def live_mode_timers(self, u):
        return [v for v in self.live.values() if (v[0], v[1]) == u and v[2] in (0, 1)]

    # --- records --------------------------------------------------------------

    def on_set(self, i, r):
        ut, g, ty, l = r["ut"], r["g"], r["ty"], r["l"]
        if ut is None:
            return
        kind = KIND.get(ut)
        u = (ut, g)
        if r.get("cl") is not None:
            self.cls[u] = r["cl"]
        f = self.frame
        req = r["req"] if r.get("req") is not None else r["x"]
        r = dict(r, req=req)
        cb = int(r["cb"], 16)
        self.count("sets")
        # U1 / U2 / U11 -------------------------------------------------------
        if (kind, ty, l) not in self.combos:
            self.err(i, "U1", f"{kind} event type {ty} list {l}: no 1.14d scheduler")
        if kind in ("object", "item") and self.handler.get((kind, ty)) == "null":
            self.err(i, "U1", f"{kind} event type {ty} has no handler (fatal in 1.14d)")
        if cb != 0 and not (ty == 14 and cb == COOLDOWN_CB):
            self.err(i, "U2", f"callback {r['cb']} for type {ty}")
        if "site" in r:
            self.by_row(i, r, kind, f)
        # U3 ---------------------------------------------------------------------
        if l == "i" and ty == 0 and (r["a1"], r["a2"]) != (0, 0):
            self.err(i, "U3", f"every-tick type 0 with arguments {r['a1']}, {r['a2']}")
        # U4 ---------------------------------------------------------------------
        # with a site, only the SUnit mode schedulers (units.md §4.2, §4.4) form groups
        mode_set = kind in ("player", "monster") and (ty == 1 or ty == 0) and (
            "site" not in r or self.site_fn(r["site"]) in MODE_FUNCS)
        if self.group and (not mode_set or self.group["u"] != u or l == "i"):
            self.close_group()
        if mode_set:
            if l == "i":
                if self.live_mode_timers(u):
                    self.err(i, "U4", f"every-tick type 0 for {u} while a type-0/1 timer is live")
            else:
                if self.group is None:
                    if self.live_mode_timers(u):
                        self.err(i, "U4", f"mode schedule for {u} while a type-0/1 timer is live")
                    a = self.anim
                    self.group = {"u": u, "f": f, "sets": [],
                                  "anim": a if a and (a[1]["ut"], a[1]["g"]) == u
                                  and a[1]["f"] == f else None}
                    self.anim = None
                self.group["sets"].append((i, r))
                if ty == 1:
                    self.close_group()
        # U5 / U6 / U7 -----------------------------------------------------------
        run = self.run
        if kind == "player" and ty == 3:
            self.expect(i, "U5", req, f + 1)
            if run and run["u"] == u and run["ty"] == 3 and not run["resched"]:
                run["resched"] = True
                if (r["a1"], r["a2"]) != run["args"]:
                    self.err(i, "U5", f"reschedule arguments {(r['a1'], r['a2'])}, run had {run['args']}")
        if kind == "monster" and ty == 3:
            self.expect(i, "U6", req, f + 1)
            if (r["a1"], r["a2"]) != (0, 0):
                self.err(i, "U6", f"arguments {(r['a1'], r['a2'])}")
        if kind == "player" and ty == 11:
            if run and run["u"] == u and run["ty"] == 11:
                self.expect(i, "U7", req, f + 30)
                run["resched11"] = run.get("resched11", 0) + 1
            elif u not in self.joined:
                self.expect(i, "U7", req, f + 250)
            else:
                self.err(i, "U7", f"event 11 for {u} outside its run after the join")
            self.joined.add(u)
        # U8 / U9 / U10 ------------------------------------------------------------
        c = self.cls.get(u)
        if kind == "object" and ty in (1, 2) and "site" not in r:
            o = self.t.obj.get(c)
            if o is None:
                self.count("object rules skipped (no table)")
            elif ty == 1:
                fc = o["fc1"]
                if req not in (f + fc + 1, f + fc, f + 2 * fc):
                    self.err(i, "U8", f"object {c} ENDANIM at {req}, FrameCnt1 {fc} gives "
                                      f"{f + fc + 1} (or {f + fc}, {f + 2 * fc})")
                else:
                    self.count("U8 object ENDANIM")
                    if req == f + fc + 1:
                        self.exact.append(i)
            else:
                self.expect(i, "U9", req, f + o["parm0"] + 1)
        if kind == "monster" and ty == 2 and run and run["u"] == u and run["ty"] == 1 \
                and "site" not in r:
            m = self.t.mon.get(c)
            if m is None:
                self.count("monster rules skipped (no table)")
            else:
                d = m["aidel"] or 15
                if req not in (f + d, f + 45):
                    self.err(i, "U10", f"monster {c} AI think at {req}, aidel gives {f + d}")
                else:
                    self.count("U10 AI after ENDANIM")
                    if req == f + d:
                        self.exact.append(i)
        self.live[r["tm"]] = (ut, g, ty, l)

    def expect(self, i, rule, got, want):
        if got != want:
            self.err(i, rule, f"expire {got}, rule gives {want}")
        else:
            self.count(rule)
            self.exact.append(i)

    def by_row(self, i, r, kind, f):
        """U1/U11 against the exact TSV row of the call site."""
        rows = [row for row in self.by_site.get(int(r["site"], 16), [])
                if int(row["type"]) == r["ty"]]
        if not rows:
            self.err(i, "U1", f"site {r['site']} type {r['ty']}: not a 1.14d scheduler")
            return
        row = rows[0]
        if kind not in row["classes"].split(","):
            self.err(i, "U1", f"site {r['site']} schedules for {kind}, TSV says {row['classes']}")
        if API_LIST[row["api"]] != r["l"]:
            self.err(i, "U1", f"site {r['site']} list {r['l']}, TSV api {row['api']}")
        if int(row["callback"], 16) != int(r["cb"], 16):
            self.err(i, "U2", f"site {r['site']} callback {r['cb']}, TSV {row['callback']}")
        for a in ("a1", "a2"):
            if row[a] not in ("reg", "?") and int(row[a], 0) != r[a]:
                self.err(i, "U1", f"site {r['site']} {a} {r[a]}, TSV {row[a]}")
        e = row["expire"]
        c = self.cls.get((r["ut"], r["g"]))
        alts = []
        for tok in e.split("|"):
            if tok.startswith("f+") and tok[2:].isdigit():
                alts.append(f + int(tok[2:]))
            elif tok in ("f+fc1+1", "f+fc1", "f+2*fc1", "f+parm0+1", "f+parm1+1"):
                o = self.t.obj.get(c)
                if o is None:
                    return
                alts.append({"f+fc1+1": f + o["fc1"] + 1, "f+fc1": f + o["fc1"],
                             "f+2*fc1": f + 2 * o["fc1"], "f+parm0+1": f + o["parm0"] + 1,
                             "f+parm1+1": f + o["parm1"] + 1}[tok])
            elif tok == "f+aidel":
                m = self.t.mon.get(c)
                if m is None:
                    return
                alts.append(f + (m["aidel"] or 15))
            elif tok == "-1":
                alts.append(-1)
            else:
                self.count("U11 unchecked expire rule")
                return
        if r["req"] not in alts:
            self.err(i, "U11", f"site {r['site']} expire {r['req']}, TSV rule {e} gives {alts}")
        else:
            self.count("U11 site rule")
            self.exact.append(i)

    def on_ex(self, i, r):
        self.end_run(i)
        self.close_group()
        self.count("runs")
        u = (r["ut"], r["g"])
        if r.get("cl") is not None and r["ut"] is not None:
            self.cls[u] = r["cl"]
        kind = KIND.get(r["ut"])
        if kind in ("object", "item") and self.handler.get((kind, r["ty"])) == "null":
            self.err(i, "U1", f"{kind} event type {r['ty']} ran without a handler")
        if r["ut"] is not None and (kind, r["ty"], r["l"]) not in self.combos:
            self.err(i, "U1", f"{kind} event type {r['ty']} list {r['l']} ran: no 1.14d scheduler")
        self.run = {"i": i, "u": u, "ty": r["ty"], "args": (r["a1"], r["a2"]), "resched": False,
                    "kind": kind}
        if r["l"] == "d":
            self.live.pop(r["tm"], None)

    def end_run(self, i):
        run, self.run = self.run, None
        if not run:
            return
        if run["kind"] == "player" and run["ty"] == 3 and not run["resched"]:
            self.err(run["i"], "U5", f"player STATREGEN run of {run['u']} did not reschedule")
        if run["kind"] == "player" and run["ty"] == 11 and run.get("resched11", 0) != 1:
            self.err(run["i"], "U7", f"player event 11 run of {run['u']} rescheduled "
                                     f"{run.get('resched11', 0)} times")

    def on_cancel(self, i, r):
        self.close_group()
        self.live.pop(r["tm"], None)

    def on_boundary(self, i, r):
        self.end_run(i)
        self.close_group()
        if r["k"] == "tick":
            self.frame = r["f"]

    def on_anim(self, i, r):
        self.close_group()
        self.anim = (i, r)
        self.count("anim records")

    def on_hin(self, i, r):
        self.cls[(r["ut"], r["g"])] = r["cl"]


def check(records, tables, out=sys.stdout, quiet=False, events=None, handlers=None):
    c = Checker(tables, events, handlers)
    h = {"set": c.on_set, "ex": c.on_ex, "cancel": c.on_cancel, "tick": c.on_boundary,
         "step": c.on_boundary, "anim": c.on_anim, "hin": c.on_hin}
    for i, r in enumerate(records):
        fn = h.get(r["k"])
        if fn:
            fn(i, r)
    c.end_run(len(records))
    c.close_group()
    c.errors.sort(key=lambda e: e[0])
    if not quiet:
        print(f"sets {c.counts.get('sets', 0)}, runs {c.counts.get('runs', 0)}, exactly "
              f"checked schedules {len(set(c.exact))}, errors {len(c.errors)}", file=out)
        for k in sorted(c.counts):
            if k not in ("sets", "runs"):
                print(f"  {k}: {c.counts[k]}", file=out)
        for _, e in c.errors[:50]:
            print("  " + e, file=out)
    return c


def perturb(records, tables, n):
    exact = sorted(set(check(records, tables, quiet=True).exact))
    if n >= len(exact):
        raise SystemExit(f"only {len(exact)} exactly checked schedules")
    i = exact[n]
    r = records[i]
    r["req"] = (r["req"] if r.get("req") is not None else r["x"]) + 1
    if r["x"] != -1:
        r["x"] += 1
    return i


def newest_tables():
    dirs = sorted(d for d in glob.glob(os.path.join(REPO, "traces", "raw", "*-tables"))
                  if os.path.exists(os.path.join(d, "manifest.json")))
    return dirs[-1] if dirs else None


def synthetic():
    """A recording built by hand from units.md's rules (Test vectors), with
    its own small tables."""
    tables = Tables({5: {"aidel": 10}, 6: {"aidel": 0}},
                    {7: {"fc1": 21, "parm0": 750, "parm1": 0}})
    rec = [{"k": "header", "format": "tick-raw-1"}, {"k": "game", "g": "0x1"},
           {"k": "hin", "u": "P", "ut": 0, "g": 1, "cl": 4},
           {"k": "hin", "u": "M", "ut": 1, "g": 1, "cl": 5},
           {"k": "hin", "u": "N", "ut": 1, "g": 2, "cl": 6},
           {"k": "hin", "u": "O", "ut": 2, "g": 1, "cl": 7}]

    def s(tm, ut, g, ty, x, l="d", a1=0, a2=0, cb="0x0", req=None, **kw):
        d = {"k": "set", "l": l, "tm": tm, "ty": ty, "x": x, "req": x if req is None else req,
             "ut": ut, "g": g, "c": None, "a1": a1, "a2": a2, "cb": cb}
        d.update(kw)
        return d

    def ex(tm, ut, g, ty, x, l="d", a1=0, a2=0):
        return {"k": "ex", "c": 0, "l": l, "tm": tm, "ty": ty, "x": x, "ut": ut, "g": g,
                "cl": None, "a1": a1, "a2": a2, "cb": "0x0"}

    def tick(f):
        return [{"k": "tick", "f": f}, {"k": "step", "s": "events", "f": f}]

    rec += tick(1)
    rec += [s("R", 0, 1, 3, 2), s("Q", 0, 1, 11, 251)]                    # join (U5, U7)
    rec += tick(2)
    rec += [ex("R", 0, 1, 3, 2), s("R", 0, 1, 3, 3)]                      # regen (U5)
    # player attack: frames 10, speed 256, bonus 0, action frame at index 5 (U4)
    rec += [{"k": "anim", "fn": "0x5539b0", "f": 2, "ut": 0, "g": 1, "cl": 4, "m": 7,
             "seq": False, "cur": 0, "fc": 2560, "sp": 256, "b": 0, "ad": "AMA11HS",
             "ad_frames": 10, "ad_speed": 256, "ev": [[5, 1]]},
            s("A0", 0, 1, 0, 7, a1=1, a2=0, site="0x553a8f", cl=4, m=7),
            s("A1", 0, 1, 1, 12, site="0x553ae2", cl=4, m=7)]
    # object: ENDANIM frame + FrameCnt1 + 1 (U8), event 2 frame + Parm0 + 1 (U9)
    rec += [s("O1", 2, 1, 1, 24), s("O2", 2, 1, 2, 753)]
    # monster with a speed-0 schedule (U4), then its ENDANIM run schedules AI (U10)
    rec += [s("M1", 1, 1, 1, 3)]
    rec += tick(3)
    rec += [ex("R", 0, 1, 3, 3), s("R", 0, 1, 3, 4),
            ex("M1", 1, 1, 1, 3), s("M2", 1, 1, 2, 13), s("M3", 1, 1, 3, 4)]
    # a second monster: walk (every-tick type 0) after a cancel, AI think aidel 0 -> 15
    rec += [s("W", 1, 2, 0, -1, l="i", req=-1), {"k": "cancel", "tm": "W", "deferred": False},
            s("N1", 1, 2, 1, 4)]
    rec += tick(4)
    rec += [ex("N1", 1, 2, 1, 4), s("N2", 1, 2, 2, 19),
            ex("R", 0, 1, 3, 4), s("R", 0, 1, 3, 5)]
    # site-tagged schedules (record_tick 0.2.0): object FREEHOVER +300, monster
    # neutral AI think (f + aidel)
    rec += [s("O3", 2, 1, 6, 304, site="0x583d6e", cl=7, m=0),
            s("M4", 1, 1, 2, 14, site="0x5a747d", cl=5, m=1)]
    rec += tick(251)
    rec += [ex("Q", 0, 1, 11, 251), s("Q", 0, 1, 11, 281)]
    return rec, tables


def selftest():
    ok = True
    base, tables = synthetic()
    c = check(base, tables, quiet=True)
    if c.errors:
        print("selftest: the synthetic recording fails:", *[e for _, e in c.errors], sep="\n  ")
        ok = False
    exact = sorted(set(c.exact))
    if len(exact) < 12:
        print(f"selftest: only {len(exact)} exactly checked schedules: {exact}")
        ok = False
    for n in range(len(exact)):
        rec = json.loads(json.dumps(base))
        at = perturb(rec, tables, n)
        m = check(rec, tables, quiet=True)
        first = m.errors[0][0] if m.errors else None
        if first != at:
            print(f"selftest: perturbation at record {at} reported at {first}: "
                  f"{[e for _, e in m.errors[:2]]}")
            ok = False
    # removing the regen reschedule must be reported at the run
    rec = json.loads(json.dumps(base))
    k = next(i for i, r in enumerate(rec) if r["k"] == "ex" and r["ty"] == 3)
    del rec[k + 1]
    m = check(rec, tables, quiet=True)
    if not m.errors or m.errors[0][0] != k:
        print(f"selftest: missing reschedule after run {k} not reported there")
        ok = False
    print(f"selftest: {'pass' if ok else 'FAIL'} ({len(exact)} perturbations)")
    return ok


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw", nargs="?", help="traces/raw/<time>-tick.jsonl")
    ap.add_argument("--tables", help="dump_tables.py output directory (default: newest)")
    ap.add_argument("--perturb", type=int, help="shift the N-th exactly checked schedule (must fail)")
    ap.add_argument("--selftest", action="store_true", help="check the checker on a synthetic recording")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(0 if selftest() else 1)
    if not a.raw:
        ap.error("a recording or --selftest is required")
    d = a.tables or newest_tables()
    if not d:
        sys.exit("no table dump found (run dump_tables.py, or pass --tables)")
    tables = Tables.from_dump(d)
    with open(a.raw, encoding="utf-8") as f:
        records = [json.loads(line) for line in f if line.strip()]
    if records[0].get("format") != "tick-raw-1":
        sys.exit(f"{a.raw}: not a tick-raw-1 recording")
    print(f"tables: {os.path.relpath(d, REPO)}")
    if a.perturb is not None:
        print(f"perturbed: requested expire + 1 at record {perturb(records, tables, a.perturb)}")
    c = check(records, tables)
    sys.exit(1 if c.errors else 0)


if __name__ == "__main__":
    main()
