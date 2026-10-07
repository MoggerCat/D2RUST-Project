"""Record object allocations / operates and room population of the original 1.14d Game.exe.

Subclass of spawn.py's SpawnRecorder (start-up, --status, --trigger spawns,
--packets side file). The record_rng.py hooks (helpers, setters, inline
steps) are armed only inside *windows*, each opened at a function entry and
closed by a one-shot trap on its return address:

  * object allocation: unit allocation 0x00555230 with ECX = 2 (sim/units.md
    §3.1; world/objects.md §3, OQ 2). Inside it the creation / init dispatch
    0x0054F5D0, every init function of the table 0x00731BC0 and the
    animation setup 0x00624390 (objects.md §4) are logged as markers, so the
    draw order (anim draw vs init draws) is in the file. At the return the
    object is logged (GUID, mode, seed, init seed, anim fields +0x44/+0x48/
    +0x4C, InteractType, the objects.txt row's Sync / InitFn / OperateFn /
    PreOperate / FrameDelta of the mode).
  * operate: dispatch 0x00584420 (objects.md §7.2), with a tail of
    --operate-ticks server frames after its return (delayed events).
  * first room population (monsters/population.md §1 r1): preset units
    0x005559A0, inactive-unit restore 0x00542B40, object population
    0x00552610, monster population 0x0054EC90; restricted to --pop-levels.
    At 0x0054EC90's entry the room's DRLG room +0x64 logical-room info
    (drlg/levels.md §11.1: flags, count, every coordinate record) is dumped
    (record "logic"), and the monsters created by the window are logged.
  * --rng-file FILE: armed while FILE exists (checked every server frame).
  * --dump-file FILE: when FILE appears (deleted on use) every DRLG room of
    the player's level is dumped (rect, type, status, flags, +0x60, rooms-near
    array, logical info): record "level_dump" (levels.md OQ 7).

Writes traces/raw/<time>-objects.jsonl (format objects-raw-0, provisional:
the rng-raw-1 draw / seed_set records, each with "win" = the innermost open
window, plus the records above). check_rng.py reads it. The game is always
terminated when the script ends. Our own code; nothing here is derived from
Blizzard code.
"""

import argparse
import datetime
import os
import struct
import sys
import types

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record_rng as rr  # noqa: E402
import spawn as sp  # noqa: E402

TOOL = "trace-recorder record_objects 0.1.0"
RAW_FORMAT = "objects-raw-0"

ALLOC = 0x555230        # units.md §3.1: ECX type, EDX class; x, y, game, room, flags, mode, guid
OBJ_INIT = 0x54F5D0     # objects.md §3
ANIM = 0x624390         # objects.md §4 (animation setup of a mode change)
OPERATE = 0x584420      # objects.md §7.2
INIT_TABLE, INIT_COUNT = 0x731BC0, 80
POP = {0x5559A0: "presets", 0x542B40: "restore", 0x552610: "objects", 0x54EC90: "monsters"}
OBJ_FIELDS = {"Sync": (373, "B"), "InitFn": (433, "B"), "OperateFn": (435, "B"),
              "PreOperate": (317, "B"), "Mode1": (320, "B")}   # specs/data/fields.tsv (objects)
FRAMEDELTA0, SELECTABLE0 = 248, 196
ENTRY_BYTES = {a_: "558bec" for a_ in (ALLOC, OBJ_INIT, ANIM, OPERATE, *POP)}
DR_NEXT, DR_X, DR_STATUS, DR_TYPE, DR_LEVEL, DR_FLAGS = 0x24, 0x34, 0x44, 0x48, 0x58, 0x28
DR_NEAR, DR_NEAR_N, DR_ACTIVE, DR_OTHER, DR_INFO = 0x08, 0x2C, 0x30, 0x60, 0x64


