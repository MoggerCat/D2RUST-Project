"""Merge-conflict markers left in tracked text (docs/HANDOFF.md §8 lesson).

An integration merge once committed conflict markers into a spec; the
spec index and coverage checks do not see them. This check fails on any
line of specs/, docs/ or crates/ that is a conflict marker:

    ^<<<<<<<   ^|||||||   ^=======$   ^>>>>>>>

Usage (from the repo root; Python 3.8+, standard library only):
    py tools/conflict_markers.py             # exit 1 on a marker (CI, gate)
    py tools/conflict_markers.py --selftest  # the check catches each marker
"""

import re
import sys
import tempfile
from pathlib import Path

ROOTS = ("specs", "docs", "crates")
MARKER = re.compile(r"^(<<<<<<<|\|\|\|\|\|\|\||=======$|>>>>>>>)")
# Binary or generated build output is skipped; everything else is text.
SKIP_DIRS = {"target", ".git"}


def scan(base: Path):
    """Yields (path, line number, line) for every marker line."""
    for root in ROOTS:
        top = base / root
        if not top.is_dir():
            continue
        for p in sorted(top.rglob("*")):
            if not p.is_file() or SKIP_DIRS.intersection(p.parts):
                continue
            try:
                text = p.read_text(encoding="utf-8")
            except (UnicodeDecodeError, OSError):
                continue
            for n, line in enumerate(text.splitlines(), 1):
                if MARKER.match(line):
                    yield p.relative_to(base), n, line


def check(base: Path) -> int:
    found = list(scan(base))
    for path, n, line in found:
        print(f"error: {path.as_posix()}:{n}: conflict marker {line[:7]!r}")
    print(f"conflict markers: {len(found)}")
    return 1 if found else 0


def selftest() -> int:
    clean = "a\n======\n=======x\n <<<<<<< indented\nb\n"
    cases = ["<<<<<<< HEAD\n", "||||||| base\n", "=======\n", ">>>>>>> other\n"]
    with tempfile.TemporaryDirectory() as d:
        base = Path(d)
        for root in ROOTS:
            (base / root).mkdir()
        f = base / "specs" / "x.md"
        f.write_text(clean, encoding="utf-8")
        if list(scan(base)):
            print("selftest FAIL: a clean file reported")
            return 1
        for root in ROOTS:
            for c in cases:
                g = base / root / "y.rs"
                g.write_text(clean + c, encoding="utf-8")
                hits = list(scan(base))
                if len(hits) != 1 or hits[0][1] != 6:
                    print(f"selftest FAIL: {c.strip()!r} in {root} not caught once: {hits}")
                    return 1
                g.unlink()
    print("selftest ok")
    return 0


def main(argv) -> int:
    if "--selftest" in argv:
        return selftest()
    return check(Path(__file__).resolve().parent.parent)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
