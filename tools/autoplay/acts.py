# Spec: specs/tools/autoplay.md (§3, §4)
"""Act plans for tools/autoplay/autoplay.py: the milestones of each act
(named as in traces/playthrough/act<N>.play where one exists), how the
bot plays each one through the client input, and the done test on the
state.

Ids are levels.txt rows, monstats rows, objects.txt rows and quest slots
(specs/world/quests.md §1.9; done = bit 0 of the slot), the same ids
traces/playthrough/act1.play names.
"""

from collections import deque

# Bot tuning (spec §3).
IGNORED_CLICKS = 12     # clicks in a row that send nothing
STUCK_SECONDS = 60      # no progress toward the objective for this long
MAX_DEATHS = 3          # deaths without reaching the milestone
FIGHT_RADIUS = 14       # sub-tiles: hostiles nearer than this are fought
PICK_RADIUS = 8         # sub-tiles: potions / gold nearer are picked
ATTACK_HOLD = 12        # passes the attack click is held
WALK_STEP = 8           # passes between walk clicks
LEG_REACH = 10          # sub-tiles ahead along the path per walk click
REMAP_EVERY = 3         # walk legs between map reads
BODY_LIFT = 24          # px above the feet: where a unit's body is clicked

PICKUP_CODES = ("gld", "hp", "mp", "rv")

# Monster classes that are not hostile outside town (monstats rows).
NEUTRAL = {
    146,  # DeckardCain (Tristram, Act I)
}

# Act I levels (levels.txt rows) and their neighbours: walkable borders
# and warps (levels.txt Vis0-7). The route between levels the DRLG has
# not allocated yet follows these.
LEVEL_GRAPH = {
    1: [2],
    2: [1, 3, 8],
    3: [2, 4, 17, 9],
    4: [3, 10],
    5: [10, 6],
    6: [5, 7, 20, 11],
    7: [6, 12, 26],
    8: [2],
    9: [3, 13],
    10: [4, 5],
    17: [3, 18, 19],
    26: [7, 27],
    27: [26, 28],
    28: [27, 29],
    29: [28, 30],
    30: [29, 31],
    31: [30, 32],
    32: [31, 33],
    33: [32, 34],
    34: [33, 35],
    35: [34, 36],
    36: [35, 37],
    37: [36],
}

# Where a town NPC stands is found by walking the town's rooms.
NPC_SPOT = {148: None, 150: None, 154: None, 147: None, 155: None}

AKARA, KASHYA, CHARSI, GHEED, WARRIV = 148, 150, 154, 147, 155
BLOOD_RAVEN, ANDARIEL, CAIN = 267, 156, 146


def next_level(here, goal, graph=LEVEL_GRAPH):
    """The neighbour of `here` on a shortest level path to `goal`."""
    if here == goal:
        return goal
    prev = {here: None}
    q = deque([here])
    while q:
        l = q.popleft()
        if l == goal:
            while prev[l] != here:
                l = prev[l]
            return l
        for n in graph.get(l, []):
            if n not in prev:
                prev[n] = l
                q.append(n)
    return None


class Milestone:
    def __init__(self, name, note, done, play):
        self.name = name
        self.note = note
        self.done = done
        self.play = play


def _q(slot, *bits):
    return lambda v: any(v.quest(slot, b) for b in bits)


# ---- plays (each uses only Bot input primitives)


def kill_one(bot):
    """Walks Blood Moor until a hostile dies to the bot."""
    kills0 = len(_dead_seen(bot))
    bot.reset_progress()
    while len(_dead_seen(bot)) <= kills0:
        v = bot.look()
        if bot.survive():
            continue
        if not bot.explore_step(v.level):
            raise_stuck("no unexplored room left and nothing killed")


_killed = {}


def _dead_seen(bot):
    """GUIDs of monsters seen dead (mode DT / DD) near the bot."""
    v = bot.v
    for u in v.monsters():
        if not v.alive(u) and "own" not in u:
            _killed[u["g"]] = v.frame
    return _killed


def raise_stuck(reason):
    from autoplay import Stuck
    raise Stuck(reason)


def clear_level(bot, level, done):
    """Explores every room of `level`, fighting, until `done(view)`."""
    bot.goto_level(level)
    bot.reset_progress()
    while True:
        v = bot.look()
        if done(v):
            return
        if bot.survive():
            continue
        if not bot.explore_step(level):
            # every room seen: walk to what is left alive in sight
            alive = [u for u in v.monsters() if v.alive(u) and "own" not in u]
            if alive:
                u = min(alive, key=lambda u: abs(u["x"] - v.pos[0]) + abs(u["y"] - v.pos[1]))
                bot.walk_to((u["x"], u["y"]), near=5, budget_s=30)
                continue
            raise_stuck(f"level {level} explored, quest step not done")


