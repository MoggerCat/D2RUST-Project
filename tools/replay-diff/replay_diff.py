"""replay-diff: replay a recorded 1.14d input stream on d2rs and compare the
state per frame over long runs (specs/tools/replay-diff.md).

    replay_diff.py run SESSION.replay [--work DIR] [--reuse] [--orig-only | --d2rs-only]
        [--types T,..] [--ignore F,..] [--next N] [--bucket N] [--json FILE] [--dry-run]
    replay_diff.py compare ORIG.state.jsonl D2RS.state.jsonl [--packets D2RS.packets.jsonl]
        [--types T,..] [--ignore F,..] [--next N] [--bucket N] [--from F] [--to F]
        [--json FILE]
    replay_diff.py sends ORIG.state.jsonl          (the --send lines d2rs gets)
    replay_diff.py --selftest

`run`: builds the session's save, records 1.14d once with record_replay.py
(record_state.py + the C->S tap: the state after every tick and every C->S
message with its frame), replays the recorded C->S messages on d2rs
(`d2-client state-dump --send "F hex ..."` per message, the session's pokes,
no input script) with its packets recorded, then `compare`.

`compare` streams both files (frame by frame; 10^4+ frames in constant
memory), prints the first state divergence, the next N, and the divergence
rate after it (frames with any difference / frames compared from the first
divergence on), per unit type and per field, per --bucket frames; and the
input channel: the C->S messages d2rs' server drained per frame against
1.14d's (the replay's own fidelity).

Exit: 0 MATCH, 1 DIVERGED, 2 PARTIAL, 3 error. Standard library only.
Our own code.
"""

import argparse
import json
import os
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(REPO, "tools", "trace-recorder"))
sys.path.insert(0, os.path.join(REPO, "tools", "scenario-diff"))
import state_diff  # noqa: E402  (FIELDS, TYPE_NAMES: the state-1 comparison order)
import packets_diff  # noqa: E402  (the C->S mask table and its reader)

TOOL = "replay-diff 0.1.0"
SUMMARY_FORMAT = "replay-summary-1"
VERDICTS = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR"}
FIELDS = state_diff.FIELDS
TYPE_NAMES = state_diff.TYPE_NAMES

# PROVISIONAL (specs/tools/replay-diff.md §3 r2, REC-1370): the C->S ids the
# d2rs headless bridge sends on its own (state-dump: 0x67 create before the
# first pump, 0x6B the answer to S->C 0x02, 0x5F the position resync); they
# are not replayed, the input channel compares them where each side sent
# them. Settled by the first replay whose input channel shows these ids in
# the same frames on both sides.
BRIDGE_OWN = frozenset({0x5F, 0x67, 0x6B})


class ReplayError(Exception):
    pass


# --- reading ------------------------------------------------------------------

def read_lines(path):
    """Yields the JSON records of a JSON-lines file, in order."""
    try:
        f = open(path, encoding="utf-8")
    except OSError as e:
        raise ReplayError(f"{path}: {e}")
    with f:
        for n, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except ValueError as e:
                raise ReplayError(f"{path}:{n}: not JSON ({e})")


class StateStream:
    """One state-1 file read once, front to back: header first, then
    `snaps()` yields the snapshots in file order; the c2s lines (record_replay.py)
    and the footer are kept as they pass."""

    def __init__(self, path):
        self.path = path
        self.it = read_lines(path)
        self.header = next(self.it, None)
        if not self.header or self.header.get("k") != "header" or \
                self.header.get("format") != state_diff.FORMAT:
            raise ReplayError(f"{path}: not a {state_diff.FORMAT} file (first line must be "
                              f"its header)")
        self.footer = None
        self.c2s = []

    def snaps(self):
        last = None
        for rec in self.it:
            k = rec.get("k")
            if k == "snap":
                f = rec["f"]
                if last is not None and f <= last:
                    raise ReplayError(f"{self.path}: frame {f} after {last} (frames must "
                                      f"ascend)")
                last = f
                yield rec
            elif k == "c2s":
                self.c2s.append(rec)
            elif k == "footer":
                self.footer = rec


def orig_c2s(path):
    """The 1.14d C->S messages of a record_replay.py file: [(f, q, bytes)]."""
    out = []
    for rec in read_lines(path):
        if rec.get("k") == "c2s":
            out.append((rec["f"], rec["q"], bytes.fromhex(rec["bytes"])))
    return out


def d2rs_c2s(path):
    """The C->S messages d2rs' server drained (packets-raw-1 `c2s` / `c2s_sys`),
    as [(window, q, bytes)]: window = frame + 1 (the drain before tick
    frame + 1, packets-trace.md §3 rule 1), None before the first tick."""
    out = []
    first = True
    for rec in read_lines(path):
        if first:
            first = False
            if rec.get("type") != "header" or rec.get("format") != packets_diff.FORMAT:
                raise ReplayError(f"{path}: not a {packets_diff.FORMAT} file")
            continue
        t = rec.get("type")
        if t in ("c2s", "c2s_sys"):
            fr = rec.get("frame")
            w = None if fr is None else (fr if rec.get("phase") == "tick" else fr + 1)
            out.append((w, "game" if t == "c2s" else "sys", bytes.fromhex(rec["bytes"])))
    return out


