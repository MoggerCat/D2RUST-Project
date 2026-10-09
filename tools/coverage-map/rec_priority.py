#!/usr/bin/env python3
# Spec: specs/tools/coverage-map.md
"""Ranks the open PROVISIONAL REC points by how often real runs execute the
code they mark: LLVM source coverage (an `-C instrument-coverage` d2-client)
of the playthrough and the checks' d2rs side, mapped onto the code
locations of docs/handoff/provisional-index.tsv.

    python3 tools/coverage-map/rec_priority.py --run NAME=FILE.lcov [--run ...]
        [--top 30] [--md docs/handoff/rec-priority.md] [--tsv FILE]
    python3 tools/coverage-map/rec_priority.py --selftest

A location (`file:line` of a PROVISIONAL comment) counts the execution count
of the first instrumented line within 12 lines after it; failing that, of
the function that contains it; a module header (before the first function)
counts the file's busiest function. Locations with `spec-settled` status are
not open and are skipped. Standard library only. Our own code.
"""

import argparse
import collections
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
LOOKAHEAD = 12


def read_lcov(path):
    """-> {relative file: {"da": {line: count}, "fn": [(start, count)]}}"""
    files = {}
    cur = None
    fn_start = {}
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.startswith("SF:"):
                p = line[3:]
                if p.startswith(REPO + "/"):
                    p = p[len(REPO) + 1:]
                cur = files.setdefault(p, {"da": {}, "fn": []})
                fn_start = {}
            elif cur is None:
                continue
            elif line.startswith("FN:"):
                n, name = line[3:].split(",", 1)
                fn_start[name] = int(n)
            elif line.startswith("FNDA:"):
                c, name = line[5:].split(",", 1)
                if name in fn_start:
                    cur["fn"].append((fn_start[name], int(c)))
            elif line.startswith("DA:"):
                n, c = line[3:].split(",")[:2]
                n, c = int(n), int(c)
                cur["da"][n] = max(cur["da"].get(n, 0), c)
            elif line == "end_of_record":
                cur = None
    for v in files.values():
        v["fn"].sort()
    return files


def hits_at(cov, path, line):
    """(count, how) for a code location, or (None, why) when not built."""
    f = cov.get(path)
    if f is None:
        return None, "not in the binary"
    for n in range(line, line + LOOKAHEAD + 1):
        if n in f["da"]:
            return f["da"][n], "line"
    fns = [(s, c) for s, c in f["fn"] if s <= line]
    if fns:
        return max(c for s, c in fns if s == fns[-1][0]), "function"
    if f["fn"]:
        return max(c for _, c in f["fn"]), "file"
    return 0, "file"


def read_tsv(path):
    with open(path, encoding="utf-8") as f:
        rows = [l.rstrip("\n").split("\t") for l in f]
    head = rows[0]
    return [dict(zip(head, r + [""] * (len(head) - len(r)))) for r in rows[1:]]


def handoff_entries(path):
    """REC id -> (title line, Settles text, recorded?) from HANDOFF §7."""
    out = {}
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    i = 0
    while i < len(lines):
        m = re.match(r"^#####\s+(REC-\d+)\s+(.*)$", lines[i])
        if not m:
            i += 1
            continue
        rec, title = m.group(1), m.group(2)
        j = i + 1
        settles, recorded = "", False
        while j < len(lines) and not lines[j].startswith("#####") and not lines[j].startswith("## "):
            t = lines[j]
            if t.startswith("- Settles:"):
                settles = t[len("- Settles:"):].strip()
            if re.match(r"^- (RECORDED|SETTLED|DONE)", t):
                recorded = True
            j += 1
        out.setdefault(rec, (title, settles, recorded))
        i = j
    return out


def spec_refs(*texts):
    seen = []
    for t in texts:
        for m in re.finditer(r"((?:specs/)?(?:[a-z0-9_-]+/)?[a-z0-9_-]+\.md)`?(\s*§\s*[0-9A-Za-z.]*[0-9A-Za-z])?", t):
            r = m.group(1) + (" " + m.group(2).strip() if m.group(2) else "")
            if r not in seen and not r.startswith(("HANDOFF", "METHODS", "PLAN")):
                seen.append(r)
    return seen


def rank(locs, runs, entries):
    """-> sorted list of per-REC dicts."""
    per = collections.defaultdict(lambda: {"sites": [], "hits": collections.Counter()})
    for row in locs:
        rec = row["rec"]
        if not rec.startswith("REC-") or "spec-settled" in row.get("status", ""):
            continue
        path, _, n = row["location"].rpartition(":")
        if not path.startswith("crates/") and not path.startswith("tools/"):
            per[rec]["sites"].append((row["location"], None, "spec text", row["chosen"]))
            continue
        total, how = 0, None
        for name, cov in runs.items():
            c, h = hits_at(cov, path, int(n))
            how = how or h
            if c is not None:
                per[rec]["hits"][name] += c
                total += c
        per[rec]["sites"].append((row["location"], total if how not in (None, "not in the binary") else None,
                                  how, row["chosen"]))
    out = []
    for rec, d in per.items():
        title, settles, recorded = entries.get(rec, ("", "", False))
        code = [s for s in d["sites"] if s[1] is not None]
        out.append({
            "rec": rec, "title": title, "settles": settles, "recorded": recorded,
            "hits": dict(d["hits"]), "total": sum(d["hits"].values()),
            "sites": d["sites"], "sites_hit": sum(1 for s in code if s[1]),
            "sites_code": len(code),
            "specs": spec_refs(settles, *[s[3] for s in d["sites"]])[:4],
        })
    out.sort(key=lambda r: (-r["total"], -r["sites_hit"], r["rec"]))
    return out


