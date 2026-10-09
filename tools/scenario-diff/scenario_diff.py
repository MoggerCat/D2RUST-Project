"""One command per 1.14d check: build the save, run 1.14d with the chosen
recorders and d2rs with the same save / seed / input, and print the first
difference per channel (specs/tools/scenario-diff.md).

    python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check
        [--channels state,draws] [--work DIR] [--reuse] [--orig-only | --d2rs-only]
        [--next N] [--dry-run]
    python3 tools/scenario-diff/scenario_diff.py --selftest

Linux (cloud): 1.14d runs under Wine through tools/cloud-game/run.sh
--python (setup: tools/cloud-game/README.md). Windows (PC 1): the
recorders run with this Python directly. D2_GAME_DIR names the install
(default $HOME/game on Linux, game/ in the repo on Windows).

Exit code: the worst channel's (0 match, 1 diverged, 2 partial, 3 error).
Standard library only. Our own code.
"""

import argparse
import os
import shlex
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import packets_channel  # noqa: E402  (the packets channel, scenario-diff.md §3)
import rng_channel  # noqa: E402  (the rng channel, scenario-diff.md §3)
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
REC = os.path.join(REPO, "tools", "trace-recorder")
FORMAT_LINE = "check 1"
CHANNELS = ("state", "draws", "rng", "packets")
WINDOWS = os.name == "nt"


class CheckError(Exception):
    pass


# --- the check file (scenario-diff.md §2) -------------------------------------

def parse(text):
    """A check file -> dict. Strict: unknown keywords, repeats, missing
    required lines are errors naming the line."""
    c = {"input": {}, "ignore": [], "poke": [], "difficulty": "normal", "seconds": 300,
         "channels": ["state"], "save_args": [], "draws_at": None}
    seen = set()
    lines = [(n, ln.split("#", 1)[0].strip() if not ln.lstrip().startswith("input ")
              else ln.strip()) for n, ln in enumerate(text.splitlines(), 1)]
    lines = [(n, ln) for n, ln in lines if ln]
    if not lines or lines[0][1] != FORMAT_LINE:
        raise CheckError(f"line {lines[0][0] if lines else 1}: first line must be '{FORMAT_LINE}'")
    for n, ln in lines[1:]:
        kw, _, rest = ln.partition(" ")
        rest = rest.strip()
        toks = rest.split()

        def once(key):
            if key in seen:
                raise CheckError(f"line {n}: '{key}' repeated")
            seen.add(key)

        def num(s, lo, hi):
            try:
                v = int(s, 0)
            except ValueError:
                raise CheckError(f"line {n}: '{s}' is not a number")
            if not lo <= v <= hi:
                raise CheckError(f"line {n}: {v} outside {lo}..{hi}")
            return v

        if kw == "name":
            once(kw)
            if len(toks) != 1 or not all(ch.isalnum() and ch == ch.lower() or ch == "-"
                                         for ch in toks[0]):
                raise CheckError(f"line {n}: name is one token of [a-z0-9-]")
            c["name"] = toks[0]
        elif kw == "save":
            once(kw)
            if not toks:
                raise CheckError(f"line {n}: save <Char> [d2s-tool new options]")
            c["char"], c["save_args"] = toks[0], toks[1:]
        elif kw == "seed":
            once(kw)
            if len(toks) != 1:
                raise CheckError(f"line {n}: seed <n>")
            c["seed"] = num(toks[0], 1, 0x7FFFFFFF)
        elif kw == "ticks":
            once(kw)
            if len(toks) != 1:
                raise CheckError(f"line {n}: ticks <n>")
            c["ticks"] = num(toks[0], 1, 1_000_000)
        elif kw == "seconds":
            once(kw)
            c["seconds"] = num(toks[0] if len(toks) == 1 else "x", 1, 86_400)
        elif kw == "difficulty":
            once(kw)
            if toks not in (["normal"], ["nightmare"], ["hell"]):
                raise CheckError(f"line {n}: difficulty normal|nightmare|hell")
            c["difficulty"] = toks[0]
        elif kw == "channels":
            once(kw)
            bad = [t for t in toks if t not in CHANNELS]
            if not toks or bad or len(set(toks)) != len(toks):
                raise CheckError(f"line {n}: channels from {' '.join(CHANNELS)}, each once")
            c["channels"] = toks
        elif kw == "draws-at":
            once(kw)
            c["draws_at"] = num(toks[0] if len(toks) == 1 else "x", 1, 1_000_000)
        elif kw == "input":
            # `input <script>` (shared, both sides, §2 rule 4) or `input orig|d2rs <script>`
            side, _, script = rest.partition(" ")
            if side not in ("orig", "d2rs"):
                side, script = "shared", rest
                err = shared_script_error(script)
                if err:
                    raise CheckError(f"line {n}: input <shared script>: {err}")
            if not script.strip():
                raise CheckError(f"line {n}: input [orig|d2rs] <script>")
            if side in c["input"]:
                raise CheckError(f"line {n}: 'input {side}' repeated")
            c["input"][side] = script.strip()
        elif kw == "ignore":
            c["ignore"] += toks
        elif kw == "at":
            # `at <frame> poke <directive> <args...>` (specs/tools/poke.md §2 rule 6):
            # passed to both sides as --poke "<frame> <directive> <args...>"
            if len(toks) < 3 or toks[1] != "poke":
                raise CheckError(f"line {n}: at <frame> poke <directive> <args...>")
            c["poke"].append((n, num(toks[0], 1, 1_000_000), " ".join(toks[2:])))
        else:
            raise CheckError(f"line {n}: unknown keyword '{kw}'")
    for req in ("name", "save", "seed", "ticks"):
        if req not in seen:
            raise CheckError(f"missing '{req}' line")
    if "shared" in c["input"] and len(c["input"]) > 1:
        raise CheckError("a shared 'input <script>' and 'input orig|d2rs' lines exclude each other")
    if "draws" in c["channels"] and c["draws_at"] is None:
        raise CheckError("channel draws needs a 'draws-at <tick>' line")
    if c["draws_at"] is not None and c["draws_at"] > c["ticks"]:
        raise CheckError("draws-at is after the last tick")
    return c