# --- the d2rs input -----------------------------------------------------------

def replay_sends(c2s):
    """The `--send` values that replay `c2s` on d2rs, and what was left out.
    Every message with a frame whose id is not one the bridge sends itself
    (BRIDGE_OWN), in recorded order; state-dump injects all sends of frame F
    in the drain before tick F, in command-line order (scenario-diff.md §2
    `at … send`)."""
    sends, skipped = [], {"before-first-tick": 0, "bridge-own": 0, "empty": 0}
    for f, _q, b in c2s:
        if f is None or f < 1:
            skipped["before-first-tick"] += 1
        elif not b:
            skipped["empty"] += 1
        elif b[0] in BRIDGE_OWN:
            skipped["bridge-own"] += 1
        else:
            sends.append(f"{f} hex " + " ".join(f"{x:02x}" for x in b))
    return sends, skipped


# --- the state comparison -----------------------------------------------------

def frame_diffs(x, y, fields, types):
    """The differences of one frame, in state-snapshot.md §4 rule 3 order."""
    out = []
    if x.get("seed") != y.get("seed"):
        out.append({"f": x["f"], "unit": None, "field": "seed", "exp": x.get("seed"),
                    "got": y.get("seed"), "a": None, "b": None})
    ua = {(u["ut"], u["g"]): u for u in x["units"] if types is None or u["ut"] in types}
    ub = {(u["ut"], u["g"]): u for u in y["units"] if types is None or u["ut"] in types}
    n = 0
    for k in sorted(set(ua) | set(ub)):
        p, q = ua.get(k), ub.get(k)
        if q is None:
            out.append({"f": x["f"], "unit": k, "field": "(unit)", "exp": "present",
                        "got": "missing", "a": p, "b": None})
            continue
        if p is None:
            out.append({"f": x["f"], "unit": k, "field": "(unit)", "exp": "absent",
                        "got": "extra", "a": None, "b": q})
            continue
        n += 1
        for fld in fields:
            if p.get(fld) != q.get(fld):
                out.append({"f": x["f"], "unit": k, "field": fld,
                            "exp": p.get(fld, "(absent)"), "got": q.get(fld, "(absent)"),
                            "a": p, "b": q})
    return out, n


class Stats:
    """Everything `compare` reports, accumulated frame by frame."""

    def __init__(self, limit, bucket):
        self.limit, self.bucket = limit, max(1, bucket)
        self.frames = 0
        self.lo = self.hi = None
        self.units = 0
        self.divs = []                 # the first divergence and the next `limit`
        self.total = 0
        self.first = None              # frame of the first divergence
        self.after = 0                 # frames compared from the first divergence on
        self.after_div = 0             # ... of them with a difference
        self.equal_runs = []           # (start, end) of equal stretches after the first
        self.run_start = None
        self.longest_diverged_run = 0
        self.cur_div_run = 0
        self.by_field = {}             # field -> [first div, pairs]
        self.by_type = {}              # type (or "game") -> [first frame, frames diverged]
        self.buckets = {}              # bucket start -> [frames, diverged]

    def add_frame(self, f, diffs, units):
        self.frames += 1
        self.units += units
        self.lo = f if self.lo is None else self.lo
        self.hi = f
        b = self.buckets.setdefault(f - f % self.bucket, [0, 0])
        b[0] += 1
        if diffs:
            b[1] += 1
        if diffs and self.first is None:
            self.first = f
        if self.first is not None:
            self.after += 1
            if diffs:
                self.after_div += 1
                self.cur_div_run += 1
                self.longest_diverged_run = max(self.longest_diverged_run, self.cur_div_run)
                if self.run_start is not None:
                    self.equal_runs.append((self.run_start, self.prev))
                    self.run_start = None
            else:
                self.cur_div_run = 0
                if self.run_start is None:
                    self.run_start = f
        self.prev = f
        types_hit = set()
        for d in diffs:
            self.total += 1
            if len(self.divs) <= self.limit:
                self.divs.append(d)
            e = self.by_field.setdefault(d["field"], [d, 0])
            e[1] += 1
            types_hit.add("game" if d["unit"] is None else d["unit"][0])
        for t in types_hit:
            e = self.by_type.setdefault(t, [f, 0, 0])
            e[1] += 1
        for e in self.by_type.values():
            e[2] += 1  # frames compared from that type's first difference on

    def finish(self):
        if self.run_start is not None:
            self.equal_runs.append((self.run_start, self.prev))
            self.run_start = None


