"""Record what the original 1.14d Game.exe hands to DirectSound, per voice,
with the server tick of every call (specs/tools/audio-diff.md §2).

Built on record_tick.TickRecorder (the shared Win32 debugger: persistent
INT3s, stepped over and re-armed). The game imports DSOUND.dll statically
(IAT slot 0x006CC088 = ordinal 1, DirectSoundCreate), so the hooks are
found at run time and need no Game.exe internals besides the tick hook:

  entry point       read the IAT slot, arm DirectSoundCreate
  DirectSoundCreate return: *ppDS -> IDirectSound vtable: arm
                    CreateSoundBuffer (3) and DuplicateSoundBuffer (5)
  CreateSoundBuffer return: *ppBuf, with the DSBUFFERDESC (flags, bytes,
                    format) read at entry; the first time a vtable is
                    seen its IDirectSoundBuffer methods are armed:
                    Release 2, Lock 11, Play 12, SetCurrentPosition 13,
                    SetVolume 15, SetPan 16, SetFrequency 17, Stop 18,
                    Unlock 19
  Unlock            the bytes written (both lock regions) with the
                    offset of the matching Lock: one `write` record,
                    the bytes in a blob file named by their sha256
  0x004B9A00        the sound request entry (id, unit, delay, flags,
                    offset), as record_frames.py --sounds

Every record carries the server frame `f` (record_tick's TICK hook; None
before the game ticks), the sound tick `T` ([0x7BC9BC]), the client
update counter `C` ([0x7A0498]) and the calling thread. Output: JSON
lines (format audio-raw-1) in --out; blobs in --blob-dir (default
game/captures/audio-<time>/, gitignored: they are the game's samples,
CLAUDE.md rule 1, and never go into the repository; the JSON lines carry
only digests and sizes). The game is always terminated when this ends.

Under Wine the game needs an audio device: tools/audio-diff/capture.sh
writes an ALSA configuration whose default device is a file sink and
runs this script through tools/cloud-game/run.sh --python.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import hashlib
import json
import os
import struct
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(REPO, "tools", "trace-recorder"))
import autostart  # noqa: E402
import x86emu  # noqa: E402
import poke  # noqa: E402  (--poke: state injection, specs/tools/poke.md)
import send  # noqa: E402  (--send: C->S messages, specs/tools/original-hooks.md §1 r4)

TOOL = "audio-diff record_audio 1.0.0"
FORMAT = "audio-raw-1"

IAT_DSOUND_CREATE = 0x6CC088      # DSOUND.dll ordinal 1 (import table of the 1.14d Game.exe)
SOUND_REQUEST = 0x4B9A00          # audio/triggers.md §1 r1 (record_frames.py SOUND_REQUEST)
SOUND_REQUEST_BYTES = b"\x55\x8B\xEC\x83\xEC\x18"
SOUND_ROLL = 0x4E40A0             # roll(n) on the local player's client seed (audio/sound-table.md §4 r5)
SOUND_ROLL_BYTES = b"\x56\x8B\xF1\xE8"
ROLL_GENERIC = 0x45C3E0           # roll(seed in ECX, n in EDX), sim/rng.md §3
ROLL_GENERIC_BYTES = b"\x56\x8B\xF2\x85"
ROLL_RANGE = 0x472280             # roll_range(seed in ECX, lo in EDX, n on the stack)
ROLL_RANGE_BYTES = b"\x55\x8B\xEC\x8B"
PLAYER_PTR = 0x7A6A70             # local player unit; seed {lo, hi} at +0x20, +0x24
SOUND_TICK = 0x7BC9BC             # T (audio/sound-table.md §6.1)
CLIENT_UPDATES = 0x7A0498         # C (record_frames.py CLIENT_UPDATES)

# COM vtable slots (dsound.h)
DS_CREATE_BUFFER, DS_DUPLICATE = 3, 5
BUF_METHODS = {2: "release", 11: "lock", 12: "play", 13: "setpos", 15: "volume", 16: "pan",
               17: "freq", 18: "stop", 19: "unlock"}
DSBCAPS_PRIMARYBUFFER = 0x1
DSBLOCK_ENTIREBUFFER = 0x2


CAVE_SIZE = 0x1000
GAME_IMAGE_END = 0x01000000       # Game.exe's image ends below; DLL code is above
RELOCATABLE_NOT = ("jcc", "jmp")


def trampoline_bytes(addr, code, at):
    """The first instruction of `code` (at `addr`) copied to `at`, then a jmp
    rel32 to the next instruction; None when it cannot move (undecodable,
    or a relative jump)."""
    try:
        insn = x86emu.decode(addr, code)
    except x86emu.Unsupported:
        return None
    if insn.op in RELOCATABLE_NOT:
        return None
    n = insn.len
    rel = (addr + n) - (at + n + 5)
    return bytes(code[:n]) + b"\xE9" + struct.pack("<i", rel)


def virtual_alloc(h_process, size):
    import ctypes as C
    from ctypes import wintypes as W
    k32 = C.WinDLL("kernel32", use_last_error=True)
    f = k32.VirtualAllocEx
    f.restype = C.c_void_p
    f.argtypes = [W.HANDLE, C.c_void_p, C.c_size_t, W.DWORD, W.DWORD]
    p = f(h_process, None, size, 0x3000, 0x40)   # MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE
    if not p:
        raise OSError(f"VirtualAllocEx failed: {C.get_last_error()}")
    return p


def signed(v):
    return struct.unpack("<i", struct.pack("<I", v & 0xFFFFFFFF))[0]


def entry_point(exe_bytes):
    """(address, first 3 bytes) of the PE entry point (image base 0x400000)."""
    pe = struct.unpack_from("<I", exe_bytes, 0x3C)[0]
    rva = struct.unpack_from("<I", exe_bytes, pe + 0x28)[0]
    nsec = struct.unpack_from("<H", exe_bytes, pe + 6)[0]
    opt = pe + 24 + struct.unpack_from("<H", exe_bytes, pe + 20)[0]
    for i in range(nsec):
        vsize, va, rsize, raw = struct.unpack_from("<IIII", exe_bytes, opt + 40 * i + 8)
        if va <= rva < va + max(vsize, rsize):
            off = rva - va + raw
            return 0x400000 + rva, exe_bytes[off:off + 3]
    raise ValueError("entry point outside every section")


def parse_desc(raw):
    """DSBUFFERDESC (dwSize, dwFlags, dwBufferBytes, dwReserved, lpwfxFormat) -> dict."""
    size, flags, nbytes, _res, fmt = struct.unpack_from("<5I", raw)
    return {"size": size, "flags": flags, "bytes": nbytes, "fmt_ptr": fmt}


def parse_wfx(raw):
    tag, ch, rate, avg, align, bits = struct.unpack_from("<HHIIHH", raw)
    return {"tag": tag, "ch": ch, "rate": rate, "avg": avg, "align": align, "bits": bits}


class BlobStore:
    """Bytes kept once per sha256 (`<sha>.pcm`); None directory = digests only."""
    def __init__(self, directory):
        self.dir = directory
        self.seen = set()
        if directory:
            os.makedirs(directory, exist_ok=True)

    def put(self, data):
        h = hashlib.sha256(data).hexdigest()
        if self.dir and h not in self.seen:
            path = os.path.join(self.dir, h + ".pcm")
            if not os.path.exists(path):
                with open(path, "wb") as f:
                    f.write(data)
        self.seen.add(h)
        return h


class AudioCore:
    """The hook logic, independent of the debugger (the selftest drives it with
    fake memory). `mem` has read(addr, n), u32(addr), arm(addr), emit(rec) and
    stamp() -> dict."""

    def __init__(self, mem, blobs):
        self.m, self.blobs = mem, blobs
        self.dsc = None            # DirectSoundCreate address
        self.ds_vtbls = set()
        self.buf_vtbls = {}        # vtable -> {method address: name}
        self.methods = {}          # method address -> name
        self.create_at = set()     # CreateSoundBuffer / Duplicate addresses
        self.create_at_kind = {}   # address -> "create" | "dup"
        self.returns = {}          # return address -> {tid: [(kind, data), ...]}
        self.locks = {}            # (buffer, thread) -> (offset, bytes, flags)
        self.buffers = {}          # buffer -> {"n": id, ...}
        self.next_buffer = 0
        self.counts = {}

    def count(self, k):
        self.counts[k] = self.counts.get(k, 0) + 1

    def push_return(self, ret, tid, kind, data):
        self.m.arm(ret)
        self.returns.setdefault(ret, {}).setdefault(tid, []).append((kind, data))

    def hooks(self):
        return set(self.methods) | self.create_at | set(self.returns) | ({self.dsc} if self.dsc else set())

    def entry(self):
        """At the game's entry point: imports are bound."""
        self.dsc = self.m.u32(IAT_DSOUND_CREATE)
        self.m.emit({"k": "dsound", "create": f"{self.dsc:#x}"})
        if self.dsc:
            self.m.arm(self.dsc)

    def handle(self, addr, tid, esp, regs):
        """True when `addr` is one of ours."""
        if addr in self.returns and tid in self.returns[addr] and self.returns[addr][tid]:
            kind, data = self.returns[addr][tid].pop()
            self.on_return(kind, data, regs)
            return True
        if addr == self.dsc:
            ret, _guid, pp = struct.unpack("<3I", self.m.read(esp, 12))
            self.push_return(ret, tid, "dsc", pp)
            return True
        if addr in self.create_at:
            ret, this, a1, a2 = struct.unpack("<4I", self.m.read(esp, 16))
            if self.create_at_kind[addr] == "create":
                desc = parse_desc(self.m.read(a1, 20))
                if desc["fmt_ptr"]:
                    desc["fmt"] = parse_wfx(self.m.read(desc["fmt_ptr"], 16))
                del desc["fmt_ptr"]
                self.push_return(ret, tid, "create", (a2, desc, None))
            else:  # DuplicateSoundBuffer(this, original, ppDup)
                self.push_return(ret, tid, "create", (a2, {"dup_of": self.buffer_no(a1)}, a1))
            return True
        name = self.methods.get(addr)
        if name is None:
            return False
        self.method(name, tid, esp)
        return True

    def buffer_no(self, buf):
        b = self.buffers.get(buf)
        return b["n"] if b else None

    def on_return(self, kind, data, regs):
        hr = regs.get("eax", 0)
        if kind == "dsc":
            ds = self.m.u32(data) if hr == 0 else 0
            self.m.emit({"k": "ds", "hr": f"{hr:#x}", "ds": f"{ds:#x}"})
            if ds:
                vt = self.m.u32(ds)
                if vt not in self.ds_vtbls:
                    self.ds_vtbls.add(vt)
                    for slot, k in ((DS_CREATE_BUFFER, "create"), (DS_DUPLICATE, "dup")):
                        fn = self.m.u32(vt + 4 * slot)
                        self.create_at.add(fn)
                        self.create_at_kind[fn] = k
                        self.m.arm(fn)
        elif kind == "create":
            pp, desc, _orig = data
            buf = self.m.u32(pp) if hr == 0 else 0
            rec = {"k": "create", "hr": f"{hr:#x}", **desc}
            if buf:
                self.next_buffer += 1
                self.buffers[buf] = {"n": self.next_buffer}
                rec["b"] = self.next_buffer
                rec["ptr"] = f"{buf:#x}"
                vt = self.m.u32(buf)
                primary = bool(desc.get("flags", 0) & DSBCAPS_PRIMARYBUFFER)
                if vt not in self.buf_vtbls and not primary:
                    table = {}
                    for slot, name in BUF_METHODS.items():
                        fn = self.m.u32(vt + 4 * slot)
                        table[fn] = name
                        self.methods[fn] = name
                        self.m.arm(fn)
                    self.buf_vtbls[vt] = table
                rec["vt"] = f"{vt:#x}"
            self.count("create")
            self.m.emit(self.stamped(rec))

    def stamped(self, rec):
        rec.update(self.m.stamp())
        return rec

    def method(self, name, tid, esp):
        # Lock(this, off, bytes, pp1, pn1, pp2, pn2, flags) has the most arguments
        args = struct.unpack("<8I", self.m.read(esp + 4, 32))
        buf = args[0]
        b = self.buffer_no(buf)
        if b is None and name != "release":
            # a buffer this run did not see created (primary, or before the hooks)
            b = f"?{buf:#x}"
        self.count(name)
        if name == "lock":
            self.locks[(buf, tid)] = (args[1], args[2], args[7])
            return   # the write record is made at Unlock
        if name == "unlock":
            p1, n1, p2, n2 = args[1:5]
            data = (self.m.read(p1, n1) if p1 and n1 else b"") + (self.m.read(p2, n2) if p2 and n2 else b"")
            off, want, flags = self.locks.pop((buf, tid), (None, None, 0))
            rec = {"k": "write", "b": b, "off": off, "n1": n1, "n2": n2,
                   "entire": bool(flags & DSBLOCK_ENTIREBUFFER), "len": len(data),
                   "sha256": self.blobs.put(data) if data else None, "tid": tid}
            self.m.emit(self.stamped(rec))
            return
        if name == "release":
            if b is None:
                return
            rec = {"k": "release", "b": b}
        elif name == "play":
            rec = {"k": "play", "b": b, "flags": args[3]}
        elif name == "stop":
            rec = {"k": "stop", "b": b}
        elif name == "volume":
            rec = {"k": "volume", "b": b, "v": signed(args[1])}
        elif name == "pan":
            rec = {"k": "pan", "b": b, "v": signed(args[1])}
        elif name == "freq":
            rec = {"k": "freq", "b": b, "v": args[1]}
        else:  # setpos
            rec = {"k": "setpos", "b": b, "v": args[1]}
        rec["tid"] = tid
        self.m.emit(self.stamped(rec))


