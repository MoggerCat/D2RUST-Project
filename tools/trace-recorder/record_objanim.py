"""Record the object animation set-up of the original 1.14d Game.exe: every
call of the animation re-init `0x00624390` on an object unit (server or
client), with the unit's seed, frame, frame count and speed before and
after, the call's return address and the code addresses on the stack
(which caller ran it), plus the entries of the client object init
`0x004BC720` (S->C 0x51) and the client object mode change `0x004BCF60`
(S->C 0x0E code 3) and the server tick markers.

Settles REC-440 (`world/objects-client.md` §25 r8: which client call runs
the set-up, and whether `reinit` / `set_mode` of the client functions
draw). Addresses: `world/objects.md` §4, `world/objects-client.md` §25 r5,
`client/msg-units.md` §1.3, `client/model.md` §8 r5, `sim/tick.md` (the
tick hook of record_tick.py), unit offsets `sim/units.md` §1 (+0x20 seed).

The unit argument's register is not assumed: at the entry every general
register and the first two stack arguments are tried, and each that
points at a record whose type dword is 2 (object) is followed to the
return. Our own code. Nothing here is derived from Blizzard code.

  py tools/trace-recorder/record_objanim.py --seconds 200 --auto ScnAma --seed 1234 \
      --input "wait 20; end" --out traces/raw/objanim.jsonl
  py tools/trace-recorder/record_objanim.py --steps --range ... (also the generic
      step 0x004BCBB0, the range test 0x00623660 and the 0x13 object case 0x00548B00)
  py tools/trace-recorder/record_objanim.py --selftest
"""

import argparse
import ctypes as C
import datetime
import os
import struct
import sys

sys.dont_write_bytecode = True
import record_tick as rt  # noqa: E402
import record_rng as rr  # noqa: E402
import autostart  # noqa: E402

TOOL = "trace-recorder record_objanim 0.2.0"
FORMAT = "objanim-raw-1"

ANIMSET = 0x624390     # animation re-init (objects.md §4)
SET_MODE = 0x624690    # mode set (sim/units.md §4.1)
OBJ_INIT = 0x4BC720    # client object init at S->C 0x51 (msg-units.md §1.3)
OBJ_MODE = 0x4BCF60    # client object mode change, 0x0E code 3 (model.md §8 r5)
GSTEP = 0x4BCBB0       # client generic object step (objects-client.md §25, §26.16)
RANGE = 0x623660       # interact range test (P, U) (objects.md §7.1 r3, ui/controls.md §6 r9.2)
OP13 = 0x548B00        # C->S 0x13 object case (objects.md §7.3)
U_SEED = 0x20
CODE_LO, CODE_HI = 0x401000, 0x6F0000


def unit_fields(read, u):
    """The fields the set-up writes, from a reader read(addr, n) -> bytes."""
    head = read(u, 0x10)
    typ, cls, _, guid = struct.unpack("<4I", head)
    mode = struct.unpack("<I", read(u + 0x10, 4))[0]
    lo, hi = struct.unpack("<2I", read(u + U_SEED, 8))
    cur, fc = struct.unpack("<iI", read(u + 0x44, 8))
    sp = struct.unpack("<h", read(u + 0x4C, 2))[0]
    flags2 = struct.unpack("<I", read(u + rt.U_FLAGS2, 4))[0]
    return {"t": typ, "cl": cls, "g": guid, "m": mode, "srv": bool(flags2 & rt.SERVER_UNIT),
            "seed": [lo, hi], "cur": cur, "fc": fc, "sp": sp}


def pos(read, u):
    """Sub-tile position: static path (types 2, 4, 5) +0x0C/+0x10, else
    the dynamic path's 16.16 fixed point (record_frames.read_unit)."""
    t = struct.unpack("<I", read(u, 4))[0]
    path = struct.unpack("<I", read(u + 0x2C, 4))[0]
    if not path:
        return None
    if t in (2, 4, 5):
        return list(struct.unpack("<2i", read(path + 0x0C, 8)))
    x, y = struct.unpack("<2I", read(path, 8))
    return [x >> 16, y >> 16, x & 0xFFFF, y & 0xFFFF]


