#!/usr/bin/env python3
"""Route a first-difference report to the spec section(s), crate module(s)
and owner session that own the differing field or message.

    python3 tools/coord/route.py REPORT        # a file, or - / nothing for stdin
    python3 tools/coord/route.py --json REPORT # machine-readable routes
    python3 tools/coord/route.py --selftest

Accepted reports (text or JSON):
  state_diff.py      "FIRST DIVERGENCE: frame N <type> T:G class C, field F: ..."
  packets_diff.py    "frame W stream s2c|c2s #i ... (id 0xNN)"
  rng_diff.py        "frame N, <owner>, draw #i, field F"
  facts-compare      "tick N draw row n (Op) column c: ..."
  diff-summary-1 / scenario-diff-result-1 JSON (each channel's `first`)
  playthrough        `--json` result (playthrough-result-1, every act's
                     first blocker) or its text ("<milestone> (<kind>) f<N>: <predicate>: ...")

Field -> spec: the Owner column of specs/tools/state-snapshot.md §2. Message
-> spec: specs/sim/client-messages.tsv / server-messages.tsv give the name,
then the specs that name `0xNN` together with it. Spec -> module: the
`// Spec:` headers in crates/. Spec / module -> owner: tools/coord/owners.tsv
(area, path prefixes, session id, branch; longest prefix wins). Anything not
matched is "unrouted". Python stdlib only.
"""

import argparse
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OWNERS = os.path.join(REPO, "tools", "coord", "owners.tsv")
SPEC_REF = re.compile(r"`?((?:specs/)?[a-z0-9_-]+/[a-z0-9_.-]+\.md)`?((?:,? ?§[0-9A-Za-z.–-]+(?: r\d+)?)*)")
TYPE_NAMES = {"player": 0, "monster": 1, "object": 2, "missile": 3, "item": 4, "tile": 5}
# Report anchors that are not a §2 table row.
EXTRA_FIELDS = {
    "seed": [("specs/sim/rng.md", "§5.2")],
    "(unit)": [("specs/sim/unit-order.md", "§2"), ("specs/sim/units.md", "§2")],
}
PRESENCE_BY_TYPE = {  # a missing / extra unit: who creates units of that type
    1: [("specs/monsters/init.md", "")],
    2: [("specs/world/objects.md", "")],
    3: [("specs/missiles/missiles.md", "")],
    4: [("specs/items/generation.md", "")],
}
QUEST = [("specs/world/quests.md", "§1.1–§1.4")]


def rd(path):
    with open(os.path.join(REPO, path), encoding="utf-8") as f:
        return f.read()


def norm_spec(p):
    return p if p.startswith("specs/") else "specs/" + p


def parse_refs(text):
    out = []
    for m in SPEC_REF.finditer(text):
        out.append((norm_spec(m.group(1)), m.group(2).strip(", ")))
    return out


# ---------------------------------------------------------------- knowledge


