#!/usr/bin/env python3
"""packet-census: per message id (C->S and S->C), which checks' packets
recordings carry it and its first equal / diverged instance
(docs/handoff/fidelity-gaps.md §4; comparison = packets_diff.py's record
compare, per id).

    python3 tools/packet-census/packet_census.py run [--filter GLOB] [--reuse-orig]
    python3 tools/packet-census/packet_census.py report [--dir traces/raw/census]
        [--md docs/handoff/packet-census.md] [--tsv docs/handoff/packet-census.tsv]
        [--ledger FIDELITY_LEDGER.tsv --part docs/handoff/ledger/q-tool-packet-census.tsv]
    python3 packet_census.py --selftest

`run` records every traces/checks/*.check with the packets channel only
(scenario_diff.py --channels packets --work <dir>/<check>; 1.14d needs
Wine, see docs/LOCAL-RUN.md / tools/coord/session-setup.sh). `report`
reads <dir>/<check>/orig.packets.jsonl + d2rs.packets.jsonl (also
traces/raw/suite/<check>/ with `--dir`), pairs the records window by
window and stream by stream as packets_diff.diff does (first difference
per window and stream; later records of that window are carried but not
paired), and counts per (stream, id): checks carrying it (1.14d side),
records, equal and diverged pairs, first equal / first diverged instance.
Transport ids (packets_diff.transport_rows) are excluded as there.
State of a ledger row net.<c2s|s2c>.0xNN from the data: any diverged pair
-> DIVERGED; carried and all pairs equal -> EQUAL; carried only by checks
whose comparison did not run to a pair -> NO-CHECK; not carried -> keeps
NO-CHECK / NOT-IMPLEMENTED.

Standard library only. Our own code.
"""

import argparse
import csv
import fnmatch
import glob
import json
import os
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(REPO, "tools", "trace-recorder"))
import packets_diff as pd  # noqa: E402

FORMAT = "packet-census-1"
CHECKS = os.path.join(REPO, "traces", "checks")
DEFAULT_DIR = os.path.join(REPO, "traces", "raw", "census")
SUITE_DIR = os.path.join(REPO, "traces", "raw", "suite")
SCENARIO = os.path.join(REPO, "tools", "scenario-diff", "scenario_diff.py")
STREAMS = ("c2s", "s2c")
PART_COLS = ["area", "kind", "group", "source_1.14d", "specs", "spec_status", "checks",
             "last_verdict", "exercised", "provisional", "needs_pc1", "owner", "state",
             "size", "note"]


def check_names(pattern=None):
    names = sorted(os.path.basename(p)[:-6] for p in glob.glob(os.path.join(CHECKS, "*.check")))
    return [n for n in names if not pattern or fnmatch.fnmatch(n, pattern)]


# --- run ----------------------------------------------------------------------

def cmd_run(args, run=subprocess.run):
    """Record every check with the packets channel only. Returns the failures."""
    bad = []
    for n in check_names(args.filter):
        work = os.path.join(args.dir, n)
        cmd = [sys.executable, SCENARIO, os.path.join(CHECKS, n + ".check"),
               "--channels", "packets", "--work", work]
        if args.reuse_orig:
            cmd.append("--reuse-orig")
        print("census:", n, flush=True)
        r = run(cmd, check=False)
        if r.returncode == 3:   # packets_diff's error code; 1/2 are verdicts
            bad.append(n)
    return bad


# --- census -------------------------------------------------------------------

def new_row():
    return {"checks": set(), "records": 0, "equal": 0, "diverged": 0, "d2rs_only": 0,
            "first_equal": None, "first_diverged": None}


