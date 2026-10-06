"""Tiny Win32 UI driver for the 1.14d Game.exe window (stdlib only).
Own code. Used by make_saves.py."""
import ctypes, ctypes.wintypes as wt, time, struct, os
u32 = ctypes.windll.user32
g32 = ctypes.windll.gdi32
u32.SetProcessDPIAware()

def find_window(pid=None, timeout=60):
    t0 = time.time()
    found = []
    @ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
    def cb(h, _):
        if not u32.IsWindowVisible(h):
            return True
        p = wt.DWORD(); u32.GetWindowThreadProcessId(h, ctypes.byref(p))
        buf = ctypes.create_unicode_buffer(64); u32.GetClassNameW(h, buf, 64)
        if (pid is None or p.value == pid) and buf.value == "Diablo II":
            found.append(h)
        return True
    while time.time() - t0 < timeout:
        found.clear(); u32.EnumWindows(cb, 0)
        if found: return found[0]
        time.sleep(0.5)
    return None

def client_origin(h):
    pt = wt.POINT(0, 0); u32.ClientToScreen(h, ctypes.byref(pt)); return pt.x, pt.y

def focus(h):
    u32.ShowWindow(h, 9); u32.SetForegroundWindow(h); time.sleep(0.2)

class INPUT(ctypes.Structure):
    class _U(ctypes.Union):
        _fields_ = [("ki", ctypes.c_byte * 24), ("mi", ctypes.c_byte * 32)]
    _fields_ = [("type", wt.DWORD), ("u", _U)]

def _mouse_ev(flags):
    # MOUSEINPUT: dx,dy,mouseData,dwFlags,time,dwExtraInfo
    class MI(ctypes.Structure):
        _fields_ = [("dx", ctypes.c_long), ("dy", ctypes.c_long), ("md", wt.DWORD),
                    ("fl", wt.DWORD), ("t", wt.DWORD), ("ex", ctypes.c_size_t)]
    class I(ctypes.Structure):
        _fields_ = [("type", wt.DWORD), ("mi", MI)]
    i = I(0, MI(0, 0, 0, flags, 0, 0))
    u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(i))

def move(h, x, y):
    ox, oy = client_origin(h)
    u32.SetCursorPos(ox + x, oy + y); time.sleep(0.08)

def click(h, x, y, right=False, delay=0.12):
    move(h, x, y)
    d, up = (8, 16) if right else (2, 4)
    _mouse_ev(d); time.sleep(delay); _mouse_ev(up); time.sleep(0.25)

def key(vk, hold=0.06, ext=False):
    sc = u32.MapVirtualKeyW(vk, 0)
    class KI(ctypes.Structure):
        _fields_ = [("vk", wt.WORD), ("sc", wt.WORD), ("fl", wt.DWORD), ("t", wt.DWORD), ("ex", ctypes.c_size_t)]
    class I(ctypes.Structure):
        _fields_ = [("type", wt.DWORD), ("ki", KI), ("pad", ctypes.c_byte * 8)]
    fl = 8 | (1 if ext else 0)
    for f in (fl, fl | 2):
        i = I(1, KI(0, sc, f, 0, 0))
        u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(i))
        time.sleep(hold)
    time.sleep(0.15)

def keydown(vk, down=True):
    sc = u32.MapVirtualKeyW(vk, 0)
    class KI(ctypes.Structure):
        _fields_ = [("vk", wt.WORD), ("sc", wt.WORD), ("fl", wt.DWORD), ("t", wt.DWORD), ("ex", ctypes.c_size_t)]
    class I(ctypes.Structure):
        _fields_ = [("type", wt.DWORD), ("ki", KI), ("pad", ctypes.c_byte * 8)]
    i = I(1, KI(0, sc, 8 | (0 if down else 2), 0, 0))
    u32.SendInput(1, ctypes.byref(i), ctypes.sizeof(i))

def typetext(s):
    for c in s:
        vk = u32.VkKeyScanW(ord(c)) & 0xFF
        shift = (u32.VkKeyScanW(ord(c)) >> 8) & 1
        if shift: keydown(0x10)
        key(vk)
        if shift: keydown(0x10, False)

def screenshot(h, path):
    """Screen-DC copy of the client area -> 24bit BMP."""
    ox, oy = client_origin(h)
    r = wt.RECT(); u32.GetClientRect(h, ctypes.byref(r)); w, hh = r.right, r.bottom
    sdc = u32.GetDC(0); mdc = g32.CreateCompatibleDC(sdc)
    bmp = g32.CreateCompatibleBitmap(sdc, w, hh); g32.SelectObject(mdc, bmp)
    g32.BitBlt(mdc, 0, 0, w, hh, sdc, ox, oy, 0x00CC0020 | 0x40000000)
    class BIH(ctypes.Structure):
        _fields_ = [("sz", wt.DWORD), ("w", ctypes.c_long), ("h", ctypes.c_long), ("pl", wt.WORD),
                    ("bc", wt.WORD), ("co", wt.DWORD), ("si", wt.DWORD), ("xp", ctypes.c_long),
                    ("yp", ctypes.c_long), ("cu", wt.DWORD), ("ci", wt.DWORD)]
    bih = BIH(40, w, hh, 1, 24, 0, 0, 0, 0, 0, 0)
    stride = (w * 3 + 3) & ~3
    buf = ctypes.create_string_buffer(stride * hh)
    g32.GetDIBits(mdc, bmp, 0, hh, buf, ctypes.byref(bih), 0)
    import zlib
    raw = buf.raw
    rows = []
    for y in range(hh - 1, -1, -1):
        row = raw[y * stride:y * stride + w * 3]
        rgb = bytearray(row)
        rgb[0::3], rgb[2::3] = row[2::3], row[0::3]
        rows.append(b"\0" + bytes(rgb))
    def ch(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + ch(b"IHDR", struct.pack(">IIBBBBB", w, hh, 8, 2, 0, 0, 0))
                + ch(b"IDAT", zlib.compress(b"".join(rows), 3)) + ch(b"IEND", b""))
    g32.DeleteObject(bmp); g32.DeleteDC(mdc); u32.ReleaseDC(0, sdc)
    return w, hh

VK = dict(ENTER=0x0D, ESC=0x1B, TAB=0x09, UP=0x26, DOWN=0x28, LEFT=0x25, RIGHT=0x27, SPACE=0x20)

def pkey(h, vk, hold=0.08):
    """PostMessage key (in-game the SendInput scancode path is ignored)."""
    sc = u32.MapVirtualKeyW(vk, 0)
    u32.PostMessageW(h, 0x100, vk, 1 | (sc << 16)); time.sleep(hold)
    u32.PostMessageW(h, 0x101, vk, 1 | (sc << 16) | (3 << 30)); time.sleep(0.2)

def pclick(h, x, y, right=False):
    """PostMessage mouse click at 800x600 logical coords."""
    lp = (int(y) << 16) | int(x)
    u32.PostMessageW(h, 0x200, 0, lp); time.sleep(0.15)
    d, up = (0x204, 0x205) if right else (0x201, 0x202)
    u32.PostMessageW(h, d, 2 if right else 1, lp); time.sleep(0.1)
    u32.PostMessageW(h, up, 0, lp); time.sleep(0.25)

def getpixel(h, x, y):
    """(r,g,b) of a client-area pixel (screen DC)."""
    ox, oy = client_origin(h)
    sdc = u32.GetDC(0); c = g32.GetPixel(sdc, ox + x, oy + y); u32.ReleaseDC(0, sdc)
    return c & 255, (c >> 8) & 255, (c >> 16) & 255
