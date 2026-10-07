#!/usr/bin/env python3
"""Check a 1.14d character save (.d2s) against specs/formats/d2s.md.

Spec: specs/formats/d2s.md (framing, header, checksum, sections) and
specs/items/bitstream.md (item records, save format).

Usage: py tools/d2s_check.py <save.d2s> [<save.d2s> ...]

Reads the 1.14d tables (itemstatcost, weapons, armor, misc, itemtypes)
from $D2_GAME_DIR/extracted/patch_d2/data/global/excel. Prints field
values and a PASS/FAIL line per check. The save is opened read-only;
nothing is written. Contains no save data.
"""
import os
import struct
import sys

EXCEL_SUB = os.path.join("extracted", "patch_d2", "data", "global", "excel")
CLASS_NAMES = ["Amazon", "Sorceress", "Necromancer", "Paladin",
               "Barbarian", "Druid", "Assassin"]
CLASS_FIRST_SKILL = [6, 36, 66, 96, 126, 221, 251]  # runtime-maps.md §5


def read_txt(path):
    with open(path, encoding="latin-1") as f:
        lines = f.read().split("\n")
    head = lines[0].rstrip("\r").split("\t")
    rows = []
    for ln in lines[1:]:
        ln = ln.rstrip("\r")
        if not ln:
            continue
        cells = ln.split("\t")
        rows.append({h: (cells[i] if i < len(cells) else "") for i, h in enumerate(head)})
    return rows


def ival(s):
    try:
        return int(s)
    except ValueError:
        return 0


class Tables:
    def __init__(self, game_dir):
        d = os.path.join(game_dir, EXCEL_SUB)
        self.isc = {}
        for r in read_txt(os.path.join(d, "itemstatcost.txt")):
            if r.get("Stat", "") in ("", "Expansion"):
                continue
            i = ival(r["ID"])
            self.isc[i] = r
        self.isc_count = max(self.isc) + 1
        self.items = {}
        for fn in ("weapons.txt", "armor.txt", "misc.txt"):
            for r in read_txt(os.path.join(d, fn)):
                c = r.get("code", "")
                if c and c != "Expansion":
                    self.items[c] = r
        self.types = {}
        for r in read_txt(os.path.join(d, "itemtypes.txt")):
            if r.get("Code"):
                self.types[r["Code"]] = r

    def isc_i(self, stat, col):
        r = self.isc.get(stat)
        return ival(r.get(col, "")) if r else 0

    def type_is(self, t, target, depth=0):
        if not t or depth > 20:
            return False
        if t == target:
            return True
        r = self.types.get(t)
        if not r:
            return False
        return (self.type_is(r.get("Equiv1", ""), target, depth + 1)
                or self.type_is(r.get("Equiv2", ""), target, depth + 1))


class Bits:
    """LSB-first bit reader (items/bitstream.md §1 rule 1)."""

    def __init__(self, data, start):
        self.d = data
        self.start = start
        self.pos = start * 8
        self.over = False

    def r(self, n):
        v = 0
        for i in range(n):
            byte = self.pos >> 3
            if byte >= len(self.d):
                self.over = True
                b = 0
            else:
                b = (self.d[byte] >> (self.pos & 7)) & 1
            v |= b << i
            self.pos += 1
        return v

    def end_byte(self):
        return (self.pos + 7) >> 3


def checksum(buf):
    s = 0
    for b in buf:
        s = (((s << 1) | (s >> 31)) & 0xFFFFFFFF)
        s = (s + b) & 0xFFFFFFFF
    return s


class Report:
    def __init__(self):
        self.ok = 0
        self.bad = 0

    def check(self, name, cond, info=""):
        if cond:
            self.ok += 1
        else:
            self.bad += 1
        print(f"  {'PASS' if cond else 'FAIL'} {name} {info}")


def read_name7(b):
    s = ""
    for _ in range(16):
        c = b.r(7)
        if c == 0:
            break
        s += chr(c)
    return s


def read_trailer(b, out):
    if b.r(1):
        a, c, z = b.r(32), b.r(32), b.r(32)
        out.append(f"trailer=1({a:#x},{c:#x},{z:#x})")
    else:
        out.append("trailer=0")


def read_props(b, T, out):
    group = {17: [18], 48: [49], 50: [51], 52: [53], 54: [55, 56], 57: [58, 59]}
    stats = []
    while True:
        s = b.r(9)
        if s == 0x1FF:
            break
        if b.over or s not in T.isc or T.isc_i(s, "Save Bits") == 0:
            out.append(f"BADSTAT {s}")
            return False
        if s == 326:
            stats.append((s,))
            continue
        pb = T.isc_i(s, "Save Param Bits")
        p = b.r(pb) if (pb > 0 and s not in group) else None
        v = b.r(T.isc_i(s, "Save Bits")) - T.isc_i(s, "Save Add")
        e = [s, v] if p is None else [s, p, v]
        for q in group.get(s, []):
            e.append(b.r(T.isc_i(q, "Save Bits")) - T.isc_i(q, "Save Add"))
        stats.append(tuple(e))
    out.append(f"stats={stats}")
    return True


