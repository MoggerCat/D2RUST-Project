#!/usr/bin/env python3
"""check-gen: writes scenario-diff .check files from the 1.14d tables.

Spec: specs/tools/scenario-diff.md (the .check syntax, section 2),
specs/tools/poke.md (the poke directives used).  Fidelity plan:
docs/handoff/fidelity-gaps.md section 4 (check-gen).

Families (one check per table row, written under traces/checks/gen/):

  lvl   one level warp per levels.txt row         (a*-warp-* pattern)
  wp    one waypoint travel per waypoints.tsv row (combat-cold-plains-wp)
  ai    one monster spawn per monstats AI name    (combat-random-boss)
  su    one superunique per superuniques row      (missile-superunique.poke)
  boss  one boss spawn per monstats boss=1 row
  umod  one unique spawn per monumod row          (boss-kinds.poke)
  skill one right-click cast per class skill      (dru-* / bar-* / ass-*)
  shrine one shrine operated per reachable shrines.txt row
  obj   one object created and operated per objects.txt row (interact-operate-*)
  itemq the same items at each quality (low .. crafted) over three game seeds
  aud   one audio-diff scenario per reachable system.audio ledger row group (channel
        audio; written to traces/audio/gen/, outside the scenario-diff suite)
  item  a census of ITEM_CHUNK base items per check, each created on the ground by
        the game's own creation path (poke `item`), compared by the items channel

Every generated file starts with a header naming this generator, its
format version, the family and the table row; the files are never edited
by hand (regenerate, `--check` fails when they differ).  Inputs are the
live excel view of the 1.14d install (`$D2_GAME_DIR/extracted/patch_d2/
data/global/excel`, built by `data-tool excel-dir`), strict: a missing
column or an unknown value is an error, nothing is defaulted.

Usage:
  check_gen.py [--excel DIR] [--out DIR] [--family F]... [--ledger FILE]
               [--waypoints TSV] [--check] [--list] [--selftest]
"""
import argparse
import csv
import os
import re
import sys

GEN_VERSION = 1
GEN_NAME = "tools/check-gen/check_gen.py"
FAMILIES = ["lvl", "wp", "ai", "su", "boss", "umod", "skill", "shrine", "item", "itemq", "netc2s", "missile", "state", "mon", "obj", "aud", "fmt"]
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
CLASSES = ["ama", "sor", "nec", "pal", "bar", "dru", "ass"]
CLASS_SAVE = {"ama": "ScnAma", "sor": "ScnSor", "nec": "ScnNec", "pal": "ScnPal",
              "bar": "ScnBar", "dru": "ScnDru", "ass": "ScnAss"}
# The save of the act a level or waypoint belongs to: the form of the
# existing a2-a5 warp checks (act index 0..4).
ACT_SAVE = ["ScnAma --class ama --expansion"] + [
    f"ScnAm{a + 1} --class ama --expansion --act {a} --quests acts={a}" for a in range(1, 5)]
SORC_ACT_SAVE = ["ScnSo1 --class sor --expansion"] + [
    f"ScnSo{a + 1} --class sor --expansion --act {a} --quests acts={a}" for a in range(1, 5)]


class GenError(Exception):
    pass


def slug(text):
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    if not s:
        raise GenError(f"empty slug from {text!r}")
    return s


class Table:
    """A tab separated game table: header row, rows addressed by column name."""

    def __init__(self, path, need):
        with open(path, encoding="latin-1", newline="") as f:
            rows = list(csv.reader(f, delimiter="\t"))
        self.path = path
        self.head = rows[0]
        self.idx = {}
        for k, n in enumerate(self.head):
            self.idx.setdefault(n, k)
        for n in need:
            if n not in self.idx:
                raise GenError(f"{os.path.basename(path)}: column {n!r} missing")
        self.rows = rows[1:]

    def get(self, row, col):
        k = self.idx[col]
        return row[k] if k < len(row) else ""


def excel(d, name, need):
    p = os.path.join(d, name)
    if not os.path.isfile(p):
        raise GenError(f"{p}: not found (build the excel view: data-tool excel-dir)")
    return Table(p, need)


def read_tsv(path, need):
    with open(path, encoding="utf-8", newline="") as f:
        rows = [r for r in csv.reader(f, delimiter="\t") if r and not r[0].startswith("#")]
    head = rows[0]
    for n in need:
        if n not in head:
            raise GenError(f"{path}: column {n!r} missing")
    return [dict(zip(head, r)) for r in rows[1:]]


# ---------------------------------------------------------------- output

class Check:
    def __init__(self, name, family, row, title, save, ticks, seconds, channels,
                 lines, variant=None, comment=(), seed=1234, extra=None):
        self.name, self.family, self.row, self.title = name, family, row, title
        self.save, self.ticks, self.seconds = save, ticks, seconds
        self.channels, self.lines, self.variant = channels, lines, variant
        self.comment, self.seed, self.extra = comment, seed, extra or {}
        self.area = "-"

    def render(self):
        if not re.fullmatch(r"[a-z0-9-]+", self.name):
            raise GenError(f"bad check name {self.name!r}")
        out = ["check 1",
               f"# GENERATED by {GEN_NAME} (format {GEN_VERSION}), family {self.family}, "
               f"row {self.row}: {self.title}.",
               f"# Ledger area: {self.area}. Do not edit: regenerate with "
               f"`python3 {GEN_NAME}`."]
        out += [f"# {c}" for c in self.comment]
        out += ["# Spec: specs/tools/scenario-diff.md, specs/tools/poke.md.",
                f"name {self.name}", f"save {self.save}", f"seed {self.seed}",
                f"ticks {self.ticks}", f"seconds {self.seconds}",
                f"channels {self.channels}"]
        if self.variant:
            out.append(f"variant {self.variant}")
        out += self.lines
        return "\n".join(out) + "\n"


SEEDS = ["at 30 poke seed-game 0x00001234 666",
         "at 30 poke seed-unit @player 0x00000055 666"]
BM = ["at 4 poke warp 2"]  # Blood Moor (variant blood-moor-empty: no population)



# -------------------------------------------------------------- families

