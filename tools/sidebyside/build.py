"""Side-by-side viewer: 1.14d and d2rs frames of the recorded scenes, one HTML page.

    python3 tools/sidebyside/build.py --out DIR [--groups G,...] [--reuse] [--jobs N]

For every scene group (tools/cloud-game/scene_defs.py, plus the arrival scene of
traces/checks/draws-town-arrival-ama.check):
1. The group's input in the shared frame-anchored form (`frame F; click X Y`,
   specs/tools/scenario-diff.md §2 r4; `framed`): both sides get each step in the same
   server frame.
2. 1.14d under Wine (tools/cloud-game/run.sh): record_frames.py with that input, a frame
   and its draw log every tick it draws, the PNGs in DIR/orig/<group>/img (`--img-dir`,
   outside the repository).
3. Per scene: the scene frame is the first 1.14d frame with draws at or after its mark's
   tick (scenes.py's pick); d2rs `play --dump-draws --at-tick <every 1.14d frame tick up
   to it> --dump-image` (specs/tools/facts-render.md §5 r19) with the steps acting up to
   that tick, on its own Xvfb with WGPU_BACKEND=vulkan.
4. Per compared tick: pixels equal (RGB after each side's palette) or not; the scene's
   match % is the scene frame's; the first differing tick; `facts-compare` on the scene
   frame (`--ignore tick,index_sha256 --skip-weather`) for the first difference line.
5. DIR/side-by-side.html: one self-contained page (images inline), an index with the
   match % per scene, per scene the frames (first, first differing, fixed ticks, the
   scene frame) with 1.14d / d2rs, a slider and a toggle between them, a diff overlay,
   the first difference line and a pass / differs badge.

The frames are rendered game art (CLAUDE.md rule 1): everything is written under DIR,
which must lie outside the repository; the page is published to the private data repo
(reports/side-by-side/<date>/), never to the public one. Our own code.
"""
import argparse
import base64
import datetime
import hashlib
import html
import io
import json
import os
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor

REPO = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
REC = os.path.join(REPO, "tools", "trace-recorder")
sys.path.insert(0, os.path.join(REPO, "tools", "cloud-game"))
from scene_defs import GROUPS as SCENE_GROUPS  # noqa: E402

VERSION = "sidebyside 0.1.0"
GAME = os.environ.get("D2_GAME_DIR", os.path.expanduser("~/game"))
FIXED = (40, 100, 200)  # fixed ticks shown per scene (snapped to a 1.14d frame), below its tick

# the arrival scene of traces/checks/draws-town-arrival-ama.check: ScnAma, no input, tick 73
GROUPS = dict(SCENE_GROUPS)
GROUPS["arrival"] = {"char": "ScnAma", "seed": 1234, "script": "waitticks 71; mark t; waitticks 4; end",
                     "scenes": {"a1-town-arrival-ama": ("t", 0)}}
SKIP = {"npc"}  # its `goto` steps have no d2rs form; its scenes are not named scenes


def log(*a):
    print(*a, flush=True)


def run(argv, timeout=None, env=None, out=None):
    log("$ " + " ".join(argv))
    with open(out, "w") if out else open(os.devnull, "w") as f:
        return subprocess.run(argv, cwd=REPO, timeout=timeout, env=env, stdout=f,
                              stderr=subprocess.STDOUT).returncode


# --- input scripts ---------------------------------------------------------------------

def parse_script(script):
    return [s.strip().split() for s in script.split(";") if s.strip()]


START = 2  # the tick a group script's first `waitticks` counts from (q-scenes-compare's schedule)


def framed(script):
    """The group script in the shared frame-anchored form of `specs/tools/scenario-diff.md`
    §2 r4 (both sides take `frame F; click X Y`: the step acts in server frame F, d2rs
    delivers it at tick F - 1). autostart's plain steps wait wall-clock time around each
    click (0.65 s), so their ticks drift by one to three per click under Wine; framed steps
    land on their frame. Returns (script, steps, marks): steps [(F, words, index)], marks
    {name: (nominal tick, index)}; the tick of a mark is START plus the `waitticks` before
    it. `wait S` (seconds) is kept: the steps after it (in a paused game) post at once."""
    out, steps, marks, tick, need = [], [], {}, START, False
    for i, w in enumerate(parse_script(script)):
        if w[0] == "waitticks":
            tick += int(w[1])
            need = True
        elif w[0] == "wait":
            out.append(" ".join(w))
            need = False
        elif w[0] == "mark":
            marks[w[1]] = (tick, i)
            out.append(" ".join(w))
        elif w[0] in ("click", "rclick", "key", "move"):
            if need:
                out.append(f"frame {tick + 1}")
                need = False
            out.append(" ".join(w))
            steps.append((tick + 1, w, i))
        elif w[0] == "end":
            if need:
                out.append(f"frame {tick + 1}")
            out.append("end")
            break
        else:
            raise SystemExit(f"script step {' '.join(w)}: no d2rs form")
    return "; ".join(out), steps, marks


