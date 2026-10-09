#!/usr/bin/env python3
# Spec: specs/tools/coverage-map.md
"""Behaviour coverage map: merges the `coverage-map 1` counter files that a
d2-sim built with the `coverage-map` feature writes (D2_COVERAGE_DIR), per
source run, and compares them with the install's tables: what was exercised
and how often, and what the tables hold that was never exercised, ranked by
how early a player meets it.

    python3 tools/coverage-map/report.py --excel DIR[,DIR...] \
        --run NAME=COVDIR [--run NAME=COVDIR ...] [--md OUT.md] [--json OUT.json]
    python3 tools/coverage-map/report.py --selftest

--excel: the excel folders, highest priority first (a table is read from
the first folder that has it), e.g. the private repo's
extracted/Patch_D2.mpq/data/global/excel, extracted/d2exp.mpq/...,
extracted/d2data.mpq/... Standard library only. Our own code.
"""

import argparse
import collections
import glob
import json
import os
import sys

VERSION = "0.1.0"
FORMAT = "coverage-map 1"
CATS = ("skill", "monster", "monster-ai", "object", "item", "npc-topic",
        "quest", "level", "missile", "state")

# Quest slots (specs/world/quests.md §1.9 and its slot list): slot -> name.
QUESTS = {
    0: "Warriv gossip", 1: "Den of Evil", 2: "Sisters' Burial Grounds",
    3: "Tools of the Trade", 4: "Search for Cain", 5: "The Forgotten Tower",
    6: "Sisters to the Slaughter", 7: "Act I completed", 8: "Act II intro",
    9: "Radament's Lair", 10: "The Horadric Staff", 11: "Tainted Sun",
    12: "Arcane Sanctuary", 13: "The Summoner", 14: "The Seven Tombs",
    15: "Act II completed", 16: "Act III intro", 17: "Lam Esen's Tome",
    18: "Khalim's Will", 19: "Blade of the Old Religion",
    20: "The Golden Bird", 21: "The Blackened Temple", 22: "The Guardian",
    23: "Act III completed", 24: "Act IV intro", 25: "The Fallen Angel",
    26: "Terror's End", 27: "Hell's Forge", 28: "Act IV completed",
    35: "Siege on Harrogath", 36: "Rescue on Mount Arreat",
    37: "Prison of Ice", 38: "Betrayal of Harrogath", 39: "Rite of Passage",
    40: "Eve of Destruction",
}
QUALITY = {1: "low", 2: "normal", 3: "superior", 4: "magic", 5: "set",
           6: "rare", 7: "unique", 8: "crafted", 9: "tempered"}
NPC_KIND = {0: "interact", 1: "menu", 2: "message"}
# Character level at which a normal-difficulty player typically enters
# each act (d2rs-own estimate for the ranking only).
CLVL_ACT = (1, 15, 24, 30, 35)


class ReportError(Exception):
    pass


# ---------------------------------------------------------------- input


def read_counts(paths):
    """Merges counter files -> {cat: Counter{(a, b): n}}."""
    out = {c: collections.Counter() for c in CATS}
    for p in paths:
        with open(p, encoding="utf-8") as f:
            lines = f.read().splitlines()
        if not lines or lines[0] != FORMAT:
            raise ReportError(f"{p}: first line must be '{FORMAT}'")
        for no, line in enumerate(lines[1:], 2):
            parts = line.split("\t")
            if len(parts) != 4 or parts[0] not in out:
                raise ReportError(f"{p}:{no}: bad line {line!r}")
            out[parts[0]][(int(parts[1]), int(parts[2]))] += int(parts[3])
    return out


def read_table(dirs, name):
    """A .txt table as a list of dicts (header keys), from the first dir
    that has it (case-insensitive file name)."""
    for d in dirs:
        if not os.path.isdir(d):
            continue
        for f in os.listdir(d):
            if f.lower() == name.lower() + ".txt":
                with open(os.path.join(d, f), encoding="latin-1") as fh:
                    rows = [l.rstrip("\r\n").split("\t") for l in fh]
                head = rows[0]
                return [dict(zip(head, r)) for r in rows[1:]]
    raise ReportError(f"table {name}.txt not found in {dirs}")


def num(v, default=0):
    try:
        return int(v)
    except (TypeError, ValueError):
        return default


# ---------------------------------------------------------------- tables


