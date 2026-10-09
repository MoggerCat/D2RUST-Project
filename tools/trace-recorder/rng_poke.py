"""record_rng.py's draw log with pokes and frame markers: the 1.14d RNG
draws after a state injection (`poke.py`), for the `rng` channel of a
poked check (specs/tools/rng-trace.md §6 r3) and for finding the first
draw that differs from d2rs at a scenario_diff divergence.

    wine python.exe tools/trace-recorder/rng_poke.py --game GAME.EXE --seconds N \
        [--frames [--ticks N]] [--keep-frames A-B] [--poke "F directive ..."] [--forms FILE] \
        [autostart options] [--out FILE]

Uses record_rng.py's Recorder unchanged plus the tick-return stop
0x0052FD1E (ESI = game): there the pokes of the next frame run (as in
record_state.py) and a `frame_end` record {frame: game +0xA8} is written,
so each draw belongs to the frame whose `frame_end` follows it.
--frames and --ticks are record_rng.py's (tick markers, owners).
--keep-frames A-B keeps the draw records of frames A..B only (the inline
hooks stay armed; the others are dropped when written). Our own code.
"""

import argparse
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import record_rng as rr  # noqa: E402
import autostart  # noqa: E402
import poke  # noqa: E402

TOOL = "trace-recorder rng_poke 0.2.0"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", required=True)
    ap.add_argument("--seconds", type=float, default=120)
    ap.add_argument("--frames", action="store_true",
                    help="record_rng.py --frames: tick markers, frames and owners")
    ap.add_argument("--ticks", type=int, default=0,
                    help="with --frames: stop at the entry of tick N + 1 (0 = no limit)")
    ap.add_argument("--keep-frames", default=None, help="A-B: keep draws of these frames only")
    ap.add_argument("--out", default=os.path.join(HERE, "..", "..", "traces", "raw", "rng-poke.jsonl"))
    autostart.add_options(ap)
    poke.add_options(ap)
    a = ap.parse_args()
    if a.ticks and not a.frames:
        ap.error("--ticks needs --frames")
    gargs, auto = autostart.setup(a, ["-w", "-ns"])
    layer = poke.PokeLayer.from_args(a)
    lo, hi = (int(x) for x in a.keep_frames.split("-")) if a.keep_frames else (None, None)
    rr.TOOL = TOOL
    state = {"frame": 0}

    orig_install = rr.Recorder.install

    def install(self, base):
        n = orig_install(self, base)
        if self.read(poke.TICK_RET, 3) != poke.TICK_RET_BYTES:
            raise RuntimeError("unexpected code at 0x0052FD1E: not the 1.14d Game.exe?")
        self.add_role(poke.TICK_RET, "tickret")
        return n
    rr.Recorder.install = install

    orig_emit = rr.Recorder.emit

    def emit(self, rec):
        if "type" not in rec and "k" in rec:
            rec = dict(rec, type=rec["k"])
        t = rec.get("type")
        if lo is not None and t not in ("header", "footer", "tick", "frame_end", "poke", "poke_f0"):
            nxt = state["frame"] + 1   # the frame being run
            if not lo <= nxt <= hi:
                return
        orig_emit(self, rec)
    rr.Recorder.emit = emit

    orig_bp = rr.Recorder.on_breakpoint

    def on_breakpoint(self, tid, addr):
        if "tickret" in self.roles.get(addr, ()):
            ctx = self.get_ctx(tid)
            game = ctx.Esi
            frame = struct.unpack("<i", self.read(game + poke.G_FRAME, 4))[0]
            state["frame"] = frame
            self.emit({"type": "frame_end", "frame": frame})
            if layer is not None:
                self.poke_tid = tid
                layer.on_tick_return(self, game, frame, tid, ctx)
        return orig_bp(self, tid, addr)
    rr.Recorder.on_breakpoint = on_breakpoint

    r = rr.Recorder(os.path.abspath(a.game), gargs, a.out, a.seconds, True, 0)
    r.auto = auto
    r.poke_tid = None
    r.frames, r.max_ticks = a.frames, a.ticks
    try:
        r.run()
    except KeyboardInterrupt:
        pass
    if a.frames:
        import rng_owners
        summary = rng_owners.assign_file(a.out)
        print("owners:", "  ".join(f"{k}={v}" for k, v in sorted(summary.items())))
    print(f"wrote {a.out}")
    for n in r.notes:
        print("note:", n)


if __name__ == "__main__":
    sys.exit(main())
