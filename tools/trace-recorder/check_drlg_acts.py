"""Check `autostart.py` `dumpdrlg` records against `specs/drlg/levels.md`
§3–§4 (act DRLG creation and level allocation). Our own code.

A `dumpdrlg` record is the client act's DRLG and level list read in the
running 1.14d game (`autostart.py drlg_dump`). It lands in the footer
notes of any recorder run with `--auto ... --input "...; dumpdrlg LABEL"`,
or in the printed log of `autostart.py --try`. For each record:

  D1  init seed copy (drlg +0x458) = act init seed (act +0x0C)
  D2  dwStartSeed (drlg +0x470) = lo' of one step from {init seed, 666}
  D3  Act II (act no 1): staff / boss tombs (+0x94 / +0x484) = 66 + a,
      66 + b from the §3 step-4 loop (two draws per try until a != b)
  D4  Act III (act no 2): jungle-link bit (+0x474) = lo' & 1 of the draw
      after the start-seed step
  D5  acts 0, 3, 4: tombs and jungle bit 0
  D6  every level of type maze (1) or outdoor (3) with no room built has
      seed {dwStartSeed + id (u32), 666} (§4.3: init_low, no draw)
  D7  level flags carry 0x10 (client copy) and the drlg flags bit 0

  py tools/trace-recorder/check_drlg_acts.py FILE [FILE ...]   # .jsonl raw or a log
  py tools/trace-recorder/check_drlg_acts.py FILE --perturb N   # must fail at record N
  py tools/trace-recorder/check_drlg_acts.py --selftest
"""

import argparse
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import d2rng  # noqa: E402

M32 = 0xFFFFFFFF
PREFIX = "autostart: dumpdrlg "


def records_from(path):
    out = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            notes = []
            if line.startswith("{"):
                try:
                    obj = json.loads(line)
                except ValueError:
                    continue
                if obj.get("type") == "footer" or obj.get("k") == "footer":
                    notes = obj.get("notes", [])
            else:
                notes = [line[line.find(PREFIX):]] if PREFIX in line else []
            for n in notes:
                if isinstance(n, str) and n.startswith(PREFIX):
                    out.append(json.loads(n[len(PREFIX):]))
    seen, uniq = set(), []          # a --try log prints each record twice (log, notes)
    for r in out:
        k = json.dumps(r, sort_keys=True)
        if k not in seen:
            seen.add(k)
            uniq.append(r)
    return uniq


def expected(init_seed, act_no):
    lo, hi = init_seed & M32, 666
    lo, hi = d2rng.step(lo, hi)
    e = {"start_seed": lo, "staff_tomb": 0, "boss_tomb": 0, "jungle_bit": 0}
    if act_no == 1:
        while True:
            lo, hi = d2rng.step(lo, hi)
            a = lo % 7
            lo, hi = d2rng.step(lo, hi)
            b = lo % 7
            if a != b:
                break
        e["staff_tomb"], e["boss_tomb"] = 66 + a, 66 + b
    elif act_no == 2:
        lo, hi = d2rng.step(lo, hi)
        e["jungle_bit"] = lo & 1
    return e


def check(rec):
    """List of (rule, message) failures for one record."""
    bad = []
    if "drlg_seed" not in rec:
        return [("D0", "no DRLG in the record")]
    init, act_no = rec["init_seed"], rec["act_no"]
    if rec["init_seed_copy"] != init:
        bad.append(("D1", f"init seed copy {rec['init_seed_copy']} != {init}"))
    e = expected(init, act_no)
    if rec["start_seed"] != e["start_seed"]:
        bad.append(("D2", f"start seed {rec['start_seed']}, rule {e['start_seed']}"))
    for k, rule in (("staff_tomb", "D3"), ("boss_tomb", "D3"), ("jungle_bit", "D4")):
        if rec[k] != e[k]:
            r = rule if (act_no == 1 and rule == "D3") or (act_no == 2 and rule == "D4") else "D5"
            bad.append((r, f"{k} {rec[k]}, rule {e[k]}"))
    if not rec["flags"] & 1:
        bad.append(("D7", f"drlg flags {rec['flags']:#x} without the client bit"))
    for lv in rec.get("levels", []):
        if not lv["flags"] & 0x10:
            bad.append(("D7", f"level {lv['id']} flags {lv['flags']:#x} without 0x10"))
        if lv["drlg_type"] in (1, 3) and lv["rooms"] == 0:
            want = [(rec["start_seed"] + lv["id"]) & M32, 666]
            if lv["seed"] != want:
                bad.append(("D6", f"level {lv['id']} seed {lv['seed']}, rule {want}"))
    return bad


def run(recs, verbose=True):
    errors = 0
    counts = {}
    for i, r in enumerate(recs):
        bad = check(r)
        n6 = sum(1 for lv in r.get("levels", []) if lv["drlg_type"] in (1, 3) and lv["rooms"] == 0)
        counts["D6 levels"] = counts.get("D6 levels", 0) + n6
        counts[f"act {r.get('act_no')}"] = counts.get(f"act {r.get('act_no')}", 0) + 1
        for rule, msg in bad:
            errors += 1
            if verbose:
                print(f"record {i} ({r.get('label')}): {rule} {msg}")
        if verbose and not bad:
            e = expected(r["init_seed"], r["act_no"])
            print(f"record {i} ({r.get('label')}): act {r['act_no']} init {r['init_seed']} "
                  f"start {e['start_seed']} tombs {e['staff_tomb']}/{e['boss_tomb']} "
                  f"jungle {e['jungle_bit']}, {n6} unbuilt level seeds: ok")
    return errors, counts


def perturb(recs, n):
    r = json.loads(json.dumps(recs))
    rec = r[n]
    if rec["act_no"] == 1:
        rec["staff_tomb"] ^= 1
    elif rec["act_no"] == 2:
        rec["jungle_bit"] ^= 1
    else:
        rec["start_seed"] = (rec["start_seed"] + 1) & M32
    return r


def selftest():
    recs = []
    for act in range(5):
        init = 644409375
        e = expected(init, act)
        recs.append({"label": f"a{act}", "act_no": act, "init_seed": init, "init_seed_copy": init,
                     "drlg_seed": [0, 0], "flags": 1, **e,
                     "levels": [{"id": 3 + act, "drlg_type": 3, "rooms": 0, "flags": 16,
                                 "seed": [(e["start_seed"] + 3 + act) & M32, 666]}]})
    # levels.md Test vectors: init seed 644409375 -> dwStartSeed 4014346869
    assert recs[0]["start_seed"] == 4014346869, recs[0]["start_seed"]
    assert run(recs, False)[0] == 0
    for n in range(len(recs)):
        assert run(perturb(recs, n), False)[0] >= 1, n
    r = json.loads(json.dumps(recs))
    r[0]["levels"][0]["seed"][0] += 1
    assert run(r, False)[0] == 1
    r = json.loads(json.dumps(recs))
    r[3]["levels"][0]["flags"] = 0
    assert run(r, False)[0] == 1
    print("selftest ok: rules D1-D7 on built vectors (start seed 4014346869 = levels.md); "
          "every record perturbation and a level seed / flag change fail")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("files", nargs="*")
    ap.add_argument("--perturb", type=int, default=None)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return
    recs = [r for f in a.files for r in records_from(f)]
    if not recs:
        sys.exit("no dumpdrlg records")
    if a.perturb is not None:
        recs = perturb(recs, a.perturb)
    errors, counts = run(recs)
    print(f"{len(recs)} records, errors {errors}, " + ", ".join(f"{k} {v}" for k, v in counts.items()))
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
