#!/usr/bin/env python3
"""Real-data regression check against a committed baseline.

Runs the game-file (#[ignore]) test set

    cargo nextest run -p d2-client -p d2-server -p d2-sim -p test-fixtures \
        --release --run-ignored only

with D2_GAME_DIR set, and compares each test's result with
tools/coord/realdata-baseline.tsv (test -> pass|fail). Exit 1 only on a NEW
failure: a test that passes in the baseline and fails (or no longer builds)
now. New passes, new tests and tests gone from the run are printed, never
fatal.

    python3 tools/coord/realdata.py                    # check
    python3 tools/coord/realdata.py --update-baseline  # rewrite the baseline from this run
    python3 tools/coord/realdata.py -E 'test(/skill/)' # nextest filter; compare only what ran
    python3 tools/coord/realdata.py --from-junit FILE  # compare an existing JUnit file, run nothing
    python3 tools/coord/realdata.py --selftest

Speed: a release build kept between runs in target/coord-realdata
(CARGO_PROFILE_RELEASE_DEBUG=0, CARGO_INCREMENTAL=0; d2-sim at opt-level 2,
since rustc 1.99 crashes on its test crate at opt-level 3), nextest runs every
test binary in parallel, a hung test is killed after 10 minutes (counts as a
failure).
GPU, memory-dump, known-bug-repro, recording-replay and D2_SAVE tests are
skipped as in tools/realdata-gate.sh (tools/realdata_inventory.py
--skip-names); --no-skip runs them too.

Exit 0: no new failure; 1: a new failure (or the build broke); 2: usage /
setup error (no D2_GAME_DIR, no nextest, no baseline). Python stdlib only.
"""

import argparse
import os
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HERE = os.path.join(REPO, "tools", "coord")
BASELINE = os.path.join(HERE, "realdata-baseline.tsv")
PACKAGES = ["d2-client", "d2-server", "d2-sim", "test-fixtures"]
FORMAT = "realdata-baseline 1"
NEXTEST_CONFIG = """\
[profile.coord]
fail-fast = false
slow-timeout = { period = "60s", terminate-after = 10 }

[profile.coord.junit]
path = "junit.xml"
store-success-output = false
store-failure-output = false
"""


class Setup(Exception):
    pass


def load_baseline(path):
    out = {}
    with open(path, encoding="utf-8") as f:
        lines = f.read().splitlines()
    if not lines or lines[0] != "# " + FORMAT:
        raise Setup(f"{path}: first line must be '# {FORMAT}'")
    for no, line in enumerate(lines[1:], 2):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) != 2 or parts[1] not in ("pass", "fail"):
            raise Setup(f"{path}:{no}: expected 'test<TAB>pass|fail'")
        out[parts[0]] = parts[1]
    return out


def write_baseline(path, results, head, cmd):
    with open(path, "w", encoding="utf-8") as f:
        f.write(f"# {FORMAT}\n")
        f.write(f"# head {head}; made by: {cmd}\n")
        f.write(f"# {sum(v == 'pass' for v in results.values())} pass, "
                f"{sum(v == 'fail' for v in results.values())} fail\n")
        for name in sorted(results):
            f.write(f"{name}\t{results[name]}\n")


def parse_junit(path):
    """JUnit from nextest -> {"<binary-id> <test name>": "pass"|"fail"}.
    Skipped testcases are left out (they did not run)."""
    out = {}
    for tc in ET.parse(path).getroot().iter("testcase"):
        name = f"{tc.get('classname')} {tc.get('name')}"
        tags = {c.tag for c in tc}
        if "skipped" in tags:
            continue
        out[name] = "fail" if tags & {"failure", "error", "rerunFailure", "flakyFailure"} else "pass"
    return out


def compare(base, now, filtered):
    """Returns (new_fail, new_pass, new_tests, gone). `gone` (in the
    baseline, not run) is only reported for unfiltered runs."""
    new_fail = sorted(t for t, r in now.items() if r == "fail" and base.get(t) == "pass")
    new_pass = sorted(t for t, r in now.items() if r == "pass" and base.get(t) == "fail")
    new_tests = sorted((t, r) for t, r in now.items() if t not in base)
    gone = [] if filtered else sorted(t for t in base if t not in now)
    return new_fail, new_pass, new_tests, gone


def git_head():
    r = subprocess.run(["git", "rev-parse", "--short=8", "HEAD"], cwd=REPO, capture_output=True, text=True)
    return r.stdout.strip() or "?"