class Tables:
    """The pieces of the install's tables the report needs, with a
    'progression position' per level: (act, row) of the levels in the order
    a player meets them (act, then Levels.txt order)."""

    def __init__(self, dirs):
        self.levels = read_table(dirs, "Levels")
        self.monstats = read_table(dirs, "monstats")
        self.skills = read_table(dirs, "skills")
        self.missiles = read_table(dirs, "Missiles")
        self.states = read_table(dirs, "states")
        self.objects = read_table(dirs, "objects")
        self.items = []
        for t in ("weapons", "armor", "misc"):
            self.items += [r for r in read_table(dirs, t)
                           if r.get("code") and r.get("name", "") != "Expansion"]
        self.superuniques = read_table(dirs, "SuperUniques")
        self._index()

    def _index(self):
        self.level_name = {num(r["Id"], -1): r.get("LevelName") or r["Name"]
                           for r in self.levels if r.get("Id")}
        self.level_act = {num(r["Id"], -1): num(r["Act"]) for r in self.levels if r.get("Id")}
        # monstats: row index = hcIdx; Id = the text id.
        self.mon_by_text = {r["Id"]: num(r["hcIdx"], i) for i, r in enumerate(self.monstats)}
        self.mon_name = {num(r["hcIdx"], i): r["Id"] for i, r in enumerate(self.monstats)}
        self.skill_by_name = {r["skill"]: num(r["Id"], i) for i, r in enumerate(self.skills)}
        self.skill_name = {num(r["Id"], i): r["skill"] for i, r in enumerate(self.skills)}
        self.miss_by_name = {r["Missile"]: num(r["Id"], i) for i, r in enumerate(self.missiles)}
        self.miss_name = {num(r["Id"], i): r["Missile"] for i, r in enumerate(self.missiles)}
        self.state_by_name = {r["state"]: num(r["id"], i) for i, r in enumerate(self.states)}
        self.state_name = {num(r["id"], i): r["state"] for i, r in enumerate(self.states)}
        self.obj_name = {num(r["Id"], i): r["Name"] for i, r in enumerate(self.objects)}
        # Progression: the order a player meets levels (act, then row).
        order = sorted((self.level_act[i], i) for i in self.level_name if i > 0)
        self.level_rank = {lid: n for n, (_, lid) in enumerate(order)}
        # Monster -> earliest level rank and level count (Levels.txt mon /
        # nmon / umon columns; superuniques by their class).
        self.mon_rank, self.mon_levels = {}, collections.Counter()
        for r in self.levels:
            lid = num(r.get("Id"), -1)
            if lid not in self.level_rank:
                continue
            seen = set()
            for col in [f"{p}mon{k}" for p in ("", "n", "u") for k in range(1, 11)]:
                m = self.mon_by_text.get(r.get(col, ""))
                if m is not None and m not in seen:
                    seen.add(m)
                    self.mon_levels[m] += 1
                    self.mon_rank[m] = min(self.mon_rank.get(m, 1 << 30), self.level_rank[lid])
        # Minions and spawned classes reachable from a ranked monster.
        for _ in range(3):
            for r in self.monstats:
                m = num(r["hcIdx"], -1)
                if m not in self.mon_rank:
                    continue
                for col in ("minion1", "minion2", "spawn"):
                    c = self.mon_by_text.get(r.get(col, ""))
                    if c is not None and c not in self.mon_rank:
                        self.mon_rank[c] = self.mon_rank[m]
        # Skill rank: player skills by required level (rank 0..99 scaled
        # into the level ranks of the act a level-L character plays), monster
        # skills by their earliest monster.
        self.skill_rank = {}
        act_first = {}
        for lid, rank in self.level_rank.items():
            a = self.level_act[lid]
            act_first[a] = min(act_first.get(a, 1 << 30), rank)
        for i, r in enumerate(self.skills):
            sid = num(r["Id"], i)
            if r.get("charclass"):
                lvl = num(r.get("reqlevel"), 1)
                act = max(a for a, start in enumerate(CLVL_ACT) if lvl >= start)
                self.skill_rank[sid] = act_first.get(act, 0) + (lvl - CLVL_ACT[act])
        for r in self.monstats:
            m = num(r["hcIdx"], -1)
            if m not in self.mon_rank:
                continue
            for k in range(1, 9):
                s = self.skill_by_name.get(r.get(f"Skill{k}", ""))
                if s is not None and s not in self.skill_rank:
                    self.skill_rank[s] = self.mon_rank[m]
                elif s is not None:
                    self.skill_rank[s] = min(self.skill_rank[s], self.mon_rank[m])
        # Missile / state rank: from the skills that make them, and monster
        # mode missiles.
        self.miss_rank, self.state_rank = {}, {}
        for i, r in enumerate(self.skills):
            rank = self.skill_rank.get(num(r["Id"], i))
            if rank is None:
                continue
            for col in ("srvmissile", "srvmissilea", "srvmissileb", "srvmissilec"):
                mi = self.miss_by_name.get(r.get(col, ""))
                if mi is not None:
                    self.miss_rank[mi] = min(self.miss_rank.get(mi, 1 << 30), rank)
            for col in ("aurastate", "auratargetstate", "passivestate"):
                st = self.state_by_name.get(r.get(col, ""))
                if st is not None:
                    self.state_rank[st] = min(self.state_rank.get(st, 1 << 30), rank)
        for r in self.monstats:
            m = num(r["hcIdx"], -1)
            if m not in self.mon_rank:
                continue
            for col in ("MissA1", "MissA2", "MissS1", "MissS2", "MissS3", "MissS4", "MissC", "MissSQ"):
                mi = self.miss_by_name.get(r.get(col, ""))
                if mi is not None:
                    self.miss_rank[mi] = min(self.miss_rank.get(mi, 1 << 30), self.mon_rank[m])


