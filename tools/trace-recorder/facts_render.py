"""Turn one frame capture (record_frames.py, format frames-raw-2,
specs/render/capture.md §5) into rendering facts (docs/handoff/pc1-data.md
Step 3):

    py tools/trace-recorder/facts_render.py CAPTURE.jsonl --scene NAME [--frame SEQ] [--out facts/render]

- <out>/scenes/<scene>/draws.tsv: every draw call of the chosen frame, in order;
- <out>/scenes/<scene>/frame.tsv: the frame's state and its two digests;
- <out>/sprites.tsv: one row per distinct (cel file, direction, frame) drawn,
  merged with the file already there (union, sorted; a conflicting row for
  the same key is an error and nothing is written).

The frame is chosen by its capture sequence number `seq` (default: the last
frame with a draw log). Rule 1 (CLAUDE.md): measurements and digests only;
no pixels, no image content, no raw memory blobs (cel contexts are reduced
to their decoded fields, tile light arrays to a digest). Fields the raw
format does not record are listed in a `# missing:` line, never invented.

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
TOOL = "trace-recorder facts_render 0.1.0"
FORMAT = "frames-raw-2"
TILE_OPS = {"FloorTileDraw", "TileDrawLit", "TileDrawTrans", "ShadowTileDraw"}
RECT_OPS = {"UtilDiamond", "UtilRect"}
PRIM_OPS = {"StartDraw", "ClearScreen", "DrawBox", "DrawBoxAlpha", "DrawLine"}

DRAW_COLS = ["i", "op", "at", "kind", "cel_file", "cel_path", "dir", "frame", "tokens", "x", "y",
             "world", "light", "alpha", "open_mode", "tile", "unit", "rect", "args"]
FRAME_COLS = ["seq", "tick", "draw", "client_update", "w", "h", "view_rect", "tile_origin",
              "unit_origin", "shift_x", "open_mode", "shake", "player_type", "player_mode",
              "player_cur", "player_fixed", "player_client", "act", "level_id", "env",
              "light_quality", "draw_rate", "light_opt_a", "light_opt_b", "render_kind", "rain",
              "snow", "lightning", "flash", "weather_update", "cursor_key", "seed_start", "seed_end",
              "clear_counter", "res_mode", "draw_count", "index_sha256", "palette_sha256"]
SPRITE_KEY = ["cel_path", "dir", "frame"]
SPRITE_MISSING = ["width", "height", "x_offset", "y_offset"]
DRAWS_MISSING = ["sprite width/height/offsets (cel frame headers are not recorded)",
                 "cel_path of unit component cels (no load hook: capture.md §3.6, OQ 4)",
                 "DT1 file / tile index of tile draws (capture.md §3.5, OQ 3)"]
FRAME_MISSING = ["player direction (not read for the player record)"]


class FactsError(Exception):
    pass


def cell(v):
    """One TSV cell: lists joined with ',', dict items k=v joined with ';', None empty."""
    if v is None:
        return ""
    if isinstance(v, bool):
        return str(int(v))
    if isinstance(v, (list, tuple)):
        return ",".join(cell(x) for x in v)
    if isinstance(v, dict):
        return ";".join(f"{k}={cell(x)}" for k, x in v.items())
    return str(v).replace("\t", " ").replace("\n", " ")


def light_digest(v):
    """Tile light: the recorder's 16-hex digest; full arrays (--draws-light) get the same digest."""
    if isinstance(v, str) and len(v) > 16:
        return hashlib.sha256(bytes.fromhex(v)).hexdigest()[:16]
    return v