SHARED_OPS = {"frame": 1, "move": 2, "click": 2, "rclick": 2, "hold": 3, "key": 1}


def shared_script_error(text):
    """Why `text` is not the shared input form (scenario-diff.md §2 rule 4),
    or None: `;`-separated steps from SHARED_OPS, the first `frame F`, frames
    >= 1 and never going back, integer arguments (key: any token)."""
    steps = [w.split() for w in text.split(";") if w.split()]
    if not steps or steps[0][0] != "frame":
        return "the script starts with 'frame F'"
    last = 0
    for w in steps:
        op, a = w[0], w[1:]
        if op not in SHARED_OPS:
            return f"'{op}' is not a shared step ({', '.join(SHARED_OPS)})"
        if len(a) != SHARED_OPS[op]:
            return f"'{' '.join(w)}': {op} takes {SHARED_OPS[op]} argument(s)"
        if op == "key":
            continue
        try:
            v = [int(x, 0) for x in a]
        except ValueError:
            return f"'{' '.join(w)}': integer arguments"
        if op == "frame":
            if v[0] < 1 or v[0] < last:
                return f"'{' '.join(w)}': frames are >= 1 and never go back"
            last = v[0]
        elif op == "hold" and v[2] < 1:
            return f"'{' '.join(w)}': hold N >= 1 frames"
    return None


# --- running ------------------------------------------------------------------

