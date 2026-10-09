"""Selftest of check-gen (python3 tools/check-gen/check_gen.py --selftest).

Runs on a small synthetic excel view (so CI needs no game files) and, when
D2_GAME_DIR holds the live excel view, once more on the real tables.
Every generated file must parse with the strict `.check` parser of
tools/scenario-diff/scenario_diff.py; and the checks can fail (M08): a
changed table row changes exactly its file, a hand-edited file and a stray
file make `--check` fail, a missing column is an error.
"""
import os
import shutil
import sys
import tempfile

import check_gen as cg

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "scenario-diff"))
sys.path.insert(0, os.path.join(HERE, "..", "trace-recorder"))
import scenario_diff  # noqa: E402


def table(path, head, rows):
    with open(path, "w", encoding="latin-1", newline="") as f:
        f.write("\t".join(head) + "\n")
        for r in rows:
            f.write("\t".join(str(x) for x in r) + "\n")


def synthetic(d):
    table(os.path.join(d, "levels.txt"), ["Name", "Id", "Act", "DrlgType"],
          [["Null", 0, 0, 0], ["Act 1 - Town", 1, 0, 2], ["Act 1 - Cave 1", 8, 0, 1],
           ["Act 2 - Town", 40, 1, 2], ["Act 5 - Siege 1", 110, 4, 3], ["Expansion", "", "", ""]])
    table(os.path.join(d, "monstats.txt"), ["Id", "hcIdx", "AI", "enabled", "boss"],
          [["skeleton1", 0, "Skeleton", 1, 0], ["skeleton2", 1, "Skeleton", 1, 0],
           ["fallen1", 19, "Fallen", 1, 0], ["andariel", 156, "Andariel", 1, 1]])
    table(os.path.join(d, "superuniques.txt"), ["Superunique", "Name", "Class", "hcIdx"],
          [["Bishibosh", "Bishibosh", "fallenshaman1", 0], ["", "", "", ""],
           ["Rakanishu", "Rakanishu", "fallen2", 3]])
    table(os.path.join(d, "monumod.txt"), ["uniquemod", "id", "enabled"],
          [["none", 0, 0], ["rndname", 1, 1], ["lifeyes", 5, 1]])
    table(os.path.join(d, "skills.txt"), ["skill", "Id", "charclass", "passive"],
          [["Attack", 0, "", 0], ["Fire Bolt", 36, "sor", ""], ["Warmth", 37, "sor", 1],
           ["Magic Arrow", 6, "ama", ""], ["Wolf", 223, "dru", ""]])
    table(os.path.join(d, "shrines.txt"), ["Shrine Type", "Shrine name", "Code"],
          [["None", "None", 0], ["Recharge", "Refill", 1], ["Recharge", "Health Boost", 2]])
    # an Id of 7 twice (the second row is dropped), an empty Id, a NEL (0x85) in a description
    table(os.path.join(d, "objects.txt"),
          ["Name", "description - not loaded", "Id", "OperateFn", "PopulateFn", "InitFn"],
          [["Casket", "Casket #5", 1, 1, 1, 0], ["Shrine", "Shrine", 2, 2, 2, 1],
           ["a trap", "exploding cow \x85\x85Very Rare", 250, 30, 9, 0],
           ["Dup", "second Id 2", 2, 0, 0, 0], ["Expansion", "", "", "", "", ""]])


def wp_tsv(path):
    with open(path, "w", encoding="utf-8") as f:
        f.write("wp\tlevel\tact\ttown\ttile_calc\tlevel_name\n")
        f.write("0\t1\t0\t1\t13\tRogue Encampment\n1\t3\t0\t0\t0\tCold Plains\n")
        f.write("9\t40\t1\t1\t0\tLut Gholein\n")


class T:
    def __init__(self):
        self.n = 0

    def ok(self, cond, what):
        self.n += 1
        if not cond:
            raise AssertionError(what)


def mkctx(excel, wp, shrines=None):
    c = cg.Ctx()
    c.excel, c.waypoints = excel, wp
    c.ledger = os.path.join(HERE, "ledger-areas.tsv")
    c.waypoint_towns = os.path.join(HERE, "waypoint-towns.tsv")
    c.shrine_seeds = shrines or {1: (1234, 2, "d2rs")}
    return c


def parse_all(checks, t):
    for c in checks:
        text = c.render()
        p = scenario_diff.parse(text)
        t.ok(p is not None, c.name)
        t.ok(f"name {c.name}\n" in text, c.name)


