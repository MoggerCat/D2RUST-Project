#!/usr/bin/env python3
"""Fidelity ledger: validate the part files, merge them, render the ledger.

    python3 tools/coord/ledger.py --check            # validate parts + merged output up to date
    python3 tools/coord/ledger.py                    # merge -> fidelity-ledger.tsv + .md
    python3 tools/coord/ledger.py --parts DIR --out-tsv F --out-md F --status F
    python3 tools/coord/ledger.py --selftest

Parts: docs/handoff/ledger/<part>.tsv (one per ledger session), format
`ledger 1`: line 1 `#ledger 1`, line 2 the header (COLS below), tab-separated.
Cells: kind entity|system|message|ui|content; last_verdict MATCH|PARTIAL|
DIVERGED@frame|-; exercised yes|no|?; provisional a count; needs_pc1 y|n;
state EQUAL|DIVERGED|NO-CHECK|NOT-IMPLEMENTED|UNKNOWN; size S (<2 session-
hours) | M (2-8) | L (>8), `-` exactly for EQUAL. Fixed by the coordinator
(docs/handoff/q-ledger-monsters-task.md on claude/q-ledger-monsters).
Merge: rows of the coverage parts (group `coverage`) whose area is also in an
entity part only set that row's `exercised` (yes wins over no over ?); the
other coverage rows are added as rows. A duplicate area between entity parts
keeps the first (part files in name order) and is listed as a conflict.
Completeness: every spec file under specs/, every traces/checks/*.check and
every message id of specs/sim/client-messages.tsv / server-messages.tsv must
be named by some row (specs / checks columns; area net.c2s.0xNN /
net.s2c.0xNN); what is not is listed under "Not covered by any row".
Verdicts: when docs/handoff/checks-status.md exists (else the copy on
origin/claude/q-fix-check-triage) each row's last_verdict is recomputed from
its checks; a row whose state disagrees with its checks is listed.
Python 3 stdlib only. Our own code.
"""

import argparse
import collections
import glob
import json
import os
import re
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
VERSION = "1.0.0"
PART_MAGIC = "#ledger 1"
OUT_MAGIC = "#fidelity-ledger 1"
COLS = ["area", "kind", "group", "source_1.14d", "specs", "spec_status", "checks",
        "last_verdict", "exercised", "provisional", "needs_pc1", "owner", "state",
        "size", "note"]
KINDS = {"entity", "system", "message", "ui", "content"}
STATES = ["DIVERGED", "NOT-IMPLEMENTED", "NO-CHECK", "UNKNOWN", "EQUAL"]
SIZES = {"S", "M", "L", "-"}
EXERCISED = {"yes", "no", "?"}
PC1 = {"y", "n"}
AREA_RE = re.compile(r"^[a-z0-9][a-z0-9._+-]*$")
VERDICT_RE = re.compile(r"^(MATCH|PARTIAL|DIVERGED(@\d+)?|-)$")
STATUS_BRANCH = "origin/claude/q-fix-check-triage"
STATUS_PATH = "docs/handoff/checks-status.md"
# Session-hours per size (S < 2, M 2-8, L > 8): bounds for the totals.
SIZE_HOURS = {"S": (0.5, 2), "M": (2, 8), "L": (8, None)}


class Repo:
    """What a row may reference, read from a repository root."""

    def __init__(self, root):
        self.root = root
        self.specs = sorted(
            os.path.relpath(p, root).replace(os.sep, "/")
            for p in glob.glob(os.path.join(root, "specs", "**", "*"), recursive=True)
            if os.path.isfile(p) and p.endswith((".md", ".tsv"))
            and os.path.basename(p) not in ("README.md", "_TEMPLATE.md")
            # specs/tools/ describe our own measuring tools, not 1.14d behaviour
            and not os.path.relpath(p, root).replace(os.sep, "/").startswith("specs/tools/"))
        self.checks = sorted(os.path.basename(p)[:-6] for p in
                             glob.glob(os.path.join(root, "traces", "checks", "*.check")))
        self.messages = []
        for d, f in (("c2s", "client-messages.tsv"), ("s2c", "server-messages.tsv")):
            p = os.path.join(root, "specs", "sim", f)
            if not os.path.exists(p):
                continue
            with open(p, encoding="utf-8") as fh:
                rows = fh.read().splitlines()[1:]
            for line in rows:
                mid = line.split("\t", 1)[0].strip()
                if re.match(r"^0x[0-9A-Fa-f]{2}$", mid):
                    self.messages.append(f"net.{d}.0x{mid[2:].lower()}")


# ---------------------------------------------------------------- parts

