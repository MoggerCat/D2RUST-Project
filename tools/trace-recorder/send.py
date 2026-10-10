"""Send: scripted client->server messages injected into the original 1.14d Game.exe.

The 1.14d side of the `at <frame> send ...` lines of a check file
(specs/tools/scenario-diff.md §2) and of the scenario steps
(specs/tools/scenario.md §3, §4 rule 2 (a)), by the procedure of
specs/tools/original-hooks.md §1 rule 4: the server thread is stopped at
the drain call of the single-player frame 0x0044F136 (`call 0x0052CFE0`,
bytes E8 A5 DE 0D 00); at the first such stop after tick f - 1 returned
(the tick-return stop 0x0052FD1E that poke.py / autostart.py share, ESI =
game, game +0xA8 = the frame just run), each message of frame f is written
to a scratch page, the transport send 0x0052AE50 is called on the stopped
thread (stdcall: [ESP+4] size, [ESP+8] channel 1, [ESP+0xC] message; `ret
0xC`), its return is trapped (INT3 at the scratch page) and EAX checked (1
= queued, 0 = not queued), then the saved context is restored and the
drain runs as usual: the message takes the real path after the client's
duplicate filter (classifier, queue 1 in FIFO order after what the client
queued, the drain, the dispatcher).

Before the first injection (§1 rule 5): game type global 0x007A0610 = 0,
local mode 0x00882D10 = 1, connected flag 0x00882B34 != 0, and client 0 in
state 4 (game +0x88 -> client +0x04). A message due before that is written
as unresolved (`@player`), as d2rs writes it.

Message syntax (`--send "<f> <Name> <field>=<value>..."` or `--send "<f>
hex <byte>..."`): the scenario script's typed messages (scenario.md §3
rules 1-2) from specs/sim/client-messages.tsv: every layout field of a
fixed-size row given once, little-endian at its offset, `uN` / `bitN`
OR-ed into the u32 at the offset (bitN at bit N), uncovered bytes 0.
Values: numbers (decimal or 0x hex) or references (§3 rule 3) resolved on
the live unit lists at the injection (poke.py's readers: the hash lists of
original-hooks.md §4 rule 1, the path position of original-hooks-spawn.md
§2 rule 3): @player, @x, @y, @x+N, @y-N, @<type>[:<class>][#n], and
`@wp[#n]` = the GUID of the n-th object of the 16 waypoint classes (the
same list poke.py's `@wp` uses).

Each result is a record {"k": "send", "f", "frame", "i", "r", "bytes",
"eax", "note", "src"} in the recorder's output (r: ok = EAX 1, dropped =
EAX 0, unresolved, gap, failed), the fields d2rs `state-dump --send` writes.

Used by record_state.py, record_packets.py and record_frames.py through
add_options() / SendLayer.from_args() / attach(). `--selftest` checks the
encoder against scenario.md's test vectors and the call layout on a fake
context (no game, any OS).

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import os
import re
import struct
import sys

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import poke  # noqa: E402  (unit-list readers, the call procedure, the tick-return stop)

TOOL = "trace-recorder send 0.1.0"
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
TSV = os.path.join(REPO, "specs", "sim", "client-messages.tsv")

DRAIN_CALL = 0x0044F136          # the single-player frame's drain call (original-hooks.md §1 rule 3, §3)
DRAIN_CALL_BYTES = bytes.fromhex("E8A5DE0D00")  # call 0x0052CFE0 (§1 rule 4)
TRANSPORT_SEND = 0x0052AE50      # stdcall size, channel, message; ret 0xC (§1 table)
TRANSPORT_SEND_BYTES = bytes.fromhex("558BEC")  # entry bytes, as record_packets.py `client_out`
CHANNEL = 1                      # callers pass 1 or 0; not read on the local path (§1 table)
GAME_TYPE = 0x007A0610           # 0 = the single-player branch at 0x0044F132 (§1 rule 5)
LOCAL_MODE = 0x00882D10          # 1 (set by the single-player connect 0x0052A750, §1 rule 5)
CONNECTED = 0x00882B34           # != 0 (§1 rule 5)
SCRATCH_MSG = 0x10               # S+0x10: the message bytes (§1 rule 4 step 3)
MAX_SIZE = 0x204                 # larger is a fatal assert in the transport send (§1 table)
U32 = 0xFFFFFFFF


class SendError(ValueError):
    pass


# --- the message table (specs/sim/client-messages.tsv) -----------------------

FIELD_RE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*):([a-z0-9]+)(?:@(\d+))?\Z")


def field_type(t):
    """(kind, n) of a layout type: ('u', bits) for u8/u16/u32/uN, ('bit', N)
    for bitN; None for any other (cstr, data, ...: not typed)."""
    m = re.match(r"(u|bit)(\d+)\Z", t)
    if not m:
        return None
    n = int(m.group(2))
    if m.group(1) == "bit":
        return ("bit", n) if n < 32 else None
    return ("u", n) if 1 <= n <= 32 else None


def field_max(ft):
    kind, n = ft
    return 1 if kind == "bit" else (1 << n) - 1


class Message:
    def __init__(self, mid, name, size, fields, why=None):
        self.id, self.name, self.size, self.fields, self.why = mid, name, size, fields, why


def load_table(path=TSV):
    """{name: Message} of every named row. A row that cannot be typed
    (scenario.md §3 rule 1) keeps `why`."""
    out = {}
    with open(path, encoding="utf-8") as f:
        head = f.readline().rstrip("\n").split("\t")
        ix = {k: i for i, k in enumerate(head)}
        for line in f:
            c = line.rstrip("\n").split("\t")
            if len(c) < len(head) or c[ix["name"]] == "-":
                continue
            name, size, layout = c[ix["name"]], c[ix["transport_size"]], c[ix["layout"]]
            mid = int(c[ix["id"]], 16)
            why, fields = None, []
            if not size.isdigit():
                why = f"{name} has no fixed size: write it as hex"
            elif int(size) == 0:
                why = f"{name} is never a valid message"
            for tok in layout.split():
                m = FIELD_RE.match(tok)
                ft = field_type(m.group(2)) if m else None
                if not m or ft is None or m.group(3) is None:
                    why = why or f"{name} field {tok.split(':')[0]} cannot be typed: write the message as hex"
                    continue
                fields.append((m.group(1), ft, int(m.group(3))))
            out[name] = Message(mid, name, int(size) if size.isdigit() else 0, fields, why)
    return out


_TABLE = None


def table():
    global _TABLE
    if _TABLE is None:
        _TABLE = load_table()
    return _TABLE


# --- values and references (scenario.md §2 rule 4, §3 rule 3) ---------------

def num(t):
    if re.fullmatch(r"0x[0-9a-fA-F]+", t):
        v = int(t, 16)
    elif re.fullmatch(r"[0-9]+", t):
        v = int(t)
    else:
        raise SendError(f"bad number {t!r}")
    if v > U32:
        raise SendError(f"number {t!r} does not fit 32 bits")
    return v


def signed(t):
    neg = t.startswith("-")
    v = num(t[1:] if neg else t)
    v = -v if neg else v
    if not -0x80000000 <= v <= 0x7FFFFFFF:
        raise SendError(f"number {t!r} does not fit i32")
    return v


class Ref:
    """kind: player | x | y | unit | wp; off for x/y; ty, cls, n for unit / wp."""

    def __init__(self, kind, off=0, ty=None, cls=None, n=0):
        self.kind, self.off, self.ty, self.cls, self.n = kind, off, ty, cls, n

    def __eq__(self, o):
        return isinstance(o, Ref) and vars(self) == vars(o)

    def text(self):
        nth = f"#{self.n}" if self.n else ""
        if self.kind == "player":
            return "@player"
        if self.kind in ("x", "y"):
            return f"@{self.kind}" + ("" if not self.off else f"{self.off:+d}")
        if self.kind == "wp":
            return "@wp" + nth
        return f"@{self.ty}" + ("" if self.cls is None else f":{self.cls}") + nth


def parse_ref(t):
    """The reference grammar of conformance::scenario::script (strict as it is)."""
    body = t[1:]
    if body == "player":
        return Ref("player")
    for axis in ("x", "y"):
        if body.startswith(axis):
            rest = body[1:]
            if not rest:
                return Ref(axis)
            if rest[0] == "+" and not rest[1:].startswith("-"):
                d = signed(rest[1:])
            elif rest[0] == "-" and len(rest) > 1:
                d = signed(rest)
            else:
                raise SendError(f"bad reference {t!r}")
            if d == 0:
                raise SendError(f"{t!r}: write a zero offset as @{axis}")
            return Ref(axis, off=d)
    head, _, n = body.partition("#")
    if _:
        n = num(n)
        if n == 0:
            raise SendError(f"{t!r}: write #0 as nothing")
    else:
        n = 0
    if head == "wp":
        return Ref("wp", n=n)
    ty, _, cls = head.partition(":")
    if not re.fullmatch(r"[0-5]", ty):
        raise SendError(f"bad reference {t!r}")
    return Ref("unit", ty=int(ty), cls=num(cls) if _ else None, n=n)


# --- messages ------------------------------------------------------------------

class Msg:
    """A message of a send step: raw bytes (hex) or a typed row with its
    fields in layout order [(name, ftype, offset, value)], value an int or a Ref."""

    def __init__(self, raw=None, row=None, values=None):
        self.raw, self.row, self.values = raw, row, values

    def text(self):
        if self.raw is not None:
            return "hex " + " ".join(f"{b:02x}" for b in self.raw)
        return " ".join([self.row.name] + [
            f"{n}={v.text() if isinstance(v, Ref) else v}" for n, _, _, v in self.values])

    def refs(self):
        return [] if self.raw is not None else [v for *_, v in self.values if isinstance(v, Ref)]


def parse_message(toks, tbl=None):
    """`hex <byte>...` or `<Name> <field>=<value>...` (scenario.md §2-§3,
    the errors of conformance::scenario::script::parse_message)."""
    if not toks:
        raise SendError("a message: `hex <byte>...` or `<Name> <field>=<value>...`")
    if toks[0] == "hex":
        b = []
        for t in toks[1:]:
            if not re.fullmatch(r"[0-9a-fA-F]{2}", t):
                raise SendError(f"hex byte {t!r}: two hex digits")
            b.append(int(t, 16))
        if not 1 <= len(b) <= MAX_SIZE:
            raise SendError(f"hex message of {len(b)} bytes: 1..={MAX_SIZE}")
        return Msg(raw=bytes(b))
    tbl = tbl or table()
    name = toks[0]
    row = tbl.get(name)
    if row is None:
        raise SendError(f"unknown message {name!r}")
    if row.why:
        raise SendError(row.why)
    given = {}
    for a in toks[1:]:
        n, eq, v = a.partition("=")
        if not eq:
            raise SendError(f"{a!r}: <field>=<value>")
        f = next((f for f in row.fields if f[0] == n), None)
        if f is None:
            raise SendError(f"{name} has no field {n!r}")
        if n in given:
            raise SendError(f"field {n} given twice")
        if v.startswith("@"):
            given[n] = parse_ref(v)
        else:
            x = num(v)
            if x > field_max(f[1]):
                raise SendError(f"{n}={x} does not fit the field")
            given[n] = x
    values = []
    for n, ft, off in row.fields:
        if n not in given:
            raise SendError(f"{name} needs field {n}")
        values.append((n, ft, off, given[n]))
    return Msg(row=row, values=values)


class Unresolved(Exception):
    def __init__(self, ref, why):
        super().__init__(why)
        self.ref, self.why = ref, why


class Gap(Exception):
    pass


def encode(msg, resolve=None):
    """The bytes of `msg` (scenario.md §3 rule 2); `resolve(Ref) -> int`
    for references (raises Unresolved / Gap)."""
    if msg.raw is not None:
        return bytes(msg.raw)
    out = bytearray(msg.row.size)
    out[0] = msg.row.id
    for n, ft, off, v in msg.values:
        if isinstance(v, Ref):
            x = resolve(v)
            if not 0 <= x <= field_max(ft):
                raise Unresolved(v.text(), f"value {x} does not fit field {n}")
            v = x
        kind, bits = ft
        if kind == "u" and bits in (8, 16, 32):
            out[off:off + bits // 8] = v.to_bytes(bits // 8, "little")
        else:  # uN at bit 0, bitN at bit N: OR-ed into the u32 at the offset
            shift = bits if kind == "bit" else 0
            old = int.from_bytes(out[off:off + 4], "little")
            out[off:off + 4] = ((old | (v << shift)) & U32).to_bytes(4, "little")
    return bytes(out)


def resolver(mem, game):
    """resolve(Ref) on the live game (scenario.md §3 rules 3-4): the unit
    lists and the player's path position, read through poke.py's readers."""
    def resolve(r):
        if r.kind == "wp":
            # the n-th object of the 16 waypoint classes, as poke.py resolves `@wp` (poke.md §1):
            # the 0x49 field is that object's GUID, so no operate address is needed
            us = [g for g, c, _ in poke.units_of(mem, game, poke.OBJECT_TYPE) if c in poke.WP_CLASSES]
            if r.n >= len(us):
                raise Unresolved(r.text(), f"{r.text()}: no waypoint object")
            return us[r.n]
        if r.kind in ("player", "x", "y"):
            pl = poke.player_of(mem, game)
            if not pl:
                raise Unresolved(r.text(), f"{r.text()}: no player")
            if r.kind == "player":
                return mem.read_u32(pl + poke.U_GUID)
            p = poke.player_pos(mem, pl)
            if not p:
                raise Unresolved(r.text(), f"{r.text()}: no player position")
            return (p[0] if r.kind == "x" else p[1]) + r.off
        us = [g for g, c, _ in poke.units_of(mem, game, r.ty) if r.cls is None or c == r.cls]
        if r.n >= len(us):
            raise Unresolved(r.text(), f"{r.text()}: no such unit")
        return us[r.n]
    return resolve


# --- steps ----------------------------------------------------------------------

class Step:
    def __init__(self, f, msg, index=0):
        self.f, self.msg, self.index = f, msg, index

    def text(self):
        return self.msg.text()


def parse_send_option(s, index=0, tbl=None):
    """One --send value: `<f> <Name> <field>=<value>...` or `<f> hex <byte>...`."""
    toks = s.split()
    if not toks:
        raise SendError('--send "<frame> <Name> <field>=<value>..." or "<frame> hex <byte>..."')
    if not re.fullmatch(r"[0-9]+", toks[0]) or int(toks[0]) < 1:
        raise SendError(f"--send: frame {toks[0]!r}: a server frame >= 1")
    try:
        return Step(int(toks[0]), parse_message(toks[1:], tbl), index)
    except SendError as e:
        raise SendError(f"--send {s!r}: {e}")


def call_layout(saved_esp, scratch, size):
    """(esp, stack bytes at esp, eip) of one transport-send call (original-hooks.md
    §1 rule 4 step 4): [ESP] = S (return trap), [ESP+4] = size,
    [ESP+8] = channel 1, [ESP+0xC] = S+0x10; ESP = saved - 16."""
    esp = saved_esp - 16
    stack = struct.pack("<4I", scratch, size, CHANNEL, scratch + SCRATCH_MSG)
    return esp, stack, TRANSPORT_SEND


# --- the layer --------------------------------------------------------------------

class SendLayer:
    """Holds the --send steps; injects each at the first 0x0044F136 stop
    after tick f - 1 returned."""

    def __init__(self, steps):
        self.steps = list(steps)
        self.done = set()
        self.results = []
        self.game = None
        self.frame = None           # last tick-return frame (game +0xA8)
        self.last_frame = None      # stop at which the last step ran
        self.caller = poke.PokeLayer()  # scratch page and call procedure (original-hooks-spawn.md §5)
        self.checked = False

    @classmethod
    def from_args(cls, a):
        vals = getattr(a, "send", None) or []
        if not vals:
            return None
        try:
            return cls([parse_send_option(s, i) for i, s in enumerate(vals)])
        except SendError as e:
            raise SystemExit(str(e))

    def pending(self):
        return [s for s in self.steps if id(s) not in self.done]

    def header(self):
        return {"k": "send_steps", "tool": TOOL,
                "steps": [{"f": s.f, "src": s.text()} for s in self.steps]}

    # --- attaching -------------------------------------------------------
    def attach(self, rec):
        """For a record_tick.TickRecorder-based recorder: arm 0x0052FD1E
        (frame tracking; shared) and 0x0044F136 (the injection stop) and
        route their stops here. Call before rec.run()."""
        rt = sys.modules.get("record_tick")
        if rt is None:
            import record_tick as rt
        rt.EXPECT.setdefault(poke.TICK_RET, poke.TICK_RET_BYTES)
        rt.EXPECT.setdefault(DRAIN_CALL, DRAIN_CALL_BYTES)
        orig_bp, orig_handle = rec.on_breakpoint, rec.handle
        layer = self

        def on_breakpoint(tid, addr):
            rec.send_tid = tid
            return orig_bp(tid, addr)

        def handle(addr, ctx):
            r = orig_handle(addr, ctx)
            if addr == poke.TICK_RET:
                layer.on_tick_return(rec, ctx.Esi)
            elif addr == DRAIN_CALL:
                layer.on_drain_call(rec, rec.send_tid)
            return r

        rec.on_breakpoint, rec.handle = on_breakpoint, handle

    def on_tick_return(self, mem, game):
        if self.game in (None, game):
            self.game = game
            self.frame = struct.unpack("<i", mem.read(game + poke.G_FRAME, 4))[0]

    # --- the injection stop --------------------------------------------------
    def due(self):
        if self.frame is None:
            return []
        return [s for s in self.steps if id(s) not in self.done and s.f - 1 <= self.frame]

    def on_drain_call(self, rec, tid):
        due = self.due()
        if not due:
            return []
        rr = poke._rr()
        saved = rr.WOW64_CONTEXT()
        saved.ContextFlags = 0x1003F  # WOW64_CONTEXT_ALL
        if not rr.Wow64GetThreadContext(rec.threads[tid], rr.C.byref(saved)):
            raise rr.winerr("Wow64GetThreadContext")
        saved.Eip = DRAIN_CALL  # rewound (§1 rule 4 step 2)
        try:
            return self.inject(rec, tid, saved, due)
        finally:
            # restore the saved context exactly (step 6); the recorder then
            # steps over the original call as for any hook
            if not rr.Wow64SetThreadContext(rec.threads[tid], rr.C.byref(saved)):
                raise rr.winerr("Wow64SetThreadContext")

    def gate(self, mem):
        """Why nothing may be injected yet (§1 rule 5), or None."""
        if mem.read_u32(GAME_TYPE) != 0:
            return "failed", "game type 0x007A0610 != 0 (not the single-player branch)"
        if mem.read_u32(LOCAL_MODE) != 1 or mem.read_u32(CONNECTED) == 0:
            return "failed", "local mode 0x00882D10 != 1 or connected 0x00882B34 = 0"
        c = mem.read_u32(self.game + poke.G_CLIENTS)
        if not c or mem.read_u32(c + poke.C_STATE) != poke.CLIENT_IN_GAME:
            return "unresolved", "@player: client 0 not in state 4 yet (original-hooks.md §1 rule 5)"
        return None

    def inject(self, rec, tid, saved, steps):
        """Every step now, in order (the caller holds the saved context)."""
        out = []
        why = self.gate(rec)
        resolve = resolver(rec, self.game)
        for i, s in enumerate(steps):
            self.done.add(id(s))
            r = {"k": "send", "f": s.f, "frame": self.frame, "i": i}
            if s.f - 1 < self.frame:
                r["late"] = True
            try:
                if why:
                    r.update(r=why[0], note=why[1])
                else:
                    b = encode(s.msg, resolve)
                    eax = self.send(rec, tid, saved, b)
                    r.update(r="ok" if eax == 1 else "dropped", bytes=b.hex(), eax=eax)
                    if eax != 1:
                        r["note"] = f"transport send returned {eax} (not queued)"
            except Unresolved as e:
                r.update(r="unresolved", note=e.why)
            except Gap as e:
                r.update(r="gap", note=str(e))
            except poke.CallFault as e:
                r.update(r="failed", note=str(e))
            r["src"] = s.text()
            self.results.append(r)
            rec.emit(r)
            out.append(r)
        self.last_frame = self.frame
        return out

    def send(self, rec, tid, saved, b):
        """One call of the transport send (§1 rule 4 steps 3-5); EAX."""
        if not self.checked:  # the entry bytes, under a recorder's own INT3 (record_packets'
            # `client_out` hooks 0x0052AE50) the byte it saved
            code = bytearray(rec.read(TRANSPORT_SEND, 3))
            orig = getattr(rec, "bp_orig", {}).get(TRANSPORT_SEND)
            if orig is not None:
                code[0] = orig
            if bytes(code) != TRANSPORT_SEND_BYTES:
                raise poke.PokeFatal("unexpected code at 0x0052AE50: not the 1.14d Game.exe?")
            self.checked = True
        S = self.caller.page(rec)
        rec.write(S + SCRATCH_MSG, b)
        _, stack, eip = call_layout(saved.Esp, S, len(b))
        # PokeLayer.call pushes [ESP] = S+0 (trap) then the arguments, ESP = saved - 16
        args = struct.unpack("<3I", stack[4:])
        return self.caller.call(rec, tid, saved, eip, {}, list(args))


def add_options(ap):
    g = ap.add_argument_group("sends (send.py)")
    g.add_argument("--send", action="append", default=[], metavar='"F NAME FIELD=VALUE..."',
                   help="a C->S message injected before absolute frame F's drain (at the first "
                        "0x0044F136 stop after tick F-1): a typed message of "
                        "specs/sim/client-messages.tsv or `F hex BYTES...`; repeatable, file order")


def print_results(layer):
    if not layer:
        return
    for x in layer.results:
        print(f"send: f {x['f']} {x['src']}: {x['r']}"
              + (f" bytes {x['bytes']}" if x.get("bytes") else "")
              + (f" ({x['note']})" if x.get("note") else ""))
    if layer.pending():
        print(f"note: {len(layer.pending())} send(s) not reached")


# --- selftest ---------------------------------------------------------------------

class _FakeMem:
    def __init__(self):
        self.m = {}

    def read(self, a, n):
        return bytes(self.m.get(a + i, 0) for i in range(n))

    def read_u32(self, a):
        return struct.unpack("<I", self.read(a, 4))[0]

    def write(self, a, data):
        for i, b in enumerate(data):
            self.m[a + i] = b

    def u32(self, a, v):
        self.write(a, struct.pack("<I", v))


def selftest():
    ok = 0
    tbl = table()
    enc = lambda t, res=None: encode(parse_message(t.split(), tbl), res).hex(" ")  # noqa: E731
    # scenario.md test vectors
    assert enc("Walk x=10 y=20") == "01 0a 00 14 00"
    assert enc("Walk y=20 x=10") == "01 0a 00 14 00"
    assert enc("SelectSkill skill=36 left=0 item=0xFFFFFFFF") == "3c 24 00 00 00 ff ff ff ff"
    assert enc("SelectSkill skill=36 left=1 item=0") == "3c 24 00 00 80 00 00 00 00"
    assert enc("hex 01 02") == "01 02"
    assert enc("BuyItem npc=6 item=0x12 transaction=0 client_price=0x38") == \
        "32 06 00 00 00 12 00 00 00 00 00 00 00 38 00 00 00"  # vendors.md §7.1 rule 10
    assert enc("InitEntityChat id=9") == "2f 00 00 00 00 09 00 00 00"
    ok += 7
    # references (the conformance test's world): GUIDs in ascending order per class
    units = {1: [(12, 148, 0), (30, 148, 0)], 2: [(4, 9, 0), (5, 0, 0)]}

    def res(r):
        if r.kind == "player":
            return 7
        if r.kind in ("x", "y"):
            return (100 if r.kind == "x" else 200) + r.off
        us = [g for g, c, _ in units.get(r.ty, []) if r.cls is None or c == r.cls]
        if r.n >= len(us):
            raise Unresolved(r.text(), f"{r.text()}: no such unit")
        return us[r.n]
    assert enc("Walk x=@x+5 y=@y-3", res) == "01 69 00 c5 00"
    assert enc("InteractWithEntity type=1 id=@1:148", res) == "13 01 00 00 00 0c 00 00 00"
    assert enc("InteractWithEntity type=1 id=@1:148#1", res) == "13 01 00 00 00 1e 00 00 00"
    assert enc("InteractWithEntity type=0 id=@player", res) == "13 00 00 00 00 07 00 00 00"
    for t, ref in (("InteractWithEntity type=1 id=@1:148#2", "@1:148#2"),
                   ("Walk x=@x-101 y=0", "@x-101")):
        try:
            enc(t, res)
            raise AssertionError(t)
        except Unresolved as e:
            assert e.ref == ref, (e.ref, ref)
    ok += 6
    # canonical text, round trip
    m = parse_message("BuyItem item=@4#3 npc=@1:148 client_price=0 transaction=0".split(), tbl)
    assert m.text() == "BuyItem npc=@1:148 item=@4#3 transaction=0 client_price=0", m.text()
    assert parse_message(m.text().split(), tbl).text() == m.text()
    ok += 1
    # strict errors (the conformance parser's)
    for bad in ("", "hex", "hex 1", "hex zz", "Walk x=1", "Nope a=1", "Walk x=70000 y=0",
                "Walk x=1 y=2 z=3", "Walk x=1 x=2 y=3", "Chat", "Walk x=@x+0 y=1",
                "Walk x=@1#0 y=1", "Walk x=@9 y=1", "SelectSkill skill=0x80000000 left=0 item=0",
                "SelectSkill skill=1 left=2 item=0"):
        try:
            parse_message(bad.split(), tbl)
            raise AssertionError(f"accepted {bad!r}")
        except SendError:
            ok += 1
    for bad in ("0 Walk x=1 y=2", "x Walk x=1 y=2", "4", "4 hex"):
        try:
            parse_send_option(bad, tbl=tbl)
            raise AssertionError(f"accepted --send {bad!r}")
        except SendError:
            ok += 1
    s = parse_send_option("12 Walk x=10 y=20", tbl=tbl)
    assert (s.f, s.text()) == (12, "Walk x=10 y=20")
    ok += 1
    # the call layout on a fake context (original-hooks.md §1 rule 4 step 4)
    esp, stack, eip = call_layout(0x0019F000, 0x00A00000, 9)
    assert esp == 0x0019EFF0 and eip == 0x0052AE50
    assert struct.unpack("<4I", stack) == (0x00A00000, 9, 1, 0x00A00010)
    ok += 1
    # the layer on a fake process: due at the first drain stop after tick f - 1,
    # bytes at S+0x10, the call made with (size, 1, S+0x10), EAX checked, records
    mem = _FakeMem()
    game, client, player, path = 0x1000, 0x2000, 0x3000, 0x4000
    mem.u32(LOCAL_MODE, 1)
    mem.u32(CONNECTED, 1)
    mem.u32(game + poke.G_CLIENTS, client)
    mem.u32(client + poke.C_STATE, poke.CLIENT_IN_GAME)
    mem.u32(game + poke.HASH_BASE + poke.HASH_OFFSETS[0] + 4 * 1, player)  # bucket 1: GUID 1
    mem.u32(player + poke.U_TYPE, 0)
    mem.u32(player + poke.U_GUID, 1)
    mem.u32(player + poke.U_PATH, path)
    mem.write(path + poke.P_X, struct.pack("<H", 4880))
    mem.write(path + poke.P_Y, struct.pack("<H", 4230))
    for g, cls, u in ((7, 148, 0x5000), (4, 154, 0x5100)):  # monsters, two buckets
        mem.u32(game + poke.HASH_BASE + poke.HASH_OFFSETS[1] + 4 * g, u)
        mem.u32(u + poke.U_TYPE, 1)
        mem.u32(u + poke.U_CLASS, cls)
        mem.u32(u + poke.U_GUID, g)
    wp = 0x5200  # one waypoint object (class 119, GUID 9): `@wp` resolves to its GUID
    mem.u32(game + poke.HASH_BASE + poke.HASH_OFFSETS[poke.OBJECT_TYPE] + 4 * 9, wp)
    mem.u32(wp + poke.U_TYPE, poke.OBJECT_TYPE)
    mem.u32(wp + poke.U_CLASS, 119)
    mem.u32(wp + poke.U_GUID, 9)
    mem.emitted = []
    mem.emit = mem.emitted.append
    layer = SendLayer([parse_send_option(x, i, tbl) for i, x in enumerate((
        "5 Walk x=@x+2 y=@y", "5 InteractWithEntity type=1 id=@1:148",
        "5 InteractWithEntity type=1 id=@1:148#1", "5 InteractWithEntity type=2 id=@wp",
        "5 InteractWithEntity type=2 id=@wp#1",
        "6 hex 2f 00 00 00 00 07 00 00 00"))])
    calls = []

    class Saved:
        Esp = 0x0019F000

    def fake_send(rec, tid, saved, b):
        S = 0x00A00000
        rec.write(S + SCRATCH_MSG, b)
        esp, stack, eip = call_layout(saved.Esp, S, len(b))
        calls.append((eip, stack, rec.read(S + SCRATCH_MSG, len(b))))
        return 0 if b[0] == 0x2F else 1
    layer.send = fake_send
    mem.write(game + poke.G_FRAME, struct.pack("<i", 3))
    layer.on_tick_return(mem, game)
    assert layer.due() == []  # frame 3 ran: frame 5's steps wait
    mem.write(game + poke.G_FRAME, struct.pack("<i", 4))
    layer.on_tick_return(mem, game)
    rs = layer.inject(mem, 0, Saved, layer.due())
    assert [r["r"] for r in rs] == ["ok", "ok", "unresolved", "ok", "unresolved"], rs
    assert rs[3]["bytes"] == "130200000009000000", rs[3]
    assert rs[4]["note"] == "@wp#1: no waypoint object", rs[4]
    assert rs[0]["bytes"] == "0112138610", rs[0]           # 4882 = 0x1312, 4230 = 0x1086
    assert rs[1]["bytes"] == "130100000007000000" and rs[1]["eax"] == 1, rs[1]
    assert rs[2]["note"] == "@1:148#1: no such unit", rs[2]
    assert [c[0] for c in calls] == [TRANSPORT_SEND] * 3
    assert struct.unpack("<4I", calls[1][1]) == (0x00A00000, 9, 1, 0x00A00010)
    assert calls[1][2] == bytes.fromhex("130100000007000000")
    assert (rs[0]["f"], rs[0]["frame"], rs[0]["i"], rs[3]["i"]) == (5, 4, 0, 3)
    mem.write(game + poke.G_FRAME, struct.pack("<i", 5))
    layer.on_tick_return(mem, game)
    rs = layer.inject(mem, 0, Saved, layer.due())
    assert rs[0]["r"] == "dropped" and rs[0]["eax"] == 0, rs  # EAX 0: not queued
    assert not layer.pending() and len(mem.emitted) == 6
    # the gate (§1 rule 5): client not in state 4 -> unresolved, nothing called
    mem.u32(client + poke.C_STATE, 2)
    layer2 = SendLayer([parse_send_option("6 Walk x=1 y=2", 0, tbl)])
    layer2.send = fake_send
    layer2.on_tick_return(mem, game)
    n = len(calls)
    rs = layer2.inject(mem, 0, Saved, layer2.due())
    assert rs[0]["r"] == "unresolved" and len(calls) == n, rs
    ok += 3
    # send() itself on a fake caller: the entry check sees through a recorder's
    # INT3 (record_packets hooks 0x0052AE50), bytes at S+0x10, the call's stack
    class FakeCaller:
        made = []

        def page(self, rec):
            return 0x00A00000

        def call(self, rec, tid, saved, entry, regs, args):
            self.made.append((entry, regs, args))
            return 1
    mem.write(TRANSPORT_SEND, b"\xCC\x8B\xEC")
    mem.bp_orig = {TRANSPORT_SEND: 0x55}
    layer3 = SendLayer([])
    layer3.caller = FakeCaller()
    assert layer3.send(mem, 0, Saved, bytes.fromhex("010a001400")) == 1
    assert FakeCaller.made == [(TRANSPORT_SEND, {}, [5, 1, 0x00A00010])], FakeCaller.made
    assert mem.read(0x00A00010, 5) == bytes.fromhex("010a001400")
    mem.bp_orig = {}
    layer4 = SendLayer([])
    layer4.caller = FakeCaller()
    try:
        layer4.send(mem, 0, Saved, b"\x01")
        raise AssertionError("accepted other code at 0x0052AE50")
    except poke.PokeFatal:
        pass
    ok += 1
    # every typed row of the table encodes with all fields 0
    for name, row in tbl.items():
        if not row.why:
            b = encode(parse_message([name] + [f"{f[0]}=0" for f in row.fields], tbl))
            assert len(b) == row.size and b[0] == row.id, name
    ok += 1
    print(f"send selftest: {ok} checks passed")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--selftest", action="store_true", help="run the selftest (no game needed)")
    ap.add_argument("--encode", nargs="+", metavar="TOKEN",
                    help="print the bytes of one message without references (`Walk x=1 y=2`)")
    a = ap.parse_args()
    if a.selftest:
        selftest()
        return 0
    if a.encode:
        try:
            print(encode(parse_message(a.encode)).hex(" "))
        except (SendError, TypeError) as e:
            raise SystemExit(f"error: {e}")
        return 0
    ap.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
