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
import threading

VERSION = "0.3.0"
FORMAT = "playthrough 1"
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Death modes per unit type (specs/sim/units.md §1): player DT 0 / DD 17,
# monster DT 0 / DD 12.
DEAD_MODES = {0: (0, 17), 1: (0, 12)}
OPS = ("==", "!=", ">=", "<=", ">", "<")

# ------------------------------------------------- class x difficulty matrix
# (spec §4). Ids are rows of the user's 1.14d tables, read with the command
# in the spec's §4 Provenance line: skills.txt `Id` and `charclass`,
# monstats.txt `hcIdx` (the summoned unit's class), weapons.txt /
# armor.txt / misc.txt `code`.
CLASSES = ("ama", "sor", "nec", "pal", "bar", "dru", "ass")
DIFFS = ("normal", "nightmare", "hell")
# skills.txt: the class's first skill id; the save's skill byte i of the
# class list is skill FIRST + i (formats/d2s.md, d2s-tool --skill)
FIRST_SKILL = {"ama": 6, "sor": 36, "nec": 66, "pal": 96, "bar": 126, "dru": 221, "ass": 251}
# Roles a milestone can name as {role}: skill ids; {pet} / {trapunit}:
# the monstats class the summon / trap skill creates. `main` is the
# right skill of every matrix save unless the save names another role.
PROFILES = {
    "ama": {"main": 12, "summon": 32, "pet": 357,              # Multiple Shot; Valkyrie
            "gear": [("lbw", "8lb", "6lb", 4), ("aqv", "aqv", "aqv", 5)]},
    "sor": {"main": 44, "move": 54, "fire": 36,                # Frost Nova; Teleport; Fire Bolt
            "gear": [("lst", "8ls", "6ls", 4)]},
    "nec": {"main": 84, "summon": 75, "pet": 289,              # Bone Spear; Clay Golem
            "gear": [("wnd", "9wn", "7wn", 4), ("ne1", "ne6", "neb", 5)]},
    "pal": {"main": 112, "aura": 102,                          # Blessed Hammer; Holy Fire
            "gear": [("scp", "9sc", "7sc", 4), ("kit", "xit", "uit", 5)]},
    "bar": {"main": 151, "buff": 149,                          # Whirlwind; Battle Orders
            "gear": [("axe", "9ax", "7ax", 4), ("ba1", "ba6", "bab", 1)]},
    "dru": {"main": 225, "summon": 227, "pet": 420, "shift": 223,  # Firestorm; Spirit Wolf; Werewolf
            "gear": [("clb", "9cl", "7cl", 4), ("dr1", "dr6", "drb", 1)]},
    "ass": {"main": 271, "trap": 271, "trapunit": 412, "summon": 268, "pet": 417,  # Lightning Sentry; Shadow Warrior
            "gear": [("ktr", "9ar", "7ar", 4)]},
}
SKILL_ROLES = ("main", "summon", "aura", "buff", "shift", "trap", "move", "fire")
# per difficulty: character level, base stats str/ene/dex/vit (0-3), life
# and mana (6-9, whole points; the save stores 1/256), body armour code
TIERS = {
    "normal": {"level": 30, "base": (60, 50, 60, 100), "life": 500, "mana": 300, "armor": "qui"},
    "nightmare": {"level": 60, "base": (120, 80, 120, 200), "life": 1500, "mana": 600, "armor": "xui"},
    "hell": {"level": 85, "base": (180, 100, 180, 300), "life": 3000, "mana": 1000, "armor": "uui"},
}
# A finished difficulty (world/quests.md §1.8 r4, §1.9): bit 0 of every
# quest slot §1.9 lists, and the act transitions (§8.1, d2s-tool acts=4)
DONE_SLOTS = (1, 2, 4, 6, 9, 10, 11, 12, 13, 14, 17, 18, 19, 20, 21, 22,
              25, 26, 27, 35, 36, 37, 38, 39, 40)
# save flags the matrix replaces (spec §4 r3); a value with '{' is a role
# placeholder and is kept
CELL_FLAGS = ("--class", "--name", "--level", "--skill", "--all-skills", "--stat", "--item",
              "--left-skill", "--right-skill", "--difficulty", "--difficulty-unlocked")


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
                "checkpoint": None,
                "seed": 1,
                "ticks": None,
                "difficulty": None,
                "pokes": [],
                "input": None,
                "need": [],
                "sweep": None,
                "find": None,
                "note": "",
                "only": {},
            }
            play["milestones"].append(cur)
        else:
            if cur is None:
                raise PlayError(f"{where}: '{word}' outside a milestone")
            if word == "use":
                if rest not in play["saves"]:
                    raise PlayError(f"{where}: unknown save '{rest}'")
                cur["save"] = rest
            elif word == "checkpoint":
                # spec §1 r5: the checkpoint's save and start pokes (traces/checkpoints/)
                if not re.fullmatch(r"[a-z0-9-]+", rest):
                    raise PlayError(f"{where}: checkpoint <name>")
                cur["checkpoint"] = rest
            elif word == "goto":
                # spec §1 r6: `goto <frame> unit ...|preset ...` = `poke <frame> goto ...`
                f, _, d = rest.partition(" ")
                _int(f[1:] if f.startswith("+") else f, where)
                if not d.strip().startswith(("unit ", "preset ")):
                    raise PlayError(f"{where}: goto <frame> unit [<type>:]<class> | preset <level> [<type>:]<class>")
                cur["pokes"].append(f"{f} goto {d.strip()}")
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
            elif word == "only":
                what, _, lst = rest.partition(" ")
                vals = [v for v in lst.replace(" ", "").split(",") if v]
                known = {"class": CLASSES, "difficulty": DIFFS}.get(what)
                if known is None or not vals or any(v not in known for v in vals):
                    raise PlayError(f"{where}: only class <{'|'.join(CLASSES)},...> | "
                                    f"only difficulty <{'|'.join(DIFFS)},...>")
                cur["only"][what] = vals
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
        if (m["save"] is None) == (m["checkpoint"] is None):
            raise PlayError(f"{w}: exactly one of 'use <save>' and 'checkpoint <name>'")
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


