"""Record 1.14d rendering-fact scenes under Wine (docs/handoff/q-facts-scenes.md).

    python3 tools/cloud-game/scenes.py GROUP [--runs 2] [--install] [--tmp DIR]

A group is one game run (character, seed, tick-based input script with `mark NAME`
lines) that yields several scenes; a scene is the first frame with a draw log at or
after the recorder tick of its mark (+ offset). The group is run N times; each run's
frames go through facts_render.py into DIR/runK/, and the scenes' draws.tsv /
frame.tsv are compared across runs (equal rows = stable capture; `seq`, `tick` and
the `at` column are reported separately). With --install the first run's facts are
written to facts/render/ (scenes + the merged sprites.tsv).
Our own code; no game data is written into the repository except measurements.
"""
import argparse, glob, json, os, subprocess, sys

REPO = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
GAME = os.environ.get("D2_GAME_DIR", os.path.expanduser("~/game"))
sys.path.insert(0, os.path.join(REPO, "tools", "cloud-game"))
from scene_defs import GROUPS  # noqa: E402


def record(g, out, seconds):
    cmd = ["tools/cloud-game/run.sh", "--python", "--seconds", str(seconds + 60), "--shot-at", "1",
           "--out", out, "--", "tools/trace-recorder/record_frames.py", "--game", GAME + "/Game.exe",
           "--seconds", str(seconds), "--every", "1", "--draws-every", "1"] + ([] if os.environ.get("SCENE_IMAGES") else ["--no-save"]) + [
           "--auto", g["char"], "--seed", str(g["seed"]), "--input", g["script"]]
    before = set(glob.glob(REPO + "/traces/raw/*-frames.jsonl"))
    subprocess.run(cmd, cwd=REPO, check=True, stdout=subprocess.DEVNULL)
    new = sorted(set(glob.glob(REPO + "/traces/raw/*-frames.jsonl")) - before)
    if len(new) != 1:
        raise SystemExit(f"expected one new capture, got {new}")
    return new[0]


def marks(cap):
    m = {}
    for l in open(cap):
        if '"footer"' in l:
            for n in json.loads(l)["notes"]:
                if "mark " in n:
                    w = n.split("mark ")[1].split()
                    m[w[0]] = int(w[1].split("=")[1])
    return m


def pick(cap, tick, mode="first"):
    """first: the first frame with draws at tick >= `tick`; last: the last frame with draws at
    tick == `tick` (a paused game, the Esc menu, redraws without ticking)."""
    best = None
    for l in open(cap):
        if '"k": "frame"' in l or '"k":"frame"' in l:
            r = json.loads(l)
            if not r.get("draws"):
                continue
            if mode == "first" and r["f"] >= tick:
                return r["seq"], r["f"]
            if mode == "last":
                if r["f"] == tick:
                    best = (r["seq"], r["f"])
                elif r["f"] > tick:
                    break
    return best


def facts(cap, scene, seq, out):
    subprocess.run([sys.executable, "-I", "tools/trace-recorder/facts_render.py", cap, "--scene", scene,
                    "--frame", str(seq), "--tile-light", "unknown", "--out", out], cwd=REPO, check=True, stdout=subprocess.DEVNULL)


def body(p):
    return open(p).read().split("\n", 1)[1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("group")
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--install", action="store_true")
    ap.add_argument("--tmp", default=os.path.expanduser("~/scene-runs"))
    ap.add_argument("--seconds", type=int, default=420)
    ap.add_argument("--reuse", nargs="*", default=[], help="existing capture files instead of recording")
    a = ap.parse_args()
    g = GROUPS[a.group]
    caps = list(a.reuse)
    while len(caps) < a.runs:
        caps.append(record(g, f"{a.tmp}/{a.group}-rec{len(caps)}", a.seconds))
    res = {}
    for k, cap in enumerate(caps):
        mk = marks(cap)
        for sc, (mark, off, *mode) in g["scenes"].items():
            if mark not in mk:
                res.setdefault(sc, []).append(None)
                continue
            p = pick(cap, mk[mark] + off, *mode)
            res.setdefault(sc, []).append(p)
            if p:
                facts(cap, sc, p[0], f"{a.tmp}/{a.group}-run{k}")
    print("capture files:", *caps, sep="\n  ")
    ok = True
    for sc in g["scenes"]:
        ps = res[sc]
        if None in ps or any(x is None for x in ps):
            print(f"{sc}: MISSING (mark or frame not found): {ps}")
            ok = False
            continue
        runs = [(f"{a.tmp}/{a.group}-run{k}/scenes/{sc}") for k in range(len(caps))]
        d0 = [body(r + "/draws.tsv").rstrip("\n").split("\n") for r in runs]
        f0 = [dict(l.split("\t") for l in body(r + "/frame.tsv").rstrip("\n").split("\n")[1:]) for r in runs]
        # `frame.tsv` body starts at the column row; the rain is drawn with DrawLine / DrawBox rows
        # whose positions differ between two runs of the same frame (weather is not reproducible)
        def norm(rows):
            # the row number `i` shifts when the weather rows differ in number: not compared
            return [r.split("\t", 1)[1] for r in rows if r.split("\t")[1] not in ("DrawLine", "DrawBox")]
        def row_eq(x, y):
            # equal, or every differing cell is `?` (unmeasured) on one side: not a conflict
            return x == y or all(p == q or "?" in (p, q) for p, q in zip(x.split("\t"), y.split("\t")))

        def lists_eq(x, y):
            return len(x) == len(y) and all(row_eq(p, q) for p, q in zip(x, y))
        all_eq = all(x == d0[0] for x in d0[1:]) and all(x == f0[0] for x in f0[1:])
        solid = all(lists_eq(norm(x), norm(d0[0])) for x in d0[1:])
        keys = [k for k in f0[0] if any(f[k] != f0[0][k] for f in f0[1:])]
        if all_eq:
            verdict = "STABLE"
        elif solid and set(keys) <= {"index_sha256", "draws"}:
            nl = [len(x) - len(norm(x)) for x in d0]
            verdict = f"STABLE except weather lines (DrawLine/DrawBox rows {nl}; frame.tsv differs in {keys})"
        else:
            nd = [i for i, (x, y) in enumerate(zip(norm(d0[0]), norm(d0[1]))) if not row_eq(x, y)]
            verdict = f"DIFFERENT: {len(nd)} non-weather draw rows differ (first {nd[:3]}); frame.tsv keys {keys}"
        print(f"{sc}: frames (seq,f) per run {ps}: {verdict}")
        if a.install:
            facts(caps[0], sc, ps[0][0], os.path.join(REPO, "facts", "render"))


main()
