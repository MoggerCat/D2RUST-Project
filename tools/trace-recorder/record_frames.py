"""Capture the presented frames of the original 1.14d Game.exe: at the
in-game EndScene call, read the 8-bit index framebuffer and the palette,
tie the frame to the last server tick, and log the state the frame was
drawn from: camera and player, level and act, the mouse cursor, the
player's client seed, light quality, weather and, on selected frames, every
draw call of the frame (tiles, units, UI cels and primitives).

Reuses the debugger of record_tick.py (TickRecorder: process control,
breakpoints, the server tick hook); this script adds the in-game draw
entry, EndScene, the cel-file loader and the draw-call hooks. Every address
and offset is documented in specs/render/capture.md (each constant below
names its section). Output: one JSON line per frame in
traces/raw/<time>-frames.jsonl (format frames-raw-3, capture.md §5) and,
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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import autostart  # noqa: E402  (unattended start, input script)
import poke  # noqa: E402  (--poke / --poke-file: state injection, specs/tools/poke.md §2 rule 6)

sys.dont_write_bytecode = True

TOOL = "trace-recorder record_frames 0.3.0"
FORMAT = "frames-raw-3"

# capture.md §2: hooks and the in-game caller
END_SCENE = 0x4F6190
END_SCENE_BYTES = b"\x55\x8B\xEC\x83\xEC\x10"
IN_GAME_RET = 0x44CB4F            # return address of the call at 0x44CB4A in 0x44C990
FRAME_START = 0x44C990            # in-game draw entry (capture.md §2)
FRAME_START_BYTES = b"\x55\x8B\xEC\x83\xEC\x1C"
CEL_LOADED = 0x478946             # cel-file loader 0x4788B0 after the load (capture.md §3.6)
CEL_LOADED_BYTES = b"\x8B\x45\xFC"
UNIT_DRAW = 0x471EC0              # one unit draw: ECX unit, EDX light, stack px, py, a3, a4 (§3.5)
UNIT_DRAW_BYTES = b"\x55\x8B\xEC\x83\xEC\x70"
# capture.md §3.5: D2GFX draw wrappers (stdcall, every argument on the stack): name, argument count
DRAWS = {
    0x4F60F0: ("StartDraw", 4), 0x4F63B0: ("ClearScreen", 1),
    0x4F68E0: ("FloorTileDraw", 9), 0x4F6920: ("TileDrawLit", 5), 0x4F6950: ("TileDrawTrans", 6),
    0x4F6980: ("ShadowTileDraw", 5),
    0x4F6450: ("CelFlatSpriteDraw", 7), 0x4F6480: ("CelDraw", 6), 0x4F64B0: ("CelDrawColor", 6),
    0x4F64E0: ("CelDrawEx", 6), 0x4F6510: ("CelDrawClipped", 5), 0x4F6540: ("CelDrawShadow", 3),
    0x4F6570: ("CelDrawHilight", 4),
    0x4F6280: ("UtilDiamond", 2), 0x4F62A0: ("UtilRect", 2), 0x4F6300: ("DrawBox", 6),
    0x4F6340: ("DrawBoxAlpha", 6), 0x4F6380: ("DrawLine", 6),
}
TILE_OPS = {"FloorTileDraw", "TileDrawLit", "TileDrawTrans", "ShadowTileDraw"}
CEL_OPS = {"CelFlatSpriteDraw", "CelDraw", "CelDrawColor", "CelDrawEx", "CelDrawClipped",
           "CelDrawShadow", "CelDrawHilight"}
RECT_OPS = {"UtilDiamond", "UtilRect"}
CEL_CONTEXT_SIZE = 0x48
# capture.md §3.5 (raw-3): the common cel rasterizer 0x6014C0 (sprite-placement.md §1), stdcall,
# [ESP+4] = the cel context; the cel it draws is the context's +0x3C (orientation word, w, h,
# xoff, yoff: the dc6.md §Frame layout)
RASTER = 0x6014C0
RASTER_BYTES = b"\x55\x8B\xEC\x51\x8B\x4D\x08"
CTX_CEL = 0x3C
# capture.md §3.6 (raw-3): unit component files, right after the path is written by 0x5FE610
# (unit-composite.md §6 r1-r2): EBX = the path buffer, [EBP+8] = the composed name
COMP_PATH_DCC, COMP_PATH_DCC_BYTES = 0x5FE77C, b"\x83\x4E\x44\x01"
COMP_PATH_DC6, COMP_PATH_DC6_BYTES = 0x5FE7A8, b"\xB8\x01\x00\x00\x00"
# capture.md §3.5: loaded DT1 list (0x110-byte records: path, +0x104 library, +0x10C next);
# library: tile count +0x10C, tile array +0x110, 0x60 bytes per tile
DT1_LIST = 0x8ADBB4
DT1_TILE_SIZE = 0x60
# capture.md §3.1: framebuffer and palette (GDI driver)
FB_PTR, FB_W, FB_H = 0x7C9154, 0x7C9138, 0x7C913C
COLOR_TABLE = 0x989C40            # 256 x (B, G, R, 0)
VIDEO_TYPE, RES_MODE = 0x7C8CB0, 0x7C8CB8
GDI = 1
# capture.md §3.1: camera state
DRAW_COUNTER = 0x7A0494
CLIENT_UPDATES = 0x7A0498
PLAYER = 0x7A6A70
U_TYPE, U_CLASS, U_GUID, U_MODE, U_SEED, U_PATH = 0x00, 0x04, 0x0C, 0x10, 0x20, 0x2C
U_SEQ, U_SEQ_MODE, U_CUR, U_FC, U_GFX, U_FLAGS, U_FLAGS2 = 0x30, 0x40, 0x44, 0x48, 0x54, 0xC4, 0xC8
VIEW = 0x7A0640                   # view +0x04..+0x10 rect, +0x24/+0x28 tile origin
UNIT_ORIGIN_X, UNIT_ORIGIN_Y = 0x7A520C, 0x7A5208
SHIFT_X, OPEN_MODE = 0x7A5214, 0x7A5210
SHAKE_AMP, SHAKE_DX, SHAKE_DY = 0x7B9534, 0x7B9538, 0x7B8D20
CLEAR_COUNTER = 0x70F2C0
# capture.md §3.2: level and act
ACT = 0x7A0634                    # client act: +0x04 environment, +0x14 act number
# capture.md §3.3: cursor
CUR_VISIBLE, CUR_STATE, CUR_TYPE, CUR_FRAME = 0x7A6B08, 0x7A6AF0, 0x7A6ADC, 0x7A6AE0
CUR_X, CUR_Y, CUR_ADJ, CUR_ITEM = 0x7A6AB0, 0x7A6AAC, 0x7A6AB4, 0x7A6ABC
CUR_LAST_STEP, CUR_IDLE_SINCE = 0x7A6AEC, 0x7A6AE8
# capture.md §3.4: light quality and weather
LIGHT_QUALITY, DRAW_RATE, LIGHT_OPT_A, LIGHT_OPT_B, RENDER_KIND = (0x7B567C, 0x7A04A8, 0x72DA50,
                                                                  0x72A348, 0x712CCC)
RAIN_ON, SNOW_ON, LIGHTNING, FLASH, WEATHER_UPDATE = 0x7A8A44, 0x7A8A40, 0x7A89E8, 0x7BB390, 0x7A8A0C


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


# --- state readers (capture.md §3); `mem` has read(addr, n) ------------------

def u32(mem, a):
    return struct.unpack("<I", mem.read(a, 4))[0]


def i32(mem, a):
    return struct.unpack("<i", mem.read(a, 4))[0]


def u8(mem, a):
    return mem.read(a, 1)[0]


def text4(raw):
    """A 4-byte token field as text when printable, else None."""
    s = raw.split(b"\0")[0]
    return s.decode("latin-1") if s and all(32 <= c < 127 for c in s) else None


def read_seed(mem, unit):
    return [u32(mem, unit + U_SEED), u32(mem, unit + U_SEED + 4)] if unit else None


def read_level(mem, unit):
    """capture.md §3.2: the player's level id through path -> room -> level, and the act."""
    out = {}
    if unit:
        path = u32(mem, unit + U_PATH)
        room = (u32(mem, path) if u32(mem, unit) in (2, 4, 5) else u32(mem, path + 0x1C)) if path else 0
        drlg = u32(mem, room + 0x10) if room else 0
        level = u32(mem, drlg + 0x58) if drlg else 0
        out["level_id"] = u32(mem, level + 0x1D0) if level else None
    act = u32(mem, ACT)
    if act:
        out["act"] = u8(mem, act + 0x14)
        env = u32(mem, act + 4)
        if env:
            out["env"] = [i32(mem, env + 0x0C)] + list(mem.read(env + 0x18, 3))
    return out