def sweep_targets(f0, every, radius, step, cx, cy, mode="spiral"):
    """The sweep's target points (spec §1 r4); sweep_pokes walks them in hops.
    Original form: absolute `pos @player` pokes from
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
        return [(px, py) for px, py in pts if px >= 0 and py >= 0]
    if mode == "grid":
        # rows of the square, serpentine, from the (-R, -R) corner
        pts = []
        for row in range(-n, n + 1):
            xs = range(-n, n + 1) if (row + n) % 2 == 0 else range(n, -n - 1, -1)
            pts += [(cx + i * step, cy + row * step) for i in xs]
        pts.append((cx, cy))
        return [(px, py) for px, py in pts if px >= 0 and py >= 0]
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
    return [(px, py) for px, py in pts if px >= 0 and py >= 0]


HOP = 16  # largest move of one `hop` poke per axis (poke.md §1, d2_sim::poke::HOP)


def sweep_pokes(f0, every, radius, step, cx, cy, mode="spiral"):
    """`sweep F0 EVERY R STEP` (spec §1 r4): the targets of sweep_targets,
    walked with `hop @player x y` pokes, one every EVERY frames. A hop
    moves the player at most HOP sub-tiles per axis toward the target, to
    the first free spot around the step (poke.md §1 `hop`), from wherever
    the player stands; a blocked hop leaves it there and the next one
    tries again. Each leg gets ceil(distance / HOP) + 1 hops."""
    pts = sweep_targets(f0, every, radius, step, cx, cy, mode)
    out, k, prev = [], 0, (cx, cy)
    for tx, ty in pts:
        n = -(-max(abs(tx - prev[0]), abs(ty - prev[1])) // HOP) + 1
        for _ in range(n):
            out.append(f"{f0 + k * every} hop @player {tx} {ty}")
            k += 1
        prev = (tx, ty)
    return out


def _rel(base, d):
    return base if d == 0 else f"{base}{d:+d}"


def parse_pred(text, where):
    """`player <field> <op> <n>` | `player moved >= <n>` |
    `unit <filters> present|absent|dead|count <op> <n>` |
    `quest <slot> <bit> set|clear` (spec §2)."""
    t = text.split()
    if not t:
        raise PlayError(f"{where}: empty predicate")
    if t[0] == "player" and len(t) >= 2 and t[-2] == "since":
        # `... since F`: moved / delta measured from the player at frame F
        pred = parse_pred(" ".join(t[:-2]), where)
        if not (pred.get("delta") or pred.get("field") == "moved"):
            raise PlayError(f"{where}: 'since F' only with 'player moved' or 'player <field> delta'")
        pred["since"] = _int(t[-1], where)
        pred["src"] = text
        return pred
    if t[0] == "quest":
        if len(t) != 4 or t[3] not in ("set", "clear"):
            raise PlayError(f"{where}: quest <slot> <bit> set|clear")
        slot, bit = _int(t[1], where), _int(t[2], where)
        if not (0 <= slot <= 41 and 0 <= bit <= 15):
            raise PlayError(f"{where}: quest slot 0-41, bit 0-15")
        return {"kind": "quest", "slot": slot, "bit": bit, "want": t[3] == "set", "src": text}
    if t[0] == "player" and len(t) == 5 and t[2] == "delta":
        if t[3] not in OPS:
            raise PlayError(f"{where}: player <field> delta <op> <n>")
        return {"kind": "player", "field": t[1], "delta": True, "op": t[3], "value": _int(t[4], where), "src": text}
    if t[0] == "player":
        if len(t) != 4 or t[2] not in OPS:
            raise PlayError(f"{where}: player <field> <op> <n>")
        return {"kind": "player", "field": t[1], "op": t[2], "value": _int(t[3], where), "src": text}
    if t[0] == "unit":
        filt = {}
        i = 1
        while i < len(t) and t[i] in ("ut", "cl", "lv", "g", "lvl"):
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
        raise PlayError(f"{where}: unit [ut|cl|lv|lvl|g V]... present|absent|seen|dead|killed|count <op> <n>")
    raise PlayError(f"{where}: predicate must start with player, unit or quest")


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


def quest_bit(q, slot, bit):
    """Bit `bit` of quest slot `slot` in a player's `q` (state-snapshot.md
    §2: [slot, word] for the non-zero slots; a slot not listed is 0)."""
    for s_, word in q:
        if s_ == slot:
            return bool(word >> bit & 1)
    return False


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


def _prefix(pred, snaps, guids, cache):
    """Per snapshot, how many snapshots up to and including it hold a unit matching
    the predicate's filter (built once per predicate: the `seen` / `dead` tests and
    the `ever` loops would otherwise rescan the prefix at every snapshot)."""
    key = ("seen", id(pred))
    if key not in cache:
        out, n = [], 0
        for s_ in snaps:
            n += any(_matches(u, pred["filter"], guids) for u in s_["units"])
            out.append(n)
        cache[key] = out
    return cache[key]


def eval_pred(pred, snaps, idx, guids, first_player=None, cache=None):
    """Evaluates `pred` at snaps[idx]. Returns (ok, evidence)."""
    if cache is None:
        cache = {}
    snap = snaps[idx]
    if pred["kind"] == "quest":
        p = player_of(snap)
        if p is None:
            return False, "no player unit"
        if "q" not in p:
            return False, "player has no 'q' (no quest record in the state file)"
        on = quest_bit(p["q"], pred["slot"], pred["bit"])
        return on == pred["want"], f"quest {pred['slot']}.{pred['bit']} {'set' if on else 'clear'}"
    if pred["kind"] == "player":
        p = player_of(snap)
        if p is None:
            return False, "no player unit"
        if "since" in pred:
            k = ("since", id(pred))
            if k not in cache:
                cache[k] = next((i for i, s_ in enumerate(snaps) if s_["f"] >= pred["since"]
                                 and (q := player_of(s_)) and "x" in q), None)
            i0 = cache[k]
            first_player = player_of(snaps[i0]) if i0 is not None and i0 <= idx else None
        if pred.get("delta"):
            v0 = (first_player or {}).get(pred["field"])
            v1 = p.get(pred["field"])
            if v0 is None or v1 is None:
                return False, f"player has no '{pred['field']}'"
            v = v1 - v0
            return _cmp(v, pred["op"], pred["value"]), f"player {pred['field']} {v0} -> {v1} (delta {v:+d})"
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
        pre = _prefix(pred, snaps, guids, cache)
        n = pre[idx]
        first = next((snaps[i]["f"] for i in range(idx + 1) if pre[i]), None) if n else None
        return n > 0, f"seen in {n} snapshots" + (f" from f{first}" if first is not None else "")
    if test == "absent":
        return not hit, f"{len(hit)} matching"
    if test == "count":
        return _cmp(len(hit), pred["op"], pred["value"]), f"{len(hit)} matching"
    if test == "killed":
        dead = [u for u in hit if u.get("m") in DEAD_MODES.get(u.get("ut"), (0,))]
        return bool(dead), f"{len(dead)} of {len(hit)} matching in a death mode"
    # dead: seen alive-or-dead before, and now every match in a death mode or gone
    seen = _prefix(pred, snaps, guids, cache)[idx] > 0
    if not seen:
        return False, "never present"
    alive = [u for u in hit if u.get("m") not in DEAD_MODES.get(u.get("ut"), (0,))]
    if alive:
        u = alive[0]
        return False, (f"{len(alive)} alive, e.g. ut {u.get('ut')} g {u.get('g')} cl {u.get('cl')} "
                       f"m {u.get('m')} hp {u.get('hp')} at ({u.get('x')},{u.get('y')})")
    return True, f"{len(hit)} dead, {'gone' if not hit else 'in death modes'}"


def eval_final(p, snaps, guids, first_player, cache=None):
    """A predicate at the deadline (the last snapshot); an `ever`
    predicate holds when it held at any snapshot (spec §2)."""
    if p.get("ever"):
        for i in range(len(snaps)):
            ok, e = eval_pred(p, snaps, i, guids, first_player, cache)
            if ok:
                return True, f"{e} at f{snaps[i]['f']}"
        return False, "never held; at deadline " + eval_pred(p, snaps, len(snaps) - 1, guids, first_player, cache)[1]
    return eval_pred(p, snaps, len(snaps) - 1, guids, first_player, cache)


def evaluate(m, snaps, pokes):
    """A milestone's verdict on its state file (spec §3). Returns dict
    with status reached|stuck|missing-unit|wrong-level, frame, evidence."""
    if not snaps:
        return {"status": "stuck", "frame": None, "evidence": "no snapshot"}
    guids, cache = poke_guids(pokes), {}
    first_player = next((p for p in map(player_of, snaps) if p and "x" in p), None)
    first_all = None
    for i in range(len(snaps)):
        if all(eval_pred(p, snaps, i, guids, first_player, cache)[0] for p in m["need"] if not p.get("ever")):
            first_all = snaps[i]["f"]
            break
    last = len(snaps) - 1
    results = [(p, *eval_final(p, snaps, guids, first_player, cache)) for p in m["need"]]
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
    failed_pokes = [q for q in pokes if q.get("r") != "ok" and not str(q.get("src", "")).startswith(("pos @player ", "hop @player "))]
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


def load_checkpoint(name):
    """The parsed definition traces/checkpoints/<name>.checkpoint (through
    tools/checkpoints/make.py)."""
    sys.path.insert(0, os.path.join(REPO, "tools", "checkpoints"))
    try:
        import make as ckmake
    finally:
        sys.path.pop(0)
    path = os.path.join(ckmake.DEFS, f"{name}.checkpoint")
    try:
        with open(path, encoding="utf-8") as f:
            ck = ckmake.parse(f.read(), os.path.basename(path))
    except OSError:
        raise PlayError(f"no checkpoint '{name}' ({path})") from None
    except ckmake.CkError as e:
        raise PlayError(str(e)) from None
    return ck, ckmake.d2s_args(ck)


def run_milestone(m, play, client, d2s, work, game_dir):
    if m["checkpoint"]:
        ck, args = load_checkpoint(m["checkpoint"])
        save = os.path.join(work, f"ck-{m['checkpoint']}.d2s")
        if m["difficulty"] is None:
            m["difficulty"] = ck["difficulty"]
        # the checkpoint's start pokes first (their indices are @p0...), then the milestone's
        start = [f"{f} {d}" for f, d in ck["start"]]
        if not m.get("_started"):
            m["pokes"] = start + m["pokes"]
            m["_started"] = True
    else:
        save = os.path.join(work, f"{m['save']}.d2s")
        args = play["saves"][m["save"]]
    if not os.path.exists(save):
        cmd = [d2s, "new"] + args + ["-o", save]
        r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir))
        if r.returncode != 0:
            raise PlayError(f"save {m['save'] or m['checkpoint']}: {first_error_line(r.stderr + r.stdout)}")
    pokes = [p for p in m["pokes"] if not p.startswith("+")]
    pokes.sort(key=lambda p: int(p.split()[0]))
    script = m["input"]
    if any(PREF.search(p) for p in pokes):
        pokes = resolve_poke_refs(m, pokes, save, client, work, game_dir)
        if isinstance(pokes, dict):
            return pokes
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
        sw = sweep_pokes(f0, every, radius, step, pl["x"], pl["y"], mode)
        pokes += sw
        pokes.sort(key=lambda p: int(p.split()[0]))
        if sw:  # the run covers the whole sweep (spec §1 r4)
            m["run_ticks"] = max(m.get("run_ticks", m["ticks"]), int(sw[-1].split()[0]) + 10)
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


# `@pI` in a poke: the unit poke I of the milestone created (spec §4 r8)
PREF = re.compile(r"@p(\d+)\b")
# unit type of what a creating directive makes (poke.md §1 r2)
POKE_TYPES = {"spawn": 1, "superunique": 1, "object": 2, "missile": 3, "item": 4}


def resolve_poke_refs(m, pokes, save, client, work, game_dir):
    """`@pI` (spec §4 r8): a probe run up to the frame before the first
    poke that names one finds the GUIDs the earlier pokes created; each
    `@pI` becomes `<type>/<guid>` (poke.md §1 r1). Pokes are taken in file
    order, which must also be frame order up to that point."""
    f0 = min(int(p.split()[0]) for p in pokes if PREF.search(p))
    before = [p for p in pokes if int(p.split()[0]) < f0]
    if before != pokes[: len(before)]:
        raise PlayError(f"milestone {m['name']}: '@pI' needs the earlier pokes in file and frame order")
    probe = os.path.join(work, f"{m['name']}.ref.jsonl")
    cmd = [client, "state-dump", "--save", save, "--seed", str(m["seed"]), "--ticks", str(f0 - 1),
           "--out", probe, "--date", "2026-01-01"] + _flags(m, before)
    r = subprocess.run(cmd, capture_output=True, text=True, env=_env(game_dir), timeout=900)
    if r.returncode != 0:
        m["cmd"] = " ".join(shlex.quote(c) for c in cmd)
        return {"status": "crash", "frame": None, "evidence": f"@p probe exit {r.returncode}: {first_error_line(r.stderr)}"}
    _, _, ppokes, _ = read_state(probe)
    guids = poke_guids(ppokes)

    def one(g):
        i = int(g.group(1))
        if i >= len(before) or i not in guids:
            raise PlayError(f"milestone {m['name']}: @p{i}: poke {i} created no unit before frame {f0}")
        t = POKE_TYPES.get(before[i].split()[1])
        if t is None:
            raise PlayError(f"milestone {m['name']}: @p{i}: poke {i} ({before[i].split()[1]}) creates no unit")
        return f"{t}/{guids[i]}"
    try:
        return [PREF.sub(one, p) for p in pokes]
    except PlayError as e:
        return {"status": "missing-unit", "frame": f0, "evidence": str(e)}


def find_probe(m, pokes, save, client, work, game_dir):
    """`find` (spec §1 r5): runs the whole sweep to the deadline and
    returns the first frame at which a unit matching the filter is
    present, or a blocker verdict when none ever is."""
    probe = os.path.join(work, f"{m['name']}.find.jsonl")
    ticks = m.get("run_ticks", m["ticks"])  # the whole sweep (spec §1 r4), not just the deadline
    cmd = [client, "state-dump", "--save", save, "--seed", str(m["seed"]), "--ticks", str(ticks),
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
    `hop` / `pos` pokes of the player are dropped), `poke +N` runs at F + N
    and `frame +N` of the input script is frame F + N."""
    keep = [p for p in pokes if not (p.split()[1:3] in (["hop", "@player"], ["pos", "@player"])
                                     and int(p.split()[0]) > found)]
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