def make_recorder(rt):
    class Recorder(rt.TickRecorder):
        def __init__(self, exe, args, out, seconds, max_ticks, blob_dir):
            super().__init__(exe, args, out, seconds, 0, max_ticks)
            self.core = AudioCore(self, BlobStore(blob_dir))
            self.entry_addr = None
            self.caves, self.cave, self.cave_used = {}, None, 0

        # Race-free hooks (§2 rule 4): the INT3 stays in place and the thread
        # resumes in a trampoline holding the relocated first instruction and a
        # jump back. Lifting the INT3 for a single step lets a second thread
        # (the stream thread and the main thread both enter Lock / Unlock) run
        # through it unseen, and suspending the other threads instead
        # deadlocks under Wine. Undecodable first instructions fall back to the
        # base recorder's step-over.
        def arm(self, addr):
            if addr in self.bp_orig:
                return
            code = self.read(addr, 16)
            self.bp_orig[addr] = code[0]
            if addr < GAME_IMAGE_END:
                # Game.exe hooks (tick, request, entry, returns, the poke and
                # send layers, which redirect the thread in their handlers):
                # the base step-over
                self.write(addr, rt.rr.INT3)
                return
            tramp = trampoline_bytes(addr, code, self.cave_next())
            if tramp is not None:
                at = self.cave_next()
                self.write(at, tramp)
                self.cave_used += len(tramp)
                self.caves[addr] = at
            else:
                self.notes.append(f"hook {addr:#x}: first instruction not relocatable, single-stepped")
            self.write(addr, rt.rr.INT3)

        def cave_next(self):
            if self.cave is None or self.cave_used + 32 > CAVE_SIZE:
                self.cave = virtual_alloc(self.h_process, CAVE_SIZE)
                self.cave_used = 0
            return self.cave + self.cave_used

        def on_breakpoint(self, tid, addr):
            at = self.caves.get(addr)
            if at is None:
                super().on_breakpoint(tid, addr)
                return
            ctx = self.get_ctx(tid)
            ctx.Eip = addr
            try:
                self.handle(addr, ctx)
            finally:
                ctx.Eip = at
                self.set_ctx(tid, ctx)

        def stamp(self):
            return {"f": self.frame, "T": self.u32(SOUND_TICK), "C": self.u32(CLIENT_UPDATES)}

        def handle(self, addr, ctx):
            if addr == self.entry_addr:
                self.core.entry()
                return
            tid = self.pending[1] if self.pending else 0
            regs = {"eax": ctx.Eax}
            if self.core.handle(addr, tid, ctx.Esp, regs):
                return
            if addr == SOUND_REQUEST:
                unit = ctx.Edx
                rec = {"k": "request", "id": signed(ctx.Ecx),
                       "unit": [self.u32(unit), self.u32(unit + 0xC)] if unit else None,
                       "delay": self.u32(ctx.Esp + 4), "flags": self.u32(ctx.Esp + 8),
                       "offset": self.u32(ctx.Esp + 12), "ret": f"{self.u32(ctx.Esp):#x}"}
                p = self.u32(PLAYER_PTR)
                if p:
                    rec["seed"] = [self.u32(p + 0x20), self.u32(p + 0x24)]
                self.emit(self.core.stamped(rec))
                return
            if addr == SOUND_ROLL:
                p = self.u32(PLAYER_PTR)
                rec = {"k": "roll", "n": signed(ctx.Ecx), "ret": f"{self.u32(ctx.Esp):#x}",
                       "seed": [self.u32(p + 0x20), self.u32(p + 0x24)] if p else None}
                self.emit(self.core.stamped(rec))
                return
            if addr in (ROLL_GENERIC, ROLL_RANGE):
                p = self.u32(PLAYER_PTR)
                if p and ctx.Ecx == p + 0x20:
                    n = signed(ctx.Edx) if addr == ROLL_GENERIC else signed(self.u32(ctx.Esp + 4))
                    rec = {"k": "roll", "src": f"{addr:#x}", "n": n, "ret": f"{self.u32(ctx.Esp):#x}",
                           "seed": [self.u32(p + 0x20), self.u32(p + 0x24)]}
                    self.emit(self.core.stamped(rec))
                return
            super().handle(addr, ctx)

    return Recorder


