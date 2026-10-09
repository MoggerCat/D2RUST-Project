#!/usr/bin/env python3
# Spec: specs/tools/checkpoints.md
"""Checkpoint saves: one .d2s per playthrough milestone, built with
d2s-tool from a definition in traces/checkpoints/<name>.checkpoint, so a
session starts right at its blocker instead of walking there.

    python3 tools/checkpoints/make.py                      # every checkpoint -> target/checkpoints/
    python3 tools/checkpoints/make.py a1-andariel --verify # build, then load in d2rs (state-dump)
    python3 tools/checkpoints/make.py --list
    python3 tools/checkpoints/make.py --selftest

Saves are generated, never committed (CLAUDE.md rule 1; *.d2s is
gitignored and the default output is under target/). `--verify` runs
`d2-client state-dump` on each save, with and without the checkpoint's
`start` pokes, and writes the d2rs start state (`--record FILE` writes
the `checkpoint-start-1` table, spec §4). Exit 0 ok, 1 a check failed,
3 error. Python stdlib only.
"""

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import tempfile

VERSION = "0.1.0"
GOTO_TICKS = 420  # ticks after a `goto` start poke: its step limit (poke.md §6 r3.4) and margin
FORMAT = "checkpoint 1"
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DEFS = os.path.join(REPO, "traces", "checkpoints")
CLASSES = ("ama", "sor", "nec", "pal", "bar", "dru", "ass")
DIFFS = ("normal", "nightmare", "hell")
# keyword -> (min args, max args or None, repeatable)
KEYS = {
    "name": (1, 1, False),
    "title": (1, None, False),
    "class": (1, 1, False),
    "level": (1, 1, False),
    "expansion": (0, 0, False),
    "hardcore": (0, 0, False),
    "difficulty": (1, 1, False),
    "act": (1, 1, False),
    "stat": (2, 2, True),
    "skill": (2, 2, True),
    "left-skill": (1, 1, False),
    "right-skill": (1, 1, False),
    "gold": (1, 1, False),
    "quests": (1, None, True),
    "waypoints": (1, None, True),
    "item": (1, 1, True),
    "start": (2, None, True),
    "need": (1, None, True),
    "note": (1, None, True),
}
REQUIRED = ("name", "title", "class", "level", "difficulty", "act")
QUEST_RE = re.compile(r"^(?:(?:normal|nightmare|hell):)?(?:acts=[0-4]|\d{1,2}\.\d{1,2})$")
WP_RE = re.compile(r"^(?:(?:normal|nightmare|hell):)?(?:lv=\d+|\d+)$")
NAME_RE = re.compile(r"^[A-Za-z]{2,15}$")
FILE_RE = re.compile(r"^[a-z0-9-]+$")


class CkError(Exception):
    """A definition or tool error (exit 3)."""


def _int(s, where, lo=None, hi=None):
    try:
        v = int(s, 0)
    except ValueError:
        raise CkError(f"{where}: not a number: '{s}'") from None
    if (lo is not None and v < lo) or (hi is not None and v > hi):
        raise CkError(f"{where}: {v} out of range {lo}..{hi}")
    return v