# ----------------------------------------------------------------- matrix


def save_class(args):
    """The class code of a save's `--class` (d2s-tool: a code or 0..6)."""
    for i, a in enumerate(args[:-1]):
        if a == "--class":
            v = args[i + 1]
            return CLASSES[int(v)] if v.isdigit() and int(v) < 7 else v
    return None


def resolve(text, cls, where):
    """`{role}` placeholders (spec §4 r4) -> the class profile's value."""
    prof = PROFILES.get(cls)

    def one(g):
        key = g.group(1)
        if prof is None:
            raise PlayError(f"{where}: {{{key}}} needs a class (the save's --class or --class)")
        if key not in prof or key == "gear":
            raise PlayError(f"{where}: class {cls} has no '{key}' (add 'only class' to the milestone)")
        return str(prof[key])
    return re.sub(r"\{([a-z]+)\}", one, text)


def cell_save_args(args, cls, diff, sname):
    """A save's d2s-tool arguments for the matrix cell (spec §4 r3): the
    class profile's class, level, stats, role skills, mouse skills and
    gear, the cell's difficulty (town byte, unlock) and the finished lower
    difficulties' quest bits; --quests, --waypoints, --act, --gold,
    --expansion and the rest are kept. A --left-skill / --right-skill
    whose value is a {role} placeholder is kept."""
    d = DIFFS.index(diff)
    prof, tier = PROFILES[cls], TIERS[diff]
    out, quests, mouse = [], None, {}
    i = 0
    while i < len(args):
        a = args[i]
        if a in CELL_FLAGS or a == "--quests":
            if i + 1 >= len(args):
                raise PlayError(f"save {sname}: {a} needs a value")
            v = args[i + 1]
            i += 2
            if a == "--quests":
                quests = v
            elif a in ("--left-skill", "--right-skill") and "{" in v:
                mouse[a] = v
            continue
        out.append(a)
        i += 1
    expansion = "--expansion" in out
    name = re.sub(r"[^A-Za-z]", "", f"Pt{cls.capitalize()}{diff[0].upper()}{sname.capitalize()}")[:15]
    out += ["--class", cls, "--name", name, "--level", str(tier["level"])]
    for sid, v in zip((0, 1, 2, 3), tier["base"]):
        out += ["--stat", f"{sid}={v}"]
    for sid, v in ((6, tier["life"]), (7, tier["life"]), (8, tier["mana"]), (9, tier["mana"])):
        out += ["--stat", f"{sid}={v * 256}"]
    for sk in sorted({prof[r] for r in SKILL_ROLES if r in prof}):
        out += ["--skill", f"{sk - FIRST_SKILL[cls]}=20"]
    out += ["--right-skill", mouse.get("--right-skill", str(prof["main"])),
            "--left-skill", mouse.get("--left-skill", "0")]
    for *codes, body in prof["gear"]:
        out += ["--item", f"{codes[d]}/body={body}"]
    out += ["--item", f"{tier['armor']}/body=3"]
    if d > 0:
        out += ["--difficulty-unlocked", diff]
        done = []
        for lower in DIFFS[:d]:
            done.append(f"{lower}:acts={4 if expansion else 3}")
            done += [f"{lower}:{slot}.0" for slot in DONE_SLOTS if expansion or slot < 29]
        quests = ",".join(([quests] if quests and quests != "none" else []) + done)
    if quests:
        out += ["--quests", quests]
    out += ["--difficulty", diff]
    return out