def split_list(cell):
    if cell in ("", "-"):
        return []
    return [x.strip() for x in re.split(r"[,;]", cell) if x.strip()]


def read_part(path):
    """Returns (rows, errors). rows: list of dicts with _file/_line."""
    errs = []
    rows = []
    name = os.path.basename(path)
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    if not lines or lines[0].rstrip("\r") != PART_MAGIC:
        return [], [f"{name}:1: line 1 must be exactly '{PART_MAGIC}'"]
    if len(lines) < 2 or lines[1].rstrip("\r").split("\t") != COLS:
        return [], [f"{name}:2: header must be: " + "\\t".join(COLS)]
    for i, line in enumerate(lines[2:], 3):
        line = line.rstrip("\r")
        if not line.strip() or line.startswith("#"):
            continue
        cells = line.split("\t")
        if len(cells) != len(COLS):
            errs.append(f"{name}:{i}: {len(cells)} fields, want {len(COLS)}")
            continue
        r = dict(zip(COLS, (c.strip() for c in cells)))
        r["_file"], r["_line"] = name, i
        rows.append(r)
    return rows, errs


def check_row(r, repo):
    """Format errors of one row (list of strings)."""
    at = f"{r['_file']}:{r['_line']}: {r['area'] or '(no area)'}"
    e = []
    if not AREA_RE.match(r["area"]):
        e.append(f"{at}: area must be lowercase dotted ([a-z0-9._+-])")
    if r["kind"] not in KINDS:
        e.append(f"{at}: kind '{r['kind']}' not in {sorted(KINDS)}")
    if not r["group"]:
        e.append(f"{at}: empty group")
    if not r["source_1.14d"]:
        e.append(f"{at}: empty source_1.14d")
    for s in split_list(r["specs"]):
        p = s.split("§")[0].split("#")[0].strip().rstrip(":")
        if p and not os.path.exists(os.path.join(repo.root, p)):
            e.append(f"{at}: spec '{p}' does not exist")
    for c in split_list(r["checks"]):
        if not any(fnmatch_name(c, k) for k in repo.checks):
            e.append(f"{at}: check '{c}' matches no traces/checks/*.check")
    if not VERDICT_RE.match(r["last_verdict"]):
        e.append(f"{at}: last_verdict '{r['last_verdict']}' not MATCH|PARTIAL|DIVERGED@N|-")
    if r["exercised"] not in EXERCISED:
        e.append(f"{at}: exercised '{r['exercised']}' not yes|no|?")
    if not re.match(r"^\d+$", r["provisional"]):
        e.append(f"{at}: provisional '{r['provisional']}' not a count")
    if r["needs_pc1"] not in PC1:
        e.append(f"{at}: needs_pc1 '{r['needs_pc1']}' not y|n")
    if r["state"] not in STATES:
        e.append(f"{at}: state '{r['state']}' not in {STATES}")
    if r["size"] not in SIZES:
        e.append(f"{at}: size '{r['size']}' not S|M|L|-")
    elif (r["size"] == "-") != (r["state"] == "EQUAL"):
        e.append(f"{at}: size must be '-' exactly when state is EQUAL")
    if r["state"] == "UNKNOWN" and not r["note"]:
        e.append(f"{at}: UNKNOWN needs the reason in note")
    return e


def fnmatch_name(pattern, name):
    import fnmatch
    return fnmatch.fnmatchcase(name, pattern.removesuffix(".check"))


# ---------------------------------------------------------------- verdicts

def load_status(root, path=None):
    """{check: [(channel, verdict, first_frame or None)]} and a source label."""
    text, label = None, None
    p = path or os.path.join(root, STATUS_PATH)
    if os.path.exists(p):
        with open(p, encoding="utf-8") as fh:
            text = fh.read()
        label = os.path.relpath(p, root).replace(os.sep, "/")
    elif path is None:
        try:
            text = subprocess.run(["git", "show", f"{STATUS_BRANCH}:{STATUS_PATH}"], cwd=root,
                                  capture_output=True, text=True, check=True).stdout
            label = f"{STATUS_BRANCH}:{STATUS_PATH}"
        except (OSError, subprocess.CalledProcessError):
            return {}, None
    if text is None:
        return {}, None
    st = collections.defaultdict(list)
    run = ""
    for line in text.splitlines():
        if line.startswith("Suite run"):
            run = line.split("`")[0].strip().rstrip(":")
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 5 or cells[2] not in ("MATCH", "PARTIAL", "DIVERGED"):
            continue
        frame = None
        if cells[2] == "DIVERGED":
            m = re.search(r"(?:frame|tick) (\d+)", cells[4])
            frame = int(m.group(1)) if m else None
        st[cells[0]].append((cells[1], cells[2], frame, cells[4]))
    return dict(st), (f"{label} ({run})" if run else label)