def run():
    t = T()
    tmp = tempfile.mkdtemp(prefix="check-gen-")
    try:
        ex = os.path.join(tmp, "excel")
        os.makedirs(ex)
        synthetic(ex)
        wp = os.path.join(tmp, "wp.tsv")
        wp_tsv(wp)
        ctx = mkctx(ex, wp)
        checks = cg.generate(ctx, cg.FAMILIES)
        by = {}
        for c in checks:
            by.setdefault(c.family, []).append(c.name)
        t.ok(by["lvl"] == ["gen-lvl-1", "gen-lvl-8", "gen-lvl-40", "gen-lvl-110"], by["lvl"])
        t.ok(by["wp"] == ["gen-wp-0", "gen-wp-1", "gen-wp-9"], by["wp"])
        t.ok(by["ai"] == ["gen-ai-andariel", "gen-ai-fallen", "gen-ai-skeleton"], by["ai"])
        t.ok(by["su"] == ["gen-su-0", "gen-su-2"], by["su"])
        t.ok(by["boss"] == ["gen-boss-156"], by["boss"])
        t.ok(by["umod"] == ["gen-umod-1", "gen-umod-5"], by["umod"])
        t.ok(by["skill"] == ["gen-skill-ama-6", "gen-skill-sor-36", "gen-skill-sor-37",
                             "gen-skill-dru-223"], by["skill"])
        t.ok(by["shrine"] == ["gen-shrine-1"], by["shrine"])
        t.ok(by["obj"] == ["gen-obj-1", "gen-obj-2", "gen-obj-250"], by["obj"])
        t.ok("\x85" not in next(c for c in checks if c.name == "gen-obj-250").render(),
             "NEL kept in a header comment")
        parse_all(checks, t)
        # the lowest enabled non-boss class of an AI is the spawn
        sk = next(c for c in checks if c.name == "gen-ai-skeleton")
        t.ok("spawn 0 @x+5 @y-4 normal" in sk.render(), "ai class")
        # deterministic
        t.ok([c.render() for c in checks] == [c.render() for c in cg.generate(ctx, cg.FAMILIES)],
             "deterministic")

        # write, then --check
        out = os.path.join(tmp, "out")
        args = ["--excel", ex, "--waypoints", wp, "--out", out,
                "--waypoint-towns", os.path.join(HERE, "waypoint-towns.tsv"),
                "--shrine-seeds", os.path.join(tmp, "seeds.tsv")]
        with open(os.path.join(tmp, "seeds.tsv"), "w") as f:
            f.write("shrine\tseed\tobject_class\tverified\n1\t1234\t2\td2rs\n")
        t.ok(cg.main(args) == 0, "write")
        t.ok(cg.main(args + ["--check"]) == 0, "current after write")
        # M08: perturbations must be reported
        p = os.path.join(out, "gen-lvl-8.check")
        txt = open(p).read()
        open(p, "w").write(txt.replace("warp 8", "warp 9"))
        t.ok(cg.main(args + ["--check"]) == 1, "edited file detected")
        open(p, "w").write(txt)
        open(os.path.join(out, "gen-stray.check"), "w").write("check 1\n")
        t.ok(cg.main(args + ["--check"]) == 1, "stray file detected")
        os.remove(os.path.join(out, "gen-stray.check"))
        t.ok(cg.main(args + ["--check"]) == 0, "restored")
        # a changed table row changes exactly its file
        table(os.path.join(ex, "levels.txt"), ["Name", "Id", "Act", "DrlgType"],
              [["Null", 0, 0, 0], ["Act 1 - Town", 1, 0, 2], ["Act 1 - Cave 1", 8, 0, 1],
               ["Act 2 - Town", 40, 0, 2], ["Act 5 - Siege 1", 110, 4, 3]])
        t.ok(cg.main(args + ["--check"]) == 1, "table change detected")
        stale = []
        for c in cg.generate(mkctx(ex, wp), cg.FAMILIES):
            pth = os.path.join(out, c.name + ".check")
            if open(pth).read() != c.render():
                stale.append(c.name)
        t.ok(stale == ["gen-lvl-40"], stale)
        # strict: a missing column is an error, not a default
        table(os.path.join(ex, "levels.txt"), ["Name", "Id", "DrlgType"], [["Null", 0, 0]])
        try:
            cg.generate(mkctx(ex, wp), ["lvl"])
            t.ok(False, "missing column accepted")
        except cg.GenError:
            t.ok(True, "")
        # unknown act is an error
        table(os.path.join(ex, "levels.txt"), ["Name", "Id", "Act", "DrlgType"], [["X", 3, 7, 0]])
        try:
            cg.generate(mkctx(ex, wp), ["lvl"])
            t.ok(False, "act 7 accepted")
        except cg.GenError:
            t.ok(True, "")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    gd = os.environ.get("D2_GAME_DIR")
    real = os.path.join(gd, "extracted", "patch_d2", "data", "global", "excel") if gd else ""
    if real and os.path.isfile(os.path.join(real, "levels.txt")):
        ctx = mkctx(real, os.path.join(cg.ROOT, "specs", "world", "waypoints.tsv"))
        ctx.shrine_seeds = {}
        checks = cg.generate(ctx, [f for f in cg.FAMILIES if f != "shrine"])
        parse_all(checks, t)
        print(f"selftest: real tables: {len(checks)} checks parse")
    else:
        print("selftest: D2_GAME_DIR excel view not found: real-table pass skipped")
    print(f"selftest: {t.n} checks passed")
    return 0
