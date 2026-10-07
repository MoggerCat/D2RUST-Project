"""Spec rule coverage: which checks prove which spec rules (docs/COVERAGE.md).

Rules are read from the specs (`specs/**/*.md`), claims from `Covers:`
comments next to tests and checks (`crates/**/*.rs`, `tools/**/*.rs`,
`tools/**/*.py`). A claim naming a rule that does not exist is an error.

Usage (from the repo root; Python 3.8+, standard library only):
    py tools/coverage.py                # per-spec and total table, uncovered rules
    py tools/coverage.py --summary      # table only
    py tools/coverage.py --rules SPEC   # list the rule IDs of one spec
    py tools/coverage.py --check        # exit 1 on a bad claim, rule ID or exemption (CI); never on low coverage
    py tools/coverage.py --selftest     # perturbation tests (METHODS M08)
"""

import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Top-level sections that hold no rules (docs/COVERAGE.md §1). Every other
# `##` section of a spec holds rules.
NOT_RULES = {
    "summary",
    "inputs",
    "outputs-state-changes",
    "constants-data-dependencies",
    "randomness",
    "test-vectors",
    "provenance",
    "open-questions",
    "example",
    "survey-1-14d-data",
    "observations-1-14d-install",
}
HEAD = re.compile(r"^(#{2,4}) (.+?)\s*$")
NUMBERED = re.compile(r"^((?:\d+\.)*\d+)\.?\s")  # "5.2 Title", "1. Title"
ITEM = re.compile(r"^(\d+)\. ")
CLAIM = re.compile(r"^\s*(?://|#) Covers:(.*)$")
RULE_REF = re.compile(
    r"^§[0-9a-z][0-9a-z.\-]*(?: text|(?: l[2-9][0-9]*)? r(?:0|[1-9][0-9]*)|(?: t[2-9][0-9]*)? row[1-9][0-9]*)?$"
)
ROWS_MARK = re.compile(r"^<!-- rows -->\s*$")  # opt-in marker directly above a table  # r0: a list numbered from 0
TIERS = ("unit", "game", "trace")


def slug(title):
    return re.sub(r"[^0-9a-z]+", "-", title.lower()).strip("-")


def anchor(title):
    m = NUMBERED.match(title)
    return m.group(1) if m else slug(title)


class Spec:
    """Rule IDs of one spec (docs/COVERAGE.md §1).

    `rules` maps every claimable ID (sections and items) to its line, its
    ancestors and the units it covers; `leaves` lists the counted units in
    spec order. A section without items or subsections is one unit; a
    section with them has the unit `§x text` for its own text (lines at
    column 0 outside its items), if it has any.
    """

    def __init__(self, path, text):
        self.path = path
        self.rules = {}  # id -> {"line", "anc", "own", "leaves"}
        self.leaves = []
        self.errors = []
        lines = text.replace("\r\n", "\n").split("\n")
        stack = []  # [(level, id)]; id None inside a section without rules
        lists = {}  # section id -> (list number, items seen)
        tables = {}  # section id -> row tables seen
        marked = None  # line of a `<!-- rows -->` marker awaiting its table
        tbl = None  # [section, table number, lines seen] inside a row table
        fence = False
        for no, line in enumerate(lines, 1):
            sec = stack[-1][1] if stack else None
            if line.startswith("```"):
                fence = not fence
                if sec:
                    self.rules[sec]["own"] = True
                continue
            if fence:
                continue
            h = HEAD.match(line)
            if h:
                if marked:
                    self.errors.append(f"{self.path}:{marked}: row marker is not directly above a table")
                    marked = None
                tbl = None
                level, title = len(h.group(1)), h.group(2)
                while stack and stack[-1][0] >= level:
                    stack.pop()
                if level == 2 and slug(title) in NOT_RULES:
                    stack = [(2, None)]
                    continue
                if stack and stack[-1][1] is None:
                    continue
                rid = "§" + anchor(title)
                self._add(rid, no, [s[1] for s in stack])
                stack.append((level, rid))
                continue
            if not sec:
                continue
            if ROWS_MARK.match(line):
                if marked:
                    self.errors.append(f"{self.path}:{marked}: row marker is not directly above a table")
                marked = no
                tbl = None
                continue
            if tbl is not None and line.startswith("|"):
                tbl[2] += 1
                if tbl[2] > 2:  # line 1 is the header, line 2 the separator
                    k, n = tbl[1], tbl[2] - 2
                    self._add(f"{sec} row{n}" if k == 1 else f"{sec} t{k} row{n}", no, [s[1] for s in stack])
                continue
            tbl = None
            if marked:
                if line.startswith("|"):
                    tables[sec] = tables.get(sec, 0) + 1
                    tbl = [sec, tables[sec], 1]
                    marked = None
                    continue
                self.errors.append(f"{self.path}:{marked}: row marker is not directly above a table")
                marked = None
            m = ITEM.match(line)
            if m:
                n = int(m.group(1))
                k, seen = lists.get(sec, (1, 0))
                if n == 1 and seen:
                    k += 1  # a new list in the same section
                lists[sec] = (k, seen + 1)
                lst = f" l{k}" if k > 1 else ""
                self._add(f"{sec}{lst} r{n}", no, [s[1] for s in stack])
            elif line.strip() and not line[0].isspace():
                self.rules[sec]["own"] = True
        if marked:
            self.errors.append(f"{self.path}:{marked}: row marker is not directly above a table")
        parents = {p for r in self.rules.values() for p in r["anc"]}
        for rid, r in list(self.rules.items()):
            if rid not in parents:
                unit = rid
            elif r["own"]:
                unit = rid + " text"  # the section's own text beside its items
                self.rules[unit] = {"line": r["line"], "anc": r["anc"] + [rid], "own": True, "leaves": []}
            else:
                continue
            self.leaves.append(unit)
            for a in self.rules[unit]["anc"] + [unit]:
                self.rules[a]["leaves"].append(unit)

    def _add(self, rid, no, anc):
        if rid in self.rules:
            self.errors.append(
                f"{self.path}:{no}: rule id {rid} repeats line {self.rules[rid]['line']}"
                " (renumber the list or the heading)"
            )
            return
        self.rules[rid] = {"line": no, "anc": anc, "own": False, "leaves": []}