def parse(text, fname):
    """Parses a `checkpoint 1` definition (spec §2) into a dict."""
    ck = {"stat": [], "skill": [], "quests": [], "waypoints": [], "item": [], "start": [],
          "need": [], "note": [], "expansion": False, "hardcore": False}
    seen_format = False
    seen = set()
    for no, raw in enumerate(text.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        where = f"{fname}:{no}"
        if not seen_format:
            if line != FORMAT:
                raise CkError(f"{where}: first line must be '{FORMAT}'")
            seen_format = True
            continue
        word, *args = line.split()
        if word not in KEYS:
            raise CkError(f"{where}: unknown keyword '{word}'")
        lo, hi, rep = KEYS[word]
        if len(args) < lo or (hi is not None and len(args) > hi):
            raise CkError(f"{where}: '{word}' takes {lo}" + ("" if hi == lo else f"..{hi if hi is not None else 'n'}")
                          + f" argument(s), got {len(args)}")
        if not rep and word in seen:
            raise CkError(f"{where}: '{word}' given twice")
        seen.add(word)
        if word in ("name", "class", "difficulty"):
            v = args[0]
            if word == "name" and not NAME_RE.match(v):
                raise CkError(f"{where}: name: 2-15 letters (a character name)")
            if word == "class" and v not in CLASSES:
                raise CkError(f"{where}: class one of {', '.join(CLASSES)}")
            if word == "difficulty" and v not in DIFFS:
                raise CkError(f"{where}: difficulty one of {', '.join(DIFFS)}")
            ck[word] = v
        elif word == "title":
            ck["title"] = " ".join(args)
        elif word == "level":
            ck["level"] = _int(args[0], where, 1, 99)
        elif word == "act":
            ck["act"] = _int(args[0], where, 0, 4)
        elif word == "gold":
            ck["gold"] = _int(args[0], where, 0)
        elif word in ("expansion", "hardcore"):
            ck[word] = True
        elif word == "stat":
            ck["stat"].append((_int(args[0], where, 0, 15), _int(args[1], where, 0)))
        elif word == "skill":
            ck["skill"].append((_int(args[0], where, 0, 29), _int(args[1], where, 0, 99)))
        elif word in ("left-skill", "right-skill"):
            ck[word] = _int(args[0], where, 0, 0xFFFF)
        elif word in ("quests", "waypoints"):
            pat = QUEST_RE if word == "quests" else WP_RE
            for a in args:
                if not pat.match(a):
                    raise CkError(f"{where}: {word} item '{a}'")
            ck[word] += args
        elif word == "item":
            ck["item"].append(args[0])
        elif word == "start":
            f = _int(args[0], where, 1)
            if ck["start"] and f < ck["start"][-1][0]:
                raise CkError(f"{where}: start frames must not decrease")
            ck["start"].append((f, " ".join(args[1:])))
        elif word in ("need", "note"):
            ck[word].append(" ".join(args))
    if not seen_format:
        raise CkError(f"{fname}: empty definition")
    for k in REQUIRED:
        if k not in ck:
            raise CkError(f"{fname}: no '{k}'")
    if ck["act"] == 4 and not ck["expansion"]:
        raise CkError(f"{fname}: act 4 (Act V) needs 'expansion'")
    return ck


def d2s_args(ck):
    """The `d2s-tool new` arguments of a checkpoint (spec §3)."""
    a = ["--name", ck["name"], "--class", ck["class"], "--level", str(ck["level"])]
    if ck["expansion"]:
        a.append("--expansion")
    if ck["hardcore"]:
        a.append("--hardcore")
    for s, v in ck["stat"]:
        a += ["--stat", f"{s}={v}"]
    for i, v in ck["skill"]:
        a += ["--skill", f"{i}={v}"]
    for k in ("left-skill", "right-skill"):
        if k in ck:
            a += [f"--{k}", str(ck[k])]
    if "gold" in ck:
        a += ["--gold", str(ck["gold"])]
    if ck["quests"]:
        a += ["--quests", ",".join(ck["quests"])]
    if ck["waypoints"]:
        a += ["--waypoints", ",".join(ck["waypoints"])]
    for it in ck["item"]:
        a += ["--item", it]
    a += ["--act", str(ck["act"]), "--difficulty", ck["difficulty"]]
    # fixed seeds and times: the same definition gives the same bytes
    a += ["--seed", "1", "--map-seed", "1", "--time", "0x60000000"]
    return a


def load_all(names=None):
    files = sorted(f for f in os.listdir(DEFS) if f.endswith(".checkpoint"))
    out = []
    for f in files:
        stem = f[:-len(".checkpoint")]
        if not FILE_RE.match(stem):
            raise CkError(f"{f}: file name must match [a-z0-9-]+")
        if names and stem not in names:
            continue
        with open(os.path.join(DEFS, f), encoding="utf-8") as fh:
            ck = parse(fh.read(), f)
        ck["file"] = stem
        out.append(ck)
    if names:
        missing = set(names) - {c["file"] for c in out}
        if missing:
            raise CkError(f"no checkpoint {', '.join(sorted(missing))}")
    seen = {}
    for c in out:
        if c["name"] in seen:
            raise CkError(f"{c['file']}: character name {c['name']} also used by {seen[c['name']]}")
        seen[c["name"]] = c["file"]
    return out


def _env(game_dir):
    e = dict(os.environ)
    if game_dir:
        e["D2_GAME_DIR"] = game_dir
    e.setdefault("RUST_BACKTRACE", "0")
    return e


def _err(text):
    lines = [l for l in text.splitlines() if l.strip()]
    for key in ("panicked", "Error:", "error:"):
        for l in lines:
            if key in l:
                return l.strip()
    return lines[-1].strip() if lines else "(no output)"


def build(ck, d2s, out, game_dir):
    path = os.path.join(out, f"{ck['file']}.d2s")
    cmd = [d2s, "new"] + d2s_args(ck) + ["-o", path]
    r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir))
    if r.returncode != 0:
        raise CkError(f"{ck['file']}: d2s-tool new: {_err(r.stderr + r.stdout)}")
    r = subprocess.run([d2s, "check", path], capture_output=True, text=True, env=_env(game_dir))
    if r.returncode != 0 or ": OK:" not in r.stdout:
        raise CkError(f"{ck['file']}: d2s-tool check: {_err(r.stderr + r.stdout)}")
    with open(os.path.join(out, f"{ck['file']}.start"), "w", encoding="utf-8") as f:
        f.write(f"# {FORMAT} start pokes of {ck['file']} (absolute frames, poke.md §2 r6)\n")
        for fr, d in ck["start"]:
            f.write(f"{fr} {d}\n")
    return path, cmd