def stack_code(read, esp, n=8, depth=0x180):
    """The first n dwords on the stack that point into Game.exe's code."""
    raw = read(esp, depth)
    out = []
    for (v,) in struct.iter_unpack("<I", raw):
        if CODE_LO <= v < CODE_HI:
            out.append(f"{v:#x}")
            if len(out) == n:
                break
    return out


class ObjAnimRecorder(rt.TickRecorder):
    def __init__(self, *a, **k):
        super().__init__(*a, **k)
        self.pending_ret = {}  # (tid, ret addr) -> list of (entry esp, unit, before, stack)
        self.ret_bps = set()
        self.gstep, self.gwraps = {}, {}

    def read(self, addr, n):
        return b"" if n == 0 else rt.TickRecorder.read(self, addr, n)

    def obj_ptr(self, v, types=(2,)):
        if not v or v < 0x10000:
            return False
        try:
            return self.u32(v) in types and self.u32(v + 0x10) < 32 and self.u32(v + 0x2C) > 0x10000
        except OSError:
            return False

    def brief(self, u):
        f = unit_fields(self.read, u)
        return {"t": f["t"], "cl": f["cl"], "g": f["g"], "m": f["m"], "srv": f["srv"],
                "pos": pos(self.read, u)}

    def handle(self, addr, ctx):
        if addr == rt.TICK:
            return super().handle(addr, ctx)
        key = (self.pending_tid, addr)
        if addr in self.ret_bps and key in self.pending_ret:
            keep = []
            for esp, u, before, stack, fn in self.pending_ret.pop(key):
                if ctx.Esp <= esp:  # a deeper (recursive) call returns first
                    keep.append((esp, u, before, stack, fn))
                    continue
                if fn == "range":
                    self.emit({"k": "range", "f": self.frame, "eax": ctx.Eax, "units": before,
                               "stack": stack})
                    continue
                after = unit_fields(self.read, u)
                self.emit({"k": fn, "f": self.frame, "u": f"{u:#x}", "before": before,
                           "after": after, "drew": before["seed"] != after["seed"],
                           "stack": stack})
            if keep:
                self.pending_ret[key] = keep
        if addr == GSTEP:
            for v in (ctx.Ecx, ctx.Edx, ctx.Esi, self.u32(ctx.Esp + 4)):
                if self.obj_ptr(v):
                    f = unit_fields(self.read, v)
                    last = self.gstep.get(v)
                    now = (f["m"], f["cur"])
                    self.gstep[v] = now
                    n = self.gwraps.get(v, 0)
                    if last is None or last[0] != now[0] or (now[1] < last[1] and n < 3):
                        if last is not None and last[0] == now[0]:
                            self.gwraps[v] = n + 1
                        self.emit({"k": "gstep", "f": self.frame, "u": f"{v:#x}", "last": last,
                                   "unit": f})
                    break
            return
        if addr in (RANGE, OP13):
            esp = ctx.Esp
            a1, a2, a3 = struct.unpack("<3I", self.read(esp + 4, 12))
            units = []
            for v in [ctx.Ecx, ctx.Edx, ctx.Esi, ctx.Edi, ctx.Ebx, ctx.Eax, a1, a2, a3]:
                if v not in units and self.obj_ptr(v, (0, 1, 2, 5)):
                    units.append(v)
            stack = stack_code(self.read, esp, n=4)
            if addr == OP13:
                self.emit({"k": "op13", "f": self.frame, "units": [self.brief(u) for u in units],
                           "regs": [f"{x:#x}" for x in (ctx.Ecx, ctx.Edx, a1, a2, a3)],
                           "stack": stack})
                return
            ret = self.u32(esp)
            if ret not in self.bp_orig:
                self.arm(ret)
            self.ret_bps.add(ret)
            self.pending_ret.setdefault((self.pending_tid, ret), []).append(
                (esp, 0, [self.brief(u) for u in units], stack, "range"))
            return
        if addr in (ANIMSET, SET_MODE, OBJ_INIT, OBJ_MODE):
            esp = ctx.Esp
            a1, a2 = struct.unpack("<2I", self.read(esp + 4, 8))
            cands = [ctx.Ecx, ctx.Edx, ctx.Esi, ctx.Edi, ctx.Ebx, ctx.Eax, a1, a2]
            units = []
            for v in cands:
                if v not in units and self.obj_ptr(v):
                    units.append(v)
            if not units:
                return
            name = {ANIMSET: "anim", SET_MODE: "setmode", OBJ_INIT: "init51",
                    OBJ_MODE: "mode0e"}[addr]
            stack = stack_code(self.read, esp)
            ret = self.u32(esp)
            if addr in (OBJ_INIT, OBJ_MODE):
                for u in units:
                    self.emit({"k": name, "f": self.frame, "u": f"{u:#x}",
                               "unit": unit_fields(self.read, u), "regs": {
                                   "ecx": f"{ctx.Ecx:#x}", "edx": f"{ctx.Edx:#x}",
                                   "a1": f"{a1:#x}", "a2": f"{a2:#x}"}, "stack": stack})
                return
            if ret not in self.bp_orig:
                self.arm(ret)
            self.ret_bps.add(ret)
            lst = self.pending_ret.setdefault((self.pending_tid, ret), [])
            for u in units:
                lst.append((esp, u, unit_fields(self.read, u), stack, name))

    def on_breakpoint(self, tid, addr):
        self.pending_tid = tid
        super().on_breakpoint(tid, addr)


