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

Call forms (poke.md §4 rule 8): every function a directive calls is an entry
of CALL_FORMS (address, argument names, cited spec, `form`). A form says which
argument goes in which register (EAX, EBX, ECX, EDX, ESI, EDI) and which on the
stack; `form=None` = not stated by a spec yet, and every directive calling that
function returns "gap" naming it and its pc1-data.md item. Record fields the
directives use are FIELDS (None = gap). `--forms FILE` (JSON, poke-forms-1)
overrides both for one run. Every directive has its forms (pos, warp, item,
stat, state: pc1-data.md Step 4 item 22 (a)-(d), stated in the owning specs).
Any `@wp` reference is a gap too (this tool does not read the objects table's
operate function).

`msg <id> <value>...` (poke.md §1 `msg`) writes one C→S message (layout from
specs/sim/client-messages.tsv) at S+0x300 and calls the client sender
0x00478350 (EDI size, [ESP+4] message; sim/intents-events.md §2.1 rule 1), so
the server drains it in the next frame as if the client had sent it. The
sender's duplicate filter (wall clock) is not visible: the result is `ok`.

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
SEND = 0x00478350                # client game-message sender: EDI size, [ESP+4] message; duplicate
#                                  filter (sim/intents-events.md §2.1 rule 1; poke.md §1 `msg`)

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
SCRATCH_ITEM = 0x200             # S+0x200: the item request (0x84 bytes)
SCRATCH_MSG = 0x300              # S+0x300: a `msg` message (< 0x200 bytes, the sender asserts it)
CALL_TIMEOUT = 20.0              # seconds a call may take before the run is abandoned

TYPE_NAMES = {0: "player", 1: "monster", 2: "object", 3: "missile", 4: "item", 5: "tile"}
KINDS = ("normal", "random-boss", "champion", "unique")
QUALITIES = ("low", "normal", "superior", "magic", "set", "rare", "unique", "crafted")
U32 = 0xFFFFFFFF

U_STATS = 0x5C                   # the unit's own stat list (sim/stat-lists.md §5 rule 2)
DYNAMIC_PATH_TYPES = (0, 1, 3)   # players, monsters, missiles (sim/path-placement.md §10 rule 1)
POS_VIA = ("teleport", "place")  # `pos`: the first of these with a form runs; the teleport
#                                  is d2rs' path (poke.md §1 `pos`), the placement adds messages
POS_EXACT, POS_ALT = 1, 0        # 0x00554EA0 exact 1: no free-point search (path-placement.md §10)
BOSS_GUID_NEW = 0xFFFFFFFF       # GUID -1: a new unit (monsters/init.md §25.1)
UNIQUE_MIN, UNIQUE_MAX = 3, 6    # 0x005A2120(boss, 3, 6) (scenario.md §3.1 `unique`; init.md §25.3)
UMOD_MAX = 9                     # umod list length (monsters/init.md §25.3 rule 1)
ITEM_MODE_GROUND = 3             # request spawn mode 3 = ground (items/generation.md Inputs)
ITEM_INIT_FLAGS = 1              # request init flags 1 (world/objects-2.md §20.7 rule 3)
ITEM_ILVL_DEFAULT = 1            # `item` without ilvl: 1, as d2rs' request (d2-sim::poke)
# `goto` (poke.md §6): the walk's room records
GOTO_MAX_STEPS = 400             # steps after which the walk ends failed (§6 rule 3.4)
GOTO_EXACT, GOTO_ALT = 0, 0      # 0x00554EA0 exact 0: the free-point search, mask 0x1C09 (path-placement.md §10 r3)
AR_DRLG = 0x10                   # active room +0x10 -> DRLG room (drlg/rooms.md §1; tools/state-snapshot.md §2 `lv`)
AR_SUB = 0x4C                    # active room +0x4C sub-tile x, y, w, h (u32 each; sim/pathing.md §13.3)
AR_COLL = 0x20                   # active room +0x20 collision record (drlg/rooms.md §10.3, Open question 11)
CR_X, CR_Y, CR_W, CR_H, CR_MASKS = 0x00, 0x04, 0x08, 0x0C, 0x20  # record x0, y0, w, h, u16 masks
#                                  (drlg/rooms.md §10.3; items/treasure.md §8: index (y-y0)*w + (x-x0))
PLAYER_MOVE = 0x1C09             # the player's move mask (sim/path-placement.md §3)
LAND_MASK = PLAYER_MOVE | 0x4    # a landing cell has none of these: move bits + missile-blocking 0x4 (poke.md §1 `pos`; REC-1080)
DR_NEAR, DR_NEAR_N = 0x08, 0x2C  # DRLG room near array, its count (drlg/rooms.md §1)
DR_ACTIVE = 0x30                 # DRLG room +0x30 active room, 0 = none (drlg/rooms.md §1)
DR_TX, DR_TY = 0x34, 0x38        # DRLG room tile x, y (drlg/rooms.md §1)
DR_LEVEL, L_ID = 0x58, 0x1D0     # DRLG room +0x58 level -> +0x1D0 level id (drlg/rooms.md §1; drlg/levels.md)
SP_ROOM, SP_X, SP_Y = 0x00, 0x0C, 0x10  # static path: room, u32 x, y (tools/state-snapshot.md §2)
ITEM_REQ_SIZE = 0x84             # D2ItemDropStrc (items/generation.md Inputs)
ITEM_REQUEST = {                 # field -> (offset, width) (items/generation.md Inputs table)
    "unit": (0x00, 4), "game": (0x08, 4), "ilvl": (0x0C, 4), "item": (0x14, 4),
    "mode": (0x18, 4), "x": (0x1C, 4), "y": (0x20, 4), "room": (0x24, 4),
    "init_flags": (0x28, 2), "format": (0x2A, 2), "quality": (0x30, 4), "flags2": (0x80, 4)}

# Record fields and table addresses the gap directives read or write. Each is
# cited; None = not stated by any spec yet: the directive needing it is a gap.
# PC 1 corrects or fills them here (or in a --forms file, "fields").
FIELDS = {
    "item_format": 0x78,         # game +0x78 -> request format (world/objects-2.md §20.7 r3; cube.md §7.4)
    "items_header": 0x0096CA58,  # combined items: count +0x00, records +0x04 (items/treasure.md §9.1)
    "item_record_size": 424,     # weapons/armor/misc record size (data/loading.md §6 rows 15-17)
    "item_code": 0x80,           # u32 code, space-padded (data/loading.md §7 rows 15-17)
    "mon_data": 0x14,            # unit +0x14 = monster data (monsters/init.md §25.3 rule 1)
    "mon_umods": 0x1C,           # monster data +0x1C: umod list, 9 bytes (init.md §25.3 r1; §1 table)
}


# --- call forms (poke.md §4 rule 8) --------------------------------------------

REGISTERS = ("eax", "ebx", "ecx", "edx", "esi", "edi")
RESULTS = ("unit", "bool", "none")   # EAX = created unit (0 failed) / 1 ok, 0 failed / ignored
FORMS_FORMAT = "poke-forms-1"


class Form:
    """How one 1.14d function takes its arguments. `regs`: register -> argument
    name (or a literal int); `stack`: the stack arguments, first at [ESP+4], each
    a name or a literal int; `ret`: the callee's `ret N` (0 or 4 x len(stack));
    `result`: what EAX means when the function's entry leaves it open."""
    __slots__ = ("regs", "stack", "ret", "result")

    def __init__(self, regs=None, stack=(), ret=None, result=None):
        self.regs, self.stack, self.ret, self.result = dict(regs or {}), list(stack), ret, result

    def __eq__(self, o):
        return isinstance(o, Form) and all(getattr(self, k) == getattr(o, k) for k in self.__slots__)

    def __repr__(self):
        return f"Form(regs={self.regs}, stack={self.stack}, ret={self.ret}, result={self.result})"

    def to_json(self):
        d = {"regs": self.regs, "stack": self.stack}
        if self.ret is not None:
            d["ret"] = self.ret
        if self.result is not None:
            d["result"] = self.result
        return d

    @classmethod
    def from_json(cls, d):
        extra = set(d) - {"regs", "stack", "ret", "result"}
        if not isinstance(d, dict) or extra:
            raise ValueError(f"a form is an object with regs, stack, ret?, result? (extra {sorted(extra)})")
        return cls(d.get("regs"), d.get("stack", ()), d.get("ret"), d.get("result"))


class Fn:
    """One 1.14d function a directive calls: address, the arguments poke.py
    supplies by name, what EAX means (None: the form says), the spec it is
    cited from, the pc1-data item that asks for it, and its form (None = gap)."""
    __slots__ = ("addr", "args", "result", "cite", "ask", "form")

    def __init__(self, addr, args, result, cite, ask, form):
        self.addr, self.args, self.result = addr, tuple(args), result
        self.cite, self.ask, self.form = cite, ask, form


# One entry per function a directive calls. PC 1 replaces a `form=None` with a
# Form once the owning spec states it (README "Call forms"). Argument names are
# what poke.py supplies; a form places each exactly once (or lists literals).
CALL_FORMS = {
    "alloc": Fn(ALLOC, ("type", "class", "x", "y", "game", "room", "flags", "mode", "guid"), "unit",
                "sim/units.md §3.1; world/objects-2.md §22 r4", None,
                Form({"ecx": "type", "edx": "class"}, ["x", "y", "game", "room", "flags", "mode", "guid"])),
    "spawn": Fn(SPAWN, ("game", "room", "x", "y", "class", "mode", "spread", "flags"), "unit",
                "original-hooks-spawn.md §1 entry 1", None,
                Form({"ecx": "game", "edx": "room"}, ["x", "y", "class", "mode", "spread", "flags"], 0x18)),
    "random_boss": Fn(BOSS, ("game", "room", "cl", "class", "champion", "x", "y", "warp"), "unit",
                      "original-hooks-spawn.md §1 entry 3; monsters/init.md §25.1", None,
                      Form({"ecx": "game", "edx": "room"},
                           ["cl", "class", "champion", "x", "y", "warp"], 0x18)),
    "superunique": Fn(SUPERUNIQUE, ("game", "room", "x", "y", "row"), "unit",
                      "original-hooks-spawn.md §1 entry 4", None,
                      Form({"ecx": "game", "edx": "room"}, ["x", "y", "row"], 0xC)),
    "champion_mark": Fn(CHAMPION_MARK, ("game", "unit", "umod"), "none",
                        "original-hooks-spawn.md §1 entry 5; monsters/init.md §16.2, §25.1", None,
                        Form({"ecx": "game", "edx": "unit"}, ["umod"], 4)),
    "room_at": Fn(ROOM_AT, ("room", "x", "y"), "unit", "original-hooks-spawn.md §1 entry 6", None,
                  Form({"ecx": "room", "edx": "x"}, ["y"], 4)),
    "missile": Fn(MISSILE, ("game", "record"), "unit", "original-hooks.md §7.1", None,
                  Form({"ecx": "game", "edx": "record"}, [])),
    "boss_spawn": Fn(0x005A09E0, ("game", "class", "room", "cl", "x", "y", "guid", "warp"), "unit",
                     "original-hooks-spawn.md §1 rule 1; monsters/init.md §25.1", "22 (e)",
                     Form({"edi": "game", "ebx": "class"}, ["room", "cl", "x", "y", "guid", "warp"], 0x18)),
    "champion_minions": Fn(0x0054E1E0, ("boss", "game", "cl", "class"), "none",
                           "monsters/init.md §25.1; monsters/population.md §6.4", "22 (e)",
                           Form({"esi": "boss", "edi": "game"}, ["cl", "class"], 8)),
    "send": Fn(SEND, ("size", "message"), "none", "sim/intents-events.md §2.1 rule 1", None,
               Form({"edi": "size"}, ["message"])),
    "boss_minions": Fn(0x005A2120, ("min", "cl", "max", "game", "unit", "minions"), "none",
                       "monsters/init.md §18, §25.1", "22 (e)",
                       Form({"ecx": "min", "edx": "cl", "eax": "max"}, ["game", "unit", "minions"], 0xC)),
    # pc1-data.md Step 4 item 22 (a)-(d) (poke.md Open questions 1-4), forms from the asm
    "teleport": Fn(0x00650BE0, ("path", "unit", "room", "x", "y"), "bool",
                   "sim/path-placement.md §6 r4", "22 (a)",
                   Form({}, ["path", "unit", "room", "x", "y"], 0x14)),
    "place": Fn(0x00554EA0, ("game", "unit", "room", "x", "y", "exact", "alt"), "bool",
                "sim/path-placement.md §10", "22 (a)",
                Form({"ecx": "game", "edx": "unit"}, ["room", "x", "y", "exact", "alt"], 0x14)),
    "warp": Fn(0x0053AEC0, ("game", "player", "level", "tile"), None,
               "world/waypoints.md §7 r5", "22 (b)",
               Form({"ecx": "game", "edx": "player"}, ["level", "tile"], 8, "none")),
    "item_create": Fn(0x00558D90, ("game", "request", "use_seed"), "unit",
                      "items/generation.md §3; world/objects-2.md §20.7", "22 (c)",
                      Form({"ecx": "game", "edx": "request"}, ["use_seed"], 4)),
    "stat_set": Fn(0x00627260, ("unit", "stat", "value", "layer"), "none",
                   "sim/stat-lists.md §5 r2", "22 (d)",
                   Form({}, ["unit", "stat", "value", "layer"], 0x10)),
    "state_set": Fn(0x00639DB0, ("unit", "state", "on"), "none",
                    "sim/stat-lists.md §9.2", "22 (d)",
                    Form({}, ["unit", "state", "on"], 0xC)),
}