def read_state(path):
    snaps, pokes = [], []
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.strip():
                r = json.loads(line)
                if r.get("k") == "snap":
                    snaps.append(r)
                elif r.get("k") == "poke":
                    pokes.append(r)
    return snaps, pokes


def player(snap):
    return next((u for u in snap.get("units", []) if u.get("ut") == 0), None)


def quest_bits(q):
    """`q` ([slot, word] pairs) as sorted `slot.bit` strings."""
    return [f"{s}.{b}" for s, w in q for b in range(16) if w >> b & 1]


def verify(ck, save, client, out, game_dir, ticks):
    """Loads the save in d2rs twice (spec §4): plain (the load check) and
    with the start pokes (where the session begins). Returns a row."""
    row = {"name": ck["file"], "load": "fail"}
    for tag, pokes, n in (("load", [], ticks), ("start", ck["start"], max([ticks] + [f + (GOTO_TICKS if d.startswith("goto") else 20) for f, d in ck["start"]]))):
        st = os.path.join(out, f"{ck['file']}.{tag}.jsonl")
        cmd = [client, "state-dump", "--save", save, "--seed", "1", "--ticks", str(n), "--out", st,
               "--date", "2026-01-01", "--difficulty", ck["difficulty"]]
        for f, d in pokes:
            cmd += ["--poke", f"{f} {d}"]
        row[f"{tag}_cmd"] = " ".join(shlex.quote(c) for c in cmd)
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir), timeout=1200)
        except subprocess.TimeoutExpired:
            row[tag] = "timeout"
            continue
        if r.returncode != 0:
            row[tag] = f"crash: {_err(r.stderr)}"
            continue
        snaps, pk = read_state(st)
        pl = player(snaps[-1]) if snaps else None
        if not pl:
            row[tag] = "no player"
            continue
        row[tag] = "ok"
        row[f"{tag}_state"] = {k: pl.get(k) for k in ("lv", "act", "x", "y", "lvl", "hp", "hpx", "mp", "mpx", "m")}
        row[f"{tag}_state"]["f"] = snaps[-1]["f"]
        row[f"{tag}_quests"] = quest_bits(pl.get("q", []))
        if pokes:
            row["pokes"] = [f"{p.get('d')}:{p.get('r')}" for p in pk]
            if any(p.get("r") != "ok" for p in pk) or len(pk) != len(pokes):
                row["start"] = "pokes " + " ".join(row["pokes"])
    return row


