"""Turn one frame capture (record_frames.py, format frames-raw-3 or
frames-raw-2, specs/render/capture.md §5) into rendering facts in the
format of specs/tools/facts-render.md §1-§4 (docs/handoff/pc1-data.md
Step 3):

    py tools/trace-recorder/facts_render.py CAPTURE.jsonl --scene NAME [--frame SEQ] [--out facts/render]
    py tools/trace-recorder/facts_render.py --merge-sprites A.tsv B.tsv ... [--out facts/render]
    py tools/trace-recorder/facts_render.py --selftest

- <out>/scenes/<scene>/draws.tsv: every draw call of the chosen frame, in order (§2);
- <out>/scenes/<scene>/frame.tsv: the frame's state and its two digests (§3);
- <out>/sprites.tsv: one row per distinct (cel file, direction, frame) drawn (§4),
  merged with the file already there (union, sorted; a conflicting row for
  the same key is an error and nothing is written).

The frame is chosen by its capture sequence number `seq` (default: the last
frame with a draw log). A frames-raw-2 capture has no cel headers, unit
component paths or tile files: those cells are `?` (measured by raw-3).
Rule 1 (CLAUDE.md): measurements and digests only; no pixels, no image
content, no raw memory blobs. What a cell does not measure is `?`, never
invented; the counts are printed.

Our own code. Nothing here is derived from Blizzard code.
"""

import argparse
import hashlib
import json
import os
import sys

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
TOOL = "trace-recorder facts_render 0.2.0"
FORMATS = ("frames-raw-2", "frames-raw-3")
TILE_OPS = {"FloorTileDraw", "TileDrawLit", "TileDrawTrans", "ShadowTileDraw"}
RECT_OPS = {"UtilDiamond", "UtilRect"}
NA, UNK = "-", "?"
NO_TILE_LIGHT = False   # --tile-light unknown: the 768-byte read is not reproducible (q-facts-scenes.md)

# facts-render.md §2-§4 (the columns d2-client facts-compare reads)
DRAW_COLS = ["i", "op", "file", "dir", "frame", "tile", "x", "y", "w", "h", "xoff", "yoff",
             "mode", "light", "pal", "unit", "at"]
FRAME_KEYS = ["seq", "tick", "w", "h", "act", "level", "player_x", "player_y", "tile_origin_x",
              "tile_origin_y", "unit_origin_x", "unit_origin_y", "open_mode", "shift_x",
              "light_quality", "rain", "snow", "draws", "index_sha256", "palette_sha256"]
SPRITE_COLS = ["file", "dir", "frame", "w", "h", "xoff", "yoff"]


class FactsError(Exception):
    pass


def val(v):
    """One cell: integers in decimal, None as `?` (§1 r2)."""
    if v is None:
        return UNK
    if isinstance(v, bool):
        return str(int(v))
    return str(v)


def hex32(v):
    return UNK if v is None else f"{v & 0xFFFFFFFF:#x}"


def canon(path, ext=".dc6"):
    """§1 r3: lowercase, `/`, no leading `/`, with the extension (the cel loader's
    paths have none: DC6, capture.md §3.6)."""
    if not path:
        return None
    p = path.replace("\\", "/").lower().lstrip("/")
    return p if os.path.splitext(p)[1] in (".dc6", ".dcc", ".dt1") else p + ext


def comp_name(tokens):
    """unit-composite.md §6 r1: unit + component + armor class + mode + weapon class,
    each up to 3 characters, stopping at the first space."""
    if not tokens or any(not t for t in tokens):
        return None
    return "".join(t[:3].split(" ")[0] for t in tokens)


def cel_file(c, celfiles, compfiles):
    """§2 r3: the cel file of a context: the cel loader's path for +0x34, else the unit
    component file named by its tokens (raw-3 `compfile`)."""
    if c.get("file") not in (None, "0x0"):
        return canon(celfiles.get(c["file"]))
    name = comp_name(c.get("tokens"))
    return canon(compfiles.get(name.lower())) if name else None


