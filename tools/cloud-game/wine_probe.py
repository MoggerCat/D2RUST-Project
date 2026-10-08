"""Run the trace recorder's Win32 debugger against the stand-in under Wine.

    wine python.exe tools/cloud-game/wine_probe.py STANDIN_EXE SYMS [--seconds N] [--out FILE]

Windows Python (tools/cloud-game/setup_winpy.sh) under Wine. Imports
tools/trace-recorder/record_rng.py unchanged and points it at the
stand-in (tools/cloud-game/standin): its SHA-256, the helper addresses
from SYMS (nm output of build.sh) instead of the 1.14d ones, the stand-in's
seed setter, and the inline `mov ecx, K; mul ecx` site it finds by the same
.text scan as for Game.exe. Everything else (CreateProcessW under
DEBUG_ONLY_THIS_PROCESS, INT3 planting, Wow64Get/SetThreadContext,
single-stepping, the kill guarantees, the rng-raw-1 writer) is the code
that runs on PC 1. Then check_rng.py must pass on the recording, and
autostart.py's window side must work: find the window by pid, PrintWindow
screenshot, PostMessageW click and key (the stand-in prints them), WM_CLOSE.

Our own code. Exit 0 when the recording has helper draws, inline draws and
a seed set, check_rng.py passes and the window probe passes.
"""

import argparse
import hashlib
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "trace-recorder"))
import record_rng as rr  # noqa: E402
import check_rng  # noqa: E402
import autostart  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("exe")
    ap.add_argument("syms")
    ap.add_argument("--seconds", type=float, default=4.0)
    ap.add_argument("--busy", action="store_true",
                    help="stand-in without its 40 ms sleep: measures debug-event throughput")
    ap.add_argument("--out", default=os.path.join(HERE, "..", "..", "traces", "raw",
                                                  "wine-probe-rng.jsonl"))
    a = ap.parse_args()
    syms = {}
    for line in open(a.syms):
        addr, _, name = line.split()
        syms[name.lstrip("_")] = int(addr, 16)
    exe = os.path.abspath(a.exe)
    rr.GAME_EXE_SHA256 = hashlib.sha256(open(exe, "rb").read()).hexdigest()
    order = sorted(syms.values())

    def size(name):  # up to the next stand-in symbol
        i = order.index(syms[name])
        return order[i + 1] - order[i] if i + 1 < len(order) else 0x40
    rr.HELPERS = {syms["standin_rng_step"]: ("step", "", size("standin_rng_step")),
                  syms["standin_rng_roll"]: ("roll", "n", size("standin_rng_roll"))}
    rr.SETTERS = {syms["standin_rng_set"]: "set"}

    def install(self, base):
        if base != rr.IMAGE_BASE:
            raise RuntimeError(f"stand-in loaded at {base:#x}, expected {rr.IMAGE_BASE:#x}")
        for addr in rr.HELPERS:
            self.add_role(addr, "helper")
        for addr in rr.SETTERS:
            self.add_role(addr, "setter")
        for addr in self.inline_sites:
            self.add_role(addr, "inline")
        return len(self.inline_sites)
    rr.Recorder.install = install

    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    gargs = ["-w", "-ns", "-seconds", "3"] + (["-busy"] if a.busy else [])
    r = rr.Recorder(exe, gargs, a.out, a.seconds, True, 0)
    t0 = time.perf_counter()
    counts = r.run()
    dt = time.perf_counter() - t0
    n_dbg = sum(r.dbg.values())
    print(f"throughput: {n_dbg} debug events, {r.seq} records in {dt:.2f} s: "
          f"{n_dbg / dt:.0f} debug events/s, {r.seq / dt:.0f} records/s")
    print("inline sites:", [hex(s) for s in r.inline_sites])
    print("events:", r.seq, counts)
    for n in r.notes:
        print("note:", n)
    print("debug events:", r.dbg, "foreign exceptions:", r.exc)
    rc = check_rng.main(a.out)
    need = ("draw:helper:step", "draw:helper:roll", "draw:inline:step", "seed_set:set")
    have = {k: v for k, v in counts.items()}
    missing = [k for k in need if not any(k in key for key in have)]
    if missing:
        print("missing event kinds:", missing, "have:", sorted(have))
        return 1
    if rc:
        return rc
    return window_probe(exe, os.path.join(os.path.dirname(os.path.abspath(a.out)),
                                          "wine-probe-shot.png"))


def window_probe(exe, shot):
    """autostart.py's Win32 side: find the window by pid, PrintWindow PNG,
    PostMessageW click and key, then WM_CLOSE (the stand-in exits 7).
    The PNG is not judged: under Wine 9.0 PrintWindow gives a black image
    (docs/handoff/q-cloud-game.md); run.sh's X screenshots show the screen."""
    import subprocess
    p = subprocess.Popen([exe, "-w", "-ns", "-seconds", "20"], stdout=subprocess.PIPE, text=True)
    hwnd = None
    for _ in range(100):
        hwnd = autostart.find_window(p.pid)
        if hwnd:
            break
        time.sleep(0.05)
    if not hwnd:
        p.kill()
        print("window probe: no window")
        return 1
    time.sleep(0.5)
    autostart.screenshot_png(hwnd, shot)
    autostart.post(hwnd, autostart.WM_LBUTTONDOWN, autostart.MK_LBUTTON, autostart.lparam(600, 300))
    autostart.post(hwnd, autostart.WM_LBUTTONUP, 0, autostart.lparam(600, 300))
    autostart.post(hwnd, autostart.WM_KEYDOWN, autostart.vk_code("ESC"), 0)
    time.sleep(0.3)
    autostart.post(hwnd, 0x0010, 0, 0)  # WM_CLOSE
    out, _ = p.communicate(timeout=10)
    print(out.strip())
    ok = "click 600 300" in out and "key 27" in out and p.returncode == 7 and os.path.exists(shot)
    print(f"window probe: hwnd {hwnd:#x}, shot {shot} ({os.path.getsize(shot)} bytes), "
          f"exit {p.returncode}: {'OK' if ok else 'FAILED'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
