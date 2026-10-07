"""Record the server tick of the original 1.14d Game.exe: tick and step
markers, every timer event scheduled, cancelled and run, and every change
to the unit, room and update-queue lists, plus periodic snapshots of those
lists.

A minimal Windows debugger (ctypes + the Win32 debug API, standard library
only; the Win32 definitions are shared with record_rng.py). It starts
game/Game.exe under DEBUG_ONLY_THIS_PROCESS and plants persistent INT3s
(each one is stepped over and re-armed). Every hooked address, register
and offset is documented in specs/sim/tick.md and specs/sim/unit-order.md
(each constant below names its section). Output: one JSON line per
record in traces/raw/<time>-tick.jsonl (format tick-raw-1, README.md);
checked by check_tick.py.

Only the first game that ticks is recorded. Records from client-side
copies of the shared room and unit code (client DRLG, client units) are
dropped: units need the server flag (unit +0xC8 bit 0x04000000) and rooms
must belong to one of the recorded game's acts.

The game process is always terminated when this script ends (time limit,
Ctrl+C, any error, and kill-on-exit if the debugger dies).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import ctypes as C
import datetime
import hashlib
import json
import os
import struct
import sys
import time

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import record_rng as rr  # noqa: E402  (the shared Win32 debugger definitions)
import autostart  # noqa: E402  (unattended start, input script)

TOOL = "trace-recorder record_tick 0.2.0"
FORMAT = "tick-raw-1"

# Game record offsets (tick.md, unit-order.md)
G_FRAME, G_CLIENTS, G_ACTS = 0xA8, 0x88, 0xBC
HASH_BASE = 0x1120
HASH_OFFSETS = {0: 0x000, 1: 0x200, 2: 0x400, 4: 0x600, 3: 0x800}  # unit type -> offset
TILE_LIST = 0x1B20
U_TYPE, U_CLASS, U_GUID, U_MODE, U_FLAGS2 = 0x00, 0x04, 0x0C, 0x10, 0xC8
# animation fields read by the mode schedulers (units.md §4)
U_SEQ, U_SEQ_FC, U_SEQ_SPEED, U_CUR, U_FC, U_SPEED, U_ANIMDATA = 0x30, 0x34, 0x3C, 0x44, 0x48, 0x4C, 0x50
U_HASH_NEXT, U_ROOM_NEXT, U_UPD_NEXT = 0xE4, 0xE8, 0xE0
SERVER_UNIT = 0x04000000
ACT_ROOMS, ROOM_NEXT, ROOM_UNITS, ROOM_UPD, ROOM_ACT = 0x10, 0x7C, 0x74, 0x1C, 0x2C
ROOM_ADJ, ROOM_ADJ_N = 0x00, 0x24
C_NEXT = 0x4A8
T_TYPE, T_FLAGS, T_EXPIRE, T_UNIT, T_A1, T_A2, T_CB = 0x00, 0x02, 0x04, 0x08, 0x14, 0x18, 0x2C
CLASS_OF_TYPE = {0: 0, 1: 1, 3: 2, 2: 3, 4: 4}  # unit type -> timer class (0x006E0B9C)

TICK = 0x52D870
# call sites inside the tick (tick.md §3): address -> step name
STEPS = {
    0x52D8DF: "env", 0x52D8EB: "rooms", 0x52D8F2: "events", 0x52D8F9: "clients",
    0x52D900: "updq", 0x52D907: "dels", 0x52D920: "quests", 0x52D938: "deact",
    0x52D95F: "inactive", 0x52D96C: "items", 0x52D971: "end",
}
# timer runs (tick.md §5.5): address of `or word ptr [esi+2], 1`, ESI = timer
RUNS = {
    0x541278: (2, "d"), 0x5412CF: (2, "i"), 0x540F98: (0, "d"), 0x540FFC: (0, "i"),
    0x541098: (1, "d"), 0x5410FC: (1, "i"), 0x541198: (3, "d"), 0x5411EF: (3, "i"),
    0x541358: (4, "d"), 0x5413AF: (4, "i"),
}
SET_ENTRY = 0x5416B0   # ECX type, EAX requested expire, EDX game, [esp+4] unit (tick.md §5.2)
SET_TIMED = 0x541721   # after allocation: EAX timer, ESI final expire, EBX type, [ebp+8..] unit, cb, a1, a2
SET_EVERY = 0x5415F2   # after allocation: EAX timer, ESI unit, EBX type, [ebp+8..] cb, a1, a2 (§5.3)
CANCEL = 0x540CD0      # ESI timer, EDI game (§5.4)
HASH_IN = 0x553060     # ECX unit, EDX game (unit-order.md §2.1)
HASH_OUT = 0x5530F0    # ESI unit, EDX game (§2.2)
ROOM_IN = 0x64C2C0     # [esp+4] unit, [esp+8] room (§5.2)
ROOM_OUT = 0x64C370    # [esp+4] unit (§5.3)
UPD_IN = 0x64C0AD      # the prepend: ESI unit, EAX = &room->update head (§6.2)
UPD_OUT = 0x64C1B0     # [esp+4] unit (§5.3)
UPD_CLEAR = 0x64C160   # [esp+4] room (§6.4)
ROOM_ACTIVATE = 0x619925  # after the prepend: EBX room, EDI act (§4.2)
ROOM_DEACTIVATE = 0x61A910  # [esp+4] act, [esp+8] room (§4.3)
# mode animation schedulers (units.md §4): after the frame-bonus call in 0x5539B0
# (EAX bonus, ESI unit, EDI game), and the entries of the three variants
# (ECX game, EDX unit, [esp+4] argument)
ANIM_MAIN = 0x5539CC
ANIM_VARIANTS = {0x553B10: "0x553b10", 0x553C70: "0x553c70", 0x553DC0: "0x553dc0"}
SCHED_WRAPPERS = (0x5416B0, 0x541830)  # 0x5416B0 .. end of 0x541800: not a caller

EXPECT = {  # first bytes at each hook: refuses any other executable layout
    TICK: b"\x53\x56\x57", SET_ENTRY: b"\x55\x8B\xEC", CANCEL: b"\x0F\xB7\x46\x02",
    HASH_IN: b"\x8B\x01", HASH_OUT: b"\x8B\x06", ROOM_IN: b"\x55\x8B\xEC",
    ROOM_OUT: b"\x55\x8B\xEC", UPD_IN: b"\x8B\x08", UPD_CLEAR: b"\x55\x8B\xEC",
    SET_TIMED: b"\x83\xFE\xFF", SET_EVERY: b"\x8B\x4D\x0C", ROOM_ACTIVATE: b"\xC7\x47\x54",
    ROOM_DEACTIVATE: b"\x55\x8B\xEC", UPD_OUT: b"\x55\x8B\xEC",
    **{a: b"\x66\x83\x4E\x02\x01" for a in RUNS}, **{a: b"\xE8" for a in STEPS if a != 0x52D971},
    0x52D971: b"\x5F\x5E\x5B", ANIM_MAIN: b"\xC1\xE0\x08",
    **{a: b"\x55\x8B\xEC" for a in ANIM_VARIANTS},
}


class TickRecorder:
    auto = None  # autostart.AutoStart (unattended start, input script)

    def __init__(self, exe, args, out_path, seconds, snap_every, max_ticks):
        self.exe, self.args, self.out_path, self.seconds = exe, args, out_path, seconds
        self.snap_every, self.max_ticks = snap_every, max_ticks
        self.h_process = None
        self.threads = {}
        self.bp_orig = {}
        self.reinsert = {}
        self.pending = None
        self.notes = []
        self.initial_bps = 0
        self.game = None
        self.frame = None
        self.ticks = 0
        self.counts = {}
        self.set_req = {}  # tid -> requested expire from SET_ENTRY
        self.out = None
        self.done = False

    read = rr.Recorder.read
    read_u32 = rr.Recorder.read_u32
    write = rr.Recorder.write
    get_ctx = rr.Recorder.get_ctx
    set_ctx = rr.Recorder.set_ctx
    kill = rr.Recorder.kill
    close_event_handles = staticmethod(rr.Recorder.close_event_handles)

    # --- helpers -------------------------------------------------------------

    def u32(self, addr):
        return self.read_u32(addr)

    def i32(self, addr):
        return struct.unpack("<i", self.read(addr, 4))[0]

    def emit(self, rec):
        self.counts[rec["k"]] = self.counts.get(rec["k"], 0) + 1
        self.out.write(json.dumps(rec, separators=(",", ":")) + "\n")

    def unit_id(self, u):
        """(type, guid, class) of a unit, or None for a client-side unit."""
        if not u:
            return None
        if not self.u32(u + U_FLAGS2) & SERVER_UNIT:
            return None
        return self.u32(u + U_TYPE), self.u32(u + U_GUID), self.u32(u + U_CLASS)

    def site_of(self, ebp):
        """Call site of the public scheduling wrapper (units.md, Test vectors):
        the caller's return address two frames up, minus the 5-byte call."""
        fp = self.u32(ebp)
        ret = self.u32(fp + 4)
        if SCHED_WRAPPERS[0] <= ret < SCHED_WRAPPERS[1]:  # every-tick via 0x5416B0
            ret = self.u32(self.u32(fp) + 4)
        return f"{ret - 5:#x}"

    def unit_more(self, u):
        """(class, mode) of a server unit at schedule time, or (None, None)."""
        if not u or not self.u32(u + U_FLAGS2) & SERVER_UNIT:
            return None, None
        return self.u32(u + U_CLASS), self.u32(u + U_MODE)

    def anim(self, fn, unit, bonus=None, arg=None):
        """The unit fields a mode animation schedule reads (units.md §4)."""
        uid = self.unit_id(unit)
        if not uid:
            return
        seq = self.u32(unit + U_SEQ)
        ad = self.u32(unit + U_ANIMDATA)
        rec = {"k": "anim", "fn": fn, "f": self.frame, "ut": uid[0], "g": uid[1], "cl": uid[2],
               "m": self.u32(unit + U_MODE), "seq": seq != 0, "cur": self.i32(unit + U_CUR)}
        if seq:
            rec["fc"], rec["sp"] = self.i32(unit + U_SEQ_FC), self.i32(unit + U_SEQ_SPEED)
        else:
            rec["fc"] = self.i32(unit + U_FC)
            rec["sp"] = struct.unpack("<h", self.read(unit + U_SPEED, 2))[0]
        if bonus is not None:
            rec["b"] = bonus
        if arg is not None:
            rec["arg"] = arg
        if ad:
            raw = self.read(ad, 0xA0)
            rec["ad"] = raw[:8].split(b"\0")[0].decode("latin-1")
            rec["ad_frames"], rec["ad_speed"] = struct.unpack_from("<II", raw, 8)
            rec["ev"] = [[i, b] for i, b in enumerate(raw[0x10:0xA0]) if b]
        self.emit(rec)

    def acts(self):
        return [self.u32(self.game + G_ACTS + 4 * i) for i in range(5)]

    def server_room(self, room):
        return room and self.game and self.u32(room + ROOM_ACT) in [a for a in self.acts() if a]

    def walk(self, head, nxt, limit=100000):
        out, p = [], head
        while p and len(out) < limit:
            out.append(p)
            p = self.u32(p + nxt)
        return out

    def ids(self, units):
        res = []
        for u in units:
            res.append([self.u32(u + U_TYPE), self.u32(u + U_GUID)])
        return res

    def snapshot(self, frame):
        g = self.game
        hashes = {}
        for t, off in HASH_OFFSETS.items():
            buckets = []
            for b in range(128):
                head = self.u32(g + HASH_BASE + off + 4 * b)
                if head:
                    buckets.append([b, [self.u32(u + U_GUID) for u in self.walk(head, U_HASH_NEXT)]])
            hashes[str(t)] = buckets
        tiles = [self.u32(u + U_GUID) for u in self.walk(self.u32(g + TILE_LIST), U_HASH_NEXT)]
        acts = []
        for i, act in enumerate(self.acts()):
            if not act:
                acts.append(None)
                continue
            rooms = []
            for r in self.walk(self.u32(act + ACT_ROOMS), ROOM_NEXT):
                n = self.u32(r + ROOM_ADJ_N)
                adj = list(struct.unpack(f"<{n}I", self.read(self.u32(r + ROOM_ADJ), 4 * n))) \
                    if 0 < n < 256 else []
                rooms.append({"r": f"{r:#x}",
                              "u": self.ids(self.walk(self.u32(r + ROOM_UNITS), U_ROOM_NEXT)),
                              "q": self.ids(self.walk(self.u32(r + ROOM_UPD), U_UPD_NEXT)),
                              "adj": [f"{a:#x}" for a in adj]})
            acts.append({"a": f"{act:#x}", "rooms": rooms})
        clients = [f"{c:#x}" for c in self.walk(self.u32(g + G_CLIENTS), C_NEXT)]
        self.emit({"k": "snap", "f": frame, "hash": hashes, "tiles": tiles, "acts": acts,
                   "clients": clients})

    # --- breakpoints ----------------------------------------------------------

    def arm(self, addr):
        if addr not in self.bp_orig:
            self.bp_orig[addr] = self.read(addr, 1)[0]
            self.write(addr, rr.INT3)

    def on_breakpoint(self, tid, addr):
        ctx = self.get_ctx(tid)
        ctx.Eip = addr
        try:
            self.handle(addr, ctx)
        finally:
            # step over the original instruction, then re-arm
            self.write(addr, bytes([self.bp_orig[addr]]))
            self.reinsert[tid] = addr
            ctx.EFlags |= rr.TRAP_FLAG
            self.set_ctx(tid, ctx)

    def handle(self, addr, ctx):
        if addr == TICK:
            game = ctx.Ecx
            if self.game is None:
                self.game = game
                self.emit({"k": "game", "g": f"{game:#x}", "frame_before": self.i32(game + G_FRAME)})
            if game != self.game:
                return
            self.frame = self.i32(game + G_FRAME) + 1
            self.ticks += 1
            if self.snap_every and (self.ticks == 1 or self.frame % self.snap_every == 0):
                self.snapshot(self.frame)  # the state the tick starts from
            self.emit({"k": "tick", "f": self.frame})
            if self.max_ticks and self.ticks > self.max_ticks:
                self.notes.append(f"tick limit {self.max_ticks} reached")
                self.done = True
            return
        if self.game is None:
            return
        if addr in STEPS:
            if ctx.Edi == self.game:  # EDI holds the game for the whole tick
                self.emit({"k": "step", "s": STEPS[addr], "f": self.frame})
        elif addr in RUNS:
            if ctx.Edi != self.game:
                return
            cls, lst = RUNS[addr]
            tm = ctx.Esi
            u = self.u32(tm + T_UNIT)
            uid = self.unit_id(u) or (None, None, None)
            self.emit({"k": "ex", "c": cls, "l": lst, "tm": f"{tm:#x}",
                       "ty": self.read(tm + T_TYPE, 1)[0], "x": self.i32(tm + T_EXPIRE),
                       "ut": uid[0], "g": uid[1], "cl": uid[2],
                       "a1": self.i32(tm + T_A1), "a2": self.i32(tm + T_A2),
                       "cb": f"{self.u32(tm + T_CB):#x}"})
        elif addr == SET_ENTRY:
            if ctx.Edx == self.game:
                # keyed by the entry ESP; SET_TIMED finds it at EBP + 4 (after push ebp)
                self.set_req[ctx.Esp] = ctx.Eax
        elif addr == SET_TIMED:
            if ctx.Edi != self.game:
                return
            ebp = ctx.Ebp
            unit, cb, a1, a2 = struct.unpack("<4I", self.read(ebp + 8, 16))
            uid = self.unit_id(unit) or (None, None, None)
            cl, mode = self.unit_more(unit)
            req = self.set_req.pop(ebp + 4, None)
            self.emit({"k": "set", "l": "d", "tm": f"{ctx.Eax:#x}", "ty": ctx.Ebx & 0xFF,
                       "x": C.c_int32(ctx.Esi).value, "req": None if req is None
                       else C.c_int32(req).value, "ut": uid[0], "g": uid[1],
                       "c": CLASS_OF_TYPE.get(uid[0]), "a1": C.c_int32(a1).value,
                       "a2": C.c_int32(a2).value, "cb": f"{cb:#x}", "site": self.site_of(ebp),
                       "cl": cl, "m": mode})
        elif addr == SET_EVERY:
            if ctx.Edi != self.game:
                return
            ebp = ctx.Ebp
            cb, a1, a2 = struct.unpack("<3I", self.read(ebp + 8, 12))
            uid = self.unit_id(ctx.Esi) or (None, None, None)
            cl, mode = self.unit_more(ctx.Esi)
            self.emit({"k": "set", "l": "i", "tm": f"{ctx.Eax:#x}", "ty": ctx.Ebx & 0xFF,
                       "x": -1, "req": -1, "ut": uid[0], "g": uid[1],
                       "c": CLASS_OF_TYPE.get(uid[0]), "a1": C.c_int32(a1).value,
                       "a2": C.c_int32(a2).value, "cb": f"{cb:#x}", "site": self.site_of(ebp),
                       "cl": cl, "m": mode})
        elif addr == ANIM_MAIN:
            if ctx.Edi == self.game:
                self.anim("0x5539b0", ctx.Esi, bonus=C.c_int32(ctx.Eax).value)
        elif addr in ANIM_VARIANTS:
            if ctx.Ecx == self.game:
                self.anim(ANIM_VARIANTS[addr], ctx.Edx,
                          arg=C.c_int32(self.u32(ctx.Esp + 4)).value)
        elif addr == CANCEL:
            if ctx.Edi != self.game:
                return
            tm = ctx.Esi
            flags = struct.unpack("<H", self.read(tm + T_FLAGS, 2))[0]
            self.emit({"k": "cancel", "tm": f"{tm:#x}", "deferred": bool(flags & 1)})
        elif addr == HASH_IN:
            uid = self.unit_id(ctx.Ecx)
            if uid and ctx.Edx == self.game:
                self.emit({"k": "hin", "u": f"{ctx.Ecx:#x}", "ut": uid[0], "g": uid[1], "cl": uid[2]})
        elif addr == HASH_OUT:
            uid = self.unit_id(ctx.Esi)
            if uid and ctx.Edx == self.game:
                self.emit({"k": "hout", "u": f"{ctx.Esi:#x}", "ut": uid[0], "g": uid[1]})
        elif addr in (ROOM_IN, ROOM_OUT, UPD_OUT):
            unit, room = struct.unpack("<2I", self.read(ctx.Esp + 4, 8))
            uid = self.unit_id(unit)
            if not uid:
                return
            kind = {ROOM_IN: "rin", ROOM_OUT: "rout", UPD_OUT: "qout"}[addr]
            rec = {"k": kind, "u": f"{unit:#x}", "ut": uid[0], "g": uid[1]}
            if addr == ROOM_IN:
                if not self.server_room(room):
                    return
                rec["r"] = f"{room:#x}"
            self.emit(rec)
        elif addr == UPD_IN:
            room = ctx.Eax - ROOM_UPD
            uid = self.unit_id(ctx.Esi)
            if uid and self.server_room(room):
                self.emit({"k": "qin", "u": f"{ctx.Esi:#x}", "ut": uid[0], "g": uid[1],
                           "r": f"{room:#x}"})
        elif addr == UPD_CLEAR:
            room = self.u32(ctx.Esp + 4)
            if self.server_room(room):
                self.emit({"k": "qclear", "r": f"{room:#x}"})
        elif addr == ROOM_ACTIVATE:
            if ctx.Edi in self.acts():
                self.emit({"k": "ract", "r": f"{ctx.Ebx:#x}", "a": f"{ctx.Edi:#x}"})
        elif addr == ROOM_DEACTIVATE:
            act, room = struct.unpack("<2I", self.read(ctx.Esp + 4, 8))
            if act in self.acts():
                self.emit({"k": "rdeact", "r": f"{room:#x}", "a": f"{act:#x}"})

    def on_single_step(self, tid):
        addr = self.reinsert.pop(tid, None)
        if addr is not None:
            self.write(addr, rr.INT3)

    # --- process control ------------------------------------------------------

    def run(self):
        exe_bytes = open(self.exe, "rb").read()
        self.sha = hashlib.sha256(exe_bytes).hexdigest()
        if self.sha != rr.GAME_EXE_SHA256:
            raise RuntimeError(f"{self.exe}: sha256 {self.sha} is not the reference 1.14d Game.exe")
        si = rr.STARTUPINFOW()
        si.cb = C.sizeof(si)
        pi = rr.PROCESS_INFORMATION()
        cmd = C.create_unicode_buffer(" ".join([f'"{self.exe}"'] + self.args))
        if not rr.CreateProcessW(self.exe, cmd, None, None, False, rr.DEBUG_ONLY_THIS_PROCESS,
                                 None, os.path.dirname(self.exe), C.byref(si), C.byref(pi)):
            raise rr.winerr("CreateProcessW")
        self.h_process = pi.hProcess
        rr.DebugSetProcessKillOnExit(True)
        os.makedirs(os.path.dirname(self.out_path), exist_ok=True)
        self.out = open(self.out_path, "w", encoding="utf-8", newline="\n")
        self.out.write(json.dumps({"k": "header", "format": FORMAT, "tool": TOOL,
                                   "date": datetime.date.today().isoformat(),
                                   "game_exe_sha256": self.sha, "args": self.args,
                                   "snap_every": self.snap_every}) + "\n")
        t0 = time.perf_counter()
        try:
            self.loop(t0 + self.seconds)
        finally:
            self.kill()
            rr.CloseHandle(pi.hThread)
            self.out.write(json.dumps({"k": "footer", "ticks": self.ticks, "counts": self.counts,
                                       "notes": self.notes}) + "\n")
            self.out.close()

    def loop(self, deadline):
        ev = rr.DEBUG_EVENT()
        while not self.done:
            if self.auto is not None and self.auto.poll(self):
                self.notes.append("autostart: input script ended the recording")
                return
            if time.perf_counter() > deadline:
                self.notes.append(f"time limit {self.seconds}s reached")
                return
            if not rr.WaitForDebugEvent(C.byref(ev), 100):
                continue
            code, tid = ev.dwDebugEventCode, ev.dwThreadId
            self.pending = (ev.dwProcessId, tid)
            status = rr.DBG_CONTINUE
            if code == rr.CREATE_PROCESS_DEBUG_EVENT:
                info = ev.u.CreateProcessInfo
                self.threads[tid] = info.hThread
                if info.hFile:
                    rr.CloseHandle(info.hFile)
                if (info.lpBaseOfImage or 0) != rr.IMAGE_BASE:
                    raise RuntimeError("Game.exe not loaded at its image base")
                for addr, want in EXPECT.items():
                    if self.read(addr, len(want)) != want:
                        raise RuntimeError(f"unexpected code at {addr:#x}: not the 1.14d Game.exe?")
                for addr in EXPECT:
                    self.arm(addr)
            elif code == rr.CREATE_THREAD_DEBUG_EVENT:
                self.threads[tid] = ev.u.CreateThread.hThread
            elif code == rr.EXIT_THREAD_DEBUG_EVENT:
                self.threads.pop(tid, None)
            elif code == rr.LOAD_DLL_DEBUG_EVENT:
                if ev.u.LoadDll.hFile:
                    rr.CloseHandle(ev.u.LoadDll.hFile)
            elif code == rr.EXIT_PROCESS_DEBUG_EVENT:
                self.notes.append(f"game exited, code {ev.u.ExitProcess.dwExitCode:#x}")
                rr.ContinueDebugEvent(ev.dwProcessId, tid, rr.DBG_CONTINUE)
                self.pending = None
                return
            elif code == rr.EXCEPTION_DEBUG_EVENT:
                status = self.on_exception(tid, ev.u.Exception)
            rr.ContinueDebugEvent(ev.dwProcessId, tid, status)
            self.pending = None

    def on_exception(self, tid, info):
        rec = info.ExceptionRecord
        code, addr = rec.ExceptionCode, rec.ExceptionAddress or 0
        if code in rr.BREAKPOINT_CODES:
            if addr in self.bp_orig:
                self.on_breakpoint(tid, addr)
                return rr.DBG_CONTINUE
            if self.initial_bps < 2:
                self.initial_bps += 1
                return rr.DBG_CONTINUE
            return rr.DBG_EXCEPTION_NOT_HANDLED
        if code in rr.SINGLE_STEP_CODES and tid in self.reinsert:
            self.on_single_step(tid)
            return rr.DBG_CONTINUE
        return rr.DBG_EXCEPTION_NOT_HANDLED


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0,
                    help="kill the game after this many seconds (default 120)")
    ap.add_argument("--ticks", type=int, default=0,
                    help="stop after this many recorded ticks (default: no limit)")
    ap.add_argument("--snap-every", type=int, default=25,
                    help="list snapshot at the first tick and every N frames (default 25; 0 = none)")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-tick.jsonl)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"],
                    help="Game.exe arguments (default: -w -ns)")
    autostart.add_options(ap)
    a = ap.parse_args()
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-tick.jsonl")
    r = TickRecorder(os.path.abspath(a.game), gargs, out, a.seconds,
                     a.snap_every, a.ticks)
    r.auto = auto
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    print(f"wrote {out}: {r.ticks} ticks, {r.counts}")


if __name__ == "__main__":
    main()