class Runner:
    def __init__(self, check, work, dry=False, reuse=False):
        self.c, self.work, self.dry, self.reuse = check, work, dry, reuse
        self.game_dir = os.environ.get("D2_GAME_DIR") or (
            os.path.join(REPO, "game") if WINDOWS else os.path.expanduser("~/game"))
        self.log = []

    def poke_args(self):
        """--poke "<frame> <directive> <args>" per `at` line, file order (both sides)."""
        out = []
        for _, frame, text in self.c["poke"]:
            out += ["--poke", f"{frame} {text}"]
        return out

    def sh(self, argv, timeout=None, check=True, env=None):
        line = " ".join(shlex.quote(a) for a in argv)
        self.log.append(line)
        print(f"$ {line}", flush=True)
        if self.dry:
            return 0
        r = subprocess.run(argv, cwd=REPO, timeout=timeout, env=env)
        if check and r.returncode != 0:
            raise CheckError(f"command failed ({r.returncode}): {line}")
        return r.returncode

    def path(self, *p):
        return os.path.join(self.work, *p)

    def cargo(self, pkg, args, timeout=7200):
        env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0", D2_GAME_DIR=self.game_dir)
        return self.sh(["cargo", "run", "--release", "-q", "-p", pkg, "--"] + args,
                       timeout=timeout, env=env)

    # save ----------------------------------------------------------------
    def save_dir_orig(self):
        if os.environ.get("D2_SAVE_DIR"):
            return os.environ["D2_SAVE_DIR"]
        if WINDOWS:
            return os.path.join(os.path.expanduser("~"), "Saved Games", "Diablo II")
        prefix = os.environ.get("WINEPREFIX", os.path.expanduser("~/.wine-d2"))
        user = os.environ.get("USER") or "root"
        return os.path.join(prefix, "drive_c", "users", user, "Saved Games", "Diablo II")

    def build_save(self):
        c = self.c
        out = self.path(c["char"] + ".d2s")
        if not (self.reuse and os.path.exists(out)):
            self.cargo("d2s-tool", ["new", "--name", c["char"]] + c["save_args"] + ["-o", out])
        dst = self.save_dir_orig()
        print(f"save: {out} -> {dst}")
        if not self.dry:
            os.makedirs(dst, exist_ok=True)
            shutil.copyfile(out, os.path.join(dst, c["char"] + ".d2s"))
        return out

    # 1.14d -----------------------------------------------------------------
    def recorder(self, script, args, out):
        """Run one recorder on 1.14d with the check's start and input."""
        c = self.c
        game = os.path.join(self.game_dir, "Game.exe")
        rec = [os.path.join(REC, script), "--game", game, "--seconds", str(c["seconds"]),
               "--ticks", str(c["ticks"]), "--auto", c["char"], "--seed", str(c["seed"]),
               "--out", out] + args + self.poke_args()
        if self.orig_input():
            rec += ["--input", self.orig_input()]
        if self.reuse and os.path.exists(out):
            print(f"reuse {out}")
            return
        if WINDOWS:
            self.sh([sys.executable] + rec, timeout=c["seconds"] + 120)
        else:
            run = os.path.join(REPO, "tools", "cloud-game", "run.sh")
            self.sh([run, "--python", "--seconds", str(c["seconds"] + 60),
                     "--out", self.path("run-" + script.split(".")[0])] + ["--"] + rec,
                    timeout=c["seconds"] + 180, check=False)
        if not self.dry and not os.path.exists(out):
            raise CheckError(f"{script} wrote no {out} (see {self.path('run-' + script.split('.')[0])})")

    # d2rs ------------------------------------------------------------------
    def orig_input(self):
        """The 1.14d recorders' --input: the shared script, else `input orig`."""
        return self.c["input"].get("shared") or self.c["input"].get("orig")

    def d2rs_input(self):
        """d2rs' --input: the shared script, else `input d2rs` (play's tick form;
        state-dump takes it only when it is in the shared form)."""
        return self.c["input"].get("shared") or self.c["input"].get("d2rs")

    def d2rs_common(self, save):
        c = self.c
        a = ["--save", save, "--seed", str(c["seed"]), "--difficulty", c["difficulty"]]
        return a + self.poke_args()

    # channels --------------------------------------------------------------
    def state(self, save, sides):
        orig, d2rs = self.path("orig.state.jsonl"), self.path("d2rs.state.jsonl")
        if "orig" in sides:
            self.recorder("record_state.py", ["--snap-every", "1"], orig)
        if "d2rs" in sides and not (self.reuse and os.path.exists(d2rs)):
            args = ["state-dump"] + self.d2rs_common(save) + ["--ticks", str(self.c["ticks"]),
                                                              "--out", d2rs]
            if self.d2rs_input() and not shared_script_error(self.d2rs_input()):
                args += ["--input", self.d2rs_input()]
            self.cargo("d2-client", args)
        if sides != {"orig", "d2rs"}:
            return None
        argv = [sys.executable, os.path.join(REC, "state_diff.py"), orig, d2rs,
                "--next", str(self.next)]
        if self.c["ignore"]:
            argv += ["--ignore", ",".join(self.c["ignore"])]
        code = self.sh(argv, check=False)
        if self.d2rs_input() and shared_script_error(self.d2rs_input()):
            # state-dump takes the shared frame form only (scenario-diff.md §3 rule 8):
            # the d2rs side ran without this input, so a match is only partial
            print("[state] d2rs ran without 'input d2rs' (not in the shared frame form, "
                  "which state-dump needs): partial at best")
            code = max(code, 2) if code != 1 else 1
        return code

    def not_available(self, ch):
        print(f"[{ch}] not compared: no d2rs recorder for this channel yet "
              f"(scenario-diff.md §3 rule 4)")
        return 2

    def draws(self, save, sides):
        """draws (scenario-diff.md §3 rule 3, rule 7): 1.14d frame capture with
        draw logs -> facts_render on the frame of `draws-at`; d2rs `play
        --dump-draws --at-tick` (Linux: Xvfb + Vulkan); facts-compare."""
        c = self.c
        tick = c["draws_at"]
        raw = self.path("orig.frames.jsonl")
        scene_o, scene_d = self.path("draws-orig"), self.path("draws-d2rs")
        if "orig" in sides:
            self.recorder("record_frames.py", ["--every", "1", "--draws-every", "1", "--no-save"],
                          raw)
        # the compared tick: 1.14d's last drawn frame at or before draws-at
        # (which ticks it draws depends on its frame pacing, rule 7.2); d2rs
        # then dumps that same tick
        seq = at = None
        if not self.dry and ("orig" in sides or os.path.exists(raw)):
            seq, at = frame_seq_at(raw, tick)
            if seq is None:
                raise CheckError(f"{raw}: no captured frame with a draw log at or before tick "
                                 f"{tick} (see {self.path('run-record_frames')})")
            if at != tick:
                print(f"[draws] 1.14d drew no frame at tick {tick}: comparing tick {at} "
                      f"(seq {seq}) on both sides")
        at = at or tick
        if "orig" in sides:
            fr = [sys.executable, os.path.join(REC, "facts_render.py"), raw, "--scene", "s",
                  "--out", scene_o, "--frame", str(seq if seq is not None else "SEQ")]
            self.sh(fr, timeout=600)
        need_bin = "d2rs" in sides or sides == {"orig", "d2rs"}
        exe = self.d2_client_bin() if need_bin else None
        if "d2rs" in sides:
            dump = os.path.join(scene_d, "draws.tsv")
            if self.reuse and os.path.exists(dump) and scene_tick(scene_d) == at:
                print(f"reuse {scene_d}")
            else:
                if not self.dry and os.path.isdir(scene_d):
                    shutil.rmtree(scene_d)  # a stale dump never counts as this run's
                args = [exe, "play"] + self.d2rs_common(save) + [
                    "--dump-draws", scene_d, "--at-tick", str(at)]
                if self.d2rs_input():
                    args += ["--input", self.d2rs_input()]
                with self.display() as env:
                    code = self.sh(args, timeout=self.draws_timeout, check=False, env=env)
                if not self.dry and not os.path.exists(dump):
                    raise CheckError(f"d2rs play wrote no {dump} (exit {code})")
                if code != 0:
                    # the dump is complete when play prints its line; a failure
                    # after it (leave / teardown) is reported, not fatal
                    print(f"[draws] warning: d2rs play exited {code} after writing the dump")
        if sides != {"orig", "d2rs"}:
            return None
        return self.sh([exe, "facts-compare", os.path.join(scene_o, "scenes", "s"), scene_d,
                        "--ignore", "tick"], check=False, timeout=300)

    draws_timeout = 900  # seconds for one d2rs play run (lavapipe draws ~3 ticks/s)

    def d2_client_bin(self):
        """Builds d2-client (release) once and returns the binary's path; the
        play run then needs no cargo in its time limit."""
        env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0")
        self.sh(["cargo", "build", "--release", "-q", "-p", "d2-client"], timeout=7200, env=env)
        target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(REPO, "target")
        return os.path.join(target, "release", "d2-client" + (".exe" if WINDOWS else ""))

    def display(self):
        """Environment for a windowed d2rs run. Windows: as is. Linux: an X
        display (DISPLAY if it answers, else an own Xvfb on D2_DRAWS_DISPLAY,
        default :98, stopped afterwards) and WGPU_BACKEND=vulkan (Mesa
        lavapipe without a GPU); scenario-diff.md §3 rule 7."""
        return _Display(self)


