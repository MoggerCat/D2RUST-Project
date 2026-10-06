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
  {"version": 1, "seed": <u32>, "save": "<character name>", "difficulty": 0,
   "steps": [{"tick": N, "c2s": "<hex message>"}, ...], "ticks": <total>,
   "streams": ["units", "packets", "rng"]}

Streams: "units" (one snapshot per tick), "packets" (the message hooks of
record_packets.py), "rng" (every seeded draw and seed set).

Game-side facts (injection point and method, seed override point, tick
hook, unit fields, non-interactive start, save loading) come from
specs/tools/original-hooks.md. Where that spec leaves a question open, a
`--probe <name>` mode prints what the coordinator needs to settle it.

  --selftest   everything that needs no game (parsing and its errors,
               hex parsing, writer output, hash perturbation check)
  --dry-run    print the plan and exit; starts nothing

The game process is always terminated when this script ends. Never run
by the cloud: the coordinator runs it with the user at the game.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import datetime
import hashlib
import json
import os
import re
import sys
import tempfile

sys.dont_write_bytecode = True  # no __pycache__ next to the scripts
import record_rng as rr  # noqa: E402  (the shared Win32 debugger definitions)
import record_packets as rp  # noqa: E402  (message hooks, loop-phase markers)
import record_tick as rt  # noqa: E402  (game / unit-list layout and readers)

TOOL = "trace-recorder run_scenario 0.1.0"
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

    seed = int_field("seed", 0, 0xFFFFFFFF)
    ticks = int_field("ticks", 1, MAX_TICKS)
    difficulty = int_field("difficulty", 0, 2) if "difficulty" in d else 0
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
    return {"seed": seed, "save": save, "difficulty": difficulty, "ticks": ticks,
            "streams": list(streams), "steps": steps}


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
            "seed": scenario["seed"], "save": scenario["save"],
            "difficulty": scenario["difficulty"], "ticks": scenario["ticks"],
            "streams": scenario["streams"], "steps": len(scenario["steps"]), "args": args}


# --- game-side facts: specs/tools/original-hooks.md ---------------------------
# Filled in from the spec in phase 2; None = not known yet. Any use of a None
# raises HookMissing, so a probe or a clear error shows instead of a guess.

class HookMissing(RuntimeError):
    pass


HOOK = {
    "start_args": None,       # command line for a non-interactive single-player start
    "seed_override": None,    # where / how the game seed is replaced before any draw
    "inject": None,           # where / how a C->S message enters the server queue
    "unit_x": None, "unit_y": None, "unit_life": None, "unit_mana": None,
}


def need(name):
    if HOOK[name] is None:
        raise HookMissing(f"hook {name!r} is not implemented: specs/tools/original-hooks.md "
                          f"did not settle it (see `--probe {name}`)")
    return HOOK[name]


# --- the recorder --------------------------------------------------------------

class ScenarioRecorder(rp.PacketRecorder):
    """PacketRecorder (message hooks + the shared debug loop, which also hooks
    the RNG helpers and setters) plus tick counting, seed override, injection
    and per-tick unit snapshots."""

    def __init__(self, exe, args, writer, scenario, seconds):
        super().__init__(exe, args, "", seconds, 0)
        self.out, self.writer = writer, writer
        self.sc = scenario
        self.want = set(scenario["streams"])
        self.game = None
        self.tick_no = -1              # index of the current tick since the first one
        self.steps = list(scenario["steps"])
        self.injected = 0
        self.recording = False

    # deterministic output: no wall-clock ms, no thread ids
    def emit(self, rec):
        stream = STREAM_OF.get(rec["type"])
        if (stream is not None and stream not in self.want) or not self.recording:
            return
        rec.pop("tid", None)
        rec["seq"] = self.seq
        self.seq += 1
        self.count(":".join(x for x in (rec["type"], rec.get("via"), rec.get("op")) if x))
        self.writer.record(rec)

    def on_hook(self, tid, addr, ctx):
        if rp.HOOKS[addr][0] == "tick":
            if self.game is None:
                self.game = ctx.Ecx
                self.recording = True
            if ctx.Ecx != self.game:
                return
            if self.tick_no >= 0:
                self.snapshot_units(self.tick_no)   # end state of the previous tick
            self.tick_no += 1
            if self.tick_no >= self.sc["ticks"]:
                self.notes.append(f"{self.sc['ticks']} ticks recorded")
                self.max_events = max(1, self.seq)  # the shared loop stops on its event limit
                return
        super().on_hook(tid, addr, ctx)
        if rp.HOOKS[addr][0] == "tick":
            self.emit({"type": "tick_no", "n": self.tick_no, "frame": self.frame})

    # --- units stream -------------------------------------------------------

    u32 = rt.TickRecorder.u32
    i32 = rt.TickRecorder.i32
    walk = rt.TickRecorder.walk

    def snapshot_units(self, tick):
        if "units" not in self.want:
            return
        units = []
        for t, off in rt.HASH_OFFSETS.items():
            for b in range(128):
                head = self.u32(self.game + rt.HASH_BASE + off + 4 * b)
                for u in self.walk(head, rt.U_HASH_NEXT) if head else ():
                    if not self.u32(u + rt.U_FLAGS2) & rt.SERVER_UNIT:
                        continue
                    units.append(self.unit_record(u))
        units.sort(key=lambda r: (r["t"], r["id"]))
        self.emit({"type": "units", "tick": tick, "units": units})

    def unit_record(self, u):
        rec = {"id": self.u32(u + rt.U_GUID), "t": self.u32(u + rt.U_TYPE),
               "cl": self.u32(u + rt.U_CLASS), "m": self.u32(u + rt.U_MODE)}
        for k in ("unit_x", "unit_y", "unit_life", "unit_mana"):
            rec[k[5:]] = self.read_field(k, u) if HOOK[k] is not None else None
        return rec

    def read_field(self, name, unit):
        raise HookMissing(f"unit field {name}")  # replaced once the spec gives the layout

    def run_scenario(self, scenario_path, scenario_sha, game_sha):
        raise HookMissing("start-up, seed override and injection come from "
                          "specs/tools/original-hooks.md")


