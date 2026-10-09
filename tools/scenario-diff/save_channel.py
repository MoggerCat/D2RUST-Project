"""The `save` channel of scenario_diff.py (specs/tools/scenario-diff.md §3 rule 13,
specs/formats/d2s.md): at the end of the check both sides leave the game
through Save and Exit (C->S 0x69 injected as the check's last `send`, so the
frame is the same on both sides) and the `.d2s` each one wrote is compared
byte by byte. 1.14d runs without `-nosave` (`record_state.py --write-save`)
and writes the file into its save folder; d2rs runs `d2-client state-dump
--save-out FILE`, whose server uses `play`'s character writer.

The one value that is wall-clock on both sides, the save time at +0x30
(`formats/d2s.md` §2.1), and the checksum that covers it are normalised
(the field zeroed, the checksum recomputed per `formats/d2s.md` §3) after
each file's own checksum was verified; every other byte must be equal.

Standalone: `py save_channel.py ORIG.d2s D2RS.d2s [--json F]` compares two files
(exit 0 match, 1 diverged, 3 error); `--selftest` needs no game.

Our own code. Standard library only.
"""

import argparse
import json
import os
import shutil
import struct
import sys
import tempfile

MAGIC = 0xAA55AA55
SIZE_AT, SUM_AT, TIME_AT = 0x08, 0x0C, 0x30
VERDICTS = {0: "MATCH", 1: "DIVERGED", 2: "PARTIAL", 3: "ERROR"}

# header fields (formats/d2s.md §2.1): (offset, size, name)
HEADER = [
    (0x00, 4, "magic"), (0x04, 4, "version"), (0x08, 4, "file size"), (0x0C, 4, "checksum"),
    (0x10, 4, "weapon switch"), (0x14, 16, "name"), (0x24, 2, "status"), (0x26, 2, "unused"),
    (0x28, 1, "class"), (0x29, 1, "stat count"), (0x2A, 1, "skill count"), (0x2B, 1, "level"),
    (0x2C, 4, "create time"), (0x30, 4, "save time"), (0x34, 4, "unused 0x34"),
    (0x38, 64, "hotkeys"), (0x78, 4, "left skill"), (0x7C, 4, "right skill"),
    (0x80, 4, "swap left skill"), (0x84, 4, "swap right skill"),
    (0x88, 16, "appearance components"), (0x98, 16, "appearance colours"),
    (0xA8, 3, "town per difficulty"), (0xAB, 4, "map seed"), (0xAF, 32, "hireling block"),
    (0xCF, 1, "client byte"), (0xD0, 127, "reserved"),
]
# fixed sections after the header (§4-§6) and the variable ones' markers (§1 rule 5)
FIXED = [(0x14F, 0x279, "quests"), (0x279, 0x2C9, "waypoints"), (0x2C9, 0x2FD, "npc flags")]
MARKERS = [(b"gf", "stats"), (b"if", "skills"), (b"JM", "items"), (b"jf", "hireling items"),
           (b"kf", "golem item")]


class SaveError(Exception):
    pass


def checksum(buf):
    """formats/d2s.md §3: s = rotl(s, 1) + byte, over the file with +0x0C = 0."""
    s = 0
    for b in buf:
        s = (((s << 1) | (s >> 31)) + b) & 0xFFFFFFFF
    return s


def check_file(buf):
    """(size, stored checksum, computed checksum) after the framing checks."""
    if len(buf) < 0x2FF:
        raise SaveError(f"{len(buf)} bytes: shorter than the fixed sections")
    magic, = struct.unpack_from("<I", buf, 0)
    if magic != MAGIC:
        raise SaveError(f"magic {magic:#010x}, not {MAGIC:#010x}")
    size, stored = struct.unpack_from("<II", buf, SIZE_AT)
    z = bytearray(buf)
    z[SUM_AT:SUM_AT + 4] = bytes(4)
    return size, stored, checksum(z)


def normalised(buf):
    """The file with the save time zeroed and the checksum recomputed."""
    z = bytearray(buf)
    z[TIME_AT:TIME_AT + 4] = bytes(4)
    z[SUM_AT:SUM_AT + 4] = bytes(4)
    z[SUM_AT:SUM_AT + 4] = struct.pack("<I", checksum(z))
    return bytes(z)