# ---------------------------------------------------------------- analysis


def act_of_rank(t, rank):
    """The act (1-5) of a level rank, for display."""
    if rank is None:
        return "-"
    for lid, r in t.level_rank.items():
        if r == rank:
            return str(t.level_act[lid] + 1)
    # skill ranks are 2 x reqlevel: roughly act by level band
    return "?"


def analyse(t, runs):
    """runs: {name: counts}. Returns the per-category sections."""
    total = {c: collections.Counter() for c in CATS}
    for counts in runs.values():
        for c in CATS:
            total[c].update(counts[c])

    def per(cat, key=lambda ab: ab[0]):
        out = collections.Counter()
        for ab, n in total[cat].items():
            out[key(ab)] += n
        return out

    def run_list(cat, ident, key=lambda ab: ab[0]):
        return sorted(name for name, c in runs.items()
                      if any(key(ab) == ident for ab in c[cat]))

    sec = {}

    # Levels.
    seen = per("level")
    all_ids = sorted(i for i in t.level_name if i > 0)
    sec["level"] = {
        "title": "Levels entered",
        "seen": [(i, t.level_name.get(i, "?"), seen[i], run_list("level", i)) for i in sorted(seen)],
        "table": len(all_ids),
        "gaps": sorted(((t.level_rank[i], i, t.level_name[i], t.level_act[i] + 1)
                        for i in all_ids if i not in seen)),
        "rank_note": "act, then Levels.txt order (the order a player walks into them)",
    }

    # Monsters.
    seen = per("monster")
    spawnable = {m for m in t.mon_rank}
    sec["monster"] = {
        "title": "Monster classes created",
        "seen": [(m, t.mon_name.get(m, "?"), seen[m], run_list("monster", m)) for m in sorted(seen)],
        "table": len(t.monstats),
        "spawnable": len(spawnable),
        "gaps": sorted((t.mon_rank[m], m, t.mon_name.get(m, "?"), t.mon_levels[m])
                       for m in spawnable if m not in seen),
        "rank_note": "earliest level whose Levels.txt mon/nmon/umon columns name the class (minions and spawns inherit their parent's)",
    }

    # Monster AI: by monstats AI column (name) over the classes whose AI ran.
    ai_ran = collections.Counter()
    fn_ran = collections.Counter()
    for (fn, cls), n in total["monster-ai"].items():
        row = t.monstats[cls] if 0 <= cls < len(t.monstats) else {}
        ai_ran[row.get("AI", "?")] += n
        fn_ran[fn] += n
    ai_rank = {}
    for m, rank in t.mon_rank.items():
        if 0 <= m < len(t.monstats):
            ai = t.monstats[m].get("AI", "")
            if ai:
                ai_rank[ai] = min(ai_rank.get(ai, 1 << 30), rank)
    sec["monster-ai"] = {
        "title": "Monster AI (monstats AI column of the classes whose AI function ran)",
        "seen": [(0, ai, n, []) for ai, n in sorted(ai_ran.items())],
        "functions": sorted(fn_ran.items()),
        "table": len({r.get("AI") for r in t.monstats if r.get("AI")}),
        "gaps": sorted((r, ai) for ai, r in ai_rank.items() if ai not in ai_ran),
        "rank_note": "earliest spawn rank of a monster with that AI",
    }

    # Skills.
    seen = per("skill")
    sec["skill"] = {
        "title": "Skills started or done (srvst / srvdo)",
        "seen": [(s, t.skill_name.get(s, "?"), seen[s], run_list("skill", s)) for s in sorted(seen)],
        "start_do": {s: (total["skill"][(s, 0)], total["skill"][(s, 1)]) for s in seen},
        "table": len(t.skills),
        "gaps": sorted((t.skill_rank[s], s, t.skill_name.get(s, "?"),
                        t.skills[s].get("charclass") or "monster")
                       for s in t.skill_rank if s not in seen and 0 <= s < len(t.skills)),
        "rank_note": "player skills: the first level of the act a character of the skill's reqlevel plays (act I from 1, II 15, III 24, IV 30, V 35) plus the levels above that band; monster skills: the earliest spawn rank of a monster with the skill (Skill1-8)",
    }

    # Missiles.
    seen = per("missile")
    sec["missile"] = {
        "title": "Missiles created",
        "seen": [(m, t.miss_name.get(m, "?"), seen[m], run_list("missile", m)) for m in sorted(seen)],
        "table": len(t.missiles),
        "gaps": sorted((r, m, t.miss_name.get(m, "?")) for m, r in t.miss_rank.items() if m not in seen),
        "rank_note": "rank of the earliest skill (srvmissile, a-c) or monster mode missile that creates it",
    }

    # States.
    seen = per("state")
    sec["state"] = {
        "title": "States applied",
        "seen": [(s, t.state_name.get(s, "?"), seen[s], run_list("state", s)) for s in sorted(seen)],
        "table": len(t.states),
        "gaps": sorted((r, s, t.state_name.get(s, "?")) for s, r in t.state_rank.items() if s not in seen),
        "rank_note": "rank of the earliest skill whose aurastate / auratargetstate / passivestate it is",
    }

    # Objects (operated).
    seen = per("object")
    fns = per("object", key=lambda ab: ab[1])
    operable = [(num(r.get("Act"), 0), num(r["Id"], i), r["Name"], num(r.get("OperateFn")))
                for i, r in enumerate(t.objects) if num(r.get("OperateFn")) > 0]
    all_fns = sorted({o[3] for o in operable})
    sec["object"] = {
        "title": "Objects operated (objects row, operate function)",
        "seen": [(o, t.obj_name.get(o, "?"), seen[o], run_list("object", o)) for o in sorted(seen)],
        "functions": sorted(fns.items()),
        "table": len(operable),
        "fn_gaps": [f for f in all_fns if f not in fns],
        "gaps": sorted((fnrank(f), oid, name, f) for act, oid, name, f in operable if oid not in seen),
        "rank_note": "operate function commonness (chests, doors, shrines, wells, waypoints, portals first), then row",
    }

    # Items.
    seen = per("item")
    quals = per("item", key=lambda ab: ab[1])
    sec["item"] = {
        "title": "Items created (base item, quality)",
        "seen_count": len(seen),
        "qualities": sorted(quals.items()),
        "top": seen.most_common(25),
        "names": {i: t.items[i].get("name", "?") for i in seen if 0 <= i < len(t.items)},
        "table": len(t.items),
        "gaps": sorted((num(r.get("level")) + (1000 if num(r.get("quest")) else 0), i,
                        r.get("name", "?"), r.get("code"))
                       for i, r in enumerate(t.items) if i not in seen and num(r.get("spawnable"), 1)),
        "qual_gaps": [q for q in QUALITY if q not in quals],
        "rank_note": "base item level (level column: potions, scrolls, keys and gems are level 0); quest items after (+1000); non-spawnable rows not listed",
    }

    # NPC topics.
    npcs = collections.Counter()
    kinds = collections.Counter()
    for (cls, b), n in total["npc-topic"].items():
        npcs[cls] += n
        kinds[(cls, b >> 16, b & 0xFFFF)] += n
    town_npcs = [num(r["hcIdx"], i) for i, r in enumerate(t.monstats)
                 if r.get("npc") == "1" and r.get("interact") == "1"]
    sec["npc-topic"] = {
        "title": "NPC interactions (interact, menu action, quest message)",
        "seen": [(c, t.mon_name.get(c, "?"), npcs[c], run_list("npc-topic", c)) for c in sorted(npcs)],
        "kinds": sorted(kinds.items()),
        "table": len(town_npcs),
        "gaps": sorted((c, c, t.mon_name.get(c, "?")) for c in town_npcs if c not in npcs),
        "rank_note": "npc=1 interact=1 monstats rows in monstats order (act I NPCs first: the rows follow the acts)",
    }

    # Quests.
    seen = per("quest")
    bits = total["quest"]
    sec["quest"] = {
        "title": "Quest flags set (slot.bit)",
        "seen": [(q, QUESTS.get(q, "?"), seen[q],
                  sorted(b for (qq, b) in bits if qq == q)) for q in sorted(seen)],
        "table": len(QUESTS),
        "gaps": sorted((q, QUESTS[q]) for q in QUESTS if q not in seen),
        "done_missing": sorted((q, QUESTS[q]) for q in QUESTS if q in seen and (q, 0) not in bits),
        "rank_note": "quest slot = story order",
    }
    return sec