def read_cursor(mem):
    """capture.md §3.3: cursor state as the coming draw will use it."""
    return {"visible": u32(mem, CUR_VISIBLE), "state": i32(mem, CUR_STATE), "type": i32(mem, CUR_TYPE),
            "frame": i32(mem, CUR_FRAME), "x": i32(mem, CUR_X), "y": i32(mem, CUR_Y),
            "adj": i32(mem, CUR_ADJ), "item": u32(mem, CUR_ITEM) != 0,
            "last_step": u32(mem, CUR_LAST_STEP), "idle_since": u32(mem, CUR_IDLE_SINCE)}


def read_light(mem):
    """capture.md §3.4: light quality and its inputs (wall-clock dependent)."""
    return {"quality": i32(mem, LIGHT_QUALITY), "draw_rate": i32(mem, DRAW_RATE),
            "opt_a": u32(mem, LIGHT_OPT_A), "opt_b": u32(mem, LIGHT_OPT_B),
            "render_kind": u32(mem, RENDER_KIND)}


def read_weather(mem):
    return {"rain": u32(mem, RAIN_ON), "snow": u32(mem, SNOW_ON), "lightning": i32(mem, LIGHTNING),
            "flash": i32(mem, FLASH), "update": u32(mem, WEATHER_UPDATE)}


