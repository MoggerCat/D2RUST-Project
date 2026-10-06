"""Check a stats recording (record_stats.py, format stats-raw-1) against
specs/sim/stats.md and specs/sim/stat-lists.md, and check the specs' facts
on the 1.14d files.

A model of the specs' rules replays the recording in order. Lists are
seeded from the recording (`ssd`, or a snapshot of a list the model has not
seen) and from then on every recorded operation is applied by the spec's
rules only:

  * base writes (set, set-with-unit, add, remove-all: stat-lists.md §5),
    propagation and full-value recompute (§6, stats.md §6-§7), value-change
    notifications (§7): every notification the model predicts must be the
    next recorded callback (`scb`, same list, key, old and new value), the
    records inside it are replayed in place, and no other callback may occur;
  * attach, detach, free, park/unpark, dynamic toggles, by-time refresh
    (§8), state bits (§9), expiry order (§10.4), regeneration values (§10.1);
  * every snapshot (`ssn`) must equal the model: per list the base, full and
    mod arrays, state bits, chain links and the model-owned flag bits.

Exit status 1 on any mismatch. M08: --perturb-snap N changes one value in
snapshot N, --perturb-cb N one callback value; each must be reported at
exactly that record. --selftest runs a synthetic recording built by hand
from the specs' test vectors, plain and perturbed. --files GAME_DIR checks
the specs' 1.14d table facts on the extracted itemstatcost.bin.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import csv
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
OPS_TSV = os.path.join(REPO, "specs", "sim", "stat-ops.tsv")
FORMAT = "stats-raw-1"

EXT, DYN, PERM, SETF, NEWLEN, TEMPONLY = 0x80000000, 0x40000000, 0x20000000, 0x2000, 2, 4
MODEL_BITS = DYN | PERM
F_DMG, F_FMIN, F_FCB, F_SAVED = 1 << 2, 1 << 10, 1 << 11, 1 << 12
NO_MOD = {6, 8, 10, 13, 14}   # stat-lists.md §11.1
LIFE_STATE_FLAG = 32          # states flag group `life` (stat-lists.md §9.3, §10.1)


def i32(x):
    x &= 0xFFFFFFFF
    return x - (1 << 32) if x & 0x80000000 else x


def sar(x, n):
    return i32(x) >> (n & 31)


def tdiv(a, b):
    """C division, truncating toward zero."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def muldiv(a, b, c):
    """stats.md §5 (0x00483360)."""
    a, b, c = i32(a), i32(b), i32(c)
    if c == 0:
        return 0
    if a > 0x100000:
        if c <= (a >> 4):
            return i32(tdiv(a, c) * b)
        return i32(tdiv(a * b, c))
    if b > 0x10000:
        if c <= (b >> 4):
            return i32(tdiv(b, c) * a)
        return i32(tdiv(a * b, c))
    return i32(tdiv(i32(a * b), c))


def bytime(v, basetime):
    """stats.md §8 (0x0065CA30)."""
    v = i32(v)
    t = v & 3
    lo = ((v >> 2) & 0x3FF) - 256
    hi = ((v >> 12) & 0x3FF) - 256
    d = abs(i32(basetime - t * 90))
    a = tdiv(d + 7, 15) * 15
    if a <= 0:
        a = 0
    if a >= 359:
        a = 1
    elif a > 180:
        a = 360 - a
    return i32(hi - tdiv(i32((hi - lo) * a), 180))


def make_key(stat, layer):
    return i32((stat << 16) + (layer & 0xFFFF))


def key_stat(key):
    return (key >> 16) & 0xFFFF


def load_ops():
    with open(OPS_TSV, newline="", encoding="utf-8") as f:
        return {int(r["op"]): r for r in csv.DictReader(f, delimiter="\t")}


# --- itemstatcost load-time derivation (data/fixups.md §2), used for --files and the selftest ----

def fixup_isc(raw):
    """raw: list of dicts with flags, valshift, minaccr, keepzero, op, param, base, stats (3).
    Returns the runtime records the model uses (fixups.md §2)."""
    n = len(raw)
    recs = []
    for r in raw:
        d = dict(r)
        d["a51"] = d["a52"] = d["a53"] = 0
        d["optable"] = []
        d["deps"] = []
        recs.append(d)
    for i, r in enumerate(recs):
        op = r["op"]
        if op == 0 or op > 13:
            r["op"] = 0
            continue
        base = r["base"]
        if base < n:
            recs[base]["a51"] = 1
            if len(recs[base]["deps"]) < 64:
                recs[base]["deps"].append(i)
            if op in (4, 5):
                r["a53"] = 1
        for s in r["stats"]:
            if s >= n:
                break
            r["a51"] = 1
            t = recs[s]
            if len(t["optable"]) >= 16:
                continue
            t["optable"].append([base, i, op, r["param"]])
            t["a52"] = 1
            for special, bit in ((7, 6), (9, 7), (11, 8)):
                if i == special or s == special:
                    r["flags"] |= (1 << bit) | (1 << 5)
                    break
    return recs


def isc_from_header(rows):
    out = []
    for r in rows:
        (flags, vs, ma, kz, op, par, base, st, a51, a52, a53, ot, deps) = r
        out.append({"flags": flags, "valshift": vs, "minaccr": ma, "keepzero": kz, "op": op,
                    "param": par, "base": base, "stats": st, "a51": a51, "a52": a52, "a53": a53,
                    "optable": ot, "deps": deps})
    return out


def isc_to_header(recs):
    return [[r["flags"], r["valshift"], r["minaccr"], r["keepzero"], r["op"], r["param"], r["base"],
             r["stats"], r["a51"], r["a52"], r["a53"], r["optable"], r["deps"]] for r in recs]


# --- the model ------------------------------------------------------------------------------------

class SL:
    __slots__ = ("addr", "ext", "flags", "state", "expire", "otype", "oguid", "unit", "owner",
                 "parent", "prev", "next", "last", "setl", "base", "full", "mod", "cb")

    def __init__(self, addr):
        self.addr = addr
        self.ext = False
        self.flags = 0
        self.state = 0
        self.expire = 0
        self.otype = 0
        self.oguid = 0
        self.unit = None
        self.owner = None
        self.parent = self.prev = self.next = self.last = self.setl = None
        self.base = {}
        self.full = {}
        self.mod = set()
        self.cb = False


class Mismatch(Exception):
    pass


