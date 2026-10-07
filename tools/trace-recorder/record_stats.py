"""Record the stat lists of the original 1.14d Game.exe: every base write,
attach, detach, free, park/unpark, dynamic toggle, by-time refresh, state
toggle, expiry and value-change callback on the server's lists, the
regeneration entry points, and periodic snapshots of the players' and
monsters' list trees.

Reuses the debugger of record_tick.py (TickRecorder: process control,
breakpoints, tick and step markers); this script only adds hooks. Only the
tick and step hooks of the base recorder stay armed. Every hooked address
and offset is documented in specs/sim/stat-lists.md and specs/sim/stats.md
(each constant below names its section). Output: one JSON line per record
in traces/raw/<time>-stats.jsonl (format stats-raw-1, README.md); checked
by check_stats.py.

Only server lists are recorded: a list is tracked once it belongs to the
tree of a server unit's list (unit +0xC8 bit 0x04000000). The first time a
tree is touched it is dumped whole (`ssd`), before the operation runs.

The game process is always terminated when this script ends (time limit,
Ctrl+C, any error, and kill-on-exit if the debugger dies).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import os
import struct
import sys

sys.dont_write_bytecode = True
import record_tick as rt  # noqa: E402  (the shared tick recorder; not modified)
import autostart  # noqa: E402  (unattended start, input script)

TOOL = "trace-recorder record_stats 0.1.0"
FORMAT = "stats-raw-1"

# data tables (stats.md §3; loading.md): pointer block at [0x744304]
DT_PTR = 0x744304
DT_ISC, DT_ISC_N = 0xBCC, 0xBD4          # itemstatcost records (0x144 bytes), count
DT_CS, DT_CS_N = 0xBC4, 0xBC8            # charstats records (0xC4 bytes), count
DT_STATES_N = 0xC4                       # states count
DT_STATE_FLAGSETS = 0xCC                 # 40 bitset pointers (runtime-maps.md §4)
ISC_SIZE, CS_SIZE = 0x144, 0xC4
# unit (unit-order.md §1; stat-lists.md §1)
U_MODE, U_ACT, U_LIST = 0x10, 0x1C, 0x5C
# stat list (stat-lists.md §1)
SL_POOL, SL_UNIT, SL_OTYPE, SL_OGUID, SL_FLAGS, SL_STATE, SL_EXPIRE = 0, 4, 8, 0xC, 0x10, 0x14, 0x18
SL_BASE, SL_PREV, SL_NEXT, SL_PARENT = 0x24, 0x2C, 0x30, 0x34
SL_LAST, SL_SETL, SL_OWNER, SL_FULL, SL_MOD, SL_SBITS, SL_VCB = 0x3C, 0x40, 0x44, 0x48, 0x50, 0x58, 0x5C
EXT = 0x80000000
G_POOL = 0x1C

# hooks (stat-lists.md §4-§10): address -> name
H_ADD = 0x627030       # entry: list, stat, value, layer (§5.3)
H_SET = 0x6270B0       # entry: list, stat, value, layer (§5.1)
H_SETB = 0x627170      # entry: list, stat, value, layer, unit (§5.1)
H_RMALL = 0x627340     # entry: list (§5.4)
H_DETACH = 0x6269F0    # entry: list (§8.2)
H_ATTACH = 0x626E40    # after attach's own detach: EBX unit, EDI list, ESI unit list, [ebp+0x10] reset (§8.1)
H_FREE = 0x626C0A      # after free's own detach: ESI list (§8.3)
H_ALLOCX = 0x626E00    # extended list stored: ESI list, EDI unit (§4.2)
H_STATIC = 0x627860    # entry: unit, item (§8.6)
H_DYNAMIC = 0x627A40   # entry: unit, item (§8.6)
H_BYTIME = 0x6276C0    # entry: unit, item (§8.7)
H_TOGGLE = 0x625A70    # entry: unit, state, on (§9.2)
H_EXPIRE = 0x627460    # entry: unit, frame (§10.4)
H_XFREE = 0x6274A4     # the expiry's free call: ECX list (§10.4)
H_XEND = (0x58080D, 0x5A7EFD, 0x55F13D)  # return of the player, monster, item event-12 handlers (§10.4)
H_CB = (0x62513E, 0x6251E4)         # `call eax` of the value-change callback (§7)
H_CBX = (0x625140, 0x6251E6)        # after it
H_REGEN_LIFE = 0x580610   # player life: ESI unit (stat-lists.md §10.1)
H_REGEN_STAM = 0x580500   # player stamina: EDI unit
H_REGEN_MANA = 0x5806F0   # player mana: EDI unit
H_REGEN_MON = 0x5A6920    # monster: EDX unit
ROOMS_STEP = 0x52D8EB     # tick.md §3 step 3 call site: environment already advanced

STATS_EXPECT = {
    H_ADD: b"\x55\x8B\xEC\x53", H_SET: b"\x55\x8B\xEC\x53", H_SETB: b"\x55\x8B\xEC\x53",
    H_RMALL: b"\x55\x8B\xEC\x51", H_DETACH: b"\x55\x8B\xEC\x83", H_ATTACH: b"\x8B\xC6\x3B\xF8",
    H_FREE: b"\x8B\x56\x24", H_ALLOCX: b"\x89\x77\x5C", H_STATIC: b"\x55\x8B\xEC\x8B\x4D",
    H_DYNAMIC: b"\x55\x8B\xEC\x8B\x4D", H_BYTIME: b"\x55\x8B\xEC\x83", H_TOGGLE: b"\x55\x8B\xEC\x8B\x45",
    H_EXPIRE: b"\x55\x8B\xEC\x8B\x45", H_XFREE: b"\xE8\x57\xF7\xFF\xFF",
    H_XEND[0]: b"\xC2\x08\x00", H_XEND[1]: b"\xC2\x08\x00", H_XEND[2]: b"\xC3",
    H_CB[0]: b"\xFF\xD0", H_CB[1]: b"\xFF\xD0", H_CBX[0]: b"\x8B\xC7", H_CBX[1]: b"\x5F\x5E",
    H_REGEN_LIFE: b"\x53\x6A\x00", H_REGEN_STAM: b"\x55\x8B\xEC\x83", H_REGEN_MANA: b"\x55\x8B\xEC\x83",
    H_REGEN_MON: b"\x55\x8B\xEC\x83",
}


def s32(x):
    return x - (1 << 32) if x & 0x80000000 else x


class StatsRecorder(rt.TickRecorder):
    def __init__(self, exe, args, out_path, seconds, max_ticks, snap_every, snap_monsters):
        super().__init__(exe, args, out_path, seconds, 0, max_ticks)
        self.stat_snap_every, self.snap_monsters = snap_every, snap_monsters
        self.tracked = set()
        self.tables_done = False
        self.nstates = 0
        self.cb_stack = {}   # tid -> [bool emitted]
        self.in_expire = 0

    # --- reading ---------------------------------------------------------------
    def server_unit(self, u):
        return self.unit_id(u) if u else None

    def uid(self, u):
        i = self.server_unit(u)
        return f"{i[0]}:{i[1]}" if i else None

    def uinfo(self, u):
        i = self.server_unit(u)
        if not i:
            return None
        act = self.u32(u + U_ACT)
        return {"t": i[0], "c": i[2], "act": f"{act:#x}" if act else None}

    def arr(self, ptr, count):
        if not ptr or count <= 0:
            return []
        raw = self.read(ptr, 8 * count)
        return [[st, ly, v] for ly, st, v in struct.iter_unpack("<HHi", raw)]

    def dump(self, a, units):
        h = self.read(a, 0x3C)  # stat-lists.md §1: 0x3C bytes, 0x64 when extended
        fl = struct.unpack_from("<I", h, SL_FLAGS)[0]
        ext = bool(fl & EXT)
        h = h + (self.read(a + 0x3C, 0x28) if ext else bytes(0x28))
        g = lambda o: struct.unpack_from("<I", h, o)[0]
        hx = lambda v: f"{v:#x}" if v else None
        unit = g(SL_UNIT)
        d = {"L": f"{a:#x}", "ext": ext, "fl": fl, "st": g(SL_STATE), "ex": s32(g(SL_EXPIRE)),
             "ot": g(SL_OTYPE), "og": g(SL_OGUID), "u": self.uid(unit), "par": hx(g(SL_PARENT)),
             "prev": hx(g(SL_PREV)), "next": hx(g(SL_NEXT)),
             "b": self.arr(g(SL_BASE), struct.unpack_from("<h", h, SL_BASE + 4)[0])}
        if unit and d["u"]:
            units[d["u"]] = self.uinfo(unit)
        if ext:
            owner = g(SL_OWNER)
            d.update({"last": hx(g(SL_LAST)), "setl": hx(g(SL_SETL)), "ow": self.uid(owner),
                      "F": self.arr(g(SL_FULL), struct.unpack_from("<h", h, SL_FULL + 4)[0]),
                      "cb": bool(g(SL_VCB))})
            mc = struct.unpack_from("<h", h, SL_MOD + 4)[0]
            d["m"] = list(struct.unpack(f"<{mc}I", self.read(g(SL_MOD), 4 * mc))) \
                if mc > 0 and g(SL_MOD) else []
            w = (self.nstates + 31) // 32
            if g(SL_SBITS) and w:
                words = struct.unpack(f"<{w}I", self.read(g(SL_SBITS), 4 * w))
                d["sb"] = [32 * i + b for i, x in enumerate(words) for b in range(32) if x >> b & 1]
            if owner and d["ow"]:
                units[d["ow"]] = self.uinfo(owner)
        return d

    def tree(self, a, out, depth=0):
        """The list and everything attached below it (stat-lists.md §8)."""
        if not a or a in out or depth > 8:
            return
        out.append(a)
        if self.u32(a + SL_FLAGS) & EXT:
            for head in (self.u32(a + SL_LAST), self.u32(a + SL_SETL)):
                c, n = head, 0
                while c and n < 4096:
                    self.tree(c, out, depth + 1)
                    c, n = self.u32(c + SL_PREV), n + 1

    def root_of(self, a):
        n = 0
        while a and n < 64:
            p = self.u32(a + SL_PARENT)
            if not p:
                return a
            a, n = p, n + 1
        return a

    def server_root(self, r):
        return r and self.u32(r + SL_FLAGS) & EXT and self.server_unit(self.u32(r + SL_OWNER))

    def seed(self, top):
        lists, units = [], {}
        out = []
        self.tree(top, out)
        for a in out:
            lists.append(self.dump(a, units))
            self.tracked.add(a)
        self.emit({"k": "ssd", "lists": lists, "units": units})

    def ensure(self, a):
        """Track a list if it belongs to a server unit's tree (seeding the tree first)."""
        if not a:
            return False
        if a in self.tracked:
            return True
        r = self.root_of(a)
        if not self.server_root(r):
            return False
        if r in self.tracked:  # a list below a tracked root that was never seen: seed it alone
            self.seed(a)
        else:
            self.seed(r)
        return a in self.tracked

    def tables(self):
        dt = self.u32(DT_PTR)
        isc, n = self.u32(dt + DT_ISC), self.u32(dt + DT_ISC_N)
        raw = self.read(isc, ISC_SIZE * n)
        rows = []
        for i in range(n):
            r = raw[i * ISC_SIZE:(i + 1) * ISC_SIZE]
            ot = [list(struct.unpack_from("<HHBB", r, 0xDE + 6 * m)) for m in range(16)]
            ot = [e for e in ot if e[2]]
            deps = []
            for m in range(64):
                v = struct.unpack_from("<H", r, 0x5E + 2 * m)[0]
                if v == 0xFFFF:
                    break
                deps.append(v)
            rows.append([struct.unpack_from("<I", r, 4)[0], r[0x18], s32(struct.unpack_from("<I", r, 0x2C)[0]),
                         r[0x50], r[0x54], r[0x55], struct.unpack_from("<H", r, 0x56)[0],
                         list(struct.unpack_from("<3H", r, 0x58)), r[0x51], r[0x52], r[0x53], ot, deps])
        cs, cn = self.u32(dt + DT_CS), self.u32(dt + DT_CS_N)
        csd = {}
        for c in range(cn):
            r = self.read(cs + CS_SIZE * c, CS_SIZE)
            csd[str(c)] = [r[0x3A], r[0x46], r[0x47], r[0x48]]
        self.nstates = self.u32(dt + DT_STATES_N)
        w = (self.nstates + 31) // 32
        bs = self.u32(dt + DT_STATE_FLAGSETS + 4 * 32)  # flag bit 32 `life`
        words = struct.unpack(f"<{w}I", self.read(bs, 4 * w)) if bs and w else []
        life = [32 * i + b for i, x in enumerate(words) for b in range(32) if x >> b & 1]
        self.emit({"k": "stab", "isc": rows, "cs": csd, "life_states": life, "nstates": self.nstates})

    def env(self):
        out = {}
        for act in self.acts():
            if not act:
                continue
            e = self.u32(act + 4)
            if not e:
                continue
            div, t = self.i32(e + 0x28), self.i32(e + 8)  # stats.md §8: time / period length
            out[f"{act:#x}"] = (abs(t) // abs(div)) * (1 if (t >= 0) == (div > 0) else -1) if div else 0
        self.emit({"k": "senv", "f": self.frame, "env": out})

    def snapshot_stats(self):
        g = self.game
        lists, units, seen = [], {}, set()
        roots = []
        for t, cap in ((0, 64), (1, self.snap_monsters)):
            n = 0
            for b in range(128):
                u = self.u32(g + rt.HASH_BASE + rt.HASH_OFFSETS[t] + 4 * b)
                while u and n < cap:
                    sl = self.u32(u + U_LIST)
                    if sl and self.server_unit(u) and self.u32(sl + SL_FLAGS) & EXT:
                        roots.append(sl)
                        n += 1
                    u = self.u32(u + rt.U_HASH_NEXT)
        for r in roots:
            if r not in self.tracked:
                self.ensure(r)
                continue  # just seeded
            out = []
            self.tree(r, out)
            for a in out:
                if a not in seen:
                    seen.add(a)
                    lists.append(self.dump(a, units))
                    self.tracked.add(a)
        self.emit({"k": "ssn", "f": self.frame, "lists": lists, "units": units})

    # --- breakpoints -----------------------------------------------------------
    def args(self, ctx, n):
        return struct.unpack(f"<{n}I", self.read(ctx.Esp + 4, 4 * n))

    def handle(self, addr, ctx):
        if addr == rt.TICK or addr in rt.STEPS:
            super().handle(addr, ctx)
            if self.game is None or ctx.Ecx != self.game and addr == rt.TICK:
                return
            if addr == rt.TICK:
                if not self.tables_done:
                    self.tables()
                    self.tables_done = True
                if self.stat_snap_every and self.frame % self.stat_snap_every == 0:
                    self.snapshot_stats()
            elif addr == ROOMS_STEP and ctx.Edi == self.game:
                self.env()
            return
        if not self.tables_done:
            return
        tid = self.pending[1] if self.pending else 0
        hx = lambda v: f"{v:#x}"
        if addr in (H_ADD, H_SET, H_SETB):
            n = 5 if addr == H_SETB else 4
            a = self.args(ctx, n)
            if not a[0] or (addr == H_ADD and a[2] == 0) or not self.ensure(a[0]):
                return
            rec = {"k": "sa" if addr == H_ADD else "ss", "L": hx(a[0]), "s": a[1], "v": s32(a[2]),
                   "l": a[3] & 0xFFFF, "fl": self.u32(a[0] + SL_FLAGS)}
            if addr == H_SETB:
                rec["u"] = self.uid(a[4])
            self.emit(rec)
        elif addr == H_RMALL:
            (a,) = self.args(ctx, 1)
            if self.ensure(a):
                self.emit({"k": "sr", "L": hx(a), "fl": self.u32(a + SL_FLAGS)})
        elif addr == H_DETACH:
            (a,) = self.args(ctx, 1)
            if a in self.tracked:
                self.emit({"k": "sdt", "L": hx(a), "fl": self.u32(a + SL_FLAGS)})
        elif addr == H_ATTACH:
            unit, lst, root = ctx.Ebx, ctx.Edi, ctx.Esi
            reset = self.u32(ctx.Ebp + 0x10)
            uid = self.uid(unit)
            if not uid or not self.ensure(root):
                return
            if lst not in self.tracked:
                self.seed(lst)
            self.emit({"k": "sat", "U": uid, "L": hx(lst), "r": 1 if reset else 0,
                       "fl": self.u32(lst + SL_FLAGS), "R": hx(root)})
        elif addr == H_FREE:
            a = ctx.Esi
            if a in self.tracked:
                self.emit({"k": "sfr", "L": hx(a)})
                self.tracked.discard(a)
        elif addr == H_ALLOCX:
            lst, unit = ctx.Esi, ctx.Edi
            uid = self.uid(unit)
            if uid:
                self.tracked.add(lst)
                self.emit({"k": "sax", "L": hx(lst), "U": uid, "ui": self.uinfo(unit),
                           "fl": self.u32(lst + SL_FLAGS), "ot": self.u32(lst + SL_OTYPE),
                           "og": self.u32(lst + SL_OGUID), "cb": bool(self.u32(lst + SL_VCB))})
        elif addr in (H_STATIC, H_DYNAMIC, H_BYTIME):
            unit, item = self.args(ctx, 2)
            uid, iid = self.uid(unit), self.uid(item)
            if not uid or not iid or not self.ensure(self.u32(unit + U_LIST)):
                return
            self.ensure(self.u32(item + U_LIST))
            if addr == H_BYTIME:
                self.emit({"k": "sbt", "U": uid, "I": iid})
            else:
                self.emit({"k": "sdy", "U": uid, "I": iid, "on": 1 if addr == H_DYNAMIC else 0})
        elif addr == H_TOGGLE:
            unit, state, on = self.args(ctx, 3)
            uid = self.uid(unit)
            if uid and self.ensure(self.u32(unit + U_LIST)):
                self.emit({"k": "stg", "U": uid, "s": state, "on": 1 if on else 0})
        elif addr == H_EXPIRE:
            unit, frame = self.args(ctx, 2)
            uid = self.uid(unit)
            root = self.u32(unit + U_LIST) if unit else 0
            if not uid or frame == 0 or not self.ensure(root):
                return
            chain, c, n = [], self.u32(root + SL_LAST), 0
            while c and n < 4096:
                chain.append([hx(c), self.u32(c + SL_FLAGS), self.i32(c + SL_EXPIRE)])
                c, n = self.u32(c + SL_PREV), n + 1
            self.in_expire += 1
            self.emit({"k": "sxp", "U": uid, "f": s32(frame), "lists": chain})
        elif addr == H_XFREE:
            if self.in_expire and ctx.Ecx in self.tracked:
                self.emit({"k": "sxf", "L": hx(ctx.Ecx)})
        elif addr in H_XEND:
            if self.in_expire:
                self.in_expire -= 1
                self.emit({"k": "sxe"})
        elif addr in H_CB:
            unit, key, old, new = self.args_cb(ctx)
            emitted = ctx.Ebx in self.tracked
            self.cb_stack.setdefault(tid, []).append(emitted)
            if emitted:
                self.emit({"k": "scb", "L": hx(ctx.Ebx), "key": s32(key), "old": s32(old),
                           "new": s32(new), "u": self.uid(unit)})
        elif addr in H_CBX:
            st = self.cb_stack.get(tid)
            if st and st.pop():
                self.emit({"k": "scx"})
        elif addr in (H_REGEN_LIFE, H_REGEN_STAM, H_REGEN_MANA, H_REGEN_MON):
            unit = {H_REGEN_LIFE: ctx.Esi, H_REGEN_STAM: ctx.Edi, H_REGEN_MANA: ctx.Edi,
                    H_REGEN_MON: ctx.Edx}[addr]
            uid = self.uid(unit)
            if uid and self.ensure(self.u32(unit + U_LIST)):
                kind = {H_REGEN_LIFE: "sgl", H_REGEN_STAM: "sgs", H_REGEN_MANA: "sgm",
                        H_REGEN_MON: "sgx"}[addr]
                self.emit({"k": kind, "U": uid, "mode": self.u32(unit + U_MODE)})

    def args_cb(self, ctx):
        # pushed before `call eax`: unit, key, old, new (stat-lists.md §7)
        return struct.unpack("<4I", self.read(ctx.Esp, 16))


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=180.0, help="kill the game after N s (default 180)")
    ap.add_argument("--ticks", type=int, default=0, help="stop after N recorded ticks (default: no limit)")
    ap.add_argument("--snap-every", type=int, default=25, help="stat snapshot every N frames (default 25; 0 = none)")
    ap.add_argument("--snap-monsters", type=int, default=24, help="monsters per snapshot (default 24)")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-stats.jsonl)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"], help="Game.exe arguments (default: -w -ns)")
    autostart.add_options(ap)
    a = ap.parse_args()
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-stats.jsonl")
    # keep only the base recorder's tick and step hooks, add ours
    rt.EXPECT = {k: v for k, v in rt.EXPECT.items() if k == rt.TICK or k in rt.STEPS}
    rt.EXPECT.update(STATS_EXPECT)
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = StatsRecorder(os.path.abspath(a.game), gargs, out, a.seconds, a.ticks,
                      a.snap_every, a.snap_monsters)
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
