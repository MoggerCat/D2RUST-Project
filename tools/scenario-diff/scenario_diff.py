"""One command per 1.14d check: build the save, run 1.14d with the chosen
recorders and d2rs with the same save / seed / input, and print the first
difference per channel (specs/tools/scenario-diff.md).

    python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check
        [--channels state,draws] [--work DIR] [--reuse] [--orig-only | --d2rs-only]
        [--next N] [--dry-run] [--reuse-orig] [--json FILE]
        [--orig-cache [DIR] [--fill-cache]]
    python3 tools/scenario-diff/scenario_diff.py --selftest

Linux (cloud): 1.14d runs under Wine through tools/cloud-game/run.sh
--python (setup: tools/cloud-game/README.md). Windows (PC 1): the
recorders run with this Python directly. D2_GAME_DIR names the install
(default $HOME/game on Linux, game/ in the repo on Windows).

D2RS_BIN_DIR (set by suite.py): run the prebuilt binaries d2-client,
d2-client-rng (the rng-trace feature build) and d2s-tool from there
instead of `cargo run` (parallel runs never wait on cargo's lock).

Exit code: the worst channel's (0 match, 1 diverged, 2 partial, 3 error).
Standard library only. Our own code.
"""

import argparse
import json
import os
import shlex
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import packets_channel  # noqa: E402  (the packets channel, scenario-diff.md §3)
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "trace-recorder"))
import send as send_msg  # noqa: E402  (`at … send` lines: the message syntax, scenario.md §3)
import rng_channel  # noqa: E402  (the rng channel, scenario-diff.md §3)
import items_channel  # noqa: E402  (the items channel, scenario-diff.md §3 rule 13)
import save_channel  # noqa: E402  (the save channel, scenario-diff.md §3 rule 14)
import orig_cache  # noqa: E402  (the shared 1.14d cache, scenario-diff.md §4)
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
REC = os.path.join(REPO, "tools", "trace-recorder")
DEFAULT_CACHE = os.path.join(REPO, "traces", "orig-cache")
FORMAT_LINE = "check 1"
CHANNELS = ("state", "draws", "rng", "packets", "items", "save", "frontend")
WINDOWS = os.name == "nt"


class CheckError(Exception):
    pass


# --- the check file (scenario-diff.md §2) -------------------------------------

