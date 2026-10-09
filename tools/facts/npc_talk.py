"""Write the NPC talk facts of the 1.14d Game.exe (specs/ui/messages.md §6, §13).

Reads the static data of game/Game.exe (PE sections map VAs to file
offsets; no game run) and writes three TSV fact files in the house style
of specs/tools/facts-render.md §1 (header line, column row, rows):

  facts/ui/npc-talk-pairs.tsv   the 527 (text id u16, caption id u16) pairs
                                at 0x00722678 (messages.md §6 r3)
  facts/ui/npc-talk-intros.tsv  the 46 x 0x16-byte intro entries at
                                0x00726850 (messages.md §13 r1)
  facts/ui/npc-talk-gossip.tsv  each entry's 15-byte text records (+0x05
                                pointer, +0x09 count; messages.md §6 r5)

Measurements only (ids, flags, counts, addresses); no string text and no
code bytes. Checks (exit 1 on failure): pair 100's text id = 164; the
intro count [0x0072554C] = 46; +0x14 = 1 exactly for classes 146, 175,
176, 244, 265; +0x13 = 1 exactly for 155, 210, 367, 521; +0x15 = 1 in all.

Usage: py tools/facts/npc_talk.py [--exe game/Game.exe] [--out facts/ui]

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import hashlib
import os
import struct
import sys

sys.dont_write_bytecode = True

TOOL = "npc_talk 0.1.0"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
GAME_SHA256 = "631066c1649c4ea9ffe48bf97e24c00bca1f7a6759c21150f1a79982589adaaf"

PAIRS_VA, PAIRS_N = 0x00722678, 527
INTRO_VA, INTRO_N, INTRO_SIZE = 0x00726850, 46, 0x16
INTRO_COUNT_VA = 0x0072554C  # u32 count of intro entries (46)
REC_SIZE = 15
# Static values read 2026-10-09 (the spec had 210 under +0x14; it is +0x13).
NO_INTRO_CLASSES = [146, 175, 176, 244, 265]
B13_CLASSES = [155, 210, 367, 521]


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


def command_line():
    def q(s):
        return f'"{s}"' if (" " in s or not s) else s
    script = os.path.relpath(os.path.abspath(sys.argv[0]), REPO).replace("\\", "/")
    args = [a.replace(";", ",") for a in sys.argv[1:]]
    return " ".join(["py", q(script)] + [q(a) for a in args])


def write_tsv(path, header, cols, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join([header, "\t".join(cols)] + ["\t".join(map(str, r)) for r in rows]) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--exe", default=os.path.join(REPO, "game", "Game.exe"))
    ap.add_argument("--out", default=os.path.join(REPO, "facts", "ui"))
    a = ap.parse_args()
    data = open(a.exe, "rb").read()
    if hashlib.sha256(data).hexdigest() != GAME_SHA256:
        raise SystemExit("Game.exe is not the 1.14d build (sha256 mismatch)")
    img = Image(data)
    header = f"# facts v1; tool: {TOOL}; command: {command_line()}; game: 1.14d"
    fails = []

    pairs = [struct.unpack_from("<HH", img.read(PAIRS_VA + 4 * i, 4)) for i in range(PAIRS_N)]
    if pairs[100][0] != 164:
        fails.append(f"pair 100 text id {pairs[100][0]} != 164")

    count = struct.unpack_from("<I", img.read(INTRO_COUNT_VA, 4))[0]
    if count != INTRO_N:
        fails.append(f"intro count [0x{INTRO_COUNT_VA:08x}] = {count} != {INTRO_N}")
    intros, gossip = [], []
    for i in range(INTRO_N):
        e = img.read(INTRO_VA + INTRO_SIZE * i, INTRO_SIZE)
        cls, act, ptr, n, idx, heard, ret, b13, b14, b15 = struct.unpack("<IBIIIBBBBB", e)
        intros.append((i, cls, act, f"0x{ptr:08x}", n, idx, heard, ret, b13, b14, b15))
        for r in range(n):
            rec = img.read(ptr + REC_SIZE * r, REC_SIZE)
            text, flag, quest, arg, rcls = struct.unpack("<HBIII", rec)
            gossip.append((i, cls, r, text, flag, quest, arg, rcls))
    no_intro = sorted(row[1] for row in intros if row[9] == 1)
    if no_intro != NO_INTRO_CLASSES:
        fails.append(f"+0x14 = 1 classes {no_intro} != {NO_INTRO_CLASSES}")
    b13 = sorted(row[1] for row in intros if row[8] == 1)
    if b13 != B13_CLASSES:
        fails.append(f"+0x13 = 1 classes {b13} != {B13_CLASSES}")
    if any(row[10] != 1 for row in intros):
        fails.append("+0x15 != 1 in some entry")

    os.makedirs(a.out, exist_ok=True)
    write_tsv(os.path.join(a.out, "npc-talk-pairs.tsv"), header,
              ["index", "text_id", "caption_id"],
              [(i, t, c) for i, (t, c) in enumerate(pairs)])
    write_tsv(os.path.join(a.out, "npc-talk-intros.tsv"), header,
              ["index", "class", "act", "records_va", "count", "gossip_index",
               "heard", "return_due", "b13", "no_intro", "greeting_due"], intros)
    write_tsv(os.path.join(a.out, "npc-talk-gossip.tsv"), header,
              ["entry", "class", "record", "text_id", "flag", "u32_03", "u32_07", "player_class"],
              gossip)
    for f in fails:
        print("CHECK FAILED:", f)
    print(f"{len(pairs)} pairs, {len(intros)} intro entries, {len(gossip)} gossip records")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
