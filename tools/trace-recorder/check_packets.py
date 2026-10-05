"""Check a packets-raw-1 recording against specs/sim/intents-events.md.

Reads the two message tables of the spec (specs/sim/client-messages.tsv,
server-messages.tsv) and checks every recorded event against the rules of
the spec. Exit 1 on any failure; every failure names the event's seq.

  py tools/trace-recorder/check_packets.py traces/raw/<time>-packets.jsonl
  py tools/trace-recorder/check_packets.py <file> --perturb N   # M08: must fail at seq N
  py tools/trace-recorder/check_packets.py --selftest           # synthetic trace + perturbations

Rules checked (spec section in brackets):
  R1 [§2.1] every client->server message taken from a server queue has the
     size its id's size rule gives, and was queued (size rule not 0);
  R2 [§2.1] the server takes client messages in the order the client sent
     them, byte for byte (game queue and system queue separately);
  R3 [§2.3] every dispatched message follows its queue read; ids 1..0x66 only;
     the result code matches the spec where it is fixed (stubs, wrong size);
  R4 [§1] no client message is processed inside a tick; tick frames advance
     by exactly 1;
  R5 [§3.1] every queued server->client message has its id's size;
  R6 [§3.2] the buffers flushed to a client are the queued messages, in
     order, packed by the 0x200-byte buffer rule;
  R7 [§1] in single player a flush only follows a tick.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import csv
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SPEC_DIR = os.path.normpath(os.path.join(HERE, "..", "..", "specs", "sim"))
BUFFER_MAX = 0x200      # per-client message buffer (spec §3.2)
FLUSH_SITE = 0x52E3B5   # the flush's call of the net send (spec §3.2)
DRAIN_COPY = 0x1FC      # bytes of a client message the drain buffer holds (spec §2.1)


def load_tsv(name):
    with open(os.path.join(SPEC_DIR, name), newline="", encoding="utf-8") as f:
        return {int(r["id"], 16): r for r in csv.DictReader(f, delimiter="\t")}


def _cstrlen(b, off):
    end = b.find(b"\0", off)
    return None if end < 0 else end - off


def size_of(rule, b):
    """Size of the message starting at b[0] under a size rule of the tables,
    or None when the bytes are too short to tell (spec §2.1, §3.1)."""
    if rule.lstrip("-").isdigit():
        return int(rule)
    if rule == "chat":  # client 0x14 / 0x15
        if len(b) < 3:
            return None
        l1 = _cstrlen(b, 3)
        if l1 is None or len(b) < l1 + 4:
            return None
        l2 = _cstrlen(b, l1 + 4)
        if l2 is None:
            return None
        k = l1 + 4 + l2 + 1
        if k >= len(b):
            return None
        extra = b[k] - 256 if b[k] >= 128 else b[k]
        return k + extra + 1
    if rule == "chat26":  # server 0x26
        if len(b) < 10:
            return None
        l1 = _cstrlen(b, 10)
        if l1 is None:
            return None
        l2 = _cstrlen(b, 11 + l1)
        if l2 is None:
            return None
        return 10 + (l1 + 1) + (l2 + 1)
    if rule == "af":  # server 0xAF
        if len(b) < 2:
            return None
        return 2 if b[1] == 0 else b[1] + 1
    # <u8|u16>@<off>[*mul][+add][;cap=N][;min=N]
    parts = rule.split(";")
    opts = dict(p.split("=") for p in parts[1:])
    main = parts[0]
    add = 0
    if "+" in main:
        main, a = main.split("+")
        add = int(a, 0)
    mul = 1
    if "*" in main:
        main, m = main.split("*")
        mul = int(m, 0)
    width, off = main.split("@")
    off = int(off, 0)
    need = max(int(opts.get("min", "0"), 0), off + (1 if width == "u8" else 2))
    if len(b) < need:
        return None
    v = b[off] if width == "u8" else b[off] | (b[off + 1] << 8)
    if "cap" in opts and v > int(opts["cap"], 0):
        v = 0
    return v * mul + add


class Checker:
    def __init__(self):
        self.cs = load_tsv("client-messages.tsv")
        self.sc = load_tsv("server-messages.tsv")
        self.fail = []

    def bad(self, ev, msg, *also):
        seqs = {ev.get("seq")} | {s for s in also if s is not None}
        self.fail.append((seqs, f"seq {ev.get('seq')} {ev['type']}: {msg}"))

    def check(self, events):
        sent = {"game": [], "sys": []}       # client_out not yet taken by the server
        bufs = {}                            # client -> [[bytearray, [(seq, start, len)]]]
        in_tick = False
        last_frame = {}
        ticked_since_flush = False
        last_c2s = None
        last_dispatch = None
        for ev in events:
            t = ev["type"]
            b = bytes.fromhex(ev.get("bytes", ""))
            if t == "client_out":
                if not b:
                    self.bad(ev, "empty message")
                    continue
                q = "sys" if 0x67 <= b[0] <= 0x70 else ("game" if b[0] < 0x67 else None)
                if q:
                    sent[q].append(ev)
            elif t in ("c2s", "c2s_sys"):
                if in_tick:
                    self.bad(ev, "client message processed inside a tick (R4)")
                q = "sys" if t == "c2s_sys" else "game"
                if not b:
                    self.bad(ev, "empty message")
                    continue
                row = self.cs.get(b[0])
                if row is None or row["transport_size"] == "0":
                    self.bad(ev, f"id {b[0]:#04x} has no size rule; it is never queued (R1)")
                elif ev["size"] <= DRAIN_COPY:
                    n = size_of(row["transport_size"], b)
                    if n != ev["size"]:
                        self.bad(ev, f"id {b[0]:#04x} size {ev['size']} != rule {n} (R1)")
                if sent[q]:
                    src = sent[q].pop(0)
                    if bytes.fromhex(src["bytes"])[:len(b)] != b or src["size"] != ev["size"]:
                        self.bad(ev, f"differs from the message the client sent at seq {src.get('seq')} (R2)",
                                 src.get("seq"))
                elif any(e["type"] == "client_out" for e in events):
                    self.bad(ev, "no matching client message (R2)")
                last_c2s = ev if t == "c2s" else None
            elif t == "dispatch":
                if in_tick:
                    self.bad(ev, "dispatched inside a tick (R4)")
                if last_c2s is None or bytes.fromhex(last_c2s["bytes"])[:1] != bytes([ev["id"]]) \
                        or last_c2s["size"] != ev["size"]:
                    self.bad(ev, "does not follow its queue read (R3)")
                if not 1 <= ev["id"] <= 0x66:
                    self.bad(ev, f"id {ev['id']:#04x} must have been rejected (R3)")
                last_dispatch = ev
                last_c2s = None
            elif t == "result":
                if not ev.get("dispatched"):
                    continue
                d = last_dispatch
                row = self.cs.get(d["id"]) if d else None
                if row:
                    want = None
                    if row["kind"] == "stub0":
                        want = 0
                    elif row["kind"] == "stub3":
                        want = 3
                    elif row["handler_size"].startswith("==") and d["size"] != int(row["handler_size"][2:]):
                        want = 3
                    if want is not None and ev.get("code") != want:
                        self.bad(ev, f"result {ev.get('code')} for id {d['id']:#04x}, spec says {want} (R3)")
                last_dispatch = None
            elif t == "tick":
                in_tick = True
                g = ev.get("game")
                if g in last_frame and ev["frame"] != last_frame[g] + 1:
                    self.bad(ev, f"frame {ev['frame']} after {last_frame[g]} (R4)")
                last_frame[g] = ev["frame"]
            elif t == "tick_end":
                in_tick = False
                ticked_since_flush = True
            elif t == "flush":
                if not ticked_since_flush:
                    self.bad(ev, "flush without a tick since the last flush (R7)")
                ticked_since_flush = False
            elif t == "s2c":
                if not b:
                    self.bad(ev, "empty message")
                    continue
                row = self.sc.get(b[0])
                n = size_of(row["size"], b) if row else None
                if row is None or row["size"] == "0" or n != ev["size"]:
                    self.bad(ev, f"id {b[0]:#04x} size {ev['size']} != rule {n} (R5)")
                cb = bufs.setdefault(ev["client"], [])
                if not cb or len(cb[-1][0]) + ev["size"] > BUFFER_MAX:
                    cb.append([bytearray(), []])
                cb[-1][1].append((ev.get("seq"), len(cb[-1][0]), ev["size"]))
                cb[-1][0] += b
            elif t == "net" and int(ev.get("caller", "0"), 16) == FLUSH_SITE:
                cb = bufs.get(ev["client"])
                if not cb:
                    self.bad(ev, "flushed a buffer nothing was queued for (R6)")
                    continue
                want, parts = cb.pop(0)
                if bytes(want) != b:
                    where = next((i for i in range(min(len(want), len(b))) if want[i] != b[i]),
                                 min(len(want), len(b)))
                    culprit = next((s for s, st, ln in parts if st <= where < st + ln), None)
                    self.bad(ev, f"flushed buffer differs from the queued messages at byte {where}"
                                 f" (queued message seq {culprit}) (R6)", culprit)
        return self.fail


def read_events(path):
    out = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            rec = json.loads(line)
            if rec["type"] not in ("header", "footer"):
                out.append(rec)
    return out


def perturb(events, n):
    """Flip one byte of the message event with seq n; returns that event."""
    ev = next((e for e in events if e.get("seq") == n and e.get("bytes")), None)
    if ev is None:
        raise SystemExit(f"--perturb {n}: no message event with that seq")
    b = bytearray.fromhex(ev["bytes"])
    i = len(b) - 1
    b[i] ^= 0x5A
    ev["bytes"] = b.hex()
    return ev


def synthetic(seed=1):
    """A consistent trace built from the spec's rules (no game needed)."""
    rnd = random.Random(seed)
    cs = load_tsv("client-messages.tsv")
    sc = load_tsv("server-messages.tsv")
    c_ids = [i for i, r in cs.items() if r["kind"] == "handler" and r["transport_size"].isdigit()
             and r["handler_size"].startswith("==") and i < 0x67]
    s_ids = [i for i, r in sc.items() if r["size"].isdigit() and r["size"] != "0" and i < 0xAF]
    ev, seq = [], 0

    def add(**r):
        nonlocal seq
        r["seq"] = seq
        seq += 1
        ev.append(r)

    queued = []
    frame = 100
    for _ in range(6):
        msgs = []
        for _ in range(rnd.randint(1, 4)):
            i = rnd.choice(c_ids)
            n = int(cs[i]["transport_size"])
            m = bytes([i]) + bytes(rnd.randrange(256) for _ in range(n - 1))
            msgs.append(m)
            add(type="client_out", size=n, bytes=m.hex())
        add(type="drain", frame=frame)
        for m in msgs:
            add(type="c2s", client=1, size=len(m), bytes=m.hex(), frame=frame)
            add(type="dispatch", id=m[0], size=len(m), game_frame=frame, unit=1, frame=frame)
            add(type="result", dispatched=True, code=0, frame=frame)
        frame += 1
        add(type="tick", game="0x1000", frame=frame)
        for _ in range(rnd.randint(1, 60)):
            i = rnd.choice(s_ids)
            n = int(sc[i]["size"])
            m = bytes([i]) + bytes(rnd.randrange(256) for _ in range(n - 1))
            queued.append(m)
            add(type="s2c", client=1, size=n, bytes=m.hex(), caller="0x53b330", frame=frame)
        add(type="tick_end", frame=frame)
        add(type="flush", frame=frame)
        buf = []
        for m in queued:
            if not buf or len(buf[-1]) + len(m) > BUFFER_MAX:
                buf.append(bytearray())
            buf[-1] += m
        for x in buf:
            add(type="net", kind=1, client=1, size=len(x), bytes=bytes(x).hex(),
                caller=f"{FLUSH_SITE:#x}", frame=frame)
        queued = []
    return ev


