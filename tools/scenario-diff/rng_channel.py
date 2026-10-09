"""The `rng` channel of scenario_diff.py (specs/tools/scenario-diff.md §3,
specs/tools/rng-trace.md): 1.14d through `record_rng.py --frames` (every
seeded draw, tick markers, owners), d2rs through `d2-client state-dump
--rng` (built with the `rng-trace` feature), compared by rng_diff.py.

Our own code. Standard library only.
"""

import os
import sys


def run(r, save, sides):
    """One rng run on the sides asked; the comparator's exit code when both
    ran (None when one side only). `r` is scenario_diff's Runner."""
    c = r.c
    rec = _rec()
    orig, d2rs = r.path("orig.rng.jsonl"), r.path("d2rs.rng.jsonl")
    if "orig" in sides:
        # the DRLG inline sites are left unhooked: their draws are always
        # other:drlg, never compared (rng-trace.md §6 r4)
        r.recorder("record_rng.py", ["--frames", "--skip-inline", "drlg"], orig)
    if "d2rs" in sides and not (r.reuse and os.path.exists(d2rs)):
        args = ["state-dump"] + r.d2rs_common(save) + [
            "--ticks", str(c["ticks"]), "--out", r.path("d2rs.rng-state.jsonl"), "--rng", d2rs]
        if c["input"].get("shared"):  # pokes and sends come with d2rs_common
            args += ["--input", c["input"]["shared"]]
        r.cargo("d2-client", args, features="rng-trace")
    if sides != {"orig", "d2rs"}:
        return None
    code = r.sh([sys.executable, os.path.join(rec, "rng_diff.py"), orig, d2rs,
                 "--next", str(r.next)] + r.json_args("rng"), check=False)
    if c["input"].get("d2rs") or c["input"].get("orig"):
        # state-dump runs no 'input d2rs' / wall-clock 'input orig' script
        print("[rng] the check's input ran on 1.14d only: partial at best")
        code = max(code, 2) if code != 1 else 1
    return code


def _rec():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.normpath(os.path.join(here, "..", "trace-recorder"))


def selftest(runner_cls, check):
    """Dry run: the recorder with --frames, the dump with --rng and the
    rng-trace feature, the comparator. Returns the number of checks passed."""
    # pokes, sends and the shared input reach both sides (rng-trace.md §6 r3)
    r = runner_cls(dict(check, poke=[(1, 5, "time 1 0")], send=[(2, 6, "hex 13 01 00 00 00")],
                        input={"shared": "frame 10; click 600 300"}), "/tmp/w", dry=True)
    r.next = 5
    run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    for what in ("record_rng.py", "state-dump"):
        line = next(x for x in r.log if what in x)
        assert "--poke '5 time 1 0'" in line and "--send '6 hex 13 01 00 00 00'" in line, line
        assert "--input 'frame 10; click 600 300'" in line, line
    r = runner_cls(dict(check, poke=[], input={}), "/tmp/w", dry=True)
    r.next = 5
    code = run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    rec = next(x for x in r.log if "record_rng.py" in x)
    assert "--ticks 20" in rec and "--auto ScnAma --seed 1234" in rec, rec
    assert "--out /tmp/w/orig.rng.jsonl" in rec and "--frames" in rec, rec
    assert "--skip-inline drlg" in rec, rec
    dump = next(x for x in r.log if "state-dump" in x)
    assert "--features rng-trace" in dump and "--rng /tmp/w/d2rs.rng.jsonl" in dump, dump
    assert "--seed 1234" in dump and "--ticks 20" in dump, dump
    assert "rng_diff.py /tmp/w/orig.rng.jsonl /tmp/w/d2rs.rng.jsonl" in r.log[-1], r.log[-1]
    assert "--json /tmp/w/rng.summary.json" in r.log[-1], r.log[-1]
    assert code == 0
    r.log = []
    run(r, "/tmp/w/ScnAma.d2s", {"d2rs"})  # one side: no compare
    assert not any("rng_diff" in x or "record_rng" in x for x in r.log)
    return 4
