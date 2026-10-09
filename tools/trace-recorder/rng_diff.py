"""Compare two RNG draw recordings (format rng-raw-1 with frames and owners:
1.14d `record_rng.py --frames`, d2rs `d2-client state-dump --rng`) and
print the first divergence, then the next N, the game-seed sequence and a
summary per owner (specs/tools/rng-trace.md §5).

    python3 rng_diff.py ORIG.rng.jsonl D2RS.rng.jsonl [--next 20]
        [--owners game,unit] [--from F] [--to F] [--state-only] [--json FILE]
    python3 rng_diff.py --selftest

Alignment: by frame. Within a frame, each owner's ordered draws (the game
seed, every unit seed) are compared position by position: seed before,
seed after, then, where the 1.14d draw went through a helper, op, n, min
and the returned value. Draws on other seeds (items, DRLG, client) are
counted, not compared. A draw that does not step (roll with n < 1) is
left out on both sides. The last frame both files reach is compared as a
prefix (the 1.14d recorder stops at the next tick's entry).

Exit codes: 0 MATCH, 1 DIVERGED, 2 PARTIAL, 3 error.
Standard library only; our own code.
"""

import argparse
import copy
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rng_owners  # noqa: E402

FORMAT = "rng-raw-1"
M32 = 0xFFFFFFFF


class RngError(Exception):
    pass