def read_item(data, pos, T, depth=0):
    """Read one item stream at byte pos; return (end_pos, desc, children, ok)."""
    b = Bits(data, pos)
    out = []
    if b.r(16) != 0x4D4A:
        return pos, "no JM", 0, False
    F = b.r(32)
    out.append(f"F={F:#x}")
    ver = b.r(10)
    mode = b.r(3)
    out.append(f"ver={ver} mode={mode}")
    if mode in (3, 5):
        x, y = b.r(16), b.r(16)
        out.append(f"x={x} y={y}")
    else:
        body, x, y, page = b.r(4), b.r(4), b.r(4), b.r(3)
        out.append(f"body={body} x={x} y={y} page+1={page}")
    if F & 0x200000:
        if F & 0x10000:
            ec, el = b.r(3), b.r(7)
            out.append(f"ear {ec} {el} {read_name7(b)}")
            code = "ear "
        else:
            code = b.r(32).to_bytes(4, "little").decode("latin-1")
            out.append(f"code='{code}'")
            row = T.items.get(code.strip())
            if row and T.type_is(row.get("type", ""), "gold"):
                big = b.r(1)
                out.append(f"gold={b.r(32 if big else 12)}")
            if row and ival(row.get("quest", "")) and ival(row.get("questdiffcheck", "")):
                out.append(f"qdiff={b.r(T.isc_i(356, 'Save Bits'))}")
        read_trailer(b, out)
        return b.end_byte(), " ".join(out), 0, not b.over
    code = b.r(32).to_bytes(4, "little").decode("latin-1")
    out.append(f"code='{code}'")
    row = T.items.get(code.strip())
    if row is None:
        return b.end_byte(), " ".join(out) + " UNKNOWN CODE", 0, False
    filled = b.r(3)
    uid = b.r(32)
    ilvl = b.r(7)
    q = b.r(4)
    out.append(f"filled={filled} id={uid:#010x} ilvl={ilvl} q={q}")
    t = row.get("type", "")
    tr = T.types.get(t, {})
    if b.r(1):
        out.append(f"gfx={b.r(3)}")
    if b.r(1):
        out.append(f"auto={b.r(11)}")
    shown = True  # save format
    if q in (1, 3):
        out.append(f"fidx={b.r(3)}")
    elif q == 4:
        out.append(f"pre={b.r(11)} suf={b.r(11)}")
    elif q in (5, 7):
        out.append(f"fidx={b.r(12)}")
    elif q in (6, 8):
        out.append(f"rare={b.r(8)},{b.r(8)}")
        for _ in range(6):
            if b.r(1):
                out.append(f"aff={b.r(11)}")
    elif q == 9:
        out.append(f"rare={b.r(8)},{b.r(8)}")
    else:
        if T.type_is(t, "char") and shown:
            b.r(1)
            out.append(f"charm={b.r(11)}")
        if T.type_is(t, "body") and not T.type_is(t, "play"):
            out.append(f"bodyidx={b.r(10)}")
        if T.type_is(t, "scro") or T.type_is(t, "book"):
            out.append(f"book={b.r(5)}")
    if F & 0x4000000:
        out.append(f"rw={b.r(16)}")
    if F & 0x10000:
        out.append(f"ear {b.r(3)} {b.r(7)} {read_name7(b)}")
    elif F & 0x1000000:
        out.append(f"name={read_name7(b)}")
    read_trailer(b, out)
    if T.type_is(t, "armo"):
        out.append(f"def={b.r(T.isc_i(31, 'Save Bits')) - T.isc_i(31, 'Save Add')}")
        md = b.r(T.isc_i(73, "Save Bits"))
        out.append(f"maxdur={md}")
        if md:
            out.append(f"dur={b.r(T.isc_i(72, 'Save Bits'))}")
    elif T.type_is(t, "weap"):
        md = b.r(T.isc_i(73, "Save Bits"))
        out.append(f"maxdur={md}")
        if md:
            out.append(f"dur={b.r(T.isc_i(72, 'Save Bits'))}")
    elif T.type_is(t, "gold"):
        big = b.r(1)
        out.append(f"gold={b.r(32 if big else 12)}")
    if ival(row.get("stackable", "")):
        out.append(f"qty={b.r(9)}")
    if F & 0x800:
        out.append(f"sockets={b.r(T.isc_i(194, 'Save Bits'))}")
    ok = True
    if q in (1, 2, 3, 4, 5, 6, 7, 8, 9):
        L = 0
        if q == 5:
            m = b.r(5)
            out.append(f"setmask={m}")
            L = m.bit_length()
        if F & 0x4000000:
            L += 1
        ok = read_props(b, T, out)
        for c in range(L):
            if not ok:
                break
            ok = read_props(b, T, out)
    else:
        ok = b.r(9) == 0x1FF
    return b.end_byte(), " ".join(out), filled, ok and not b.over