def d2rs_script(steps, scene_tick, mark_index, mode):
    """d2rs `--input` for a scene drawn at tick T: the framed steps acting in a frame up to
    T; a paused ("last") scene: the steps before its mark (they act before the pause)."""
    parts = []
    for f, w, i in steps:
        if (i > mark_index) if mode == "last" else (f > scene_tick):
            continue
        parts.append(f"frame {f}; {' '.join(w)}")
    return "; ".join(parts)


# --- 1.14d ---------------------------------------------------------------------------

def reset_game_settings():
    """1.14d writes `Mini Panel` at exit (control-panel.md §9 r9: 1 = closed next game), so a
    second run in one Wine prefix starts with the mini panel closed. d2rs has no registry and
    opens it, as a first game does: the recording starts from the same settings (no value)."""
    reg = os.path.join(os.environ.get("WINEPREFIX", os.path.expanduser("~/.wine-d2")), "user.reg")
    if not os.path.exists(reg):
        return
    lines = open(reg, encoding="utf-8", errors="surrogateescape", newline="").read().split("\n")
    keep = [l for l in lines if not l.startswith(('"Mini Panel"=', '"Help Menu"='))]
    if len(keep) != len(lines):
        open(reg, "w", encoding="utf-8", errors="surrogateescape", newline="").write("\n".join(keep))


def record_orig(name, g, out, reuse):
    cap, img = os.path.join(out, "cap.jsonl"), os.path.join(out, "img")
    if reuse and os.path.exists(cap):
        log(f"[{name}] reuse {cap}")
        return cap
    if os.path.isdir(out):
        shutil.rmtree(out)
    os.makedirs(img)
    reset_game_settings()
    # Wine runs about 1.1 server ticks per second with a PNG per frame: a long group (an outdoor
    # level by the town waypoint, ~500 ticks) needs more than the fixed 420 s (q-chk-render-world)
    total = sum(int(m) for m in re.findall(r"waitticks (\d+)", g["script"]))
    secs = max(420, int(total * float(os.environ.get("SBS_SLOW", "1.2"))) + 90)
    code = run(["tools/cloud-game/run.sh", "--python", "--seconds", str(secs + 60), "--out", os.path.join(out, "run"),
                "--", "tools/trace-recorder/record_frames.py", "--game", os.path.join(GAME, "Game.exe"),
                "--seconds", str(secs), "--every", "1", "--draws-every", "1", "--sounds", "--img-dir", img,
                "--out", cap, "--auto", g["char"], "--seed", str(g["seed"]), "--input", framed(g["script"])[0]],
               timeout=secs + 180, out=os.path.join(out, "run.log"))
    if not os.path.exists(cap):
        raise SystemExit(f"[{name}] 1.14d wrote no capture (exit {code}, see {out}/run)")
    return cap


def read_capture(cap):
    """Frames with draws and an image (seq, tick, image, player mode), the marks, and the
    sound request records (facts-render.md §5 r20) as rows of SOUND_KEY."""
    frames, marks, sounds = [], {}, []
    for line in open(cap):
        r = json.loads(line)
        if r.get("k") == "sound":
            u = r["unit"]
            sounds.append((r["f"], r["id"], u[0] if u else None, u[1] if u else None,
                           r["delay"], r["flags"], r["offset"]))
        elif r.get("k") == "frame" and r.get("draws") and r.get("image"):
            frames.append({"seq": r["seq"], "tick": r["f"], "image": r["image"],
                           "mode": (r.get("player") or {}).get("mode")})
        elif r.get("k") == "footer":
            for n in r.get("notes", []):
                if "mark " in n:
                    w = n.split("mark ")[1].split()
                    marks[w[0]] = int(w[1].split("=")[1])
    return frames, marks, sounds


def pick(frames, tick, mode):
    if mode == "last":
        same = [f for f in frames if f["tick"] == tick]
        return same[-1] if same else None
    return next((f for f in frames if f["tick"] >= tick), None)


# --- d2rs ----------------------------------------------------------------------------

class Xvfb:
    def __init__(self, display, logpath):
        self.display, self.logpath, self.proc = display, logpath, None

    def __enter__(self):
        self.proc = subprocess.Popen(["Xvfb", self.display, "-screen", "0", "1024x768x24", "-nolisten", "tcp"],
                                     stdout=open(self.logpath, "w"), stderr=subprocess.STDOUT)
        for _ in range(100):
            if subprocess.run(["xdpyinfo", "-display", self.display], stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL).returncode == 0:
                return self
            time.sleep(0.1)
        raise SystemExit(f"Xvfb {self.display} did not answer")

    def __exit__(self, *exc):
        self.proc.terminate()
        self.proc.wait(10)


def save_path(char):
    prefix = os.environ.get("WINEPREFIX", os.path.expanduser("~/.wine-d2"))
    user = os.environ.get("USER") or "root"
    return os.path.join(prefix, "drive_c", "users", user, "Saved Games", "Diablo II", char + ".d2s")