class Kb:
    def __init__(self):
        self.fields = self._fields()
        self.c2s, self.s2c = self._messages("specs/sim/client-messages.tsv"), self._messages("specs/sim/server-messages.tsv")
        self.modules = self._modules()
        self.owners = self._owners()
        self._spec_cache = {}

    @staticmethod
    def _fields():
        """state-snapshot.md §2 table: key -> [(spec, section)]."""
        out = {}
        for line in rd("specs/tools/state-snapshot.md").splitlines():
            if not line.startswith("| `"):
                continue
            cols = [c.strip() for c in line.strip("|").split("|")]
            keys = re.findall(r"`([a-z]+)`", cols[0])
            refs = parse_refs(cols[-1])
            for k in keys:
                out.setdefault(k, refs)
        for k, v in EXTRA_FIELDS.items():
            out.setdefault(k, v)
        return out

    @staticmethod
    def _messages(path):
        out = {}
        lines = rd(path).splitlines()
        head = lines[0].split("\t")
        for line in lines[1:]:
            row = dict(zip(head, line.split("\t")))
            try:
                out[int(row["id"], 16)] = row
            except (KeyError, ValueError):
                pass
        return out

    @staticmethod
    def _modules():
        """spec path -> [(module path, sections named, number of specs in the header)]."""
        out = {}
        for d, _, files in os.walk(os.path.join(REPO, "crates")):
            if "/target" in d:
                continue
            for fn in files:
                if not fn.endswith(".rs"):
                    continue
                p = os.path.join(d, fn)
                with open(p, encoding="utf-8", errors="replace") as f:
                    head = [l for _, l in zip(range(40), f) if l.startswith("// Spec:") or l.startswith("//! Spec:")]
                if not head:
                    continue
                refs = parse_refs(" ".join(head))
                rel = os.path.relpath(p, REPO)
                for spec, sec in refs:
                    out.setdefault(spec, []).append((rel, sec, len(refs)))
        return out

    @staticmethod
    def _owners():
        rows = []
        if not os.path.exists(OWNERS):
            return rows
        for line in rd(os.path.relpath(OWNERS, REPO)).splitlines():
            if not line.strip() or line.startswith("#") or line.startswith("area\t"):
                continue
            c = line.split("\t")
            if len(c) < 4:
                continue
            rows.append({"area": c[0], "paths": [p.strip() for p in c[1].split(",") if p.strip()],
                         "session": c[2], "branch": c[3]})
        return rows

    def spec_lines(self, spec):
        if spec not in self._spec_cache:
            try:
                self._spec_cache[spec] = rd(spec).splitlines()
            except OSError:
                self._spec_cache[spec] = None
        return self._spec_cache[spec]

    def section_at(self, spec, lineno):
        lines = self.spec_lines(spec) or []
        for i in range(min(lineno, len(lines) - 1), -1, -1):
            if re.match(r"#{2,4} ", lines[i]):
                return lines[i].lstrip("#").strip()
        return ""

    def message_specs(self, mid, name, direction):
        """Specs naming the message: lines with `0xNN` and the name (or
        `<dir> 0xNN`); ranked by hits. -> [(spec, section)]."""
        hexid = f"0x{mid:02X}"
        hits = {}
        for d, _, files in os.walk(os.path.join(REPO, "specs")):
            for fn in files:
                if not fn.endswith(".md"):
                    continue
                spec = os.path.relpath(os.path.join(d, fn), REPO)
                for i, l in enumerate(self.spec_lines(spec) or []):
                    if re.search(rf"\b{hexid}\b", l, re.I) and (
                            (name and name != "-" and re.search(rf"\b{re.escape(name)}\b", l))
                            or re.search(rf"{direction}\W{{0,3}}{hexid}", l, re.I)):
                        hits.setdefault(spec, []).append(i)
        ranked = sorted(hits.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:3]
        return [(s, self.section_at(s, ls[0])) for s, ls in ranked]

    def modules_for(self, spec, sec):
        mods = self.modules.get(spec, [])
        num = re.match(r"§([0-9]+)", sec or "")

        def score(m):
            path, msec, n = m
            hit = bool(num) and re.search(rf"§{num.group(1)}\b", msec or "") is not None
            test = "/tests/" in path or path.endswith("tests.rs") or path.endswith("_tests.rs")
            return (test, not hit, n, path)
        return [m[0] for m in sorted(mods, key=score)[:4]]

    def owner_for(self, paths):
        """The owner of the first path any row covers (paths in route order:
        the primary spec first); longest prefix among rows for that path."""
        for p in paths:
            best, blen = None, -1
            for row in self.owners:
                for pre in row["paths"]:
                    if p.startswith(pre) and len(pre) > blen:
                        best, blen = row, len(pre)
            if best:
                return best
        return None


# ---------------------------------------------------------------- parsing


def findings(text):
    """Report -> [(channel, line)]: the first-difference lines to route."""
    t = text.strip()
    if t.startswith("{"):
        try:
            return json_findings(json.loads(t))
        except ValueError:
            pass
    out = []
    for line in t.splitlines():
        s = line.strip()
        if re.search(r"FIRST DIVERGENCE:.*, field ", s) and "draw #" not in s:
            out.append(("state", s))
        elif "draw #" in s and "field" in s:
            out.append(("rng", s))
        elif re.search(r"\bstream (s2c|c2s|tick|buf)\b", s, re.I):
            out.append(("packets", s))
        elif re.search(r"\bdraw row \d+ \(", s):
            out.append(("draws", s))
        elif re.match(r"^(?:first blocker:\s*)?[\w.-]+ \((stuck|missing-unit|crash|error|timeout|[a-z-]+)\) f\d+:", s):
            out.append(("playthrough", s))
    if not out:  # a bare state line from the next-N list or a per-field summary
        for line in t.splitlines():
            if re.search(r"frame \d+ .*: \w+ \S+ vs \S+", line):
                out.append(("state", line.strip()))
                break
    return out


