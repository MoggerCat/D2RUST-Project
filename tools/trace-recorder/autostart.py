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

Recorders take `--auto CHAR [--seed N] [--input SCRIPT]`; standalone:

  py tools/trace-recorder/autostart.py --try ScnAma --seed 1234     # start, report, kill
  py tools/trace-recorder/autostart.py --selftest                     # no game needed

Input script: `;`-separated commands, run in order once the player is in
a level: `wait S`, `move X Y`, `click X Y`, `rclick X Y`, `hold X Y S`
(left button down S seconds), `key K [S]` (K: a letter or digit, or
ESC, TAB, ENTER, SPACE, SHIFT, CTRL, ALT, F1..F12, or a number), `shot
NAME` (PNG of the client area into the shot directory), `end` (stop the
recording; the game is killed). X, Y are client pixels (800x600 window).
"""

import argparse
import ctypes as C
import hashlib
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

TOOL = "trace-recorder autostart 0.1.0"
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


def vk_code(k):
    k = k.upper()
    if k in VK:
        return VK[k]
    if len(k) == 1 and k.isalnum():
        return ord(k)
    return int(k, 0)


def parse_script(text):
    """Script text -> list of timed events (t, op, args), t relative to the
    player's arrival. Button and key presses become separate down / up
    events so the debugger loop never sleeps."""
    t, out = 0.0, []
    for raw in (text or "").split(";"):
        w = raw.split()
        if not w:
            continue
        op, a = w[0].lower(), w[1:]
        if op == "wait":
            t += float(a[0])
        elif op == "move":
            out.append((t, "move", (int(a[0]), int(a[1]))))
        elif op in ("click", "rclick"):
            b = "l" if op == "click" else "r"
            xy = (int(a[0]), int(a[1]))
            out += [(t, "move", xy), (t, b + "down", xy), (t + 0.05, b + "up", xy)]
            t += 0.1
        elif op == "hold":
            xy, s = (int(a[0]), int(a[1])), float(a[2])
            out += [(t, "move", xy), (t, "ldown", xy), (t + s, "lup", xy)]
            t += s + 0.05
        elif op == "key":
            s = float(a[1]) if len(a) > 1 else 0.05
            out += [(t, "kdown", vk_code(a[0])), (t + s, "kup", vk_code(a[0]))]
            t += s + 0.05
        elif op == "shot":
            out.append((t, "shot", a[0] if a else "shot"))
        elif op == "end":
            out.append((t, "end", None))
        else:
            raise ValueError(f"input script: unknown command {raw.strip()!r}")
    return sorted(out, key=lambda e: e[0])


class AutoStart:
    """Leaves the menu `after` seconds after creation, notes the arrival in
    a level, then plays the input script. A recorder calls `poll(self)`
    from its debug loop (the recorder has read_u32, write and h_process);
    poll returns True when the script has ended the recording."""

    def __init__(self, after=DEFAULT_AFTER, script="", shot_dir=None, log=None):
        self.after = after
        self.events = parse_script(script)
        self.shot_dir = shot_dir
        self.log = log or (lambda s: print(s, flush=True))
        self.t0 = time.perf_counter()
        self.forced_at = None
        self.arrived_at = None
        self.level = None
        self.next_poll = 0.0
        self.hwnd = None
        self.done = False
        self.played = []

    def poll(self, mem):
        now = time.perf_counter()
        if now < self.next_poll or self.done:
            return self.done
        self.next_poll = now + 0.05
        el = now - self.t0
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
        while self.events and self.events[0][0] <= el - self.arrived_at:
            _, op, arg = self.events.pop(0)
            self.do(mem, op, arg)
            if self.done:
                return True
        return False

    def window(self, mem):
        if not self.hwnd:
            self.hwnd = find_window(kernel32.GetProcessId(mem.h_process))
        return self.hwnd

    def do(self, mem, op, arg):
        self.played.append((round(time.perf_counter() - self.t0, 2), op, arg))
        if op == "end":
            self.done = True
            self.log("autostart: input script ended the recording")
            return
        hwnd = self.window(mem)
        if not hwnd:
            self.log(f"autostart: no game window for {op}")
            return
        if op == "move":
            post(hwnd, WM_MOUSEMOVE, 0, lparam(*arg))
        elif op == "ldown":
            post(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, lparam(*arg))
        elif op == "lup":
            post(hwnd, WM_LBUTTONUP, 0, lparam(*arg))
        elif op == "rdown":
            post(hwnd, WM_RBUTTONDOWN, MK_RBUTTON, lparam(*arg))
        elif op == "rup":
            post(hwnd, WM_RBUTTONUP, 0, lparam(*arg))
        elif op == "kdown":
            post(hwnd, WM_KEYDOWN, arg, 1 | user32.MapVirtualKeyW(arg, 0) << 16)
        elif op == "kup":
            post(hwnd, WM_KEYUP, arg, 1 | user32.MapVirtualKeyW(arg, 0) << 16 | 3 << 30)
        elif op == "shot" and self.shot_dir:
            os.makedirs(self.shot_dir, exist_ok=True)
            shot_async(hwnd, os.path.join(self.shot_dir, arg + ".png"))


def add_options(ap):
    g = ap.add_argument_group("unattended start (autostart.py)")
    g.add_argument("--auto", metavar="CHAR", default=None,
                   help="start a single-player game with this expansion character unattended "
                        "(adds -nosave -name CHAR to the game arguments)")
    g.add_argument("--seed", type=int, default=None, help="with --auto: map / game seed (-seed N)")
    g.add_argument("--auto-after", type=float, default=DEFAULT_AFTER,
                   help=f"seconds in the menu before leaving it (default {DEFAULT_AFTER})")
    g.add_argument("--input", default="", help="with --auto: input script (autostart.py doc)")
    g.add_argument("--shots", default=None, help="directory for the script's `shot` PNGs")


def setup(a, game_args_list):
    """(game arguments, AutoStart or None) from parsed options."""
    if not a.auto:
        if a.seed is not None or a.input:
            raise SystemExit("--seed / --input need --auto CHAR")
        return game_args_list, None
    return game_args(a.auto, a.seed, game_args_list), AutoStart(a.auto_after, a.input, a.shots)


# --- Win32 window, input and screenshots ------------------------------------

WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP = 0x200, 0x201, 0x202
WM_RBUTTONDOWN, WM_RBUTTONUP = 0x204, 0x205
WM_KEYDOWN, WM_KEYUP = 0x100, 0x101
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
        notes = []

    p = Probe()
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
    ev = parse_script("wait 1; click 10 20; hold 5 6 2; key r; key F1 0.5; shot a; end")
    want = [(1.0, "move", (10, 20)), (1.0, "ldown", (10, 20)), (1.05, "lup", (10, 20)),
            (1.1, "move", (5, 6)), (1.1, "ldown", (5, 6)), (3.1, "lup", (5, 6)),
            (3.15, "kdown", ord("R")), (3.2, "kup", ord("R")),
            (3.25, "kdown", 0x70), (3.75, "kup", 0x70), (3.8, "shot", "a"), (3.8, "end", None)]
    assert [(round(t, 2), o, x) for t, o, x in ev] == want, ev
    try:
        parse_script("jump 1")
        raise AssertionError("unknown command accepted")
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
    logs = []
    m = Mem()
    s = AutoStart(after=0, script="end", log=logs.append)
    assert s.poll(m) is False and s.forced_at is not None
    assert m.m[NEXT_MODE] == 1 and m.m[MENU_LOOP] == 0
    s.next_poll = 0
    assert s.poll(m) is False and s.arrived_at is None      # no player yet
    m.m.update({PLAYER: 0x1000, 0x1000 + U_PATH: 0x2000, 0x2000 + 0x1C: 0x3000,
                0x3000 + 0x10: 0x4000, 0x4000 + 0x58: 0x5000, 0x5000 + 0x1D0: 1})
    s.next_poll = 0
    assert s.poll(m) is True and s.level == 1                # arrived; `end` stops
    m.m[0x5000 + 0x1D0] = 2                                  # perturbation is visible
    assert player_level(m) == 2
    m.m[GAME_MODE] = 1                                       # not in the menu: never forced
    s2 = AutoStart(after=0, log=logs.append)
    assert s2.poll(m) is False and s2.forced_at is None
    assert png_rgb(1, 1, [b"\1\2\3"]).startswith(b"\x89PNG")
    print("selftest ok: arguments, input script timing, menu force only in mode 4, "
          "arrival from the player chain, script end")


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--try", dest="char", help="start this character unattended, report, kill")
    ap.add_argument("--seed", type=int, default=None)
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
    auto = AutoStart(a.after, a.input, a.shots)
    for n in probe(exe, game_args(a.char, a.seed), auto, a.seconds):
        print("note:", n)
    sys.exit(0 if auto.arrived_at is not None else 1)


if __name__ == "__main__":
    main()