def read_unit(mem, unit):
    """capture.md §3.5: the raw unit fields a unit draw reads (meaning: unit-composite.md)."""
    t = u32(mem, unit + U_TYPE)
    rec = {"type": t, "class": u32(mem, unit + U_CLASS), "guid": u32(mem, unit + U_GUID),
           "mode": u32(mem, unit + U_MODE), "seq": u32(mem, unit + U_SEQ) != 0,
           "seq_mode": u32(mem, unit + U_SEQ_MODE), "cur": i32(mem, unit + U_CUR),
           "fc": i32(mem, unit + U_FC), "flags": u32(mem, unit + U_FLAGS),
           "flags2": u32(mem, unit + U_FLAGS2)}
    path = u32(mem, unit + U_PATH)
    if path and t in (2, 4, 5):
        rec["client"] = [i32(mem, path + 4), i32(mem, path + 8)]
        rec["subtile"] = [i32(mem, path + 0x0C), i32(mem, path + 0x10)]
        rec["dir"] = u8(mem, path + 0x1C)
    elif path:
        rec["fixed"] = [u32(mem, path), u32(mem, path + 4)]
        rec["client"] = [i32(mem, path + 8), i32(mem, path + 0x0C)]
        rec["dir"] = u8(mem, path + 0x64)
    gfx = u32(mem, unit + U_GFX)
    sub = u32(mem, gfx + 0x30) if gfx else 0
    rec["extra"] = [i32(mem, sub + 0x34), i32(mem, sub + 0x38), i32(mem, sub + 0x3C)] if sub else [0, 0, 0]
    return rec


def read_tile(mem, tile):
    """capture.md §3.5: tile header fields (layout: formats/dt1.md §Tile header)."""
    return {"ptr": f"{tile:#x}", "roof": struct.unpack("<H", mem.read(tile + 4, 2))[0],
            "orient": u32(mem, tile + 0x14), "main": u32(mem, tile + 0x18),
            "sub": u32(mem, tile + 0x1C), "rarity": u32(mem, tile + 0x20)}


def read_cel_header(mem, cel):
    """The cel the rasterizer draws (dc6.md §Frame layout): orientation word, w, h, xoff, yoff."""
    flip, w, h, xoff, yoff = struct.unpack("<Iiiii", mem.read(cel, 20))
    return {"flip": flip, "w": w, "h": h, "xoff": xoff, "yoff": yoff}


def read_dt1_list(mem, limit=4096):
    """capture.md §3.5: every loaded DT1 as (tile array, count, path)."""
    out, rec, seen = [], u32(mem, DT1_LIST), set()
    while rec and rec not in seen and len(out) < limit:
        seen.add(rec)
        path = mem.read(rec, 260).split(b"\0")[0].decode("latin-1")
        lib = u32(mem, rec + 0x104)
        if lib:
            out.append((u32(mem, lib + 0x110), u32(mem, lib + 0x10C), path))
        rec = u32(mem, rec + 0x10C)
    return out