def json_findings(j):
    fmt = j.get("format", "")
    if fmt == "diff-summary-1":
        return [(j.get("channel", "?"), j["first"]["text"])] if j.get("first") else []
    if fmt == "scenario-diff-result-1":
        out = []
        for ch, c in (j.get("channels") or {}).items():
            sm = c.get("summary") or {}
            if sm.get("first"):
                out.append((ch, sm["first"]["text"]))
        return out
    if fmt == "playthrough-result-1" or "acts" in j:
        out = []
        for a in j.get("acts", []):
            fb = a.get("first_blocker")
            if fb:
                out.append(("playthrough", f"{fb['milestone']} ({fb['kind']}) f{fb['frame']}: {fb['evidence']}"))
        return out
    return []


# ---------------------------------------------------------------- routing


def route_state(kb, s):
    m = re.search(r"field ([\w()]+)", s)
    if not m:
        return [], "no field"
    f = m.group(1)
    refs = list(kb.fields.get(f, []))
    if f == "(unit)":
        t = re.search(r"\b(player|monster|object|missile|item|tile) (\d)", s)
        if t:
            refs = PRESENCE_BY_TYPE.get(int(t.group(2)), []) + refs
    return refs, f"state field `{f}`"


def route_packets(kb, s):
    st = re.search(r"\bstream (\w+)", s)
    idm = re.search(r"\(id (0x[0-9a-fA-F]+)\)", s) or re.search(r"\bid (0x[0-9a-fA-F]+)", s)
    if not idm:
        return [("specs/tools/packets-trace.md", "")], f"packets stream {st.group(1) if st else '?'}, no message id"
    mid = int(idm.group(1), 16)
    direction = (st.group(1).lower() if st else "s2c")
    table = kb.c2s if direction == "c2s" else kb.s2c
    row = table.get(mid)
    name = row.get("name") if row else None
    tsv = "specs/sim/client-messages.tsv" if direction == "c2s" else "specs/sim/server-messages.tsv"
    refs = ([(tsv, f"row 0x{mid:02X}")] if row else []) + kb.message_specs(mid, name, "C→S" if direction == "c2s" else "S→C")
    what = f"{direction.upper()} 0x{mid:02X} {name or '(no table row)'}"
    if row:
        what += f" ({tsv}: produced_by {row.get('produced_by', row.get('scope', '?'))})"
    return refs, what


def route_rng(kb, s):
    refs = [("specs/sim/rng.md", "")]
    own = re.search(r"frame \d+, ([^,]+),", s)
    owner = own.group(1) if own else "?"
    extra = []
    t = re.search(r"\b(player|monster|object|missile|item)\b", owner)
    if t:
        extra = PRESENCE_BY_TYPE.get(TYPE_NAMES[t.group(1)], [])
    # an owner naming a spec or a function site the specs cite
    extra += parse_refs(owner)
    return extra + refs, f"rng draw by `{owner}`"


def route_draws(kb, s):
    m = re.search(r"draw row \d+ \((\w+)\)", s)
    op = m.group(1) if m else "?"
    refs = []
    for spec in ("specs/client/render-pipeline.md", "specs/tools/facts-render.md"):
        lines = kb.spec_lines(spec) or []
        hit = next((i for i, l in enumerate(lines) if re.search(rf"`{re.escape(op)}`", l)), None)
        if hit is not None:
            refs.append((spec, kb.section_at(spec, hit)))
    return refs or [("specs/client/render-pipeline.md", "")], f"draw op `{op}`"


def route_playthrough(kb, s):
    refs, what = [], []
    q = re.search(r"\bquest (\d+) (\d+)", s)
    if q:
        refs += QUEST
        what.append(f"quest slot {q.group(1)} bit {q.group(2)}")
    for m in re.finditer(r"\bplayer (\w+) (?:==|!=|>=|<=|>|<)", s):
        f = m.group(1)
        if f == "moved":
            f = "x"
        refs += kb.fields.get(f, [])
        what.append(f"player `{m.group(1)}`")
    u = re.search(r"\bunit\b.*?\b(present|absent|count|seen|dead)\b", s)
    if u:
        t = re.search(r"\but (\d)", s)
        refs += PRESENCE_BY_TYPE.get(int(t.group(1)) if t else 1, [])
        if u.group(1) == "dead":
            refs += kb.fields.get("hp", [])
        what.append(f"unit {u.group(1)}")
    kind = re.search(r"\(([a-z-]+)\) f\d+", s)
    return refs, f"playthrough {kind.group(1) if kind else '?'}: " + (", ".join(what) or "no predicate")


