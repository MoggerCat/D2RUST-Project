"""Unattended game start and scripted input for the trace recorders
(Windows, reference 1.14d Game.exe).

Takes the game from the start-up menu into a single-player game with a
chosen character and a chosen map seed, with nobody at the keyboard, then
optionally plays a fixed input script (clicks, keys, screenshots) into the
game window. Our own code (standard library and ctypes); the 1.14d facts it
uses are listed here and in the README ("autostart.py").

1. Character, seed and no-save are the game's own command-line options,
   read into the launcher config by the option table at 0x705040 (0x5C
   bytes per row, reader 0x405450): `-name <char>` (config +0xBD),
   `-seed <n>` (config +0x21A; its handler 0x44D860 sets the fixed-seed
   global 0x731004, so game creation 0x52C280 takes game seed {n, 666},
   game +0x7C = n and +0x84 = 1, and the DRLG uses n instead of the
   save's map ID: `specs/sim/rng.md` §5.2, `specs/drlg/levels.md` OQ 5),
   `-nosave` (config +0x219: the save file is left untouched).
2. The menu (launcher mode 4 in 0x74C704) is left for client mode the way
   `dump_tables.py` does it: next mode 1 into 0x7795E8 (the menu routine
   0x4359D0 returns it) and 0 into the menu message-loop flag 0x72DDD4.
   Client mode (0x44B8A0) then starts a single-player game (config game
   type +0x19 = 0) for the configured character. The game is an
   expansion game: the character must be an expansion character (status
   byte 0x24 bit 0x20 of its .d2s), else the client shows "A Diablo II
   character cannot join a game created by a Diablo II Expansion
   character" and returns to the menu.
3. The start is proved by the client player unit (0x7A6A70) standing in a
   level (path -> room -> level id, `record_frames.read_level`); level 1
   is the Rogue Encampment.
4. Input goes to the game window with PostMessage (WM_MOUSEMOVE /
   WM_xBUTTONDOWN/UP with client coordinates, WM_KEYDOWN/UP), so the
   window may be in the background.

5. `--menu SCRIPT` (instead of `--auto`): the menu is not left by force;
   SCRIPT (the same commands; `waitlevel` and `goto` excepted) is played
   into the front end from launch, e.g. to create a character in the
   game's own create screen (the new-character stub path). Once the
   player stands in a level the `--input` script runs as with `--auto`.

Recorders take `--auto CHAR [--seed N] [--input SCRIPT]` or `--menu
SCRIPT [--seed N] [--input SCRIPT]`; standalone:

  py tools/trace-recorder/autostart.py --try ScnAma --seed 1234     # start, report, kill
  py tools/trace-recorder/autostart.py --selftest                     # no game needed

Input script: `;`-separated commands, run in order once the player is in
a level: `wait S`, `move X Y`, `click X Y`, `rclick X Y`, `hold X Y S`
(left button down S seconds), `waitticks N` (wait N server ticks of the recorder, not seconds),
`mark NAME` (note `autostart: mark NAME ticks=N` in the recording's notes), `key K [S]` (K: a letter or digit, or
ESC, TAB, ENTER, SPACE, SHIFT, CTRL, ALT, F1..F12, or a number), `shot
NAME` (PNG of the client area into the shot directory), `goto T C[,C..]
[S DX DY]` (walk to the nearest unit of type T and one of the classes C,
−1 = any class, and click it), `units T` (log GUID, class, client and
screen point of every client unit of type T), `end` (stop the
recording; the game is killed). X, Y are client pixels (800x600 window).
Frame-anchored steps (specs/tools/scenario-diff.md §2 rule 4): `frame F`
waits for the tick-return stop 0x0052FD1E of game frame F - 1 (game
+0xA8); the steps after it are posted while the game is stopped there, so
the window takes them before frame F's drain. After a `frame` step,
click / rclick / key post all their messages at once and `hold X Y N`
holds N frames (up posted at the stop of frame F + N - 1). Needs a
recorder that calls `AutoStart.attach` (record_state, record_frames, poke).
"""

import argparse
import ctypes as C
import hashlib
import json
import os
import struct
import sys
import threading
import time
import zlib
if os.name == "nt":
    from ctypes import wintypes as W

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

TOOL = "trace-recorder autostart 0.2.0"
GAME_MODE = 0x74C704      # launcher mode: 4 menu, 1 client
MENU_LOOP = 0x72DDD4      # menu message-loop flag
NEXT_MODE = 0x7795E8      # mode the menu routine returns
PLAYER = 0x7A6A70         # client player unit
U_PATH = 0x2C
DEFAULT_AFTER = 6.0       # seconds before leaving the menu (the main menu must be up)


def game_args(char, seed=None, extra=("-w", "-ns")):
    """Game.exe arguments for an unattended start of `char` (a save in the
    game's save folder). -nosave keeps the save file untouched."""
    a = list(extra) + ["-nosave", "-name", char]
    if seed is not None:
        a += ["-seed", str(int(seed))]
    return a


def player_level(mem):
    """Level id of the client player unit, or None."""
    try:
        unit = mem.read_u32(PLAYER)
        if not unit:
            return None
        path = mem.read_u32(unit + U_PATH)
        room = mem.read_u32(path + 0x1C) if path else 0
        drlg = mem.read_u32(room + 0x10) if room else 0
        level = mem.read_u32(drlg + 0x58) if drlg else 0
        return mem.read_u32(level + 0x1D0) if level else None
    except OSError:
        return None


CLIENT_ACT = 0x7A0634     # client act (capture.md §3.2); +0x0C init seed (drlg/levels.md §1)


def act_init_seed(mem):
    """The client act's init seed (the DRLG seed source), or None."""
    try:
        act = mem.read_u32(CLIENT_ACT)
        return mem.read_u32(act + 0x0C) if act else None
    except OSError:
        return None


def player_pos(mem):
    """Client player position: the two 16.16 words at path +0x00 as
    record_frames.py logs them (`player.fixed`), in subtiles, or None."""
    try:
        unit = mem.read_u32(PLAYER)
        path = mem.read_u32(unit + U_PATH) if unit else 0
        if not path:
            return None
        return [mem.read_u32(path) >> 16, mem.read_u32(path + 4) >> 16]
    except OSError:
        return None


# --- input script -----------------------------------------------------------