def layout(buf):
    """[(start, end, name)] of the file's regions: header fields, fixed
    sections, then the variable ones by the first marker found in order
    (a label only; the comparison is byte by byte)."""
    out = [(o, o + n, f"header {name}") for o, n, name in HEADER]
    out += [(a, b, name) for a, b, name in FIXED]
    pos = 0x2FD
    at = []
    for mark, name in MARKERS:
        i = buf.find(mark, pos if buf[pos:pos + 2] != mark else pos)
        if i < 0:
            break
        at.append((i, name))
        pos = i + 2
    for k, (i, name) in enumerate(at):
        end = at[k + 1][0] if k + 1 < len(at) else len(buf)
        out.append((i, end, name))
    return out


def where(buf, off):
    """Name of the region holding byte `off`."""
    for a, b, name in layout(buf):
        if a <= off < b:
            return f"{name} +{off - a:#x}"
    return "after the last region" if off >= 0x2FD else "unnamed header byte"


def hexs(b):
    return " ".join(f"{x:02x}" for x in b) or "(none)"


def compare(orig, d2rs):
    """(code, report) of two files: 0 equal under the normalisation, 1 not."""
    notes, flags = [], []
    for side, buf in (("1.14d", orig), ("d2rs", d2rs)):
        try:
            size, stored, calc = check_file(buf)
        except SaveError as e:
            if side == "1.14d":
                raise
            return 1, {"first": f"d2rs file invalid: {e}", "differences": None,
                       "bytes": (len(orig), len(d2rs)), "notes": notes, "offsets": []}
        if size != len(buf):
            flags.append(f"{side} file size field {size} != {len(buf)} bytes")
        if stored != calc:
            flags.append(f"{side} checksum {stored:#010x} != computed {calc:#010x}")
    t = [struct.unpack_from("<I", b, TIME_AT)[0] for b in (orig, d2rs)]
    notes.append(f"save time (+0x30, wall clock, normalised): 1.14d {t[0]}, d2rs {t[1]}")
    a, b = normalised(orig), normalised(d2rs)
    n = min(len(a), len(b))
    # the checksum follows from the other bytes (each file's own was verified above)
    offs = [i for i in range(n) if a[i] != b[i] and not SUM_AT <= i < SUM_AT + 4]
    if len(a) != len(b):
        offs += list(range(n, max(len(a), len(b))))
    first = None
    if offs:
        i = offs[0]
        who = orig if i < len(orig) else d2rs
        first = (f"byte {i:#06x} ({where(who, i)}): 1.14d {hexs(a[i:i + 4]) if i < len(a) else 'EOF'}"
                 f" vs d2rs {hexs(b[i:i + 4]) if i < len(b) else 'EOF'};"
                 f" {len(offs)} of {max(len(a), len(b))} bytes differ"
                 + (f"; sizes {len(a)} vs {len(b)}" if len(a) != len(b) else ""))
    elif flags:
        first = flags[0]
    return (1 if offs or flags else 0), {
        "first": first, "differences": len(offs), "bytes": (len(orig), len(d2rs)),
        "notes": notes + flags, "offsets": offs[:64]}


def summary(code, rep):
    n = max(rep["bytes"])
    d = rep["differences"] or 0
    return {"format": "diff-summary-1", "channel": "save", "tool": "save_channel.py",
            "code": code, "verdict": VERDICTS[code], "rows_compared": n, "rows_equal": n - d,
            "frames_compared": 1, "frames_equal": 0 if code else 1,
            "differences": d, "bytes_orig": rep["bytes"][0], "bytes_d2rs": rep["bytes"][1],
            "first": {"frame": None, "text": rep["first"]} if rep["first"] else None}


