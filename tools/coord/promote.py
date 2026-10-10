#!/usr/bin/env python3
"""Promote PARTIAL ledger rows by the DECIDED REC-2055 / REC-2056 rule.

    python3 tools/coord/promote.py SUITE.json [--ledger F] [--checks-dir D ...]
            [--work-dir traces/raw/suite] [--part-name NAME] [--out PART.tsv]
            [--list]
    python3 tools/coord/promote.py --selftest

SUITE.json is `suite.py --json`. Rule (specs/tools/scenario-diff.md open
questions 6-7): a check qualifies when
  * every channel is MATCH, except
  * a state channel PARTIAL whose only cause is the d2rs header's client gap
    (`state_dump.rs` RUN_GAPS: gap text starting "client: headless bridge", no
    field on one side only), read from the two state headers in the check's
    work dir (<work-dir>/<check>/orig.state.jsonl, d2rs.state.jsonl), and
  * an items channel PARTIAL only when no item was created on either side
    (items_orig = items_d2rs = 0, REC-2056);
  * the check file has no `ignore` line, and
  * it is pokes-only (no `input`, no `at .. send`) or its packets channel is MATCH.
A ledger row (checks column: comma list) with last_verdict PARTIAL and state
DIVERGED becomes EQUAL when every one of its checks qualifies; otherwise its
state follows the fresh verdict of its checks: DIVERGED if any check diverged
(or errored), else NO-CHECK with the reason in the note. Rows with a check not
in the suite json are left out of the part. Python 3 stdlib only. Our own code.
"""

import argparse
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
GAP_PREFIX = "client: headless bridge"
RULE = "REC-2055/2056 (specs/tools/scenario-diff.md open questions 6-7)"


def read_check(path):
    """(has_ignore, has_input_or_send) of a check file's lines."""
    ign = inp = False
    with open(path, encoding="utf-8") as f:
        for ln in f:
            w = ln.split()
            if not w or w[0].startswith("#"):
                continue
            if w[0] == "ignore":
                ign = True
            elif w[0] == "input":
                inp = True
            elif w[0] == "at" and len(w) > 2 and w[2] == "send":
                inp = True
    return ign, inp


def read_header(path):
    with open(path, encoding="utf-8") as f:
        return json.loads(f.readline())


def state_cause(work):
    """None when the state PARTIAL's only cause is the client gap, else why not."""
    try:
        ha = read_header(os.path.join(work, "orig.state.jsonl"))
        hb = read_header(os.path.join(work, "d2rs.state.jsonl"))
    except (OSError, ValueError) as e:
        return f"state headers unreadable ({e})"
    fa, fb = set(ha.get("fields", [])), set(hb.get("fields", []))
    one = sorted((fa ^ fb) - {"ut", "g"})
    if one:
        return "state fields on one side only: " + " ".join(one)
    gaps = list(ha.get("gaps", [])) + list(hb.get("gaps", []))
    if not gaps:
        return "state PARTIAL without a gap"
    other = [g for g in gaps if not g.startswith(GAP_PREFIX)]
    if other:
        return "state gap other than the client gap: " + other[0][:80]
    return None


def classify(name, rec, check_path, work_root):
    """(verdict, reason): verdict EQUAL | PARTIAL | DIVERGED | ERROR; reason
    is '' for EQUAL (the rule holds), else the first condition that fails."""
    ch = rec.get("channels") or {}
    if rec.get("error") or not ch:
        return "ERROR", rec.get("error") or "no channels"
    vs = {c: (v or {}).get("verdict") for c, v in ch.items()}
    bad = sorted(c for c, v in vs.items() if v in ("DIVERGED", "ERROR", None))
    if bad:
        kind = "DIVERGED" if "DIVERGED" in (vs[c] for c in bad) else "ERROR"
        return kind, "channel " + ", ".join(f"{c} {vs[c]}" for c in bad)
    why = []
    try:
        ign, inp = read_check(check_path)
    except OSError as e:
        return "ERROR", f"check file unreadable ({e})"
    if ign:
        why.append("check has an ignore line")
    for c, v in sorted(vs.items()):
        if v == "MATCH":
            continue
        if v != "PARTIAL":
            why.append(f"{c} {v}")
        elif c == "state":
            r = state_cause(os.path.join(work_root, name))
            if r:
                why.append(r)
        elif c == "items":
            sm = ch[c].get("summary") or {}
            if sm.get("items_orig") != 0 or sm.get("items_d2rs") != 0:
                why.append(f"items PARTIAL with items 1.14d {sm.get('items_orig')} / "
                           f"d2rs {sm.get('items_d2rs')} (not empty on both sides)")
        else:
            why.append(f"{c} PARTIAL")
    if inp and vs.get("packets") != "MATCH":
        why.append(f"check has input/send and packets is {vs.get('packets', 'not a channel')}")
    if not why:
        return "EQUAL", ""
    return "PARTIAL", "; ".join(why)


