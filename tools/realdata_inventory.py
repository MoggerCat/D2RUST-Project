#!/usr/bin/env python3
"""Inventory of real-data tests: every #[ignore] test and every test fn that
reads D2_GAME_DIR (directly or through a helper in the same file).

  python3 tools/realdata_inventory.py > docs/handoff/realdata-tests.tsv
  python3 tools/realdata_inventory.py --crates     # crate names that have ignored tests

  python3 tools/realdata_inventory.py --skip-names [--no-recordings]
                                                   # gpu / dump tests (+ recording replays) the gate skips

Columns: crate, file, test, kind (ignored|env), needs (data|gpu|dump|recording; no ignored test opens a window),
files (heuristic from names in the test body), what (doc line / ignore reason).
"""
import os, re, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")
FILE_RE = re.compile(r"[\w\\/.\-*]+\.(?:mpq|bin|txt|tbl|ds1|dt1|dc6|dcc|cof|wav|dat|pl2|d2s|d2|toml|d2stack|exe|dll)\b", re.I)
EXTRA = [("excel", "excel/*.bin|txt"), ("extracted", "extracted/ (private repo)"), ("D2_TABLES_DUMP", "local memory dump D2_TABLES_DUMP"),
         ("D2_SAVE_DIR", "D2_SAVE_DIR saves")]


def crate_of(path):
    p = path.split(os.sep)
    return p[1] if p[0] == "crates" else p[1] if p[0] == "tools" else p[0]


def scan(path):
    lines = open(path, encoding="utf-8", errors="replace").read().split("\n")
    # fn spans: start line -> end line by brace depth
    fns = []
    i = 0
    while i < len(lines):
        m = FN.match(lines[i])
        if m:
            depth, j, seen = 0, i, False
            while j < len(lines):
                depth += lines[j].count("{") - lines[j].count("}")
                if "{" in lines[j]:
                    seen = True
                if seen and depth <= 0:
                    break
                j += 1
            fns.append((m.group(1), i, j))
        i += 1
    file_reads_env = {n for n, a, b in fns if any("D2_GAME_DIR" in l for l in lines[a:b + 1])}
    rows = []
    for name, a, b in fns:
        # attributes / docs directly above
        k, attrs, docs = a - 1, [], []
        while k >= 0 and (lines[k].strip().startswith(("#[", "///", "//")) or lines[k].strip() == ""):
            s = lines[k].strip()
            if s.startswith("#["):
                attrs.append(s)
            elif s.startswith("///"):
                docs.append(s[3:].strip())
            elif s == "" and k < a - 1 and not (attrs or docs):
                break
            k -= 1
            if a - k > 40:
                break
        ign = [x for x in attrs if x.startswith("#[ignore")]
        body = "\n".join(lines[a:b + 1])
        calls_env = any(re.search(r"\b%s\s*\(" % re.escape(h), body) for h in file_reads_env if h != name)
        reads_env = name in file_reads_env or calls_env
        if not ign and not (reads_env and any(x.startswith("#[test]") for x in attrs)):
            continue
        reason = ""
        if ign:
            m = re.search(r'"([^"]*)"', ign[0])
            reason = m.group(1) if m else ""
        what = " ".join(reversed(docs))[:160] or reason
        need = ["data"]
        if "GPU" in reason:
            need = ["gpu"]
        elif "D2_TABLES_DUMP" in body + reason:
            need = ["dump"]
        elif "traces/raw" in " ".join(docs) + body + reason:
            need = ["recording"]
        files = sorted(set(x.replace("\\", "/") for x in FILE_RE.findall(body)))[:6]
        for key, label in EXTRA:
            if key in body and label not in files:
                files.append(label)
        rows.append((crate_of(os.path.relpath(path, ROOT)), os.path.relpath(path, ROOT).replace(os.sep, "/"), name,
                     "ignored" if ign else "env", "+".join(need), ",".join(files) or "install (D2_GAME_DIR)", what.replace("\t", " ")))
    return rows


def main():
    rows = []
    for top in ("crates", "tools"):
        for d, _, fs in os.walk(os.path.join(ROOT, top)):
            if "target" in d.split(os.sep):
                continue
            for f in fs:
                if f.endswith(".rs"):
                    rows += scan(os.path.join(d, f))
    rows.sort()
    if "--skip-names" in sys.argv:
        skip = ("gpu", "dump", "recording") if "--no-recordings" in sys.argv else ("gpu", "dump")
        print("\n".join(sorted({r[2] for r in rows if r[4] in skip})))
        return
    if "--crates" in sys.argv:
        print("\n".join(sorted({r[0] for r in rows if r[3] == "ignored"})))
        return
    print("crate\tfile\ttest\tkind\tneeds\tfiles\twhat")
    for r in rows:
        print("\t".join(r))


main()
