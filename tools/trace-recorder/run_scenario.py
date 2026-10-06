"""Run a scenario in the original 1.14d Game.exe and record it.

A scenario (JSON, `traces/scenarios/`) names a save, a seed, a tick count
and C->S messages to inject at given ticks. This tool starts game/Game.exe
under the Win32 debugger (the loop of record_rng.py, the message hooks of
record_packets.py, the unit-list reads of record_tick.py), starts
single-player with that save without menus, overrides the seed before any
draw, injects each step's message so the server processes it in its tick,
records the requested streams per tick and writes
traces/raw/<time>-scenario.jsonl (format scenario-raw-0, traces/FORMAT.md;
provisional until the scenario format of specs/tools/scenario.md lands).

Scenario file (version 1, minimal):
  {"version": 1, "seed": <time value, < 2^31>, "init_seed": <u32, optional>, "save": "<character name>", "difficulty": 0,
   "steps": [{"tick": N, "c2s": "<hex message>"}, ...], "ticks": <total>,
   "streams": ["units", "packets", "rng"]}

Streams: "units" (one snapshot per tick), "packets" (the message hooks of
record_packets.py), "rng" (every seeded draw and seed set).

Game-side facts (injection point and method, seed override point, tick
hook, unit fields, non-interactive start, save loading) come from
specs/tools/original-hooks.md; comments cite its sections. The start is
`Game.exe -w -ns -nosave -name <save> -<class>` with the menu ended by the
tool (spec §5.4); `--manual-start` lets a person start the game instead.
`--probe <name>` settles the spec's open questions on the running game:
start (OQ2), inject (OQ1), seed (OQ3, two runs), savepath (OQ4), plus
ready (first tick the client accepts messages) and unit_fields (raw dump
of the player unit to confirm the snapshot fields).

  --selftest   everything that needs no game (parsing and its errors,
               hex parsing, writer output, hash perturbation check)
  --dry-run    print the plan and exit; starts nothing

The game process is always terminated when this script ends. Never run
by the cloud: the coordinator runs it with the user at the game.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import ctypes as C
import datetime
import hashlib
import json
import os
import re
import struct
import sys
import tempfile
import threading
import time

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import record_rng as rr  # noqa: E402  (the shared Win32 debugger definitions)
import record_packets as rp  # noqa: E402  (message hooks, loop-phase markers)
import record_tick as rt  # noqa: E402  (game / unit-list layout and readers)

TOOL = "trace-recorder run_scenario 0.2.0"
RAW_FORMAT = "scenario-raw-0"
RAW_FORMAT_VERSION = 0          # bumps when a record changes meaning (FORMAT.md)
SCENARIO_VERSION = 1            # the scenario file version this tool reads
STREAMS = ("units", "packets", "rng")
MAX_MSG = 0x1FC                 # the server's per-message buffer (record_packets.py)
MAX_TICKS = 1_000_000
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))

# Which stream a record type belongs to (record types of record_packets.py and
# record_rng.py). "tick", "inject", "seed_override" and the like are always kept.
STREAM_OF = {
    "draw": "rng", "seed_set": "rng",
    "units": "units",
    **{kind: "packets" for kind, _ in rp.HOOKS.values() if kind not in ("tick", "tick_end")},
    "tick_end": "packets",
}


class ScenarioError(ValueError):
    """A scenario file or message that cannot be used."""


# --- scenario file -----------------------------------------------------------

def parse_hex(text, what="message"):
    """'01 f1 13' / '01f113' / '0x01 0xf1' -> bytes. Errors name `what`."""
    if not isinstance(text, str):
        raise ScenarioError(f"{what}: hex must be a string")
    s = re.sub(r"0x", "", text, flags=re.I)
    s = re.sub(r"[\s,:_-]", "", s)
    if not s:
        raise ScenarioError(f"{what}: empty message")
    if len(s) % 2 or not re.fullmatch(r"[0-9a-fA-F]+", s):
        raise ScenarioError(f"{what}: not an even number of hex digits: {text!r}")
    b = bytes.fromhex(s)
    if len(b) > MAX_MSG:
        raise ScenarioError(f"{what}: {len(b)} bytes, the server takes at most {MAX_MSG}")
    return b


def parse_scenario(text):
    """Scenario JSON text -> normalized dict (steps sorted by tick, stable)."""
    try:
        d = json.loads(text)
    except json.JSONDecodeError as e:
        raise ScenarioError(f"not valid JSON: {e}") from None
    if not isinstance(d, dict):
        raise ScenarioError("top level must be an object")
    if d.get("version") != SCENARIO_VERSION:
        raise ScenarioError(f"version {d.get('version')!r} not supported (need {SCENARIO_VERSION})")

    def int_field(name, lo, hi):
        v = d.get(name)
        if isinstance(v, bool) or not isinstance(v, int):
            raise ScenarioError(f"{name}: integer required, got {v!r}")
        if not lo <= v <= hi:
            raise ScenarioError(f"{name}: {v} outside {lo}..{hi}")
        return v

    # specs/tools/original-hooks.md §2.1: the time value is below 2^31; the init
    # value (game +0x7C, DRLG seed when the save has none) is any u32 and defaults to it
    seed = int_field("seed", 0, 0x7FFFFFFF)
    init_seed = int_field("init_seed", 0, 0xFFFFFFFF) if "init_seed" in d else seed
    ticks = int_field("ticks", 1, MAX_TICKS)
    difficulty = int_field("difficulty", 0, 2) if "difficulty" in d else 0
    cls = d.get("class", "ama")
    if cls not in ("ama", "sor", "nec", "pal", "bar", "dru", "ass"):
        raise ScenarioError(f"class: {cls!r} is not one of ama sor nec pal bar dru ass")
    save = d.get("save")
    if not isinstance(save, str) or not save:
        raise ScenarioError("save: character name required")
    if not re.fullmatch(r"[A-Za-z][A-Za-z_-]{1,14}", save):
        raise ScenarioError(f"save: {save!r} is not a character name (2-15 letters, - or _)")
    streams = d.get("streams", list(STREAMS))
    if not isinstance(streams, list) or not streams:
        raise ScenarioError("streams: non-empty list required")
    for s in streams:
        if s not in STREAMS:
            raise ScenarioError(f"streams: unknown stream {s!r} (known: {', '.join(STREAMS)})")
    if len(set(streams)) != len(streams):
        raise ScenarioError("streams: duplicate entry")
    raw_steps = d.get("steps", [])
    if not isinstance(raw_steps, list):
        raise ScenarioError("steps: list required")
    steps = []
    for i, st in enumerate(raw_steps):
        if not isinstance(st, dict):
            raise ScenarioError(f"steps[{i}]: object required")
        t = st.get("tick")
        if isinstance(t, bool) or not isinstance(t, int) or not 0 <= t < ticks:
            raise ScenarioError(f"steps[{i}]: tick must be an integer in 0..{ticks - 1}, got {t!r}")
        steps.append({"tick": t, "c2s": parse_hex(st.get("c2s"), f"steps[{i}].c2s")})
    steps.sort(key=lambda s: s["tick"])  # stable: same-tick steps keep file order
    return {"seed": seed, "init_seed": init_seed, "save": save, "class": CLASS_NAMES[cls],
            "difficulty": difficulty, "ticks": ticks,
            "streams": list(streams), "steps": steps}


def scenario_run_exe():
    """`scenario-run` binary: $SCENARIO_RUN, else the cargo target dir's debug build."""
    env = os.environ.get("SCENARIO_RUN")
    if env:
        return env
    tgt = os.environ.get("CARGO_TARGET_DIR", os.path.join(REPO, "target"))
    return os.path.join(tgt, "debug", "scenario-run.exe" if os.name == "nt" else "scenario-run")