def load_specs(root):
    specs = {}
    for p in sorted((root / "specs").rglob("*.md")):
        if p.name in ("README.md", "_TEMPLATE.md"):
            continue
        rel = p.relative_to(root).as_posix()
        specs[rel] = Spec(rel, p.read_text(encoding="utf-8"))
    return specs


def claim_files(root):
    for pat in ("crates/**/*.rs", "tools/**/*.rs", "tools/**/*.py"):
        for p in sorted(root.glob(pat)):
            if p.name == "coverage.py" or "target" in p.relative_to(root).parts:
                continue
            yield p


def tier_of(rel, lines, i):
    """Tier of the claim on line i (0-based), or an error string.

    The claim belongs to the next `fn` / `def`; only comments, attributes
    and blank lines may sit between them.
    """
    attrs = []
    for j in range(i + 1, len(lines)):
        s = lines[j].strip()
        if not s or s.startswith("//") or s.startswith("#"):
            if s.startswith("#[") or s.startswith("#!["):
                attrs.append(s)
            continue
        if not re.match(r"^(pub(\([a-z]+\))? )?(async )?(fn|def) ", s):
            return None, f"claim must sit just above a test or check function, found {s[:40]!r}"
        test = any(a.startswith("#[test") or "::test" in a for a in attrs)
        ignored = any(a.startswith("#[ignore") for a in attrs)
        if rel.startswith("crates/conformance/") or rel.startswith("tools/trace-recorder/"):
            return "trace", None
        if rel.endswith(".py"):
            return None, "Python claims are allowed only in trace checkers (tools/trace-recorder/)"
        if test:
            return ("game" if ignored else "unit"), None
        if rel.startswith(("tools/data-tool/", "tools/mpq-tool/")):
            return "game", None
        return None, "claim on a function that is neither a test nor a game-file check"
    return None, "claim at end of file"


def parse_claim(body):
    """[(spec, rule)] from 'specs/a.md §1 r2, §3; specs/b.md §4', or raise ValueError."""
    out = []
    for group in body.split(";"):
        group = group.strip()
        path, _, refs = group.partition(" ")
        if not path.startswith("specs/") or not path.endswith(".md") or not refs.strip():
            raise ValueError(f"expected 'specs/<file>.md §<rule>[, §<rule>]', got {group!r}")
        for ref in refs.split(","):
            ref = " ".join(ref.split())
            if not RULE_REF.match(ref):
                raise ValueError(f"malformed rule {ref!r} (want §<anchor>, §<anchor> text, §<anchor>[ l<K>] r<N> or §<anchor>[ t<K>] row<N>)")
            out.append((path, ref))
    return out