def md(ranked, runs, top, argv):
    o = []
    w = o.append
    w("# Open PROVISIONAL RECs by how often real runs hit their code")
    w("")
    w(f"Generated by `tools/coverage-map/rec_priority.py` (`{' '.join(argv)}`).")
    w("Source: LLVM source coverage of a `-C instrument-coverage` release `d2-client` on the real")
    w("1.14d install, per run: " + ", ".join(f"`{n}`" for n in runs) + ". Locations: the open")
    w("(not `spec-settled`) rows of `docs/handoff/provisional-index.tsv`; a location counts the")
    w(f"first instrumented line within {LOOKAHEAD} lines after its comment, else its function, else")
    w("(module header) the file's busiest function. Hits are executions summed over the runs; a")
    w("REC sums its locations. Question = the REC's Settles line in `docs/HANDOFF.md` §7, else its")
    w("title. RECs whose HANDOFF entry already says RECORDED are marked: their capture exists and")
    w("only the fold is left.")
    w("")
    hit = [r for r in ranked if r["total"] > 0]
    w(f"Open RECs with code locations: {sum(1 for r in ranked if r['sites_code'])}; hit by at least one run: {len(hit)}; "
      f"never hit: {sum(1 for r in ranked if r['sites_code'] and not r['total'])}; spec-text only: "
      f"{sum(1 for r in ranked if not r['sites_code'])}.")
    w("")
    w("| # | REC | Hits (" + " / ".join(runs) + ") | Sites hit | Spec | Question |")
    w("|---|---|---|---|---|---|")
    for i, r in enumerate(ranked[:top], 1):
        hits = " / ".join(f"{r['hits'].get(n, 0):,}" for n in runs)
        q = r["settles"] or r["title"]
        q = q.replace("|", "\\|")
        if len(q) > 420:
            q = q[:417] + "…"
        title = r["title"].replace("|", "\\|")
        mark = " (RECORDED, fold pending)" if r["recorded"] else ""
        specs = ", ".join(f"`{s}`" for s in r["specs"]) or "-"
        w(f"| {i} | **{r['rec']}**{mark}: {title} | {hits} | {r['sites_hit']}/{r['sites_code']} | {specs} | {q} |")
    w("")
    w("## Hot locations of the top 30")
    w("")
    for r in ranked[:top]:
        best = sorted((s for s in r["sites"] if s[1]), key=lambda s: -s[1])[:3]
        if best:
            w(f"- {r['rec']}: " + "; ".join(f"`{s[0]}` {s[1]:,} ({s[2]})" for s in best))
    w("")
    never = [r for r in ranked if r["sites_code"] and not r["total"]]
    w(f"## Open RECs whose code no run reached ({len(never)})")
    w("")
    w(", ".join(r["rec"] for r in sorted(never, key=lambda r: int(r["rec"][4:]))) or "None.")
    w("")
    return "\n".join(o) + "\n"


def selftest():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "a.lcov")
        with open(p, "w") as f:
            f.write(f"SF:{REPO}/crates/x.rs\nFN:10,f\nFNDA:7,f\nFN:40,g\nFNDA:0,g\n"
                    "DA:11,7\nDA:12,9\nDA:41,0\nend_of_record\n")
        cov = read_lcov(p)
        assert hits_at(cov, "crates/x.rs", 11) == (7, "line")
        assert hits_at(cov, "crates/x.rs", 5) == (7, "line")  # line 11 within lookahead
        assert hits_at(cov, "crates/x.rs", 30) == (0, "line")  # line 41 within lookahead
        assert hits_at(cov, "crates/x.rs", 100) == (0, "function")
        assert hits_at(cov, "crates/y.rs", 1)[0] is None
        locs = [{"rec": "REC-1", "location": "crates/x.rs:11", "status": "", "chosen": "see `items/a.md` §2"},
                {"rec": "REC-2", "location": "crates/x.rs:100", "status": "", "chosen": ""},
                {"rec": "REC-3", "location": "crates/x.rs:11", "status": "spec-settled REC-3->q", "chosen": ""}]
        r = rank(locs, {"a": cov}, {})
        assert [x["rec"] for x in r] == ["REC-1", "REC-2"] and r[0]["total"] == 7
        assert r[0]["specs"] == ["items/a.md §2"]
    print("selftest ok")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--run", action="append", default=[], help="NAME=FILE.lcov")
    ap.add_argument("--index", default=os.path.join(REPO, "docs/handoff/provisional-index.tsv"))
    ap.add_argument("--handoff", default=os.path.join(REPO, "docs/HANDOFF.md"))
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--md")
    ap.add_argument("--tsv")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not a.run:
        ap.error("at least one --run NAME=FILE.lcov")
    runs = {}
    for r in a.run:
        name, _, p = r.partition("=")
        runs[name] = read_lcov(p)
    ranked = rank(read_tsv(a.index), runs, handoff_entries(a.handoff))
    argv = sys.argv[1:] if argv is None else argv
    text = md(ranked, runs, a.top, [os.path.relpath(x, REPO) if os.path.isabs(x) else x for x in argv])
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(text)
    else:
        sys.stdout.write(text)
    if a.tsv:
        with open(a.tsv, "w", encoding="utf-8") as f:
            f.write("rec\ttotal\t" + "\t".join(runs) + "\tsites_hit\tsites_code\n")
            for r in ranked:
                f.write(f"{r['rec']}\t{r['total']}\t" + "\t".join(str(r["hits"].get(n, 0)) for n in runs)
                        + f"\t{r['sites_hit']}\t{r['sites_code']}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
