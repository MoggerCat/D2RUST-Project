"""Compare two game-state snapshot files (format state-1) and print the
first divergence in game state, then the next N, then per field the
first frame it differs (specs/tools/state-snapshot.md §4).

    python3 state_diff.py ORIG.state.jsonl D2RS.state.jsonl [--next 20]
        [--ignore FIELD,...] [--types 0,1,...] [--frame-offset K]
        [--from F] [--to F] [--perturb FRAME:TYPE:GUID:FIELD]
    python3 state_diff.py --selftest

Exit codes: 0 MATCH, 1 DIVERGED, 2 PARTIAL, 3 error.

Standard library only; runs on Linux and Windows. Our own code.
"""

import argparse
import json
import sys

FORMAT = "state-1"
# Unit fields in the order of the spec's §2 table (comparison order).
FIELDS = ["ut", "g", "cl", "m", "x", "y", "xf", "yf", "tx", "ty", "d", "fr", "fc", "sp",
          "s", "act", "lv", "hp", "hpx", "mp", "mpx", "st", "stx", "str", "ene", "dex",
          "vit", "lvl", "own"]
TYPE_NAMES = {0: "player", 1: "monster", 2: "object", 3: "missile", 4: "item", 5: "tile"}


class StateError(Exception):
    pass


def load(path):
    """(header, {frame: snap}, footer) of one state-1 file."""
    header, snaps, footer = None, {}, None
    with open(path, encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except ValueError as e:
                raise StateError(f"{path}:{n}: not JSON ({e})")
            k = rec.get("k")
            if header is None:
                if k != "header" or rec.get("format") != FORMAT:
                    raise StateError(f"{path}:{n}: not a {FORMAT} file (first line must be "
                                     f"its header)")
                header = rec
            elif k == "snap":
                snaps[rec["f"]] = rec
            elif k == "footer":
                footer = rec
    if header is None:
        raise StateError(f"{path}: empty")
    return header, snaps, footer


def key_of(u):
    return (u["ut"], u["g"])


def uname(u):
    t = u.get("ut")
    return f"{TYPE_NAMES.get(t, t)} {t}:{u.get('g')}"


def compare(a, b, ignore=(), types=None, frame_offset=0, lo=None, hi=None, perturb=None,
            limit=20):
    """Compare two loaded files. Returns a dict with the divergences (in
    report order, at most 1 + limit kept), per-field firsts and counts,
    and the summary numbers."""
    ha, sa, _ = a
    hb, sb, _ = b
    fa, fb = set(ha.get("fields", [])), set(hb.get("fields", []))
    ign = set(ignore)
    fields = [k for k in FIELDS if k in fa and k in fb and k not in ign and k not in ("ut", "g")]
    one_sided = sorted((fa ^ fb) - ign - {"ut", "g"},
                       key=lambda k: FIELDS.index(k) if k in FIELDS else 99)
    # d2rs frame + offset = 1.14d frame
    sb2 = {f + frame_offset: s for f, s in sb.items()}
    frames = sorted(set(sa) & set(sb2))
    if lo is not None:
        frames = [f for f in frames if f >= lo]
    if hi is not None:
        frames = [f for f in frames if f <= hi]
    if not frames:
        raise StateError("no common frame (check --frame-offset, --from, --to)")
    if perturb:
        pf, pt, pg, pk = perturb
        snap = sb2.get(pf)
        hit = None
        if snap:
            for u in snap["units"]:
                if u["ut"] == pt and u["g"] == pg:
                    hit = u
        if hit is None and pk != "seed":
            raise StateError(f"--perturb: no unit {pt}:{pg} at frame {pf}")
        if pk == "seed":
            snap["seed"] = [snap["seed"][0] ^ 1, snap["seed"][1]]
        elif pk in hit:
            v = hit[pk]
            hit[pk] = [v[0] ^ 1, v[1]] if isinstance(v, list) else v + 1
        else:
            hit[pk] = 0
    divs, total = [], 0
    first_by_field, count_by_field = {}, {}
    bad_frames = set()
    units_compared = 0

    def add(d):
        nonlocal total
        total += 1
        bad_frames.add(d["f"])
        name = d["field"]
        count_by_field[name] = count_by_field.get(name, 0) + 1
        if name not in first_by_field:
            first_by_field[name] = d
        if len(divs) <= limit:
            divs.append(d)

    for f in frames:
        x, y = sa[f], sb2[f]
        if x.get("seed") != y.get("seed"):
            add({"f": f, "unit": None, "field": "seed", "exp": x.get("seed"), "got": y.get("seed"),
                 "a": None, "b": None})
        ua = {key_of(u): u for u in x["units"] if types is None or u["ut"] in types}
        ub = {key_of(u): u for u in y["units"] if types is None or u["ut"] in types}
        for k in sorted(set(ua) | set(ub)):
            p, q = ua.get(k), ub.get(k)
            if q is None:
                add({"f": f, "unit": k, "field": "(unit)", "exp": "present", "got": "missing",
                     "a": p, "b": None})
                continue
            if p is None:
                add({"f": f, "unit": k, "field": "(unit)", "exp": "absent", "got": "extra",
                     "a": None, "b": q})
                continue
            units_compared += 1
            for fld in fields:
                if p.get(fld) != q.get(fld):
                    add({"f": f, "unit": k, "field": fld,
                         "exp": p.get(fld, "(absent)"), "got": q.get(fld, "(absent)"),
                         "a": p, "b": q})
    return {"divs": divs, "total": total, "first_by_field": first_by_field,
            "count_by_field": count_by_field, "frames": frames, "fields": fields,
            "one_sided": one_sided, "units_compared": units_compared,
            "clean_frames": len([f for f in frames if f not in bad_frames])}


def where(d):
    if d["unit"] is None:
        return f"frame {d['f']} game"
    t, g = d["unit"]
    cls = []
    for side in ("a", "b"):
        u = d[side]
        cls.append("-" if u is None else str(u.get("cl", "?")))
    c = cls[0] if cls[0] == cls[1] else f"{cls[0]}/{cls[1]}"
    return f"frame {d['f']} {TYPE_NAMES.get(t, t)} {t}:{g} class {c}"


def fmt_unit(u):
    if u is None:
        return "(none)"
    return " ".join(f"{k}={u[k]}" for k in FIELDS if k in u) + \
        "".join(f" {k}={v}" for k, v in u.items() if k not in FIELDS)


def report(r, ha, hb, out=sys.stdout):
    w = out.write
    w(f"1.14d: {ha.get('tool')}  {ha.get('command', '')}\n")
    w(f"d2rs:  {hb.get('tool')}  {hb.get('command', '')}\n")
    divs = r["divs"]
    if divs:
        d = divs[0]
        w(f"\nFIRST DIVERGENCE: {where(d)}, field {d['field']}: "
          f"1.14d {d['exp']} vs d2rs {d['got']}\n")
        if d["unit"] is not None:
            w(f"  1.14d: {fmt_unit(d['a'])}\n  d2rs:  {fmt_unit(d['b'])}\n")
        if len(divs) > 1:
            w(f"\nnext {len(divs) - 1}:\n")
            for d in divs[1:]:
                w(f"  {where(d)}: {d['field']} {d['exp']} vs {d['got']}\n")
        w("\nfirst frame per field (field: where, 1.14d vs d2rs; differing (frame, unit) pairs):\n")
        for fld in sorted(r["first_by_field"], key=lambda k: (r["first_by_field"][k]["f"],
                                                                k != "seed",
                                                                k != "(unit)",
                                                                FIELDS.index(k) if k in FIELDS
                                                                else -1)):
            d = r["first_by_field"][fld]
            w(f"  {fld}: {where(d)}, {d['exp']} vs {d['got']}; {r['count_by_field'][fld]}\n")
    fr = r["frames"]
    w(f"\nframes compared: {len(fr)} ({fr[0]}..{fr[-1]}), unit records compared: "
      f"{r['units_compared']}, fields: {' '.join(r['fields'])}\n")
    if fr:
        w(f"frames with zero divergence: {r['clean_frames']}/{len(fr)} "
          f"({100 * r['clean_frames'] // len(fr)}%)\n")
    if r["one_sided"]:
        w(f"not compared (one side only): {' '.join(r['one_sided'])}\n")
    for side, h in (("1.14d", ha), ("d2rs", hb)):
        for g in h.get("gaps", []):
            w(f"gap ({side}): {g}\n")
    if divs:
        verdict, code = "DIVERGED", 1
        w(f"\n{verdict}: {r['total']} differences\n")
    elif r["one_sided"] or ha.get("gaps") or hb.get("gaps"):
        verdict, code = "PARTIAL", 2
        w(f"\n{verdict}: no difference in what was compared\n")
    else:
        verdict, code = "MATCH", 0
        w(f"\n{verdict}\n")
    return code


def parse_perturb(s):
    f, t, g, k = s.split(":")
    return int(f), int(t), int(g), k


# --- self-test ---------------------------------------------------------------

def synthetic(gaps=()):
    hdr = {"k": "header", "format": FORMAT, "side": "orig", "tool": "selftest", "command": "x",
           "fields": FIELDS[:-1], "gaps": list(gaps)}
    snaps = {}
    for f in range(1, 4):
        units = []
        for t, g in ((0, 1), (1, 3), (1, 4), (2, 7), (4, 9)):
            u = {"ut": t, "g": g}
            for i, k in enumerate(FIELDS[2:-1]):
                u[k] = [f * 100 + g, i] if k == "s" else f * 1000 + g * 10 + i
            units.append(u)
        snaps[f] = {"k": "snap", "f": f, "seed": [f, 666], "units": units}
    return hdr, snaps, {"k": "footer"}


def clone(x):
    return json.loads(json.dumps(x))


def selftest():
    import io
    a = synthetic()
    sink = io.StringIO()
    ok = 0
    # self compare: match
    r = compare(a, clone_file(a))
    assert report(r, a[0], a[0], sink) == 0 and not r["divs"], "self compare must match"
    ok += 1
    # every field of every unit of every frame, perturbed: found first at that place
    for f, snap in a[1].items():
        for u in snap["units"]:
            for k in FIELDS[2:-1]:
                b = clone_file(a)
                r = compare(a, b, perturb=(f, u["ut"], u["g"], k))
                d = r["divs"][0]
                assert (d["f"], d["unit"], d["field"]) == (f, (u["ut"], u["g"]), k), \
                    (f, u["ut"], u["g"], k, d)
                assert report(r, a[0], b[0], sink) == 1
                ok += 1
        b = clone_file(a)
        r = compare(a, b, perturb=(f, 0, 1, "seed"))
        assert (r["divs"][0]["f"], r["divs"][0]["field"]) == (f, "seed")
        ok += 1
    # a unit removed on the d2rs side: missing at its first frame
    b = clone_file(a)
    for snap in b[1].values():
        snap["units"] = [u for u in snap["units"] if (u["ut"], u["g"]) != (2, 7)]
    r = compare(a, b)
    assert (r["divs"][0]["f"], r["divs"][0]["unit"], r["divs"][0]["got"]) == (1, (2, 7), "missing")
    ok += 1
    # extra unit
    b = clone_file(a)
    b[1][2]["units"].append({"ut": 3, "g": 50})
    r = compare(a, b)
    assert (r["divs"][0]["f"], r["divs"][0]["got"]) == (2, "extra")
    ok += 1
    # one-sided field: partial
    b = clone_file(a)
    b[0]["fields"] = [k for k in b[0]["fields"] if k != "fr"]
    r = compare(a, b)
    assert report(r, a[0], b[0], sink) == 2 and r["one_sided"] == ["fr"]
    ok += 1
    # gap: partial
    b = clone_file(synthetic(gaps=["x"]))
    assert report(compare(a, b), a[0], b[0], sink) == 2
    ok += 1
    # frame offset: d2rs frames shifted by -1 align with +1
    b = clone_file(a)
    b = (b[0], {f - 1: s for f, s in b[1].items()}, b[2])
    assert not compare(a, b, frame_offset=1)["divs"]
    ok += 1
    # --ignore and --types
    b = clone_file(a)
    r = compare(a, b, perturb=(1, 2, 7, "m"), ignore=["m"])
    assert not r["divs"]
    b = clone_file(a)
    assert not compare(a, b, perturb=(1, 2, 7, "m"), types={0, 1})["divs"]
    ok += 2
    # no common frame: error
    try:
        compare(a, clone_file(a), lo=10)
        raise AssertionError("expected an error")
    except StateError:
        ok += 1
    print(f"state_diff selftest: {ok} checks passed")
    return 0


def clone_file(a):
    return clone(a[0]), {f: clone(s) for f, s in a[1].items()}, clone(a[2])


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("orig", nargs="?")
    ap.add_argument("d2rs", nargs="?")
    ap.add_argument("--next", type=int, default=20, help="divergences after the first (20)")
    ap.add_argument("--ignore", default="", help="fields not compared, comma-separated")
    ap.add_argument("--types", default="", help="unit types compared, comma-separated")
    ap.add_argument("--frame-offset", type=int, default=0, help="d2rs frame + K = 1.14d frame")
    ap.add_argument("--from", dest="lo", type=int, default=None)
    ap.add_argument("--to", dest="hi", type=int, default=None)
    ap.add_argument("--perturb", default=None, help="FRAME:TYPE:GUID:FIELD (self-test aid)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not a.orig or not a.d2rs:
        ap.error("two files needed")
    try:
        fa, fb = load(a.orig), load(a.d2rs)
        r = compare(fa, fb, ignore=[x for x in a.ignore.split(",") if x],
                    types={int(x) for x in a.types.split(",") if x} or None,
                    frame_offset=a.frame_offset, lo=a.lo, hi=a.hi,
                    perturb=parse_perturb(a.perturb) if a.perturb else None, limit=a.next)
    except (StateError, OSError, KeyError, ValueError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 3
    return report(r, fa[0], fb[0])


if __name__ == "__main__":
    sys.exit(main())