def hunt(bot, level, cls, done):
    """Explores `level` until a unit of class `cls` is seen, then fights
    it until `done(view)`."""
    bot.goto_level(level)
    bot.reset_progress()
    while True:
        v = bot.look()
        if done(v):
            return
        targets = [u for u in v.find(1, cls) if v.alive(u)]
        if targets:
            t = targets[0]
            bot.note_progress(abs(t.get("hp", 0)))
            if abs(t["x"] - v.pos[0]) + abs(t["y"] - v.pos[1]) > 10:
                if bot.survive():
                    continue
                bot.walk_to((t["x"], t["y"]), near=6, budget_s=20)
            else:
                bot.attack(t)
            continue
        if bot.survive():
            continue
        if not bot.explore_step(level):
            raise_stuck(f"monster {cls} not found in level {level}")


def to_town_and_talk(bot, npc, until, what):
    bot.goto_level(1)
    bot.talk(npc, until, what)


def touch_waypoint(bot, level):
    """Walks to the level's waypoint (object 119 in Act I) and clicks it:
    the menu opens (ui 0x14), then Esc closes it."""
    bot.goto_level(level)
    bot.reset_progress()
    while True:
        v = bot.look()
        if v.open(0x14):
            bot.key("esc")
            return
        wps = [u for u in v.units if u["ut"] == 2 and u.get("cl") in WAYPOINT_OBJECTS and "x" in u]
        if wps:
            w = wps[0]
            d = abs(w["x"] - v.pos[0]) + abs(w["y"] - v.pos[1])
            bot.note_progress(d)
            if d > 8:
                bot.walk_to((w["x"], w["y"]), near=5, budget_s=30)
                continue
            bot.click(_screen(bot, w["x"], w["y"]))
            bot.wait(30)
            continue
        if bot.survive():
            continue
        if not bot.explore_step(level):
            raise_stuck(f"no waypoint found in level {level}")


WAYPOINT_OBJECTS = {119, 145, 156, 157, 237, 238, 288, 323, 324, 398, 402, 429, 494, 496, 511, 539}


def _screen(bot, x, y):
    from autoplay import to_screen
    return to_screen(*bot.v.pos, x, y)


# ---- Act I


def act1():
    den_cleared = _q(1, 1, 13, 0)
    raven_dead = _q(2, 1, 13, 0)
    return {
        "class": "amazon",
        "name": "AutoAma",
        "attack_button": "L",
        "milestones": [
            Milestone("town-start", "in the Rogue Encampment", lambda v: v.level == 1, lambda b: None),
            Milestone("blood-moor", "walk out of town into Blood Moor (2)", lambda v: v.level == 2,
                      lambda b: b.goto_level(2)),
            Milestone("kill-zombie", "kill a monster in Blood Moor",
                      lambda v: len(_killed) > 0, kill_one),
            Milestone("den-of-evil", "find the Den of Evil (8) in Blood Moor", lambda v: v.level == 8,
                      lambda b: b.goto_level(8)),
            Milestone("den-of-evil-cleared", "clear the Den (quest 1 bit 1 / 13)", den_cleared,
                      lambda b: clear_level(b, 8, den_cleared)),
            Milestone("den-of-evil-done", "talk to Akara (148): quest 1 done (1.0)", _q(1, 0),
                      lambda b: to_town_and_talk(b, AKARA, _q(1, 0), "complete the Den of Evil")),
            Milestone("cold-plains-wp", "Cold Plains (3) waypoint", lambda v: v.level == 3 and v.open(0x14) or _wp_taken.get(3),
                      lambda b: _take_wp(b, 3)),
            Milestone("burial-grounds", "Burial Grounds (17)", lambda v: v.level == 17,
                      lambda b: b.goto_level(17)),
            Milestone("blood-raven-killed", "kill Blood Raven (267): quest 2 bit 1 / 13", raven_dead,
                      lambda b: hunt(b, 17, BLOOD_RAVEN, raven_dead)),
            Milestone("blood-raven-done", "talk to Kashya (150): quest 2 done (2.0)", _q(2, 0),
                      lambda b: to_town_and_talk(b, KASHYA, _q(2, 0), "complete Sisters' Burial Grounds")),
        ],
    }


_wp_taken = {}


def _take_wp(bot, level):
    touch_waypoint(bot, level)
    _wp_taken[level] = True


def plan(act):
    if act == 1:
        return act1()
    raise SystemExit(f"autoplay: no plan for act {act} yet")
