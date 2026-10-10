"""Compare two packet recordings (format packets-raw-1: 1.14d from
record_packets.py, d2rs from `d2-client state-dump --packets`) frame by
frame and print the first divergence, then the next N, then a summary
(specs/tools/packets-trace.md §3, specs/sim/intents-events.md §6).

    python3 packets_diff.py ORIG.packets.jsonl D2RS.packets.jsonl [--next 20]
        [--streams c2s,s2c,buf] [--from F] [--to F] [--masks TSV]
        [--masks-c2s TSV] [--json FILE]
    python3 packets_diff.py --selftest

Per frame window F (intents-events.md §6 rule 1: from the end of tick
F-1 to the end of tick F) three streams are compared record by record:
`c2s` (every client message the server took from its queues, game and
system queue, in drain order), `s2c` (every server message queued for a
client, plus direct sends in their position) and `buf` (the buffers the
flush handed to delivery: client and size; their bytes are the s2c
messages, packed, §6 rule 2). Masked S->C bytes (specs/tools/
scenario-masks.tsv, rules of specs/tools/scenario.md §6, keys read from
the 1.14d record) and masked C->S bytes (specs/tools/scenario-masks-c2s.tsv,
§6 rule 4) are skipped.

Exit codes: 0 MATCH, 1 DIVERGED, 2 PARTIAL, 3 error.

Standard library only; runs on Linux and Windows. Our own code.
"""

import argparse
import copy
import csv
import json
import os
import sys

FORMAT = "packets-raw-1"
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
SPECS = os.path.join(REPO, "specs")
MASKS = os.path.join(SPECS, "tools", "scenario-masks.tsv")
MASKS_C2S = os.path.join(SPECS, "tools", "scenario-masks-c2s.tsv")
FLUSH_SITE = 0x52E3B5   # the flush's call of the net send (intents-events.md §3.2 rule 4)
STREAMS = ("c2s", "s2c", "buf")
CONTEXT = 8             # bytes either side of a differing offset


class PacketsError(Exception):
    pass


# --- inputs -------------------------------------------------------------------