class _Display:
    def __init__(self, runner):
        self.r, self.proc = runner, None

    def __enter__(self):
        env = dict(os.environ, D2_GAME_DIR=self.r.game_dir)
        if WINDOWS:
            return env
        env.setdefault("WGPU_BACKEND", "vulkan")
        if self.r.dry:
            env["DISPLAY"] = os.environ.get("D2_DRAWS_DISPLAY", ":98")
            print(f"(Xvfb {env['DISPLAY']}, WGPU_BACKEND={env['WGPU_BACKEND']})")
            return env
        missing = [p for p, ok in (
            ("xvfb (Xvfb)", shutil.which("Xvfb")),
            ("x11-utils (xdpyinfo)", shutil.which("xdpyinfo")),
            ("mesa-vulkan-drivers (lvp_icd)", any(
                os.path.exists(os.path.join(d, "vulkan", "icd.d", f))
                for d in ("/usr/share", "/etc", "/usr/local/share")
                for f in ("lvp_icd.json", "lvp_icd.x86_64.json"))),
            ("libxkbcommon-x11-0", any(os.path.exists(os.path.join(d, "libxkbcommon-x11.so.0"))
                                       for d in ("/usr/lib/x86_64-linux-gnu", "/usr/lib64",
                                                 "/usr/lib", "/usr/lib/aarch64-linux-gnu"))),
        ) if not ok]
        if missing:
            raise CheckError("d2rs draws need " + ", ".join(missing) +
                             " (tools/cloud-setup.sh; specs/tools/scenario-diff.md §3 rule 7)")

        def answers(d):
            return subprocess.run(["xdpyinfo", "-display", d], stdout=subprocess.DEVNULL,
                                  stderr=subprocess.DEVNULL).returncode == 0

        cur = os.environ.get("DISPLAY")
        if cur and answers(cur) and not os.environ.get("D2_DRAWS_DISPLAY"):
            return env
        d = os.environ.get("D2_DRAWS_DISPLAY", ":98")
        env["DISPLAY"] = d
        if not answers(d):
            log = open(self.r.path("xvfb-d2rs.txt"), "w")
            self.proc = subprocess.Popen(["Xvfb", d, "-screen", "0", "1024x768x24", "-nolisten",
                                          "tcp"], stdout=log, stderr=log)
            for _ in range(100):
                if answers(d):
                    break
                if self.proc.poll() is not None:
                    raise CheckError(f"Xvfb {d} exited {self.proc.returncode} "
                                     f"(see {self.r.path('xvfb-d2rs.txt')})")
                time.sleep(0.1)
            else:
                raise CheckError(f"Xvfb {d} did not answer in 10 s")
        print(f"(d2rs on X display {d}, WGPU_BACKEND={env['WGPU_BACKEND']})")
        return env

    def __exit__(self, *exc):
        if self.proc:
            self.proc.terminate()
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        return False