def dt1_lookup(libs, tile):
    """(path, tile index) of a drawn tile header, or None."""
    for tiles, count, path in libs:
        if tiles <= tile < tiles + DT1_TILE_SIZE * count:
            return path, (tile - tiles) // DT1_TILE_SIZE
    return None


def read_cel_context(mem, ctx):
    raw = mem.read(ctx, CEL_CONTEXT_SIZE)
    return {"raw": raw.hex(), "ctx": f"{ctx:#x}", "frame": struct.unpack_from("<i", raw, 0)[0],
            "dir": struct.unpack_from("<i", raw, 0x40)[0],
            "file": f"{struct.unpack_from('<I', raw, 0x34)[0]:#x}",
            "tokens": [text4(raw[o:o + 4]) for o in (0x18, 0x1C, 0x20, 0x24, 0x28)]}


def read_draw(mem, name, args, light_full=False, libs=None):
    """One draw call: arguments plus the record each pointer argument names. `libs`
    (read_dt1_list) names a tile's DT1 file and index (raw-3)."""
    rec = {"op": name}
    if name in TILE_OPS:
        rec["tile"] = read_tile(mem, args[0]) if args[0] else None
        hit = dt1_lookup(libs, args[0]) if (libs is not None and args[0]) else None
        if rec["tile"] is not None and libs is not None:
            rec["tile"]["dt1"] = hit[0] if hit else None
            rec["tile"]["index"] = hit[1] if hit else None
        rec["a"] = [struct.unpack("<i", struct.pack("<I", v))[0] for v in args[1:]]
        light = args[1] if name == "FloorTileDraw" else (args[3] if name != "ShadowTileDraw" else 0)
        if light:
            raw = mem.read(light, 768 if name == "FloorTileDraw" else 32)
            rec["light"] = raw.hex() if light_full else hashlib.sha256(raw).hexdigest()[:16]
    elif name in CEL_OPS:
        rec["cel"] = read_cel_context(mem, args[0]) if args[0] else None
        rec["a"] = [struct.unpack("<i", struct.pack("<I", v))[0] for v in args[1:]]
    elif name in RECT_OPS:
        rec["rect"] = list(struct.unpack("<4i", mem.read(args[0], 16))) if args[0] else None
        rec["a"] = args[1:]
    else:
        rec["a"] = [struct.unpack("<i", struct.pack("<I", v))[0] for v in args]
    return rec


def stability(records):
    """capture.md §7: frames with equal state keys must have equal index frames.
    Returns (keys seen at least twice, frames in them, keys with differing frames, verdict)."""
    groups = {}
    for r in records:
        if "index_sha256" not in r:
            continue
        key = json.dumps({k: r.get(k) for k in STATE_KEY}, sort_keys=True)
        groups.setdefault(key, []).append(r["index_sha256"])
    multi = [g for g in groups.values() if len(g) >= 2]
    bad = sum(1 for g in multi if len(set(g)) > 1)
    verdict = "FAIL" if bad else ("PASS" if len(multi) >= 2 else "NOT ENOUGH")
    return len(multi), sum(len(g) for g in multi), bad, verdict


# capture.md §7: the state key (cursor: the drawn state only; timers are not part of it)
STATE_KEY = ("player", "tile_origin", "unit_origin", "shake", "open_mode", "palette_sha256",
             "cursor_key", "light_key", "level")


