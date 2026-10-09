# Spec: specs/tools/autoplay.md (§3 r1-r2)
"""Path planning for the autoplay bot: A* over the sub-tiles of the rooms
the `map` read lists. Sub-tiles of active rooms are known from their
collision grids (`#` blocked, `D` door, `.` free) and remembered, as a
player remembers the map they have seen; sub-tiles not seen yet are taken
as passable at a higher cost, so the plan explores and is redone when a
wall shows up.
"""

import heapq
import math

BLOCK = 40          # spatial hash cell (sub-tiles) for the room index
UNKNOWN_COST = 1.4  # an unseen sub-tile costs this much more than a free one
DOOR_COST = 4.0
DIAG = math.sqrt(2)
MAX_EXPAND = 400_000


class Known:
    """The remembered collision of every room seen active."""

    def __init__(self):
        self.cells = {}   # room id -> (x, y, w, h, cells str)

    def merge(self, m):
        for r in m.rooms.values():
            g = r.get("grid")
            if g:
                x, y, w, h = g["rect"]
                self.cells[r["id"]] = (x, y, w, h, g["cells"])


class Space:
    """The searchable sub-tiles: the rooms of `levels` of map `m`."""

    def __init__(self, m, known, levels):
        self.rooms = [r for r in m.rooms.values() if r["level"] in levels]
        self.index = {}
        for r in self.rooms:
            x, y, w, h = r["rect"]
            for bx in range(x // BLOCK, (x + w - 1) // BLOCK + 1):
                for by in range(y // BLOCK, (y + h - 1) // BLOCK + 1):
                    self.index.setdefault((bx, by), []).append(r)
        self.known = known

    def room_of(self, x, y):
        for r in self.index.get((x // BLOCK, y // BLOCK), ()):
            rx, ry, rw, rh = r["rect"]
            if rx <= x < rx + rw and ry <= y < ry + rh:
                return r
        return None

    def cell(self, x, y):
        """'#', 'D', '.', '?' (unseen) or None (outside every room)."""
        r = self.room_of(x, y)
        if r is None:
            return None
        k = self.known.cells.get(r["id"])
        if k is None:
            return "?"
        gx, gy, gw, gh, cells = k
        if gx <= x < gx + gw and gy <= y < gy + gh:
            return cells[(y - gy) * gw + (x - gx)]
        return "?"


def cost_of(c):
    if c == "." or c is None:
        return 1.0
    if c == "?":
        return UNKNOWN_COST
    if c == "D":
        return DOOR_COST
    return None


def astar(space, start, goal_test, h):
    """A path of sub-tiles from `start` to the first cell with
    `goal_test(x, y)`, 8-connected; None when there is none."""
    sx, sy = start
    openq = [(h(sx, sy), 0.0, sx, sy)]
    g = {(sx, sy): 0.0}
    prev = {}
    n = 0
    while openq:
        f, gc, x, y = heapq.heappop(openq)
        if gc > g.get((x, y), math.inf):
            continue
        if goal_test(x, y):
            path = [(x, y)]
            while (x, y) in prev:
                x, y = prev[(x, y)]
                path.append((x, y))
            return path[::-1]
        n += 1
        if n > MAX_EXPAND:
            return None
        for dx, dy, step in ((1, 0, 1), (-1, 0, 1), (0, 1, 1), (0, -1, 1),
                             (1, 1, DIAG), (1, -1, DIAG), (-1, 1, DIAG), (-1, -1, DIAG)):
            nx, ny = x + dx, y + dy
            c = space.cell(nx, ny)
            if c is None:
                continue
            k = cost_of(c)
            if k is None:
                continue
            if dx and dy:
                # no corner cutting past a wall
                if cost_of(space.cell(x + dx, y)) is None or cost_of(space.cell(x, y + dy)) is None:
                    continue
            ng = gc + step * k
            if ng < g.get((nx, ny), math.inf):
                g[(nx, ny)] = ng
                prev[(nx, ny)] = (x, y)
                heapq.heappush(openq, (ng + h(nx, ny), ng, nx, ny))
    return None


def rect_dist(rect, x, y):
    rx, ry, rw, rh = rect
    dx = max(rx - x, 0, x - (rx + rw - 1))
    dy = max(ry - y, 0, y - (ry + rh - 1))
    return max(dx, dy)


def path_to_rooms(space, start, rooms):
    """A path into any of `rooms` (their rects)."""
    rects = [r["rect"] for r in rooms]
    ids = {r["id"] for r in rooms}

    def goal(x, y):
        r = space.room_of(x, y)
        return r is not None and r["id"] in ids and space.cell(x, y) != "#"

    def h(x, y):
        return min(rect_dist(rc, x, y) for rc in rects)

    return astar(space, start, goal, h)


def path_to_point(space, start, target, near=1):
    tx, ty = target

    def goal(x, y):
        return max(abs(x - tx), abs(y - ty)) <= near

    def h(x, y):
        return max(abs(x - tx), abs(y - ty))

    return astar(space, start, goal, h)


def ahead(path, reach):
    """The path point about `reach` sub-tiles along, keeping to known
    straight stretches (the client's own pathing walks the rest)."""
    if not path:
        return None
    i = min(len(path) - 1, reach)
    return path[i]