VK = {"ESC": 0x1B, "TAB": 0x09, "ENTER": 0x0D, "SPACE": 0x20, "SHIFT": 0x10, "CTRL": 0x11,
      "ALT": 0x12, **{f"F{i}": 0x6F + i for i in range(1, 13)}}
UNITS_S = 0x7A5E70        # client unit set S: 6 types x 128 bucket heads (client/model.md §2)
U_CLASS, U_NEXT = 0x04, 0xE4
VIEW_W, VIEW_H = 800, 600  # camera.md §3: player drawn at (W/2, H/2 - 8)


def vk_code(k):
    k = k.upper()
    if k in VK:
        return VK[k]
    if len(k) == 1 and k.isalnum():
        return ord(k)
    return int(k, 0)


SCRIPT_OPS = {"wait": (1, 1), "move": (2, 2), "click": (2, 2), "rclick": (2, 2), "hold": (3, 3),
              "key": (1, 2), "text": (1, 99), "shot": (0, 1), "waitlevel": (1, 2),
              "goto": (2, 5), "dumpdrlg": (0, 1), "waitticks": (1, 1), "mark": (1, 1), "clickunit": (2, 4), "rclickunit": (2, 4),
              "units": (1, 1), "end": (0, 0), "frame": (1, 1)}
TICK_RET = 0x0052FD1E            # tick return, ESI = game (poke.py, original-hooks-spawn.md §5 r2)
TICK_RET_BYTES = bytes.fromhex("8B7618")
G_FRAME = 0xA8                   # game frame (tick.md §2)


def parse_script(text):
    """Script text -> list of (op, args); unknown commands and wrong
    argument counts are errors before the game starts. After a `frame F`
    step (scenario-diff.md §2 rule 4) `hold X Y N` holds N frames and is
    returned as ("holdf", [X, Y, N]); frames must not go back."""
    out = []
    framed, last = False, 0
    for raw in (text or "").split(";"):
        w = raw.split()
        if not w:
            continue
        op, a = w[0].lower(), w[1:]
        if op not in SCRIPT_OPS:
            raise ValueError(f"input script: unknown command {raw.strip()!r}")
        lo, hi = SCRIPT_OPS[op]
        if not lo <= len(a) <= hi:
            raise ValueError(f"input script: {op} takes {lo}..{hi} arguments: {raw.strip()!r}")
        if op == "frame":
            f = int(a[0], 0)
            if f < 1 or f < last:
                raise ValueError(f"input script: frame {f} is below 1 or before frame {last}")
            framed, last = True, f
            out.append(("frame", [f]))
            continue
        if op == "hold" and framed:
            v = [int(x, 0) for x in a]
            if v[2] < 1:
                raise ValueError(f"input script: hold needs N >= 1 frames: {raw.strip()!r}")
            out.append(("holdf", v))
            continue
        if op == "text":
            a = [raw.strip()[4:].strip()]
        elif op == "key":
            a = [vk_code(a[0])] + [float(x) for x in a[1:]]
        elif op in ("clickunit", "rclickunit"):
            a = [int(a[0], 0), a[1]] + [int(x, 0) for x in a[2:]]
        elif op == "goto":
            a = ([int(a[0], 0), tuple(int(c, 0) for c in a[1].split(","))]
                 + [float(x) for x in a[2:3]] + [int(x, 0) for x in a[3:]])
        elif op not in ("shot", "dumpdrlg", "mark"):
            a = [float(x) if "." in x else int(x, 0) for x in a]
        out.append((op, a))
    return out


def client_px(mem, unit):
    """Client pixel position of a unit (camera.md §2), or None."""
    t = mem.read_u32(unit)
    path = mem.read_u32(unit + U_PATH)
    if not path:
        return None
    if t in (2, 4, 5):    # static path: client pixels at +4 / +8
        return (struct.unpack("<i", struct.pack("<I", mem.read_u32(path + 4)))[0],
                struct.unpack("<i", struct.pack("<I", mem.read_u32(path + 8)))[0])
    a, b = mem.read_u32(path) >> 11, mem.read_u32(path + 4) >> 11
    return ((a - b) >> 1, (a + b) >> 2)