def report(orig_path, d2rs_path, json_path=None):
    """Compares two files, prints the report, returns the exit code."""
    try:
        with open(orig_path, "rb") as f:
            orig = f.read()
        with open(d2rs_path, "rb") as f:
            d2rs = f.read()
        code, rep = compare(orig, d2rs)
    except (OSError, SaveError) as e:
        print(f"[save] ERROR: {e}")
        if json_path:
            _write(json_path, {"format": "diff-summary-1", "channel": "save", "code": 3,
                               "verdict": "ERROR", "first": None, "error": str(e)})
        return 3
    print(f"[save] 1.14d {rep['bytes'][0]} bytes, d2rs {rep['bytes'][1]} bytes")
    for n in rep["notes"]:
        print(f"[save] note: {n}")
    if code == 0:
        print(f"[save] MATCH: {rep['bytes'][0]} bytes equal (save time and checksum normalised)")
    else:
        print(f"[save] DIVERGED: {rep['first']}")
        for i in rep["offsets"][:8]:
            print(f"[save]   {i:#06x} {where(orig if i < len(orig) else d2rs, i)}")
    if json_path:
        _write(json_path, summary(code, rep))
    return code


def _write(path, obj):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=1)
        f.write("\n")


# --- the channel (scenario_diff.py's Runner) ----------------------------------

def run(r, save, sides):
    """One save run per side asked; the comparator's exit code when both ran
    (None when one side only). `r` is scenario_diff's Runner. Every channel
    that follows starts from `save`, so the 1.14d save folder gets it back."""
    c = r.c
    orig, d2rs = r.path("orig.saved.d2s"), r.path("d2rs.saved.d2s")
    last = c["ticks"] + 1   # the frame whose drain runs Save and Exit
    leave = (last, "hex 69")
    if "orig" in sides:
        folder = r.save_dir_orig()
        written = os.path.join(folder, c["char"] + ".d2s")
        try:
            r.recorder("record_state.py", ["--snap-every", "1", "--write-save", "--save-watch",
                                           written], r.path("orig.save-run.jsonl"),
                       ticks=last, sends=[leave])
            if not r.dry:
                _take_orig(save, written, orig)
        finally:
            if not r.dry and os.path.exists(save):
                shutil.copyfile(save, written)   # the next channel's start
    if "d2rs" in sides and not (r.reuse and os.path.exists(d2rs)):
        args = ["state-dump"] + r.d2rs_common(save) + [
            "--ticks", str(last), "--out", r.path("d2rs.save-run.jsonl"), "--save-out", d2rs,
            "--send", f"{last} {leave[1]}"]
        if r.d2rs_input() and not r.shared_error(r.d2rs_input()):
            args += ["--input", r.d2rs_input()]
        if not r.dry and os.path.exists(d2rs):
            os.unlink(d2rs)
        r.cargo("d2-client", args)
        if not r.dry and not os.path.exists(d2rs):
            raise _missing("d2rs", "d2-client state-dump --save-out")
    if sides != {"orig", "d2rs"}:
        return None
    if r.dry:
        r.log.append(f"save_channel.py {orig} {d2rs} --json {r.path('save.summary.json')}")
        return 0
    return report(orig, d2rs, r.path("save.summary.json"))


def _missing(side, tool):
    return RuntimeError(f"{tool} wrote no save for {side}: Save and Exit did not run")


def _take_orig(before, written, dest):
    """Copies the file 1.14d wrote; refuses the unchanged start file."""
    if not os.path.exists(written):
        raise _missing("1.14d", "record_state.py --write-save")
    with open(before, "rb") as f, open(written, "rb") as g:
        if f.read() == g.read():
            raise _missing("1.14d", "record_state.py --write-save (file unchanged)")
    shutil.copyfile(written, dest)