def find_check(name, dirs):
    for d in dirs:
        p = os.path.join(d, name + ".check")
        if os.path.exists(p):
            return p
    return None


def read_tsv(path):
    with open(path, encoding="utf-8") as f:
        lines = [ln.rstrip("\n") for ln in f]
    magic = [ln for ln in lines if ln.startswith("#") and not ln.startswith("# ")][0]
    body = [ln for ln in lines if not ln.startswith("#")]
    return magic, body[0].split("\t"), [ln.split("\t") for ln in body[1:] if ln.strip()]


def promote(rows, cols, results, part, today):
    """The part's rows (dicts) for the PARTIAL/DIVERGED rows whose checks all
    ran; results: name -> (verdict, reason)."""
    out, stats = [], {"EQUAL": 0, "DIVERGED": 0, "NO-CHECK": 0, "skipped": 0}
    for r in rows:
        d = dict(zip(cols, r))
        if d["last_verdict"] != "PARTIAL" or d["state"] != "DIVERGED":
            continue
        names = [c for c in d["checks"].split(",") if c and c != "-"]
        if not names or any(n not in results for n in names):
            stats["skipped"] += 1
            continue
        rs = [results[n] for n in names]
        if all(v == "EQUAL" for v, _ in rs):
            d["state"], d["size"] = "EQUAL", "-"
            d["note"] = (f"{','.join(names)} ({part}, {today}, fresh run): every channel MATCH "
                         f"except a state channel PARTIAL only for the d2rs header's client gap "
                         f"(RUN_GAPS); every unit field of both sides compared, no ignore line; "
                         f"pokes-only or packets MATCH; DECIDED {RULE}")
            stats["EQUAL"] += 1
        elif any(v in ("DIVERGED", "ERROR") for v, _ in rs):
            bad = [f"{n}: {w}" for n, (v, w) in zip(names, rs) if v in ("DIVERGED", "ERROR")]
            d["state"] = "DIVERGED"
            d["note"] = f"{part} {today} fresh run: " + "; ".join(bad)
            stats["DIVERGED"] += 1
        else:
            why = [f"{n}: {w}" for n, (v, w) in zip(names, rs) if v == "PARTIAL"]
            d["state"] = "NO-CHECK"
            d["note"] = (f"{part} {today} fresh run: PARTIAL, not promoted by the REC-2055/2056 "
                         f"rule: " + "; ".join(why))
            stats["NO-CHECK"] += 1
        out.append(d)
    return out, stats