def verdict_of(checks, repo, status, area=""):
    """Worst verdict over the row's checks: DIVERGED@first > PARTIAL > MATCH; '-' if none ran.
    A message row (net.c2s.0xNN / net.s2c.0xNN) reads only the packets channel, and a
    packets divergence counts against it only when the first differing message is that
    id in that direction; one at another message leaves it compared up to there (PARTIAL)."""
    names = sorted({k for c in checks for k in repo.checks if fnmatch_name(c, k)})
    res = [v[:3] for n in names for v in status.get(n, [])]
    m = re.match(r"^net\.(c2s|s2c)\.0x([0-9a-f]{2})$", area)
    if m:
        res = []
        for n in names:
            for ch, v, f, first in status.get(n, []):
                if ch != "packets":
                    continue
                if v == "DIVERGED":
                    hit = re.search(r"stream (c2s|s2c) .*\(id 0x([0-9A-Fa-f]{2})\)", first)
                    if not (hit and hit.group(1) == m.group(1) and hit.group(2).lower() == m.group(2)):
                        v = "PARTIAL"
                res.append((ch, v, f))
    if not res:
        return "-"
    div = [f for _, v, f in res if v == "DIVERGED"]
    if div:
        frames = [f for f in div if f is not None]
        return f"DIVERGED@{min(frames)}" if frames else "DIVERGED"
    if any(v == "PARTIAL" for _, v, _ in res):
        return "PARTIAL"
    return "MATCH"


# ---------------------------------------------------------------- merge

def quest_slot(act, q):
    """Quest slot of act (1-5) quest q (world/quests.md §1.9): acts I-IV 8 per act, act V from 35."""
    return 34 + q if act == 5 else (act - 1) * 8 + q


def canon(area):
    """Alias key so a coverage row finds the entity row of the same thing under another id
    scheme: level.a1.5.x / level.5-x -> level#5; quest.a2q1-x / quest.slot9-x -> quest#9;
    monster-ai.x -> monster.ai.x; otherwise the family plus the alphanumerics of the rest."""
    m = re.match(r"^cov\.(level|quest)\.(\d+)$", area)
    if m:
        return f"{m.group(1)}#{int(m.group(2))}"
    m = re.match(r"^level\.(?:a\d\.)?(\d+)[.-]", area)
    if m:
        return f"level#{int(m.group(1))}"
    m = re.match(r"^quest\.a(\d)q(\d)\b", area)
    if m:
        return f"quest#{quest_slot(int(m.group(1)), int(m.group(2)))}"
    m = re.match(r"^quest\.slot(\d+)\b", area)
    if m:
        return f"quest#{int(m.group(1))}"
    if area.startswith("monster-ai."):
        area = "monster.ai." + area[len("monster-ai."):]
    fam, _, rest = area.partition(".")
    return fam + "#" + re.sub(r"[^a-z0-9.]", "", rest)


def reconcile(r, repo, status):
    """Bring a row in line with its checks; returns what was wrong (empty = consistent).
    last_verdict := the checks' verdict; a row naming a DIVERGED check is DIVERGED;
    UNKNOWN with PARTIAL checks is NO-CHECK; a non-EQUAL row has a size."""
    issues = []
    v = verdict_of(split_list(r["checks"]), repo, status, r["area"])
    if v != "-" and r["last_verdict"] != v:
        issues.append(f"last_verdict {r['last_verdict']} but checks say {v}")
        r["last_verdict"] = v
    v = r["last_verdict"]
    if v.startswith("DIVERGED") and r["state"] in ("NO-CHECK", "UNKNOWN", "EQUAL"):
        issues.append(f"state {r['state']} but checks say {v}")
        r["note"] += f" [ledger.py: {r['state']} -> DIVERGED from its checks]"
        r["state"] = "DIVERGED"
    elif v == "PARTIAL" and r["state"] == "UNKNOWN":
        issues.append(f"state UNKNOWN but checks say {v}")
        r["note"] += " [ledger.py: UNKNOWN -> NO-CHECK: its checks are PARTIAL]"
        r["state"] = "NO-CHECK"
    if r["state"] != "EQUAL" and r["size"] == "-":
        issues.append(f"state {r['state']} without a size")
        r["size"] = "M"
        r["note"] += " [ledger.py: size M assumed]"
    return issues


SRC_ID = re.compile(r"(?i)^(skills|levels|states|monstats)\.txt\b.*?\((\d+)\)")
# coverage-map categories whose counter, when 0 in every run, means "not instrumented"
# for these area families (the coverage parts write 'no' for every row then).
UNINSTRUMENTED = {"object": ("object", "shrine", "waypoint"), "npc-topic": ("npc",)}


