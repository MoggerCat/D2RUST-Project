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
            side, _, script = rest.partition(" ")
            if side not in ("orig", "d2rs") or not script.strip():
                raise CheckError(f"line {n}: input orig|d2rs <script>")
            if side in c["input"]:
                raise CheckError(f"line {n}: 'input {side}' repeated")
            c["input"][side] = script.strip()
        elif kw == "ignore":
            c["ignore"] += toks
        elif kw == "at":
            # `at <frame> poke <directive> <args...>` (state injection, q-tool-poke):
            # kept verbatim, passed to both sides once they take --poke
            if len(toks) < 3 or toks[1] != "poke":
                raise CheckError(f"line {n}: at <frame> poke <directive> <args...>")
            c["poke"].append((n, num(toks[0], 0, 1_000_000), " ".join(toks[2:])))
        else:
            raise CheckError(f"line {n}: unknown keyword '{kw}'")
    for req in ("name", "save", "seed", "ticks"):
        if req not in seen:
            raise CheckError(f"missing '{req}' line")
    if "draws" in c["channels"] and c["draws_at"] is None:
        raise CheckError("channel draws needs a 'draws-at <tick>' line")
    if c["draws_at"] is not None and c["draws_at"] > c["ticks"]:
        raise CheckError("draws-at is after the last tick")
    return c


# --- running ------------------------------------------------------------------

class Runner:
    def __init__(self, check, work, dry=False, reuse=False):
        self.c, self.work, self.dry, self.reuse = check, work, dry, reuse
        self.game_dir = os.environ.get("D2_GAME_DIR") or (
            os.path.join(REPO, "game") if WINDOWS else os.path.expanduser("~/game"))
        self.log = []

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
        env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0")
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
               "--out", out] + args
        if c["input"].get("orig"):
            rec += ["--input", c["input"]["orig"]]
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
    def d2rs_common(self, save):
        c = self.c
        a = ["--save", save, "--seed", str(c["seed"]), "--difficulty", c["difficulty"]]
        return a

    # channels --------------------------------------------------------------
    def state(self, save, sides):
        orig, d2rs = self.path("orig.state.jsonl"), self.path("d2rs.state.jsonl")
        if "orig" in sides:
            self.recorder("record_state.py", ["--snap-every", "1"], orig)
        if "d2rs" in sides and not (self.reuse and os.path.exists(d2rs)):
            args = ["state-dump"] + self.d2rs_common(save) + ["--ticks", str(self.c["ticks"]),
                                                              "--out", d2rs]
            if self.c["input"].get("d2rs"):
                args += ["--input", self.c["input"]["d2rs"]]
            self.cargo("d2-client", args)
        if sides != {"orig", "d2rs"}:
            return None
        argv = [sys.executable, os.path.join(REC, "state_diff.py"), orig, d2rs,
                "--next", str(self.next)]
        if self.c["ignore"]:
            argv += ["--ignore", ",".join(self.c["ignore"])]
        return self.sh(argv, check=False)

    def draws(self, save, sides):
        c = self.c
        tick = c["draws_at"]
        raw = self.path("orig.frames.jsonl")
        scene_o, scene_d = self.path("draws-orig"), self.path("draws-d2rs")
        if "orig" in sides:
            self.recorder("record_frames.py", ["--every", "1", "--draws-every", "1"], raw)
            seq = None if self.dry else frame_seq_at(raw, tick)
            fr = [sys.executable, os.path.join(REC, "facts_render.py"), raw, "--scene", "s",
                  "--out", scene_o, "--images", self.path("captures")]
            if seq is not None:
                fr += ["--frame", str(seq)]
            self.sh(fr)
        if "d2rs" in sides:
            args = ["play"] + self.d2rs_common(save) + ["--dump-draws", scene_d, "--at-tick",
                                                       str(tick)]
            if c["input"].get("d2rs"):
                args += ["--input", c["input"]["d2rs"]]
            self.cargo("d2-client", args)
        if sides != {"orig", "d2rs"}:
            return None
        return self.sh(["cargo", "run", "--release", "-q", "-p", "d2-client", "--",
                        "facts-compare", os.path.join(scene_o, "scenes", "s"), scene_d,
                        "--ignore", "tick"], check=False)

    def not_available(self, ch):
        print(f"[{ch}] not compared: no d2rs recorder for this channel yet "
              f"(scenario-diff.md §3 rule 4)")
        return 2


def frame_seq_at(raw, tick):
    """seq of the first captured frame tied to server tick `tick` that has
    a draw log (record_frames' `frame` records: `f` = last server tick)."""
    import json
    best = None
    with open(raw, encoding="utf-8") as f:
        for line in f:
            if '"k": "frame"' not in line and '"k":"frame"' not in line:
                continue
            r = json.loads(line)
            if r.get("f") == tick and r.get("draws"):
                return r["seq"]
            if r.get("f") is not None and r["f"] <= tick and r.get("draws"):
                best = r["seq"]
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
    ok += 1
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
        if c["poke"]:
            print(f"note: {len(c['poke'])} poke line(s) not run (state injection not wired yet)")
            pending_poke = True
        else:
            pending_poke = False
        save = r.build_save()
        codes = {}
        for ch in c["channels"]:
            print(f"\n=== channel {ch} ===", flush=True)
            if ch == "state":
                codes[ch] = r.state(save, sides)
            elif ch == "draws":
                codes[ch] = r.draws(save, sides)
            else:
                codes[ch] = r.not_available(ch)
        if pending_poke:
            codes["poke"] = 2  # the run is not the check as written: partial at best
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