def check_row(ck, row):
    """The load check (spec §4 r2): the player is at the save's level,
    in the town of the save's act, with the save's quest bits; and every
    `need` of the definition holds after the start pokes."""
    fails = []
    if row.get("load") != "ok":
        return [f"load {row.get('load')}"]
    s = row["load_state"]
    if s.get("lvl") != ck["level"]:
        fails.append(f"lvl {s.get('lvl')} != {ck['level']}")
    if s.get("act") != ck["act"]:
        fails.append(f"act {s.get('act')} != {ck['act']}")
    want = {q for q in ck["quests"] if not q.split(":")[-1].startswith("acts=")}
    want = {q.split(":")[-1] for q in want if ":" not in q or q.startswith(ck["difficulty"] + ":")}
    missing = sorted(want - set(row.get("load_quests", [])), key=lambda q: tuple(map(int, q.split("."))))
    if missing:
        fails.append("quest bits missing " + ",".join(missing))
    for n in ck["need"]:
        t = n.split()
        if row.get("start") != "ok":
            fails.append(f"start {row.get('start')}")
            break
        if len(t) == 3 and t[0] == "player" and t[1] in ("lv", "act", "lvl"):
            v = row["start_state"].get(t[1])
            if v != _int(t[2], ck.get("file", ck["name"])):
                fails.append(f"start {t[1]} {v} != {t[2]}")
        else:
            fails.append(f"need '{n}': only 'player lv|act|lvl N' is checked here")
    return fails


TSV_COLS = ("name", "load", "lvl", "act", "lv", "x", "y", "hp", "mp", "quests", "start", "start_lv", "start_x",
            "start_y", "pokes")


def tsv(rows, cmd):
    out = [f"# checkpoint-start-1 (tools/checkpoints/make.py {VERSION}; specs/tools/checkpoints.md §4)",
           f"# command: {cmd}",
           "# d2rs only: what d2rs makes of each save (load = state after the load ticks; start = after the start pokes).",
           "\t".join(TSV_COLS)]
    for r in rows:
        s = r.get("load_state", {})
        t = r.get("start_state", {})
        out.append("\t".join(str(v) for v in (
            r["name"], r.get("load"), s.get("lvl"), s.get("act"), s.get("lv"), s.get("x"), s.get("y"),
            s.get("hp"), s.get("mp"), ",".join(r.get("load_quests", [])) or "-", r.get("start"),
            t.get("lv"), t.get("x"), t.get("y"), " ".join(r.get("pokes", [])) or "-")))
    return "\n".join(out) + "\n"


def tools(build_first):
    rel = os.path.join(REPO, "target", "release")
    client, d2s = os.path.join(rel, "d2-client"), os.path.join(rel, "d2s-tool")
    if build_first:
        env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0", CARGO_INCREMENTAL="0")
        if subprocess.run(["cargo", "build", "--release", "-q", "-p", "d2-client", "-p", "d2s-tool"],
                          cwd=REPO, env=env).returncode != 0:
            raise CkError("cargo build failed")
    for p in (client, d2s):
        if not os.path.exists(p):
            raise CkError(f"{p} missing: run with --build")
    return client, d2s


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("names", nargs="*", help="checkpoint names (default: all)")
    ap.add_argument("--out", default=os.path.join(REPO, "target", "checkpoints"))
    ap.add_argument("--game-dir", default=os.environ.get("D2_GAME_DIR"))
    ap.add_argument("--build", action="store_true", help="cargo build --release d2-client and d2s-tool first")
    ap.add_argument("--verify", action="store_true", help="load each save in d2rs (state-dump)")
    ap.add_argument("--ticks", type=int, default=25, help="load-check ticks (default 25)")
    ap.add_argument("--record", help="with --verify: write the checkpoint-start-1 table here")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    try:
        cks = load_all(a.names)
        if a.list:
            for c in cks:
                print(f"{c['file']:<16} {c['class']} lvl {c['level']:<2} act {c['act'] + 1} {c['difficulty']:<9} {c['title']}")
            return 0
        if not a.game_dir:
            raise CkError("D2_GAME_DIR or --game-dir is needed (d2s-tool reads the tables)")
        client, d2s = tools(a.build)
        os.makedirs(a.out, exist_ok=True)
        rows, worst = [], 0
        for ck in cks:
            path, cmd = build(ck, d2s, a.out, a.game_dir)
            print(f"{ck['file']}: {path}")
            if a.verify:
                row = verify(ck, path, client, a.out, a.game_dir, a.ticks)
                fails = check_row(ck, row)
                rows.append(row)
                if fails:
                    worst = 1
                    print(f"  FAIL {'; '.join(fails)}")
                    print(f"  reproduce: D2_GAME_DIR=... {row.get('load_cmd' if row.get('load') != 'ok' else 'start_cmd')}")
                else:
                    s, t = row["load_state"], row.get("start_state", {})
                    print(f"  ok: load lv {s['lv']} act {s['act']} at ({s['x']},{s['y']}); "
                          f"start lv {t.get('lv')} at ({t.get('x')},{t.get('y')}) {' '.join(row.get('pokes', []))}")
        if a.record and rows:
            with open(a.record, "w", encoding="utf-8") as f:
                f.write(tsv(rows, "python3 tools/checkpoints/make.py --verify --record " + os.path.relpath(a.record, REPO)))
        return worst
    except (CkError, OSError) as e:
        print(f"checkpoints: error: {e}", file=sys.stderr)
        return 3