def compare_state(orig_path, d2rs_path, ignore=(), types=None, lo=None, hi=None, limit=20,
                  bucket=1000, perturb=None):
    """Streams the two files; returns (Stats, the two headers, fields, one-sided,
    the two StateStreams)."""
    a, b = StateStream(orig_path), StateStream(d2rs_path)
    fa, fb = set(a.header.get("fields", [])), set(b.header.get("fields", []))
    ign = set(ignore)
    fields = [k for k in FIELDS if k in fa and k in fb and k not in ign and k not in ("ut", "g")]
    one_sided = sorted((fa ^ fb) - ign - {"ut", "g"},
                       key=lambda k: FIELDS.index(k) if k in FIELDS else 99)
    st = Stats(limit, bucket)
    ia, ib = a.snaps(), b.snaps()
    x, y = next(ia, None), next(ib, None)
    while x is not None and y is not None:
        if x["f"] < y["f"]:
            x = next(ia, None)
            continue
        if y["f"] < x["f"]:
            y = next(ib, None)
            continue
        f = x["f"]
        if (lo is None or f >= lo) and (hi is None or f <= hi):
            if perturb and perturb[0] == f:
                apply_perturb(y, perturb)
            diffs, n = frame_diffs(x, y, fields, types)
            st.add_frame(f, diffs, n)
        x, y = next(ia, None), next(ib, None)
    # drain the rest (c2s lines, footers)
    for _ in ia:
        pass
    for _ in ib:
        pass
    st.finish()
    if not st.frames:
        raise ReplayError("no common frame (check --from / --to and both files)")
    return st, fields, one_sided, a, b


def apply_perturb(snap, perturb):
    """--perturb F:T:G:FIELD (self-test aid, as state_diff.py): changes that
    value on the d2rs side before comparing."""
    _f, t, g, k = perturb
    if k == "seed":
        snap["seed"] = [snap["seed"][0] ^ 1, snap["seed"][1]]
        return
    for u in snap["units"]:
        if u["ut"] == t and u["g"] == g:
            v = u.get(k)
            u[k] = 0 if v is None else ([v[0] ^ 1, v[1]] if isinstance(v, list) else v + 1)
            return
    raise ReplayError(f"--perturb: no unit {t}:{g} at frame {_f}")


# --- the input channel --------------------------------------------------------

def compare_input(orig, d2rs, lo, hi, masks):
    """Per frame window F in [lo, hi], 1.14d's drained C->S messages against
    d2rs', index by index (kind, id, size, bytes outside the C->S masks).
    Returns a dict: windows compared / equal, messages per side, the first
    differences."""
    def by_window(msgs):
        w = {}
        for f, q, b in msgs:
            if f is not None and lo <= f <= hi:
                w.setdefault(f, []).append((q, b))
        return w

    wa, wb = by_window(orig), by_window(d2rs)
    frames = sorted(set(wa) | set(wb))
    divs, diverged = [], 0
    for f in frames:
        xa, xb = wa.get(f, []), wb.get(f, [])
        bad = None
        for i in range(max(len(xa), len(xb))):
            p = xa[i] if i < len(xa) else None
            q = xb[i] if i < len(xb) else None
            if p is None or q is None:
                bad = (i, "missing in d2rs" if q is None else "extra (d2rs only)", p, q)
                break
            why = record_diff(p, q, masks)
            if why:
                bad = (i, why, p, q)
                break
        if bad:
            diverged += 1
            if len(divs) < 20:
                divs.append({"f": f, "index": bad[0], "what": bad[1],
                             "orig": fmt_msg(bad[2]), "d2rs": fmt_msg(bad[3]),
                             "orig_ids": " ".join(f"{b[0]:02x}" for _, b in xa if b),
                             "d2rs_ids": " ".join(f"{b[0]:02x}" for _, b in xb if b)})
    return {"windows": len(frames), "diverged": diverged, "orig_msgs": sum(map(len, wa.values())),
            "d2rs_msgs": sum(map(len, wb.values())), "divs": divs}


def record_diff(p, q, masks):
    (qa, a), (qb, b) = p, q
    if qa != qb:
        return f"queue {qa} vs {qb}"
    if a[:1] != b[:1]:
        return f"id {a[:1].hex() or '-'} vs {b[:1].hex() or '-'}"
    if len(a) != len(b):
        return f"size {len(a)} vs {len(b)}"
    m = packets_diff.masked(masks, a)
    for k in range(1, len(a)):
        if k not in m and a[k] != b[k]:
            return f"bytes[{k}] {a[k]:02x} vs {b[k]:02x}"
    return None


def fmt_msg(m):
    if m is None:
        return "(none)"
    q, b = m
    return f"{q} {b.hex()}"


# --- report -------------------------------------------------------------------

