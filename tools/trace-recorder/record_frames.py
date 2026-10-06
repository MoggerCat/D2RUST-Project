"""Capture the presented frames of the original 1.14d Game.exe: at the
in-game EndScene call, read the 8-bit index framebuffer and the palette,
tie the frame to the last server tick, and log the camera and player state
the frame was drawn from.

Reuses the debugger of record_tick.py (TickRecorder: process control,
breakpoints, the server tick hook); this script only adds the EndScene
hook. Every address and offset is documented in specs/render/capture.md
(each constant below names its section). Output: one JSON line per frame
in traces/raw/<time>-frames.jsonl (format frames-raw-1, capture.md §5) and,
with --save (default), one 8-bit palettized PNG per frame in
game/captures/<time>/ (gitignored: the images show Blizzard art).

Needs the GDI driver (Game.exe -w, display type 1) at 800 x 600
(capture.md §1); any other configuration is refused per frame.

The game process is always terminated when this script ends (time limit,
Ctrl+C, any error, and kill-on-exit if the debugger dies).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import hashlib
import json
import os
import struct
import sys
import zlib

sys.dont_write_bytecode = True
import record_tick as rt  # noqa: E402  (the shared tick recorder; not modified)

TOOL = "trace-recorder record_frames 0.1.0"
FORMAT = "frames-raw-1"

# capture.md §2: hook and the in-game caller
END_SCENE = 0x4F6190
END_SCENE_BYTES = b"\x55\x8B\xEC\x83\xEC\x10"
IN_GAME_RET = 0x44CB4F            # return address of the call at 0x44CB4A in 0x44C990
# capture.md §3: framebuffer and palette (GDI driver)
FB_PTR, FB_W, FB_H = 0x7C9154, 0x7C9138, 0x7C913C
COLOR_TABLE = 0x989C40            # 256 x (B, G, R, 0)
VIDEO_TYPE, RES_MODE = 0x7C8CB0, 0x7C8CB8
GDI = 1
# capture.md §3: state
DRAW_COUNTER = 0x7A0494
PLAYER = 0x7A6A70
U_TYPE, U_MODE, U_PATH, U_CUR = 0x00, 0x10, 0x2C, 0x44
VIEW = 0x7A0640                   # view +0x04..+0x10 rect, +0x24/+0x28 tile origin
UNIT_ORIGIN_X, UNIT_ORIGIN_Y = 0x7A520C, 0x7A5208
SHIFT_X, OPEN_MODE = 0x7A5214, 0x7A5210
SHAKE_AMP, SHAKE_DX, SHAKE_DY = 0x7B9534, 0x7B9538, 0x7B8D20
CLEAR_COUNTER = 0x70F2C0


def png_bytes(width, height, pixels, palette_rgb):
    """8-bit palettized PNG (color type 3): the index bytes are stored as is."""
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
    rows = b"".join(b"\x00" + pixels[y * width:(y + 1) * width] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 3, 0, 0, 0))
            + chunk(b"PLTE", palette_rgb)
            + chunk(b"IDAT", zlib.compress(rows, 6))
            + chunk(b"IEND", b""))


def png_indices(data):
    """Inverse of png_bytes for its own output (filter 0 only): (w, h, pixels, palette)."""
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, w = 8, b"", None
    while pos < len(data):
        n = struct.unpack(">I", data[pos:pos + 4])[0]
        kind, body = data[pos + 4:pos + 8], data[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            w, h = struct.unpack(">II", body[:8])
        elif kind == b"PLTE":
            pal = body
        elif kind == b"IDAT":
            idat += body
        pos += 12 + n
    raw = zlib.decompress(idat)
    rows = [raw[y * (w + 1):(y + 1) * (w + 1)] for y in range(h)]
    assert all(r[0] == 0 for r in rows)
    return w, h, b"".join(r[1:] for r in rows), pal


class FrameRecorder(rt.TickRecorder):
    def __init__(self, exe, args, out_path, seconds, max_ticks, img_dir, every, max_frames,
                 allow_size):
        super().__init__(exe, args, out_path, seconds, 0, max_ticks)
        self.img_dir, self.every, self.max_frames = img_dir, every, max_frames
        self.allow_size = allow_size
        self.frames = 0
        self.seen = 0
        self.groups = {}  # state key -> set of index hashes (capture.md §7)

    def i32(self, addr):
        return struct.unpack("<i", self.read(addr, 4))[0]

    def state(self):
        st = {"draw": self.u32(DRAW_COUNTER), "open_mode": self.u32(OPEN_MODE),
              "shift_x": self.i32(SHIFT_X), "unit_origin": [self.i32(UNIT_ORIGIN_X),
                                                            self.i32(UNIT_ORIGIN_Y)],
              "shake": [self.u32(SHAKE_AMP), self.i32(SHAKE_DX), self.i32(SHAKE_DY)],
              "clear_counter": self.i32(CLEAR_COUNTER), "res_mode": self.u32(RES_MODE)}
        view = self.u32(VIEW)
        if view:
            st["view_rect"] = list(struct.unpack("<4i", self.read(view + 4, 16)))
            st["tile_origin"] = list(struct.unpack("<2i", self.read(view + 0x24, 8)))
        unit = self.u32(PLAYER)
        if unit:
            p = {"type": self.u32(unit + U_TYPE), "mode": self.u32(unit + U_MODE),
                 "cur": self.i32(unit + U_CUR)}
            path = self.u32(unit + U_PATH)
            if path and p["type"] == 0:
                p["fixed"] = list(struct.unpack("<2I", self.read(path, 8)))
                p["client"] = list(struct.unpack("<2i", self.read(path + 8, 8)))
            st["player"] = p
        return st

    def capture(self, ctx):
        if self.u32(ctx.Esp) != IN_GAME_RET:
            return
        self.seen += 1
        if (self.seen - 1) % self.every:
            return
        vt, w, h, base = (self.u32(VIDEO_TYPE), self.i32(FB_W), self.i32(FB_H), self.u32(FB_PTR))
        rec = {"k": "frame", "f": self.frame, "video_type": vt, "w": w, "h": h}
        if vt != GDI or not base or (not self.allow_size and (w, h) != (800, 600)):
            rec["refused"] = "needs GDI (-w) at 800x600 (capture.md §1)"
            self.emit(rec)
            return
        pixels = self.read(base, w * h)
        ct = self.read(COLOR_TABLE, 1024)
        pal = b"".join(bytes((ct[4 * i + 2], ct[4 * i + 1], ct[4 * i])) for i in range(256))
        rec["index_sha256"] = hashlib.sha256(pixels).hexdigest()
        rec["palette_sha256"] = hashlib.sha256(pal).hexdigest()
        rec.update(self.state())
        if self.img_dir:
            name = f"frame-{rec['draw']:07d}.png"
            with open(os.path.join(self.img_dir, name), "wb") as f:
                f.write(png_bytes(w, h, pixels, pal))
            rec["image"] = name
        key = json.dumps({k: rec.get(k) for k in ("player", "tile_origin", "unit_origin",
                                                   "shake", "open_mode", "palette_sha256")},
                         sort_keys=True)
        self.groups.setdefault(key, set()).add(rec["index_sha256"])
        self.emit(rec)
        self.frames += 1
        if self.max_frames and self.frames >= self.max_frames:
            self.notes.append(f"frame limit {self.max_frames} reached")
            self.done = True

    def handle(self, addr, ctx):
        if addr == END_SCENE:
            self.capture(ctx)
            return
        super().handle(addr, ctx)

    def stability(self):
        """capture.md §7: frames drawn from the same recorded state are identical."""
        multi = [s for s in self.groups.values()]
        bad = sum(1 for s in multi if len(s) > 1)
        return len(multi), bad


def selftest():
    w, h = 5, 3
    pixels = bytes((7 * i) & 0xFF for i in range(w * h))
    pal = bytes(range(256)) * 3
    w2, h2, px2, pal2 = png_indices(png_bytes(w, h, pixels, pal))
    assert (w2, h2, px2, pal2) == (w, h, pixels, pal), "PNG round trip changed the frame"
    bad = bytearray(pixels)
    bad[4] ^= 1
    assert hashlib.sha256(bytes(bad)).hexdigest() != hashlib.sha256(pixels).hexdigest()
    print("selftest ok: PNG keeps indices and palette; a 1-byte change changes the hash")


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0, help="kill the game after N s (default 120)")
    ap.add_argument("--ticks", type=int, default=0, help="stop after N server ticks (default: no limit)")
    ap.add_argument("--every", type=int, default=1, help="capture every N-th in-game frame (default 1)")
    ap.add_argument("--max-frames", type=int, default=0, help="stop after N captures (default: no limit)")
    ap.add_argument("--no-save", action="store_true", help="hashes and state only, no PNG files")
    ap.add_argument("--allow-any-size", action="store_true", help="also capture at 640x480")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-frames.jsonl)")
    ap.add_argument("--selftest", action="store_true", help="check the PNG writer and exit")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"], help="Game.exe arguments (default: -w -ns)")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = a.out or os.path.join(repo, "traces", "raw", stamp + "-frames.jsonl")
    img_dir = None if a.no_save else os.path.join(repo, "game", "captures", stamp)
    if img_dir:
        os.makedirs(img_dir, exist_ok=True)
    # keep only the base recorder's tick hook, add EndScene
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], END_SCENE: END_SCENE_BYTES}
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = FrameRecorder(os.path.abspath(a.game), a.game_args or ["-w", "-ns"], out, a.seconds,
                      a.ticks, img_dir, max(1, a.every), a.max_frames, a.allow_any_size)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    groups, bad = r.stability()
    print(f"wrote {out}: {r.frames} frames, {r.ticks} ticks; images: {img_dir or 'none'}")
    print(f"stability: {groups} state groups, {bad} with differing frames")


if __name__ == "__main__":
    main()