def scan(root, specs, files=None):
    """(claims [(file, line, tier, spec, rule)], errors)."""
    claims, errors = [], []
    for p in files if files is not None else claim_files(root):
        rel = p.relative_to(root).as_posix()
        lines = p.read_text(encoding="utf-8").split("\n")
        for i, line in enumerate(lines):
            m = CLAIM.match(line)
            if not m:
                continue
            where = f"{rel}:{i + 1}"
            try:
                refs = parse_claim(m.group(1))
            except ValueError as e:
                errors.append(f"{where}: {e}")
                continue
            tier, err = tier_of(rel, lines, i)
            if err:
                errors.append(f"{where}: {err}")
                continue
            for spec, rule in refs:
                if spec not in specs:
                    errors.append(f"{where}: dangling claim: no spec {spec}")
                elif rule not in specs[spec].rules:
                    errors.append(f"{where}: dangling claim: {spec} has no rule {rule}")
                else:
                    claims.append((rel, i + 1, tier, spec, rule))
    return claims, errors


def coverage(specs, claims):
    """{spec: {leaf: set(tiers)}}"""
    cov = {s: {leaf: set() for leaf in sp.leaves} for s, sp in specs.items()}
    for _, _, tier, spec, rule in claims:
        for leaf in specs[spec].rules[rule]["leaves"]:
            cov[spec][leaf].add(tier)
    return cov


EXEMPT_FILE = "docs/coverage-exempt.tsv"


def load_exempt(root, specs):
    """({(spec, leaf): reason}, errors) from docs/coverage-exempt.tsv (docs/COVERAGE.md §5).

    Columns: spec, rule (any claimable ID: a section exempts every unit
    inside it; `*` is the whole spec), reason. Lines starting with `#` and blank lines are skipped.
    """
    exempt, errors = {}, []
    path = root / EXEMPT_FILE
    if not path.exists():
        return exempt, errors
    for no, line in enumerate(path.read_text(encoding="utf-8").split("\n"), 1):
        if not line.strip() or line.startswith("#"):
            continue
        where = f"{EXEMPT_FILE}:{no}: "
        cols = line.split("\t")
        if len(cols) != 3 or not all(c.strip() for c in cols):
            errors.append(where + "want three tab-separated columns: spec, rule, reason")
            continue
        spec, rule, reason = (c.strip() for c in cols)
        rule = " ".join(rule.split())
        if spec not in specs:
            errors.append(where + f"exempt entry names no spec {spec}")
        elif rule == "*":  # the whole spec
            for leaf in specs[spec].leaves:
                exempt.setdefault((spec, leaf), reason)
        elif not RULE_REF.match(rule):
            errors.append(where + f"malformed rule {rule!r}")
        elif rule not in specs[spec].rules:
            errors.append(where + f"exempt entry names a rule that does not exist: {spec} has no rule {rule}")
        else:
            for leaf in specs[spec].rules[rule]["leaves"]:
                exempt.setdefault((spec, leaf), reason)
    return exempt, errors


def check_exempt(specs, exempt, claims):
    """Errors for every exempt unit that is also claimed: it must be one or the other."""
    errors = []
    for f, line, _, spec, rule in claims:
        both = [u for u in specs[spec].rules[rule]["leaves"] if (spec, u) in exempt]
        if both:
            shown = ", ".join(both[:3]) + (f" (+{len(both) - 3} more)" if len(both) > 3 else "")
            errors.append(f"{f}:{line}: {spec} {rule} claims exempt rule(s) {shown}: remove the claim or the exemption")
    return errors


def pct(n, d):
    return f"{100 * n / d:5.1f}%" if d else "    -"