# Operate functions roughly by how often a player uses them (our own
# ordering from objects.txt use: chests 4, doors 8/16?, shrines 2, wells 22,
# waypoints 23, portals). Unknown functions after.
COMMON_FNS = [1, 4, 2, 8, 23, 22, 5, 3, 14, 15, 18, 19]


def fnrank(f):
    return COMMON_FNS.index(f) if f in COMMON_FNS else 100 + f


# ---------------------------------------------------------------- output


def md(t, sec, runs, top_gaps=25):
    o = []
    w = o.append
    w("# Behaviour coverage map")
    w("")
    w(f"Generated by `tools/coverage-map/report.py` {VERSION} from the `coverage-map 1`")
    w("counters of a d2-sim built with `--features coverage-map`; the tables are the")
    w("install's (Patch_D2 > d2exp > d2data excel). Runs merged:")
    w("")
    for name, c in runs.items():
        n = sum(sum(v.values()) for v in c.values())
        w(f"- `{name}`: {n:,} events")
    w("")
    w("## Summary")
    w("")
    w("| Category | Distinct exercised | In the tables | Reachable, never exercised (ranked below) |")
    w("|---|---|---|---|")
    for c in CATS:
        s = sec[c]
        dist = s.get("seen_count", len(s.get("seen", [])))
        tab = s.get("spawnable", s["table"])
        w(f"| {s['title']} | {dist} | {tab} | {len(s['gaps'])} |")
    w("")
    w("\"In the tables\" counts every row (monsters: the classes a level can spawn,")
    w("with minions; objects: rows with an operate function). \"Reachable\" gaps are")
    w("the rows a player can meet by the table links of the ranking rule; rows")
    w("with no link (unused, test or client-only rows) are not listed.")
    w("")
    for c in CATS:
        s = sec[c]
        w(f"## {s['title']}")
        w("")
        w(f"Gap ranking: {s['rank_note']}.")
        w("")
        if c == "item":
            w(f"Distinct base items created: {s['seen_count']} of {s['table']}. Qualities: "
              + ", ".join(f"{QUALITY.get(q, q)} {n:,}" for q, n in s["qualities"])
              + ". Never: " + (", ".join(QUALITY[q] for q in s["qual_gaps"]) or "none") + ".")
            w("")
            w("Most created: " + ", ".join(f"{s['names'].get(i, i)} ({n:,})" for i, n in s["top"]) + ".")
        elif c == "quest":
            w("| Slot | Quest | Flag sets | Bits set |")
            w("|---|---|---|---|")
            for q, name, n, b in s["seen"]:
                w(f"| {q} | {name} | {n:,} | {', '.join(map(str, b))} |")
            if s["done_missing"]:
                w("")
                w("Touched but never completed (bit 0 never set): "
                  + ", ".join(f"{q} {n}" for q, n in s["done_missing"]) + ".")
        else:
            seen = s["seen"]
            w("<details><summary>Exercised (" + str(len(seen)) + ")</summary>")
            w("")
            w("| Id | Name | Count | Runs |")
            w("|---|---|---|---|")
            for i, name, n, rl in seen:
                extra = ""
                if c == "skill":
                    st, do = s["start_do"][i]
                    extra = f" (start {st:,}, do {do:,})"
                w(f"| {i} | {name}{extra} | {n:,} | {', '.join(rl) if isinstance(rl, list) else rl} |")
            w("")
            w("</details>")
            if c == "object":
                w("")
                w("Operate functions run: " + ", ".join(f"{f} ({n:,})" for f, n in s["functions"])
                  + ". Never run: " + ", ".join(map(str, s["fn_gaps"])) + ".")
            if c == "monster-ai":
                w("")
                w(f"Distinct AI function addresses run: {len(s['functions'])}.")
        w("")
        gaps = s["gaps"]
        w(f"**Never exercised: {len(gaps)}.** First {min(top_gaps, len(gaps))} by likelihood:")
        w("")
        for g in gaps[:top_gaps]:
            w("- " + gap_text(t, c, g))
        if len(gaps) > top_gaps:
            w(f"- … and {len(gaps) - top_gaps} more (`--json` lists all)")
        w("")
    return "\n".join(o) + "\n"