class Model:
    def __init__(self, records):
        self.recs = records
        self.pos = 0
        self.errors = []
        self.isc = []
        self.ops = load_ops()
        self.cs = {}
        self.life_states = set()
        self.lists = {}
        self.units = {}     # uid -> {"t", "c", "act", "root"}
        self.sbits = {}     # uid -> set of states
        self.env = {}       # act -> base time
        self.frame = 0
        self.counts = {}
        self.pending = []   # regen expectations: (index, list, stat, kind, value)
        self.xp = None      # expiry window: {"root", "f", "cursor", "restart"}
        self.cur = 0

    # -- helpers ---------------------------------------------------------------
    def err(self, msg, i=None):
        self.errors.append(f"record {self.cur if i is None else i}: {msg}")

    def rec(self, stat):
        return self.isc[stat] if 0 <= stat < len(self.isc) else None

    def L(self, a):
        return self.lists.get(a) if a else None

    def utype(self, uid):
        u = self.units.get(uid)
        return u["t"] if u else None

    def root_of(self, uid):
        u = self.units.get(uid)
        return self.L(u["root"]) if u and u.get("root") else None

    def value_in(self, lst, key):
        return (lst.full if lst.ext else lst.base).get(key, 0)

    def fmin(self, lst, rec, v):
        if rec and rec["flags"] & F_FMIN and lst.ext and self.utype(lst.owner) in (0, 1):
            if v < rec["minaccr"]:
                v = i32(rec["minaccr"] << rec["valshift"])
        return v

    def total(self, lst, key, rec):
        arr = lst.full if lst.ext else lst.base
        return self.fmin(lst, rec, arr[key]) if key in arr else 0

    def basev(self, lst, key, rec):
        return self.fmin(lst, rec, lst.base[key]) if key in lst.base else 0

    def unit_total(self, uid, stat, layer=0):
        lst, rec = self.root_of(uid), self.rec(stat)
        return self.total(lst, make_key(stat, layer), rec) if lst and rec else 0

    def unit_base(self, uid, stat, layer=0):
        lst, rec = self.root_of(uid), self.rec(stat)
        return self.basev(lst, make_key(stat, layer), rec) if lst and rec else 0

    def children(self, lst):
        c = self.L(lst.last)
        while c:
            yield c
            c = self.L(c.prev)

    # -- stats.md §6: evaluation ---------------------------------------------
    def eval(self, lst, key):
        stat = key_stat(key)
        rec = self.rec(stat)
        if rec is None:
            return 0
        v = lst.base.get(key, 0)
        if lst.ext:
            for c in self.children(lst):
                if rec["flags"] & F_DMG and c.flags & DYN:
                    continue
                v = i32(v + self.value_in(c, key))
        if not rec["a52"]:
            return v
        acc, prev = v, v
        owner_t = self.utype(lst.owner) if lst.ext else None
        for ebase, src, op, param in rec["optable"][:16]:
            if op == 0:
                break
            row = self.ops.get(op)
            if row is None:
                continue
            g = row["guard"]
            if g == "never":
                continue
            if g == "owner" and owner_t is None:
                continue
            if g == "owner_item" and owner_t != 4:
                continue
            if g == "listtype_pm" and lst.otype not in (0, 1):
                continue
            if g == "unit_pm" and self.utype(lst.unit) not in (0, 1):
                continue
            if g == "owner_act" and not (owner_t is not None and self.units[lst.owner].get("act")):
                continue
            if g == "owner_player" and (owner_t != 0 or self.units[lst.owner].get("c") not in self.cs):
                continue
            if g == "owner_pm_prev" and (prev == 0 or owner_t not in (0, 1)):
                continue
            if row["prev"] == "owner_item_base" and owner_t == 4:
                prev = self.unit_base(lst.owner, stat, 0)
            x = None
            if row["operand"] in ("opbase_list_total", "opbase_unit_total"):
                if ebase == 0xFFFF:
                    continue
                rb = self.rec(ebase)
                if rb is None:
                    continue
                if row["operand"] == "opbase_list_total":
                    xv = self.total(lst, make_key(ebase, 0), None)
                else:
                    xv = self.unit_total(lst.unit, ebase, 0)
                x = sar(xv, rb["valshift"])
                if x <= 0:
                    continue
            c = row["contribution"]
            skey = make_key(src, 0)
            if c == "muldiv_prev_r":
                if prev == 0:
                    continue
                r = self.eval(lst, skey)
                if r:
                    acc = i32(acc + muldiv(prev, r, 100))
            elif c == "shift_r_x":
                r = self.eval(lst, skey)
                if r:
                    acc = i32(acc + sar(i32(r * x), param))
            elif c == "muldiv_prev_shift_r_x":
                r = self.eval(lst, skey)
                if r:
                    acc = i32(acc + muldiv(prev, sar(i32(r * x), param), 100))
            elif c == "bytime_r":
                r = self.eval(lst, skey)
                if r:
                    acc = i32(acc + bytime(r, self.env.get(self.units[lst.owner]["act"], 0)))
            elif c == "muldiv_acc_bytime_r":
                r = self.eval(lst, skey)
                if r:
                    adj = bytime(r, self.env.get(self.units[lst.owner]["act"], 0))
                    acc = i32(acc + muldiv(acc, adj, 100))
            elif c in ("charstat_mana_bonus", "charstat_vit_bonus"):
                cs = self.cs[self.units[lst.owner]["c"]]
                bonus = i32(self.eval(lst, skey) - lst.base.get(skey, 0))
                if bonus:
                    if c == "charstat_mana_bonus":
                        per = cs[3]
                    else:
                        per = cs[2] if stat == 11 else cs[1]
                    acc = i32(acc + i32((per * bonus) << 6))
        return acc

    # -- stat-lists.md §6: full writes, propagation, recompute ----------------
    def notify(self, lst, key, old, new, rec):
        if lst.cb and rec["flags"] & F_FCB:
            self.expect_cb(lst, key, old, new)

    def set_full(self, lst, key, v, rec):
        if rec is None:
            return
        if key not in lst.full:
            if v == 0:
                return
            lst.full[key] = 0
        old = lst.full[key]
        if v == 0 and not rec["keepzero"]:
            del lst.full[key]
        else:
            lst.full[key] = v
            if rec["a53"]:
                lst.flags |= PERM
        if old != v:
            self.notify(lst, key, old, v, rec)

    def add_full(self, lst, key, d, rec):
        old = lst.full.get(key, 0)
        new = i32(old + d)
        lst.full[key] = new
        if new > 0 and rec["a53"]:
            lst.flags |= PERM
        if new == 0 and not rec["keepzero"]:
            del lst.full[key]
        self.notify(lst, key, old, new, rec)

    def recompute(self, lst, key, rec):
        v = self.eval(lst, key)
        if not rec["a51"]:
            self.set_full(lst, key, v, rec)
            return v
        if v == 0:
            self.set_full(lst, key, 0, rec)
        upd = True
        block = self.ops[rec["op"]]["recompute_block"] if rec["op"] in self.ops else "none"
        for k in range(3):
            s = rec["stats"][k]
            if s == 0xFFFF:
                break
            rs = self.rec(s)
            if rs is None:
                continue
            vs = self.recompute(lst, make_key(s, 0), rs)
            self.set_full(lst, make_key(s, 0), vs, rs)
            if vs == 0:
                continue
            ebase = rec["optable"][k][0] if k < len(rec["optable"]) else 0
            if block == "always":
                upd = False
            elif block == "listtype_pm":
                if lst.otype in (0, 1):
                    upd = False
            elif block == "listtype_item":
                if lst.otype == 4:
                    upd = False
            elif block == "listtype_pm_and_entrybase_list_total_pos":
                if lst.otype in (0, 1) and ebase != 0xFFFF and \
                        self.total(lst, make_key(ebase, 0), None) > 0:
                    upd = False
            elif block == "unit_pm_and_entrybase_unit_total_pos":
                if self.utype(lst.unit) in (0, 1) and ebase != 0xFFFF and \
                        self.unit_total(lst.unit, ebase, 0) > 0:
                    upd = False
        if upd and v != 0:
            self.set_full(lst, key, v, rec)
            for d in rec["deps"][:64]:
                rd = self.rec(d)
                if rd is None:
                    continue
                vd = self.recompute(lst, make_key(d, 0), rd)
                self.set_full(lst, make_key(d, 0), vd, rd)
        return v

    def propagate(self, lst, key, d):
        if d == 0:
            return
        rec = self.rec(key_stat(key))
        if rec is None or lst.flags & SETF:
            return
        dmg = rec["flags"] & F_DMG
        cur = lst if lst.ext else self.L(lst.parent)
        while cur:
            if not rec["a51"] and not rec["a52"]:
                self.add_full(cur, key, d, rec)
            else:
                self.recompute(cur, key, rec)
            if cur.flags & SETF or (dmg and cur.flags & DYN):
                break
            cur = self.L(cur.parent)

    def mod_insert(self, lst, key):
        if not lst.ext:
            return
        stat = key_stat(key)
        rec = self.rec(stat)
        if stat in NO_MOD or rec is None or not rec["flags"] & F_SAVED:
            return
        lst.mod.add(key)

    # -- stat-lists.md §5: base writes -----------------------------------------
    def set_stat(self, lst, stat, value, layer):
        key = make_key(stat, layer)
        if key not in lst.base:
            if value == 0:
                return
            lst.base[key] = 0
        diff = i32(value - lst.base[key])
        if diff == 0:
            return
        if value == 0:
            del lst.base[key]
        else:
            lst.base[key] = value
        self.propagate(lst, key, diff)
        if lst.ext and lst.otype == 0:
            self.mod_insert(lst, key)

    def add_stat(self, lst, stat, value, layer):
        if value == 0:
            return
        key = make_key(stat, layer)
        nv = i32(lst.base.get(key, 0) + value)
        if nv == 0:
            lst.base.pop(key, None)
        else:
            lst.base[key] = nv
        self.propagate(lst, key, value)
        if lst.ext and lst.otype == 0:
            self.mod_insert(lst, key)

    def remove_all(self, lst):
        while lst.base:
            key = min(lst.base)
            d = -lst.base[key]
            if d == 0:
                self.err("remove-all over a zero base value (1.14d loops forever, stat-lists.md §5.4)")
                return
            del lst.base[key]
            self.propagate(lst, key, d)
            if lst.ext and lst.otype == 0:
                self.mod_insert(lst, key)

    # -- stat-lists.md §8: chain ------------------------------------------------
    def detach(self, lst):
        p = self.L(lst.parent)
        if p:
            if p.ext:
                if p.last == lst.addr:
                    p.last = lst.prev
                if p.setl == lst.addr:
                    p.setl = lst.prev
            lst.parent = None
        if lst.next and self.L(lst.next):
            self.L(lst.next).prev = lst.prev
        if lst.prev and self.L(lst.prev):
            self.L(lst.prev).next = lst.next
        lst.prev = lst.next = None
        lst.unit = None
        if lst.flags & SETF:
            return
        if lst.ext and lst.flags & PERM:
            keys = [k for k in sorted(lst.full) if (self.rec(key_stat(k)) or {}).get("a53")]
            for k in keys[:16]:
                self.recompute(lst, k, self.rec(key_stat(k)))
        if p:
            dyn = lst.flags & DYN
            arr = lst.full if lst.ext else lst.base
            for k in sorted(arr):
                rec = self.rec(key_stat(k))
                if dyn and (rec is None or rec["flags"] & F_DMG):
                    continue
                self.propagate(p, k, -arr[k])

    def attach(self, uid, lst, reset):
        r = self.root_of(uid)
        if r is None or not r.ext:
            self.err(f"attach to unit {uid} without a modelled unit list")
            return
        a = r
        while a:
            if a is lst:
                return
            a = self.L(a.parent)
        if lst.flags & TEMPONLY:
            r.flags |= NEWLEN
        if lst.flags & SETF:
            lst.prev = r.setl
            if self.L(r.setl):
                self.L(r.setl).next = lst.addr
            lst.next = None
            r.setl = lst.addr
            lst.parent, lst.unit = r.addr, uid
            return
        lst.prev = r.last
        if self.L(r.last):
            self.L(r.last).next = lst.addr
        lst.parent = r.addr
        r.last = lst.addr
        lst.unit = uid
        if lst.ext and lst.flags & PERM:
            keys = [k for k in sorted(lst.full) if (self.rec(key_stat(k)) or {}).get("a53")]
            for k in keys[:16]:
                self.recompute(lst, k, self.rec(key_stat(k)))
        arr = lst.full if lst.ext else lst.base
        if reset:
            lst.flags &= ~DYN
        else:
            lst.flags |= DYN
        for k in sorted(arr):
            rec = self.rec(key_stat(k))
            if not reset and (rec is None or rec["flags"] & F_DMG):
                continue
            self.propagate(r, k, arr[k])

    def free(self, lst):
        if lst.ext:
            c = self.L(lst.last)
            while c:
                p = c.prev
                c.parent = None
                c.unit = None
                if c.ext:
                    c = self.L(p)
                    continue
                if lst.last == c.addr:
                    lst.last = p
                # the child's own free (records follow) unlinks it from its siblings
                if self.L(c.next):
                    self.L(c.next).prev = c.prev
                if self.L(c.prev):
                    self.L(c.prev).next = c.next
                c.prev = c.next = None
                c = self.L(lst.last)
            for uid, u in self.units.items():
                if u.get("root") == lst.addr:
                    u["root"] = None
        self.lists.pop(lst.addr, None)

    # -- callbacks (stat-lists.md §7) -------------------------------------------
    def expect_cb(self, lst, key, old, new):
        if self.pos >= len(self.recs):
            raise Mismatch(f"predicted callback on {lst.addr} key {key:#x} {old}->{new}: recording ended")
        i = self.pos
        r = self.recs[i]
        self.pos += 1
        self.cur = i
        if r["k"] != "scb":
            raise Mismatch(f"predicted callback on {lst.addr} stat {key_stat(key)} {old}->{new}, "
                           f"recorded {r['k']}")
        got = (r["L"], r["key"], r["old"], r["new"])
        want = (lst.addr, key, old, new)
        if got != want:
            raise Mismatch(f"callback {got} != model {want}")
        self.counts["callbacks"] = self.counts.get("callbacks", 0) + 1
        # replay what happened inside the callback, in place
        while True:
            if self.pos >= len(self.recs):
                raise Mismatch("recording ended inside a callback")
            j = self.pos
            r = self.recs[j]
            self.pos += 1
            self.cur = j
            if r["k"] == "scx":
                return
            self.dispatch(j, r)

    # -- record handling --------------------------------------------------------
    def adopt_unit(self, uid, info):
        if uid is None or info is None:
            return
        u = self.units.setdefault(uid, {})
        u["t"], u["c"], u["act"] = info["t"], info["c"], info.get("act")

    def adopt_list(self, d):
        lst = self.lists.get(d["L"]) or SL(d["L"])
        self.lists[d["L"]] = lst
        lst.ext, lst.flags, lst.state, lst.expire = d["ext"], d["fl"], d["st"], d["ex"]
        lst.otype, lst.oguid, lst.unit, lst.owner = d["ot"], d["og"], d["u"], d.get("ow")
        lst.parent, lst.prev, lst.next = d["par"], d["prev"], d["next"]
        lst.last, lst.setl = d.get("last"), d.get("setl")
        lst.base = {make_key(s, l): v for s, l, v in d["b"]}
        lst.full = {make_key(s, l): v for s, l, v in d.get("F", [])}
        lst.mod = set(i32(k) for k in d.get("m", []))
        lst.cb = d.get("cb", False)
        if lst.ext and lst.owner is not None:
            self.units.setdefault(lst.owner, {})["root"] = lst.addr
            if "sb" in d:
                self.sbits[lst.owner] = set(d["sb"])
        return lst

    def check_flags(self, lst, fl, what):
        if (lst.flags ^ fl) & MODEL_BITS:
            self.err(f"{what}: list {lst.addr} flags {fl:#x} != model {lst.flags:#x} "
                     f"(bits {MODEL_BITS:#x})")
        lst.flags = (fl & ~MODEL_BITS) | (lst.flags & MODEL_BITS)

    def need(self, a):
        lst = self.lists.get(a)
        if lst is None:
            self.err(f"operation on list {a} the model has not seen")
        return lst

    def compare_list(self, d):
        lst = self.lists.get(d["L"])
        if lst is None:
            self.adopt_list(d)
            self.counts["seeded_by_snapshot"] = self.counts.get("seeded_by_snapshot", 0) + 1
            return
        bad = []
        sb = sorted((make_key(s, l), v) for s, l, v in d["b"])
        if sb != sorted(lst.base.items()):
            bad.append(f"base {fmt(sb)} != model {fmt(sorted(lst.base.items()))}")
        if d["ext"]:
            sf = sorted((make_key(s, l), v) for s, l, v in d.get("F", []))
            if sf != sorted(lst.full.items()):
                bad.append(f"full {fmt(sf)} != model {fmt(sorted(lst.full.items()))}")
            if set(i32(k) for k in d.get("m", [])) != lst.mod:
                bad.append(f"mod {sorted(d.get('m', []))} != model {sorted(lst.mod)}")
            if d.get("ow") is not None and "sb" in d and \
                    set(d["sb"]) != self.sbits.get(d["ow"], set()):
                bad.append(f"state bits {sorted(d['sb'])} != model "
                           f"{sorted(self.sbits.get(d['ow'], set()))}")
        for f, m in (("par", lst.parent), ("prev", lst.prev), ("next", lst.next),
                     ("last", lst.last), ("setl", lst.setl)):
            if f in d and d[f] != m and not (f in ("last", "setl") and not lst.ext):
                bad.append(f"{f} {d[f]} != model {m}")
        if (d["fl"] ^ lst.flags) & MODEL_BITS:
            bad.append(f"flags {d['fl']:#x} != model {lst.flags:#x}")
        if bad:
            self.err(f"snapshot frame {self.frame}: list {d['L']} " + "; ".join(bad))
            self.adopt_list(d)  # resync, so one error is reported once

    def dispatch(self, i, r):
        k = r["k"]
        self.counts[k] = self.counts.get(k, 0) + 1
        if k == "stab":
            self.isc = isc_from_header(r["isc"])
            self.cs = {int(c): v for c, v in r["cs"].items()}
            self.life_states = set(r.get("life_states", []))
        elif k == "tick":
            self.close_pending(i)
            self.frame = r["f"]
        elif k == "senv":
            self.env = {a: v for a, v in r["env"].items()}
        elif k == "ssd":
            for uid, info in r.get("units", {}).items():
                self.adopt_unit(uid, info)
            for d in r["lists"]:
                self.adopt_list(d)
        elif k == "ssn":
            for uid, info in r.get("units", {}).items():
                self.adopt_unit(uid, info)
            for d in r["lists"]:
                self.compare_list(d)
        elif k == "sax":
            self.adopt_unit(r["U"], r["ui"])
            lst = SL(r["L"])
            lst.ext, lst.flags, lst.otype, lst.oguid = True, r["fl"], r["ot"], r["og"]
            lst.owner, lst.cb = r["U"], r["cb"]
            self.lists[r["L"]] = lst
            self.units[r["U"]]["root"] = r["L"]
            self.sbits[r["U"]] = set()
        elif k in ("ss", "sa", "sr"):
            lst = self.need(r["L"])
            if lst is None:
                return
            self.check_flags(lst, r["fl"], k)
            if k == "ss":
                self.check_pending(i, lst, r["s"], "set", r["v"])
                self.set_stat(lst, r["s"], r["v"], r["l"])
            elif k == "sa":
                self.check_pending(i, lst, r["s"], "add", r["v"])
                self.add_stat(lst, r["s"], r["v"], r["l"])
            else:
                self.remove_all(lst)
        elif k == "sdt":
            lst = self.need(r["L"])
            if lst:
                self.check_flags(lst, r["fl"], k)
                self.detach(lst)
        elif k == "sat":
            lst = self.need(r["L"])
            if lst:
                self.check_flags(lst, r["fl"], k)
                self.attach(r["U"], lst, r["r"])
        elif k == "sfr":
            lst = self.need(r["L"])
            if lst:
                self.free(lst)
                if self.xp is not None:
                    self.xp["restart"] = True
        elif k == "sdy":
            self.dynamic(r["U"], r["I"], r["on"])
        elif k == "sbt":
            self.bytime_refresh(r["U"], r["I"])
        elif k == "stg":
            bits = self.sbits.setdefault(r["U"], set())
            if r["on"]:
                bits.add(r["s"])
            else:
                bits.discard(r["s"])
        elif k == "sxp":
            root = self.root_of(r["U"])
            if root is None:
                self.err(f"expiry for unit {r['U']} without a modelled list")
                return
            for a, fl, ex in r["lists"]:
                lst = self.lists.get(a)
                if lst:
                    lst.flags = (fl & ~MODEL_BITS) | (lst.flags & MODEL_BITS)
                    lst.expire = ex
            self.xp = {"root": root, "f": r["f"], "cursor": root.last, "restart": False}
        elif k == "sxf":
            self.expiry_free(r["L"])
        elif k == "sxe":
            self.expiry_end()
        elif k in ("sgl", "sgs", "sgm", "sgx"):
            self.regen(i, k, r)
        elif k == "scb":
            self.err(f"callback on {r['L']} stat {key_stat(r['key'])} {r['old']}->{r['new']} "
                     f"not predicted by the model")
        elif k == "scx":
            self.err("callback return without a callback")

    def dynamic(self, uid, iid, on):
        il = self.root_of(iid)
        r = self.root_of(uid)
        if il is None or r is None or il.unit != uid:
            return  # not attached to this unit: the equip path records follow (stat-lists.md §8.4)
        if on:
            if il.flags & DYN:
                return
            il.flags |= DYN
            sign = -1
        else:
            if not il.flags & DYN:
                return
            il.flags &= ~DYN
            sign = 1
        arr = il.full if il.ext else il.base
        for k in sorted(arr):
            rec = self.rec(key_stat(k))
            if rec and rec["flags"] & F_DMG:
                self.propagate(r, k, sign * arr[k])

    def bytime_refresh(self, uid, iid):
        if self.utype(uid) not in (0, 1):
            return
        il, r = self.root_of(iid), self.root_of(uid)
        if il is None or r is None or not il.ext:
            return
        for k in sorted(il.full):
            rec = self.rec(key_stat(k))
            if rec is None or rec["op"] not in (6, 7):
                continue
            for s in rec["stats"]:
                if s == 0xFFFF:
                    break
                rs = self.rec(s)
                self.set_full(r, make_key(s, 0), self.eval(r, make_key(s, 0)), rs)

    # -- expiry (stat-lists.md §10.4) --------------------------------------------
    def next_due(self):
        xp = self.xp
        if xp["restart"]:
            xp["cursor"], xp["restart"] = xp["root"].last, False
        c = self.L(xp["cursor"])
        while c:
            if c.flags & NEWLEN and c.expire <= xp["f"]:
                return c
            c = self.L(c.prev)
        return None

    def expiry_free(self, a):
        if self.xp is None:
            self.err("expiry free outside an expiry")
            return
        c = self.next_due()
        if c is None or c.addr != a:
            self.err(f"expiry frees {a}, model expects {c.addr if c else 'nothing'}")
            return
        if c.ext:
            self.err(f"expiry of extended list {a} (1.14d does not free it and loops, §10.4)")
        self.xp["cursor"] = c.addr
        self.xp["restart"] = True

    def expiry_end(self):
        if self.xp is None:
            return
        c = self.next_due()
        if c is not None:
            self.err(f"expiry ended with list {c.addr} still due (expire {c.expire} <= {self.xp['f']})")
        self.xp = None

    # -- regeneration (stat-lists.md §10.1) -------------------------------------------
    def regen(self, i, k, r):
        uid = r["U"]
        root = self.root_of(uid)
        if root is None:
            return
        tot = lambda s: self.unit_total(uid, s, 0)
        if k == "sgl":
            rg = tot(74)
            if rg == 0:
                return
            hp = i32(tot(6) + rg)
            mx = tot(7)
            if hp > mx:
                hp = mx
            if hp < 256:
                hp = 256
            self.pending.append((i, root.addr, 6, "set", hp))
        elif k == "sgs":
            s, bonus = tot(10), tot(28)
            mode = r["mode"]
            shift = 8
            if mode == 2:
                shift = 9
                if not (s & 0xFFFFFF00):
                    return
            elif mode == 6:
                shift = 9
            elif mode in (1, 5):
                pass
            elif bonus < 1000:
                return
            mx = tot(11)
            if s >= mx:
                return
            inc = sar(mx, shift)
            if bonus:
                inc = i32(inc + tdiv(i32(inc * bonus), 100))
            s = i32(s + inc)
            if s > mx:
                s = mx
            self.pending.append((i, root.addr, 10, "set", s))
        elif k == "sgm":
            m, mx = tot(8), tot(9)
            inc = 0
            if 85 not in self.sbits.get(uid, set()):
                u = self.units.get(uid, {})
                cs = self.cs.get(u.get("c"))
                if cs is None:
                    return
                div = cs[0] * 25 or 7500
                inc = tdiv(mx, div)
                if inc < 1:
                    inc = 1
                inc = muldiv(inc, i32(tot(27) + 100), 100)
            inc = i32(inc + tot(26))
            if inc > i32(mx - m):
                inc = i32(mx - m)
            if inc < -m:
                inc = -m
            if inc:
                self.pending.append((i, root.addr, 8, "add", inc))
        elif k == "sgx":
            rg = tot(74)
            if self.sbits.get(uid, set()) & self.life_states:
                rg = i32(rg - self.unit_base(uid, 74, 0))
            if 52 in self.sbits.get(uid, set()) and rg >= 0:
                return
            if rg == 0:
                return
            hp, mx = tot(6), tot(7)
            if rg < 0 and hp < 256:
                return  # room-dependent branch, not predicted (stat-lists.md open question 2)
            hp = i32(hp + rg)
            if hp > mx:
                hp = mx
            if hp < 1:
                hp = 0
            self.pending.append((i, root.addr, 6, "set", hp))

    def check_pending(self, i, lst, stat, kind, value):
        for n, (j, a, s, kd, v) in enumerate(self.pending):
            if a == lst.addr and s == stat and kd == kind:
                del self.pending[n]
                self.counts["regen_checked"] = self.counts.get("regen_checked", 0) + 1
                if v != value:
                    self.err(f"regeneration (record {j}) writes stat {stat} {kind} {value}, "
                             f"model {v}", i)
                return

    def close_pending(self, i):
        for j, a, s, kd, v in self.pending:
            self.err(f"regeneration predicted {kd} of stat {s} = {v} on {a}, not recorded", j)
        self.pending = []

    def run(self):
        while self.pos < len(self.recs):
            i = self.pos
            self.pos += 1
            self.cur = i
            try:
                self.dispatch(i, self.recs[i])
            except Mismatch as e:
                self.err(str(e))
        self.close_pending(len(self.recs))


