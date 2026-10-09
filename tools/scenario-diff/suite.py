"""The check suite: every 1.14d check in traces/checks through scenario_diff.py
in parallel, a match % per check, area and overall, and the playthrough's
playability per act next to it (specs/tools/scenario-diff.md §4).

    python3 tools/scenario-diff/suite.py [--filter GLOB] [--area A[,B]] [--workers N]
        [--orig-cache [DIR] [--fill-cache]] [--fresh] [--no-checks] [--no-playthrough] [--json F] [--md F]
        [--no-build] [--auto-after S] [--dry-run]
    python3 tools/scenario-diff/suite.py --selftest

Per check: work dir traces/raw/suite/<name>, scenario_diff.py --work there
--json result.json. The 1.14d recordings are reused when the check file,
the save (d2s-tool's bytes with a fixed --time) and Game.exe's sha256 are
unchanged (the key in <work>/suite.key); then only d2rs and the comparators
run. --fresh records 1.14d again. Workers: one Wine prefix each
(~/.wine-d2-suite-<k>, a hard-link copy of the base prefix with its own
registry files and save folder) and one X display each (:90+k for 1.14d,
:100+k for d2rs' draws window). d2rs is built once up front into
target/suite-bin (d2-client, d2s-tool, d2-client-rng when a check runs
the rng channel) and handed to scenario_diff through D2RS_BIN_DIR.

Exit code 0 (whatever the checks found), 3 when the suite itself failed
(build, prefix, no check). Standard library only. Our own code.
"""

import argparse
import fnmatch
import glob
import hashlib
import json
import os
import queue
import shlex
import shutil
import subprocess
import sys
import threading
import time

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import scenario_diff as sd  # noqa: E402

REPO = sd.REPO
CHECKS_DIR = os.path.join(REPO, "traces", "checks")
PLAY_DIR = os.path.join(REPO, "traces", "playthrough")
PLAYTHROUGH = os.path.join(REPO, "tools", "playthrough", "playthrough.py")
SUITE_DIR = os.path.join(REPO, "traces", "raw", "suite")
KEY_FORMAT = "suite-key-1"
RESULT_FORMAT = "suite-result-1"
# the 1.14d outputs of each channel (scenario-diff.md §3), removed for a fresh run
ORIG_OUTPUTS = {"state": ["orig.state.jsonl", "run-record_state"],
                "draws": ["orig.frames.jsonl", "run-record_frames", "draws-orig"],
                "rng": ["orig.rng.jsonl", "run-record_rng"],
                "packets": ["orig.packets.jsonl", "run-record_packets"],
                "items": ["orig.packets.jsonl", "run-record_packets"]}  # packets' recording
# measured 2026-10-09 under Wine (scenario-diff.md §4): the menu is left as soon
# as it is up (autostart waits for launcher mode 4), 0 s passed 6 runs in a row
AUTO_AFTER = "0"
VERDICT_ORDER = ("MATCH", "DIVERGED", "PARTIAL", "ERROR", "RECORDED")


class SuiteError(Exception):
    pass


# --- checks ---------------------------------------------------------------------

def area_of(name):
    return name.split("-", 1)[0]


def discover(checks_dir=CHECKS_DIR, pattern=None, areas=None):
    """[(name, path, parsed check)] of the check files, sorted by name; a
    glob on the name (or file name) and an area list narrow it."""
    out = []
    for p in sorted(glob.glob(os.path.join(checks_dir, "*.check"))):
        name = os.path.basename(p)[:-len(".check")]
        if pattern and not (fnmatch.fnmatch(name, pattern) or
                            fnmatch.fnmatch(os.path.basename(p), pattern)):
            continue
        if areas and area_of(name) not in areas:
            continue
        with open(p, encoding="utf-8") as f:
            text = f.read()
        out.append((name, p, sd.parse(text)))
    return out


def cost(c):
    """A rough order (slowest first): draws, rng, then ticks."""
    ch = c["channels"]
    return (("draws" in ch) * 2 + ("rng" in ch), c["ticks"] * len(ch))


# --- build -----------------------------------------------------------------------