def load(path):
    """(header, [records], footer) of one packets-raw-1 file."""
    header, recs, footer = None, [], None
    with open(path, encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except ValueError as e:
                raise PacketsError(f"{path}:{n}: not JSON ({e})")
            t = rec.get("type")
            if header is None:
                if t != "header" or rec.get("format") != FORMAT:
                    raise PacketsError(f"{path}:{n}: not a {FORMAT} file (first line must be "
                                       f"its header)")
                header = rec
            elif t == "footer":
                footer = rec
            else:
                recs.append(rec)
    if header is None:
        raise PacketsError(f"{path}: empty")
    return header, recs, footer


def names():
    """{('c2s'|'s2c', id): name} from the two message tables."""
    out = {}
    for side, fn in (("c2s", "client-messages.tsv"), ("s2c", "server-messages.tsv")):
        p = os.path.join(SPECS, "sim", fn)
        if not os.path.exists(p):
            continue
        with open(p, newline="", encoding="utf-8") as f:
            for r in csv.DictReader(f, delimiter="\t"):
                out[(side, int(r["id"], 16))] = r["name"]
    return out


def c2s_ids():
    """The C->S ids with a row in client-messages.tsv (named, not '-')."""
    p = os.path.join(SPECS, "sim", "client-messages.tsv")
    with open(p, newline="", encoding="utf-8") as f:
        return {int(r["id"], 16) for r in csv.DictReader(f, delimiter="\t") if r["name"] != "-"}


def read_masks(path=MASKS, side="s2c"):
    """The mask table, strictly (scenario.md §6 rules 3-4): {id: [(key,
    off, len)]}. `side` 's2c': ids are S->C ids <= 0xB4; 'c2s': ids are
    C->S ids of client-messages.tsv."""
    known = c2s_ids() if side == "c2s" else None
    out = {}
    with open(path, newline="", encoding="utf-8") as f:
        lines = f.read().splitlines()
    if not lines or lines[0].split("\t") != ["id", "key", "offset", "length", "source"]:
        raise PacketsError(f"{path}: header must be id key offset length source")
    for n, line in enumerate(lines[1:], 2):
        if not line.strip():
            continue
        cols = line.split("\t")
        if len(cols) != 5:
            raise PacketsError(f"{path}:{n}: five columns")
        sid, key, off, ln, src = cols
        try:
            mid = int(sid, 16)
            if not sid.lower().startswith("0x"):
                raise ValueError
            if (mid > 0xB4) if known is None else (mid not in known):
                raise ValueError
        except ValueError:
            raise PacketsError(f"{path}:{n}: id must be a 0x S->C id <= 0xB4" if known is None
                               else f"{path}:{n}: id must be a 0x C->S id of client-messages.tsv")
        k = None
        if key != "-":
            try:
                w, rest = key.split("@")
                o, v = rest.split("=")
                if w not in ("u8", "u16") or not v.lower().startswith("0x"):
                    raise ValueError
                k = (1 if w == "u8" else 2, int(o, 10), int(v, 16))
            except ValueError:
                raise PacketsError(f"{path}:{n}: key '-' or u8@<off>=0x<v> / u16@<off>=0x<v>")
        if off.startswith("nul@") and off[4:].isdigit():
            o = ("nul", int(off[4:]))
        elif off.isdigit():
            o = ("at", int(off))
        else:
            raise PacketsError(f"{path}:{n}: offset decimal or nul@<n>")
        if ln == "*":
            L = ("end", None)
        elif ln.startswith("..") and ln[2:].isdigit():
            L = ("thru", int(ln[2:]))
        elif ln.isdigit() and int(ln) > 0:
            L = ("len", int(ln))
        else:
            raise PacketsError(f"{path}:{n}: length non-zero decimal, * or ..<n>")
        if not src.strip():
            raise PacketsError(f"{path}:{n}: empty source")
        out.setdefault(mid, []).append((k, o, L))
    return out


def load_masks(s2c=MASKS, c2s=MASKS_C2S):
    """Both mask tables, per stream: {'s2c': table, 'c2s': table}."""
    return {"s2c": read_masks(s2c), "c2s": read_masks(c2s, "c2s")}


def masked(masks, b):
    """Offsets of message `b` (the original's bytes) that `masks` (one
    stream's table) masks."""
    out = set()
    if not b:
        return out
    for key, (okind, oval), (lkind, lval) in masks.get(b[0], ()):
        if key is not None:
            w, o, v = key
            if len(b) < o + w or int.from_bytes(b[o:o + w], "little") != v:
                continue
        if okind == "nul":
            z = b.find(b"\0", oval)
            if z < 0:
                continue
            start = z + 1
        else:
            start = oval
        if lkind == "end":
            end = len(b)
        elif lkind == "thru":
            end = lval + 1
        else:
            end = start + lval
        out.update(range(start, max(start, end)))
    return out


# --- windows and streams ------------------------------------------------------

# S->C 0xB4 ConnectionRefused: compared although it is a transport row.
KEPT_S2C = 0xB4


def transport_rows():
    """Ids excluded with their rows (intents-events.md §6 rule 3, §4 rule
    4): C->S 0x66 (warden) and 0x6D (ping); S->C rows `produced_by`
    transport (0x8F pong, 0xAE, 0xAF-0xB3). 0xB4 (ConnectionRefused) is kept:
    it is the load result of a refused join (formats/d2s-load.md §5 r2a),
    sent only then."""
    c2s, s2c = {0x66, 0x6D}, set()
    p = os.path.join(SPECS, "sim", "server-messages.tsv")
    if os.path.exists(p):
        with open(p, newline="", encoding="utf-8") as f:
            s2c = {int(r["id"], 16) for r in csv.DictReader(f, delimiter="\t")
                   if r.get("produced_by") == "transport"} - {KEPT_S2C}
    return {"c2s": c2s, "s2c": s2c}


def windows(recs, exclude=None):
    """{window: {stream: [record]}}, the last complete window (tick frame of
    the last tick_end), the first tick frame and the records excluded.
    Window of a record: its frame while a tick runs (phase 'tick') and for
    a flushed buffer, else frame + 1 (the next tick's); before the first
    tick, the first tick's frame. `exclude` ({'c2s': ids, 's2c': ids}) drops those messages; a
    flushed buffer's `size` then counts only the messages kept (its
    queued messages are the window's s2c records of its client, in
    order)."""
    ticks = [r["frame"] for r in recs if r.get("type") == "tick" and r.get("frame") is not None]
    ends = [r["frame"] for r in recs if r.get("type") == "tick_end" and r.get("frame") is not None]
    first = ticks[0] if ticks else None
    last = ends[-1] if ends else None
    clients = {}
    out = {}
    for r in recs:
        t = r.get("type")
        if t in ("c2s", "c2s_sys"):
            st = "c2s"
        elif t == "s2c":
            st = "s2c"
        elif t == "net":
            flushed = r.get("via") != "direct" and _site(r.get("caller")) == FLUSH_SITE
            st = "buf" if flushed else "s2c"
        else:
            continue
        f = r.get("frame")
        if (r.get("phase") == "tick" or st == "buf") and f is not None:
            w = f  # a flushed buffer goes with the tick it follows (§6 rule 2)
        elif f is not None:
            w = f + 1
        elif first is not None:
            w = first
        else:
            continue
        c = clients.setdefault(r.get("client"), len(clients))
        b = bytes.fromhex(r.get("bytes", ""))
        kind = {"c2s": "game", "c2s_sys": "system", "s2c": "queued"}.get(
            t, "flushed" if st == "buf" else "direct")
        out.setdefault(w, {}).setdefault(st, []).append(
            {"kind": kind, "client": c, "size": r.get("size", len(b)), "bytes": b,
             "seq": r.get("seq")})
    dropped = 0
    if exclude:
        for w, sts in out.items():
            for b in sts.get("buf", []):
                b["raw_size"] = b["size"]
            queued = {}
            for m in sts.get("s2c", []):
                if m["kind"] == "queued":
                    queued.setdefault(m["client"], []).append(m)
            for b in sts.get("buf", []):
                q, n, kept = queued.get(b["client"], []), 0, 0
                while q and n < b["size"]:
                    m = q.pop(0)
                    n += m["size"]
                    if m["bytes"][:1] and m["bytes"][0] not in exclude["s2c"]:
                        kept += m["size"]
                if n == b["size"]:
                    b["size"] = kept
            for st in ("c2s", "s2c"):
                keep = [m for m in sts.get(st, [])
                        if not (m["bytes"][:1] and m["bytes"][0] in exclude[st])]
                dropped += len(sts.get(st, [])) - len(keep)
                if st in sts:
                    sts[st] = keep
    return out, last, first, dropped


def _site(s):
    try:
        return int(str(s), 16)
    except ValueError:
        return None


def _hexctx(b, k, marks=frozenset()):
    lo, hi = max(0, k - CONTEXT), min(len(b), k + CONTEXT + 1)
    parts = []
    for i in range(lo, hi):
        x = "--" if i in marks else f"{b[i]:02x}"
        parts.append(f"[{x}]" if i == k else x)
    return f"@{lo}: " + " ".join(parts)


def compare_record(stream, a, b, masks):
    """(where, expected, got, skipped) of the first difference of two
    records of one stream, or (None, ..., skipped)."""
    if a["client"] != b["client"]:
        return "client", a["client"], b["client"], 0
    if stream == "buf":
        if a["size"] != b["size"]:
            return "size", a["size"], b["size"], 0
        return None, None, None, 0
    if a["kind"] != b["kind"]:
        return "kind", a["kind"], b["kind"], 0
    ia, ib = a["bytes"][:1], b["bytes"][:1]
    if ia != ib:
        return "id", ia.hex() or "-", ib.hex() or "-", 0
    if a["size"] != b["size"]:
        return "size", a["size"], b["size"], 0
    m = masked(masks.get(stream, {}), a["bytes"])
    n = max(len(a["bytes"]), len(b["bytes"]))
    for k in range(n):
        if k in m:
            continue
        x = a["bytes"][k] if k < len(a["bytes"]) else None
        y = b["bytes"][k] if k < len(b["bytes"]) else None
        if x != y:
            return f"bytes[{k}]", x, y, len([i for i in m if i < n])
    return None, None, None, len([i for i in m if i < n])


def diff(orig, d2rs, masks, streams=STREAMS, lo=None, hi=None, exclude=None):
    """Every divergence (first per window and stream), and the summary."""
    wo, last_o, first_o, xo = windows(orig, exclude)
    wd, last_d, first_d, xd = windows(d2rs, exclude)
    summary = {"windows": 0, "records": {s: 0 for s in streams}, "masked": 0,
               "first_tick": (first_o, first_d), "last_tick": (last_o, last_d), "partial": [],
               "excluded": (xo, xd)}
    divs = []
    if last_o is None or last_d is None:
        summary["partial"].append("a side has no complete tick")
        return divs, summary
    if first_o != first_d:
        divs.append({"window": min(first_o, first_d), "stream": "tick", "index": 0,
                     "where": "first tick frame", "expected": first_o, "got": first_d})
    top = min(last_o, last_d)
    if last_o != last_d:
        summary["partial"].append(f"ticks differ: 1.14d to frame {last_o}, d2rs to {last_d}; "
                                  f"compared to {top}")
    start = min(x for x in (first_o, first_d) if x is not None)
    keys = [k for k in range(min([start] + list(wo) + list(wd)), top + 1)
            if (lo is None or k >= lo) and (hi is None or k <= hi)]
    for w in keys:
        summary["windows"] += 1
        for st in streams:
            ra, rb = wo.get(w, {}).get(st, []), wd.get(w, {}).get(st, [])
            for i in range(max(len(ra), len(rb))):
                a = ra[i] if i < len(ra) else None
                b = rb[i] if i < len(rb) else None
                summary["records"][st] += 1
                if a is None or b is None:
                    divs.append({"window": w, "stream": st, "index": i,
                                 "where": "extra (d2rs only)" if a is None else "missing in d2rs",
                                 "a": a, "b": b, "prev": ra[i - 1] if i and i - 1 < len(ra) else None,
                                 "counts": (len(ra), len(rb))})
                    break
                where, x, y, skipped = compare_record(st, a, b, masks)
                summary["masked"] += skipped
                if where:
                    divs.append({"window": w, "stream": st, "index": i, "where": where,
                                 "expected": x, "got": y, "a": a, "b": b,
                                 "prev": ra[i - 1] if i else None, "counts": (len(ra), len(rb))})
                    break
    return divs, summary


def describe(d, nm, masks):
    st = d["stream"]
    lines = [f"frame {d['window']}  stream {st}  #{d['index']}  {d['where']}"]
    if "expected" in d:
        lines.append(f"  1.14d: {d['expected']}   d2rs: {d['got']}")
    for side, r in (("1.14d", d.get("a")), ("d2rs ", d.get("b"))):
        if r is None:
            continue
        b = r["bytes"]
        mid = b[0] if b else None
        name = nm.get(("c2s" if st == "c2s" else "s2c", mid), "?") if st != "buf" else "buffer"
        head = f"  {side} seq {r['seq']}: {r['kind']} client#{r['client']} "
        head += f"size {r['size']}" if st == "buf" else f"id {mid:#04x} {name} size {r['size']}"
        lines.append(head)
        if st != "buf" and b:
            k = int(d["where"][6:-1]) if d["where"].startswith("bytes[") else 0
            marks = masked(masks.get(st, {}), d["a"]["bytes"]) if d.get("a") else set()
            lines.append("    " + _hexctx(b, k, marks))
    if d.get("counts"):
        lines.append(f"  records in this frame's stream: 1.14d {d['counts'][0]}, "
                     f"d2rs {d['counts'][1]}")
    p = d.get("prev")
    if p is not None and p["bytes"]:
        lines.append(f"  previous (both equal): id {p['bytes'][0]:#04x} size {p['size']}")
    return "\n".join(lines)


def report(divs, summary, nm, masks, nxt, out=print):
    if divs:
        out("FIRST DIVERGENCE")
        out(describe(divs[0], nm, masks))
        if len(divs) > 1 and nxt:
            out(f"\nNEXT {min(nxt, len(divs) - 1)} (first per frame and stream)")
            for d in divs[1:1 + nxt]:
                out(describe(d, nm, masks))
    first = {}
    for d in divs:
        first.setdefault(d["stream"], d["window"])
    out("\nSUMMARY")
    out(f"  frames compared: {summary['windows']} (ticks 1.14d {summary['first_tick'][0]}.."
        f"{summary['last_tick'][0]}, d2rs {summary['first_tick'][1]}..{summary['last_tick'][1]})")
    out("  records compared: " + "  ".join(f"{k}={v}" for k, v in summary["records"].items()))
    out(f"  masked bytes skipped: {summary['masked']}")
    xo, xd = summary.get("excluded", (0, 0))
    if xo or xd:
        out(f"  transport rows excluded (intents-events.md §6 rule 3): 1.14d {xo}, d2rs {xd}")
    out(f"  divergent frame/stream pairs: {len(divs)}"
        + ("" if not first else "; first per stream: "
           + ", ".join(f"{k} frame {v}" for k, v in first.items())))
    for p in summary["partial"]:
        out(f"  partial: {p}")
    if divs:
        out("DIVERGED")
        return 1
    if summary["partial"] or summary["windows"] == 0:
        out("PARTIAL")
        return 2
    out("MATCH")
    return 0


VERDICTS = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR"}


def summary(divs, s, code):
    """The machine-readable summary (`--json FILE`, format diff-summary-1;
    specs/tools/scenario-diff.md §4): frame windows compared, windows with
    no divergence in any stream, the verdict and the first divergence."""
    bad = {d["window"] for d in divs}
    first = None
    if divs:
        d = divs[0]
        text = f"frame {d['window']} stream {d['stream']} #{d['index']} {d['where']}"
        if "expected" in d:
            text += f": 1.14d {d['expected']} vs d2rs {d['got']}"
        m = d.get("a") or d.get("b")
        if m and d["stream"] != "buf" and m["bytes"]:
            text += f" (id {m['bytes'][0]:#04x})"
        first = {"frame": d["window"], "text": text}
    return {"format": "diff-summary-1", "channel": "packets", "tool": "packets_diff.py",
            "code": code, "verdict": VERDICTS[code], "frames_compared": s["windows"],
            "frames_equal": s["windows"] - len(bad), "records": s["records"],
            "differences": len(divs), "first": first}


def write_json(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=1)
        f.write("\n")


# --- self-test ----------------------------------------------------------------

def synthetic(frames=4):
    """A recording in the 1.14d shape: drains, c2s with dispatch, ticks
    with s2c messages (two masked ids among them), flushes."""
    recs = []
    seq = 0

    def add(frame, phase, **r):
        nonlocal seq
        r.update(frame=frame, phase=phase, seq=seq)
        seq += 1
        recs.append(r)

    frame = None
    add(None, "start", type="client_out", size=46, bytes=("67" + "00" * 45))
    add(None, "input", type="drain")
    add(None, "input", type="c2s_sys", client=1, size=46, bytes=("67" + "11" * 45))
    add(None, "input", type="s2c", client=1, size=1, bytes="02")
    for f in range(1, frames + 1):
        if f > 1:
            add(frame, "input", type="drain")
            add(frame, "input", type="c2s", client=1, size=5, bytes=f"01{f:02x}00{f + 7:02x}00")
            add(frame, "input", type="dispatch", id=1, size=5, game_frame=frame)
            add(frame, "input", type="result", dispatched=True, code=0)
        frame = f
        add(frame, "tick", type="tick", game="0x1000")
        msgs = [bytes([0x0C]) + bytes((f * 3 + i) & 0xFF for i in range(8)),
                bytes([0x7E, 9, 9, 9, 9]),                              # 1-4 masked
                bytes([0x50, 4, 0]) + bytes(range(10)) + b"\xaa\xbb",   # keyed: 13-14
                bytes([0x15]) + bytes(range(f, f + 10))]
        for m in msgs:
            add(frame, "tick", type="s2c", client=1, size=len(m), bytes=m.hex())
        add(frame, "post", type="tick_end")
        add(frame, "flush", type="flush")
        buf = b"".join(msgs) if f > 1 else b"\x02" + b"".join(msgs)
        add(frame, "flush", type="net", kind=1, client=1, size=len(buf), bytes=buf.hex(),
            caller=f"{FLUSH_SITE:#x}")
        if f == 2:
            add(frame, "input", type="drain")
            add(frame, "input", type="net", kind=1, client=1, size=1, bytes="b0",
                caller="0x53b22e")
    return recs


def as_d2rs(recs):
    """The same records as the d2rs recorder writes them: other client id,
    `via: direct` instead of a 1.14d caller."""
    out = copy.deepcopy(recs)
    for r in out:
        if "client" in r:
            r["client"] = 0
        if r.get("type") == "net" and _site(r.get("caller")) != FLUSH_SITE:
            del r["caller"]
            r["via"] = "direct"
    return out


# Covers: specs/tools/packets-trace.md §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3 r6, §4 r1
# Covers: specs/tools/scenario.md §6 r4
def selftest():
    masks = load_masks()
    nm = names()
    quiet = lambda *a, **k: None  # noqa: E731
    o = synthetic()
    d = as_d2rs(o)
    divs, s = diff(o, d, masks)
    assert not divs and report(divs, s, nm, masks, 0, quiet) == 0, divs
    assert s["masked"] > 0
    sm = summary(divs, s, 0)
    assert (sm["frames_compared"], sm["frames_equal"], sm["first"]) == (4, 4, None), sm
    ok, missed = 2, []
    # every byte of every message record of the d2rs side, perturbed
    for i, r in enumerate(d):
        if not r.get("bytes") or r["type"] not in ("c2s", "c2s_sys", "s2c", "net"):
            continue  # client-side records are not compared
        b = bytes.fromhex(r["bytes"])
        st = {"s2c": "s2c", "c2s": "c2s", "c2s_sys": "c2s"}.get(r["type"])
        mk = masked(masks.get(st, {}), bytes.fromhex(o[i]["bytes"]))
        for k in range(len(b)):
            p = copy.deepcopy(d)
            bb = bytearray(b)
            bb[k] ^= 0x5A
            p[i]["bytes"] = bb.hex()
            divs, _ = diff(o, p, masks)
            if k in mk:
                if divs:
                    missed.append(f"seq {r['seq']} masked byte {k} reported")
                else:
                    ok += 1
                continue
            if r["type"] == "net" and r.get("caller") == f"{FLUSH_SITE:#x}":
                # buffers are compared by size; their bytes are the s2c records
                ok += 1
                continue
            hit = [x for x in divs if x.get("b") is not None and x["b"]["seq"] == r["seq"]]
            want = "id" if k == 0 else f"bytes[{k}]"
            if not hit or hit[0]["where"] != want:
                missed.append(f"seq {r['seq']} byte {k}: {divs[:1]}")
            else:
                ok += 1
    # a size change, a dropped record, an extra record (the last s2c of
    # frame 1), a buffer split
    last = next(i for i, r in enumerate(d) if r.get("frame") == 1 and r.get("bytes", "")[:2] == "15")
    for what, mutate, where in (
            ("size", lambda p: p[3].update(size=2), "size"),
            ("drop", lambda p: p.pop(last), "missing in d2rs"),
            ("extra", lambda p: p.insert(last + 1, dict(p[last], seq=999)), "extra (d2rs only)"),
            ("buf", lambda p: next(r for r in p if r["type"] == "net").update(size=7), "size")):
        p = copy.deepcopy(d)
        mutate(p)
        divs, s = diff(o, p, masks)
        if not divs or divs[0]["where"] != where:
            missed.append(f"{what}: {divs[:1]}")
        else:
            ok += 1
            sm = summary(divs, s, 1)
            bad = len({x["window"] for x in divs})
            if sm["frames_equal"] != sm["frames_compared"] - bad or not sm["first"] \
                    or sm["first"]["frame"] != divs[0]["window"] or bad < 1:
                missed.append(f"{what}: summary {sm}")
    # the key of a keyed mask is compared, and its row then does not apply
    p = copy.deepcopy(d)
    r = next(r for r in p if r.get("bytes", "").startswith("5004"))
    bb = bytearray.fromhex(r["bytes"])
    bb[1] = 5
    r["bytes"] = bb.hex()
    divs, _ = diff(o, p, masks)
    assert divs and divs[0]["where"] == "bytes[1]", divs[:1]
    ok += 1
    # transport rows (ping, pong, 0xAF) on the 1.14d side only: excluded,
    # and the buffer that carried the pong is compared without it
    t = copy.deepcopy(o)
    first_tick = next(i for i, r in enumerate(t) if r["type"] == "tick")
    t.insert(0, dict(type="net", kind=1, client=1, size=2, bytes="af00", caller="0x52b796",
                     frame=None, phase="start", seq=-1))
    t.insert(first_tick, dict(type="c2s_sys", client=1, size=13, bytes="6d" + "11" * 12,
                              frame=None, phase="input", seq=-2))
    t.insert(first_tick + 1, dict(type="s2c", client=1, size=33, bytes="8f" + "22" * 32,
                                  frame=None, phase="input", seq=-3))
    buf = next(r for r in t if r["type"] == "net" and r.get("caller") == f"{FLUSH_SITE:#x}")
    buf["bytes"] = buf["bytes"][:2] + "8f" + "22" * 32 + buf["bytes"][2:]
    buf["size"] += 33
    divs, s = diff(t, d, masks, exclude=transport_rows())
    assert not divs and s["excluded"] == (4, 1), (divs[:1], s)
    divs, _ = diff(t, d, masks)
    assert divs and divs[0]["window"] == 1, divs[:1]
    ok += 1
    # fewer ticks on one side: partial, not a match
    short = [r for r in d if r.get("frame") is None or r["frame"] < 4
             or (r["frame"] == 4 and r["phase"] == "input")]
    divs, s = diff(o, short, masks)
    assert not divs and report(divs, s, nm, masks, 0, quiet) == 2, (divs, s)
    ok += 1
    # the mask reader is strict
    import tempfile
    for bad in ("0xB5\t-\t1\t1\tx", "0x50\tu32@1=0x1\t1\t1\tx", "0x50\t-\t1\t0\tx",
                "0x50\t-\tnul\t1\tx", "0x50\t-\t1\t1\t", "0x50\t-\t1"):
        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as f:
            f.write("id\tkey\toffset\tlength\tsource\n" + bad + "\n")
        try:
            read_masks(f.name)
        except PacketsError:
            ok += 1
        else:
            missed.append(f"mask row accepted: {bad!r}")
        finally:
            os.unlink(f.name)
    # nul@ and ..n: 0x82 name after its NUL through byte 20
    m82 = bytes([0x82, 1, 2, 3, 4]) + b"ab\0" + bytes(21)
    got = masked(masks["s2c"], m82[:29])
    assert got == set(range(8, 21)), sorted(got)
    ok += 1
    # the C->S table: ids are C->S ids of client-messages.tsv (0x67 is
    # one, 0x4A has no row, 0xB5 is past the table), read as strictly
    for bad, side in (("0x4A\t-\t1\t1\tx", "c2s"), ("0xFF\t-\t1\t1\tx", "c2s"),
                      ("0x67\t-\t1\t0\tx", "c2s")):
        with tempfile.NamedTemporaryFile("w", suffix=".tsv", delete=False) as f:
            f.write("id\tkey\toffset\tlength\tsource\n" + bad + "\n")
        try:
            read_masks(f.name, side)
        except PacketsError:
            ok += 1
        else:
            missed.append(f"c2s mask row accepted: {bad!r}")
        finally:
            os.unlink(f.name)
    # C->S 0x67 (scenario.md §6 rule 4, client/model.md §7 r9): the 1.14d
    # client leaves stack bytes after the game name NUL (through 16) and
    # after the character name NUL (through 36); d2rs sends zeros
    def m67(game, char, tail, gtype=0):
        b = bytearray(46)
        b[0] = 0x67
        g = game + b"\0"
        b[1:1 + len(g)] = g
        b[1 + len(g):17] = tail[:16 - len(g)]
        b[0x11] = gtype
        c = char + b"\0"
        b[0x15:0x15 + len(c)] = c
        b[0x15 + len(c):0x25] = tail[:0x25 - 0x15 - len(c)]
        return bytes(b)
    junk = bytes([0x45, 0x00, 0xFA, 0x05, 0x74, 0x00, 0x0C, 0x02, 0, 0,
                  0x20, 0xC5, 0x44, 0x00, 0x5C, 0x33])
    got = masked(masks["c2s"], m67(b"", b"ScnAma", junk))
    assert got == set(range(2, 17)) | set(range(0x15 + 7, 37)), sorted(got)
    ok += 1

    def one(b67):
        return [dict(type="c2s_sys", client=1, size=46, bytes=b67.hex(), frame=None,
                     phase="input", seq=0),
                dict(type="tick", frame=1, phase="tick", seq=1),
                dict(type="tick_end", frame=1, phase="post", seq=2)]
    orig67, d2rs67 = m67(b"", b"ScnAma", junk), m67(b"", b"ScnAma", bytes(16))
    assert orig67 != d2rs67
    divs, s = diff(one(orig67), one(d2rs67), masks)
    assert not divs and s["masked"] == 15 + 9, (divs[:1], s["masked"])
    ok += 1
    # the game type byte 0x11 is not masked
    divs, _ = diff(one(orig67), one(m67(b"", b"ScnAma", bytes(16), gtype=3)), masks)
    assert divs and divs[0]["stream"] == "c2s" and divs[0]["where"] == "bytes[17]", divs[:1]
    ok += 1
    # a byte inside the character name (before its NUL) is still compared
    divs, _ = diff(one(orig67), one(m67(b"", b"ScnAmb", bytes(16))), masks)
    assert divs and divs[0]["where"] == "bytes[26]", divs[:1]
    ok += 1
    # without the C->S table the stack bytes are reported
    divs, _ = diff(one(orig67), one(d2rs67), {"s2c": masks["s2c"]})
    assert divs and divs[0]["where"] == "bytes[2]", divs[:1]
    ok += 1
    if missed:
        print("packets_diff selftest FAILED:", *missed[:20], sep="\n  ")
        return 1
    print(f"packets_diff selftest: {ok} checks passed")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("orig", nargs="?")
    ap.add_argument("d2rs", nargs="?")
    ap.add_argument("--next", type=int, default=20, help="divergences after the first (default 20)")
    ap.add_argument("--streams", default=",".join(STREAMS))
    ap.add_argument("--from", dest="lo", type=int, default=None)
    ap.add_argument("--to", dest="hi", type=int, default=None)
    ap.add_argument("--masks", default=MASKS)
    ap.add_argument("--masks-c2s", default=MASKS_C2S)
    ap.add_argument("--keep-transport", action="store_true",
                    help="also compare the transport rows (ping, pong, 0xAE-0xB4)")
    ap.add_argument("--json", default=None, metavar="FILE",
                    help="also write a machine-readable summary (diff-summary-1) here")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not (a.orig and a.d2rs):
        ap.error("two recordings (1.14d first) or --selftest")
    try:
        streams = tuple(x for x in a.streams.split(",") if x)
        if not streams or any(x not in STREAMS for x in streams):
            raise PacketsError(f"--streams from {','.join(STREAMS)}")
        ho, ro, _ = load(a.orig)
        hd, rd, _ = load(a.d2rs)
        masks = load_masks(a.masks, a.masks_c2s)
    except (PacketsError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        if a.json:
            write_json(a.json, {"format": "diff-summary-1", "channel": "packets", "code": 3,
                                "verdict": "ERROR", "error": str(e)})
        return 3
    print(f"1.14d: {a.orig} ({len(ro)} records, {ho.get('tool')})")
    print(f"d2rs:  {a.d2rs} ({len(rd)} records, {hd.get('tool')})")
    for g in hd.get("gaps", []) + ho.get("gaps", []):
        print(f"gap: {g}")
    divs, s = diff(ro, rd, masks, streams, a.lo, a.hi,
                   None if a.keep_transport else transport_rows())
    if hd.get("gaps") or ho.get("gaps"):
        s["partial"].append("a side lists gaps")
    code = report(divs, s, names(), masks, a.next)
    if a.json:
        write_json(a.json, summary(divs, s, code))
    return code


if __name__ == "__main__":
    sys.exit(main())