OPEN_QUESTION = {"teleport": 1, "place": 1, "warp": 2, "item_create": 3, "stat_set": 4,
                 "state_set": 4}


def check_form(name, fn, form):
    """Raise ValueError when `form` cannot describe `fn`'s call."""
    if not isinstance(form, Form):
        raise ValueError(f"{name}: not a Form")
    used = []
    for r, v in form.regs.items():
        if r not in REGISTERS:
            raise ValueError(f"{name}: register {r!r} is not one of {', '.join(REGISTERS)}")
        used.append(v)
    used += form.stack
    names = [v for v in used if not isinstance(v, int)]
    for v in used:
        if isinstance(v, bool) or not isinstance(v, (int, str)):
            raise ValueError(f"{name}: {v!r} is neither an argument name nor an int")
        if isinstance(v, str) and v not in fn.args:
            raise ValueError(f"{name}: unknown argument {v!r} (poke.py supplies {', '.join(fn.args)})")
    for a in fn.args:
        if names.count(a) != 1:
            raise ValueError(f"{name}: argument {a!r} placed {names.count(a)} times (once expected)")
    if form.ret not in (None, 0, 4 * len(form.stack)):
        raise ValueError(f"{name}: ret {form.ret:#x} but {len(form.stack)} stack argument(s)")
    if form.result is not None and form.result not in RESULTS:
        raise ValueError(f"{name}: result {form.result!r} is not one of {', '.join(RESULTS)}")


for _k, _f in CALL_FORMS.items():
    if _f.form is not None:
        check_form(_k, _f, _f.form)


def load_forms(obj, forms=None, fields=None):
    """(forms, fields): copies of CALL_FORMS / FIELDS with a --forms object applied
    ({"format": "poke-forms-1", "forms": {name: form or null}, "fields": {name: int or null}})."""
    forms = {k: Fn(f.addr, f.args, f.result, f.cite, f.ask, f.form)
             for k, f in (forms or CALL_FORMS).items()}
    fields = dict(fields or FIELDS)
    if not isinstance(obj, dict) or obj.get("format") != FORMS_FORMAT:
        raise ValueError(f'expected an object with "format": "{FORMS_FORMAT}"')
    extra = set(obj) - {"format", "forms", "fields"}
    if extra:
        raise ValueError(f"unknown key(s) {sorted(extra)}")
    for k, v in (obj.get("forms") or {}).items():
        if k not in forms:
            raise ValueError(f"forms: unknown function {k!r} (one of {', '.join(forms)})")
        if v is None:
            forms[k].form = None
            continue
        f = Form.from_json(v)
        check_form(k, forms[k], f)
        forms[k].form = f
    for k, v in (obj.get("fields") or {}).items():
        if k not in fields:
            raise ValueError(f"fields: unknown field {k!r} (one of {', '.join(fields)})")
        if v is not None and (isinstance(v, bool) or not isinstance(v, int) or v < 0):
            raise ValueError(f"fields: {k} = {v!r} is not a non-negative int or null")
        fields[k] = v
    return forms, fields


def build(name, values, forms=None):
    """(regs {register: value}, stack [values]) of one call of CALL_FORMS[name];
    Gap when its form is None."""
    fn = (forms or CALL_FORMS)[name]
    if fn.form is None:
        raise Gap(gap_note([name], forms))
    missing = set(fn.args) - set(values)
    if missing:
        raise AssertionError(f"{name}: no value for {sorted(missing)}")

    def val(v):
        return (v if isinstance(v, int) else values[v]) & U32
    return ({r: val(v) for r, v in fn.form.regs.items()}, [val(v) for v in fn.form.stack])


def result_of(name, forms=None):
    fn = (forms or CALL_FORMS)[name]
    return fn.result or (fn.form.result if fn.form else None) or "none"


def gap_note(names, forms=None, fields=()):
    forms = forms or CALL_FORMS
    parts = []
    for n in names:
        f = forms[n]
        oq = f" / poke.md Open question {OPEN_QUESTION[n]}" if n in OPEN_QUESTION else ""
        parts.append(f"{n} {f.addr:#010x} ({f.cite}; pc1-data.md Step 4 item {f.ask or '-'}{oq})")
    note = "no 1.14d call form: " + ", ".join(parts) if parts else ""
    if fields:
        note += ("; " if note else "") + "no field value: " + ", ".join(fields)
    return note


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
    "hop": [("unit", "ref"), ("x", "pos"), ("y", "pos")],
    "warp": [_n("level")],
    "item": [("code", "code"), ("x", "pos"), ("y", "pos")],
    "stat": [("unit", "ref"), _n("stat", 0, 0xFFFF), _n("layer", 0, 0xFFFF), ("value", "i32")],
    "state": [("unit", "ref"), _n("state", 0, 0xFFFF), ("on", "onoff")],
    "freeze": [_n("seconds", 0, 3600)],
}
OPTIONAL = {  # keyword -> its argument kinds, in the table (canonical) order
    "object": {"mode": [_n("mode")]},
    "missile": {"skill": [_n("skill"), _n("level")], "owner": [("owner", "ref")]},
    "pos": {"free": []},  # a flag: land on the nearest LAND_MASK-free cell (poke.md §1)
    "warp": {"tile": [_n("tile")]},
    "item": {"quality": [("quality", "enum", QUALITIES)], "ilvl": [_n("ilvl", 1, 99)]},
}


def needs(d, a):
    """The CALL_FORMS entries a directive calls, and the FIELDS it reads
    (poke.md §4 rule 8). `pos` needs one of POS_VIA: listed as a tuple."""
    room = ["room_at"]
    if d in ("seed-game", "seed-unit", "time", "freeze"):
        return [], []
    if d == "missile":
        return ["missile"], []
    if d == "msg":
        return ["send"], []
    if d == "object":
        return room + ["alloc"], []
    if d == "superunique":
        return room + ["superunique"], []
    if d in ("pos", "hop"):
        return room + [POS_VIA], []
    if d == "warp":
        return ["warp"], []
    if d == "goto":
        return (["warp"] if a["level"] is not None else []) + ["place"], []
    if d == "item":
        return room + ["item_create"], ["item_format", "items_header", "item_record_size",
                                         "item_code"]
    if d == "stat":
        return ["stat_set"], []
    if d == "state":
        return ["state_set"], []
    if d == "spawn":
        return room + {"normal": ["spawn"],
                       "random-boss": ["random_boss", "champion_minions"],
                       "champion": ["boss_spawn", "champion_mark", "champion_minions"],
                       "unique": ["boss_spawn", "boss_minions"]}[a["kind"]], \
            (["mon_data", "mon_umods"] if a["kind"] == "unique" else [])
    raise AssertionError(d)


def missing(d, a, forms=None, fields=None):
    """(functions without a form, fields without a value) of one directive."""
    forms, fields = forms or CALL_FORMS, fields or FIELDS
    fns, flds = needs(d, a)
    out = []
    for n in fns:
        if isinstance(n, tuple):
            if all(forms[x].form is None for x in n):
                out += list(n)
        elif forms[n].form is None:
            out.append(n)
    return out, [f for f in flds if fields.get(f) is None]


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
    if d == "goto":
        return d, _parse_goto(rest, line)
    if d == "msg":
        return d, _parse_msg(rest, line)
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
        if not need:
            args[kw] = True
            rest = rest[1:]
            continue
        if len(rest) < 1 + len(need):
            raise PokeError(line, f"{d} {kw}: missing argument {need[len(rest) - 1][0]}")
        for s, t in zip(need, rest[1:1 + len(need)]):
            args[s[0]] = _arg(s, t, line, d)
        rest = rest[1 + len(need):]
    if d == "object":
        args.setdefault("mode", 0)  # default mode 0 (poke.md §1)
    return d, args


def _parse_goto(rest, line):
    """goto unit [<type>:]<class> | goto preset <level> [<type>:]<class> (poke.md §1, §6)."""
    if len(rest) == 2 and rest[0] == "unit":
        level, tok = None, rest[1]
    elif len(rest) == 3 and rest[0] == "preset":
        level, tok = parse_num(rest[1], 0, 0xFFFF, line, "goto level"), rest[2]
    else:
        raise PokeError(line, "goto unit [<type>:]<class> | goto preset <level> [<type>:]<class>")
    ty, _, cl = tok.rpartition(":")
    return {"level": level, "type": parse_num(ty, 1, 2, line, "goto unit type (1 monster, 2 object)") if ty else 1,
            "class": parse_num(cl, 0, 0xFFFF, line, "goto class")}
# --- msg: C→S layouts (specs/sim/client-messages.tsv; poke.md §1 `msg`) --------

CLIENT_TSV = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "specs", "sim",
                          "client-messages.tsv")
FIELD_RE = re.compile(r"(\w+):(u8|u16|u32|u[0-9]+|bit[0-9]+)@(0x[0-9A-Fa-f]+|[0-9]+)\Z")
_LAYOUTS = None