def where(d):
    if d["unit"] is None:
        return f"frame {d['f']} game"
    t, g = d["unit"]
    cls = ["-" if d[s] is None else str(d[s].get("cl", "?")) for s in ("a", "b")]
    c = cls[0] if cls[0] == cls[1] else f"{cls[0]}/{cls[1]}"
    return f"frame {d['f']} {TYPE_NAMES.get(t, t)} {t}:{g} class {c}"


def ratio(n, m):
    return f"{n}/{m}" if m else "0/0"


def report(st, fields, one_sided, a, b, inp, w=sys.stdout.write):
    ha, hb = a.header, b.header
    w(f"1.14d: {ha.get('tool')}  {ha.get('command', '')}\n")
    w(f"d2rs:  {hb.get('tool')}  {hb.get('command', '')[:300]}\n")
    w(f"\nframes compared: {st.frames} ({st.lo}..{st.hi}), unit records compared: {st.units}\n")
    if st.divs:
        d = st.divs[0]
        w(f"\nFIRST DIVERGENCE: {where(d)}, field {d['field']}: 1.14d {d['exp']} vs d2rs "
          f"{d['got']}\n")
        if d["unit"] is not None:
            w(f"  1.14d: {state_diff.fmt_unit(d['a'])}\n  d2rs:  {state_diff.fmt_unit(d['b'])}\n")
        if len(st.divs) > 1:
            w(f"\nnext {len(st.divs) - 1}:\n")
            for d in st.divs[1:]:
                w(f"  {where(d)}: {d['field']} {d['exp']} vs {d['got']}\n")
        w(f"\nDIVERGENCE RATE after frame {st.first}: {ratio(st.after_div, st.after)} frames "
          f"with a difference; {len(st.equal_runs)} equal stretch(es) after it"
          + (f" (longest {max(e - s + 1 for s, e in st.equal_runs)} frames, first "
             f"{st.equal_runs[0][0]}..{st.equal_runs[0][1]})" if st.equal_runs else "")
          + f"; longest diverged run {st.longest_diverged_run} frames\n")
        w("\nper unit type (first frame; frames with a difference of that type / frames from "
          "it on):\n")
        for t in sorted(st.by_type, key=lambda t: (st.by_type[t][0], str(t))):
            f0, n, m = st.by_type[t]
            name = "game seed" if t == "game" else TYPE_NAMES.get(t, t)
            w(f"  {name}: {f0}; {ratio(n, m)}\n")
        w("\nper field (first place, 1.14d vs d2rs; differing (frame, unit) pairs):\n")
        for k in sorted(st.by_field, key=lambda k: (st.by_field[k][0]["f"],
                                                    FIELDS.index(k) if k in FIELDS else -1)):
            d, n = st.by_field[k]
            w(f"  {k}: {where(d)}, {d['exp']} vs {d['got']}; {n}\n")
        w(f"\nper {st.bucket} frames (frames with a difference / frames):\n")
        w("  " + "  ".join(f"{s}:{ratio(v[1], v[0])}" for s, v in sorted(st.buckets.items()))
          + "\n")
    w(f"\nfields: {' '.join(fields)}\n")
    if one_sided:
        w(f"not compared (one side only): {' '.join(one_sided)}\n")
    for side, h in (("1.14d", ha), ("d2rs", hb)):
        for g in h.get("gaps", []):
            w(f"gap ({side}): {g}\n")
    w("\ninput channel (C->S drained per frame, 1.14d vs the d2rs replay):\n")
    if inp is None:
        w("  not compared (no d2rs packets file)\n")
    else:
        w(f"  windows {inp['windows']}, diverged {inp['diverged']}; messages 1.14d "
          f"{inp['orig_msgs']}, d2rs {inp['d2rs_msgs']}\n")
        fi = inp["divs"][0]["f"] if inp["divs"] else None
        if fi is not None and (st.first is None or fi <= st.first):
            w(f"  THE REPLAY'S INPUT DIVERGED FIRST: frame {fi}"
              + (f" (state from frame {st.first}): the state divergence may be the input's"
                 if st.first is not None else "") + "\n")
        for d in inp["divs"][:5]:
            w(f"  frame {d['f']} #{d['index']}: {d['what']}\n    1.14d {d['orig']}\n    "
              f"d2rs  {d['d2rs']}\n    ids 1.14d [{d['orig_ids']}] d2rs [{d['d2rs_ids']}]\n")
    code = verdict(st, one_sided, ha, hb, inp)
    w(f"\n{VERDICTS[code]}" + (f": {st.total} state differences" if st.divs else "")
      + (f", {inp['diverged']} input windows diverged" if inp and inp["diverged"] else "")
      + "\n")
    return code


def verdict(st, one_sided, ha, hb, inp):
    if st.divs or (inp and inp["diverged"]):
        return 1
    if one_sided or ha.get("gaps") or hb.get("gaps") or inp is None:
        return 2
    return 0