def load_script(path, save_override=None):
    """A `.scenario` script (specs/tools/scenario.md) through `scenario-run export`
    (the one strict parser and canonical writer; handoff scenario-harness §3 change 1).
    Returns (sc, sha256 of the canonical text, export dict). Times are relative
    (`rel`): tick t is frame F0 + 1 + t (scenario.md §4 rule 2)."""
    import subprocess
    r = subprocess.run([scenario_run_exe(), "export", path], capture_output=True, text=True)
    if r.returncode:
        raise ScenarioError(f"{path}: scenario-run export failed: {(r.stderr or r.stdout).strip()}")
    ex = json.loads(r.stdout)
    save = save_override or ex["save"]
    if not save:
        raise ScenarioError(f"{path}: no `char save`: the original side needs a save (handoff §3)")
    if ex["class"] is None:
        raise ScenarioError(f"{path}: no `char class`: needed to start the game")
    cls = ex["class"]
    if not 0 <= cls <= 6:
        raise ScenarioError(f"{path}: class {cls}")
    streams = ["packets", "units"]            # raw hooks the trace is built from
    steps = []
    for s in ex["steps"]:
        steps.append(s)
    return ({"seed": ex["seed"], "init_seed": ex["init"], "save": save, "class": cls,
             "difficulty": ex["difficulty"], "ticks": ex["end"], "streams": streams,
             "steps": steps, "rel": True, "export": ex, "save_overridden": bool(save_override)},
            ex["sha256"], ex)


def load_scenario(path):
    with open(path, "rb") as f:
        raw = f.read()
    try:
        sc = parse_scenario(raw.decode("utf-8"))
    except (UnicodeDecodeError, ScenarioError) as e:
        raise ScenarioError(f"{path}: {e}") from None
    return sc, hashlib.sha256(raw).hexdigest()


# --- output ------------------------------------------------------------------

class ScenarioWriter:
    """JSON lines to `path`; hashes the record lines (not header and footer)
    so two runs and a perturbed run can be compared."""

    def __init__(self, path):
        self.path, self.n = path, 0
        self.sha = hashlib.sha256()
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        self.f = open(path, "w", encoding="utf-8", newline="\n")

    @staticmethod
    def line(rec):
        return json.dumps(rec, separators=(",", ":"), sort_keys=True) + "\n"

    def header(self, rec):
        self.f.write(self.line(rec))

    def record(self, rec):
        s = self.line(rec)
        self.sha.update(s.encode("utf-8"))
        self.n += 1
        self.f.write(s)

    # base Recorder code writes pre-serialized lines
    def write(self, s):
        self.sha.update(s.encode("utf-8"))
        self.n += 1
        self.f.write(s)

    def footer(self, rec):
        self.f.write(self.line({**rec, "type": "footer", "records": self.n,
                                "records_sha256": self.sha.hexdigest()}))

    def close(self):
        self.f.close()


def make_header(scenario, scenario_path, scenario_sha, game_sha, args):
    return {"type": "header", "format": RAW_FORMAT, "format_version": RAW_FORMAT_VERSION,
            "tool": TOOL, "date": datetime.date.today().isoformat(),
            "game_exe_sha256": game_sha, "scenario": scenario_path.replace("\\", "/"),
            "scenario_sha256": scenario_sha, "scenario_version": SCENARIO_VERSION,
            "seed": scenario["seed"], "init_seed": scenario["init_seed"], "save": scenario["save"],
            "class": scenario["class"], "difficulty": scenario["difficulty"], "ticks": scenario["ticks"],
            "streams": scenario["streams"], "steps": len(scenario["steps"]), "args": args}


# --- game-side facts: specs/tools/original-hooks.md ---------------------------
# Every address below is cited to the section of the spec that owns it. What the
# spec does not settle yet is None: any use raises HookMissing (and a probe, below,
# prints what the coordinator needs), nothing is guessed.

class HookMissing(RuntimeError):
    pass


INJECT_POINT = 0x44F136   # §1.3, §3: `call drain` of the single-player client frame
INJECT_BYTES = bytes.fromhex("E8A5DE0D00")
TRANSPORT_SEND = 0x52AE50  # §1: stdcall (size, channel, message), ret 0xC, EAX 1 = queued
SEED_TIME = 0x52C2BB      # §2.1: `mov edx, eax` after the first time_value call
SEED_TIME_BYTES = bytes.fromhex("8BD0")
SEED_INIT = 0x52C2E3      # §2.1: `mov [edi+0x7C], eax`
SEED_INIT_BYTES = bytes.fromhex("89477C")
TICK_RETURN = 0x52FD1E    # §3: tick driver return, ESI = game, game+0xA8 = frame N
CLIENT_STATE_OK = 4       # §1.5: client list head game+0x88, state client+0x04
G_CLIENTS, C_STATE = rt.G_CLIENTS, 0x04
SCRATCH_SIZE = 0x1000     # §1.4.1: one RWX page, S+0 is the return trap, S+0x10 the message
SCRATCH_MSG = 0x10

# Unit fields. The spec's §4 is not written yet; these come from the specs that own
# them (cited) and `--probe unit_fields` prints the raw bytes to confirm them.
U_PATH, U_STATLIST = 0x2C, 0x5C           # units.md §2
PATH_DYN_X, PATH_DYN_Y = 0x02, 0x06       # path-placement.md §2.1 (types 0, 1, 3: u16 sub-tile)
PATH_STATIC_X, PATH_STATIC_Y = 0x0C, 0x10  # path-placement.md §2.1 (types 2, 4, 5: u32)
SL_FULL_ARRAY, SL_FULL_COUNT = 0x48, 0x4C  # stat-lists.md §1: extended list full array, i16 count
STAT_LIFE, STAT_MANA = 6, 8               # stats.md §2: stored in 1/256 points

# §5 start without a human
CLIENT_ENTRY = 0x44B8A0   # §5.4.3: stdcall, [ESP+8] = config; bytes 55 8B EC 56 8B 75
CLIENT_ENTRY_BYTES = bytes.fromhex("558BEC568B75")
CFG_DIFFICULTY = 0x210    # §5.2: config byte that becomes 0x67 byte +0x14
CFG_CLASS_EXTRA = {5: 0x8A, 6: 0x8B}   # §5.1.3: classes with no switch
CLASS_SWITCH = {0: "-ama", 1: "-sor", 2: "-nec", 3: "-pal", 4: "-bar"}
CLASS_NAMES = {"ama": 0, "sor": 1, "nec": 2, "pal": 3, "bar": 4, "dru": 5, "ass": 6}
GAME_MODE, NEXT_MODE, MENU_LOOP = 0x74C704, 0x7795E8, 0x72DDD4   # §5.4.2 (dump_tables.py)
MENU_MODE = 4
CLIENT_STATE_WORD = 0x70EF18  # §5 OQ2: logged while no tick runs
SAVE_FOPEN = 0x534410     # §5.3.2 / OQ4: [ESP] = path string
MANUAL_ARGS = ["-w", "-ns"]   # --manual-start: a person clicks through the menus


def start_args(sc):
    """§5.4.1 command line (classes 5, 6 have no switch; written to the config instead)."""
    args = ["-w", "-ns", "-nosave", "-name", sc["save"]]
    if sc["class"] in CLASS_SWITCH:
        args.append(CLASS_SWITCH[sc["class"]])
    return args


def need(value, what):
    if value is None:
        raise HookMissing(f"{what} is not known: specs/tools/original-hooks.md did not settle it")
    return value


def steps_due(steps, done_frame):
    """Steps to inject at the stop after tick `done_frame` returned (§3.1)."""
    return [s for s in steps if s["tick"] == done_frame + 1]


def stat_value(entries, count, stat, layer=0):
    """Value of (stat, layer) in a full array: 8-byte entries u16 layer, u16 stat, i32
    value (stat-lists.md §1); absent = 0 (a missing stat reads 0)."""
    for i in range(min(count, 4096)):
        lay, st, val = struct.unpack_from("<HHi", entries, 8 * i)
        if st == stat and lay == layer:
            return val
    return 0


G_SEED = 0xD0             # handoff scenario-harness §3 change 4: game seed (lo, hi) at game +0xD0
TRACE_FORMAT, TRACE_VERSION = "scenario-trace", 1      # traces/FORMAT.md §Scenario traces
RANK = {"c2s": 0, "spawn": 0, "s2c": 2, "rng": 3, "draw": 4, "unit": 5, "stats": 6, "end": 9}