def skip_filter():
    """Same skip set as tools/realdata-gate.sh: gpu / dump / repro tests,
    recording replays without traces/raw/*.jsonl, D2_SAVE tests without D2_SAVE."""
    flags = ["--skip-names"]
    d = os.path.join(REPO, "traces", "raw")
    if not (os.path.isdir(d) and any(f.endswith(".jsonl") for f in os.listdir(d))):
        flags.append("--no-recordings")
    if not os.environ.get("D2_SAVE"):
        flags.append("--no-save")
    r = subprocess.run([sys.executable, os.path.join(REPO, "tools", "realdata_inventory.py")] + flags,
                       cwd=REPO, capture_output=True, text=True)
    if r.returncode != 0:
        raise Setup("tools/realdata_inventory.py --skip-names failed: " + r.stderr.strip())
    return " | ".join(f"test(/(^|::){n}$/)" for n in r.stdout.split())


def run(args):
    if not os.environ.get("D2_GAME_DIR"):
        raise Setup("D2_GAME_DIR is not set (see tools/realdata-gate.sh for assembling the install)")
    if subprocess.run(["cargo", "nextest", "--version"], capture_output=True).returncode != 0:
        raise Setup("cargo-nextest missing: cargo install cargo-nextest --locked")
    work = tempfile.mkdtemp(prefix="realdata-")
    cfg = os.path.join(work, "nextest.toml")
    with open(cfg, "w") as f:
        f.write(NEXTEST_CONFIG)
    pk = []
    for p in args.package or PACKAGES:
        pk += ["-p", p]
    exprs = []
    if not args.no_skip:
        sk = skip_filter()
        if sk:
            exprs.append(f"not ({sk})")
    if args.filter:
        exprs.append(f"({args.filter})")
    env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="0", CARGO_INCREMENTAL="0",
               CARGO_TARGET_DIR=args.target_dir)
    # rustc 1.99 at opt-level 3 never finishes the d2-sim test crate (LLVM's
    # full-unroll pass recurses until the stack is gone, at any RUST_MIN_STACK):
    # d2-sim builds at opt-level 2 here, in its own target dir so that
    # playthrough.py --build (opt-level 3) and this run never rebuild each other.
    common = ["--release", "--config", "profile.release.package.d2-sim.opt-level=2"] + pk
    t0 = time.monotonic()
    b = subprocess.run(["cargo", "nextest", "run", "--no-run"] + common + ["--config-file", cfg, "--profile", "coord"],
                       cwd=REPO, env=env)
    t1 = time.monotonic()
    if b.returncode != 0:
        print(f"realdata: BUILD FAILED (build {t1 - t0:.0f}s): every baseline pass counts as a new failure")
        return None, (t1 - t0, 0.0), "build"
    junit = os.path.join(args.target_dir, "nextest", "coord", "junit.xml")
    if os.path.exists(junit):
        os.remove(junit)
    cmd = (["cargo", "nextest", "run"] + common + ["--run-ignored", "only", "--no-fail-fast", "--no-tests=pass",
           "--config-file", cfg, "--profile", "coord", "--color", "never", "--status-level", "fail",
           "--final-status-level", "fail", "--hide-progress-bar"]
           + (["-E", " and ".join(exprs)] if exprs else []))
    if args.verbose:
        print("+", " ".join(cmd))
    r = subprocess.run(cmd, cwd=REPO, env=env)
    t2 = time.monotonic()
    if not os.path.exists(junit):
        print(f"realdata: nextest wrote no JUnit (exit {r.returncode})")
        return None, (t1 - t0, t2 - t1), "run"
    return parse_junit(junit), (t1 - t0, t2 - t1), None


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--update-baseline", action="store_true", help="write this run's results as the baseline")
    ap.add_argument("-E", "--filter", help="nextest filter expression; only tests that ran are compared")
    ap.add_argument("-p", "--package", action="append", help=f"override the packages (default {' '.join(PACKAGES)})")
    ap.add_argument("--no-skip", action="store_true", help="also run the gpu/dump/repro/recording/save tests")
    ap.add_argument("--baseline", default=BASELINE)
    ap.add_argument("--target-dir", default=os.path.join(REPO, "target", "coord-realdata"),
                    help="cargo target dir, kept between runs (default target/coord-realdata)")
    ap.add_argument("--from-junit", help="compare this nextest JUnit file instead of running")
    ap.add_argument("-v", "--verbose", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    t_start = time.monotonic()
    try:
        filtered = bool(a.filter or a.package)
        if a.from_junit:
            now, times, broke = parse_junit(a.from_junit), (0.0, 0.0), None
        else:
            now, times, broke = run(a)
        if a.update_baseline:
            if broke:
                raise Setup("not updating the baseline from a broken build/run")
            if filtered:
                raise Setup("--update-baseline needs the full set (no -E / -p)")
            write_baseline(a.baseline, now, git_head(),
                           "python3 tools/coord/realdata.py --update-baseline")
            print(f"realdata: baseline written: {a.baseline} ({len(now)} tests, "
                  f"{sum(v == 'fail' for v in now.values())} fail)")
            print(f"wall time: {time.monotonic() - t_start:.0f}s (build {times[0]:.0f}s, tests {times[1]:.0f}s)")
            return 0
        if not os.path.exists(a.baseline):
            raise Setup(f"no baseline at {a.baseline}: run with --update-baseline on the staging head")
        base = load_baseline(a.baseline)
    except Setup as e:
        print(f"realdata: error: {e}", file=sys.stderr)
        return 2
    if broke:
        new_fail = sorted(t for t, r in base.items() if r == "pass")
        print(f"NEW FAILURES: {broke} broke; {len(new_fail)} baseline passes did not run")
        print(f"wall time: {time.monotonic() - t_start:.0f}s")
        return 1
    new_fail, new_pass, new_tests, gone = compare(base, now, filtered)
    npass = sum(v == "pass" for v in now.values())
    print(f"realdata: {len(now)} tests ran: {npass} pass, {len(now) - npass} fail; baseline "
          f"{sum(v == 'pass' for v in base.values())} pass / {len(base)}")
    for title, items in (("NEW FAILURES (pass in the baseline)", new_fail),
                         ("new passes (fail in the baseline; --update-baseline to record)", new_pass)):
        if items:
            print(f"{title}: {len(items)}")
            for t in items:
                print(f"  {t}")
    if new_tests:
        print(f"tests not in the baseline: {len(new_tests)}")
        for t, r in new_tests:
            print(f"  {r}  {t}")
    if gone:
        print(f"baseline tests that did not run (renamed / removed?): {len(gone)}")
        for t in gone:
            print(f"  {t}")
    print(f"wall time: {time.monotonic() - t_start:.0f}s (build {times[0]:.0f}s, tests {times[1]:.0f}s)")
    if new_fail:
        print(f"realdata: FAIL: {len(new_fail)} new failure(s)")
        return 1
    print("realdata: OK: no new failure")
    return 0


def selftest():
    junit = """<?xml version="1.0"?><testsuites><testsuite name="x">
<testcase name="a::passes" classname="d2-sim"/>
<testcase name="a::broke" classname="d2-sim"><failure type="test failure"/></testcase>
<testcase name="a::fixed" classname="d2-sim::bin/x"/>
<testcase name="a::still" classname="d2-sim"><failure/></testcase>
<testcase name="a::new" classname="d2-sim"><failure/></testcase>
<testcase name="a::skip" classname="d2-sim"><skipped/></testcase>
</testsuite></testsuites>"""
    d = tempfile.mkdtemp(prefix="realdata-selftest-")
    jp, bp = os.path.join(d, "j.xml"), os.path.join(d, "b.tsv")
    with open(jp, "w") as f:
        f.write(junit)
    now = parse_junit(jp)
    assert now == {"d2-sim a::passes": "pass", "d2-sim a::broke": "fail", "d2-sim::bin/x a::fixed": "pass",
                   "d2-sim a::still": "fail", "d2-sim a::new": "fail"}, now
    write_baseline(bp, {"d2-sim a::passes": "pass", "d2-sim a::broke": "pass", "d2-sim::bin/x a::fixed": "fail",
                        "d2-sim a::still": "fail", "d2-sim a::gone": "pass"}, "abc", "selftest")
    base = load_baseline(bp)
    nf, np, nt, gone = compare(base, now, False)
    assert nf == ["d2-sim a::broke"] and np == ["d2-sim::bin/x a::fixed"], (nf, np)
    assert nt == [("d2-sim a::new", "fail")] and gone == ["d2-sim a::gone"], (nt, gone)
    assert compare(base, now, True)[3] == []
    # exit codes through main: a new failure is 1, the same results are 0
    with open(os.devnull, "w") as dn:
        old, sys.stdout = sys.stdout, dn
        try:
            rc1 = main(["--from-junit", jp, "--baseline", bp])
            write_baseline(bp, now, "abc", "selftest")
            rc0 = main(["--from-junit", jp, "--baseline", bp])
        finally:
            sys.stdout = old
    assert (rc1, rc0) == (1, 0), (rc1, rc0)
    print("realdata selftest: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