def load_coverage(parts_dir):
    """Seen keys from the coverage parts' report JSON (tools/coverage-map/report.py
    --json, format coverage-report 1): ({('canon', key) | ('src', table, id)}, set of
    categories that read 0 in every report)."""
    seen, zero, any_report = set(), None, False
    for f in sorted(glob.glob(os.path.join(parts_dir, "*.json"))):
        try:
            with open(f, encoding="utf-8") as fh:
                j = json.load(fh)
        except (OSError, ValueError):
            continue
        if not isinstance(j, dict) or j.get("format") != "coverage-report 1":
            continue
        any_report = True
        s = j["sections"]
        here = set()
        for cat, sec in s.items():
            n = len(sec.get("seen", [])) if "seen" in sec else sec.get("seen_count", 0)
            if n == 0 and sec.get("table"):
                here.add(cat)
        zero = here if zero is None else zero & here
        for e in s.get("level", {}).get("seen", []):
            seen.add(("canon", f"level#{e[0]}"))
            seen.add(("src", "levels", e[0]))
        for e in s.get("quest", {}).get("seen", []):
            seen.add(("canon", f"quest#{e[0]}"))
        for e in s.get("monster-ai", {}).get("seen", []):
            seen.add(("canon", canon("monster.ai." + str(e[1]).lower())))
        for cat, table, fam in (("skill", "skills", None), ("state", "states", "state"),
                                ("monster", "monstats", "monster"), ("missile", None, "missile")):
            for e in s.get(cat, {}).get("seen", []):
                if table:
                    seen.add(("src", table, e[0]))
                if fam:
                    seen.add(("canon", canon(f"{fam}.{str(e[1]).lower()}")))
    return seen, (zero or set()) if any_report else set()


def apply_seen(rows, seen, zero):
    """exercised = yes for rows a coverage report saw; '?' (not 'no') for rows of a
    category whose counter read 0 everywhere. Returns (yes count, downgraded count)."""
    yes = down = 0
    fams = {f for c in zero for f in UNINSTRUMENTED.get(c, ())}
    for r in rows:
        m = SRC_ID.match(r["source_1.14d"])
        hit = ("canon", canon(r["area"])) in seen or (
            m and ("src", m.group(1).lower(), int(m.group(2))) in seen)
        if hit and r["exercised"] != "yes":
            r["exercised"] = "yes"
            yes += 1
        elif r["exercised"] == "no" and r["area"].split(".")[0] in fams:
            r["exercised"] = "?"
            r["note"] += " [ledger.py: its coverage counter read 0 in every run: may be uninstrumented]"
            down += 1
    return yes, down


def merge(parts, repo, status, coverage=(set(), set())):
    """parts: [(name, rows)] in name order. Returns (rows, notes dict)."""
    rank = {"yes": 2, "no": 1, "?": 0}
    out, by_area = [], {}
    conflicts, cov_rows = [], []
    for name, rows in parts:
        for r in rows:
            if r["group"] == "coverage":
                cov_rows.append(r)
                continue
            if r["area"] in by_area:
                first = by_area[r["area"]]
                conflicts.append(f"`{r['area']}`: {first['_file']}:{first['_line']} kept, "
                                 f"{r['_file']}:{r['_line']} dropped")
                continue
            by_area[r["area"]] = r
            out.append(r)
    cov_applied = 0
    by_canon, by_src = {}, {}
    for r in out:
        by_canon.setdefault(canon(r["area"]), r)
        m = SRC_ID.match(r["source_1.14d"])
        if m:
            by_src.setdefault((m.group(1).lower(), int(m.group(2))), r)
    for r in cov_rows:
        tgt = by_area.get(r["area"]) or by_canon.get(canon(r["area"]))
        m = re.match(r"^cov\.(skill|state|monster)\.(\d+)$", r["area"])
        if tgt is None and m:
            tgt = by_src.get(({"skill": "skills", "state": "states", "monster": "monstats"}[m.group(1)],
                              int(m.group(2))))
        if tgt is None:
            by_area[r["area"]] = r
            out.append(r)
        elif tgt["group"] == "coverage":
            if rank[r["exercised"]] > rank[tgt["exercised"]]:
                tgt["exercised"] = r["exercised"]
        else:
            if rank[r["exercised"]] > rank[tgt["exercised"]]:
                tgt["exercised"] = r["exercised"]
            cov_applied += 1
    seen_yes, seen_down = apply_seen(out, *coverage)
    disagree = []
    if status:
        for r in out:
            reconcile(r, repo, status)
            v = r["last_verdict"]
            if r["state"] == "EQUAL" and v != "MATCH":
                disagree.append(f"`{r['area']}`: EQUAL but checks say {v}")
    named_specs, named_checks = set(), set()
    for r in out:
        for s in split_list(r["specs"]):
            named_specs.add(s.split("§")[0].split("#")[0].strip().rstrip(":"))
        for c in split_list(r["checks"]):
            named_checks.update(k for k in repo.checks if fnmatch_name(c, k))
    areas = {r["area"] for r in out}
    notes = {
        "conflicts": conflicts,
        "coverage_applied": cov_applied,
        "seen_yes": seen_yes,
        "seen_down": seen_down,
        "uninstrumented": sorted(coverage[1]),
        "disagree": disagree,
        "specs_uncovered": [s for s in repo.specs if s not in named_specs],
        "checks_uncovered": [c for c in repo.checks if c not in named_checks],
        "messages_uncovered": [m for m in repo.messages if m not in areas],
    }
    out.sort(key=lambda r: (r["group"], r["area"]))
    return out, notes


