"""The `packets` channel of scenario_diff.py (specs/tools/scenario-diff.md
§3, specs/tools/packets-trace.md): 1.14d through record_packets.py, d2rs
through `d2-client state-dump --packets`, compared by packets_diff.py.

Our own code. Standard library only.
"""

import os
import sys


def run(r, save, sides, shared_script_error):
    """One packets run on the sides asked; the comparator's exit code when
    both ran (None when one side only). `r` is scenario_diff's Runner."""
    c = r.c
    rec = _rec()
    orig, d2rs = r.path("orig.packets.jsonl"), r.path("d2rs.packets.jsonl")
    # pokes and sends reach both sides: record_packets.py has the poke and the
    # send layers, state-dump takes both (scenario-diff.md §3 rules 9-10)
    if "orig" in sides:
        r.recorder("record_packets.py", [], orig)
    if "d2rs" in sides and not (r.reuse and os.path.exists(d2rs)):
        args = ["state-dump"] + r.d2rs_common(save) + [
            "--ticks", str(c["ticks"]), "--out", r.path("d2rs.packets-state.jsonl"),
            "--packets", d2rs]
        if r.d2rs_input() and not shared_script_error(r.d2rs_input()):
            args += ["--input", r.d2rs_input()]
        r.cargo("d2-client", args)
    if sides != {"orig", "d2rs"}:
        return None
    code = r.sh([sys.executable, os.path.join(rec, "packets_diff.py"), orig, d2rs,
                 "--next", str(r.next)] + r.json_args("packets"), check=False)
    if r.d2rs_input() and shared_script_error(r.d2rs_input()):
        print("[packets] d2rs ran without 'input d2rs' (not in the shared frame form, "
              "which state-dump needs): partial at best")
        code = max(code, 2) if code != 1 else 1
    return code


def _rec():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.normpath(os.path.join(here, "..", "trace-recorder"))


def selftest(runner_cls, check, shared_script_error):
    """Dry run: the recorder, the dump with --packets and the comparator,
    with the check's save and seed. Returns the number of checks passed."""
    poked = dict(check, poke=[(1, 5, "time 1 0")], send=[(2, 7, "Walk x=10 y=20")])
    r = runner_cls(poked, "/tmp/w", dry=True)
    r.next = 5
    run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"}, shared_script_error)
    for what in ("record_packets.py", "state-dump"):  # pokes and sends on both sides
        line = next(x for x in r.log if what in x)
        assert "--poke '5 time 1 0'" in line and "--send '7 Walk x=10 y=20'" in line, line
    r = runner_cls(dict(check, poke=[]), "/tmp/w", dry=True)
    r.next = 5
    code = run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"}, shared_script_error)
    rec = next(x for x in r.log if "record_packets.py" in x)
    assert "--ticks 20" in rec and "--auto ScnAma --seed 1234" in rec, rec
    assert "--out /tmp/w/orig.packets.jsonl" in rec, rec
    dump = next(x for x in r.log if "state-dump" in x)
    assert "--packets /tmp/w/d2rs.packets.jsonl" in dump and "--seed 1234" in dump, dump
    assert "--ticks 20" in dump, dump
    cmp_ = r.log[-1]
    assert "packets_diff.py /tmp/w/orig.packets.jsonl /tmp/w/d2rs.packets.jsonl" in cmp_, cmp_
    assert "--json /tmp/w/packets.summary.json" in cmp_, cmp_
    assert code == 0  # a dry run's commands "succeed"
    r.log = []
    run(r, "/tmp/w/ScnAma.d2s", {"d2rs"}, shared_script_error)   # one side: no compare
    assert not any("packets_diff" in x or "record_packets" in x for x in r.log)
    return 4
