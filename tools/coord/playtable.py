#!/usr/bin/env python3
"""Playability table: runs the playthrough harness on a head and writes
docs/handoff/playability.md (per act: reached / total, consecutive, first
blocker; the head commit and time; milestones gained and lost against the
previous table; a short history).

    python3 tools/coord/playtable.py                    # current checkout (builds d2-client, d2s-tool)
    python3 tools/coord/playtable.py --head origin/claude/specs-staging-7
                                                        # that commit, in a worktree under target/
    python3 tools/coord/playtable.py --from-json r.json # write the table from an existing
                                                        # `playthrough.py --all --json` result
    python3 tools/coord/playtable.py --selftest

The previous table's per-milestone results are kept in the file itself (a
`<!-- playtable-data ... -->` comment), so the change is computed from the
committed file. Exit 0 when written, 1 when a milestone was lost against the
previous table, 2 on error. Needs D2_GAME_DIR (unless --from-json).
Python stdlib only.
"""

import argparse
import datetime
import json
import os
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(REPO, "docs", "handoff", "playability.md")
DATA_TAG = "<!-- playtable-data "
HISTORY_MAX = 30


def git(*args, cwd=REPO):
    r = subprocess.run(["git"] + list(args), cwd=cwd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {r.stderr.strip()}")
    return r.stdout.strip()


def run_playthrough(head):
    """Runs `playthrough.py --all --json` on `head` (None: the current
    checkout). Another commit is checked out in a worktree under
    target/coord-wt whose target/ links to this repo's, so the release build is
    shared."""
    root = REPO
    if head:
        root = os.path.join(REPO, "target", "coord-wt")
        if os.path.isdir(root):
            git("worktree", "remove", "--force", root)
        git("worktree", "add", "--detach", "--force", root, head)
        if not os.path.exists(os.path.join(root, "target")):
            os.symlink(os.path.join(REPO, "target"), os.path.join(root, "target"))
    fd, js = tempfile.mkstemp(prefix="playtable-", suffix=".json")
    os.close(fd)
    cmd = [sys.executable, os.path.join(root, "tools", "playthrough", "playthrough.py"), "--all", "--build",
           "--json", js]
    r = subprocess.run(cmd, cwd=root)
    if r.returncode == 3 or not os.path.getsize(js):
        raise RuntimeError(f"playthrough failed (exit {r.returncode})")
    with open(js, encoding="utf-8") as f:
        res = json.load(f)
    sha = git("rev-parse", "HEAD", cwd=root)
    subj = git("log", "-1", "--format=%s", cwd=root)
    if head:
        git("worktree", "remove", "--force", root)
    return res, sha, subj


def compact(res):
    """playthrough-result-1 -> {act: {"reached", "total", "consecutive",
    "first_blocker", "milestones": {name: status}}} (keys as strings)."""
    out = {}
    for a in res["acts"]:
        fb = a.get("first_blocker")
        out[row_key(a)] = {
            "reached": a["reached"], "total": a["total"], "consecutive": a["consecutive"],
            "first_blocker": None if not fb else f"{fb['milestone']} ({fb['kind']}) f{fb['frame']}: {fb['evidence']}",
            "milestones": {m["name"]: m["status"] for m in a.get("milestones", [])},
        }
    return out


def read_previous(path):
    """Returns (data, history) from an existing playability.md, or ({}, [])."""
    if not os.path.exists(path):
        return {}, []
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith(DATA_TAG):
                d = json.loads(line[len(DATA_TAG):].rsplit("-->", 1)[0])
                return d.get("prev", {}), d.get("history", [])
    return {}, []


def diff(prev, cur):
    """Milestones gained / lost per act: [(act, name, old, new)]."""
    gained, lost = [], []
    for act in sorted(set(prev) | set(cur), key=act_order):
        pm = prev.get(act, {}).get("milestones", {})
        cm = cur.get(act, {}).get("milestones", {})
        for name in sorted(set(pm) | set(cm)):
            o, n = pm.get(name), cm.get(name)
            if o != "reached" and n == "reached":
                gained.append((act, name, o or "new", n))
            elif o == "reached" and n != "reached":
                lost.append((act, name, o, n or "removed"))
    return gained, lost


def row_key(a):
    """The row of one objective file: "N" for `actN.play`, else "N/<stem>"
    (several files can share an act: classes.play, act4-forge.play, ...)."""
    stem = os.path.basename(a.get("play") or "").removesuffix(".play")
    return str(a["act"]) if stem in ("", f"act{a['act']}") else f"{a['act']}/{stem}"


def act_order(k):
    act, _, stem = k.partition("/")
    return (int(act) if act.isdigit() else 99, stem)


def cell(s):
    s = (s or "none").replace("|", "\\|").replace("\n", " ")
    return s if len(s) <= 200 else s[:197] + "..."


def render(cur, prev, history, sha, subj, when):
    gained, lost = diff(prev, cur)
    tot = sum(a["reached"] for a in cur.values()), sum(a["total"] for a in cur.values())
    acts = sorted(cur, key=act_order)
    history = ([{"head": sha[:8], "time": when, "reached": tot[0], "total": tot[1],
                 "per_act": {a: cur[a]["reached"] for a in acts}, "gained": len(gained), "lost": len(lost)}]
               + history)[:HISTORY_MAX]
    L = ["# Playability", "",
         "Generated by `python3 tools/coord/playtable.py` from `tools/playthrough/playthrough.py --all --json`",
         "(spec `specs/tools/playthrough.md`); do not edit by hand. The coordinator reruns it after every",
         "staging push.", "",
         f"- head: `{sha[:8]}` {cell(subj)}",
         f"- time: {when}",
         f"- total: {tot[0]}/{tot[1]} milestones reached", "",
         "| act | reached | consecutive | first blocker |", "|---|---|---|---|"]
    for a in acts:
        c = cur[a]
        L.append(f"| {a} | {c['reached']}/{c['total']} | {c['consecutive']}/{c['total']} | {cell(c['first_blocker'])} |")
    L += ["", "## Change versus the previous table", ""]
    if not prev:
        L.append("No previous table.")
    elif not gained and not lost:
        L.append(f"No milestone gained or lost since `{history[1]['head'] if len(history) > 1 else '?'}`.")
    else:
        L.append(f"Since `{history[1]['head'] if len(history) > 1 else '?'}`:")
        L.append("")
        for act, name, o, n in gained:
            L.append(f"- gained: act {act} `{name}` ({o} -> {n})")
        for act, name, o, n in lost:
            L.append(f"- **lost**: act {act} `{name}` ({o} -> {n})")
    L += ["", "## History", "", "| head | time | total | " + " | ".join(f"act {a}" for a in acts) + " | +/- |",
          "|---|---|---|" + "---|" * len(acts) + "---|"]
    for h in history:
        L.append(f"| `{h['head']}` | {h['time']} | {h['reached']}/{h['total']} | "
                 + " | ".join(str(h["per_act"].get(a, "-")) for a in acts) + f" | +{h['gained']}/-{h['lost']} |")
    L += ["", DATA_TAG + json.dumps({"format": "playtable 1", "prev": cur, "history": history},
                                    separators=(",", ":")) + " -->", ""]
    return "\n".join(L), gained, lost


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--head", help="commit to run (default: the current checkout)")
    ap.add_argument("--from-json", help="an existing playthrough --all --json result (then --sha names its head)")
    ap.add_argument("--sha", help="with --from-json: the head it was run on (default HEAD)")
    ap.add_argument("--out", default=OUT)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    try:
        if a.from_json:
            with open(a.from_json, encoding="utf-8") as f:
                res = json.load(f)
            sha = git("rev-parse", a.sha or "HEAD")
            subj = git("log", "-1", "--format=%s", sha)
        else:
            if not os.environ.get("D2_GAME_DIR"):
                raise RuntimeError("D2_GAME_DIR is not set")
            res, sha, subj = run_playthrough(a.head)
    except (RuntimeError, OSError, ValueError) as e:
        print(f"playtable: error: {e}", file=sys.stderr)
        return 2
    prev, history = read_previous(a.out)
    when = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    text, gained, lost = render(compact(res), prev, history, sha, subj, when)
    with open(a.out, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"playtable: wrote {os.path.relpath(a.out, REPO)} for {sha[:8]}: +{len(gained)} / -{len(lost)} milestones")
    for act, name, o, n in lost:
        print(f"  LOST act {act} {name}: {o} -> {n}")
    return 1 if lost else 0


def selftest():
    def res(stats):
        return {"format": "playthrough-result-1", "acts": [
            {"act": act, "reached": sum(s == "reached" for s in ms.values()), "total": len(ms), "consecutive": 0,
             "first_blocker": next(({"milestone": k, "kind": v, "frame": 9, "evidence": "e|x"}
                                    for k, v in ms.items() if v != "reached"), None),
             "milestones": [{"name": k, "status": v} for k, v in ms.items()]} for act, ms in stats.items()]}
    d = tempfile.mkdtemp(prefix="playtable-selftest-")
    out = os.path.join(d, "p.md")
    t1 = render(compact(res({1: {"a": "reached", "b": "stuck"}, 2: {"c": "reached"}})), {}, [], "1" * 40, "s", "t")[0]
    open(out, "w").write(t1)
    prev, hist = read_previous(out)
    assert prev["1"]["milestones"] == {"a": "reached", "b": "stuck"} and len(hist) == 1
    cur = compact(res({1: {"a": "stuck", "b": "reached", "n": "reached"}, 2: {"c": "reached"}}))
    t2, g, l = render(cur, prev, hist, "2" * 40, "s", "t")
    assert g == [("1", "b", "stuck", "reached"), ("1", "n", "new", "reached")], g
    assert l == [("1", "a", "reached", "stuck")], l
    assert "**lost**: act 1 `a`" in t2 and "since `11111111`" in t2.lower() and "e\\|x" in t2
    open(out, "w").write(t2)
    assert len(read_previous(out)[1]) == 2
    # two objective files of one act are two rows, neither hides the other
    two = res({1: {"a": "reached"}})
    two["acts"][0]["play"] = "traces/playthrough/act1.play"
    two["acts"].append(dict(res({1: {"k": "stuck"}})["acts"][0], play="traces/playthrough/classes.play"))
    rows = compact(two)
    assert sorted(rows) == ["1", "1/classes"], rows
    t3 = render(rows, {}, [], "3" * 40, "s", "t")[0]
    assert "| 1 | 1/1 |" in t3 and "| 1/classes | 0/1 |" in t3 and "total: 1/2" in t3, t3
    print("playtable selftest: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