def report(specs, claims, errors, summary, exempt):
    cov = coverage(specs, claims)
    head = ("spec", "rules", "exempt", "unit", "game", "trace", "verified", "any")
    rows, tot = [], [0] * 7
    for s, all_leaves in cov.items():
        leaves = {k: v for k, v in all_leaves.items() if (s, k) not in exempt}
        counts = [len(leaves), len(all_leaves) - len(leaves)]
        counts += [sum(t in v for v in leaves.values()) for t in TIERS]
        counts.append(sum(bool(v & {"game", "trace"}) for v in leaves.values()))
        counts.append(sum(bool(v) for v in leaves.values()))
        tot = [a + b for a, b in zip(tot, counts)]
        rows.append((s, *counts))
    w = max(len(r[0]) for r in rows)
    print(f"{head[0]:<{w}}  {head[1]:>7}  {head[2]:>6}  " + "  ".join(f"{h:>14}" for h in head[3:]))
    for r in rows + [("total", *tot)]:
        cells = [f"{c:>6} {pct(c, r[1])}" for c in r[3:]]
        print(f"{r[0]:<{w}}  {r[1]:>7}  {r[2]:>6}  " + "  ".join(cells))
    print(
        f"\n{len(claims)} claims. Claimable rules {tot[0]} (+{tot[1]} exempt, {EXEMPT_FILE}); covered by any tier"
        f" {tot[6]}/{tot[0]} ({pct(tot[6], tot[0]).strip()}). verified = game-file or trace checks against 1.14d"
        f" (CLAUDE.md hard rule 10): {tot[5]}/{tot[0]} rules ({pct(tot[5], tot[0]).strip()})."
    )
    if not summary:
        print("\nUncovered rules:")
        for s, leaves in cov.items():
            miss = [leaf for leaf, v in leaves.items() if not v and (s, leaf) not in exempt]
            if miss:
                print(f"{s} ({len(miss)}): " + ", ".join(miss))
    for e in errors:
        print("error: " + e)