def selftest(runner_cls, check, shared_script_error):
    """Dry run of the channel and the comparator on synthetic files.
    Returns the number of checks passed."""
    n = 0
    r = runner_cls(dict(check, poke=[(1, 5, "time 1 0")], send=[(2, 7, "Walk x=10 y=20")],
                        input={}), "/tmp/w", dry=True)
    r.next = 5
    r.shared_error = shared_script_error
    code = run(r, "/tmp/w/ScnAma.d2s", {"orig", "d2rs"})
    rec = next(x for x in r.log if "record_state.py" in x)
    assert "--ticks 21" in rec and "--write-save" in rec and "--snap-every 1" in rec, rec
    assert "--save-watch" in rec and "ScnAma.d2s" in rec, rec
    assert "--send '21 hex 69'" in rec and "--send '7 Walk x=10 y=20'" in rec, rec
    assert "--auto ScnAma --seed 1234" in rec, rec
    dump = next(x for x in r.log if "state-dump" in x)
    assert "--ticks 21" in dump and "--save-out /tmp/w/d2rs.saved.d2s" in dump, dump
    assert "--send '21 hex 69'" in dump and "--send '7 Walk x=10 y=20'" in dump, dump
    assert "--seed 1234" in dump, dump
    assert "save_channel.py /tmp/w/orig.saved.d2s /tmp/w/d2rs.saved.d2s" in r.log[-1], r.log[-1]
    assert code == 0
    n += 1
    r.log = []
    r.reuse = False
    assert run(r, "/tmp/w/ScnAma.d2s", {"d2rs"}) is None   # one side: no compare
    assert not any("record_state" in x or "save_channel" in x for x in r.log)
    n += 1
    return n + selftest_compare()


def fake_save(extra=b"", time=1000, level=1, name=b"Sav"):
    """A synthetic file of the right framing (335 + sections + a tail)."""
    b = bytearray(0x2FD)
    struct.pack_into("<II", b, 0, MAGIC, 0x60)
    b[0x14:0x14 + len(name)] = name
    b[0x2B] = level
    struct.pack_into("<I", b, TIME_AT, time)
    b[0x14F:0x153] = b"Woo!"
    b[0x279:0x27B] = b"WS"
    b[0x2C9:0x2CB] = b"\x01\x77"
    b += b"gf" + bytes(8) + b"if" + bytes(30) + b"JM\x00\x00" + extra + b"jf" + b"kf"
    struct.pack_into("<I", b, SIZE_AT, len(b))
    struct.pack_into("<I", b, SUM_AT, checksum(b))
    return bytes(b)


def selftest_compare():
    """The comparator on synthetic files: equal under the time normalisation,
    each kind of difference found at its first byte and region."""
    a = fake_save()
    code, rep = compare(a, fake_save(time=99999))
    assert code == 0 and rep["differences"] == 0, rep   # the save time alone is normalised
    b = bytearray(fake_save())
    b[0x2B] = 2                                          # level
    struct.pack_into("<I", b, SUM_AT, 0)
    struct.pack_into("<I", b, SUM_AT, checksum(b))
    code, rep = compare(a, bytes(b))
    assert code == 1 and "byte 0x002b (header level +0x0)" in rep["first"], rep
    code, rep = compare(a, fake_save(extra=b"\x01\x02"))
    assert code == 1 and "sizes" in rep["first"], rep    # a longer items section
    bad = bytearray(a)
    bad[0x30] ^= 1                                       # time changed, checksum stale
    code, rep = compare(a, bytes(bad))
    assert code == 1 and any("checksum" in x for x in rep["notes"]), rep
    for broken in (b"", bytes(0x400)):                   # d2rs side invalid: a divergence
        code, rep = compare(a, broken)
        assert code == 1 and "d2rs file invalid" in rep["first"], rep
    try:
        compare(b"", a)
        raise AssertionError("an invalid 1.14d file must be an error")
    except SaveError:
        pass
    d = tempfile.mkdtemp()
    try:
        po, pd, pj = (os.path.join(d, x) for x in ("o", "d", "j"))
        for p, v in ((po, a), (pd, fake_save(time=5))):
            with open(p, "wb") as f:
                f.write(v)
        assert report(po, pd, pj) == 0
        with open(pj, encoding="utf-8") as f:
            sm = json.load(f)
        assert sm["verdict"] == "MATCH" and sm["rows_equal"] == sm["rows_compared"], sm
        assert report(po, os.path.join(d, "none"), pj) == 3
    finally:
        shutil.rmtree(d)
    return 7


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("orig", nargs="?")
    ap.add_argument("d2rs", nargs="?")
    ap.add_argument("--json", default=None)
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    if a.selftest:
        print(f"selftest ok: {selftest_compare()} comparator checks")
        return 0
    if not (a.orig and a.d2rs):
        ap.error("two .d2s files")
    return report(a.orig, a.d2rs, a.json)


if __name__ == "__main__":
    sys.exit(main())