def make_recorder(rt):
    """The recorder class, built on record_tick.TickRecorder (Windows only)."""
    class Recorder(rt.TickRecorder):
        def __init__(self, exe, args, out_path, seconds, max_ticks, img_dir, every, max_frames,
                     allow_size, draws_every, light_full):
            super().__init__(exe, args, out_path, seconds, 0, max_ticks)
            self.img_dir, self.every, self.max_frames = img_dir, every, max_frames
            self.allow_size, self.draws_every, self.light_full = allow_size, draws_every, light_full
            self.frames = 0
            self.seen = 0
            self.seq = 0
            self.keys = []         # stability records (capture.md §7)
            self.start = None      # state read at the in-game draw entry
            self.draws = None      # draw log of the current frame, or None
            self.libs = None       # DT1 list of the current frame (raw-3)
            self.raster_other = 0  # rasterizer calls not matched to the last cel op
            self.armed = set()
            self.meta = {"k": "capture", "images": os.path.basename(img_dir) if img_dir else None,
                         "every": every, "draws_every": draws_every, "state_key": list(STATE_KEY)}

        # breakpoints: draw hooks are verified at start but armed per selected frame
        def arm(self, addr):
            if addr not in self.bp_orig:
                self.bp_orig[addr] = self.read(addr, 1)[0]
            if addr in DRAWS or addr in (UNIT_DRAW, RASTER):
                return
            self.write(addr, rt.rr.INT3)
            self.armed.add(addr)

        def set_draw_hooks(self, on):
            for addr in list(DRAWS) + [UNIT_DRAW, RASTER]:
                if on and addr not in self.armed:
                    self.write(addr, rt.rr.INT3)
                    self.armed.add(addr)
                elif not on and addr in self.armed:
                    self.write(addr, bytes([self.bp_orig[addr]]))
                    self.armed.discard(addr)

        def on_single_step(self, tid):
            addr = self.reinsert.pop(tid, None)
            if addr is not None and addr in self.armed:
                self.write(addr, rt.rr.INT3)

        def frame_start(self, ctx):
            unit = self.u32(PLAYER)
            self.start = {"seed": read_seed(self, unit), "cursor": read_cursor(self)}
            want = self.draws_every and self.seen % self.draws_every == 0
            self.draws = [] if want else None
            self.libs = None       # DT1 list, read at the frame's first tile draw
            self.set_draw_hooks(bool(want))

        def draw_call(self, addr, ctx):
            if self.draws is None:
                return
            ret = self.u32(ctx.Esp)
            if addr == UNIT_DRAW:
                a = struct.unpack("<4i", self.read(ctx.Esp + 4, 16))
                rec = {"op": "unit", "at": f"{ret - 5:#x}", "u": read_unit(self, ctx.Ecx) if ctx.Ecx else None,
                       "light": ctx.Edx, "a": list(a)}
            else:
                name, n = DRAWS[addr]
                args = list(struct.unpack(f"<{n}I", self.read(ctx.Esp + 4, 4 * n)))
                if name in TILE_OPS and self.libs is None:
                    self.libs = read_dt1_list(self)
                rec = read_draw(self, name, args, self.light_full, self.libs)
                rec["at"] = f"{ret - 5:#x}"
            self.draws.append(rec)

        def raster(self, ctx):
            """The cel the last cel op draws (raw-3): attached to that draw when the context
            matches; any other rasterizer call is counted in the frame's `raster_other`."""
            if self.draws is None:
                return
            cctx = self.u32(ctx.Esp + 4)
            cel = self.u32(cctx + CTX_CEL) if cctx else 0
            last = self.draws[-1] if self.draws else None
            c = last.get("cel") if last else None
            if c and c.get("ctx") == f"{cctx:#x}" and "hdr" not in c and cel:
                c["hdr"] = read_cel_header(self, cel)
            else:
                self.raster_other += 1

        def capture(self, ctx):
            if self.u32(ctx.Esp) != IN_GAME_RET:
                return
            draws, self.draws = self.draws, None
            raster_other, self.raster_other = self.raster_other, 0
            self.set_draw_hooks(False)
            self.seen += 1
            if (self.seen - 1) % self.every:
                return
            self.seq += 1
            vt, w, h, base = (self.u32(VIDEO_TYPE), self.i32(FB_W), self.i32(FB_H), self.u32(FB_PTR))
            rec = {"k": "frame", "seq": self.seq, "f": self.frame, "video_type": vt, "w": w, "h": h}
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
                name = f"frame-{self.seq:07d}.png"
                with open(os.path.join(self.img_dir, name), "wb") as f:
                    f.write(png_bytes(w, h, pixels, pal))
                rec["image"] = name
            if draws is not None:
                rec["draws"] = draws
                rec["raster_other"] = raster_other
            self.keys.append({k: rec.get(k) for k in STATE_KEY + ("index_sha256",)})
            self.emit(rec)
            self.frames += 1
            if self.max_frames and self.frames >= self.max_frames:
                self.notes.append(f"frame limit {self.max_frames} reached")
                self.done = True

        def state(self):
            st = {"draw": self.u32(DRAW_COUNTER), "client_update": self.u32(CLIENT_UPDATES),
                  "open_mode": self.u32(OPEN_MODE), "shift_x": self.i32(SHIFT_X),
                  "unit_origin": [self.i32(UNIT_ORIGIN_X), self.i32(UNIT_ORIGIN_Y)],
                  "shake": [self.u32(SHAKE_AMP), self.i32(SHAKE_DX), self.i32(SHAKE_DY)],
                  "clear_counter": self.i32(CLEAR_COUNTER), "res_mode": self.u32(RES_MODE),
                  "light": read_light(self), "weather": read_weather(self)}
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
                    p["dir"] = self.read(path + 0x64, 1)[0]   # direction byte (capture.md §3.5)
                st["player"] = p
                st["seed_end"] = read_seed(self, unit)
            st["level"] = read_level(self, unit)
            if self.start:
                st["seed_start"] = self.start["seed"]
                c = self.start["cursor"]
                st["cursor"] = c
                st["cursor_key"] = [c["visible"], c["type"], c["frame"] >> 8, c["x"], c["y"], c["adj"],
                                    c["item"]]
            st["light_key"] = st["light"]["quality"]
            return st

        def handle(self, addr, ctx):
            if self.meta:  # capture.md §5: the second line names the image directory
                self.emit(self.meta)
                self.meta = None
            if addr == END_SCENE:
                self.capture(ctx)
            elif addr == FRAME_START:
                self.frame_start(ctx)
            elif addr == CEL_LOADED:
                path = self.read(ctx.Ebp - 0x108, 0x104).split(b"\0")[0].decode("latin-1")
                self.emit({"k": "celfile", "ptr": f"{self.u32(ctx.Ebp - 4):#x}", "path": path})
            elif addr in (COMP_PATH_DCC, COMP_PATH_DC6):
                name = self.read(self.u32(ctx.Ebp + 8), 64).split(b"\0")[0].decode("latin-1")
                path = self.read(ctx.Ebx, 260).split(b"\0")[0].decode("latin-1")
                self.emit({"k": "compfile", "name": name, "path": path})
            elif addr == RASTER:
                self.raster(ctx)
            elif addr in DRAWS or addr == UNIT_DRAW:
                self.draw_call(addr, ctx)
            else:
                super().handle(addr, ctx)

    return Recorder