def d2rs_dump(exe, g, ticks, script, out, display, reuse):
    done = os.path.join(out, "done.json")
    key = {"ticks": ticks, "script": script, "char": g["char"], "seed": g["seed"], "bin": bin_sha(exe)}
    if reuse and os.path.exists(done) and json.load(open(done)) == key:
        return 0
    if os.path.isdir(out):
        shutil.rmtree(out)
    os.makedirs(out)
    argv = [exe, "play", "--save", save_path(g["char"]), "--seed", str(g["seed"]),
            "--dump-draws", os.path.join(out, "dump"), "--at-tick", ",".join(map(str, ticks)), "--dump-image",
            "--sound-log", os.path.join(out, "sounds.tsv")]
    if script:
        argv += ["--input", script]
    env = dict(os.environ, DISPLAY=display, D2_GAME_DIR=GAME)
    env.setdefault("WGPU_BACKEND", "vulkan")
    code = run(argv, timeout=2400, env=env, out=os.path.join(out, "play.log"))
    json.dump(key, open(done, "w"))
    return code


_BIN_SHA = {}


def bin_sha(exe):
    """The d2-client binary's sha256: a reused dump must come from the same build."""
    if exe not in _BIN_SHA:
        h = hashlib.sha256()
        with open(exe, "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
        _BIN_SHA[exe] = h.hexdigest()
    return _BIN_SHA[exe]


def d2rs_dir(out, ticks, t):
    return os.path.join(out, "dump") if len(ticks) == 1 else os.path.join(out, "dump", f"tick-{t}")


# --- images --------------------------------------------------------------------------

def load_rgb(path):
    from PIL import Image
    return Image.open(path).convert("RGB")


def pixel_compare(a, b):
    """(equal pixels, total, diff mask or None) of two RGB images; the mask is a two-colour
    palettized image: magenta where they differ, transparent elsewhere."""
    from PIL import ImageChops
    if a.size != b.size:
        return 0, a.size[0] * a.size[1], None
    d = ImageChops.difference(a, b).convert("L").point(lambda v: 1 if v else 0)
    n = a.size[0] * a.size[1]
    bad = d.histogram()[1]
    if not bad:
        return n, n, None
    mask = d.convert("P")
    mask.putpalette([0, 0, 0, 255, 0, 255])
    mask.info["transparency"] = 0
    return n - bad, n, mask


class Blobs:
    """Images inline once each (content hash), referenced by id from the scenes."""

    def __init__(self):
        self.data, self.ids = {}, {}

    def add(self, path_or_img, lossless=True):
        """A frame file (palettized PNG: kept palettized when lossless, else WebP q80) or a
        mask image (PNG)."""
        from PIL import Image
        img = Image.open(path_or_img) if isinstance(path_or_img, str) else path_or_img
        b = io.BytesIO()
        if lossless:
            img.save(b, "PNG", optimize=True)
            mime = "image/png"
        else:
            img.convert("RGB").save(b, "WEBP", quality=80)
            mime = "image/webp"
        raw = b.getvalue()
        h = hashlib.sha256(raw).hexdigest()[:16]
        if h not in self.data:
            self.data[h] = f"data:{mime};base64," + base64.b64encode(raw).decode()
        return h


# --- sounds and input feel ------------------------------------------------------------

SOUND_KEY = ("tick", "id", "unit_type", "guid", "delay", "flags", "offset")
# player mode tokens by mode number (PlrMode.txt order)
PLAYER_MODES = ("DT", "NU", "WL", "RN", "GH", "TN", "TW", "A1", "A2", "BL", "SC", "TH", "KK", "S1", "S2",
                "S3", "S4", "DD", "SQ", "KB")


def read_sound_log(path):
    """d2rs `--sound-log` rows (facts-render.md §5 r20) as SOUND_KEY tuples."""
    rows = []
    if not os.path.exists(path):
        return None
    for line in open(path):
        if line.startswith("#") or line.startswith("tick\t"):
            continue
        t, _st, i, ut, g, d, f, o = line.rstrip("\n").split("\t")
        rows.append((int(t), int(i), None if ut == "-" else int(ut), None if g == "-" else int(g),
                     int(d), int(f), int(o)))
    return rows


def sound_compare(orig, d2rs, T):
    """Both sides' request calls up to tick T: (rows per side, index of the first differing
    call or None, a one-line description)."""
    o = [r for r in orig if r[0] <= T]
    d = [r for r in (d2rs or []) if r[0] <= T]
    if d2rs is None:
        return o, d, 0, "no d2rs sound log"
    for i, (a, b) in enumerate(zip(o, d)):
        if a != b:
            cols = [k for k, x, y in zip(SOUND_KEY, a, b) if x != y]
            return o, d, i, f"call {i}: 1.14d {fmt_sound(a)} vs d2rs {fmt_sound(b)} ({', '.join(cols)})"
    if len(o) != len(d):
        i = min(len(o), len(d))
        extra = ("1.14d", o[i]) if len(o) > len(d) else ("d2rs", d[i])
        return o, d, i, f"call {i}: only {extra[0]} has {fmt_sound(extra[1])} ({len(o)} vs {len(d)} calls)"
    return o, d, None, f"equal: {len(o)} calls"


def sound_sets(o, d):
    """Order aside: per tick, the calls only one side made (multisets); a line naming the
    first tick that differs and the counts."""
    from collections import Counter
    co, cd = Counter(o), Counter(d)
    key = lambda r: tuple(-1 if x is None else x for x in r)  # noqa: E731
    only_o, only_d = sorted((co - cd).elements(), key=key), sorted((cd - co).elements(), key=key)
    if not only_o and not only_d:
        return "same calls per tick (order aside)"
    first = min(r[0] for r in only_o + only_d)
    fo = [fmt_sound(r) for r in only_o if r[0] == first][:3]
    fd = [fmt_sound(r) for r in only_d if r[0] == first][:3]
    return (f"order aside: {len(only_o)} calls only in 1.14d, {len(only_d)} only in d2rs; first at tick {first}: "
            f"1.14d only [{'; '.join(fo) or '-'}], d2rs only [{'; '.join(fd) or '-'}]")


def fmt_sound(r):
    unit = "-" if r[2] is None else f"{r[2]}:{r[3]}"
    extra = "" if r[4:] == (0, 0, 0) else f" d{r[4]} f{r[5]} o{r[6]}"
    return f"tick {r[0]} id {r[1]} unit {unit}{extra}"


def d2rs_mode(dump_dir):
    """The local player's mode in a d2rs dump: the mode token of the first player cel row
    (data/global/chars/<cls>/<comp>/<cls><comp><armor><MODE><wclass>.dcc)."""
    p = os.path.join(dump_dir, "draws.tsv")
    if not os.path.exists(p):
        return None
    for line in open(p):
        for cell in line.split("\t"):
            if cell.startswith("data/global/chars/"):
                tok = os.path.basename(cell)[7:9].upper()
                return PLAYER_MODES.index(tok) if tok in PLAYER_MODES else None
    return None


def input_feel(steps, ticks, orig_modes, d2rs_modes):
    """Per pointer / key step: drawn ticks from the step's tick (F - 1) to the first drawn
    tick whose player mode differs from the mode before the step, on each side; None when
    the mode did not change before the next step or the scene's end."""
    out = []
    for k, (f, w, _i) in enumerate(steps):
        start = f - 1
        end = steps[k + 1][0] - 1 if k + 1 < len(steps) else ticks[-1] + 1
        if start > ticks[-1]:
            break
        row = {"tick": start, "step": " ".join(w)}
        for side, modes in (("orig", orig_modes), ("d2rs", d2rs_modes)):
            before = [t for t in ticks if t < start]
            m0 = modes.get(before[-1]) if before else None
            hit = next((t for t in ticks if start <= t < end and modes.get(t) is not None
                        and modes.get(t) != m0), None)
            row[side] = None if hit is None else (hit - start, PLAYER_MODES[modes[hit]]
                                                  if modes[hit] < len(PLAYER_MODES) else modes[hit])
        out.append(row)
    return out


# --- per scene -------------------------------------------------------------------------

def facts_line(exe, cap, seq, scene, work, d2rs_scene_dir):
    """facts_render.py on the 1.14d scene frame, then facts-compare: (exit code, first line)."""
    odir = os.path.join(work, "facts-orig")
    run([sys.executable, "-I", os.path.join(REC, "facts_render.py"), cap, "--scene", scene, "--frame", str(seq),
         "--tile-light", "unknown", "--out", odir], timeout=600)
    p = subprocess.run([exe, "facts-compare", os.path.join(odir, "scenes", scene), d2rs_scene_dir,
                        "--ignore", "tick,index_sha256", "--skip-weather"], cwd=REPO,
                       capture_output=True, text=True, timeout=300)
    text = (p.stdout + p.stderr).strip()
    return p.returncode, text


def scene_result(exe, name, scene, g, cap, frames, marks, steps_sounds, work, display, reuse):
    steps, orig_sounds = steps_sounds
    mark, off, *mode = g["scenes"][scene]
    mode = mode[0] if mode else "first"
    nominal, mark_index = steps[1][mark]
    # a paused scene's tick is where 1.14d stopped (its capture's mark); else the nominal one
    at = marks.get(mark) if mode == "last" else nominal + off
    f = pick(frames, at, mode) if at is not None else None
    if f is None:
        return {"scene": scene, "group": name, "error": f"no 1.14d frame for mark {mark} (tick {at})"}
    T = f["tick"]
    shown = [x for x in frames if x["tick"] <= T and (mode != "last" or x["seq"] <= f["seq"])]
    # one frame per tick (the last drawn, as the d2rs dump of a tick is that tick's frame)
    per_tick = {}
    for x in shown:
        if x["tick"] < T or x["seq"] == f["seq"]:
            per_tick[x["tick"]] = x
    ticks = sorted(per_tick)
    script = d2rs_script(steps[0], T, mark_index, mode)
    out = os.path.join(work, "d2rs", scene)
    code = d2rs_dump(exe, g, ticks, script, out, display, reuse)
    img_dir = os.path.join(os.path.dirname(cap), "img")
    rows, first_diff = [], None
    for t in ticks:
        dd = d2rs_dir(out, ticks, t)
        dp = os.path.join(dd, "frame.png")
        op = os.path.join(img_dir, per_tick[t]["image"])
        if not os.path.exists(dp):
            rows.append({"tick": t, "seq": per_tick[t]["seq"], "orig": op, "d2rs": None, "eq": 0, "n": 1})
            if first_diff is None:
                first_diff = t
            continue
        eq, n, _ = pixel_compare(load_rgb(op), load_rgb(dp))
        rows.append({"tick": t, "seq": per_tick[t]["seq"], "orig": op, "d2rs": dp, "eq": eq, "n": n})
        if eq != n and first_diff is None:
            first_diff = t
    last = rows[-1] if rows else None
    fc_code, fc_text = (3, "no d2rs dump of the scene frame")
    if last and last["d2rs"]:
        fc_code, fc_text = facts_line(exe, cap, f["seq"], scene, out, d2rs_dir(out, ticks, T))
    so, sd, sdiff, stext = sound_compare(orig_sounds, read_sound_log(os.path.join(out, "sounds.tsv")), T)
    feel = input_feel([x for x in steps[0] if (x[2] <= mark_index if mode == "last" else x[0] <= T)],
                      ticks, {t: per_tick[t].get("mode") for t in ticks},
                      {t: d2rs_mode(d2rs_dir(out, ticks, t)) for t in ticks})
    return {"scene": scene, "group": name, "char": g["char"], "seed": g["seed"], "tick": T, "seq": f["seq"],
            "script": script, "play_exit": code, "rows": rows, "first_diff": first_diff,
            "match": (last["eq"] / last["n"]) if last else 0.0, "facts_exit": fc_code, "facts": fc_text,
            "sounds": {"orig": so, "d2rs": sd, "first_diff": sdiff, "text": stext}, "feel": feel}


def chosen(res):
    """The frames shown: first, first differing, fixed ticks, the scene frame."""
    rows = res["rows"]
    if not rows:
        return []
    by_tick = {r["tick"]: r for r in rows}
    want = [(rows[0]["tick"], "first frame")]
    if res["first_diff"] is not None:
        want.append((res["first_diff"], "first differing frame"))
    for t in FIXED:
        snap = next((r["tick"] for r in rows if r["tick"] >= t), None)
        if snap is not None and snap < res["tick"]:
            want.append((snap, f"fixed tick {t}"))
    want.append((res["tick"], "scene frame"))
    seen, out = {}, []
    for t, label in want:
        if t in seen:
            seen[t]["labels"].append(label)
            continue
        e = {"row": by_tick[t], "labels": [label]}
        seen[t] = e
        out.append(e)
    return sorted(out, key=lambda e: e["row"]["tick"])


# --- page ----------------------------------------------------------------------------

PAGE_CSS = """
:root{--bg:#f6f5f2;--fg:#1d1d1f;--mute:#62626a;--card:#fff;--line:#d8d6cf;--ok:#1d7a3a;--bad:#b3261e;--accent:#6b4bd8}
@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){--bg:#141416;--fg:#ececef;--mute:#a0a0aa;--card:#1e1e22;--line:#34343a;--ok:#55c27a;--bad:#ff7a70;--accent:#a990ff}}
:root[data-theme="dark"]{--bg:#141416;--fg:#ececef;--mute:#a0a0aa;--card:#1e1e22;--line:#34343a;--ok:#55c27a;--bad:#ff7a70;--accent:#a990ff}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,sans-serif}
main{max-width:1240px;margin:0 auto;padding:16px}h1{font-size:22px;margin:8px 0}h2{font-size:18px;margin:0}
.meta{color:var(--mute);font-size:12px;word-break:break-all}
table{border-collapse:collapse;width:100%;background:var(--card)}th,td{border-bottom:1px solid var(--line);padding:6px 8px;text-align:left;vertical-align:top}
td.num{font-variant-numeric:tabular-nums;text-align:right}
.badge{display:inline-block;padding:1px 8px;border-radius:10px;font-size:12px;font-weight:600;color:#fff}
.pass{background:var(--ok)}.differs{background:var(--bad)}.err{background:var(--mute)}
section.scene{background:var(--card);border:1px solid var(--line);border-radius:8px;margin:20px 0;padding:12px}
.head{display:flex;gap:10px;align-items:center;flex-wrap:wrap}
pre{white-space:pre-wrap;word-break:break-word;background:var(--bg);padding:8px;border-radius:6px;font-size:12px;max-height:160px;overflow:auto}
.strip{display:flex;gap:6px;flex-wrap:wrap;margin:8px 0}.strip button{border:1px solid var(--line);background:var(--bg);color:var(--fg);border-radius:6px;padding:4px 8px;cursor:pointer;font-size:12px}
.strip button.on{border-color:var(--accent);outline:2px solid var(--accent)}
.view{position:relative;width:100%;max-width:800px;aspect-ratio:4/3;background:#000;user-select:none}
.view img{position:absolute;inset:0;width:100%;height:100%;image-rendering:pixelated}
.view .top{clip-path:inset(0 0 0 50%)}.view .split{position:absolute;top:0;bottom:0;width:2px;background:var(--accent);left:50%}
.view .lab{position:absolute;top:4px;font-size:11px;background:rgba(0,0,0,.6);color:#fff;padding:1px 6px;border-radius:4px}
.ctl{display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin:8px 0;max-width:800px}.ctl input[type=range]{flex:1;min-width:160px}
.ctl button{border:1px solid var(--line);background:var(--bg);color:var(--fg);border-radius:6px;padding:4px 10px;cursor:pointer}
a{color:var(--accent)}
table.small{font-size:12px;max-width:800px;margin-bottom:8px}table.small td,table.small th{padding:2px 6px}
tr.bad td{background:color-mix(in srgb,var(--bad) 14%,transparent)}
"""

PAGE_JS = """
const B=JSON.parse(document.getElementById('blobs').textContent);
document.querySelectorAll('section.scene').forEach(sec=>{
  const F=JSON.parse(sec.querySelector('script.frames').textContent);
  const o=sec.querySelector('img.o'),d=sec.querySelector('img.d'),m=sec.querySelector('img.m');
  const r=sec.querySelector('input.slide'),sp=sec.querySelector('.split'),info=sec.querySelector('.finfo');
  const dl=sec.querySelector('.lab.dl');
  const btns=sec.querySelectorAll('.strip button');
  function show(i){const f=F[i];o.src=B[f.o];d.src=f.d?B[f.d]:'';m.src=f.m?B[f.m]:'';m.style.display=(f.m&&sec.querySelector('input.diff').checked)?'':'none';
    btns.forEach((b,j)=>b.classList.toggle('on',j===i));info.textContent=f.info;dl.textContent=f.d?'d2rs':'d2rs: no frame';}
  function setx(v){d.style.clipPath='inset(0 0 0 '+v+'%)';sp.style.left=v+'%';}
  r.addEventListener('input',()=>setx(r.value));
  sec.querySelector('button.tog').addEventListener('click',()=>{r.value=(+r.value>50)?0:100;setx(r.value);});
  sec.querySelector('input.diff').addEventListener('change',()=>{const i=[...btns].findIndex(b=>b.classList.contains('on'));show(i);});
  btns.forEach((b,i)=>b.addEventListener('click',()=>show(i)));
  show(F.length-1);setx(r.value);
});
"""


def badge(res):
    if res.get("error"):
        return '<span class="badge err">error</span>'
    ok = res["first_diff"] is None and res["facts_exit"] == 0 and res["sounds"]["first_diff"] is None
    return f'<span class="badge {"pass" if ok else "differs"}">{"pass" if ok else "differs"}</span>'


def first_line(text):
    """facts-compare's verdict and its first difference line ("DIVERGED: first difference:
    draws.tsv, row 168, column y")."""
    lines = [x.strip() for x in text.splitlines() if x.strip()]
    verdict = next((i for i, x in enumerate(lines) if x.split()[0].rstrip(":") in ("MATCH", "DIVERGED", "PARTIAL")), None)
    if verdict is None:
        return lines[-1] if lines else ""
    return ": ".join(lines[verdict:verdict + 2])


def sound_html(r):
    """The sound request calls up to the scene tick, side by side, around the first difference."""
    e, sd = html.escape, r["sounds"]
    o, d, i = sd["orig"], sd["d2rs"], sd["first_diff"]
    lo = 0 if i is None else max(0, i - 6)
    hi = min(max(len(o), len(d)), (i if i is not None else 0) + 12 if i is not None else 40)
    rows = []
    for k in range(lo, hi):
        a = fmt_sound(o[k]) if k < len(o) else ""
        b = fmt_sound(d[k]) if k < len(d) else ""
        cls = ' class="bad"' if i is not None and k == i else ""
        rows.append(f"<tr{cls}><td class=\"num\">{k}</td><td>{e(a)}</td><td>{e(b)}</td></tr>")
    more = max(len(o), len(d)) - hi
    return (f'<p><b>Sounds</b> (request calls up to tick {r["tick"]}; 1.14d {len(o)}, d2rs {len(d)}): '
            f'{e(sd["text"])}<br><span class="meta">{e(sound_sets(o, d))}</span></p><table class="small"><thead><tr><th>#</th><th>1.14d</th><th>d2rs</th></tr>'
            f'</thead><tbody>{"".join(rows)}</tbody></table>'
            + (f'<p class="meta">{more} more calls not shown</p>' if more > 0 else ""))


def feel_text(x):
    return "no change" if x is None else f"{x[0]} ticks ({x[1]})"


def feel_html(r):
    """Input feel: ticks from each input step to the player's first mode change."""
    e = html.escape
    rows = [f'<tr{"" if row["orig"] == row["d2rs"] else " class=\"bad\""}><td class="num">{row["tick"]}</td>'
            f'<td>{e(row["step"])}</td><td>{feel_text(row["orig"])}</td><td>{feel_text(row["d2rs"])}</td></tr>'
            for row in r["feel"] if row["orig"] is not None or row["d2rs"] is not None]
    if not rows:
        return '<p><b>Input feel</b>: no input step changed the player\'s mode before this frame.</p>'
    return ('<p><b>Input feel</b> (drawn ticks from the input to the player\'s first mode change, and the '
            'new mode; 1.14d draws every second tick from about tick 13, both sides are read at its drawn '
            f'ticks):</p><table class="small"><thead><tr><th>Tick</th><th>Input</th><th>1.14d</th><th>d2rs</th></tr>'
            f'</thead><tbody>{"".join(rows)}</tbody></table>')


def feel_summary(r):
    rows = [x for x in r["feel"] if x["orig"] is not None or x["d2rs"] is not None]
    if not rows:
        return "-"
    return f'{sum(1 for x in rows if x["orig"] == x["d2rs"])} / {len(rows)} equal'


def sound_summary(r):
    sd = r["sounds"]
    if sd["first_diff"] is None:
        return f'equal ({len(sd["orig"])})'
    return f'differs at call {sd["first_diff"]} of {len(sd["orig"])} / {len(sd["d2rs"])}'


def page(results, meta, blobs):
    from PIL import Image
    e = html.escape
    idx = []
    for r in results:
        if r.get("error"):
            idx.append(f'<tr><td><a href="#{e(r["scene"])}">{e(r["scene"])}</a></td><td>{badge(r)}</td>'
                       f'<td class="num">-</td><td class="num">-</td><td>-</td><td>-</td><td>{e(r["error"])}</td></tr>')
            continue
        fd = "-" if r["first_diff"] is None else str(r["first_diff"])
        idx.append(f'<tr><td><a href="#{e(r["scene"])}">{e(r["scene"])}</a></td><td>{badge(r)}</td>'
                   f'<td class="num">{100 * r["match"]:.2f} %</td><td class="num">{r["tick"]} / {fd}</td>'
                   f'<td>{e(sound_summary(r))}</td><td>{e(feel_summary(r))}</td>'
                   f'<td>{e(first_line(r["facts"]))}</td></tr>')
    secs = []
    for r in results:
        if r.get("error"):
            secs.append(f'<section class="scene" id="{e(r["scene"])}"><div class="head"><h2>{e(r["scene"])}</h2>'
                        f'{badge(r)}</div><pre>{e(r["error"])}</pre></section>')
            continue
        frames = []
        for c in chosen(r):
            row = c["row"]
            final = row["tick"] == r["tick"] or "first differing frame" in c["labels"]
            fo = blobs.add(row["orig"], lossless=final)
            fd = fm = None
            if row["d2rs"]:
                fd = blobs.add(row["d2rs"], lossless=final)
                _, _, mask = pixel_compare(load_rgb(row["orig"]), load_rgb(row["d2rs"]))
                fm = blobs.add(mask) if mask is not None else None
            pct = 100 * row["eq"] / row["n"]
            info = (f'tick {row["tick"]} (1.14d frame seq {row["seq"]}): {", ".join(c["labels"])}; '
                    f'{pct:.2f} % of pixels equal' + ("" if final else "; preview images are lossy WebP, "
                                                      "the diff overlay is exact"))
            frames.append({"o": fo, "d": fd, "m": fm, "label": f'{row["tick"]}: {", ".join(c["labels"])}',
                           "info": info})
        strip = "".join(f'<button>{e(f["label"])}</button>' for f in frames)
        secs.append(f'''<section class="scene" id="{e(r["scene"])}">
<div class="head"><h2>{e(r["scene"])}</h2>{badge(r)}<span class="meta">{100 * r["match"]:.2f} % equal at tick {r["tick"]};
first differing tick: {"none" if r["first_diff"] is None else r["first_diff"]} of {len(r["rows"])} compared;
group {e(r["group"])}, {e(r["char"])}, seed {r["seed"]}</span></div>
<div class="strip">{strip}</div>
<div class="view"><img class="o" alt="1.14d"><img class="d top" alt="d2rs"><img class="m" alt="diff">
<div class="split"></div><span class="lab" style="left:4px">1.14d</span><span class="lab dl" style="right:4px">d2rs</span></div>
<div class="ctl"><span>1.14d</span><input class="slide" type="range" min="0" max="100" value="50"><span>d2rs</span>
<button class="tog">toggle</button><label><input class="diff" type="checkbox" checked> diff overlay (magenta)</label></div>
<div class="meta finfo"></div>
<p><b>First difference</b> (facts-compare, exit {r["facts_exit"]}):</p><pre>{e(r["facts"])}</pre>
{sound_html(r)}
{feel_html(r)}
<p class="meta">d2rs input: {e(r["script"] or "(none)")}; play exit {r["play_exit"]}</p>
<script type="application/json" class="frames">{json.dumps(frames)}</script>
</section>''')
    npass = sum(1 for r in results if not r.get("error") and r["first_diff"] is None and r["facts_exit"] == 0
                and r["sounds"]["first_diff"] is None)
    return f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>D2 side by side</title><style>{PAGE_CSS}</style></head><body><main>
<h1>1.14d vs d2rs, side by side</h1>
<p class="meta">{e(meta["date"])}; {e(meta["tool"])}; public repo {e(meta["commit"])}; d2-client sha256 {e(meta["d2_client_sha256"])}; command: {e(meta["command"])}</p>
<p>{npass} of {len(results)} scenes pass (every compared tick pixel-identical, facts-compare equal and the same sound request calls).
Pixels compare as colours after each side's palette. Pass means exact; the match % is the scene frame's equal pixels.</p>
<table><thead><tr><th>Scene</th><th></th><th>Match</th><th>Tick / first diff</th><th>Sounds</th><th>Input feel</th><th>First difference (facts-compare)</th></tr></thead>
<tbody>{"".join(idx)}</tbody></table>
{"".join(secs)}
<script type="application/json" id="blobs">{json.dumps(blobs.data)}</script>
<script>{PAGE_JS}</script></main></body></html>'''


# --- main ----------------------------------------------------------------------------

def d2_client_bin():
    env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0")
    subprocess.run(["cargo", "build", "--release", "-q", "-p", "d2-client"], cwd=REPO, env=env, check=True)
    target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(REPO, "target")
    return os.path.join(target, "release", "d2-client")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", required=True, help="output folder, outside the repository (rule 1)")
    ap.add_argument("--groups", default=",".join(g for g in GROUPS if g not in SKIP))
    ap.add_argument("--scenes", default="", help="only these scenes (comma list)")
    ap.add_argument("--reuse", action="store_true", help="keep existing 1.14d captures and d2rs dumps")
    ap.add_argument("--jobs", type=int, default=2, help="d2rs runs at once")
    ap.add_argument("--display", default=":97")
    a = ap.parse_args()
    out = os.path.abspath(a.out)
    if os.path.commonpath([out, REPO]) == REPO:
        raise SystemExit(f"--out {out} lies inside the repository: frames are game art (CLAUDE.md rule 1)")
    os.makedirs(out, exist_ok=True)
    only = {s for s in a.scenes.split(",") if s}
    jobs = []
    for name in a.groups.split(","):
        g = GROUPS[name]
        stop = os.environ.get("SBS_STOP_MARK")  # slow host: record only up to this mark
        if stop and f"mark {stop};" in g["script"]:
            g = dict(g, script=g["script"].split(f"mark {stop};")[0] + f"mark {stop}; waitticks 5; end")
        scenes = [s for s in g["scenes"] if (not only or s in only)
                  and (not stop or f"mark {g['scenes'][s][0]};" in g["script"])]
        if not scenes:
            continue
        cap = record_orig(name, g, os.path.join(out, "orig", name), a.reuse)
        frames, marks, sounds = read_capture(cap)
        _, st, nominal = framed(g["script"])
        steps = (st, nominal)
        jobs += [(name, s, g, cap, frames, marks, (steps, sounds)) for s in scenes]
    exe = d2_client_bin()
    with Xvfb(a.display, os.path.join(out, "xvfb.txt")), ThreadPoolExecutor(a.jobs) as pool:
        results = list(pool.map(lambda j: scene_result(exe, j[0], j[1], j[2], j[3], j[4], j[5], j[6], out,
                                                         a.display, a.reuse), jobs))
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=REPO, capture_output=True,
                            text=True).stdout.strip()
    meta = {"date": datetime.date.today().isoformat(), "tool": VERSION, "commit": commit,
            "d2_client_sha256": bin_sha(exe)[:16],
            "command": "python3 tools/sidebyside/build.py " + " ".join(sys.argv[1:])}
    summary = [{k: v for k, v in r.items() if k not in ("rows", "sounds")}
               | {"compared": len(r.get("rows", []))}
               | ({"sounds": {"text": r["sounds"]["text"], "first_diff": r["sounds"]["first_diff"],
                              "orig": len(r["sounds"]["orig"]), "d2rs": len(r["sounds"]["d2rs"])}}
                  if "sounds" in r else {}) for r in results]
    json.dump({"meta": meta, "scenes": summary}, open(os.path.join(out, "summary.json"), "w"), indent=1)
    path = os.path.join(out, "side-by-side.html")
    with open(path, "w", encoding="utf-8") as f:
        f.write(page(results, meta, Blobs()))
    log(f"wrote {path} ({os.path.getsize(path) // 1024} KiB)")
    for r in summary:
        log(f'{r["scene"]}: ' + (r["error"] if r.get("error") else
                                 f'{100 * r["match"]:.2f} %, first diff {r["first_diff"]}; {first_line(r["facts"])}; '
                                 f'sounds {r["sounds"]["text"]}; input {feel_summary(r)}'))


if __name__ == "__main__":
    main()
