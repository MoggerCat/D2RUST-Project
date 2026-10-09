"""The `items` channel of scenario_diff.py (specs/tools/scenario-diff.md §3
rule 13): the packets channel's two recordings (record_packets.py,
`d2-client state-dump --packets`), recorded once per run when both
channels are asked, compared by items_diff.py item by item.

Our own code. Standard library only.
"""

import os
import sys

import packets_channel


def run(r, save, sides, shared_script_error):
    """One items run on the sides asked; items_diff's exit code when both
    ran (None when one side only). `r` is scenario_diff's Runner."""
    packets_channel.record(r, save, sides, shared_script_error)
    if sides != {"orig", "d2rs"}:
        return None
    here = os.path.dirname(os.path.abspath(__file__))
    code = r.sh([sys.executable, os.path.join(here, "items_diff.py"),
                 r.path("orig.packets.jsonl"), r.path("d2rs.packets.jsonl"), "--list",
                 "--next", str(r.next)] + r.json_args("items"), check=False)
    if r.d2rs_input() and shared_script_error(r.d2rs_input()):
        print("[items] d2rs ran without 'input d2rs' (not in the shared frame form, "
              "which state-dump needs): partial at best")
        code = max(code, 2) if code != 1 else 1
    return code


def selftest(runner_cls, check, shared_script_error):
    """Dry run: one recording pair serves both channels; the comparator."""
    r = runner_cls(dict(check, poke=[]), "/tmp/w", dry=True)
    r.next = 5
    packets_channel.run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"}, shared_script_error)
    run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"}, shared_script_error)
    assert sum("record_packets.py" in x for x in r.log) == 1, r.log
    assert sum("state-dump" in x for x in r.log) == 1, r.log
    cmp_ = r.log[-1]
    assert "items_diff.py /tmp/w/orig.packets.jsonl /tmp/w/d2rs.packets.jsonl" in cmp_, cmp_
    assert "--json /tmp/w/items.summary.json" in cmp_, cmp_
    r = runner_cls(dict(check, poke=[]), "/tmp/w", dry=True)   # items alone records too
    r.next = 5
    run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"}, shared_script_error)
    assert any("record_packets.py" in x for x in r.log) and \
        any("--packets /tmp/w/d2rs.packets.jsonl" in x for x in r.log), r.log
    return 2