def selftest():
    """Perturbation tests: each changed input is reported exactly (M08)."""
    spec = (
        "# S\n\n## Summary\n\n1. not a rule\n\n## Rules\n\n### 1. One\n\n1. a\n2. b\n\n"
        "### 2. Two\n\ntext\n\n### Header (4 bytes)\n\n| a | b |\n\n1. x\n   1. nested\n\n"
        "Then:\n\n1. y\n\n## Open questions\n\n1. q\n"
    )
    leaves = ["§1 r1", "§1 r2", "§2", "§header-4-bytes text", "§header-4-bytes r1", "§header-4-bytes l2 r1"]
    test = (
        "#[cfg(test)]\nmod tests {{\n    // Covers: specs/x/s.md {rule}\n    #[test]\n"
        "    fn t() {{}}\n\n    // Covers: specs/x/s.md §2\n    #[test]\n    #[ignore]\n    fn g() {{}}\n}}\n"
    )
    bad = "crates/c/src/lib.rs:3: "
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        (root / "specs/x").mkdir(parents=True)
        (root / "crates/c/src").mkdir(parents=True)
        (root / "specs/x/s.md").write_text(spec, encoding="utf-8")
        specs = load_specs(root)
        sp = specs["specs/x/s.md"]
        assert sp.leaves == leaves, sp.leaves
        assert not sp.errors, sp.errors
        src = root / "crates/c/src/lib.rs"
        cases = {
            "§1": [],
            "§1 r2": [],
            "§header-4-bytes l2 r1; specs/x/s.md §header-4-bytes text": [],
            "§1 r3": [bad + "dangling claim: specs/x/s.md has no rule §1 r3"],
            "§3": [bad + "dangling claim: specs/x/s.md has no rule §3"],
            "§1 r1, §summary": [bad + "dangling claim: specs/x/s.md has no rule §summary"],
            "§open-questions r1": [bad + "dangling claim: specs/x/s.md has no rule §open-questions r1"],
            "§2 text": [bad + "dangling claim: specs/x/s.md has no rule §2 text"],
            "§header-4-bytes l3 r1": [bad + "dangling claim: specs/x/s.md has no rule §header-4-bytes l3 r1"],
            "1 r1": [bad + "malformed rule '1 r1' (want §<anchor>, §<anchor> text, §<anchor>[ l<K>] r<N> or §<anchor>[ t<K>] row<N>)"],
        }
        for rule, want in cases.items():
            src.write_text(test.format(rule=rule), encoding="utf-8")
            claims, errors = scan(root, specs, [src])
            assert errors == want, (rule, errors)
        # A list numbered from 0: item 0 is §<s> r0 (claimable), the "1." after it
        # starts list 2 (docs/COVERAGE.md §1); r00 / r01 stay malformed.
        (root / "specs/x/z.md").write_text("# Z\n\n## Rules\n\n### 1. Zero\n\n0. a\n1. b\n", encoding="utf-8")
        specs = load_specs(root)
        assert specs["specs/x/z.md"].leaves == ["§1 r0", "§1 l2 r1"], specs["specs/x/z.md"].leaves
        zero = {
            "§1 r0": [],
            "§1 r0, §1 l2 r1": [],
            "§1 r00": [bad + "malformed rule '§1 r00' (want §<anchor>, §<anchor> text, §<anchor>[ l<K>] r<N> or §<anchor>[ t<K>] row<N>)"],
            "§1 r01": [bad + "malformed rule '§1 r01' (want §<anchor>, §<anchor> text, §<anchor>[ l<K>] r<N> or §<anchor>[ t<K>] row<N>)"],
            "§1 l2 r0": [bad + "dangling claim: specs/x/z.md has no rule §1 l2 r0"],
        }
        for rule, want in zero.items():
            src.write_text(test.format(rule=rule).replace("s.md", "z.md", 1), encoding="utf-8")
            assert scan(root, specs, [src])[1] == want, (rule, scan(root, specs, [src])[1])
        src.write_text(test.format(rule="§1").replace("s.md §1", "t.md §1", 1), encoding="utf-8")
        assert scan(root, specs, [src])[1] == [bad + "dangling claim: no spec specs/x/t.md"]
        src.write_text(test.format(rule="§1").replace("#[test]\n    fn t", "fn t", 1), encoding="utf-8")
        assert scan(root, specs, [src])[1] == [
            bad + "claim on a function that is neither a test nor a game-file check"
        ]
        src.write_text(test.format(rule="§header-4-bytes"), encoding="utf-8")
        claims, _ = scan(root, specs, [src])
        cov = coverage(specs, claims)["specs/x/s.md"]
        want = {u: {"unit"} for u in leaves[3:]}
        want.update({"§1 r1": set(), "§1 r2": set(), "§2": {"game"}})
        assert cov == want, cov
        (root / "specs/x/s.md").write_text(spec + "\n## Rules again\n\n### 1. Again\n", encoding="utf-8")
        assert load_specs(root)["specs/x/s.md"].errors == [
            "specs/x/s.md:35: rule id §1 repeats line 9 (renumber the list or the heading)"
        ], load_specs(root)["specs/x/s.md"].errors
        # Table rows (opt-in `<!-- rows -->` marker): a plain table stays text,
        # a marked one gives `row<N>` units (`t<K> row<N>` from the 2nd marked
        # table); the section id still covers them all.
        rows = (
            "# R\n\n## Rules\n\n### 1. Plain\n\n| a |\n|---|\n| x |\n\n"
            "### 2. Marked\n\n<!-- rows -->\n| a | b |\n|---|---|\n| x | y |\n| z | w |\n\nafter\n\n"
            "<!-- rows -->\n| c |\n|---|\n| q |\n"
        )
        (root / "specs/x/r.md").write_text(rows, encoding="utf-8")
        specs = load_specs(root)
        r = specs["specs/x/r.md"]
        assert not r.errors, r.errors
        assert r.leaves == ["§1", "§2 text", "§2 row1", "§2 row2", "§2 t2 row1"], r.leaves
        assert r.rules["§2"]["leaves"] == r.leaves[1:]
        for rule, want in {
            "§2 row2": [],
            "§2 t2 row1": [],
            "§2": [],
            "§2 row3": [bad + "dangling claim: specs/x/r.md has no rule §2 row3"],
            "§2 t3 row1": [bad + "dangling claim: specs/x/r.md has no rule §2 t3 row1"],
            "§1 row1": [bad + "dangling claim: specs/x/r.md has no rule §1 row1"],
            "§2 row0": [bad + "malformed rule '§2 row0' (want §<anchor>, §<anchor> text, §<anchor>[ l<K>] r<N> or §<anchor>[ t<K>] row<N>)"],
        }.items():
            src.write_text(test.format(rule=rule).replace("s.md", "r.md", 1), encoding="utf-8")
            assert scan(root, specs, [src])[1] == want, (rule, scan(root, specs, [src])[1])
        (root / "specs/x/r.md").write_text("# R\n\n## Rules\n\n### 1. A\n\n<!-- rows -->\ntext\n", encoding="utf-8")
        assert load_specs(root)["specs/x/r.md"].errors == [
            "specs/x/r.md:7: row marker is not directly above a table"
        ]
    # Exemptions (docs/COVERAGE.md §5): a missing rule, a bad row and a rule
    # that is both exempt and claimed are each reported exactly; a good list
    # removes the units from the denominator.
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        (root / "specs/x").mkdir(parents=True)
        (root / "crates/c/src").mkdir(parents=True)
        (root / "docs").mkdir()
        (root / "specs/x/s.md").write_text(spec, encoding="utf-8")
        specs = load_specs(root)
        ex = root / EXEMPT_FILE
        w = EXEMPT_FILE + ":"
        cases = {
            "specs/x/s.md\t§1\twhy\n": [],
            "# c\n\nspecs/x/s.md\t§header-4-bytes l2 r1\twhy\n": [],
            "specs/x/s.md\t§9\twhy\n": [w + "1: exempt entry names a rule that does not exist: specs/x/s.md has no rule §9"],
            "specs/x/s.md\t§1 r3\twhy\n": [w + "1: exempt entry names a rule that does not exist: specs/x/s.md has no rule §1 r3"],
            "specs/x/t.md\t§1\twhy\n": [w + "1: exempt entry names no spec specs/x/t.md"],
            "specs/x/s.md\t*\twhy\n": [],
            "specs/x/s.md\t§1\n": [w + "1: want three tab-separated columns: spec, rule, reason"],
            "specs/x/s.md\t1 r1\twhy\n": [w + "1: malformed rule '1 r1'"],
        }
        for text, want in cases.items():
            ex.write_text(text, encoding="utf-8")
            assert load_exempt(root, specs)[1] == want, (text, load_exempt(root, specs)[1])
        ex.write_text("specs/x/s.md\t§1\twhy\n", encoding="utf-8")
        exempt, _ = load_exempt(root, specs)
        assert sorted(exempt) == [("specs/x/s.md", "§1 r1"), ("specs/x/s.md", "§1 r2")], exempt
        ex.write_text("specs/x/s.md\t*\twhy\n", encoding="utf-8")
        assert len(load_exempt(root, specs)[0]) == len(specs["specs/x/s.md"].leaves)
        ex.write_text("specs/x/s.md\t§1\twhy\n", encoding="utf-8")
        src = root / "crates/c/src/lib.rs"
        for rule, want in {
            "§2": [],
            "§1 r2": ["crates/c/src/lib.rs:3: specs/x/s.md §1 r2 claims exempt rule(s) §1 r2: remove the claim or the exemption"],
            "§1": ["crates/c/src/lib.rs:3: specs/x/s.md §1 claims exempt rule(s) §1 r1, §1 r2: remove the claim or the exemption"],
        }.items():
            src.write_text(test.format(rule=rule), encoding="utf-8")
            claims, errs = scan(root, specs, [src])
            assert not errs, errs
            assert check_exempt(specs, exempt, claims) == want, (rule, check_exempt(specs, exempt, claims))
    # The real repository: rename one claimed rule; exactly that claim is reported.
    specs = load_specs(ROOT)
    claims, errors = scan(ROOT, specs)
    assert not errors, errors
    assert claims, "no claims in the repository"
    f, line, _, spec, rule = claims[0]
    path = ROOT / f
    text = path.read_text(encoding="utf-8").split("\n")
    with tempfile.TemporaryDirectory() as d:
        copy = Path(d) / f
        copy.parent.mkdir(parents=True)
        text[line - 1] = text[line - 1].replace(rule, "§999 r1", 1)
        copy.write_text("\n".join(text), encoding="utf-8")
        _, errs = scan(Path(d), specs, [copy])
        want = f"{f}:{line}: dangling claim: {spec} has no rule §999 r1"
        assert want in errs and len([e for e in errs if "§999" in e]) == 1, errs
    print("coverage selftest: ok")


def main():
    args = sys.argv[1:]
    if "--selftest" in args:
        selftest()
        return
    specs = load_specs(ROOT)
    if "--rules" in args:
        sp = specs[args[args.index("--rules") + 1]]
        for rid in sp.leaves:
            print(f"{sp.rules[rid]['line']:>5}  {rid}")
        return
    claims, errors = scan(ROOT, specs)
    exempt, exempt_errors = load_exempt(ROOT, specs)
    errors = [e for sp in specs.values() for e in sp.errors] + errors + exempt_errors
    errors += check_exempt(specs, exempt, claims)
    if "--check" in args:
        for e in errors:
            print("error: " + e)
        print(f"coverage: {len(claims)} claims, {len(errors)} errors")
        sys.exit(1 if errors else 0)
    report(specs, claims, errors, "--summary" in args, exempt)
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