def screen_of(mem, unit):
    """Screen point where `unit` is drawn (camera.md §3–§4, mode 0, no
    shake, extra offsets ignored), or None."""
    p = mem.read_u32(PLAYER)
    pp, up = (client_px(mem, p) if p else None), client_px(mem, unit)
    if pp is None or up is None:
        return None
    # the drawn camera: the frame's unit origin (record_frames UNIT_ORIGIN_X / _Y), when the
    # process has it; else the player-centred projection of camera.md
    ox = struct.unpack("<i", struct.pack("<I", mem.read_u32(0x7A520C)))[0]
    oy = struct.unpack("<i", struct.pack("<I", mem.read_u32(0x7A5208)))[0]
    if (ox, oy) != (0, 0) and abs(ox - pp[0]) < 1000 and abs(oy - pp[1]) < 1000:
        return (up[0] - ox, up[1] - oy)
    return (up[0] - pp[0] + VIEW_W // 2, up[1] - pp[1] + VIEW_H // 2 - 8)


def units_of(mem, utype):
    """Every unit of set S with this type (bucket order, chain order)."""
    out = []
    for b in range(128):
        u = mem.read_u32(UNITS_S + utype * 0x200 + 4 * b)
        n = 0
        while u and n < 10000:
            out.append(u)
            u = mem.read_u32(u + U_NEXT)
            n += 1
    return out


def drlg_dump(mem, label=""):
    """The client act's DRLG and level list (`drlg/levels.md` §1 offsets).
    The client copy is built by the same `0x00642DA0` from the same init
    seed (flag 1), so its act choices, level rects and seeds are the
    server's; built rooms differ (the client builds what it sees)."""
    act = mem.read_u32(CLIENT_ACT)
    if not act:
        return {"label": label, "act": None}
    d = mem.read_u32(act + 0x48)
    r = {"label": label, "act_no": mem.read_u32(act + 0x14) & 0xFF,
         "init_seed": mem.read_u32(act + 0x0C), "player_level": player_level(mem),
         "player": player_pos(mem)}
    if not d:
        return r
    r.update({"drlg_seed": [mem.read_u32(d), mem.read_u32(d + 4)],
              "start_seed": mem.read_u32(d + 0x470), "init_seed_copy": mem.read_u32(d + 0x458),
              "flags": mem.read_u32(d + 0x8C), "staff_tomb": mem.read_u32(d + 0x94),
              "boss_tomb": mem.read_u32(d + 0x484), "jungle_bit": mem.read_u32(d + 0x474),
              "act_byte": mem.read_u32(d + 0x480) & 0xFF})
    levels, lv, n = [], mem.read_u32(d + 0x47C), 0
    while lv and n < 200:
        levels.append({"id": mem.read_u32(lv + 0x1D0), "drlg_type": mem.read_u32(lv),
                       "flags": mem.read_u32(lv + 4), "rooms": mem.read_u32(lv + 8),
                       "rect": [struct.unpack("<i", struct.pack("<I", mem.read_u32(lv + o)))[0]
                                for o in (0x1C, 0x20, 0x24, 0x28)],
                       "level_type": mem.read_u32(lv + 0x1C0),
                       "seed": [mem.read_u32(lv + 0x1C4), mem.read_u32(lv + 0x1C8)],
                       "jungle_clearings": mem.read_u32(lv + 0x1B8),
                       "jungle_blocks": mem.read_u32(lv + 0x1BC),
                       "warp_centres": mem.read_u32(lv + 0x228)})
        ids = mem.read_u32(lv + 0x1BC)      # Act III jungles: pointer to the block ids
        if ids and 76 <= levels[-1]["id"] <= 78:   # (outdoor-act3-act5.md §2.8; 2 x 6 blocks)
            levels[-1]["jungle_blocks"] = [mem.read_u32(ids + 4 * i) for i in range(12)]
        lv = mem.read_u32(lv + 0x1AC)
        n += 1
    r["levels"] = sorted(levels, key=lambda x: x["id"])
    return r


def nearest(mem, utype, cls):
    best = None
    for u in units_of(mem, utype):
        # class -1 (goto) or None (clickunit `*`): any class; None also keeps living monsters only
        if cls is not None and -1 not in cls and mem.read_u32(u + U_CLASS) not in cls:
            continue
        if cls is None and mem.read_u32(u + 0x10) in (0, 12):   # mode 0 death, 12 dead
            continue
        s = screen_of(mem, u)
        if s is None:
            continue
        d = (s[0] - VIEW_W // 2) ** 2 + (s[1] - VIEW_H // 2) ** 2
        if best is None or d < best[0]:
            best = (d, u, s)
    return best


class AutoStart:
    """Leaves the menu `after` seconds after creation, notes the arrival in
    a level, then plays the input script. A recorder calls `poll(self)`
    from its debug loop (the recorder has read_u32, write and h_process);
    poll returns True when the script has ended the recording."""

    def __init__(self, after=DEFAULT_AFTER, script="", shot_dir=None, log=None, clock=None,
                 menu=""):
        self.after = after
        self.script = parse_script(script)
        self.menu = parse_script(menu)    # --menu: played from launch, no forced start
        for op, _ in self.menu:
            if op in ("waitlevel", "goto", "dumpdrlg"):
                raise ValueError(f"menu script: {op} needs a level")
        self.menu_runner = None
        self.menu_wake = 0.0
        self.shot_dir = shot_dir
        self.sink = None          # the recorder's notes list (footer), found on the first poll
        self._log = log or (lambda s: print(s, flush=True))
        self.clock = clock or time.perf_counter
        self.t0 = self.clock()
        self.forced_at = None
        self.arrived_at = None
        self.level = None
        self.next_poll = 0.0
        self.hwnd = None
        self.done = False
        self.runner = None
        self.wake = 0.0
        self.played = []          # (seconds after launch, op, args), for the notes / selftest
        self.shots = []           # screenshot threads
        self.dumps = []           # dumpdrlg records
        self.wait_frame = None    # a `frame F` step waits for the tick return of frame F - 1
        self.anchor = None        # F of the last `frame` step run (framed mode)
        self.stop_frame = None    # game +0xA8 at the last tick-return stop seen
        self.attached = False     # attach() called: tick-return stops reach on_tick_return
        self.framed_log = []      # (F, stop frame, op, args): the frame-anchored steps posted

    def has_frames(self):
        return any(op == "frame" for op, _ in self.script)

    def attach(self, rec):
        """Route the recorder's 0x0052FD1E stops (ESI = game) to
        on_tick_return. Shares the stop with poke.py's PokeLayer: arms the
        address only when nobody did (one INT3 per address), wraps
        rec.handle after the recorder's own handler (record_state takes its
        snapshot of frame F - 1 first). Call before rec.run()."""
        rt = sys.modules.get("record_tick")
        if rt is None:
            import record_tick as rt
        if TICK_RET not in rt.EXPECT:
            rt.EXPECT[TICK_RET] = TICK_RET_BYTES
        if getattr(rec, "h_process", None) and TICK_RET not in rec.bp_orig:
            if rec.read(TICK_RET, 3) != TICK_RET_BYTES:
                raise RuntimeError("unexpected code at 0x0052FD1E: not the 1.14d Game.exe?")
            rec.arm(TICK_RET)
        orig, auto = rec.handle, self

        def handle(addr, ctx):
            r = orig(addr, ctx)
            if addr == TICK_RET and getattr(rec, "game", None) in (None, ctx.Esi):
                frame = struct.unpack("<i", rec.read(ctx.Esi + G_FRAME, 4))[0]
                auto.on_tick_return(rec, frame)
            return r

        rec.handle = handle
        self.attached = True

    def on_tick_return(self, mem, frame):
        """The game is stopped at the tick return of `frame` (game +0xA8):
        a `frame F` step with F - 1 <= frame runs now, and every step after
        it up to the next `frame` / hold / timed step is posted while the
        game is stopped, so the window takes the messages before frame F's
        drain."""
        self.stop_frame = frame
        if self.done:
            return
        if self.arrived_at is None:
            if self.forced_at is None:
                return
            lv = player_level(mem)
            if lv is None:
                return
            self.arrived_at, self.level = self.clock() - self.t0, lv
            self.log(f"autostart: player in level {lv} at the stop of frame {frame}, "
                     f"position {player_pos(mem)}, act init seed {act_init_seed(mem)}")
            self.runner = self.run(mem)
            self.wake = self.clock()
            self._advance(mem)
        while not self.done and self.wait_frame is not None and frame >= self.wait_frame - 1:
            f = self.wait_frame
            if frame > f - 1:
                self.log(f"autostart: frame {f} late: posted at the stop of frame {frame}")
            self.anchor, self.wait_frame = f, None
            self._advance(mem)

    def _advance(self, mem):
        """Run the script until it waits (seconds, or a frame)."""
        while not self.done and self.wait_frame is None:
            try:
                y = next(self.runner)
            except StopIteration:
                self.runner = iter(())
                self.wake = float("inf")
                return
            if y:
                self.wake = self.clock() + y
                return

    def log(self, msg):
        self._log(msg)
        if self.sink is not None:
            self.sink.append(msg)

    def poll(self, mem):
        if self.sink is None and isinstance(getattr(mem, "notes", None), list):
            self.sink = mem.notes
        now = self.clock()
        if now < self.next_poll or self.done:
            return self.done
        self.next_poll = now + 0.05
        el = now - self.t0
        if self.menu and self.arrived_at is None:
            if self.menu_runner is None:
                self.log("autostart: menu script started")
                self.menu_runner = self.run(mem, self.menu)
                self.menu_wake = now
            while now >= self.menu_wake:
                try:
                    self.menu_wake = now + next(self.menu_runner)
                except StopIteration:
                    self.menu_wake = float("inf")
            if self.done:
                return True
            self.forced_at = el    # never forced: arrival is checked below
        if self.forced_at is None:
            if el >= self.after:
                try:
                    mode = mem.read_u32(GAME_MODE)
                except OSError:
                    return False
                if mode == 4:
                    mem.write(NEXT_MODE, struct.pack("<I", 1))
                    mem.write(MENU_LOOP, struct.pack("<I", 0))
                    self.forced_at = el
                    self.log(f"autostart: menu left for client mode at {el:.1f}s")
            return False
        if self.arrived_at is None:
            lv = player_level(mem)
            if lv is None:
                return False
            self.arrived_at, self.level = el, lv
            self.log(f"autostart: player in level {lv} at {el:.1f}s, position {player_pos(mem)}, "
                     f"act init seed {act_init_seed(mem)}")
            if self.has_frames() and not self.attached:
                self.log("autostart: `frame` steps need a tick-return recorder "
                         "(record_state, record_frames, poke): they never run here")
            self.runner = self.run(mem)
            self.wake = now
        while not self.done and now >= self.wake and self.wait_frame is None:
            try:
                self.wake = now + next(self.runner)
            except StopIteration:
                self.runner = iter(())
                self.wake = float("inf")
        return self.done

    def window(self, mem):
        if not self.hwnd:
            self.hwnd = find_window(kernel32.GetProcessId(mem.h_process)) if mem.h_process else None
        return self.hwnd

    def send(self, mem, msg, wp, lp):
        hwnd = self.window(mem)
        if hwnd:
            if msg == WM_MOUSEMOVE and os.name == "nt":
                # the game reads the real pointer for hover and cursor draws (the cursor of the
                # frame records stays at the window centre when only messages are posted)
                pt = W.POINT(lp & 0xFFFF, (lp >> 16) & 0xFFFF)
                user32.ClientToScreen(hwnd, C.byref(pt))
                user32.SetCursorPos(pt.x, pt.y)
            post(hwnd, msg, wp, lp)

    def click(self, mem, x, y, right=False):
        down, up, mk = ((WM_RBUTTONDOWN, WM_RBUTTONUP, MK_RBUTTON) if right
                        else (WM_LBUTTONDOWN, WM_LBUTTONUP, MK_LBUTTON))
        self.send(mem, WM_MOUSEMOVE, 0, lparam(x, y))
        yield 0.4      # the game finds the hovered unit in a frame (about 12 per second under Wine)
        self.send(mem, down, mk, lparam(x, y))
        yield 0.25     # held across at least one game frame (a click shorter than a frame can be lost)
        self.send(mem, up, 0, lparam(x, y))

    def posted(self, op, a):
        """Logs a frame-anchored step (framed mode): frame F, the stop."""
        self.framed_log.append((self.anchor, self.stop_frame, op, a))
        self.log(f"autostart: frame {self.anchor}: {op} {' '.join(str(x) for x in a)} "
                 f"posted at the stop of frame {self.stop_frame}")

    def run(self, mem, script=None):
        """The script as a generator: each yield is the seconds to wait
        (0 after a `frame` / framed `hold` step set wait_frame: the next
        tick-return stop resumes it). In framed mode (after a `frame`
        step) pointer and key steps post all their messages at once.
        `script` runs another step list (the menu script) instead."""
        for op, a in (self.script if script is None else script):
            self.played.append((round(self.clock() - self.t0, 2), op, a))
            framed = self.anchor is not None
            if op == "frame":
                self.wait_frame = a[0]
                yield 0
            elif op == "wait":
                yield a[0]
            elif op == "waitticks":
                # server ticks of the recorder (its `ticks` counter), not wall-clock seconds:
                # under Wine the game runs about 5x slower than real time, so scripted input
                # lands on the same ticks only when it waits on ticks
                until = getattr(mem, "ticks", 0) + a[0]
                while getattr(mem, "ticks", 0) < until:
                    yield 0.01
            elif op == "mark":
                self.log(f"autostart: mark {a[0]} ticks={getattr(mem, 'ticks', 0)}")
            elif op == "move":
                self.send(mem, WM_MOUSEMOVE, 0, lparam(*a))
                if framed:
                    self.posted(op, a)
            elif op in ("click", "rclick") and framed:
                down, up, mk = ((WM_RBUTTONDOWN, WM_RBUTTONUP, MK_RBUTTON) if op == "rclick"
                                else (WM_LBUTTONDOWN, WM_LBUTTONUP, MK_LBUTTON))
                self.send(mem, WM_MOUSEMOVE, 0, lparam(a[0], a[1]))
                self.send(mem, down, mk, lparam(a[0], a[1]))
                self.send(mem, up, 0, lparam(a[0], a[1]))
                self.posted(op, a)
            elif op in ("click", "rclick"):
                yield from self.click(mem, a[0], a[1], op == "rclick")
                yield 0.05
            elif op == "holdf":
                self.send(mem, WM_MOUSEMOVE, 0, lparam(a[0], a[1]))
                self.send(mem, WM_LBUTTONDOWN, MK_LBUTTON, lparam(a[0], a[1]))
                self.posted("hold", a)
                self.wait_frame = self.anchor + a[2]
                yield 0
                self.send(mem, WM_LBUTTONUP, 0, lparam(a[0], a[1]))
                self.posted("release", a[:2])
            elif op == "key" and framed:
                sc = user32.MapVirtualKeyW(a[0], 0) if os.name == "nt" else 0
                self.send(mem, WM_KEYDOWN, a[0], 1 | sc << 16)
                if 0x30 <= a[0] <= 0x5A:
                    self.send(mem, WM_CHAR, a[0] | 0x20 if a[0] >= 0x41 else a[0], 1 | sc << 16)
                self.send(mem, WM_KEYUP, a[0], 1 | sc << 16 | 3 << 30)
                self.posted(op, a[:1])
            elif op == "hold":
                self.send(mem, WM_MOUSEMOVE, 0, lparam(a[0], a[1]))
                self.send(mem, WM_LBUTTONDOWN, MK_LBUTTON, lparam(a[0], a[1]))
                yield a[2]
                self.send(mem, WM_LBUTTONUP, 0, lparam(a[0], a[1]))
            elif op == "key":
                sc = user32.MapVirtualKeyW(a[0], 0) if os.name == "nt" else 0
                self.send(mem, WM_KEYDOWN, a[0], 1 | sc << 16)
                if a[0] >= 0x30 and a[0] <= 0x5A:
                    self.send(mem, WM_CHAR, a[0] | 0x20 if a[0] >= 0x41 else a[0], 1 | sc << 16)
                yield a[1] if len(a) > 1 else 0.05
                self.send(mem, WM_KEYUP, a[0], 1 | sc << 16 | 3 << 30)
                yield 0.05
            elif op == "text":
                for ch in a[0]:
                    self.send(mem, WM_CHAR, ord(ch), 1)
                    yield 0.03
            elif op == "shot":
                hwnd = self.window(mem)
                if hwnd and self.shot_dir:
                    os.makedirs(self.shot_dir, exist_ok=True)
                    self.shots.append(shot_async(
                        hwnd, os.path.join(self.shot_dir, (a[0] if a else "shot") + ".png")))
            elif op == "waitlevel":
                limit = self.clock() + (a[1] if len(a) > 1 else 120)
                while player_level(mem) != a[0]:
                    if self.clock() > limit:
                        self.log(f"autostart: waitlevel {a[0]} timed out (level {player_level(mem)})")
                        self.done = True
                        yield 0
                        return
                    yield 0.2
                self.log(f"autostart: level {a[0]} at {self.clock() - self.t0:.1f}s, "
                         f"position {player_pos(mem)}")
            elif op == "goto":
                yield from self.goto(mem, *a)
            elif op in ("clickunit", "rclickunit"):
                # click the nearest unit (utype, cls; `*` = any class) where it is drawn now (no
                # walking); rclickunit with the right button
                best = nearest(mem, a[0], None if a[1] == "*" else tuple(int(c, 0) for c in a[1].split(",")))
                if best is None:
                    self.log(f"autostart: clickunit {a[0]}:{a[1]} not found")
                else:
                    x, y = screen_of(mem, best[1])
                    dx, dy = (a[2], a[3]) if len(a) == 4 else (0, -8)
                    self.log(f"autostart: {op} {a[0]}:{a[1]} clicks ({x + dx}, {y + dy})")
                    yield from self.click(mem, x + dx, y + dy, op == "rclickunit")
            elif op == "units":
                rows = []
                for u in units_of(mem, a[0]):
                    rows.append([mem.read_u32(u + 0x0C), mem.read_u32(u + U_CLASS),
                                 client_px(mem, u), screen_of(mem, u)])
                self.log(f"autostart: units {a[0]} " + json.dumps(rows, separators=(",", ":")))
            elif op == "dumpdrlg":
                rec = drlg_dump(mem, a[0] if a else "")
                self.dumps.append(rec)
                self.log("autostart: dumpdrlg " + json.dumps(rec, separators=(",", ":")))
            elif op == "end":
                limit = self.clock() + 5     # let pending screenshots finish (never join:
                while any(t.is_alive() for t in self.shots) and self.clock() < limit:
                    yield 0.1                # the debugger thread must keep running)
                self.log("autostart: input script ended the recording")
                self.done = True
                yield 0
                return

    def goto(self, mem, utype, cls, timeout=60, dx=0, dy=-8):
        """Walk toward the nearest unit (utype, cls) of set S with clicks;
        once it is on screen, wait for the player to stop and click it."""
        limit = self.clock() + timeout
        while self.clock() < limit:
            best = nearest(mem, utype, cls)
            if best is None:
                yield 0.3
                continue
            _, u, (x, y) = best
            if 60 <= x <= VIEW_W - 60 and 60 <= y <= VIEW_H - 120:
                # stand-still test on the recorder's ticks when it has them (under Wine the
                # game runs about 5x slower than real time: 3 s are 15 ticks, not 75)
                tk = getattr(mem, "ticks", None)
                still = self.clock() + 3 if tk is None else self.clock() + 40
                last, since = player_pos(mem), tk
                while self.clock() < still:
                    yield 0.25
                    now = player_pos(mem)
                    if now != last:
                        last, since = now, getattr(mem, "ticks", None)
                    elif tk is None or getattr(mem, "ticks", 0) - since >= 8:
                        break
                x, y = screen_of(mem, u)
                self.log(f"autostart: goto {utype}:{cls} clicks ({x + dx}, {y + dy}), "
                         f"unit guid {mem.read_u32(u + 0x0C)}, player {player_pos(mem)}")
                yield from self.click(mem, x + dx, y + dy)
                yield 0.1
                return
            vx, vy = x - VIEW_W // 2, y - (VIEW_H // 2 - 8)
            n = max(1.0, (vx * vx + vy * vy) ** 0.5)
            yield from self.click(mem, int(VIEW_W // 2 + vx * 220 / n),
                                  int(VIEW_H // 2 - 8 + vy * 180 / n))
            yield 0.7
        self.log(f"autostart: goto {utype}:{cls} timed out")
        self.done = True
        yield 0


def add_options(ap):
    g = ap.add_argument_group("unattended start (autostart.py)")
    g.add_argument("--auto", metavar="CHAR", default=None,
                   help="start a single-player game with this expansion character unattended "
                        "(adds -nosave -name CHAR to the game arguments)")
    g.add_argument("--seed", type=int, default=None, help="with --auto: map / game seed (-seed N)")
    g.add_argument("--auto-after", type=float, default=DEFAULT_AFTER,
                   help=f"seconds in the menu before leaving it (default {DEFAULT_AFTER})")
    g.add_argument("--input", default="", help="with --auto / --menu: input script (autostart.py doc)")
    g.add_argument("--menu", default="", metavar="SCRIPT",
                   help="instead of --auto: play SCRIPT into the front end from launch "
                        "(no forced start), then --input once in a level")
    g.add_argument("--shots", default=None, help="directory for the script's `shot` PNGs")


def setup(a, game_args_list):
    """(game arguments, AutoStart or None) from parsed options."""
    if getattr(a, "menu", "") and a.auto:
        raise SystemExit("--menu and --auto exclude each other")
    if getattr(a, "menu", ""):
        args = list(game_args_list)
        if a.seed is not None:
            args += ["-seed", str(int(a.seed))]
        return args, AutoStart(None, a.input, a.shots, menu=a.menu)
    if not a.auto:
        if a.seed is not None or a.input:
            raise SystemExit("--seed / --input need --auto CHAR or --menu SCRIPT")
        return game_args_list, None
    return game_args(a.auto, a.seed, game_args_list), AutoStart(a.auto_after, a.input, a.shots)


# --- Win32 window, input and screenshots ------------------------------------

WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP = 0x200, 0x201, 0x202
WM_RBUTTONDOWN, WM_RBUTTONUP = 0x204, 0x205
WM_KEYDOWN, WM_KEYUP, WM_CHAR = 0x100, 0x101, 0x102
MK_LBUTTON, MK_RBUTTON = 1, 2

if os.name == "nt":  # import stays possible elsewhere (CI runs the selftest)
    user32 = C.WinDLL("user32", use_last_error=True)
    gdi32 = C.WinDLL("gdi32", use_last_error=True)
    kernel32 = C.WinDLL("kernel32", use_last_error=True)
    EnumWindowsProc = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
    user32.EnumWindows.argtypes = [EnumWindowsProc, W.LPARAM]
    user32.GetWindowThreadProcessId.argtypes = [W.HWND, C.POINTER(W.DWORD)]
    user32.GetWindowThreadProcessId.restype = W.DWORD
    user32.PostMessageW.argtypes = [W.HWND, W.UINT, W.WPARAM, W.LPARAM]
    user32.IsWindowVisible.argtypes = [W.HWND]
    user32.GetClientRect.argtypes = [W.HWND, C.POINTER(W.RECT)]
    user32.GetDC.argtypes = [W.HWND]
    user32.GetDC.restype = W.HDC
    user32.ReleaseDC.argtypes = [W.HWND, W.HDC]
    user32.PrintWindow.argtypes = [W.HWND, W.HDC, W.UINT]
    gdi32.CreateCompatibleDC.argtypes = [W.HDC]
    gdi32.CreateCompatibleDC.restype = W.HDC
    gdi32.CreateCompatibleBitmap.argtypes = [W.HDC, C.c_int, C.c_int]
    gdi32.CreateCompatibleBitmap.restype = W.HBITMAP
    gdi32.SelectObject.argtypes = [W.HDC, W.HGDIOBJ]
    gdi32.DeleteObject.argtypes = [W.HGDIOBJ]
    gdi32.DeleteDC.argtypes = [W.HDC]
    gdi32.GetDIBits.argtypes = [W.HDC, W.HBITMAP, W.UINT, W.UINT, C.c_void_p, C.c_void_p, W.UINT]
    kernel32.GetProcessId.argtypes = [W.HANDLE]
    kernel32.GetProcessId.restype = W.DWORD


def lparam(x, y):
    return (int(y) & 0xFFFF) << 16 | (int(x) & 0xFFFF)


def post(hwnd, msg, wp, lp):
    user32.PostMessageW(hwnd, msg, wp, lp)


def find_window(pid):
    """The visible top-level window of process `pid`, or None."""
    found = []

    def cb(hwnd, _):
        p = W.DWORD()
        user32.GetWindowThreadProcessId(hwnd, C.byref(p))
        if p.value == pid and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
            return False
        return True

    user32.EnumWindows(EnumWindowsProc(cb), 0)
    return found[0] if found else None


def png_rgb(w, h, rows):
    def chunk(k, x):
        return struct.pack(">I", len(x)) + k + x + struct.pack(">I", zlib.crc32(k + x))
    raw = b"".join(b"\0" + r for r in rows)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))


def screenshot_png(hwnd, path):
    """Client area to an RGB PNG via PrintWindow (PW_CLIENTONLY). For
    looking only: the pixel-exact capture is record_frames.py."""
    class _BIH(C.Structure):
        _fields_ = [("biSize", W.DWORD), ("biWidth", W.LONG), ("biHeight", W.LONG),
                    ("biPlanes", W.WORD), ("biBitCount", W.WORD), ("biCompression", W.DWORD),
                    ("biSizeImage", W.DWORD), ("a", W.LONG), ("b", W.LONG), ("c", W.DWORD),
                    ("d", W.DWORD)]
    r = W.RECT()
    user32.GetClientRect(hwnd, C.byref(r))
    w, h = r.right - r.left, r.bottom - r.top
    if w <= 0 or h <= 0:
        return False
    hdc = user32.GetDC(hwnd)
    mdc = gdi32.CreateCompatibleDC(hdc)
    bmp = gdi32.CreateCompatibleBitmap(hdc, w, h)
    gdi32.SelectObject(mdc, bmp)
    user32.PrintWindow(hwnd, mdc, 1)
    stride = (w * 3 + 3) & ~3
    bih = _BIH(C.sizeof(_BIH), w, -h, 1, 24, 0, stride * h, 0, 0, 0, 0)
    buf = (C.c_ubyte * (stride * h))()
    gdi32.GetDIBits(mdc, bmp, 0, h, buf, C.byref(bih), 0)
    gdi32.DeleteObject(bmp)
    gdi32.DeleteDC(mdc)
    user32.ReleaseDC(hwnd, hdc)
    data = bytes(buf)
    rows = []
    for y in range(h):
        row = bytearray(data[y * stride:y * stride + w * 3])
        row[0::3], row[2::3] = row[2::3], row[0::3]   # BGR -> RGB
        rows.append(bytes(row))
    with open(path, "wb") as f:
        f.write(png_rgb(w, h, rows))
    return True


def shot_async(hwnd, path):
    """Screenshot from another thread: PrintWindow waits for the game's
    window thread, which can be stopped at a debug event that only the
    debugger thread (the caller) can continue."""
    t = threading.Thread(target=screenshot_png, args=(hwnd, path), daemon=True)
    t.start()
    return t


# --- standalone probe ---------------------------------------------------------

def probe(exe, args, auto, seconds):
    """Minimal debugger with no hooks: runs the autostart, kills the game."""
    import record_rng as rr  # Win32 debugger code (Windows only)

    class Probe:
        read = rr.Recorder.read
        read_u32 = rr.Recorder.read_u32
        write = rr.Recorder.write
        kill = rr.Recorder.kill
        close_event_handles = staticmethod(rr.Recorder.close_event_handles)
        h_process = None
        pending = None

    p = Probe()
    p.notes = []
    si = rr.STARTUPINFOW()
    si.cb = C.sizeof(si)
    pi = rr.PROCESS_INFORMATION()
    cmd = C.create_unicode_buffer(" ".join([f'"{exe}"'] + args))
    if not rr.CreateProcessW(exe, cmd, None, None, False, rr.DEBUG_ONLY_THIS_PROCESS,
                             None, os.path.dirname(exe), C.byref(si), C.byref(pi)):
        raise rr.winerr("CreateProcessW")
    p.h_process = pi.hProcess
    rr.DebugSetProcessKillOnExit(True)
    ev = rr.DEBUG_EVENT()
    deadline = time.perf_counter() + seconds
    initial = 0
    try:
        while time.perf_counter() < deadline:
            if auto.poll(p):
                break
            if not rr.WaitForDebugEvent(C.byref(ev), 50):
                continue
            code = ev.dwDebugEventCode
            p.pending = (ev.dwProcessId, ev.dwThreadId)
            status = rr.DBG_CONTINUE
            p.close_event_handles(ev)
            if code == rr.EXIT_PROCESS_DEBUG_EVENT:
                p.notes.append(f"game exited, code {ev.u.ExitProcess.dwExitCode:#x}")
                rr.ContinueDebugEvent(ev.dwProcessId, ev.dwThreadId, rr.DBG_CONTINUE)
                p.pending = None
                break
            if code == rr.EXCEPTION_DEBUG_EVENT:
                ex = ev.u.Exception.ExceptionRecord.ExceptionCode
                if ex in rr.BREAKPOINT_CODES and initial < 2:
                    initial += 1
                else:
                    status = rr.DBG_EXCEPTION_NOT_HANDLED
            rr.ContinueDebugEvent(ev.dwProcessId, ev.dwThreadId, status)
            p.pending = None
        p.notes.append(f"player level at end: {player_level(p)}, position {player_pos(p)}")
    finally:
        p.kill()
        rr.CloseHandle(pi.hThread)
    return p.notes


def selftest():
    assert game_args("TestAma", 77) == ["-w", "-ns", "-nosave", "-name", "TestAma", "-seed", "77"]
    assert game_args("X", None, ["-w"]) == ["-w", "-nosave", "-name", "X"]
    assert lparam(3, 5) == 0x00050003 and lparam(-1, 0) == 0xFFFF
    assert parse_script("wait 1; click 10 20; key r 0.5; text hi there; goto 2 119") == [
        ("wait", [1]), ("click", [10, 20]), ("key", [ord("R"), 0.5]), ("text", ["hi there"]),
        ("goto", [2, (119,)])]
    assert parse_script("waitticks 25; mark a1") == [("waitticks", [25]), ("mark", ["a1"])]
    for bad in ("jump 1", "click 1", "wait"):
        try:
            parse_script(bad)
            raise AssertionError(f"accepted {bad!r}")
        except ValueError:
            pass

    class Mem:
        h_process = None

        def __init__(self):
            self.m = {GAME_MODE: 4}

        def read_u32(self, a):
            return self.m.get(a, 0)

        def write(self, a, data):
            self.m[a] = struct.unpack("<I", data)[0]

    class Clock:
        t = 0.0

        def __call__(self):
            return self.t

    def drive(s, m, clk, until):
        while clk.t < until and not s.done:
            s.poll(m)
            clk.t = round(clk.t + 0.01, 2)

    sent = []
    clk = Clock()
    m = Mem()
    s = AutoStart(after=1, script="wait 1; click 10 20; text ab; waitlevel 2 5; goto 2 157,119; end",
                  log=lambda x: None, clock=clk)
    s.send = lambda mem, msg, wp, lp: sent.append((clk.t, msg, wp, lp))
    drive(s, m, clk, 0.99)
    assert s.forced_at is None                                # not before `after`
    drive(s, m, clk, 1.5)
    assert s.forced_at is not None and m.m[NEXT_MODE] == 1 and m.m[MENU_LOOP] == 0
    assert s.arrived_at is None                               # no player yet
    # player at subtile (5000, 4000) in level 1: unit -> path -> room -> drlg room -> level
    m.m.update({PLAYER: 0x1000, 0x1000 + U_PATH: 0x2000, 0x2000: 5000 << 16, 0x2004: 4000 << 16,
                0x2000 + 0x1C: 0x3000, 0x3000 + 0x10: 0x4000, 0x4000 + 0x58: 0x5000,
                0x5000 + 0x1D0: 1})
    drive(s, m, clk, 2.0)
    assert s.level == 1 and abs(s.arrived_at - 1.5) < 0.06
    drive(s, m, clk, 4.0)
    ops = [(msg, wp) for _, msg, wp, _ in sent]
    assert ops == [(WM_MOUSEMOVE, 0), (WM_LBUTTONDOWN, MK_LBUTTON), (WM_LBUTTONUP, 0),
                   (WM_CHAR, ord("a")), (WM_CHAR, ord("b"))], ops
    t_click = sent[0][0]
    assert 2.5 <= t_click <= 2.6, t_click                     # wait 1 after the arrival
    assert sent[1][3] == lparam(10, 20)
    assert not s.done                                          # waitlevel 2 is waiting
    m.m[0x5000 + 0x1D0] = 2                                    # the player changes level
    # an object (type 2, class 119) at subtile (5003, 4001): static path client pixels
    obj = 0x8000
    m.m.update({UNITS_S + 2 * 0x200 + 4 * 5: obj, obj: 2, obj + U_CLASS: 119, obj + U_PATH: 0x9000,
                0x9004: (5003 - 4001) * 16, 0x9008: (5003 + 4001) * 8})
    drive(s, m, clk, 6.0)
    assert s.done, "goto and end did not run"
    x, y = sent[-1][3] & 0xFFFF, sent[-1][3] >> 16
    # player client px: ((5000-4000)*32, (5000+4000)*16) / camera.md §2 shifts
    assert (x, y) == (400 + (3 - 1) * 16, 292 + (3 + 1) * 8 - 8), (x, y)
    m.m[obj + U_CLASS] = 120                                   # perturbation: wrong class is not found
    assert nearest(m, 2, (119,)) is None
    m.m[GAME_MODE] = 1                                         # not in the menu: never forced
    s2 = AutoStart(after=0, log=lambda x: None)
    assert s2.poll(m) is False and s2.forced_at is None
    s3 = AutoStart(after=0, script="waitlevel 9 1", log=lambda x: None, clock=Clock())
    s3.clock.t = 0
    drive(s3, m, s3.clock, 0.5)                                # forced? mode is 1: no
    assert png_rgb(1, 1, [b"\1\2\3"]).startswith(b"\x89PNG")
    # frame-anchored steps (scenario-diff.md §2 rule 4): parsing
    assert parse_script("frame 10; click 600 300; hold 1 2 3; frame 20; key r; hold 3 4 1") == [
        ("frame", [10]), ("click", [600, 300]), ("holdf", [1, 2, 3]), ("frame", [20]),
        ("key", [ord("R")]), ("holdf", [3, 4, 1])]
    assert parse_script("hold 1 2 1.5") == [("hold", [1, 2, 1.5])]          # seconds before `frame`
    for bad in ("frame 0", "frame 5; frame 4", "frame", "frame 3; hold 1 2 0", "frame 3; hold 1 2 .5"):
        try:
            parse_script(bad)
            raise AssertionError(f"accepted {bad!r}")
        except ValueError:
            pass
    # posting at the tick-return stops: each step at the stop of frame F - 1
    m4 = Mem()
    clk4 = Clock()
    s4 = AutoStart(after=0, script="frame 10; click 600 300; hold 1 2 3; frame 20; key r; "
                   "frame 20; move 5 6", log=lambda x: None, clock=clk4)
    s4.attached = True
    sent4 = []
    s4.send = lambda mem, msg, wp, lp: sent4.append((s4.stop_frame, msg, wp, lp))
    s4.poll(m4)                                                # menu left
    m4.m.update({PLAYER: 0x1000, 0x1000 + U_PATH: 0x2000, 0x2000 + 0x1C: 0x3000,
                 0x3000 + 0x10: 0x4000, 0x4000 + 0x58: 0x5000, 0x5000 + 0x1D0: 1})
    for f in range(1, 30):
        s4.on_tick_return(m4, f)
        clk4.t += 0.04
        s4.poll(m4)                                            # the time loop never runs a framed step
    got = [(f, msg, wp) for f, msg, wp, _ in sent4]
    assert got == [(9, WM_MOUSEMOVE, 0), (9, WM_LBUTTONDOWN, MK_LBUTTON), (9, WM_LBUTTONUP, 0),
                   (9, WM_MOUSEMOVE, 0), (9, WM_LBUTTONDOWN, MK_LBUTTON),
                   (12, WM_LBUTTONUP, 0),                      # hold 3 frames: up before frame 13
                   (19, WM_KEYDOWN, ord("R")), (19, WM_CHAR, ord("r")), (19, WM_KEYUP, ord("R")),
                   (19, WM_MOUSEMOVE, 0)], got
    assert sent4[1][3] == lparam(600, 300) and sent4[-1][3] == lparam(5, 6)
    assert [x[:2] for x in s4.framed_log] == [(10, 9), (10, 9), (13, 12), (20, 19), (20, 19)]
    # a late stop: the step runs at the first stop at or after F - 1, noted
    s5 = AutoStart(after=0, script="frame 3; click 1 1", log=lambda x: None, clock=Clock())
    s5.attached, s5.forced_at = True, 0.0
    s5.send = lambda *a: None
    s5.on_tick_return(m4, 7)
    assert s5.framed_log == [(3, 7, "click", [1, 1])]
    # waitticks waits on the recorder's tick counter (not the clock); mark notes the tick
    notes = []
    s6 = AutoStart(after=0, script="waitticks 3; mark m1; end", log=notes.append, clock=Clock())
    m.m[GAME_MODE] = 4
    m.ticks = 10
    s6.send = lambda *a: None
    for i in range(40):
        s6.clock.t += 0.1
        s6.poll(m)
    assert not s6.done and not any("mark" in n for n in notes), notes
    m.ticks = 12
    s6.clock.t += 0.1
    s6.poll(m)
    assert not s6.done, "waitticks 3 ended at 2 ticks"
    m.ticks = 13
    for i in range(5):
        s6.clock.t += 0.1
        s6.poll(m)
    assert any(n == "autostart: mark m1 ticks=13" for n in notes) and s6.done, notes
    print("selftest ok: arguments, script parsing, menu force only in mode 4 and after the delay, "
          "arrival from the player chain, click / text timing, waitlevel, goto projection "
          "(camera.md), wrong class not found, end; frame steps posted at the tick-return stop "
          "of frame F - 1 (click, framed hold, key, move), late stop noted")


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--try", dest="char", help="start this character unattended, report, kill")
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--seeds", default=None,
                    help="comma-separated seeds: one game per seed, one after the other")
    ap.add_argument("--after", type=float, default=DEFAULT_AFTER)
    ap.add_argument("--seconds", type=float, default=60.0)
    ap.add_argument("--input", default="wait 2; shot arrival; end",
                    help="input script after the arrival (default: a screenshot, then end)")
    ap.add_argument("--shots", default=None, help="screenshot directory (default: none)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    if not a.char:
        ap.error("--try CHAR or --selftest")
    import record_rng as rr
    exe = os.path.abspath(a.game)
    if hashlib.sha256(open(exe, "rb").read()).hexdigest() != rr.GAME_EXE_SHA256:
        sys.exit(f"{exe}: not the reference 1.14d Game.exe")
    seeds = [int(x, 0) for x in a.seeds.split(",")] if a.seeds else [a.seed]
    ok = True
    for seed in seeds:
        auto = AutoStart(a.after, a.input, a.shots)
        notes = probe(exe, game_args(a.char, seed), auto, a.seconds)
        for n in notes:
            if not n.startswith("autostart:"):     # already printed by the log
                print("note:", n)
        ok = ok and auto.arrived_at is not None
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
