"""C->S tap for the 1.14d state recorder (specs/tools/replay-diff.md §2).

Adds the two server-side entries of the client->server path that
record_packets.py hooks (specs/tools/packets-trace.md §1 rule 3: the game
entry 0x0053F3D0, ECX = client id + message, EDX = size; the system entry
0x0053F100, same registers) to a record_tick.TickRecorder-based recorder
(record_state.py's StateRecorder) and writes one line per message into the
same state-1 file, between the snapshots:

    {"k":"c2s","f":F,"q":"game"|"sys","client":C,"size":N,"bytes":"<hex>"}

F is the frame whose drain takes the message: the last tick return's
game +0xA8 plus one (the drain before tick F, `sim/intents-events.md` §6
rule 1); null before the first tick return. state_diff.py and replay_diff.py
skip unknown kinds when they read the snapshots, so the file stays a
state-1 file.

Standard library only; `attach` needs the Windows recorder, `selftest`
runs anywhere. Our own code. Nothing here is derived from Blizzard code.
"""

import struct

C2S_GAME = 0x53F3D0     # game entry (record_packets.py HOOKS "c2s")
C2S_SYS = 0x53F100      # system entry (record_packets.py HOOKS "c2s_sys")
ENTRY_BYTES = bytes.fromhex("558bec")
TICK_RET = 0x52FD1E     # tick driver return, ESI = game (original-hooks.md §3 rule 3)
G_FRAME = 0xA8          # tick.md §2
MAX_BYTES = 0x1FC       # the drain copy (packets-trace.md §1 rule 3)


class Tap:
    """Frame tracking and the record of one message; `read(addr, n)` is the
    debuggee's memory."""

    def __init__(self):
        self.frame = None   # game +0xA8 at the last tick return
        self.game = None
        self.count = 0

    def on_tick_return(self, read, game):
        if self.game in (None, game):
            self.game = game
            self.frame = struct.unpack("<i", read(game + G_FRAME, 4))[0]

    def record(self, read, addr, ecx, edx):
        size = edx
        n = max(0, min(size, MAX_BYTES))
        client = struct.unpack("<I", read(ecx, 4))[0]
        self.count += 1
        return {"k": "c2s", "f": None if self.frame is None else self.frame + 1,
                "q": "game" if addr == C2S_GAME else "sys", "client": client, "size": size,
                "bytes": read(ecx + 4, n).hex() if n else ""}


def attach(rec, rt):
    """Arm the two entries on a record_tick.TickRecorder (`rt` = the
    record_tick module) and route their stops to the tap. Call right after
    the recorder is built, before the other layers attach (their handlers
    wrap this one) and before rec.run()."""
    for addr in (C2S_GAME, C2S_SYS):
        rt.EXPECT.setdefault(addr, ENTRY_BYTES)
    rt.EXPECT.setdefault(TICK_RET, bytes.fromhex("8b7618"))
    tap, orig = Tap(), rec.handle
    rec.c2s_tap = tap

    def handle(addr, ctx):
        r = orig(addr, ctx)
        if addr == TICK_RET and getattr(rec, "game", None) in (None, ctx.Esi):
            tap.on_tick_return(rec.read, ctx.Esi)
        elif addr in (C2S_GAME, C2S_SYS):
            rec.emit(tap.record(rec.read, addr, ctx.Ecx, ctx.Edx))
        return r

    rec.handle = handle
    return tap


def selftest():
    mem = {}

    def put(a, b):
        for i, x in enumerate(b):
            mem[a + i] = x

    def read(a, n):
        return bytes(mem.get(a + i, 0) for i in range(n))

    game, msg = 0x1000, 0x2000
    put(game + G_FRAME, struct.pack("<i", 41))
    put(msg, struct.pack("<I", 7) + bytes.fromhex("0301000200"))
    t = Tap()
    r = t.record(read, C2S_SYS, msg, 5)
    assert r["f"] is None and r["q"] == "sys", r
    t.on_tick_return(read, game)
    r = t.record(read, C2S_GAME, msg, 5)
    assert r == {"k": "c2s", "f": 42, "q": "game", "client": 7, "size": 5,
                 "bytes": "0301000200"}, r
    t.on_tick_return(read, 0x9999)  # another game: ignored
    assert t.frame == 41
    r = t.record(read, C2S_GAME, msg, 0x400)
    assert r["size"] == 0x400 and len(r["bytes"]) == 2 * MAX_BYTES, r
    assert t.count == 3
    print("c2s_tap selftest: OK")


if __name__ == "__main__":
    selftest()