class Unresolved(Exception):
    """A reference that does not resolve (scenario.md §3 rule 5): `ref` is the text."""

    def __init__(self, ref, why):
        super().__init__(f"{ref}: {why}")
        self.ref, self.why = ref, why


def resolve_ref(a, units, player):
    """Value of an exported reference (scenario.md §3 rule 3) on `units`: dicts with
    t (type), cl (class), id (GUID), x, y; `player` is the type-0 unit or None."""
    k = a["kind"]
    if k == "player":
        if not player:
            raise Unresolved(a["ref"], "no player")
        return player["id"]
    if k in ("x", "y"):
        if not player:
            raise Unresolved(a["ref"], "no player")
        return player[k] + a["d"]
    if k == "wp":
        raise Unresolved(a["ref"], "waypoint objects are not resolved by this tool "
                                   "(the objects table is not read)")
    guids = sorted(u["id"] for u in units if u["t"] == a["ty"]
                   and (a["class"] is None or u["cl"] == a["class"]))
    if a["n"] >= len(guids):
        raise Unresolved(a["ref"], "no such unit")
    return guids[a["n"]]


def encode_step(step, units, player):
    """Bytes of an exported step (scenario.md §3 rule 2): the row's size, byte 0 the
    id, fields little endian at their offsets, `bits` fields OR-ed into the u32."""
    if "hex" in step:
        return bytes.fromhex(step["hex"])
    msg = bytearray(step["size"])
    msg[0] = step["id"]
    for f in step["fields"]:
        a = f["arg"]
        v = a["num"] if "num" in a else resolve_ref(a, units, player)
        top = 0xFFFFFFFF if f["bits"] == 32 else (1 << f["bits"]) - 1
        if not 0 <= v <= top:
            raise Unresolved(a.get("ref", str(v)), f"value {v} does not fit field {f['name']}")
        off = f["offset"]
        if f["ty"] == "u8":
            msg[off] = v
        elif f["ty"] == "u16":
            msg[off:off + 2] = v.to_bytes(2, "little")
        elif f["ty"] == "u32":
            msg[off:off + 4] = v.to_bytes(4, "little")
        else:
            old = int.from_bytes(msg[off:off + 4], "little")
            msg[off:off + 4] = (old | (v << f["shift"])).to_bytes(4, "little")
    return bytes(msg)


def trace_lines(header, recs):
    """scenario-trace lines: header, records by (tick, stream rank, order), `end` last."""
    order = sorted(recs, key=lambda r: (r[0], r[1], r[2]))

    def dump(o):
        return json.dumps(o, sort_keys=True, separators=(",", ":")) + "\n"
    return [dump(header)] + [dump(r[3]) for r in order]


# --- the recorder --------------------------------------------------------------