def build(bin_dir, rng, run=subprocess.run, log=print):
    """cargo builds d2-client (+ the rng-trace feature build when `rng`) and
    d2s-tool once, then hard-links the binaries into bin_dir (cargo replaces
    target/release/<bin> on the next build, never writes through it). The
    plain build runs last so target/release/d2-client stays the plain one."""
    env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0")
    target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(REPO, "target")
    exe = ".exe" if sd.WINDOWS else ""
    os.makedirs(bin_dir, exist_ok=True)
    steps = []
    if rng:
        steps.append((["cargo", "build", "--release", "-q", "-p", "d2-client", "--features",
                       "rng-trace"], [("d2-client", "d2-client-rng")]))
    steps.append((["cargo", "build", "--release", "-q", "-p", "d2-client", "-p", "d2s-tool"],
                  [("d2-client", "d2-client"), ("d2s-tool", "d2s-tool")]))
    for argv, links in steps:
        log("$ " + " ".join(argv))
        r = run(argv, cwd=REPO, env=env)
        if r.returncode != 0:
            raise SuiteError(f"build failed ({r.returncode}): {' '.join(argv)}")
        for src, dst in links:
            s, d = os.path.join(target, "release", src + exe), os.path.join(bin_dir, dst + exe)
            if os.path.exists(d):
                os.unlink(d)
            try:
                os.link(s, d)
            except OSError:
                shutil.copy2(s, d)


# --- Wine prefixes ------------------------------------------------------------------

def save_folder(prefix):
    user = os.environ.get("USER") or "root"
    return os.path.join(prefix, "drive_c", "users", user, "Saved Games")


def make_prefix(base, dst):
    """A hard-link copy of the Wine prefix `base` at `dst` (once): the
    registry files and the save folder as real copies (Wine and the save
    step replace them; a hard link would write through to `base`), no run
    lock (a shared lock inode would serialize the prefixes)."""
    if os.path.isdir(dst):
        return False
    if not os.path.isfile(os.path.join(base, "system.reg")):
        raise SuiteError(f"no Wine prefix at {base} (tools/cloud-game/README.md setup)")
    tmp = dst + ".tmp"
    if os.path.exists(tmp):
        shutil.rmtree(tmp)
    r = subprocess.run(["cp", "-al", base, tmp])
    if r.returncode != 0:
        raise SuiteError(f"cp -al {base} {tmp} failed")
    lock = os.path.join(tmp, ".run.lock")
    if os.path.exists(lock):
        os.unlink(lock)
    for reg in glob.glob(os.path.join(base, "*.reg")):
        d = os.path.join(tmp, os.path.basename(reg))
        os.unlink(d)
        shutil.copy2(reg, d)
    s = save_folder(tmp)
    if os.path.isdir(s):
        shutil.rmtree(s)
    if os.path.isdir(save_folder(base)):
        shutil.copytree(save_folder(base), s, symlinks=True)
    os.rename(tmp, dst)
    return True


# --- the 1.14d cache key -------------------------------------------------------------

def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def key_save_args(c, out):
    """d2s-tool arguments of the key save: the check's save with a fixed
    --time (d2s-tool otherwise stamps the current time)."""
    args = ["new", "--name", c["char"]] + c["save_args"]
    if "--time" not in c["save_args"]:
        args += ["--time", "1"]
    return args + ["-o", out]


def cache_key(check_text, save_bytes, game_sha):
    return {"format": KEY_FORMAT, "check": hashlib.sha256(check_text.encode()).hexdigest(),
            "save": hashlib.sha256(save_bytes).hexdigest(), "game_exe": game_sha}