def draw_row(i, d, celfiles, compfiles, sprites):
    """One draws.tsv row (§2) and, for a cel with a header, its sprites.tsv row."""
    op, a = d["op"], d.get("a") or []
    r = dict.fromkeys(DRAW_COLS, NA)
    r.update(i=str(i), op=op, at=d.get("at") or NA)
    sprite = None
    if op == "unit":  # §2 r5: ECX unit, EDX light, stack client x, y
        u = d.get("u") or {}
        r.update(x=val(a[0] if a else None), y=val(a[1] if len(a) > 1 else None),
                 light=hex32(d.get("light")),
                 unit=f"{u['type']}:{u['guid']}" if "type" in u and "guid" in u else UNK)
    elif op in TILE_OPS:  # §2 r4
        t = d.get("tile") or {}
        r.update(file=val(canon(t.get("dt1"), ".dt1")), frame=val(t.get("index")),
                 tile=(".".join(str(t[k]) for k in ("orient", "main", "sub", "rarity"))
                       if all(k in t for k in ("orient", "main", "sub", "rarity")) else UNK))
        if op == "FloorTileDraw":  # light grid, X, Y, world x, world y, alpha, open mode, data
            r.update(x=val(a[1]), y=val(a[2]), light=UNK if NO_TILE_LIGHT else d.get("light") or UNK)
        else:  # X, Y, light, open mode (+ alpha for TileDrawTrans); shadow: no light
            r.update(x=val(a[0]), y=val(a[1]))
            if op != "ShadowTileDraw":
                r["light"] = UNK if NO_TILE_LIGHT else d.get("light") or UNK
            if op == "TileDrawTrans":
                r["mode"] = val(a[4] if len(a) > 4 else None)
    elif "cel" in d:  # §2 r3: cel context, X, Y, ...
        c = d["cel"] or {}
        f = cel_file(c, celfiles, compfiles)
        h = c.get("hdr")
        r.update(file=val(f), dir=val(c.get("dir")), frame=val(c.get("frame")),
                 x=val(a[0] if a else None), y=val(a[1] if len(a) > 1 else None))
        if f and h:
            sprite = [f, str(c["dir"]), str(c["frame"]), str(h["w"]), str(h["h"]),
                      str(h["xoff"]), str(h["yoff"])]
        size = sprites.get((f, c.get("dir"), c.get("frame"))) if f else None
        if sprite:
            size = sprite[3:]
        r.update(zip(("w", "h", "xoff", "yoff"), size or [UNK] * 4))
        if op == "CelDraw":  # (X, Y, light, mode, palette): capture.md §8 cursor vector
            r.update(light=hex32(a[2] if len(a) > 2 else None), mode=val(a[3] if len(a) > 3 else None),
                     pal="0" if len(a) > 4 and a[4] == 0 else UNK)
        else:  # argument positions of the other cel wrappers are not specified
            r.update(mode=UNK, light=UNK, pal=UNK)
    elif op in RECT_OPS:  # §2 r6: RECT*, color
        rect = d.get("rect")
        r.update(x=val(rect[0] if rect else None), y=val(rect[1] if rect else None),
                 mode=val(a[0] if a else None))
    else:  # StartDraw, ClearScreen, DrawBox, DrawBoxAlpha, DrawLine: §2 r6
        r.update(x=val(a[0] if a else None), y=val(a[1] if len(a) > 1 else None) if len(a) > 1 else NA)
        if op in ("DrawLine", "DrawBox"):  # x0, y0, x1, y1, color, alpha / mode: blend-modes.md §8
            r["mode"] = val(a[4] if len(a) > 4 else None)
        elif op != "StartDraw" and op != "ClearScreen":
            r["mode"] = UNK  # DrawBoxAlpha: which argument is the color is not specified
    return [r[c] for c in DRAW_COLS], sprite