def census_pair(name, orig, d2rs, masks, table, exclude):
    """Add one check's pairs to `table` {(stream, id): row}. `orig`/`d2rs`
    are the loaded record lists; d2rs may be None (1.14d only: carried)."""
    wo, last_o, _, _ = pd.windows(orig, exclude)
    wd, last_d, _, _ = pd.windows(d2rs, exclude) if d2rs is not None else ({}, None, None, 0)
    top = min(x for x in (last_o, last_d) if x is not None) if last_o is not None and last_d is not None else None
    for w in sorted(wo):
        for st in STREAMS:
            ra = wo[w].get(st, [])
            for m in ra:
                if m["bytes"]:
                    row = table.setdefault((st, m["bytes"][0]), new_row())
                    row["checks"].add(name)
                    row["records"] += 1
            if top is None or w > top:
                continue
            rb = wd.get(w, {}).get(st, [])
            for i in range(max(len(ra), len(rb))):
                a = ra[i] if i < len(ra) else None
                b = rb[i] if i < len(rb) else None
                m = a or b
                if not m["bytes"]:
                    continue
                row = table.setdefault((st, m["bytes"][0]), new_row())
                if a is None or b is None:
                    where, x, y = ("extra (d2rs only)" if a is None else "missing in d2rs"), None, None
                    if a is None:
                        row["checks"].discard(None)
                        row["d2rs_only"] += 1
                else:
                    where, x, y, _ = pd.compare_record(st, a, b, masks.get(st, {}))
                if where:
                    row["diverged"] += 1
                    if row["first_diverged"] is None:
                        row["first_diverged"] = (name, w, where, x, y)
                    break   # later records of the window are not paired
                row["equal"] += 1
                if row["first_equal"] is None:
                    row["first_equal"] = (name, w)
    # windows with d2rs records only (1.14d had nothing in them)
    for w in sorted(wd):
        if w in wo or top is None or w > top:
            continue
        for st in STREAMS:
            for m in wd[w].get(st, []):
                if m["bytes"]:
                    row = table.setdefault((st, m["bytes"][0]), new_row())
                    row["d2rs_only"] += 1
                    row["diverged"] += 1
                    if row["first_diverged"] is None:
                        row["first_diverged"] = (name, w, "extra (d2rs only)", None, None)
                    break


def build(dirs, pattern=None):
    """({(stream, id): row}, {check: status}) over the work dirs."""
    masks = pd.load_masks()
    exclude = pd.transport_rows()
    table, status = {}, {}
    for n in check_names(pattern):
        o = d = None
        for base in dirs:
            po = os.path.join(base, n, "orig.packets.jsonl")
            if os.path.exists(po):
                o, d = po, os.path.join(base, n, "d2rs.packets.jsonl")
                break
        if o is None:
            status[n] = "no 1.14d packets recording"
            continue
        try:
            orig = pd.load(o)[1]
            d2rs = pd.load(d)[1] if os.path.exists(d) else None
        except pd.PacketsError as e:
            status[n] = f"unreadable: {e}"
            continue
        census_pair(n, orig, d2rs, masks, table, exclude)
        status[n] = "ok" if d2rs is not None else "1.14d only (no d2rs recording)"
    return table, status


def all_ids():
    nm = pd.names()
    return {st: sorted(i for (s, i) in nm if s == st) for st in STREAMS}, nm


def ids_state(row):
    if row is None or not row["checks"]:
        return "NOT-CARRIED"
    if row["diverged"]:
        return "DIVERGED"
    if row["equal"]:
        return "EQUAL"
    return "CARRIED-UNPAIRED"


def tsv_lines(table):
    ids, nm = all_ids()
    out = ["\t".join(["stream", "id", "name", "state", "checks", "carried_by", "records", "equal",
                      "diverged", "first_equal", "first_diverged"])]
    for st in STREAMS:
        for i in sorted(set(ids[st]) | {k[1] for k in table if k[0] == st}):
            r = table.get((st, i))
            fe = fd = "-"
            if r and r["first_equal"]:
                fe = f"{r['first_equal'][0]}@{r['first_equal'][1]}"
            if r and r["first_diverged"]:
                n, w, where, x, y = r["first_diverged"]
                fd = f"{n}@{w}:{where}" + ("" if x is None else f" 1.14d {x} vs d2rs {y}")
            out.append("\t".join([
                st, f"0x{i:02X}", nm.get((st, i), "-"), ids_state(r),
                str(len(r["checks"])) if r else "0",
                ",".join(sorted(r["checks"])[:3]) + ("…" if r and len(r["checks"]) > 3 else "") if r else "-",
                str(r["records"]) if r else "0", str(r["equal"]) if r else "0",
                str(r["diverged"]) if r else "0", fe, fd]))
    return out


