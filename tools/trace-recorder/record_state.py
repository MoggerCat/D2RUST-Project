"""Record game-state snapshots of the original 1.14d Game.exe: after every
server tick (or every N-th), the game seed and every server unit with the
fields of specs/tools/state-snapshot.md §2, in the `state-1` format that
`state_diff.py` compares with the d2rs export (`d2-client state-dump`).

Reuses the debugger of record_tick.py (TickRecorder: process control,
breakpoints); only its tick entry hook stays armed (to learn the game,
as record_stats.py does), plus the tick driver's return `0x0052FD1E`
(ESI = game), the snapshot point of `specs/tools/original-hooks.md` §3
rule 3. Every offset read is owned by the spec named next to its
constant (gathered in `specs/tools/original-hooks.md` §4).

Output: one JSON line per record in traces/raw/<time>-state.jsonl
(format `state-1`, specs/tools/state-snapshot.md §1). `--selftest` runs
the snapshot reader on a synthetic process image (no game, any OS).

Pokes (`poke.py`, specs/tools/poke.md §2 rule 6): `--poke "<f> <directive>
<args>..."` (repeatable) runs at the 0x0052FD1E stop whose game +0xA8 =
f - 1, after that stop's snapshot, so the snapshot of frame f is the first
with its effect; each result is a `{"k":"poke","f",...}` line between the
two snapshots (state_diff.py skips the kind). `--poke-file` as poke.py.

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
import autostart  # noqa: E402  (unattended start, input script; imports on any OS)
import poke  # noqa: E402  (--poke / --poke-file: state injection, specs/tools/poke.md §2 rule 6)

TOOL = "trace-recorder record_state 0.1.0"
FORMAT = "state-1"
SIDE = "orig"

# snapshot point (original-hooks.md §3 rule 3); bytes as record_packets.py `tick_end`
TICK_END = 0x52FD1E
TICK_END_BYTES = b"\x8B\x76\x18"

# game record
G_FRAME = 0xA8            # tick.md §2 (frame, +1 at the start of every tick)
G_SEED = 0xD0             # rng.md §5.2: pGameSeed, u32 x 2 inline
HASH_BASE = 0x1120        # unit-order.md §2 rule 4: 5 tables x 128 buckets
HASH_OFFSETS = ((0, 0x000), (1, 0x200), (2, 0x400), (4, 0x600), (3, 0x800))  # type, offset
TILE_LIST = 0x1B20        # unit-order.md §2 rule 4: tiles (type 5), same link
BUCKETS = 128
# unit record (units.md §1-§2, §4; unit-order.md §1; rng.md §5.3; stat-lists.md §1)
U_TYPE, U_CLASS, U_GUID, U_MODE, U_ACT, U_SEED, U_PATH = 0x00, 0x04, 0x0C, 0x10, 0x18, 0x20, 0x2C
U_CUR, U_FC, U_SPEED, U_LIST, U_HASH_NEXT = 0x44, 0x48, 0x4C, 0x5C, 0xE4
UNIT_BYTES = 0xE8         # one read covers every field above
# paths (path-placement.md §2.1-§2.3)
DYNAMIC, STATIC = (0, 1, 3), (2, 4, 5)
DP_XF, DP_X, DP_YF, DP_Y, DP_TX, DP_TY, DP_ROOM, DP_DIR = 0x00, 0x02, 0x04, 0x06, 0x10, 0x12, 0x1C, 0x64
DP_BYTES = 0x68
SP_ROOM, SP_X, SP_Y, SP_DIR = 0x00, 0x0C, 0x10, 0x1C
SP_BYTES = 0x20
# level id: active room +0x10 -> DRLG room +0x58 -> level +0x1D0 (drlg/rooms.md §1, drlg/levels.md)
R_DRLG, DR_LEVEL, L_ID = 0x10, 0x58, 0x1D0
# stat list (stat-lists.md §1, §2)
SL_FLAGS, SL_BASE, SL_FULL = 0x10, 0x24, 0x48
SL_PLAIN_BYTES = 0x3C     # plain list size; extended lists are 0x64
EXTENDED = 0x80000000     # stat-lists.md §2: flag bit of the 0x64-byte form
MAX_STATS = 0x2000        # sanity bound on an array count (i16) before reading it
WALK_LIMIT = 100000       # per list: a longer chain is a broken link

FULL_STATS = (("hp", 6), ("hpx", 7), ("mp", 8), ("mpx", 9), ("st", 10), ("stx", 11))
BASE_STATS = (("str", 0), ("ene", 1), ("dex", 2), ("vit", 3), ("lvl", 12))
# state-snapshot.md §2 table order, `own` excluded (no 1.14d source)
FIELDS = ["ut", "g", "cl", "m", "x", "y", "xf", "yf", "tx", "ty", "d", "fr", "fc", "sp", "s",
          "act", "lv", "hp", "hpx", "mp", "mpx", "st", "stx", "str", "ene", "dex", "vit", "lvl"]
GAPS = ["own: no 1.14d address in a spec (pc1-data.md Step 4 item 21)"]


# --- the snapshot reader (pure: takes read(addr, n) -> bytes) ----------------

class StateReader:
    """Reads one snapshot through `read(addr, n)`. `src` (when tracking) maps
    (unit address, key) -> the source addresses of that key's value, for the
    selftest's perturbation check."""

    def __init__(self, read, track=False):
        self.read = read
        self.track = track
        self.src = {}
        self.notes = {}     # note text -> count (footer)
        self.levels = {}    # active room -> level id or None (per snapshot cache)

    def note(self, text):
        self.notes[text] = self.notes.get(text, 0) + 1

    def u32(self, a):
        return struct.unpack("<I", self.read(a, 4))[0]

    def put(self, rec, ua, key, value, *addrs):
        rec[key] = value
        if self.track:
            self.src[(ua, key)] = [a for a0, n in addrs for a in range(a0, a0 + n)]

    def level_of(self, room):
        """Level id of an active room, None when a link is 0 or unreadable; plus its sources."""
        if room in self.levels:
            return self.levels[room]
        res = None
        try:
            drlg = self.u32(room + R_DRLG)
            level = self.u32(drlg + DR_LEVEL) if drlg else 0
            if level:
                res = (self.u32(level + L_ID), [(room + R_DRLG, 4), (drlg + DR_LEVEL, 4),
                                                (level + L_ID, 4)])
            else:
                self.note("lv absent: active room without DRLG room or level")
        except OSError:
            self.note("lv absent: unreadable room / DRLG room / level")
        self.levels[room] = res
        return res

    def stats(self, ptr, count):
        """{(stat, layer): i32} of an 8-byte-entry array (u16 layer, u16 stat, i32 value)."""
        if not ptr or count <= 0:
            return {}, {}
        raw = self.read(ptr, 8 * count)
        vals, where = {}, {}
        for i, (layer, stat, v) in enumerate(struct.iter_unpack("<HHi", raw)):
            vals[(stat, layer)] = v
            where[(stat, layer)] = ptr + 8 * i + 4
        return vals, where

    def unit(self, ua):
        u = self.read(ua, UNIT_BYTES)
        g32 = lambda o: struct.unpack_from("<I", u, o)[0]  # noqa: E731
        ut = g32(U_TYPE)
        rec = {}
        self.put(rec, ua, "ut", ut, (ua + U_TYPE, 4))
        self.put(rec, ua, "g", g32(U_GUID), (ua + U_GUID, 4))
        self.put(rec, ua, "cl", g32(U_CLASS), (ua + U_CLASS, 4))
        self.put(rec, ua, "m", g32(U_MODE), (ua + U_MODE, 4))
        path = g32(U_PATH)
        room = 0
        if path and ut in DYNAMIC:
            p = self.read(path, DP_BYTES)
            h = lambda o: struct.unpack_from("<H", p, o)[0]  # noqa: E731
            self.put(rec, ua, "x", h(DP_X), (path + DP_X, 2))
            self.put(rec, ua, "y", h(DP_Y), (path + DP_Y, 2))
            self.put(rec, ua, "xf", h(DP_XF), (path + DP_XF, 2))
            self.put(rec, ua, "yf", h(DP_YF), (path + DP_YF, 2))
            self.put(rec, ua, "tx", h(DP_TX), (path + DP_TX, 2))
            self.put(rec, ua, "ty", h(DP_TY), (path + DP_TY, 2))
            self.put(rec, ua, "d", p[DP_DIR], (path + DP_DIR, 1))
            room = struct.unpack_from("<I", p, DP_ROOM)[0]
        elif path and ut in STATIC:
            p = self.read(path, SP_BYTES)
            self.put(rec, ua, "x", struct.unpack_from("<I", p, SP_X)[0], (path + SP_X, 4))
            self.put(rec, ua, "y", struct.unpack_from("<I", p, SP_Y)[0], (path + SP_Y, 4))
            self.put(rec, ua, "d", p[SP_DIR], (path + SP_DIR, 1))
            room = struct.unpack_from("<I", p, SP_ROOM)[0]
        elif path:
            self.note("position absent: unit type outside 0-5")
        self.put(rec, ua, "fr", struct.unpack_from("<i", u, U_CUR)[0], (ua + U_CUR, 4))
        self.put(rec, ua, "fc", struct.unpack_from("<i", u, U_FC)[0], (ua + U_FC, 4))
        self.put(rec, ua, "sp", struct.unpack_from("<h", u, U_SPEED)[0], (ua + U_SPEED, 2))
        self.put(rec, ua, "s", list(struct.unpack_from("<II", u, U_SEED)), (ua + U_SEED, 8))
        self.put(rec, ua, "act", u[U_ACT], (ua + U_ACT, 1))
        if room:
            lv = self.level_of(room)
            if lv is not None:
                self.put(rec, ua, "lv", lv[0], *lv[1])
        sl = g32(U_LIST)
        if sl:
            h = self.read(sl, SL_PLAIN_BYTES)
            flags = struct.unpack_from("<I", h, SL_FLAGS)[0]
            bptr, bcount = struct.unpack_from("<Ih", h, SL_BASE)
            if bcount > MAX_STATS:
                self.note("base stats absent: base array count out of range")
            else:
                vals, where = self.stats(bptr, bcount)
                for key, stat in BASE_STATS:
                    self.put(rec, ua, key, vals.get((stat, 0), 0), *([(where[(stat, 0)], 4)]
                                                                      if (stat, 0) in where else []))
            # the full array exists only on an extended list (stat-lists.md §1, flag §2)
            if flags & EXTENDED:
                fptr, fcount = struct.unpack("<Ih", self.read(sl + SL_FULL, 6))
                if fcount > MAX_STATS:
                    self.note("hp..stx absent: full array count out of range")
                else:
                    vals, where = self.stats(fptr, fcount)
                    for key, stat in FULL_STATS:
                        self.put(rec, ua, key, vals.get((stat, 0), 0), *([(where[(stat, 0)], 4)]
                                                                          if (stat, 0) in where else []))
            else:
                self.note("hp..stx absent: unit +0x5C list is not extended")
        return rec

    def units_of(self, game):
        """[(type of the list, unit address)] in walk order: 5 hash tables, then tiles."""
        out = []
        table = self.read(game + HASH_BASE, TILE_LIST - HASH_BASE + 4)
        heads = [(t, struct.unpack_from("<I", table, off + 4 * b)[0])
                 for t, off in HASH_OFFSETS for b in range(BUCKETS)]
        heads.append((5, struct.unpack_from("<I", table, TILE_LIST - HASH_BASE)[0]))
        for t, ua in heads:
            n = 0
            while ua:
                if n >= WALK_LIMIT:
                    self.note("list walk stopped at the limit (broken link?)")
                    break
                out.append((t, ua))
                ua = self.u32(ua + U_HASH_NEXT)
                n += 1
        return out

    def snapshot(self, game):
        """{"seed": [lo, hi], "units": [...] sorted by (ut, g)}, and the unit addresses."""
        self.levels = {}
        seed = list(struct.unpack("<II", self.read(game + G_SEED, 8)))
        recs = []
        for t, ua in self.units_of(game):
            rec = self.unit(ua)
            if rec["ut"] != t:
                self.note("unit type differs from its list's type")
            recs.append((ua, rec))
        recs.sort(key=lambda r: (r[1]["ut"], r[1]["g"]))
        return {"seed": seed, "units": [r for _, r in recs]}, [ua for ua, _ in recs]


