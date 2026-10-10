"""The `cstate` channel of scenario_diff.py (specs/tools/scenario-diff.md §3,
specs/tools/state-snapshot.md §3 r5): the client's own unit sets. 1.14d side:
the recordings `traces/pc1/client-state/<check>.cstate.jsonl` made on Windows by
`record_state.py --client-out` (a check `cs-<name>` uses `<name>.cstate.jsonl`);
d2rs side: `d2-client state-dump --client-out` (the ClientWorld model); compared
by cstate_diff.py.

Our own code. Standard library only.
"""

import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RECORDED = os.path.normpath(os.path.join(HERE, "..", "..", "traces", "pc1", "client-state"))


def run(r, save, sides, shared_script_error):
    c = r.c
    base = c["name"][3:] if c["name"].startswith("cs-") else c["name"]
    orig, d2rs = r.path("orig.cstate.jsonl"), r.path("d2rs.cstate.jsonl")
    if "orig" in sides:
        src = os.path.join(RECORDED, base + ".cstate.jsonl")
        if not os.path.exists(src):
            raise_missing(src)
        shutil.copyfile(src, orig)
    if "d2rs" in sides and not (r.reuse and os.path.exists(d2rs)):
        args = ["state-dump"] + r.d2rs_common(save) + [
            "--ticks", str(c["ticks"]), "--out", r.path("d2rs.cstate-state.jsonl"),
            "--client-out", d2rs]
        if r.d2rs_input() and not shared_script_error(r.d2rs_input()):
            args += ["--input", r.d2rs_input()]
        r.cargo("d2-client", args)
    if sides != {"orig", "d2rs"}:
        return None
    code = r.sh([sys.executable, os.path.join(HERE, "cstate_diff.py"), orig, d2rs,
                 "--next", str(r.next)] + r.json_args("cstate"), check=False)
    if r.d2rs_input() and shared_script_error(r.d2rs_input()):
        print("[cstate] d2rs ran without 'input d2rs' (not in the shared frame form): partial at best")
        code = max(code, 2) if code != 1 else 1
    return code


def raise_missing(src):
    raise FileNotFoundError(f"no 1.14d client recording {src}: record it on Windows with "
                     "record_state.py --client-out (state-snapshot.md section 3 rule 5)")