def selftest():
    with tempfile.TemporaryDirectory() as t:
        cd, wd = os.path.join(t, "c"), os.path.join(t, "w")
        os.makedirs(cd)

        def mk(name, text, fa=("hp", "x"), fb=("hp", "x"), gaps=(GAP_PREFIX + " ...",)):
            with open(os.path.join(cd, name + ".check"), "w") as f:
                f.write(text)
            w = os.path.join(wd, name)
            os.makedirs(w)
            for fn, fl, gp in (("orig.state.jsonl", fa, ()), ("d2rs.state.jsonl", fb, gaps)):
                with open(os.path.join(w, fn), "w") as f:
                    f.write(json.dumps({"k": "header", "fields": list(fl), "gaps": list(gp)}) + "\n")

        def rec(**ch):
            return {"channels": {c: {"verdict": v, "summary": s} for c, (v, s) in ch.items()}}
        P, M = ("PARTIAL", None), ("MATCH", None)
        mk("ok", "check 1\nchannels state\n")
        mk("ign", "check 1\nignore q\n")
        mk("inp", "check 1\ninput frame 1\n")
        mk("snd", "check 1\nat 3 send Foo a=1\n")
        mk("one", "check 1\n", fb=("hp",))
        mk("gap2", "check 1\n", gaps=("other gap",))
        p = lambda n: os.path.join(cd, n + ".check")
        c = lambda n, r: classify(n, r, p(n), wd)
        assert c("ok", rec(state=P))[0] == "EQUAL"
        assert c("ok", rec(state=M, packets=M))[0] == "EQUAL"
        assert c("ok", rec(state=P, rng=P))[0] == "PARTIAL"
        assert "ignore" in c("ign", rec(state=P))[1]
        assert c("inp", rec(state=P))[0] == "PARTIAL"
        assert c("inp", rec(state=P, packets=M))[0] == "EQUAL"
        assert c("snd", rec(state=P, packets=P))[0] == "PARTIAL"
        assert "one side" in c("one", rec(state=P))[1]
        assert "other than" in c("gap2", rec(state=P))[1]
        empty = {"items_orig": 0, "items_d2rs": 0}
        assert c("ok", rec(state=P, items=("PARTIAL", empty)))[0] == "EQUAL"
        assert c("ok", rec(state=P, items=("PARTIAL", {"items_orig": 1, "items_d2rs": 1})))[0] \
            == "PARTIAL"
        assert c("ok", rec(state=P, packets=("DIVERGED", None)))[0] == "DIVERGED"
        assert c("ok", {"channels": {}, "error": "x"})[0] == "ERROR"
        cols = ["area", "checks", "last_verdict", "state", "size", "note"]
        rows = [["a", "ok", "PARTIAL", "DIVERGED", "M", "old"],
                ["b", "ok,ign", "PARTIAL", "DIVERGED", "M", "old"],
                ["c", "gone", "PARTIAL", "DIVERGED", "M", "old"],
                ["d", "ok", "MATCH", "EQUAL", "-", "old"]]
        res = {"ok": ("EQUAL", ""), "ign": ("PARTIAL", "check has an ignore line")}
        out, st = promote(rows, cols, res, "rc-x", "2026-10-10")
        assert [d["state"] for d in out] == ["EQUAL", "NO-CHECK"] and st["skipped"] == 1, (out, st)
    print("promote.py selftest ok")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("suite_json", nargs="?")
    ap.add_argument("--ledger", default=os.path.join(REPO, "docs/handoff/fidelity-ledger.tsv"))
    ap.add_argument("--checks-dir", action="append", default=None)
    ap.add_argument("--work-dir", default=os.path.join(REPO, "traces/raw/suite"))
    ap.add_argument("--part-name", default="rc-promote")
    ap.add_argument("--date", default=None)
    ap.add_argument("--out", default=None, help="ledger part to write")
    ap.add_argument("--list", action="store_true", help="print the per-check classification")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if not a.suite_json:
        ap.error("SUITE.json required")
    dirs = a.checks_dir or [os.path.join(REPO, "traces/checks"), os.path.join(REPO, "traces/checks/gen")]
    with open(a.suite_json, encoding="utf-8") as f:
        suite = json.load(f)
    results = {}
    for rec in suite["checks"]:
        n = rec["name"]
        path = find_check(n, dirs)
        results[n] = classify(n, rec, path, a.work_dir) if path else ("ERROR", "no check file")
    if a.list:
        for n, (v, w) in sorted(results.items()):
            print(f"{n}\t{v}\t{w}")
    import datetime
    today = a.date or datetime.date.today().isoformat()
    magic, cols, rows = read_tsv(a.ledger)
    out, st = promote(rows, cols, results, a.part_name, today)
    print(f"promote: {len(results)} checks; rows: {st}")
    if a.out:
        with open(a.out, "w", encoding="utf-8", newline="\n") as f:
            f.write("#ledger 1\n" + "\t".join(cols) + "\n")
            for d in sorted(out, key=lambda d: d["area"]):
                f.write("\t".join(d[c] for c in cols) + "\n")
        print(f"wrote {a.out} ({len(out)} rows)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