def selftest():
    events = synthetic()
    fails = Checker().check(events)
    if fails:
        print("selftest: the synthetic trace fails:", *[m for _, m in fails[:5]], sep="\n  ")
        return 1
    msg_seqs = [e["seq"] for e in events if e.get("bytes")]
    missed = []
    for n in msg_seqs:
        evs = json.loads(json.dumps(events))
        perturb(evs, n)
        fails = Checker().check(evs)
        if not any(n in seqs for seqs, _ in fails):
            missed.append(n)
    print(f"selftest: clean synthetic trace passes ({len(events)} events); "
          f"{len(msg_seqs) - len(missed)}/{len(msg_seqs)} single-byte perturbations reported at their seq")
    if missed:
        print("selftest: perturbations not reported:", missed[:20])
        return 1
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("file", nargs="?", help="traces/raw/<time>-packets.jsonl")
    ap.add_argument("--perturb", type=int, default=None,
                    help="flip one byte of the message event with this seq first (must fail)")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(selftest())
    if not a.file:
        ap.error("a recording or --selftest is required")
    events = read_events(a.file)
    if a.perturb is not None:
        perturb(events, a.perturb)
    fails = Checker().check(events)
    if not any(e["type"] in ("c2s", "s2c") for e in events):
        fails.append((set(), "no client or server message recorded (was a game entered?)"))
    kinds = {}
    for e in events:
        kinds[e["type"]] = kinds.get(e["type"], 0) + 1
    print("events: " + "  ".join(f"{k}={v}" for k, v in sorted(kinds.items())))
    for _, m in fails[:50]:
        print("FAIL", m)
    if len(fails) > 50:
        print(f"... {len(fails) - 50} more")
    if a.perturb is not None:
        hit = any(a.perturb in seqs for seqs, _ in fails)
        print(f"perturbation at seq {a.perturb}: {'reported' if hit else 'NOT reported'}")
        sys.exit(0 if hit else 1)
    print("OK" if not fails else f"{len(fails)} failures")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