def fam_lvl(ctx):
    t = excel(ctx.excel, "levels.txt", ["Name", "Id", "Act", "DrlgType"])
    out = []
    for r in t.rows:
        lid = t.get(r, "Id")
        if not lid.isdigit() or int(lid) == 0:
            continue  # the "Null" row and the "Expansion" separator
        lid, act = int(lid), int(t.get(r, "Act"))
        if not 0 <= act <= 4:
            raise GenError(f"levels row {lid}: act {act}")
        name = t.get(r, "Name")
        out.append(Check(
            f"gen-lvl-{lid}", "lvl", f"levels.txt Id {lid}",
            f"level generation of {name} (level {lid}, act {act + 1})",
            ACT_SAVE[act], 160, 900, "state rng", ["at 20 poke warp %d" % lid],
            comment=[f"Level generation of {name}: the character starts in its act's town, "
                     f"idle until frame 20, then the level warp (poke `warp {lid}`, tile 0) on "
                     "both sides; rooms, presets, populated units and the draws."]))
        out[-1].extra = {"level": lid}
    return out


def walk_steps(p):
    """`pos` pokes from the join point to 3 sub-tiles beside the waypoint object:
    10 sub-tiles per step, 3 frames apart (the form of milestone-hephasto), so
    each target lies in a room the previous step activated."""
    sx, sy = int(p["sx"]), int(p["sy"])
    tx, ty = int(p["wx"]) - 3, int(p["wy"]) + 5
    n = max(1, -(-max(abs(tx - sx), abs(ty - sy)) // 10))
    return [f"at {8 + 3 * i} poke pos @player {sx + (tx - sx) * (i + 1) // n} "
            f"{sy + (ty - sy) * (i + 1) // n}" for i in range(n)]


def fam_wp(ctx):
    rows = read_tsv(ctx.waypoints, ["wp", "level", "act", "town", "level_name"])
    towns = {int(r["act"]): int(r["wp"]) for r in rows if r["town"] == "1"}
    pos = {int(r["act"]): r for r in read_tsv(ctx.waypoint_towns,
                                              ["act", "wp_class", "wx", "wy", "sx", "sy"])}
    out = []
    for r in rows:
        wp, lvl, act = int(r["wp"]), int(r["level"]), int(r["act"])
        town = towns[act]
        known = sorted({town, wp})
        out.append(Check(
            f"gen-wp-{wp}", "wp", f"waypoints.tsv wp {wp}",
            f"waypoint travel to {r['level_name']} (wp {wp}, level {lvl}, act {act + 1})",
            SORC_ACT_SAVE[act] + " --waypoints " + ",".join(map(str, known)), 460, 900, "state",
            walk_steps(pos[act]) + [f"at 400 poke msg 0x49 @wp {lvl}"],
            comment=[f"Waypoint travel (world/waypoints.md section 6-7): a sorceress in the town "
                     f"of act {act + 1} that knows waypoint {wp} is moved by `pos` pokes (10 sub-tiles every "
                     f"3 frames from frame 8, as milestone-hephasto) from the join point "
                     f"({pos[act]['sx']}, {pos[act]['sy']}) to the waypoint object "
                     f"(class {pos[act]['wp_class']} at {pos[act]['wx']}, {pos[act]['wy']}; "
                     "reach 22 sub-tiles, section 6.2 step 3), then the C->S 0x49 to level "
                     f"{lvl} at frame 400 (after the 10 s hostile delay, section 6.1) with the "
                     "town's waypoint object as `@wp`."
                     + (" The target is the start town itself: the travel to the town waypoint."
                        if wp == town else "")]))
    return out


def monster_rows(ctx):
    t = excel(ctx.excel, "monstats.txt", ["Id", "hcIdx", "AI", "enabled", "boss"])
    return t, [(int(t.get(r, "hcIdx")), t.get(r, "Id"), t.get(r, "AI"),
                t.get(r, "enabled"), t.get(r, "boss")) for r in t.rows
               if t.get(r, "hcIdx").isdigit()]


def spawn_check(name, family, row, title, spawn, comment, ticks=150):
    return Check(name, family, row, title, "ScnAma --class ama --expansion", ticks, 360,
                 "state", BM + SEEDS + ["at 30 poke " + spawn],
                 variant="blood-moor-empty",
                 comment=comment + ["Blood Moor warp (level 2) before frame 4 under variant "
                                    "blood-moor-empty (no population), game and unit seeds "
                                    "fixed at frame 30, then the spawn; the state channel "
                                    "compares the units and their AI frame by frame."])


def fam_ai(ctx):
    _, mons = monster_rows(ctx)
    first = {}
    for hc, ident, ai, enabled, boss in mons:
        if not ai:
            continue
        cur = first.get(ai)
        # prefer an enabled class that is not a boss, then the lowest row
        key = (enabled != "1", boss == "1", hc)
        if cur is None or key < cur[0]:
            first[ai] = (key, hc, ident)
    out = []
    for ai in sorted(first, key=str.lower):
        _, hc, ident = first[ai]
        c = spawn_check(f"gen-ai-{slug(ai)}", "ai", f"monstats.txt AI {ai}",
                        f"monster AI {ai} (class {hc} {ident})",
                        f"spawn {hc} @x+5 @y-4 normal",
                        [f"Monster AI {ai}: its first class {hc} ({ident}) spawned normal next "
                         "to the player."])
        c.extra = {"ai": ai, "class": hc}
        out.append(c)
    return out


def fam_mon(ctx):
    """One spawn check per monstats row that has its own ledger row `monster.<id>`
    (the class rows of the coverage group): spawn, idle and think frames, state and rng."""
    _, mons = monster_rows(ctx)
    wanted = set()
    for area, source in load_ledger(ctx.ledger):
        m = re.fullmatch(r"monstats\.txt \S+ \(hcIdx (\d+)\)", source)
        if m and area.startswith("monster."):
            wanted.add(int(m.group(1)))
    out = []
    for hc, ident, ai, enabled, boss in mons:
        if hc not in wanted:
            continue
        c = spawn_check(f"gen-mon-{hc}", "mon", f"monstats.txt row {hc}",
                        f"monster class {hc} {ident} (AI {ai})",
                        f"spawn {hc} @x+5 @y-4 normal",
                        [f"Monster class {hc} ({ident}, AI {ai}) spawned normal next to the "
                         "player: its spawn state and its idle and think frames."])
        c.channels = "state rng"
        # the player's quest list (q) and the game seed differ on every check of every
        # family at frame 2 / 30 (harness level, not the monster): not compared here
        c.lines = ["ignore q seed"] + c.lines
        c.extra = {"class": hc, "id": ident}
        out.append(c)
    return out


def fam_su(ctx):
    t = excel(ctx.excel, "superuniques.txt", ["Superunique", "Name", "Class", "hcIdx"])
    out = []
    for n, r in enumerate(t.rows):
        if not t.get(r, "Superunique") or not t.get(r, "hcIdx"):
            continue  # a blank row
        hc, name = t.get(r, "hcIdx"), t.get(r, "Name")
        c = spawn_check(f"gen-su-{n}", "su", f"superuniques.txt row {n}",
                        f"superunique {name} (hcIdx {hc}, class {t.get(r, 'Class')})",
                        f"superunique {n} @x+4 @y+4",
                        [f"Superunique row {n} ({name}) and its minions through the superunique "
                         "creation (poke `superunique`)."])
        c.extra = {"hcIdx": hc}
        out.append(c)
    return out


def fam_boss(ctx):
    _, mons = monster_rows(ctx)
    out = []
    for hc, ident, ai, enabled, boss in mons:
        if boss != "1":
            continue
        c = spawn_check(f"gen-boss-{hc}", "boss", f"monstats.txt row {hc}",
                        f"boss {ident} (class {hc}, AI {ai})", f"spawn {hc} @x+4 @y+4 normal",
                        [f"Act or quest boss {ident} (monstats boss=1, AI {ai}) spawned normal "
                         "next to the player."])
        c.extra = {"class": hc, "id": ident}
        out.append(c)
    return out


def fam_umod(ctx):
    t = excel(ctx.excel, "monumod.txt", ["uniquemod", "id", "enabled"])
    out = []
    for r in t.rows:
        uid = t.get(r, "id")
        if not uid.isdigit() or int(uid) == 0:
            continue
        name = t.get(r, "uniquemod")
        c = spawn_check(f"gen-umod-{uid}", "umod", f"monumod.txt id {uid}",
                        f"unique modifier {uid} {name}",
                        f"spawn 19 @x+4 @y+4 unique umod {uid}",
                        [f"Unique modifier {uid} ({name}) on a unique fallen (monstats 19), the "
                         "`unique` kind of scenario.md section 3.1."])
        c.extra = {"umod": int(uid)}
        out.append(c)
    return out


def fam_skill(ctx):
    t = excel(ctx.excel, "skills.txt", ["skill", "Id", "charclass", "passive"])
    out = []
    for cl in CLASSES:
        for r in t.rows:
            if t.get(r, "charclass") != cl:
                continue
            sid, name = int(t.get(r, "Id")), t.get(r, "skill")
            passive = t.get(r, "passive") == "1"
            c = Check(
                f"gen-skill-{cl}-{sid}", "skill", f"skills.txt Id {sid}",
                f"{cl} skill {name} ({sid})", f"{CLASS_SAVE[cl]} --class {cl} --expansion "
                f"--level 30 --all-skills 20 --right-skill {sid}", 70, 300, "state",
                BM + ["input frame 20; rclick 330 300"], variant="blood-moor-empty",
                comment=[f"Cast of {name} (skill {sid}, class {cl}): expansion character, level 30, "
                         f"every skill at 20, right skill {sid}; warp to the Blood Moor "
                         "(variant blood-moor-empty: no target monster), one right click at client "
                         "pixel (330, 300) before frame 20's drain. Compares the cast (mode, "
                         "states, stats), its missiles' or summons' effects and the draws."
                         + (" Passive skill: the click casts nothing; the check compares the "
                            "passive's stats and the idle click." if passive else "")])
            c.extra = {"skill": sid, "class": cl, "passive": passive}
            out.append(c)
    return out


def fam_shrine(ctx):
    """Shrines.txt rows the shrine pick can reach through `poke object`
    (tools/check-gen/shrine-seeds.tsv, written by find_shrine_seeds.py):
    rows 0, 4, 5 and 16 are never picked (world/objects.md section 5.1
    rule 4), so they have no check."""
    t = excel(ctx.excel, "shrines.txt", ["Shrine Type", "Shrine name", "Code"])
    out = []
    for n, r in enumerate(t.rows):
        if n not in ctx.shrine_seeds:
            continue
        seed, cls, how = ctx.shrine_seeds[n]
        name = t.get(r, "Shrine name")
        c = Check(
            f"gen-shrine-{n}", "shrine", f"shrines.txt row {n}",
            f"shrine {name} ({t.get(r, 'Shrine Type')})",
            "ScnAm2 --class ama --expansion --level 30 --act 1 --quests acts=1", 120, 300, "state",
            [f"at 30 poke object {cls} @x+3 @y",
             f"at 40 send InteractWithEntity type=2 id=@2:{cls}"],
            seed=seed,
            comment=[f"Shrine {n} {name}: objects row {cls} created next to the player in "
                     f"Lut Gholein (level 40, a town; its level id is above every LevelMin of "
                     f"shrines.txt) at frame 30, operated by C->S 0x13 at frame 40. With -seed "
                     f"{seed} the shrine pick of the object control (world/objects.md section "
                     f"5.1) gives row {n} ({how}; PROVISIONAL REC-1330: the seed is d2rs's pick, settled by the first 1.14d run); the check compares the shrine's effect, the "
                     "player's stats and states and the draws."])
        c.extra = {"shrine": n}
        out.append(c)
    return out


ITEM_CHUNK = 20
ITEM_TABLES = ["weapons.txt", "armor.txt", "misc.txt"]


def item_rows(ctx):
    """(combined index, code, name) over weapons, armor, misc in file order: the
    combined items array of items/treasure.md section 9.1 (ledger ids item.<id>-<code>)."""
    out = []
    for tn in ITEM_TABLES:
        t = excel(ctx.excel, tn, ["code", "name"])
        for r in t.rows:
            code = t.get(r, "code")
            if code.strip() == "ear":
                out.append((len(out), "ear", t.get(r, "name")))  # keeps the index; not poked
            elif code.strip():
                out.append((len(out), code.strip(), t.get(r, "name")))
    return out


def fam_item(ctx):
    rows = item_rows(ctx)
    out = []
    for k in range(0, len(rows), ITEM_CHUNK):
        chunk = rows[k:k + ITEM_CHUNK]
        n = k // ITEM_CHUNK
        lines = ["at 4 poke seed-game 0x00001234 666"]
        for j, (idx, code, _) in enumerate(chunk):
            if code == "ear":
                continue  # the ear is made from a killed player (PK); `poke item` refuses it
            dy = 2 * (j // 5)
            lines.append(f"at 4 poke item {code} @x+{2 + 2 * (j % 5)} " + (f"@y+{dy}" if dy else "@y"))
        c = Check(f"gen-item-{n:02d}", "item",
                  f"items {chunk[0][0]}-{chunk[-1][0]} (combined index)",
                  f"base items {chunk[0][1]} .. {chunk[-1][1]} ({len(chunk)} items)",
                  "ScnAma --class ama --expansion", 20, 240, "items", lines,
                  comment=["Census of base items (items/generation.md section 3): each item is "
                           "created on the ground at normal quality by the game's own creation "
                           "path with the game seed fixed first; the items channel compares the "
                           "S->C 0x9C bit streams item by item. Items: " +
                           ", ".join(f"{i}-{cd}" for i, cd, _ in chunk) + "."])
        c.extra = {"items": [(i, cd) for i, cd, _ in chunk]}
        out.append(c)
    return out


def fam_netc2s(ctx):
    """C->S message types no recorded check carries (docs/handoff/packet-census.tsv,
    state NOT-CARRIED): one check per id sends that message once, with the
    packets channel comparing bytes on both sides. Fields: unit references
    become @player / @x / @y, every other numeric field 0; messages without
    a layout are sent as `hex` with `size` zero bytes after the id. System
    ids (0x67 and up) and variable-size 0x66 are session-level: no check."""
    rows = read_tsv(ctx.client_messages, ["id", "name", "transport_size", "layout", "kind"])
    carried = set()
    for r in read_tsv(ctx.census, ["stream", "id", "state"]):
        if r["stream"] == "c2s" and r["state"] != "NOT-CARRIED":
            carried.add(int(r["id"], 16))
    out = []
    for r in rows:
        n = int(r["id"], 16)
        if n in carried or n >= 0x66 or r["name"] == "-" or r["kind"] in ("none", "system"):
            continue
        if not r["transport_size"].isdigit() or int(r["transport_size"]) == 0:
            continue
        size = int(r["transport_size"])
        name = r["name"]
        fields = []
        for f in r["layout"].split():
            fn, ft = f.split("@")[0].split(":")
            if ft.startswith("cstr") or ft.startswith("bit"):
                fields = None
                break
            if fn in ("id", "unit", "player", "npc", "merc", "orifice") and ft == "u32":
                fields.append(f"{fn}=@player")
            elif fn == "x" or fn.startswith("x"):
                fields.append(f"{fn}=@x+2")
            elif fn == "y" or fn.startswith("y"):
                fields.append(f"{fn}=@y")
            else:
                fields.append(f"{fn}=0")
        if fields is None or not r["layout"].strip():
            body = " ".join(["%02x" % n] + ["00"] * (size - 1))
            line = f"at 20 send hex {body}"
        else:
            line = f"at 20 send {name} " + " ".join(fields)
        c = Check(f"gen-netc2s-{n:02x}", "netc2s", f"client-messages.tsv {r['id']} {name}",
                  f"C->S {r['id']} {name}", "ScnAma --class ama --expansion", 40, 240,
                  "packets", [line],
                  comment=[f"One {name} ({r['id']}) at frame 20 in the Rogue Encampment; the "
                           "packets channel compares the C->S bytes, the dispatch result and "
                           "every S->C message the handler queues. Default field values "
                           "(PROVISIONAL REC-1750): a handler that needs a live target answers "
                           "its refusal path on both sides."])
        c.extra = {"msg": n}
        out.append(c)
    return out


QUALITIES = {1: "low", 2: "normal", 3: "superior", 4: "magic", 5: "set", 6: "rare",
             7: "unique", 8: "crafted"}
QSEEDS = [(0x1234, 666), (0x2222, 77), (0x5A5A, 4242)]
QEXTRA = ["rin", "amu", "cm1", "jew"]


def fam_itemq(ctx):
    rows = item_rows(ctx)
    wa = [r for r in rows if r[0] < 100000 and r[1] not in QEXTRA and not r[1].startswith(("hp", "mp", "rv"))]
    body = [r for r in wa if r[0] < next(i for i, c, _ in rows if c == "key")]  # weapons + armor
    pick = [body[k * len(body) // 16] for k in range(16)]
    codes = [c for _, c, _ in pick] + QEXTRA
    for need in QEXTRA:
        if need not in [c for _, c, _ in rows]:
            raise GenError(f"itemq: code {need} missing from the item tables")
    out = []
    for q, qn in QUALITIES.items():
        for si, (lo, hi) in enumerate(QSEEDS):
            lines = [f"at 4 poke seed-game 0x{lo:08X} {hi}"]
            for j, code in enumerate(codes):
                dy = 2 * (j // 5)
                lines.append(f"at 4 poke item {code} @x+{2 + 2 * (j % 5)} "
                             + (f"@y+{dy}" if dy else "@y") + f" quality {qn} ilvl 85")
            c = Check(f"gen-itemq-{qn}-{si}", "itemq", f"quality {q} {qn}, seed set {si}",
                      f"{len(codes)} items at quality {qn} (game seed 0x{lo:X} {hi})",
                      "ScnAma --class ama --expansion", 20, 240, "items", lines,
                      comment=["Quality census (items/quality.md): the same spread of base items "
                               f"created at quality {q} ({qn}) and item level 85 by the poke "
                               "`item` path; the items channel compares the 0x9C bit streams "
                               "(affix and unique picks, properties, sockets) item by item. "
                               "Items: " + ", ".join(codes) + "."])
            c.extra = {"quality": qn}
            out.append(c)
    return out



def missile_sources(ledger):
    """{lowercase Missiles.txt name: (ledger area, skill id or None)} from the
    source column of the missile.* ledger areas: skills rows list their
    Missiles.txt names and the skills.txt row that makes them; coverage rows
    name one row. The first area wins for a row two areas name."""
    out = {}
    for area, src in load_ledger(ledger):
        if not area.startswith("missile."):
            continue
        m = re.fullmatch(r"Missiles\.txt rows (.*) \(made by skills\.txt .* (\d+)\)", src)
        if m:
            for n in m.group(1).split(","):
                out.setdefault(n.strip().lower(), (area, int(m.group(2))))
            continue
        m = re.fullmatch(r"missiles\.txt (.*) \(\d+\)", src)
        if m:
            out.setdefault(m.group(1).strip().lower(), (area, None))
    return out


def fam_missile(ctx):
    """One check per Missiles.txt row a missile.* ledger area names: the
    row's missile created by `poke missile` (the creator 0x0059FA30 on
    1.14d, create_missile on d2rs) at the player, aimed at a cow 6 sub-tiles
    away along +x, with the skill that makes it (level 20 for a character
    class skill, 10 for any other: PROVISIONAL REC-1730, the level only
    scales damage and counts); rows no skill makes carry no skill."""
    t = excel(ctx.excel, "missiles.txt", ["Missile", "Id"])
    sk = excel(ctx.excel, "skills.txt", ["skill", "Id", "charclass"])
    cls = {int(sk.get(r, "Id")): sk.get(r, "charclass") for r in sk.rows}
    src = missile_sources(ctx.ledger)
    out = []
    for r in t.rows:
        name, mid = t.get(r, "Missile"), int(t.get(r, "Id"))
        if name.lower() not in src:
            continue
        area, skill = src[name.lower()]
        tail = ""
        if skill is not None:
            lvl = 20 if cls.get(skill) else 10
            tail = f" skill {skill} {lvl}"
        c = Check(
            f"gen-missile-{mid}", "missile", f"missiles.txt Id {mid}",
            f"missile {name} ({mid})", "ScnSor --class sor --expansion --level 30 --all-skills 20",
            70, 300, "state rng packets",
            BM + ["at 8 poke spawn 179 @x+6 @y normal",
                  f"at 12 poke missile {mid} @x @y @x+6 @y{tail}"],
            variant="blood-moor-empty",
            comment=[f"Missiles.txt row {mid} ({name}) created by the poke at frame 12 at the "
                     "player, aimed at a cow (class 179) 6 sub-tiles along +x spawned at "
                     "frame 8, in the Blood Moor (variant blood-moor-empty: no other monster)"
                     + (f"; skill {skill} at level {lvl}, as the row's maker (PROVISIONAL "
                        "REC-1730)." if skill is not None else "; no skill (no skill makes it).")
                     + " Compares the missile unit (class, mode, path, frames), what it hits, "
                     "the seeds and the packets."])
        c.extra = {"missile": mid, "name": name, "area": area}
        out.append(c)
    return out


def fam_state(ctx):
    """One check per states.txt row 1.. : `poke state` on the player and on a
    cow at frame 12, off at frame 24 (the toggle and its update-queue insert,
    stat-lists.md section 9.2); the packets channel carries the set/end state
    messages, the state channel the stats the toggle moves."""
    t = excel(ctx.excel, "states.txt", ["state", "id"])
    out = []
    for r in t.rows:
        name, sid = t.get(r, "state"), int(t.get(r, "id"))
        if sid == 0 or not name:
            continue
        c = Check(
            f"gen-state-{sid}", "state", f"states.txt id {sid}",
            f"state {name} ({sid})", "ScnSor --class sor --expansion --level 30 --all-skills 20",
            40, 300, "state packets",
            BM + ["at 8 poke spawn 179 @x+4 @y normal",
                  f"at 12 poke state @player {sid} on", f"at 12 poke state @1:179 {sid} on",
                  f"at 24 poke state @player {sid} off", f"at 24 poke state @1:179 {sid} off"],
            variant="blood-moor-empty",
            comment=[f"States.txt row {sid} ({name}) set on the player and on a cow (class 179, "
                     "Blood Moor, variant blood-moor-empty) at frame 12 and cleared at frame 24. "
                     "Compares the set and end state messages and the stats the toggle moves."])
        c.extra = {"state": sid, "name": name}
        out.append(c)
    return out


def fam_obj(ctx):
    """One object of every objects.txt row (the Id column; the 143 rows
    without a ledger area are generated too, area `-`): created next to the
    player (same position: inside every operate box, world/objects.md section 7.1) in the Rogue Encampment with `poke object <Id>` at frame 10 and
    operated at once with `poke operate @2:<Id>` at frame 20 (the server's
    dispatcher, no range; specs/tools/poke.md, interact-operate-waypoint).
    Channels: the state of the object and of everything its operate function
    creates or changes, the items it drops, and the draws of the RNG."""
    t = excel(ctx.excel, "objects.txt",
              ["Name", "description - not loaded", "Id", "OperateFn", "PopulateFn", "InitFn"])
    out, seen = [], set()
    for r in t.rows:
        oid = t.get(r, "Id")
        if not oid.isdigit() or int(oid) in seen:
            continue
        oid = int(oid)
        seen.add(oid)
        name, desc = (re.sub(r"[^\x20-\x7e]+", " ", t.get(r, x)).strip()
                      for x in ("Name", "description - not loaded"))
        c = Check(
            f"gen-obj-{oid}", "obj", f"objects.txt Id {oid}",
            f"object {name} ({oid}) created and operated",
            "ScnAma --class ama --expansion", 80, 300, "state items rng",
            [f"at 10 poke object {oid} @x @y",
             f"at 20 poke operate @2:{oid}"],
            comment=[f"Object {oid} {name} ({desc}; OperateFn {t.get(r, 'OperateFn')}, "
                     f"PopulateFn {t.get(r, 'PopulateFn')}, InitFn {t.get(r, 'InitFn')}): "
                     "created at the player's position in the Rogue Encampment at frame 10 (inside every object's operate box, world/objects.md section 7.1) "
                     "(the allocator with the per-kind init, world/objects.md section 3), "
                     "operated at once at frame 20 (0x13 {2, GUID} through the dispatcher). "
                     "The check compares the object's state after the operate, what it "
                     "creates (items, missiles, monsters, shrine effects) and the RNG draws."])
        c.extra = {"object": oid}
        out.append(c)
    return out

# Audio scenarios (family aud).  Each entry is one audio-diff check
# (specs/tools/audio-diff.md): 1.14d's DirectSound writes and d2rs' voices
# compared per voice (decoded samples, start tick, device volume and pan)
# and mixed.  `areas` are the system.audio ledger rows the scenario reaches;
# AUD_NOCHECK lists the rows no scenario can reach, with the reason.
AUD_TOWN = "ScnAma --class ama --expansion"
AUD_SOR = "ScnSor --class sor --expansion"
AUD_SCEN = [
    dict(id="town-idle", ticks=250, save=AUD_TOWN, lines=[],
         areas=["sound-table.3-file-path", "sound-table.4-groups-and-variants", "sound-table.5-requests",
                "sound-table.7-starting-on-a-channel", "sound-table.9-settings", "sound-table.10-sample-cache",
                "sound-table.11-live-data-1-14d", "sound-table.12-edge-cases-kept", "triggers.1-conventions-and-shared-state"],
         what="the Rogue Encampment idle for 250 ticks: town music, ambience bed, NPC voices; every voice's sample file, "
              "group, variant, channel start, settings-driven volume and the sound tick / client update counters"),
    dict(id="warp-levels", ticks=260, save=AUD_TOWN,
         lines=["at 4 poke warp 2", "at 120 poke warp 3", "at 200 poke warp 1"],
         areas=["environment.8-sample-pins-on-level-change-0x004e42e0-last-pa"],
         what="three level changes (Blood Moor, Cold Plains, back to the Rogue Encampment): the ambience, "
              "music and sample pins re-made on each level change"),
    dict(id="npc-talk-akara", ticks=120, save=AUD_TOWN,
         lines=["at 4 poke pos @player 4888 4228", "at 7 poke pos @player 4903 4224",
                "at 10 poke pos @player 4918 4216", "at 13 poke pos @player 4928 4210",
                "at 16 poke talk @1:148 trade"],
         areas=["triggers.10-npc-speech"],
         what="the player poked next to Akara and talking to her (greeting line, chat open)"),
    dict(id="item-drop", ticks=120, save=AUD_TOWN,
         lines=["at 10 poke item hp1 @x+2 @y", "at 14 poke item cap @x-2 @y", "at 18 poke item axe @x @y+2"],
         areas=["triggers.9-items"],
         what="a potion, a cap and an axe created on the ground: the flippy and drop sounds (mode 5)"),
    dict(id="object-chest", ticks=140, save=AUD_TOWN,
         lines=["at 10 poke object 5 @x @y", "at 30 poke operate @2:5",
                "at 60 poke object 26 @x @y"],
         areas=["triggers.7-object-mode-sounds-0x004cb460-objects", "triggers-2.20-when-object-units-make-their-mode-sounds"],
         what="a chest created and opened, then Cain's gibbet (class 26) created next to the player: object mode loops, "
              "transitions and the gibbet's help line"),
    dict(id="monster-idle", ticks=420, save=AUD_TOWN, variant="blood-moor-empty",
         lines=["at 4 poke warp 2"] + SEEDS + ["at 30 poke spawn 19 @x+6 @y normal", "at 30 poke spawn 19 @x-6 @y normal"],
         areas=["triggers.6-monster-idle-voices", "triggers-2.18-sound-identity-of-a-unit-and-its-monsounds-re",
                "triggers-2.19-a-unit-s-request-list-u-0x78"],
         what="two fallen near the player in the empty Blood Moor, seeds pinned, 420 ticks: the neutral idle voices, "
              "their timers and each unit's request list"),
    dict(id="monster-attack", ticks=260, save=AUD_TOWN, variant="blood-moor-empty",
         lines=["at 4 poke warp 2"] + SEEDS + ["at 30 poke spawn 19 @x+2 @y normal"],
         areas=["triggers-2.15-animation-event-3-sound-audio-triggers-md-ope"],
         what="a fallen attacking the player: the animation-event sound (event 3) of its attack frames"),
    dict(id="ui-panels", ticks=120, save=AUD_TOWN, input="frame 10; key I; frame 30; key I; frame 40; key C; frame 60; key C",
         lines=[],
         areas=["triggers.11-ui-sounds", "triggers-2.17-options-menu-ui-sounds-audio-triggers-md-open"],
         what="the inventory and character panels opened and closed: the UI cursor sounds"),
    dict(id="player-chat-event", ticks=80, save=AUD_TOWN,
         lines=["at 10 poke msg 0x3F 25"],
         areas=["triggers.3-player-event-sounds-0x004cb9c0-u-event-e", "triggers.2-server-sound-events-s-c-0x2c"],
         what="C->S 0x3F PlayAudio event 25 (chat line 1): the player event sound and the server sound event path"),
    dict(id="hit-sequence", ticks=260, save=AUD_TOWN, variant="blood-moor-empty",
         lines=["at 4 poke warp 2"] + SEEDS + ["at 30 poke spawn 19 @x+2 @y normal"],
         input="frame 40; click 430 300; frame 80; click 430 300",
         areas=["triggers-2.13-remaining-fixed-request-conditions-audio-trig"],
         what="the player attacking a fallen by clicking it: attack, impact and death sounds of the fixed requests"),
]
AUD_NOCHECK = {
    "environment.1-sound-environment": "EAX room settings run in mixer mode 2 only and are not reproduced; mode 0 (what d2rs and the capture use) has no effect on decoded samples or trigger ticks",
    "environment.3-quest-stingers-0x004dcd40-m-dm-h-k-s-ds-play": "stingers start from quest events (server 0x2C events 33-83) that no poke can raise; needs a quest-completion scenario",
    "environment.9-front-end-music-options-music-answers-open-que": "front-end music plays in the menu screens, outside the in-game audio capture",
    "sound-table.1-loading-the-table": "table loading has no timing or sample output of its own; sounds.txt content is compared row by row through the voices of every audio check",
    "sound-table.2-sound-environment-table-load-only": "load only, no behaviour a scenario can reach (the table feeds the EAX path, not reproduced)",
    "sound-table.13-d2rs-mapping": "d2rs-internal mapping of the original structures, no 1.14d output to compare",
    "sound-table-2.14-other-users-of-the-local-player-s-client-unit": "needs a run that consumes the client seed from another user (weather thunder, event cues) at a pinned tick; none is reachable by pokes yet",
    "sound-table-2.15-options-menu-sliders-audio-sound-table-md-ope": "the options-menu sliders are front-end/menu input not covered by the in-game capture",
    "sound-table-2.16-sample-cache-exact-refines-audio-sound-table-": "cache residency (hits, evictions) is not visible in decoded samples or trigger ticks",
    "sound-table-2.17-start-failures-on-a-channel-refines-audio-sou": "needs more than 16 simultaneous voices at pinned ticks; no poke raises that load deterministically",
    "triggers.12-other-fixed-requests": "the fixed requests are wall-clock driven (shake, panels) or need events no poke raises (thunder, steal life, Inifuss); the reachable ones are covered by hit-sequence",
    "triggers-2.14-server-senders-of-s-c-0x2c-audio-triggers-md-": "senders are server events (quests, cube, NPC, inventory, AI); only the client side of 0x2C is reachable by pokes",
    "triggers-2.16-progsound-conditions-audio-triggers-md-open-q": "ProgSound needs the quest/progress states of the original; no poke sets them",
    "triggers-2.21-what-the-driver-needs-per-rule-inputs-and-own": "a table of driver inputs and owners, no behaviour of its own",
}


def fam_aud(ctx):
    out = []
    for sc in AUD_SCEN:
        c = Check(
            f"gen-aud-{sc['id']}", "aud", f"audio scenario {sc['id']}",
            f"audio {sc['id']}", sc["save"], sc["ticks"], 300, "audio",
            list(sc["lines"]) + (["input " + sc["input"]] if sc.get("input") else []),
            variant=sc.get("variant"),
            comment=[f"Audio scenario {sc['id']}: {sc['what']}.",
                     "Compared by tools/audio-diff (decoded samples, start ticks, device "
                     "volume and pan per voice, and the mixed output).",
                     "Ledger rows: " + ", ".join("system.audio." + a for a in sc["areas"]) + "."])
        c.extra = {"areas": ["system.audio." + a for a in sc["areas"]], "input": sc.get("input")}
        out.append(c)
    return out


# File-format rows (ledger area system.formats.*): a scenario reaches a
# format only through what the game does with the file, so each check runs a
# fixed scene in which the formats' output shows: the draw list of the town
# arrival (every DCC/DC6/DT1 file drawn, palette, COF layer order, animation
# frame data), the load of a saved character (the .d2s sections), a
# walk/run (animation frame counts drive the movement ticks). The rows each
# check stands for are listed in FMT_ROWS (area prefix -> check).
FMT_ROWS = {
    "gen-fmt-draws-town": ["dcc.", "cof.", "dt1.", "palette.", "dc6.", "animdata.1-", "animdata.2-",
                           "animdata.3-", "d2s-appearance.1-", "d2s-appearance.2-",
                           "d2s-appearance.3-", "d2s-appearance.6-"],
    "gen-fmt-load-ama": ["d2s.1-", "d2s.9-", "d2s-load.1-", "d2s-load.2-", "d2s-load.7-", "d2s-load.8-"],
    "gen-fmt-anim-walk": ["animdata.4-", "animdata.5-", "animdata.6-"],
}


def fam_fmt(ctx):
    town = Check(
        "gen-fmt-draws-town", "fmt", "formats: town arrival draw list",
        "draw list of the Rogue Encampment arrival (DCC/DC6/DT1/COF/palette/animdata)",
        "ScnAma --class ama --expansion", 76, 600, "draws", ["draws-at 73"],
        comment=["Every file the town arrival draws goes through the format readers: the draw "
                 "rows name the DCC, DC6 and DT1 files, the palette shift and the COF layer "
                 "order. Same scene as draws-town-arrival-ama (server tick 73)."])
    load = Check(
        "gen-fmt-load-ama", "fmt", "formats: load of a saved character",
        "load of a saved expansion Amazon, 20 idle ticks: state and packets",
        "ScnAma --class ama --expansion", 20, 300, "state packets", [],
        comment=["The .d2s sections (header, quests, waypoints, stats, skills, items) "
                 "become the player's state; the join packets carry the loaded values "
                 "(specs/formats/d2s-load.md sections 1, 2, 7, 8)."])
    anim = Check(
        "gen-fmt-anim-walk", "fmt", "formats: animation data in movement",
        "Walk and Run in the Rogue Encampment: movement ticks come from AnimData",
        "ScnAma --class ama --expansion --level 12", 80, 300, "state packets",
        ["ignore q",
         "at 6 send Walk x=@x+6 y=@y",
         "at 36 send Run x=@x y=@y+5"],
        comment=["Frame counts and speed of the walk/run modes come from the animation data "
                 "(specs/formats/animdata.md sections 4-6); the player's position per tick shows them."])
    out = [town, load, anim]
    for c in out:
        c.extra = {"fmt": True}
    return out


FAMILY_FN = {"lvl": fam_lvl, "wp": fam_wp, "ai": fam_ai, "su": fam_su, "boss": fam_boss, "umod": fam_umod, "skill": fam_skill, "shrine": fam_shrine, "item": fam_item, "itemq": fam_itemq, "netc2s": fam_netc2s,
             "missile": fam_missile, "state": fam_state, "mon": fam_mon, "obj": fam_obj, "aud": fam_aud, "fmt": fam_fmt}


# ----------------------------------------------------------- ledger join

def load_ledger(path):
    """area ids of the fidelity ledger, for the headers of the checks."""
    areas = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#"):
                continue
            cols = line.rstrip("\n").split("\t")
            if cols[0] == "area":
                continue
            areas.append((cols[0], cols[3] if len(cols) > 3 else cols[1]))
    return areas


def resolve_area(c, areas):
    f, x = c.family, c.extra
    pick = []
    if f == "lvl":
        pick = [a for a, _ in areas if re.fullmatch(rf"level\.a\d\.{x['level']}\..*", a)]
    elif f == "wp":
        n = c.row.split()[-1]
        pick = [a for a, _ in areas if a.startswith(f"waypoint.{n}.")]
    elif f == "ai":
        pick = [a for a, _ in areas if a == f"monster.ai.{x['ai'].lower()}"]
    elif f == "su":
        pick = [a for a, s in areas if a.startswith("monster.superunique.")
                and f"(hcIdx {x['hcIdx']}," in s]
    elif f == "boss":
        pick = [a for a, s in areas if a.startswith("monster.boss.")
                and f"({x['class']})" in s]
    elif f == "umod":
        pick = [a for a, _ in areas if a.startswith(f"monster.umod.{x['umod']}-")]
    elif f == "skill":
        pick = [a for a, s in areas if a.startswith(f"skill.{x['class']}.")
                and f"({x['skill']})" in s]
    elif f == "shrine":
        pick = [a for a, _ in areas if a.startswith(f"shrine.{x['shrine']}.")]
    elif f == "item":
        ids = {a: a for a, _ in areas}
        pick = [f"item.{i}-{cd}" for i, cd in x["items"] if f"item.{i}-{cd}" in ids]
        # the ledger lists the bases "never created" in run a1a2 only; others have no row
        c.area = ",".join(pick) if pick else "-"
        return

    elif f == "netc2s":
        pick = [a for a, _ in areas if a == f"net.c2s.0x{x['msg']:02x}"]
    elif f == "missile":
        pick = [x["area"]] if x["area"] in {a for a, _ in areas} else []
    elif f == "state":
        pick = [a for a, s in areas if a.startswith("state.") and f"({x['state']})" in s]
    elif f == "mon":
        pick = [a for a, s in areas if a.startswith("monster.")
                and s.endswith(f"(hcIdx {x['class']})")]
    elif f == "obj":
        pick = [a for a, _ in areas if re.fullmatch(rf"object\.{x['object']}-.*", a)]
    elif f == "aud":
        c.area = ",".join(x["areas"])
        return
    elif f == "fmt":
        pre = FMT_ROWS[c.name]
        c.area = ",".join(a for a, _ in areas if a.startswith("system.formats.")
                          and any(a[len("system.formats."):].split(".", 1)[-1].startswith(p)
                                  or a[len("system.formats."):].startswith(p) for p in pre)) or "-"
        return
    if len(pick) > 1:
        raise GenError(f"{c.name}: {len(pick)} ledger areas {pick}")
    c.area = pick[0] if pick else "-"


# ------------------------------------------------------------------ main

class Ctx:
    pass


def default_excel():
    gd = os.environ.get("D2_GAME_DIR")
    if not gd:
        raise GenError("--excel DIR or D2_GAME_DIR needed")
    return os.path.join(gd, "extracted", "patch_d2", "data", "global", "excel")


def generate(ctx, families):
    checks = []
    for f in families:
        checks += FAMILY_FN[f](ctx)
    names = [c.name for c in checks]
    if len(set(names)) != len(names):
        raise GenError("duplicate check names")
    if ctx.ledger:
        areas = load_ledger(ctx.ledger)
        for c in checks:
            resolve_area(c, areas)
    return checks


def index_text(checks):
    out = [f"#check-gen-index {GEN_VERSION}",
           f"# generated by {GEN_NAME}; one line per generated check",
           "name\tfamily\trow\tledger_area\ttitle"]
    for c in checks:
        out.append(f"{c.name}\t{c.family}\t{c.row}\t{c.area}\t{c.title}")
    return "\n".join(out) + "\n"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--excel")
    ap.add_argument("--out", default=os.path.join(ROOT, "traces", "checks", "gen"))
    ap.add_argument("--family", action="append", choices=FAMILIES)
    ap.add_argument("--audio-out", default=os.path.join(ROOT, "traces", "audio", "gen"),
                    help="where the aud family is written (audio checks stay out of the suite dir)")
    ap.add_argument("--ledger", default=os.path.join(HERE, "ledger-areas.tsv"),
                    help="ledger area ids for the headers (default: the committed snapshot; "
                         "the full fidelity-ledger.tsv is read too)")
    ap.add_argument("--waypoints", default=os.path.join(ROOT, "specs", "world", "waypoints.tsv"))
    ap.add_argument("--waypoint-towns", default=os.path.join(HERE, "waypoint-towns.tsv"))
    ap.add_argument("--shrine-seeds", default=os.path.join(HERE, "shrine-seeds.tsv"))
    ap.add_argument("--client-messages", default=os.path.join(ROOT, "specs", "sim", "client-messages.tsv"))
    ap.add_argument("--census", default=os.path.join(ROOT, "docs", "handoff", "packet-census.tsv"))
    ap.add_argument("--check", action="store_true", help="fail when the files differ")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        import selftest
        return selftest.run()
    ctx = Ctx()
    ctx.excel = a.excel or (None if a.family == ["aud"] else default_excel())
    ctx.waypoints = a.waypoints
    ctx.waypoint_towns = a.waypoint_towns
    ctx.ledger = a.ledger
    ctx.client_messages = a.client_messages
    ctx.census = a.census
    ctx.shrine_seeds = {}
    if os.path.isfile(a.shrine_seeds):
        for r in read_tsv(a.shrine_seeds, ["shrine", "seed", "object_class", "verified"]):
            ctx.shrine_seeds[int(r["shrine"])] = (int(r["seed"]), int(r["object_class"]),
                                                 r["verified"])
    fams = a.family or FAMILIES
    try:
        checks = generate(ctx, fams)
    except GenError as e:
        print(f"check-gen: {e}", file=sys.stderr)
        return 2
    if a.list:
        for c in checks:
            print(c.name, c.area, c.title, sep="\t")
        return 0
    aud = [c for c in checks if c.family == "aud"]
    checks = [c for c in checks if c.family != "aud"]
    groups = []
    if checks or not aud:
        groups.append((a.out, checks, True))
    if aud:
        groups.append((a.audio_out, aud, False))
    status = 0
    for out_dir, grp, with_index in groups:
        want = {f"{c.name}.check": c.render() for c in grp}
        index_name = "INDEX.tsv"
        if with_index and not a.family:
            want[index_name] = index_text(grp)
        os.makedirs(out_dir, exist_ok=True)
        have = {f for f in os.listdir(out_dir) if f.endswith(".check") or f == index_name}
        whole = not a.family or (not with_index)
        if a.check:
            bad = []
            for f, text in want.items():
                p = os.path.join(out_dir, f)
                if not os.path.isfile(p) or open(p, encoding="utf-8").read() != text:
                    bad.append(f)
            if whole:
                bad += sorted(have - set(want))
            if bad:
                print(f"check-gen: {len(bad)} generated files are stale or missing, e.g. {bad[:3]}",
                      file=sys.stderr)
                status = 1
            else:
                print(f"check-gen: {len(want)} files current")
            continue
        for f, text in want.items():
            with open(os.path.join(out_dir, f), "w", encoding="utf-8", newline="\n") as fh:
                fh.write(text)
        if whole:
            for f in sorted(have - set(want)):
                os.remove(os.path.join(out_dir, f))
        counts = {}
        for c in grp:
            counts[c.family] = counts.get(c.family, 0) + 1
        print("check-gen: wrote %d checks to %s: %s" % (
            len(grp), os.path.relpath(out_dir, ROOT),
            ", ".join(f"{k} {v}" for k, v in counts.items())))
    return status


if __name__ == "__main__":
    sys.exit(main())
