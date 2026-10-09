"""Which code path builds each DRLG room of 1.14d (drlg/rooms.md §4).

    wine python.exe tools/cloud-game/probe_room_builds.py --game GAME.EXE [--seconds N] [autostart options]

Uses record_rng.py's debugger unchanged with one hook: the seed setter
init_low (0x650E40) when called from the room seed reset 0x66EE49
(rooms.md §4.4 step 2: room seed := init_low(dwInitSeed)). At that hit,
ECX - 0x14 is the room (seed at room +0x14, dwInitSeed at +0x04); the
EBP chain gives the return addresses of the callers (the build 0x0061B190
/ the stream 0x0061B730, and above them set handler 1 0x0061B2D0, the
client build timer 0x0061B920, the 0x07 in-sight path 0x0061B640, ...).
Prints one line per room build: seq, ms, thread, dwInitSeed, room
pointer, caller chain. Our own code.
"""

import argparse
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "trace-recorder"))
import record_rng as rr  # noqa: E402
import autostart  # noqa: E402

RESET_RET = 0x66EE4E  # return address of the init_low call at 0x66EE49


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--game", required=True)
    ap.add_argument("--seconds", type=float, default=120)
    ap.add_argument("--out", default=os.path.join(HERE, "..", "..", "traces", "raw", "probe-room-builds.jsonl"))
    autostart.add_options(ap)
    a = ap.parse_args()
    gargs, auto = autostart.setup(a, ["-w", "-ns"])
    rr.HELPERS = {}
    rr.SETTERS = {0x650E40: "init_low"}
    builds = []

    def install(self, base):
        self.add_role(0x650E40, "setter")
        return 0
    rr.Recorder.install = install

    def on_setter(self, tid, addr, ctx):
        if self.read_u32(ctx.Esp) != RESET_RET:
            return
        room = ctx.Ecx - 0x14
        chain, ebp = [], ctx.Ebp
        for _ in range(8):
            if not ebp:
                break
            try:
                nxt, ret = struct.unpack("<II", self.read(ebp, 8))
            except Exception:
                break
            chain.append(f"{ret:#x}")
            if nxt <= ebp:
                break
            ebp = nxt
        builds.append((self.seq, tid, self.read_u32(room + 4), room, chain))
        self.seq += 1
        print(f"BUILD seq {self.seq} tid {tid} init {self.read_u32(room + 4)} room {room:#x} "
              f"callers {' '.join(chain)}", flush=True)
    rr.Recorder.on_setter = on_setter

    r = rr.Recorder(os.path.abspath(a.game), gargs, a.out, a.seconds, False, 0)
    r.auto = auto
    rr.GAME_EXE_SHA256 = rr.GAME_EXE_SHA256  # the reference check stays on
    r.run()
    for n in r.notes:
        print("note:", n)
    print(f"{len(builds)} room builds")


if __name__ == "__main__":
    main()
