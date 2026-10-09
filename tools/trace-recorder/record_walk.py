"""Record the local player's walk on the original 1.14d Game.exe: the
client unit's path position against the server unit's, per server tick
and per in-game frame, plus the camera globals before the first drawn
frame (REC-51 / REC-277 / REC-286 / REC-288, docs/HANDOFF.md).

Reuses the debugger of record_tick.py (TickRecorder: process control, the
server tick hook 0x52D870); adds the in-game draw entry 0x44C990
(capture.md §2). Addresses and offsets: specs/render/capture.md §3.1 /
§3.5 (client player 0x7A6A70, unit +0x2C path, path +0x00 / +0x04 16.16
position, camera globals 0x7A520C / 0x7A5208 / 0x7A5214), sim/tick.md and
sim/unit-order.md (game +0xA8 frame, +0x1120 unit hash, unit +0xE4 hash
next), sim/pathing.md (path +0x10 / +0x12 target, +0x24 index, +0x28
count).

Output (format walk-raw-1): a header line, then
  {"k":"cam0", "at":"tick"|"frame", ...}   camera globals, first tick and
                                           first draw entry (before the
                                           first drawn frame)
  {"k":"t", "f":tick, "c":client, "s":server}  at each server tick start
  {"k":"fr", "f":last tick, "u":client updates, "c":client, "s":server}
                                           at each in-game draw entry
  {"k":"units", "f":tick, "client":[unit, path], "server":[unit, path]}
                                           whenever the pointers change
where client / server = [x16.16, y16.16, mode, target x, target y,
index, count] (null when the unit or path is missing).

The game process is always terminated when this script ends.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.dont_write_bytecode = True
import autostart  # noqa: E402

TOOL = "trace-recorder record_walk 0.1.0"
FORMAT = "walk-raw-1"

FRAME_START = 0x44C990            # in-game draw entry (capture.md §2)
FRAME_START_BYTES = b"\x55\x8B\xEC\x83\xEC\x1C"
PLAYER = 0x7A6A70                 # client player unit (capture.md §3.1)
CLIENT_UPDATES = 0x7A0498
UNIT_ORIGIN_X, UNIT_ORIGIN_Y, SHIFT_X, OPEN_MODE = 0x7A520C, 0x7A5208, 0x7A5214, 0x7A5210
DRAW_COUNTER = 0x7A0494
U_GUID, U_MODE, U_PATH, U_HASH_NEXT = 0x0C, 0x10, 0x2C, 0xE4
HASH_BASE = 0x1120                # game: type 0 buckets, 128 heads (unit-order.md)


def path_rec(mem, unit):
    """[x, y, mode, tx, ty, index, count] of a dynamic-path unit, or None."""
    if not unit:
        return None
    path = mem.u32(unit + U_PATH)
    if not path:
        return None
    x, y = struct.unpack("<2I", mem.read(path, 8))
    tx, ty = struct.unpack("<2H", mem.read(path + 0x10, 4))
    idx, cnt = struct.unpack("<2i", mem.read(path + 0x24, 8))
    return [x, y, mem.u32(unit + U_MODE), tx, ty, idx, cnt]


def cam(mem):
    return {"origin": [mem.i32(UNIT_ORIGIN_X), mem.i32(UNIT_ORIGIN_Y)], "shift_x": mem.i32(SHIFT_X),
            "open_mode": mem.u32(OPEN_MODE), "draw": mem.u32(DRAW_COUNTER),
            "client_update": mem.u32(CLIENT_UPDATES), "player": mem.u32(PLAYER) != 0}


def make_recorder(rt):
    class Recorder(rt.TickRecorder):
        def __init__(self, *a):
            super().__init__(*a)
            self.cam_tick = self.cam_frame = False
            self.server_unit = 0
            self.frames = 0
            self.ptrs = None

        def server_player(self):
            unit = self.u32(PLAYER)
            if not unit or not self.game:
                return 0
            guid = self.u32(unit + U_GUID)
            s = self.server_unit
            if s and self.u32(s + U_GUID) == guid:
                return s
            for b in range(128):
                u, n = self.u32(self.game + HASH_BASE + 4 * b), 0
                while u and n < 1000:
                    if self.u32(u + U_GUID) == guid:
                        self.server_unit = u
                        return u
                    u, n = self.u32(u + U_HASH_NEXT), n + 1
            return 0

        def both(self):
            cu, su = self.u32(PLAYER), self.server_player()
            ptrs = (cu, self.u32(cu + U_PATH) if cu else 0, su, self.u32(su + U_PATH) if su else 0)
            if ptrs != self.ptrs:  # the units and paths read (client and server must differ)
                self.ptrs = ptrs
                self.emit({"k": "units", "f": self.frame, "client": [f"{p:#x}" for p in ptrs[:2]],
                           "server": [f"{p:#x}" for p in ptrs[2:]]})
            return path_rec(self, cu), path_rec(self, su)

        def handle(self, addr, ctx):
            if addr == FRAME_START:
                if not self.cam_frame:
                    self.cam_frame = True
                    self.emit({"k": "cam0", "at": "frame", "f": self.frame, **cam(self)})
                c, s = self.both()
                self.frames += 1
                self.emit({"k": "fr", "f": self.frame, "u": self.u32(CLIENT_UPDATES), "c": c, "s": s})
                return
            super().handle(addr, ctx)
            if addr == rt.TICK and self.game and ctx.Ecx == self.game:
                if not self.cam_tick:
                    self.cam_tick = True
                    self.emit({"k": "cam0", "at": "tick", "f": self.frame, **cam(self)})
                c, s = self.both()
                self.emit({"k": "t", "f": self.frame, "c": c, "s": s})

    return Recorder


class FakeMem:
    def __init__(self):
        self.b = {}

    def put(self, addr, fmt, *v):
        for i, x in enumerate(struct.pack(fmt, *v)):
            self.b[addr + i] = x

    def read(self, addr, n):
        return bytes(self.b.get(addr + i, 0) for i in range(n))

    def u32(self, a):
        return struct.unpack("<I", self.read(a, 4))[0]

    def i32(self, a):
        return struct.unpack("<i", self.read(a, 4))[0]


def selftest():
    m = FakeMem()
    m.put(0x1000 + U_PATH, "<I", 0x2000)
    m.put(0x1000 + U_MODE, "<I", 3)
    m.put(0x2000, "<2I", 5000 << 16 | 0x8000, 4400 << 16)
    m.put(0x2010, "<2H", 5010, 4405)
    m.put(0x2024, "<2i", 1, 2)
    assert path_rec(m, 0x1000) == [5000 << 16 | 0x8000, 4400 << 16, 3, 5010, 4405, 1, 2]
    assert path_rec(m, 0) is None and path_rec(m, 0x3000) is None
    m.put(UNIT_ORIGIN_X, "<i", -7)
    assert cam(m)["origin"] == [-7, 0] and cam(m)["player"] is False
    print("selftest ok")


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0)
    ap.add_argument("--ticks", type=int, default=0)
    ap.add_argument("--out", default=None)
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"])
    autostart.add_options(ap)
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    import record_tick as rt  # noqa: E402  (Windows only)
    out = a.out or os.path.join(repo, "traces", "raw",
                                datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-walk.jsonl")
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], FRAME_START: FRAME_START_BYTES}
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = make_recorder(rt)(os.path.abspath(a.game), gargs, out, a.seconds, 0, a.ticks)
    r.auto = auto
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    print(f"wrote {out}: {r.frames} frames, {r.ticks} ticks")


if __name__ == "__main__":
    main()
