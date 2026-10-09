"""Compare the items two packet recordings show being created (the `items`
channel of scenario_diff.py, specs/tools/scenario-diff.md §3 rule 13):
1.14d from record_packets.py, d2rs from `d2-client state-dump --packets`
(format packets-raw-1, specs/tools/packets-trace.md).

    python3 items_diff.py ORIG.packets.jsonl D2RS.packets.jsonl [--next 20]
        [--json FILE] [--list]
    python3 items_diff.py --selftest

An item is created, for this channel, by the first S->C 0x9C / 0x9D that
carries its GUID (specs/items/inventory-moves.md §11: drops, store fill,
gamble list, cube output, quest items, the save's own items at join).
Both sides' items are listed in that order and compared item by item:
the creation frame (packets-trace.md §3 rule 1's window), the message id
and action (the creating event), the category, the owner (0x9D: type;
an item owner by its index in the list) and the item bit stream
(specs/items/bitstream.md), byte for byte. The GUID itself is the packets
channel's to compare, not this one's.

Exit codes: 0 MATCH, 1 DIVERGED, 2 PARTIAL, 3 error.

Standard library only; runs on Linux and Windows. Our own code.
"""

import argparse
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                                "trace-recorder"))
import packets_diff  # noqa: E402  (load, windows: packets-trace.md §3 rule 1)

ITEM_IDS = (0x9C, 0x9D)
STREAM_AT = {0x9C: 8, 0x9D: 13}   # bitstream.md Summary
VERDICTS = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR"}
F_COMPACT, F_EAR, F_ALT = 0x200000, 0x10000, 0x2000000


class ItemsError(Exception):
    pass


# --- the item list of one recording --------------------------------------------

def items_of(recs):
    """([item], last complete window): one entry per item GUID, at its first
    0x9C / 0x9D, in window then record order. The `s2c` stream of a window
    is the queued messages and the direct sends in record order."""
    wins, last, _first, _ = packets_diff.windows(recs)
    seen, out = set(), []
    for w in sorted(wins):
        for m in wins[w].get("s2c", []):
            b = m["bytes"]
            if len(b) < 8 or b[0] not in ITEM_IDS:
                continue
            guid = int.from_bytes(b[4:8], "little")
            if guid in seen or guid == 0xFFFFFFFF:
                continue
            seen.add(guid)
            at = STREAM_AT[b[0]]
            it = {"n": len(out), "frame": w, "id": b[0], "action": b[1], "category": b[3],
                  "guid": guid, "stream": b[at:], "seq": m["seq"], "bytes": b}
            if b[0] == 0x9D and len(b) >= 13:
                it["owner_type"] = b[8]
                it["owner_guid"] = int.from_bytes(b[9:13], "little")
            out.append(it)
    index = {it["guid"]: it["n"] for it in out}
    for it in out:
        if it.get("owner_type") == 4:
            it["owner_item"] = index.get(it["owner_guid"])
    return out, last


# --- naming bits of the stream (bitstream.md §2-§4.1) ---------------------------

def head_fields(stream):
    """[(name, first bit, bits, value)] of the stream's head: flags, version,
    mode, location, code (none for an ear). Short streams stop early."""
    total = len(stream) * 8
    val = int.from_bytes(stream, "little")
    out, pos = [], 0

    def take(name, n):
        nonlocal pos
        if pos + n > total:
            return None
        v = (val >> pos) & ((1 << n) - 1)
        out.append((name, pos, n, v))
        pos += n
        return v

    flags = take("flags", 32)
    if flags is None or take("version", 10) is None:
        return out
    mode = take("mode", 3)
    if mode is None:
        return out
    if mode in (3, 5):
        take("x", 16)
        take("y", 16)
    else:
        for name, n in (("body", 4), ("x", 4), ("y", 4), ("page", 3)):
            take(name, n)
    if not (flags & F_COMPACT and flags & F_EAR):
        take("code", 32)
    return out


def code_of(stream):
    for name, _p, _n, v in head_fields(stream):
        if name == "code":
            s = v.to_bytes(4, "little").decode("latin-1").rstrip(" \0")
            return s if s.isprintable() else f"{v:#010x}"
    return "?"


def bit_field(stream, bit):
    """The head field holding `bit`, else 'body bit K past the head'."""
    h = head_fields(stream)
    for name, p, n, _v in h:
        if p <= bit < p + n:
            return f"{name} (bit {bit - p} of {n})"
    end = h[-1][1] + h[-1][2] if h else 0
    return f"after the head (bit {bit - end} past bit {end})"


def describe(it):
    if it is None:
        return "-"
    return (f"#{it['n']} frame {it['frame']} {it['id']:#04x} action {it['action']:#04x} "
            f"{code_of(it['stream'])} guid {it['guid']} ({len(it['stream'])} stream bytes)")


# --- comparison ----------------------------------------------------------------

