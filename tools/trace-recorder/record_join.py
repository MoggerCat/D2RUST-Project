"""Read a few values of the original 1.14d Game.exe at a single-player join.

Subclass of spawn.py's SpawnRecorder with no spawn: the same start-up (menu
forced, character from -name), the record_packets.py message hooks always
armed (packets-raw-1 side file, check_packets.py reads it), and these reads:

  * object control (specs/world/objects.md §2, client/model.md §11 r1): the
    entry of 0x00546C60 (registers, stack arguments) and its result (EAX,
    by a one-shot trap on the return address), then game +0x80 at the first
    server tick and at every S->C 0x03 queued (C85);
  * client monster creation (specs/client/msg-units.md §1.2, model.md §2 r6,
    §12 r5, OQ 9): at the entry of 0x00466360 the seed (+0x6C) and rect
    (+0x4C..+0x58) of every active room of the client act ([0x007A0634],
    list at act +0x10, next +0x7C); at its return the new unit (EAX: type,
    class, GUID, seed +0x20, init seed +0x28, path point, path room) and the
    rooms again, plus the server unit of the same GUID (seed, init seed,
    its room's seed) (C83).

Writes traces/raw/<time>-join.jsonl (format join-raw-0, provisional) and
<out>-packets.jsonl. The game is always terminated when the script ends.
Our own code. Nothing here is derived from Blizzard code.
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

TOOL = "trace-recorder record_join 0.1.0"
RAW_FORMAT = "join-raw-0"

OBJ_CONTROL = 0x546C60      # objects.md §2: builds the object control, result = lo'
CL_MONSTER = 0x466360       # msg-units.md §1.2: client monster create (S->C 0xAC)
CL_ACT = 0x7A0634           # model.md §2 r7: the client act
ENTRY = {OBJ_CONTROL: None, CL_MONSTER: None}
A_ROOMS, R_NEXT = 0x10, 0x7C
G_OBJSEED = 0x80


class JoinRecorder(sp.SpawnRecorder):
    def __init__(self, exe, args, out, a):
        super().__init__(exe, args, out, a)
        self.pending_ret = {}    # (tid, ret addr) -> (kind, entry info)
        self.creates = 0

    def install(self, base):
        r = super().install(base)
        for addr in ENTRY:
            ENTRY[addr] = self.orig_code(addr, 3).hex()
            self.add_role(addr, "join")
        self.notes.append("join hooks: " + ", ".join(f"{a:#x}={b}" for a, b in ENTRY.items()))
        return r

    # --- reads ------------------------------------------------------------
    def client_rooms(self):
        act = self.read_u32(CL_ACT)
        out = []
        r = self.read_u32(act + A_ROOMS) if act else 0
        while r and len(out) < 4000:
            x, y, w, h = struct.unpack("<4i", self.read(r + 0x4C, 16))
            out.append([f"{r:#x}", x, y, w, h, *struct.unpack("<II", self.read(r + 0x6C, 8))])
            r = self.read_u32(r + R_NEXT)
        return {"act": f"{act:#x}", "rooms": out}

    def read_game_obj(self, where):
        if self.game is None:
            return
        self.log({"type": "game_obj", "where": where, "game": f"{self.game:#x}",
                  "frame": self.read_u32(self.game + sp.G_FRAME),
                  "obj_seed": self.read_u32(self.game + G_OBJSEED),
                  "game_seed": list(struct.unpack("<II", self.read(self.game + sp.G_SEED, 8)))})

    def client_unit(self, u):
        rec = {"addr": f"{u:#x}", "kind": self.read_u32(u), "class": self.read_u32(u + 4),
               "guid": self.read_u32(u + 0x0C), "mode": self.read_u32(u + 0x10),
               "seed": list(struct.unpack("<II", self.read(u + 0x20, 8))),
               "init_seed": self.read_u32(u + 0x28)}
        path = self.read_u32(u + sp.U_PATH)
        if path:
            rec["x"], rec["y"] = self.u16(path + 2), self.u16(path + 6)
            rec["room"] = f"{self.read_u32(path + 0x1C):#x}"
        return rec

    def server_unit(self, guid):
        if self.game is None:
            return None
        u = self.monsters(self.game).get(guid)
        if not u:
            return None
        rec = {"addr": f"{u:#x}", "class": self.read_u32(u + 4),
               "seed": list(struct.unpack("<II", self.read(u + 0x20, 8))),
               "init_seed": self.read_u32(u + 0x28)}
        path = self.read_u32(u + sp.U_PATH)
        if path:
            rec["x"], rec["y"] = self.u16(path + 2), self.u16(path + 6)
            room = self.read_u32(path + 0x1C)
            if room:
                rec["room"] = f"{room:#x}"
                rec["room_rect"] = list(struct.unpack("<4i", self.read(room + 0x4C, 16)))
                rec["room_seed"] = list(struct.unpack("<II", self.read(room + 0x6C, 8)))
        return rec

    # --- events -------------------------------------------------------------
    def on_breakpoint(self, tid, addr):
        roles = self.roles.get(addr, ())
        if "jret" in roles:
            ctx = self.get_ctx(tid)
            key = (tid, addr)
            if key in self.pending_ret and ctx.Esp > self.pending_ret[key][1]["esp"]:
                kind, info = self.pending_ret.pop(key)
                if not any(k[1] == addr for k in self.pending_ret):
                    self.drop_role(addr, "jret")
                self.on_join_return(tid, kind, info, ctx)
        if "join" in roles:
            ctx = self.get_ctx(tid)
            esp = ctx.Esp
            ret = self.read_u32(esp)
            info = {"esp": esp, "ret": ret, "ecx": ctx.Ecx, "edx": ctx.Edx,
                    "stack": [self.read_u32(esp + 4 * k) for k in range(1, 7)]}
            if addr == CL_MONSTER:
                info["before"] = self.client_rooms()
            self.pending_ret[(tid, ret)] = (addr, info)
            self.add_role(ret, "jret")
        if "pkt" in roles and self.pkt is not None and addr == 0x53B280:
            ctx = self.get_ctx(tid)
            msg = self.read_u32(ctx.Esp + 4)  # record_packets.py: [ESP+4] = message
            if msg and self.read(msg, 1)[0] == 0x03:
                self.read_game_obj("s2c_0x03")
        first = self.game is None
        super().on_breakpoint(tid, addr)
        if first and self.game is not None:
            self.read_game_obj("first_tick")

    def on_join_return(self, tid, kind, info, ctx):
        base = {"tid": tid, "caller": f"{self.call_site(info['ret']):#x}",
                "ecx": f"{info['ecx']:#x}", "edx": f"{info['edx']:#x}",
                "stack": [f"{v:#x}" for v in info["stack"]], "eax": ctx.Eax,
                "frame": self.pkt.frame if self.pkt else None}
        if kind == OBJ_CONTROL:
            base["type"] = "obj_control"
            self.log(base)
            self.read_game_obj("obj_control_return")
            return
        self.creates += 1
        if self.creates > self.a.max_creates:
            return
        base["type"] = "cl_monster"
        u = ctx.Eax
        if u:
            try:
                base["unit"] = self.client_unit(u)
            except OSError as e:
                base["unit_error"] = str(e)
        base["rooms_before"] = info["before"]
        base["rooms_after"] = self.client_rooms()
        before = {r[0]: r[5:] for r in info["before"]["rooms"]}
        base["rooms_changed"] = [r for r in base["rooms_after"]["rooms"]
                                 if before.get(r[0]) != r[5:]]
        if u and "unit" in base:
            s = self.server_unit(base["unit"]["guid"])
            if s:
                base["server_unit"] = s
        self.log(base)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=60.0)
    ap.add_argument("--max-creates", type=int, default=200,
                    help="log at most this many client monster creations (default 200)")
    ap.add_argument("--status", default=None, help="as spawn.py --status")
    ap.add_argument("--out", default=None, help="output .jsonl (default traces/raw/<time>-join.jsonl)")
    ap.add_argument("game_args", nargs="*", default=None,
                    help="Game.exe arguments (default -w -ns -nosave -name bdAma -ama)")
    a = ap.parse_args()
    args = a.game_args or ["-w", "-ns", "-nosave", "-name", "bdAma", "-ama"]
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-join.jsonl")
    pk = (out[:-6] if out.endswith(".jsonl") else out) + "-packets.jsonl"
    ns = types.SimpleNamespace(
        seconds=a.seconds, no_inline=True, packets=pk, status=a.status, level=None,
        trigger=out + ".never", tick=1 << 30, cls=0, kind="normal", superunique=None,
        dx=0, dy=0, spread=0, flags=0, after_ticks=0, force_after=4.0, no_force=False,
        max_creates=a.max_creates)
    sp.TOOL, sp.RAW_FORMAT = TOOL, RAW_FORMAT
    r = JoinRecorder(os.path.abspath(a.game), args, out, ns)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    print(f"wrote {out}")
    print(f"wrote {pk} ({r.pkt.seq} message events)")
    for n in r.notes:
        print("note:", n)


if __name__ == "__main__":
    main()