TWO_PART = ("system", "net", "skill", "item")


def family(area):
    p = area.split(".")
    return ".".join(p[:2]) if p[0] in TWO_PART and len(p) > 1 else p[0]


def esc_md(s):
    return s.replace("|", "\\|")


def render_tsv(rows, inputs, status_label):
    lines = [OUT_MAGIC,
             f"# generated by tools/coord/ledger.py {VERSION} from {', '.join(inputs) or '(no parts)'}; "
             f"verdicts: {status_label or 'as written in the parts'}. Do not edit: edit the part files.",
             "\t".join(COLS)]
    for r in rows:
        lines.append("\t".join(r[c] for c in COLS))
    return "\n".join(lines) + "\n"


def hours(counts):
    lo = sum(SIZE_HOURS[s][0] * n for s, n in counts.items() if s in SIZE_HOURS)
    hi = sum((SIZE_HOURS[s][1] or 0) * n for s, n in counts.items() if s in SIZE_HOURS)
    open_ended = counts.get("L", 0) > 0
    return f"{lo:g}–{hi:g}{'+' if open_ended else ''}"


def render_md(rows, notes, inputs, status_label):
    w = []
    a = w.append
    a("# Fidelity ledger")
    a("")
    a(f"Generated by `python3 tools/coord/ledger.py` {VERSION} from the part files "
      f"{', '.join('`' + i + '`' for i in inputs) or '(none)'}; do not edit, edit the parts "
      f"and rerun. Verdicts: {status_label or 'as written in the parts'}. Machine-readable: "
      "`docs/handoff/fidelity-ledger.tsv`. Gaps and plan: `docs/handoff/fidelity-gaps.md`.")
    a("")
    a("States: EQUAL = a passing 1.14d check; DIVERGED = a check shows a difference; "
      "NO-CHECK = implemented, nothing compares it with 1.14d; NOT-IMPLEMENTED; UNKNOWN = "
      "not yet assessed (reason in the note). Size of the work left to reach EQUAL, in "
      "session-hours: S < 2, M 2–8, L > 8 (hour totals below count S as 0.5–2, M 2–8, "
      "L 8 and up, so the upper bound is open when an L is left).")
    a("")
    groups = sorted({r["group"] for r in rows})
    a("## Summary")
    a("")
    a("| Group | Rows | " + " | ".join(STATES) + " | S | M | L | Session-hours left | Needs PC 1 | Exercised yes / no / ? |")
    a("|---|---" + "|---" * len(STATES) + "|---|---|---|---|---|---|")
    tot_st, tot_sz, tot_pc, tot_ex = collections.Counter(), collections.Counter(), 0, collections.Counter()
    for g in groups + ["**all**"]:
        rs = rows if g == "**all**" else [r for r in rows if r["group"] == g]
        st = collections.Counter(r["state"] for r in rs)
        sz = collections.Counter(r["size"] for r in rs)
        ex = collections.Counter(r["exercised"] for r in rs)
        pc = sum(1 for r in rs if r["needs_pc1"] == "y")
        a(f"| {g} | {len(rs)} | " + " | ".join(str(st[s]) for s in STATES)
          + f" | {sz['S']} | {sz['M']} | {sz['L']} | {hours(sz)} | {pc} | "
          f"{ex['yes']} / {ex['no']} / {ex['?']} |")
    a("")
    a("## By family")
    a("")
    a("Family = the first part of the area id (two parts for `system.*`, `net.*`, `skill.*`, `item.*`).")
    a("")
    a("NO-CHECK rows whose checks ran PARTIAL are compared in part (every compared frame "
      "equal, some field not recorded on one side): column \"of which partly compared\".")
    a("")
    a("| Family | Rows | " + " | ".join(STATES) + " | of which partly compared | S | M | L | Needs PC 1 | Exercised no |")
    a("|---|---" + "|---" * len(STATES) + "|---|---|---|---|---|---|")
    fams = collections.defaultdict(list)
    for r in rows:
        fams[family(r["area"])].append(r)
    for f in sorted(fams):
        rs = fams[f]
        st = collections.Counter(r["state"] for r in rs)
        sz = collections.Counter(r["size"] for r in rs)
        part_cmp = sum(r["state"] == "NO-CHECK" and r["last_verdict"] in ("PARTIAL", "MATCH") for r in rs)
        a(f"| `{f}` | {len(rs)} | " + " | ".join(str(st[s]) for s in STATES)
          + f" | {part_cmp} | {sz['S']} | {sz['M']} | {sz['L']} | {sum(r['needs_pc1'] == 'y' for r in rs)} | "
          f"{sum(r['exercised'] == 'no' for r in rs)} |")
    a("")
    a("## Not covered by any row")
    a("")
    a("Every area of 1.14d must be a row; these names appear in no row yet (an empty list is the goal).")
    a("")
    for key, title in (("specs_uncovered", "Spec files named by no row"),
                       ("checks_uncovered", "Checks named by no row"),
                       ("messages_uncovered", "Message ids with no `net.*` row")):
        items = notes[key]
        a(f"- {title}: {len(items)}" + (": " + ", ".join(f"`{x}`" for x in items) if items else ""))
    a("")
    a("## Merge notes")
    a("")
    a(f"- Coverage rows applied to entity rows (exercised): {notes['coverage_applied']}")
    a(f"- Rows set exercised = yes from the coverage reports' seen lists: {notes['seen_yes']}")
    a(f"- Coverage categories that read 0 in every report (may be uninstrumented): "
      f"{', '.join(notes['uninstrumented']) or 'none'}; rows set from no to ?: {notes['seen_down']}")
    a(f"- Duplicate areas between parts: {len(notes['conflicts'])}")
    for c in notes["conflicts"]:
        a(f"  - {c}")
    a(f"- Rows whose state disagrees with their checks: {len(notes['disagree'])}")
    for c in notes["disagree"][:60]:
        a(f"  - {c}")
    if len(notes["disagree"]) > 60:
        a(f"  - … and {len(notes['disagree']) - 60} more (rerun with the tsv to list them)")
    a("")
    for g in groups:
        rs = [r for r in rows if r["group"] == g]
        a(f"## {g}")
        a("")
        a("| Area | Kind | State | Size | Verdict | Exercised | Prov. | PC 1 | Owner | Specs | Note |")
        a("|---|---|---|---|---|---|---|---|---|---|---|")
        order = {s: i for i, s in enumerate(STATES)}
        for r in sorted(rs, key=lambda r: (order.get(r["state"], 9), r["area"])):
            a(f"| `{r['area']}` | {r['kind']} | {r['state']} | {r['size']} | {r['last_verdict']} | "
              f"{r['exercised']} | {r['provisional']} | {r['needs_pc1']} | {esc_md(r['owner'])} | "
              f"{esc_md(r['specs'])} | {esc_md(r['note'])} |")
        a("")
    return "\n".join(w) + "\n"


