"""Record the client<->server messages of the original 1.14d Game.exe.

A minimal Windows debugger (the Win32 debug loop of record_rng.py, standard
library only). It starts game/Game.exe under DEBUG_ONLY_THIS_PROCESS, plants
INT3 breakpoints on the message path of the in-process single-player server
and writes one JSON line per event to traces/raw/<time>-packets.jsonl.

What it hooks (addresses and rules: specs/sim/intents-events.md):
  * server loop markers: drain of the incoming queues (0x0052CFE0), the tick
    (0x0052D870 entry, ECX = game; frame = game+0xA8 after its increment),
    the tick's return (0x0052FD1E, 0x00564608) and the flush (0x0052FD90);
  * client->server: every game message as the server takes it from its queue
    (0x0053F3D0: ECX = client id + message, EDX = size), every system message
    (0x0053F100), the dispatcher (0x0054D750: ECX = game, EDX = player,
    [ESP+4] = message, [ESP+8] = size) and its result (0x0053F45E, EAX);
  * server->client: every message queued for a client (0x0053B280: EDI =
    client, [ESP+4] = message, [ESP+8] = size) and every buffer handed to the
    net layer (0x0052B330: [ESP+4] type, [ESP+8] client id, [ESP+0xC]
    buffer, [ESP+0x10] size);
  * the client side: the game-message sender before its duplicate filter
    (0x00478350: EDI = size, [ESP+4] = message) and what leaves the client
    (0x0052AE50: [ESP+4] = size, [ESP+0xC] = message).

Every record carries the last tick's frame number (null before the first
tick) and the loop phase; `--ticks N` ends the recording at the first
queue drain after tick N's flush (that drain is not written), as d2rs
`state-dump --ticks N --packets FILE` ends (specs/tools/packets-trace.md).

Pokes (`poke.py`, specs/tools/poke.md): `--poke "<f> <directive ...>"` and
`--poke-file FILE` run at the tick-return stop 0x0052FD1E (already hooked
here as `tick_end`), before its record, as `poke.py` runs them; their
`poke` records go into the same file. Sends (`send.py`, specs/tools/scenario-diff.md
§2 `at … send`): `--send "<f> <Name> <field>=<value>..."` / `--send "<f> hex
<bytes>"` inject C->S messages at the first stop of the drain call 0x0044F136
after tick f - 1 (specs/tools/original-hooks.md §1 rule 4); their `send`
records go into the same file, followed by the `client_out`, `c2s`,
`dispatch` and `result` records of the message. The
game process is always terminated when this script ends (time limit,
Ctrl+C, any error, and kill-on-exit if the debugger dies).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import os
import struct
import sys

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import record_rng as rr  # noqa: E402  (the shared Win32 debugger)
import autostart  # noqa: E402  (unattended start, input script)
import poke  # noqa: E402  (--poke / --poke-file: state injection)
import send  # noqa: E402  (--send: C->S message injection)

TOOL = "trace-recorder record_packets 0.3.0"
RAW_FORMAT = "packets-raw-1"
MAX_BYTES = 0x204  # largest message the net layer accepts (spec §3)

# address -> (event name, first bytes expected in the 1.14d Game.exe)
HOOKS = {
    0x52CFE0: ("drain", "558bec"),
    0x53F3D0: ("c2s", "558bec"),
    0x53F100: ("c2s_sys", "558bec"),
    0x54D750: ("dispatch", "558bec"),
    0x53F45E: ("result", "8bcee8"),
    0x52D870: ("tick", "535657"),
    0x52FD1E: ("tick_end", "8b7618"),
    0x564608: ("tick_end", "33d28b"),
    0x52FD90: ("flush", "558bec"),
    0x53B280: ("s2c", "558bec"),
    0x52B330: ("net", "558bec"),
    0x478350: ("client_send", "558bec"),
    0x52AE50: ("client_out", "558bec"),
}
PHASE_AFTER = {"drain": "input", "tick": "tick", "tick_end": "post", "flush": "flush"}


class _Notes(list):
    """The shared debug loop logs its RNG hook counts; they do not apply here."""

    def append(self, s):
        if "helpers," in s and "setters," in s:
            return
        super().append(s)


class PacketRecorder(rr.Recorder):
    def __init__(self, exe, args, out, seconds, max_events, max_ticks=0):
        super().__init__(exe, args, out, seconds, False, max_events)
        self.notes = _Notes()
        self.frame = None
        self.phase = "start"
        self.last_kind = None
        self.max_ticks, self.ticks = max_ticks, 0
        self.poke_layer = None
        self.poke_tid = None
        self.send_layer = None

    def install(self, base):
        if base != rr.IMAGE_BASE:
            raise RuntimeError(f"Game.exe loaded at {base:#x}, expected {rr.IMAGE_BASE:#x}")
        for addr, (_, first) in HOOKS.items():
            want = bytes.fromhex(first)
            if self.read(addr, len(want)) != want:
                raise RuntimeError(f"unexpected code at {addr:#x}: not the 1.14d Game.exe?")
        for addr in HOOKS:
            self.add_role(addr, "pkt")
        self.notes.append(f"{len(HOOKS)} message hooks")
        if self.send_layer is not None:  # the injection stop (original-hooks.md §1 rule 4)
            if self.read(send.DRAIN_CALL, 5) != send.DRAIN_CALL_BYTES:
                raise RuntimeError("unexpected code at 0x0044F136: not the 1.14d Game.exe?")
            self.add_role(send.DRAIN_CALL, "send")
        return 0

    def on_breakpoint(self, tid, addr):
        if "pkt" in self.roles.get(addr, ()):
            self.on_hook(tid, addr, self.get_ctx(tid))
        if "send" in self.roles.get(addr, ()):
            self.send_layer.on_drain_call(self, tid)
        super().on_breakpoint(tid, addr)  # steps over the original instruction

    def emit(self, rec):
        if "type" not in rec and "k" in rec:  # poke.py records name their kind `k`
            rec = dict(rec, type=rec["k"])
        super().emit(rec)

    def blob(self, ptr, size):
        n = max(0, min(size, MAX_BYTES))
        return self.read(ptr, n).hex() if n else ""

    def on_hook(self, tid, addr, ctx):
        kind = HOOKS[addr][0]
        if kind == "drain" and self.max_ticks and self.ticks >= self.max_ticks:
            if not self.max_events or self.max_events > self.seq:
                self.notes.append(f"tick limit {self.max_ticks} reached")
                self.max_events = max(1, self.seq)  # the debug loop stops before the next event
            return
        if addr == poke.TICK_RET and self.poke_layer is not None:
            self.poke_tid = tid
            frame = struct.unpack("<i", self.read(ctx.Esi + poke.G_FRAME, 4))[0]
            self.poke_layer.on_tick_return(self, ctx.Esi, frame, tid, ctx)
        if addr == poke.TICK_RET and self.send_layer is not None:
            self.send_layer.on_tick_return(self, ctx.Esi)
        if addr == poke.TICK_RET and self.auto is not None and self.auto.has_frames():
            # `frame F` input steps (autostart.py): posted at the tick return of F - 1,
            # after the pokes, as record_state.py does through AutoStart.attach
            frame = struct.unpack("<i", self.read(ctx.Esi + poke.G_FRAME, 4))[0]
            self.auto.on_tick_return(self, frame)
        esp = ctx.Esp
        arg = lambda k: self.read_u32(esp + 4 * k)  # noqa: E731  ([ESP+4k])
        rec = {"type": kind, "tid": tid}
        if kind == "tick":
            self.ticks += 1
            self.frame = (self.read_u32(ctx.Ecx + 0xA8) + 1) & rr.M32
            rec["game"] = f"{ctx.Ecx:#x}"
            rec["game_type"] = self.read(ctx.Ecx + 0x6A, 1)[0]  # spec §3.2 rule 5
        elif kind in ("c2s", "c2s_sys"):
            size = ctx.Edx
            rec["client"] = self.read_u32(ctx.Ecx)
            rec["size"] = size
            rec["bytes"] = self.blob(ctx.Ecx + 4, min(size, 0x1FC))  # drain buffer is 0x200
        elif kind == "dispatch":
            size = arg(2)
            rec["game_frame"] = self.read_u32(ctx.Ecx + 0xA8)
            rec["unit"] = self.read_u32(ctx.Edx + 0x0C)
            rec["size"] = size
            rec["id"] = self.read(arg(1), 1)[0]
        elif kind == "result":
            rec["dispatched"] = self.last_kind == "dispatch"
            if rec["dispatched"]:
                rec["code"] = ctx.Eax
        elif kind == "s2c":
            size = arg(2)
            rec["client"] = self.read_u32(ctx.Edi)
            rec["size"] = size
            rec["bytes"] = self.blob(arg(1), size)
            rec["caller"] = f"{self.call_site(arg(0)):#x}"
        elif kind == "net":
            size = arg(4)
            rec["kind"] = arg(1) & 0xFF
            rec["client"] = arg(2)
            rec["size"] = size
            rec["bytes"] = self.blob(arg(3), size)
            rec["caller"] = f"{self.call_site(arg(0)):#x}"
        elif kind == "client_send":
            size = ctx.Edi
            rec["size"] = size
            rec["bytes"] = self.blob(arg(1), size)
        elif kind == "client_out":
            size = arg(1) & 0xFFFF
            rec["size"] = size
            rec["bytes"] = self.blob(arg(3), size)
        self.last_kind = kind
        self.phase = PHASE_AFTER.get(kind, self.phase)
        rec["frame"] = self.frame
        rec["phase"] = self.phase
        self.emit(rec)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0,
                    help="stop and kill the game after this many seconds (default 120)")
    ap.add_argument("--max-events", type=int, default=0, help="stop after N events (0 = no limit)")
    ap.add_argument("--ticks", type=int, default=0,
                    help="stop at the first queue drain after N server ticks (0 = no limit)")
    ap.add_argument("--out", default=None,
                    help="output .jsonl (default traces/raw/<time>-packets.jsonl)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"],
                    help="Game.exe arguments (default: -w -ns)")
    autostart.add_options(ap)
    poke.add_options(ap)
    send.add_options(ap)
    a = ap.parse_args()
    layer = poke.PokeLayer.from_args(a)
    sends = send.SendLayer.from_args(a)
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-packets.jsonl")
    rr.TOOL, rr.RAW_FORMAT = TOOL, RAW_FORMAT  # header written by the shared run()
    r = PacketRecorder(os.path.abspath(a.game), gargs, out, a.seconds,
                       a.max_events, a.ticks)
    r.auto = auto
    r.poke_layer = layer
    if layer is not None:
        r.notes.append(f"pokes: {len(layer.pending())} directive(s)")
    r.send_layer = sends
    if sends is not None:
        r.notes.append(f"sends: {len(sends.pending())} message(s)")
    try:
        counts = r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
        counts = r.counts
    print(f"wrote {out}")
    print(f"events: {r.seq}  " + "  ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    for n in r.notes:
        print("note:", n)
    send.print_results(sends)


if __name__ == "__main__":
    main()