def compare_item(a, b):
    """The first difference of two paired items as (where, 1.14d, d2rs), or None."""
    for k in ("frame", "id", "action", "category", "owner_type"):
        if a.get(k) != b.get(k):
            return k, a.get(k), b.get(k)
    if a.get("owner_type") == 4 and a.get("owner_item") != b.get("owner_item"):
        return "owner item #", a.get("owner_item"), b.get("owner_item")
    sa, sb = a["stream"], b["stream"]
    for i in range(min(len(sa), len(sb))):
        if sa[i] != sb[i]:
            x = sa[i] ^ sb[i]
            bit = i * 8 + (x & -x).bit_length() - 1
            return (f"stream byte {i}, {bit_field(sa, bit)}",
                    sa[max(0, i - 4):i + 5].hex(), sb[max(0, i - 4):i + 5].hex())
    if len(sa) != len(sb):
        return "stream length", len(sa), len(sb)
    return None


def compare(oa, ob):
    """{items, divs, s}: paired by index within the common frame range."""
    a, last_a = oa
    b, last_b = ob
    upto = min(x for x in (last_a, last_b) if x is not None) \
        if last_a is not None and last_b is not None else None
    if upto is not None:
        a = [x for x in a if x["frame"] <= upto]
        b = [x for x in b if x["frame"] <= upto]
    divs = []
    for i in range(max(len(a), len(b))):
        x = a[i] if i < len(a) else None
        y = b[i] if i < len(b) else None
        if x is None or y is None:
            divs.append({"index": i, "where": "missing in d2rs" if y is None
                         else "missing in 1.14d", "a": x, "b": y,
                         "frame": (x or y)["frame"]})
            continue
        d = compare_item(x, y)
        if d:
            divs.append({"index": i, "where": d[0], "expected": d[1], "got": d[2], "a": x,
                         "b": y, "frame": min(x["frame"], y["frame"])})
    partial = last_a is None or last_b is None or last_a != last_b
    return {"a": a, "b": b, "divs": divs, "upto": upto, "last": (last_a, last_b),
            "partial": partial}


def verdict(res):
    if res["divs"]:
        return 1
    if res["partial"] or not res["a"]:
        return 2   # different ranges, or nothing created: nothing compared
    return 0


def summary(res, code):
    divs, n = res["divs"], max(len(res["a"]), len(res["b"]))
    first = None
    if divs:
        d = divs[0]
        text = f"item #{d['index']} ({code_of((d['a'] or d['b'])['stream'])}) {d['where']}"
        if "expected" in d:
            text += f": 1.14d {d['expected']} vs d2rs {d['got']}"
        first = {"frame": d["frame"], "text": text}
    return {"format": "diff-summary-1", "channel": "items", "tool": "items_diff.py",
            "code": code, "verdict": VERDICTS[code], "frames_compared": n,
            "frames_equal": n - len(divs), "items_orig": len(res["a"]),
            "items_d2rs": len(res["b"]), "differences": len(divs), "first": first}


def report(res, code, nxt, listing=False):
    a, b, divs = res["a"], res["b"], res["divs"]
    print(f"items: 1.14d {len(a)}, d2rs {len(b)} created up to frame {res['upto']} "
          f"(last complete frame 1.14d {res['last'][0]}, d2rs {res['last'][1]})")
    if listing:
        for i in range(max(len(a), len(b))):
            print(f"  1.14d {describe(a[i] if i < len(a) else None)}")
            print(f"  d2rs  {describe(b[i] if i < len(b) else None)}")
    for k, d in enumerate(divs[:nxt + 1]):
        head = "first divergence" if k == 0 else "next"
        line = f"{head}: item #{d['index']} {d['where']}"
        if "expected" in d:
            line += f": 1.14d {d['expected']} vs d2rs {d['got']}"
        print(line)
        print(f"  1.14d {describe(d['a'])}\n  d2rs  {describe(d['b'])}")
    if res["partial"]:
        print("partial: the two recordings end at different frames (or one has no tick)")
    if not a and not b:
        print("partial: no item created on either side")
    eq = max(len(a), len(b)) - len(divs)
    print(f"summary: {max(len(a), len(b))} items compared, {eq} equal, "
          f"{len(divs)} differences: {VERDICTS[code]}")


# --- self test -----------------------------------------------------------------

def _msg(mid, action, guid, stream, owner=None):
    head = bytes([mid, action, 0, 0]) + guid.to_bytes(4, "little")
    if mid == 0x9D:
        t, g = owner or (6, 0xFFFFFFFF)
        head += bytes([t]) + g.to_bytes(4, "little")
    m = bytearray(head + stream)
    m[2] = len(m)
    return bytes(m)


def _stream(code, mode=3, flags=0x800010, rest=b"\x12\x34"):
    v = flags | (101 << 32) | (mode << 42)
    pos = 45
    if mode in (3, 5):
        v |= (4880 << pos) | (4223 << (pos + 16))
        pos += 32
    else:
        pos += 15
    v |= int.from_bytes(code.encode().ljust(4), "little") << pos
    pos += 32
    n = (pos + 7) // 8
    return v.to_bytes(n, "little") + rest


