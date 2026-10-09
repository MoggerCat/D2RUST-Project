"""Record the client animation state of the original 1.14d Game.exe: per
client unit update the unit's mode, frame, frame count, speed, frame
event and motion-record offsets, every footstep call, every leap start
and every client frame advance.

Reuses the debugger of record_tick.py (TickRecorder: process control,
breakpoints, the tick marker); this script only adds hooks. Every hooked
address and offset is documented in a spec (each constant below names
it). Output: one JSON line per record in traces/raw/<time>-anim.jsonl
(format anim-raw-1). Settles REC-430 (R-ANIM-1, `docs/HANDOFF.md`) and
REC-275 (the leap arc and whirlwind spin per client update).

Records:
- `upd`: entry of the per-unit client update `0x00480810`
  (`client/model.md` §5 rule 2; ECX = unit): C = `[0x7A0498]`, unit type,
  class, GUID, mode +0x10, frame +0x44, frame count +0x48, speed +0x4C
  (i16), event +0x4E, footstep stamp +0x84, the client path position
  (path +0x2C: +0x00/+0x04 16.16 fixed), and the motion record (gfx
  +0x54 -> +0x30; `render/unit-composite.md` §8: flags, x, y, z, ticks
  left, ox, oy, oz) when there is one. Players and monsters only.
- `fs`: entry of the footsteps `0x004CAF60(U)` (`audio/triggers.md` §5
  and its check table): U, +0x44, +0x48, +0x4C, +0x84, C.
- `leap`: entry of `0x004C8670(unit, point)` (`unit-composite.md` §8,
  creator `0x004C8726`): the raw ECX, EDX and the first 3 stack words.
- `wu` / `wm` (`--weather`): entries of the weather update `0x00473F50`
  and the particle move `0x004732C0` (`render/draw-order-2.md` §11.2,
  §11.9): C, the update mark `[0x007A8A0C]`, the move counter F
  `[0x007A8A34]`, the particle pool's live count (`[0x007A8A04]` +0x114),
  the stored rain flag `[0x007A8A44]` and the target `[0x007A89E0]`.
- `adv`: entry of the frame advance `0x00623E00` (`audio/triggers-2.md`
  §15 r1) for a client player or monster: the raw candidates and the
  unit's frame fields before.

The game process is always terminated when this script ends.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import os
import struct
import sys

sys.dont_write_bytecode = True
import record_tick as rt  # noqa: E402
import autostart  # noqa: E402

TOOL = "trace-recorder record_anim 0.2.0"
FORMAT = "anim-raw-1"

CLIENT_UPDATES = 0x7A0498   # C (render/capture.md §3.1, audio/triggers.md checks)
PLAYER = 0x7A6A70           # client player unit
U_TYPE, U_CLASS, U_GUID, U_MODE, U_PATH = 0x00, 0x04, 0x0C, 0x10, 0x2C
U_CUR, U_FC, U_SPEED, U_EVENT, U_GFX, U_STAMP = 0x44, 0x48, 0x4C, 0x4E, 0x54, 0x84
GFX_MOTION = 0x30

H_UPDATE = 0x480810    # client/model.md §5 rule 2
H_FOOT = 0x4CAF60      # audio/triggers.md §5
H_LEAP = 0x4C8670      # render/unit-composite.md §8 creators
H_ADV = 0x623E00       # audio/triggers-2.md §15 r1
H_WUPD = 0x473F50      # render/draw-order-2.md §11.2
H_WMOVE = 0x4732C0     # render/draw-order-2.md §11.9
# render/draw-order-2.md §11.1: update mark, move counter F, particle pool, stored rain flag, target
W_MARK, W_F, W_POOL, W_RAIN, W_TARGET = 0x7A8A0C, 0x7A8A34, 0x7A8A04, 0x7A8A44, 0x7A89E0
POOL_LIVE = 0x114
WEATHER_EXPECT = {H_WUPD: b"\x55\x8B\xEC\x51\x53\x56\x57", H_WMOVE: b"\x55\x8B\xEC\x83\xEC\x08"}

ANIM_EXPECT = {  # first bytes (the 1.14d file image)
    H_UPDATE: b"\x55\x8B\xEC\x51\x56\x8B\xF1", H_FOOT: b"\x55\x8B\xEC\x83\xEC\x2C",
    H_LEAP: b"\x55\x8B\xEC\x83\xEC\x08", H_ADV: b"\x55\x8B\xEC\x56\x8B\x75\x08",
}


class AnimRecorder(rt.TickRecorder):
    def __init__(self, exe, args, out_path, seconds, max_ticks, types, adv, arm_level):
        super().__init__(exe, args, out_path, seconds, 0, max_ticks)
        self.types, self.adv, self.arm_level = types, adv, arm_level
        self.n = 0
        self.lazy = []   # hooks armed once the player is in level `arm_level`

    def arm(self, addr):
        if self.arm_level is not None and addr != rt.TICK and addr not in self.bp_orig:
            self.lazy.append(addr)
            self.bp_orig[addr] = self.read(addr, 1)[0]
            return
        super().arm(addr)

    def is_unit(self, p):
        if not p or p < 0x10000:
            return False
        try:
            t, c = struct.unpack("<II", self.read(p, 8))
        except OSError:
            return False
        return t in self.types and c < 0x1000

    def fields(self, u):
        b = self.read(u, 0x88)
        t, c, _, g, m = struct.unpack_from("<5I", b, 0)
        r = {"t": t, "c": c, "g": g, "m": m,
             "f": struct.unpack_from("<i", b, U_CUR)[0], "F": struct.unpack_from("<i", b, U_FC)[0],
             "s": struct.unpack_from("<h", b, U_SPEED)[0], "e": b[U_EVENT],
             "st": struct.unpack_from("<I", b, U_STAMP)[0], "C": self.u32(CLIENT_UPDATES)}
        return r

    def extra(self, u, r):
        path = self.u32(u + U_PATH)
        if path:
            r["p"] = list(struct.unpack("<2I", self.read(path, 8)))
        gfx = self.u32(u + U_GFX)
        rec = self.u32(gfx + GFX_MOTION) if gfx else 0
        if rec:
            w = struct.unpack("<19i", self.read(rec, 0x4C))
            r["mo"] = {"fl": w[0], "xyz": list(w[1:4]), "v": list(w[4:7]), "a": list(w[7:10]),
                       "o": list(w[13:16]), "n": w[18]}
        if u == self.u32(PLAYER):
            r["me"] = 1
        return r

    def handle(self, addr, ctx):
        if addr == rt.TICK:
            super().handle(addr, ctx)
            if self.lazy and autostart.player_level(self) == self.arm_level:
                for a in self.lazy:
                    self.write(a, rt.rr.INT3)
                self.notes.append(f"hooks armed in level {self.arm_level} at tick {self.frame}")
                self.emit({"k": "armed", "f": self.frame, "C": self.u32(CLIENT_UPDATES)})
                self.lazy = []
            return
        if addr == H_UPDATE:
            u = ctx.Ecx
            if self.is_unit(u):
                self.emit({"k": "upd", **self.extra(u, self.fields(u))})
        elif addr == H_FOOT:
            st = struct.unpack("<2I", self.read(ctx.Esp + 4, 8))
            for src, u in (("ecx", ctx.Ecx), ("edx", ctx.Edx), ("s0", st[0]), ("s1", st[1])):
                if self.is_unit(u):
                    self.emit({"k": "fs", "via": src, **self.fields(u)})
                    break
        elif addr == H_LEAP:
            st = struct.unpack("<3I", self.read(ctx.Esp + 4, 12))
            r = {"k": "leap", "ecx": ctx.Ecx, "edx": ctx.Edx, "stack": list(st),
                 "C": self.u32(CLIENT_UPDATES)}
            for src, u in (("ecx", ctx.Ecx), ("edx", ctx.Edx), ("s0", st[0])):
                if self.is_unit(u):
                    r["via"] = src
                    r["u"] = self.extra(u, self.fields(u))
                    break
            self.emit(r)
        elif addr in (H_WUPD, H_WMOVE):
            pool = self.u32(W_POOL)
            self.emit({"k": "wu" if addr == H_WUPD else "wm", "C": self.u32(CLIENT_UPDATES),
                       "mark": self.u32(W_MARK), "F": self.u32(W_F),
                       "live": self.i32(pool + POOL_LIVE) if pool else None,
                       "rain": self.u32(W_RAIN), "target": self.i32(W_TARGET)})
        elif addr == H_ADV and self.adv:
            st = struct.unpack("<2I", self.read(ctx.Esp + 4, 8))
            u = st[0]
            if self.is_unit(u) and not (self.u32(u + 0xC8) & rt.SERVER_UNIT):
                self.emit({"k": "adv", "ret": f"{self.u32(ctx.Esp):#x}", "a1": st[1],
                           **self.fields(u)})

    def emit(self, rec):
        self.n += 1
        super().emit(rec)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0, help="kill the game after N s")
    ap.add_argument("--ticks", type=int, default=0, help="stop after N server ticks (0: no limit)")
    ap.add_argument("--types", default="0,1", help="unit types logged (default players, monsters)")
    ap.add_argument("--adv", action="store_true", help="also log the frame advance 0x623E00")
    ap.add_argument("--weather", action="store_true",
                    help="also log the weather update 0x473F50 and the particle move 0x4732C0")
    ap.add_argument("--arm-level", type=int, default=None,
                    help="arm the animation hooks only once the player is in this level")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-anim.jsonl)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"])
    autostart.add_options(ap)
    a = ap.parse_args()
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-anim.jsonl")
    rt.EXPECT = {k: v for k, v in rt.EXPECT.items() if k == rt.TICK}
    rt.EXPECT.update(ANIM_EXPECT if a.adv else {k: v for k, v in ANIM_EXPECT.items() if k != H_ADV})
    if a.weather:
        rt.EXPECT.update(WEATHER_EXPECT)
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    types = {int(x) for x in a.types.split(",")}
    r = AnimRecorder(os.path.abspath(a.game), gargs, out, a.seconds, a.ticks, types, a.adv, a.arm_level)
    r.auto = auto
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    print(f"wrote {out}: {r.ticks} ticks, {r.n} records")


if __name__ == "__main__":
    main()
