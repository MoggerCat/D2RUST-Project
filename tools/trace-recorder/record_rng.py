"""Record every seeded-RNG step of the original 1.14d Game.exe.

A minimal Windows debugger (ctypes + the Win32 debug API, standard library
only). It starts game/Game.exe under DEBUG_ONLY_THIS_PROCESS, plants INT3
breakpoints and writes one JSON line per RNG event to traces/raw/.

What it hooks (addresses: specs/sim/rng.md, Provenance):
  * the six non-inlined RNG helpers: entry (seed pointer, arguments, seed
    before) and return (returned value, seed after);
  * the three seed setters (init / init-low / set);
  * every inlined step: each `mov ecx|edx, 0x6AC690C5` in .text (found by
    scanning Game.exe at start-up; all 852 are followed by a `mul` of that
    register). From the `mov` the thread is single-stepped: the first
    `mul ecx|edx` executed while that register still holds the multiplier
    is the step (this follows either branch when one `mov` serves two
    `mul`s), and EAX there is the low seed word. Stepping continues until
    the add/adc pair that forms the new 64-bit state has run; their
    destination registers are the new (lo, hi). The old hi follows from
    lo' - low32(lo * K) (spec §2).

While a thread single-steps, every other thread is suspended, so no thread
can run through a temporarily removed breakpoint.

Emulation (`--emulate on`, the default; specs/tools/rng-trace.md §4 r7):
single steps are the cost (two debug events each), so at a breakpoint the
instructions the single steps would run are run instead by `x86emu.py` on
the stopped thread's registers: an inline site's whole trace from the
`mov` to the `adc` (one debug event per draw instead of about seven), and
the one instruction under a helper, setter, tick or return breakpoint
(no step-over). The same rules pick the `mul` and the add/adc pair; the
registers, flags and stack writes are committed only when every
instruction was understood and every status flag is defined, else the
thread is single-stepped as before. `--emulate check` single-steps
everything and compares each emulated result with the real one;
`--emulate off` never emulates.

The game process is always terminated when this script ends: on the time
limit, on Ctrl+C, on any error (finally block), and by the system if the
debugger dies (kill-on-exit).

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
from ctypes import wintypes as W

import x86emu

if __name__ == "__main__":
    # poke.py / send.py import `record_rng` for the Win32 definitions: give them
    # this module, not a second copy whose ctypes classes differ
    sys.modules.setdefault("record_rng", sys.modules[__name__])

TOOL = "trace-recorder 0.3.0"
RAW_FORMAT = "rng-raw-1"
GAME_EXE_SHA256 = "631066c1649c4ea9ffe48bf97e24c00bca1f7a6759c21150f1a79982589adaaf"
IMAGE_BASE = 0x400000
MULTIPLIER = 0x6AC690C5
M32 = 0xFFFFFFFF

# Non-inlined helpers (spec §3). name, argument layout, function size.
#   args: "n" = EDX; "min,n" = EDX, [ESP+4] at entry.
HELPERS = {
    0x45C370: ("step", "", 28),
    0x45C390: ("roll", "n", 75),
    0x45C3E0: ("roll", "n", 75),
    0x472210: ("mask", "n", 39),
    0x472240: ("mask_range", "min,n", 50),
    0x472280: ("roll_range", "min,n", 98),
}
# Seed setters (spec §4). value layout at entry.
SETTERS = {
    0x650E30: "init",      # {1, 666}
    0x650E40: "init_low",  # {EDX, 666}
    0x650E60: "set",       # {EDX, [ESP+4]}
}

# Frames and owners (--frames; specs/tools/rng-trace.md §4). Tick entry
# 0x0052D870, ECX = game (sim/tick.md §3; record_tick.py); frame = game +0xA8
# + 1. Game seed at game +0xD0 (sim/rng.md §5.2); unit seed at unit +0x20,
# server-unit flag unit +0xC8 bit 0x04000000, type +0x00, GUID +0x0C, the
# five server hash lists at game +0x1120 (128 buckets per type, next
# unit +0xE4) (tools/state-snapshot.md §2, sim/unit-order.md §2).
TICK = 0x52D870
TICK_BYTES = b"\x53\x56\x57"
G_FRAME, G_SEED, G_HASH = 0xA8, 0xD0, 0x1120
HASH_TYPES = ((0, 0x000), (1, 0x200), (2, 0x400), (3, 0x800), (4, 0x600))
U_TYPE, U_GUID, U_SEED, U_FLAGS2, U_HASH_NEXT = 0x00, 0x0C, 0x20, 0xC8, 0xE4
SERVER_UNIT = 0x04000000

# --- Win32 -----------------------------------------------------------------

k32 = C.WinDLL("kernel32", use_last_error=True)

DEBUG_ONLY_THIS_PROCESS = 0x00000002
EXCEPTION_DEBUG_EVENT = 1
CREATE_THREAD_DEBUG_EVENT = 2
CREATE_PROCESS_DEBUG_EVENT = 3
EXIT_THREAD_DEBUG_EVENT = 4
EXIT_PROCESS_DEBUG_EVENT = 5
LOAD_DLL_DEBUG_EVENT = 6
DBG_CONTINUE = 0x00010002
DBG_EXCEPTION_NOT_HANDLED = 0x80010001
EXCEPTION_BREAKPOINT = 0x80000003
EXCEPTION_SINGLE_STEP = 0x80000004
STATUS_WX86_SINGLE_STEP = 0x4000001E
STATUS_WX86_BREAKPOINT = 0x4000001F
BREAKPOINT_CODES = (EXCEPTION_BREAKPOINT, STATUS_WX86_BREAKPOINT)
SINGLE_STEP_CODES = (EXCEPTION_SINGLE_STEP, STATUS_WX86_SINGLE_STEP)
WOW64_CONTEXT_FULL = 0x00010007
TRAP_FLAG = 0x100
PAGE_EXECUTE_READWRITE = 0x40
INT3 = bytes([0xCC])
WAIT_OBJECT_0 = 0


class STARTUPINFOW(C.Structure):
    _fields_ = [("cb", W.DWORD), ("lpReserved", W.LPWSTR), ("lpDesktop", W.LPWSTR),
                ("lpTitle", W.LPWSTR), ("dwX", W.DWORD), ("dwY", W.DWORD),
                ("dwXSize", W.DWORD), ("dwYSize", W.DWORD), ("dwXCountChars", W.DWORD),
                ("dwYCountChars", W.DWORD), ("dwFillAttribute", W.DWORD),
                ("dwFlags", W.DWORD), ("wShowWindow", W.WORD), ("cbReserved2", W.WORD),
                ("lpReserved2", C.c_void_p), ("hStdInput", W.HANDLE),
                ("hStdOutput", W.HANDLE), ("hStdError", W.HANDLE)]


class PROCESS_INFORMATION(C.Structure):
    _fields_ = [("hProcess", W.HANDLE), ("hThread", W.HANDLE),
                ("dwProcessId", W.DWORD), ("dwThreadId", W.DWORD)]


class EXCEPTION_RECORD(C.Structure):
    _fields_ = [("ExceptionCode", W.DWORD), ("ExceptionFlags", W.DWORD),
                ("ExceptionRecord", C.c_void_p), ("ExceptionAddress", C.c_void_p),
                ("NumberParameters", W.DWORD), ("ExceptionInformation", C.c_size_t * 15)]


class EXCEPTION_DEBUG_INFO(C.Structure):
    _fields_ = [("ExceptionRecord", EXCEPTION_RECORD), ("dwFirstChance", W.DWORD)]


class CREATE_THREAD_DEBUG_INFO(C.Structure):
    _fields_ = [("hThread", W.HANDLE), ("lpThreadLocalBase", C.c_void_p),
                ("lpStartAddress", C.c_void_p)]


class CREATE_PROCESS_DEBUG_INFO(C.Structure):
    _fields_ = [("hFile", W.HANDLE), ("hProcess", W.HANDLE), ("hThread", W.HANDLE),
                ("lpBaseOfImage", C.c_void_p), ("dwDebugInfoFileOffset", W.DWORD),
                ("nDebugInfoSize", W.DWORD), ("lpThreadLocalBase", C.c_void_p),
                ("lpStartAddress", C.c_void_p), ("lpImageName", C.c_void_p),
                ("fUnicode", W.WORD)]


class EXIT_CODE_INFO(C.Structure):
    _fields_ = [("dwExitCode", W.DWORD)]


class LOAD_DLL_DEBUG_INFO(C.Structure):
    _fields_ = [("hFile", W.HANDLE), ("lpBaseOfDll", C.c_void_p),
                ("dwDebugInfoFileOffset", W.DWORD), ("nDebugInfoSize", W.DWORD),
                ("lpImageName", C.c_void_p), ("fUnicode", W.WORD)]


class DEBUG_EVENT_UNION(C.Union):
    _fields_ = [("Exception", EXCEPTION_DEBUG_INFO),
                ("CreateThread", CREATE_THREAD_DEBUG_INFO),
                ("CreateProcessInfo", CREATE_PROCESS_DEBUG_INFO),
                ("ExitThread", EXIT_CODE_INFO), ("ExitProcess", EXIT_CODE_INFO),
                ("LoadDll", LOAD_DLL_DEBUG_INFO)]


class DEBUG_EVENT(C.Structure):
    _fields_ = [("dwDebugEventCode", W.DWORD), ("dwProcessId", W.DWORD),
                ("dwThreadId", W.DWORD), ("u", DEBUG_EVENT_UNION)]


class WOW64_FLOATING_SAVE_AREA(C.Structure):
    _fields_ = [(n, W.DWORD) for n in ("ControlWord", "StatusWord", "TagWord",
                                       "ErrorOffset", "ErrorSelector", "DataOffset",
                                       "DataSelector")] + \
               [("RegisterArea", C.c_ubyte * 80), ("Cr0NpxState", W.DWORD)]


REGS = ("Eax", "Ecx", "Edx", "Ebx", "Esp", "Ebp", "Esi", "Edi")  # x86 reg numbers 0..7


class WOW64_CONTEXT(C.Structure):
    _fields_ = [("ContextFlags", W.DWORD)] + \
               [(n, W.DWORD) for n in ("Dr0", "Dr1", "Dr2", "Dr3", "Dr6", "Dr7")] + \
               [("FloatSave", WOW64_FLOATING_SAVE_AREA)] + \
               [(n, W.DWORD) for n in ("SegGs", "SegFs", "SegEs", "SegDs", "Edi", "Esi",
                                       "Ebx", "Edx", "Ecx", "Eax", "Ebp", "Eip", "SegCs",
                                       "EFlags", "Esp", "SegSs")] + \
               [("ExtendedRegisters", C.c_ubyte * 512)]


assert C.sizeof(WOW64_CONTEXT) == 716


def _proto(name, res, *args):
    f = getattr(k32, name)
    f.restype = res
    f.argtypes = list(args)
    return f


_CreateProcessW = _proto("CreateProcessW", W.BOOL, W.LPCWSTR, W.LPWSTR, C.c_void_p,
                         C.c_void_p, W.BOOL, W.DWORD, C.c_void_p, W.LPCWSTR,
                         C.POINTER(STARTUPINFOW), C.POINTER(PROCESS_INFORMATION))
_CreateMutexW = _proto("CreateMutexW", W.HANDLE, C.c_void_p, W.BOOL, W.LPCWSTR)
_WaitForSingleObject = _proto("WaitForSingleObject", W.DWORD, W.HANDLE, W.DWORD)

# One 1.14d at a time on this machine (several sessions / worktrees record on
# one PC; two games under the debugger break each other's runs). Every
# recorder launches the game through CreateProcessW below, which first takes
# the named mutex GAME_LOCK_NAME (held until this Python process exits: a
# recorder runs its games one after the other) and then waits until no
# Game.exe is running (a game started without the lock: by hand or by an older
# copy of these tools). D2_GAME_LOCK=0 turns both off.
GAME_LOCK_NAME = "Local\\d2rs-original-game-1.14d"
GAME_LOCK_MAX_WAIT = 3600.0
_game_lock = None


def _game_exe_running():
    import subprocess
    try:
        out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq Game.exe", "/NH"],
                             capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return False
    return "game.exe" in out.lower()


def _lock_file_path():
    return os.path.join(os.environ.get("TEMP") or os.environ.get("TMP") or ".", "d2-game.lock")


def _take_lock_file(start):
    """The file half of the rule (docs/handoff/pc1-data.md, "One Game.exe open
    at a time"): create %TEMP%\\d2-game.lock exclusively, wait while another
    holder has it, delete it when this process exits. A lock file older than
    LOCK_FILE_STALE s while no Game.exe runs is taken as left behind."""
    import atexit
    path = _lock_file_path()
    noted = False
    while True:
        try:
            fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.write(fd, ("pid %d %s\n" % (os.getpid(), " ".join(sys.argv))).encode())
            os.close(fd)
            break
        except FileExistsError:
            try:
                age = time.time() - os.path.getmtime(path)
            except OSError:
                continue
            if age > LOCK_FILE_STALE and not _game_exe_running():
                print("note: removing a stale %s (%d s old, no Game.exe)" % (path, age),
                      file=sys.stderr)
                try:
                    os.remove(path)
                except OSError:
                    pass
                continue
            if not noted:
                print("note: %s exists (another session runs 1.14d); waiting" % path,
                      file=sys.stderr)
                noted = True
            if time.monotonic() - start > GAME_LOCK_MAX_WAIT:
                raise RuntimeError("game lock: %s held for %d s" % (path, GAME_LOCK_MAX_WAIT))
            time.sleep(5)

    def _drop():
        try:
            os.remove(path)
        except OSError:
            pass
    atexit.register(_drop)


LOCK_FILE_STALE = 900.0


def acquire_game_lock():
    """Take the one-game-at-a-time lock (idempotent within a process)."""
    global _game_lock
    if _game_lock is not None or os.environ.get("D2_GAME_LOCK") == "0":
        return
    h = _CreateMutexW(None, False, GAME_LOCK_NAME)
    if not h:
        raise winerr("CreateMutexW")
    start = time.monotonic()
    noted = False
    while True:
        r = _WaitForSingleObject(h, 5000)
        if r in (0, 0x80):            # WAIT_OBJECT_0, WAIT_ABANDONED (holder died)
            break
        if r != 0x102:                # not WAIT_TIMEOUT
            raise winerr("WaitForSingleObject")
        if not noted:
            print("note: another 1.14d run holds the game lock; waiting", file=sys.stderr)
            noted = True
        if time.monotonic() - start > GAME_LOCK_MAX_WAIT:
            raise RuntimeError("game lock: waited %d s" % GAME_LOCK_MAX_WAIT)
    _game_lock = h
    _take_lock_file(start)
    noted = False
    while _game_exe_running():
        if not noted:
            print("note: a Game.exe is already running (not ours); waiting for it to exit",
                  file=sys.stderr)
            noted = True
        if time.monotonic() - start > GAME_LOCK_MAX_WAIT:
            raise RuntimeError("game lock: a Game.exe kept running for %d s" % GAME_LOCK_MAX_WAIT)
        time.sleep(5)


def CreateProcessW(*args):
    acquire_game_lock()
    return _CreateProcessW(*args)
WaitForDebugEvent = _proto("WaitForDebugEvent", W.BOOL, C.POINTER(DEBUG_EVENT), W.DWORD)
ContinueDebugEvent = _proto("ContinueDebugEvent", W.BOOL, W.DWORD, W.DWORD, W.DWORD)
ReadProcessMemory = _proto("ReadProcessMemory", W.BOOL, W.HANDLE, C.c_void_p, C.c_void_p,
                           C.c_size_t, C.POINTER(C.c_size_t))
WriteProcessMemory = _proto("WriteProcessMemory", W.BOOL, W.HANDLE, C.c_void_p,
                            C.c_void_p, C.c_size_t, C.POINTER(C.c_size_t))
VirtualProtectEx = _proto("VirtualProtectEx", W.BOOL, W.HANDLE, C.c_void_p, C.c_size_t,
                          W.DWORD, C.POINTER(W.DWORD))
FlushInstructionCache = _proto("FlushInstructionCache", W.BOOL, W.HANDLE, C.c_void_p,
                               C.c_size_t)
Wow64GetThreadContext = _proto("Wow64GetThreadContext", W.BOOL, W.HANDLE,
                               C.POINTER(WOW64_CONTEXT))
Wow64SetThreadContext = _proto("Wow64SetThreadContext", W.BOOL, W.HANDLE,
                               C.POINTER(WOW64_CONTEXT))
SuspendThread = _proto("SuspendThread", W.DWORD, W.HANDLE)
ResumeThread = _proto("ResumeThread", W.DWORD, W.HANDLE)
TerminateProcess = _proto("TerminateProcess", W.BOOL, W.HANDLE, W.UINT)
WaitForSingleObject = _proto("WaitForSingleObject", W.DWORD, W.HANDLE, W.DWORD)
CloseHandle = _proto("CloseHandle", W.BOOL, W.HANDLE)
DebugSetProcessKillOnExit = _proto("DebugSetProcessKillOnExit", W.BOOL, W.BOOL)


def winerr(what):
    return OSError(f"{what} failed: WinError {C.get_last_error()}")


# --- Instruction decoding (only what the inline trace needs) ----------------

def decode_add_adc(code):
    """If `code` starts with a 32-bit ADD or ADC whose destination is a
    register, return ("add"|"adc", reg_index). Otherwise None.
    Handles 01/03/05, 11/13/15, 81/83 /0 and /2 (register form)."""
    i = 0
    while i < len(code) and code[i] in (0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0xF2, 0xF3):
        i += 1  # segment / rep prefixes (operand-size 0x66 would make it 16-bit)
    if i >= len(code):
        return None
    op = code[i]
    modrm = code[i + 1] if i + 1 < len(code) else 0
    mod, reg, rm = modrm >> 6, (modrm >> 3) & 7, modrm & 7
    if op in (0x03, 0x13):
        return ("add" if op == 0x03 else "adc", reg)
    if op in (0x01, 0x11) and mod == 3:
        return ("add" if op == 0x01 else "adc", rm)
    if op in (0x05, 0x15):
        return ("add" if op == 0x05 else "adc", 0)
    if op in (0x81, 0x83) and mod == 3 and reg in (0, 2):
        return ("add" if reg == 0 else "adc", rm)
    return None


MUL_FOR_MOV = {0xB9: b"\xF7\xE1", 0xBA: b"\xF7\xE2"}  # mov ecx -> mul ecx, mov edx -> mul edx


def text_section(exe_bytes):
    """[lo, hi) of Game.exe's .text in memory (the first section)."""
    pe = struct.unpack_from("<I", exe_bytes, 0x3C)[0]
    optsz = struct.unpack_from("<H", exe_bytes, pe + 20)[0]
    name, vsz, va = struct.unpack_from("<8sII", exe_bytes, pe + 24 + optsz)
    assert name.rstrip(b"\0") == b".text"
    return IMAGE_BASE + va, IMAGE_BASE + va + ((vsz + 0xFFF) & ~0xFFF)


def find_inline_sites(exe_bytes):
    """Scan .text for `mov ecx|edx, K` followed within 14 bytes by
    `mul ecx|edx` (spec §3.4). Returns the `mov` addresses outside the
    helper functions, and the number of immediates that did not fit."""
    pe = struct.unpack_from("<I", exe_bytes, 0x3C)[0]
    optsz = struct.unpack_from("<H", exe_bytes, pe + 20)[0]
    sec = pe + 24 + optsz
    name, _vsz, va, rsz, rptr = struct.unpack_from("<8sIIII", exe_bytes, sec)
    assert name.rstrip(b"\0") == b".text"
    text = exe_bytes[rptr:rptr + rsz]
    imm = struct.pack("<I", MULTIPLIER)
    helper_ranges = [(a, a + size) for a, (_, _, size) in HELPERS.items()]
    sites, odd, i = [], 0, 0
    while True:
        i = text.find(imm, i)
        if i < 0:
            break
        mul = MUL_FOR_MOV.get(text[i - 1])
        if mul is not None and text.find(mul, i + 4, i + 4 + 14) >= 0:
            addr = IMAGE_BASE + va + i - 1
            if not any(lo <= addr < hi for lo, hi in helper_ranges):
                sites.append(addr)
        else:
            odd += 1
        i += 1
    return sites, odd


# --- The debugger -----------------------------------------------------------

class Recorder:
    auto = None  # autostart.AutoStart (unattended start, input script)
    poke_layer = None  # poke.PokeLayer (--poke), run at the tick return 0x0052FD1E
    send_layer = None  # send.SendLayer (--send), injected at the drain call 0x0044F136
    frames = False   # --frames: tick markers, frame and owner hints (rng-trace.md §4)
    max_ticks = 0    # --ticks N: stop at the entry of tick N + 1
    skip_ranges = ()  # --skip-inline: [lo, hi) code ranges whose inline sites are not hooked
    emulate = "off"  # --emulate on|off|check (x86emu instead of single steps;
    #                  main() defaults to on, subclasses and probes keep off)

    def __init__(self, exe, args, out, seconds, with_inline, max_events):
        self.exe, self.args, self.out_path = exe, args, out
        self.seconds, self.with_inline, self.max_events = seconds, with_inline, max_events
        self.h_process = None
        self.pid = None
        self.threads = {}        # tid -> handle
        self.suspended = set()   # tids we suspended
        self.stepping = {}       # tid -> dict(reinsert=addr|None, trace=dict|None)
        self.bp_orig = {}        # addr -> original byte
        self.bp_inserted = set()
        self.bp_out = {}         # addr -> set(tid) stepping over it
        self.roles = {}          # addr -> set of roles: "helper" "setter" "inline" "ret"
        self.ret_refs = {}       # addr -> count of pending returns
        self.calls = {}          # tid -> stack of pending helper calls
        self.seq = 0
        self.t0 = None
        self.counts = {}
        self.out = None
        self.initial_bps = 0
        self.pending = None      # (pid, tid) of an event not yet continued
        self.dbg = {}            # debug event code -> count
        self.exc = {}            # foreign exception code -> count
        self.notes = []
        self.game = None         # --frames: the first game that ticks
        self.frame = 0           # its frame (0 before the first tick)
        self.ticks = 0
        self.done = False
        self.emu_notes = set()
        self.text_range = (0, 0)  # Game.exe .text [lo, hi) (orig_code's page cache)
        self.code_pages = {}

    # memory / context
    def read(self, addr, n):
        buf = (C.c_ubyte * n)()
        got = C.c_size_t()
        if not ReadProcessMemory(self.h_process, C.c_void_p(addr), buf, n, C.byref(got)):
            raise winerr(f"ReadProcessMemory {addr:#x}")
        return bytes(buf)

    def read_u32(self, addr):
        return struct.unpack("<I", self.read(addr, 4))[0]

    def write(self, addr, data):
        old = W.DWORD()
        VirtualProtectEx(self.h_process, C.c_void_p(addr), len(data),
                         PAGE_EXECUTE_READWRITE, C.byref(old))
        buf = (C.c_ubyte * len(data)).from_buffer_copy(data)
        got = C.c_size_t()
        ok = WriteProcessMemory(self.h_process, C.c_void_p(addr), buf, len(data),
                                C.byref(got))
        VirtualProtectEx(self.h_process, C.c_void_p(addr), len(data), old.value,
                         C.byref(W.DWORD()))
        if not ok:
            raise winerr(f"WriteProcessMemory {addr:#x}")
        FlushInstructionCache(self.h_process, C.c_void_p(addr), len(data))

    def get_ctx(self, tid):
        ctx = WOW64_CONTEXT()
        ctx.ContextFlags = WOW64_CONTEXT_FULL
        if not Wow64GetThreadContext(self.threads[tid], C.byref(ctx)):
            raise winerr("Wow64GetThreadContext")
        return ctx

    def set_ctx(self, tid, ctx):
        if not Wow64SetThreadContext(self.threads[tid], C.byref(ctx)):
            raise winerr("Wow64SetThreadContext")

    # breakpoints
    def add_role(self, addr, role):
        self.roles.setdefault(addr, set()).add(role)
        if addr not in self.bp_orig:
            self.bp_orig[addr] = self.read(addr, 1)[0]
        if addr not in self.bp_inserted and not self.bp_out.get(addr):
            self.write(addr, INT3)
            self.bp_inserted.add(addr)

    def drop_role(self, addr, role):
        r = self.roles.get(addr)
        if r is None:
            return
        r.discard(role)
        if not r:
            del self.roles[addr]
            if addr in self.bp_inserted:
                self.write(addr, bytes([self.bp_orig[addr]]))
                self.bp_inserted.discard(addr)

    def orig_code(self, addr, n):
        """Memory with our INT3s replaced by the original bytes. With
        emulation on, Game.exe's .text is read once per 4 KB page and kept
        (the code does not change; our own INT3s are undone from bp_orig)."""
        lo, hi = self.text_range
        if self.emulate != "off" and lo <= addr and addr + n <= hi:
            out, a = bytearray(), addr
            while a < addr + n:
                page = a & ~0xFFF
                p = self.code_pages.get(page)
                if p is None:
                    p = bytearray(self.read(page, 0x1000))
                    for b_addr, v in self.bp_orig.items():
                        if page <= b_addr < page + 0x1000:
                            p[b_addr - page] = v
                    self.code_pages[page] = p
                take = min(addr + n, page + 0x1000) - a
                out += p[a - page:a - page + take]
                a += take
            return bytes(out)
        b = bytearray(self.read(addr, n))
        for k in range(n):
            if addr + k in self.bp_inserted:
                b[k] = self.bp_orig[addr + k]
        return bytes(b)

    # logging
    def count(self, key):
        self.counts[key] = self.counts.get(key, 0) + 1

    def emit(self, rec):
        if "type" not in rec and "k" in rec:  # poke.py / send.py records name their kind `k`
            rec = dict(rec, type=rec["k"])
        rec["seq"] = self.seq
        rec["ms"] = round((time.perf_counter() - self.t0) * 1000, 1)
        self.seq += 1
        self.count(":".join(x for x in (rec["type"], rec.get("via"), rec.get("op")) if x))
        if self.frames:
            rec["frame"] = self.frame
            if "seed" in rec:
                hint = self.owner_hint(int(rec["seed"], 16))
                if hint:
                    rec["unit" if hint != "game" else "game_seed"] = hint if hint != "game" else True
        self.out.write(json.dumps(rec, separators=(",", ":")) + "\n")

    # thread suspension around single steps
    def freeze_others(self):
        for tid, h in self.threads.items():
            if tid not in self.stepping and tid not in self.suspended:
                if SuspendThread(h) != 0xFFFFFFFF:
                    self.suspended.add(tid)

    def thaw(self, tid):
        if tid in self.suspended:
            ResumeThread(self.threads[tid])
            self.suspended.discard(tid)

    def thaw_all(self):
        for tid in list(self.suspended):
            if tid in self.threads:
                ResumeThread(self.threads[tid])
        self.suspended.clear()

    # event handlers
    def on_breakpoint(self, tid, addr):
        ctx = self.get_ctx(tid)
        ctx.Eip = addr
        st = self.stepping.get(tid)
        if st:  # a new hit before the previous step finished
            self.finish_reinsert(tid, st)
            if st.get("trace"):
                self.finish_trace(tid, st, None, "interrupted")
        roles = set(self.roles.get(addr, ()))
        trace = None
        if "tickret" in roles:
            self.on_tick_return(tid, ctx)
        if "drain" in roles and self.send_layer is not None:
            # original-hooks.md §1 rule 4: injects and restores this thread's context
            self.send_layer.on_drain_call(self, tid)
        if "tick" in roles:
            self.on_tick(ctx)
        if "ret" in roles:
            self.on_return(tid, addr, ctx)
        if "helper" in roles:
            self.on_helper_entry(tid, addr, ctx)
        if "setter" in roles:
            self.on_setter(tid, addr, ctx)
        emu = None
        if "inline" in roles:
            if self.emulate != "off":
                emu = self.emu_inline(addr, ctx)
                if emu is not None and self.emulate == "on":
                    self.commit(tid, ctx, emu[1])
                    self.count("emu:inline")
                    mul, lo, new = emu[0]
                    self.emit(self.inline_record(tid, addr, mul, lo, new, None))
                    self.end_stepping(tid)
                    return
            # at the mov: seek the mul of that register, then the add/adc
            trace = {"site": addr, "mul": None, "lo": None, "steps": 0, "pending": None,
                     "new_lo": None, "reg": "Ecx" if self.bp_orig[addr] == 0xB9 else "Edx",
                     "emu": emu}
        elif self.emulate != "off":
            emu = self.emu_one(addr, ctx)
            if emu is not None and self.emulate == "on":
                self.commit(tid, ctx, emu)
                self.count("emu:one")
                self.end_stepping(tid)
                return
        # step over the original instruction
        if addr in self.bp_inserted:
            self.write(addr, bytes([self.bp_orig[addr]]))
            self.bp_inserted.discard(addr)
        self.bp_out.setdefault(addr, set()).add(tid)
        ctx.EFlags |= TRAP_FLAG
        self.set_ctx(tid, ctx)
        self.stepping[tid] = {"reinsert": addr, "trace": trace,
                              "emu": emu if trace is None else None}
        self.thaw(tid)
        self.freeze_others()

    def on_tick_return(self, tid, ctx):
        """The tick return 0x0052FD1E (ESI = game, game +0xA8 = the frame
        that ran): the pokes due after it, the send layer's frame, and the
        `frame F` input steps (as record_packets.py)."""
        import poke
        game = ctx.Esi
        frame = struct.unpack("<i", self.read(game + poke.G_FRAME, 4))[0]
        if self.poke_layer is not None:
            self.poke_layer.on_tick_return(self, game, frame, tid, ctx)
        if self.send_layer is not None:
            self.send_layer.on_tick_return(self, game)
        if self.auto is not None and self.auto.has_frames():
            self.auto.on_tick_return(self, frame)

    def end_stepping(self, tid):
        """An emulated hit that arrived while this thread was still being
        stepped (its step landed on a breakpoint): it steps no more."""
        if self.stepping.pop(tid, None) is not None and not self.stepping:
            self.thaw_all()

    def on_single_step(self, tid):
        st = self.stepping[tid]
        self.finish_reinsert(tid, st)
        if st.get("emu") is not None:  # --emulate check: one instruction
            self.emu_compare(tid, self.get_ctx(tid), st["emu"], "one")
            st["emu"] = None
        tr = st["trace"]
        if tr is not None and self.trace_step(tid, st, tr):
            return
        del self.stepping[tid]
        if not self.stepping:
            self.thaw_all()

    def trace_step(self, tid, st, tr):
        """One single-step of an inline trace. True = keep stepping."""
        ctx = self.get_ctx(tid)
        tr["steps"] += 1
        if tr["mul"] is None:
            # phase 1: the first `mul ecx|edx` while the register holds K
            if tr["steps"] > 24:
                st["trace"] = None
                self.count("inline:no_mul")  # e.g. a path that skips the draw
                return False
            code = self.orig_code(ctx.Eip, 2)
            want = MUL_FOR_MOV[0xB9 if tr["reg"] == "Ecx" else 0xBA]
            if code == want and getattr(ctx, tr["reg"]) == MULTIPLIER:
                tr["mul"], tr["lo"] = ctx.Eip, ctx.Eax
        else:
            # phase 2: the add then adc that build the new state
            if tr["pending"] is not None:
                kind, reg = tr["pending"]
                val = getattr(ctx, REGS[reg])
                tr["pending"] = None
                if kind == "add":
                    tr["new_lo"] = val
                elif kind == "adc" and tr["new_lo"] is not None:
                    if tr.get("emu") is not None:  # --emulate check: the whole trace
                        e = tr["emu"]
                        got = (tr["mul"], tr["lo"], (tr["new_lo"], val))
                        self.emu_compare(tid, ctx, e[1], "inline",
                                         None if e[0] == got else f"draw {e[0]} vs {got}")
                    self.finish_trace(tid, st, (tr["new_lo"], val), None)
                    return False
            if tr["steps"] > 64:
                self.finish_trace(tid, st, None, "no add/adc within 64 steps")
                return False
            tr["pending"] = decode_add_adc(self.orig_code(ctx.Eip, 6))
        ctx.EFlags |= TRAP_FLAG
        self.set_ctx(tid, ctx)
        return True

    def finish_reinsert(self, tid, st):
        a = st["reinsert"]
        if a is not None:
            st["reinsert"] = None
            out = self.bp_out.get(a, set())
            out.discard(tid)
            if not out and a in self.roles and a not in self.bp_inserted:
                self.write(a, INT3)
                self.bp_inserted.add(a)

    def finish_trace(self, tid, st, new_state, problem):
        tr = st["trace"]
        st["trace"] = None
        if tr.get("emu") is not None and problem is not None:
            self.count("emu_check:unfinished")
        if tr["mul"] is None:  # interrupted before the mul: no draw yet
            return
        self.emit(self.inline_record(tid, tr["site"], tr["mul"], tr["lo"], new_state, problem))

    @staticmethod
    def inline_record(tid, site, mul, lo, new_state, problem):
        rec = {"type": "draw", "via": "inline", "op": "step", "tid": tid,
               "site": f"{site:#x}", "mul": f"{mul:#x}"}
        if new_state is None:
            rec["before"] = [lo, None]
            rec["after"] = None
            rec["problem"] = problem
        else:
            lo2, hi2 = new_state
            hi = (lo2 - (lo * MULTIPLIER)) & M32  # spec §2: old hi from lo, lo'
            rec["before"] = [lo, hi]
            rec["after"] = [lo2, hi2]
            rec["ret"] = lo2
        return rec

    # --- emulation instead of single steps (x86emu; rng-trace.md §4 r7) ----
    def cpu_from(self, ctx, addr):
        return x86emu.Cpu([getattr(ctx, n) for n in REGS], addr, ctx.EFlags, self.read)

    def hooked_inside(self, ins, start):
        """True when an address of `ins` other than `start` holds one of our
        breakpoints: running it here would skip that hook."""
        return any(a in self.roles for a in range(ins.addr, ins.addr + ins.len) if a != start)

    def emu_one(self, addr, ctx):
        """The instruction under a breakpoint, run on a copy; None when it
        cannot be emulated exactly."""
        try:
            ins = x86emu.decode(addr, self.orig_code(addr, 16))
            if self.hooked_inside(ins, addr):
                return None
            cpu = self.cpu_from(ctx, addr)
            cpu.execute(ins)
        except (x86emu.Unsupported, OSError) as e:
            self.count("emu:fallback_one")
            self.emu_why(addr, e)
            return None
        if not cpu.exact():
            self.count("emu:fallback_one")
            return None
        return cpu

    def emu_inline(self, addr, ctx):
        """The inline trace (trace_step's rules: the first `mul` of the
        register while it holds K, then the add/adc pair) run on a copy from
        the `mov`. ((mul, lo, (lo', hi')), cpu), or None when an
        instruction is not emulated, a hook lies on the path, a limit is
        reached or a flag stays undefined (then the thread is stepped)."""
        reg = 1 if self.bp_orig[addr] == 0xB9 else 2
        want = MUL_FOR_MOV[self.bp_orig[addr]]
        win_lo, win = addr, self.orig_code(addr, 64)

        def code(a, n):
            nonlocal win_lo, win
            if not (win_lo <= a and a + n <= win_lo + len(win)):
                win_lo, win = a, self.orig_code(a, 64)
            return win[a - win_lo:a - win_lo + n]
        try:
            cpu = self.cpu_from(ctx, addr)
            steps, mul, lo, pending, new_lo = 0, None, None, None, None
            while True:
                if steps:
                    if mul is None:
                        if steps > 24:
                            raise x86emu.Unsupported("no mul within 24 instructions")
                        if code(cpu.eip, 2) == want and cpu.r[reg] == MULTIPLIER:
                            mul, lo = cpu.eip, cpu.r[0]
                    else:
                        if pending is not None:
                            kind, r = pending
                            val = cpu.r[r]
                            pending = None
                            if kind == "add":
                                new_lo = val
                            elif kind == "adc" and new_lo is not None:
                                break
                        if steps > 64:
                            raise x86emu.Unsupported("no add/adc within 64 instructions")
                        pending = decode_add_adc(code(cpu.eip, 6))
                ins = x86emu.decode(cpu.eip, code(cpu.eip, 16))
                if self.hooked_inside(ins, addr if not steps else None):
                    raise x86emu.Unsupported(f"hook inside {ins.addr:#x}")
                cpu.execute(ins)
                steps += 1
            if not cpu.exact():
                raise x86emu.Unsupported("undefined flag at the end")
        except (x86emu.Unsupported, OSError) as e:
            self.count("emu:fallback_inline")
            self.emu_why(addr, e)
            return None
        return (mul, lo, (new_lo, val)), cpu

    def emu_why(self, addr, e):
        key = f"emulation fell back at {addr:#x}: {e}"
        if key not in self.emu_notes:
            self.emu_notes.add(key)
            if len(self.emu_notes) <= 20:
                self.notes.append(key)

    def commit(self, tid, ctx, cpu):
        """Registers, eip, status flags and buffered writes of an emulated
        run into the thread (the breakpoint stays armed)."""
        run, start = b"", None
        for a in sorted(cpu.writes):
            if start is not None and a == start + len(run):
                run += bytes([cpu.writes[a]])
                continue
            if run:
                self.write_data(start, run)
            start, run = a, bytes([cpu.writes[a]])
        if run:
            self.write_data(start, run)
        for i, n in enumerate(REGS):
            setattr(ctx, n, cpu.r[i])
        ctx.Eip = cpu.eip
        ctx.EFlags = (ctx.EFlags & ~x86emu.STATUS & M32) | (cpu.fl & x86emu.STATUS)
        self.set_ctx(tid, ctx)

    def write_data(self, addr, data):
        buf = (C.c_ubyte * len(data)).from_buffer_copy(data)
        got = C.c_size_t()
        if not WriteProcessMemory(self.h_process, C.c_void_p(addr), buf, len(data),
                                  C.byref(got)):
            raise winerr(f"WriteProcessMemory {addr:#x}")

    def emu_compare(self, tid, ctx, cpu, what, extra=None):
        """--emulate check: the real state after the single steps against
        the emulated one."""
        diffs = [f"{n} {getattr(ctx, n):#x} vs {cpu.r[i]:#x}" for i, n in enumerate(REGS)
                 if getattr(ctx, n) != cpu.r[i]]
        if ctx.Eip != cpu.eip:
            diffs.append(f"eip {ctx.Eip:#x} vs {cpu.eip:#x}")
        if (ctx.EFlags ^ cpu.fl) & x86emu.STATUS:
            diffs.append(f"flags {ctx.EFlags & x86emu.STATUS:#x} vs {cpu.fl & x86emu.STATUS:#x}")
        for a, v in sorted(cpu.writes.items()):
            if self.read(a, 1)[0] != v:
                diffs.append(f"byte {a:#x}")
                break
        if extra:
            diffs.append(extra)
        self.count(f"emu_check:{what}_{'diff' if diffs else 'ok'}")
        if diffs and len(self.emu_notes) < 40:
            self.emu_notes.add(f"check {what} {tid}")
            self.notes.append(f"emulation check {what} differs at eip {ctx.Eip:#x}: "
                              + "; ".join(diffs))

    def on_helper_entry(self, tid, addr, ctx):
        op, layout, _ = HELPERS[addr]
        esp = ctx.Esp
        ret_addr = self.read_u32(esp)
        call = {"op": op, "fn": addr, "ptr": ctx.Ecx, "ret_addr": ret_addr, "esp": esp,
                "before": list(struct.unpack("<II", self.read(ctx.Ecx, 8)))}
        if layout == "n":
            call["n"] = ctx.Edx
        elif layout == "min,n":
            call["min"] = ctx.Edx
            call["n"] = self.read_u32(esp + 4)
        self.calls.setdefault(tid, []).append(call)
        self.ret_refs[ret_addr] = self.ret_refs.get(ret_addr, 0) + 1
        self.add_role(ret_addr, "ret")

    def on_return(self, tid, addr, ctx):
        stack = self.calls.get(tid)
        if stack and stack[-1]["ret_addr"] == addr and ctx.Esp > stack[-1]["esp"]:
            call = stack.pop()
            rec = {"type": "draw", "via": "helper", "op": call["op"], "tid": tid,
                   "fn": f"{call['fn']:#x}", "site": f"{self.call_site(addr):#x}",
                   "seed": f"{call['ptr']:#x}", "before": call["before"],
                   "after": list(struct.unpack("<II", self.read(call["ptr"], 8))),
                   "ret": ctx.Eax}
            for k in ("n", "min"):
                if k in call:
                    rec[k] = call[k]
            self.emit(rec)
            self.ret_refs[addr] -= 1
            if self.ret_refs[addr] == 0:
                del self.ret_refs[addr]
                if self.emulate == "off":
                    self.drop_role(addr, "ret")
                # else the return INT3 stays: a hit without a pending call is
                # ignored above, and two memory writes per call are saved

    # --- frames and owners (--frames) ---------------------------------------
    def on_tick(self, ctx):
        """Tick entry: the frame, the game seed and every server unit's seed
        at the start of the tick (rng-trace.md §4 r1-r3)."""
        game = ctx.Ecx
        if self.game is None:
            self.game = game
        if game != self.game:
            return
        self.ticks += 1
        if self.max_ticks and self.ticks > self.max_ticks:
            if not self.done:
                self.notes.append(f"tick limit {self.max_ticks} reached")
            self.done = True
            return
        self.frame = (self.read_u32(game + G_FRAME) + 1) & M32
        gseed = list(struct.unpack("<II", self.read(game + G_SEED, 8)))
        units = []
        heads = self.read(game + G_HASH, 0xA00)
        for t, off in HASH_TYPES:
            for b in range(128):
                u = struct.unpack_from("<I", heads, off + 4 * b)[0]
                n = 0
                while u and n < 100000:
                    raw = self.read(u, U_HASH_NEXT + 4)
                    ut, = struct.unpack_from("<I", raw, U_TYPE)
                    g, = struct.unpack_from("<I", raw, U_GUID)
                    lo, hi = struct.unpack_from("<II", raw, U_SEED)
                    units.append([ut, g, lo, hi])
                    u = struct.unpack_from("<I", raw, U_HASH_NEXT)[0]
                    n += 1
        units.sort()
        self.emit({"type": "tick", "f": self.frame, "game": f"{game:#x}",
                   "gseed": gseed, "units": units})

    def owner_hint(self, addr):
        """'game' for game +0xD0, 'T:G' when addr - 0x20 is a server unit,
        else None (read now: the unit may be freed later)."""
        if self.game is not None and addr == self.game + G_SEED:
            return "game"
        try:
            raw = self.read(addr - U_SEED, U_FLAGS2 + 4)
        except OSError:
            return None
        ut, = struct.unpack_from("<I", raw, U_TYPE)
        g, = struct.unpack_from("<I", raw, U_GUID)
        fl, = struct.unpack_from("<I", raw, U_FLAGS2)
        if ut <= 5 and fl & SERVER_UNIT:
            return f"{ut}:{g}"
        return None

    def call_site(self, ret_addr):
        b = self.orig_code(ret_addr - 5, 5)
        return ret_addr - 5 if b[0] == 0xE8 else ret_addr

    def on_setter(self, tid, addr, ctx):
        kind = SETTERS[addr]
        ptr = ctx.Ecx
        if kind == "init":
            new = [1, 666]
        elif kind == "init_low":
            new = [ctx.Edx, 666]
        else:
            new = [ctx.Edx, self.read_u32(ctx.Esp + 4)]
        ret_addr = self.read_u32(ctx.Esp)
        self.emit({"type": "seed_set", "op": kind, "tid": tid, "fn": f"{addr:#x}",
                   "site": f"{self.call_site(ret_addr):#x}", "seed": f"{ptr:#x}",
                   "old": list(struct.unpack("<II", self.read(ptr, 8))), "new": new})

    def install(self, base):
        if base != IMAGE_BASE:
            raise RuntimeError(f"Game.exe loaded at {base:#x}, expected {IMAGE_BASE:#x}")
        expect = {0x45C370: b"\x8B\x01", 0x45C390: b"\x56\x8B", 0x650E30: b"\xC7\x01"}
        for a, b in expect.items():
            if self.read(a, len(b)) != b:
                raise RuntimeError(f"unexpected code at {a:#x}: not the 1.14d Game.exe?")
        for a in HELPERS:
            self.add_role(a, "helper")
        for a in SETTERS:
            self.add_role(a, "setter")
        if self.frames:
            if self.read(TICK, 3) != TICK_BYTES:
                raise RuntimeError(f"unexpected code at {TICK:#x}: not the 1.14d Game.exe?")
            self.add_role(TICK, "tick")
        if (self.poke_layer is not None or self.send_layer is not None
                or (self.auto is not None and self.auto.has_frames())):
            import poke
            if self.read(poke.TICK_RET, len(poke.TICK_RET_BYTES)) != poke.TICK_RET_BYTES:
                raise RuntimeError("unexpected code at 0x0052FD1E: not the 1.14d Game.exe?")
            self.add_role(poke.TICK_RET, "tickret")
        if self.send_layer is not None:
            import send
            if self.read(send.DRAIN_CALL, len(send.DRAIN_CALL_BYTES)) != send.DRAIN_CALL_BYTES:
                raise RuntimeError("unexpected code at 0x0044F136: not the 1.14d Game.exe?")
            self.add_role(send.DRAIN_CALL, "drain")
        n_inline = 0
        if self.with_inline:
            for a in self.inline_sites:
                if any(lo <= a < hi for lo, hi in self.skip_ranges):
                    continue
                code = self.read(a, 5)
                if code[0] not in MUL_FOR_MOV or code[1:] != struct.pack("<I", MULTIPLIER):
                    raise RuntimeError(f"inline site {a:#x} does not match Game.exe")
                self.add_role(a, "inline")
                n_inline += 1
        return n_inline

    def run(self):
        exe_bytes = open(self.exe, "rb").read()
        sha = hashlib.sha256(exe_bytes).hexdigest()
        if sha != GAME_EXE_SHA256:
            raise RuntimeError(f"{self.exe}: sha256 {sha} is not the reference 1.14d Game.exe")
        self.inline_sites, odd = find_inline_sites(exe_bytes)
        self.text_range = text_section(exe_bytes)
        if odd:
            self.notes.append(f"{odd} multiplier immediates without a nearby mul (not hooked)")
        os.makedirs(os.path.dirname(self.out_path), exist_ok=True)
        self.out = open(self.out_path, "w", encoding="utf-8", newline="\n")
        si = STARTUPINFOW()
        si.cb = C.sizeof(si)
        pi = PROCESS_INFORMATION()
        cmd = C.create_unicode_buffer(" ".join([f'"{self.exe}"'] + self.args))
        cwd = os.path.dirname(self.exe)
        if not CreateProcessW(self.exe, cmd, None, None, False, DEBUG_ONLY_THIS_PROCESS,
                              None, cwd, C.byref(si), C.byref(pi)):
            raise winerr("CreateProcessW")
        self.h_process, self.pid = pi.hProcess, pi.dwProcessId
        DebugSetProcessKillOnExit(True)
        self.t0 = time.perf_counter()
        deadline = self.t0 + self.seconds
        header = {"type": "header", "format": RAW_FORMAT, "tool": TOOL,
                  "date": datetime.date.today().isoformat(), "game_exe_sha256": sha,
                  "args": self.args, "pid": self.pid, "seconds": self.seconds,
                  "inline": self.with_inline, "side": "orig", "frames": self.frames,
                  "max_ticks": self.max_ticks, "emulate": self.emulate,
                  "skip_inline": [[f"{a:#x}", f"{b:#x}"] for a, b in self.skip_ranges]}
        self.out.write(json.dumps(header) + "\n")
        try:
            self.loop(deadline)
        finally:
            self.kill()
            self.out.write(json.dumps({"type": "footer", "events": self.seq,
                                       "counts": self.counts, "notes": self.notes,
                                       "debug_events": self.dbg,
                                       "foreign_exceptions": self.exc}) + "\n")
            self.out.close()
            CloseHandle(pi.hThread)
        return self.counts

    def kill(self):
        if self.h_process and WaitForSingleObject(self.h_process, 0) != WAIT_OBJECT_0:
            TerminateProcess(self.h_process, 1)
            if self.pending:  # an event we never continued (error path)
                ContinueDebugEvent(self.pending[0], self.pending[1], DBG_CONTINUE)
                self.pending = None
            # drain events so the process can actually exit
            ev = DEBUG_EVENT()
            t_end = time.perf_counter() + 10
            while time.perf_counter() < t_end:
                if WaitForDebugEvent(C.byref(ev), 200):
                    code = ev.dwDebugEventCode
                    self.close_event_handles(ev)
                    ContinueDebugEvent(ev.dwProcessId, ev.dwThreadId, DBG_CONTINUE)
                    if code == EXIT_PROCESS_DEBUG_EVENT:
                        break
                elif WaitForSingleObject(self.h_process, 0) == WAIT_OBJECT_0:
                    break
            if WaitForSingleObject(self.h_process, 5000) != WAIT_OBJECT_0:
                self.notes.append("process did not exit after TerminateProcess")

    @staticmethod
    def close_event_handles(ev):
        if ev.dwDebugEventCode == CREATE_PROCESS_DEBUG_EVENT and ev.u.CreateProcessInfo.hFile:
            CloseHandle(ev.u.CreateProcessInfo.hFile)
        elif ev.dwDebugEventCode == LOAD_DLL_DEBUG_EVENT and ev.u.LoadDll.hFile:
            CloseHandle(ev.u.LoadDll.hFile)

    def loop(self, deadline):
        ev = DEBUG_EVENT()
        while True:
            if self.auto is not None and self.auto.poll(self):
                self.notes.append("autostart: input script ended the recording")
                return
            if time.perf_counter() > deadline:
                self.notes.append(f"time limit {self.seconds}s reached")
                return
            if self.max_events and self.seq >= self.max_events:
                self.notes.append(f"event limit {self.max_events} reached")
                return
            if self.done:
                return
            if not WaitForDebugEvent(C.byref(ev), 100):
                continue
            code, tid = ev.dwDebugEventCode, ev.dwThreadId
            self.pending = (ev.dwProcessId, tid)
            self.dbg[code] = self.dbg.get(code, 0) + 1
            status = DBG_CONTINUE
            if code == CREATE_PROCESS_DEBUG_EVENT:
                info = ev.u.CreateProcessInfo
                self.threads[tid] = info.hThread
                if info.hFile:
                    CloseHandle(info.hFile)
                n = self.install(info.lpBaseOfImage or 0)
                self.notes.append(f"{len(HELPERS)} helpers, {len(SETTERS)} setters, "
                                  f"{n} inline sites hooked")
            elif code == CREATE_THREAD_DEBUG_EVENT:
                self.threads[tid] = ev.u.CreateThread.hThread
                if self.stepping:  # keep the freeze invariant
                    if SuspendThread(self.threads[tid]) != 0xFFFFFFFF:
                        self.suspended.add(tid)
            elif code == EXIT_THREAD_DEBUG_EVENT:
                self.threads.pop(tid, None)
                self.suspended.discard(tid)
                self.stepping.pop(tid, None)
                self.calls.pop(tid, None)
            elif code == LOAD_DLL_DEBUG_EVENT:
                if ev.u.LoadDll.hFile:
                    CloseHandle(ev.u.LoadDll.hFile)
            elif code == EXIT_PROCESS_DEBUG_EVENT:
                self.notes.append(f"game exited, code {ev.u.ExitProcess.dwExitCode:#x}")
                ContinueDebugEvent(ev.dwProcessId, tid, DBG_CONTINUE)
                self.pending = None
                return
            elif code == EXCEPTION_DEBUG_EVENT:
                status = self.on_exception(tid, ev.u.Exception)
            ContinueDebugEvent(ev.dwProcessId, tid, status)
            self.pending = None

    def on_exception(self, tid, info):
        rec = info.ExceptionRecord
        code = rec.ExceptionCode
        addr = rec.ExceptionAddress or 0
        if code in BREAKPOINT_CODES:
            if addr in self.bp_orig:  # one of ours (possibly removed since)
                self.on_breakpoint(tid, addr)
                return DBG_CONTINUE
            if self.initial_bps < 2:  # loader breakpoints (64-bit and WOW64)
                self.initial_bps += 1
                return DBG_CONTINUE
            return DBG_EXCEPTION_NOT_HANDLED
        if code in SINGLE_STEP_CODES and tid in self.stepping:
            self.on_single_step(tid)
            return DBG_CONTINUE
        key = f"{code:#x}"
        self.exc[key] = self.exc.get(key, 0) + 1
        if not info.dwFirstChance:
            self.notes.append(f"second-chance exception {code:#x} at {addr:#x} (tid {tid})")
        return DBG_EXCEPTION_NOT_HANDLED


# --skip-inline presets: code ranges whose inline draws step no game or unit
# seed in the measured runs (specs/tools/rng-trace.md §4 r6).
SKIP_PRESETS = {
    "drlg": ((0x642000, 0x643000), (0x66B000, 0x682000)),  # rng_owners.DRLG_SITES
}


def parse_ranges(text):
    out = []
    for part in (text or "").split(","):
        part = part.strip()
        if not part:
            continue
        if part in SKIP_PRESETS:
            out.extend(SKIP_PRESETS[part])
            continue
        lo, _, hi = part.partition("-")
        out.append((int(lo, 16), int(hi, 16)))
    return tuple(out)


def main():
    import autostart  # unattended start, input script
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=30.0,
                    help="stop and kill the game after this many seconds (default 30)")
    ap.add_argument("--max-events", type=int, default=0, help="stop after N events (0 = no limit)")
    ap.add_argument("--no-inline", action="store_true",
                    help="hook only the helpers and setters, not the inlined steps")
    ap.add_argument("--out", default=None, help="output .jsonl (default traces/raw/<time>-rng.jsonl)")
    ap.add_argument("--frames", action="store_true",
                    help="tick markers with every server unit's seed, a frame on every record, "
                         "owner hints, and the owner post-pass (specs/tools/rng-trace.md §4)")
    ap.add_argument("--ticks", type=int, default=0,
                    help="with --frames: stop at the entry of tick N + 1 (0 = no limit)")
    ap.add_argument("--skip-inline", default="",
                    help="LO-HI[,LO-HI...]: inline sites in these code ranges are not hooked "
                         "(e.g. 'drlg' = %s); faster, those draws are missing" % (
                             ",".join(f"{a:#x}-{b:#x}" for a, b in SKIP_PRESETS["drlg"])))
    ap.add_argument("--emulate", choices=("on", "off", "check"), default="on",
                    help="run the instructions under a breakpoint and an inline trace in "
                         "x86emu.py instead of single-stepping (on, default), never (off), or "
                         "single-step and compare every emulated result (check)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"],
                    help="Game.exe arguments (default: -w -ns)")
    autostart.add_options(ap)
    import poke  # --poke / --poke-file (specs/tools/poke.md §2 rule 6)
    import send  # --send (specs/tools/scenario-diff.md §2 `at … send`)
    poke.add_options(ap)
    send.add_options(ap)
    a = ap.parse_args()
    if a.ticks and not a.frames:
        ap.error("--ticks needs --frames")
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-rng.jsonl")
    r = Recorder(os.path.abspath(a.game), gargs, out, a.seconds,
                 not a.no_inline, a.max_events)
    r.auto = auto
    r.frames, r.max_ticks = a.frames, a.ticks
    r.skip_ranges = parse_ranges(a.skip_inline)
    r.emulate = a.emulate
    r.poke_layer = poke.PokeLayer.from_args(a)
    r.send_layer = send.SendLayer.from_args(a)
    for what, layer in (("pokes", r.poke_layer), ("sends", r.send_layer)):
        if layer is not None:
            r.notes.append(f"{what}: {len(layer.pending())} directive(s)")
    if (r.poke_layer is not None or r.send_layer is not None) and not a.frames:
        ap.error("--poke / --send need --frames (frame-anchored)")
    t0 = time.perf_counter()
    try:
        counts = r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
        counts = r.counts
    secs = time.perf_counter() - t0
    if a.frames:
        import rng_owners
        summary = rng_owners.assign_file(out)
        print("owners:", "  ".join(f"{k}={v}" for k, v in sorted(summary.items())))
    print(f"wrote {out}")
    print(f"speed: {r.seq} records in {secs:.1f} s ({r.seq / max(secs, 0.001):.0f}/s), "
          f"debug events {sum(r.dbg.values())} ({sum(r.dbg.values()) / max(secs, 0.001):.0f}/s), "
          f"ticks {r.ticks}")
    print(f"events: {r.seq}  " + "  ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    for n in r.notes:
        print("note:", n)


if __name__ == "__main__":
    main()
