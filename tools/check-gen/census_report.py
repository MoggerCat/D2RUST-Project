"""Report half of census.py: the records of traces/raw/census -> the ledger part
docs/handoff/ledger/q-run-missiles-states.tsv, a per-check census TSV and a
markdown summary. Our own code."""
import collections
import glob
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(ROOT, "traces", "raw", "census")
COLS = ["area", "kind", "group", "source_1.14d", "specs", "spec_status", "checks",
        "last_verdict", "exercised", "provisional", "needs_pc1", "owner", "state",
        "size", "note"]
CENSUS_TSV = os.path.join(ROOT, "docs", "handoff", "q-run-missiles-states-census.tsv")
SESSION = "q-run-missiles-states"


def read_ledger(path):
    rows = [l.rstrip("\n").split("\t") for l in open(path, encoding="utf-8") if not l.startswith("#")]
    head = rows[0]
    return [dict(zip(head, r)) for r in rows[1:]]


def load_records():
    recs = {}
    for p in sorted(glob.glob(os.path.join(OUT, "*.json"))):
        d = json.load(open(p))
        recs[d["name"]] = d
    return recs


def channels(rec):
    """[(channel, summary)] that differ from the poke on; the state channel's all-unit
    diff counts only when the units before the poke were equal (else the setup differs
    and the cow / player fields are not this row's)."""
    out = []
    clean_before = (rec.get("state_before") or {}).get("verdict") not in ("DIVERGED", "ERROR")
    for key, label in (("state_types3", "state(missile units)"), ("state_all", "state(all units)"),
                       ("rng", "rng"), ("packets", "packets")):
        s = rec.get(key)
        if not s:
            continue
        if key == "state_all" and not clean_before:
            continue
        if s["verdict"] in ("DIVERGED", "ERROR"):
            out.append((label, s))
    return out


def verdict(rec):
    """(verdict string, state, first frame or None, note, exercised)."""
    po, pd = rec.get("pokes_orig", []), rec.get("pokes_d2rs", [])
    bad = [p for p in po if p["r"] != "ok"] + [p for p in pd if p["r"] != "ok"]
    want = 4 if rec["family"] == "state" else 3
    if bad or len(po) < want or len(pd) < want:
        why = ", ".join(f"{p['d']} {p['r']}" for p in bad) or "a poke did not run"
        return "-", "NO-CHECK", None, f"no verdict: {why}", "?"
    if rec["family"] == "missile":
        ex = "yes" if rec.get("seen_orig", 0) > 0 else "no"
    else:
        ex = "yes" if rec.get("pkts_orig") else "no"
    ch = channels(rec)
    if ch:
        label, s = min(ch, key=lambda c: (c[1]["frame"] if c[1]["frame"] is not None else 10 ** 9))
        f = s["frame"]
        return f"DIVERGED@{f}", "DIVERGED", f, f"{label} {s['text']}", ex
    return "PARTIAL", "NO-CHECK", None, "", ex


def owner_of(label, text):
    """Branch of the owner session route.py names for a first-difference line."""
    line = text if label.startswith("packets") or label == "rng" else "FIRST DIVERGENCE: " + text
    try:
        p = subprocess.run([sys.executable, os.path.join(ROOT, "tools", "coord", "route.py"), "--json", "-"],
                           input=line, capture_output=True, text=True, timeout=60)
        d = json.loads(p.stdout)
        r = d["routes"][0]
        if r.get("routed") and r.get("owner"):
            return r["owner"]["branch"]
    except Exception:
        pass
    return "claude/coord-resume-3"


def clip(text, n=240):
    text = re.sub(r"\s+", " ", text).strip()
    return text if len(text) <= n else text[:n - 1] + "…"


