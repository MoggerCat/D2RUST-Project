#!/usr/bin/env python3
# Spec: specs/tools/playthrough.md
"""Playthrough harness: drives the headless d2rs game (`d2-client
state-dump`) through the milestones of an objective file
(`traces/playthrough/actN.play`) and reports, per act, how far the game
gets and the first blocker.

    python3 tools/playthrough/playthrough.py traces/playthrough/act1.play
    python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --only kill-zombie
    python3 tools/playthrough/playthrough.py --selftest

Each milestone is its own state-dump run (fresh save and seed), so one
blocker does not hide the rest. Exit 0: every milestone reached; 1: a
blocker; 3: error (objective file, missing tool, unreadable state file).

d2rs only; no 1.14d side (spec Summary). Python stdlib only.
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
FORMAT = "playthrough 1"
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Death modes per unit type (specs/sim/units.md §1): player DT 0 / DD 17,
# monster DT 0 / DD 12.
DEAD_MODES = {0: (0, 17), 1: (0, 12)}
OPS = ("==", "!=", ">=", "<=", ">", "<")


class PlayError(Exception):
    """An objective-file or tool error (exit 3)."""


# ---------------------------------------------------------------- parsing


def parse_play(text, name="<play>"):
    """Parses an objective file (spec §1). Returns
    {"act": n, "saves": {name: [args]}, "milestones": [...]}."""
    play = {"act": None, "saves": {}, "milestones": []}
    cur = None
    seen_format = False
    for no, raw in enumerate(text.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        where = f"{name}:{no}"
        if not seen_format:
            if line != FORMAT:
                raise PlayError(f"{where}: first line must be '{FORMAT}'")
            seen_format = True
            continue
        word, _, rest = line.partition(" ")
        rest = rest.strip()
        if word == "act":
            play["act"] = _int(rest, where)
        elif word == "save":
            sname, _, args = rest.partition(" ")
            if not sname or not args:
                raise PlayError(f"{where}: save <name> <d2s-tool new args>")
            play["saves"][sname] = shlex.split(args)
        elif word == "milestone":
            parts = rest.split()
            if len(parts) != 1:
                raise PlayError(f"{where}: milestone <name>")
            cur = {
                "name": parts[0],
                "line": no,
                "save": None,
                "seed": 1,
                "ticks": None,
                "difficulty": None,
                "pokes": [],
                "input": None,
                "need": [],
                "sweep": None,
                "find": None,
                "note": "",
            }
            play["milestones"].append(cur)
        else:
            if cur is None:
                raise PlayError(f"{where}: '{word}' outside a milestone")
            if word == "use":
                if rest not in play["saves"]:
                    raise PlayError(f"{where}: unknown save '{rest}'")
                cur["save"] = rest
            elif word == "seed":
                cur["seed"] = _int(rest, where)
            elif word == "deadline":
                cur["ticks"] = _int(rest, where)
            elif word == "difficulty":
                cur["difficulty"] = rest
            elif word == "poke":
                f, _, d = rest.partition(" ")
                _int(f[1:] if f.startswith("+") else f, where)
                if not d.strip():
                    raise PlayError(f"{where}: poke <frame> <directive>")
                cur["pokes"].append(f"{f} {d.strip()}")
            elif word == "sweep":
                if cur["sweep"]:
                    raise PlayError(f"{where}: one sweep per milestone")
                t = rest.split()
                mode = t.pop() if t and t[-1] in ("spiral", "grid", "cross") else "spiral"
                cur["sweep"] = [_int(x, where) for x in _nargs(" ".join(t), 4, where, "sweep <frame> <every> <radius> <step> [spiral|grid|cross]")] + [mode]
            elif word == "find":
                pred = parse_pred(f"unit {rest} present", where)
                cur["find"] = pred
            elif word == "input":
                cur["input"] = rest
            elif word == "need":
                ever = rest.startswith("ever ")
                pred = parse_pred(rest[5:] if ever else rest, where)
                pred["ever"] = ever
                pred["src"] = rest
                cur["need"].append(pred)
            elif word == "note":
                cur["note"] = rest
            else:
                raise PlayError(f"{where}: unknown keyword '{word}'")
    if not seen_format:
        raise PlayError(f"{name}: empty objective file")
    names = set()
    for m in play["milestones"]:
        w = f"{name}:{m['line']} milestone {m['name']}"
        if m["name"] in names:
            raise PlayError(f"{w}: duplicate name")
        names.add(m["name"])
        if m["save"] is None:
            raise PlayError(f"{w}: no 'use <save>'")
        if m["ticks"] is None:
            raise PlayError(f"{w}: no 'deadline <frames>'")
        if not m["need"]:
            raise PlayError(f"{w}: no 'need' predicate")
        rel = [p for p in m["pokes"] if p.startswith("+")] or ("frame +" in (m["input"] or ""))
        if m["find"] and not m["sweep"]:
            raise PlayError(f"{w}: 'find' needs a 'sweep'")
        if rel and not m["find"]:
            raise PlayError(f"{w}: relative '+N' frames need a 'find'")
    return play


def _int(s, where):
    try:
        return int(s, 0)
    except ValueError:
        raise PlayError(f"{where}: not a number: '{s}'") from None


def _nargs(rest, n, where, usage):
    t = rest.split()
    if len(t) != n:
        raise PlayError(f"{where}: {usage}")
    return t


def sweep_pokes(f0, every, radius, step, cx, cy, mode="spiral"):
    """`sweep F0 EVERY R STEP` (spec §1): absolute `pos @player` pokes from
    frame F0, one every EVERY frames, on a square spiral outward from
    (cx, cy) (the player's position before F0; a probe run finds it),
    STEP sub-tiles apart, out to half-side R, then back to (cx, cy). Each
    target is next to the previous one, so the room it needs is near the
    player even after a refused move (a refused move leaves the player
    where it was)."""
    if every < 1 or step < 1 or radius < step:
        raise PlayError("sweep: every >= 1, step >= 1, radius >= step")
    n = radius // step
    if mode == "cross":
        # out and back along +x, +y, -x, -y (world axes: the iso diagonals)
        pts = []
        for dx, dy in ((1, 0), (0, 1), (-1, 0), (0, -1)):
            arm = [(cx + dx * i * step, cy + dy * i * step) for i in range(1, n + 1)]
            pts += arm + arm[-2::-1] + [(cx, cy)]
        return [f"{f0 + k * every} pos @player {px} {py}" for k, (px, py) in enumerate(pts) if px >= 0 and py >= 0]
    if mode == "grid":
        # rows of the square, serpentine, from the (-R, -R) corner
        pts = []
        for row in range(-n, n + 1):
            xs = range(-n, n + 1) if (row + n) % 2 == 0 else range(n, -n - 1, -1)
            pts += [(cx + i * step, cy + row * step) for i in xs]
        pts.append((cx, cy))
        return [f"{f0 + k * every} pos @player {px} {py}" for k, (px, py) in enumerate(pts) if px >= 0 and py >= 0]
    pts, x, y = [], 0, 0
    dirs = [(1, 0), (0, 1), (-1, 0), (0, -1)]
    leg, d = 1, 0
    while max(abs(x), abs(y)) <= n:
        for _ in range(2):
            dx, dy = dirs[d % 4]
            for _ in range(leg):
                x, y = x + dx, y + dy
                if max(abs(x), abs(y)) <= n:
                    pts.append((cx + x * step, cy + y * step))
            d += 1
        leg += 1
    pts.append((cx, cy))
    return [f"{f0 + k * every} pos @player {px} {py}" for k, (px, py) in enumerate(pts) if px >= 0 and py >= 0]


def _rel(base, d):
    return base if d == 0 else f"{base}{d:+d}"


def parse_pred(text, where):
    """`player <field> <op> <n>` | `player moved >= <n>` |
    `unit <filters> present|absent|dead|count <op> <n>` (spec §2)."""
    t = text.split()
    if not t:
        raise PlayError(f"{where}: empty predicate")
    if t[0] == "player":
        if len(t) != 4 or t[2] not in OPS:
            raise PlayError(f"{where}: player <field> <op> <n>")
        return {"kind": "player", "field": t[1], "op": t[2], "value": _int(t[3], where), "src": text}
    if t[0] == "unit":
        filt = {}
        i = 1
        while i < len(t) and t[i] in ("ut", "cl", "lv", "g"):
            if i + 1 >= len(t):
                raise PlayError(f"{where}: {t[i]} needs a value")
            v = t[i + 1]
            if t[i] == "g" and v.startswith("@p"):
                filt["g"] = ("poke", _int(v[2:], where))
            else:
                filt[t[i]] = [_int(x, where) for x in v.split(",")]
            i += 2
        rest = t[i:]
        if rest in (["present"], ["absent"], ["dead"], ["seen"], ["killed"]):
            return {"kind": "unit", "filter": filt, "test": rest[0], "src": text}
        if len(rest) == 3 and rest[0] == "count" and rest[1] in OPS:
            return {"kind": "unit", "filter": filt, "test": "count", "op": rest[1],
                    "value": _int(rest[2], where), "src": text}
        raise PlayError(f"{where}: unit [ut|cl|lv|g V]... present|absent|seen|dead|killed|count <op> <n>")
    raise PlayError(f"{where}: predicate must start with player or unit")


# ------------------------------------------------------------- state file


def read_state(path):
    """Reads a state-1 file: (header, snaps, pokes, footer)."""
    header, snaps, pokes, footer = None, [], [], None
    with open(path, encoding="utf-8") as f:
        for no, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                r = json.loads(line)
            except ValueError as e:
                raise PlayError(f"{path}:{no}: bad JSON ({e})") from None
            k = r.get("k")
            if k == "header":
                if r.get("format") != "state-1":
                    raise PlayError(f"{path}: format {r.get('format')!r}, want state-1")
                header = r
            elif k == "snap":
                snaps.append(r)
            elif k == "poke":
                pokes.append(r)
            elif k == "footer":
                footer = r
    if header is None:
        raise PlayError(f"{path}: no header")
    return header, snaps, pokes, footer


def player_of(snap):
    for u in snap["units"]:
        if u.get("ut") == 0:
            return u
    return None


def _cmp(a, op, b):
    return {"==": a == b, "!=": a != b, ">=": a >= b, "<=": a <= b, ">": a > b, "<": a < b}[op]


def _matches(u, filt, guids):
    for k, v in filt.items():
        if k == "g" and isinstance(v, tuple):
            g = guids.get(v[1])
            if g is None or u.get("g") != g:
                return False
        elif u.get(k) not in v:
            return False
    return True


def poke_guids(pokes):
    """Poke index (file order of the milestone) -> created GUID."""
    out = {}
    for i, p in enumerate(pokes):
        if p.get("guid") is not None:
            out[i] = p["guid"]
    return out


def eval_pred(pred, snaps, idx, guids, first_player=None):
    """Evaluates `pred` at snaps[idx]. Returns (ok, evidence)."""
    snap = snaps[idx]
    if pred["kind"] == "player":
        p = player_of(snap)
        if p is None:
            return False, "no player unit"
        if pred["field"] == "moved":
            if first_player is None or "x" not in p or "x" not in first_player:
                return False, "no player position"
            v = max(abs(p["x"] - first_player["x"]), abs(p["y"] - first_player["y"]))
        else:
            v = p.get(pred["field"])
            if v is None:
                return False, f"player has no '{pred['field']}'"
        return _cmp(v, pred["op"], pred["value"]), f"player {pred['field']}={v}"
    filt = pred["filter"]
    if "g" in filt and isinstance(filt["g"], tuple) and filt["g"][1] not in guids:
        return False, f"poke {filt['g'][1]} created no unit"
    hit = [u for u in snap["units"] if _matches(u, filt, guids)]
    test = pred["test"]
    if test == "present":
        return bool(hit), f"{len(hit)} matching"
    if test == "seen":
        n = sum(1 for s in snaps[: idx + 1] if any(_matches(u, filt, guids) for u in s["units"]))
        first = next((s["f"] for s in snaps[: idx + 1] if any(_matches(u, filt, guids) for u in s["units"])), None)
        return n > 0, f"seen in {n} snapshots" + (f" from f{first}" if first is not None else "")
    if test == "absent":
        return not hit, f"{len(hit)} matching"
    if test == "count":
        return _cmp(len(hit), pred["op"], pred["value"]), f"{len(hit)} matching"
    if test == "killed":
        dead = [u for u in hit if u.get("m") in DEAD_MODES.get(u.get("ut"), (0,))]
        return bool(dead), f"{len(dead)} of {len(hit)} matching in a death mode"
    # dead: seen alive-or-dead before, and now every match in a death mode or gone
    seen = any(_matches(u, filt, guids) for s in snaps[: idx + 1] for u in s["units"])
    if not seen:
        return False, "never present"
    alive = [u for u in hit if u.get("m") not in DEAD_MODES.get(u.get("ut"), (0,))]
    if alive:
        u = alive[0]
        return False, (f"{len(alive)} alive, e.g. ut {u.get('ut')} g {u.get('g')} cl {u.get('cl')} "
                       f"m {u.get('m')} hp {u.get('hp')} at ({u.get('x')},{u.get('y')})")
    return True, f"{len(hit)} dead, {'gone' if not hit else 'in death modes'}"


def eval_final(p, snaps, guids, first_player):
    """A predicate at the deadline (the last snapshot); an `ever`
    predicate holds when it held at any snapshot (spec §2)."""
    if p.get("ever"):
        for i in range(len(snaps)):
            ok, e = eval_pred(p, snaps, i, guids, first_player)
            if ok:
                return True, f"{e} at f{snaps[i]['f']}"
        return False, "never held; at deadline " + eval_pred(p, snaps, len(snaps) - 1, guids, first_player)[1]
    return eval_pred(p, snaps, len(snaps) - 1, guids, first_player)


def evaluate(m, snaps, pokes):
    """A milestone's verdict on its state file (spec §3). Returns dict
    with status reached|stuck|missing-unit|wrong-level, frame, evidence."""
    if not snaps:
        return {"status": "stuck", "frame": None, "evidence": "no snapshot"}
    guids = poke_guids(pokes)
    first_player = next((p for p in map(player_of, snaps) if p and "x" in p), None)
    first_all = None
    for i in range(len(snaps)):
        if all(eval_pred(p, snaps, i, guids, first_player)[0] for p in m["need"] if not p.get("ever")):
            first_all = snaps[i]["f"]
            break
    last = len(snaps) - 1
    results = [(p, *eval_final(p, snaps, guids, first_player)) for p in m["need"]]
    f = snaps[last]["f"]
    if all(ok for _, ok, _ in results):
        ev = "; ".join(f"{p['src']}: {e}" for p, _, e in results)
        return {"status": "reached", "frame": first_all, "evidence": f"first held f{first_all}, at f{f}: {ev}"}
    bad = [(p, e) for p, ok, e in results if not ok]
    p, e = bad[0]
    pl = player_of(snaps[last]) or {}
    where = f"player lv {pl.get('lv')} m {pl.get('m')} at ({pl.get('x')},{pl.get('y')})"
    # a refused sweep step is expected (spec §1 sweep); any other poke's
    # refusal is the blocker's evidence
    failed_pokes = [q for q in pokes if q.get("r") != "ok" and not str(q.get("src", "")).startswith("pos @player ")]
    if failed_pokes:
        q = failed_pokes[0]
        return {"status": "missing-unit" if q.get("r") == "unresolved" else "stuck", "frame": f,
                "evidence": f"poke '{q.get('src')}' -> {q.get('r')} {q.get('note', '')}; {p['src']}: {e}; {where}"}
    if p["kind"] == "player" and p["field"] == "lv":
        status = "wrong-level"
    elif p["kind"] == "unit" and p["test"] in ("present", "count", "dead") and e.startswith(("0 matching", "never present", "poke ", "seen in 0")):
        status = "missing-unit"
    else:
        status = "stuck"
    return {"status": status, "frame": f, "evidence": f"at deadline f{f}: {p['src']}: {e}; {where}"}


# ---------------------------------------------------------------- running


def tool_paths(args):
    rel = os.path.join(REPO, "target", "release")
    client = os.path.join(rel, "d2-client")
    d2s = os.path.join(rel, "d2s-tool")
    if args.build:
        env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0", CARGO_INCREMENTAL="0")
        r = subprocess.run(["cargo", "build", "--release", "-q", "-p", "d2-client", "-p", "d2s-tool"],
                           cwd=REPO, env=env)
        if r.returncode != 0:
            raise PlayError("cargo build failed")
    for p in (client, d2s):
        if not os.path.exists(p):
            raise PlayError(f"{p} missing: run with --build (or cargo build --release -p d2-client -p d2s-tool)")
    return client, d2s


def first_error_line(text):
    lines = [l for l in text.splitlines() if l.strip()]
    for key in ("panicked", "Error:", "error:"):
        for l in lines:
            if key in l:
                return l.strip()
    return lines[-1].strip() if lines else "(no output)"


def run_milestone(m, play, client, d2s, work, game_dir):
    save = os.path.join(work, f"{m['save']}.d2s")
    if not os.path.exists(save):
        cmd = [d2s, "new"] + play["saves"][m["save"]] + ["-o", save]
        r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir))
        if r.returncode != 0:
            raise PlayError(f"save {m['save']}: {first_error_line(r.stderr + r.stdout)}")
    pokes = [p for p in m["pokes"] if not p.startswith("+")]
    script = m["input"]
    if m["sweep"]:
        f0, every, radius, step, mode = m["sweep"]
        probe = os.path.join(work, f"{m['name']}.probe.jsonl")
        cmd = [client, "state-dump", "--save", save, "--seed", str(m["seed"]), "--ticks", str(f0 - 1),
               "--out", probe, "--date", "2026-01-01"] + _flags(m, [p for p in pokes if int(p.split()[0]) < f0])
        r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir), timeout=900)
        if r.returncode != 0:
            m["cmd"] = " ".join(shlex.quote(c) for c in cmd)
            return {"status": "crash", "frame": None, "evidence": f"sweep probe exit {r.returncode}: {first_error_line(r.stderr)}"}
        _, snaps, _, _ = read_state(probe)
        pl = player_of(snaps[-1]) if snaps else None
        if not pl or "x" not in pl:
            return {"status": "stuck", "frame": f0 - 1, "evidence": "sweep probe: no player position"}
        pokes += sweep_pokes(f0, every, radius, step, pl["x"], pl["y"], mode)
        pokes.sort(key=lambda p: int(p.split()[0]))
        if m["find"]:
            found = find_probe(m, pokes, save, client, work, game_dir)
            if isinstance(found, dict):
                return found
            pokes, script = shift_after_find(m, pokes, found)
    out = os.path.join(work, f"{m['name']}.state.jsonl")
    cmd = [client, "state-dump", "--save", save, "--seed", str(m["seed"]),
           "--ticks", str(m.get("run_ticks", m["ticks"])), "--out", out, "--date", "2026-01-01"]
    cmd += _flags(m, pokes)
    if script:
        cmd += ["--input", script]
    m["cmd"] = " ".join(shlex.quote(c) for c in cmd)
    if len(m["cmd"]) > 600:
        script = os.path.join(work, f"{m['name']}.sh")
        with open(script, "w", encoding="utf-8") as f:
            f.write("#!/bin/sh\n# playthrough.py: milestone " + m["name"] + "\n" + m["cmd"] + "\n")
        m["cmd"] = f"sh {script}  # {len(pokes)} pokes"
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir), timeout=900)
    except subprocess.TimeoutExpired:
        return {"status": "crash", "frame": None, "evidence": "timeout (900 s)"}
    if r.returncode != 0:
        return {"status": "crash", "frame": None,
                "evidence": f"exit {r.returncode}: {first_error_line(r.stderr)}"}
    _, snaps, pokes, _ = read_state(out)
    return evaluate(m, snaps, pokes)


def find_probe(m, pokes, save, client, work, game_dir):
    """`find` (spec §1 r5): runs the whole sweep to the deadline and
    returns the first frame at which a unit matching the filter is
    present, or a blocker verdict when none ever is."""
    probe = os.path.join(work, f"{m['name']}.find.jsonl")
    cmd = [client, "state-dump", "--save", save, "--seed", str(m["seed"]), "--ticks", str(m["ticks"]),
           "--out", probe, "--date", "2026-01-01"] + _flags(m, pokes)
    m["cmd"] = " ".join(shlex.quote(c) for c in cmd)
    r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir), timeout=900)
    if r.returncode != 0:
        return {"status": "crash", "frame": None, "evidence": f"find probe exit {r.returncode}: {first_error_line(r.stderr)}"}
    _, snaps, ppokes, _ = read_state(probe)
    guids = poke_guids(ppokes)
    for i, s in enumerate(snaps):
        if eval_pred(m["find"], snaps, i, guids)[0]:
            return s["f"]
    pl = (player_of(snaps[-1]) if snaps else None) or {}
    return {"status": "missing-unit", "frame": snaps[-1]["f"] if snaps else None,
            "evidence": f"find {m['find']['src']}: never present in the sweep; "
                        f"player lv {pl.get('lv')} m {pl.get('m')} at ({pl.get('x')},{pl.get('y')})"}


def shift_after_find(m, pokes, found):
    """The run after a `find` at frame F: the sweep stops at F (its later
    `pos` pokes are dropped), `poke +N` runs at F + N and `frame +N` of
    the input script is frame F + N."""
    keep = [p for p in pokes if not (p.split()[1:3] == ["pos", "@player"] and int(p.split()[0]) > found)]
    for p in m["pokes"]:
        if p.startswith("+"):
            n, _, d = p.partition(" ")
            keep.append(f"{found + int(n[1:])} {d}")
    keep.sort(key=lambda p: int(p.split()[0]))
    rel = [int(p.split()[0][1:]) for p in m["pokes"] if p.startswith("+")]
    rel += [int(x) for x in re.findall(r"frame \+(\d+)", m["input"] or "")]
    # the deadline covers every relative step plus 60 frames to see its effect
    m["run_ticks"] = max(m["ticks"], found + max(rel, default=0) + 60)
    script = m["input"]
    if script:
        script = re.sub(r"frame \+(\d+)", lambda g: f"frame {found + int(g.group(1))}", script)
    return keep, script


def _flags(m, pokes):
    out = ["--difficulty", m["difficulty"]] if m["difficulty"] else []
    for p in pokes:
        out += ["--poke", p]
    return out


def _env(game_dir):
    e = dict(os.environ)
    if game_dir:
        e["D2_GAME_DIR"] = game_dir
    e.setdefault("RUST_BACKTRACE", "0")
    return e


def report(play, results, path):
    n = len(results)
    first_block = next(((m, r) for m, r in results if r["status"] != "reached"), None)
    furthest = 0
    for _, r in results:
        if r["status"] != "reached":
            break
        furthest += 1
    reached = sum(1 for _, r in results if r["status"] == "reached")
    print(f"playthrough {VERSION}: {path} (act {play['act']})")
    print(f"{'#':>2}  {'milestone':<22} {'status':<13} evidence")
    for i, (m, r) in enumerate(results, 1):
        print(f"{i:>2}  {m['name']:<22} {r['status']:<13} {r['evidence']}")
    print(f"furthest consecutive: {furthest}/{n}; reached {reached}/{n}")
    if first_block:
        m, r = first_block
        print(f"first blocker: {m['name']} ({r['status']}) frame {r['frame']}: {r['evidence']}")
        print(f"reproduce: D2_GAME_DIR=... {m.get('cmd', '(not run)')}")
        return 1
    print("all milestones reached")
    return 0


def summary(play, results, path):
    """One act's line of the `--all` table and of `--json` (spec Outputs):
    act, reached / total, furthest consecutive, first blocker."""
    consecutive = 0
    for _, r in results:
        if r["status"] != "reached":
            break
        consecutive += 1
    first = next(((m, r) for m, r in results if r["status"] != "reached"), None)
    fb = None
    if first:
        m, r = first
        fb = {"milestone": m["name"], "kind": r["status"], "frame": r["frame"],
              "evidence": r["evidence"], "command": m.get("cmd")}
    return {"play": path, "act": play["act"], "reached": sum(1 for _, r in results if r["status"] == "reached"),
            "total": len(results), "consecutive": consecutive, "first_blocker": fb}


def print_table(rows):
    print(f"{'act':>3}  {'reached':>7}  {'consec':>6}  first blocker")
    for s in rows:
        fb = s["first_blocker"]
        if fb:
            ev = fb["evidence"] if len(fb["evidence"]) <= 160 else fb["evidence"][:157] + "..."
            b = f"{fb['milestone']} ({fb['kind']}) f{fb['frame']}: {ev}"
        else:
            b = "none"
        print(f"{s['act']:>3}  {s['reached']:>3}/{s['total']:<3}  {s['consecutive']:>3}/{s['total']:<2}  {b}")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("play", nargs="*", help="objective files (traces/playthrough/actN.play)")
    ap.add_argument("--only", help="comma-separated milestone names")
    ap.add_argument("--work", help="work dir (default: a temp dir under target/playthrough)")
    ap.add_argument("--game-dir", default=os.environ.get("D2_GAME_DIR"))
    ap.add_argument("--build", action="store_true", help="cargo build --release d2-client and d2s-tool first")
    ap.add_argument("--all", action="store_true", help="every traces/playthrough/act*.play, then a per-act table")
    ap.add_argument("--json", help="also write the results as JSON here")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if a.all:
        d = os.path.join(REPO, "traces", "playthrough")
        a.play += sorted(os.path.join(d, f) for f in os.listdir(d)
                         if f.startswith("act") and f.endswith(".play"))
    if not a.play:
        ap.error("an objective file is needed (or --all)")
    try:
        client, d2s = tool_paths(a)
        worst = 0
        all_json = []
        for path in a.play:
            with open(path, encoding="utf-8") as f:
                play = parse_play(f.read(), path)
            ms = play["milestones"]
            if a.only:
                keep = set(a.only.split(","))
                ms = [m for m in ms if m["name"] in keep]
            base = a.work or os.path.join(REPO, "target", "playthrough")
            os.makedirs(base, exist_ok=True)
            work = tempfile.mkdtemp(prefix=f"act{play['act']}-", dir=base) if not a.work else base
            results = []
            for m in ms:
                r = run_milestone(m, play, client, d2s, work, a.game_dir)
                results.append((m, r))
                print(f"  {m['name']}: {r['status']}", file=sys.stderr)
            worst = max(worst, report(play, results, path))
            print(f"work dir: {work}")
            all_json.append(dict(summary(play, results, path), milestones=[
                {"name": m["name"], "command": m.get("cmd"), **r} for m, r in results]))
        if len(all_json) > 1 or a.all:
            print()
            print_table(all_json)
        if a.json:
            with open(a.json, "w", encoding="utf-8") as f:
                json.dump({"format": "playthrough-result-1", "tool": f"playthrough.py {VERSION}",
                           "acts": all_json}, f, indent=1)
        return worst
    except (PlayError, OSError) as e:
        print(f"playthrough: error: {e}", file=sys.stderr)
        return 3


# --------------------------------------------------------------- selftest


def selftest():
    play_text = """
playthrough 1
act 1
save ama --class ama --expansion --name PtAma   # a comment
milestone town
  use ama
  deadline 10
  need player lv == 1
  need unit ut 1 cl 148 present
milestone kill
  use ama
  seed 7
  deadline 10
  poke 2 spawn 5 @x+3 @y normal
  input frame 4; rclick 450 300
  need unit ut 1 g @p0 dead
milestone walk
  use ama
  deadline 10
  need player moved >= 3
  need unit ut 1 lv 2 count >= 2
"""
    play = parse_play(play_text, "self")
    assert play["act"] == 1 and play["saves"]["ama"][0] == "--class"
    town, kill, walk = play["milestones"]
    assert kill["seed"] == 7 and kill["pokes"] == ["2 spawn 5 @x+3 @y normal"]
    assert kill["input"] == "frame 4; rclick 450 300", kill["input"]
    for bad in ["act 1", "playthrough 1\nmilestone a\n deadline 3\n need player lv == 1",
                "playthrough 1\nsave a --x\nmilestone a\n use a\n need player lv == 1",
                "playthrough 1\nsave a --x\nmilestone a\n use a\n deadline 2\n need player lv ~ 1",
                "playthrough 1\nsave a --x\nmilestone a\n use a\n deadline 2\n need unit cl 5 alive",
                "playthrough 1\nsave a --x\nmilestone a\n use b\n deadline 2\n need player lv == 1",
                "playthrough 1\nfoo 3"]:
        try:
            parse_play(bad, "bad")
        except PlayError:
            continue
        raise AssertionError(f"accepted: {bad!r}")

    def snap(f, units):
        return {"k": "snap", "f": f, "seed": [0, 0], "units": units}

    def pl(lv, x=100, y=100, m=1):
        return {"ut": 0, "g": 1, "cl": 0, "m": m, "x": x, "y": y, "lv": lv}

    def mon(g, cl, m=1, lv=1):
        return {"ut": 1, "g": g, "cl": cl, "m": m, "x": 5, "y": 5, "lv": lv, "hp": 256}

    tmp = tempfile.mkdtemp(prefix="playthrough-selftest-")
    path = os.path.join(tmp, "s.jsonl")
    lines = [{"k": "header", "format": "state-1", "side": "d2rs", "fields": [], "gaps": []},
             snap(1, [pl(1), mon(1, 148)]),
             {"k": "poke", "f": 2, "i": 0, "d": "spawn", "r": "ok", "guid": 9, "src": "spawn 5 @x+3 @y normal"},
             snap(2, [pl(1), mon(1, 148), mon(9, 5)]),
             snap(3, [pl(2, 104, 101), mon(1, 148), mon(9, 5, m=0), mon(10, 7, lv=2), mon(11, 7, lv=2)]),
             {"k": "footer", "snaps": 3, "notes": []}]
    with open(path, "w") as f:
        f.write("\n".join(json.dumps(l) for l in lines) + "\n")
    _, snaps, pokes, footer = read_state(path)
    assert len(snaps) == 3 and footer["snaps"] == 3
    # town: player lv at the deadline is 2 -> wrong level
    r = evaluate(town, snaps, pokes)
    assert r["status"] == "wrong-level", r
    # kill: unit 9 dead at the deadline
    r = evaluate(kill, snaps, pokes)
    assert r["status"] == "reached" and r["frame"] == 3, r
    # an undead revival is stuck
    snaps2 = snaps + [snap(4, [pl(2), mon(9, 5, m=1)])]
    r = evaluate(kill, snaps2, pokes)
    assert r["status"] == "stuck" and "1 alive" in r["evidence"], r
    # the spawn failed: missing unit
    r = evaluate(kill, snaps, [dict(pokes[0], r="failed", guid=None)])
    assert r["status"] == "stuck" and "failed" in r["evidence"], r
    r = evaluate(kill, snaps, [dict(pokes[0], r="unresolved", guid=None)])
    assert r["status"] == "missing-unit", r
    # killed: one of the matches in a death mode
    kd = dict(kill, need=[parse_pred("unit ut 1 cl 5 killed", "t")])
    assert evaluate(kd, snaps, pokes)["status"] == "reached"
    assert evaluate(kd, snaps[:2], pokes)["status"] == "stuck"
    # walk: moved 4 >= 3, two lv-2 monsters
    r = evaluate(walk, snaps, pokes)
    assert r["status"] == "reached", r
    # count not met -> missing unit
    walk2 = dict(walk, need=[parse_pred("unit ut 1 cl 156 present", "t")])
    assert evaluate(walk2, snaps, pokes)["status"] == "missing-unit"
    # crash text
    assert first_error_line("x\nthread 'main' panicked at a.rs:1\nnote") == "thread 'main' panicked at a.rs:1"
    # report exit codes
    devnull = open(os.devnull, "w")
    old = sys.stdout
    sys.stdout = devnull
    try:
        assert report(play, [(town, {"status": "reached", "frame": 1, "evidence": ""})], "p") == 0
        assert report(play, [(town, {"status": "reached", "frame": 1, "evidence": ""}),
                             (kill, {"status": "crash", "frame": None, "evidence": "x"})], "p") == 1
    finally:
        sys.stdout = old
        devnull.close()
    try:
        read_state(os.path.join(tmp, "nope.jsonl"))
        raise AssertionError("missing file read")
    except OSError:
        pass
    # the per-act summary: consecutive count and first blocker keys
    sm = summary(play, [(town, {"status": "reached", "frame": 1, "evidence": ""}),
                        (kill, {"status": "stuck", "frame": 9, "evidence": "e"}),
                        (walk, {"status": "reached", "frame": 2, "evidence": ""})], "p")
    assert (sm["act"], sm["reached"], sm["total"], sm["consecutive"]) == (1, 2, 3, 1), sm
    assert {k: sm["first_blocker"][k] for k in ("milestone", "kind", "frame", "evidence")} == \
        {"milestone": "kill", "kind": "stuck", "frame": 9, "evidence": "e"}, sm
    # ever: held at some snapshot, not at the deadline
    ev = dict(town, need=[dict(parse_pred("player lv == 1", "t"), ever=True, src="ever player lv == 1")])
    r = evaluate(ev, snaps, pokes)
    assert r["status"] == "reached" and "at f1" in r["evidence"], r
    ev["need"][0]["ever"] = False
    assert evaluate(ev, snaps, pokes)["status"] == "wrong-level"
    # sweep: serpentine grid around the centre, back to it, absolute
    sw = sweep_pokes(10, 2, 60, 30, 1000, 2000)
    assert len(sw) == 25 and sw[0] == "10 pos @player 1030 2000", sw[0]
    assert sw[1] == "12 pos @player 1030 2030" and sw[-1] == "58 pos @player 1000 2000", sw
    gr = sweep_pokes(10, 2, 60, 30, 1000, 2000, "grid")
    assert gr[0] == "10 pos @player 940 1940" and gr[5] == "20 pos @player 1060 1970", gr
    cr = sweep_pokes(10, 1, 60, 30, 1000, 2000, "cross")
    assert [q.split(None, 1)[1] for q in cr[:4]] == ["pos @player 1030 2000", "pos @player 1060 2000",
                                                     "pos @player 1030 2000", "pos @player 1000 2000"], cr
    assert len(cr) == 16 and cr[-1] == "25 pos @player 1000 2000", cr
    pts = {tuple(map(int, q.split()[3:])) for q in sw}
    assert pts == {(1000 + 30 * i, 2000 + 30 * j) for i in range(-2, 3) for j in range(-2, 3)}, pts
    print("playthrough selftest: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