# ---------------------------------------------------------------- driver

def run(root, parts_dir, out_tsv, out_md, status_path, check, fix=False):
    repo = Repo(root)
    status, label = load_status(root, status_path)
    files = sorted(glob.glob(os.path.join(parts_dir, "*.tsv")))
    errs, parts = [], []
    for f in files:
        rows, e = read_part(f)
        if fix and status and not e:
            changed = 0
            for r in rows:
                changed += bool(reconcile(r, repo, status))
            if changed:
                with open(f, "w", encoding="utf-8", newline="\n") as fh:
                    fh.write(PART_MAGIC + "\n" + "\t".join(COLS) + "\n"
                             + "".join("\t".join(r[c] for c in COLS) + "\n" for r in rows))
                print(f"ledger: --fix rewrote {changed} row(s) of {os.path.basename(f)}")
        errs += e
        seen = set()
        for r in rows:
            errs += check_row(r, repo)
            if status:
                for i in reconcile(dict(r), repo, status):
                    errs.append(f"{r['_file']}:{r['_line']}: {r['area']}: contradiction: {i} (--fix applies it)")
            if r["area"] in seen:
                errs.append(f"{r['_file']}:{r['_line']}: {r['area']}: duplicate area in this part")
            seen.add(r["area"])
        parts.append((os.path.basename(f), rows))
    rows, notes = merge(parts, repo, status, load_coverage(parts_dir))
    inputs = [os.path.relpath(f, root).replace(os.sep, "/") for f in files]
    tsv = render_tsv(rows, inputs, label)
    md = render_md(rows, notes, inputs, label)
    if check:
        for path, want in ((out_tsv, tsv), (out_md, md)):
            if not os.path.exists(path):
                continue
            with open(path, encoding="utf-8") as fh:
                if fh.read() != want:
                    errs.append(f"{os.path.relpath(path, root)}: stale; rerun tools/coord/ledger.py")
        for e in errs:
            print(e, file=sys.stderr)
        print(f"ledger: {len(files)} part(s), {len(rows)} row(s), {len(errs)} error(s)")
        return 1 if errs else 0
    for e in errs:
        print("warning: " + e, file=sys.stderr)
    for path, text in ((out_tsv, tsv), (out_md, md)):
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
    st = collections.Counter(r["state"] for r in rows)
    print(f"ledger: {len(rows)} rows from {len(files)} part(s) -> {os.path.relpath(out_tsv, root)}, "
          f"{os.path.relpath(out_md, root)}; " + ", ".join(f"{s} {st[s]}" for s in STATES)
          + f"; {len(errs)} format error(s)")
    return 0