class FakeMem:
    def __init__(self):
        self.mem, self.armed, self.out, self.frame = {}, set(), [], 7

    def put(self, addr, data):
        for i, c in enumerate(data):
            self.mem[addr + i] = c

    def put32(self, addr, *vals):
        self.put(addr, struct.pack(f"<{len(vals)}I", *vals))

    def read(self, addr, n):
        return bytes(self.mem.get(addr + i, 0) for i in range(n))

    def u32(self, addr):
        return struct.unpack("<I", self.read(addr, 4))[0]

    def arm(self, addr):
        self.armed.add(addr)

    def emit(self, rec):
        self.out.append(json.loads(json.dumps(rec)))

    def stamp(self):
        return {"f": self.frame, "T": 3, "C": 9}


def selftest():
    """The hook chain on fake memory: IAT -> DirectSoundCreate -> CreateSoundBuffer
    -> Lock / Unlock / Play / SetVolume, and the PE entry reader."""
    m = FakeMem()
    core = AudioCore(m, BlobStore(None))
    DSC, DS, DSVT, CSB, BUF, BVT = 0x7B000000, 0x100000, 0x7B100000, 0x7B000100, 0x200000, 0x7B200000
    m.put32(IAT_DSOUND_CREATE, DSC)
    core.entry()
    assert DSC in m.armed
    m.put32(0x5000, 0x401234, 0, 0x6000)               # DirectSoundCreate(NULL, &ds, NULL)
    assert core.handle(DSC, 1, 0x5000, {})
    m.put32(0x6000, DS)
    m.put32(DS, DSVT)
    m.put32(DSVT + 4 * DS_CREATE_BUFFER, CSB)
    m.put32(DSVT + 4 * DS_DUPLICATE, CSB + 0x40)
    assert core.handle(0x401234, 1, 0x5004, {"eax": 0})
    assert CSB in m.armed and CSB + 0x40 in m.armed
    # CreateSoundBuffer(this, &desc, &buf, NULL): 22,050 Hz 16-bit mono, 8 bytes
    m.put32(0x7000, 0x401300, DS, 0x8000, 0x9000)
    m.put32(0x8000, 20, 0xE0, 8, 0, 0xA000)
    m.put(0xA000, struct.pack("<HHIIHH", 1, 1, 22050, 44100, 2, 16))
    assert core.handle(CSB, 2, 0x7000, {})
    m.put32(0x9000, BUF)
    m.put32(BUF, BVT)
    for slot in BUF_METHODS:
        m.put32(BVT + 4 * slot, 0x7B300000 + slot * 0x10)
    assert core.handle(0x401300, 2, 0x7004, {"eax": 0})
    c = m.out[-1]
    assert c["k"] == "create" and c["b"] == 1 and c["bytes"] == 8 and c["fmt"]["rate"] == 22050, c
    lock, unlock, play, vol = (0x7B300000 + s * 0x10 for s in (11, 19, 12, 15))
    m.put32(0xB000, 0x401400, BUF, 0, 8, 0xB100, 0xB104, 0, 0)  # Lock(buf, 0, 8, ...)
    assert core.handle(lock, 2, 0xB000, {})
    samples = struct.pack("<4h", 1, -2, 300, -32768)
    m.put(0xC000, samples)
    m.put32(0xB200, 0x401500, BUF, 0xC000, 8, 0, 0)               # Unlock(buf, p1, 8, 0, 0)
    assert core.handle(unlock, 2, 0xB200, {})
    w = m.out[-1]
    assert w == {"k": "write", "b": 1, "off": 0, "n1": 8, "n2": 0, "entire": False, "len": 8,
                 "sha256": hashlib.sha256(samples).hexdigest(), "tid": 2, "f": 7, "T": 3, "C": 9}, w
    # two threads with locks open on one buffer: each Unlock pairs with its own thread's Lock
    m.put32(0xB000, 0x401400, BUF, 64, 8, 0xB100, 0xB104, 0, 0)
    assert core.handle(lock, 3, 0xB000, {})
    m.put32(0xB000, 0x401400, BUF, 32, 8, 0xB100, 0xB104, 0, 0)
    assert core.handle(lock, 4, 0xB000, {})
    m.put32(0xB200, 0x401500, BUF, 0xC000, 8, 0, 0)
    assert core.handle(unlock, 3, 0xB200, {})
    assert m.out[-1]["off"] == 64 and m.out[-1]["tid"] == 3, m.out[-1]
    assert core.handle(unlock, 4, 0xB200, {})
    assert m.out[-1]["off"] == 32, m.out[-1]
    m.put32(0xB300, 0x401600, BUF, 0, 0, 1)                       # Play(buf, 0, 0, LOOPING)
    assert core.handle(play, 2, 0xB300, {})
    assert m.out[-1]["k"] == "play" and m.out[-1]["flags"] == 1
    m.put32(0xB400, 0x401700, BUF, 0xFFFFF830)                    # SetVolume(buf, -2000)
    assert core.handle(vol, 2, 0xB400, {})
    assert m.out[-1]["v"] == -2000
    assert not core.handle(0x12345, 2, 0xB400, {})
    exe = bytearray(0x400)
    struct.pack_into("<I", exe, 0x3C, 0x80)
    struct.pack_into("<H", exe, 0x86, 1)
    struct.pack_into("<H", exe, 0x94, 0xE0)
    struct.pack_into("<I", exe, 0xA8, 0x1010)
    struct.pack_into("<IIII", exe, 0x80 + 24 + 0xE0 + 8, 0x100, 0x1000, 0x100, 0x200)
    exe[0x210:0x213] = b"\x55\x8B\xEC"
    assert entry_point(bytes(exe)) == (0x401010, b"\x55\x8B\xEC")
    t = trampoline_bytes(0x1000, b"\x55\x8B\xEC" + b"\x90" * 13, 0x2000)
    assert t == b"\x55\xE9" + struct.pack("<i", 0x1001 - 0x2006), t
    t = trampoline_bytes(0x1000, b"\x8B\xFF\x55" + b"\x90" * 13, 0x2000)   # mov edi, edi
    assert t[:2] == b"\x8B\xFF" and t[2] == 0xE9
    assert trampoline_bytes(0x1000, b"\xEB\x05" + b"\x90" * 14, 0x2000) is None   # jmp short
    print("record_audio selftest: OK")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(REPO, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0)
    ap.add_argument("--ticks", type=int, default=0, help="stop after N server ticks (default: no limit)")
    ap.add_argument("--out", default=None, help="default traces/raw/<time>-audio.jsonl")
    ap.add_argument("--blob-dir", default=None, help="default game/captures/audio-<time>")
    ap.add_argument("--no-blobs", action="store_true", help="digests only")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("game_args", nargs="*", default=["-w"], help="Game.exe arguments (default -w: sound on)")
    autostart.add_options(ap)
    poke.add_options(ap)
    send.add_options(ap)
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    gargs, auto = autostart.setup(a, a.game_args or ["-w"])
    if "-ns" in gargs:
        raise SystemExit("-ns turns the game's sound off; nothing would be recorded")
    import record_tick as rt  # noqa: E402  (Windows only)
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = a.out or os.path.join(REPO, "traces", "raw", stamp + "-audio.jsonl")
    blob_dir = None if a.no_blobs else (a.blob_dir or os.path.join(REPO, "game", "captures", "audio-" + stamp))
    exe = os.path.abspath(a.game)
    ep, ep_bytes = entry_point(open(exe, "rb").read())
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], SOUND_REQUEST: SOUND_REQUEST_BYTES,
                SOUND_ROLL: SOUND_ROLL_BYTES, ROLL_GENERIC: ROLL_GENERIC_BYTES,
                ROLL_RANGE: ROLL_RANGE_BYTES, ep: ep_bytes}
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = make_recorder(rt)(exe, gargs, out, a.seconds, a.ticks, blob_dir)
    r.entry_addr = ep
    r.auto = auto
    if auto and auto.has_frames():
        auto.attach(r)
    layer = poke.PokeLayer.from_args(a)
    if layer:
        layer.attach(r)
    sends = send.SendLayer.from_args(a)
    if sends:
        sends.attach(r)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    send.print_results(sends)
    print(f"wrote {out}: {r.ticks} ticks, dsound calls {r.core.counts}; blobs {blob_dir}")


if __name__ == "__main__":
    main()