def report(a):
    ledger = read_ledger(a.ledger)
    recs = load_records()
    idx = {}  # lowercase Missiles.txt name -> check name
    for line in open(os.path.join(ROOT, "traces", "checks", "gen", "INDEX.tsv"), encoding="utf-8"):
        c = line.rstrip("\n").split("\t")
        if len(c) >= 5 and c[1] == "missile":
            m = re.fullmatch(r"missile (.*) \((\d+)\)", c[4])
            idx[m.group(1).lower()] = c[0]
    state_by_id = {n.rsplit("-", 1)[1]: n for n in recs if n.startswith("gen-state-")}
    rows, census = [], []
    for r in ledger:
        area = r["area"]
        if not (area.startswith(("missile.", "state.")) and r["group"] in ("skills", "coverage")):
            continue
        src = r["source_1.14d"]
        if area.startswith("missile."):
            m = re.search(r"[Mm]issiles\.txt rows? (.*?)(?: \(made by| \(\d+\)$|$)", src)
            names = [n.strip().lower() for n in m.group(1).split(",")] if r["group"] == "skills" \
                else [re.fullmatch(r"missiles\.txt (.*) \(\d+\)", src).group(1).lower()]
            checks = [idx[n] for n in names if n in idx]
        else:
            sid = re.search(r"\((\d+)\)", src).group(1)
            checks = [state_by_id[sid]] if sid in state_by_id else []
        have = [recs[c] for c in checks if c in recs]
        out = dict(r)
        out["checks"] = "-"  # ledger.py only knows traces/checks/*.check, not gen/: the names go in the note
        gen = ",".join(checks)
        if not have:
            out.update(last_verdict="-", state="NO-CHECK", exercised=r["exercised"],
                       note="no census record (check missing or run failed)")
            rows.append(out)
            continue
        vs = [verdict(h) for h in have]
        for h, v in zip(have, vs):
            census.append((h["name"], area, h["poke_frame"], v[0], v[4],
                           *[(h.get(k) or {}).get("verdict", "-") for k in
                             ("state_types3", "state_all", "state_before", "rng", "packets")],
                           clip(v[3], 200)))
        div = [(v, h) for v, h in zip(vs, have) if v[1] == "DIVERGED"]
        none = [(v, h) for v, h in zip(vs, have) if v[0] == "-"]
        if div:
            (v, h) = min(div, key=lambda x: x[0][2])
            label = v[3].split(" ", 1)[0]
            out.update(last_verdict=v[0], state="DIVERGED", size="M",
                       owner=owner_of(label, v[3].split(" ", 1)[1]),
                       note=clip(f"[{gen}] {v[3]}" + (f" (+{len(div) - 1} more rows diverge)" if len(div) > 1 else ""), 220))
        elif none and len(none) == len(vs):
            out.update(last_verdict="-", state="NO-CHECK", note=clip(f"[{gen}] {none[0][0][3]}"))
        else:
            n = len(have)
            out.update(last_verdict="PARTIAL", state="NO-CHECK", size="S",
                       note=clip(f"[{gen}] {n} check(s): state, rng and packets equal from the poke on "
                                 "(PARTIAL: own/client not compared)"
                                 + (f"; {len(none)} row(s) without a verdict" if none else "")))
        exs = {v[4] for v in vs}
        out["exercised"] = "yes" if "yes" in exs else ("no" if "no" in exs else "?")
        out["provisional"] = "1" if area.startswith("missile.") and r["group"] == "skills" else "0"
        rows.append(out)
    with open(a.part, "w", encoding="utf-8", newline="\n") as f:
        f.write("#ledger 1\n" + "\t".join(COLS) + "\n")
        for o in sorted(rows, key=lambda x: x["area"]):
            f.write("\t".join(str(o.get(c, "-")).replace("\t", " ") for c in COLS) + "\n")
    with open(CENSUS_TSV, "w", encoding="utf-8", newline="\n") as f:
        f.write("#census 1\n# generated by tools/check-gen/census.py report; one line per generated check, "
                "compared from the poke frame on (state ignoring q)\n")
        f.write("check\tarea\tpoke_frame\tverdict\texercised\tstate_missiles\tstate_all\tstate_before\trng\tpackets\tfirst\n")
        for c in sorted(census):
            f.write("\t".join(str(x) for x in c) + "\n")
    cnt = collections.Counter((o["area"].split(".")[0], o["last_verdict"].split("@")[0]) for o in rows)
    print(f"report: {len(rows)} rows -> {os.path.relpath(a.part, ROOT)}: {dict(cnt)}")
    return 0
