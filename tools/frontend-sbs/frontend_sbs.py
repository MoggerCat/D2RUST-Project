#!/usr/bin/env python3
"""Front-end screens: 1.14d (Wine, Xvfb :99) vs d2rs (Xvfb :98), X screenshots.

Our own code (REC-1550). Both sides get the same input script at wall-clock
offsets from their own "ready" mark; every shot is cropped to the 800x600
game area and compared per pixel. Output goes outside the repository (rule 1):

  python3 tools/frontend-sbs/frontend_sbs.py OUT [--side orig|ours|both] [--script NAME]

Compare step: `compare OUT` -> OUT/summary.json (per screen: differing
pixels, bounding box of the difference). Frames are Blizzard art: never
write OUT inside the repo.
"""
import json, os, subprocess, sys, time, pathlib

REPO = pathlib.Path(__file__).resolve().parents[2]
GAME = os.environ.get("D2_GAME_DIR", "/home/user/game")
PREFIX = os.environ.get("WINEPREFIX", os.path.expanduser("~/.wine-d2"))
SAVES = os.path.join(PREFIX, "drive_c/users/root/Saved Games/Diablo II")
DISP_O = os.environ.get("D2_DISPLAY", ":99")        # 1.14d (suite workers: :90+k)
DISP_U = os.environ.get("D2_DRAWS_DISPLAY", ":98")  # d2rs (suite workers: :100+k)
CLIENT = os.path.join(os.environ.get("D2RS_BIN_DIR") or str(REPO / "target/release"), "d2-client")
# (t after ready, kind, args); kinds: click X Y, key K, shot NAME
# Window-relative 800x600 points.
EXP = {"ama": (100, 300), "asn": (232, 330), "nec": (301, 300), "bar": (400, 300),
       "pal": (521, 300), "sor": (626, 320), "dru": (720, 330)}
MENUS = [(0.0, "click", (400, 300)), (3, "shot", "main"),
         (3.5, "click", (400, 305)), (7, "shot", "charselect"),
         (7.5, "click", (110, 496)), (11, "shot", "create-idle")]
t = 11.5
for k, p in EXP.items():
    MENUS += [(t, "click", p), (t + 3, "shot", "create-" + k)]
    t += 3.5
MENUS += [(t, "key", "Escape"), (t + 2, "shot", "charselect-back"),
          (t + 2.5, "key", "Escape"), (t + 4.5, "shot", "main-back"),
          (t + 5, "click", (330, 516)), (t + 8, "shot", "credits"),
          (t + 8.5, "key", "Escape"), (t + 10.5, "shot", "main-back2"),
          (t + 11, "click", (470, 516)), (t + 14, "shot", "cinematics")]
# Paper-doll check (REC-2295): charselect, then a shot series. 1.14d: 12 shots a
# little over a frame period apart; d2rs: 70 back-to-back shots so every
# animation phase is sampled (dolls_check.py matches them).
DOLLS_O = [(0.0, "click", (400, 300)), (3.5, "click", (400, 305))] + \
          [(7 + 0.37 * k, "shot", f"d{k:02d}") for k in range(12)]
DOLLS_U = DOLLS_O[:2] + [(7 + 0.05 * k, "shot", f"d{k:02d}") for k in range(70)]
SCRIPTS = {"menus": MENUS, "dolls": DOLLS_O, "dolls-dense": DOLLS_U}


def xdo(display, off, ev):
    env = dict(os.environ, DISPLAY=display)
    t, kind, a = ev
    if kind == "click":
        x, y = a[0] + off[0], a[1] + off[1]
        subprocess.run(["xdotool", "mousemove", str(x), str(y)], env=env)
        time.sleep(0.1)
        subprocess.run(["xdotool", "mousedown", "1"], env=env)
        time.sleep(0.08)
        subprocess.run(["xdotool", "mouseup", "1"], env=env)
    elif kind == "key":
        w = subprocess.run(["xdotool", "search", "--name", "d2rs|Diablo II"], env=env,
                           capture_output=True, text=True).stdout.split()
        if w:
            subprocess.run(["xdotool", "windowfocus", w[0]], env=env)
        subprocess.run(["xdotool", "key", a], env=env)
    elif kind == "shot":
        subprocess.run(["import", "-display", display, "-window", "root",
                        a], env=env)  # a is a path set by caller