ROUTERS = {"state": route_state, "packets": route_packets, "rng": route_rng, "draws": route_draws,
           "playthrough": route_playthrough}


def route(kb, ch, s):
    refs, what = ROUTERS.get(ch, lambda kb, s: ([], "unknown channel"))(kb, s)
    seen, specs = set(), []
    for spec, sec in refs:
        if (spec, sec) in seen or kb.spec_lines(spec) is None:
            continue
        seen.add((spec, sec))
        specs.append({"spec": spec, "section": sec})
    mods = []
    for sp in specs:
        for m in kb.modules_for(sp["spec"], sp["section"]):
            if m not in mods:
                mods.append(m)
    own = kb.owner_for([s["spec"] for s in specs] + mods)
    return {"channel": ch, "line": s, "what": what, "specs": specs, "modules": mods[:6],
            "owner": own and {"area": own["area"], "session": own["session"], "branch": own["branch"]},
            "routed": bool(specs) and own is not None}


def fmt(r):
    L = [f"[{r['channel']}] {r['what']}", f"  from: {r['line'][:200]}"]
    if r["specs"]:
        for s in r["specs"]:
            L.append(f"  spec:   {s['spec']}{' ' + s['section'] if s['section'] else ''}")
    else:
        L.append("  spec:   unrouted")
    for m in r["modules"]:
        L.append(f"  module: {m}")
    o = r["owner"]
    L.append(f"  owner:  {o['area']} {o['session']} {o['branch']}" if o else "  owner:  unrouted")
    return "\n".join(L)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("report", nargs="?", default="-")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        return selftest()
    text = sys.stdin.read() if a.report == "-" else open(a.report, encoding="utf-8").read()
    kb = Kb()
    fs = findings(text)
    routes = [route(kb, ch, s) for ch, s in fs]
    if a.json:
        print(json.dumps({"format": "route-1", "routes": routes}, indent=1))
    elif not routes:
        print("unrouted: no first-difference line recognised")
    else:
        print("\n".join(fmt(r) for r in routes))
    return 0 if routes and all(r["routed"] for r in routes) else 1


def selftest():
    kb = Kb()
    ok = 0

    def chk(text, ch, spec_sub, what_sub=None):
        nonlocal ok
        fs = findings(text)
        assert fs and fs[0][0] == ch, (text, fs)
        r = route(kb, *fs[0])
        assert any(spec_sub in s["spec"] for s in r["specs"]), (text, r)
        if what_sub:
            assert what_sub in r["what"], (text, r["what"])
        ok += 1
        return r
    r = chk("FIRST DIVERGENCE: frame 12 monster 1:7 class 5, field x: 1.14d 5000 vs d2rs 5001",
            "state", "sim/path-placement.md", "`x`")
    assert r["modules"], r  # the path-placement spec has implementing modules
    chk("FIRST DIVERGENCE: frame 3 game, field seed: 1.14d [1, 2] vs d2rs [1, 3]", "state", "sim/rng.md")
    chk("FIRST DIVERGENCE: frame 40 monster 1:9 class 5/5, field (unit): 1.14d present vs d2rs missing",
        "state", "monsters/init.md")
    chk("FIRST DIVERGENCE: frame 4 player 0:1 class 0, field q: 1.14d [] vs d2rs [[0, 1]]", "state", "world/quests.md")
    chk('{"format": "diff-summary-1", "channel": "packets", "first": {"frame": 5, "text": '
        '"frame 5 stream s2c #2 bytes[3]: 1.14d 1 vs d2rs 2 (id 0x01)"}}', "packets", "", "GameFlags")
    chk("frame 9, monster 1:4, draw #3, field site: 1.14d site 0x1 vs d2rs site 0x2", "rng", "sim/rng.md")
    chk('{"format": "playthrough-result-1", "acts": [{"act": 1, "first_blocker": {"milestone": "den", '
        '"kind": "stuck", "frame": 900, "evidence": "at deadline f900: quest 1 0 set: no quest record"}}]}',
        "playthrough", "world/quests.md")
    chk("enter-cold-plains (stuck) f600: at deadline f600: player lv == 3: lv=2", "playthrough", "drlg/rooms.md")
    assert findings("nothing to see") == []
    assert route(kb, "state", "FIRST DIVERGENCE: frame 1 game, field zz: a vs b")["specs"] == []
    ok += 1
    print(f"route selftest: {ok} checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
