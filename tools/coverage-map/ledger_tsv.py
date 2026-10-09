#!/usr/bin/env python3
# Spec: specs/tools/coverage-map.md
"""Turns a `report.py --json` coverage report into fidelity-ledger part rows
(`#ledger 1`, docs/handoff/ledger/<part>.tsv): one `cov.<category>.<row>-<slug>`
row per exercised and per never-exercised table row.

    python3 tools/coverage-map/ledger_tsv.py REPORT.json OUT.tsv --scope TEXT \
        [--unrun-classes ama,nec,bar,dru,ass] [--unrun-acts 1,2]

`exercised` is `yes` (seen), `no` (a cell that covered the thing's act and
class finished and never reached it) or `?` (only a cell that did not run could
reach it: a level of an act in --unrun-acts, a skill of a class in
--unrun-classes). State is always UNKNOWN: coverage measures execution, the
owner part rates the check. Standard library only. Our own code.
"""
import argparse
import json
import re

HEADER = ("area kind group source_1.14d specs spec_status checks last_verdict exercised "
          "provisional needs_pc1 owner state size note").split()
SPEC = "specs/tools/coverage-map.md"


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", str(s).lower()).strip("-")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("report")
    ap.add_argument("out")
    ap.add_argument("--scope", required=True)
    ap.add_argument("--unrun-classes", default="")
    ap.add_argument("--unrun-acts", default="")
    a = ap.parse_args()
    s = json.load(open(a.report))["sections"]
    unrun_cls = set(filter(None, a.unrun_classes.split(",")))
    unrun_act = set(int(x) for x in filter(None, a.unrun_acts.split(",")))
    rows = []

    def add(cat, rid, name, src, ex, extra=""):
        note = (f"exercised {ex} time(s) in {a.scope}" if isinstance(ex, int)
                else f"exercised in {a.scope}" if ex == "yes"
                else f"not exercised in {a.scope}" if ex == "no"
                else f"? no finished cell reaches it ({a.scope})")
        if extra:
            note += f"; {extra}"
        note += "; state UNKNOWN: coverage measures execution, the owner part rates the check"
        e = "yes" if isinstance(ex, int) else ex
        rows.append([f"cov.{cat}.{rid}-{slug(name)}", "entity", "coverage",
                     f"{src} row {rid} ({name})", SPEC, "implemented", "-", "-", e,
                     "0", "n", "-", "UNKNOWN", "-", note])

    for r, name, n, _ in s["level"]["seen"]:
        add("level", r, name, "Levels.txt", n)
    for _, r, name, act in s["level"]["gaps"]:
        add("level", r, name, "Levels.txt", "?" if act in unrun_act else "no", f"act {act}")
    for r, name, n, _ in s["monster"]["seen"]:
        add("monster", r, name, "monstats.txt", n)
    for _, r, name, _n in s["monster"]["gaps"]:
        add("monster", r, name, "monstats.txt", "no")
    for _, name, n, _ in s["monster-ai"]["seen"]:
        rows.append([f"cov.monster-ai.{name.lower()}-{name.lower()}", "entity", "coverage",
                     f"monstats.txt AI column row {name} ({name})", SPEC, "implemented", "-", "-",
                     "yes", "0", "n", "-", "UNKNOWN", "-",
                     f"exercised {n} time(s) in {a.scope}; state UNKNOWN: coverage measures execution, the owner part rates the check"])
    for _, name in s["monster-ai"]["gaps"]:
        rows.append([f"cov.monster-ai.{name.lower()}-{name.lower()}", "entity", "coverage",
                     f"monstats.txt AI column row {name} ({name})", SPEC, "implemented", "-", "-",
                     "no", "0", "n", "-", "UNKNOWN", "-",
                     f"not exercised in {a.scope}; state UNKNOWN: coverage measures execution, the owner part rates the check"])
    for r, name, n, _ in s["skill"]["seen"]:
        add("skill", r, name, "skills.txt", n)
    for _, r, name, cls in s["skill"]["gaps"]:
        add("skill", r, name, "skills.txt", "?" if cls in unrun_cls else "no", cls)
    for r, name, n, _ in s["missile"]["seen"]:
        add("missile", r, name, "missiles.txt", n)
    for _, r, name in s["missile"]["gaps"]:
        add("missile", r, name, "missiles.txt", "no")
    for r, name, n, _ in s["state"]["seen"]:
        add("state", r, name, "states.txt", n)
    for _, r, name in s["state"]["gaps"]:
        add("state", r, name, "states.txt", "no")
    for r, name, n, _ in s["object"]["seen"]:
        add("object", r, name, "objects.txt", n)
    for _, r, name, _n in s["object"]["gaps"]:
        add("object", r, name, "objects.txt", "no")
    for r, name, n, bits in s["quest"]["seen"]:
        add("quest", r, name, "quest slot (specs/world/quests.md 1.9)", n, f"bits {bits}")
    for r, name in s["quest"]["gaps"]:
        add("quest", r, name, "quest slot (specs/world/quests.md 1.9)", "no")
    for _, r, name in s["npc-topic"]["gaps"]:
        add("npc-topic", r, name, "monstats.txt (npc=1 interact=1)", "no")
    it = s["item"]
    counts = dict(it["top"])
    for r, name in sorted(it["names"].items(), key=lambda kv: int(kv[0])):
        add("item", r, name, "armor/weapons/misc.txt", counts.get(int(r), "yes"))
    for _, r, name, code in it["gaps"]:
        add("item", r, name, "armor/weapons/misc.txt", "no", f"code {code}")
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write("#ledger 1\n" + "\t".join(HEADER) + "\n")
        for row in rows:
            f.write("\t".join(row) + "\n")
    print(f"{len(rows)} rows -> {a.out}")


if __name__ == "__main__":
    main()