def scene_tick(scene):
    """The `tick` row of a scene's frame.tsv (facts-render.md §3), or None."""
    try:
        with open(os.path.join(scene, "frame.tsv"), encoding="utf-8") as f:
            for line in f:
                k, _, v = line.rstrip("\n").partition("\t")
                if k == "tick":
                    return int(v)
    except (OSError, ValueError):
        pass
    return None


def frame_seq_at(raw, tick):
    """(seq, f) of the captured frame with a draw log tied to server tick
    `tick` (record_frames' `frame` records: `seq` capture number, `f` the
    last server tick, `draws` the log), else the last such frame before
    it; (None, None) if none. Under the debugger with a draw log on every
    capture, 1.14d draws about one frame per two ticks (measured
    2026-10-09: frames on odd ticks 3..79), so not every tick has a frame."""
    import json
    best = (None, None)
    with open(raw, encoding="utf-8") as f:
        for line in f:
            if '"k": "frame"' not in line and '"k":"frame"' not in line:
                continue
            r = json.loads(line)
            if not r.get("draws") or r.get("f") is None:
                continue
            if r["f"] == tick:
                return r["seq"], r["f"]
            if r["f"] <= tick:
                best = (r["seq"], r["f"])
    return best


# --- self-test ------------------------------------------------------------------

