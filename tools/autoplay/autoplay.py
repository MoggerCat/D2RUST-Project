#!/usr/bin/env python3
# Spec: specs/tools/autoplay.md
"""Autoplay bot: plays d2rs (our build) through the real client input
path, from a fresh character or a save, and reports where a real player
would get stuck.

    python3 tools/autoplay/autoplay.py --act 1                  # fresh Amazon
    python3 tools/autoplay/autoplay.py --act 1 --save X.d2s
    python3 tools/autoplay/autoplay.py --act 1 --only den-of-evil
    python3 tools/autoplay/autoplay.py --selftest

The game runs in `d2-client autoplay-host` (the `play` client headless,
line protocol `autoplay-1`, spec §1). The bot only moves the mouse,
clicks and presses keys; it reads the state (`state`, `map`) to decide.
No pokes, no warps, no hand-written messages.

Exit 0: every milestone of the act reached; 1: stuck (report written);
3: error (missing tool, host failed to start). Python stdlib only.
"""

import argparse
import json
import math
import os
import subprocess
import sys
import time
from collections import deque

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import acts  # noqa: E402
import nav  # noqa: E402

VERSION = "0.1.0"
RESULT_FORMAT = "autoplay-result-1"
REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

TICKS_PER_SECOND = 25
# Player / monster death modes (specs/sim/units.md §1): DT 0, DD 17 / 12.
PLAYER_DEAD = (0, 17)
MONSTER_DEAD = (0, 12)
TOWNS = (1, 40, 75, 103, 109)

# The 800 x 600 frame (spec §1 r2). The unit draw origin of the camera
# (render camera §3-§4): a unit at the player's cell is drawn at
# (400, 292); one sub-tile is 16 px across and 8 px down per axis.
CX, CY = 400, 292
# Clicks stay inside the play area, above the HUD panel.
SAFE = (20, 30, 780, 520)


class HostError(Exception):
    """The host process failed (exit, panic, protocol error)."""


# ---------------------------------------------------------------- geometry


def to_screen(px, py, wx, wy):
    """Frame pixel of world sub-tile (wx, wy) with the player at (px, py)."""
    dx, dy = wx - px, wy - py
    return CX + (dx - dy) * 16, CY + (dx + dy) * 8


def on_screen(p, margin=0):
    x, y = p
    return SAFE[0] + margin <= x <= SAFE[2] - margin and SAFE[1] + margin <= y <= SAFE[3] - margin


def clamp_screen(p):
    x, y = p
    return (int(min(max(x, SAFE[0]), SAFE[2])), int(min(max(y, SAFE[1]), SAFE[3])))


def toward(px, py, wx, wy, reach=10):
    """A frame point on the way from the player to (wx, wy), at most
    `reach` sub-tiles ahead, inside the play area."""
    dx, dy = wx - px, wy - py
    d = math.hypot(dx, dy)
    if d > reach:
        dx, dy = dx * reach / d, dy * reach / d
    # shrink until the point is on screen (the frame is wider than tall)
    for _ in range(20):
        p = to_screen(0, 0, dx, dy)
        if on_screen(p):
            return (int(p[0]), int(p[1]))
        dx, dy = dx * 0.8, dy * 0.8
    return clamp_screen(to_screen(0, 0, dx, dy))


def cheb(a, b):
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]))


def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


# ---------------------------------------------------------------- map / routes


