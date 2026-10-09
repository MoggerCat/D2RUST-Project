"""Measures how 1.14d calls a function: at each hit of --at ADDR (function
entry) logs the registers, the return address and the first --args stack
words; a one-shot breakpoint at that return address then logs EAX and how
far ESP moved (4 = plain `ret`, 4 + 4n = `ret 4n`: n stack arguments the
callee pops). For call forms the specs leave open (e.g. the level warp
0x0053AEC0, specs/tools/poke.md open question 2), measured on calls the
game makes itself.

    wine python.exe tools/cloud-game/probe_call.py --game GAME.EXE --at 0x53AEC0 \
        [--args 6] [--seconds N] [autostart options] [--out FILE]

Uses record_rng.py's debugger unchanged. One JSON line per entry / return
(format probe-call-1). Our own code.
"""

import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "trace-recorder"))
import record_rng as rr  # noqa: E402
import autostart  # noqa: E402

TOOL = "cloud-game probe_call 0.1.0"
FORMAT = "probe-call-1"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", required=True)
    ap.add_argument("--at", required=True, help="function entry address(es), comma-separated")
    ap.add_argument("--args", type=int, default=6, help="stack words to log at entry")
    ap.add_argument("--seconds", type=float, default=120)
    ap.add_argument("--out", default=os.path.join(HERE, "..", "..", "traces", "raw", "probe-call.jsonl"))
    autostart.add_options(ap)
    a = ap.parse_args()
    gargs, auto = autostart.setup(a, ["-w", "-ns"])
    entries = [int(x, 0) for x in a.at.split(",")]
    rr.HELPERS, rr.SETTERS = {}, {}
    rr.TOOL, rr.RAW_FORMAT = TOOL, FORMAT
    pending = {}  # return address -> list of (tid, esp at entry, entry)

    def install(self, base):
        for e in entries:
            self.add_role(e, "probe")
        self.notes.append(f"probing {', '.join(hex(e) for e in entries)}")
        return 0
    rr.Recorder.install = install

    orig_bp = rr.Recorder.on_breakpoint

    def on_breakpoint(self, tid, addr):
        roles = self.roles.get(addr, ())
        if "probe" in roles or "probe_ret" in roles:
            ctx = self.get_ctx(tid)
            if "probe" in roles:
                ret = self.read_u32(ctx.Esp)
                words = [self.read_u32(ctx.Esp + 4 * (i + 1)) for i in range(a.args)]
                self.emit({"type": "entry", "at": f"{addr:#x}", "tid": tid, "ret": f"{ret:#x}",
                           "eax": f"{ctx.Eax:#x}", "ecx": f"{ctx.Ecx:#x}", "edx": f"{ctx.Edx:#x}",
                           "ebx": f"{ctx.Ebx:#x}", "esi": f"{ctx.Esi:#x}", "edi": f"{ctx.Edi:#x}",
                           "esp": f"{ctx.Esp:#x}", "stack": [f"{w:#x}" for w in words]})
                pending.setdefault(ret, []).append((tid, ctx.Esp, addr))
                self.add_role(ret, "probe_ret")
            if "probe_ret" in roles:
                waits = pending.get(addr, [])
                for i, (t, esp0, e) in enumerate(waits):
                    if t == tid:
                        self.emit({"type": "return", "at": f"{e:#x}", "tid": tid, "ret": f"{addr:#x}",
                                   "eax": f"{ctx.Eax:#x}", "esp_moved": ctx.Esp - esp0})
                        del waits[i]
                        break
        return orig_bp(self, tid, addr)
    rr.Recorder.on_breakpoint = on_breakpoint

    r = rr.Recorder(os.path.abspath(a.game), gargs, a.out, a.seconds, False, 0)
    r.auto = auto
    try:
        r.run()
    except KeyboardInterrupt:
        pass
    print(f"wrote {a.out}")
    for n in r.notes:
        print("note:", n)


if __name__ == "__main__":
    sys.exit(main())
