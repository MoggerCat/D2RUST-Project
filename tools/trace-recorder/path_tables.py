"""Constant tables of the 1.14d Game.exe used by unit pathing and
placement, as `specs/sim/path-tables.tsv` lists them
(spec: `specs/sim/pathing.md` and `specs/sim/path-placement.md`).

    py tools/trace-recorder/path_tables.py                 # check the TSV against game/Game.exe
    py tools/trace-recorder/path_tables.py --write         # regenerate the TSV from game/Game.exe
    py tools/trace-recorder/path_tables.py --perturb N     # change row N in memory; must report row N
    py tools/trace-recorder/path_tables.py --selftest      # every single-row perturbation is reported

Exit 0 = every row equals the bytes at its address; 1 = mismatch.
Reads only the executable's file image (no process); our own code.
"""

import hashlib
import os
import struct
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TSV = os.path.join(REPO, "specs", "sim", "path-tables.tsv")
REF_SHA = "631066c1649c4ea9ffe48bf97e24c00bca1f7a6759c21150f1a79982589adaaf"

# name: (va, rows, columns per row, element format)
TABLES = [
    ("pathtype_flags", 0x006EB690, 18, 1, "<i"),
    ("pathtype_diroff", 0x006EB648, 18, 1, "<i"),
    ("pattern_of_size", 0x006EB3DC, 4, 1, "<i"),
    ("dir8_toward", 0x006F1798, 8, 2, "<i"),
    ("dir8_target", 0x006EB398, 8, 2, "<i"),
    ("testdir", 0x006F1518, 25, 3, "<i"),
    ("altdir", 0x006F1648, 25, 3, "<B"),
    ("dist8_path", 0x006F1698, 64, 1, "<i"),
    ("dist8_unit", 0x006EB180, 64, 1, "<i"),
    ("snap9", 0x006EB3F0, 81, 1, "<i"),
    ("tan", 0x006EB7E0, 128, 3, "<i"),
    ("dirdiff", 0x006EB548, 64, 1, "<i"),
    ("field_dx", 0x00749780, 9, 1, "<i"),
    ("field_dy", 0x007497A4, 9, 1, "<i"),
    ("velmod_player", 0x006E8A00, 20, 5, "<i"),
    ("velmod_monster", 0x006E8B90, 16, 5, "<i"),
    ("velmod_monster_x", 0x006E8CD0, 16, 5, "<i"),
    ("animstat", 0x006E8E24, 5, 3, "<i"),
]


def find_exe():
    env = os.environ.get("D2_GAME_DIR")
    base = env if env else os.path.join(os.path.dirname(REPO.rstrip("\\/")), "game")
    for cand in (os.path.join(base, "Game.exe"), os.path.join(REPO, "game", "Game.exe"),
                 os.path.join(REPO, "..", "..", "..", "game", "Game.exe")):
        if os.path.exists(cand):
            return os.path.abspath(cand)
    raise SystemExit("Game.exe not found (set D2_GAME_DIR)")


class Image:
    def __init__(self, path):
        self.data = open(path, "rb").read()
        sha = hashlib.sha256(self.data).hexdigest()
        if sha != REF_SHA:
            raise SystemExit(f"{path}: sha256 {sha} is not the reference 1.14d Game.exe")
        pe = struct.unpack_from("<I", self.data, 0x3C)[0]
        nsec = struct.unpack_from("<H", self.data, pe + 6)[0]
        optsz = struct.unpack_from("<H", self.data, pe + 20)[0]
        self.base = struct.unpack_from("<I", self.data, pe + 24 + 28)[0]
        self.secs = []
        off = pe + 24 + optsz
        for i in range(nsec):
            vsz, va, rsz, raw = struct.unpack_from("<IIII", self.data, off + 40 * i + 8)
            self.secs.append((va, max(vsz, rsz), raw, rsz))

    def read(self, va, n):
        rva = va - self.base
        for sva, ssz, raw, rsz in self.secs:
            if sva <= rva < sva + ssz:
                o = rva - sva
                if o + n <= rsz:
                    return self.data[raw + o: raw + o + n]
                return (self.data[raw + o: raw + rsz] + b"\0" * n)[:n]
        raise ValueError(hex(va))


def rows_from_exe(img):
    out = []
    for name, va, n, k, fmt in TABLES:
        sz = struct.calcsize(fmt)
        for i in range(n):
            a = va + i * k * sz
            vals = [struct.unpack_from(fmt, img.read(a + j * sz, sz))[0] for j in range(k)]
            out.append((name, i, vals, a))
    return out


def fmt_row(r):
    name, i, vals, a = r
    cells = [str(v) for v in vals] + [""] * (5 - len(vals))
    return "\t".join([name, str(i)] + cells + [f"0x{a:08X}"])


HEADER = "table\tindex\ta\tb\tc\td\te\tva"


def read_tsv():
    lines = open(TSV, encoding="utf-8").read().splitlines()
    if lines[0] != HEADER:
        raise SystemExit("bad header in " + TSV)
    return lines[1:]


def compare(expected, got):
    bad = []
    if len(expected) != len(got):
        bad.append(f"row count {len(got)} != {len(expected)}")
    for i, (e, g) in enumerate(zip(expected, got)):
        if e != g:
            bad.append(f"row {i + 1}: tsv '{g}' != exe '{e}'")
    return bad


def main():
    args = sys.argv[1:]
    img = Image(find_exe())
    expected = [fmt_row(r) for r in rows_from_exe(img)]
    if "--write" in args:
        with open(TSV, "w", encoding="utf-8", newline="\n") as f:
            f.write(HEADER + "\n" + "\n".join(expected) + "\n")
        print(f"wrote {len(expected)} rows to {TSV}")
        return 0
    got = read_tsv()
    if "--selftest" in args:
        for n in range(len(got)):
            pert = list(got)
            pert[n] = pert[n] + "x"
            bad = compare(expected, pert)
            if bad != [f"row {n + 1}: tsv '{pert[n]}' != exe '{expected[n]}'"]:
                print(f"SELFTEST FAIL at row {n + 1}: {bad}")
                return 1
        print(f"selftest ok: {len(got)} single-row perturbations each reported exactly")
        return 0
    if "--perturb" in args:
        n = int(args[args.index("--perturb") + 1])
        got = list(got)
        got[n - 1] = got[n - 1] + "x"
    bad = compare(expected, got)
    for b in bad:
        print("MISMATCH", b)
    if bad:
        return 1
    print(f"ok: {len(got)} rows equal Game.exe ({len(TABLES)} tables)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