class Map:
    """The `map` read (spec §2 r2): rooms by id, levels by id."""

    def __init__(self, m):
        self.act = m["act"]
        self.levels = {l["id"]: l for l in m["levels"]}
        self.rooms = {}
        self._adj = None
        for l in m["levels"]:
            for r in l["rooms"]:
                self.rooms[r["id"]] = r

    def level_graph(self):
        """acts.LEVEL_GRAPH with the allocated levels' vis arrays."""
        g = {k: list(v) for k, v in acts.LEVEL_GRAPH.items()}
        for lid, l in self.levels.items():
            for n in l.get("vis") or []:
                if n and n not in g.setdefault(lid, []):
                    g[lid].append(n)
        return g

    @staticmethod
    def inside(r, p, pad=0):
        x, y, w, h = r["rect"]
        return x - pad <= p[0] < x + w + pad and y - pad <= p[1] < y + h + pad

    @staticmethod
    def centre(r):
        x, y, w, h = r["rect"]
        return (x + w // 2, y + h // 2)

    def room_at(self, p, level=None):
        best = None
        for r in self.rooms.values():
            if level is not None and r["level"] != level:
                continue
            if self.inside(r, p):
                return r
            d = dist(self.centre(r), p)
            if best is None or d < best[0]:
                best = (d, r)
        return best[1] if best else None

    def edges(self, r, blocked=frozenset()):
        """(next room, kind) pairs: 'near' for rooms whose rects share an
        edge (the DRLG's near lists exist only for active rooms), 'warp'
        for a warp link."""
        if self._adj is None:
            self._adj = {}
            rs = list(self.rooms.values())
            for i, a in enumerate(rs):
                for b in rs[i + 1:]:
                    if touching(a, b):
                        self._adj.setdefault(a["id"], []).append(b["id"])
                        self._adj.setdefault(b["id"], []).append(a["id"])
        out = []
        for w in r.get("warps") or []:
            if w["to"] in self.rooms and (r["id"], w["to"]) not in blocked:
                out.append((self.rooms[w["to"]], "warp"))
        for n in self._adj.get(r["id"], []):
            if (r["id"], n) not in blocked:
                out.append((self.rooms[n], "near"))
        return out

    def route(self, start, goal, blocked=frozenset()):
        """Rooms from `start` to a room satisfying `goal(room)` (BFS by
        steps, then by walked distance), or None."""
        prev = {start["id"]: None}
        kind = {}
        q = deque([start])
        while q:
            r = q.popleft()
            if goal(r):
                path = []
                cur = r["id"]
                while cur is not None:
                    path.append((self.rooms[cur], kind.get(cur)))
                    cur = prev[cur]
                return list(reversed(path))
            nexts = sorted(self.edges(r, blocked), key=lambda e: dist(self.centre(r), self.centre(e[0])))
            for nr, k in nexts:
                if nr["id"] not in prev:
                    prev[nr["id"]] = r["id"]
                    kind[nr["id"]] = k
                    q.append(nr)
        return None


def touching(a, b):
    """Two room rects share an edge (or overlap) - walkable neighbours
    for an outdoor level; dungeon rooms may still be walled off."""
    ax, ay, aw, ah = a["rect"]
    bx, by, bw, bh = b["rect"]
    gx = max(bx - (ax + aw), ax - (bx + bw))
    gy = max(by - (ay + ah), ay - (by + bh))
    return gx <= 0 and gy <= 0 and not (gx == 0 and gy == 0)


def crossing_point(a, b):
    """A sub-tile inside room b next to its shared edge with room a."""
    ax, ay, aw, ah = a["rect"]
    bx, by, bw, bh = b["rect"]
    lo_x, hi_x = max(ax, bx), min(ax + aw, bx + bw)
    lo_y, hi_y = max(ay, by), min(ay + ah, by + bh)
    cx = (lo_x + hi_x) // 2 if lo_x < hi_x else None
    cy = (lo_y + hi_y) // 2 if lo_y < hi_y else None
    if cx is None:
        cx = bx + 3 if bx >= ax + aw else bx + bw - 3
    if cy is None:
        cy = by + 3 if by >= ay + ah else by + bh - 3
    if cx is not None and lo_x < hi_x:
        pass
    return (cx, cy)


# ---------------------------------------------------------------- the host


class Host:
    """`d2-client autoplay-host` over its line protocol (spec §1)."""

    def __init__(self, client, args, work, game_dir):
        self.log = open(os.path.join(work, "run.jsonl"), "w")
        self.err = open(os.path.join(work, "host.stderr"), "w")
        env = dict(os.environ)
        if game_dir:
            env["D2_GAME_DIR"] = game_dir
        self.cmdline = [client, "autoplay-host"] + args
        self.p = subprocess.Popen(self.cmdline, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=self.err, text=True, env=env, bufsize=1)
        hello = self._read()
        if hello.get("k") != "hello":
            raise HostError(f"no hello: {hello}")
        self.passes = hello["passes"]

    def _read(self):
        while True:
            line = self.p.stdout.readline()
            if not line:
                self.p.wait(timeout=30)
                raise HostError(f"host exited ({self.p.returncode}): {self.stderr_tail()}")
            # protocol lines are JSON objects; the game's own stdout
            # notes (e.g. "game folder: ...") are skipped
            if line.startswith("{"):
                return json.loads(line)

    def stderr_tail(self, n=8):
        self.err.flush()
        try:
            with open(self.err.name) as f:
                lines = [l.rstrip() for l in f if l.strip()]
        except OSError:
            return ""
        return " | ".join(lines[-n:])

    def cmd(self, line, record=True):
        try:
            self.p.stdin.write(line + "\n")
            self.p.stdin.flush()
        except BrokenPipeError:
            raise HostError(f"host gone: {self.stderr_tail()}")
        r = self._read()
        if r.get("k") == "error":
            raise HostError(f"{line}: {r['error']}")
        if record and r.get("k") == "ok":
            self.log.write(json.dumps({"cmd": line, "passes": r.get("passes")}) + "\n")
        return r

    def step(self, n=1):
        r = self.cmd(f"step {n}", record=False)
        self.passes = r["passes"]
        return r

    def state(self):
        return self.cmd("state", record=False)

    def map(self):
        return Map(self.cmd("map", record=False))

    def close(self):
        try:
            self.p.stdin.write("quit\n")
            self.p.stdin.flush()
            self.p.wait(timeout=30)
        except Exception:
            self.p.kill()
        self.log.close()
        self.err.close()


# ---------------------------------------------------------------- the view


class View:
    """One `state` read, with the lookups the bot needs."""

    def __init__(self, s):
        self.raw = s
        self.snap = s["snap"]
        self.frame = self.snap["f"]
        self.client = s["client"]
        self.units = self.snap["units"]
        g = s.get("local")
        self.me = next((u for u in self.units if u["ut"] == 0 and (g is None or u["g"] == g)), None)

    @property
    def pos(self):
        return (self.me["x"], self.me["y"])

    @property
    def level(self):
        return self.me.get("lv")

    def life(self):
        hp, hpx = self.me.get("hp", 0), self.me.get("hpx", 1)
        return hp / max(hpx, 1)

    def mana(self):
        mp, mpx = self.me.get("mp", 0), self.me.get("mpx", 1)
        return mp / max(mpx, 1)

    def dead(self):
        return self.me.get("m") in PLAYER_DEAD

    def quest(self, slot, bit):
        for s_, word in self.me.get("q", []):
            if s_ == slot:
                return bool(word >> bit & 1)
        return False

    def monsters(self):
        return [u for u in self.units if u["ut"] == 1 and "x" in u]

    def alive(self, u):
        return u.get("m") not in MONSTER_DEAD and u.get("hp", 1) > 0

    def find(self, ut, cl):
        return [u for u in self.units if u["ut"] == ut and u.get("cl") == cl and "x" in u]

    def open(self, ui):
        return ui in self.client["open"]


# ---------------------------------------------------------------- the bot


class Stuck(Exception):
    def __init__(self, reason):
        super().__init__(reason)
        self.reason = reason


class Bot:
    def __init__(self, host, plan, log=print, deadline_s=None):
        self.h = host
        self.plan = plan
        self.say = log
        self.actions = deque(maxlen=20)
        self.deaths = 0
        self.v = None
        self.map = None
        self.known = nav.Known()
        self.deadline_s = deadline_s
        self.progress_at = 0
        self.best = None

    # ---- primitives (all input goes through these)

    def act(self, what):
        self.actions.append({"frame": self.v.frame if self.v else None, "do": what})

    def click(self, p, button="L", hold=2):
        x, y = clamp_screen(p)
        self.act(f"click {button} {x} {y}")
        before = self.sent_total()
        self.h.cmd(f"press {button} {x} {y}")
        self.h.step(hold)
        self.h.cmd(f"release {button} {x} {y}")
        self.h.step(1)
        self.check_ignored(before)

    def sent_total(self):
        return sum(self.h.state()["client"]["sent"].values())

    def check_ignored(self, before):
        """A run of clicks that send the server nothing: the client
        refuses the player's input (spec §3 r5)."""
        if self.sent_total() != before:
            self.ignored = 0
            return
        self.ignored += 1
        if self.ignored >= acts.IGNORED_CLICKS:
            v = self.look()
            cm = (v.client.get("player") or {}).get("mode")
            raise Stuck(f"the client ignores input: {self.ignored} clicks in a row sent nothing "
                        f"(client model mode {cm}, server mode {v.me.get('m')}, "
                        f"open ui {v.client['open']})")

    ignored = 0

    def key(self, k):
        self.act(f"key {k}")
        self.h.cmd(f"key {k}")
        self.h.step(2)

    def wait(self, n):
        self.h.step(n)

    def look(self):
        self.v = View(self.h.state())
        if self.v.me is None:
            raise Stuck("no local player in the state")
        return self.v

    def refresh_map(self):
        self.map = self.h.map()
        self.known.merge(self.map)

    # ---- progress / stuck

    def reset_progress(self):
        self.best = None
        self.progress_at = self.v.frame

    def note_progress(self, metric):
        """`metric` shrinks as the objective nears; no shrink for the
        stuck window is a stuck point (spec §3 r5)."""
        if self.best is None or metric < self.best - 1:
            self.best = metric
            self.progress_at = self.v.frame
        elif self.v.frame - self.progress_at > acts.STUCK_SECONDS * TICKS_PER_SECOND:
            raise Stuck(f"no progress for {acts.STUCK_SECONDS} s (metric {metric}, best {self.best})")

    # ---- survival

    def survive(self):
        """Death handling, potions, and fighting what is near. Returns True
        if it acted (the caller looks again)."""
        v = self.v
        if v.dead():
            self.deaths += 1
            self.say(f"  died at frame {v.frame} level {v.level} pos {v.pos} (death {self.deaths})")
            if self.deaths >= acts.MAX_DEATHS:
                raise Stuck(f"death loop: {self.deaths} deaths")
            # the death screen: Esc returns to town (ui/panels: death)
            for _ in range(40):
                self.key("esc")
                self.wait(10)
                if not self.look().dead():
                    break
            else:
                raise Stuck("dead and Esc does not respawn")
            self.reset_progress()
            return True
        if v.life() < 0.4 and self.drink("hp", "rv"):
            return True
        if v.mana() < 0.2 and self.drink("mp", "rv"):
            return True
        if v.level in TOWNS:
            return False
        target = self.nearest_hostile(acts.FIGHT_RADIUS)
        if target is not None:
            self.attack(target)
            return True
        item = self.nearest_pickup(acts.PICK_RADIUS)
        if item is not None:
            self.act(f"pick {item['code']} at {item['x']},{item['y']}")
            self.click(to_screen(*v.pos, item["x"], item["y"]))
            self.wait(8)
            return True
        return False

    def drink(self, *prefixes):
        for it in self.v.client["belt"]:
            if any(it["code"].startswith(p) for p in prefixes):
                col = it["slot"] % 4
                self.act(f"drink {it['code']} column {col + 1}")
                self.key(str(col + 1))
                self.wait(4)
                return True
        return False

    def nearest_hostile(self, radius):
        v = self.v
        best = None
        for u in v.monsters():
            if not v.alive(u) or "own" in u or u.get("cl") in acts.NEUTRAL:
                continue
            d = dist(v.pos, (u["x"], u["y"]))
            if d <= radius and (best is None or d < best[0]):
                best = (d, u)
        return best[1] if best else None

    def nearest_pickup(self, radius):
        v = self.v
        best = None
        for it in v.client["ground"]:
            if not any(it["code"].startswith(p) for p in acts.PICKUP_CODES):
                continue
            if it["g"] in self.ignored_items:
                continue
            d = dist(v.pos, (it["x"], it["y"]))
            if d <= radius and (best is None or d < best[0]):
                best = (d, it)
        if best:
            # one try per item: a pickup that fails (belt full) is skipped
            self.ignored_items.add(best[1]["g"])
            return best[1]
        return None

    ignored_items = set()

    def attack(self, u):
        v = self.v
        p = to_screen(*v.pos, u["x"], u["y"])
        p = (p[0], p[1] - acts.BODY_LIFT)
        self.act(f"attack cl {u.get('cl')} g {u['g']} hp {u.get('hp')} at {u['x']},{u['y']}")
        button = self.plan.get("attack_button", "L")
        self.click(p, button=button, hold=acts.ATTACK_HOLD)

    # ---- walking

    def space(self, levels):
        return nav.Space(self.map, self.known, set(levels))

    def leg(self, path, why):
        """One walk click along `path` (sub-tiles), about LEG_REACH ahead."""
        v = self.v
        p = nav.ahead(path, acts.LEG_REACH)
        self.act(f"{why}: walk {v.pos} -> {p} (path {len(path)})")
        self.click(toward(*v.pos, *p, reach=acts.LEG_REACH + 2), hold=1)
        self.wait(acts.WALK_STEP)

    def walk_to(self, target, near=3, budget_s=None, fight=True):
        """Walks to world sub-tile `target` by clicks along a planned path
        (spec §3 r2). False when the budget ran out."""
        self.reset_progress()
        start = self.v.frame
        n = 0
        while True:
            v = self.look()
            if fight and self.survive():
                continue
            if dist(v.pos, target) <= near:
                return True
            if budget_s and v.frame - start > budget_s * TICKS_PER_SECOND:
                return False
            if n % acts.REMAP_EVERY == 0:
                self.refresh_map()
            n += 1
            path = nav.path_to_point(self.space([v.level]), v.pos, target, near=max(near - 1, 0))
            if path is None:
                raise Stuck(f"no path from {v.pos} to {target} in level {v.level}")
            self.note_progress(len(path))
            self.leg(path, f"to {target}")

    def goto_level(self, level):
        """Walks (and takes warps) until the player is in `level`."""
        self.say(f"  goto level {level}")
        self.reset_progress()
        tried_tiles = set()
        while True:
            v = self.look()
            if v.level == level:
                return
            if self.survive():
                continue
            self.refresh_map()
            m = self.map
            nxt = acts.next_level(v.level, level, m.level_graph())
            if nxt is None:
                raise Stuck(f"no level route from {v.level} to {level}")
            here_rooms = [r for r in m.rooms.values() if r["level"] == v.level]
            nxt_rooms = [r for r in m.rooms.values() if r["level"] == nxt]
            border = [r for r in nxt_rooms if any(touching(r, h) for h in here_rooms)]
            if border:
                path = nav.path_to_rooms(self.space([v.level, nxt]), v.pos, border)
                if path is None:
                    raise Stuck(f"no walkable border from level {v.level} into {nxt}")
                self.note_progress(len(path))
                self.leg(path, f"to level {nxt}")
                continue
            # a warp: the warp tile in sight, else the room holding the
            # link to `nxt`, else the level's warp-room centres
            tiles = [u for u in v.units if u["ut"] == 5 and "x" in u and u["g"] not in tried_tiles]
            if tiles:
                t = min(tiles, key=lambda u: dist(v.pos, (u["x"], u["y"])))
                if dist(v.pos, (t["x"], t["y"])) > 4:
                    path = nav.path_to_point(self.space([v.level]), v.pos, (t["x"], t["y"]), near=2)
                    if path is not None:
                        self.note_progress(len(path))
                        self.leg(path, f"to warp tile {t['g']}")
                        continue
                if self.take_tile(t) and self.look().level != nxt and self.v.level != v.level:
                    self.say(f"  tile {t['g']} led to level {self.v.level}, not {nxt}: going back")
                    tried_tiles.add(t["g"])
                elif self.v.level == v.level:
                    tried_tiles.add(t["g"])
                continue
            links = [r for r in here_rooms for w in r.get("warps") or []
                     if m.rooms.get(w["to"], {}).get("level") == nxt]
            if not links:
                cs = m.levels[v.level].get("warp_centres") or []
                links = [m.room_at(tuple(c), v.level) for c in cs]
                links = [r for r in links if r is not None and r["id"] not in self.warp_rooms_seen]
            if not links:
                raise Stuck(f"level {v.level}: no border, warp link or warp room toward {nxt}")
            target = min(links, key=lambda r: dist(v.pos, Map.centre(r)))
            if Map.inside(target, v.pos):
                self.warp_rooms_seen.add(target["id"])
                continue
            path = nav.path_to_rooms(self.space([v.level]), v.pos, [target])
            if path is None:
                raise Stuck(f"no path to the warp room {target['id']} in level {v.level}")
            self.note_progress(len(path))
            self.leg(path, f"to warp room {target['id']}")

    warp_rooms_seen = set()

    def take_tile(self, t):
        """Clicks warp tile unit `t`; True when the level changed."""
        lv = self.v.level
        self.act(f"warp tile g {t['g']} cl {t.get('cl')} at {t['x']},{t['y']}")
        self.click(to_screen(*self.v.pos, t["x"], t["y"]))
        for _ in range(10):
            self.wait(10)
            if self.look().level != lv:
                return True
        return False

    def explore_step(self, level):
        """One leg toward the nearest room of `level` not seen yet (its
        collision never read). False when every room has been seen."""
        v = self.look()
        self.refresh_map()
        unseen = [r for r in self.map.rooms.values() if r["level"] == level and r["id"] not in self.known.cells]
        if not unseen:
            return False
        path = nav.path_to_rooms(self.space([level]), v.pos, unseen)
        if path is None:
            return False
        self.note_progress(-len(self.known.cells) * 1000 + len(path))
        self.leg(path, "explore")
        return True

    # ---- town

    def talk(self, npc_class, until, what):
        """Walks to the NPC, clicks it, chooses Talk, skips the dialog,
        until `until(view)` (spec §3 r4)."""
        self.say(f"  talk to npc {npc_class} ({what})")
        for attempt in range(8):
            v = self.look()
            if until(v):
                return
            npcs = v.find(1, npc_class)
            if not npcs:
                spot = acts.NPC_SPOT.get(npc_class)
                if spot is None:
                    raise Stuck(f"npc {npc_class} not in sight and no known spot")
                self.explore_town(npc_class)
                continue
            n = npcs[0]
            if dist(v.pos, (n["x"], n["y"])) > 8:
                self.walk_to((n["x"], n["y"]), near=6, budget_s=40, fight=False)
                continue
            v = self.look()
            p = to_screen(*v.pos, n["x"], n["y"])
            self.click((p[0], p[1] - acts.BODY_LIFT))
            self.wait(30)
            v = self.look()
            menu = v.client.get("npc_menu")
            if menu is None:
                self.say(f"  no NPC menu after the click (attempt {attempt})")
                continue
            talk = [r for r in menu["rows"] if r["kind"] and "Talk" in r["kind"]]
            if talk and talk[0]["at"] and not menu["talking"]:
                self.act("menu Talk")
                self.click(talk[0]["at"])
                self.wait(20)
            # skip the dialog / topic box: Esc until the menu is down
            for _ in range(30):
                v = self.look()
                if until(v):
                    break
                if v.client.get("topics"):
                    t = v.client["topics"]
                    # quest topics first (the last selectable is cancel)
                    pts = [p for p in t["at"] if p]
                    if len(pts) > 1:
                        self.act(f"topic {t['text'][0]!r}")
                        self.click(pts[0])
                        self.wait(20)
                        continue
                if v.client.get("dialog_lines") or v.client.get("npc_menu"):
                    self.key("esc")
                    self.wait(10)
                    continue
                break
            v = self.look()
            if until(v):
                return
        raise Stuck(f"talking to npc {npc_class} did not {what}")

    def explore_town(self, npc_class):
        if not self.explore_step(self.v.level):
            raise Stuck(f"npc {npc_class} not found: the whole level is seen")

    # ---- the plan

    def run(self, milestones, only=None):
        results = []
        stuck = None
        t0 = None
        for m in milestones:
            if only and m.name != only:
                continue
            v = self.look()
            if t0 is None:
                t0 = v.frame
            if m.done(v):
                self.say(f"{m.name}: already done")
                results.append({"name": m.name, "reached": True, "frame": v.frame})
                continue
            self.say(f"{m.name}: {m.note}")
            try:
                self.reset_progress()
                m.play(self)
                v = self.look()
                if not m.done(v):
                    raise Stuck(f"{m.name}: steps ran but the milestone is not reached")
                self.say(f"{m.name}: reached at frame {v.frame} (level {v.level})")
                results.append({"name": m.name, "reached": True, "frame": v.frame})
            except Stuck as e:
                v = self.v
                stuck = {
                    "milestone": m.name,
                    "reason": e.reason,
                    "frame": v.frame if v else None,
                    "level": v.level if v else None,
                    "pos": list(v.pos) if v and v.me else None,
                    "actions": list(self.actions),
                }
                self.say(f"{m.name}: STUCK at frame {stuck['frame']} level {stuck['level']} pos {stuck['pos']}: {e.reason}")
                results.append({"name": m.name, "reached": False, "frame": stuck["frame"]})
                break
        frames = (self.v.frame - t0) if (self.v and t0 is not None) else 0
        return results, stuck, frames


# ---------------------------------------------------------------- running


def tool_path(build):
    client = os.path.join(REPO, "target", "release", "d2-client")
    if build:
        r = subprocess.run(["cargo", "build", "--release", "-q", "-p", "d2-client"], cwd=REPO)
        if r.returncode != 0:
            raise HostError("cargo build failed")
    if not os.path.exists(client):
        raise HostError(f"{client} missing: run with --build")
    return client


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--act", type=int, default=1)
    ap.add_argument("--save", help="character save (.d2s); default a fresh character of the act plan's class")
    ap.add_argument("--seed", type=int)
    ap.add_argument("--only", help="run one milestone")
    ap.add_argument("--from", dest="start", help="start at this milestone")
    ap.add_argument("--game-dir", default=os.environ.get("D2_GAME_DIR"))
    ap.add_argument("--work", help="work dir (default target/autoplay/act<N>-<time>)")
    ap.add_argument("--json", help="also write the result (autoplay-result-1) here")
    ap.add_argument("--build", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    try:
        client = tool_path(a.build)
    except HostError as e:
        print(f"autoplay: {e}", file=sys.stderr)
        return 3
    plan = acts.plan(a.act)
    work = a.work or os.path.join(REPO, "target", "autoplay", f"act{a.act}-{time.strftime('%Y%m%d-%H%M%S')}")
    os.makedirs(work, exist_ok=True)
    args = ["--save", a.save] if a.save else ["--new", plan["class"], plan["name"]]
    if a.seed is not None:
        args += ["--seed", str(a.seed)]
    t_start = time.time()
    try:
        host = Host(client, args, work, a.game_dir)
    except HostError as e:
        print(f"autoplay: host did not start: {e}", file=sys.stderr)
        return 3
    print(f"autoplay {VERSION}: act {a.act}, {' '.join(host.cmdline)}")
    print(f"work dir {work}")
    bot = Bot(host, plan)
    ms = plan["milestones"]
    if a.start:
        names = [m.name for m in ms]
        if a.start not in names:
            print(f"autoplay: no milestone {a.start}", file=sys.stderr)
            return 3
        ms = ms[names.index(a.start):]
    stuck = None
    try:
        results, stuck, frames = bot.run(ms, a.only)
    except HostError as e:
        v = bot.v
        stuck = {"milestone": None, "reason": f"host failure: {e}", "frame": v.frame if v else None,
                 "level": v.level if v else None, "pos": list(v.pos) if v and v.me else None,
                 "actions": list(bot.actions)}
        results, frames = [], 0
        print(f"HOST FAILURE: {e}")
    if stuck is not None:
        try:
            stuck["state"] = host.state()
        except Exception as e:  # noqa: BLE001 - the host may be gone
            stuck["state"] = f"unavailable: {e}"
        with open(os.path.join(work, "stuck.json"), "w") as f:
            json.dump(stuck, f, indent=1)
    host.close()
    reached = 0
    for r in results:
        if not r["reached"]:
            break
        reached += 1
    result = {
        "format": RESULT_FORMAT,
        "act": a.act,
        "command": " ".join(host.cmdline),
        "reached": reached,
        "total": len(ms) if not a.only else 1,
        "milestones": results,
        "ticks": frames,
        "game_seconds": round(frames / TICKS_PER_SECOND, 1),
        "wall_seconds": round(time.time() - t_start, 1),
        "deaths": bot.deaths,
        "stuck": {k: v for k, v in stuck.items() if k != "state"} if stuck else None,
    }
    print()
    print(f"act {a.act}: {reached}/{result['total']} milestones, {result['game_seconds']} s game time, "
          f"{bot.deaths} deaths, {result['wall_seconds']} s wall")
    if stuck:
        print(f"first stuck point: {stuck['milestone']} frame {stuck['frame']} level {stuck['level']} "
              f"pos {stuck['pos']}: {stuck['reason']}")
        print("last actions:")
        for x in stuck["actions"]:
            print(f"  {x}")
    with open(os.path.join(work, "result.json"), "w") as f:
        json.dump(result, f, indent=1)
    if a.json:
        with open(a.json, "w") as f:
            json.dump(result, f, indent=1)
    return 0 if not stuck and reached == result["total"] else 1


# ---------------------------------------------------------------- self-test


def selftest():
    # projection: the player's cell is the unit origin; +x goes right-down
    assert to_screen(10, 10, 10, 10) == (CX, CY)
    assert to_screen(0, 0, 1, 0) == (CX + 16, CY + 8)
    assert to_screen(0, 0, 0, 1) == (CX - 16, CY + 8)
    p = toward(0, 0, 100, 0)
    assert on_screen(p) and p[0] > CX and p[1] > CY, p
    p = toward(0, 0, -100, -100)
    assert on_screen(p) and p[1] < CY, p
    # rooms: a 3-room strip, the middle edge blocked
    m = Map({"act": 0, "levels": [
        {"id": 1, "rooms": [
            {"id": 1, "rect": [0, 0, 40, 40], "near": [2], "warps": [], "level": 1},
            {"id": 2, "rect": [40, 0, 40, 40], "near": [1, 3], "warps": [], "level": 1}]},
        {"id": 2, "rooms": [
            {"id": 3, "rect": [80, 0, 40, 40], "near": [2], "warps": [{"to": 4, "on": True, "row": 0}], "level": 2}]},
        {"id": 8, "rooms": [
            {"id": 4, "rect": [1000, 1000, 40, 40], "near": [], "warps": [], "level": 8}]}]})
    path = m.route(m.rooms[1], lambda r: r["level"] == 8)
    assert [r["id"] for r, _ in path] == [1, 2, 3, 4], path
    assert path[-1][1] == "warp"
    assert m.route(m.rooms[1], lambda r: r["level"] == 8, frozenset({(2, 3)})) is None
    assert crossing_point(m.rooms[1], m.rooms[2]) == (43, 20)
    assert m.room_at((45, 5))["id"] == 2
    assert touching(m.rooms[1], m.rooms[2]) and not touching(m.rooms[1], m.rooms[3])
    assert acts.next_level(1, 8) == 2
    assert acts.next_level(2, 17) == 3
    print("selftest ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