# --- the recorder (Windows; built on record_tick.TickRecorder) ----------------

def make_recorder(rt):
    rr = rt.rr

    class StateRecorder(rt.TickRecorder):
        def __init__(self, exe, args, out_path, seconds, max_ticks, every, command):
            super().__init__(exe, args, out_path, seconds, 0, max_ticks)
            self.every, self.command = max(1, every), command
            self.snaps = 0
            self.reader_notes = {}

        def handle(self, addr, ctx):
            if addr == rt.TICK:
                if self.game is None:
                    self.game = ctx.Ecx  # the first game that ticks (state-snapshot.md §3 r3)
                return
            if addr != TICK_END or self.game is None or ctx.Esi != self.game:
                return
            n = self.u32(self.game + G_FRAME)
            self.frame = n
            self.ticks += 1
            if self.snaps == 0 or n % self.every == 0:
                rd = StateReader(self.read)
                snap, _ = rd.snapshot(self.game)
                self.emit({"k": "snap", "f": n, **snap})
                self.snaps += 1
                for k, v in rd.notes.items():
                    self.reader_notes[k] = self.reader_notes.get(k, 0) + v
            if self.max_ticks and self.ticks >= self.max_ticks:
                self.notes.append(f"tick limit {self.max_ticks} reached")
                self.done = True

        def run(self):
            """record_tick's run() with the state-1 header and footer."""
            self.sha = hashlib.sha256(open(self.exe, "rb").read()).hexdigest()
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
            self.out.write(json.dumps(header(self.command, self.sha, self.args, self.every),
                                      separators=(",", ":")) + "\n")
            try:
                self.loop(time.perf_counter() + self.seconds)
            finally:
                self.kill()
                rr.CloseHandle(pi.hThread)
                notes = list(self.notes) + [f"{k} ({v} units)" for k, v in self.reader_notes.items()]
                self.out.write(json.dumps({"k": "footer", "snaps": self.snaps, "notes": notes},
                                          separators=(",", ":")) + "\n")
                self.out.close()

    return StateRecorder