def cell_text(text, name, cls, diff):
    """The objective file of one cell (spec §4 r2, r4): milestones whose
    `only` lines exclude the cell are dropped, then the saves no milestone
    uses, then every `{role}` is resolved; with `cls` set (matrix), each
    save's arguments become the cell's (`cell_save_args`). `cls` None:
    each milestone's class is its save's --class and its difficulty its
    `difficulty` line (default normal). Returns the text to parse."""
    lines = text.splitlines()
    saves = {}
    for ln in lines:
        t = ln.split("#", 1)[0].split()
        if len(t) >= 2 and t[0] == "save":
            saves[t[1]] = shlex.split(ln.split("#", 1)[0])[2:]
    head, blocks, cur = [], [], None
    for ln in lines:
        t = ln.split("#", 1)[0].split()
        if t and t[0] == "milestone":
            cur = [ln]
            blocks.append(cur)
        elif cur is not None:
            cur.append(ln)
        else:
            head.append(ln)
    keep, used = [], set()
    for b in blocks:
        kv = {}
        for ln in b:
            t = ln.split("#", 1)[0].split(None, 1)
            if t and t[0] in ("use", "difficulty", "only") and len(t) > 1:
                if t[0] == "only":
                    w, _, lst = t[1].partition(" ")
                    kv.setdefault("only", {})[w] = lst.replace(" ", "").split(",")
                else:
                    kv[t[0]] = t[1].strip()
        bc = cls or save_class(saves.get(kv.get("use"), []))
        bd = diff or kv.get("difficulty", "normal")
        only = kv.get("only", {})
        if bc is not None and "class" in only and bc not in only["class"]:
            continue
        if "difficulty" in only and bd not in only["difficulty"]:
            continue
        where = f"{name} milestone {b[0].split()[1] if len(b[0].split()) > 1 else '?'}"
        keep.append("\n".join(resolve(ln, bc, where) for ln in b))
        used.add(kv.get("use"))
    out = []
    for ln in head:
        t = ln.split("#", 1)[0].split()
        if len(t) >= 2 and t[0] == "save":
            if t[1] not in used:
                continue
            args = saves[t[1]]
            sc = cls or save_class(args)
            if cls:
                args = cell_save_args(args, cls, diff, t[1])
            ln = f"save {t[1]} " + resolve(shlex.join(args), sc, f"{name} save {t[1]}")
        out.append(ln)
    return "\n".join(out + keep) + "\n"