def _recording(msgs, ticks=5):
    """packets-raw-1 records: msgs = [(frame, bytes)] queued in that tick."""
    out, seq = [], 0
    for f in range(1, ticks + 1):
        out.append({"type": "tick", "frame": f, "phase": "tick", "seq": seq})
        seq += 1
        for mf, m in msgs:
            if mf == f:
                out.append({"type": "s2c", "frame": f, "phase": "tick", "client": 1,
                            "size": len(m), "bytes": m.hex(), "seq": seq})
                seq += 1
        out.append({"type": "tick_end", "frame": f, "phase": "post", "seq": seq})
        seq += 1
    return out


def selftest():
    ok = 0
    hp = _stream("hp1", flags=0xA00010)
    cap = _stream("cap", rest=b"\xaa\xbb\xcc")
    jew = _stream("jew", mode=6, rest=b"\x01")
    base = [(2, _msg(0x9C, 0x00, 21, hp)), (3, _msg(0x9C, 0x00, 22, cap)),
            (3, _msg(0x9D, 0x13, 23, jew, (4, 22))),
            (4, _msg(0x9C, 0x01, 21, hp))]        # later message of item 21: not a creation
    a = items_of(_recording(base))
    assert [x["n"] for x in a[0]] == [0, 1, 2] and a[1] == 5, a
    assert code_of(a[0][1]["stream"]) == "cap" and a[0][2]["owner_item"] == 1, a
    ok += 1
    # GUIDs differ (party offset) but nothing else: match
    shifted = [(f, m[:4] + (int.from_bytes(m[4:8], "little") + 2).to_bytes(4, "little")
                + m[8:9] + ((int.from_bytes(m[9:13], "little") + 2).to_bytes(4, "little")
                            if m[0] == 0x9D else m[9:13]) + m[13:]) for f, m in base]
    res = compare(a, items_of(_recording(shifted)))
    assert verdict(res) == 0 and not res["divs"], res["divs"]
    ok += 1
    # each stream byte perturbed is found at its item and byte, named by field
    for k in range(len(cap)):
        bad = list(base)
        s = bytearray(cap)
        s[k] ^= 0x10
        bad[1] = (3, _msg(0x9C, 0x00, 22, bytes(s)))
        res = compare(a, items_of(_recording(bad)))
        d = res["divs"][0]
        assert verdict(res) == 1 and d["index"] == 1 and d["where"].startswith(
            f"stream byte {k},"), (k, d["where"])
    d = compare(a, items_of(_recording([base[0], (3, _msg(0x9C, 0x00, 22, _stream("cav"))),
                                         base[2]])))["divs"][0]
    assert "code" in d["where"], d
    ok += 1
    # the creating event, the frame, the owner and a missing item are reported
    for bad, where in (([base[0], (3, _msg(0x9C, 0x0B, 22, cap)), base[2]], "action"),
                       ([base[0], (2, _msg(0x9C, 0x00, 22, cap)), base[2]], "frame"),
                       ([base[0], base[1], (3, _msg(0x9D, 0x13, 23, jew, (4, 21)))],
                        "owner item #"),
                       ([base[0], base[1]], "missing in d2rs")):
        res = compare(a, items_of(_recording(bad)))
        assert res["divs"] and res["divs"][0]["where"] == where, (where, res["divs"])
        ok += 1
    # fewer ticks on one side: partial, items after the shorter range not compared
    res = compare(a, items_of(_recording(base, ticks=2)))
    assert verdict(res) == 2 and len(res["a"]) == 1, res
    # nothing created on either side: partial
    assert verdict(compare(items_of(_recording([])), items_of(_recording([])))) == 2
    sm = summary(compare(a, a), 0)
    assert (sm["frames_compared"], sm["frames_equal"], sm["first"]) == (3, 3, None), sm
    ok += 3
    print(f"items_diff selftest: {ok} checks passed")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("orig", nargs="?")
    ap.add_argument("d2rs", nargs="?")
    ap.add_argument("--next", type=int, default=20)
    ap.add_argument("--json", default=None, metavar="FILE")
    ap.add_argument("--list", action="store_true", help="print both item lists")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not (a.orig and a.d2rs):
        ap.error("two packets-raw-1 files are needed")
    try:
        oa = items_of(packets_diff.load(a.orig)[1])
        ob = items_of(packets_diff.load(a.d2rs)[1])
    except (packets_diff.PacketsError, OSError, ValueError) as e:
        print(f"error: {e}", file=sys.stderr)
        if a.json:
            packets_diff.write_json(a.json, {"format": "diff-summary-1", "channel": "items",
                                             "code": 3, "verdict": "ERROR",
                                             "frames_compared": 0, "frames_equal": 0,
                                             "differences": 0, "first": None})
        return 3
    res = compare(oa, ob)
    code = verdict(res)
    report(res, code, a.next, a.list)
    if a.json:
        packets_diff.write_json(a.json, summary(res, code))
    return code


if __name__ == "__main__":
    sys.exit(main())
