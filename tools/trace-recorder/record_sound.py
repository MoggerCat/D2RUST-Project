"""Record the sound requests of the original 1.14d Game.exe client.

Subclass of spawn.py's SpawnRecorder (start-up, --status, --trigger spawns,
--packets side file), with the hooks of specs/audio/triggers.md "Checks"
and specs/audio/environment.md "Checks" (client/audio.md §B7):

  request      0x004B9A00 entry + return: T [0x7BC9BC], C [0x7A0498], ECX id,
               EDX unit (type +0, GUID +0x0C), stack delay / flags / offset,
               the result (handle), the caller, and the local player's
               client seed ([0x7A6A70] +0x20/+0x24) before and after
  volume       0x004B9B50 entry: ECX handle, EDX value
  roll         0x004E40A0 entry + return: ECX n, EAX result, seed before/after
  footstep     0x004CAF60 entry: ECX unit, unit +0x44/+0x48/+0x4C/+0x84, C
  mode_sound   0x004CC5B0 entry: ECX unit, EDX, stack words, unit mode +0x10
  music        0x004DCAA0 entry + return: T, C, cur, last level, Tl, Ts,
               last announced, resume[cur] (table pointer [0x7BC99C])
  stinger      0x004DCD40 entry: ECX, EDX, 5 stack words, C
  ambience     0x004E42E0 entry + return: T, env period index, rain flag and
               intensity, [0x7C8C7C..0x7C8C98]
  cue_pos      0x004B99A0 entry: ECX handle, EDX, 3 stack words (raw)
  entry_line   0x004CC270 entry: ECX, EDX, stack, C, local player +0x7C

With --env (queue entry 69, render/lighting.md §9.2): the environment
update 0x0061BFC0 (per client update; ECX act, EDX player room) and the
S->C 0x53 setter 0x0061C240, each at return with the environment record
(act +0x04): period index +0x00, type +0x04, ticks +0x08, intensity +0x0C,
R G B +0x18..+0x1A, eclipse +0x30, and the room's level id.

Every record carries the hook nesting (the open hook names on that thread)
so a request can be tied to the rule that made it. Writes
traces/raw/<time>-sound.jsonl (format sound-raw-0, provisional). The game
is always terminated when the script ends.
Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import os
import struct
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record_rng as rr  # noqa: E402,F401
import spawn as sp  # noqa: E402

TOOL = "trace-recorder record_sound 0.1.0"
RAW_FORMAT = "sound-raw-0"

T_SOUND, C_UPDATE, LOCAL_PLAYER = 0x7BC9BC, 0x7A0498, 0x7A6A70
MUSIC_VARS = {"cur": 0x7C89D8, "last_level": 0x7C89DC, "tl": 0x7C89D0, "ts": 0x7C89E0,
              "last_announced": 0x7C8A04}
RESUME_PTR, SONG_FIRST = 0x7BC99C, 4657
AMB_VARS = 0x7C8C7C           # 8 dwords: last level, bed handle, rain handle, bed id, prev rain,
                              # event id, gap, last cue (environment.md §5-§8)
RAIN_ON, RAIN_INTENSITY = 0x7A8A44, 0x7A89A0   # record_frames.py WEATHER scalars
CL_ACT = 0x7A0634

# address -> (name, has return trap)
HOOKS = {
    0x4B9A00: ("request", True),
    0x4B9B50: ("volume", False),
    0x4E40A0: ("roll", True),
    0x4CAF60: ("footstep", False),
    0x4CC5B0: ("mode_sound", False),
    0x4DCAA0: ("music", True),
    0x4DCD40: ("stinger", False),
    0x4E42E0: ("ambience", True),
    0x4B99A0: ("cue_pos", False),
    0x4CC270: ("entry_line", False),
}
ENV_HOOKS = {
    0x61BFC0: ("env_update", True),
    0x61C240: ("env_set", True),
}


class SoundRecorder(sp.SpawnRecorder):
    def __init__(self, exe, args, out, a):
        super().__init__(exe, args, out, a)
        self.hooks = dict(HOOKS)
        if a.env:
            self.hooks.update(ENV_HOOKS)
        self.pending_ret = {}    # (tid, ret) -> [(name, esp, info)]
        self.open = {}           # tid -> list of open hook names
        self.env_only = a.env_only
        self.all_steps = a.footsteps == "all"

    def install(self, base):
        r = super().install(base)
        firsts = {}
        for addr, (name, _) in self.hooks.items():
            if self.env_only and addr not in ENV_HOOKS:
                continue
            firsts[name] = self.orig_code(addr, 3).hex()
            self.add_role(addr, "snd")
        self.notes.append("sound hooks: " + ", ".join(f"{k}={v}" for k, v in firsts.items()))
        return r

    # --- reads ------------------------------------------------------------
    def safe_u32(self, a):
        try:
            return self.read_u32(a)
        except OSError:
            return None

    def times(self):
        return {"T": self.safe_u32(T_SOUND), "C": self.safe_u32(C_UPDATE)}

    def pseed(self):
        p = self.safe_u32(LOCAL_PLAYER)
        if not p:
            return None
        try:
            return list(struct.unpack("<II", self.read(p + 0x20, 8)))
        except OSError:
            return None

    def unit(self, u):
        if not u:
            return None
        try:
            return [self.read_u32(u), self.read_u32(u + 0x0C)]
        except OSError:
            return [f"{u:#x}"]

    def env(self):
        act = self.safe_u32(CL_ACT)
        e = self.safe_u32(act + 4) if act else None
        if not e:
            return None
        b = self.read(e, 0x38)
        idx, typ, ticks, inten = struct.unpack_from("<iiii", b, 0)
        return {"idx": idx, "type": typ, "ticks": ticks, "I": inten, "rgb": list(b[0x18:0x1B]),
                "eclipse": struct.unpack_from("<i", b, 0x30)[0]}

    def room_level(self, room):
        try:
            drlg = self.read_u32(room + 0x10) if room else 0
            lv = self.read_u32(drlg + 0x58) if drlg else 0
            return self.read_u32(lv + 0x1D0) if lv else 0
        except OSError:
            return None

    def entry_info(self, name, ctx):
        esp = ctx.Esp
        st = [self.safe_u32(esp + 4 * k) for k in range(1, 6)]
        rec = {"ecx": ctx.Ecx, "edx": ctx.Edx}
        if name == "request":
            rec = {"id": ctx.Ecx, "unit": self.unit(ctx.Edx), "d": st[0], "f": st[1], "o": st[2],
                   "seed0": self.pseed()}
        elif name == "volume":
            rec = {"h": ctx.Ecx, "v": ctx.Edx}
        elif name == "roll":
            rec = {"n": ctx.Ecx, "seed0": self.pseed()}
        elif name == "footstep":
            u = ctx.Ecx
            rec["unit"] = self.unit(u)
            if u:
                rec["u44_48_4c"] = [self.safe_u32(u + o) for o in (0x44, 0x48, 0x4C)]
                rec["u84"] = self.safe_u32(u + 0x84)
        elif name == "mode_sound":
            rec["unit"] = self.unit(ctx.Ecx)
            rec["stack"] = st[:3]
            rec["mode"] = self.safe_u32(ctx.Ecx + 0x10) if ctx.Ecx else None
        elif name == "music":
            rec.update(self.music_vars())
        elif name == "stinger":
            rec["stack"] = st
        elif name == "ambience":
            rec.update(self.amb_vars())
        elif name == "cue_pos":
            rec["stack_hex"] = self.read(esp + 4, 12).hex()
        elif name == "entry_line":
            rec["stack"] = st[:3]
            p = self.safe_u32(LOCAL_PLAYER)
            rec["p7c"] = self.safe_u32(p + 0x7C) if p else None
        elif name in ("env_update", "env_set"):
            rec["stack"] = st[:4]
            if name == "env_update":   # stdcall: [ESP+4] act, [ESP+8] player room
                rec["L"] = self.room_level(st[1]) if st[1] else 0
        return rec

    def music_vars(self):
        out = {k: self.safe_u32(a) for k, a in MUSIC_VARS.items()}
        cur, tab = out.get("cur"), self.safe_u32(RESUME_PTR)
        if cur and tab and SONG_FIRST <= cur < SONG_FIRST + 28:
            out["resume_cur"] = self.safe_u32(tab + 4 * (cur - SONG_FIRST))
        return out

    def amb_vars(self):
        try:
            v = list(struct.unpack("<8I", self.read(AMB_VARS, 32)))
        except OSError:
            v = None
        e = self.env()
        return {"amb": v, "rain": self.safe_u32(RAIN_ON),
                "intensity_f32": self.safe_u32(RAIN_INTENSITY),
                "period": e["idx"] if e else None}

    # --- events -------------------------------------------------------------
    def on_breakpoint(self, tid, addr):
        roles = self.roles.get(addr, ())
        if "sret" in roles:
            ctx = self.get_ctx(tid)
            lst = self.pending_ret.get((tid, addr))
            if lst and ctx.Esp > lst[-1][1]:
                name, _, info = lst.pop()
                if not lst:
                    del self.pending_ret[(tid, addr)]
                    if not any(k[1] == addr for k in self.pending_ret):
                        self.drop_role(addr, "sret")
                stack = self.open.get(tid, [])
                if stack:
                    stack.pop()
                self.on_return_hook(tid, name, info, ctx)
        if "snd" in roles:
            ctx = self.get_ctx(tid)
            name, has_ret = self.hooks[addr]
            if name == "footstep" and not self.all_steps and ctx.Ecx != self.safe_u32(LOCAL_PLAYER):
                return super().on_breakpoint(tid, addr)
            ret = self.read_u32(ctx.Esp)
            rec = {"type": name, "tid": tid, "caller": f"{self.call_site(ret):#x}",
                   "in": list(self.open.get(tid, []))}
            rec.update(self.times())
            rec.update(self.entry_info(name, ctx))
            if has_ret:
                self.pending_ret.setdefault((tid, ret), []).append((name, ctx.Esp, rec))
                self.open.setdefault(tid, []).append(name)
                self.add_role(ret, "sret")
            else:
                self.log(rec)
        super().on_breakpoint(tid, addr)

    def on_return_hook(self, tid, name, rec, ctx):
        rec["ret"] = ctx.Eax
        if name in ("request", "roll"):
            rec["seed1"] = self.pseed()
        elif name == "music":
            rec["after"] = self.music_vars()
        elif name == "ambience":
            rec["after"] = self.amb_vars()
        elif name in ("env_update", "env_set"):
            rec["env"] = self.env()
            rec.update(self.times())
        self.log(rec)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=300.0)
    ap.add_argument("--status", default=None, help="as spawn.py --status")
    ap.add_argument("--trigger", default=None, help="as spawn.py --trigger (spawns on demand)")
    ap.add_argument("--class", dest="cls", type=int, default=5, help="spawn.py --class")
    ap.add_argument("--kind", default="normal", choices=["normal", "champion", "unique", "boss"])
    ap.add_argument("--superunique", type=int, default=None)
    ap.add_argument("--level", type=int, default=None, help="spawn.py --level")
    ap.add_argument("--packets", nargs="?", const="auto", default=None, help="spawn.py --packets")
    ap.add_argument("--footsteps", default="player", choices=["player", "all"],
                    help="footstep entries of the local player only (default) or of every unit")
    ap.add_argument("--env", action="store_true",
                    help="also log the environment per client update (queue entry 69)")
    ap.add_argument("--env-only", action="store_true", help="--env without the sound hooks")
    ap.add_argument("--out", default=None, help="output .jsonl (default traces/raw/<time>-sound.jsonl)")
    ap.add_argument("game_args", nargs="*", default=None,
                    help="Game.exe arguments (default -w -nosave -name bdAma -ama: sound on, "
                         "no -ns)")
    a = ap.parse_args()
    if a.env_only:
        a.env = True
    args = a.game_args or ["-w", "-nosave", "-name", "bdAma", "-ama"]
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-sound.jsonl")
    if a.packets == "auto":
        a.packets = (out[:-6] if out.endswith(".jsonl") else out) + "-packets.jsonl"
    a.no_inline, a.tick, a.dx, a.dy, a.spread, a.flags = True, 1, 4, 4, 4, 0
    a.after_ticks, a.force_after, a.no_force = 25, 4.0, False
    if not a.trigger:
        a.trigger = out + ".never"   # no spawn unless asked
    sp.TOOL, sp.RAW_FORMAT = TOOL, RAW_FORMAT
    r = SoundRecorder(os.path.abspath(a.game), args, out, a)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    print(f"wrote {out}")
    if r.pkt:
        print(f"wrote {a.packets} ({r.pkt.seq} message events)")
    print("records: " + "  ".join(f"{k}={v}" for k, v in sorted(r.counts.items())))
    for n in r.notes:
        print("note:", n)


if __name__ == "__main__":
    main()