def fmt(items):
    return "[" + ", ".join(f"{key_stat(k)}/{k & 0xFFFF}={v}" for k, v in items) + "]"


def check(records, out=sys.stdout, quiet=False):
    m = Model(records)
    m.run()
    if not quiet:
        c = m.counts
        print(f"records {len(records)}, ticks {c.get('tick', 0)}, set {c.get('ss', 0)}, "
              f"add {c.get('sa', 0)}, attach {c.get('sat', 0)}, detach {c.get('sdt', 0)}, "
              f"free {c.get('sfr', 0)}, callbacks {c.get('callbacks', 0)}, "
              f"expiry frees {c.get('sxf', 0)}, regen checked {c.get('regen_checked', 0)}, "
              f"snapshots {c.get('ssn', 0)} (lists seeded by snapshot "
              f"{c.get('seeded_by_snapshot', 0)}), errors {len(m.errors)}", file=out)
        for e in m.errors[:50]:
            print("  " + e, file=out)
    return m


def first_error_index(m):
    return int(m.errors[0].split(":")[0].split()[1]) if m.errors else None


def load(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def perturb_snap(records, n):
    idx = [i for i, r in enumerate(records) if r["k"] == "ssn"]
    for i in idx[n:]:
        for d in records[i]["lists"]:
            arr = d.get("F") or d["b"]
            if arr:
                arr[0][2] += 1
                return i
    raise SystemExit(f"no snapshot with a value at or after #{n}")


def perturb_cb(records, n):
    idx = [i for i, r in enumerate(records) if r["k"] == "scb"]
    if n >= len(idx):
        raise SystemExit(f"only {len(idx)} callbacks")
    records[idx[n]]["new"] += 1
    return idx[n]


# --- synthetic recording (selftest) -----------------------------------------------------------------

def synthetic_isc():
    """A small itemstatcost table: the stats the hand-built vectors use (stats.md, Test vectors)."""
    raw = [{"flags": 0, "valshift": 0, "minaccr": 0, "keepzero": 0, "op": 0, "param": 0,
            "base": 0xFFFF, "stats": [0xFFFF] * 3} for _ in range(220)]

    def st(i, **kw):
        raw[i].update(kw)
    st(0, flags=F_FMIN | F_SAVED, minaccr=1)                    # strength
    st(3, flags=F_FMIN | F_SAVED, minaccr=1, op=9, stats=[7, 11, 0xFFFF])  # vitality
    st(6, flags=F_SAVED, valshift=8)                            # hitpoints
    st(7, flags=F_FMIN | F_FCB | F_SAVED, valshift=8, minaccr=1)  # maxhp
    st(8, flags=F_SAVED, valshift=8, keepzero=1)                # mana
    st(9, flags=F_FMIN | F_FCB | F_SAVED, valshift=8)           # maxmana
    st(11, flags=F_FMIN | F_FCB | F_SAVED, valshift=8)          # maxstamina
    st(12, flags=F_SAVED)                                       # level
    st(19, flags=F_DMG)                                         # tohit
    st(74, flags=0)                                             # hpregen
    st(216, op=2, param=3, base=12, stats=[7, 0xFFFF, 0xFFFF], valshift=8)  # item_hp_perlevel
    return fixup_isc(raw)


def synthetic():
    """Hand-built from the test vectors of stat-lists.md (expected values computed by hand
    in the spec, not by this model)."""
    P, IL, S = "0xP", "0xI", "0xS"
    PU, IU = "0:1", "4:7"
    K = lambda s, l=0: (s << 16) | l

    def ss(L, s, v, l=0, fl=EXT):
        return {"k": "ss", "L": L, "s": s, "l": l, "v": v, "fl": fl}

    def cb(L, s, old, new):
        return {"k": "scb", "L": L, "key": K(s), "old": old, "new": new}

    cbx = {"k": "scx"}

    def lst(L, ext, fl, b, F=None, m=None, sb=None, par=None, prev=None, nxt=None, last=None,
            u=None, ow=None, st=0, ex=0, ot=0):
        d = {"L": L, "ext": ext, "fl": fl, "st": st, "ex": ex, "ot": ot, "og": 0, "u": u, "ow": ow,
             "par": par, "prev": prev, "next": nxt, "b": b, "cb": ext and ow == PU}
        if ext:
            d.update({"F": F or [], "m": m or [], "last": last, "setl": None})
            if sb is not None:
                d["sb"] = sb
        return d

    rec = [{"k": "header", "format": FORMAT},
           {"k": "stab", "isc": isc_to_header(synthetic_isc()),
            "cs": {"4": [10, 16, 4, 4]}, "life_states": []},
           {"k": "tick", "f": 1},
           {"k": "sax", "L": P, "U": PU, "ui": {"t": 0, "c": 4, "act": "A1"}, "fl": EXT,
            "ot": 0, "og": 1, "cb": True},
           ss(P, 0, 30), ss(P, 12, 10), ss(P, 3, 25),
           ss(P, 7, 12800), cb(P, 7, 0, 12800), cbx,
           ss(P, 6, 12800),
           {"k": "sax", "L": IL, "U": IU, "ui": {"t": 4, "c": 100, "act": None}, "fl": EXT,
            "ot": 4, "og": 7, "cb": False},
           ss(IL, 3, 10), ss(IL, 216, 8), ss(IL, 19, 5),
           {"k": "sat", "U": PU, "L": IL, "r": 1, "fl": EXT, "R": P},
           # vitality +10: maxhp 12800 + (16*10)<<6 + (10*8)>>3 = 23050; the callback rescales hp
           cb(P, 7, 12800, 23050), ss(P, 6, 23050), cbx,
           cb(P, 11, 0, 2560), cbx,
           {"k": "ssn", "f": 1, "lists": [
               lst(P, True, EXT, [[0, 0, 30], [3, 0, 25], [6, 0, 23050], [7, 0, 12800],
                                  [12, 0, 10]],
                   F=[[0, 0, 30], [3, 0, 35], [6, 0, 23050], [7, 0, 23050], [11, 0, 2560],
                      [12, 0, 10], [19, 0, 5]],
                   m=[K(0), K(3), K(7), K(12)], sb=[], last=IL, ow=PU),
               lst(IL, True, EXT, [[3, 0, 10], [19, 0, 5], [216, 0, 8]],
                   F=[[3, 0, 10], [19, 0, 5], [216, 0, 8]], par=P, u=PU, ow=IU, ot=4)]},
           {"k": "sdt", "L": IL, "fl": EXT},
           cb(P, 7, 23050, 12800), ss(P, 6, 12800), cbx,
           cb(P, 11, 2560, 0), cbx,
           # a state list: +5 strength until frame 20
           {"k": "ssd", "lists": [lst(S, False, NEWLEN, [[0, 0, 5]], st=30, ex=20)], "units": {}},
           {"k": "sat", "U": PU, "L": S, "r": 1, "fl": NEWLEN, "R": P},
           {"k": "stg", "U": PU, "s": 30, "on": 1},
           ss(P, 74, 100), ss(P, 6, 10000),
           {"k": "sgl", "U": PU}, ss(P, 6, 10100),
           {"k": "ssn", "f": 1, "lists": [
               lst(P, True, EXT, [[0, 0, 30], [3, 0, 25], [6, 0, 10100], [7, 0, 12800],
                                  [12, 0, 10], [74, 0, 100]],
                   F=[[0, 0, 35], [3, 0, 25], [6, 0, 10100], [7, 0, 12800], [12, 0, 10],
                      [74, 0, 100]],
                   m=[K(0), K(3), K(7), K(12)], sb=[30], last=S, ow=PU),
               lst(S, False, NEWLEN, [[0, 0, 5]], par=P, u=PU, st=30, ex=20)]},
           {"k": "tick", "f": 20},
           {"k": "sxp", "U": PU, "f": 20, "lists": [[S, NEWLEN, 20]]},
           {"k": "sxf", "L": S},
           {"k": "sdt", "L": S, "fl": NEWLEN},
           {"k": "stg", "U": PU, "s": 30, "on": 0},
           {"k": "sfr", "L": S},
           {"k": "sxe"},
           {"k": "ssn", "f": 20, "lists": [
               lst(P, True, EXT, [[0, 0, 30], [3, 0, 25], [6, 0, 10100], [7, 0, 12800],
                                  [12, 0, 10], [74, 0, 100]],
                   F=[[0, 0, 30], [3, 0, 25], [6, 0, 10100], [7, 0, 12800], [12, 0, 10],
                      [74, 0, 100]],
                   m=[K(0), K(3), K(7), K(12)], sb=[], ow=PU)]},
           {"k": "footer"}]
    return rec


def selftest():
    ok = True
    base = synthetic()
    m = check(base, quiet=True)
    if m.errors:
        print("selftest: the synthetic recording fails:", *m.errors, sep="\n  ")
        ok = False
    # unit vectors of the helpers (stats.md, Test vectors)
    vectors = [
        (muldiv(250, 40, 100), 100), (muldiv(-250, 40, 100), -100), (muldiv(7, 9, 0), 0),
        (muldiv(0x200000, 3, 0x10), 0x60000), (muldiv(0x200000, 0x100, 0x100000), 0x200),
        (muldiv(100, 0x20000, 0x1000), 0xC80), (muldiv(0x100000, 0x10000, 1), 0),
        (bytime(((356 << 12) | (306 << 2) | 0), 0), 100),
        (bytime(((356 << 12) | (306 << 2) | 0), 180), 50),
        (bytime(((356 << 12) | (306 << 2) | 1), 0), 75),
        (sar(-7, 1), -4),
    ]
    for got, want in vectors:
        if got != want:
            print(f"selftest: helper vector {got} != {want}")
            ok = False
    for name, fn, n in (("snapshot", perturb_snap, 0), ("snapshot", perturb_snap, 1),
                        ("callback", perturb_cb, 0), ("callback", perturb_cb, 3)):
        rec = json.loads(json.dumps(base))
        at = fn(rec, n)
        m = check(rec, quiet=True)
        if first_error_index(m) != at:
            print(f"selftest: {name} perturbation at record {at} reported at "
                  f"{first_error_index(m)}: {m.errors[:2]}")
            ok = False
    # a wrong expiry order and a wrong regeneration value
    rec = json.loads(json.dumps(base))
    at = next(i for i, r in enumerate(rec) if r["k"] == "sxf")
    rec[at]["L"] = "0xP"
    m = check(rec, quiet=True)
    if first_error_index(m) != at:
        print(f"selftest: expiry perturbation at {at} reported at {first_error_index(m)}")
        ok = False
    rec = json.loads(json.dumps(base))
    at = next(i for i, r in enumerate(rec) if r["k"] == "sgl") + 1
    rec[at]["v"] += 1
    m = check(rec, quiet=True)
    if first_error_index(m) != at:
        print(f"selftest: regeneration perturbation at {at} reported at {first_error_index(m)}")
        ok = False
    print("selftest:", "pass" if ok else "FAIL")
    return ok


# --- 1.14d file checks --------------------------------------------------------------------------------

FILE_FACTS = {  # stats.md §3, Test vectors (measured on patch_d2/itemstatcost.bin)
    "count": 359,
    "valshift": {6: 8, 7: 8, 8: 8, 9: 8, 10: 8, 11: 8, 216: 8, 217: 8},
    "keepzero": [8, 10],
    "fmin": {0: 1, 1: 1, 2: 1, 3: 1, 7: 1, 9: 0, 11: 0},
    "direct": [6, 8, 10, 72],
    "maxstat": {6: 7, 8: 9, 10: 11, 72: 73},
    "saved": list(range(16)),
    "updateanimrate": [67, 68, 69],
    "fcallback_count": 35, "damagerelated_count": 104, "signed_count": 294,
    "ops": {1: 2, 2: 33, 4: 2, 5: 2, 6: 34, 7: 2, 8: 1, 9: 1, 11: 2, 13: 5},
    "targets": 42,
    "flag_bits": [0, 1, 2, 3, 4, 9, 10, 11, 12],
}


def files_check(game_dir):
    p = os.path.join(game_dir, "extracted", "patch_d2", "data", "global", "excel",
                     "itemstatcost.bin")
    b = open(p, "rb").read()
    n = struct.unpack_from("<I", b, 0)[0]
    rows = [b[4 + i * 0x144: 4 + (i + 1) * 0x144] for i in range(n)]
    fl = [struct.unpack_from("<I", r, 4)[0] for r in rows]
    raw = [{"flags": fl[i], "valshift": r[0x18], "minaccr": struct.unpack_from("<I", r, 0x2C)[0],
            "keepzero": r[0x50], "op": r[0x54], "param": r[0x55],
            "base": struct.unpack_from("<H", r, 0x56)[0],
            "stats": list(struct.unpack_from("<3H", r, 0x58))} for i, r in enumerate(rows)]
    got = {
        "count": n,
        "valshift": {i: r[0x18] for i, r in enumerate(rows) if r[0x18]},
        "keepzero": [i for i, r in enumerate(rows) if r[0x50]],
        "fmin": {i: raw[i]["minaccr"] for i in range(n) if fl[i] & F_FMIN},
        "direct": [i for i in range(n) if fl[i] >> 4 & 1],
        "maxstat": {i: struct.unpack_from("<H", r, 0x32)[0] for i, r in enumerate(rows)
                    if struct.unpack_from("<H", r, 0x32)[0] not in (0, 0xFFFF)},
        "saved": [i for i in range(n) if fl[i] & F_SAVED],
        "updateanimrate": [i for i in range(n) if fl[i] >> 9 & 1],
        "fcallback_count": sum(1 for f in fl if f & F_FCB),
        "damagerelated_count": sum(1 for f in fl if f & F_DMG),
        "signed_count": sum(1 for f in fl if f & 2),
        "flag_bits": sorted({k for f in fl for k in range(32) if f >> k & 1}),
    }
    ops = {}
    for r in raw:
        if r["op"]:
            ops[r["op"]] = ops.get(r["op"], 0) + 1
    got["ops"] = ops
    recs = fixup_isc(raw)
    got["targets"] = sum(1 for r in recs if r["a52"])
    ok = True
    for k, want in FILE_FACTS.items():
        if got[k] != want:
            print(f"files: {k}: {got[k]} != spec {want}")
            ok = False
    # every op used in the data has a row in stat-ops.tsv; op >= 14 never occurs
    known = load_ops()
    for o in ops:
        if o not in known:
            print(f"files: op {o} used by the data has no stat-ops.tsv row")
            ok = False
    # vectors evaluated on the real table (stats.md, Test vectors)
    r7 = recs[7]
    if r7["optable"] != [[0xFFFF, 3, 9, 0], [0xFFFF, 76, 11, 0], [12, 216, 2, 3], [0xFFFF, 270, 6, 0]]:
        print(f"files: maxhp op entries {r7['optable']}")
        ok = False
    if recs[12]["deps"] != list(range(214, 251)):
        print(f"files: level dependants {recs[12]['deps']}")
        ok = False
    # the op graph (source -> target) has no cycle, so evaluation terminates (stats.md edge case 5)
    state = {}

    def cyclic(t):
        if state.get(t) == 1:
            return True
        if state.get(t) == 2:
            return False
        state[t] = 1
        hit = any(cyclic(src) for _, src, _, _ in recs[t]["optable"])
        state[t] = 2
        return hit
    if any(cyclic(t) for t in range(n)):
        print("files: the op graph has a cycle")
        ok = False
    a53 = [i for i, r in enumerate(recs) if r["a53"]]
    if a53 != [214, 215, 218, 219]:
        print(f"files: +0x53 stats {a53}")
        ok = False
    print("files:", "pass" if ok else "FAIL", f"({n} stats, {sum(ops.values())} ops, "
          f"{got['targets']} op targets)")
    return ok


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("raw", nargs="?", help="traces/raw/<time>-stats.jsonl")
    ap.add_argument("--perturb-snap", type=int, help="change one value in snapshot N (must fail there)")
    ap.add_argument("--perturb-cb", type=int, help="change callback N's new value (must fail there)")
    ap.add_argument("--selftest", action="store_true", help="check the checker on a synthetic recording")
    ap.add_argument("--files", metavar="GAME_DIR", help="check the specs' facts on the 1.14d tables")
    a = ap.parse_args()
    if a.selftest:
        sys.exit(0 if selftest() else 1)
    if a.files:
        sys.exit(0 if files_check(a.files) else 1)
    if not a.raw:
        ap.error("a recording, --selftest or --files is required")
    records = load(a.raw)
    if records[0].get("format") != FORMAT:
        sys.exit(f"{a.raw}: not a {FORMAT} recording")
    if a.perturb_snap is not None:
        print(f"perturbed: snapshot value changed in record {perturb_snap(records, a.perturb_snap)}")
    if a.perturb_cb is not None:
        print(f"perturbed: callback value changed in record {perturb_cb(records, a.perturb_cb)}")
    m = check(records)
    sys.exit(1 if m.errors else 0)


if __name__ == "__main__":
    main()
