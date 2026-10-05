"""Section index for specs, so agents can read only the sections they need.

Every spec in specs/ larger than LIMIT bytes gets an index block right after
its metadata list:

    <!-- index -->
    | § | Section | Lines |
    ...
    <!-- /index -->

Line ranges let a reader open just that part (e.g. Read offset/limit).

Usage (from the repo root; Python 3.8+, standard library only):
    py tools/spec_index.py          # rewrite index blocks
    py tools/spec_index.py --check  # exit 1 if any index is missing or stale (CI)
"""

import re
import sys
from pathlib import Path

LIMIT = 12_000
START, END = "<!-- index -->", "<!-- /index -->"
HEAD = re.compile(r"^(#{2,3}) (.+)$")


def strip_index(lines):
    if START not in lines:
        return lines
    a, b = lines.index(START), lines.index(END)
    rest = lines[:a] + lines[b + 1 :]
    # Drop the blank line the block left behind.
    if a < len(rest) and a > 0 and rest[a] == "" and rest[a - 1] == "":
        del rest[a]
    return rest


def build(lines):
    """Insert the index after the leading metadata list; returns new lines."""
    body = strip_index(lines)
    # Insertion point: first blank line after the first "- **" metadata line.
    try:
        meta = next(i for i, l in enumerate(body) if l.startswith("- **"))
        at = next(i for i in range(meta, len(body)) if body[i] == "")
    except StopIteration:
        at = next(i for i, l in enumerate(body) if l == "") if "" in body else len(body)
    n_heads = sum(1 for l in body if HEAD.match(l))
    block = ["", START, "| Section | Lines |", "|---|---|", *[""] * n_heads, END]
    out = body[:at] + block + body[at:]
    # Line numbers from the final layout (rows don't change the line count).
    heads = [(i, HEAD.match(l)) for i, l in enumerate(out) if HEAD.match(l)]
    for n, (i, h) in enumerate(heads):
        end = heads[n + 1][0] if n + 1 < len(heads) else len(out)
        title = ("  " if h.group(1) == "###" else "") + h.group(2)
        out[at + 4 + n] = f"| {title} | {i + 1}–{end} |"
    return out


def main():
    check = "--check" in sys.argv
    skip = {a[len("--skip="):] for a in sys.argv if a.startswith("--skip=")}
    root = Path(__file__).resolve().parent.parent / "specs"
    stale = []
    for path in sorted(root.rglob("*.md")):
        if path.name in ("README.md", "_TEMPLATE.md") or path.name in skip:
            continue
        text = path.read_text(encoding="utf-8").replace("\r\n", "\n")
        lines = text.split("\n")
        if len(text.encode("utf-8")) <= LIMIT and START not in lines:
            continue
        new = "\n".join(build(lines))
        if new != text:
            stale.append(path.relative_to(root.parent))
            if not check:
                path.write_text(new, encoding="utf-8", newline="\n")
    for p in stale:
        print(("stale index: " if check else "indexed: ") + str(p))
    if check and stale:
        print("run: py tools/spec_index.py")
        sys.exit(1)


if __name__ == "__main__":
    main()