def drive(display, off, script, outdir, t0):
    for t, kind, a in script:
        while time.time() - t0 < t:
            time.sleep(0.02)
        if kind == "shot":
            a = str(outdir / f"{a}.png")
        xdo(display, off, (t, kind, a))


def run_orig(out, script):
    outdir = out / "orig"; outdir.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, D2_GAME_DIR=GAME)
    dur = int(script[-1][0]) + 20
    p = subprocess.Popen(["bash", str(REPO / "tools/cloud-game/run.sh"), "--seconds", str(dur),
                          "--out", str(outdir / "run")], env=env)
    # trademark appears ~8 s after launch (measured, r3)
    for _ in range(100):
        r = subprocess.run(["xdpyinfo", "-display", DISP_O], capture_output=True)
        if r.returncode == 0: break
        time.sleep(0.2)
    time.sleep(9)
    drive(DISP_O, (112, 98), script, outdir, time.time())
    p.wait()


def run_ours(out, script):
    outdir = out / "ours"; outdir.mkdir(parents=True, exist_ok=True)
    xv = subprocess.Popen(["Xvfb", DISP_U, "-screen", "0", "1024x768x24", "-nolisten", "tcp"],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(2)
    env = dict(os.environ, DISPLAY=DISP_U, WGPU_BACKEND="vulkan")
    g = subprocess.Popen([CLIENT, "play", "--game-dir", GAME,
                          "--save-dir", SAVES], env=env,
                         stdout=open(outdir / "log.txt", "w"), stderr=subprocess.STDOUT)
    time.sleep(12)  # window + art load; trademark is up
    drive(DISP_U, (0, 0), script, outdir, time.time())
    g.kill(); xv.kill()


def crop(path, off):
    from PIL import Image
    return Image.open(path).convert("RGB").crop((off[0], off[1], off[0] + 800, off[1] + 600))


def compare(out):
    from PIL import ImageChops
    res = {}
    for f in sorted((out / "orig").glob("*.png")):
        o = out / "ours" / f.name
        if not o.exists():
            res[f.stem] = {"error": "no d2rs shot"}; continue
        a, b = crop(f, (112, 98)), crop(o, (0, 0))
        d = ImageChops.difference(a, b).convert("L").point(lambda v: 255 if v else 0)
        n = sum(1 for v in d.getdata() if v)
        res[f.stem] = {"diff_px": n, "bbox": d.getbbox()}
        from PIL import Image
        w = Image.new("RGB", (2400, 600)); w.paste(a, (0, 0)); w.paste(b, (800, 0)); w.paste(d.convert("RGB"), (1600, 0))
        w.save(out / f"pair-{f.stem}.png")
    (out / "summary.json").write_text(json.dumps(res, indent=1))
    for k, v in res.items(): print(k, v)


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[0] == "compare":
        compare(pathlib.Path(a[1])); sys.exit()
    out = pathlib.Path(a[0]).resolve()
    if str(out).startswith(str(REPO)) and not str(out).startswith(str(REPO / "traces" / "raw")):
        sys.exit("OUT must be outside the repository (traces/raw is gitignored)")
    side = a[a.index("--side") + 1] if "--side" in a else "both"
    script = SCRIPTS[a[a.index("--script") + 1] if "--script" in a else "menus"]
    if side in ("orig", "both"): run_orig(out, script)
    if side in ("ours", "both"):
        run_ours(out, SCRIPTS["dolls-dense"] if "--script" in a and a[a.index("--script") + 1] == "dolls" else script)
    if "--script" in a and a[a.index("--script") + 1] == "dolls":
        sys.exit()
    compare(out)