def draw_row(i, d, paths):
    op, a = d["op"], d.get("a") or []
    r = dict.fromkeys(DRAW_COLS)
    r.update(i=i, op=op, at=d.get("at"), args=a)
    if op == "unit":  # capture.md §3.5: ECX unit, EDX light, stack client x, y, two flags
        u = d.get("u") or {}
        r.update(kind="unit", x=a[0] if a else None, y=a[1] if len(a) > 1 else None,
                 light=f"{d.get('light', 0) & 0xFFFFFFFF:#010x}", dir=u.get("dir"),
                 frame=u["cur"] >> 8 if "cur" in u else None,
                 unit={k: u[k] for k in ("type", "class", "guid", "mode", "seq", "seq_mode", "cur",
                                         "fc", "flags", "flags2", "fixed", "client", "subtile",
                                         "extra") if k in u})
    elif op in TILE_OPS:  # args after the tile header pointer, capture.md §3.5
        t = d.get("tile") or {}
        r.update(kind="tile", light=light_digest(d.get("light")),
                 tile={k: t[k] for k in ("roof", "orient", "main", "sub", "rarity") if k in t})
        if op == "FloorTileDraw":   # light grid, X, Y, world x, world y, alpha, open mode, data
            r.update(x=a[1], y=a[2], world=a[3:5], alpha=a[5], open_mode=a[6])
        elif op != "ShadowTileDraw":  # X, Y, light, open mode (+ alpha for TileDrawTrans)
            r.update(x=a[0], y=a[1], open_mode=a[3], alpha=a[4] if op == "TileDrawTrans" else None)
    elif "cel" in d:  # cel context, X, Y, ...
        c = d["cel"] or {}
        r.update(kind="cel", x=a[0] if a else None, y=a[1] if len(a) > 1 else None,
                 cel_file=c.get("file"), cel_path=paths.get(c.get("file")), dir=c.get("dir"),
                 frame=c.get("frame"), tokens=[t or "" for t in c.get("tokens", [])] if c else None)
    elif op in RECT_OPS:
        r.update(kind="rect", rect=d.get("rect"))
    else:
        r["kind"] = "prim" if op in PRIM_OPS else "other"
    return r


def frame_row(f, png_dir):
    p, lv, li, we = f.get("player") or {}, f.get("level") or {}, f.get("light") or {}, f.get("weather") or {}
    idx, pal = f.get("index_sha256"), f.get("palette_sha256")
    if (not idx or not pal) and f.get("image") and png_dir:  # digests from the saved PNG (§6)
        import record_frames  # noqa: E402  (png_indices: the inverse of its own PNG writer)
        with open(os.path.join(png_dir, f["image"]), "rb") as fh:
            _, _, pixels, palette = record_frames.png_indices(fh.read())
        idx, pal = hashlib.sha256(pixels).hexdigest(), hashlib.sha256(palette).hexdigest()
    return {"seq": f["seq"], "tick": f.get("f"), "draw": f.get("draw"),
            "client_update": f.get("client_update"), "w": f.get("w"), "h": f.get("h"),
            "view_rect": f.get("view_rect"), "tile_origin": f.get("tile_origin"),
            "unit_origin": f.get("unit_origin"), "shift_x": f.get("shift_x"),
            "open_mode": f.get("open_mode"), "shake": f.get("shake"), "player_type": p.get("type"),
            "player_mode": p.get("mode"), "player_cur": p.get("cur"), "player_fixed": p.get("fixed"),
            "player_client": p.get("client"), "act": lv.get("act"), "level_id": lv.get("level_id"),
            "env": lv.get("env"), "light_quality": li.get("quality"), "draw_rate": li.get("draw_rate"),
            "light_opt_a": li.get("opt_a"), "light_opt_b": li.get("opt_b"),
            "render_kind": li.get("render_kind"), "rain": we.get("rain"), "snow": we.get("snow"),
            "lightning": we.get("lightning"), "flash": we.get("flash"),
            "weather_update": we.get("update"), "cursor_key": f.get("cursor_key"),
            "seed_start": f.get("seed_start"), "seed_end": f.get("seed_end"),
            "clear_counter": f.get("clear_counter"), "res_mode": f.get("res_mode"),
            "draw_count": len(f["draws"]) if "draws" in f else None,
            "index_sha256": idx, "palette_sha256": pal}


def read_capture(path, seq, png_root):
    """(frame record, its draw rows, image directory) of the chosen frame."""
    with open(path, encoding="utf-8") as fh:
        recs = [json.loads(line) for line in fh if line.strip()]
    if not recs or recs[0].get("k") != "header" or recs[0].get("format") != FORMAT:
        raise FactsError(f"{path}: not a {FORMAT} capture")
    cap = next((r for r in recs if r.get("k") == "capture"), {})
    frames = [(i, r) for i, r in enumerate(recs) if r.get("k") == "frame" and "refused" not in r]
    if seq is None:
        frames = [x for x in frames if x[1].get("draws")]
    else:
        frames = [x for x in frames if x[1]["seq"] == seq]
    if not frames:
        raise FactsError("no frame with a draw log" if seq is None else f"no captured frame seq {seq}")
    at, f = frames[-1]
    if not f.get("draws"):
        raise FactsError(f"frame seq {f['seq']} has no draw log (record with --draws-every N)")
    paths = {}  # cel file pointer -> path, as loaded before this frame (§3.6)
    for r in recs[:at]:
        if r.get("k") == "celfile":
            paths[r["ptr"]] = r["path"]
    png_dir = os.path.join(png_root, cap["images"]) if cap.get("images") else None
    return f, [draw_row(i, d, paths) for i, d in enumerate(f["draws"])], png_dir


