"""Spawn a monster in the original 1.14d Game.exe and record its RNG draws.

Launches game/Game.exe under the debugger (base: record_rng.py's Recorder),
starts a single-player game, and at the end of server tick N calls one of
the game's own spawn entry points on the server thread with a monstats row
and a position near the player. Every seeded-RNG draw made during the call
is recorded with record_rng.py's hooks (helpers, setters and, unless
--no-inline, every inlined step), which are armed only for the call.

Procedure, entry points, conventions and draw sites:
specs/tools/original-hooks-spawn.md (§1 entries, §2 pointers, §5 call).
Start-up: specs/tools/original-hooks.md §5.4 (menu forced as dump_tables.py
does; the character comes from -name / -<class>).

Output traces/raw/<time>-spawn.jsonl (gitignored), format spawn-raw-1: the
rng-raw-1 header / draw / seed_set / footer records (check_rng.py reads it),
each draw with "in_call" (true = on the spawning thread during the call),
plus "call" (entry, arguments, result), "unit" (every monster that exists
after the call and not before) and "spawn" (summary with seed states and
draw counts) records.

The game is always terminated when the script ends. Never writes to game/.
Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import ctypes as C
import datetime
import json
import os
import struct
import sys
import time
from ctypes import wintypes as W

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record_rng as rr  # noqa: E402  (the Win32 debugger and the RNG hooks)

TOOL = "trace-recorder spawn 0.1.0"
RAW_FORMAT = "spawn-raw-1"

# specs/tools/original-hooks-spawn.md §1 / §2 / §5
TICK_RET = 0x52FD1E          # ESI = game, game+0xA8 = frame just run
TICK_RET_BYTES = bytes.fromhex("8B7618")
SPAWN = 0x5B2F20             # ECX game, EDX room; x, y, class, mode, spread, flags; ret 0x18
BOSS = 0x5A43E0              # ECX game, EDX room; cl, class, champ allowed, x, y, warp; ret 0x18
SUPERUNIQUE = 0x5A49B0       # ECX game, EDX room; x, y, row; ret 0xC
CHAMPION_MARK = 0x5A48C0     # ECX game, EDX unit; umod; ret 4
ROOM_AT = 0x463740           # ECX room, EDX x; y; ret 4
ENTRY_BYTES = {SPAWN: b"\x55\x8B\xEC", BOSS: b"\x55\x8B\xEC", SUPERUNIQUE: b"\x55\x8B\xEC",
               CHAMPION_MARK: b"\x55\x8B\xEC", ROOM_AT: b"\x55\x8B\xEC"}
DATA_TABLES = 0x744304       # +0xA78 monstats, +0xA80 count
G_FRAME, G_DIFF, G_SEED, G_CLIENTS = 0xA8, 0x6D, 0xD0, 0x88
HASH_BASE, MON_OFS, U_NEXT = 0x1120, 0x200, 0xE4
U_PATH, U_DATA, U_SEED, U_STATS = 0x2C, 0x14, 0x20, 0x5C
R_X, R_Y, R_W, R_H, R_SEED = 0x4C, 0x50, 0x54, 0x58, 0x6C
GAME_MODE, NEXT_MODE, MENU_LOOP = 0x74C704, 0x7795E8, 0x72DDD4   # dump_tables.py
STATS = {6: "hitpoints", 7: "maxhp", 12: "level", 13: "experience", 31: "armorclass"}

MEM_COMMIT_RESERVE = 0x3000
VirtualAllocEx = rr._proto("VirtualAllocEx", C.c_void_p, W.HANDLE, C.c_void_p, C.c_size_t,
                           W.DWORD, W.DWORD)


class SpawnError(RuntimeError):
    pass


class SpawnRecorder(rr.Recorder):
    def __init__(self, exe, args, out, a):
        super().__init__(exe, args, out, a.seconds, not a.no_inline, 0)
        self.a = a
        self.scratch = None
        self.game = None
        self.call_tid = None
        self.saved = None
        self.queue = []          # pending calls: (name, entry, ecx, edx, stack args, on_return)
        self.cur = None
        self.state = "start"     # start -> waiting -> calling -> after -> done
        self.forced = False
        self.spawn_frame = None
        self.before_guids = set()
        self.ctx_info = {}
        self.result = {}
        self.draws_in_call = 0
        self.draws_other = 0

    # --- helpers ----------------------------------------------------------
    def u16(self, a):
        return struct.unpack("<H", self.read(a, 2))[0]

    def i32(self, a):
        return struct.unpack("<i", self.read(a, 4))[0]

    def emit(self, rec):
        if rec.get("type") in ("draw", "seed_set"):
            inside = self.state == "calling" and rec.get("tid") == self.call_tid
            rec["in_call"] = inside
            if rec["type"] == "draw":
                if inside:
                    self.draws_in_call += 1
                else:
                    self.draws_other += 1
        super().emit(rec)

    def log(self, rec):
        rec = dict(rec)
        rec["seq"] = self.seq
        rec["ms"] = round((time.perf_counter() - self.t0) * 1000, 1)
        self.seq += 1
        self.count(rec["type"])
        self.out.write(json.dumps(rec, separators=(",", ":")) + "\n")
        self.out.flush()

    def player(self, game):
        u = self.read_u32(game + HASH_BASE + 4 * 1)
        while u:
            if self.read_u32(u) == 0 and self.read_u32(u + 0x0C) == 1:
                return u
            u = self.read_u32(u + U_NEXT)
        return 0

    def client_ready(self, game):
        c = self.read_u32(game + G_CLIENTS)
        return bool(c) and self.read_u32(c + 4) == 4

    def monsters(self, game):
        out = {}
        for b in range(128):
            u = self.read_u32(game + HASH_BASE + MON_OFS + 4 * b)
            n = 0
            while u and n < 10000:
                out[self.read_u32(u + 0x0C)] = u
                u = self.read_u32(u + U_NEXT)
                n += 1
        return out

    def stat_values(self, u):
        sl = self.read_u32(u + U_STATS)
        vals = {}
        if not sl:
            return vals
        arr, n = self.read_u32(sl + 0x48), struct.unpack("<h", self.read(sl + 0x4C, 2))[0]
        if not arr or n <= 0 or n > 512:
            return vals
        data = self.read(arr, 8 * n)
        for i in range(n):
            layer, stat, val = struct.unpack_from("<HHi", data, 8 * i)
            if layer == 0 and stat in STATS:
                vals[STATS[stat]] = val
        return vals

    def unit_record(self, u):
        rec = {"type": "unit", "addr": f"{u:#x}", "kind": self.read_u32(u),
               "class": self.read_u32(u + 4), "guid": self.read_u32(u + 0x0C),
               "mode": self.read_u32(u + 0x10),
               "seed": list(struct.unpack("<II", self.read(u + U_SEED, 8))),
               "init_seed": self.read_u32(u + 0x28)}
        path = self.read_u32(u + U_PATH)
        if path:
            rec["x"], rec["y"] = self.u16(path + 2), self.u16(path + 6)
        md = self.read_u32(u + U_DATA)
        if md:
            rec["type_flags"] = self.u16(md + 0x16)
            umods = self.read(md + 0x1C, 9)
            rec["umods"] = list(umods.split(b"\0", 1)[0])
            rec["name_seed"] = self.u16(md + 0x14)
            rec["hcidx"] = self.u16(md + 0x26)
            rec["components"] = list(self.read(md + 0x04, 16))
        rec.update(self.stat_values(u))
        return rec

    # --- set-up -------------------------------------------------------------
    def install(self, base):
        if base != rr.IMAGE_BASE:
            raise RuntimeError(f"Game.exe loaded at {base:#x}, expected {rr.IMAGE_BASE:#x}")
        if self.read(TICK_RET, 3) != TICK_RET_BYTES:
            raise RuntimeError("unexpected code at the tick return: not the 1.14d Game.exe?")
        for a, b in ENTRY_BYTES.items():
            if self.read(a, len(b)) != b:
                raise RuntimeError(f"unexpected code at {a:#x}: not the 1.14d Game.exe?")
        p = VirtualAllocEx(self.h_process, None, 0x1000, MEM_COMMIT_RESERVE,
                           rr.PAGE_EXECUTE_READWRITE)
        if not p:
            raise rr.winerr("VirtualAllocEx")
        self.scratch = p
        self.write(p, rr.INT3)
        self.add_role(TICK_RET, "ctl")
        self.state = "waiting"
        return 0

    def arm_rng(self):
        for a in rr.HELPERS:
            self.add_role(a, "helper")
        for a in rr.SETTERS:
            self.add_role(a, "setter")
        if self.with_inline:
            for a in self.inline_sites:
                self.add_role(a, "inline")

    def disarm_rng(self):
        for a in list(self.inline_sites) + list(rr.HELPERS) + list(rr.SETTERS):
            for role in ("inline", "helper", "setter"):
                self.drop_role(a, role)
        for addr in [a for a, r in self.roles.items() if "ret" in r]:
            self.drop_role(addr, "ret")
        self.ret_refs.clear()
        self.calls.clear()

    # --- the spawn --------------------------------------------------------
    def plan(self, tid, ctx, game):
        a = self.a
        pl = self.player(game)
        path = self.read_u32(pl + U_PATH)
        px, py, room = self.u16(path + 2), self.u16(path + 6), self.read_u32(path + 0x1C)
        x, y = px + a.dx, py + a.dy
        tables = self.read_u32(DATA_TABLES)
        n_mon = self.read_u32(tables + 0xA80)
        if a.superunique is None and not (0 <= a.cls < n_mon):
            raise SpawnError(f"class {a.cls} out of range (monstats has {n_mon} rows)")
        box = [self.read_u32(room + o) for o in (R_X, R_Y, R_W, R_H)]
        inside = box[0] <= x < box[0] + box[2] and box[1] <= y < box[1] + box[3]
        self.ctx_info = {"game": game, "player": pl, "px": px, "py": py, "x": x, "y": y,
                         "room": room, "box": box, "difficulty": self.read(game + G_DIFF, 1)[0]}
        self.before_guids = set(self.monsters(game))
        self.queue = []
        if not inside:
            self.queue.append(("room_at", ROOM_AT, room, x, [y], self.got_room))
        self.queue.append(None)  # placeholder: the main call, built once the room is known
        self.call_tid = tid
        self.saved = ctx
        self.state = "calling"
        self.spawn_frame = self.read_u32(game + G_FRAME)
        self.log({"type": "spawn_start", "frame": self.spawn_frame, "tid": tid,
                  "player": [px, py], "target": [x, y], "room": f"{room:#x}",
                  "room_box": box, "inside_player_room": inside,
                  "monsters_before": len(self.before_guids)})
        self.next_call(tid)

    def got_room(self, eax):
        if not eax:
            raise SpawnError("no loaded room holds the target point")
        self.ctx_info["room"] = eax
        return True

    def main_call(self):
        a, c = self.a, self.ctx_info
        g, r, x, y = c["game"], c["room"], c["x"], c["y"]
        if a.superunique is not None:
            return ("superunique", SUPERUNIQUE, g, r, [x, y, a.superunique], self.got_unit)
        if a.kind in ("unique", "boss"):
            return (a.kind, BOSS, g, r, [0, a.cls, 1 if a.kind == "boss" else 0, x & 0xFFFF,
                                         y & 0xFFFF, 0], self.got_unit)
        return ("normal", SPAWN, g, r, [x, y, a.cls, 1, a.spread & 0xFFFFFFFF, a.flags],
                self.got_unit)

    def got_unit(self, eax):
        self.result["unit"] = eax
        if eax and self.a.kind == "champion":
            self.queue.append(("champion_mark", CHAMPION_MARK, self.ctx_info["game"], eax,
                               [16], None))
        return True

    def next_call(self, tid):
        while self.queue and self.queue[0] is None:
            self.queue[0] = self.main_call()
        name, entry, ecx, edx, args, cb = self.queue.pop(0)
        if entry in (SPAWN, BOSS, SUPERUNIQUE) and not self.result.get("armed"):
            self.arm_rng()
            self.result["armed"] = True
            g = self.ctx_info["game"]
            self.result["g_before"] = list(struct.unpack("<II", self.read(g + G_SEED, 8)))
            r = self.ctx_info["room"]
            self.result["r_before"] = list(struct.unpack("<II", self.read(r + R_SEED, 8)))
        ctx = rr.WOW64_CONTEXT.from_buffer_copy(self.saved)
        esp = self.saved.Esp - 0x40 - 4 * len(args) - 4
        self.write(esp, struct.pack("<I", self.scratch) +
                   b"".join(struct.pack("<I", v & 0xFFFFFFFF) for v in args))
        ctx.Esp, ctx.Ecx, ctx.Edx, ctx.Eip = esp, ecx, edx, entry
        ctx.EFlags &= ~rr.TRAP_FLAG
        self.set_ctx(tid, ctx)
        self.cur = (name, entry, ecx, edx, args, cb, esp)
        self.log({"type": "call", "name": name, "entry": f"{entry:#x}", "ecx": f"{ecx:#x}",
                  "edx": f"{edx:#x}", "args": args})

    def on_call_return(self, tid):
        ctx = self.get_ctx(tid)
        name, entry, ecx, edx, args, cb, esp = self.cur
        ok_esp = ctx.Esp == esp + 4 + 4 * len(args)
        self.log({"type": "call_return", "name": name, "eax": f"{ctx.Eax:#x}",
                  "esp_ok": ok_esp})
        if not ok_esp:
            self.notes.append(f"{name}: callee left ESP at {ctx.Esp:#x}, expected "
                              f"{esp + 4 + 4 * len(args):#x}")
        try:
            if cb is not None:
                cb(ctx.Eax)
        except SpawnError as e:
            self.notes.append(f"spawn failed: {e}")
            self.queue = []
        if self.queue:
            self.next_call(tid)
            return
        self.finish_spawn(tid)

    def finish_spawn(self, tid):
        g = self.ctx_info["game"]
        if self.result.get("armed"):
            self.disarm_rng()
        mons = self.monsters(g)
        new = [mons[k] for k in sorted(mons) if k not in self.before_guids]
        for u in new:
            self.log(self.unit_record(u))
        summary = {"type": "spawn", "frame": self.spawn_frame,
                   "kind": "superunique" if self.a.superunique is not None else self.a.kind,
                   "class": self.a.cls, "superunique": self.a.superunique,
                   "unit": f"{self.result.get('unit', 0):#x}", "created": len(new),
                   "draws_in_call": self.draws_in_call, "draws_other_threads": self.draws_other,
                   "difficulty": self.ctx_info.get("difficulty")}
        if "g_before" in self.result:
            summary["g_before"] = self.result["g_before"]
            summary["g_after"] = list(struct.unpack("<II", self.read(g + G_SEED, 8)))
            r = self.ctx_info["room"]
            summary["r_before"] = self.result["r_before"]
            summary["r_after"] = list(struct.unpack("<II", self.read(r + R_SEED, 8)))
        self.log(summary)
        self.result["summary"] = summary
        # restore the thread and step over the hooked instruction (spec §5 step 6)
        self.set_ctx(tid, self.saved)
        self.state = "after"
        self.saved_done = True
        super().on_breakpoint(tid, TICK_RET)

    # --- events -------------------------------------------------------------
    def on_breakpoint(self, tid, addr):
        if addr == TICK_RET and self.state in ("waiting", "after") and tid not in self.stepping:
            ctx = self.get_ctx(tid)
            ctx.Eip = addr
            game = ctx.Esi
            frame = self.read_u32(game + G_FRAME)
            if self.game is None:
                self.game = game
                self.log({"type": "first_tick", "frame": frame, "game": f"{game:#x}",
                          "tid": tid})
            if game == self.game:
                if self.state == "waiting" and frame >= self.a.tick:
                    if self.player(game) and self.client_ready(game):
                        if frame > self.a.tick and not self.result.get("late_noted"):
                            self.notes.append(f"player ready only at frame {frame}")
                        try:
                            self.plan(tid, ctx, game)
                            return
                        except SpawnError as e:
                            self.notes.append(f"spawn refused: {e}")
                            self.state = "done"
                    elif not self.result.get("late_noted"):
                        self.result["late_noted"] = True
                        self.notes.append(f"frame {frame}: player not ready, waiting")
                elif self.state == "after" and frame >= self.spawn_frame + self.a.after_ticks:
                    self.state = "done"
        super().on_breakpoint(tid, addr)

    def on_exception(self, tid, info):
        rec = info.ExceptionRecord
        if rec.ExceptionCode in rr.BREAKPOINT_CODES and self.scratch is not None and \
                (rec.ExceptionAddress or 0) == self.scratch:
            if self.state == "calling" and tid == self.call_tid:
                st = self.stepping.pop(tid, None)  # an inline trace that ran into the trap
                if st:
                    self.finish_reinsert(tid, st)
                    if st.get("trace"):
                        self.notes.append("inline trace interrupted by the return trap")
                    if not self.stepping:
                        self.thaw_all()
                self.on_call_return(tid)
                return rr.DBG_CONTINUE
        return super().on_exception(tid, info)

    def maybe_force(self):
        if self.forced or self.a.no_force or self.game is not None:
            return
        if time.perf_counter() - self.t0 < self.a.force_after:
            return
        if self.read_u32(GAME_MODE) != 4:
            return
        self.write(NEXT_MODE, struct.pack("<I", 1))
        self.write(MENU_LOOP, struct.pack("<I", 0))
        self.forced = True
        self.notes.append(f"menu left for client mode at {time.perf_counter() - self.t0:.1f}s")

    # --- run ----------------------------------------------------------------
    def run(self):
        exe_bytes = open(self.exe, "rb").read()
        import hashlib
        sha = hashlib.sha256(exe_bytes).hexdigest()
        if sha != rr.GAME_EXE_SHA256:
            raise RuntimeError(f"{self.exe}: sha256 {sha} is not the reference 1.14d Game.exe")
        self.inline_sites, odd = rr.find_inline_sites(exe_bytes)
        os.makedirs(os.path.dirname(self.out_path), exist_ok=True)
        self.out = open(self.out_path, "w", encoding="utf-8", newline="\n")
        si = rr.STARTUPINFOW()
        si.cb = C.sizeof(si)
        pi = rr.PROCESS_INFORMATION()
        cmd = C.create_unicode_buffer(" ".join([f'"{self.exe}"'] + self.args))
        if not rr.CreateProcessW(self.exe, cmd, None, None, False, rr.DEBUG_ONLY_THIS_PROCESS,
                                 None, os.path.dirname(self.exe), C.byref(si), C.byref(pi)):
            raise rr.winerr("CreateProcessW")
        self.h_process, self.pid = pi.hProcess, pi.dwProcessId
        rr.DebugSetProcessKillOnExit(True)
        self.t0 = time.perf_counter()
        a = self.a
        header = {"type": "header", "format": RAW_FORMAT, "tool": TOOL,
                  "date": datetime.date.today().isoformat(), "game_exe_sha256": sha,
                  "args": self.args, "pid": self.pid, "inline": self.with_inline,
                  "request": {"tick": a.tick, "class": a.cls, "kind": a.kind,
                              "superunique": a.superunique, "dx": a.dx, "dy": a.dy,
                              "spread": a.spread, "flags": a.flags,
                              "after_ticks": a.after_ticks}}
        self.out.write(json.dumps(header) + "\n")
        try:
            self.loop(self.t0 + self.seconds)
        finally:
            self.kill()
            self.out.write(json.dumps({"type": "footer", "events": self.seq,
                                       "counts": self.counts, "notes": self.notes,
                                       "debug_events": self.dbg,
                                       "foreign_exceptions": self.exc}) + "\n")
            self.out.close()
            rr.CloseHandle(pi.hThread)
        return self.result.get("summary")

    def loop(self, deadline):
        ev = rr.DEBUG_EVENT()
        while self.state != "done":
            if time.perf_counter() > deadline:
                self.notes.append(f"time limit {self.seconds}s reached in state {self.state}")
                return
            if not rr.WaitForDebugEvent(C.byref(ev), 100):
                self.maybe_force()
                continue
            code, tid = ev.dwDebugEventCode, ev.dwThreadId
            self.pending = (ev.dwProcessId, tid)
            self.dbg[code] = self.dbg.get(code, 0) + 1
            status = rr.DBG_CONTINUE
            if code == rr.CREATE_PROCESS_DEBUG_EVENT:
                info = ev.u.CreateProcessInfo
                self.threads[tid] = info.hThread
                if info.hFile:
                    rr.CloseHandle(info.hFile)
                self.install(info.lpBaseOfImage or 0)
            elif code == rr.CREATE_THREAD_DEBUG_EVENT:
                self.threads[tid] = ev.u.CreateThread.hThread
                if self.stepping and rr.SuspendThread(self.threads[tid]) != 0xFFFFFFFF:
                    self.suspended.add(tid)
            elif code == rr.EXIT_THREAD_DEBUG_EVENT:
                self.threads.pop(tid, None)
                self.suspended.discard(tid)
                self.stepping.pop(tid, None)
                self.calls.pop(tid, None)
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
            if not self.stepping:
                self.maybe_force()


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--name", default="bdAma", help="character (save) name (default bdAma)")
    ap.add_argument("--char-class", default="ama", choices=["ama", "sor", "nec", "pal", "bar"],
                    help="class switch passed with -name (dru/ass saves: use --manual)")
    ap.add_argument("--tick", type=int, default=50,
                    help="spawn at the end of this server frame (default 50)")
    ap.add_argument("--class", dest="cls", type=int, default=5,
                    help="monstats row (default 5 zombie1)")
    ap.add_argument("--kind", default="normal",
                    choices=["normal", "champion", "unique", "boss"],
                    help="normal; champion (normal + umod 16 mark); unique (no champion); "
                         "boss (champion or unique by the game's own roll)")
    ap.add_argument("--superunique", type=int, default=None,
                    help="superuniques row instead of --class/--kind")
    ap.add_argument("--dx", type=int, default=4, help="x offset from the player (subtiles)")
    ap.add_argument("--dy", type=int, default=4, help="y offset from the player (subtiles)")
    ap.add_argument("--spread", type=int, default=4, help="ring search limit (default 4)")
    ap.add_argument("--flags", type=lambda s: int(s, 0), default=0,
                    help="creation flags for normal spawns (default 0)")
    ap.add_argument("--after-ticks", type=int, default=25,
                    help="keep the game running this many frames after the spawn (default 25)")
    ap.add_argument("--seconds", type=float, default=180.0, help="overall limit (default 180)")
    ap.add_argument("--force-after", type=float, default=4.0,
                    help="seconds in the start-up menu before it is left for client mode")
    ap.add_argument("--no-force", "--manual", dest="no_force", action="store_true",
                    help="do not force the menu; start the game by hand")
    ap.add_argument("--no-inline", action="store_true", help="helpers and setters only")
    ap.add_argument("--out", default=None, help="output .jsonl (default traces/raw/<time>-spawn.jsonl)")
    ap.add_argument("game_args", nargs="*", default=None,
                    help="Game.exe arguments (default: -w -ns -nosave -name <name> -<class>)")
    a = ap.parse_args()
    args = a.game_args or ["-w", "-ns", "-nosave", "-name", a.name, f"-{a.char_class}"]
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-spawn.jsonl")
    r = SpawnRecorder(os.path.abspath(a.game), args, out, a)
    summary = None
    try:
        summary = r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    print(f"wrote {out}")
    for n in r.notes:
        print("note:", n)
    if not summary:
        print("FAIL: no spawn recorded", file=sys.stderr)
        sys.exit(1)
    print("spawn: " + json.dumps(summary))
    sys.exit(0 if summary["created"] else 1)


if __name__ == "__main__":
    main()