def summary(st, code, inp):
    first = None
    if st.divs:
        d = st.divs[0]
        first = {"frame": d["f"], "text": f"{where(d)}, field {d['field']}: 1.14d {d['exp']} "
                                          f"vs d2rs {d['got']}"}
    return {"format": SUMMARY_FORMAT, "channel": "replay-state", "tool": TOOL, "code": code,
            "verdict": VERDICTS[code], "frames_compared": st.frames,
            "frame_range": [st.lo, st.hi], "frames_equal": st.frames - sum(
                v[1] for v in st.buckets.values()),
            "differences": st.total, "first": first,
            "after_first": {"frames": st.after, "diverged": st.after_div,
                            "equal_stretches": len(st.equal_runs),
                            "longest_diverged_run": st.longest_diverged_run},
            "by_type": {str(t): {"first": v[0], "frames_diverged": v[1], "frames_from_first": v[2]}
                        for t, v in st.by_type.items()},
            "by_field": {k: {"first": v[0]["f"], "pairs": v[1]} for k, v in st.by_field.items()},
            "buckets": {str(s): v for s, v in sorted(st.buckets.items())},
            "input": None if inp is None else {k: inp[k] for k in
                                                ("windows", "diverged", "orig_msgs",
                                                 "d2rs_msgs")}
            | {"first": inp["divs"][0] if inp["divs"] else None}}


def write_json(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=1)
        f.write("\n")


def compare(orig, d2rs, packets=None, ignore=(), types=None, lo=None, hi=None, limit=20,
            bucket=1000, json_out=None, perturb=None, w=sys.stdout.write):
    """The `compare` command: state, then the input channel. Returns the exit code."""
    st, fields, one_sided, a, b = compare_state(orig, d2rs, ignore, types, lo, hi, limit,
                                                bucket, perturb)
    inp = None
    if packets:
        inp = compare_input([(f, q, bytes.fromhex(r)) for f, q, r in
                             ((c["f"], c["q"], c["bytes"]) for c in a.c2s)],
                            d2rs_c2s(packets), st.lo, st.hi,
                            packets_diff.load_masks()["c2s"])
    code = report(st, fields, one_sided, a, b, inp, w)
    if json_out:
        write_json(json_out, summary(st, code, inp))
    return code


# --- run: one session, both sides ---------------------------------------------

def read_session(path):
    """A session file: a scenario-diff check (`check 1`, scenario-diff.md §2)
    whose `input` line may continue on `input+ <steps>` lines (joined with
    `; `), so a long frame-anchored script stays readable."""
    import scenario_diff
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    out, extra = [], []
    for ln in lines:
        s = ln.strip()
        if s.startswith("input+ "):
            extra.append(s[len("input+ "):].strip().rstrip(";"))
        else:
            out.append(ln)
    if extra:
        k = next((i for i, ln in enumerate(out) if ln.strip().startswith("input ")), None)
        if k is None:
            raise ReplayError(f"{path}: 'input+' lines without an 'input' line")
        out[k] = out[k].rstrip().rstrip(";") + "; " + "; ".join(extra)
    try:
        c = scenario_diff.parse("\n".join(out) + "\n")
    except scenario_diff.CheckError as e:
        raise ReplayError(f"{path}: {e}")
    if c["input"].get("orig") or c["input"].get("d2rs"):
        raise ReplayError(f"{path}: use the shared 'input' line (1.14d plays it; d2rs replays "
                          f"what it sent)")
    if c["send"]:
        raise ReplayError(f"{path}: 'at … send' lines would be replayed twice (1.14d records "
                          f"them as C->S); use input or pokes")
    want = os.path.splitext(os.path.basename(path))[0]
    if c["name"] != want:
        raise ReplayError(f"{path}: name must be {want}")
    return c