def gap_text(t, c, g):
    if c == "level":
        _, i, name, act = g
        return f"{i} {name} (act {act})"
    if c == "monster":
        rank, m, name, nlev = g
        return f"{m} {name}: first in {rank_level(t, rank)}, in {nlev} level(s)"
    if c == "monster-ai":
        rank, ai = g
        return f"{ai}: first in {rank_level(t, rank)}"
    if c == "skill":
        rank, s, name, cls = g
        return f"{s} {name} ({cls}, rank {rank})"
    if c in ("missile", "state"):
        rank, i, name = g
        return f"{i} {name} (rank {rank})"
    if c == "object":
        _, i, name, f = g
        return f"{i} {name} (operate fn {f})"
    if c == "item":
        lvl, i, name, code = g
        what = f"quest item, level {lvl - 1000}" if lvl >= 1000 else f"level {lvl}"
        return f"{i} {name} `{code}` ({what})"
    if c == "npc-topic":
        _, cl, name = g
        return f"{cl} {name}"
    if c == "quest":
        q, name = g
        return f"slot {q} {name}"
    return str(g)


def rank_level(t, rank):
    for lid, r in t.level_rank.items():
        if r == rank:
            return f"level {lid} {t.level_name[lid]} (act {t.level_act[lid] + 1})"
    return "no level"