def parse(text):
    """A check file -> dict. Strict: unknown keywords, repeats, missing
    required lines are errors naming the line."""
    c = {"input": {}, "ignore": [], "poke": [], "send": [], "difficulty": "normal", "seconds": 300,
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
        elif kw == "variant":
            # `variant <name>`: both sides run on the test variant install
            # built from traces/variants/<name>/<name>.d2stack (§2, §3 rule 9)
            once(kw)
            if len(toks) != 1 or not all(ch.isalnum() and ch == ch.lower() or ch == "-"
                                         for ch in toks[0]):
                raise CheckError(f"line {n}: variant <name> (one token of [a-z0-9-])")
            c["variant"] = toks[0]
        elif kw == "ignore":
            c["ignore"] += toks
        elif kw == "at":
            # `at <frame> poke <directive> <args...>` (specs/tools/poke.md §2 rule 6):
            # passed to both sides as --poke "<frame> <directive> <args...>";
            # `at <frame> send <Name> <field>=<value>...` / `at <frame> send hex <bytes>`
            # (§2 `at … send`, scenario.md §3): passed to both sides as --send
            if len(toks) < 3 or toks[1] not in ("poke", "send"):
                raise CheckError(f"line {n}: at <frame> poke <directive> <args...> | "
                                 f"at <frame> send <Name> <field>=<value>... | "
                                 f"at <frame> send hex <bytes>")
            f = num(toks[0], 1, 1_000_000)
            if toks[1] == "send":
                try:
                    msg = send_msg.parse_message(toks[2:])
                except send_msg.SendError as e:
                    raise CheckError(f"line {n}: send: {e}")
                c["send"].append((n, f, msg.text()))
            else:
                c["poke"].append((n, f, " ".join(toks[2:])))
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


SHARED_OPS = {"frame": 1, "move": 2, "click": 2, "rclick": 2, "hold": 3, "key": 1,
              "clickunit": (2, 4), "rclickunit": (2, 4)}


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
        n = SHARED_OPS[op]
        if len(a) not in (n if isinstance(n, tuple) else (n,)):
            return f"'{' '.join(w)}': {op} takes {' or '.join(map(str, n if isinstance(n, tuple) else (n,)))} argument(s)"
        if op == "key":
            continue
        if op in ("clickunit", "rclickunit"):
            # T C[,C..]|* [DX DY] (autostart.py's form)
            a = a[:1] + ([] if a[1] == "*" else a[1].split(",")) + a[2:]
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
    def __init__(self, check, work, dry=False, reuse=False, reuse_orig=False, orig_cache=None):
        self.c, self.work, self.dry, self.reuse = check, work, dry, reuse
        self.orig_cache = orig_cache   # orig_cache.OrigCache or None (--orig-cache)
        self.check_text = None         # the check file's text (part of the cache key)
        self._key_save = None
        self.reuse_orig = reuse_orig   # keep 1.14d outputs only (suite.py's cache)
        self.bin_dir = os.environ.get("D2RS_BIN_DIR") or None
        self.game_dir = os.environ.get("D2_GAME_DIR") or (
            os.path.join(REPO, "game") if WINDOWS else os.path.expanduser("~/game"))
        self.log = []
        self.base_game_dir = self.game_dir

    def use_variant(self):
        """`variant <name>` (§3 rule 9): build the variant install next to
        the base install (`data-tool variant build`, tools/test-variants.md;
        reused when it is there) and run both sides on it."""
        name = self.c.get("variant")
        if not name:
            return
        out = os.path.join(os.path.dirname(os.path.normpath(self.base_game_dir)), "variants",
                           name)
        stack = os.path.join("traces", "variants", name, name + ".d2stack")
        if self.dry or not os.path.exists(os.path.join(out, "Game.exe")):
            env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0",
                       D2_GAME_DIR=self.base_game_dir)
            self.sh(["cargo", "run", "--release", "-q", "-p", "data-tool", "--", "variant",
                     "build", stack, "--game", self.base_game_dir, "--out", out], env=env)
        print(f"variant: {name} -> {out}")
        self.game_dir = out

    def poke_args(self):
        """--poke "<frame> <directive> <args>" per `at` line, file order (both sides)."""
        out = []
        for _, frame, text in self.c["poke"]:
            out += ["--poke", f"{frame} {text}"]
        return out

    def send_args(self):
        """--send "<frame> <message>" per `at … send` line, file order (every 1.14d
        recorder, d2rs state-dump and play; scenario-diff.md §3 rule 12)."""
        out = []
        for _, frame, text in self.c["send"]:
            out += ["--send", f"{frame} {text}"]
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

    def cargo(self, pkg, args, timeout=7200, features=None):
        """`cargo run --release -p pkg [--features F] -- args`, or the prebuilt
        binary in D2RS_BIN_DIR (`<pkg>`, `<pkg>-rng` for the rng-trace build)."""
        env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0", D2_GAME_DIR=self.game_dir)
        if self.bin_dir:
            name = pkg + ("-rng" if features == "rng-trace" else "")
            exe = os.path.join(self.bin_dir, name + (".exe" if WINDOWS else ""))
            return self.sh([exe] + args, timeout=timeout, env=env)
        feat = ["--features", features] if features else []
        return self.sh(["cargo", "run", "--release", "-q", "-p", pkg] + feat + ["--"] + args,
                       timeout=timeout, env=env)

    def json_args(self, ch):
        """The comparator's --json summary file (scenario-diff.md §4)."""
        return ["--json", self.path(f"{ch}.summary.json")]

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
            target = os.path.join(dst, c["char"] + ".d2s")
            if os.path.exists(target):
                os.unlink(target)  # never write through a hard link (a `cp -al` prefix copy)
            shutil.copyfile(out, target)
        return out

    # 1.14d -----------------------------------------------------------------
    def recorder(self, script, args, out, ticks=None, sends=()):
        """Run one recorder on 1.14d with the check's start and input.
        `ticks`: a tick limit other than the check's; `sends`: (frame, text)
        messages after the check's own (the save channel's Save and Exit)."""
        c = self.c
        game = os.path.join(self.game_dir, "Game.exe")
        rec = [os.path.join(REC, script), "--game", game, "--seconds", str(c["seconds"]),
               "--ticks", str(ticks or c["ticks"]), "--auto", c["char"], "--seed", str(c["seed"]),
               "--difficulty", c["difficulty"], "--out", out] + args + self.poke_args() + self.send_args()
        for frame, text in sends:
            rec += ["--send", f"{frame} {text}"]
        if self.orig_input():
            rec += ["--input", self.orig_input()]
        if (self.reuse or self.reuse_orig) and os.path.exists(out):
            print(f"reuse {out}")
            return
        key = self.cache_key(script)
        if key and self.orig_cache.lookup(key, script, out):
            print(f"orig cache hit ({script}): {out} restored, no 1.14d run")
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
        if key and self.orig_cache.fill:
            cmd = ["tools/trace-recorder/" + script] + [
                "<Game.exe>" if a == game else (os.path.basename(a) if a == out else a)
                for a in rec[1:]]
            ok = self.orig_cache.store(key, script, out, cmd)
            print(f"orig cache {'filled' if ok else 'NOT filled (not small text)'}: {script}")

    def cache_key(self, script):
        """The orig cache key of a recorder run, or None (no cache, a dry run, or a
        recorder the cache does not know). The key save is d2s-tool's `--time 1` one."""
        if not self.orig_cache or self.dry or script not in orig_cache.RECORDERS:
            return None
        if self._key_save is None:
            ks = self.path("key.d2s")
            args = ["new", "--name", self.c["char"]] + self.c["save_args"]
            if "--time" not in self.c["save_args"]:
                args += ["--time", "1"]
            self.cargo("d2s-tool", args + ["-o", ks])
            with open(ks, "rb") as f:
                self._key_save = f.read()
        return orig_cache.make_key(self.check_text, self._key_save, self.game_dir, REC, script)

    # d2rs ------------------------------------------------------------------
    def orig_input(self):
        """The 1.14d recorders' --input: the shared script, else `input orig`."""
        return self.c["input"].get("shared") or self.c["input"].get("orig")

    def d2rs_input(self):
        """d2rs' --input: the shared script, else `input d2rs` (play's tick form;
        state-dump takes it only when it is in the shared form)."""
        return self.c["input"].get("shared") or self.c["input"].get("d2rs")

    def d2rs_common(self, save, play=False):
        """The d2rs options both commands share (`state-dump` and `play`:
        pokes, then sends, §3 rule 12)."""
        c = self.c
        a = ["--save", save, "--seed", str(c["seed"]), "--difficulty", c["difficulty"]]
        return a + self.poke_args() + self.send_args()

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
                "--next", str(self.next)] + self.json_args("state")
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
                args = [exe, "play"] + self.d2rs_common(save, play=True) + [
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
        code = self.sh([exe, "facts-compare", os.path.join(scene_o, "scenes", "s"), scene_d,
                        "--ignore", "tick"], check=False, timeout=300)
        if not self.dry:
            sm = draws_summary(os.path.join(scene_o, "scenes", "s", "draws.tsv"),
                               os.path.join(scene_d, "draws.tsv"), code, at)
            with open(self.path("draws.summary.json"), "w", encoding="utf-8") as f:
                json.dump(sm, f, indent=1)
        return code

    draws_timeout = 900  # seconds for one d2rs play run (lavapipe draws ~3 ticks/s)

    def d2_client_bin(self):
        """Builds d2-client (release) once and returns the binary's path; the
        play run then needs no cargo in its time limit."""
        if self.bin_dir:
            return os.path.join(self.bin_dir, "d2-client" + (".exe" if WINDOWS else ""))
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


DRAWS_INFO = ("i", "at", "seq", "tick")  # never compared (facts-compare's INFO + --ignore tick)


def read_draws(path):
    """(columns, rows) of a facts draws.tsv (`#` lines skipped)."""
    with open(path, encoding="utf-8") as f:
        lines = [ln.rstrip("\n") for ln in f if not ln.startswith("#") and ln.strip()]
    if not lines:
        return [], []
    return lines[0].split("\t"), [ln.split("\t") for ln in lines[1:]]


def draws_summary(orig_tsv, d2rs_tsv, code, tick):
    """The draws channel's summary (scenario-diff.md §4): rows aligned by
    position, a row equal when every compared column is (a '?' cell counts
    as equal, as facts-compare's unmeasured cells); one compared frame.
    `code` is facts-compare's exit code (its first difference also covers
    frame.tsv and sprites, which this count does not)."""
    verdict = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL"}.get(code, "ERROR")
    out = {"format": "diff-summary-1", "channel": "draws", "tool": "scenario_diff.py",
           "code": code, "verdict": verdict, "frames_compared": 1,
           "frames_equal": 1 if code == 0 else 0, "tick": tick}
    try:
        ca, ra = read_draws(orig_tsv)
        cb, rb = read_draws(d2rs_tsv)
    except OSError as e:
        out.update(rows_compared=0, rows_equal=0, first=None, error=str(e))
        return out
    cols = [c for c in ca if c in cb and c not in DRAWS_INFO]
    ia, ib = [ca.index(c) for c in cols], [cb.index(c) for c in cols]
    equal, first = 0, None
    for n in range(max(len(ra), len(rb))):
        a = ra[n] if n < len(ra) else None
        b = rb[n] if n < len(rb) else None
        diff = None
        if a is None or b is None:
            diff = ("<row>", "absent" if a is None else "row", "absent" if b is None else "row")
        else:
            for c, x, y in zip(cols, ia, ib):
                va, vb = (a[x] if x < len(a) else ""), (b[y] if y < len(b) else "")
                if va != vb and "?" not in (va, vb):
                    diff = (c, va, vb)
                    break
        if diff is None:
            equal += 1
        elif first is None:
            op = a[ca.index("op")] if a and "op" in ca else b[cb.index("op")] if b and "op" in cb \
                else "?"
            first = {"frame": tick, "text": f"tick {tick} draw row {n} ({op}) column {diff[0]}: "
                                            f"1.14d {diff[1]} vs d2rs {diff[2]}"}
    out.update(rows_compared=max(len(ra), len(rb)), rows_equal=equal, first=first,
               rows=(len(ra), len(rb)))
    return out


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
        "variant": GOOD + "variant Only_Fallen\n",
        "variant twice": GOOD + "variant a\nvariant b\n",
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
    # the check's difficulty reaches both sides (1.14d: autostart's config +0x210 write)
    assert "--difficulty normal" in next(x for x in r.log if "record_state.py" in x)
    nm = Runner(parse(GOOD + "difficulty nightmare\n"), "/tmp/w", dry=True)
    nm.next = 5
    nm.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    assert "--difficulty nightmare" in next(x for x in nm.log if "record_state.py" in x)
    assert "--difficulty nightmare" in next(x for x in nm.log if "state-dump" in x)
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
    # `at … send` lines (§2, §3 rule 10): canonical text, --send to every 1.14d
    # recorder (record_rng.py included), to state-dump and to play (draws)
    cs = parse(GOOD + "at 6 send InteractWithEntity id=@1:148 type=1\n"
               "at 6 send hex 2f 00 00 00 00 0C 00 00 00\n")
    assert cs["send"] == [(12, 6, "InteractWithEntity type=1 id=@1:148"),
                          (13, 6, "hex 2f 00 00 00 00 0c 00 00 00")], cs["send"]
    for bad in ("at 6 send", "at 6 send Nope a=1", "at 6 send Walk x=1", "at 6 send hex 1",
                "at 0 send Walk x=1 y=2", "at 6 send Chat", "at 6 sned Walk x=1 y=2"):
        try:
            parse(GOOD + bad + "\n")
            raise AssertionError(f"accepted {bad!r}")
        except CheckError:
            ok += 1
    r = Runner(cs, "/tmp/w", dry=True)
    r.next = 5
    r.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    r.draws("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    sq = ("--send '6 InteractWithEntity type=1 id=@1:148' "
          "--send '6 hex 2f 00 00 00 00 0c 00 00 00'")
    for what in ("record_state.py", "state-dump", "record_frames.py"):
        line = next(x for x in r.log if what in x)
        assert sq in line and line.index(poke) < line.index(sq), (what, line)
    assert sq in next(x for x in r.log if " play " in x)  # play takes the sends too
    # variant: built next to the base install, then both sides run on it
    cv = parse(GOOD + "variant only-fallen\n")
    r = Runner(cv, "/tmp/w", dry=True)
    r.game_dir = r.base_game_dir = "/g/game"
    r.next = 5
    r.use_variant()
    assert r.log[-1] == ("cargo run --release -q -p data-tool -- variant build "
                         "traces/variants/only-fallen/only-fallen.d2stack --game /g/game "
                         "--out /g/variants/only-fallen"), r.log[-1]
    assert r.game_dir == "/g/variants/only-fallen"
    r.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    assert "--game /g/variants/only-fallen/Game.exe" in next(x for x in r.log if "record_state" in x)
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
    assert shared_script_error("frame 3; clickunit 1 19,0x14; rclickunit 2 * 0 -8; key 1") is None
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
                "input frame 5; shot x", "input frame 5; clickunit 1", "input frame 5; clickunit 1 x",
                "input frame 5; clickunit 1 19 2"):
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
    # items: the packets recordings (once for both channels), items_diff.py
    ok += items_channel.selftest(Runner, parse(GOOD), shared_script_error)
    # save: record_state.py --write-save + Save and Exit send, state-dump --save-out, byte compare
    ok += save_channel.selftest(Runner, parse(GOOD), shared_script_error)
    # D2RS_BIN_DIR (suite.py): prebuilt binaries, no cargo; --reuse-orig keeps 1.14d only
    old = os.environ.get("D2RS_BIN_DIR")
    os.environ["D2RS_BIN_DIR"] = "/b"
    try:
        r = Runner(dict(parse(GOOD), poke=[], input={}), "/tmp/w", dry=True)
        r.next = 5
        r.build_save()
        r.state("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
        r.draws("/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
        rng_channel.run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
        assert not any(x.startswith("cargo") for x in r.log), r.log
        assert r.log[0].startswith("/b/d2s-tool new --name ScnAma"), r.log[0]
        assert any(x.startswith("/b/d2-client state-dump") for x in r.log)
        assert any(x.startswith("/b/d2-client play") for x in r.log)
        assert any(x.startswith("/b/d2-client-rng state-dump") and "--rng" in x for x in r.log)
    finally:
        if old is None:
            os.environ.pop("D2RS_BIN_DIR")
        else:
            os.environ["D2RS_BIN_DIR"] = old
    ok += 1
    with tempfile.TemporaryDirectory() as td:
        orig = os.path.join(td, "orig.state.jsonl")
        open(orig, "w").close()
        r = Runner(dict(parse(GOOD), poke=[], input={}), td, dry=True, reuse_orig=True)
        r.next = 5
        r.state(os.path.join(td, "ScnAma.d2s"), {"orig", "d2rs"})
        assert not any("record_state" in x for x in r.log) and \
            any("state-dump" in x for x in r.log), r.log
        ok += 1
        # --orig-cache: fill on a miss, hit on the same key, M08 miss on a changed check
        gd = os.path.join(td, "game")
        os.makedirs(gd)
        with open(os.path.join(gd, "Game.exe"), "wb") as f:
            f.write(b"exe")
        os.environ["D2_PRIVATE_REPO"] = os.path.join(td, "none")
        runs = []

        def fake_sh(argv, timeout=None, check=True, env=None):
            runs.append(argv)
            out = argv[len(argv) - 1 - argv[::-1].index("--out") + 1]
            with open(out, "w") as f:
                f.write('{"k":"snap","f":1}\n')
            return 0

        def cached(text, fill, wd):
            os.makedirs(wd, exist_ok=True)
            ca = orig_cache.OrigCache(os.path.join(td, "oc"), "a1-town-arrival-ama", fill=fill)
            rr = Runner(dict(parse(GOOD), poke=[], input={}), wd, orig_cache=ca)
            rr.game_dir, rr.check_text, rr._key_save, rr.sh = gd, text, b"save", fake_sh
            rr.recorder("record_state.py", ["--snap-every", "1"], os.path.join(wd, "orig.state.jsonl"))
            return os.path.join(wd, "orig.state.jsonl")

        cached(GOOD, True, os.path.join(td, "w1"))
        assert len(runs) == 1                       # miss: recorded and filled
        o2 = cached(GOOD, False, os.path.join(td, "w2"))
        assert len(runs) == 1 and os.path.exists(o2)  # hit: no 1.14d run
        cached(GOOD + "# changed\nignore fr\n", False, os.path.join(td, "w3"))
        assert len(runs) == 1                       # comments and ignore lines: still a hit
        cached(GOOD + "at 2 poke warp 1\n", False, os.path.join(td, "w4"))
        assert len(runs) == 2                       # M08: a changed check misses
        ok += 1
        # draws_summary: rows aligned by position, i / at / tick never compared, '?' equal
        hdr = "# facts v1\ni\top\tx\tlight\tat\n"
        a_ = os.path.join(td, "a.tsv")
        b_ = os.path.join(td, "b.tsv")
        with open(a_, "w") as f:
            f.write(hdr + "0\tStart\t1\t-\t0x1\n1\tFloor\t5\tab\t0x2\n2\tWall\t7\tcd\t0x3\n")
        with open(b_, "w") as f:
            f.write(hdr + "0\tStart\t1\t-\tx\n1\tFloor\t5\t?\ty\n2\tWall\t8\tcd\tz\n"
                    "3\tLine\t1\t-\tw\n")
        sm = draws_summary(a_, b_, 1, 73)
        assert (sm["rows_compared"], sm["rows_equal"]) == (4, 2), sm
        assert sm["first"]["text"] == "tick 73 draw row 2 (Wall) column x: 1.14d 7 vs d2rs 8", sm
        assert draws_summary(a_, a_, 0, 73)["rows_equal"] == 3
        # write_result: the codes and the comparators' summaries
        with open(os.path.join(td, "state.summary.json"), "w") as f:
            json.dump({"frames_compared": 3, "frames_equal": 2}, f)
        out = os.path.join(td, "res.json")
        write_result(out, {"name": "x"}, td, {"state": 1, "rng": 2, "draws": 3}, "boom")
        with open(out) as f:
            res = json.load(f)
        assert res["channels"]["state"]["summary"]["frames_equal"] == 2, res
        assert res["channels"]["rng"]["summary"] is None and res["error"] == "boom", res
        assert res["channels"]["draws"]["verdict"] == "ERROR", res
        ok += 3
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


def write_result(path, c, work, codes, error=None):
    """--json: {"format": "scenario-diff-result-1", "check", "channels": {ch:
    {"code", "verdict", "summary": the comparator's diff-summary-1 or None}},
    "error"} (scenario-diff.md §4)."""
    names = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR", None: "RECORDED"}
    chans = {}
    for ch, code in codes.items():
        sm = None
        p = os.path.join(work, f"{ch}.summary.json") if work else None
        if p and code in (0, 1, 2) and os.path.exists(p):
            try:
                with open(p, encoding="utf-8") as f:
                    sm = json.load(f)
            except (OSError, ValueError):
                sm = None
        chans[ch] = {"code": code, "verdict": names.get(code, str(code)), "summary": sm}
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"format": "scenario-diff-result-1", "check": c and c.get("name"),
                   "channels": chans, "error": error}, f, indent=1)


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
    ap.add_argument("--reuse-orig", action="store_true",
                    help="reuse the 1.14d outputs already in the work dir; re-run d2rs (suite.py)")
    ap.add_argument("--orig-cache", nargs="?", const=DEFAULT_CACHE, default=None, metavar="DIR",
                    help="use the shared cache of recorded 1.14d sides (default "
                         "traces/orig-cache); a miss records 1.14d as usual (orig_cache.py)")
    ap.add_argument("--cache-no-read", action="store_true",
                    help="with --orig-cache: never read it (record 1.14d; --fill-cache refills)")
    ap.add_argument("--fill-cache", action="store_true",
                    help="with --orig-cache: store each fresh 1.14d recording in the cache")
    ap.add_argument("--json", default=None, metavar="FILE",
                    help="write the per-channel codes and comparator summaries here (§4)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not a.check:
        ap.error("a check file is needed")
    c = work = ch = None
    codes = {}
    try:
        with open(a.check, encoding="utf-8") as f:
            check_text = f.read()
        c = parse(check_text)
        if os.path.basename(a.check) != c["name"] + ".check":
            raise CheckError(f"file name must be {c['name']}.check")
        if a.channels:
            c["channels"] = [x for x in a.channels.split(",") if x]
        work = a.work or os.path.join(REPO, "traces", "raw", "check-" + c["name"])
        os.makedirs(work, exist_ok=True)
        oc = None
        if a.orig_cache:
            oc = orig_cache.OrigCache(a.orig_cache, c["name"], fill=a.fill_cache,
                                   read=not a.cache_no_read)
        elif a.fill_cache:
            ap.error("--fill-cache needs --orig-cache")
        r = Runner(c, work, dry=a.dry_run, reuse=a.reuse, reuse_orig=a.reuse_orig, orig_cache=oc)
        r.check_text = check_text
        r.next = a.next
        sides = {"orig", "d2rs"} - ({"d2rs"} if a.orig_only else set()) - (
            {"orig"} if a.d2rs_only else set())
        r.use_variant()
        save = r.build_save()
        for ch in c["channels"]:
            print(f"\n=== channel {ch} ===", flush=True)
            stale = r.path(f"{ch}.summary.json")
            if not a.dry_run and os.path.exists(stale):
                os.unlink(stale)  # a summary is this run's or none
            if ch == "state":
                codes[ch] = r.state(save, sides)
            elif ch == "draws":
                codes[ch] = r.draws(save, sides)
            elif ch == "packets":
                codes[ch] = packets_channel.run(r, save, sides, shared_script_error)
            elif ch == "rng":
                codes[ch] = rng_channel.run(r, save, sides)
            elif ch == "items":
                codes[ch] = items_channel.run(r, save, sides, shared_script_error)
            elif ch == "save":
                r.shared_error = shared_script_error
                codes[ch] = save_channel.run(r, save, sides)
            elif ch == "frontend":
                import frontend_channel
                codes[ch] = frontend_channel.run(r, save, sides)
            else:
                codes[ch] = r.not_available(ch)
    except (CheckError, OSError, subprocess.TimeoutExpired) as e:
        print(f"error: {e}", file=sys.stderr)
        if a.json:
            if ch is not None and ch not in codes:
                codes[ch] = 3
            write_result(a.json, c, work, codes, str(e))
        return 3
    if a.json and not a.dry_run:
        write_result(a.json, c, work, codes)
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