def header(command, sha, args, every):
    return {"k": "header", "format": FORMAT, "side": SIDE, "tool": TOOL,
            "date": datetime.date.today().isoformat(), "command": command, "fields": FIELDS,
            "gaps": GAPS, "game_exe_sha256": sha, "args": args, "snap_every": every}


# --- selftest -----------------------------------------------------------------

class FakeMem:
    """Sparse little-endian memory (unwritten bytes read 0)."""

    def __init__(self):
        self.b = {}

    def put(self, addr, fmt, *v):
        for i, x in enumerate(struct.pack(fmt, *v)):
            self.b[addr + i] = x

    def read(self, addr, n):
        return bytes(self.b.get(addr + i, 0) for i in range(n))


def build_world():
    """A game with one unit of every kind and every rule of §2 exercised."""
    m = FakeMem()
    game = 0x100000
    m.put(game + G_FRAME, "<I", 41)
    m.put(game + G_SEED, "<II", 0x89ABCDEF, 666)
    # rooms: R1 -> level 2 (Blood Moor), R2 -> level 1; R3 has a DRLG room without level
    rooms = {}
    for i, (lid, has_level) in enumerate(((2, True), (1, True), (0, False))):
        room, drlg, level = 0x200000 + 0x1000 * i, 0x210000 + 0x1000 * i, 0x220000 + 0x1000 * i
        m.put(room + R_DRLG, "<I", drlg)
        if has_level:
            m.put(drlg + DR_LEVEL, "<I", level)
            m.put(level + L_ID, "<I", lid)
        rooms[i] = room

    nxt = {}

    def unit(ua, ut, guid, cl, mode, act, seed, anim):
        m.put(ua + U_TYPE, "<I", ut)
        m.put(ua + U_CLASS, "<I", cl)
        m.put(ua + U_GUID, "<I", guid)
        m.put(ua + U_MODE, "<I", mode)
        m.put(ua + U_ACT, "<B", act)
        m.put(ua + U_SEED, "<II", *seed)
        m.put(ua + U_CUR, "<ii", anim[0], anim[1])
        m.put(ua + U_SPEED, "<h", anim[2])
        # bytes no field reads: must never show up (checked by the perturbation)
        m.put(ua + 0x24 + 4, "<I", 0x5A5A5A5A)

    def dyn(ua, path, xf, x, yf, y, tx, ty, room, d):
        m.put(ua + U_PATH, "<I", path)
        m.put(path + DP_XF, "<HHHH", xf, x, yf, y)
        m.put(path + DP_TX, "<HH", tx, ty)
        m.put(path + DP_ROOM, "<I", room)
        m.put(path + DP_DIR, "<B", d)
        m.put(path + 0x08, "<II", 0x1111, 0x2222)  # client x, y: not snapshot fields

    def stat(ua, sl, flags, base, full):
        m.put(ua + U_LIST, "<I", sl)
        m.put(sl + SL_FLAGS, "<I", flags)
        bptr, fptr = sl + 0x100, sl + 0x200
        m.put(sl + SL_BASE, "<IhH", bptr if base else 0, len(base), len(base))
        for i, (s, layer, v) in enumerate(sorted(base, key=lambda e: (e[0] << 16) | e[1])):
            m.put(bptr + 8 * i, "<HHi", layer, s, v)
        if flags & EXTENDED:
            m.put(sl + SL_FULL, "<IhH", fptr if full else 0, len(full), len(full))
            for i, (s, layer, v) in enumerate(sorted(full, key=lambda e: (e[0] << 16) | e[1])):
                m.put(fptr + 8 * i, "<HHi", layer, s, v)

    def link(head_addr, *units):
        m.put(head_addr, "<I", units[0])
        for a, b in zip(units, units[1:]):
            m.put(a + U_HASH_NEXT, "<I", b)
            nxt[a] = b

    hb = lambda t, b: game + HASH_BASE + dict(HASH_OFFSETS)[t] + 4 * b  # noqa: E731
    # player 1 (bucket 1): dynamic path in R2 (level 1), extended list with base and full arrays
    P = 0x300000
    unit(P, 0, 1, 1, 1, 0, (0x11111111, 0x22222222), (5 * 256, 8 * 256, 256))
    dyn(P, 0x310000, 0x8000, 5800, 0x4000, 5600, 5810, 5605, rooms[1], 33)
    stat(P, 0x320000, EXTENDED, [(0, 0, 20), (1, 0, 25), (2, 0, 20), (3, 0, 25), (12, 0, 1),
                                 (5, 0, 7), (0, 1, 99)],
         [(6, 0, 50 << 8), (7, 0, 50 << 8), (8, 0, 15 << 8), (9, 0, 15 << 8), (10, 0, 84 << 8),
          (11, 0, 84 << 8), (6, 1, 7), (12, 0, 1)])
    link(hb(0, 1), P)
    # monsters: bucket 3 holds GUID 9 then GUID 4 (walk order != sort order)
    M1, M2 = 0x330000, 0x340000
    unit(M1, 1, 9, 5, 2, 0, (3, 4), (0, 13 * 256, 0x100))
    dyn(M1, 0x331000, 0, 5900, 0xFFFF, 5700, 0, 0, rooms[0], 7)
    stat(M1, 0x332000, EXTENDED, [(12, 0, 2)], [(6, 0, 3 << 8)])  # no max life entry: 0
    unit(M2, 1, 4, 5, 12, 0, (5, 6), (-1, 0, -3))
    dyn(M2, 0x341000, 1, 5901, 2, 5701, 3, 4, rooms[2], 60)          # level absent (R3)
    link(hb(1, 3), M1, M2)                                           # M2: no stat list
    # object: static path in R1
    O = 0x350000
    unit(O, 2, 3, 2, 0, 0, (7, 8), (0, 0, 0))
    m.put(O + U_PATH, "<I", 0x351000)
    m.put(0x351000 + SP_ROOM, "<I", rooms[0])
    m.put(0x351000 + SP_X, "<II", 5850, 5650)
    m.put(0x351000 + SP_DIR, "<B", 2)
    stat(O, 0x352000, 0, [(3, 0, 1)], [])                            # plain list: no full stats
    link(hb(2, 127), O)
    # items: one on the ground (static path, no room: lv absent), one in an inventory (path 0)
    I1, I2 = 0x360000, 0x370000
    unit(I1, 4, 12, 25, 3, 0, (9, 10), (0, 0, 0))
    m.put(I1 + U_PATH, "<I", 0x361000)
    m.put(0x361000 + SP_X, "<II", 5801, 5601)
    m.put(0x361000 + SP_DIR, "<B", 0)
    stat(I1, 0x362000, EXTENDED, [], [])
    unit(I2, 4, 11, 26, 0, 0, (11, 12), (0, 0, 0))
    stat(I2, 0x372000, EXTENDED, [(0, 0, 3)], [(7, 0, 9)])
    link(hb(4, 0), I1, I2)
    # missile: dynamic path in R1
    X = 0x380000
    unit(X, 3, 2, 10, 0, 0, (13, 14), (2, 3, 128))
    dyn(X, 0x381000, 0x10, 5820, 0x20, 5620, 5900, 5700, rooms[0], 63)
    link(hb(3, 64), X)
    # tile list: one tile, static path in R2
    T = 0x390000
    unit(T, 5, 1, 0, 0, 0, (15, 16), (0, 0, 0))
    m.put(T + U_PATH, "<I", 0x391000)
    m.put(0x391000 + SP_ROOM, "<I", rooms[1])
    m.put(0x391000 + SP_X, "<II", 5750, 5550)
    m.put(0x391000 + SP_DIR, "<B", 0)
    link(game + TILE_LIST, T)
    return m, game