def run_cell(path, text, cls, diff, client, d2s, base, game_dir, only=None):
    """One (objective file, class, difficulty) cell: parse, run each
    milestone, summarize. Returns the summary with `class`,
    `difficulty` and per-milestone results."""
    play = parse_play(cell_text(text, path, cls, diff), path)
    ms = play["milestones"]
    if only:
        ms = [m for m in ms if m["name"] in only]
    if cls:
        for m in ms:
            m["difficulty"] = diff
    work = os.path.join(base, f"act{play['act']}-{os.path.splitext(os.path.basename(path))[0]}-{cls or 'own'}-{diff or 'own'}")
    os.makedirs(work, exist_ok=True)
    results = []
    for m in ms:
        try:
            r = run_milestone(m, play, client, d2s, work, game_dir)
        except PlayError as e:
            # a save d2s-tool refuses is this cell's blocker, not a harness error
            r = {"status": "no-save", "frame": None, "evidence": str(e)}
        results.append((m, r))
    sm = summary(play, results, path)
    sm.update({"class": cls, "difficulty": diff, "work": work,
               "milestones": [{"name": m["name"], "command": m.get("cmd"), **r} for m, r in results]})
    return play, results, sm


def print_matrix(cells, out=sys.stdout):
    """Per difficulty: act rows, class columns, reached/total."""
    acts = sorted({(c["act"], c["play"]) for c in cells})
    classes = [c for c in CLASSES if any(x["class"] == c for x in cells)]
    for diff in [d for d in DIFFS if any(x["difficulty"] == d for x in cells)]:
        print(f"{diff}:", file=out)
        print(f"  {'act / file':<28}" + "".join(f"{c:>9}" for c in classes), file=out)
        for act, play in acts:
            row = f"  {str(act) + ' ' + os.path.basename(play):<28}"
            for c in classes:
                x = next((x for x in cells if (x["act"], x["play"], x["class"], x["difficulty"]) == (act, play, c, diff)), None)
                row += f"{(str(x['reached']) + '/' + str(x['total'])) if x else '-':>9}"
            print(row, file=out)


def blockers(cells):
    """Distinct blockers over every cell (spec §4 r6): (file, milestone,
    status) -> the cells it blocks, with one cell's evidence and command."""
    out = {}
    for c in cells:
        for m in c["milestones"]:
            if m["status"] == "reached":
                continue
            k = (os.path.basename(c["play"]), m["name"], m["status"])
            e = out.setdefault(k, {"file": k[0], "milestone": k[1], "status": k[2], "cells": [],
                                   "evidence": m["evidence"], "command": m.get("command")})
            e["cells"].append(f"{c['class']}/{c['difficulty']}")
    return sorted(out.values(), key=lambda e: (e["file"], e["milestone"]))


