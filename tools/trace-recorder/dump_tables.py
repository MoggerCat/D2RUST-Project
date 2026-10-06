"""Dump the excel tables of the original 1.14d Game.exe as they sit in
memory after the full load and its post-load fix-ups.

A minimal Windows debugger (ctypes + the Win32 debug API, standard library
only; the Win32 definitions are shared with record_rng.py). It starts
game/Game.exe under DEBUG_ONLY_THIS_PROCESS and plants two INT3s:

  * the table loader 0x006122F0 (entry): name, record size and count
    pointer from its stack arguments; a one-shot INT3 on the return address
    then gives the record pointer (EAX) and the count;
  * the load-all routine 0x00619300 (entry): a one-shot INT3 on its return
    address marks the end of the load and of every post-load fix-up.

At that second stop every thread of the game is frozen (debug event); the
script reads each table's records and the runtime maps listed in MAPS,
writes traces/raw/<time>-tables/ and kills the game. Addresses:
tools/trace-recorder/README.md ("dump_tables.py") and specs/data/loading.md
(Provenance).

The game process is always terminated when this script ends (finally
block, Ctrl+C, and kill-on-exit if the debugger dies).

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

TOOL = "trace-recorder dump_tables 0.1.0"
FORMAT = "tables-dump-1"

LOAD_TABLE = 0x6122F0  # (ctx, name, fields, int *count, record size), stdcall
LOAD_ALL = 0x619300    # the full excel load, called once when client mode starts
# Leaving the start-up menu (launcher mode 4) the way a started game does:
# the menu loop runs while MENU_LOOP != 0 and then returns NEXT_MODE; mode
# 1 (client) loads the string tables, then the excel tables (0x0044B8A0).
GAME_MODE = 0x74C704
MENU_LOOP = 0x72DDD4
NEXT_MODE = 0x7795E8

# Tables whose loader buffer is freed after a copy into a combined array:
# name -> address of the global holding the part's pointer into that array.
COMBINED_PART = {
    "weapons": 0x96CA60, "armor": 0x96CA68, "misc": 0x96CA70,
    "magicsuffix": 0x96CA84, "magicprefix": 0x96CA88, "automagic": 0x96CA8C,
    "raresuffix": 0x96CAA8, "rareprefix": 0x96CAAC,
    "plrtype": 0x96D4E0, "plrmode": 0x96D4E4,
    "objtype": 0x96D4D0, "objmode": 0x96D4D4,
}
# Tables whose raw records are freed after conversion (only maps remain).
FREED = {"gamble", "automap", "treasureclassex"}
# Pointer / count globals of tables kept in place (a cross-check of the
# hooked loader return; the count global wins when it was clamped later).
GLOBALS = {
    "compcode": (0x96BCAC, 0x96BCB4), "itemtypes": (0x96C828, 0x96C82C),
    "montype": (0x96C86C, 0x96C870), "pettype": (0x96C818, 0x96C820),
    "overlay": (0x96C7EC, 0x96C7F0), "itemstatcost": (0x96C7FC, 0x96C804),
    "properties": (0x96BCD4, 0x96BCDC), "missiles": (0x96C794, 0x96C79C),
    "states": (0x96BCEC, 0x96BCF4), "skills": (0x96C7C8, 0x96C7D0),
    "skilldesc": (0x96C7BC, 0x96C7C4), "charstats": (0x96C7F4, 0x96C7F8),
    "arena": (0x96D608, None), "chartemplate": (0x96D600, 0x96D604),
    "uniqueitems": (0x96C854, 0x96C858), "sets": (0x96C83C, 0x96C840),
    "setitems": (0x96C848, 0x96C84C), "gems": (0x96CA94, 0x96CA90),
    "books": (0x96CC64, 0x96CC60), "qualityitems": (0x96CC54, 0x96CC50),
    "lowqualityitems": (0x96CC5C, 0x96CC58), "runes": (0x96CA9C, 0x96CA98),
    "itemratio": (0x96CC4C, 0x96CC48), "monmode": (0x96D4BC, 0x96D4B8),
    "composit": (0x96D4EC, None), "armtype": (0x96D4E8, None),
    "experience": (0x96C8A8, None), "uniquetitle": (0x96D488, 0x96D498),
    "uniqueprefix": (0x96D48C, 0x96D49C), "uniquesuffix": (0x96D490, 0x96D4A0),
    "uniqueappellation": (0x96D494, 0x96D4A4), "monlvl": (0x96C7A0, 0x96C7A4),
    "monstats2": (0x96C6C0, 0x96C6C8), "monprop": (0x96C860, 0x96C864),
    "monsounds": (0x96C6B4, 0x96C6BC), "monseq": (0x96C7A8, 0x96C7B0),
    "monstats": (0x96C6A8, 0x96C6B0), "monumod": (0x96C880, 0x96C884),
    "superuniques": (0x96C704, 0x96C70C), "monpreset": (0x96C6D8, None),
    "hireling": (0x96BDD0, 0x96BDD4), "npc": (0x96C5D8, 0x96C5DC),
    "monequip": (0x96C810, 0x96C814), "levels": (0x96C888, 0x96C88C),
    "leveldefs": (0x96C890, None), "lvltypes": (0x96C9F0, 0x96C9FC),
    "lvlprest": (0x96C894, 0x96C898), "lvlwarp": (0x96CA00, 0x96CA04),
    "lvlmaze": (0x96CA08, 0x96CA0C), "lvlsub": (0x96CA10, 0x96CA14),
    "objects": (0x96D470, 0x96D474), "objgroup": (0x96D478, 0x96D47C),
    "shrines": (0x96D468, 0x96D46C), "monitempercent": (0x96D484, 0x96D480),
    "inventory": (0x96D4F4, 0x96D4F0), "belts": (0x96D4F8, None),
    "cubemain": (0x96D628, 0x96D62C), "difficultylevels": (0x96C8AC, 0x96C8B0),
}

# Runtime maps: name -> (pointer global or None for a static array,
# address (static) / None, element size, count expression). Count
# expressions: an int, ("g", addr) = u32 global, ("gg", a, b) = product of
# two u32 globals, ("g7", a) = 7 × u32 global, ("w40", a) = 40 bitsets of
# ceil(u32 global / 32) words. Owner spec: specs/data/runtime-maps.md.
MAPS = {
    "itemtypes_equiv": (0x96C834, None, 4, ("gg", 0x96C82C, 0x96C830)),
    "montype_equiv": (0x96C874, None, 4, ("gg", 0x96C870, 0x96C878)),
    "skills_class_counts": (0x96C7D4, None, 4, 7),
    "skills_class_lists": (0x96C7DC, None, 2, ("g7", 0x96C7D8)),
    "skills_desc_list": (0x96C7E4, None, 2, ("g", 0x96C7E0)),  # the passive-skill list
    "items_f6_list": (0x96CA78, None, 2, ("g", 0x96CA58)),
    "gamble_index": (0x96CAB4, None, 4, ("g", 0x96CAB0)),
    "gamble_levels": (None, 0x96CAB8, 4, 100),
    "automap_runtime": (0x96CA28, None, 32, ("g", 0x96CA2C)),
    "automap_level_index": (None, 0x96C8D0, 8, 36),  # (first, end) row per level type
    "superunique_hc": (None, 0x96C710, 2, 66),
    "stat_stuff": (None, 0x96C89C, 4, 2),
    "monpreset_acts": (None, 0x96C6DC, 4, 10),
    "item_counts": (None, 0x96CA58, 4, 8),
    "affix_count": (None, 0x96CA7C, 4, 1),
    "rare_count": (None, 0x96CAA0, 4, 1),
    "isc_desc_list": (0x96C808, None, 2, ("g", 0x96C80C)),
    "states_bitsets": (0x96BCF8, None, 4, ("w40", 0x96BCF4)),
    "states_pgsv": (0x96BD9C, None, 2, ("g", 0x96BDA0)),
    "states_curse": (0x96BDA4, None, 2, ("g", 0x96BDA8)),
    "states_disguise": (0x96BDAC, None, 2, ("g", 0x96BDB0)),
    "states_active": (0x96BDB4, None, 2, ("g", 0x96BDB8)),
    "states_itemtype": (0x96BDBC, None, 2, ("g", 0x96BDC0)),
    "hireling_first": (None, 0x96BDD8, 4, 512),
    "leveldefs_portals": (0x96C9F4, None, 4, ("g", 0x96C9F8)),
    "monseq_index": (0x96C7B4, None, 12, ("g", 0x96C7B8)),
    "lvlsub_type_first": (0x96CA18, None, 4, 13),  # max lvlsub Type + 1 (12 in 1.14d)
    # Treasure classes (specs/items/treasure.md §1.1, §1.6); the entry
    # lists the records point to are dumped as map-tc_entries.bin.
    "tc_records": (0x96C5EC, None, 0x2C, ("g", 0x96C5F0)),
    "tc_chest": (None, 0x96C5F4, 4, 45),
}

TC_ENTRY_SIZE = 0x1C


class Dumper:
    def __init__(self, exe, args, out_dir, seconds, force_after):
        self.exe, self.args, self.out_dir, self.seconds = exe, args, out_dir, seconds
        self.force_after = force_after
        self.forced = False
        self.h_process = None
        self.threads = {}
        self.bp_orig = {}       # addr -> original byte (inserted INT3s)
        self.reinsert = {}      # tid -> addr to re-arm after the single step
        self.calls = {}         # return addr -> list of (tid, name, size, count_ptr, caller)
        self.pending = None     # (pid, tid) of the debug event left uncontinued (kill())
        self.loads = []
        self.load_all_ret = None
        self.done = False
        self.notes = []
        self.initial_bps = 0
        self.t0 = None

    read = rr.Recorder.read
    read_u32 = rr.Recorder.read_u32
    write = rr.Recorder.write
    get_ctx = rr.Recorder.get_ctx
    set_ctx = rr.Recorder.set_ctx
    kill = rr.Recorder.kill
    close_event_handles = staticmethod(rr.Recorder.close_event_handles)

    def arm(self, addr):
        if addr not in self.bp_orig:
            self.bp_orig[addr] = self.read(addr, 1)[0]
            self.write(addr, rr.INT3)

    def disarm(self, addr):
        orig = self.bp_orig.pop(addr, None)
        if orig is not None:
            self.write(addr, bytes([orig]))

    def cstring(self, addr, n=64):
        return self.read(addr, n).split(b"\0", 1)[0].decode("latin-1")

    def on_breakpoint(self, tid, addr):
        ctx = self.get_ctx(tid)
        ctx.Eip = addr
        esp = ctx.Esp
        if addr == LOAD_TABLE:
            ret, _, name_p, _, count_p, size = struct.unpack("<6I", self.read(esp, 24))
            name = self.cstring(name_p)
            self.calls.setdefault(ret, []).append((tid, name, size, count_p, ret - 5))
            self.arm(ret)
            # step over the entry instruction, then re-arm
            self.disarm(addr)
            self.reinsert[tid] = addr
            ctx.EFlags |= rr.TRAP_FLAG
        elif addr == LOAD_ALL:
            self.load_all_ret = self.read_u32(esp)
            self.disarm(addr)
            self.arm(self.load_all_ret)
            self.notes.append(f"load-all entered at {time.perf_counter() - self.t0:.1f}s, "
                              f"returns to {self.load_all_ret:#x}")
        elif addr in self.calls:
            calls = self.calls[addr]
            k = next((i for i, c in enumerate(calls) if c[0] == tid), None)
            if k is not None:
                _, name, size, count_p, caller = calls.pop(k)
                ptr = ctx.Eax
                header = self.read_u32(ptr - 4) if ptr >= 4 else None
                count = self.read_u32(count_p) if count_p else header
                self.loads.append({"name": name, "record_size": size, "ptr": ptr,
                                   "count": count, "header_count": header,
                                   "count_global": count_p or None, "call": caller})
            if not calls:
                del self.calls[addr]
                if addr != self.load_all_ret:
                    self.disarm(addr)
        if addr == self.load_all_ret and not self.done:
            self.disarm(addr)
            self.dump()
            self.done = True
        self.set_ctx(tid, ctx)

    def on_single_step(self, tid):
        addr = self.reinsert.pop(tid, None)
        if addr is not None:
            self.arm(addr)

    # --- the dump ---------------------------------------------------------

    def save(self, name, data):
        with open(os.path.join(self.out_dir, name), "wb") as f:
            f.write(data)

    def dump(self):
        os.makedirs(self.out_dir, exist_ok=True)
        tables = {}
        seen = {}
        for ld in self.loads:
            seen.setdefault(ld["name"], []).append(ld)
        for name, lds in seen.items():
            ld = lds[-1]
            if len(lds) > 1:
                self.notes.append(f"{name} loaded {len(lds)} times; the last load is dumped")
            if name in FREED:
                tables[name] = {"freed": True, "count": ld["count"],
                                "record_size": ld["record_size"]}
                continue
            ptr, count, src = ld["ptr"], ld["count"], "loader return"
            if name in COMBINED_PART:
                ptr, src = self.read_u32(COMBINED_PART[name]), f"global {COMBINED_PART[name]:#x}"
            elif name in GLOBALS:
                pg, cg = GLOBALS[name]
                gptr = self.read_u32(pg)
                if gptr != ptr:
                    self.notes.append(f"{name}: global {pg:#x} = {gptr:#x}, loader gave {ptr:#x}")
                ptr, src = gptr, f"global {pg:#x}"
                if cg is not None:
                    gc = self.read_u32(cg)
                    if gc != count:
                        self.notes.append(f"{name}: count global {cg:#x} = {gc}, loaded {count}")
                    count = gc
            else:
                self.notes.append(f"{name}: no known global; loader pointer used")
            size = ld["record_size"]
            data = self.read(ptr, count * size) if count else b""
            self.save(f"{name}.bin", data)
            tables[name] = {"address": f"{ptr:#x}", "source": src, "count": count,
                            "record_size": size, "bytes": len(data),
                            "loader_ptr": f"{ld['ptr']:#x}", "loaded_count": ld["count"],
                            "header_count": ld["header_count"], "call": f"{ld['call']:#x}"}
        maps = {}
        for name, (pg, static, esize, cnt) in MAPS.items():
            if isinstance(cnt, int):
                n = cnt
            elif cnt[0] == "g":
                n = self.read_u32(cnt[1])
            elif cnt[0] == "gg":
                n = self.read_u32(cnt[1]) * self.read_u32(cnt[2])
            elif cnt[0] == "w40":
                n = 40 * ((self.read_u32(cnt[1]) + 31) // 32)
            else:
                n = 7 * self.read_u32(cnt[1])
            addr = self.read_u32(pg) if pg is not None else static
            data = self.read(addr, n * esize) if n and addr else b""
            self.save(f"map-{name}.bin", data)
            maps[name] = {"address": f"{addr:#x}", "pointer_global": pg and f"{pg:#x}",
                          "element_size": esize, "count": n, "bytes": len(data)}
            if name == "tc_records":
                maps["tc_entries"] = self.dump_tc_entries(data, addr)
        self.manifest = {"format": FORMAT, "tool": TOOL,
                         "date": datetime.date.today().isoformat(),
                         "game_exe_sha256": self.sha, "args": self.args,
                         "load_all": f"{LOAD_ALL:#x}", "table_loader": f"{LOAD_TABLE:#x}",
                         "dumped_at_return": f"{self.load_all_ret:#x}",
                         "loads": [{**ld, "ptr": f"{ld['ptr']:#x}", "call": f"{ld['call']:#x}",
                                    "count_global": ld["count_global"]
                                    and f"{ld['count_global']:#x}"} for ld in self.loads],
                         "tables": tables, "maps": maps}

    def dump_tc_entries(self, records, base):
        """Entry lists of every TC record, concatenated in record order;
        `tc_chest` pointers become record indices via `tc_base`."""
        out, total = bytearray(), 0
        for i in range(len(records) // 0x2C):
            rec = records[i * 0x2C:(i + 1) * 0x2C]
            count = int.from_bytes(rec[4:8], "little", signed=True)
            ptr = int.from_bytes(rec[0x28:0x2C], "little")
            if count > 0 and ptr:
                out += self.read(ptr, count * TC_ENTRY_SIZE)
                total += count
        self.save("map-tc_entries.bin", bytes(out))
        return {"address": "lists of tc_records +0x28", "tc_base": f"{base:#x}",
                "pointer_global": None, "element_size": TC_ENTRY_SIZE,
                "count": total, "bytes": len(out)}

    # --- process control --------------------------------------------------

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
        self.t0 = time.perf_counter()
        self.manifest = None
        try:
            self.loop(self.t0 + self.seconds)
        finally:
            self.kill()
            rr.CloseHandle(pi.hThread)
        if self.manifest is not None:
            self.manifest["notes"] = self.notes
            with open(os.path.join(self.out_dir, "manifest.json"), "w", encoding="utf-8",
                      newline="\n") as f:
                json.dump(self.manifest, f, indent=1)
            # the table and map list as TSV, for data-tool dump-compare
            with open(os.path.join(self.out_dir, "manifest.tsv"), "w", encoding="utf-8",
                      newline="\n") as f:
                f.write("kind\tname\taddress\tcount\trecord_size\n")
                for name, t in self.manifest["tables"].items():
                    kind = "freed" if t.get("freed") else "table"
                    f.write(f"{kind}\t{name}\t{t.get('address', '-')}\t{t['count']}\t"
                            f"{t['record_size']}\n")
                for name, mp in self.manifest["maps"].items():
                    f.write(f"map\t{name}\t{mp['address']}\t{mp['count']}\t"
                            f"{mp['element_size']}\n")
        return self.manifest

    def loop(self, deadline):
        ev = rr.DEBUG_EVENT()
        while not self.done:
            if time.perf_counter() > deadline:
                self.notes.append(f"time limit {self.seconds}s reached before the load ended "
                                  f"({len(self.loads)} tables loaded, {len(self.threads)} threads)")
                self.where()
                return
            if not rr.WaitForDebugEvent(C.byref(ev), 100):
                self.maybe_force()
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
                if self.read(LOAD_TABLE, 3) != b"\x55\x8B\xEC" or \
                        self.read(LOAD_ALL, 1) != b"\x55":
                    raise RuntimeError("unexpected code: not the 1.14d Game.exe?")
                self.arm(LOAD_TABLE)
                self.arm(LOAD_ALL)
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
            if self.done:
                return  # the game stays frozen at the dump point; kill() ends it
            rr.ContinueDebugEvent(ev.dwProcessId, tid, status)
            self.pending = None

    def maybe_force(self):
        """In the start-up menu with no table loaded after force_after seconds:
        end the menu loop with next mode 1 (client), as starting a game does."""
        if self.forced or self.force_after is None or self.loads or self.load_all_ret:
            return
        if time.perf_counter() - self.t0 < self.force_after:
            return
        mode = self.read_u32(GAME_MODE)
        if mode != 4:
            return
        self.write(NEXT_MODE, struct.pack("<I", 1))
        self.write(MENU_LOOP, struct.pack("<I", 0))
        self.forced = True
        self.notes.append(f"menu left for client mode at "
                          f"{time.perf_counter() - self.t0:.1f}s (forced)")

    def where(self):
        """Note each thread's EIP and the Game.exe return addresses on its stack
        (diagnoses a start-up that never reaches the load)."""
        self.notes.append(f"game mode (0x74c704) = {self.read_u32(0x74C704)}")
        for tid, h in self.threads.items():
            if rr.SuspendThread(h) == 0xFFFFFFFF:
                continue
            try:
                ctx = self.get_ctx(tid)
                stack = self.read(ctx.Esp, 0x2000)
                rets = [v for (v,) in struct.iter_unpack("<I", stack)
                        if rr.IMAGE_BASE + 0x1000 <= v < 0x6D0000][:12]
                self.notes.append(f"thread {tid}: eip {ctx.Eip:#x}, stack "
                                  + " ".join(f"{v:#x}" for v in rets))
            except OSError as e:
                self.notes.append(f"thread {tid}: {e}")
            finally:
                rr.ResumeThread(h)

    def on_exception(self, tid, info):
        rec = info.ExceptionRecord
        code, addr = rec.ExceptionCode, rec.ExceptionAddress or 0
        if code in rr.BREAKPOINT_CODES:
            if addr in self.bp_orig or addr in self.calls or addr == self.load_all_ret:
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
    ap.add_argument("--seconds", type=float, default=60.0,
                    help="give up and kill the game after this many seconds (default 60)")
    ap.add_argument("--force-after", type=float, default=6.0,
                    help="seconds in the start-up menu before the script leaves it for "
                         "client mode itself (default 6)")
    ap.add_argument("--no-force", action="store_true",
                    help="never leave the menu; wait for a game to be started by hand")
    ap.add_argument("--out", default=None,
                    help="output directory (default traces/raw/<time>-tables)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"],
                    help="Game.exe arguments (default: -w -ns)")
    a = ap.parse_args()
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-tables")
    d = Dumper(os.path.abspath(a.game), a.game_args or ["-w", "-ns"], out, a.seconds,
               None if a.no_force else a.force_after)
    try:
        m = d.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
        m = None
    for n in d.notes:
        print("note:", n)
    if m is None:
        print("no dump written", file=sys.stderr)
        sys.exit(1)
    print(f"wrote {out}: {len(m['tables'])} tables, {len(m['maps'])} maps, "
          f"{len(m['loads'])} loader calls")


if __name__ == "__main__":
    main()