EXPECTED = [
    {"ut": 0, "g": 1, "cl": 1, "m": 1, "x": 5800, "y": 5600, "xf": 0x8000, "yf": 0x4000, "tx": 5810,
     "ty": 5605, "d": 33, "fr": 1280, "fc": 2048, "sp": 256, "s": [0x11111111, 0x22222222], "act": 0,
     "lv": 1, "str": 20, "ene": 25, "dex": 20, "vit": 25, "lvl": 1, "hp": 12800, "hpx": 12800,
     "mp": 3840, "mpx": 3840, "st": 21504, "stx": 21504},
    {"ut": 1, "g": 4, "cl": 5, "m": 12, "x": 5901, "y": 5701, "xf": 1, "yf": 2, "tx": 3, "ty": 4,
     "d": 60, "fr": -1, "fc": 0, "sp": -3, "s": [5, 6], "act": 0},
    {"ut": 1, "g": 9, "cl": 5, "m": 2, "x": 5900, "y": 5700, "xf": 0, "yf": 0xFFFF, "tx": 0, "ty": 0,
     "d": 7, "fr": 0, "fc": 3328, "sp": 256, "s": [3, 4], "act": 0, "lv": 2, "str": 0, "ene": 0,
     "dex": 0, "vit": 0, "lvl": 2, "hp": 768, "hpx": 0, "mp": 0, "mpx": 0, "st": 0, "stx": 0},
    {"ut": 2, "g": 3, "cl": 2, "m": 0, "x": 5850, "y": 5650, "d": 2, "fr": 0, "fc": 0, "sp": 0,
     "s": [7, 8], "act": 0, "lv": 2, "str": 0, "ene": 0, "dex": 0, "vit": 1, "lvl": 0},
    {"ut": 3, "g": 2, "cl": 10, "m": 0, "x": 5820, "y": 5620, "xf": 0x10, "yf": 0x20, "tx": 5900,
     "ty": 5700, "d": 63, "fr": 2, "fc": 3, "sp": 128, "s": [13, 14], "act": 0, "lv": 2},
    {"ut": 4, "g": 11, "cl": 26, "m": 0, "fr": 0, "fc": 0, "sp": 0, "s": [11, 12], "act": 0,
     "str": 3, "ene": 0, "dex": 0, "vit": 0, "lvl": 0, "hp": 0, "hpx": 9, "mp": 0, "mpx": 0, "st": 0,
     "stx": 0},
    {"ut": 4, "g": 12, "cl": 25, "m": 3, "x": 5801, "y": 5601, "d": 0, "fr": 0, "fc": 0, "sp": 0,
     "s": [9, 10], "act": 0, "str": 0, "ene": 0, "dex": 0, "vit": 0, "lvl": 0, "hp": 0, "hpx": 0,
     "mp": 0, "mpx": 0, "st": 0, "stx": 0},
    {"ut": 5, "g": 1, "cl": 0, "m": 0, "x": 5750, "y": 5550, "d": 0, "fr": 0, "fc": 0, "sp": 0,
     "s": [15, 16], "act": 0, "lv": 1},
]
POS_KEYS = ("x", "y", "xf", "yf", "tx", "ty", "d", "lv")
# flips that keep a unit type in its path class (dynamic 0, 1, 3; static 2, 4, 5)
TYPE_FLIP = {0: 1, 1: 0, 3: 1, 2: 4, 4: 2, 5: 4}