# ---------------------------------------------------------------- main


def selftest():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        p = os.path.join(d, "cov-1-0.tsv")
        with open(p, "w") as f:
            f.write(FORMAT + "\nskill\t36\t1\t3\nlevel\t2\t0\t1\nskill\t36\t1\t2\n")
        c = read_counts([p])
        assert c["skill"][(36, 1)] == 5 and c["level"][(2, 0)] == 1
        with open(p, "w") as f:
            f.write(FORMAT + "\nbogus\t1\t2\t3\n")
        try:
            read_counts([p])
            raise AssertionError("bad category accepted")
        except ReportError:
            pass
        with open(p, "w") as f:
            f.write("coverage-map 0\n")
        try:
            read_counts([p])
            raise AssertionError("bad format accepted")
        except ReportError:
            pass
    print("selftest ok")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--excel", help="excel dirs, highest priority first, comma-separated")
    ap.add_argument("--run", action="append", default=[], help="NAME=COVDIR (repeatable)")
    ap.add_argument("--md")
    ap.add_argument("--json")
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    if not a.excel or not a.run:
        ap.error("--excel and at least one --run are needed")
    try:
        t = Tables(a.excel.split(","))
        runs = {}
        for r in a.run:
            name, _, d = r.partition("=")
            files = sorted(glob.glob(os.path.join(d, "cov-*.tsv")))
            if not files:
                raise ReportError(f"{d}: no cov-*.tsv files")
            runs[name] = read_counts(files)
        sec = analyse(t, runs)
    except ReportError as e:
        print(f"coverage-map: {e}", file=sys.stderr)
        return 3
    if a.md:
        with open(a.md, "w", encoding="utf-8") as f:
            f.write(md(t, sec, runs, a.top))
    if a.json:
        with open(a.json, "w", encoding="utf-8") as f:
            json.dump({"format": "coverage-report 1", "sections": sec}, f, indent=1, default=str)
    if not a.md and not a.json:
        sys.stdout.write(md(t, sec, runs, a.top))
    return 0


if __name__ == "__main__":
    sys.exit(main())
