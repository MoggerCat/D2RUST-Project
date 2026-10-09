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
    if c["poke"]:
        # record_rng.py has no poke layer
        print("[rng] not compared: record_rng.py takes no --poke (rng-trace.md §6 r3)")
        return 2
    if c["input"].get("shared"):
        # frame-anchored input needs a recorder that stops at the tick return
        print("[rng] not compared: record_rng.py does not run the shared frame-anchored "
              "input (rng-trace.md §6 r3)")
        return 2
    if "orig" in sides:
        r.recorder("record_rng.py", ["--frames"], orig)
    if "d2rs" in sides and not (r.reuse and os.path.exists(d2rs)):
        args = ["state-dump"] + r.d2rs_common(save) + [
            "--ticks", str(c["ticks"]), "--out", r.path("d2rs.rng-state.jsonl"), "--rng", d2rs]
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
    r = runner_cls(dict(check, poke=[(1, 5, "time 1 0")]), "/tmp/w", dry=True)
    r.next = 5
    assert run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"}) == 2
    assert not r.log  # nothing run: record_rng.py has no poke layer
    r = runner_cls(dict(check, poke=[], input={}), "/tmp/w", dry=True)
    r.next = 5
    code = run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    rec = next(x for x in r.log if "record_rng.py" in x)
    assert "--ticks 20" in rec and "--auto ScnAma --seed 1234" in rec, rec
    assert "--out /tmp/w/orig.rng.jsonl" in rec and "--frames" in rec, rec
    dump = next(x for x in r.log if "state-dump" in x)
    assert "--features rng-trace" in dump and "--rng /tmp/w/d2rs.rng.jsonl" in dump, dump
    assert "--seed 1234" in dump and "--ticks 20" in dump, dump
    assert "rng_diff.py /tmp/w/orig.rng.jsonl /tmp/w/d2rs.rng.jsonl" in r.log[-1], r.log[-1]
    assert "--json /tmp/w/rng.summary.json" in r.log[-1], r.log[-1]
    assert code == 0
    r.log = []
    run(r, "/tmp/w/ScnAma.d2s", {"d2rs"})  # one side: no compare
    assert not any("rng_diff" in x or "record_rng" in x for x in r.log)
    return 3