def markdown(cells):
    """The matrix as Markdown (docs/handoff/playability-matrix.md body)."""
    lines = []
    classes = [c for c in CLASSES if any(x["class"] == c for x in cells)]
    acts = sorted({(c["act"], c["play"]) for c in cells})
    for diff in [d for d in DIFFS if any(x["difficulty"] == d for x in cells)]:
        lines += [f"### {diff.capitalize()}", "",
                  "| act (file) | " + " | ".join(classes) + " |",
                  "|---|" + "---|" * len(classes)]
        for act, play in acts:
            row = []
            for c in classes:
                x = next((x for x in cells if (x["act"], x["play"], x["class"], x["difficulty"]) == (act, play, c, diff)), None)
                if not x:
                    row.append("-")
                    continue
                fb = x["first_blocker"]
                row.append(f"{x['reached']}/{x['total']}" + (f" `{fb['milestone']}`" if fb else ""))
            lines.append(f"| {act} ({os.path.basename(play)}) | " + " | ".join(row) + " |")
        lines.append("")
    return "\n".join(lines)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("play", nargs="*", help="objective files (traces/playthrough/actN.play)")
    ap.add_argument("--only", help="comma-separated milestone names")
    ap.add_argument("--work", help="work dir (default: a temp dir under target/playthrough)")
    ap.add_argument("--game-dir", default=os.environ.get("D2_GAME_DIR"))
    ap.add_argument("--build", action="store_true", help="cargo build --release d2-client and d2s-tool first")
    ap.add_argument("--all", action="store_true", help="every traces/playthrough/*.play, then a per-act table")
    ap.add_argument("--class", dest="classes",
                    help="matrix: comma-separated classes (ama,sor,nec,pal,bar,dru,ass or 'all'); "
                         "each save becomes the class profile's (spec §4)")
    ap.add_argument("--difficulty", dest="diffs",
                    help="matrix: comma-separated difficulties (normal,nightmare,hell or 'all')")
    ap.add_argument("--jobs", type=int, default=1, help="matrix cells run in parallel")
    ap.add_argument("--json", help="also write the results as JSON here")
    ap.add_argument("--markdown", help="matrix: also write the act x class tables as Markdown here")
    ap.add_argument("--profiles", action="store_true", help="print the class profiles' save arguments and exit")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    try:
        classes = _list(a.classes, CLASSES, "--class")
        diffs = _list(a.diffs, DIFFS, "--difficulty")
    except PlayError as e:
        ap.error(str(e))
    matrix = bool(classes or diffs)
    if matrix:
        classes = classes or list(CLASSES)
        diffs = diffs or ["normal"]
    if a.profiles:
        for c in classes or CLASSES:
            for d in diffs or DIFFS:
                print(f"{c} {d}: d2s-tool new " + shlex.join(cell_save_args(["--expansion"], c, d, "x")))
        return 0
    if a.all:
        d = os.path.join(REPO, "traces", "playthrough")
        a.play += sorted(os.path.join(d, f) for f in os.listdir(d) if f.endswith(".play"))
    if not a.play:
        ap.error("an objective file is needed (or --all)")
    try:
        client, d2s = tool_paths(a)
        base = a.work or tempfile.mkdtemp(prefix="run-", dir=_mk(os.path.join(REPO, "target", "playthrough")))
        os.makedirs(base, exist_ok=True)
        texts = {}
        for path in a.play:
            with open(path, encoding="utf-8") as f:
                texts[path] = f.read()
            parse_play(cell_text(texts[path], path, None, None), path)  # objective-file errors first
        only = set(a.only.split(",")) if a.only else None
        if not matrix:
            worst, all_json = 0, []
            for path in a.play:
                play, results, sm = run_cell(path, texts[path], None, None, client, d2s, base, a.game_dir, only)
                worst = max(worst, report(play, results, path))
                print(f"work dir: {sm['work']}")
                all_json.append(sm)
            if len(all_json) > 1 or a.all:
                print()
                print_table(all_json)
            if a.json:
                _write_json(a.json, "playthrough-result-1", {"acts": all_json})
            return worst
        jobs = [(p, c, d) for p in a.play for c in classes for d in diffs]
        cells = []
        lock = threading.Lock()

        def one(job):
            p, c, d = job
            _, _, sm = run_cell(p, texts[p], c, d, client, d2s, base, a.game_dir, only)
            if a.json:
                # each finished cell at once, so a long run that stops early keeps its cells
                with lock, open(a.json + ".cells.jsonl", "a", encoding="utf-8") as f:
                    f.write(json.dumps(sm) + "\n")
            print(f"  {os.path.basename(p)} {c} {d}: {sm['reached']}/{sm['total']}"
                  + (f", first blocker {sm['first_blocker']['milestone']} ({sm['first_blocker']['kind']})"
                     if sm["first_blocker"] else ""), file=sys.stderr, flush=True)
            return sm
        if a.jobs > 1:
            from concurrent.futures import ThreadPoolExecutor
            with ThreadPoolExecutor(a.jobs) as ex:
                cells = list(ex.map(one, jobs))
        else:
            cells = [one(j) for j in jobs]
        print(f"playthrough {VERSION}: matrix {len(a.play)} file(s) x {len(classes)} class(es) x {len(diffs)} difficulty(ies)")
        print_matrix(cells)
        print()
        print("distinct blockers (file, milestone, status: cells):")
        bl = blockers(cells)
        for b in bl:
            ev = b["evidence"] if len(b["evidence"]) <= 200 else b["evidence"][:197] + "..."
            print(f"  {b['file']} {b['milestone']} ({b['status']}): {len(b['cells'])} cells "
                  f"[{', '.join(b['cells'][:6])}{', ...' if len(b['cells']) > 6 else ''}]: {ev}")
        print(f"work dir: {base}")
        if a.json:
            _write_json(a.json, "playthrough-matrix-1", {"classes": classes, "difficulties": diffs,
                                                          "cells": cells, "blockers": bl})
        if a.markdown:
            with open(a.markdown, "w", encoding="utf-8") as f:
                f.write(markdown(cells))
        return 1 if bl else 0
    except (PlayError, OSError) as e:
        print(f"playthrough: error: {e}", file=sys.stderr)
        return 3