class FakeMem:
    """Sparse little-endian memory for the selftest."""
    def __init__(self):
        self.b = {}

    def put(self, addr, fmt, *v):
        for i, x in enumerate(struct.pack(fmt, *v)):
            self.b[addr + i] = x

    def read(self, addr, n):
        return bytes(self.b.get(addr + i, 0) for i in range(n))


def selftest():
    w, h = 5, 3
    pixels = bytes((7 * i) & 0xFF for i in range(w * h))
    pal = bytes(range(256)) * 3
    w2, h2, px2, pal2 = png_indices(png_bytes(w, h, pixels, pal))
    assert (w2, h2, px2, pal2) == (w, h, pixels, pal), "PNG round trip changed the frame"
    bad = bytearray(pixels)
    bad[4] ^= 1
    assert hashlib.sha256(bytes(bad)).hexdigest() != hashlib.sha256(pixels).hexdigest()

    # readers on a synthetic process image; every source byte changes exactly its field (M08)
    m = FakeMem()
    unit, path, room, drlg, level, act, env = 0x1000, 0x2000, 0x3000, 0x4000, 0x5000, 0x6000, 0x7000
    gfx, sub, tile = 0x8000, 0x9000, 0xA000
    m.put(PLAYER, "<I", unit)
    m.put(unit, "<IIII", 0, 1, 0, 0x1234)            # type, class, pool, guid
    m.put(unit + U_MODE, "<I", 1)
    m.put(unit + U_SEED, "<II", 0x11111111, 0x22222222)
    m.put(unit + U_PATH, "<I", path)
    m.put(unit + U_CUR, "<i", 0x380)
    m.put(unit + U_GFX, "<I", gfx)
    m.put(gfx + 0x30, "<I", sub)
    m.put(sub + 0x34, "<iii", 3, -4, 5)
    m.put(path, "<IIii", 10 << 16, 12 << 16, 7, 9)
    m.put(path + 0x1C, "<I", room)
    m.put(path + 0x64, "<B", 13)
    m.put(room + 0x10, "<I", drlg)
    m.put(drlg + 0x58, "<I", level)
    m.put(level + 0x1D0, "<I", 8)
    m.put(ACT, "<I", act)
    m.put(act + 4, "<I", env)
    m.put(act + 0x14, "<B", 0)
    m.put(env + 0x0C, "<i", 255)
    m.put(env + 0x18, "<BBB", 1, 2, 3)
    m.put(CUR_VISIBLE, "<I", 1)
    m.put(CUR_STATE, "<i", 4)
    m.put(CUR_TYPE, "<i", 3)
    m.put(CUR_FRAME, "<i", 0x260)
    m.put(CUR_X, "<i", 580)
    m.put(CUR_Y, "<i", 250)
    m.put(tile + 4, "<H", 96)
    m.put(tile + 0x14, "<IIII", 15, 2, 1, 0)
    cases = [  # (reader, address of a source byte, field that must change)
        (lambda: read_level(m, unit), level + 0x1D0, "level_id"),
        (lambda: read_level(m, unit), act + 0x14, "act"),
        (lambda: read_level(m, unit), env + 0x19, "env"),
        (lambda: read_cursor(m), CUR_FRAME + 1, "frame"),
        (lambda: read_cursor(m), CUR_X, "x"),
        (lambda: read_cursor(m), CUR_TYPE, "type"),
        (lambda: read_unit(m, unit), unit + U_GUID, "guid"),
        (lambda: read_unit(m, unit), unit + U_CUR, "cur"),
        (lambda: read_unit(m, unit), path + 0x64, "dir"),
        (lambda: read_unit(m, unit), path + 9, "client"),
        (lambda: read_unit(m, unit), sub + 0x38, "extra"),
        (lambda: read_tile(m, tile), tile + 4, "roof"),
        (lambda: read_tile(m, tile), tile + 0x1C, "sub"),
    ]
    want = {"level_id": 8, "act": 0, "env": [255, 1, 2, 3]}
    assert {k: read_level(m, unit)[k] for k in want} == want, read_level(m, unit)
    u = read_unit(m, unit)
    assert (u["guid"], u["cur"], u["dir"], u["client"], u["extra"]) == (0x1234, 0x380, 13, [7, 9], [3, -4, 5]), u
    assert read_cursor(m)["frame"] >> 8 == 2 and read_tile(m, tile)["orient"] == 15
    for reader, addr, field in cases:
        before = reader()
        m.b[addr] = m.b.get(addr, 0) ^ 0x01
        after = reader()
        m.b[addr] ^= 0x01
        changed = sorted(k for k in before if before[k] != after[k])
        assert changed == [field], f"{addr:#x}: changed {changed}, expected [{field}]"

    # raw-3 readers: cel header, DT1 list and tile lookup (M08: each source byte changes its field)
    cel, rec1, rec2, lib1, lib2 = 0xB000, 0xC000, 0xC400, 0xD000, 0xD400
    m.put(cel, "<Iiiii", 1, 34, 50, -3, 7)
    assert read_cel_header(m, cel) == {"flip": 1, "w": 34, "h": 50, "xoff": -3, "yoff": 7}
    for off, field in ((0, "flip"), (4, "w"), (8, "h"), (0xC, "xoff"), (0x10, "yoff")):
        before = read_cel_header(m, cel)
        m.b[cel + off] ^= 1
        changed = sorted(k for k, v in read_cel_header(m, cel).items() if before[k] != v)
        m.b[cel + off] ^= 1
        assert changed == [field], (off, changed)
    for i, ch in enumerate(b"data\\global\\tiles\\a.dt1"):
        m.b[rec1 + i] = ch
    for i, ch in enumerate(b"b.dt1"):
        m.b[rec2 + i] = ch
    m.put(DT1_LIST, "<I", rec1)
    m.put(rec1 + 0x104, "<I", lib1)
    m.put(rec1 + 0x10C, "<I", rec2)
    m.put(rec2 + 0x104, "<I", lib2)
    m.put(lib1 + 0x10C, "<II", 3, 0x20000)
    m.put(lib2 + 0x10C, "<II", 10, 0x30000)
    libs = read_dt1_list(m)
    assert libs == [(0x20000, 3, "data\\global\\tiles\\a.dt1"), (0x30000, 10, "b.dt1")], libs
    assert dt1_lookup(libs, 0x20000 + 2 * 0x60) == ("data\\global\\tiles\\a.dt1", 2)
    assert dt1_lookup(libs, 0x20000 + 3 * 0x60) is None          # one past the count
    assert dt1_lookup(libs, 0x30000 + 9 * 0x60 + 0x5F) == ("b.dt1", 9)
    m.put(0x20000 + 0x60 + 0x14, "<IIII", 1, 2, 3, 4)
    d = read_draw(m, "TileDrawLit", [0x20000 + 0x60, 10, 20, 0, 0], libs=libs)
    assert (d["tile"]["dt1"], d["tile"]["index"], d["tile"]["main"]) == ("data\\global\\tiles\\a.dt1", 1, 2)
    m.b[lib1 + 0x110 + 1] ^= 1                                   # tile array moved: no file
    assert read_draw(m, "TileDrawLit", [0x20000 + 0x60, 10, 20, 0, 0],
                     libs=read_dt1_list(m))["tile"]["dt1"] is None
    m.b[lib1 + 0x110 + 1] ^= 1
    m.put(rec1 + 0x10C, "<I", rec1)                              # a cyclic list ends
    assert len(read_dt1_list(m)) == 1

    # stability (capture.md §7): singletons never count; one changed hash fails exactly one key
    recs = [{"player": {"cur": c}, "index_sha256": hh} for c, hh in
            ((0, "a"), (0, "a"), (1, "b"), (1, "b"), (2, "c"))]
    assert stability(recs) == (2, 4, 0, "PASS"), stability(recs)
    recs[3]["index_sha256"] = "x"
    assert stability(recs) == (2, 4, 1, "FAIL"), stability(recs)
    assert stability(recs[:2] + recs[4:])[3] == "NOT ENOUGH"
    print("selftest ok: PNG keeps indices and palette; a 1-byte change changes the hash; "
          "raw-3 cel header, DT1 list and tile lookup; "
          f"{len(cases)} state fields each follow exactly their source bytes; stability counts "
          "only repeated keys and reports exactly the changed key")


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0, help="kill the game after N s (default 120)")
    ap.add_argument("--ticks", type=int, default=0, help="stop after N server ticks (default: no limit)")
    ap.add_argument("--every", type=int, default=1, help="capture every N-th in-game frame (default 1)")
    ap.add_argument("--max-frames", type=int, default=0, help="stop after N captures (default: no limit)")
    ap.add_argument("--draws-every", type=int, default=0,
                    help="log every draw call of every N-th in-game frame (default 0: none; slow)")
    ap.add_argument("--draws-light", action="store_true",
                    help="with --draws-every: store tile light arrays in full, not as digests")
    ap.add_argument("--no-save", action="store_true", help="hashes and state only, no PNG files")
    ap.add_argument("--allow-any-size", action="store_true", help="also capture at 640x480")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-frames.jsonl)")
    ap.add_argument("--selftest", action="store_true", help="check the PNG writer and the readers, exit")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"], help="Game.exe arguments (default: -w -ns)")
    autostart.add_options(ap)
    poke.add_options(ap)
    a = ap.parse_args()
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    if a.selftest:
        selftest()
        return
    import record_tick as rt  # noqa: E402  (the shared tick recorder; Windows only; not modified)
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = a.out or os.path.join(repo, "traces", "raw", stamp + "-frames.jsonl")
    img_dir = None if a.no_save else os.path.join(repo, "game", "captures", stamp)
    if img_dir:
        os.makedirs(img_dir, exist_ok=True)
    # keep only the base recorder's tick hook, add ours (draw hooks: verified, armed per frame)
    rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK], END_SCENE: END_SCENE_BYTES,
                 FRAME_START: FRAME_START_BYTES, CEL_LOADED: CEL_LOADED_BYTES,
                 UNIT_DRAW: UNIT_DRAW_BYTES, RASTER: RASTER_BYTES, COMP_PATH_DCC: COMP_PATH_DCC_BYTES,
                 COMP_PATH_DC6: COMP_PATH_DC6_BYTES, **{d: b"\x55\x8B\xEC" for d in DRAWS}}
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = make_recorder(rt)(os.path.abspath(a.game), gargs, out, a.seconds,
                          a.ticks, img_dir, max(1, a.every), a.max_frames, a.allow_any_size,
                          max(0, a.draws_every), a.draws_light)
    r.auto = auto
    if auto and auto.has_frames():
        auto.attach(r)  # `frame F` input steps at the tick-return stop of F - 1
    layer = poke.PokeLayer.from_args(a)
    if layer:
        layer.attach(r)  # arms 0x0052FD1E; {"k":"poke",...} records land in the frames file
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    keys, frames, bad, verdict = stability(r.keys)
    print(f"wrote {out}: {r.frames} frames, {r.ticks} ticks; images: {img_dir or 'none'}")
    print(f"stability: {keys} state keys seen twice or more ({frames} frames), "
          f"{bad} with differing frames: {verdict}")


if __name__ == "__main__":
    main()