def selftest():
    good = """
# a comment
checkpoint 1
name CkTest
title Test: a test
class sor
level 24
expansion
difficulty normal
act 1
stat 0 35
skill 8 10
skill 0 5
right-skill 44
gold 5000
quests acts=1 9.0
quests nightmare:1.0
waypoints lv=1 lv=40 3
item cap/body=1
start 5 warp 49
start 20 goto unit 229
need player lv 49
"""
    ck = parse(good, "t")
    assert ck["name"] == "CkTest" and ck["level"] == 24 and ck["act"] == 1 and ck["expansion"]
    assert ck["quests"] == ["acts=1", "9.0", "nightmare:1.0"] and ck["waypoints"] == ["lv=1", "lv=40", "3"]
    assert ck["start"] == [(5, "warp 49"), (20, "goto unit 229")], ck["start"]
    a = d2s_args(ck)
    s = " ".join(a)
    for frag in ("--name CkTest", "--class sor", "--level 24", "--expansion", "--stat 0=35", "--skill 8=10",
                 "--right-skill 44", "--gold 5000", "--quests acts=1,9.0,nightmare:1.0",
                 "--waypoints lv=1,lv=40,3", "--item cap/body=1", "--act 1 --difficulty normal", "--seed 1"):
        assert frag in s, (frag, s)
    base = good.replace("start 20 goto unit 229\n", "")
    for bad, why in ((good.replace("checkpoint 1", "checkpoint 2"), "version"),
                     (good.replace("class sor", "class xyz"), "class"),
                     (good.replace("level 24", "level 100"), "level range"),
                     (good.replace("act 1", "act 5"), "act range"),
                     (good.replace("name CkTest", "name Ck_1"), "name"),
                     (good + "level 3\n", "twice"),
                     (good + "frob 1\n", "keyword"),
                     (good + "quests 1-0\n", "quest item"),
                     (good + "waypoints lv=x\n", "waypoint item"),
                     (good + "start 3 warp 2\n", "start order"),
                     (good.replace("skill 8 10", "skill 30 1"), "skill index"),
                     (base.replace("difficulty normal\n", ""), "required"),
                     (good.replace("act 1", "act 4").replace("expansion\n", ""), "act V classic"),
                     ("", "empty")):
        try:
            parse(bad, "bad")
        except CkError:
            continue
        raise AssertionError(f"accepted ({why})")
    # the load check
    row = {"name": "t", "load": "ok", "load_state": {"lvl": 24, "act": 1, "lv": 40, "x": 1, "y": 2},
           "load_quests": ["7.0", "9.0"], "start": "ok", "start_state": {"lv": 49}}
    assert check_row(ck, row) == [], check_row(ck, row)
    assert check_row(ck, dict(row, load_quests=["7.0"])) == ["quest bits missing 9.0"]
    assert check_row(ck, dict(row, load_state=dict(row["load_state"], act=0))) == ["act 0 != 1"]
    assert check_row(ck, dict(row, start_state={"lv": 48})) == ["start lv 48 != 49"]
    assert check_row(ck, dict(row, load="crash: x")) == ["load crash: x"]
    assert quest_bits([[1, 0x2002], [6, 1]]) == ["1.1", "1.13", "6.0"]
    # the committed definitions parse, names unique
    cks = load_all()
    assert len(cks) >= 13, len(cks)
    t = tsv([row], "x")
    assert t.splitlines()[0].startswith("# checkpoint-start-1") and t.splitlines()[3].split("\t") == list(TSV_COLS)
    print(f"checkpoints selftest: ok ({len(cks)} definitions)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