def frame_rows(f, draws, png_dir):
    """frame.tsv (§3)."""
    p, lv, li, we = f.get("player") or {}, f.get("level") or {}, f.get("light") or {}, f.get("weather") or {}
    idx, pal = f.get("index_sha256"), f.get("palette_sha256")
    if (not idx or not pal) and f.get("image") and png_dir:  # digests from the saved PNG (§6)
        import record_frames  # noqa: E402  (png_indices: the inverse of its own PNG writer)
        with open(os.path.join(png_dir, f["image"]), "rb") as fh:
            _, _, pixels, palette = record_frames.png_indices(fh.read())
        idx, pal = hashlib.sha256(pixels).hexdigest(), hashlib.sha256(palette).hexdigest()
    client = p.get("client") or [None, None]
    to, uo = f.get("tile_origin") or [None, None], f.get("unit_origin") or [None, None]
    v = {"seq": f["seq"], "tick": f.get("f"), "w": f.get("w"), "h": f.get("h"),
         "act": lv.get("act"), "level": lv.get("level_id"), "player_x": client[0],
         "player_y": client[1], "tile_origin_x": to[0], "tile_origin_y": to[1],
         "unit_origin_x": uo[0], "unit_origin_y": uo[1], "open_mode": f.get("open_mode"),
         "shift_x": f.get("shift_x"), "light_quality": li.get("quality"), "rain": we.get("rain"),
         "snow": we.get("snow"), "draws": len(draws), "index_sha256": idx, "palette_sha256": pal}
    return [[k, val(v[k])] for k in FRAME_KEYS]


def read_capture(path, seq):
    """(frame record, celfile map, compfile map, image directory name) of the chosen frame."""
    with open(path, encoding="utf-8") as fh:
        recs = [json.loads(line) for line in fh if line.strip()]
    if not recs or recs[0].get("k") != "header" or recs[0].get("format") not in FORMATS:
        raise FactsError(f"{path}: not a {' / '.join(FORMATS)} capture")
    cap = next((r for r in recs if r.get("k") == "capture"), {})
    frames = [(i, r) for i, r in enumerate(recs) if r.get("k") == "frame" and "refused" not in r]
    frames = [x for x in frames if (x[1].get("draws") if seq is None else x[1]["seq"] == seq)]
    if not frames:
        raise FactsError("no frame with a draw log" if seq is None else f"no captured frame seq {seq}")
    at, f = frames[-1]
    if not f.get("draws"):
        raise FactsError(f"frame seq {f['seq']} has no draw log (record with --draws-every N)")
    celfiles, compfiles = {}, {}  # as loaded before this frame (capture.md §3.6)
    for r in recs[:at]:
        if r.get("k") == "celfile":
            celfiles[r["ptr"]] = r["path"]
        elif r.get("k") == "compfile":
            compfiles[r["name"].lower()] = r["path"]
    return f, celfiles, compfiles, cap.get("images")


def sort_key(row):
    return (row[0].encode(), int(row[1]), int(row[2]))


def read_sprites(path):
    """Existing sprites.tsv rows by key (§4)."""
    rows = {}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            lines = fh.read().split("\n")
        if len(lines) < 2 or not lines[0].startswith("# facts v1; ") or lines[1].split("\t") != SPRITE_COLS:
            raise FactsError(f"{path}: not a facts v1 sprites.tsv")
        for line in lines[2:]:
            if line:
                row = line.split("\t")
                rows[(row[0], int(row[1]), int(row[2]))] = row
    return rows


def merge_sprites(paths, out_path):
    """Union of several sprites.tsv files into out_path (exit 1 on a conflicting key)."""
    merged, conflicts = {}, []
    try:
        for p in paths:
            if not os.path.exists(p):
                raise FactsError(f"{p}: no such file")
            for key, row in read_sprites(p).items():
                old = merged.setdefault(key, row)
                if old != row:
                    conflicts.append(f"{key}: {old} vs {row} ({p})")
    except (FactsError, OSError, ValueError) as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
    if conflicts:
        for c in conflicts:
            print(f"error: conflicting sprite row {c}", file=sys.stderr)
        sys.exit(1)
    header = f"# facts v1; tool: {TOOL}; command: {command_line()}; game: 1.14d"
    write_tsv(out_path, header, SPRITE_COLS, sorted(merged.values(), key=sort_key))
    print(f"sprites.tsv {len(merged)} rows from {len(paths)} files")