class ObjectRecorder(sp.SpawnRecorder):
    def __init__(self, exe, args, out, a):
        super().__init__(exe, args, out, a)
        self.win = []            # open windows: labels
        self.opend = {}        # (tid, ret) -> list of (kind, info)
        self.tail_until = None   # operate tail (server frame)
        self.file_win = False
        self.init_fns = {}
        self.markers_on = False
        self.frame = None
        self.pop_seen = 0

    # --- windows ------------------------------------------------------------
    def emit(self, rec):
        if rec.get("type") in ("draw", "seed_set") and self.win:
            rec["win"] = self.win[-1]
        super().emit(rec)

    def disarm_rng(self):
        if self.win or self.tail_until is not None or self.file_win:
            return  # one of our windows still needs the hooks (a spawn ended inside it)
        super().disarm_rng()

    def any_open(self):
        return bool(self.win) or self.tail_until is not None or self.file_win

    def open_win(self, label):
        if not self.any_open() and self.state != "calling":
            self.arm_rng()
        self.win.append(label)
        if not self.markers_on:
            for a_ in [OBJ_INIT, ANIM] + list(self.init_fns):
                self.add_role(a_, "omark")
            self.markers_on = True

    def close_win(self, label):
        if label in self.win:
            self.win.reverse()
            self.win.remove(label)
            self.win.reverse()
        self.maybe_disarm()

    def maybe_disarm(self):
        if not self.any_open():
            if self.markers_on:
                for a_ in [OBJ_INIT, ANIM] + list(self.init_fns):
                    self.drop_role(a_, "omark")
                self.markers_on = False
            if self.state != "calling":
                super().disarm_rng()

    # --- reads ------------------------------------------------------------
    def level_of_active(self, room):
        try:
            dr = self.read_u32(room + 0x10)
            lv = self.read_u32(dr + DR_LEVEL) if dr else 0
            return self.read_u32(lv + 0x1D0) if lv else None
        except OSError:
            return None

    def is_active_room(self, v):
        if not v or v < 0x10000:
            return False
        try:
            dr = self.read_u32(v + 0x10)
            return bool(dr) and self.read_u32(dr + DR_ACTIVE) == v
        except OSError:
            return False

    def find_room(self, info):
        for v in [info["ecx"], info["edx"]] + info["stack"]:
            if self.is_active_room(v):
                return v
        return 0

    def logic_info(self, dr):
        p = self.read_u32(dr + DR_INFO)
        if not p:
            return None
        flags, n = self.read_u32(p), self.read_u32(p + 4)
        recs, r, k = [], self.read_u32(p + 0x30), 0
        while r and k < 4096:
            b = struct.unpack("<12i", self.read(r, 0x30))
            recs.append({"box": list(b[0:4]), "clip": list(b[4:8]), "node": b[8], "x24": b[9],
                         "index": b[10], "addr": f"{r:#x}"})
            r, k = b[11] & 0xFFFFFFFF, k + 1
        return {"info": f"{p:#x}", "flags": flags, "count": n, "records": recs}

    def drlg_room(self, dr):
        x, y, w, h = struct.unpack("<4i", self.read(dr + DR_X, 16))
        near_p, near_n = self.read_u32(dr + DR_NEAR), self.read_u32(dr + DR_NEAR_N)
        near = []
        if near_p and 0 < near_n < 64:
            near = [f"{v:#x}" for v in struct.unpack(f"<{near_n}I", self.read(near_p, 4 * near_n))]
        return {"dr": f"{dr:#x}", "rect": [x, y, w, h], "type": self.read_u32(dr + DR_TYPE),
                "status": self.read(dr + DR_STATUS, 1)[0], "flags": f"{self.read_u32(dr + DR_FLAGS):#x}",
                "other": self.read_u32(dr + DR_OTHER), "active": f"{self.read_u32(dr + DR_ACTIVE):#x}",
                "near": near, "logic": self.logic_info(dr)}

    def level_dump(self, why):
        if self.game is None:
            return
        pl = self.player(self.game)
        path = self.read_u32(pl + sp.U_PATH) if pl else 0
        room = self.read_u32(path + 0x1C) if path else 0
        dr0 = self.read_u32(room + 0x10) if room else 0
        lv = self.read_u32(dr0 + DR_LEVEL) if dr0 else 0
        if not lv:
            return
        rooms, dr, n = [], self.read_u32(lv + 0x10), 0
        while dr and n < 1000:
            rooms.append(self.drlg_room(dr))
            dr, n = self.read_u32(dr + DR_NEXT), n + 1
        self.log({"type": "level_dump", "why": why, "frame": self.frame,
                  "level": self.read_u32(lv + 0x1D0), "level_rect": list(struct.unpack("<4i", self.read(lv + 0x1C, 16))),
                  "level_type": self.read_u32(lv), "counter": self.read_u32(lv + 0x1DC),
                  "room_count": self.read_u32(lv + 0x08), "player_dr": f"{dr0:#x}", "rooms": rooms})

    def obj_record(self, u):
        rec = {"addr": f"{u:#x}", "kind": self.read_u32(u), "class": self.read_u32(u + 4),
               "guid": self.read_u32(u + 0x0C), "mode": self.read_u32(u + 0x10),
               "seed": list(struct.unpack("<II", self.read(u + 0x20, 8))),
               "init_seed": self.read_u32(u + 0x28),
               "anim": {"cur": self.i32(u + 0x44), "fc": self.i32(u + 0x48),
                        "speed": struct.unpack("<h", self.read(u + 0x4C, 2))[0]},
               "flags": f"{self.read_u32(u + 0xC4):#x}", "spark": self.read(u + 0x78, 1)[0]}
        path = self.read_u32(u + sp.U_PATH)
        if path:
            rec["x"], rec["y"] = self.read_u32(path + 0x0C), self.read_u32(path + 0x10)
        od = self.read_u32(u + sp.U_DATA)
        if od:
            txt = self.read_u32(od)
            rec["interact"] = self.read(od + 4, 1)[0]
            if txt:
                rec["name"] = self.read(txt, 63).split(b"\0", 1)[0].decode("latin-1")
                for k, (o, f) in OBJ_FIELDS.items():
                    rec[k] = struct.unpack_from("<" + f, self.read(txt + o, 1))[0]
                m = rec["mode"]
                if 0 <= m < 8:
                    rec["FrameDelta"] = struct.unpack("<h", self.read(txt + FRAMEDELTA0 + 2 * m, 2))[0]
        return rec

    # --- hooks --------------------------------------------------------------
    def install(self, base):
        r = super().install(base)
        for a_, want in ENTRY_BYTES.items():
            got = self.orig_code(a_, len(want) // 2).hex()
            if got != want:
                raise RuntimeError(f"unexpected code at {a_:#x} ({got}, want {want}): not the 1.14d Game.exe?")
        tbl = self.read(INIT_TABLE, 4 * INIT_COUNT)
        for i in range(INIT_COUNT):
            f = struct.unpack_from("<I", tbl, 4 * i)[0]
            if f:
                self.init_fns.setdefault(f, []).append(i)
        self.add_role(ALLOC, "oent")
        self.add_role(OPERATE, "oent")
        if self.a.pop:
            for a_ in POP:
                self.add_role(a_, "oent")
        self.notes.append(f"object hooks: alloc, operate, {len(self.init_fns)} init fns; pop {self.a.pop}")
        return r

    def on_breakpoint(self, tid, addr):
        roles = self.roles.get(addr, ())
        if "oret" in roles:
            ctx = self.get_ctx(tid)
            key = (tid, addr)
            lst = self.opend.get(key)
            if lst and ctx.Esp > lst[-1][1]["esp"]:
                kind, info = lst.pop()
                if not lst:
                    del self.opend[key]
                if not any(k[1] == addr for k in self.opend):
                    self.drop_role(addr, "oret")
                self.on_ret(tid, kind, info, ctx)
        if "omark" in roles and self.win:
            ctx = self.get_ctx(tid)
            esp = ctx.Esp
            rec = {"type": "mark", "fn": f"{addr:#x}", "tid": tid, "win": self.win[-1],
                   "ecx": f"{ctx.Ecx:#x}", "edx": f"{ctx.Edx:#x}",
                   "stack": [f"{self.read_u32(esp + 4 * k):#x}" for k in range(0, 4)]}
            if addr in self.init_fns:
                rec["init_index"] = self.init_fns[addr]
            if addr == ANIM:
                for v in (ctx.Ecx, ctx.Edx, self.read_u32(esp + 4), self.read_u32(esp + 8)):
                    try:
                        if v > 0x10000 and self.read_u32(v) == 2:
                            rec["obj"] = {"class": self.read_u32(v + 4), "guid": self.read_u32(v + 0x0C),
                                          "mode": self.read_u32(v + 0x10)}
                            break
                    except OSError:
                        pass
            self.log(rec)
        if "oent" in roles:
            ctx = self.get_ctx(tid)
            self.on_entry(tid, addr, ctx)
        if addr == sp.TICK_RET and self.game is not None:
            ctx = self.get_ctx(tid)
            if ctx.Esi == self.game:
                self.on_tick(self.read_u32(self.game + sp.G_FRAME))
        super().on_breakpoint(tid, addr)

    def on_tick(self, frame):
        self.frame = frame
        if self.tail_until is not None and frame >= self.tail_until:
            self.tail_until = None
            self.log({"type": "tail_end", "frame": frame})
            self.maybe_disarm()
        a = self.a
        if a.rng_file:
            on = os.path.exists(a.rng_file)
            if on != self.file_win:
                if on and not self.any_open() and self.state != "calling":
                    self.arm_rng()
                self.file_win = on
                self.log({"type": "rng_file", "on": on, "frame": frame})
                if not on:
                    self.maybe_disarm()
        if a.dump_file and os.path.exists(a.dump_file):
            try:
                why = open(a.dump_file, encoding="utf-8").read().strip() or "dump_file"
                os.remove(a.dump_file)
            except OSError:
                why = None
            if why is not None:
                self.level_dump(why)

    def on_entry(self, tid, addr, ctx):
        esp = ctx.Esp
        ret = self.read_u32(esp)
        info = {"esp": esp, "ret": ret, "ecx": ctx.Ecx, "edx": ctx.Edx,
                "stack": [self.read_u32(esp + 4 * k) for k in range(1, 8)]}
        if addr == ALLOC:
            if ctx.Ecx != 2 or (self.game is not None and info["stack"][2] != self.game):
                return
            kind = "alloc"
            self.log({"type": "obj_alloc", "frame": self.frame, "tid": tid, "class": ctx.Edx,
                      "x": info["stack"][0], "y": info["stack"][1], "room": f"{info['stack'][3]:#x}",
                      "flags": info["stack"][4], "mode": info["stack"][5], "fixed_guid": info["stack"][6],
                      "caller": f"{self.call_site(ret):#x}", "win_outer": self.win[-1] if self.win else None})
            self.open_win(f"alloc:{ctx.Edx}")
        elif addr == OPERATE:
            kind = "operate"
            self.log({"type": "operate", "frame": self.frame, "tid": tid, "ecx": f"{ctx.Ecx:#x}",
                      "edx": f"{ctx.Edx:#x}", "stack": [f"{v:#x}" for v in info["stack"][:6]],
                      "caller": f"{self.call_site(ret):#x}"})
            self.open_win("operate")
        else:
            room = self.find_room(info)
            lvl = self.level_of_active(room) if room else None
            if self.a.pop_levels and lvl not in self.a.pop_levels:
                return
            kind = POP[addr]
            info["room"], info["level"] = room, lvl
            rec = {"type": "pop", "fn": kind, "frame": self.frame, "tid": tid,
                   "room": f"{room:#x}", "level": lvl, "caller": f"{self.call_site(ret):#x}"}
            if room:
                rec["room_rect"] = list(struct.unpack("<4i", self.read(room + 0x4C, 16)))
                rec["room_seed"] = list(struct.unpack("<II", self.read(room + 0x6C, 8)))
                if self.game is not None:
                    rec["game_seed"] = list(struct.unpack("<II", self.read(self.game + sp.G_SEED, 8)))
            if addr == 0x54EC90 and room:
                info["before"] = set(self.monsters(self.game)) if self.game else set()
                try:
                    rec["drlg_room"] = self.drlg_room(self.read_u32(room + 0x10))
                except OSError as e:
                    rec["drlg_error"] = str(e)
            self.log(rec)
            self.open_win(f"pop:{kind}")
        info["label"] = self.win[-1]
        self.opend.setdefault((tid, ret), []).append((kind, info))
        self.add_role(ret, "oret")

    def on_ret(self, tid, kind, info, ctx):
        label = info["label"]
        if kind == "alloc":
            rec = {"type": "obj_alloc_ret", "frame": self.frame, "tid": tid, "eax": f"{ctx.Eax:#x}"}
            if ctx.Eax:
                try:
                    rec["obj"] = self.obj_record(ctx.Eax)
                except OSError as e:
                    rec["error"] = str(e)
            self.log(rec)
        elif kind == "operate":
            self.log({"type": "operate_ret", "frame": self.frame, "tid": tid, "eax": ctx.Eax})
            if self.a.operate_ticks > 0 and self.frame is not None:
                if self.tail_until is None:
                    self.log({"type": "tail_start", "frame": self.frame, "until": self.frame + self.a.operate_ticks})
                self.tail_until = max(self.tail_until or 0, self.frame + self.a.operate_ticks)
        else:
            rec = {"type": "pop_ret", "fn": kind, "frame": self.frame, "tid": tid,
                   "room": f"{info.get('room', 0):#x}", "eax": f"{ctx.Eax:#x}"}
            room = info.get("room")
            if room:
                rec["room_seed"] = list(struct.unpack("<II", self.read(room + 0x6C, 8)))
                if self.game is not None:
                    rec["game_seed"] = list(struct.unpack("<II", self.read(self.game + sp.G_SEED, 8)))
            if "before" in info and self.game is not None:
                mons = self.monsters(self.game)
                new = [mons[k] for k in sorted(mons) if k not in info["before"]]
                rec["created"] = [self.unit_record(u) for u in new]
            self.log(rec)
        self.close_win(label)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=300.0)
    ap.add_argument("--no-inline", action="store_true", help="helpers and setters only")
    ap.add_argument("--no-pop", dest="pop", action="store_false", help="no population windows")
    ap.add_argument("--pop-levels", default="", help="comma list of level ids for population windows "
                                                      "(default: every level)")
    ap.add_argument("--operate-ticks", type=int, default=50,
                    help="keep the RNG hooks armed this many frames after an operate (default 50)")
    ap.add_argument("--rng-file", default=None, help="RNG hooks armed while this file exists")
    ap.add_argument("--dump-file", default=None, help="dump the player's level when this file appears")
    ap.add_argument("--status", default=None, help="as spawn.py --status")
    ap.add_argument("--trigger", default=None, help="as spawn.py --trigger (spawns)")
    ap.add_argument("--level", type=int, default=None, help="as spawn.py --level")
    ap.add_argument("--class", dest="cls", type=int, default=5)
    ap.add_argument("--kind", default="normal", choices=["normal", "champion", "unique", "boss"])
    ap.add_argument("--packets", nargs="?", const="auto", default=None)
    ap.add_argument("--out", default=None, help="output .jsonl (default traces/raw/<time>-objects.jsonl)")
    ap.add_argument("game_args", nargs="*", default=None,
                    help="Game.exe arguments (default -w -ns -nosave -name bdAma -ama)")
    a = ap.parse_args()
    args = a.game_args or ["-w", "-ns", "-nosave", "-name", "bdAma", "-ama"]
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-objects.jsonl")
    pk = None
    if a.packets:
        pk = a.packets if a.packets != "auto" else (out[:-6] if out.endswith(".jsonl") else out) + "-packets.jsonl"
    ns = types.SimpleNamespace(
        seconds=a.seconds, no_inline=a.no_inline, packets=pk, status=a.status, level=a.level,
        trigger=a.trigger or (out + ".never"), tick=0, cls=a.cls, kind=a.kind, superunique=None,
        dx=4, dy=4, spread=4, flags=0, after_ticks=0, force_after=4.0, no_force=False,
        pop=a.pop, pop_levels={int(x) for x in a.pop_levels.split(",") if x.strip()},
        operate_ticks=a.operate_ticks, rng_file=a.rng_file, dump_file=a.dump_file)
    sp.TOOL, sp.RAW_FORMAT = TOOL, RAW_FORMAT
    r = ObjectRecorder(os.path.abspath(a.game), args, out, ns)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    print(f"wrote {out}")
    if pk:
        print(f"wrote {pk} ({r.pkt.seq} message events)")
    for n in r.notes:
        print("note:", n)


if __name__ == "__main__":
    main()