def selftest():
    hdr = PART_MAGIC + "\n" + "\t".join(COLS) + "\n"

    def row(**kw):
        d = dict(zip(COLS, ["x", "entity", "g", "src", "-", "none", "-", "-", "?", "0", "n", "-",
                            "NO-CHECK", "S", "n"]))
        d.update(kw)
        return "\t".join(d[c] for c in COLS) + "\n"

    with tempfile.TemporaryDirectory() as root:
        os.makedirs(os.path.join(root, "specs", "sim"))
        os.makedirs(os.path.join(root, "traces", "checks"))
        os.makedirs(os.path.join(root, "parts"))
        for f in ("a.md", "b.md"):
            open(os.path.join(root, "specs", "sim", f), "w").close()
        with open(os.path.join(root, "specs", "sim", "client-messages.tsv"), "w") as fh:
            fh.write("id\tname\n0x01\tWalk\n0x02\tRun\n")
        for c in ("c-one", "c-two", "orphan"):
            open(os.path.join(root, "traces", "checks", c + ".check"), "w").close()
        status = os.path.join(root, "status.md")
        with open(status, "w") as fh:
            fh.write("Suite run 2026-01-01 00:00 UTC at abc: `x`.\n"
                     "| c-one | state | DIVERGED | 4/10 | frame 5 game, field seed | o |\n"
                     "| c-one | rng | DIVERGED | 1/3 | frame 2, unit | o |\n"
                     "| c-one | packets | DIVERGED | 0/3 | frame 1 stream c2s #0 bytes[18]: 0 vs 4 (id 0x02) | o |\n"
                     "| c-two | packets | MATCH | 10/10 | - | - |\n")
        with open(os.path.join(root, "parts", "a.tsv"), "w") as fh:
            fh.write(hdr + row(area="m.one", specs="specs/sim/a.md §2", checks="c-one", state="EQUAL", size="-")
                     + row(area="net.c2s.0x01", kind="message", checks="c-*", state="EQUAL", size="-")
                     + row(area="net.c2s.0x02", kind="message", checks="c-one", state="DIVERGED", size="S")
                     + row(area="level.a1.5.act-1-wilderness-4"))
        with open(os.path.join(root, "parts", "b.tsv"), "w") as fh:
            fh.write(hdr + row(area="m.one", group="h") + row(area="m.bad", state="WRONG", size="Q"))
        with open(os.path.join(root, "parts", "c.tsv"), "w") as fh:
            fh.write(hdr + row(area="m.one", group="coverage", exercised="yes")
                     + row(area="level.5-dark-wood", group="coverage", exercised="no")
                     + row(area="never.seen", group="coverage", exercised="no"))
        with open(os.path.join(root, "parts", "c.json"), "w") as fh:
            json.dump({"format": "coverage-report 1", "sections": {
                "level": {"seen": [[2, "Blood Moor", 9, ["r"]]], "table": 3},
                "object": {"seen": [], "table": 4}}}, fh)
        with open(os.path.join(root, "parts", "d.tsv"), "w") as fh:
            fh.write(hdr + row(area="level.a1.2.blood", group="d", exercised="no")
                     + row(area="object.1-casket", group="coverage", exercised="no"))
        out_tsv, out_md = os.path.join(root, "o.tsv"), os.path.join(root, "o.md")
        import contextlib
        import io
        with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
            assert run(root, os.path.join(root, "parts"), out_tsv, out_md, status, True) == 1
            assert run(root, os.path.join(root, "parts"), out_tsv, out_md, status, False) == 0
        text = open(out_tsv).read().splitlines()
        assert text[0] == OUT_MAGIC and text[2].split("\t") == COLS
        rows = {ln.split("\t")[0]: dict(zip(COLS, ln.split("\t"))) for ln in text[3:]}
        assert set(rows) == {"m.one", "net.c2s.0x01", "net.c2s.0x02", "m.bad", "never.seen",
                             "level.a1.5.act-1-wilderness-4", "level.a1.2.blood", "object.1-casket"}
        assert rows["level.a1.5.act-1-wilderness-4"]["exercised"] == "no"
        assert rows["level.a1.2.blood"]["exercised"] == "yes", rows["level.a1.2.blood"]
        assert rows["object.1-casket"]["exercised"] == "?"
        assert canon("quest.a5q1-siege") == canon("quest.slot35-siege-on-harrogath")
        assert canon("quest.a2q1-radament") == canon("quest.slot9-radament-s-lair")
        assert canon("monster-ai.foulcrownest") == canon("monster.ai.foulcrownest")
        assert canon("missile.fire-arrow") == canon("missile.firearrow")
        assert canon("cov.level.5") == canon("level.5-dark-wood"), rows
        assert rows["m.one"]["exercised"] == "yes" and rows["m.one"]["last_verdict"] == "DIVERGED@1"
        assert rows["net.c2s.0x01"]["last_verdict"] == "PARTIAL", rows["net.c2s.0x01"]
        assert rows["net.c2s.0x02"]["last_verdict"] == "DIVERGED@1"
        md = open(out_md).read()
        assert rows["m.one"]["state"] == "DIVERGED"
        assert "`m.one`: a.tsv:3 kept, b.tsv:3 dropped" in md
        assert "`specs/sim/b.md`" in md and "`orphan`" in md and "`net.c2s.0x02`" not in md.split("no `net.*` row")[1].split("\n")[0]
        assert "`specs/sim/a.md`" not in md.split("Spec files named by no row")[1].split("\n")[0]
        # bad row reported; fixed parts pass --check once the outputs are current
        _, e = read_part(os.path.join(root, "parts", "b.tsv"))
        bad = [x for r in read_part(os.path.join(root, "parts", "b.tsv"))[0] for x in check_row(r, Repo(root))]
        assert any("state 'WRONG'" in x for x in bad) and any("size 'Q'" in x for x in bad), bad
        with open(os.path.join(root, "parts", "b.tsv"), "w") as fh:
            fh.write(hdr + row(area="m.two", checks="nope"))
        bad = [x for r in read_part(os.path.join(root, "parts", "b.tsv"))[0] for x in check_row(r, Repo(root))]
        assert any("check 'nope'" in x for x in bad), bad
        with open(os.path.join(root, "parts", "b.tsv"), "w") as fh:
            fh.write(hdr + row(area="m.two") + row(area="m.three", checks="c-one", state="UNKNOWN", note="n/a"))
        with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
            assert run(root, os.path.join(root, "parts"), out_tsv, out_md, status, True) == 1
            run(root, os.path.join(root, "parts"), out_tsv, out_md, status, False, fix=True)
            assert run(root, os.path.join(root, "parts"), out_tsv, out_md, status, True) == 0
        three = [ln for ln in open(out_tsv).read().splitlines() if ln.startswith("m.three\t")][0].split("\t")
        assert three[COLS.index("state")] == "DIVERGED", three
        with open(os.path.join(root, "parts", "z.tsv"), "w") as fh:
            fh.write("#ledger 2\n")
        assert read_part(os.path.join(root, "parts", "z.tsv"))[1]
        assert hours({"S": 2, "M": 1, "L": 1}) == "11–12+"
    print("ledger selftest: ok")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="validate parts and that the outputs are current")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--fix", action="store_true",
                    help="rewrite the part files: last_verdict from the checks, states that contradict them, missing sizes")
    ap.add_argument("--parts", default=os.path.join(REPO, "docs", "handoff", "ledger"))
    ap.add_argument("--out-tsv", default=os.path.join(REPO, "docs", "handoff", "fidelity-ledger.tsv"))
    ap.add_argument("--out-md", default=os.path.join(REPO, "docs", "handoff", "fidelity-ledger.md"))
    ap.add_argument("--status", help=f"checks-status.md to read (default {STATUS_PATH}, else {STATUS_BRANCH})")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    return run(REPO, a.parts, a.out_tsv, a.out_md, a.status, a.check, a.fix)


if __name__ == "__main__":
    sys.exit(main())
