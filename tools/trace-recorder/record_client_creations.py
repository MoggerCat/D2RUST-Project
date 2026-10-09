"""Record every client-only creation and each critter's first think timer on 1.14d.

Hooks (specs/client/model.md §5 r6.3-6.4): the creator entry 0x00466730
(ECX class, EDX x, [esp+4] y, [esp+8] type, [esp+12] mode; the new GUID is
[0x00711F30] + 1) and 0x0046D794 in the critter AI (after 0x004AE110: EAX =
&think timer, ESI = unit), logged once per unit. Output: one JSON record per
line, format `client-creations-1`: a header, then `create` {guid, ret (the
return address: the creating pass), cls, x, y, type, mode, f (server frame)}
and `think0` {guid, cls, T, f}.

  py tools/trace-recorder/record_client_creations.py --auto ScnAma --seed 1234 --seconds 45 --out FILE
"""
import argparse
import datetime
import json
import os
import struct
import sys

import autostart
import record_tick as rt

FORMAT, TOOL = "client-creations-1", "trace-recorder record_client_creations 0.1.0"
CREATE, THINK, COUNTER = 0x466730, 0x46D794, 0x711F30


class CreationRecorder(rt.TickRecorder):
    log = None
    seen = None

    def handle(self, addr, ctx):
        if addr == CREATE:
            ret, y, typ, mode = struct.unpack("<4I", self.read(ctx.Esp, 16))
            guid = (self.read_u32(COUNTER) + 1) & 0xFFFFFFFF
            guid = 0 if guid == 0xFFFFFFFF else guid
            self.log.append({"k": "create", "guid": guid, "ret": f"{ret:#x}", "cls": ctx.Ecx,
                             "x": ctx.Edx, "y": y, "type": typ, "mode": mode, "f": self.frame})
        elif addr == THINK:
            g = self.read_u32(ctx.Esi + 0x0C)
            if g not in self.seen:
                self.seen.add(g)
                self.log.append({"k": "think0", "guid": g, "cls": self.read_u32(ctx.Esi + 4),
                                 "T": self.read_u32(ctx.Eax), "f": self.frame})
        else:
            super().handle(addr, ctx)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=45.0)
    ap.add_argument("--out", required=True)
    autostart.add_options(ap)
    a = ap.parse_args()
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], CREATE: b"\x55\x8B\xEC", THINK: b"\x8B\x08"}
    gargs, auto = autostart.setup(a, ["-w", "-ns"])
    r = CreationRecorder(os.path.abspath(a.game), gargs, a.out + ".tick.jsonl", a.seconds, 0, 0)
    r.auto, r.log, r.seen = auto, [], set()
    try:
        r.run()
    finally:
        with open(a.out, "w", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps({"k": "header", "format": FORMAT, "tool": TOOL,
                                "date": datetime.date.today().isoformat(), "args": gargs,
                                "command": " ".join(["record_client_creations.py"] + sys.argv[1:])}) + "\n")
            for x in r.log:
                f.write(json.dumps(x) + "\n")
        os.remove(a.out + ".tick.jsonl") if os.path.exists(a.out + ".tick.jsonl") else None
    for n in r.notes:
        print("note:", n)
    print(f"wrote {a.out}: {len(r.log)} records")


if __name__ == "__main__":
    main()