def selftest():
    m, game = build_world()
    rd = StateReader(m.read, track=True)
    snap, addrs = rd.snapshot(game)
    assert snap["seed"] == [0x89ABCDEF, 666], snap["seed"]
    for got, want in zip(snap["units"], EXPECTED):
        assert got == want, f"unit record\n got  {got}\n want {want}"
        assert set(got) <= set(FIELDS)
    assert len(snap["units"]) == len(EXPECTED), len(snap["units"])
    base = dict(zip(addrs, snap["units"]))
    # every source byte, changed alone, changes exactly the keys that read it
    by_addr = {}
    for (ua, key), src in rd.src.items():
        for a in src:
            by_addr.setdefault(a, set()).add((ua, key))
    for a in by_addr:
        assert a in m.b, f"source {a:#x} was never written"
    checked = 0
    for a, want in sorted(by_addr.items()):
        old = m.b[a]
        flip = old ^ 0x01
        tu = [ua for ua, key in want if key == "ut"]
        if tu and a == tu[0] + U_TYPE:  # low type byte: a flip within the same path class
            flip = TYPE_FLIP[old]
        elif tu:  # a high type byte: no unit type 0-5, so the position fields go too
            want = want | {(tu[0], k) for k in POS_KEYS if k in base[tu[0]]}
        m.b[a] = flip
        s2, a2 = StateReader(m.read).snapshot(game)
        m.b[a] = old
        now = dict(zip(a2, s2["units"]))
        diff = {(ua, k) for ua in base for k in set(base[ua]) | set(now[ua])
                if base[ua].get(k) != now[ua].get(k)}
        assert diff == want, f"byte {a:#x}: changed {sorted(diff)}, expected {sorted(want)}"
        assert s2["seed"] == snap["seed"]
        checked += 1
    for a in range(game + G_SEED, game + G_SEED + 8):
        old = m.b.get(a, 0)
        m.b[a] = old ^ 1
        s2, _ = StateReader(m.read).snapshot(game)
        m.b[a] = old
        assert s2["seed"] != snap["seed"] and s2["units"] == snap["units"], f"seed byte {a:#x}"
        checked += 1
    # bytes inside unit and path records that no field reads change nothing
    quiet = 0
    for ua in addrs:
        for a in [ua + 0x28] + [ua + o for o in range(0x60, U_HASH_NEXT)]:
            if a in by_addr:
                continue
            old = m.b.get(a, 0)
            m.b[a] = old ^ 0x40
            s2, _ = StateReader(m.read).snapshot(game)
            m.b[a] = old
            assert s2 == snap, f"unread byte {a:#x} changed the snapshot"
            quiet += 1
    # a non-extended unit list leaves hp..stx out (object) and says so
    assert "hp..stx absent: unit +0x5C list is not extended" in rd.notes
    print(f"selftest ok: {len(EXPECTED)} unit records exact; {checked} source bytes each change "
          f"exactly their keys; {quiet} unread bytes change nothing")