PROBES = {}  # name -> function(args); filled in phase 2 from the spec's open questions


# --- plan, selftest, main ----------------------------------------------------

def plan_text(sc, path, sha, game):
    lines = [f"scenario   {path} (sha256 {sha[:16]}...)",
             f"game       {game}",
             f"save       {sc['save']}  difficulty {sc['difficulty']}  seed {sc['seed']:#010x}",
             f"ticks      {sc['ticks']}   streams {', '.join(sc['streams'])}",
             f"steps      {len(sc['steps'])}"]
    for s in sc["steps"]:
        lines.append(f"  tick {s['tick']:>6}  C->S {len(s['c2s'])} bytes  {s['c2s'].hex(' ')}")
    for name in HOOK:
        lines.append(f"hook {name:<14} {'implemented' if HOOK[name] is not None else 'NOT KNOWN YET'}")
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
                        ({"seed": 2 ** 32}, "seed"), ({"seed": "1"}, "seed"),
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

    # hook table: nothing guessed
    try:
        need("inject")
    except HookMissing:
        pass
    else:
        check(HOOK["inject"] is not None, "need() accepted an unknown hook")

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
    ap.add_argument("--probe", metavar="NAME", help="print what the coordinator needs "
                    "to settle an open question of original-hooks.md")
    a = ap.parse_args()
    if a.selftest:
        return selftest()
    if a.probe:
        if a.probe not in PROBES:
            print(f"unknown probe {a.probe!r}; known: {', '.join(sorted(PROBES)) or '(none yet)'}")
            return 2
        return PROBES[a.probe](a)
    if not a.scenario:
        ap.error("scenario file required")
    try:
        sc, sha = load_scenario(a.scenario)
    except (ScenarioError, OSError) as e:
        print("error:", e, file=sys.stderr)
        return 2
    game = os.path.abspath(a.game)
    rel = os.path.relpath(a.scenario, REPO) if os.path.abspath(a.scenario).startswith(REPO) \
        else a.scenario
    rel = rel.replace("\\", "/")
    if a.dry_run:
        print(plan_text(sc, rel, sha, game))
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
    try:
        need("start_args")
    except HookMissing as e:
        print("error:", e, file=sys.stderr)
        return 3
    writer = ScenarioWriter(out)
    r = ScenarioRecorder(game, HOOK["start_args"], writer, sc, a.seconds)
    writer.header(make_header(sc, rel, sha, game_sha, HOOK["start_args"]))
    try:
        r.run_scenario(rel, sha, game_sha)
    except KeyboardInterrupt:
        print("interrupted; game terminated", file=sys.stderr)
    finally:
        writer.footer({"counts": r.counts, "notes": r.notes, "ticks": r.tick_no,
                       "injected": r.injected})
        writer.close()
    for n in r.notes:
        print("note:", n)
    print(f"wrote {out}: {r.tick_no} ticks, {r.counts}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