def md_report(table, status, tool_cmd):
    ids, nm = all_ids()
    L = ["# Packet census", "",
         f"Generated by `{tool_cmd}` (format {FORMAT}). Per message id: the checks whose "
         "1.14d packets recording carries it and its first equal / diverged instance "
         "(packets_diff record compare, transport ids excluded). Machine-readable: "
         "`docs/handoff/packet-census.tsv`.", ""]
    ok = sum(1 for v in status.values() if v == "ok")
    L += [f"Checks: {len(status)}; with both recordings {ok}.", ""]
    miss = [f"{n} ({v})" for n, v in sorted(status.items()) if v != "ok"]
    if miss:
        L += ["Not in the census: " + "; ".join(miss), ""]
    for st, title in (("c2s", "C->S"), ("s2c", "S->C")):
        rows = [(i, table.get((st, i))) for i in ids[st]]
        cnt = {}
        for _, r in rows:
            cnt[ids_state(r)] = cnt.get(ids_state(r), 0) + 1
        L += [f"## {title}", "", ", ".join(f"{k} {v}" for k, v in sorted(cnt.items())), "",
              "| id | name | state | checks | records | equal | diverged | first equal | first diverged |",
              "|---|---|---|---|---|---|---|---|---|"]
        for i, r in rows:
            fe = f"{r['first_equal'][0]}@{r['first_equal'][1]}" if r and r["first_equal"] else "-"
            fd = "-"
            if r and r["first_diverged"]:
                n, w, where, x, y = r["first_diverged"]
                fd = f"{n}@{w}: {where}" + ("" if x is None else f" ({x} vs {y})")
            L.append(f"| 0x{i:02X} | {nm.get((st, i), '-')} | {ids_state(r)} | "
                     f"{len(r['checks']) if r else 0} | {r['records'] if r else 0} | "
                     f"{r['equal'] if r else 0} | {r['diverged'] if r else 0} | {fe} | {fd} |")
        L.append("")
    L += ["## Ids no check carries: scripted checks that would", "",
          "Proposed minimal checks (feed `check-gen`); a poke/send line per id family is "
          "the cheapest way to make 1.14d emit it.", ""]
    L += propose(table, ids, nm)
    return "\n".join(L) + "\n"


# The shape of a scripted check per message family: how to make 1.14d carry the id.
# C->S ids are produced by `send` lines; S->C ids by a poke or by a C->S id's reply.
def propose(table, ids, nm):
    out = ["| stream | uncarried ids | proposed check |", "|---|---|---|"]
    for st in STREAMS:
        miss = [i for i in ids[st] if not (table.get((st, i)) and table[(st, i)]["checks"])]
        if not miss:
            out.append(f"| {st} | none | - |")
            continue
        names = ", ".join(f"0x{i:02X} {nm.get((st, i), '-')}" for i in miss)
        how = ("one `send <frame> <Name> k=v ...` line per id in one check "
               "`net-c2s-<family>.check` (family = specs/sim/client-messages.tsv kind), after the poke "
               "that makes the gate pass (alive, in game, a unit to target)" if st == "c2s" else
               "the reply to the C->S id above, or a poke that triggers the sender "
               "(`net-s2c-<family>.check`); ids with produced_by session/transport need a "
               "session-level check")
        out.append(f"| {st} | {names} | {how} |")
    return out


# --- ledger part --------------------------------------------------------------

def part_rows(table, ledger_path):
    """Rows of the net.* ledger area, state from the data (the other columns
    from the ledger row). Returns (header lines, rows, changed)."""
    with open(ledger_path, newline="", encoding="utf-8") as f:
        lines = [x for x in f.read().splitlines() if not x.startswith("#")]
    rd = csv.DictReader(lines, delimiter="\t")
    out = []
    for r in rd:
        a = r["area"]
        parts = a.split(".")
        if len(parts) != 3 or parts[0] != "net" or parts[1] not in STREAMS or not parts[2].startswith("0x"):
            continue
        _, st, idtxt = parts
        row = table.get((st, int(idtxt, 16)))
        s = ids_state(row)
        if s == "DIVERGED":
            n, w, where, x, y = row["first_diverged"]
            r.update(state="DIVERGED", last_verdict=f"DIVERGED@{w}",
                     checks=",".join(sorted(row["checks"])[:6]), exercised="yes",
                     note=f"packet-census: first diverged {n}@{w} {where}"
                          + ("" if x is None else f" (1.14d {x} vs d2rs {y})")
                          + f"; {row['equal']} equal pair(s)")
        elif s == "EQUAL":
            fe = row["first_equal"]
            r.update(state="EQUAL", last_verdict="MATCH", size="-", exercised="yes",
                     checks=",".join(sorted(row["checks"])[:6]),
                     note=f"packet-census: {row['equal']} equal pair(s), first {fe[0]}@{fe[1]}; "
                          "no diverged pair in any check")
        elif s == "CARRIED-UNPAIRED":
            r.update(last_verdict="-", exercised="yes", checks=",".join(sorted(row["checks"])[:6]),
                     note="packet-census: carried by 1.14d recording(s) but never paired with d2rs; "
                          + r["note"])
        else:
            r["note"] = "packet-census: no check carries this id; " + r["note"]
        out.append(r)
    return out