def run(a):
    import scenario_diff
    c = read_session(a.session)
    work = a.work or os.path.join(REPO, "traces", "raw", "replay-" + c["name"])
    os.makedirs(work, exist_ok=True)

    class Runner(scenario_diff.Runner):
        def sh(self, argv, timeout=None, check=True, env=None):
            line = " ".join(shlex.quote(x) for x in argv)
            sends = sum(1 for x in argv if x == "--send")
            if len(line) > 600:  # thousands of --send values: print the shape only
                line = line[:600] + f" ... ({sends} --send values, {len(argv)} arguments)"
            self.log.append(line)
            print(f"$ {line}", flush=True)
            if self.dry:
                return 0
            r = subprocess.run(argv, cwd=REPO, timeout=timeout, env=env)
            if check and r.returncode != 0:
                raise scenario_diff.CheckError(f"command failed ({r.returncode}): {line}")
            return r.returncode

        def recorder(self, script, args, out):
            """scenario_diff.Runner.recorder for a recorder outside
            tools/trace-recorder (its run dir is named after the script)."""
            c, name = self.c, os.path.splitext(os.path.basename(script))[0]
            game = os.path.join(self.game_dir, "Game.exe")
            rec = [script, "--game", game, "--seconds", str(c["seconds"]), "--ticks",
                   str(c["ticks"]), "--auto", c["char"], "--seed", str(c["seed"]),
                   "--out", out] + args + self.poke_args()
            if self.orig_input():
                rec += ["--input", self.orig_input()]
            if self.reuse and os.path.exists(out):
                print(f"reuse {out}")
                return
            if scenario_diff.WINDOWS:
                self.sh([sys.executable] + rec, timeout=c["seconds"] + 120)
            else:
                run_sh = os.path.join(REPO, "tools", "cloud-game", "run.sh")
                self.sh([run_sh, "--python", "--seconds", str(c["seconds"] + 60), "--out",
                         self.path("run-" + name), "--"] + rec, timeout=c["seconds"] + 180,
                        check=False)
            if not self.dry and not os.path.exists(out):
                raise scenario_diff.CheckError(f"{name} wrote no {out} (see "
                                               f"{self.path('run-' + name)})")

    r = Runner(c, work, dry=a.dry_run, reuse=a.reuse)
    r.next = a.next
    save = r.build_save()
    orig = r.path("orig.state.jsonl")
    d2rs, packets = r.path("d2rs.state.jsonl"), r.path("d2rs.packets.jsonl")
    if not a.d2rs_only:
        r.recorder(os.path.join(HERE, "record_replay.py"), ["--snap-every", "1"], orig)
    if a.orig_only:
        return 0
    if a.dry_run and not os.path.exists(orig):
        sends, skipped = ["<F> hex <bytes>"], {}
    else:
        sends, skipped = replay_sends(orig_c2s(orig))
    print(f"replay: {len(sends)} C->S message(s) to d2rs; not replayed: "
          + ", ".join(f"{k} {v}" for k, v in skipped.items()))
    if not (a.reuse and os.path.exists(d2rs) and os.path.exists(packets)):
        args = ["state-dump", "--save", save, "--seed", str(c["seed"]), "--difficulty",
                c["difficulty"]] + r.poke_args()
        for s in sends:
            args += ["--send", s]
        args += ["--ticks", str(c["ticks"]), "--out", d2rs, "--packets", packets]
        r.cargo("d2-client", args)
    if a.dry_run:
        return 0
    ignore = list(c["ignore"]) + [x for x in (a.ignore or "").split(",") if x]
    return compare(orig, d2rs, packets, ignore, parse_types(a.types), None, None, a.next,
                   a.bucket, a.json or os.path.join(work, "replay.summary.json"))


def parse_types(s):
    return None if not s else {int(x) for x in s.split(",") if x}


# --- selftest -----------------------------------------------------------------

def _synthetic(frames, units=3):
    hdr = {"k": "header", "format": state_diff.FORMAT, "side": "orig", "tool": "selftest",
           "command": "x", "fields": ["ut", "g", "cl", "m", "x", "y", "hp"], "gaps": []}
    snaps = []
    for f in range(1, frames + 1):
        us = [{"ut": t % 2, "g": t, "cl": 10 + t, "m": f % 3, "x": 100 + f, "y": 200 + t,
               "hp": 256} for t in range(units)]
        snaps.append({"k": "snap", "f": f, "seed": [f, 666], "units": us})
    return hdr, snaps


def _write(path, hdr, snaps, extra=(), footer=True):
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(hdr) + "\n")
        ex = {}
        for e in extra:
            ex.setdefault(e.get("f"), []).append(e)
        for e in ex.get(None, []):
            f.write(json.dumps(e) + "\n")
        for s in snaps:
            for e in ex.get(s["f"], []):  # c2s of frame F come before its snapshot
                f.write(json.dumps(e) + "\n")
            f.write(json.dumps(s) + "\n")
        if footer:
            f.write(json.dumps({"k": "footer", "snaps": len(snaps), "notes": []}) + "\n")


def _packets(path, msgs, ticks):
    """A d2rs packets-raw-1 file with `msgs` [(window, q, bytes)] drained before
    tick `window`."""
    recs = [{"type": "header", "format": packets_diff.FORMAT, "side": "d2rs"}]
    by = {}
    for wdw, q, b in msgs:
        by.setdefault(wdw, []).append((q, b))
    for q, b in by.get(None, []):
        recs.append({"type": "c2s" if q == "game" else "c2s_sys", "frame": None,
                     "phase": "input", "bytes": b.hex(), "size": len(b), "client": 1})
    for t in range(1, ticks + 1):
        recs.append({"type": "drain", "frame": t - 1, "phase": "input"})
        for q, b in by.get(t, []):
            recs.append({"type": "c2s" if q == "game" else "c2s_sys", "frame": t - 1,
                         "phase": "input", "bytes": b.hex(), "size": len(b), "client": 1})
        recs.append({"type": "tick", "frame": t, "phase": "tick"})
        recs.append({"type": "tick_end", "frame": t, "phase": "post"})
    recs.append({"type": "footer"})
    with open(path, "w", encoding="utf-8") as f:
        for r in recs:
            f.write(json.dumps(r) + "\n")