def read_key(work):
    try:
        with open(os.path.join(work, "suite.key"), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def key_matches(old, new):
    return bool(old) and all(old.get(k) == new[k] for k in ("format", "check", "save", "game_exe"))


def clear_orig(work):
    """Remove the 1.14d outputs (and the key) of a work dir."""
    for names in ORIG_OUTPUTS.values():
        for n in names:
            p = os.path.join(work, n)
            if os.path.isdir(p):
                shutil.rmtree(p)
            elif os.path.exists(p):
                os.unlink(p)
    p = os.path.join(work, "suite.key")
    if os.path.exists(p):
        os.unlink(p)


def orig_seconds(work):
    """Seconds the 1.14d runs of this work dir took (run.sh's run.json)."""
    t = 0.0
    for p in glob.glob(os.path.join(work, "run-*", "run.json")):
        try:
            with open(p, encoding="utf-8") as f:
                t += float(json.load(f).get("seconds") or 0)
        except (OSError, ValueError):
            pass
    return round(t, 1)


# --- one check ------------------------------------------------------------------------

def run_check(job, k, opts, game_sha, log):
    """Runs one check in worker slot k; returns its record for the report."""
    name, path, c = job
    work = os.path.join(opts.suite_dir, name)
    os.makedirs(work, exist_ok=True)
    rec = {"name": name, "area": area_of(name), "check": os.path.relpath(path, REPO),
           "work": os.path.relpath(work, REPO), "channels": {}, "error": None,
           "reused_orig": False, "worker": k}
    t0 = time.time()
    env = dict(os.environ, WINEPREFIX=os.path.join(opts.prefix_root, f".wine-d2-suite-{k}"),
               D2_DISPLAY=f":{90 + k}", D2_DRAWS_DISPLAY=f":{100 + k}",
               D2_AUTO_AFTER=opts.auto_after)
    if opts.bin_dir:
        env["D2RS_BIN_DIR"] = opts.bin_dir
    env.pop("D2_SAVE_DIR", None)  # the save goes to the worker's prefix
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
        key = None
        if not opts.dry_run:
            ks = os.path.join(work, "key.d2s")
            exe = os.path.join(opts.bin_dir, "d2s-tool" + (".exe" if sd.WINDOWS else ""))
            kenv = dict(env, D2_GAME_DIR=env.get("D2_GAME_DIR") or os.path.expanduser("~/game"))
            r = subprocess.run([exe] + key_save_args(c, ks), env=kenv, capture_output=True,
                               text=True, timeout=600)
            if r.returncode != 0:
                raise SuiteError(f"d2s-tool (key save) failed: {r.stderr.strip()[:200]}")
            with open(ks, "rb") as f:
                key = cache_key(text, f.read(), game_sha)
        reuse = (not opts.fresh and key is not None and key_matches(read_key(work), key))
        if not reuse and not opts.dry_run:
            clear_orig(work)
        rec["reused_orig"] = reuse
        argv = [sys.executable, os.path.join(HERE, "scenario_diff.py"), path, "--work", work,
                "--json", os.path.join(work, "result.json"), "--next", "5"]
        if reuse:
            argv.append("--reuse-orig")
        if opts.dry_run:
            argv.append("--dry-run")
        if getattr(opts, "orig_cache", None):
            argv += ["--orig-cache", opts.orig_cache] + (
                (["--fill-cache"] if opts.fill_cache else []) +
                (["--cache-no-read"] if opts.fresh else []))
        log(f"[w{k}] {name}: {'reuse 1.14d' if reuse else 'record 1.14d'}")
        res_path = os.path.join(work, "result.json")
        if os.path.exists(res_path):
            os.unlink(res_path)
        limit = (c["seconds"] + 300) * len(c["channels"]) + 1800
        with open(os.path.join(work, "suite.log"), "w", encoding="utf-8") as lf:
            lf.write("$ " + " ".join(shlex.quote(a) for a in argv) + "\n")
            lf.flush()
            r = subprocess.run(argv, cwd=REPO, env=env, stdout=lf, stderr=subprocess.STDOUT,
                               timeout=limit)
        rec["exit"] = r.returncode
        if getattr(opts, "orig_cache", None):
            with open(os.path.join(work, "suite.log"), encoding="utf-8", errors="replace") as lf:
                rec["orig_cache_hits"] = lf.read().count("orig cache hit")
        try:
            with open(res_path, encoding="utf-8") as f:
                res = json.load(f)
        except (OSError, ValueError):
            res = {"channels": {}, "error": f"scenario_diff exit {r.returncode}, no result "
                                            f"(see {os.path.relpath(work, REPO)}/suite.log)"}
        rec["channels"] = {ch: v for ch, v in (res.get("channels") or {}).items()}
        if res.get("error"):
            rec["error"] = res["error"]
        if not opts.dry_run and key is not None and any(
                os.path.exists(os.path.join(work, ORIG_OUTPUTS[ch][0])) for ch in c["channels"]):
            with open(os.path.join(work, "suite.key"), "w", encoding="utf-8") as f:
                json.dump(key, f, indent=1)
    except (SuiteError, OSError, subprocess.TimeoutExpired, sd.CheckError) as e:
        rec["error"] = str(e)
    for ch in c["channels"]:
        rec["channels"].setdefault(ch, {"code": 3, "verdict": "ERROR", "summary": None})
    rec["seconds"] = round(time.time() - t0, 1)
    rec["orig_seconds"] = 0.0 if rec["reused_orig"] else orig_seconds(work)
    return rec


def run_all(jobs, opts, game_sha, log=print):
    """The jobs on opts.workers threads, slowest first; records in name order."""
    q = queue.Queue()
    for j in sorted(jobs, key=lambda j: cost(j[2]), reverse=True):
        q.put(j)
    out, lock = [], threading.Lock()

    def worker(k):
        while True:
            try:
                j = q.get_nowait()
            except queue.Empty:
                return
            rec = run_check(j, k, opts, game_sha, log)
            with lock:
                out.append(rec)
                log(f"[w{k}] {rec['name']}: " + ", ".join(
                    f"{ch} {v['verdict']}" for ch, v in rec["channels"].items())
                    + f" ({rec['seconds']} s)")

    threads = [threading.Thread(target=worker, args=(k,)) for k in range(1, opts.workers + 1)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    return sorted(out, key=lambda r: r["name"])


# --- match % ---------------------------------------------------------------------------

def channel_numbers(v):
    """(ticks compared, ticks equal, match % or None, first divergence) of
    one channel record. draws: the % is equal rows / rows."""
    sm = v.get("summary") or {}
    n, e = sm.get("frames_compared") or 0, sm.get("frames_equal") or 0
    if v.get("code") == 3:
        n = e = 0
    if "rows_compared" in sm and sm["rows_compared"]:
        pct = 100.0 * sm["rows_equal"] / sm["rows_compared"]
    else:
        pct = 100.0 * e / n if n else None
    first = (sm.get("first") or {}).get("text")
    return n, e, pct, first


def aggregate(records):
    """Per area and overall: checks, channel runs, ticks compared, ticks
    equal, % and verdict counts."""
    def empty():
        return {"checks": 0, "runs": 0, "ticks": 0, "equal": 0,
                "verdicts": {v: 0 for v in VERDICT_ORDER}}
    areas, total = {}, empty()
    for r in records:
        a = areas.setdefault(r["area"], empty())
        for agg in (a, total):
            agg["checks"] += 1
        for ch, v in r["channels"].items():
            n, e, _, _ = channel_numbers(v)
            for agg in (a, total):
                agg["runs"] += 1
                agg["ticks"] += n
                agg["equal"] += e
                verdict = v.get("verdict") or "ERROR"
                agg["verdicts"][verdict] = agg["verdicts"].get(verdict, 0) + 1
    for agg in list(areas.values()) + [total]:
        agg["pct"] = round(100.0 * agg["equal"] / agg["ticks"], 1) if agg["ticks"] else None
    return areas, total


# --- playthrough -----------------------------------------------------------------------

def run_playthrough(work, timeout=3600, log=print):
    """playthrough.py --json on every traces/playthrough/act*.play; returns
    {"acts": [...], "error"}. A missing script or file is reported, not an error."""
    plays = sorted(glob.glob(os.path.join(PLAY_DIR, "act*.play")))
    if not os.path.exists(PLAYTHROUGH):
        return {"acts": [], "error": f"no {os.path.relpath(PLAYTHROUGH, REPO)}"}
    if not plays:
        return {"acts": [], "error": "no traces/playthrough/act*.play"}
    os.makedirs(work, exist_ok=True)
    out = os.path.join(work, "playthrough.json")
    if os.path.exists(out):
        os.unlink(out)
    argv = [sys.executable, PLAYTHROUGH] + plays + ["--json", out, "--work", work]
    log("$ " + " ".join(shlex.quote(a) for a in argv))
    t0 = time.time()
    try:
        env = dict(os.environ, D2_GAME_DIR=os.environ.get("D2_GAME_DIR") or (
            os.path.join(REPO, "game") if sd.WINDOWS else os.path.expanduser("~/game")))
        with open(os.path.join(work, "playthrough.log"), "w", encoding="utf-8") as lf:
            r = subprocess.run(argv, cwd=REPO, stdout=lf, stderr=subprocess.STDOUT,
                               timeout=timeout, env=env)
        code = r.returncode
    except subprocess.TimeoutExpired:
        return {"acts": [], "error": f"playthrough.py: time limit {timeout} s"}
    try:
        with open(out, encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, ValueError):
        return {"acts": [], "error": f"playthrough.py exit {code}, no JSON "
                                     f"(see {os.path.relpath(work, REPO)}/playthrough.log)"}
    acts = [act_row(a) for a in (data.get("acts") or [])]
    return {"acts": acts, "error": None, "exit": code, "seconds": round(time.time() - t0, 1)}


def act_row(a):
    """One act of playthrough.py's JSON as {act, reached, total,
    consecutive, first_blocker}: the act's own keys when present (the
    `--all` contract), else counted from its milestones."""
    ms = a.get("milestones") or []
    reached = a.get("reached")
    if reached is None:
        reached = sum(1 for m in ms if m.get("status") == "reached")
    total = a.get("total")
    if total is None:
        total = len(ms)
    cons = a.get("consecutive")
    if cons is None:
        cons = 0
        for m in ms:
            if m.get("status") != "reached":
                break
            cons += 1
    fb = a.get("first_blocker", "absent")
    if fb == "absent":
        m = next((m for m in ms if m.get("status") != "reached"), None)
        fb = None if m is None else {"milestone": m.get("name"), "kind": m.get("status"),
                                     "frame": m.get("frame"), "evidence": m.get("evidence")}
    return {"act": a.get("act"), "reached": reached, "total": total, "consecutive": cons,
            "first_blocker": fb}


def blocker_text(fb):
    if not fb:
        return "-"
    fr = fb.get("frame")
    return (f"{fb.get('milestone')} ({fb.get('kind')})" + (f" frame {fr}" if fr is not None else "")
            + f": {fb.get('evidence') or ''}").rstrip(": ")


# --- report ------------------------------------------------------------------------------

def pct_text(p):
    return "-" if p is None else f"{p:.1f}%"


def clip(s, n):
    s = (s or "-").replace("\n", " ")
    return s if len(s) <= n else s[:n - 1] + "…"


def verdict_counts(v):
    return " ".join(f"{k} {v[k]}" for k in VERDICT_ORDER if v.get(k))


def text_report(res):
    L = []
    w = L.append
    if res.get("checks") is not None:
        recs = res["checks"]
        t = res.get("timing") or {}
        w(f"checks: {len(recs)}, workers {res.get('workers')}, wall {t.get('wall', '?')} s "
          f"(1.14d {t.get('orig', '?')} s, reused for {t.get('reused', 0)}/{len(recs)} checks)")
        w(f"{'check':<28} {'channel':<8} {'ticks':>6} {'equal':>6} {'match':>7}  "
          f"{'verdict':<9} first divergence")
        for r in recs:
            for ch, v in r["channels"].items():
                n, e, p, first = channel_numbers(v)
                if v.get("code") == 3:
                    first = r.get("error") or first
                w(f"{clip(r['name'], 28):<28} {ch:<8} {n:>6} {e:>6} {pct_text(p):>7}  "
                  f"{v.get('verdict', '?'):<9} {clip(first, 90)}")
        w("")
        w(f"{'area':<10} {'checks':>6} {'runs':>5} {'ticks':>7} {'equal':>7} {'match':>7}  verdicts")
        for a, agg in sorted(res["areas"].items()):
            w(f"{a:<10} {agg['checks']:>6} {agg['runs']:>5} {agg['ticks']:>7} {agg['equal']:>7} "
              f"{pct_text(agg['pct']):>7}  {verdict_counts(agg['verdicts'])}")
        agg = res["overall"]
        w(f"{'overall':<10} {agg['checks']:>6} {agg['runs']:>5} {agg['ticks']:>7} "
          f"{agg['equal']:>7} {pct_text(agg['pct']):>7}  {verdict_counts(agg['verdicts'])}")
    if res.get("playthrough") is not None:
        pt = res["playthrough"]
        w("")
        w("playability (playthrough.py, d2rs only):")
        if pt.get("error"):
            w(f"  not available: {pt['error']}")
        w(f"{'act':<5} {'reached':>8} {'consec.':>8}  first blocker")
        for a in pt.get("acts", []):
            w(f"{str(a['act']):<5} {a['reached']:>4}/{a['total']:<3} {a['consecutive']:>8}  "
              f"{clip(blocker_text(a['first_blocker']), 100)}")
    return "\n".join(L) + "\n"


def md_cell(s):
    return (s or "-").replace("|", "\\|").replace("\n", " ")


def md_report(res):
    L = []
    w = L.append
    w(f"Suite run {res.get('date', '')} (`{res.get('command', 'suite.py')}`)")
    w("")
    if res.get("checks") is not None:
        t = res.get("timing") or {}
        w(f"Checks: {len(res['checks'])}, workers {res.get('workers')}, wall {t.get('wall')} s, "
          f"1.14d reused for {t.get('reused', 0)}.")
        w("")
        w("| Check | Channel | Ticks | Equal | Match | Verdict | First divergence |")
        w("|---|---|---|---|---|---|---|")
        for r in res["checks"]:
            for ch, v in r["channels"].items():
                n, e, p, first = channel_numbers(v)
                if v.get("code") == 3:
                    first = r.get("error") or first
                w(f"| {r['name']} | {ch} | {n} | {e} | {pct_text(p)} | {v.get('verdict')} | "
                  f"{md_cell(clip(first, 160))} |")
        w("")
        w("| Area | Checks | Runs | Ticks | Equal | Match | Verdicts |")
        w("|---|---|---|---|---|---|---|")
        for a, agg in sorted(res["areas"].items()) + [("**overall**", res["overall"])]:
            w(f"| {a} | {agg['checks']} | {agg['runs']} | {agg['ticks']} | {agg['equal']} | "
              f"{pct_text(agg['pct'])} | {verdict_counts(agg['verdicts'])} |")
        w("")
    if res.get("playthrough") is not None:
        pt = res["playthrough"]
        if pt.get("error"):
            w(f"Playthrough not available: {pt['error']}")
            w("")
        w("| Act | Reached | Furthest consecutive | First blocker |")
        w("|---|---|---|---|")
        for a in pt.get("acts", []):
            w(f"| {a['act']} | {a['reached']}/{a['total']} | {a['consecutive']} | "
              f"{md_cell(clip(blocker_text(a['first_blocker']), 160))} |")
    return "\n".join(L) + "\n"


# --- main ------------------------------------------------------------------------------------

def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--filter", default=None, help="glob on the check name")
    ap.add_argument("--area", default=None, help="areas (first dash token of the name), comma list")
    ap.add_argument("--checks-dir", default=CHECKS_DIR,
                    help="directory of the .check files (default traces/checks; the generated "
                         "ones are in traces/checks/gen)")
    ap.add_argument("--workers", type=int, default=max(1, min((os.cpu_count() or 2) - 1, 3)))
    ap.add_argument("--orig-cache", nargs="?", const=sd.DEFAULT_CACHE, default=None, metavar="DIR",
                    help="use the shared 1.14d cache (default traces/orig-cache; a miss records "
                         "1.14d as usual; orig_cache.py)")
    ap.add_argument("--fill-cache", action="store_true",
                    help="with --orig-cache: store fresh 1.14d recordings in the cache")
    ap.add_argument("--fresh", action="store_true", help="record 1.14d again (no reuse)")
    ap.add_argument("--no-checks", action="store_true")
    ap.add_argument("--no-playthrough", action="store_true")
    ap.add_argument("--no-build", action="store_true",
                    help="use the binaries already in target/suite-bin")
    ap.add_argument("--auto-after", default=os.environ.get("D2_AUTO_AFTER") or AUTO_AFTER,
                    help=f"autostart's menu delay in seconds (default {AUTO_AFTER}, measured)")
    ap.add_argument("--json", default=None, metavar="F")
    ap.add_argument("--md", default=None, metavar="F")
    ap.add_argument("--dry-run", action="store_true", help="scenario_diff --dry-run per check")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    a.suite_dir = SUITE_DIR
    a.prefix_root = os.path.expanduser("~")
    a.bin_dir = os.path.join(os.environ.get("CARGO_TARGET_DIR") or os.path.join(REPO, "target"),
                             "suite-bin")
    res = {"format": RESULT_FORMAT, "date": time.strftime("%Y-%m-%d %H:%M UTC", time.gmtime()),
           "command": "suite.py " + " ".join(shlex.quote(x) for x in (argv or sys.argv[1:])),
           "workers": a.workers, "checks": None, "playthrough": None}
    t0 = time.time()
    try:
        jobs = [] if a.no_checks else discover(
            a.checks_dir, pattern=a.filter, areas=set(x for x in (a.area or "").split(",") if x) or None)
        if not a.no_checks and not jobs:
            raise SuiteError("no check matches")
        game_sha = None
        if jobs and not a.dry_run:
            if not a.no_build:
                build(a.bin_dir, any("rng" in c["channels"] for _, _, c in jobs))
            game_dir = os.environ.get("D2_GAME_DIR") or os.path.expanduser("~/game")
            game_sha = sha256_file(os.path.join(game_dir, "Game.exe"))
            base = os.environ.get("WINEPREFIX") or os.path.expanduser("~/.wine-d2")
            for k in range(1, a.workers + 1):
                if make_prefix(base, os.path.join(a.prefix_root, f".wine-d2-suite-{k}")):
                    print(f"prefix ~/.wine-d2-suite-{k}: hard-link copy of {base}")
        elif not a.no_build and not a.no_playthrough and not a.dry_run:
            build(a.bin_dir, False)
        pt_box = {}
        pt_thread = None
        if not a.no_playthrough and not a.dry_run:
            pt_thread = threading.Thread(
                target=lambda: pt_box.update(r=run_playthrough(os.path.join(SUITE_DIR,
                                                                            "playthrough"))))
            pt_thread.start()
        if jobs:
            recs = run_all(jobs, a, game_sha)
            areas, total = aggregate(recs)
            res.update(checks=recs, areas=areas, overall=total)
            res["timing"] = {"wall": round(time.time() - t0, 1),
                             "orig": round(sum(r["orig_seconds"] for r in recs), 1),
                             "checks_sum": round(sum(r["seconds"] for r in recs), 1),
                             "reused": sum(1 for r in recs if r["reused_orig"])}
        if pt_thread:
            pt_thread.join()
            res["playthrough"] = pt_box.get("r") or {"acts": [], "error": "no result"}
    except (SuiteError, OSError, sd.CheckError) as e:
        print(f"suite: error: {e}", file=sys.stderr)
        return 3
    res.setdefault("timing", {})["total_wall"] = round(time.time() - t0, 1)
    print()
    sys.stdout.write(text_report(res))
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump(res, f, indent=1)
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(md_report(res))
    return 0


# --- self-test ---------------------------------------------------------------------------------

def selftest():
    import tempfile
    ok = 0
    with tempfile.TemporaryDirectory() as td:
        # discovery: glob, area (first dash token), the committed checks parse
        for n in ("state-a", "state-b", "walk-x"):
            with open(os.path.join(td, n + ".check"), "w") as f:
                f.write(f"check 1\nname {n}\nsave C\nseed 1\nticks {len(n)}\n"
                        + ("channels rng\n" if n == "state-b" else ""))
        assert [x[0] for x in discover(td)] == ["state-a", "state-b", "walk-x"]
        assert [x[0] for x in discover(td, pattern="*-b")] == ["state-b"]
        assert [x[0] for x in discover(td, areas={"walk"})] == ["walk-x"]
        assert [x[0] for x in discover(td, areas={"state"})] == ["state-a", "state-b"]
        assert discover(CHECKS_DIR), "no committed check parses"
        jobs = discover(td)
        assert sorted(jobs, key=lambda j: cost(j[2]), reverse=True)[0][0] == "state-b"
        ok += 5
        # the cache key: check text, save bytes, Game.exe; a change in any one misses
        k = cache_key("check 1", b"save", "abc")
        assert key_matches(k, cache_key("check 1", b"save", "abc"))
        for other in (cache_key("check 1 ", b"save", "abc"), cache_key("check 1", b"sav", "abc"),
                      cache_key("check 1", b"save", "abd"), None, {}):
            assert not key_matches(other, k)
        c = jobs[0][2]
        assert key_save_args(c, "/k.d2s") == ["new", "--name", "C", "--time", "1", "-o", "/k.d2s"]
        ok += 2
        # clear_orig removes the 1.14d outputs and the key, keeps d2rs'
        w = os.path.join(td, "w")
        os.makedirs(os.path.join(w, "run-record_state"))
        for n in ("orig.state.jsonl", "d2rs.state.jsonl", "suite.key"):
            open(os.path.join(w, n), "w").close()
        clear_orig(w)
        assert sorted(os.listdir(w)) == ["d2rs.state.jsonl"], os.listdir(w)
        ok += 1
        # make_prefix: hard links for the bulk, real copies of *.reg and saves, no lock
        base = os.path.join(td, "base")
        sv = os.path.join(save_folder(base), "Diablo II")
        os.makedirs(sv)
        os.makedirs(os.path.join(base, "drive_c", "windows"))
        for n, body in (("system.reg", "r"), (".run.lock", ""), ("drive_c/windows/x.dll", "d")):
            with open(os.path.join(base, n), "w") as f:
                f.write(body)
        with open(os.path.join(sv, "A.d2s"), "w") as f:
            f.write("s")
        dst = os.path.join(td, "p1")
        assert make_prefix(base, dst) and not make_prefix(base, dst)
        ino = lambda *p: os.stat(os.path.join(*p)).st_ino  # noqa: E731
        assert ino(dst, "drive_c/windows/x.dll") == ino(base, "drive_c/windows/x.dll")
        assert ino(dst, "system.reg") != ino(base, "system.reg")
        assert ino(save_folder(dst), "Diablo II", "A.d2s") != ino(sv, "A.d2s")
        assert not os.path.exists(os.path.join(dst, ".run.lock"))
        try:
            make_prefix(os.path.join(td, "none"), os.path.join(td, "p2"))
            raise AssertionError("made a prefix from nothing")
        except SuiteError:
            ok += 2
        # build: rng build first, the plain one last, binaries linked
        calls = []
        tgt = os.path.join(td, "target", "release")
        os.makedirs(tgt)
        for n in ("d2-client", "d2s-tool"):
            open(os.path.join(tgt, n), "w").close()

        class R:
            returncode = 0
        old = os.environ.get("CARGO_TARGET_DIR")
        os.environ["CARGO_TARGET_DIR"] = os.path.join(td, "target")
        try:
            build(os.path.join(td, "bin"), True, run=lambda argv, **kw: calls.append(argv) or R(),
                  log=lambda s: None)
        finally:
            if old is None:
                os.environ.pop("CARGO_TARGET_DIR")
            else:
                os.environ["CARGO_TARGET_DIR"] = old
        assert "rng-trace" in calls[0] and "rng-trace" not in calls[-1], calls
        assert sorted(os.listdir(os.path.join(td, "bin"))) == ["d2-client", "d2-client-rng",
                                                                "d2s-tool"]
        ok += 1
    # match %: per channel, area, overall; draws by rows; errors count 0 ticks
    recs = [
        {"name": "a1-x", "area": "a1", "error": None, "channels": {
            "state": {"code": 1, "verdict": "DIVERGED", "summary": {
                "frames_compared": 40, "frames_equal": 30,
                "first": {"frame": 31, "text": "frame 31 monster 1:3 class 150, field m"}}}}},
        {"name": "a1-y", "area": "a1", "error": "boom", "channels": {
            "rng": {"code": 3, "verdict": "ERROR", "summary": None}}},
        {"name": "draws-z", "area": "draws", "error": None, "channels": {
            "draws": {"code": 1, "verdict": "DIVERGED", "summary": {
                "frames_compared": 1, "frames_equal": 0, "rows_compared": 200,
                "rows_equal": 150, "first": {"text": "tick 73 draw row 1"}}}}},
    ]
    assert channel_numbers(recs[0]["channels"]["state"])[2] == 75.0
    assert channel_numbers(recs[2]["channels"]["draws"])[2] == 75.0
    areas, total = aggregate(recs)
    assert (areas["a1"]["checks"], areas["a1"]["ticks"], areas["a1"]["equal"]) == (2, 40, 30)
    assert areas["a1"]["pct"] == 75.0 and areas["a1"]["verdicts"]["ERROR"] == 1
    assert (total["ticks"], total["equal"], total["runs"]) == (41, 30, 3), total
    ok += 3
    # playthrough rows: the --all contract's keys, else counted from milestones
    full = {"act": 2, "reached": 3, "total": 5, "consecutive": 2,
            "first_blocker": {"milestone": "m3", "kind": "stuck", "frame": 9, "evidence": "e"}}
    assert act_row(full) == {k: full[k] for k in ("act", "reached", "total", "consecutive",
                                                   "first_blocker")}
    old = {"act": 1, "milestones": [{"name": "a", "status": "reached"},
                                    {"name": "b", "status": "stuck", "frame": 7, "evidence": "x"},
                                    {"name": "c", "status": "reached"}]}
    row = act_row(old)
    assert (row["reached"], row["total"], row["consecutive"]) == (2, 3, 1), row
    assert row["first_blocker"]["milestone"] == "b" and blocker_text(row["first_blocker"]) == \
        "b (stuck) frame 7: x"
    assert act_row({"act": 3, "milestones": [{"name": "a", "status": "reached"}]})[
        "first_blocker"] is None
    ok += 3
    # the reports carry every line
    res = {"workers": 3, "checks": recs, "areas": areas, "overall": total,
           "timing": {"wall": 10, "orig": 5, "reused": 1},
           "playthrough": {"acts": [row], "error": None}}
    txt = text_report(res)
    assert "a1-x" in txt and "75.0%" in txt and "boom" in txt and "b (stuck) frame 7" in txt
    assert "overall" in txt and "DIVERGED 2 ERROR 1" in txt, txt
    md = md_report(res)
    assert "| a1-x | state | 40 | 30 | 75.0% | DIVERGED |" in md, md
    assert "| 1 | 2/3 | 1 | b (stuck) frame 7: x |" in md, md
    ok += 2
    print(f"suite selftest: {ok} checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
