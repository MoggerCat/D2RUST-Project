"""Poke: set up game state in the original 1.14d Game.exe at a chosen tick.

The 1.14d side of specs/tools/poke.md (§4): a debugger (base:
record_tick.py's TickRecorder, Win32 debug API through ctypes, standard
library only) stops the server thread at the tick-return hook 0x0052FD1E
and, between two ticks, runs poke directives: it calls the game's own
creation functions (scratch page with an INT3 return trap, saved context,
arguments pushed above the return address; specs/tools/original-hooks-spawn.md
§5) or writes the field the game keeps the state in.

Tick 0 is F0, the frame (game +0xA8) of the tick in which client 0 first
reached state 4 (poke.md §4 rule 2, specs/tools/original-hooks.md §1 rule 5:
client list head game +0x88, state at client +0x04). It is read at each
0x0052FD1E stop until it holds; `--start-frame N` gives F0 explicitly
instead. A directive of tick t runs at the stop whose game +0xA8 = F0 + t,
i.e. before frame F0 + t + 1 runs (poke.md §2 rule 4).

Sources of directives:
  --poke-file FILE   a `poke 1` file (poke.md §2), ticks relative to F0;
  --poke "F D ..."   one directive at ABSOLUTE frame F (game +0xA8): run at
                     the stop whose +0xA8 = F - 1, i.e. before frame F runs.
                     Repeatable; for other recorders too (add_options).

Gaps on this side (result "gap", not run; poke.md Open questions 1-4):
pos, warp, item, stat, state; `spawn ... unique umod ...` (a chosen umod
set cannot be imposed, scenario.md Open question 2); any `@wp` reference
(this tool does not read the objects table's operate function).

Output (CLI): traces/raw/<time>-poke.jsonl (gitignored), format
poke-raw-1: the TickRecorder header (format, tool, date, Game.exe sha256,
args), `poke_file` (path, sha256 of the file, steps), `poke_f0`, one
`poke` record per directive {f, frame, t?, i, d, r, guid?, eax?, args,
note?}, the base `game` / `tick` records, footer.

Library use (record_state.py, record_frames.py, run_scenario.py):
    poke.add_options(ap); layer = poke.PokeLayer.from_args(a)
    rt.EXPECT = {...}            # the recorder's own hook set, then:
    layer.attach(recorder)       # arms 0x0052FD1E, routes its stops here
or call layer.on_tick_return(recorder, game, frame, tid, ctx) /
layer.run_tick(recorder, game, t) from the recorder's own hook handler.

Selftest (no Windows needed): `python3 tools/trace-recorder/poke.py --selftest`.
The parser and the record/layout builders import nothing Windows-only; the
debugger base is imported lazily.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import glob
import hashlib
import os
import re
import struct
import sys
import time

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import autostart  # noqa: E402  (unattended start; imports on any OS)

TOOL = "trace-recorder poke 0.1.0"
FORMAT = "poke-raw-1"

# --- 1.14d addresses and offsets (each from the spec named) ------------------

TICK_RET = 0x0052FD1E            # tick-return hook, ESI = game (original-hooks-spawn.md §5 rule 2)
TICK_RET_BYTES = bytes.fromhex("8B7618")  # `mov esi, [esi+0x18]` (poke.md §4 rule 1)
ALLOC = 0x00555230               # unit allocator: ECX type, EDX class; x, y, game, room, flags, mode, GUID
#                                  (sim/units.md §3.1; world/objects-2.md §22 rule 4: mode = 6th stack arg)
SPAWN = 0x005B2F20               # entry 1: ECX game, EDX room; x, y, class, mode, spread, flags
#                                  (original-hooks-spawn.md §1)
BOSS = 0x005A43E0                # entry 3: ECX game, EDX room; list, class, champ, x, y, warp (§1)
SUPERUNIQUE = 0x005A49B0         # entry 4: ECX game, EDX room; x, y, row (§1; poke.md §1)
CHAMPION_MARK = 0x005A48C0       # entry 5: ECX game, EDX unit; umod (§1)
ROOM_AT = 0x00463740             # entry 6: ECX room, EDX x; y (§1; poke.md §4 rule 5)
MISSILE = 0x0059FA30             # ECX game, EDX record (original-hooks.md §7.1)
MISSILE_BYTES = bytes.fromhex("558BEC83EC28")  # entry bytes (original-hooks.md §7.1 rule 1)

G_FRAME = 0xA8                   # frame just run (original-hooks.md §3)
G_CLIENTS = 0x88                 # client list head; client +0x04 state (original-hooks.md §1 rule 5)
C_STATE = 0x04
CLIENT_IN_GAME = 4               # client state 4 (original-hooks.md §1 rule 5; sim/tick.md §6)
G_ACTS = 0xBC                    # act records, 4 per act (render/lighting.md §9.1; poke.md §1 `time`)
G_SEED = 0xD0                    # game seed lo, hi (sim/rng.md §5.2; original-hooks-spawn.md Constants)
HASH_BASE = 0x1120               # unit hash lists (original-hooks.md §4 rule 1)
HASH_OFFSETS = {0: 0x000, 1: 0x200, 2: 0x400, 4: 0x600, 3: 0x800}  # type -> offset (§4 rule 1)
TILE_LIST = 0x1B20               # tiles (type 5): one list (original-hooks.md §4 rule 1)
U_TYPE, U_CLASS, U_GUID = 0x00, 0x04, 0x0C   # original-hooks.md §4 rule 2
U_ACT = 0x18                     # u8 act (original-hooks.md §4 rule 2)
U_SEED = 0x20                    # unit seed lo +0x20, hi +0x24 (sim/rng.md §5.3; original-hooks.md §4)
U_PATH = 0x2C                    # path (original-hooks-spawn.md §2 rule 3)
U_HASH_NEXT = 0xE4               # hash-list link (original-hooks.md §4 rule 1)
P_X, P_Y, P_ROOM = 0x02, 0x06, 0x1C  # u16 x, u16 y, room (original-hooks-spawn.md §2 rule 3)
ACT_ENV = 0x04                   # act +0x04 = environment record (render/lighting.md §9.1)
ENV_PERIOD, ENV_TICKS = 0x00, 0x08   # period index 0-5, ticks (render/lighting.md §9.1)
MIS_FLAGS = 0x21                 # position given (1) | target absolute (0x20) (missiles.md §R2.1; poke.md §1)
MIS_SIZE = 0x5C                  # parameter record size (missiles.md §R2.1)
OBJECT_TYPE = 2                  # unit type of objects (poke.md §1 `object`)
OBJECT_FLAGS = 1                 # allocation flags 1 = SUNIT_Add (sim/units.md §3.1 step 8)
SPAWN_MODE = 1                   # monster mode 1 (original-hooks-spawn.md §2 rule 6)
SPAWN_SPREAD = 0xFFFFFFFF        # radius -1: one ring d = 0, the exact point (scenario.md §3.1 `normal`;
#                                  monsters/population.md §9.3)
SPAWN_FLAGS = 0                  # flags 0 (original-hooks-spawn.md §3 `normal`)
PLAYER_GUID = 1                  # single-player player (original-hooks-spawn.md §2 rule 2)

ENTRY_BYTES = {TICK_RET: TICK_RET_BYTES, MISSILE: MISSILE_BYTES}  # checked before use
SCRATCH_TRAP = 0x000             # S+0: INT3 return trap (original-hooks-spawn.md §5 rule 3)
SCRATCH_RECORD = 0x100           # S+0x100: the missile record
CALL_TIMEOUT = 20.0              # seconds a call may take before the run is abandoned

TYPE_NAMES = {0: "player", 1: "monster", 2: "object", 3: "missile", 4: "item", 5: "tile"}
KINDS = ("normal", "random-boss", "champion", "unique")
QUALITIES = ("low", "normal", "superior", "magic", "set", "rare", "unique", "crafted")
U32 = 0xFFFFFFFF


# --- parsing (poke.md §1-§2; scenario.md §2 rules 1, 4) ----------------------

class PokeError(ValueError):
    def __init__(self, line, msg):
        super().__init__(f"line {line}: {msg}" if line else msg)
        self.line = line


class Ref:
    """A unit reference: @player, @<type>[:<class>][#n], @wp[#n], <type>/<guid>."""
    __slots__ = ("kind", "type", "cls", "n", "guid")

    def __init__(self, kind, type=None, cls=None, n=0, guid=None):
        self.kind, self.type, self.cls, self.n, self.guid = kind, type, cls, n, guid

    def __eq__(self, o):
        return isinstance(o, Ref) and all(getattr(self, k) == getattr(o, k) for k in self.__slots__)

    def text(self):
        if self.kind == "player":
            return "@player"
        if self.kind == "guid":
            return f"{self.type}/{self.guid}"
        s = "@wp" if self.kind == "wp" else f"@{self.type}" + (
            f":{self.cls}" if self.cls is not None else "")
        return s + (f"#{self.n}" if self.n else "")

    __repr__ = text


class Pos:
    """A position reference @x±N / @y±N (the player's position ± N)."""
    __slots__ = ("axis", "off")

    def __init__(self, axis, off):
        self.axis, self.off = axis, off

    def __eq__(self, o):
        return isinstance(o, Pos) and (self.axis, self.off) == (o.axis, o.off)

    def text(self):
        return f"@{self.axis}" + ("" if self.off == 0 else f"{self.off:+d}")

    __repr__ = text


NUM_RE = re.compile(r"-?(0x[0-9a-fA-F]+|[0-9]+)\Z")


def parse_num(tok, lo, hi, line, what, signed=False):
    if not NUM_RE.match(tok) or (tok.startswith("-") and not signed):
        raise PokeError(line, f"{what}: {tok!r} is not a{' signed' if signed else 'n unsigned'} "
                              "number (decimal or 0x hex)")
    neg = tok.startswith("-")
    body = tok[1:] if neg else tok
    v = int(body, 16) if body.lower().startswith("0x") else int(body, 10)
    v = -v if neg else v
    if not lo <= v <= hi:
        raise PokeError(line, f"{what}: {v} out of range {lo}..{hi}")
    return v


REF_RE = re.compile(r"@(?:(player)|(wp)(?:#(\d+))?|([0-5])(?::(\d+))?(?:#(\d+))?)\Z")
GUID_RE = re.compile(r"([0-5])/(\d+)\Z")
POS_RE = re.compile(r"@([xy])(?:([+-])(0x[0-9a-fA-F]+|[0-9]+))?\Z")


def parse_ref(tok, line, what="unit"):
    m = REF_RE.match(tok)
    if m:
        if m.group(1):
            return Ref("player")
        if m.group(2):
            return Ref("wp", n=int(m.group(3) or 0))
        cls = m.group(5)
        return Ref("type", int(m.group(4)), None if cls is None else int(cls),
                   int(m.group(6) or 0))
    m = GUID_RE.match(tok)
    if m:
        g = int(m.group(2))
        if g > U32:
            raise PokeError(line, f"{what}: GUID {g} out of range")
        return Ref("guid", int(m.group(1)), guid=g)
    raise PokeError(line, f"{what}: {tok!r} is not a unit reference (@player, @<type 0-5>"
                          "[:<class>][#n], @wp[#n] or <type>/<guid>)")


def parse_pos(tok, line, what):
    m = POS_RE.match(tok)
    if m:
        off = 0
        if m.group(2):
            n = m.group(3)
            off = int(n, 16) if n.lower().startswith("0x") else int(n)
            if off > 0xFFFF:
                raise PokeError(line, f"{what}: offset {off} out of range")
            off = -off if m.group(2) == "-" else off
        return Pos(m.group(1), off)
    if tok.startswith("@"):
        raise PokeError(line, f"{what}: {tok!r} is not a position (@x, @y, @x±N, @y±N)")
    return parse_num(tok, 0, 0xFFFF, line, what)


# argument kinds: (name, kind, lo, hi); kind: num, i32, pos, ref, code, enum, onoff
def _n(name, lo=0, hi=U32):
    return (name, "num", lo, hi)


POSITIONAL = {
    "object": [_n("class"), ("x", "pos"), ("y", "pos")],
    "superunique": [_n("row"), ("x", "pos"), ("y", "pos")],
    "missile": [_n("class"), ("x", "pos"), ("y", "pos"), ("tx", "pos"), ("ty", "pos")],
    "seed-game": [_n("lo"), _n("hi")],
    "seed-unit": [("unit", "ref"), _n("lo"), _n("hi")],
    "time": [_n("period", 0, 5), _n("ticks")],
    "pos": [("unit", "ref"), ("x", "pos"), ("y", "pos")],
    "warp": [_n("level")],
    "item": [("code", "code"), ("x", "pos"), ("y", "pos")],
    "stat": [("unit", "ref"), _n("stat", 0, 0xFFFF), _n("layer", 0, 0xFFFF), ("value", "i32")],
    "state": [("unit", "ref"), _n("state", 0, 0xFFFF), ("on", "onoff")],
    "freeze": [_n("seconds", 0, 3600)],
}
OPTIONAL = {  # keyword -> its argument kinds, in the table (canonical) order
    "object": {"mode": [_n("mode")]},
    "missile": {"skill": [_n("skill"), _n("level")], "owner": [("owner", "ref")]},
    "warp": {"tile": [_n("tile")]},
    "item": {"quality": [("quality", "enum", QUALITIES)], "ilvl": [_n("ilvl", 1, 99)]},
}
GAPS = {"pos": "Open question 1", "warp": "Open question 2", "item": "Open question 3",
        "stat": "Open question 4", "state": "Open question 4"}


def _arg(spec, tok, line, d):
    name, kind = spec[0], spec[1]
    what = f"{d} {name}"
    if kind == "num":
        return parse_num(tok, spec[2], spec[3], line, what)
    if kind == "i32":
        return parse_num(tok, -0x80000000, 0x7FFFFFFF, line, what, signed=True)
    if kind == "pos":
        return parse_pos(tok, line, what)
    if kind == "ref":
        return parse_ref(tok, line, what)
    if kind == "code":
        if not re.fullmatch(r"[a-z0-9]{3,4}", tok):
            raise PokeError(line, f"{what}: {tok!r} is not an item code ([a-z0-9], 3-4 chars)")
        return tok
    if kind == "enum":
        if tok not in spec[2]:
            raise PokeError(line, f"{what}: {tok!r} is not one of {', '.join(spec[2])}")
        return tok
    if kind == "onoff":
        if tok not in ("on", "off"):
            raise PokeError(line, f"{what}: {tok!r} is not on or off")
        return tok == "on"
    raise AssertionError(kind)


def parse_directive(toks, line):
    """(directive keyword, args dict) from the tokens after `at <tick>`."""
    if not toks:
        raise PokeError(line, "missing directive")
    d, rest = toks[0], toks[1:]
    if d == "spawn":
        return d, _parse_spawn(rest, line)
    if d not in POSITIONAL:
        raise PokeError(line, f"unknown directive {d!r}")
    pos = POSITIONAL[d]
    if len(rest) < len(pos):
        raise PokeError(line, f"{d}: missing argument {pos[len(rest)][0]}")
    args = {s[0]: _arg(s, t, line, d) for s, t in zip(pos, rest)}
    rest = rest[len(pos):]
    opts = OPTIONAL.get(d, {})
    while rest:
        kw = rest[0]
        if kw not in opts:
            raise PokeError(line, f"{d}: extra argument {kw!r}")
        if kw in args or any(s[0] in args for s in opts[kw]):
            raise PokeError(line, f"{d}: {kw} given twice")
        need = opts[kw]
        if len(rest) < 1 + len(need):
            raise PokeError(line, f"{d} {kw}: missing argument {need[len(rest) - 1][0]}")
        for s, t in zip(need, rest[1:1 + len(need)]):
            args[s[0]] = _arg(s, t, line, d)
        rest = rest[1 + len(need):]
    if d == "object":
        args.setdefault("mode", 0)  # default mode 0 (poke.md §1)
    return d, args


def _parse_spawn(rest, line):
    """spawn <class> <x> <y> <kind> [umod <id>...] (poke.md §1 rule 3; scenario.md §3.1)."""
    names = ("class", "x", "y", "kind")
    if len(rest) < 4:
        raise PokeError(line, f"spawn: missing argument {names[len(rest)]}")
    args = {"class": parse_num(rest[0], 0, U32, line, "spawn class"),
            "x": parse_pos(rest[1], line, "spawn x"), "y": parse_pos(rest[2], line, "spawn y")}
    if rest[3] not in KINDS:
        raise PokeError(line, f"spawn kind: {rest[3]!r} is not one of {', '.join(KINDS)}")
    kind = args["kind"] = rest[3]
    umods = []
    tail = rest[4:]
    if tail:
        if tail[0] != "umod":
            raise PokeError(line, f"spawn: extra argument {tail[0]!r}")
        if len(tail) == 1:
            raise PokeError(line, "spawn umod: missing argument id")
        umods = [parse_num(t, 0, 255, line, "spawn umod") for t in tail[1:]]
    lo, hi = {"normal": (0, 0), "random-boss": (0, 0), "champion": (1, 1),
              "unique": (1, 9)}[kind]  # umod counts (scenario.md §3.1 table)
    if not lo <= len(umods) <= hi:
        raise PokeError(line, f"spawn {kind}: {len(umods)} umods, needs {lo}"
                              + ("" if lo == hi else f"..{hi}"))
    args["umods"] = umods
    return args


def tokens(text):
    """Tokens of one line without its comment (scenario.md §2 rule 1)."""
    out = []
    for t in text.split():
        if t.startswith("#"):
            break
        out.append(t)
    return out


class Step:
    """One directive: at relative tick `t` (file) or before absolute frame `f` (--poke)."""

    def __init__(self, d, args, line=0, t=None, f=None, src=""):
        self.d, self.args, self.line, self.t, self.f, self.src = d, args, line, t, f, src

    def text(self):
        return canonical(self.d, self.args)


def parse_poke(text):
    """Steps of a `poke 1` file (poke.md §2). Raises PokeError naming the line."""
    steps, version_seen, last = [], False, -1
    for no, raw in enumerate(text.split("\n"), 1):
        if raw.endswith("\r"):
            raise PokeError(no, "CR line end (files are LF)")
        toks = tokens(raw)
        if not toks:
            continue
        if not version_seen:
            if toks != ["poke", "1"]:
                raise PokeError(no, f"expected `poke 1`, got {' '.join(toks)!r}")
            version_seen = True
            continue
        if toks[0] != "at":
            raise PokeError(no, f"unknown keyword {toks[0]!r} (expected `at`)")
        if len(toks) < 2:
            raise PokeError(no, "at: missing tick")
        t = parse_num(toks[1], 0, 1_000_000, no, "tick")
        if t < last:
            raise PokeError(no, f"tick {t} before tick {last} (ticks are non-decreasing)")
        last = t
        d, args = parse_directive(toks[2:], no)
        steps.append(Step(d, args, no, t=t, src=" ".join(toks)))
    if not version_seen:
        raise PokeError(0, "empty file: no `poke 1` line")
    return steps


def parse_poke_option(s, index=0):
    """One --poke value "<frame> <directive> <args...>" (absolute frame, >= 1)."""
    toks = tokens(s)
    if not toks:
        raise PokeError(0, f"--poke #{index + 1}: empty")
    try:
        f = parse_num(toks[0], 1, U32, 0, "frame")
        d, args = parse_directive(toks[1:], 0)
    except PokeError as e:
        raise PokeError(0, f"--poke #{index + 1} {s!r}: {e}") from None
    return Step(d, args, 0, f=f, src=" ".join(toks))


def _fmt(v):
    if isinstance(v, (Ref, Pos)):
        return v.text()
    if isinstance(v, bool):
        return "on" if v else "off"
    return str(v)


def canonical(d, args):
    """Canonical text of a directive (poke.md §3 rule 2): numbers decimal,
    optional arguments in the table order."""
    if d == "spawn":
        s = f"spawn {args['class']} {_fmt(args['x'])} {_fmt(args['y'])} {args['kind']}"
        return s + ("" if not args["umods"] else " umod " + " ".join(map(str, args["umods"])))
    out = [d] + [_fmt(args[s[0]]) for s in POSITIONAL[d]]
    for kw, specs in OPTIONAL.get(d, {}).items():
        if specs[0][0] in args:
            out += [kw] + [_fmt(args[s[0]]) for s in specs]
    return " ".join(out)


# --- pure builders: call layouts and the missile record ----------------------

def layout(name, **k):
    """(ECX, EDX, [stack arguments, first at [ESP+4]]) of one call."""
    if name == "object":      # sim/units.md §3.1; world/objects-2.md §22 rule 4; poke.md §1
        return (OBJECT_TYPE, k["cls"],
                [k["x"], k["y"], k["game"], k["room"], OBJECT_FLAGS, k["mode"], 0])
    if name == "superunique":  # original-hooks-spawn.md §1 entry 4
        return (k["game"], k["room"], [k["x"], k["y"], k["row"]])
    if name == "spawn":       # entry 1, flags 0 (original-hooks-spawn.md §1, §3 `normal`)
        return (k["game"], k["room"],
                [k["x"], k["y"], k["cls"], SPAWN_MODE, SPAWN_SPREAD, SPAWN_FLAGS])
    if name == "boss":        # entry 3: no coordinate list, x/y as u16, no warp check (§1, §3)
        return (k["game"], k["room"],
                [0, k["cls"], 1 if k["champion"] else 0, k["x"] & 0xFFFF, k["y"] & 0xFFFF, 0])
    if name == "champion":    # entry 5 (§1, §3 `champion`)
        return (k["game"], k["unit"], [k["umod"] & 0xFF])
    if name == "room_at":     # entry 6 (§1; poke.md §4 rule 5)
        return (k["room"], k["x"], [k["y"]])
    if name == "missile":     # original-hooks.md §7.1 rule 1
        return (k["game"], k["record"], [])
    raise KeyError(name)


def missile_record(owner, cls, x, y, tx, ty, skill=0, level=0):
    """The 0x5C-byte parameter record (missiles.md §R2.1; poke.md §1 `missile`):
    flags 0x21, owner, origin = owner, class, x, y, tx, ty, skill, level; rest 0."""
    b = bytearray(MIS_SIZE)
    for off, v in ((0x00, MIS_FLAGS), (0x04, owner), (0x08, owner), (0x10, cls),
                   (0x14, x), (0x18, y), (0x1C, tx), (0x20, ty), (0x2C, skill), (0x30, level)):
        struct.pack_into("<I", b, off, v & U32)
    return bytes(b)


# --- reading the game (mem: read(addr, n), read_u32(addr)) -------------------

def u16(mem, a):
    return struct.unpack("<H", mem.read(a, 2))[0]


def walk(mem, head, limit=100000):
    p, n = head, 0
    while p and n < limit:
        yield p
        p = mem.read_u32(p + U_HASH_NEXT)
        n += 1


def units_of(mem, game, utype):
    """[(guid, class, unit)] of one unit type, ascending GUID (original-hooks.md §4 rule 1)."""
    if utype == 5:
        heads = [mem.read_u32(game + TILE_LIST)]
    else:
        base = game + HASH_BASE + HASH_OFFSETS[utype]
        heads = [mem.read_u32(base + 4 * b) for b in range(128)]
    out = []
    for h in heads:
        for u in walk(mem, h):
            out.append((mem.read_u32(u + U_GUID), mem.read_u32(u + U_CLASS), u))
    return sorted(out)


def find_unit(mem, game, utype, guid):
    if utype == 5:
        heads = [mem.read_u32(game + TILE_LIST)]
    else:
        heads = [mem.read_u32(game + HASH_BASE + HASH_OFFSETS[utype] + 4 * (guid & 0x7F))]
    for h in heads:
        for u in walk(mem, h):
            if mem.read_u32(u + U_GUID) == guid and mem.read_u32(u + U_TYPE) == utype:
                return u
    return 0


def player_of(mem, game):
    """The player unit: player list bucket 1, type 0 and GUID 1 (original-hooks-spawn.md §2 rule 2)."""
    return find_unit(mem, game, 0, PLAYER_GUID)


def player_pos(mem, unit):
    """(x, y, room) from the dynamic path (original-hooks-spawn.md §2 rule 3)."""
    path = mem.read_u32(unit + U_PATH)
    if not path:
        return None
    return u16(mem, path + P_X), u16(mem, path + P_Y), mem.read_u32(path + P_ROOM)


class Unresolved(Exception):
    pass


class Gap(Exception):
    pass


def resolve_unit(mem, game, ref):
    """The unit pointer of a reference (scenario.md §3 rules 3-4), or Unresolved."""
    if ref.kind == "player":
        u = player_of(mem, game)
    elif ref.kind == "wp":
        raise Gap("@wp needs the objects table's operate function; not read by poke.py")
    elif ref.kind == "guid":
        u = find_unit(mem, game, ref.type, ref.guid)
    else:
        us = [x for x in units_of(mem, game, ref.type) if ref.cls is None or x[1] == ref.cls]
        u = us[ref.n][2] if ref.n < len(us) else 0
    if not u:
        raise Unresolved(f"{ref.text()} matches no unit")
    return u


def resolve_pos(mem, game, v):
    if not isinstance(v, Pos):
        return v
    pl = player_of(mem, game)
    p = player_pos(mem, pl) if pl else None
    if not p:
        raise Unresolved(f"{v.text()}: no player position")
    r = (p[0] if v.axis == "x" else p[1]) + v.off
    if not 0 <= r <= 0xFFFF:
        raise Unresolved(f"{v.text()} = {r} is outside 0..65535")
    return r


def unit_label(mem, u):
    return f"{mem.read_u32(u + U_TYPE)}/{mem.read_u32(u + U_GUID)}"


def resolve_args(mem, game, args):
    """(resolved args for the record, unit pointers by name)."""
    out, ptrs = {}, {}
    for k, v in args.items():
        if isinstance(v, Ref):
            u = resolve_unit(mem, game, v)
            ptrs[k] = u
            out[k] = unit_label(mem, u)
        elif isinstance(v, Pos):
            out[k] = resolve_pos(mem, game, v)
        else:
            out[k] = v
    return out, ptrs


def time_target(mem, game):
    """Address of the player's act's environment record, or 0 (poke.md §1 `time`;
    render/lighting.md §9.1; act u8 at unit +0x18, original-hooks.md §4 rule 2)."""
    pl = player_of(mem, game)
    if not pl:
        raise Unresolved("@player matches no unit")
    act = mem.read(pl + U_ACT, 1)[0]
    if act > 4:
        return 0
    rec = mem.read_u32(game + G_ACTS + 4 * act)
    return mem.read_u32(rec + ACT_ENV) if rec else 0


# --- the layer: runs directives at 0x0052FD1E stops --------------------------

def _rr():
    import record_rng as rr  # Windows only (ctypes.WinDLL)
    return rr


class CallFault(Exception):
    pass


class PokeFatal(RuntimeError):
    pass


class PokeLayer:
    """Holds the directives and makes the calls. Relative steps (a poke file)
    run at the stop whose game +0xA8 = F0 + t; absolute steps (--poke) at the
    stop whose +0xA8 = f - 1."""

    def __init__(self, rel=(), absolute=(), start_frame=None, source=None):
        self.rel, self.abs = list(rel), list(absolute)
        self.f0 = start_frame
        self.f0_how = "--start-frame" if start_frame is not None else None
        self.source = source            # (path, sha256) of the poke file
        self.scratch = None
        self.done = set()               # id(step)
        self.results = []
        self.last_frame = None          # stop at which the last step ran
        self.vproc = None

    # --- construction --------------------------------------------------
    @classmethod
    def from_args(cls, a):
        """From add_options' --poke lines and --poke-file; None when neither."""
        rel, src = [], None
        if getattr(a, "poke_file", None):
            data = open(a.poke_file, "rb").read()
            try:
                rel = parse_poke(data.decode("utf-8"))
            except PokeError as e:
                raise SystemExit(f"{a.poke_file}: {e}")
            src = (a.poke_file, hashlib.sha256(data).hexdigest())
        try:
            absolute = [parse_poke_option(s, i) for i, s in enumerate(getattr(a, "poke", None) or [])]
        except PokeError as e:
            raise SystemExit(str(e))
        if not rel and not absolute:
            return None
        return cls(rel, absolute, getattr(a, "start_frame", None), src)

    def pending(self):
        return [s for s in self.abs + self.rel if id(s) not in self.done]

    def header(self):
        return {"k": "poke_file", "path": self.source[0] if self.source else None,
                "sha256": self.source[1] if self.source else None, "start_frame": self.f0,
                "steps": [{"t": s.t, "f": s.f, "line": s.line, "d": s.d, "text": s.text()}
                          for s in self.abs + self.rel]}

    # --- attaching to a TickRecorder-based recorder --------------------
    def attach(self, rec, before=True):
        """Arm 0x0052FD1E on `rec` (unless armed) and route its stops to
        on_tick_return. Call after the recorder's module-level EXPECT is set.
        `before`: the pokes run before the recorder's own handler of that stop
        (default); False: after it (record_state.py: the snapshot of frame
        f - 1 is taken first, then the pokes of frame f run, as on d2rs)."""
        rt = sys.modules.get("record_tick")
        if rt is None:
            import record_tick as rt
        if TICK_RET not in rt.EXPECT:
            rt.EXPECT[TICK_RET] = TICK_RET_BYTES
        if getattr(rec, "h_process", None) and TICK_RET not in rec.bp_orig:
            if rec.read(TICK_RET, 3) != TICK_RET_BYTES:
                raise RuntimeError("unexpected code at 0x0052FD1E: not the 1.14d Game.exe?")
            rec.arm(TICK_RET)
        orig_bp, orig_handle = rec.on_breakpoint, rec.handle
        layer = self

        def on_breakpoint(tid, addr):
            rec.poke_tid = tid
            return orig_bp(tid, addr)

        def pokes(addr, ctx):
            if addr == TICK_RET:
                game = ctx.Esi
                if getattr(rec, "game", None) in (None, game):
                    frame = struct.unpack("<i", rec.read(game + G_FRAME, 4))[0]
                    layer.on_tick_return(rec, game, frame, rec.poke_tid, ctx)

        def handle(addr, ctx):
            if before:
                pokes(addr, ctx)
                return orig_handle(addr, ctx)
            r = orig_handle(addr, ctx)
            pokes(addr, ctx)
            return r

        rec.on_breakpoint, rec.handle = on_breakpoint, handle

    # --- scheduling ------------------------------------------------------
    def on_tick_return(self, rec, game, frame, tid, ctx=None):
        """Run every step due at this stop (game +0xA8 = frame just run).
        Returns the results; each is also written with rec.emit."""
        if self.f0 is None and self.rel:
            c = rec.read_u32(game + G_CLIENTS)
            if c and rec.read_u32(c + C_STATE) == CLIENT_IN_GAME:
                self.f0, self.f0_how = frame, "client 0 in state 4"
                rec.emit({"k": "poke_f0", "f0": frame, "how": self.f0_how})
        due, missed = [], []
        for s in self.abs:
            if id(s) not in self.done:
                (due if s.f - 1 == frame else missed if s.f - 1 < frame else []).append(s)
        if self.f0 is not None:
            for s in self.rel:
                if id(s) not in self.done:
                    w = self.f0 + s.t
                    (due if w == frame else missed if w < frame else []).append(s)
        res = []
        for s in missed:
            self.done.add(id(s))
            res.append(self._emit(rec, s, frame, -1, {"r": "failed", "note": "stop already passed"}))
        if due:
            res += self.run_steps(rec, game, frame, tid, due)
        return res

    def run_tick(self, rec, game, t, tid=None):
        """Run the poke-file steps of relative tick t now (the caller is at the
        0x0052FD1E stop of frame F0 + t). For run_scenario.py."""
        tid = tid if tid is not None else rec.poke_tid
        frame = struct.unpack("<i", rec.read(game + G_FRAME, 4))[0]
        steps = [s for s in self.rel if s.t == t and id(s) not in self.done]
        return self.run_steps(rec, game, frame, tid, steps)

    def run_steps(self, rec, game, frame, tid, steps):
        rr = _rr()
        saved = rr.WOW64_CONTEXT()
        saved.ContextFlags = 0x1003F  # WOW64_CONTEXT_ALL: integer, control, segments, FPU, debug, SSE
        if not rr.Wow64GetThreadContext(rec.threads[tid], rr.C.byref(saved)):
            raise rr.winerr("Wow64GetThreadContext")
        saved.Eip = TICK_RET  # rewound (original-hooks-spawn.md §5 rule 4)
        out = []
        try:
            for i, s in enumerate(steps):
                self.done.add(id(s))
                out.append(self._emit(rec, s, frame, i, self.apply(rec, game, tid, saved, s)))
        finally:
            # restore the saved context exactly (§5 rule 6); the recorder then
            # steps over the hooked instruction as for any INT3
            if not rr.Wow64SetThreadContext(rec.threads[tid], rr.C.byref(saved)):
                raise rr.winerr("Wow64SetThreadContext")
        self.last_frame = frame
        return out

    def _emit(self, rec, s, frame, i, r):
        rec_ = {"k": "poke", "f": frame + 1, "frame": frame, "i": i, "d": s.d}
        if s.t is not None:
            rec_["t"] = s.t
        rec_.update(r)
        rec_["src"] = s.src
        self.results.append(rec_)
        rec.emit(rec_)
        return rec_

    # --- one directive ---------------------------------------------------
    def apply(self, rec, game, tid, saved, s):
        d, a = s.d, s.args
        if d in GAPS:
            return {"r": "gap", "note": f"no 1.14d call form (poke.md {GAPS[d]})"}
        if d == "spawn" and a["kind"] == "unique":
            return {"r": "gap", "note": "a chosen umod set cannot be imposed "
                                        "(scenario.md Open question 2)"}
        if d == "spawn" and a["kind"] in ("champion", "random-boss"):
            # scenario.md §3.1 names boss spawn 0x005A09E0 (EDI/EBX convention) and the
            # minion call 0x0054E1E0, whose register form no spec states; entries 1/3/5
            # alone would not match d2rs' scenario spawn, so the kind is not run here.
            return {"r": "gap", "note": f"{a['kind']}: scenario.md §3.1 call sequence has "
                                        "no stated form for 0x0054E1E0"}
        try:
            args, ptrs = resolve_args(rec, game, a)
        except Gap as e:
            return {"r": "gap", "note": str(e)}
        except Unresolved as e:
            return {"r": "unresolved", "note": str(e)}
        r = {"args": args}
        try:
            r.update(self._apply(rec, game, tid, saved, d, args, ptrs))
        except Unresolved as e:
            r.update({"r": "unresolved", "note": str(e)})
        except CallFault as e:
            r.update({"r": "failed", "note": str(e)})
        return r

    def _created(self, rec, eax):
        if not eax:
            return {"r": "failed", "eax": 0}
        return {"r": "ok", "eax": f"{eax:#x}", "guid": rec.read_u32(eax + U_GUID)}

    def _room(self, rec, game, tid, saved, x, y):
        pl = player_of(rec, game)
        p = player_pos(rec, pl) if pl else None
        if not p or not p[2]:
            raise Unresolved("no player room")
        return self.call(rec, tid, saved, ROOM_AT, *layout("room_at", room=p[2], x=x, y=y))

    def _apply(self, rec, game, tid, saved, d, a, ptrs):
        if d == "seed-game":  # sim/rng.md §5.2
            rec.write(game + G_SEED, struct.pack("<II", a["lo"], a["hi"]))
            return {"r": "ok"}
        if d == "seed-unit":  # sim/rng.md §5.3
            rec.write(ptrs["unit"] + U_SEED, struct.pack("<II", a["lo"], a["hi"]))
            return {"r": "ok"}
        if d == "time":       # render/lighting.md §9.1
            env = time_target(rec, game)
            if not env:
                return {"r": "failed", "note": "no environment record for the player's act"}
            rec.write(env + ENV_PERIOD, struct.pack("<I", a["period"]))
            rec.write(env + ENV_TICKS, struct.pack("<I", a["ticks"]))
            return {"r": "ok"}
        if d == "freeze":     # poke.md §4 rule 2: the thread stays stopped at the hook
            time.sleep(a["seconds"])
            return {"r": "ok"}
        if d == "missile":    # original-hooks.md §7.1; missiles.md §R2.1
            owner = ptrs.get("owner") or player_of(rec, game)
            if not owner:
                raise Unresolved("@player matches no unit")
            S = self.page(rec)
            if rec.read(MISSILE, len(MISSILE_BYTES)) != MISSILE_BYTES:
                raise PokeFatal("unexpected code at 0x0059FA30: not the 1.14d Game.exe?")
            rec.write(S + SCRATCH_RECORD, missile_record(
                owner, a["class"], a["x"], a["y"], a["tx"], a["ty"], a.get("skill", 0),
                a.get("level", 0)))
            eax = self.call(rec, tid, saved, MISSILE,
                            *layout("missile", game=game, record=S + SCRATCH_RECORD))
            return self._created(rec, eax)
        room = self._room(rec, game, tid, saved, a["x"], a["y"])
        if not room:
            return {"r": "failed", "note": "no loaded room holds the point (entry 6 returned 0)"}
        if d == "object":
            eax = self.call(rec, tid, saved, ALLOC, *layout(
                "object", cls=a["class"], x=a["x"], y=a["y"], game=game, room=room, mode=a["mode"]))
            return self._created(rec, eax)
        if d == "superunique":
            eax = self.call(rec, tid, saved, SUPERUNIQUE, *layout(
                "superunique", game=game, room=room, x=a["x"], y=a["y"], row=a["row"]))
            return self._created(rec, eax)
        if d == "spawn":
            kind = a["kind"]
            if kind == "random-boss":
                eax = self.call(rec, tid, saved, BOSS, *layout(
                    "boss", game=game, room=room, cls=a["class"], champion=True, x=a["x"], y=a["y"]))
                return self._created(rec, eax)
            eax = self.call(rec, tid, saved, SPAWN, *layout(
                "spawn", game=game, room=room, x=a["x"], y=a["y"], cls=a["class"]))
            r = self._created(rec, eax)
            if eax and kind == "champion":
                self.call(rec, tid, saved, CHAMPION_MARK, *layout(
                    "champion", game=game, unit=eax, umod=a["umods"][0]))
            return r
        raise AssertionError(d)

    # --- the call procedure (original-hooks-spawn.md §5) ------------------
    def page(self, rec):
        if self.scratch is None:  # §5 rule 3: one RWX page, S+0 = INT3
            rr = _rr()
            if self.vproc is None:
                self.vproc = rr._proto("VirtualAllocEx", rr.C.c_void_p, rr.W.HANDLE, rr.C.c_void_p,
                                       rr.C.c_size_t, rr.W.DWORD, rr.W.DWORD)
            p = self.vproc(rec.h_process, None, 0x1000, 0x3000, rr.PAGE_EXECUTE_READWRITE)
            if not p:
                raise rr.winerr("VirtualAllocEx")
            rec.write(p + SCRATCH_TRAP, rr.INT3)
            self.scratch = p
        return self.scratch

    def call(self, rec, tid, saved, entry, ecx, edx, args):
        """Call entry on the stopped thread; returns EAX (§5 rules 4-5)."""
        rr = _rr()
        S = self.page(rec)
        esp = saved.Esp - 4 * len(args) - 4
        rec.write(esp, struct.pack("<I", S + SCRATCH_TRAP) +
                  b"".join(struct.pack("<I", v & U32) for v in args))
        ctx = rr.WOW64_CONTEXT.from_buffer_copy(saved)
        ctx.ContextFlags = rr.WOW64_CONTEXT_FULL
        ctx.Esp, ctx.Ecx, ctx.Edx, ctx.Eip = esp, ecx & U32, edx & U32, entry
        ctx.EFlags &= ~rr.TRAP_FLAG
        rec.set_ctx(tid, ctx)
        eax, esp_after = self.pump(rec, tid, S + SCRATCH_TRAP)
        if esp_after != saved.Esp:
            rec.notes.append(f"call {entry:#x}: ESP {esp_after:#x} after return, saved {saved.Esp:#x}")
        return eax

    def pump(self, rec, tid, trap):
        """Continue the stopped event and handle debug events until the return
        trap is hit on `tid`; that event stays pending (the recorder's loop
        continues it after the stop has been restored)."""
        rr = _rr()
        C = rr.C
        pid, ptid = rec.pending
        rr.ContinueDebugEvent(pid, ptid, rr.DBG_CONTINUE)
        rec.pending = None
        ev = rr.DEBUG_EVENT()
        deadline = time.perf_counter() + CALL_TIMEOUT
        while True:
            if time.perf_counter() > deadline:
                raise PokeFatal(f"call did not return within {CALL_TIMEOUT}s")
            if not rr.WaitForDebugEvent(C.byref(ev), 100):
                continue
            code, t = ev.dwDebugEventCode, ev.dwThreadId
            rec.pending = (ev.dwProcessId, t)
            status = rr.DBG_CONTINUE
            if code == rr.EXCEPTION_DEBUG_EVENT:
                er = ev.u.Exception.ExceptionRecord
                addr = er.ExceptionAddress or 0
                if t == tid and er.ExceptionCode in rr.BREAKPOINT_CODES and addr == trap:
                    c = rec.get_ctx(tid)
                    return c.Eax, c.Esp
                status = rec.on_exception(t, ev.u.Exception)
                if t == tid and status == rr.DBG_EXCEPTION_NOT_HANDLED:
                    # left pending; the saved context is restored by run_steps
                    raise CallFault(f"exception {er.ExceptionCode:#x} at {addr:#x} in the call")
            elif code == rr.CREATE_THREAD_DEBUG_EVENT:
                rec.threads[t] = ev.u.CreateThread.hThread
            elif code == rr.EXIT_THREAD_DEBUG_EVENT:
                rec.threads.pop(t, None)
            elif code in (rr.LOAD_DLL_DEBUG_EVENT, rr.CREATE_PROCESS_DEBUG_EVENT):
                rec.close_event_handles(ev)
            elif code == rr.EXIT_PROCESS_DEBUG_EVENT:
                rr.ContinueDebugEvent(ev.dwProcessId, t, rr.DBG_CONTINUE)
                rec.pending = None
                raise PokeFatal(f"game exited during a call, code {ev.u.ExitProcess.dwExitCode:#x}")
            rr.ContinueDebugEvent(ev.dwProcessId, t, status)
            rec.pending = None


def add_options(ap):
    """--poke (repeatable, absolute frame), --poke-file, --start-frame."""
    g = ap.add_argument_group("pokes (poke.py)")
    g.add_argument("--poke", action="append", default=[], metavar='"F DIRECTIVE ARGS..."',
                   help="a directive run before absolute frame F (at the 0x0052FD1E stop whose "
                        "game +0xA8 = F-1); repeatable")
    g.add_argument("--poke-file", default=None, metavar="FILE",
                   help="a `poke 1` file (specs/tools/poke.md §2); ticks relative to F0")
    g.add_argument("--start-frame", type=int, default=None, metavar="F0",
                   help="F0 for --poke-file ticks (default: the frame of the tick in which "
                        "client 0 first reached state 4)")


# --- CLI ----------------------------------------------------------------------

def make_recorder_class():
    import record_tick as rt

    class PokeRecorder(rt.TickRecorder):
        layer = None
        after = 0

        def loop(self, deadline):  # the header is written; add the poke file record
            self.emit(self.layer.header())
            return super().loop(deadline)

        def handle(self, addr, ctx):
            super().handle(addr, ctx)
            if addr == TICK_RET and not self.layer.pending() and self.layer.last_frame is not None:
                frame = struct.unpack("<i", self.read(ctx.Esi + G_FRAME, 4))[0]
                if frame >= self.layer.last_frame + self.after:
                    self.notes.append(f"all pokes done; stopped {self.after} frames after the last")
                    self.done = True

    return rt, PokeRecorder


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.normpath(os.path.join(here, "..", ".."))
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--selftest", action="store_true", help="run the selftest (no game needed)")
    ap.add_argument("--game", default=os.path.join(repo, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=120.0, help="kill the game after N s (default 120)")
    ap.add_argument("--after", type=int, default=50,
                    help="frames to keep running after the last poke, then stop (default 50)")
    ap.add_argument("--snap-every", type=int, default=0,
                    help="record_tick list snapshots every N frames (default 0 = none)")
    ap.add_argument("--tick-records", action="store_true",
                    help="arm all record_tick hooks (timers, lists) too; default: tick hook only")
    ap.add_argument("--out", default=None, help="output file (default traces/raw/<time>-poke.jsonl)")
    ap.add_argument("game_args", nargs="*", default=["-w", "-ns"],
                    help="Game.exe arguments (default: -w -ns)")
    add_options(ap)
    autostart.add_options(ap)
    a = ap.parse_args()
    if a.selftest:
        selftest(repo)
        return
    layer = PokeLayer.from_args(a)
    if layer is None:
        ap.error("--poke-file FILE and/or --poke \"F DIRECTIVE ...\" required")
    gargs, auto = autostart.setup(a, a.game_args or ["-w", "-ns"])
    out = a.out or os.path.join(
        repo, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-poke.jsonl")
    rt, PokeRecorder = make_recorder_class()
    if not a.tick_records:
        rt.EXPECT = {rt.TICK: rt.EXPECT[rt.TICK]}
    rt.FORMAT, rt.TOOL = FORMAT, TOOL
    r = PokeRecorder(os.path.abspath(a.game), gargs, out, a.seconds, a.snap_every, 0)
    r.auto, r.layer, r.after = auto, layer, a.after
    layer.attach(r)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    for n in r.notes:
        print("note:", n)
    by = {}
    for x in layer.results:
        by[x["r"]] = by.get(x["r"], 0) + 1
    left = len(layer.pending())
    print(f"wrote {out}: F0 {layer.f0} ({layer.f0_how}), results {by}, not reached {left}")
    sys.exit(0 if not left and set(by) <= {"ok", "gap"} else 1)


# --- selftest -------------------------------------------------------------------

class FakeMem:
    """Sparse little-endian memory for the selftest."""

    def __init__(self):
        self.m = {}

    def read(self, a, n):
        return bytes(self.m.get(a + i, 0) for i in range(n))

    def read_u32(self, a):
        return struct.unpack("<I", self.read(a, 4))[0]

    def write(self, a, data):
        for i, b in enumerate(data):
            self.m[a + i] = b

    def w32(self, a, v):
        self.write(a, struct.pack("<I", v))


def selftest(repo):
    n = 0
    # 1. every committed poke file parses and round-trips through the canonical form
    files = sorted(glob.glob(os.path.join(repo, "traces", "pokes", "*.poke")))
    assert files, "no traces/pokes/*.poke"
    for p in files:
        name = os.path.basename(p)[:-5]
        assert re.fullmatch(r"[a-z0-9-]+", name), f"{p}: name"
        steps = parse_poke(open(p, encoding="utf-8").read())
        assert steps, p
        canon = "poke 1\n" + "".join(f"at {s.t} {s.text()}\n" for s in steps)
        again = parse_poke(canon)
        assert [(s.t, s.d, s.args) for s in again] == [(s.t, s.d, s.args) for s in steps], p
        n += 1

    # 2. malformed lines: an error naming the line
    bad = [
        ("poke 2\n", 1), ("\n# c\npoke 1 x\n", 3), ("at 0 freeze 1\n", 1),
        ("poke 1\nat 0 fly 1\n", 2), ("poke 1\nat 0 object 1 2\n", 2),
        ("poke 1\nat 0 object 1 2 3 4\n", 2), ("poke 1\nat 0 object 1 2 3 mode\n", 2),
        ("poke 1\nat 0 object 1 2 3 mode 1 mode 2\n", 2), ("poke 1\nat 0 time 6 0\n", 2),
        ("poke 1\nat 0 time -1 0\n", 2), ("poke 1\nat 0 seed-game 0x100000000 0\n", 2),
        ("poke 1\n\nat 3 freeze 1\nat 2 freeze 1\n", 4), ("poke 1\nat 1000001 freeze 0\n", 2),
        ("poke 1\nat x freeze 0\n", 2), ("poke 1\nat\n", 2), ("poke 1\nfreeze 1\n", 2),
        ("poke 1\nat 0\n", 2), ("poke 1\nat 0 spawn 19 1 2 boss\n", 2),
        ("poke 1\nat 0 spawn 19 1 2 champion\n", 2), ("poke 1\nat 0 spawn 19 1 2 normal umod 1\n", 2),
        ("poke 1\nat 0 spawn 19 1 2 unique umod\n", 2), ("poke 1\nat 0 spawn 19 1 2 normal x\n", 2),
        ("poke 1\nat 0 spawn 19 1 2 unique umod 1 2 3 4 5 6 7 8 9 10\n", 2),
        ("poke 1\nat 0 spawn 19 1 2 champion umod 256\n", 2), ("poke 1\nat 0 spawn 19 1\n", 2),
        ("poke 1\nat 0 seed-unit @6 1 2\n", 2), ("poke 1\nat 0 seed-unit @player 1\n", 2),
        ("poke 1\nat 0 seed-unit player 1 2\n", 2), ("poke 1\nat 0 object 1 @z 3\n", 2),
        ("poke 1\nat 0 object 1 65536 3\n", 2), ("poke 1\nat 0 object 1 @x+0x10000 3\n", 2),
        ("poke 1\nat 0 missile 1 2 3 4 5 skill 1\n", 2), ("poke 1\nat 0 missile 1 2 3 4 5 owner\n", 2),
        ("poke 1\nat 0 item ABC 1 2\n", 2), ("poke 1\nat 0 item hp5 1 2 quality great\n", 2),
        ("poke 1\nat 0 item hp5 1 2 ilvl 0\n", 2), ("poke 1\nat 0 state @player 3 maybe\n", 2),
        ("poke 1\nat 0 stat @player 0 0 0x80000000\n", 2), ("poke 1\nat 0 freeze 3601\n", 2),
        ("poke 1\nat 0 freeze +1\n", 2), ("poke 1\nat 0 freeze 1\r\n", 2), ("# only\n", 0),
        ("poke 1\nat 0 warp 1 tile\n", 2), ("poke 1\nat 0 superunique 1 2 3 4\n", 2),
    ]
    for text, line in bad:
        try:
            parse_poke(text)
        except PokeError as e:
            assert e.line == line, f"{text!r}: line {e.line}, want {line} ({e})"
            n += 1
            continue
        raise AssertionError(f"accepted {text!r}")

    # every directive parses and re-parses from its canonical text
    good = ["object 39 @x-3 @y+3", "object 39 10 20 mode 2", "superunique 0 @x @y",
            "missile 1 @x @y 0x40 50 skill 36 5 owner @1:19#2", "missile 1 1 2 3 4 owner 1/77",
            "seed-game 0x12345678 666", "seed-unit @player 1 2", "time 5 4294967295",
            "pos @1 3 4", "warp 3 tile 2", "item hp5 1 2 quality unique ilvl 99", "item hp5 1 2",
            "stat @player 13 0 -5", "state @1:19 11 off", "freeze 0", "spawn 19 @x+3 @y-3 normal",
            "spawn 19 1 2 champion umod 16", "spawn 19 1 2 unique umod 1 2 9",
            "spawn 0x13 1 2 random-boss", "seed-unit @wp#1 1 2"]
    for g in good:
        d, args = parse_directive(g.split(), 1)
        d2, args2 = parse_directive(canonical(d, args).split(), 1)
        assert (d, args) == (d2, args2), g
        n += 1
    assert canonical(*parse_directive("object 0x27 1 2".split(), 1)) == "object 39 1 2 mode 0"
    assert canonical(*parse_directive("missile 1 1 2 3 4 owner @player skill 3 4".split(), 1)) \
        == "missile 1 1 2 3 4 skill 3 4 owner @player"

    # 3. references
    refs = {"@player": Ref("player"), "@1": Ref("type", 1, None, 0), "@1:19": Ref("type", 1, 19, 0),
            "@1:148#1": Ref("type", 1, 148, 1), "@2#3": Ref("type", 2, None, 3),
            "@wp": Ref("wp", n=0), "@wp#2": Ref("wp", n=2), "1/77": Ref("guid", 1, guid=77),
            "5/0": Ref("guid", 5, guid=0)}
    for t, want in refs.items():
        assert parse_ref(t, 1) == want, t
        assert parse_ref(want.text(), 1) == want, t
        n += 1
    for t in ("@7", "@1:", "@1#", "@player#1", "6/1", "1/", "@x", "player", "@1:-1"):
        try:
            parse_ref(t, 9)
            raise AssertionError(f"ref {t!r} accepted")
        except PokeError as e:
            assert e.line == 9
            n += 1
    for t, want in {"@x": Pos("x", 0), "@y+3": Pos("y", 3), "@x-0x10": Pos("x", -16), "7": 7}.items():
        assert parse_pos(t, 1, "x") == want, t
        n += 1

    # --poke option lines (absolute frames)
    s = parse_poke_option("120 spawn 19 @x+3 @y normal  # a comment")
    assert (s.f, s.d, s.args["class"], s.args["x"], s.t) == (120, "spawn", 19, Pos("x", 3), None)
    s = parse_poke_option("0x10 seed-game 1 2")
    assert (s.f, s.args) == (16, {"lo": 1, "hi": 2})
    for b in ("0 freeze 1", "5", "5 nope", "-1 freeze 1", "x freeze 1", "5 time 9 0", ""):
        try:
            parse_poke_option(b)
            raise AssertionError(f"--poke {b!r} accepted")
        except PokeError:
            n += 1

    # from_args
    ns = argparse.Namespace(poke=["10 freeze 0"], poke_file=files[0], start_frame=7)
    lay = PokeLayer.from_args(ns)
    assert lay.f0 == 7 and len(lay.abs) == 1 and lay.rel and lay.source[0] == files[0]
    assert PokeLayer.from_args(argparse.Namespace(poke=[], poke_file=None, start_frame=None)) is None

    # 4. the missile record, exact bytes (missiles.md §R2.1; poke.md §1)
    want = bytes.fromhex(
        "21000000" "44332211" "44332211" "00000000"    # flags 0x21, owner, origin = owner, target 0
        "3e000000" "34120000" "78560000" "40120000"    # class 62, x, y, tx
        "80560000" "00000000" "00000000" "24000000"    # ty, gfx, velocity, skill 36
        "05000000" + "00" * 0x28)                      # level 5, rest 0
    assert len(want) == 0x5C
    got = missile_record(0x11223344, 62, 0x1234, 0x5678, 0x1240, 0x5680, 36, 5)
    assert got == want, got.hex()
    for i in range(len(want)):  # perturbation: every changed byte is seen
        pert = bytearray(want)
        pert[i] ^= 0x01
        assert got != bytes(pert)
        n += 1

    # 5. call layouts against the spec argument orders (literal, written from the specs)
    G, R, U = 0x0A000000, 0x0B000000, 0x0C000000
    expected = {
        # sim/units.md §3.1: ECX type, EDX class; x, y, game, room, flags, mode, GUID
        ("object", (("cls", 39), ("x", 100), ("y", 200), ("game", G), ("room", R), ("mode", 2))):
            (2, 39, [100, 200, G, R, 1, 2, 0]),
        # original-hooks-spawn.md §1 entry 4: ECX game, EDX room; x, y, row
        ("superunique", (("game", G), ("room", R), ("x", 5), ("y", 6), ("row", 3))):
            (G, R, [5, 6, 3]),
        # entry 1: ECX game, EDX room; x, y, class, mode 1, spread, flags 0
        ("spawn", (("game", G), ("room", R), ("x", 5), ("y", 6), ("cls", 19))):
            (G, R, [5, 6, 19, 1, 0xFFFFFFFF, 0]),
        # entry 3: ECX game, EDX room; coord list 0, class, champion allowed, x, y, warp check 0
        ("boss", (("game", G), ("room", R), ("cls", 19), ("champion", True), ("x", 0x15), ("y", 6))):
            (G, R, [0, 19, 1, 0x15, 6, 0]),
        # entry 5: ECX game, EDX unit; umod
        ("champion", (("game", G), ("unit", U), ("umod", 16))): (G, U, [16]),
        # entry 6: ECX room, EDX x; y
        ("room_at", (("room", R), ("x", 5), ("y", 6))): (R, 5, [6]),
        # original-hooks.md §7.1: ECX game, EDX record; no stack arguments
        ("missile", (("game", G), ("record", U))): (G, U, []),
    }

    def check(exp):
        return [k for k, v in exp.items() if layout(k[0], **dict(k[1])) != v]

    assert check(expected) == [], check(expected)
    for k, (ecx, edx, st) in expected.items():  # perturbation: one changed value → detected
        variants = [(ecx + 1, edx, st), (ecx, edx + 1, st)] + \
                   [(ecx, edx, st[:i] + [st[i] + 1] + st[i + 1:]) for i in range(len(st))] + \
                   [(ecx, edx, st + [0])]
        for v in variants:
            assert check({k: v}) == [k], (k, v)
            n += 1

    # 6. reading a fake game: references, positions, the time target
    m = FakeMem()
    game, path, room = 0x100000, 0x300000, 0x400000
    units = [  # (addr, type, guid, class)
        (0x200000, 0, 1, 1), (0x200100, 1, 9, 19), (0x200200, 1, 0x85, 19), (0x200300, 1, 5, 20),
        (0x200400, 1, 7, 19), (0x200500, 2, 3, 39), (0x200600, 5, 2, 0)]
    for addr, t, g, c in units:
        m.w32(addr + U_TYPE, t)
        m.w32(addr + U_GUID, g)
        m.w32(addr + U_CLASS, c)
        head = game + TILE_LIST if t == 5 else game + HASH_BASE + HASH_OFFSETS[t] + 4 * (g & 0x7F)
        m.w32(addr + U_HASH_NEXT, m.read_u32(head))  # prepend
        m.w32(head, addr)
    m.w32(0x200000 + U_PATH, path)
    m.write(path + P_X, struct.pack("<H", 5000))
    m.write(path + P_Y, struct.pack("<H", 4000))
    m.w32(path + P_ROOM, room)
    assert [g for g, _, _ in units_of(m, game, 1)] == [5, 7, 9, 0x85]
    cases = {"@player": 0x200000, "@1": 0x200300, "@1:19": 0x200400, "@1:19#1": 0x200100,
             "@1:19#2": 0x200200, "@2:39": 0x200500, "1/133": 0x200200, "5/2": 0x200600,
             "@5": 0x200600, "0/1": 0x200000}
    for t, want_u in cases.items():
        assert resolve_unit(m, game, parse_ref(t, 1)) == want_u, t
        n += 1
    for t in ("@1:19#3", "@1:21", "1/6", "@3", "@4", "0/2"):
        try:
            resolve_unit(m, game, parse_ref(t, 1))
            raise AssertionError(f"{t} resolved")
        except Unresolved:
            n += 1
    try:
        resolve_unit(m, game, parse_ref("@wp", 1))
        raise AssertionError("@wp resolved")
    except Gap:
        n += 1
    assert resolve_pos(m, game, Pos("x", 3)) == 5003 and resolve_pos(m, game, Pos("y", -10)) == 3990
    try:
        resolve_pos(m, game, Pos("x", 0xFFFF))
        raise AssertionError("position out of range resolved")
    except Unresolved:
        n += 1
    args, ptrs = resolve_args(m, game, parse_directive("seed-unit @1:19 1 2".split(), 1)[1])
    assert args == {"unit": "1/7", "lo": 1, "hi": 2} and ptrs == {"unit": 0x200400}
    m.write(0x200000 + U_ACT, b"\x02")
    act, env = 0x500000, 0x600000
    m.w32(game + G_ACTS + 4 * 2, act)
    m.w32(act + ACT_ENV, env)
    assert time_target(m, game) == env
    m.write(0x200000 + U_ACT, b"\x01")
    assert time_target(m, game) == 0  # act 1 record absent

    # 7. scheduling with a fake recorder (field writes only, no calls)
    class Rec(FakeMem):
        def __init__(self):
            super().__init__()
            self.out, self.notes = [], []

        def emit(self, r):
            self.out.append(r)

    rec = Rec()
    rec.m = dict(m.m)
    lay = PokeLayer(parse_poke("poke 1\nat 0 seed-game 1 2\nat 2 seed-unit @1:19 3 4\n"
                               "at 2 pos @player 1 2\nat 2 seed-unit @1:77 0 0\n"),
                    [parse_poke_option("12 seed-game 5 6")])
    lay.run_steps = lambda r, g, f, tid, steps: [  # no thread context here
        lay._emit(r, s, f, i, lay.apply(r, g, tid, None, s)) for i, s in enumerate(steps)
        if not lay.done.add(id(s))]
    client = 0x700000
    rec.w32(game + G_CLIENTS, client)
    rec.w32(client + C_STATE, 3)
    assert lay.on_tick_return(rec, game, 10, 1) == [] and lay.f0 is None
    rec.w32(client + C_STATE, 4)
    r = lay.on_tick_return(rec, game, 11, 1)  # F0 = 11: tick 0 and the --poke at frame 12 run
    assert lay.f0 == 11 and [(x["d"], x["r"], x["f"]) for x in r] == [
        ("seed-game", "ok", 12), ("seed-game", "ok", 12)], r
    assert rec.read(game + G_SEED, 8) == struct.pack("<II", 1, 2)  # the file's line ran last
    assert lay.on_tick_return(rec, game, 12, 1) == []
    r = lay.on_tick_return(rec, game, 13, 1)
    assert [(x["t"], x["i"], x["d"], x["r"]) for x in r] == [
        (2, 0, "seed-unit", "ok"), (2, 1, "pos", "gap"), (2, 2, "seed-unit", "unresolved")], r
    assert rec.read(0x200400 + U_SEED, 8) == struct.pack("<II", 3, 4)
    assert not lay.pending() and [x["k"] for x in rec.out][0] == "poke_f0"
    n += 1

    # 8. attach: the stop's handler order (before / after the recorder's own)
    class FakeRt:
        EXPECT = {}
    saved_rt = sys.modules.get("record_tick")
    sys.modules["record_tick"] = FakeRt
    try:
        for before in (True, False):
            order = []

            class Ctx:
                Esi = game

            class AttRec(Rec):
                game = None
                poke_tid = None

                def on_breakpoint(self, tid, addr):
                    return None

                def handle(self, addr, ctx):
                    order.append("own")

            ar = AttRec()
            ar.m = dict(rec.m)
            ar.w32(game + G_FRAME, 19)
            al = PokeLayer(absolute=[parse_poke_option("20 seed-game 7 8")])
            al.run_steps = lambda r, g, f, tid, steps: [order.append("poke") or al.done.add(id(s))
                                                        for s in steps]
            al.attach(ar, before=before)
            assert FakeRt.EXPECT[TICK_RET] == TICK_RET_BYTES
            ar.on_breakpoint(7, TICK_RET)
            ar.handle(TICK_RET, Ctx)
            assert ar.poke_tid == 7
            assert order == (["poke", "own"] if before else ["own", "poke"]), (before, order)
            n += 1
    finally:
        if saved_rt is None:
            sys.modules.pop("record_tick", None)
        else:
            sys.modules["record_tick"] = saved_rt
    print(f"selftest ok: {len(files)} poke file(s), {n} checks (malformed lines, references, "
          "--poke lines, missile record bytes, call layouts with perturbation, fake-game "
          "resolution and scheduling)")


if __name__ == "__main__":
    main()