def selftest():
    import copy
    import io
    import tempfile
    d = tempfile.mkdtemp(prefix="replay-diff-selftest-")
    o, x, p = (os.path.join(d, n) for n in ("o.jsonl", "d.jsonl", "p.jsonl"))
    quiet = io.StringIO().write
    n = 12000
    hdr, snaps = _synthetic(n)
    c2s = [{"k": "c2s", "f": None, "q": "game", "client": 1, "size": 1, "bytes": "67"},
           {"k": "c2s", "f": 10, "q": "game", "client": 1, "size": 5, "bytes": "0301000200"},
           {"k": "c2s", "f": 10, "q": "game", "client": 1, "size": 1, "bytes": "6b"},
           {"k": "c2s", "f": 500, "q": "sys", "client": 1, "size": 13,
            "bytes": "6d" + "00" * 12},
           {"k": "c2s", "f": 9000, "q": "game", "client": 1, "size": 9,
            "bytes": "060100000013000000"}]
    _write(o, hdr, snaps, c2s)
    hd = dict(hdr, side="d2rs")
    _write(x, hd, snaps)
    msgs = [(f, q, bytes.fromhex(b)) for f, q, b in
            ((e["f"], e["q"], e["bytes"]) for e in c2s)]
    _packets(p, msgs, n)
    # 1. a file against itself (and the replayed input equal): MATCH over 12,000 frames
    code = compare(o, x, p, w=quiet)
    assert code == 0, code
    # 2. the sends: frame, hex, bridge-own and pre-tick messages left out
    sends, skipped = replay_sends(orig_c2s(o))
    assert sends == ["10 hex 03 01 00 02 00", "500 hex 6d" + " 00" * 12,
                     "9000 hex 06 01 00 00 00 13 00 00 00"], sends
    assert skipped == {"before-first-tick": 1, "bridge-own": 1, "empty": 0}, skipped
    # 3. a perturbed field is the first divergence, at exactly its place; rate after it
    for f, t, g, k in ((1, 0, 0, "m"), (777, 1, 1, "hp"), (11999, 0, 2, "x"),
                       (5000, 0, 0, "seed")):
        st, *_ = compare_state(o, x, perturb=(f, t, g, k))
        dv = st.divs[0]
        assert (dv["f"], dv["field"]) == (f, k), (dv, f, k)
        if k != "seed":
            assert dv["unit"] == (t, g), dv
        assert st.first == f and st.after == n - f + 1 and st.after_div == 1, st.__dict__
        assert len(st.equal_runs) == (1 if f < n else 0)
    # 4. a drift from frame 3000 on (every frame), back to equal at 9000..: rate and stretches
    snaps2 = copy.deepcopy(snaps)
    for s in snaps2:
        if 3000 <= s["f"] < 9000:
            s["units"][1]["x"] += 1
        if s["f"] >= 6000 and s["f"] < 6010:
            s["units"].pop(2)  # a missing unit
    _write(x, hd, snaps2)
    out = io.StringIO()
    js = os.path.join(d, "s.json")
    code = compare(o, x, p, w=out.write, json_out=js, bucket=1000)
    txt = out.getvalue()
    assert code == 1, code
    assert "FIRST DIVERGENCE: frame 3000 monster 1:1 class 11, field x: 1.14d 3100 vs d2rs 3101" \
        in txt, txt
    assert "DIVERGENCE RATE after frame 3000: 6000/9001 frames" in txt, txt
    sm = json.load(open(js))
    assert sm["after_first"] == {"frames": 9001, "diverged": 6000, "equal_stretches": 1,
                                 "longest_diverged_run": 6000}, sm["after_first"]
    assert sm["by_field"]["(unit)"] == {"first": 6000, "pairs": 10}, sm["by_field"]
    assert sm["by_type"]["0"] == {"first": 6000, "frames_diverged": 10,
                                  "frames_from_first": 6001}, sm["by_type"]
    assert sm["buckets"]["3000"] == [1000, 1000] and sm["buckets"]["9000"] == [1000, 0]
    assert sm["frames_equal"] == n - 6000
    # --types and --ignore narrow it
    st, *_ = compare_state(o, x, types={0})
    assert st.first == 6000, st.first
    st, *_ = compare_state(o, x, ignore=["x"], types={1})
    assert st.first is None
    # 5. the input channel: a changed byte, a dropped and an extra message
    _write(x, hd, snaps)
    m03, m6b = (10, "game", bytes.fromhex("0301000200")), (10, "game", b"\x6b")
    for bad, what in (([(10, "game", bytes.fromhex("0301000300")), m6b], "#0: bytes[3] 02 vs 03"),
                      ([m03], "#1: missing in d2rs"),
                      ([m03, m6b, m03], "#2: extra (d2rs only)"),
                      ([(10, "sys", m03[2]), m6b], "#0: queue game vs sys"),
                      ([m6b, m03], "#0: id 03 vs 6b")):
        m2 = [mm for mm in msgs if mm[0] != 10] + bad
        _packets(p, m2, n)
        out = io.StringIO()
        code = compare(o, x, p, w=out.write)
        assert code == 1 and f"frame 10 {what}" in out.getvalue(), (what, out.getvalue())
    # the input diverging at or before the first state divergence is said so
    _write(x, hd, [s if s["f"] != 10 else dict(s, seed=[0, 0]) for s in snaps])
    _packets(p, [mm for mm in msgs if mm[0] != 10] + [m6b, m03], n)
    out = io.StringIO()
    compare(o, x, p, w=out.write)
    assert "INPUT DIVERGED FIRST: frame 10 (state from frame 10)" in out.getvalue(), \
        out.getvalue()
    _write(x, hd, snaps)
    # without the packets file: partial
    assert compare(o, x, None, w=quiet) == 2
    # 6. one-sided field: partial; unknown file: error
    _write(x, dict(hd, fields=hd["fields"] + ["lv"]), snaps)
    _packets(p, msgs, n)
    out = io.StringIO()
    assert compare(o, x, p, w=out.write) == 2 and "not compared (one side only): lv" in \
        out.getvalue()
    try:
        compare(o, os.path.join(d, "none.jsonl"), w=quiet)
        raise AssertionError("missing file accepted")
    except ReplayError:
        pass
    # 7. frames must ascend; files with no common frame are an error
    _write(x, hd, list(reversed(snaps[:3])))
    for bad in (x,):
        try:
            compare_state(o, bad)
            raise AssertionError("descending frames accepted")
        except ReplayError:
            pass
    # 8. a session file: input+ continuation, sends refused, name checked
    sess = os.path.join(d, "s1.replay")
    with open(sess, "w") as f:
        f.write("check 1\nname s1\nsave A --class ama\nseed 1\nticks 10\n"
                "input frame 5; click 1 2\ninput+ frame 7; click 3 4;\ninput+ frame 9; key R\n")
    c = read_session(sess)
    assert c["input"]["shared"] == "frame 5; click 1 2; frame 7; click 3 4; frame 9; key R", c
    with open(sess, "a") as f:
        f.write("at 3 send hex 03 00 00 00 00\n")
    try:
        read_session(sess)
        raise AssertionError("send line accepted")
    except ReplayError:
        pass
    import c2s_tap
    c2s_tap.selftest()
    print("replay_diff selftest: OK")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--selftest", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--types", default=None, help="unit types compared (e.g. 0,1)")
    common.add_argument("--ignore", default=None, help="fields not compared (comma list)")
    common.add_argument("--next", type=int, default=20)
    common.add_argument("--bucket", type=int, default=1000)
    common.add_argument("--json", default=None, metavar="FILE")
    r = sub.add_parser("run", parents=[common], help="record 1.14d, replay on d2rs, compare")
    r.add_argument("session")
    r.add_argument("--work", default=None, help="work dir (default traces/raw/replay-<name>)")
    r.add_argument("--reuse", action="store_true", help="reuse outputs already in the work dir")
    r.add_argument("--orig-only", action="store_true")
    r.add_argument("--d2rs-only", action="store_true")
    r.add_argument("--dry-run", action="store_true")
    c = sub.add_parser("compare", parents=[common], help="compare two recorded runs")
    c.add_argument("orig")
    c.add_argument("d2rs")
    c.add_argument("--packets", default=None, help="the d2rs packets file (input channel)")
    c.add_argument("--from", dest="lo", type=int, default=None)
    c.add_argument("--to", dest="hi", type=int, default=None)
    s = sub.add_parser("sends", help="print the --send values d2rs gets")
    s.add_argument("orig")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    try:
        if a.cmd == "run":
            return run(a)
        if a.cmd == "compare":
            return compare(a.orig, a.d2rs, a.packets,
                           [x for x in (a.ignore or "").split(",") if x], parse_types(a.types),
                           a.lo, a.hi, a.next, a.bucket, a.json)
        if a.cmd == "sends":
            sends, skipped = replay_sends(orig_c2s(a.orig))
            for x in sends:
                print(x)
            print(f"# not replayed: {skipped}", file=sys.stderr)
            return 0
        ap.print_help()
        return 3
    except ReplayError as e:
        print(f"error: {e}", file=sys.stderr)
        return 3
    except Exception as e:  # scenario_diff.CheckError, OSError, timeouts
        if type(e).__name__ in ("CheckError", "TimeoutExpired") or isinstance(e, OSError):
            print(f"error: {e}", file=sys.stderr)
            return 3
        raise


if __name__ == "__main__":
    sys.exit(main())