def build(f, celfiles, compfiles, old_sprites):
    """(draw rows, merged sprite rows, conflicts) of one frame."""
    merged, conflicts = dict(old_sprites), []
    for i, d in enumerate(f["draws"]):  # pass 1: this frame's measured cels join sprites.tsv
        _, sprite = draw_row(i, d, celfiles, compfiles, {})
        if sprite:
            key = (sprite[0], int(sprite[1]), int(sprite[2]))
            old = merged.setdefault(key, sprite)
            if old != sprite:
                conflicts.append(f"{key}: existing {old}, new {sprite}")
    # pass 2 (§2 r3): every cel row takes its sprite's row, also a draw without its own
    # header (e.g. CelDrawShadow, which does not reach the rasterizer of capture.md §3.5)
    lookup = {k: v[3:] for k, v in merged.items()}
    draws = [draw_row(i, d, celfiles, compfiles, lookup)[0] for i, d in enumerate(f["draws"])]
    return draws, sorted(merged.values(), key=sort_key), conflicts


def write_tsv(path, header, cols, rows):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\n".join([header, "\t".join(cols)] + ["\t".join(r) for r in rows]) + "\n")


def command_line():
    def q(s):
        return f'"{s}"' if (" " in s or not s) else s
    script = os.path.relpath(os.path.abspath(sys.argv[0]), REPO).replace("\\", "/")
    args = [a.replace(";", ",") for a in sys.argv[1:]]  # `;` separates header fields
    return " ".join(["py", q(script)] + [q(a) for a in args])