def read_list(data, pos, T, label, rep):
    if pos + 4 > len(data) or data[pos:pos + 2] != b"JM":
        rep.check(f"{label} JM header", False, f"at {pos:#x}")
        return pos, False
    n = struct.unpack_from("<H", data, pos + 2)[0]
    print(f"  {label}: count {n} at {pos:#x}")
    pos += 4
    for i in range(n):
        end, desc, kids, ok = read_item(data, pos, T)
        print(f"    item {i} [{pos:#x}..{end:#x}) {end - pos} B: {desc}")
        if not ok:
            rep.check(f"{label} item {i}", False)
            return end, False
        pos = end
        for k in range(kids):
            end, desc, _, ok = read_item(data, pos, T, 1)
            print(f"      child {k} [{pos:#x}..{end:#x}) {end - pos} B: {desc}")
            if not ok:
                rep.check(f"{label} item {i} child {k}", False)
                return end, False
            pos = end
    return pos, True


def check(path, T):
    with open(path, "rb") as f:
        d = f.read()
    rep = Report()
    print(f"== {os.path.basename(path)}: {len(d)} bytes")
    if len(d) < 0x14F:
        rep.check("length >= 0x14F", False)
        return rep
    u32 = lambda o: struct.unpack_from("<I", d, o)[0]
    u16 = lambda o: struct.unpack_from("<H", d, o)[0]
    rep.check("magic", u32(0) == 0xAA55AA55, f"{u32(0):#x}")
    rep.check("version 0x60", u32(4) == 0x60, f"{u32(4):#x}")
    rep.check("size field = length", u32(8) == len(d), f"{u32(8)}")
    z = bytearray(d)
    z[0x0C:0x10] = b"\0\0\0\0"
    cs = checksum(z)
    rep.check("checksum", cs == u32(0x0C), f"stored {u32(0x0C):#010x} computed {cs:#010x}")
    name = d[0x14:0x24].split(b"\0")[0].decode("latin-1")
    st = u16(0x24)
    cls = d[0x28]
    print(f"  weaponswitch={u32(0x10):#x} name={name!r} status={st:#06x} "
          f"(new={st & 1} b1={st >> 1 & 1} hc={st >> 2 & 1} dead={st >> 3 & 1} "
          f"exp={st >> 5 & 1} ladder={st >> 6 & 1} prog={st >> 8 & 0x1F}) +0x26={u16(0x26):#x}")
    print(f"  class={cls} ({CLASS_NAMES[cls] if cls < 7 else '?'}) +0x29={d[0x29]:#x} "
          f"+0x2A={d[0x2A]} level={d[0x2B]} create={u32(0x2C)} save={u32(0x30)} +0x34={u32(0x34):#x}")
    rep.check("name NUL-padded", all(c == 0 for c in d[0x14 + len(name):0x24]))
    rep.check("+0x29 = 0x10", d[0x29] == 0x10)
    rep.check("+0x2A = 30", d[0x2A] == 30)
    hk = [(u16(0x38 + 4 * i), u16(0x3A + 4 * i)) for i in range(16)]
    print(f"  hotkeys={['%04x/%d' % h for h in hk]}")
    ms = [(u16(o), u16(o + 2)) for o in (0x78, 0x7C, 0x80, 0x84)]
    print(f"  mouse L/R/swapL/swapR={['%d/%d' % m for m in ms]}")
    print(f"  appearance comp={d[0x88:0x98].hex(' ')}")
    print(f"  appearance col ={d[0x98:0xA8].hex(' ')}")
    print(f"  town={d[0xA8:0xAB].hex(' ')} mapseed={u32(0xAB):#010x}")
    hf, hs, hn, hid, hx = u32(0xAF), u32(0xB3), u16(0xB7), u16(0xB9), u32(0xBB)
    hire = (hs | hn | hx) != 0
    print(f"  hireling flags={hf:#x} seed={hs:#010x} nameidx={hn} id={hid} exp={hx} "
          f"rest={d[0xBF:0xCF].hex()} present={hire}")
    rep.check("hireling +0xBF..+0xCE zero", all(c == 0 for c in d[0xBF:0xCF]))
    print(f"  +0xCF={d[0xCF]:#x}")
    rep.check("+0xD0..+0x14E zero", all(c == 0 for c in d[0xD0:0x14F]),
              "" if all(c == 0 for c in d[0xD0:0x14F]) else
              f"nonzero at {[hex(0xD0 + i) for i, c in enumerate(d[0xD0:0x14F]) if c][:12]}")
    if st & 1:
        print("  new-character stub (status bit 0)")
        rep.check("stub length 0x14F", len(d) == 0x14F)
        return rep
    # quests
    p = 0x14F
    rep.check("Woo! at 0x14F", d[p:p + 4] == b"Woo!" and u32(p + 4) == 6 and u16(p + 8) == 0x12A,
              f"{d[p:p + 10].hex(' ')}")
    p = 0x279
    rep.check("WS at 0x279", d[p:p + 2] == b"WS" and u32(p + 2) == 1 and u16(p + 6) == 0x50,
              f"{d[p:p + 8].hex(' ')}")
    for k in range(3):
        rec = d[p + 8 + 24 * k:p + 8 + 24 * k + 24]
        print(f"  wp[{k}] record={rec[:16].hex()} tail={rec[16:].hex()}")
    p = 0x2C9
    rep.check("NPC 01 77 34 00 at 0x2C9", d[p:p + 4] == b"\x01\x77\x34\x00", f"{d[p:p + 4].hex(' ')}")
    print(f"  npc A={d[p + 4:p + 0x1C].hex()} B={d[p + 0x1C:p + 0x34].hex()}")
    p = 0x2FD
    rep.check("gf at 0x2FD", d[p:p + 2] == b"gf")
    b = Bits(d, p + 2)
    stats = []
    good = True
    prev = -1
    while True:
        sid = b.r(9)
        if sid == 0x1FF:
            break
        n = T.isc_i(sid, "CSvBits")
        if sid >= T.isc_count or n == 0 or b.over:
            good = False
            break
        pp = T.isc_i(sid, "CSvParam")
        lay = b.r(pp) if pp else 0
        v = b.r(n)
        if n < 32 and T.isc_i(sid, "CSvSigned") and v >> (n - 1):
            v -= 1 << n
        stats.append((sid, lay, v))
        good = good and sid > prev
        prev = sid
    ends = b.end_byte()
    pad = b.pos & 7
    padbits = (d[ends - 1] >> pad) if pad else 0
    view = {s: (v >> 8 if 6 <= s <= 11 else v) for s, _, v in stats}
    print(f"  stats={[(s, v) for s, _, v in stats]}  (6-11 >>8: {view})")
    rep.check("stats parse, ascending ids, 0x1FF", good, f"end {ends:#x}")
    rep.check("stats pad bits zero", padbits == 0)
    rep.check("stats: no zero values", all(v != 0 for _, _, v in stats))
    rep.check("stats level = header +0x2B", view.get(12, 0) == d[0x2B])
    p = ends
    rep.check("if after stats", d[p:p + 2] == b"if", f"at {p:#x}")
    sk = d[p + 2:p + 32]
    if cls < 7:
        lv = {CLASS_FIRST_SKILL[cls] + i: x for i, x in enumerate(sk) if x}
        print(f"  skills (id:level)={lv}")
    p += 32
    p, ok = read_list(d, p, T, "player", rep)
    rep.check("player list parses", ok)
    if not ok:
        return rep
    # corpse
    if d[p:p + 2] == b"JM":
        n = u16(p + 2)
        print(f"  corpse: count {n} at {p:#x}")
        rep.check("corpse count < 2", n < 2)
        p += 4
        if n == 1:
            print(f"    corpse u32a={u32(p):#010x} x={u32(p + 4)} y={u32(p + 8)}")
            p, ok = read_list(d, p + 12, T, "corpse items", rep)
            rep.check("corpse list parses", ok)
    else:
        rep.check("corpse JM", False, f"at {p:#x}")
        return rep
    if st & 0x20:
        if len(d) - p >= 2:
            rep.check("jf", d[p:p + 2] == b"jf", f"at {p:#x}")
            p += 2
            if hire:
                p, ok = read_list(d, p, T, "hireling", rep)
                rep.check("hireling list parses", ok)
        if len(d) - p >= 2:
            rep.check("kf", d[p:p + 2] == b"kf", f"at {p:#x}")
            g = d[p + 2] if p + 2 < len(d) else None
            print(f"  kf g={g}")
            p += 3
            if g:
                end, desc, kids, ok = read_item(d, p, T)
                print(f"    golem item [{p:#x}..{end:#x}): {desc}")
                rep.check("golem item parses", ok)
                p = end
    rep.check("sections end exactly at file end", p == len(d), f"end {p:#x} len {len(d):#x}")
    return rep


def main():
    gd = os.environ.get("D2_GAME_DIR")
    if not gd or len(sys.argv) < 2:
        print(__doc__)
        return 2
    T = Tables(gd)
    bad = 0
    for path in sys.argv[1:]:
        rep = check(path, T)
        print(f"  RESULT {os.path.basename(path)}: {rep.ok} pass, {rep.bad} fail")
        bad += rep.bad
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
