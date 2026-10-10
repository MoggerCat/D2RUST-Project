#!/usr/bin/env python3
"""Front-end whole screens: 1.14d captures vs d2rs frames (REC-3760).

Our own code. Input: the OUT directory of
  python3 tools/frontend-sbs/frontend_sbs.py OUT --script screens
(per screen 6 timed 1.14d shots NAME_00..05 and 40 dense d2rs shots; Blizzard
art, OUT stays outside the repository). The animations (logo fire, snow, credits
scroll) run on wall-clock time on both sides, so a shot pair is never phase
locked; the check therefore compares the layers that do not move:
 - per side, per pixel: the median over the screen's shots, and the pixel is
   STABLE when at least STABLE_FRAC of the shots equal the median exactly;
 - the compared set is the pixels stable on both sides (the animated rest is
   counted as `anim_px`, not compared; it is not "equal", it is unchecked);
 - on the compared set the two medians must be identical (tolerance 0, any
   channel); the screen is EQUAL when 0 pixels differ and the compared set is
   at least MIN_COVER of the 800x600 frame.
The first differing pixel (x, y) in row-major order and its bounding box are reported.
Usage: screens_check.py OUT      exit 0 all screens EQUAL, 1 otherwise.
Spec: specs/ui/frontend-menus.md §F2.11, specs/tools/scenario-diff.md §3 rule 16.
"""
import glob, json, os, sys
import numpy as np
from PIL import Image

OFF_O = (112, 98)
OFF_U = (0, 1)
STABLE_FRAC = 0.7
MIN_COVER = 0.5


def load(path, off):
    im = Image.open(path).convert("RGB").crop((off[0], off[1], off[0] + 800, off[1] + 600))
    return np.asarray(im).astype(int)


def dilate(a):
    b = a.copy()
    for dy in (-1, 0, 1):
        for dx in (-1, 0, 1):
            b |= np.roll(np.roll(a, dy, 0), dx, 1)
    return b


def run(out):
    names = sorted({os.path.basename(f).rsplit("_", 1)[0] for f in glob.glob(out + "/orig/*_[0-9][0-9].png")})
    if not names:
        sys.exit("need orig/NAME_NN.png (frontend_sbs.py OUT --script screens)")
    res = {}
    for n in names:
        orig = np.stack([load(f, OFF_O) for f in sorted(glob.glob(f"{out}/orig/{n}_[0-9][0-9].png"))])
        ours = [load(f, OFF_U) for f in sorted(glob.glob(f"{out}/ours/{n}_[0-9][0-9].png"))]
        if not ours:
            res[n] = {"verdict": "DIVERGED", "error": "no d2rs shots"}
            continue
        ours = np.stack(ours)
        mo, mu = np.median(orig, axis=0), np.median(ours, axis=0)
        so = (orig == mo).all(3).mean(0) >= STABLE_FRAC
        su = (ours == mu).all(3).mean(0) >= STABLE_FRAC
        both = so & su
        diff = (mo != mu).any(2) & both
        n_diff = int(diff.sum())
        ys, xs = np.nonzero(diff)
        cover = both.sum() / both.size
        ok = n_diff == 0 and cover >= MIN_COVER
        res[n] = {"compared_px": int(both.sum()), "anim_px": int((~both).sum()), "diff_px": n_diff,
                  "first": [int(xs[0]), int(ys[0])] if n_diff else None,
                  "bbox": [int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())] if n_diff else None,
                  "verdict": "EQUAL" if ok else "DIVERGED"}
    return res


if __name__ == "__main__":
    r = run(sys.argv[1].rstrip("/"))
    print(json.dumps(r, indent=1))
    sys.exit(0 if all(v["verdict"] == "EQUAL" for v in r.values()) else 1)
