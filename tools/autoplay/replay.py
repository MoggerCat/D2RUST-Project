#!/usr/bin/env python3
# Spec: specs/tools/autoplay.md (§3)
"""Real-input smoke routes: replays a fixed route file through
`d2-client autoplay-host` (the `play` client headless, mouse and key
events only) and checks the state at each milestone.

    python3 tools/autoplay/replay.py tools/autoplay/routes/act2-probe.route
    python3 tools/autoplay/replay.py --all
    python3 tools/autoplay/replay.py --selftest

Route format `autoplay-route 1` (spec §3): `#` comments; `host ARGS`
(extra autoplay-host options, e.g. `--new amazon Name --seed 1234`);
`save D2S-TOOL-NEW-ARGS` (the character, made with `d2s-tool new`);
then host commands (`step N`, `move`, `press`, `release`, `key`) sent
as they are, and checks: `check level N`, `check sent ID >= N` (C->S
message counts, hex id). No decisions are made: the same build, save
and seed give the same game, so the same clicks.

Exit 0: every check held; 1: a check failed or the host died; 3: error.
Python stdlib only.
"""

import argparse
import glob
import json
import os
import shlex
import subprocess
import sys
import tempfile

FORMAT = "autoplay-route 1"
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ROUTES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "routes")


class RouteError(Exception):
    pass


def parse(text, name="<route>"):
    """(host args, save args or None, steps); a step is (line no, text)."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != FORMAT:
        raise RouteError(f"{name}: first line must be '{FORMAT}'")
    host, save, steps = [], None, []
    for no, raw in enumerate(lines[1:], 2):
        t = raw.strip()
        if not t:
            continue
        if t.startswith("#"):
            steps.append((no, t))
            continue
        word, _, rest = t.partition(" ")
        if word == "host":
            host += shlex.split(rest)
        elif word == "save":
            save = shlex.split(rest)
        elif word == "check":
            c = rest.split()
            ok = (len(c) == 2 and c[0] == "level") or (len(c) == 4 and c[0] == "sent" and c[2] == ">=")
            if not ok:
                raise RouteError(f"{name}:{no}: check level N | check sent ID >= N")
            steps.append((no, t))
        elif word in ("step", "move", "press", "release", "key"):
            steps.append((no, t))
        else:
            raise RouteError(f"{name}:{no}: unknown line '{word}'")
    return host, save, steps


def check(st, text):
    """None when `check ...` holds on state `st`, else why not."""
    c = text.split()[1:]
    me = next((u for u in st["snap"]["units"] if u["ut"] == 0 and u["g"] == st.get("local")), None)
    if c[0] == "level":
        lv = me.get("lv") if me else None
        return None if lv == int(c[1]) else f"player in level {lv}, expected {c[1]}"
    n = st["client"]["sent"].get(c[1].upper(), 0)
    return None if n >= int(c[3]) else f"C->S 0x{c[1]} sent {n} times, expected >= {c[3]}"


def replay(path, game_dir, work):
    name = os.path.basename(path)
    host_args, save_args, steps = parse(open(path).read(), name)
    rel = os.path.join(REPO, "target", "release")
    env = dict(os.environ, **({"D2_GAME_DIR": game_dir} if game_dir else {}))
    args = list(host_args)
    if save_args is not None:
        save = os.path.join(work, os.path.splitext(name)[0] + ".d2s")
        r = subprocess.run([os.path.join(rel, "d2s-tool"), "new"] + save_args + ["-o", save],
                           capture_output=True, text=True, env=env)
        if r.returncode != 0:
            raise RouteError(f"d2s-tool new: {(r.stderr or r.stdout).strip()}")
        args = ["--save", save] + args
    err = open(os.path.join(work, os.path.splitext(name)[0] + ".stderr"), "w")
    p = subprocess.Popen([os.path.join(rel, "d2-client"), "autoplay-host"] + args, stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE, stderr=err, text=True, env=env, bufsize=1)

    def read():
        while True:
            line = p.stdout.readline()
            if not line:
                return None
            if line.startswith("{"):
                return json.loads(line)

    def send(line):
        p.stdin.write(line + "\n")
        p.stdin.flush()
        return read()

    milestone, passed, failed = None, [], None
    try:
        if (read() or {}).get("k") != "hello":
            raise RouteError("the host did not start")
        for no, t in steps:
            if t.startswith("# milestone "):
                milestone = t[len("# milestone "):].split(":")[0]
                continue
            if t.startswith("#"):
                continue
            if t.startswith("check "):
                st = send("state")
                if st is None or st.get("k") != "state":
                    failed = (no, milestone, f"host gone or no state: {st}")
                    break
                why = check(st, t)
                if why:
                    failed = (no, milestone, why)
                    break
                if milestone and milestone not in passed:
                    passed.append(milestone)
                continue
            r = send(t)
            if r is None:
                failed = (no, milestone, "the host exited (see the .stderr file)")
                break
            if r.get("k") == "error":
                failed = (no, milestone, f"host error: {r['error']}")
                break
    finally:
        try:
            p.stdin.write("quit\n")
            p.stdin.flush()
            p.wait(timeout=30)
        except Exception:  # noqa: BLE001 - the host may be gone
            p.kill()
        err.close()
    return passed, failed


def selftest():
    host, save, steps = parse(f"{FORMAT}\n# c\nhost --seed 7\nsave --name A --class sor\n"
                              "step 3\npress L 1 2\n# milestone m: x\ncheck level 2\ncheck sent 2F >= 1\n")
    assert host == ["--seed", "7"] and save == ["--name", "A", "--class", "sor"]
    assert [t for _, t in steps][1:] == ["step 3", "press L 1 2", "# milestone m: x", "check level 2",
                                         "check sent 2F >= 1"]
    st = {"local": 1, "snap": {"units": [{"ut": 0, "g": 1, "lv": 2}]}, "client": {"sent": {"2F": 1}}}
    assert check(st, "check level 2") is None and check(st, "check level 3")
    assert check(st, "check sent 2F >= 1") is None and check(st, "check sent 30 >= 1")
    for bad in ["route 1\n", f"{FORMAT}\nfly 1\n", f"{FORMAT}\ncheck level\n"]:
        try:
            parse(bad)
        except RouteError:
            continue
        raise AssertionError(bad)
    for f in glob.glob(os.path.join(ROUTES, "*.route")):
        parse(open(f).read(), f)
    print("selftest ok")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("routes", nargs="*")
    ap.add_argument("--all", action="store_true", help="every route in tools/autoplay/routes/")
    ap.add_argument("--game-dir", default=os.environ.get("D2_GAME_DIR"))
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    routes = sorted(glob.glob(os.path.join(ROUTES, "*.route"))) if a.all else a.routes
    if not routes:
        ap.error("name a route or --all")
    work = tempfile.mkdtemp(prefix="autoplay-replay-", dir=os.path.join(REPO, "target"))
    rc = 0
    for r in routes:
        try:
            passed, failed = replay(r, a.game_dir, work)
        except (RouteError, OSError) as e:
            print(f"{os.path.basename(r)}: ERROR {e}")
            return 3
        if failed:
            no, m, why = failed
            rc = 1
            print(f"{os.path.basename(r)}: FAIL at line {no} (milestone {m}): {why}; passed {len(passed)}: {' '.join(passed)}")
        else:
            print(f"{os.path.basename(r)}: ok, {len(passed)} milestones: {' '.join(passed)}")
    print(f"work dir {work}")
    return rc


if __name__ == "__main__":
    sys.exit(main())