def selftest():
    mem = bytearray(0x200)
    struct.pack_into("<4I", mem, 0, 2, 37, 0, 99)
    struct.pack_into("<I", mem, 0x10, 1)
    struct.pack_into("<2I", mem, 0x20, 0x1234, 666)
    struct.pack_into("<iI", mem, 0x44, 256, 0x1800)
    struct.pack_into("<h", mem, 0x4C, 191)
    struct.pack_into("<I", mem, 0xC8, 0)
    struct.pack_into("<3I", mem, 0x100, 5, 0x4BC7AB, 0x624700)

    def read(a, n):
        return bytes(mem[a:a + n])
    f = unit_fields(read, 0)
    assert f == {"t": 2, "cl": 37, "g": 99, "m": 1, "srv": False, "seed": [0x1234, 666],
                 "cur": 256, "fc": 0x1800, "sp": 191}, f
    assert stack_code(read, 0x100, depth=12) == ["0x4bc7ab", "0x624700"]
    struct.pack_into("<I", mem, 0x2C, 0x180)
    struct.pack_into("<2i", mem, 0x18C, 4900, 4200)
    assert pos(read, 0) == [4900, 4200]
    mem[0x4C] ^= 1  # perturbation must show
    assert unit_fields(read, 0)["sp"] != 191
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
    ap.add_argument("--steps", action="store_true",
                    help="also hook the client generic object step (mode changes and wraps)")
    ap.add_argument("--range", action="store_true",
                    help="also hook the interact range test and the C->S 0x13 object case")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"])
    autostart.add_options(ap)
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-objanim.jsonl")
    # only the tick marker of record_tick.py, plus this recorder's hooks
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], ANIMSET: b"", SET_MODE: b"", OBJ_INIT: b"",
                 OBJ_MODE: b""}
    if a.steps:
        rt.EXPECT[GSTEP] = b""
    if a.range:
        rt.EXPECT[RANGE] = b""
        rt.EXPECT[OP13] = b""
    rt.TOOL, rt.FORMAT = TOOL, FORMAT
    r = ObjAnimRecorder(os.path.abspath(a.game), gargs, out, a.seconds, 0, a.ticks)
    r.pending_tid = None
    r.auto = auto
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    print(f"wrote {out}: {r.ticks} ticks, {r.counts}")


if __name__ == "__main__":
    _ = C, rr
    main()
