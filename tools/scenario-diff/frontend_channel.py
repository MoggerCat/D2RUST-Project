#!/usr/bin/env python3
"""The `frontend` channel: character-select paper dolls (REC-2295).

Our own code. Unlike the in-game channels this one drives the front end of
both programs by X input (tools/frontend-sbs/frontend_sbs.py, script `dolls`)
and compares the figure pixels (tools/frontend-sbs/dolls_check.py). The save
line of the check is only the character the in-game channels would use; the
charselect shows every save in the folder. Spec: specs/tools/scenario-diff.md
§3 rule 13, specs/ui/frontend-menus.md §F2.10.
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SBS = os.path.join(HERE, "..", "frontend-sbs")
sys.path.insert(0, SBS)


def run(r, save, sides):
    out = r.path("frontend")
    side = "both" if {"orig", "d2rs"} <= set(sides) else ("orig" if "orig" in sides else "ours")
    argv = [sys.executable, os.path.join(SBS, "frontend_sbs.py"), out, "--script", "dolls",
            "--side", side]
    if r.reuse_orig and side == "both" and os.path.isdir(os.path.join(out, "orig")):
        argv[-1] = "ours"
    rc = r.sh(argv, check=False)
    if r.dry:
        r.log.append("dolls_check.py " + out)
        return 0
    if "orig" not in sides or "d2rs" not in sides:
        return None
    if rc != 0:
        raise RuntimeError(f"frontend_sbs.py exit {rc}")
    import dolls_check
    res = dolls_check.run(out)
    bad = {k: v for k, v in res.items() if v["verdict"] != "EQUAL"}
    code = 1 if bad else 0
    for k, v in res.items():
        print(f"[frontend] {k}: {v['verdict']} (worst differing fraction {v['worst_frac']}, "
              f"dy probe {v['dy_probe']})")
    first = None
    if bad:
        k = sorted(bad)[0]
        first = {"frame": None, "text": f"{k}: {bad[k]['worst_frac']:.4f} of the doll pixels differ"}
    summ = {"format": "diff-summary-1", "channel": "frontend", "tool": "dolls_check.py",
            "code": code, "verdict": "DIVERGED" if code else "MATCH",
            "rows_compared": len(res), "rows_equal": len(res) - len(bad),
            "frames_compared": len(res), "frames_equal": len(res) - len(bad),
            "differences": len(bad), "first": first, "slots": res}
    with open(r.path("frontend.summary.json"), "w", encoding="utf-8") as f:
        json.dump(summ, f, indent=1)
    print(f"[frontend] {'DIVERGED: ' + first['text'] if code else 'MATCH: every slot equal'}")
    return code
