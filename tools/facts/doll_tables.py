"""Write the paper-doll static tables of the 1.14d Game.exe
(specs/ui/frontend-menus.md §F2.10 r7).

Reads the static data of game/Game.exe (PE sections map VAs to file
offsets; no game run) and writes two TSV fact files in the house style of
specs/tools/facts-render.md §1 (header line, column row, rows):

  facts/ui/doll-static-tokens.tsv  the 283 x 12-byte entries at 0x0072E1E0
                                   (index, code, hit class, item type),
                                   count [0x0072EF24]; read by 0x00504AF0
                                   (+4), 0x00504D60 (+0, +8), 0x00506000 (+8)
  facts/ui/doll-names.tsv          the name tables of the doll's files:
                                   class tokens 0x0072E050 (count
                                   [0x0072E04C]), mode tokens 0x0072E0B8
                                   ([0x0072E0B4]), component tokens
                                   0x0072E108 (16), weapon class names
                                   0x0072E15C ([0x0072E198]), the hit class
                                   name -> id pairs 0x0072EF68
                                   ([0x0072EFD0]) and the id -> weapon
                                   class numbers 0x0072EF30 (14)

Measurements only (indices, 3-letter codes, numbers, addresses); no code
bytes. Checks (exit 1 on failure): the count is 283; entries 1..3 are
lit/med/hvy with hit class 0 and type 1; every hit class is 0..8; the
(code, type) pairs equal the in-game reference table at 0x00744CA8
(8-byte entries, formats/d2s-appearance.md §1 r2.4) entry for entry; the
three counts are 25 / 20 / 15.

Usage: py tools/facts/doll_tables.py [--exe game/Game.exe] [--out facts/ui]

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import hashlib
import os
import struct
import sys

sys.dont_write_bytecode = True

TOOL = "doll_tables 0.1.0"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
GAME_SHA256 = "631066c1649c4ea9ffe48bf97e24c00bca1f7a6759c21150f1a79982589adaaf"

STATIC_VA, STATIC_COUNT_VA, STATIC_N = 0x0072E1E0, 0x0072EF24, 283
INGAME_REF_VA = 0x00744CA8
NAME_TABLES = [
    # (table, va, count va or None, fixed count)
    ("class_token", 0x0072E050, 0x0072E04C, 25),
    ("mode_token", 0x0072E0B8, 0x0072E0B4, 20),
    ("component_token", 0x0072E108, None, 16),
    ("weapon_class", 0x0072E15C, 0x0072E198, 15),
]
HIT_PAIRS_VA, HIT_PAIRS_COUNT_VA = 0x0072EF68, 0x0072EFD0
HIT_TO_CLASS_VA, HIT_TO_CLASS_N = 0x0072EF30, 14


class Image:
    def __init__(self, data):
        self.data = data
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        if data[pe:pe + 4] != b"PE\0\0":
            raise SystemExit("not a PE file")
        nsec = struct.unpack_from("<H", data, pe + 6)[0]
        opt = struct.unpack_from("<H", data, pe + 20)[0]
        self.base = struct.unpack_from("<I", data, pe + 24 + 28)[0]
        self.secs = []
        sh = pe + 24 + opt
        for i in range(nsec):
            _, vsize, va, rsize, raw = struct.unpack_from("<8sIIII", data, sh + 40 * i)
            self.secs.append((self.base + va, max(vsize, rsize), rsize, raw))

    def read(self, va, n):
        for start, vsize, rsize, raw in self.secs:
            if start <= va and va + n <= start + vsize:
                off = va - start
                if off + n > rsize:
                    raise SystemExit(f"0x{va:08x}+{n} lies in uninitialised data")
                return self.data[raw + off: raw + off + n]
        raise SystemExit(f"0x{va:08x} not in any section")

    def u32(self, va):
        return struct.unpack("<I", self.read(va, 4))[0]


def code(b):
    """A 4-byte space-padded code as text ('-' for the all-zero code)."""
    s = b.rstrip(b" \0")
    if not s:
        return "-"
    if not all(0x21 <= c < 0x7F for c in s):
        raise SystemExit(f"code bytes {b!r} are not printable")
    return s.decode("ascii")


def command_line():
    def q(s):
        return f'"{s}"' if (" " in s or not s) else s
    script = os.path.relpath(os.path.abspath(sys.argv[0]), REPO).replace("\\", "/")
    args = [a.replace(";", ",") for a in sys.argv[1:]]
    return " ".join(["py", q(script)] + [q(a) for a in args])


def write_tsv(path, header, columns, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(header + "\n")
        f.write("\t".join(columns) + "\n")
        for r in rows:
            f.write("\t".join(str(v) for v in r) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--exe", default=os.path.join(REPO, "game", "Game.exe"))
    ap.add_argument("--out", default=os.path.join(REPO, "facts", "ui"))
    a = ap.parse_args()
    with open(a.exe, "rb") as f:
        data = f.read()
    if hashlib.sha256(data).hexdigest() != GAME_SHA256:
        raise SystemExit("Game.exe is not the 1.14d build this tool was written for")
    img = Image(data)
    header = f"# facts v1; tool: {TOOL}; command: {command_line()}; game: 1.14d"
    fails = []

    n = img.u32(STATIC_COUNT_VA)
    if n != STATIC_N:
        fails.append(f"count [0x{STATIC_COUNT_VA:08X}] = {n}, not {STATIC_N}")
    static = []
    for i in range(STATIC_N):
        c, hit, typ = struct.unpack("<4sii", img.read(STATIC_VA + 12 * i, 12))
        static.append((i, code(c), hit, typ))
        rc, rtyp = struct.unpack("<4si", img.read(INGAME_REF_VA + 8 * i, 8))
        if (rc, rtyp) != (c, typ):
            fails.append(f"entry {i}: (code, type) differs from 0x{INGAME_REF_VA:08X}")
    if [r[1:] for r in static[1:4]] != [("lit", 0, 1), ("med", 0, 1), ("hvy", 0, 1)]:
        fails.append("entries 1..3 are not lit/med/hvy, hit class 0, type 1")
    if any(not 0 <= r[2] <= 8 for r in static):
        fails.append("a hit class outside 0..8")

    names = []
    for table, va, count_va, fixed in NAME_TABLES:
        cnt = img.u32(count_va) if count_va else fixed
        if cnt != fixed:
            fails.append(f"{table}: count {cnt}, not {fixed}")
        for i in range(fixed):
            names.append((table, i, code(img.read(va + 4 * i, 4)), "-"))
    pairs = img.u32(HIT_PAIRS_COUNT_VA)
    for i in range(pairs):
        c, hid = struct.unpack("<4sI", img.read(HIT_PAIRS_VA + 8 * i, 8))
        names.append(("hit_class_id", i, code(c), hid))
    for i in range(HIT_TO_CLASS_N):
        names.append(("hit_id_to_weapon_class", i, "-", img.u32(HIT_TO_CLASS_VA + 4 * i)))

    os.makedirs(a.out, exist_ok=True)
    write_tsv(os.path.join(a.out, "doll-static-tokens.tsv"), header,
              ["index", "code", "hit_class", "item_type"], static)
    write_tsv(os.path.join(a.out, "doll-names.tsv"), header,
              ["table", "index", "code", "value"], names)
    for f in fails:
        print("CHECK FAILED:", f)
    hits = sorted({r[2] for r in static})
    types = sorted({r[3] for r in static})
    print(f"{len(static)} static entries (hit classes {hits}, item types {types}), "
          f"{len(names)} name rows, {pairs} hit class pairs")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