def load(path):
    """(header, draws, footer, info) of one file; draws carry frame and owner."""
    header, recs, footer = None, [], None
    with open(path, encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
            except ValueError as e:
                raise RngError(f"{path}:{n}: not JSON ({e})")
            if header is None:
                if r.get("type") != "header" or r.get("format") != FORMAT:
                    raise RngError(f"{path}:{n}: not a {FORMAT} file (first line must be its header)")
                header = r
                continue
            if r.get("type") == "footer":
                footer = r
            else:
                recs.append(r)
    if header is None:
        raise RngError(f"{path}: empty")
    return header, recs, footer


def prepare(header, recs):
    """Owners (post-pass when missing), filtered draws and counts."""
    draws = [r for r in recs if r.get("type") == "draw"]
    if any("frame" not in d for d in draws):
        raise RngError("draws without a frame (record with record_rng.py --frames)")
    if any("owner" not in d for d in draws):
        if not any(r.get("type") == "tick" for r in recs):
            raise RngError("draws without an owner and no tick records to assign them")
        rng_owners.assign(recs)
    info = {"unresolved": 0, "nostep": 0, "other": 0}
    out = []
    for d in draws:
        if d.get("after") is None or None in (d.get("before") or [None]):
            info["unresolved"] += 1
            continue
        if list(d["before"]) == list(d["after"]):
            info["nostep"] += 1
            continue
        if not is_compared(d["owner"]):
            info["other"] += 1
            continue
        out.append(d)
    return out, info


def is_compared(owner):
    return owner == "game" or owner.startswith("unit ")


def owner_key(o):
    if o == "game":
        return (0, 0, 0)
    t, g = o.split(" ", 1)[1].split(":")
    return (1, int(t), int(g))


def group(draws):
    by = {}
    for d in draws:
        by.setdefault(d["frame"], {}).setdefault(d["owner"], []).append(d)
    return by


def fields_differ(a, b, state_only):
    """The first differing field of orig draw a and d2rs draw b, or None."""
    for k in ("before", "after"):
        if list(a[k]) != list(b[k]):
            return k
    if state_only or a.get("via") != "helper":
        return None
    if a.get("op") != b.get("op"):
        return "op"
    for k in ("n", "min"):
        if (a.get(k) is None) != (b.get(k) is None):
            return k
        if a.get(k) is not None and a[k] & M32 != b[k] & M32:
            return k
    if (a.get("ret") or 0) & M32 != (b.get("ret") or 0) & M32:
        return "ret"
    return None


def last_frame(recs):
    fs = [r["frame"] for r in recs if r.get("type") == "draw" and "frame" in r]
    fs += [r["f"] for r in recs if r.get("type") == "tick"]
    return max(fs) if fs else 0


def compare(a, b, owners=("game", "unit"), lo=None, hi=None, state_only=False):
    """a = orig (header, recs), b = d2rs. Returns the result dict."""
    da, ia = prepare(*a)
    db, ib = prepare(*b)
    keep = lambda d: d["owner"].split(" ")[0] in owners  # noqa: E731
    da, db = [d for d in da if keep(d)], [d for d in db if keep(d)]
    top = min(last_frame(a[1]), last_frame(b[1]))
    if hi is not None:
        top = min(top, hi)
    bottom = lo if lo is not None else 0
    ga, gb = group(da), group(db)
    diffs, per = [], {}
    frames = sorted(f for f in set(ga) | set(gb) if bottom <= f <= top)
    for f in frames:
        oa, ob = ga.get(f, {}), gb.get(f, {})
        for o in sorted(set(oa) | set(ob), key=owner_key):
            xa, xb = oa.get(o, []), ob.get(o, [])
            if f == top:  # the last frame: a prefix (module docs)
                n = min(len(xa), len(xb))
                xa, xb = xa[:n], xb[:n]
            s = per.setdefault(o, {"orig": 0, "d2rs": 0, "diff": 0, "first": None})
            s["orig"] += len(xa)
            s["d2rs"] += len(xb)
            for i in range(max(len(xa), len(xb))):
                ra = xa[i] if i < len(xa) else None
                rb = xb[i] if i < len(xb) else None
                if ra is None:
                    field = "extra"
                elif rb is None:
                    field = "missing"
                else:
                    field = fields_differ(ra, rb, state_only)
                if field is None:
                    continue
                d = {"frame": f, "owner": o, "index": i, "field": field, "orig": ra, "d2rs": rb}
                diffs.append(d)
                s["diff"] += 1
                if s["first"] is None:
                    s["first"] = d
    seq = game_sequence(da, db, bottom, top)
    return {"diffs": diffs, "per": per, "frames": (bottom, top), "n_frames": len(frames),
            "info": (ia, ib), "seq": seq}


def game_sequence(da, db, lo, hi):
    """The game seed's draws in order across frames: the first position
    whose states differ, and the shift k (|k| <= 4) for which d2rs draw i
    has the state of 1.14d draw i + k for the longest run."""
    sa = [d for d in da if d["owner"] == "game" and lo <= d["frame"] <= hi]
    sb = [d for d in db if d["owner"] == "game" and lo <= d["frame"] <= hi]
    first = None
    for i in range(max(len(sa), len(sb))):
        ra = sa[i] if i < len(sa) else None
        rb = sb[i] if i < len(sb) else None
        if ra is None or rb is None or list(ra["before"]) != list(rb["before"]) or \
                list(ra["after"]) != list(rb["after"]):
            first = (i, ra, rb)
            break
    moved = None  # the first position with equal states in different frames
    for i in range(min(len(sa), len(sb), first[0] if first else len(sa))):
        if sa[i]["frame"] != sb[i]["frame"]:
            moved = (i, sa[i], sb[i])
            break
    best = None
    for k in range(-4, 5):
        run = 0
        for i, rb in enumerate(sb):
            j = i + k
            if 0 <= j < len(sa) and list(sa[j]["after"]) == list(rb["after"]):
                run += 1
        if run and (best is None or run > best[1]):
            best = (k, run)
    return {"orig": len(sa), "d2rs": len(sb), "first": first, "shift": best, "moved": moved}


def fmt(d):
    if d is None:
        return "(none)"
    op = d.get("op", "?")
    args = ",".join(f"{k}={d[k]}" for k in ("min", "n") if d.get(k) is not None)
    via = d.get("via", "?")
    return (f"{via} {op}({args}) before {d['before']} after {d['after']} ret {d.get('ret')} "
            f"site {d.get('site')} frame {d.get('frame')}")


def where(d):
    return f"frame {d['frame']}, {d['owner']}, draw #{d['index']}, field {d['field']}"


def report(r, ha, hb, nxt=20, out=sys.stdout):
    p = lambda *x: print(*x, file=out)  # noqa: E731
    p(f"orig: {ha.get('tool')} {' '.join(ha.get('args') or [])}")
    p(f"d2rs: {hb.get('tool')} {hb.get('command', '')}")
    lo, hi = r["frames"]
    p(f"frames compared: {r['n_frames']} ({lo}..{hi}; frame {hi} as a prefix)")
    diffs = r["diffs"]
    if diffs:
        d = diffs[0]
        p(f"\nFIRST DIVERGENCE: {where(d)}")
        p(f"  orig: {fmt(d['orig'])}")
        p(f"  d2rs: {fmt(d['d2rs'])}")
        if len(diffs) > 1:
            p(f"\nnext {min(nxt, len(diffs) - 1)} of {len(diffs) - 1}:")
            for d in diffs[1:1 + nxt]:
                p(f"  {where(d)}")
                p(f"    orig: {fmt(d['orig'])}")
                p(f"    d2rs: {fmt(d['d2rs'])}")
    s = r["seq"]
    p(f"\ngame seed, all frames in order: orig {s['orig']} draws, d2rs {s['d2rs']}")
    if s["first"] is not None:
        i, ra, rb = s["first"]
        p(f"  first difference at game draw #{i}")
        p(f"    orig: {fmt(ra)}")
        p(f"    d2rs: {fmt(rb)}")
    else:
        p("  identical states")
    if s["moved"] is not None:
        i, ra, rb = s["moved"]
        p(f"  game draw #{i} has the same state on both sides but runs in orig frame "
          f"{ra['frame']} (site {ra.get('site')}), d2rs frame {rb['frame']} "
          f"(site {rb.get('site')})")
    if s["shift"] and s["shift"][0] != 0:
        k, run = s["shift"]
        p(f"  d2rs game draw i has the state of orig draw i{k:+d} for {run} draws "
          f"(d2rs is {abs(k)} game-seed step(s) {'behind' if k > 0 else 'ahead'})")
    p("\nper owner (draws orig / d2rs, positions differing, first):")
    for o in sorted(r["per"], key=owner_key):
        v = r["per"][o]
        first = where(v["first"]) if v["first"] else "-"
        p(f"  {o:14s} {v['orig']:6d} / {v['d2rs']:<6d} {v['diff']:6d}  {first}")
    (ia, ib) = r["info"]
    p(f"not compared: other seeds orig {ia['other']}, d2rs {ib['other']}; no-step draws orig "
      f"{ia['nostep']}, d2rs {ib['nostep']}; unresolved inline orig {ia['unresolved']}")
    gaps = []
    if ha.get("skip_inline"):
        gaps.append(f"orig inline sites skipped in {ha['skip_inline']}")
    if ia["unresolved"]:
        gaps.append(f"{ia['unresolved']} orig inline draws without a state")
    for g in gaps:
        p(f"gap: {g}")
    if diffs:
        verdict, code = "DIVERGED", 1
    elif gaps or r["n_frames"] == 0:
        verdict, code = "PARTIAL", 2
    else:
        verdict, code = "MATCH", 0
    p(f"\n{verdict}")
    return code


VERDICTS = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR"}


def summary(r, code):
    """The machine-readable summary (`--json FILE`, format diff-summary-1;
    specs/tools/scenario-diff.md §4). Ticks compared: every frame of the
    compared range (a frame without draws on either side is equal); equal:
    the frames without a divergence."""
    lo, hi = r["frames"]
    n = max(0, hi - lo + 1)
    bad = {d["frame"] for d in r["diffs"]}
    first = None
    if r["diffs"]:
        d = r["diffs"][0]
        first = {"frame": d["frame"], "text": where(d) + (
            f": 1.14d site {(d['orig'] or {}).get('site')} vs d2rs site "
            f"{(d['d2rs'] or {}).get('site')}")}
    return {"format": "diff-summary-1", "channel": "rng", "tool": "rng_diff.py", "code": code,
            "verdict": VERDICTS[code], "frames_compared": n, "frames_equal": n - len(bad),
            "frame_range": [lo, hi], "differences": len(r["diffs"]), "first": first}


def write_json(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=1)
        f.write("\n")


# --- self-test -----------------------------------------------------------------

K = 0x6AC690C5


def step(s):
    v = s[0] * K + s[1]
    return [v & M32, v >> 32]


def synthetic():
    """(orig records, d2rs records): game creation in frame 0, then three
    frames of game and unit draws; the 1.14d side mixes inline and helper
    draws and has no owners (the post-pass finds them)."""
    g, u = [1234, 666], [77, 666]
    orig = [{"type": "header", "format": FORMAT, "tool": "record_rng test", "args": []}]
    d2rs = [{"type": "header", "format": FORMAT, "tool": "d2rs test", "command": "x"}]
    seq = 0

    def both(frame, owner, op, n, cur, inline, seed_addr=None):
        nonlocal seq
        nxt = step(cur)
        ret = nxt[0] % n if op == "roll" else nxt[0]
        o = {"type": "draw", "via": "inline" if inline else "helper", "op": "step" if inline
             else op, "before": list(cur), "after": nxt, "ret": nxt[0] if inline else ret,
             "frame": frame, "site": f"{0x500000 + seq:#x}"}
        if n is not None and not inline:
            o["n"] = n
        if seed_addr:
            o["seed"] = seed_addr
        b = {"type": "draw", "via": "helper", "op": op, "before": list(cur), "after": nxt,
             "ret": ret, "frame": frame, "owner": owner, "site": f"crates/x.rs:{seq}",
             "seed": owner}
        if n is not None:
            b["n"] = n
        orig.append(o)
        d2rs.append(b)
        seq += 1
        return nxt

    g = both(0, "game", "step", None, g, True)
    g = both(0, "game", "step", None, g, True)
    for f in (1, 2, 3):
        orig.append({"type": "tick", "f": f, "game": "0x1000", "gseed": list(g),
                     "units": [[1, 5, u[0], u[1]]]})
        g = both(f, "game", "roll", 100, g, False, "0x10d0")
        u = both(f, "unit 1:5", "roll", 10, u, False, "0x2020")
        u = both(f, "unit 1:5", "step", None, u, True)
        # an item seed: other on both sides
        x = [f, 666]
        nx = step(x)
        orig.append({"type": "draw", "via": "helper", "op": "step", "seed": "0x3004",
                     "before": x, "after": nx, "ret": nx[0], "frame": f})
        d2rs.append({"type": "draw", "via": "helper", "op": "step", "before": x, "after": nx,
                     "ret": nx[0], "frame": f, "owner": "other", "seed": "d2rs#0"})
    orig.append({"type": "tick", "f": 4, "game": "0x1000", "gseed": list(g),
                 "units": [[1, 5, u[0], u[1]]]})
    orig.append({"type": "footer"})
    d2rs.append({"type": "footer"})
    return orig, d2rs


def split(recs):
    return recs[0], [r for r in recs[1:] if r.get("type") != "footer"]


def run(orig, d2rs, **kw):
    return compare(split(copy.deepcopy(orig)), split(copy.deepcopy(d2rs)), **kw)


# Covers: specs/tools/rng-trace.md §1 r2, §5 r1, §5 r2, §5 r3, §5 r4, §5 r5
def selftest():
    ok = 0
    orig, d2rs = synthetic()
    r = run(orig, d2rs)
    assert not r["diffs"], r["diffs"][:2]
    assert r["per"]["game"]["orig"] == 5 and r["per"]["unit 1:5"]["d2rs"] == 6, r["per"]
    sink = open(os.devnull, "w")
    assert report(r, orig[0], d2rs[0], out=sink) == 0
    ok += 1
    # the d2rs draws of frame 2 in file order: game, unit, unit, other
    idx = [i for i, x in enumerate(d2rs) if x.get("frame") == 2]
    cases = [  # (d2rs record, field change, expected (frame, owner, index, field))
        (idx[0], ("before", [1, 2]), (2, "game", 0, "before")),
        (idx[0], ("after", [1, 2]), (2, "game", 0, "after")),
        (idx[0], ("ret", 5), (2, "game", 0, "ret")),
        (idx[0], ("op", "mask"), (2, "game", 0, "op")),
        (idx[0], ("n", 99), (2, "game", 0, "n")),
        (idx[1], ("ret", 3), (2, "unit 1:5", 0, "ret")),
        (idx[2], ("after", [9, 9]), (2, "unit 1:5", 1, "after")),
        (idx[1], ("owner", "unit 1:6"), (2, "unit 1:5", 0, "before")),
    ]
    for i, (k, v), want in cases:
        b = copy.deepcopy(d2rs)
        b[i][k] = v
        r = run(orig, b)
        d = r["diffs"][0]
        got = (d["frame"], d["owner"], d["index"], d["field"])
        assert got == want, (k, v, got, want)
        assert report(r, orig[0], b[0], out=sink) == 1
        sm = summary(r, 1)
        assert sm["first"]["frame"] == 2 and sm["frames_compared"] == 4, sm
        assert sm["frames_equal"] == 3 and sm["verdict"] == "DIVERGED", sm
        ok += 1
    sm = summary(run(orig, d2rs), 0)
    assert (sm["frames_compared"], sm["frames_equal"], sm["first"]) == (4, 4, None), sm
    ok += 1
    # the inline unit draw's ret is not compared (1.14d gives lo')
    b = copy.deepcopy(d2rs)
    b[idx[2]]["ret"] = 12345
    assert not run(orig, b)["diffs"]
    ok += 1
    # a dropped d2rs draw: missing; an extra one: extra
    b = copy.deepcopy(d2rs)
    del b[idx[2]]
    d = run(orig, b)["diffs"][0]
    assert (d["frame"], d["owner"], d["index"], d["field"]) == (2, "unit 1:5", 1, "missing"), d
    b = copy.deepcopy(d2rs)
    extra = copy.deepcopy(b[idx[2]])
    b.insert(idx[2] + 1, extra)
    d = run(orig, b)["diffs"][0]
    assert (d["frame"], d["owner"], d["index"], d["field"]) == (2, "unit 1:5", 2, "extra"), d
    ok += 2
    # a draw moved to the next frame
    b = copy.deepcopy(d2rs)
    b[idx[0]]["frame"] = 3
    d = run(orig, b)["diffs"][0]
    assert (d["frame"], d["owner"], d["field"]) == (2, "game", "missing"), d
    ok += 1
    # d2rs one game-seed step behind from creation: the shift is found
    b = copy.deepcopy(d2rs)
    first = next(x for x in b if x.get("owner") == "game")
    b.remove(first)
    r = run(orig, b, state_only=True)
    assert r["seq"]["first"][0] == 0 and r["seq"]["shift"][0] == 1, r["seq"]
    ok += 1
    # the same game states one frame later: the move is named
    b = copy.deepcopy(d2rs)
    for x in b:
        if x.get("owner") == "game" and x["frame"] >= 2:
            x["frame"] += 1
    r = run(orig, b, state_only=True)
    assert r["seq"]["first"] is None and r["seq"]["moved"][0] == 3, r["seq"]
    ok += 1
    # other seeds are never compared
    b = copy.deepcopy(d2rs)
    for x in b:
        if x.get("owner") == "other":
            x["after"] = [0, 0]
    assert not run(orig, b)["diffs"]
    ok += 1
    # a 1.14d file with skipped inline ranges is at best partial
    a = copy.deepcopy(orig)
    a[0]["skip_inline"] = [["0x642000", "0x682000"]]
    assert report(run(a, d2rs), a[0], d2rs[0], out=sink) == 2
    ok += 1
    # a file without frames is an error
    a = copy.deepcopy(orig)
    for x in a:
        x.pop("frame", None)
    try:
        run(a, d2rs)
        raise AssertionError("accepted draws without frames")
    except RngError:
        ok += 1
    sink.close()
    print(f"rng_diff selftest: {ok} checks passed")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("orig", nargs="?")
    ap.add_argument("d2rs", nargs="?")
    ap.add_argument("--next", type=int, default=20)
    ap.add_argument("--owners", default="game,unit", help="owner kinds compared (game,unit)")
    ap.add_argument("--from", dest="lo", type=int, default=None)
    ap.add_argument("--to", dest="hi", type=int, default=None)
    ap.add_argument("--state-only", action="store_true",
                    help="compare seed states only (not op / n / min / ret)")
    ap.add_argument("--json", default=None, metavar="FILE",
                    help="also write a machine-readable summary (diff-summary-1) here")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not a.orig or not a.d2rs:
        ap.error("two files are needed")
    try:
        ha, ra, _ = load(a.orig)
        hb, rb, _ = load(a.d2rs)
        owners = tuple(x for x in a.owners.split(",") if x)
        r = compare((ha, ra), (hb, rb), owners, a.lo, a.hi, a.state_only)
    except (RngError, OSError, KeyError, ValueError) as e:
        print(f"error: {e}", file=sys.stderr)
        if a.json:
            write_json(a.json, {"format": "diff-summary-1", "channel": "rng", "code": 3,
                                "verdict": "ERROR", "error": str(e)})
        return 3
    code = report(r, ha, hb, a.next)
    if a.json:
        write_json(a.json, summary(r, code))
    return code


if __name__ == "__main__":
    sys.exit(main())
