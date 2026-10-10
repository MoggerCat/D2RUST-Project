#!/usr/bin/env python3
"""Resolve a merge conflict in docs/handoff/checks-status.md row by row.

Three-way by (check, channel): start from our side (stage 2); every row the
branch (stage 3) changed or added relative to the merge base (stage 1) wins.
Rows the branch deleted stay (a verdict is never dropped by a merge).
Our own code."""
import subprocess

P = "docs/handoff/checks-status.md"


def stage(n):
    r = subprocess.run(["git", "show", f":{n}:{P}"], capture_output=True, text=True)
    return r.stdout.split("\n") if r.returncode == 0 else []


def key(l):
    p = [x.strip() for x in l.strip().strip("|").split("|")]
    return (p[0], p[1]) if l.startswith("| ") and len(p) == 6 and p[0] not in ("check", "---", "") and not p[0].startswith("-") else None


def problems(lines, label):
    """Conflict-marker lines and duplicate check+channel keys; both are errors."""
    errs, seen = [], {}
    for n, l in enumerate(lines, 1):
        if l.startswith(("<<<<<<<", "=======", ">>>>>>>")):
            errs.append(f"{label}:{n}: merge-conflict marker")
        k = key(l)
        if k and k in seen:
            errs.append(f"{label}:{n}: duplicate check+channel {k[0]} / {k[1]} (first at line {seen[k]})")
        elif k:
            seen[k] = n
    return errs


base, ours, theirs = stage(1), stage(2), stage(3)
bad = problems(ours, "ours") + problems(theirs, "theirs")
if bad:
    raise SystemExit("statusres: refusing, an input is already broken:\n" + "\n".join(bad[:20]))
b = {key(l): l for l in base if key(l)}
t = [(key(l), l) for l in theirs if key(l)]
changed = {k: l for k, l in t if b.get(k) != l}
out, done, last = [], set(), 0
for l in ours:
    k = key(l)
    if k and k in changed:
        out.append(changed[k]); done.add(k)
    else:
        out.append(l)
    if k:
        last = len(out)
rest = [l for k, l in t if k in changed and k not in done]
out[last:last] = rest
bad = problems(out, "result")
if bad:
    raise SystemExit("statusres: refusing, the result is broken:\n" + "\n".join(bad[:20]))
open(P, "w").write("\n".join(out))
print(f"checks-status: {len(done)} rows from branch, {len(rest)} appended")