def _list(v, known, flag):
    if not v:
        return None
    if v == "all":
        return list(known)
    out = [x for x in v.replace(" ", "").split(",") if x]
    bad = [x for x in out if x not in known]
    if bad:
        raise PlayError(f"{flag}: unknown {', '.join(bad)} (want {','.join(known)} or all)")
    return out


def _mk(d):
    os.makedirs(d, exist_ok=True)
    return d


def _write_json(path, fmt, body):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(dict({"format": fmt, "tool": f"playthrough.py {VERSION}"}, **body), f, indent=1)


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
    # quest: bit b of slot s in the player's `q` ([slot, word], non-zero
    # slots only), as a fixed 96-byte record would hold it
    rec = bytearray(96)
    rec[2:4] = (0x2002).to_bytes(2, "little")   # slot 1: bits 1, 13
    rec[12:14] = (0x0001).to_bytes(2, "little")  # slot 6: bit 0
    rec[82:84] = (0x8000).to_bytes(2, "little")  # slot 41: bit 15
    q = [[k, int.from_bytes(rec[2 * k:2 * k + 2], "little")] for k in range(42)
         if rec[2 * k:2 * k + 2] != b"\0\0"]
    assert q == [[1, 0x2002], [6, 1], [41, 0x8000]], q
    for slot, bit, on in ((1, 1, True), (1, 13, True), (1, 0, False), (6, 0, True), (41, 15, True),
                          (41, 14, False), (0, 0, False), (7, 0, False)):
        assert quest_bit(q, slot, bit) is on, (slot, bit)
    qp = parse_pred("quest 6 0 set", "t")
    assert (qp["kind"], qp["slot"], qp["bit"], qp["want"]) == ("quest", 6, 0, True), qp
    assert parse_pred("quest 41 15 clear", "t")["want"] is False
    lp = parse_pred("unit ut 1 lvl 36,37 present", "t")
    assert lp["filter"] == {"ut": [1], "lvl": [36, 37]}, lp
    assert _matches({"ut": 1, "lvl": 36}, lp["filter"], {}) and not _matches({"ut": 1, "lvl": 1}, lp["filter"], {})
    for bad in ("quest 42 0 set", "quest 1 16 set", "quest 1 0 on", "quest 1 set", "quest -1 0 set",
                "quest 1 0 set x"):
        try:
            parse_pred(bad, "t")
        except PlayError:
            continue
        raise AssertionError(f"accepted: {bad!r}")
    qsn = [snap(1, [dict(pl(1), q=[])]), snap(2, [dict(pl(1), q=q)]), snap(3, [dict(pl(1), q=[[1, 0x2002]])])]
    qm = dict(town, need=[parse_pred("quest 6 0 set", "t")])
    r = evaluate(qm, qsn, [])
    assert r["status"] == "stuck" and "quest 6.0 clear" in r["evidence"], r
    qm["need"] = [dict(parse_pred("quest 6 0 set", "t"), ever=True)]
    r = evaluate(qm, qsn, [])
    assert r["status"] == "reached" and "at f2" in r["evidence"], r
    qm["need"] = [parse_pred("quest 1 0 clear", "t"), parse_pred("quest 1 1 set", "t")]
    assert evaluate(qm, qsn, [])["status"] == "reached"
    r = evaluate(qm, [snap(1, [pl(1)])], [])
    assert r["status"] == "stuck" and "no 'q'" in r["evidence"], r
    qplay = parse_play("playthrough 1\nsave a --x\nmilestone a\n use a\n deadline 2\n"
                       " need ever quest 7 0 set\n need quest 1 13 clear\n", "q")
    qa, qb = qplay["milestones"][0]["need"]
    assert qa["ever"] and (qa["slot"], qa["bit"], qa["want"]) == (7, 0, True), qa
    assert not qb["ever"] and (qb["slot"], qb["bit"], qb["want"]) == (1, 13, False), qb
    # sweep: serpentine grid around the centre, back to it, absolute
    sw = sweep_targets(10, 2, 60, 30, 1000, 2000)
    assert len(sw) == 25 and sw[0] == (1030, 2000) and sw[1] == (1030, 2030), sw
    assert sw[-1] == (1000, 2000), sw
    gr = sweep_targets(10, 2, 60, 30, 1000, 2000, "grid")
    assert gr[0] == (940, 1940) and gr[5] == (1060, 1970), gr
    cr = sweep_targets(10, 1, 60, 30, 1000, 2000, "cross")
    assert cr[:4] == [(1030, 2000), (1060, 2000), (1030, 2000), (1000, 2000)], cr
    assert len(cr) == 16 and cr[-1] == (1000, 2000), cr
    assert set(sw) == {(1000 + 30 * i, 2000 + 30 * j) for i in range(-2, 3) for j in range(-2, 3)}
    # hops: one `hop` per frame toward each target, ceil(d / HOP) + 1 per leg
    hp = sweep_pokes(10, 2, 60, 30, 1000, 2000)
    assert hp[0] == "10 hop @player 1030 2000" and hp[1] == "12 hop @player 1030 2000", hp[:3]
    assert hp[3] == "16 hop @player 1030 2030", hp[:4]
    assert len({q.split()[0] for q in hp}) == len(hp) and hp[-1].endswith("hop @player 1000 2000")
    assert sum(1 for q in hp if q.endswith(" 1030 2000")) == 3  # 30 away: 2 hops + 1
    # a find at frame 14 drops the later sweep hops, keeps the rest, places +N
    mf = {"pokes": ["5 warp 83", "+2 pos @1:345 @x+3 @y"], "input": None, "ticks": 100}
    kept, _ = shift_after_find(mf, ["5 warp 83"] + hp, 14)
    assert [q for q in kept if " hop " in q] == hp[:3], kept[:6]
    assert "16 pos @1:345 @x+3 @y" in kept and mf["run_ticks"] == 100, (kept, mf)
    # checkpoint start and goto steps (spec §1 r5-r6)
    cp = parse_play("playthrough 1\nmilestone a\n checkpoint a4-hellforge\n deadline 300\n"
                    " goto 30 unit 409\n need unit ut 1 cl 409 g @p2 present\n", "c")
    cm = cp["milestones"][0]
    assert cm["checkpoint"] == "a4-hellforge" and cm["save"] is None and cm["pokes"] == ["30 goto unit 409"], cm
    ck, args = load_checkpoint("a4-hellforge")
    assert ck["start"][0] == (5, "warp 107") and "--act" in args, ck["start"]
    for bad in ("playthrough 1\nsave a --x\nmilestone a\n use a\n checkpoint b\n deadline 2\n need player lv == 1",
                "playthrough 1\nmilestone a\n deadline 2\n need player lv == 1",
                "playthrough 1\nmilestone a\n checkpoint A_B\n deadline 2\n need player lv == 1",
                "playthrough 1\nmilestone a\n checkpoint x\n deadline 2\n goto 5 here 3\n need player lv == 1",
                "playthrough 1\nmilestone a\n checkpoint x\n deadline 2\n goto x unit 3\n need player lv == 1"):
        try:
            parse_play(bad, "bad")
        except PlayError:
            continue
        raise AssertionError(f"accepted: {bad!r}")
    try:
        load_checkpoint("no-such-checkpoint")
        raise AssertionError("missing checkpoint loaded")
    except PlayError:
        pass
    # every committed objective file parses
    d = os.path.join(REPO, "traces", "playthrough")
    for f in sorted(os.listdir(d)):
        if f.endswith(".play"):
            with open(os.path.join(d, f), encoding="utf-8") as fh:
                text = fh.read()
            if "{" not in text:  # a matrix template parses per cell (cell_text)
                parse_play(text, f)
    # matrix (spec §4): only / roles / cell saves / delta / since
    mtext = """playthrough 1
act 1
save s --class sor --expansion --level 30 --skill 8=20 --quests acts=1 --act 0
save p --class nec --expansion --right-skill {summon}
milestone all
  use s
  deadline 9
  need player lv == 1
milestone pets
  only class nec,ama
  use p
  deadline 9
  need unit ut 1 cl {pet} present
milestone hell
  only difficulty hell
  use s
  deadline 9
  need player hpx delta > 0 since 3
"""
    own = parse_play(cell_text(mtext, "m", None, None), "m")
    assert [m["name"] for m in own["milestones"]] == ["all", "pets"], own
    assert own["milestones"][1]["need"][0]["filter"]["cl"] == [289], own["milestones"][1]
    pal = parse_play(cell_text(mtext, "m", "pal", "hell"), "m")
    assert [m["name"] for m in pal["milestones"]] == ["all", "hell"] and "p" not in pal["saves"], pal
    sa = pal["saves"]["s"]
    assert sa[sa.index("--class") + 1] == "pal" and "--skill" in sa and "8=20" not in sa, sa
    assert sa[sa.index("--right-skill") + 1] == "112" and sa[-2:] == ["--difficulty", "hell"], sa
    assert sa[sa.index("--difficulty-unlocked") + 1] == "hell" and "--act" in sa, sa
    q = sa[sa.index("--quests") + 1].split(",")
    assert q[0] == "acts=1" and "normal:acts=4" in q and "nightmare:40.0" in q and "hell:1.0" not in q, q
    ama = parse_play(cell_text(mtext, "m", "ama", "normal"), "m")
    pa = ama["saves"]["p"]
    assert pa[pa.index("--right-skill") + 1] == "32" and "--difficulty-unlocked" not in pa, pa
    assert ama["milestones"][1]["need"][0]["filter"]["cl"] == [357]
    assert "--quests" not in pa and "aqv/body=5" in pa, pa
    lacks = mtext.split("milestone all")[0] + "milestone x\n use p\n deadline 1\n need unit cl {pet} present\n"
    try:
        cell_text(lacks, "b", "pal", "normal")
        raise AssertionError("a role the class lacks accepted")
    except PlayError as e:
        assert "no 'pet'" in str(e), e
    assert "milestone" not in cell_text(lacks.replace(" use p", " only class nec\n use p"), "b", "pal", "normal")
    try:
        parse_play("playthrough 1\nsave a --x\nmilestone a\n use a\n deadline 2\n only class wiz\n need player lv == 1", "b")
        raise AssertionError("only class wiz accepted")
    except PlayError:
        pass
    dsn = [snap(1, [dict(pl(1), hpx=100)]), snap(3, [dict(pl(1, 300, 300), hpx=100)]),
           snap(4, [dict(pl(1, 310, 300), hpx=150)])]
    dm = dict(town, need=[parse_pred("player hpx delta > 0 since 3", "t"), parse_pred("player moved >= 10 since 3", "t")])
    r = evaluate(dm, dsn, [])
    assert r["status"] == "reached" and "delta +50" in r["evidence"], r
    dm["need"] = [parse_pred("player moved >= 11 since 3", "t")]
    assert evaluate(dm, dsn, [])["status"] == "stuck"
    dm["need"] = [parse_pred("player moved >= 11", "t")]
    assert evaluate(dm, dsn, [])["status"] == "reached"
    for bad in ("player lv == 1 since 3", "player hpx delta ~ 3"):
        try:
            parse_pred(bad, "t")
        except PlayError:
            continue
        raise AssertionError(f"accepted: {bad!r}")
    cells = [{"act": 1, "play": "a1.play", "class": "ama", "difficulty": "normal", "reached": 1, "total": 2,
              "first_blocker": {"milestone": "k"}, "milestones": [{"name": "k", "status": "stuck", "evidence": "e"}]},
             {"act": 1, "play": "a1.play", "class": "sor", "difficulty": "normal", "reached": 2, "total": 2,
              "first_blocker": None, "milestones": [{"name": "k", "status": "reached", "evidence": ""}]}]
    bl = blockers(cells)
    assert len(bl) == 1 and bl[0]["cells"] == ["ama/normal"], bl
    md = markdown(cells)
    assert "| 1 (a1.play) | 1/2 `k` | 2/2 |" in md, md
    print("playthrough selftest: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