class ScenarioRecorder(rp.PacketRecorder):
    """PacketRecorder (message hooks + the shared debug loop, which also hooks
    the RNG helpers and setters) plus seed override, message injection and
    per-tick unit snapshots."""

    def __init__(self, exe, args, writer, scenario, seconds, probe=None, probe_frame=None,
                 auto_start=False, force_after=6.0):
        super().__init__(exe, args, "", seconds, 0)
        self.out, self.writer = writer, writer
        self.sc = scenario
        self.want = set(scenario["streams"])
        self.game = None
        self.last_frame = None         # frame of the last tick that returned
        self.todo = list(scenario["steps"])
        self.queue = []                # messages of the stop being injected
        self.saved = None              # thread context at the injection point
        self.inj_tid = None
        self.scratch = None
        self.injected = 0
        self.recording = False
        self.failed = None
        self.probe, self.probe_frame = probe, probe_frame
        self.probe_out = []
        self.seeds_done = set()
        self.max_events = 0
        self.auto_start, self.force_after = auto_start, force_after
        self.forced = False
        self.stopping = False
        # scenario-trace mode (`.scenario` scripts): relative ticks, records in memory
        self.rel = bool(scenario.get("rel"))
        self.f0 = None                 # frame of the tick in which the client first is in state 4
        self.open_t = None             # tick whose stop has been handled
        self.win = False               # between that stop and the tick's return (s2c window)
        self.tr = []                   # (t, rank, order, record)
        self.rng_before = None
        self.queue_i = []
        self.cur_i = None
        self.ended = False
        self.dispatch_open = False

    # --- output: deterministic (no wall-clock ms, no thread ids) -----------

    def trace_add(self, rec):
        self.tr.append((rec["t"], RANK[rec["k"]], len(self.tr), rec))

    def emit(self, rec):
        if self.rel and rec["type"] == "s2c" and self.win and rec.get("client") == 0:
            # scenario.md §4 rule 5: messages queued for client 0 in this tick's window
            self.trace_add({"k": "s2c", "t": self.open_t, "c": 0, "b": rec["bytes"]})
        stream = STREAM_OF.get(rec["type"])
        if (stream is not None and stream not in self.want) or not self.recording:
            return
        rec.pop("tid", None)
        rec["seq"] = self.seq
        self.seq += 1
        self.count(":".join(x for x in (rec["type"], rec.get("via"), rec.get("op")) if x))
        self.writer.record(rec)

    def stop(self, why, failed=False):
        self.notes.append(why)
        if failed:
            self.failed = why
        self.max_events = max(1, self.seq)  # the shared loop stops on its event limit

    # --- install -----------------------------------------------------------

    def install(self, base):
        if base != rr.IMAGE_BASE:
            raise RuntimeError(f"Game.exe loaded at {base:#x}, expected {rr.IMAGE_BASE:#x}")
        needed = {a: rp.HOOKS[a][1] for a in rp.HOOKS
                  if "packets" in self.want or rp.HOOKS[a][0] == "tick"}
        for addr, first in needed.items():
            if self.read(addr, len(first) // 2) != bytes.fromhex(first):
                raise RuntimeError(f"unexpected code at {addr:#x}: not the 1.14d Game.exe?")
            self.add_role(addr, "pkt")
        for addr, want in ((INJECT_POINT, INJECT_BYTES), (SEED_TIME, SEED_TIME_BYTES),
                           (SEED_INIT, SEED_INIT_BYTES), (TICK_RETURN, bytes.fromhex("8b7618"))):
            if addr in needed:
                continue   # hooked and checked by the loop above (the INT3 now sits there)
            if self.read(addr, len(want)) != want:
                raise RuntimeError(f"unexpected code at {addr:#x}: not the 1.14d Game.exe?")
            self.add_role(addr, "pkt")
        if self.auto_start:
            if self.read(CLIENT_ENTRY, 6) != CLIENT_ENTRY_BYTES:
                raise RuntimeError(f"unexpected code at {CLIENT_ENTRY:#x}: not the 1.14d Game.exe?")
            self.add_role(CLIENT_ENTRY, "start")
        if self.probe == "savepath":
            self.add_role(SAVE_FOPEN, "start")       # bytes not in the spec: not checked
        if "rng" in self.want:
            rr.Recorder.install(self, base)
        return 0

    # --- breakpoints -------------------------------------------------------

    def on_breakpoint(self, tid, addr):
        if addr in (SEED_TIME, SEED_INIT):
            self.override_seed(tid, addr)           # §2.1
        elif addr == CLIENT_ENTRY:
            self.configure_client(tid)              # §5.4.3
        elif addr == SAVE_FOPEN:
            self.log_save_path(tid)                 # OQ4
        elif addr == INJECT_POINT and self.inject_stop(tid):
            return                                  # redirected into the send; no step-over yet
        super().on_breakpoint(tid, addr)

    def on_exception(self, tid, info):
        rec = info.ExceptionRecord
        if self.scratch and (rec.ExceptionAddress or 0) == self.scratch \
                and rec.ExceptionCode in rr.BREAKPOINT_CODES:
            self.inject_return(tid)
            return rr.DBG_CONTINUE
        return super().on_exception(tid, info)

    def on_hook(self, tid, addr, ctx):
        if addr not in rp.HOOKS:
            return
        kind = rp.HOOKS[addr][0]
        if kind == "tick":
            if self.game is None:
                self.game = ctx.Ecx
                self.recording = True
            elif ctx.Ecx != self.game:
                return
        if kind in ("c2s", "c2s_sys", "drain"):
            self.dispatch_open = False
        elif kind == "dispatch":
            self.dispatch_open = True
        elif kind == "result" and self.dispatch_open and self.last_kind in ("s2c", "net"):
            # §1 rule 6: the result at 0x0053F45E belongs to the dispatch before it; a handler
            # that queues a reply (s2c, net) between them made the base class call it "not
            # dispatched" and drop its code
            self.last_kind = "dispatch"
        super().on_hook(tid, addr, ctx)
        if addr == TICK_RETURN and ctx.Esi == self.game:
            self.tick_done(self.read_i32(self.game + rt.G_FRAME))

    # §5.4.3: difficulty (and class 5, 6) into the config before the handlers copy them
    def configure_client(self, tid):
        ctx = self.get_ctx(tid)
        cfg = self.read_u32(ctx.Esp + 8)
        self.write(cfg + CFG_DIFFICULTY, bytes([self.sc["difficulty"]]))
        if self.sc["class"] in CFG_CLASS_EXTRA:
            self.write(cfg + CFG_CLASS_EXTRA[self.sc["class"]], b"\x01")
        self.notes.append(f"client entry: config {cfg:#x}, difficulty {self.sc['difficulty']}, "
                          f"class {self.sc['class']}")

    def log_save_path(self, tid):
        ctx = self.get_ctx(tid)
        ptr = self.read_u32(ctx.Esp)
        path = self.read(ptr, 260).split(b"\0")[0].decode("latin-1")
        print("save path read at 0x534410:", path)
        self.notes.append(f"save path: {path}")
        if self.probe == "savepath":
            print(f"RESULT save path (OQ4): {path}")
            self.stop("probe savepath done")

    # §5.4.2: end the menu the way starting a game does (as dump_tables.py)
    def force_menu(self):
        t0 = time.perf_counter()
        last = 0
        while not self.stopping:
            time.sleep(0.1)
            try:
                el = time.perf_counter() - t0
                if not self.forced and el >= self.force_after and self.read_u32(GAME_MODE) == MENU_MODE:
                    self.write(NEXT_MODE, struct.pack("<I", 1))
                    self.write(MENU_LOOP, struct.pack("<I", 0))
                    self.forced = True
                    self.notes.append(f"menu ended by the tool at {el:.1f}s")
                elif self.forced and self.game is None and el - last >= 2:   # OQ2 diagnostics
                    last = el
                    print(f"{el:.0f}s: no tick yet, 0x70EF18 = {self.read_u32(CLIENT_STATE_WORD):#x}")
            except OSError:
                pass

    # §2.1: replace the clock-derived values before the first draw of the game seed
    def override_seed(self, tid, addr):
        ctx = self.get_ctx(tid)
        ctx.Eip = addr
        which = "time" if addr == SEED_TIME else "init"
        val = self.sc["seed"] if which == "time" else self.sc["init_seed"]
        self.recording = True                      # from game creation on
        self.emit({"type": "seed_override", "which": which, "old": ctx.Eax, "new": val})
        ctx.Eax = val
        self.set_ctx(tid, ctx)
        self.seeds_done.add(which)

    # --- injection (§1.4) --------------------------------------------------

    def client_state(self):
        """State of the first client of the game (§1.5), or None."""
        head = self.read_u32(self.game + G_CLIENTS)
        return self.read_u32(head + C_STATE) if head else None

    def inject_stop(self, tid):
        """At the drain call. True = a message call was started (thread redirected)."""
        if self.rel:
            return self.inject_stop_rel(tid)
        if self.game is None or self.last_frame is None:
            return False
        due = steps_due(self.todo, self.last_frame)
        if not due:
            return False
        if self.client_state() != CLIENT_STATE_OK:
            self.stop(f"step at tick {due[0]['tick']}: client not in state {CLIENT_STATE_OK} "
                      f"(is {self.client_state()}); the first usable tick is later (§1.5)", True)
            return False
        for s in due:
            self.todo.remove(s)
        self.queue = [s["c2s"] for s in due]
        ctx = self.get_ctx(tid)
        ctx.Eip = INJECT_POINT
        self.saved, self.inj_tid = ctx, tid
        self.inj_tick = due[0]["tick"]
        self.start_call(tid)
        return True

    def game_seed(self):
        lo, hi = struct.unpack("<II", self.read(self.game + G_SEED, 8))
        return [lo, hi]

    def inject_stop_rel(self, tid):
        """Relative-tick stop (scenario.md §4 rule 2): the first stop after frame F0 + t
        returned handles tick t: rng before, resolve and inject the steps of tick t."""
        if self.game is None or self.last_frame is None or self.f0 is None:
            return False
        t = self.last_frame - self.f0
        if t > self.sc["ticks"]:
            return False
        if self.open_t != t:
            self.open_t, self.win = t, True
            self.rng_before = self.game_seed()
            due = [s for s in self.todo if s["tick"] == t]
            for s in due:
                self.todo.remove(s)
            units = [self.unit_record(u) for u in self.server_units()]
            player = next((u for u in sorted(units, key=lambda r: r["id"]) if u["t"] == 0), None)
            self.queue, self.queue_i = [], []
            for i, s in enumerate(due):
                if s.get("spawn"):      # handoff §3 change 5: not in the tool
                    self.trace_add({"k": "spawn", "t": t, "i": i, "failed": True})
                    continue
                try:
                    self.queue.append(encode_step(s, units, player))
                    self.queue_i.append(i)
                except Unresolved as e:
                    self.trace_add({"k": "c2s", "t": t, "i": i, "unresolved": e.ref})
                    self.notes.append(f"tick {t} step {i}: unresolved: {e.why}")
        if not self.queue:
            return False
        ctx = self.get_ctx(tid)
        ctx.Eip = INJECT_POINT
        self.saved, self.inj_tid, self.inj_tick = ctx, tid, t
        self.start_call(tid)
        return True

    def rel_tick_done(self, frame):
        """Tick return (§3): find F0, then the records of tick t = frame - F0 - 1."""
        if self.f0 is None:
            if self.client_state() == CLIENT_STATE_OK:
                self.f0 = frame
            return
        t = frame - self.f0 - 1
        self.win = False
        if t < 0:
            return
        ex = self.sc["export"]
        if "rng" in ex["streams"] and self.rng_before is not None:
            self.trace_add({"k": "rng", "t": t, "before": self.rng_before,
                            "after": self.game_seed()})
        if "units" in ex["streams"] and t in ex["snapshot_ticks"]:
            for u in sorted((self.unit_record(u) for u in self.server_units()),
                            key=lambda r: (r["t"], r["id"])):
                self.trace_add({"k": "unit", "t": t, "type": u["t"], "guid": u["id"],
                                "class": u["cl"], "mode": u["m"], "x": u["x"], "y": u["y"],
                                "life": u["life"] or 0, "mana": u["mana"] or 0})
        late = [s for s in self.todo if s["tick"] <= t]
        if late:
            self.stop(f"step at tick {late[0]['tick']} was not injected (tick {t} already ran)", True)
            return
        if t >= ex["end"]:
            self.trace_add({"k": "end", "t": t})
            self.ended = True
            self.stop(f"tick {t} recorded (end)")

    def trace_header(self):
        ex = self.sc["export"]
        gaps = []
        if ex["inline_lines"]:
            gaps.append(f"char lines are not applied: the save {self.sc['save']} is loaded")
        if self.sc.get("save_overridden"):
            gaps.append(f"save {self.sc['save']} stands in for the script's {ex['save']}")
        if any(s.get("spawn") for s in ex["steps"]):
            gaps.append("spawn: not supported")
        if any((f["arg"] or {}).get("kind") == "wp" for s in ex["steps"] for f in s.get("fields", [])):
            gaps.append("@wp references are not resolved (objects table not read)")
        for st, why in (("stats", "base array of the extended stat list not specified"),
                        ("frames", "not recorded")):
            if st in ex["streams"]:
                gaps.append(f"{st}: {why}")
        streams = ["c2s"] + [x for x in ex["streams"] if x in ("s2c", "rng", "units")]
        return {"k": "header", "format": TRACE_FORMAT, "version": TRACE_VERSION,
                "game_version": "1.14d", "side": "original", "tool": TOOL, "data": "1.14d",
                "scenario": ex["name"], "scenario_sha256": ex["sha256"], "seed": ex["seed"],
                "init": ex["init"], "end": ex["end"], "streams": sorted(streams),
                "gaps": sorted(gaps)}

    def poke(self, addr, data):
        """Write without changing page protection (the stack)."""
        buf = (C.c_ubyte * len(data)).from_buffer_copy(data)
        got = C.c_size_t()
        if not rr.WriteProcessMemory(self.h_process, C.c_void_p(addr), buf, len(data), C.byref(got)):
            raise rr.winerr(f"WriteProcessMemory {addr:#x}")

    def start_call(self, tid):
        if self.scratch is None:  # §1.4.1
            self.scratch = need_alloc(self.h_process, SCRATCH_SIZE)
            self.write(self.scratch, rr.INT3)
        msg = self.queue.pop(0)
        self.cur_i = self.queue_i.pop(0) if self.queue_i else None
        self.cur_msg = msg
        self.write(self.scratch + SCRATCH_MSG, msg)
        ctx = type(self.saved).from_buffer_copy(self.saved)
        ctx.Esp = (self.saved.Esp - 16) & rr.M32
        self.poke(ctx.Esp, struct.pack("<4I", self.scratch, len(msg), 1,
                                       self.scratch + SCRATCH_MSG))
        ctx.Eip = TRANSPORT_SEND
        self.set_ctx(tid, ctx)

    def inject_return(self, tid):
        ctx = self.get_ctx(tid)
        ok = ctx.Eax == 1
        self.emit({"type": "inject", "tick": self.inj_tick, "c2s": self.cur_msg.hex(),
                   "result": ctx.Eax, "via": "0x52ae50 at 0x44f136"})
        if not ok:
            self.stop(f"injection at tick {self.inj_tick} refused (send returned {ctx.Eax}): "
                      f"message {self.cur_msg.hex()}", True)
        else:
            self.injected += 1
            if self.rel:
                self.trace_add({"k": "c2s", "t": self.inj_tick, "i": self.cur_i,
                                "b": self.cur_msg.hex()})
        if self.queue and ok:
            self.start_call(tid)
            return
        self.set_ctx(tid, self.saved)     # back at the drain call, as before the injection
        self.saved = None
        rr.Recorder.on_breakpoint(self, tid, INJECT_POINT)  # step over the original call

    # --- tick boundary (§3.3) ----------------------------------------------

    def tick_done(self, frame):
        self.last_frame = frame
        if self.rel:
            self.rel_tick_done(frame)
            return
        self.snapshot_units(frame)
        for s in self.todo:
            if s["tick"] <= frame:
                self.stop(f"step at tick {s['tick']} was not injected (tick {frame} already ran; "
                          f"the client was not ready one tick earlier, or the tick is before the "
                          f"first usable one)", True)
                return
        if self.probe:
            self.probe_tick(frame)
        if frame >= self.sc["ticks"]:
            self.stop(f"{self.sc['ticks']} ticks recorded")

    def read_i32(self, addr):
        return struct.unpack("<i", self.read(addr, 4))[0]

    # --- units stream ------------------------------------------------------

    u32 = rt.TickRecorder.u32
    i32 = rt.TickRecorder.i32
    walk = rt.TickRecorder.walk

    def server_units(self):
        out = []
        for t, off in rt.HASH_OFFSETS.items():
            heads = struct.unpack("<128I", self.read(self.game + rt.HASH_BASE + off, 512))
            for head in heads:
                for u in self.walk(head, rt.U_HASH_NEXT) if head else ():
                    if self.u32(u + rt.U_FLAGS2) & rt.SERVER_UNIT:
                        out.append(u)
        return out

    def snapshot_units(self, frame):
        if "units" not in self.want and not self.probe:
            return
        units = sorted((self.unit_record(u) for u in self.server_units()),
                       key=lambda r: (r["t"], r["id"]))
        self.emit({"type": "units", "tick": frame, "units": units})

    def unit_record(self, u):
        t = self.u32(u + rt.U_TYPE)
        rec = {"id": self.u32(u + rt.U_GUID), "t": t, "cl": self.u32(u + rt.U_CLASS),
               "m": self.u32(u + rt.U_MODE)}
        path = self.u32(u + U_PATH)
        if not path:
            rec["x"] = rec["y"] = 0                 # path-placement.md §2.1
        elif t in (0, 1, 3):
            rec["x"], rec["y"] = struct.unpack("<HH", self.read(path + PATH_DYN_X, 2) +
                                               self.read(path + PATH_DYN_Y, 2))
        else:
            rec["x"], rec["y"] = struct.unpack("<II", self.read(path + PATH_STATIC_X, 4) +
                                               self.read(path + PATH_STATIC_Y, 4))
        sl = self.u32(u + U_STATLIST)
        for key, stat in (("life", STAT_LIFE), ("mana", STAT_MANA)):
            rec[key] = self.stat_of(sl, stat) if sl else None   # raw, 1/256 points
        return rec

    def stat_of(self, statlist, stat):
        n = struct.unpack("<h", self.read(statlist + SL_FULL_COUNT, 2))[0]
        arr = self.u32(statlist + SL_FULL_ARRAY)
        if n <= 0 or not arr:
            return 0
        return stat_value(self.read(arr, 8 * min(n, 4096)), n, stat)

    # --- probes (run with the scenario machinery; see PROBES) --------------

    def probe_tick(self, frame):
        if self.probe == "ready":
            st = self.client_state()
            print(f"frame {frame}: client state {st}")
            if st == CLIENT_STATE_OK:
                print(f"RESULT first ready frame {frame}: inject at tick >= {frame + 1}")
                self.stop("probe ready done")
        elif self.probe == "unit_fields" and frame >= self.probe_frame:
            for u in self.server_units():
                if self.u32(u + rt.U_TYPE) != 0:
                    continue
                print(f"player unit {u:#x}, record:", self.unit_record(u))
                print("unit +0x00..0x100:", self.read(u, 0x100).hex(" ", 4))
                path = self.u32(u + U_PATH)
                print(f"path {path:#x} +0x00..0x40:", self.read(path, 0x40).hex(" ", 4))
                sl = self.u32(u + U_STATLIST)
                n = struct.unpack("<h", self.read(sl + SL_FULL_COUNT, 2))[0]
                arr = self.u32(sl + SL_FULL_ARRAY)
                print(f"full stat array {arr:#x}, {n} entries (layer, stat, value):")
                for i in range(min(n, 200)):
                    print("  ", struct.unpack("<HHi", self.read(arr + 8 * i, 8)))
                print("RESULT compare these with the in-game values (life, mana, position)")
            self.stop("probe unit_fields done")


def need_alloc(h_process, size):
    k32 = rr.k32
    k32.VirtualAllocEx.restype = C.c_void_p
    k32.VirtualAllocEx.argtypes = [C.c_void_p, C.c_void_p, C.c_size_t, C.c_uint32, C.c_uint32]
    p = k32.VirtualAllocEx(h_process, None, size, 0x3000, 0x40)  # commit|reserve, RWX
    if not p:
        raise rr.winerr("VirtualAllocEx")
    return p


def run_game(rec):
    """Start the game under the debugger and run the shared loop (the process part of
    record_rng.Recorder.run; header and footer are written by the caller)."""
    si = rr.STARTUPINFOW()
    si.cb = C.sizeof(si)
    pi = rr.PROCESS_INFORMATION()
    cmd = C.create_unicode_buffer(" ".join([f'"{rec.exe}"'] + rec.args))
    if not rr.CreateProcessW(rec.exe, cmd, None, None, False, rr.DEBUG_ONLY_THIS_PROCESS,
                             None, os.path.dirname(rec.exe), C.byref(si), C.byref(pi)):
        raise rr.winerr("CreateProcessW")
    rec.h_process, rec.pid = pi.hProcess, pi.dwProcessId
    rr.DebugSetProcessKillOnExit(True)
    rec.t0 = time.perf_counter()
    if rec.auto_start:
        threading.Thread(target=rec.force_menu, daemon=True).start()
    try:
        rec.loop(rec.t0 + rec.seconds)
    finally:
        rec.stopping = True
        rec.kill()
        rr.CloseHandle(pi.hThread)


# --- probes ----------------------------------------------------------------------
# Each answers an open question of original-hooks.md by running the game (the person at
# the keyboard starts a game by hand: --manual-start) and printing a RESULT line.

def probe_scenario(a, name, ticks, steps, streams):
    return {"seed": 1, "init_seed": 1, "save": a.save, "class": CLASS_NAMES[a.cls],
            "difficulty": a.difficulty, "ticks": ticks, "streams": streams, "steps": steps}


def probe_run(a, name, sc, summarize=None):
    out = os.path.join(REPO, "traces", "raw",
                       datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + f"-probe-{name}.jsonl")
    game = check_game(a.game)
    writer = ScenarioWriter(out)
    args = MANUAL_ARGS if a.manual_start else start_args(sc)
    r = ScenarioRecorder(game, args, writer, sc, a.seconds, probe=name,
                         probe_frame=a.probe_frame, auto_start=not a.manual_start,
                         force_after=a.force_after)
    writer.header(make_header(sc, f"probe:{name}", "-", rr.GAME_EXE_SHA256, args))
    print(f"probe {name}: Game.exe {' '.join(args)}"
          + ("  (start a single-player game by hand)" if a.manual_start else ""))
    try:
        run_game(r)
    finally:
        writer.footer({"counts": r.counts, "notes": r.notes})
        writer.close()
    if summarize:
        summarize(out)
    for n in r.notes:
        print("note:", n)
    return 4 if r.failed else 0


def records_of(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f]


def probe_inject(a):
    """Open question 1: does a 0x01 walk queued through 0x52AE50 at 0x44F136 give c2s,
    dispatch and result 0 in the same drain, with dispatch.game_frame = N-1?"""
    n = a.probe_frame
    walk = bytes.fromhex("01f113b513")
    sc = probe_scenario(a, "inject", n + 3, [{"tick": n, "c2s": walk}], ["packets"])

    def summarize(path):
        recs = [r for r in records_of(path) if r.get("type") in
                ("inject", "c2s", "dispatch", "result")]
        shown = [r for r in recs if r["type"] == "inject" or r.get("frame") in (n - 1, n)]
        for r in shown:
            print({k: v for k, v in r.items() if k not in ("seq", "tid")})
        d = [r for r in recs if r["type"] == "dispatch" and r.get("id") == 1
             and r.get("game_frame") == n - 1]
        res = [r for r in recs if r["type"] == "result" and r.get("dispatched")]
        c = [r for r in recs if r["type"] == "c2s" and r.get("bytes") == walk.hex()]
        print(f"RESULT inject at {n}: c2s={len(c)} dispatch(frame {n - 1})={len(d)} "
              f"result codes={[r.get('code') for r in res[-1:]]}: "
              f"{'ANSWERS Q1 yes' if c and d and res and res[-1].get('code') == 0 else 'DOES NOT MATCH'}")
    return probe_run(a, "inject", sc, summarize)


def probe_seed(a):
    """OQ3: with T and I set, does the first game-seed draw chain from one step of
    {T, 666}, and do two runs give identical rng chains? Runs the game twice."""
    sc = probe_scenario(a, "seed", 5, [], ["rng"])
    sc["seed"], sc["init_seed"] = 0x1234567, 0x89ABCDEF
    hashes = []

    def summarize(path):
        rng = [r for r in records_of(path) if r.get("type") in ("seed_override", "seed_set", "draw")]
        for r in rng[:6]:
            print({k: v for k, v in r.items() if k not in ("seq", "ms")})
        n = sum(1 for r in rng if r["type"] == "seed_override")
        h = hashlib.sha256("".join(ScenarioWriter.line(r) for r in rng).encode()).hexdigest()
        hashes.append((n, h))
        print(f"run {len(hashes)}: {n} seed_override records (want 2), {len(rng)} rng records, "
              f"sha256 {h[:16]}")
    rc = probe_run(a, "seed", sc, summarize)
    if not rc:
        rc = probe_run(a, "seed", sc, summarize)
    if len(hashes) == 2:
        print(f"RESULT OQ3 two runs identical: {hashes[0] == hashes[1] and hashes[0][0] == 2}; "
              f"check by eye that the first draw's seed-before equals one step of "
              f"{{{sc['seed']:#x}, 666}}")
    return rc


def probe_ready(a):
    """Open question: the first tick at which the client is in state 4 (first usable N)."""
    return probe_run(a, "ready", probe_scenario(a, "ready", 100000, [], ["units"]))


def probe_start(a):
    """OQ2: after the forced start does the client send 0x67 with the name, class and
    difficulty, and does tick 1 run with no input?"""
    sc = probe_scenario(a, "start", 3, [], ["packets"])

    def summarize(path):
        recs = records_of(path)
        sysmsgs = [r for r in recs if r.get("type") == "c2s_sys" and r.get("bytes", "")[:2] == "67"]
        for r in sysmsgs[:2]:
            b = bytes.fromhex(r["bytes"])
            print("0x67:", {"type": b[0x11] if len(b) > 0x11 else None,
                            "class": b[0x12] if len(b) > 0x12 else None,
                            "difficulty": b[0x14] if len(b) > 0x14 else None,
                            "name": b[0x15:0x25].split(b"\0")[0].decode("latin-1")
                            if len(b) >= 0x25 else None})
        ticks = [r for r in recs if r.get("type") == "tick"]
        print(f"RESULT OQ2 0x67 sent: {bool(sysmsgs)}; first tick frame: "
              f"{ticks[0]['frame'] if ticks else None} (want 1)")
    return probe_run(a, "start", sc, summarize)


def probe_savepath(a):
    """OQ4: the save path at the fopen of the save loader (0x534410)."""
    return probe_run(a, "savepath", probe_scenario(a, "savepath", 100000, [], ["units"]))


def probe_unit_fields(a):
    """Unit snapshot fields (§4 is not written yet): raw dumps of the player unit."""
    n = a.probe_frame
    return probe_run(a, "unit_fields", probe_scenario(a, "unit_fields", n + 1, [], ["units"]))


PROBES = {"start": probe_start, "savepath": probe_savepath, "inject": probe_inject, "seed": probe_seed, "ready": probe_ready,
          "unit_fields": probe_unit_fields}


# --- plan, selftest, main ----------------------------------------------------

def check_game(path):
    game = os.path.abspath(path)
    with open(game, "rb") as f:
        sha = hashlib.sha256(f.read()).hexdigest()
    if sha != rr.GAME_EXE_SHA256:
        raise RuntimeError(f"{game}: sha256 {sha} is not the reference 1.14d Game.exe")
    return game


def plan_text(sc, path, sha, game):
    lines = [f"scenario   {path} (sha256 {sha[:16]}...)",
             f"game       {game}",
             f"save       {sc['save']}  difficulty {sc['difficulty']}",
             f"seed       time value {sc['seed']:#x}, init value {sc['init_seed']:#x} "
             f"(overridden at {SEED_TIME:#x} and {SEED_INIT:#x}, spec §2.1)",
             f"ticks      frames 1..{sc['ticks']}   streams {', '.join(sc['streams'])}",
             f"steps      {len(sc['steps'])}"]
    for s in sc["steps"]:
        lines.append(f"  tick {s['tick']:>6}  C->S {len(s['c2s'])} bytes  {s['c2s'].hex(' ')}"
                     f"  (queued via {TRANSPORT_SEND:#x} at {INJECT_POINT:#x} after tick "
                     f"{s['tick'] - 1}, spec §1, §3.1)")
    lines.append("start-up   Game.exe " + " ".join(start_args(sc)) + f"  (spec §5.4: menu ended by "
                 f"the tool, difficulty {sc['difficulty']} written to config +{CFG_DIFFICULTY:#x} "
                 f"at {CLIENT_ENTRY:#x}); --manual-start: Game.exe {' '.join(MANUAL_ARGS)}, "
                 f"a person starts the game")
    return "\n".join(lines)


def selftest():
    fails = []

    def check(cond, what):
        if not cond:
            fails.append(what)

    def bad(text, frag):
        try:
            parse_scenario(text)
        except ScenarioError as e:
            check(frag in str(e), f"error for {frag!r} was {e}")
        else:
            fails.append(f"accepted a bad scenario ({frag})")

    base = {"version": 1, "seed": 7, "save": "Scenario", "difficulty": 0, "ticks": 20,
            "streams": ["units", "rng"], "steps": [{"tick": 5, "c2s": "01 f1 13 b5 13"},
                                                   {"tick": 2, "c2s": "0x02,00"}]}
    ok = parse_scenario(json.dumps(base))
    check([s["tick"] for s in ok["steps"]] == [2, 5], "steps sorted by tick")
    check(ok["steps"][1]["c2s"] == bytes.fromhex("01f113b513"), "hex bytes")
    check(ok["streams"] == ["units", "rng"] and ok["seed"] == 7, "fields")
    minimal = {k: base[k] for k in ("version", "seed", "save", "ticks")}
    m = parse_scenario(json.dumps(minimal))
    check(m["streams"] == list(STREAMS) and m["steps"] == [] and m["difficulty"] == 0, "defaults")
    # stable order for same-tick steps
    two = dict(base, steps=[{"tick": 3, "c2s": "aa"}, {"tick": 3, "c2s": "bb"}])
    check([s["c2s"] for s in parse_scenario(json.dumps(two))["steps"]] == [b"\xaa", b"\xbb"],
          "same-tick order")
    # every field error, one at a time
    for patch, frag in [({"version": 2}, "version"), ({"seed": -1}, "seed"),
                        ({"seed": 2 ** 31}, "seed"), ({"init_seed": 2 ** 32}, "init_seed"), ({"seed": "1"}, "seed"),
                        ({"seed": True}, "seed"), ({"ticks": 0}, "ticks"),
                        ({"ticks": 1.5}, "ticks"), ({"save": ""}, "save"),
                        ({"save": "../x"}, "save"), ({"save": 3}, "save"),
                        ({"difficulty": 3}, "difficulty"), ({"streams": []}, "streams"),
                        ({"streams": ["frames"]}, "unknown stream"),
                        ({"streams": ["rng", "rng"]}, "duplicate"),
                        ({"steps": {}}, "steps"), ({"steps": [3]}, "object"),
                        ({"steps": [{"tick": 20, "c2s": "01"}]}, "tick"),
                        ({"steps": [{"tick": -1, "c2s": "01"}]}, "tick"),
                        ({"steps": [{"tick": 1}]}, "hex"),
                        ({"steps": [{"tick": 1, "c2s": "0"}]}, "even"),
                        ({"steps": [{"tick": 1, "c2s": "zz"}]}, "even"),
                        ({"steps": [{"tick": 1, "c2s": ""}]}, "empty"),
                        ({"steps": [{"tick": 1, "c2s": "00" * 0x1FD}]}, "at most")]:
        bad(json.dumps(dict(base, **patch)), frag)
    bad("{", "not valid JSON")
    bad("[]", "object")
    d2 = dict(base)
    del d2["seed"]
    bad(json.dumps(d2), "seed")
    for t, want in [("01 02", b"\x01\x02"), ("0102", b"\x01\x02"), ("01:02", b"\x01\x02"),
                    ("0X01 0x02", b"\x01\x02")]:
        check(parse_hex(t) == want, f"parse_hex {t!r}")

    # the sample scenario in the repository must parse
    sample = os.path.join(REPO, "traces", "scenarios", "local", "walk-in-town.json")
    if os.path.exists(sample):
        sc, sha = load_scenario(sample)
        check(sc["ticks"] == 200 and sc["steps"][0]["c2s"][0] == 0x01 and len(sha) == 64,
              "sample scenario")

    # writer output, header fields and the perturbation check
    sc = ok
    records = [{"type": "tick_no", "n": i, "frame": 100 + i} for i in range(5)]
    records.append({"type": "units", "tick": 4, "units": [{"id": 1, "t": 0, "cl": 1, "m": 1,
                                                           "x": None, "y": None, "life": None,
                                                           "mana": None}]})

    def write(recs):
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, "out", "t-scenario.jsonl")
            w = ScenarioWriter(p)
            w.header(make_header(sc, "traces/scenarios/local/x.json", "ab" * 32, "cd" * 32, ["-w"]))
            for r in recs:
                w.record(dict(r))
            w.footer({"notes": []})
            w.close()
            with open(p, encoding="utf-8", newline="") as f:
                text = f.read()
        return text.split("\n")[:-1]

    lines = write(records)
    hdr, ftr = json.loads(lines[0]), json.loads(lines[-1])
    check(hdr["format"] == "scenario-raw-0" and hdr["scenario_sha256"] == "ab" * 32
          and hdr["game_exe_sha256"] == "cd" * 32 and hdr["scenario"].endswith("x.json"),
          "header fields")
    check(len(lines) == len(records) + 2 and ftr["records"] == len(records), "record count")
    check(write(records) == lines, "output is deterministic")
    base_hash = ftr["records_sha256"]
    for i in range(len(records)):
        pert = [dict(r) for r in records]
        if "n" in pert[i]:
            pert[i]["n"] += 1
        else:
            pert[i]["tick"] += 1
        check(json.loads(write(pert)[-1])["records_sha256"] != base_hash,
              f"perturbing record {i} did not change the hash")
    check(json.loads(write(records[:-1])[-1])["records_sha256"] != base_hash,
          "dropping a record did not change the hash")
    check(json.loads(write(records[::-1])[-1])["records_sha256"] != base_hash,
          "reordering did not change the hash")

    # injection schedule (§3.1): a step for tick N is due after tick N-1 returned
    st = [{"tick": 5, "c2s": b"a"}, {"tick": 5, "c2s": b"b"}, {"tick": 7, "c2s": b"c"}]
    check([s["c2s"] for s in steps_due(st, 4)] == [b"a", b"b"], "steps due after tick 4")
    check(steps_due(st, 5) == [] and len(steps_due(st, 6)) == 1, "steps due after 5 and 6")

    # unit field readers on synthetic game memory (perturbation: each field is read from
    # its own offset, so changing one byte changes exactly that field)
    class Fake(ScenarioRecorder):
        def __init__(self, mem):
            self.mem = mem

        def read(self, addr, n):
            return bytes(self.mem.get(addr + i, 0) for i in range(n))

        read_u32 = rr.Recorder.read_u32

    def put(mem, addr, data):
        for i, b in enumerate(data):
            mem[addr + i] = b

    def make():
        mem = {}
        u, path, sl, arr = 0x1000, 0x2000, 0x3000, 0x4000
        put(mem, u, struct.pack("<IIII", 0, 1, 0, 0x55))   # type, class, -, guid at +0x0C
        put(mem, u + 0x10, struct.pack("<I", 2))            # mode
        put(mem, u + 0x2C, struct.pack("<I", path))
        put(mem, u + 0x5C, struct.pack("<I", sl))
        put(mem, path + 2, struct.pack("<H", 5123))
        put(mem, path + 6, struct.pack("<H", 5045))
        put(mem, sl + 0x48, struct.pack("<I", arr))
        put(mem, sl + 0x4C, struct.pack("<h", 3))
        put(mem, arr, struct.pack("<HHi", 0, 6, 100 << 8) + struct.pack("<HHi", 0, 7, 120 << 8)
            + struct.pack("<HHi", 0, 9, 30 << 8))
        return mem, u
    mem, u = make()
    rec = Fake(mem).unit_record(u)
    check(rec == {"id": 0x55, "t": 0, "cl": 1, "m": 2, "x": 5123, "y": 5045,
                  "life": 100 << 8, "mana": 0}, f"unit record {rec}")
    mem, u = make()
    mem[0x2000 + 2] ^= 1
    check(Fake(mem).unit_record(u)["x"] == 5122, "x perturbation")
    mem, u = make()
    mem[0x4000 + 4] ^= 0x80
    r2 = Fake(mem).unit_record(u)
    check(r2["life"] != 100 << 8 and r2["mana"] == 0, "life perturbation")
    mem, u = make()
    put(mem, 0x2C + u, struct.pack("<I", 0))
    check(Fake(mem).unit_record(u)["x"] == 0, "no path reads (0, 0)")
    check(stat_value(struct.pack("<HHi", 1, 6, 9), 1, 6) == 0, "layer must match")
    check(start_args({"save": "Foo", "class": 3}) == ["-w", "-ns", "-nosave", "-name", "Foo", "-pal"]
          and start_args({"save": "Foo", "class": 5})[-1] == "Foo", "start args")
    sc_cls = parse_scenario(json.dumps(dict(base, **{"class": "pal"})))
    check(sc_cls["class"] == 3, "class parsed")
    bad(json.dumps(dict(base, **{"class": "mage"})), "class")

    # .scenario encoding (scenario.md §3, test vectors: Walk x=10 y=20 -> 01 0a 00 14 00)
    walk = {"tick": 1, "id": 1, "size": 5, "name": "Walk", "fields": [
        {"name": "x", "ty": "u16", "bits": 16, "shift": 0, "offset": 1,
         "arg": {"ref": "@x+8", "kind": "x", "d": 8, "ty": 0, "class": None, "n": 0}},
        {"name": "y", "ty": "u16", "bits": 16, "shift": 0, "offset": 3, "arg": {"num": 20}}]}
    pl = {"id": 1, "t": 0, "cl": 1, "x": 2, "y": 9}
    check(encode_step(walk, [pl], pl) == bytes.fromhex("010a001400"), "typed walk encoding")
    try:
        encode_step(walk, [], None)
    except Unresolved as e:
        check(e.ref == "@x+8", "unresolved reference text")
    else:
        fails.append("walk without a player resolved")
    sk = {"tick": 1, "id": 0x3C, "size": 9, "name": "SelectSkill", "fields": [
        {"name": "skill", "ty": "bits", "bits": 31, "shift": 0, "offset": 1, "arg": {"num": 36}},
        {"name": "left", "ty": "bits", "bits": 1, "shift": 31, "offset": 1, "arg": {"num": 0}},
        {"name": "item", "ty": "u32", "bits": 32, "shift": 0, "offset": 5,
         "arg": {"num": 0xFFFFFFFF}}]}
    check(encode_step(sk, [], None) == bytes.fromhex("3c24000000ffffffff"), "SelectSkill encoding")
    mon = [{"id": 9, "t": 1, "cl": 19, "x": 0, "y": 0}, {"id": 4, "t": 1, "cl": 19, "x": 0, "y": 0},
           {"id": 5, "t": 1, "cl": 7, "x": 0, "y": 0}]

    def ref(n, c):
        return {"ref": "@1", "kind": "unit", "d": 0, "ty": 1, "class": c, "n": n}
    check(resolve_ref(ref(0, 19), mon, pl) == 4 and resolve_ref(ref(1, 19), mon, pl) == 9
          and resolve_ref(ref(0, None), mon, pl) == 4, "unit references: ascending GUID")
    try:
        resolve_ref(ref(2, 19), mon, pl)
    except Unresolved:
        pass
    else:
        fails.append("missing unit resolved")
    tl = trace_lines({"k": "header"}, [(1, RANK["unit"], 0, {"k": "unit", "t": 1}),
                                       (1, RANK["c2s"], 1, {"k": "c2s", "t": 1, "i": 0}),
                                       (0, RANK["end"], 2, {"k": "end", "t": 0})])
    check([json.loads(x)["k"] for x in tl] == ["header", "end", "c2s", "unit"], "trace order")

    if fails:
        for f in fails:
            print("FAIL:", f)
        return 1
    print("selftest ok")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("scenario", nargs="?", help="scenario .json")
    ap.add_argument("--game", default=os.path.join(REPO, "game", "Game.exe"))
    ap.add_argument("--seconds", type=float, default=300.0,
                    help="kill the game after this many seconds (default 300)")
    ap.add_argument("--out", default=None, help="output (default traces/raw/<time>-scenario.jsonl)")
    ap.add_argument("--dry-run", action="store_true", help="print the plan and exit")
    ap.add_argument("--selftest", action="store_true", help="check everything that needs no game")
    ap.add_argument("--manual-start", action="store_true",
                    help="run Game.exe -w -ns and let a person start the game by hand")
    ap.add_argument("--force-after", type=float, default=6.0,
                    help="seconds before the tool ends the start-up menu (default 6)")
    ap.add_argument("--save", default="Probe", help="probes: character name")
    ap.add_argument("--class", dest="cls", default="ama", choices=sorted(CLASS_NAMES),
                    help="probes: character class")
    ap.add_argument("--difficulty", type=int, default=0, choices=(0, 1, 2), help="probes")
    ap.add_argument("--probe-frame", type=int, default=30,
                    help="probes inject / unit_fields: the tick to act at (default 30)")
    ap.add_argument("--save-as", metavar="NAME", default=None,
                    help=".scenario: load this save instead of the script's `char save`")
    ap.add_argument("--trace-out", default=None, help=".scenario: scenario-trace output "
                    "(default traces/raw/<name>.original.trace.jsonl)")
    ap.add_argument("--probe", metavar="NAME", help="print what the coordinator needs "
                    "to settle an open question of original-hooks.md")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if a.probe:
        if a.probe not in PROBES:
            print(f"unknown probe {a.probe!r}; known: {', '.join(sorted(PROBES)) or '(none)'}")
            return 2
        return PROBES[a.probe](a)
    if not a.scenario:
        ap.error("scenario file required")
    try:
        if a.scenario.endswith(".scenario"):
            sc, sha, _ = load_script(a.scenario, a.save_as)
        else:
            sc, sha = load_scenario(a.scenario)
    except (ScenarioError, OSError) as e:
        print("error:", e, file=sys.stderr)
        return 2
    game = os.path.abspath(a.game)
    rel = os.path.relpath(a.scenario, REPO) if os.path.abspath(a.scenario).startswith(REPO) \
        else a.scenario
    rel = rel.replace("\\", "/")
    if a.dry_run:
        print(plan_text(sc, rel, sha, game) if not sc.get("rel") else
              f"scenario {rel} sha256 {sha[:16]}.. save {sc['save']} class {sc['class']} "
              f"end {sc['ticks']} steps {len(sc['steps'])} (relative ticks, F0 + 1 + t)")
        return 0
    try:
        with open(game, "rb") as f:
            game_sha = hashlib.sha256(f.read()).hexdigest()
    except OSError as e:
        print("error:", e, file=sys.stderr)
        return 2
    if game_sha != rr.GAME_EXE_SHA256:
        print(f"error: {game}: sha256 {game_sha} is not the reference 1.14d Game.exe",
              file=sys.stderr)
        return 2
    out = a.out or os.path.join(
        REPO, "traces", "raw", datetime.datetime.now().strftime("%Y%m%d-%H%M%S") + "-scenario.jsonl")
    args = MANUAL_ARGS if a.manual_start else start_args(sc)
    writer = ScenarioWriter(out)
    r = ScenarioRecorder(game, args, writer, sc, a.seconds, auto_start=not a.manual_start,
                         force_after=a.force_after)
    writer.header(make_header(sc, rel, sha, game_sha, args))
    try:
        run_game(r)
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    finally:
        writer.footer({"counts": r.counts, "notes": r.notes, "ticks": r.last_frame,
                       "injected": r.injected, "failed": r.failed})
        writer.close()
    if sc.get("rel"):
        tout = a.trace_out or os.path.join(REPO, "traces", "raw",
                                           sc["export"]["name"] + ".original.trace.jsonl")
        os.makedirs(os.path.dirname(os.path.abspath(tout)), exist_ok=True)
        lines = trace_lines(r.trace_header(), r.tr)     # no `end` record unless the run reached it
        with open(tout, "w", encoding="utf-8", newline="\n") as f:
            f.writelines(lines)
        print(f"trace {tout}: {len(lines) - 1} records, end record {r.ended}")
    for n in r.notes:
        print("note:", n)
    print(f"wrote {out}: {r.last_frame} ticks, {r.injected} injected, {r.counts}")
    return 4 if r.failed else 0


if __name__ == "__main__":
    sys.exit(main())