def write_part(rows, path):
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write("#ledger 1\n")
        f.write("\t".join(PART_COLS) + "\n")
        for r in rows:
            f.write("\t".join(r[c] for c in PART_COLS) + "\n")


def cmd_report(args):
    dirs = [args.dir, SUITE_DIR] if args.dir != SUITE_DIR else [SUITE_DIR]
    table, status = build(dirs, args.filter)
    tool_cmd = "python3 tools/packet-census/packet_census.py report " + " ".join(sys.argv[2:])
    if args.md:
        with open(args.md, "w", encoding="utf-8") as f:
            f.write(md_report(table, status, tool_cmd))
    if args.tsv:
        with open(args.tsv, "w", encoding="utf-8") as f:
            f.write("\n".join(tsv_lines(table)) + "\n")
    if args.ledger and args.part:
        write_part(part_rows(table, args.ledger), args.part)
    states = {}
    for (st, i), r in table.items():
        states[ids_state(r)] = states.get(ids_state(r), 0) + 1
    print("census:", len(status), "checks;", json.dumps(states, sort_keys=True))
    return 0


# --- selftest -----------------------------------------------------------------

def selftest():
    masks = pd.load_masks()
    exclude = pd.transport_rows()
    orig = pd.synthetic()
    same = pd.as_d2rs(orig)
    t = {}
    census_pair("a", orig, same, masks, t, exclude)
    assert t[("c2s", 0x01)]["equal"] > 0 and t[("c2s", 0x01)]["diverged"] == 0, t[("c2s", 0x01)]
    assert ids_state(t[("s2c", 0x0C)]) == "EQUAL" and t[("s2c", 0x0C)]["checks"] == {"a"}
    assert t[("s2c", 0x0C)]["first_equal"][0] == "a"
    # transport ids (S->C 0x8F-like, C->S 0x6D) never counted
    assert ("c2s", 0x6D) not in t
    # a changed byte in id 0x0C diverges that id only
    bad = pd.as_d2rs(orig)
    for r in bad:
        if r.get("type") == "s2c" and r["bytes"].startswith("0c") and r["frame"] == 3:
            b = bytearray(bytes.fromhex(r["bytes"]))
            b[2] ^= 0xFF
            r["bytes"] = b.hex()
    t = {}
    census_pair("b", orig, bad, masks, t, exclude)
    row = t[("s2c", 0x0C)]
    assert ids_state(row) == "DIVERGED" and row["first_diverged"][:2] == ("b", 3), row
    assert row["first_diverged"][2].startswith("bytes["), row["first_diverged"]
    assert row["equal"] >= 2    # frames 1-2 paired equal first
    # 1.14d only: carried, never paired
    t = {}
    census_pair("c", orig, None, masks, t, exclude)
    assert ids_state(t[("s2c", 0x0C)]) == "CARRIED-UNPAIRED"
    assert ids_state(None) == "NOT-CARRIED"
    # tsv and md render
    assert tsv_lines(t)[0].startswith("stream\tid\tname")
    assert "## C->S" in md_report(t, {"c": "ok"}, "cmd") and "Ids no check carries" in md_report(t, {}, "cmd")
    # run: one scenario_diff call per check with the packets channel only
    calls = []

    class R:
        returncode = 0
    ns = argparse.Namespace(filter="a1-town*", dir="/tmp/x", reuse_orig=True)
    cmd_run(ns, run=lambda cmd, check: calls.append(cmd) or R())
    assert calls and all("--channels" in c and "packets" in c for c in calls), calls
    print("packet_census selftest ok")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--selftest", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    r = sub.add_parser("run")
    r.add_argument("--filter", default=None)
    r.add_argument("--dir", default=DEFAULT_DIR)
    r.add_argument("--reuse-orig", action="store_true")
    p = sub.add_parser("report")
    p.add_argument("--filter", default=None)
    p.add_argument("--dir", default=DEFAULT_DIR)
    p.add_argument("--md", default=None)
    p.add_argument("--tsv", default=None)
    p.add_argument("--ledger", default=None, help="fidelity-ledger.tsv (the net.* rows to settle)")
    p.add_argument("--part", default=None, help="part file to write")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if a.cmd == "run":
        return 3 if cmd_run(a) else 0
    if a.cmd == "report":
        return cmd_report(a)
    ap.print_help()
    return 3


if __name__ == "__main__":
    sys.exit(main())