def _layout_of(row):
    """(name, size, [(field, type, offset)]) of one TSV row, or the reason `msg`
    cannot write it: a fixed size and integer fields only (unlisted bytes are written 0)."""
    c = row.split("\t")
    mid, name = int(c[0], 16), c[1]
    if not c[2].isdigit():
        return f"msg {mid:#04x} ({name}) has no fixed size"
    size = int(c[2])
    if size == 0:
        return f"msg {mid:#04x} ({name}) has no fixed size"
    fields = []
    for tok in c[4].split():
        m = FIELD_RE.match(tok)
        if not m:
            return f"msg {mid:#04x} ({name}): field {tok.split(':')[0].split('@')[0]} is not an integer"
        f, t, off = m.group(1), m.group(2), int(m.group(3), 0)
        if t.startswith("bit"):
            b = int(t[3:])
            span = range(off + b // 8, off + b // 8 + 1)
        else:
            span = range(off, off + {"u8": 1, "u16": 2, "u32": 4}.get(t, (int(t[1:]) + 7) // 8))
        if span.stop > size:
            return f"msg {mid:#04x} ({name}): field {f} has no offset in the message"
        fields.append((f, t, off))
    return name, size, fields


def msg_layouts():
    """{id: layout or reason} for ids 0x01..0x70 (read once from the TSV)."""
    global _LAYOUTS
    if _LAYOUTS is None:
        rows = open(CLIENT_TSV, encoding="utf-8").read().split("\n")[1:]
        _LAYOUTS = {}
        for r in rows:
            if r:
                mid = int(r.split("\t")[0], 16)
                if 0x01 <= mid <= 0x70:
                    _LAYOUTS[mid] = _layout_of(r)
    return _LAYOUTS


def msg_layout(mid):
    """(name, size, fields) of id `mid`, or ValueError naming why `msg` cannot write it."""
    lay = msg_layouts().get(mid)
    if lay is None:
        raise ValueError(f"msg id {mid:#04x}: a C→S id 0x01..0x70")
    if isinstance(lay, str):
        raise ValueError(lay)
    return lay


def field_max(t):
    if t.startswith("bit"):
        return 1
    return {"u8": 0xFF, "u16": 0xFFFF, "u32": U32}.get(t) or (1 << int(t[1:])) - 1


def encode_msg(mid, values):
    """The message bytes: id, then each field little-endian at its offset (`uN`/`bitN`
    into the u32 at the offset); ValueError on a wrong count or a value too big."""
    name, size, fields = msg_layout(mid)
    if len(values) != len(fields):
        raise ValueError(f"msg {mid:#04x} ({name}) takes {len(fields)} value(s), got {len(values)}")
    b = bytearray(size)
    b[0] = mid
    for (f, t, off), v in zip(fields, values):
        if not 0 <= v <= field_max(t):
            raise ValueError(f"{v} does not fit field {f}")
        if t in ("u8", "u16", "u32"):
            w = {"u8": 1, "u16": 2, "u32": 4}[t]
            b[off:off + w] = v.to_bytes(w, "little")
        else:
            word = v << (int(t[3:]) if t.startswith("bit") else 0)
            for k, x in enumerate(word.to_bytes(4, "little")):
                if x:
                    b[off + k] |= x
    return bytes(b)


def _parse_msg(rest, line):
    """msg <id> <value>... (poke.md §1 `msg`): the id's fields in layout order."""
    if not rest:
        raise PokeError(line, "msg: missing argument id")
    mid = parse_num(rest[0], 1, 0x70, line, "msg id")
    try:
        name, _, fields = msg_layout(mid)
    except ValueError as e:
        raise PokeError(line, str(e)) from None
    vals = rest[1:]
    if len(vals) != len(fields):
        raise PokeError(line, f"msg {mid:#04x} ({name}) takes {len(fields)} value(s) "
                              f"({' '.join(f[0] for f in fields)}), got {len(vals)}")
    out = []
    for (f, t, _), tok in zip(fields, vals):
        what = f"msg field {f}"
        if tok.startswith(("@x", "@y")):
            if not POS_RE.match(tok):
                raise PokeError(line, f"{what}: {tok!r} is not a position (@x, @y, @x±N, @y±N)")
            out.append(parse_pos(tok, line, what))
        elif tok.startswith("@") or "/" in tok:
            out.append(parse_ref(tok, line, what))
        else:
            out.append(parse_num(tok, 0, field_max(t), line, what))
    return {"id": mid, "values": out}


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
    if d == "goto":
        t = f"{args['type']}:{args['class']}"
        return f"goto unit {t}" if args["level"] is None else f"goto preset {args['level']} {t}"
    if d == "spawn":
        s = f"spawn {args['class']} {_fmt(args['x'])} {_fmt(args['y'])} {args['kind']}"
        return s + ("" if not args["umods"] else " umod " + " ".join(map(str, args["umods"])))
    if d == "msg":
        return " ".join(["msg", str(args["id"])] + [_fmt(v) for v in args["values"]])
    out = [d] + [_fmt(args[s[0]]) for s in POSITIONAL[d]]
    for kw, specs in OPTIONAL.get(d, {}).items():
        if not specs:
            if args.get(kw):
                out.append(kw)
        elif specs[0][0] in args:
            out += [kw] + [_fmt(args[s[0]]) for s in specs]
    return " ".join(out)


# --- pure builders: call layouts and the missile record ----------------------

def layout(name, forms=None, **values):
    """(regs, stack) of one call: build() under a name of CALL_FORMS."""
    return build(name, values, forms)


def item_request(values):
    """The 0x84-byte item request (items/generation.md Inputs; poke.md §1 `item`):
    the ITEM_REQUEST fields from `values`, every other byte 0."""
    b = bytearray(ITEM_REQ_SIZE)
    for k, (off, w) in ITEM_REQUEST.items():
        v = values.get(k, 0)
        b[off:off + w] = (v & ((1 << 8 * w) - 1)).to_bytes(w, "little")
    return bytes(b)


def item_index(mem, code, fields=None):
    """Combined items index of a code (3-4 chars), or -1: the first record whose
    u32 code (+0x80, space-padded) matches, in the combined array of
    items/treasure.md §9.1 (count +0x00, records +0x04; data/loading.md §6, §9)."""
    fields = fields or FIELDS
    hdr = fields["items_header"]
    count, recs = mem.read_u32(hdr), mem.read_u32(hdr + 4)
    if not recs or not 0 < count < 0x10000:
        return -1
    key = code.encode("ascii").ljust(4, b" ")
    size, off = fields["item_record_size"], fields["item_code"]
    block = mem.read(recs, count * size)
    for i in range(count):
        if block[i * size + off:i * size + off + 4] == key:
            return i
    return -1


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


def unit_room_pos(mem, unit):
    """(room, x, y) of any unit: dynamic path for types 0, 1, 3, static path
    otherwise (tools/state-snapshot.md §2), or None without a path."""
    path = mem.read_u32(unit + U_PATH)
    if not path:
        return None
    if mem.read_u32(unit + U_TYPE) in DYNAMIC_PATH_TYPES:
        return mem.read_u32(path + P_ROOM), u16(mem, path + P_X), u16(mem, path + P_Y)
    return mem.read_u32(path + SP_ROOM), mem.read_u32(path + SP_X), mem.read_u32(path + SP_Y)


def drlg_level(mem, dr):
    """Level id of a DRLG room (+0x58 level, +0x1D0 id), or None."""
    lv = mem.read_u32(dr + DR_LEVEL) if dr else 0
    return mem.read_u32(lv + L_ID) if lv else None


def room_level(mem, room):
    """Level id of an active room, or None."""
    return drlg_level(mem, mem.read_u32(room + AR_DRLG)) if room else None


def drlg_near(mem, dr):
    """The DRLG rooms of dr's near array, in stored order."""
    arr, n = mem.read_u32(dr + DR_NEAR), mem.read_u32(dr + DR_NEAR_N)
    return [mem.read_u32(arr + 4 * i) for i in range(min(n, 1024))] if arr else []


def goto_hop(mem, dr, goal, seen, blocked=frozenset()):
    """poke.md §6 rule 3.3: marks dr and its near rooms with an active room as
    seen, then a breadth-first search over near arrays from dr for the first
    room of level `goal` not seen, never entering a blocked room. Returns
    (hop DRLG room, its key) or None."""
    def key(r):
        return (drlg_level(mem, r), struct.unpack("<i", mem.read(r + DR_TX, 4))[0],
                struct.unpack("<i", mem.read(r + DR_TY, 4))[0])
    seen.add(key(dr))
    for n in drlg_near(mem, dr):
        if mem.read_u32(n + DR_ACTIVE):
            seen.add(key(n))
    parent, queue, done = {}, [dr], {dr}
    target = None
    while queue:
        r = queue.pop(0)
        k = key(r)
        if k[0] == goal and k not in seen:
            target = r
            break
        for n in drlg_near(mem, r):
            if n and n not in done and key(n) not in blocked:
                done.add(n)
                parent[n] = r
                queue.append(n)
    if target is None:
        return None
    h = target
    while parent.get(h) not in (None, dr):
        h = parent[h]
    return h, key(h)


def free_cell(mem, ar, target=None):
    """poke.md §6 rule 3.3 / §1 `pos`: the cell of active room `ar` whose collision
    mask has none of LAND_MASK, nearest `target` (default the centre of its
    sub-tile rect; squared distance; row by row from the top-left, first found on
    a tie), or None."""
    x, y, wd, ht = struct.unpack("<iiii", mem.read(ar + AR_SUB, 16))
    cx, cy = target if target else (x + wd // 2, y + ht // 2)
    rec_ = mem.read_u32(ar + AR_COLL)
    if not rec_:
        return None
    x0, y0, cw, ch = struct.unpack("<iiii", mem.read(rec_ + CR_X, 16))
    masks = mem.read_u32(rec_ + CR_MASKS)
    if not masks or cw <= 0 or ch <= 0:
        return None
    grid = mem.read(masks, 2 * cw * ch)
    best = None
    for yy in range(y, y + ht):
        for xx in range(x, x + wd):
            if not (x0 <= xx < x0 + cw and y0 <= yy < y0 + ch):
                continue
            m = struct.unpack_from("<H", grid, 2 * ((yy - y0) * cw + (xx - x0)))[0]
            d2 = (xx - cx) ** 2 + (yy - cy) ** 2
            if not m & LAND_MASK and (best is None or d2 < best[0]):
                best = (d2, xx, yy)
    return best[1:] if best else None


def mask_at(mem, ar, x, y):
    """The collision mask at sub-tile (x, y) in active room `ar`'s record, or None
    (no record, or outside it)."""
    rec_ = mem.read_u32(ar + AR_COLL) if ar else 0
    if not rec_:
        return None
    x0, y0, cw, ch = struct.unpack("<iiii", mem.read(rec_ + CR_X, 16))
    masks = mem.read_u32(rec_ + CR_MASKS)
    if not masks or not (x0 <= x < x0 + cw and y0 <= y < y0 + ch):
        return None
    return struct.unpack("<H", mem.read(masks + 2 * ((y - y0) * cw + (x - x0)), 2))[0]


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


def msg_values(mem, game, values):
    """The `msg` values resolved: a unit to its GUID (+0x0C), a position to the
    player's x / y ± N; Unresolved when a reference matches nothing."""
    out = []
    for v in values:
        if isinstance(v, Ref):
            out.append(mem.read_u32(resolve_unit(mem, game, v) + U_GUID))
        elif isinstance(v, Pos):
            out.append(resolve_pos(mem, game, v))
        else:
            out.append(v)
    return out


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


def set_regs(ctx, regs):
    """Write {register: value} into a WOW64_CONTEXT (any of REGISTERS)."""
    for r, v in regs.items():
        if r not in REGISTERS:
            raise ValueError(f"register {r!r}")
        setattr(ctx, r.capitalize(), v & U32)


class PokeLayer:
    """Holds the directives and makes the calls. Relative steps (a poke file)
    run at the stop whose game +0xA8 = F0 + t; absolute steps (--poke) at the
    stop whose +0xA8 = f - 1."""

    def __init__(self, rel=(), absolute=(), start_frame=None, source=None, forms=None,
                 fields=None, forms_source=None):
        self.rel, self.abs = list(rel), list(absolute)
        self.forms = forms or CALL_FORMS    # call forms (poke.md §4 rule 8); --forms overrides
        self.fields = fields or FIELDS
        self.forms_source = forms_source    # (path, sha256) of a --forms file
        self.item_codes = {}                # code -> combined index (read once per run)
        self.f0 = start_frame
        self.f0_how = "--start-frame" if start_frame is not None else None
        self.source = source            # (path, sha256) of the poke file
        self.scratch = None
        self.done = set()               # id(step)
        self.walking = []               # (step, walk state) of goto walks still stepping (poke.md §6)
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
        forms = fields = fsrc = None
        if getattr(a, "forms", None):
            data = open(a.forms, "rb").read()
            try:
                import json
                forms, fields = load_forms(json.loads(data.decode("utf-8")))
            except ValueError as e:
                raise SystemExit(f"{a.forms}: {e}")
            fsrc = (a.forms, hashlib.sha256(data).hexdigest())
        return cls(rel, absolute, getattr(a, "start_frame", None), src, forms, fields, fsrc)

    def pending(self):
        return [s for s in self.abs + self.rel if id(s) not in self.done] + [s for s, _ in self.walking]

    def finish(self, rec, frame):
        """End of run: each goto still walking is written failed (poke.md §6 rule 4)."""
        for i, (s, w) in enumerate(self.walking):
            self._emit(rec, s, frame, i, {"r": "failed", "note": "not finished", "steps": w["steps"]})
        self.walking = []

    def header(self):
        return {"k": "poke_file", "path": self.source[0] if self.source else None,
                "sha256": self.source[1] if self.source else None, "start_frame": self.f0,
                "forms_file": self.forms_source[0] if self.forms_source else None,
                "forms_sha256": self.forms_source[1] if self.forms_source else None,
                "forms": {k: f.form.to_json() for k, f in self.forms.items() if f.form is not None},
                "fields": self.fields,
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
        if due or self.walking:
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
        walking, self.walking = self.walking, []
        items = walking + [(s, None) for s in steps]  # goto walks step first (poke.md §6)
        try:
            for i, (s, w) in enumerate(items):
                self.done.add(id(s))
                if s.d == "goto":
                    w = w if w is not None else {"goal": None, "seen": set(), "blocked": set(), "steps": 0}
                    r = self.apply(rec, game, tid, saved, s, w)
                    if r.get("r") == "pending":
                        self.walking.append((s, w))
                        continue
                    r["steps"] = w["steps"]
                else:
                    r = self.apply(rec, game, tid, saved, s)
                out.append(self._emit(rec, s, frame, i, r))
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
    def apply(self, rec, game, tid, saved, s, walk=None):
        d, a = s.d, s.args
        fns, flds = missing(d, a, self.forms, self.fields)
        if fns or flds:  # poke.md §4 rules 7-8: a function without a form is not called
            return {"r": "gap", "note": gap_note(fns, self.forms, flds)}
        if d == "msg":
            return self._msg(rec, game, tid, saved, a)
        try:
            args, ptrs = resolve_args(rec, game, a)
        except Gap as e:
            return {"r": "gap", "note": str(e)}
        except Unresolved as e:
            return {"r": "unresolved", "note": str(e)}
        r = {"args": args}
        try:
            if d == "goto":
                r.update(self._goto(rec, game, tid, saved, args, walk if walk is not None else
                                    {"goal": None, "seen": set(), "blocked": set(), "steps": 0}))
            else:
                r.update(self._apply(rec, game, tid, saved, d, args, ptrs))
        except Unresolved as e:
            r.update({"r": "unresolved", "note": str(e)})
        except CallFault as e:
            r.update({"r": "failed", "note": str(e)})
        return r

    def _msg(self, rec, game, tid, saved, a):
        """`msg`: the bytes (references resolved now) through the client sender
        0x00478350 (sim/intents-events.md §2.1 rule 1; poke.md §1 `msg`). Its
        duplicate filter is not visible here: the result is `ok` either way."""
        try:
            values = msg_values(rec, game, a["values"])
            data = encode_msg(a["id"], values)
        except Gap as e:
            return {"r": "gap", "note": str(e)}
        except (Unresolved, ValueError) as e:
            return {"r": "unresolved", "note": str(e)}
        S = self.page(rec)
        rec.write(S + SCRATCH_MSG, data)
        r = {"args": {"id": a["id"], "values": values}, "bytes": data.hex()}
        try:
            eax = self.invoke(rec, tid, saved, "send", size=len(data), message=S + SCRATCH_MSG)
        except CallFault as e:
            r.update({"r": "failed", "note": str(e)})
            return r
        r.update(self._result(rec, "send", eax))
        return r

    def _created(self, rec, eax):
        if not eax:
            return {"r": "failed", "eax": 0}
        return {"r": "ok", "eax": f"{eax:#x}", "guid": rec.read_u32(eax + U_GUID)}

    def _result(self, rec, name, eax):
        """The result of a call by what its EAX means (CALL_FORMS result)."""
        how = result_of(name, self.forms)
        if how == "unit":
            return self._created(rec, eax)
        if how == "bool":
            return {"r": "ok" if eax else "failed", "eax": f"{eax:#x}"}
        return {"r": "ok", "eax": f"{eax:#x}"}

    def invoke(self, rec, tid, saved, name, **values):
        """Call CALL_FORMS[name] with named argument values; returns EAX."""
        regs, stack = build(name, values, self.forms)
        return self.call(rec, tid, saved, self.forms[name].addr, regs, stack)

    def _pos(self, rec, game, tid, saved, u, path, x, y):
        """One `pos` of a unit with a dynamic path (path-placement.md §6 r4 / §10)."""
        room = self._room(rec, game, tid, saved, x, y, rec.read_u32(path + P_ROOM))
        if not room:
            return {"r": "failed", "note": "no loaded room holds the point (entry 6 returned 0)"}
        via = next(n for n in POS_VIA if self.forms[n].form is not None)
        if via == "teleport":
            eax = self.invoke(rec, tid, saved, "teleport", path=path, unit=u, room=room, x=x, y=y)
        else:
            eax = self.invoke(rec, tid, saved, "place", game=game, unit=u, room=room, x=x, y=y,
                              exact=POS_EXACT, alt=POS_ALT)
        r = self._result(rec, via, eax)
        r["via"] = via
        return r

    def _room(self, rec, game, tid, saved, x, y, from_room=None):
        """Room holding (x, y): entry 6 from the player's room (poke.md §4 rule 5),
        or from `from_room`."""
        if from_room is None:
            pl = player_of(rec, game)
            p = player_pos(rec, pl) if pl else None
            if not p or not p[2]:
                raise Unresolved("no player room")
            from_room = p[2]
        return self.invoke(rec, tid, saved, "room_at", room=from_room, x=x, y=y)

    def _goto(self, rec, game, tid, saved, a, w):
        """One step of a goto walk (poke.md §6 rule 3); w is the walk state."""
        w["steps"] += 1
        if w["steps"] > GOTO_MAX_STEPS:
            return {"r": "failed", "note": "step limit"}
        pl = player_of(rec, game)
        p = player_pos(rec, pl) if pl else None
        if not p or not p[2]:
            raise Unresolved("@player has no room")
        here = room_level(rec, p[2])
        if w["goal"] is None:
            w["goal"] = a["level"] if a["level"] is not None else here
        goal = w["goal"]
        if w["steps"] == 1 and here != goal:  # rule 3.1: the warp, first step only
            eax = self.invoke(rec, tid, saved, "warp", game=game, player=pl, level=goal, tile=0)
            r = self._result(rec, "warp", eax)
            return {"r": "pending"} if r["r"] == "ok" else dict(r, note="warp refused")
        for guid, cls, u in units_of(rec, game, a["type"]):  # rule 3.2: ascending GUID
            if cls != a["class"]:
                continue
            rp = unit_room_pos(rec, u)
            if not rp or room_level(rec, rp[0]) != goal:
                continue
            eax = self.invoke(rec, tid, saved, "place", game=game, unit=pl, room=rp[0], x=rp[1], y=rp[2],
                              exact=GOTO_EXACT, alt=GOTO_ALT)
            if eax:
                self._settle(rec, tid, saved, game, pl)
            if eax:
                self._settle(rec, tid, saved, game, pl)
            return {"r": "ok", "guid": guid, "eax": f"{eax:#x}"} if eax else {"r": "failed", "eax": "0x0"}
        hop = goto_hop(rec, rec.read_u32(p[2] + AR_DRLG), goal, w["seen"], w.setdefault("blocked", set()))
        if hop is None:
            return {"r": "failed", "note": f"explored, {len(w['seen'])} rooms seen"}
        h, hkey = hop
        ar = rec.read_u32(h + DR_ACTIVE)
        if not ar:
            return {"r": "failed", "note": "the next room has no active room"}
        cell = free_cell(rec, ar)
        if cell is None:  # no free cell in H: seen and blocked
            w["seen"].add(hkey)
            w["blocked"].add(hkey)
            return {"r": "pending"}
        placed = self.invoke(rec, tid, saved, "place", game=game, unit=pl, room=ar, x=cell[0],
                             y=cell[1], exact=GOTO_EXACT, alt=GOTO_ALT)
        now = player_pos(rec, pl)
        if not placed or not now or now[2] != ar:  # refused, or landed outside H: seen and blocked
            w["seen"].add(hkey)
            w["blocked"].add(hkey)
        return {"r": "pending"}

    def _settle(self, rec, tid, saved, game, pl):
        """poke.md §6 rule 3.2: when the player's cell has a LAND_MASK bit, place
        again, exact 1, on the nearest free cell of its room."""
        now = player_pos(rec, pl)
        if not now or not now[2]:
            return
        m = mask_at(rec, now[2], now[0], now[1])
        if m is not None and not m & LAND_MASK:
            return
        cell = free_cell(rec, now[2], (now[0], now[1]))
        if cell and cell != (now[0], now[1]):
            self.invoke(rec, tid, saved, "place", game=game, unit=pl, room=now[2], x=cell[0],
                        y=cell[1], exact=1, alt=GOTO_ALT)

    def _item_index(self, rec, code):
        if code not in self.item_codes:
            self.item_codes[code] = item_index(rec, code, self.fields)
        return self.item_codes[code]

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
            eax = self.invoke(rec, tid, saved, "missile", game=game, record=S + SCRATCH_RECORD)
            return self._created(rec, eax)
        if d == "warp":       # 0x0053AEC0(game, player, level, tile) (waypoints.md §7 r5)
            pl = player_of(rec, game)
            if not pl:
                raise Unresolved("@player matches no unit")
            eax = self.invoke(rec, tid, saved, "warp", game=game, player=pl, level=a["level"],
                              tile=a.get("tile", 0))
            return self._result(rec, "warp", eax)
        if d == "stat":       # 0x00627260: set on unit +0x5C (stat-lists.md §5 r2)
            u = ptrs["unit"]
            if not rec.read_u32(u + U_STATS):
                return {"r": "failed", "note": "the unit has no stat list (+0x5C)"}
            eax = self.invoke(rec, tid, saved, "stat_set", unit=u, stat=a["stat"],
                              value=a["value"], layer=a["layer"])
            return self._result(rec, "stat_set", eax)
        if d == "state":      # 0x00639DB0: toggle, then the update-queue insert (stat-lists.md §9.2)
            eax = self.invoke(rec, tid, saved, "state_set", unit=ptrs["unit"], state=a["state"],
                              on=1 if a["on"] else 0)
            return self._result(rec, "state_set", eax)
        if d in ("pos", "hop"):  # path-placement.md §6 r4 / §10
            u = ptrs["unit"]
            if rec.read_u32(u + U_TYPE) not in DYNAMIC_PATH_TYPES:
                return {"r": "failed", "note": "not a unit with a dynamic path (path-placement.md §10 r1)"}
            path = rec.read_u32(u + U_PATH)
            if not path:
                return {"r": "failed", "note": "the unit has no path"}
            if d == "pos" and a.get("free"):
                # the nearest LAND_MASK-free cell of the room holding the point
                room = self._room(rec, game, tid, saved, a["x"], a["y"], rec.read_u32(path + P_ROOM))
                cell = free_cell(rec, room, (a["x"], a["y"])) if room else None
                if cell is None:
                    return {"r": "failed", "note": "no free missile-passable cell in the room"}
                return self._pos(rec, game, tid, saved, u, path, *cell)
            if d == "pos":
                return self._pos(rec, game, tid, saved, u, path, a["x"], a["y"])
            # hop (poke.md §1): the first spot of hop_candidates the unit moves to
            here = lambda: struct.unpack("<HH", rec.read(path + P_X, 2) + rec.read(path + P_Y, 2))  # noqa: E731
            start = here()
            dx, dy = a["x"] - start[0], a["y"] - start[1]
            if abs(dx) <= 1 and abs(dy) <= 1:
                return {"r": "ok", "note": "already there"}
            step = (max(-HOP, min(HOP, dx)), max(-HOP, min(HOP, dy)))
            tries = 0
            for x, y in hop_candidates(start, step):
                tries += 1
                room = self._room(rec, game, tid, saved, x, y, rec.read_u32(path + P_ROOM))
                m = mask_at(rec, room, x, y) if room else None
                if m is None or m & LAND_MASK:  # only a free, missile-passable cell (LAND_MASK)
                    continue
                r = self._pos(rec, game, tid, saved, u, path, x, y)
                if here() != start:
                    r.update({"r": "ok", "to": [x, y], "tries": tries})
                    return r
            return {"r": "failed", "note": f"no free spot among {tries} candidates"}
        if d == "item":       # 0x00558D90(game, request, 0) (items/generation.md §3; objects-2.md §20.7)
            index = self._item_index(rec, a["code"])  # code first, then the room (as d2rs)
            if index < 0:
                return {"r": "failed", "note": f"no item record with code {a['code']!r}"}
            room = self._room(rec, game, tid, saved, a["x"], a["y"])
            if not room:
                return {"r": "failed", "note": "no loaded room holds the point (entry 6 returned 0)"}
            fmt = rec.read_u32(game + self.fields["item_format"]) & 0xFFFF
            q = a.get("quality")
            req = item_request({"unit": 0, "game": game, "ilvl": a.get("ilvl", ITEM_ILVL_DEFAULT),
                                "item": index, "mode": ITEM_MODE_GROUND, "x": a["x"], "y": a["y"],
                                "room": room, "init_flags": ITEM_INIT_FLAGS, "format": fmt,
                                "quality": QUALITIES.index(q) + 1 if q else 0})
            S = self.page(rec)
            rec.write(S + SCRATCH_ITEM, req)
            eax = self.invoke(rec, tid, saved, "item_create", game=game, request=S + SCRATCH_ITEM,
                              use_seed=0)
            return self._created(rec, eax)
        room = self._room(rec, game, tid, saved, a["x"], a["y"])
        if not room:
            return {"r": "failed", "note": "no loaded room holds the point (entry 6 returned 0)"}
        if d == "object":
            eax = self.invoke(rec, tid, saved, "alloc", type=OBJECT_TYPE, **{"class": a["class"]},
                              x=a["x"], y=a["y"], game=game, room=room, flags=OBJECT_FLAGS,
                              mode=a["mode"], guid=0)
            return self._created(rec, eax)
        if d == "superunique":
            eax = self.invoke(rec, tid, saved, "superunique", game=game, room=room, x=a["x"],
                              y=a["y"], row=a["row"])
            return self._created(rec, eax)
        if d == "spawn":      # scenario.md §3.1 rule 2: the call sequence of each kind
            kind, cls = a["kind"], a["class"]
            if kind == "normal":  # entry 1, flags 0 (original-hooks-spawn.md §1, §3 `normal`)
                eax = self.invoke(rec, tid, saved, "spawn", game=game, room=room, x=a["x"], y=a["y"],
                                  **{"class": cls}, mode=SPAWN_MODE, spread=SPAWN_SPREAD,
                                  flags=SPAWN_FLAGS)
                return self._created(rec, eax)
            if kind == "random-boss":  # 0x005A43E0 (no list, champion allowed, no warp check)
                boss = self.invoke(rec, tid, saved, "random_boss", game=game, room=room, cl=0,
                                   **{"class": cls}, champion=1, x=a["x"] & 0xFFFF,
                                   y=a["y"] & 0xFFFF, warp=0)
            else:                      # boss spawn 0x005A09E0 at (x, y), new GUID, no warp check
                boss = self.invoke(rec, tid, saved, "boss_spawn", game=game, **{"class": cls},
                                   room=room, cl=0, x=a["x"], y=a["y"], guid=BOSS_GUID_NEW, warp=0)
            r = self._created(rec, boss)
            if not boss:
                return r
            if kind == "champion":     # pack member mark (init.md §16.2)
                self.invoke(rec, tid, saved, "champion_mark", game=game, unit=boss,
                            umod=a["umods"][0] & 0xFF)
            if kind in ("random-boss", "champion"):  # champion minions (population.md §6.4)
                self.invoke(rec, tid, saved, "champion_minions", boss=boss, game=game, cl=0,
                            **{"class": cls})
            if kind == "unique":       # umods appended in order, then 0x005A2120 (init.md §25.3)
                self.append_umods(rec, boss, a["umods"])
                self.invoke(rec, tid, saved, "boss_minions", min=UNIQUE_MIN, cl=0, max=UNIQUE_MAX,
                            game=game, unit=boss, minions=1)
            return r
        raise AssertionError(d)

    def append_umods(self, rec, unit, umods):
        """Append umods to the monster's list (monster data +0x1C, 9 bytes, count =
        bytes before the first 0) while count < 9 (monsters/init.md §25.3 r1-r2)."""
        md = rec.read_u32(unit + self.fields["mon_data"])
        if not md:
            raise CallFault("the boss has no monster data")
        lst = md + self.fields["mon_umods"]
        cur = rec.read(lst, UMOD_MAX)
        n = next((i for i, b in enumerate(cur) if b == 0), UMOD_MAX)
        add = bytes(u & 0xFF for u in umods)[:UMOD_MAX - n]
        if add:
            rec.write(lst + n, add)

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

    def call(self, rec, tid, saved, entry, regs, args):
        """Call entry on the stopped thread; returns EAX (§5 rules 4-5). `regs`:
        {register: value} for any of REGISTERS; the others keep the saved values."""
        rr = _rr()
        S = self.page(rec)
        esp = saved.Esp - 4 * len(args) - 4
        rec.write(esp, struct.pack("<I", S + SCRATCH_TRAP) +
                  b"".join(struct.pack("<I", v & U32) for v in args))
        ctx = rr.WOW64_CONTEXT.from_buffer_copy(saved)
        ctx.ContextFlags = rr.WOW64_CONTEXT_FULL
        set_regs(ctx, regs)
        ctx.Esp, ctx.Eip = esp, entry
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


HOP = 16  # largest move of a `hop` per axis (poke.md §1; d2_sim::poke::HOP)
HOP_DIRS = ((1, 1), (1, 0), (0, 1), (-1, 0), (0, -1), (-1, -1), (1, -1), (-1, 1))


def hop_candidates(start, step):
    """The spots a `hop` tries, best first (as d2_sim::poke::hop_candidates):
    rings of radius 0, 2, 4, 7 around the full step, then around the half
    step, then a sidestep of 8 to either side across the step."""
    def ring(x, y):
        return [(x, y)] + [(x + r * dx, y + r * dy) for r in (2, 4, 7) for dx, dy in HOP_DIRS]
    (fx, fy), (sx, sy) = start, step
    half = (int(sx / 2), int(sy / 2))  # toward zero, as Rust's `/`
    sgn = lambda v: (v > 0) - (v < 0)  # noqa: E731
    px, py = sgn(sy) * 8, -sgn(sx) * 8
    out = ring(fx + sx, fy + sy) + ring(fx + half[0], fy + half[1]) + [(fx + px, fy + py),
                                                                      (fx - px, fy - py)]
    return [(x, y) for x, y in out if x >= 0 and y >= 0 and (x, y) != tuple(start)]


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
    g.add_argument("--forms", default=None, metavar="FILE",
                   help="a poke-forms-1 JSON file: call forms and fields overriding CALL_FORMS / "
                        "FIELDS for this run (README `Call forms`)")


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
    if auto and auto.has_frames():
        auto.attach(r)  # `frame F` input steps share the tick-return stop
    layer.attach(r)
    try:
        r.run()
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    if layer.walking:
        layer.finish(r, layer.last_frame if layer.last_frame is not None else -1)
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


def call_variants(regs, stack):
    """Every (regs, stack) one step from the given call: a register value + 1, a
    register renamed to an unused one, a stack slot + 1, one slot more or less."""
    out = []
    for r in regs:
        out.append(({**regs, r: (regs[r] + 1) & U32}, stack))
        free = [x for x in REGISTERS if x not in regs]
        if free:
            out.append(({(free[0] if k == r else k): v for k, v in regs.items()}, stack))
    out += [(regs, stack[:i] + [(stack[i] + 1) & U32] + stack[i + 1:]) for i in range(len(stack))]
    out.append((regs, stack + [0]))
    if stack:
        out.append((regs, stack[:-1]))
    return out


# The ids `msg` accepts (d2-client app::poke tests' MSG_IDS; poke.md Test vectors).
MSG_IDS = ("01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F 10 11 12 13 16 17 18 19 1A 1B 1C 1D 1E 1F 20 21 22 23 24 25 26 27 28 29 2A 2D 2E 2F 30 31 32 "
           "33 34 35 36 37 38 39 3A 3B 3C 3D 3E 3F 40 41 42 43 44 45 46 47 48 49 4B 4C 4D 4F 50 51 52 53 54 58 59 5D 5E 5F 60 61 62 63 69 6A 6B 6D 6E 70")


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
        ("poke 1\nat 0 msg\n", 2), ("poke 1\nat 0 msg 0 1\n", 2), ("poke 1\nat 0 msg 0x71\n", 2),
        ("poke 1\nat 0 msg 0x14 1 2 3 4\n", 2), ("poke 1\nat 0 msg 0x1A 1\n", 2), ("poke 1\nat 0 msg 0x01 1\n", 2),
        ("poke 1\nat 0 msg 0x01 1 2 3\n", 2), ("poke 1\nat 0 msg 0x01 65536 1\n", 2),
        ("poke 1\nat 0 msg 0x3C 1 2 3\n", 2), ("poke 1\nat 0 msg 0x01 @z 1\n", 2),
        ("poke 1\nat 0 msg 0x01 -1 2\n", 2), ("poke 1\nat 0 msg 0x01 @xx 2\n", 2),
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
            "spawn 0x13 1 2 random-boss", "seed-unit @wp#1 1 2", "msg 1 @x+2 @y", "msg 6 1 @1",
            "msg 0x3C 36 1 0xFFFFFFFF", "msg 0x60", "msg 0x02 1 1/77"]
    for g in good:
        d, args = parse_directive(g.split(), 1)
        d2, args2 = parse_directive(canonical(d, args).split(), 1)
        assert (d, args) == (d2, args2), g
        n += 1
    assert canonical(*parse_directive("object 0x27 1 2".split(), 1)) == "object 39 1 2 mode 0"
    assert canonical(*parse_directive("msg 0x01 @x+2 @y".split(), 1)) == "msg 1 @x+2 @y"

    # msg: the ids it takes (as d2-sim::poke's MSG_IDS) and the bytes (poke.md §1 `msg`)
    ok = " ".join(f"{i:02X}" for i, v in sorted(msg_layouts().items()) if not isinstance(v, str))
    assert ok == MSG_IDS, ok
    for mid, values, want in ((0x01, [0x1234, 0x5678], "0134127856"),
                              (0x06, [1, 0xAABBCCDD], "0601000000ddccbbaa"),
                              (0x3C, [36, 1, U32], "3c24000080ffffffff"),
                              (0x51, [5, 1, 3, 7], "510580030007000000"), (0x60, [], "60")):
        assert encode_msg(mid, values).hex() == want, (mid, encode_msg(mid, values).hex())
        n += 1
    for mid, values in ((0x01, [1]), (0x01, [0x10000, 1]), (0x3C, [1, 2, 3]), (0x14, [])):
        try:
            encode_msg(mid, values)
            raise AssertionError(f"encode {mid:#x} {values} accepted")
        except ValueError:
            n += 1
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

    # 5. call layouts of the spec-stated forms (literal, written from the specs)
    G, R, U = 0x0A000000, 0x0B000000, 0x0C000000
    expected = {
        # sim/units.md §3.1: ECX type, EDX class; x, y, game, room, flags, mode, GUID
        ("alloc", (("type", 2), ("class", 39), ("x", 100), ("y", 200), ("game", G), ("room", R),
                   ("flags", 1), ("mode", 2), ("guid", 0))):
            ({"ecx": 2, "edx": 39}, [100, 200, G, R, 1, 2, 0]),
        # original-hooks-spawn.md §1 entry 4: ECX game, EDX room; x, y, row
        ("superunique", (("game", G), ("room", R), ("x", 5), ("y", 6), ("row", 3))):
            ({"ecx": G, "edx": R}, [5, 6, 3]),
        # entry 1: ECX game, EDX room; x, y, class, mode, spread, flags
        ("spawn", (("game", G), ("room", R), ("x", 5), ("y", 6), ("class", 19), ("mode", 1),
                   ("spread", U32), ("flags", 0))):
            ({"ecx": G, "edx": R}, [5, 6, 19, 1, 0xFFFFFFFF, 0]),
        # entry 3: ECX game, EDX room; coord list, class, champion allowed, x, y, warp check
        ("random_boss", (("game", G), ("room", R), ("cl", 0), ("class", 19), ("champion", 1),
                         ("x", 0x15), ("y", 6), ("warp", 0))):
            ({"ecx": G, "edx": R}, [0, 19, 1, 0x15, 6, 0]),
        # entry 5: ECX game, EDX unit; umod
        ("champion_mark", (("game", G), ("unit", U), ("umod", 16))): ({"ecx": G, "edx": U}, [16]),
        # entry 6: ECX room, EDX x; y
        ("room_at", (("room", R), ("x", 5), ("y", 6))): ({"ecx": R, "edx": 5}, [6]),
        # original-hooks.md §7.1: ECX game, EDX record; no stack arguments
        ("missile", (("game", G), ("record", U))): ({"ecx": G, "edx": U}, []),
        # init.md §25.1: EDI game, EBX class; room, cl, x, y, GUID, warp check
        ("boss_spawn", (("game", G), ("class", 19), ("room", R), ("cl", 0), ("x", 5), ("y", 6),
                        ("guid", U32), ("warp", 0))):
            ({"edi": G, "ebx": 19}, [R, 0, 5, 6, U32, 0]),
        # init.md §25.1: ESI boss, EDI game; cl, class
        ("champion_minions", (("boss", U), ("game", G), ("cl", 0), ("class", 19))):
            ({"esi": U, "edi": G}, [0, 19]),
        # init.md §25.1: ECX min, EDX cl, EAX max; game, unit, spawn minions
        ("boss_minions", (("min", 3), ("cl", 0), ("max", 6), ("game", G), ("unit", U), ("minions", 1))):
            ({"ecx": 3, "edx": 0, "eax": 6}, [G, U, 1]),
        # sim/intents-events.md §2.1 rule 1: EDI size, [ESP+4] message
        ("send", (("size", 5), ("message", U))): ({"edi": 5}, [U]),
        # path-placement.md §6 r4: path, unit, room, x, y all on the stack
        ("teleport", (("path", U), ("unit", G), ("room", R), ("x", 5), ("y", 6))): ({}, [U, G, R, 5, 6]),
        # path-placement.md §10: ECX game, EDX unit; room, x, y, exact, alt
        ("place", (("game", G), ("unit", U), ("room", R), ("x", 5), ("y", 6), ("exact", 1),
                   ("alt", 0))): ({"ecx": G, "edx": U}, [R, 5, 6, 1, 0]),
        # waypoints.md §7 r5: ECX game, EDX player; level, tile
        ("warp", (("game", G), ("player", U), ("level", 3), ("tile", 13))):
            ({"ecx": G, "edx": U}, [3, 13]),
        # items/generation.md §3: ECX game, EDX request; use seed
        ("item_create", (("game", G), ("request", U), ("use_seed", 0))): ({"ecx": G, "edx": U}, [0]),
        # stat-lists.md §5 r2: unit, stat, value, layer on the stack
        ("stat_set", (("unit", U), ("stat", 13), ("value", 7), ("layer", 0))): ({}, [U, 13, 7, 0]),
        # stat-lists.md §9.2: unit, state, on on the stack
        ("state_set", (("unit", U), ("state", 11), ("on", 1))): ({}, [U, 11, 1]),
    }

    def check(exp):
        return [k for k, v in exp.items() if layout(k[0], **dict(k[1])) != v]

    assert check(expected) == [], check(expected)
    for k, (regs, st) in expected.items():  # perturbation: one changed value or register → detected
        for v in call_variants(regs, st):
            assert check({k: v}) == [k], (k, v)
            n += 1
    assert set(k[0] for k in expected) == {k for k, f in CALL_FORMS.items() if f.form is not None}

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
                    [parse_poke_option("12 seed-game 5 6")],
                    forms=load_forms({"format": FORMS_FORMAT, "forms": {"teleport": None,
                                                                        "place": None}})[0])
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
    n += selftest_forms(m, game, Rec)
    n += selftest_goto(Rec)
    print(f"selftest ok: {len(files)} poke file(s), {n} checks (malformed lines, references, "
          "--poke lines, missile record bytes, call layouts with perturbation, fake-game "
          "resolution and scheduling, call forms: gaps, --forms loading, register/stack "
          "layouts, item request bytes, umod list and msg bytes against a fake process, with "
          "perturbation, goto walks on a fake DRLG)")


# a filled-in forms file as PC 1 would write it (invented forms: the test checks
# that whatever a form says is what the call gets, not the forms themselves)
TEST_FORMS = {
    "format": FORMS_FORMAT,
    "forms": {
        "teleport": {"regs": {"ecx": "path", "edx": "room"}, "stack": ["unit", "x", "y"], "ret": 12},
        "place": {"regs": {"ecx": "game", "edx": "unit"}, "stack": ["room", "x", "y", "exact", "alt"],
                  "ret": 0x14},
        "warp": {"regs": {"esi": "game", "edi": "player"}, "stack": ["level", "tile"], "ret": 8,
                 "result": "bool"},
        "item_create": {"regs": {"ecx": "game", "edx": "request"}, "stack": ["use_seed"], "ret": 4},
        "stat_set": {"regs": {"ecx": "unit", "edx": "stat"}, "stack": ["value", "layer"], "ret": 8},
        "state_set": {"regs": {"eax": "unit", "ebx": "state"}, "stack": ["on"], "ret": 4},
    },
}


def selftest_forms(m, game, Rec):
    """Call forms (poke.md §4 rule 8) against a fake process: m is the fake game
    of selftest §6 (player 0x200000 with path 0x300000 in room 0x400000)."""
    import json
    import tempfile
    n = 0
    P, PATH, R0 = 0x200000, 0x300000, 0x400000
    RA, NEW, ITEM, S = 0x400100, 0x210000, 0x230000, 0x900000
    MD, ITEMS, FMT = 0x220000, 0x800000, 0x65

    def fake():
        rec = Rec()
        rec.m = dict(m.m)
        rec.write(MISSILE, MISSILE_BYTES)
        rec.w32(0x200400 + U_STATS, 0x250000)          # @1:19 has a stat list; 1/9 has none
        for u, g in ((NEW, 0x33), (ITEM, 0x44)):
            rec.w32(u + U_TYPE, 1 if u == NEW else 4)
            rec.w32(u + U_GUID, g)
        rec.w32(NEW + FIELDS["mon_data"], MD)
        rec.write(MD + FIELDS["mon_umods"], bytes([5]))  # the boss already has umod 5
        rec.w32(FIELDS["items_header"], 3)
        rec.w32(FIELDS["items_header"] + 4, ITEMS)
        for i, c in enumerate((b"hax ", b"hp5 ", b"rin ")):
            rec.write(ITEMS + i * FIELDS["item_record_size"] + FIELDS["item_code"], c)
        rec.w32(game + FIELDS["item_format"], FMT)
        return rec

    def run(line, forms=None, fields=None, eax=None):
        """(result, calls) of one directive on a fresh fake process."""
        rec = fake()
        lay = PokeLayer(forms=forms, fields=fields)
        lay.page = lambda r: S
        calls = []
        ret = {ROOM_AT: RA, 0x005A09E0: NEW, BOSS: NEW, SPAWN: NEW, ALLOC: NEW, SUPERUNIQUE: NEW,
               MISSILE: NEW, 0x00558D90: ITEM, 0x00554EA0: 1, 0x0053AEC0: 1, 0x00650BE0: 1}
        ret.update(eax or {})

        def call(r, tid, saved, entry, regs, stack):
            calls.append((entry, dict(regs), list(stack)))
            return ret.get(entry, 0)
        lay.call = call
        d, args = parse_directive(line.split(), 1)
        return lay.apply(rec, game, 1, None, Step(d, args, 1)), calls, rec

    lines = ["pos @player 5003 4001", "warp 3 tile 2", "item hp5 @x+1 @y quality unique ilvl 30",
             "stat @1:19 13 0 -5", "state 1/9 11 on"]
    # a. without these five forms the directives are gaps, naming the function and the item
    gap5, _ = load_forms({"format": FORMS_FORMAT, "forms": {k: None for k in OPEN_QUESTION}})
    for line, fn in zip(lines, ("teleport", "warp", "item_create", "stat_set", "state_set")):
        r, calls, _ = run(line, gap5)
        assert r["r"] == "gap" and calls == [], (line, r)
        assert f"{CALL_FORMS[fn].addr:#010x}" in r["note"] and "item 22 (" in r["note"], r
        n += 1
    # every function without a form: every directive that calls one is a gap, no call made
    none_forms, _ = load_forms({"format": FORMS_FORMAT, "forms": {k: None for k in CALL_FORMS}})
    for line in lines + ["object 39 1 2", "superunique 1 1 2", "missile 1 1 2 3 4",
                         "spawn 19 1 2 normal", "spawn 19 1 2 random-boss",
                         "spawn 19 1 2 champion umod 16", "spawn 19 1 2 unique umod 1",
                         "msg 1 1 2"]:
        r, calls, _ = run(line, none_forms)
        assert r["r"] == "gap" and calls == [] and "no 1.14d call form" in r["note"], (line, r)
        n += 1
    r, calls, rec = run("seed-game 1 2", none_forms)  # field writes need no form
    assert r["r"] == "ok" and calls == []
    _, nofield = load_forms({"format": FORMS_FORMAT, "fields": {"mon_umods": None}})
    r, calls, _ = run("spawn 19 1 2 unique umod 1", None, nofield)
    assert r["r"] == "gap" and "no field value: mon_umods" in r["note"] and calls == [], r
    n += 2

    # b. --forms: loading (file, through from_args) and its errors
    with tempfile.TemporaryDirectory() as tmp:
        fp = os.path.join(tmp, "forms.json")
        with open(fp, "w", encoding="utf-8") as f:
            json.dump(TEST_FORMS, f)
        lay = PokeLayer.from_args(argparse.Namespace(poke=["10 freeze 0"], poke_file=None,
                                                     start_frame=None, forms=fp))
        assert lay.forms["warp"].form == Form({"esi": "game", "edi": "player"}, ["level", "tile"], 8,
                                              "bool")
        assert lay.forms_source[0] == fp and lay.header()["forms"]["warp"]["regs"]["esi"] == "game"
        assert CALL_FORMS["warp"].form.regs == {"ecx": "game", "edx": "player"}  # table untouched
    forms, fields = load_forms(TEST_FORMS)
    for bad in ({"forms": {}}, {"format": "poke-forms-2"},
                {"format": FORMS_FORMAT, "other": 1},
                {"format": FORMS_FORMAT, "forms": {"nope": None}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"regs": {"ebp": "game", "edi": "player"},
                                                            "stack": ["level", "tile"]}}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"regs": {"ecx": "game"},
                                                            "stack": ["player", "level"]}}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"regs": {"ecx": "game", "edx": "game"},
                                                            "stack": ["player", "level", "tile"]}}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"regs": {"ecx": "game"},
                                                            "stack": ["player", "level", "tiles"]}}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"regs": {}, "ret": 8,
                                                            "stack": ["game", "player", "level", "tile"]}}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"stack": ["game", "player", "level", "tile"],
                                                            "result": "maybe"}}},
                {"format": FORMS_FORMAT, "forms": {"warp": {"stack": [], "extra": 1}}},
                {"format": FORMS_FORMAT, "fields": {"nope": 1}},
                {"format": FORMS_FORMAT, "fields": {"mon_umods": -1}}):
        try:
            load_forms(bad)
            raise AssertionError(f"forms {bad} accepted")
        except ValueError:
            n += 1
    # literals are allowed: a form may pass a constant the spec names
    f2, _ = load_forms({"format": FORMS_FORMAT, "forms": {"warp": {
        "regs": {"ecx": "game"}, "stack": ["player", "level", "tile", 0], "ret": 16}}})
    assert build("warp", {"game": 1, "player": 2, "level": 3, "tile": 4}, f2) == ({"ecx": 1}, [2, 3, 4, 0])

    # c. with forms filled in: each directive's exact calls on the fake process
    req = bytes.fromhex(
        "00000000" "00000000" "00001000" "1e000000"   # unit 0, -, game, ilvl 30
        "00000000" "01000000" "03000000" "89130000"   # -, item 1 (hp5), mode 3 ground, x 5001
        "a00f0000" "00014000" "01006500" "00000000"   # y 4000, room, init flags 1, format 0x65, -
        "07000000" + "00" * 0x4C + "00000000")        # quality 7 (unique), rest 0, flags2 0
    assert len(req) == ITEM_REQ_SIZE
    room_at = (ROOM_AT, {"ecx": R0, "edx": 5003}, [4003])
    boss = (0x005A09E0, {"edi": game, "ebx": 19}, [RA, 0, 5003, 4003, U32, 0])
    minions = (0x0054E1E0, {"esi": NEW, "edi": game}, [0, 19])
    cases = [
        ("pos @player 5003 4001", forms, {"r": "ok", "via": "teleport"},
         [(ROOM_AT, {"ecx": R0, "edx": 5003}, [4001]), (0x00650BE0, {"ecx": PATH, "edx": RA}, [P, 5003, 4001])]),
        ("pos @player 5003 4001", load_forms({"format": FORMS_FORMAT, "forms": {"teleport": None}},
                                             forms, fields)[0], {"r": "ok", "via": "place"},
         [(ROOM_AT, {"ecx": R0, "edx": 5003}, [4001]),
          (0x00554EA0, {"ecx": game, "edx": P}, [RA, 5003, 4001, 1, 0])]),
        ("warp 3 tile 2", forms, {"r": "ok"}, [(0x0053AEC0, {"esi": game, "edi": P}, [3, 2])]),
        ("warp 3", forms, {"r": "ok"}, [(0x0053AEC0, {"esi": game, "edi": P}, [3, 0])]),
        ("item hp5 @x+1 @y quality unique ilvl 30", forms, {"r": "ok", "guid": 0x44},
         [(ROOM_AT, {"ecx": R0, "edx": 5001}, [4000]),
          (0x00558D90, {"ecx": game, "edx": S + SCRATCH_ITEM}, [0])]),
        ("item zzz 1 2", forms, {"r": "failed"}, []),
        ("stat @1:19 13 0 -5", forms, {"r": "ok"},
         [(0x00627260, {"ecx": 0x200400, "edx": 13}, [U32 - 4, 0])]),
        ("stat 1/9 13 0 1", forms, {"r": "failed"}, []),
        ("state 1/9 11 on", forms, {"r": "ok"}, [(0x00639DB0, {"eax": 0x200100, "ebx": 11}, [1])]),
        ("state @player 11 off", forms, {"r": "ok"}, [(0x00639DB0, {"eax": P, "ebx": 11}, [0])]),
        ("spawn 19 @x+3 @y+3 normal", forms, {"r": "ok", "guid": 0x33},
         [room_at, (SPAWN, {"ecx": game, "edx": RA}, [5003, 4003, 19, 1, U32, 0])]),
        ("spawn 19 @x+3 @y+3 random-boss", forms, {"r": "ok", "guid": 0x33},
         [room_at, (BOSS, {"ecx": game, "edx": RA}, [0, 19, 1, 5003, 4003, 0]), minions]),
        ("spawn 19 @x+3 @y+3 champion umod 16", forms, {"r": "ok", "guid": 0x33},
         [room_at, boss, (CHAMPION_MARK, {"ecx": game, "edx": NEW}, [16]), minions]),
        ("spawn 19 @x+3 @y+3 unique umod 1 2 9", forms, {"r": "ok", "guid": 0x33},
         [room_at, boss, (0x005A2120, {"ecx": 3, "edx": 0, "eax": 6}, [game, NEW, 1])]),
        # msg: the bytes at S+0x300, then 0x00478350 with EDI = size, [ESP+4] = the bytes
        ("msg 0x01 @x+2 @y", forms, {"r": "ok", "bytes": "018a13a00f"},
         [(SEND, {"edi": 5}, [S + SCRATCH_MSG])]),
        ("msg 0x06 1 @1", forms, {"r": "ok", "bytes": "060100000005000000"},
         [(SEND, {"edi": 9}, [S + SCRATCH_MSG])]),
        ("msg 0x06 1 @1:21", forms, {"r": "unresolved"}, []),
        ("msg 0x3A @x 1", forms, {"r": "unresolved"}, []),
    ]
    for line, fm, want_r, want_calls in cases:
        r, calls, rec = run(line, fm)
        assert all(r.get(k) == v for k, v in want_r.items()), (line, r)
        assert calls == want_calls, (line, calls)
        for c in range(len(want_calls)):  # perturbation: one wrong register or slot → differs
            e, regs, st = want_calls[c]
            for v in call_variants(regs, st):
                assert calls != want_calls[:c] + [(e, v[0], v[1])] + want_calls[c + 1:], (line, v)
                n += 1
        if line.startswith("item hp5"):
            got = rec.read(S + SCRATCH_ITEM, ITEM_REQ_SIZE)
            assert got == req, got.hex()
            for i in range(len(req)):
                pert = bytearray(req)
                pert[i] ^= 0x01
                assert got != bytes(pert)
                n += 1
        if line.startswith("msg") and want_calls:
            assert rec.read(S + SCRATCH_MSG, want_calls[0][1]["edi"]).hex() == want_r["bytes"], line
            assert SCRATCH_MSG >= SCRATCH_ITEM + ITEM_REQ_SIZE and SCRATCH_MSG + 0x200 <= 0x1000
        if "unique" in line and "spawn" in line:  # appended after the existing umod 5
            assert rec.read(MD + FIELDS["mon_umods"], UMOD_MAX) == bytes([5, 1, 2, 9, 0, 0, 0, 0, 0])
        n += 1
    # a full list stays full (at most 9)
    r, calls, rec = run("spawn 19 1 2 unique umod 1 2 3 4 5 6 7 8 9", forms)
    assert rec.read(MD + FIELDS["mon_umods"], UMOD_MAX + 1) == bytes([5, 1, 2, 3, 4, 5, 6, 7, 8, 0])
    # a wrong form is seen: warp with ESI/EDI swapped gives other calls than the spec's
    swapped, _ = load_forms({"format": FORMS_FORMAT, "forms": {"warp": {
        "regs": {"esi": "player", "edi": "game"}, "stack": ["level", "tile"]}}}, forms, fields)
    assert run("warp 3 tile 2", swapped)[1] != [(0x0053AEC0, {"esi": game, "edi": P}, [3, 2])]
    shifted, _ = load_forms({"format": FORMS_FORMAT, "forms": {"warp": {
        "regs": {"esi": "game", "edi": "player"}, "stack": ["tile", "level"]}}}, forms, fields)
    assert run("warp 3 tile 2", shifted)[1] != [(0x0053AEC0, {"esi": game, "edi": P}, [3, 2])]
    r, _, _ = run("warp 3", forms, eax={0x0053AEC0: 0})
    assert r["r"] == "failed"  # result "bool": EAX 0
    n += 4

    # d. the context: every listed register is written, the others keep their values
    class Ctx:
        pass
    ctx = Ctx()
    for k in ("Eax", "Ebx", "Ecx", "Edx", "Esi", "Edi", "Ebp", "Esp"):
        setattr(ctx, k, 0x11)
    set_regs(ctx, {"eax": 1, "ebx": 2, "ecx": 3, "edx": 4, "esi": 5, "edi": -1})
    assert (ctx.Eax, ctx.Ebx, ctx.Ecx, ctx.Edx, ctx.Esi, ctx.Edi, ctx.Ebp, ctx.Esp) == \
        (1, 2, 3, 4, 5, U32, 0x11, 0x11)
    try:
        set_regs(ctx, {"ebp": 1})
        raise AssertionError("ebp accepted")
    except ValueError:
        n += 2
    return n