def selftest_poke_options(ap):
    """--poke on this recorder's command line becomes a poke layer (poke.py)."""
    a = ap.parse_args(["--poke", "4 spawn 19 @x+3 @y+3 normal", "--poke",
                       "4 seed-unit @1:19 0x12345678 666", "--ticks", "5"])
    layer = poke.PokeLayer.from_args(a)
    assert [(s.f, s.d) for s in layer.abs] == [(4, "spawn"), (4, "seed-unit")]
    assert poke.PokeLayer.from_args(ap.parse_args([])) is None
    print("selftest ok: --poke lines make a poke layer")


# --- main ---------------------------------------------------------------------

def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0, help="kill the game after N s (default 120)")
    ap.add_argument("--ticks", type=int, default=0, help="stop after N recorded ticks (default: no limit)")
    ap.add_argument("--snap-every", type=int, default=1,
                    help="snapshot the first recorded tick and every N-th frame (default 1)")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-state.jsonl)")
    ap.add_argument("--selftest", action="store_true", help="check the snapshot reader on a synthetic game, exit")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"], help="Game.exe arguments (default: -w -ns)")
    autostart.add_options(ap)
    poke.add_options(ap)
    a = ap.parse_args()
    if a.selftest:
        selftest()
        selftest_poke_options(ap)
        return
    layer = poke.PokeLayer.from_args(a)
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    import record_tick as rt  # noqa: E402  (the shared tick recorder; Windows only; not modified)
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-state.jsonl")
    # keep only the base recorder's tick entry hook, add the tick return
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], TICK_END: TICK_END_BYTES}
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = make_recorder(rt)(os.path.abspath(a.game), gargs, out, a.seconds, a.ticks,
                          a.snap_every, " ".join(sys.argv))
    r.auto = auto
    if auto and auto.has_frames():
        auto.attach(r)  # `frame F` input steps at the tick-return stop of F - 1 (after the snapshot)
    if layer:
        layer.attach(r, before=False)  # snapshot of frame f - 1 first, then the pokes of f
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    if layer:
        for x in layer.results:
            print(f"poke: f {x['f']} {x['d']}: {x['r']}"
                  + (f" guid {x['guid']}" if "guid" in x else "")
                  + (f" ({x['note']})" if x.get("note") else ""))
        if layer.pending():
            print(f"note: {len(layer.pending())} poke(s) not reached")
    for k, v in r.reader_notes.items():
        print(f"note: {k} ({v} units)")
    print(f"wrote {out}: {r.snaps} snapshots, {r.ticks} ticks")


if __name__ == "__main__":
    main()
