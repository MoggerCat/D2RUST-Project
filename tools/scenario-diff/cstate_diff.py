"""The client-state comparison (specs/tools/state-snapshot.md §3 rule 5, §4):
the 1.14d client's own unit sets (record_state.py --client-out) against the
d2rs client model dump (d2-client state-dump --client-out).

    python3 cstate_diff.py ORIG.cstate.jsonl D2RS.cstate.jsonl [state_diff options]

Both files are state-1 with a `set` key per unit (S: units the server
announced, C: client-only units). The comparison re-keys each unit by
(set, ut, g) (a set C unit is ut + 8, so the two sets never mix), drops the
`set` key, and drops `s` of the local player's records on both sides (the
client seed follows the wall clock: it steps once per drawn frame; state-
snapshot.md §3 rule 5). Everything else goes to state_diff.py unchanged.
Exit codes as state_diff.py. Standard library only. Our own code.
"""

import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
STATE_DIFF = os.path.join(HERE, "..", "trace-recorder", "state_diff.py")


def rekey(src, dst):
    with open(src, encoding="utf-8") as f, open(dst, "w", encoding="utf-8") as o:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rec = json.loads(line)
            if rec.get("k") == "snap":
                for u in rec["units"]:
                    st = u.pop("set", "S")
                    if st == "C":
                        u["ut"] += 8
                    elif u["ut"] == 0:
                        u.pop("s", None)
            elif rec.get("k") == "header":
                rec["side"] = "orig" if "orig" in rec.get("side", "") else "d2rs"
            o.write(json.dumps(rec, separators=(",", ":")) + "\n")


def main(argv):
    if len(argv) < 3:
        print(__doc__)
        return 3
    with tempfile.TemporaryDirectory() as td:
        a, b = os.path.join(td, "a.jsonl"), os.path.join(td, "b.jsonl")
        rekey(argv[1], a)
        rekey(argv[2], b)
        return subprocess.call([sys.executable, STATE_DIFF, a, b] + argv[3:])


if __name__ == "__main__":
    sys.exit(main(sys.argv))