GOOD = """check 1
# comment
name a1-town-arrival-ama
save ScnAma --class ama --expansion
seed 1234
ticks 20
channels state draws
draws-at 13
input orig wait 1; end
ignore fr
at 5 poke pos @player 4880 4230
"""


def selftest():
    ok = 0
    c = parse(GOOD)
    assert c["name"] == "a1-town-arrival-ama" and c["char"] == "ScnAma"
    assert c["save_args"] == ["--class", "ama", "--expansion"] and c["seed"] == 1234
    assert c["channels"] == ["state", "draws"] and c["draws_at"] == 13
    assert c["input"] == {"orig": "wait 1; end"} and c["ignore"] == ["fr"]
    assert c["poke"] == [(11, 5, "pos @player 4880 4230")]
    ok += 1
    bad = {
        "first line": GOOD.replace("check 1", "check 2"),
        "unknown keyword": GOOD + "frobnicate 1\n",
        "repeated": GOOD + "seed 5\n",
        "missing seed": GOOD.replace("seed 1234\n", ""),
        "number": GOOD.replace("ticks 20", "ticks x"),
        "range": GOOD.replace("seed 1234", "seed 0"),
        "channel": GOOD.replace("channels state draws", "channels state pixels"),
        "draws-at missing": GOOD.replace("draws-at 13\n", ""),
        "draws-at late": GOOD.replace("draws-at 13", "draws-at 99"),
        "input side": GOOD.replace("input orig", "input both"),
        "poke": GOOD + "at 5 spawn 1 2 3\n",
        "poke frame": GOOD + "at 0 poke time 1 0\n",
        "name": GOOD.replace("name a1-town-arrival-ama", "name A_B"),
    }
    for what, text in bad.items():
        try:
            parse(text)
        except CheckError:
            ok += 1
            continue
        raise AssertionError(f"accepted a bad file ({what})")
    # dry run: the commands both sides would run
    r = Runner(c, "/tmp/w", dry=True)
    r.next = 5
    r.build_save()
    r.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    joined = "\n".join(r.log)
    assert "record_state.py" in joined and "--auto ScnAma --seed 1234" in joined
    assert "state-dump --save /tmp/w/ScnAma.d2s --seed 1234" in joined
    assert "state_diff.py" in joined and "--ignore fr" in joined
    # the poke line reaches both sides as --poke "<frame> <directive args>"
    poke = "--poke '5 pos @player 4880 4230'"
    rec_line = next(x for x in r.log if "record_state.py" in x)
    dump_line = next(x for x in r.log if "state-dump" in x)
    assert poke in rec_line and poke in dump_line, (rec_line, dump_line)
    r.log = []
    r.draws("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    assert poke in next(x for x in r.log if "record_frames.py" in x)
    assert poke in next(x for x in r.log if " play " in x)
    # draws: the capture, the frame of draws-at, a built binary, the dump, the compare
    joined = "\n".join(r.log)
    assert "record_frames.py" in joined and "--every 1 --draws-every 1 --no-save" in joined
    assert "facts_render.py /tmp/w/orig.frames.jsonl --scene s" in joined and "--frame" in joined
    assert "cargo build --release -q -p d2-client" in joined
    play = next(x for x in r.log if " play " in x)
    assert "--dump-draws /tmp/w/draws-d2rs --at-tick 13" in play and "cargo" not in play
    assert "--seed 1234" in play
    cmp_line = r.log[-1]
    assert "facts-compare /tmp/w/draws-orig/scenes/s /tmp/w/draws-d2rs --ignore tick" in cmp_line
    r.log = []
    r.draws("/tmp/w/ScnAma.d2s", {"d2rs"})  # one side: no compare
    assert not any("facts-compare" in x or "record_frames" in x for x in r.log)
    ok += 1
    # frame_seq_at: record_frames' frame records (seq, f, draws), odd ticks only
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        p = os.path.join(td, "f.jsonl")
        with open(p, "w", encoding="utf-8") as fh:
            fh.write('{"k": "header"}\n{"k": "tick", "f": 71}\n')
            for seq, f, dr in ((35, 71, [1]), (36, 73, [1]), (37, 75, None), (38, 77, [1])):
                fh.write(f'{{"k": "frame", "seq": {seq}, "f": {f}, "draws": {dr and "[1]" or "null"}}}\n')
        assert frame_seq_at(p, 73) == (36, 73)
        assert frame_seq_at(p, 74) == (36, 73)  # 75 has no draw log
        assert frame_seq_at(p, 70) == (None, None)
        with open(os.path.join(td, "frame.tsv"), "w", encoding="utf-8") as fh:
            fh.write("# facts v1\nkey\tvalue\nseq\t36\ntick\t73\n")
        assert scene_tick(td) == 73 and scene_tick(os.path.join(td, "none")) is None
    ok += 1
    # a shared `input <script>` (§2 rule 4) reaches both sides of both channels
    walk = "frame 10; click 600 300; hold 400 200 3; key r"
    cs = parse(GOOD.replace("input orig wait 1; end", "input " + walk))
    assert cs["input"] == {"shared": walk}
    r = Runner(cs, "/tmp/w", dry=True)
    r.next = 5
    r.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    r.draws("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    q = "--input " + shlex.quote(walk)
    for what in ("record_state.py", "state-dump", "record_frames.py", " play "):
        assert q in next(x for x in r.log if what in x), what
    ok += 1
    for bad in ("input wait 1; click 1 2", "input click 1 2", "input frame 0; click 1 2",
                "input frame 5; frame 4", "input frame 5; hold 1 2", "input frame 5; click a b",
                "input frame 5; shot x"):
        try:
            parse(GOOD.replace("input orig wait 1; end", bad))
        except CheckError:
            ok += 1
            continue
        raise AssertionError(f"accepted a bad shared input ({bad})")
    try:
        parse(GOOD + "input " + walk + "\n")      # shared and `input orig` together
        raise AssertionError("accepted shared and side input together")
    except CheckError:
        ok += 1
    # `input d2rs` in play's tick form: not given to state-dump (partial), given to play
    cd = parse(GOOD + "input d2rs wait 3; click 1 2\n")
    r = Runner(cd, "/tmp/w", dry=True)
    r.next = 5
    r.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    r.draws("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    assert "--input" not in next(x for x in r.log if "state-dump" in x)
    assert "--input 'wait 3; click 1 2'" in next(x for x in r.log if " play " in x)
    assert "--input 'wait 1; end'" in next(x for x in r.log if "record_state.py" in x)
    ok += 1
    # packets: record_packets.py, state-dump --packets, packets_diff.py (dry run)
    ok += packets_channel.selftest(Runner, parse(GOOD), shared_script_error)
    # rng: record_rng.py --frames, state-dump --rng (rng-trace feature), rng_diff.py
    ok += rng_channel.selftest(Runner, parse(GOOD))
    # every check file in traces/checks parses
    d = os.path.join(REPO, "traces", "checks")
    for fn in sorted(os.listdir(d)) if os.path.isdir(d) else []:
        if fn.endswith(".check"):
            with open(os.path.join(d, fn), encoding="utf-8") as f:
                cc = parse(f.read())
            assert cc["name"] + ".check" == fn, f"{fn}: name line differs"
            ok += 1
    print(f"scenario_diff selftest: {ok} checks passed")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("check", nargs="?", help="traces/checks/<name>.check")
    ap.add_argument("--channels", default=None, help="override the file's channels")
    ap.add_argument("--work", default=None, help="work dir (default traces/raw/check-<name>)")
    ap.add_argument("--reuse", action="store_true", help="reuse outputs already in the work dir")
    ap.add_argument("--orig-only", action="store_true")
    ap.add_argument("--d2rs-only", action="store_true")
    ap.add_argument("--next", type=int, default=20)
    ap.add_argument("--dry-run", action="store_true", help="print the commands only")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not a.check:
        ap.error("a check file is needed")
    try:
        with open(a.check, encoding="utf-8") as f:
            c = parse(f.read())
        if os.path.basename(a.check) != c["name"] + ".check":
            raise CheckError(f"file name must be {c['name']}.check")
        if a.channels:
            c["channels"] = [x for x in a.channels.split(",") if x]
        work = a.work or os.path.join(REPO, "traces", "raw", "check-" + c["name"])
        os.makedirs(work, exist_ok=True)
        r = Runner(c, work, dry=a.dry_run, reuse=a.reuse)
        r.next = a.next
        sides = {"orig", "d2rs"} - ({"d2rs"} if a.orig_only else set()) - (
            {"orig"} if a.d2rs_only else set())
        save = r.build_save()
        codes = {}
        for ch in c["channels"]:
            print(f"\n=== channel {ch} ===", flush=True)
            if ch == "state":
                codes[ch] = r.state(save, sides)
            elif ch == "draws":
                codes[ch] = r.draws(save, sides)
            elif ch == "packets":
                codes[ch] = packets_channel.run(r, save, sides, shared_script_error)
            elif ch == "rng":
                codes[ch] = rng_channel.run(r, save, sides)
            else:
                codes[ch] = r.not_available(ch)
    except (CheckError, OSError, subprocess.TimeoutExpired) as e:
        print(f"error: {e}", file=sys.stderr)
        return 3
    names = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR", None: "recorded only"}
    print("\n=== summary ===")
    for ch, code in codes.items():
        print(f"{ch}: {'dry run' if a.dry_run else names.get(code, code)}")
    if a.dry_run:
        print("(dry run: nothing was run)")
        return 0
    real = [x for x in codes.values() if x is not None]
    return max(real) if real else 0


if __name__ == "__main__":
    sys.exit(main())