def selftest_goto(Rec):
    """goto (poke.md §6) on a fake game: three DRLG rooms of level 5 in a row
    (D0 - D1 - D2, near arrays), the player in D0's active room; the placement
    moves the player, the warp puts it in D0."""
    n = 0
    G, P, PATH = 0x100000, 0x200000, 0x300000
    D = [0x500000, 0x500100, 0x500200]       # DRLG rooms
    A = [0x600000, 0x600100, 0x600200]       # their active rooms
    LV5, LV1, D_TOWN, A_TOWN = 0x700000, 0x700400, 0x500300, 0x600300
    MON, OBJ = 0x210000, 0x220000

    def fake(active2=False, town=False):
        r = Rec()
        r.w32(LV5 + L_ID, 5)
        r.w32(LV1 + L_ID, 1)
        for i, (d, ar) in enumerate(zip(D + [D_TOWN], A + [A_TOWN])):
            r.w32(d + DR_LEVEL, LV1 if d == D_TOWN else LV5)
            r.w32(d + DR_TX, 8 * i)
            r.w32(d + DR_TY, 0)
            r.write(ar + AR_SUB, struct.pack("<iiii", 40 * i, 0, 40, 40))
            r.w32(ar + AR_DRLG, d)
            cr = 0x680000 + 0x10000 * i            # collision record, masks all 0 (free)
            r.write(cr + CR_X, struct.pack("<iiii", 40 * i, 0, 40, 40))
            r.w32(cr + CR_MASKS, cr + 0x100)
            r.w32(ar + AR_COLL, cr)
            r.w32(d + DR_ACTIVE, 0 if (d == D[2] and not active2) else ar)
        near = {D[0]: [D[0], D[1]], D[1]: [D[0], D[1], D[2]], D[2]: [D[1], D[2]], D_TOWN: [D_TOWN]}
        for k, (d, lst) in enumerate(near.items()):
            arr = 0x580000 + 0x100 * k
            for j, x in enumerate(lst):
                r.w32(arr + 4 * j, x)
            r.w32(d + DR_NEAR, arr)
            r.w32(d + DR_NEAR_N, len(lst))
        r.w32(P + U_TYPE, 0)
        r.w32(P + U_GUID, 1)
        r.w32(P + U_PATH, PATH)
        r.w32(G + HASH_BASE + HASH_OFFSETS[0] + 4, P)
        put_player(r, A_TOWN if town else A[0], 20, 20)
        return r

    def put_player(r, room, x, y):
        r.w32(PATH + P_ROOM, room)
        r.write(PATH + P_X, struct.pack("<H", x))
        r.write(PATH + P_Y, struct.pack("<H", y))

    def add_unit(r, u, t, guid, cls, room, x, y):
        r.w32(u + U_TYPE, t)
        r.w32(u + U_GUID, guid)
        r.w32(u + U_CLASS, cls)
        up = u + 0x1000
        r.w32(u + U_PATH, up)
        if t == 1:
            r.w32(up + P_ROOM, room)
            r.write(up + P_X, struct.pack("<H", x))
            r.write(up + P_Y, struct.pack("<H", y))
        else:
            r.w32(up + SP_ROOM, room)
            r.w32(up + SP_X, x)
            r.w32(up + SP_Y, y)
        head = G + HASH_BASE + HASH_OFFSETS[t] + 4 * (guid & 0x7F)
        r.w32(u + U_HASH_NEXT, r.read_u32(head))
        r.w32(head, u)

    def layer(r, calls, place_ok=True):
        lay = PokeLayer()

        def call(rr_, tid, saved, entry, regs, stack):
            calls.append((entry, dict(regs), list(stack)))
            if entry == 0x00554EA0:            # place: ECX game, EDX unit; room, x, y, exact, alt
                if place_ok:
                    put_player(r, stack[0], stack[1], stack[2])
                return 1 if place_ok else 0
            if entry == 0x0053AEC0:            # warp: the player lands in D0
                put_player(r, A[0], 20, 20)
                return 1
            return 0
        lay.call = call
        return lay

    def step(lay, r, line, w):
        d, args = parse_directive(line.split(), 1)
        return lay.apply(r, G, 1, None, Step(d, args, 1), w)

    def walk():
        return {"goal": None, "seen": set(), "blocked": set(), "steps": 0}

    # parse and canonical text
    for text, want in (("goto unit 5", "goto unit 1:5"), ("goto preset 107 2:376", "goto preset 107 2:376"),
                       ("goto preset 107 376", "goto preset 107 1:376")):
        assert canonical(*parse_directive(text.split(), 1)) == want, text
        n += 1
    for bad in ("goto unit 3:5", "goto preset 2", "goto here 5", "goto unit 1:x", "goto unit", "goto preset x 5"):
        try:
            parse_directive(bad.split(), 1)
            raise AssertionError(f"accepted {bad!r}")
        except PokeError:
            n += 1
    # a. target in an active room: one step, placed at the target's point with exact 0
    r, calls = fake(), []
    add_unit(r, MON, 1, 0x40, 156, A[1], 55, 12)
    w = walk()
    res = step(layer(r, calls), r, "goto unit 156", w)
    assert res["r"] == "ok" and res["guid"] == 0x40 and w["steps"] == 1, res
    assert calls == [(0x00554EA0, {"ecx": G, "edx": P}, [A[1], 55, 12, 0, 0])], calls
    n += 1
    # a landing cell with the missile-blocking bit 0x4: placed again, exact 1, on the nearest free cell
    r, calls = fake(), []
    add_unit(r, MON, 1, 0x40, 156, A[1], 55, 12)
    r.write(0x690000 + 0x100 + 2 * (12 * 40 + 55 - 40), struct.pack("<H", 0x0004))
    res = step(layer(r, calls), r, "goto unit 156", walk())
    assert res["r"] == "ok" and len(calls) == 2, (res, calls)
    assert calls[1] == (0x00554EA0, {"ecx": G, "edx": P}, [A[1], 55, 11, 1, 0]), calls[1]
    assert mask_at(r, A[1], 55, 12) == 4 and mask_at(r, A[1], 55, 11) == 0 and mask_at(r, A[1], 200, 0) is None
    assert free_cell(r, A[1], (55, 12)) == (55, 11) and free_cell(r, A[1], (0, 0)) == (40, 0)
    n += 1
    # `pos … free`: parse, canonical text, duplicates and extras
    assert canonical(*parse_directive("pos @player 5 6 free".split(), 1)) == "pos @player 5 6 free"
    assert canonical(*parse_directive("pos @player 5 6".split(), 1)) == "pos @player 5 6"
    for bad in ("pos @player 5 6 free free", "pos @player 5 6 near", "hop @player 5 6 free"):
        try:
            parse_directive(bad.split(), 1)
            raise AssertionError(f"accepted {bad!r}")
        except PokeError:
            n += 1
    n += 2
    # an object target reads the static path; another class or level is not a target
    r, calls = fake(), []
    add_unit(r, OBJ, 2, 0x41, 376, A[0], 7, 9)
    add_unit(r, MON, 1, 0x40, 376, A[1], 55, 12)
    res = step(layer(r, calls), r, "goto unit 2:376", walk())
    assert res["r"] == "ok" and res["guid"] == 0x41 and calls[0][2] == [A[0], 7, 9, 0, 0], (res, calls)
    n += 1
    # b. the walk: D2 not active yet -> hop into D1 (centre 60, 20), pending; then D2
    # active with the target in it -> found
    r, calls = fake(), []
    lay, w = layer(r, calls), walk()
    res = step(lay, r, "goto unit 156", w)
    assert res["r"] == "pending" and calls == [(0x00554EA0, {"ecx": G, "edx": P}, [A[1], 60, 20, 0, 0])], (res, calls)
    assert w["seen"] == {(5, 0, 0), (5, 8, 0)}, w["seen"]
    r.w32(D[2] + DR_ACTIVE, A[2])
    add_unit(r, MON, 1, 0x40, 156, A[2], 95, 30)
    res = step(lay, r, "goto unit 156", w)
    assert res["r"] == "ok" and res["guid"] == 0x40 and w["steps"] == 2, res
    n += 2
    # c. nothing anywhere: hop, then every room of the level seen -> failed "explored"
    r, calls = fake(), []
    lay, w = layer(r, calls), walk()
    assert step(lay, r, "goto unit 156", w)["r"] == "pending"
    r.w32(D[2] + DR_ACTIVE, A[2])
    res = step(lay, r, "goto unit 156", w)
    assert res["r"] == "failed" and "explored" in res["note"] and len(w["seen"]) == 3, (res, w)
    n += 1
    # a refused placement marks the hop room seen
    r, calls = fake(), []
    w = walk()
    assert step(layer(r, calls, place_ok=False), r, "goto unit 156", w)["r"] == "pending"
    assert (5, 16, 0) not in w["seen"] and len(w["seen"]) == 2
    n += 1
    # lava at D1's centre: the hop aims at D1's free cell nearest the centre
    r, calls = fake(), []
    cr1 = 0x690000
    for yy in range(0, 40):
        for xx in range(40, 80):
            if abs(xx - 60) <= 3 and abs(yy - 20) <= 3 and (xx, yy) != (63, 23):
                r.write(cr1 + 0x100 + 2 * (yy * 40 + xx - 40), struct.pack("<H", 0x0001))
    assert free_cell(r, A[1]) == (60, 16), free_cell(r, A[1])
    w = walk()
    assert step(layer(r, calls), r, "goto unit 156", w)["r"] == "pending"
    assert calls == [(0x00554EA0, {"ecx": G, "edx": P}, [A[1], 60, 16, 0, 0])], calls
    # no free cell at all: H seen, no placement
    r = fake()
    for yy in range(0, 40):
        for xx in range(40, 80):
            r.write(cr1 + 0x100 + 2 * (yy * 40 + xx - 40), struct.pack("<H", 0x0400))
    r2, calls2, w2 = r, [], walk()
    assert step(layer(r2, calls2), r2, "goto unit 156", w2)["r"] == "pending" and calls2 == [] \
        and len(w2["seen"]) == 2 and w2["blocked"] == {(5, 8, 0)}, (calls2, w2)
    # the next search does not pass through the blocked D1: D2 is unreachable -> failed
    res = step(layer(r2, calls2), r2, "goto unit 156", w2)
    assert res["r"] == "failed" and "explored" in res["note"] and calls2 == [], res
    n += 2
    # a placement that lands outside the hop room (its centre blocked) marks it seen
    r, calls = fake(), []
    lay, w = layer(r, calls), walk()
    lay.call = lambda rr_, tid, saved, entry, regs, stack: (calls.append(entry), 1)[1]
    assert step(lay, r, "goto unit 156", w)["r"] == "pending" and (5, 8, 0) in w["seen"], w["seen"]
    n += 1
    # d. preset from another level: the warp first (pending), then the walk in level 5
    r, calls = fake(town=True), []
    lay, w = layer(r, calls), walk()
    res = step(lay, r, "goto preset 5 2:376", w)
    assert res["r"] == "pending" and calls == [(0x0053AEC0, {"ecx": G, "edx": P}, [5, 0])], (res, calls)
    add_unit(r, OBJ, 2, 0x41, 376, A[0], 7, 9)
    res = step(lay, r, "goto preset 5 2:376", w)
    assert res["r"] == "ok" and res["guid"] == 0x41 and w["goal"] == 5, res
    n += 2
    # e. the step limit
    r, calls = fake(), []
    w = walk()
    w["steps"] = GOTO_MAX_STEPS
    assert step(layer(r, calls), r, "goto unit 156", w) == {"args": {"level": None, "type": 1, "class": 156},
                                                            "r": "failed", "note": "step limit"}
    n += 1
    # f. needs: the warp form only for a preset
    assert needs("goto", {"level": None})[0] == ["place"] and needs("goto", {"level": 5})[0] == ["warp", "place"]
    n += 1
    return n


if __name__ == "__main__":
    main()