def tsv(header, cols, rows, missing):
    lines = [header] + ([f"# missing: {'; '.join(missing)}"] if missing else [])
    lines.append("\t".join(cols))
    lines += ["\t".join(cell(r[c]) for c in cols) for r in rows]
    return "\n".join(lines) + "\n"


def sprite_key(row):
    return (row[0], int(row[1]), int(row[2]))


def merge_sprites(path, draws):
    """Union of the existing sprites.tsv rows and this frame's; returns (rows, conflicts)."""
    rows = {}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            body = [line.rstrip("\n") for line in fh if not line.startswith("#")]
        if body and body[0].split("\t") != SPRITE_KEY:
            raise FactsError(f"{path}: columns {body[0]!r}, expected {SPRITE_KEY}")
        for line in body[1:]:
            if line:
                row = line.split("\t")
                rows[sprite_key(row)] = row
    conflicts = []
    for d in draws:
        if d["kind"] != "cel" or d["cel_path"] is None or d["dir"] is None or d["frame"] is None:
            continue
        row = [cell(d[c]) for c in SPRITE_KEY]
        old = rows.setdefault(sprite_key(row), row)
        if old != row:
            conflicts.append(f"{path}: {sprite_key(row)}: existing {old}, new {row}")
    return [rows[k] for k in sorted(rows)], conflicts


def command_line():
    def q(s):
        return f'"{s}"' if (" " in s or not s) else s
    script = os.path.relpath(os.path.abspath(sys.argv[0]), REPO).replace("\\", "/")
    return " ".join(["py", q(script)] + [q(a) for a in sys.argv[1:]])


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("capture", help="a frames-raw-2 .jsonl from record_frames.py")
    ap.add_argument("--scene", required=True, help="scene name: the directory under scenes/")
    ap.add_argument("--frame", type=int, default=None, help="frame seq (default: last with draws)")
    ap.add_argument("--out", default=os.path.join(REPO, "facts", "render"))
    ap.add_argument("--images", default=os.path.join(REPO, "game", "captures"),
                    help="PNG root, used only when a frame lacks its digests")
    a = ap.parse_args()
    header = f"# facts v1; tool: {TOOL}; command: {command_line()}; game: 1.14d"
    try:
        f, draws, png_dir = read_capture(a.capture, a.frame, a.images)
        sprites_path = os.path.join(a.out, "sprites.tsv")
        sprites, conflicts = merge_sprites(sprites_path, draws)
    except (FactsError, OSError, ValueError, KeyError) as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
    if conflicts:
        for c in conflicts:
            print(f"error: conflicting sprite row {c}", file=sys.stderr)
        sys.exit(1)
    fr = frame_row(f, png_dir)
    scene = os.path.join(a.out, "scenes", a.scene)
    unnamed = sum(1 for d in draws if d["kind"] == "cel" and d["cel_path"] is None)
    sprite_missing = SPRITE_MISSING + ([f"{unnamed} cel draws of this frame without a file path "
                                        "(not listed)"] if unnamed else [])
    os.makedirs(scene, exist_ok=True)
    outs = [(os.path.join(scene, "draws.tsv"), tsv(header, DRAW_COLS, draws, DRAWS_MISSING)),
            (os.path.join(scene, "frame.tsv"), tsv(header, FRAME_COLS, [fr], FRAME_MISSING)),
            (sprites_path, "\n".join([header, "# missing: " + "; ".join(sprite_missing),
                                      "\t".join(SPRITE_KEY)] + ["\t".join(r) for r in sprites]) + "\n")]
    for path, text in outs:
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)
    print(f"frame seq {f['seq']} (tick {f.get('f')}): {len(draws)} draws, {unnamed} unnamed cels; "
          f"sprites.tsv {len(sprites)} rows; wrote {scene}")


if __name__ == "__main__":
    main()