def selftest():
    """Rows of a hand-made raw-3 frame, and M08 perturbations of its source fields."""
    f = {"seq": 3, "f": 40, "w": 800, "h": 600, "player": {"client": [10320, 72816], "dir": 0},
         "level": {"act": 0, "level_id": 1}, "tile_origin": [1, 2], "unit_origin": [3, 4],
         "open_mode": 0, "shift_x": 0, "light": {"quality": 2}, "weather": {"rain": 0, "snow": 0},
         "index_sha256": "a" * 64, "palette_sha256": "b" * 64, "draws": [
             {"op": "StartDraw", "a": [1, 0, 0, 0], "at": "0x44ca00"},
             {"op": "FloorTileDraw", "a": [99, 800, -56, 4865, 4195, 255, 0, 1], "light": "33f326139a413dd6",
              "tile": {"orient": 0, "main": 1, "sub": 2, "rarity": 0, "dt1": "data\\global\\tiles\\ACT1\\Town\\floor.dt1",
                       "index": 7}, "at": "0x4dea00"},
             {"op": "unit", "u": {"type": 0, "guid": 1}, "light": -1, "a": [400, 292, 0, 0], "at": "0x4dc800"},
             {"op": "CelDraw", "a": [400, 292, -1, 5, 0], "at": "0x4dbc00",
              "cel": {"ctx": "0x10", "file": "0x0", "dir": 0, "frame": 0,
                      "tokens": ["AM  ", "LG  ", "lit ", "TN  ", "1ht "],
                      "hdr": {"flip": 0, "w": 18, "h": 46, "xoff": -13, "yoff": 7}}},
             {"op": "CelDraw", "a": [320, 240, -1, 5, 0], "at": "0x4ff000",
              "cel": {"ctx": "0x20", "file": "0x4740000", "dir": 0, "frame": 0, "tokens": [None] * 5,
                      "hdr": {"flip": 0, "w": 30, "h": 27, "xoff": -1, "yoff": 24}}},
             {"op": "CelDrawShadow", "a": [400, 292], "at": "0x4dbd00",
              "cel": {"ctx": "0x10", "file": "0x0", "dir": 0, "frame": 0,
                      "tokens": ["AM  ", "LG  ", "lit ", "TN  ", "1ht "]}},
             {"op": "DrawLine", "a": [596, 151, 595, 156, 185, 127], "at": "0x47368e"},
             {"op": "CelDrawEx", "a": [29, 587, 0, 80, 5], "at": "0x4ff100",
              "cel": {"ctx": "0x30", "file": "0x4740000", "dir": 0, "frame": 1, "tokens": [None] * 5}},
         ]}
    celfiles = {"0x4740000": "DATA\\GLOBAL\\UI\\CURSOR\\protate"}
    compfiles = {"amlglittn1ht": "DATA\\GLOBAL\\CHARS\\AM\\LG\\AMLGlitTN1ht.dcc"}
    draws, sprites, conflicts = build(f, celfiles, compfiles, {})
    assert not conflicts
    want = [
        ["0", "StartDraw", "-", "-", "-", "-", "1", "0", "-", "-", "-", "-", "-", "-", "-", "-", "0x44ca00"],
        ["1", "FloorTileDraw", "data/global/tiles/act1/town/floor.dt1", "-", "7", "0.1.2.0", "800", "-56",
         "-", "-", "-", "-", "-", "33f326139a413dd6", "-", "-", "0x4dea00"],
        ["2", "unit", "-", "-", "-", "-", "400", "292", "-", "-", "-", "-", "-", "0xffffffff", "-", "0:1",
         "0x4dc800"],
        ["3", "CelDraw", "data/global/chars/am/lg/amlglittn1ht.dcc", "0", "0", "-", "400", "292", "18", "46",
         "-13", "7", "5", "0xffffffff", "0", "-", "0x4dbc00"],
        ["4", "CelDraw", "data/global/ui/cursor/protate.dc6", "0", "0", "-", "320", "240", "30", "27", "-1",
         "24", "5", "0xffffffff", "0", "-", "0x4ff000"],
        ["5", "CelDrawShadow", "data/global/chars/am/lg/amlglittn1ht.dcc", "0", "0", "-", "400", "292", "18",
         "46", "-13", "7", "?", "?", "?", "-", "0x4dbd00"],
        ["6", "DrawLine", "-", "-", "-", "-", "596", "151", "-", "-", "-", "-", "185", "-", "-", "-", "0x47368e"],
        ["7", "CelDrawEx", "data/global/ui/cursor/protate.dc6", "0", "1", "-", "29", "587", "?", "?", "?", "?",
         "?", "?", "?", "-", "0x4ff100"],
    ]
    assert draws == want, "\n".join("\t".join(r) for r in draws)
    assert [s[0] for s in sprites] == ["data/global/chars/am/lg/amlglittn1ht.dcc",
                                       "data/global/ui/cursor/protate.dc6"], sprites
    fr = dict(frame_rows(f, draws, None))
    assert (fr["player_x"], fr["level"], fr["draws"], fr["light_quality"]) == ("10320", "1", "8", "2"), fr
    # M08: one changed source field changes exactly its cell
    cases = [(lambda g: g["draws"][1]["tile"].__setitem__("rarity", 1), 1, "tile"),
             (lambda g: g["draws"][1]["tile"].__setitem__("index", 8), 1, "frame"),
             (lambda g: g["draws"][1]["tile"].__setitem__("sub", 3), 1, "tile"),
             (lambda g: g["draws"][4]["a"].__setitem__(3, 4), 4, "mode"),
             (lambda g: g["draws"][2]["u"].__setitem__("guid", 2), 2, "unit"),
             (lambda g: g["draws"][6]["a"].__setitem__(4, 186), 6, "mode")]
    for change, row, col in cases:
        g = json.loads(json.dumps(f))
        change(g)
        d2, _, _ = build(g, celfiles, compfiles, {})
        diff = [(i, DRAW_COLS[j]) for i in range(len(want)) for j in range(len(DRAW_COLS))
                if d2[i][j] != want[i][j]]
        assert diff == [(row, col)], (row, col, diff)
    # the measured header reaches every draw of that sprite (the shadow row too)
    g = json.loads(json.dumps(f))
    g["draws"][3]["cel"]["hdr"]["yoff"] = 8
    d2, _, _ = build(g, celfiles, compfiles, {})
    assert (d2[3][11], d2[5][11]) == ("8", "8") and d2[4] == want[4], (d2[3], d2[5])
    # a sprite seen twice with different headers is a conflict
    g = json.loads(json.dumps(f))
    g["draws"][4]["cel"]["hdr"]["w"] = 31
    _, _, conflicts = build(g, celfiles, compfiles, {k: v for k, v in
                                                     ((tuple([s[0], int(s[1]), int(s[2])]), s) for s in sprites)})
    assert len(conflicts) == 1, conflicts
    # --merge-sprites: a union of two files; a key with two rows is reported, nothing written
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        hdr = "# facts v1; selftest"
        a_p, b_p, out = (os.path.join(tmp, n) for n in ("a.tsv", "b.tsv", "out.tsv"))
        write_tsv(a_p, hdr, SPRITE_COLS, [sprites[0]])
        write_tsv(b_p, hdr, SPRITE_COLS, sprites)
        merge_sprites([a_p, b_p], out)
        assert list(read_sprites(out).values()) == sprites, read_sprites(out)
        bad = list(sprites[0][:3]) + ["99"] + list(sprites[0][4:])
        write_tsv(b_p, hdr, SPRITE_COLS, [bad])
        os.remove(out)
        try:
            merge_sprites([a_p, b_p], out)
            raise AssertionError("conflicting merge passed")
        except SystemExit as e:
            assert e.code == 1 and not os.path.exists(out)
    print("selftest ok: rows of every draw kind follow facts-render.md §2-§4; "
          f"{len(cases)} source fields each change exactly their cell; a sprite conflict is reported (build and --merge-sprites)")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("capture", nargs="?", help="a frames-raw-3 (or -2) .jsonl from record_frames.py")
    ap.add_argument("--scene", help="scene name: the directory under scenes/")
    ap.add_argument("--frame", type=int, default=None, help="frame seq (default: last with draws)")
    ap.add_argument("--out", default=os.path.join(REPO, "facts", "render"))
    ap.add_argument("--images", default=os.path.join(REPO, "game", "captures"),
                    help="PNG root, used only when a frame lacks its digests")
    ap.add_argument("--tile-light", choices=("digest", "unknown"), default="digest",
                    help="tile rows' light column: the recorder's digest, or `?` (the 768-byte floor light "
                         "read differs between two runs of the same scene: bytes the game does not set)")
    ap.add_argument("--merge-sprites", nargs="+", metavar="SPRITES_TSV",
                    help="write <out>/sprites.tsv as the union of these sprites.tsv files (§4; a key with two "
                         "different rows is a conflict): joins the sprite tables of two branches' recordings")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args()
    global NO_TILE_LIGHT
    NO_TILE_LIGHT = a.tile_light == "unknown"
    if a.selftest:
        selftest()
        return
    if a.merge_sprites:
        merge_sprites(a.merge_sprites, os.path.join(a.out, "sprites.tsv"))
        return
    if not a.capture or not a.scene:
        ap.error("CAPTURE and --scene are required")
    header = f"# facts v1; tool: {TOOL}; command: {command_line()}; game: 1.14d"
    sprites_path = os.path.join(a.out, "sprites.tsv")
    try:
        f, celfiles, compfiles, images = read_capture(a.capture, a.frame)
        draws, sprites, conflicts = build(f, celfiles, compfiles, read_sprites(sprites_path))
    except (FactsError, OSError, ValueError, KeyError) as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
    if conflicts:
        for c in conflicts:
            print(f"error: conflicting sprite row {c}", file=sys.stderr)
        sys.exit(1)
    png_dir = os.path.join(a.images, images) if images else None
    scene = os.path.join(a.out, "scenes", a.scene)
    os.makedirs(scene, exist_ok=True)
    write_tsv(os.path.join(scene, "draws.tsv"), header, DRAW_COLS, draws)
    write_tsv(os.path.join(scene, "frame.tsv"), header, ["key", "value"], frame_rows(f, draws, png_dir))
    write_tsv(sprites_path, header, SPRITE_COLS, sprites)
    unk = {c: sum(1 for r in draws if r[j] == UNK) for j, c in enumerate(DRAW_COLS)}
    print(f"frame seq {f['seq']} (tick {f.get('f')}): {len(draws)} draws; sprites.tsv {len(sprites)} rows; "
          f"wrote {scene}")
    print("unmeasured (?) cells per column: " + ", ".join(f"{c} {n}" for c, n in unk.items() if n))


if __name__ == "__main__":
    main()
