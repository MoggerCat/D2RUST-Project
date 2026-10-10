#!/usr/bin/env python3
"""Character-select paper dolls: 1.14d captures vs d2rs frames (REC-2295).

Our own code. Input: the OUT directory of
  python3 tools/frontend-sbs/frontend_sbs.py OUT --script dolls
(12 timed 1.14d shots, 70 dense d2rs shots; Blizzard art, OUT stays outside
the repository). Per slot (figure rectangles of the 3-save charselect):
 - doll mask = pixels where a d2rs frame departs from the per-pixel median of
   all d2rs frames (the animated figure; snow flakes add a few pixels);
 - each 1.14d shot is matched to the d2rs frame with the fewest differing mask
   pixels (frame phase is not tick-anchored: REC-2182);
 - a pixel differs when a channel differs by more than TOL (the whole-screen
   brightness offset of REC-1550 is up to 8, dark noise up to 16);
 - the slot is EQUAL when every shot matches with <= 1 % of the mask differing.
Also prints the dy probe (d2rs figure shifted -2..2 rows against 1.14d, REC-2183)
and the feet-band signed brightness difference (shadow probe, REC-2184).
Usage: dolls_check.py OUT      exit 0 all slots EQUAL, 1 otherwise.
Spec: specs/ui/frontend-menus.md §F2.10.
"""
import glob, json, sys
import numpy as np
from PIL import Image

TOL = 16
MASK_MIN = 30
MAX_FRAC = 0.01
RECTS = {"slot0": (40, 90, 100, 175), "slot1": (310, 90, 370, 175), "slot2": (40, 185, 100, 275)}


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
    orig = [load(f, (112, 98)) for f in sorted(glob.glob(out + "/orig/d*.png"))]
    ours = [load(f, (0, 0)) for f in sorted(glob.glob(out + "/ours/d*.png"))]
    if not orig or len(ours) < 20:
        sys.exit("need orig/d*.png and >= 20 ours/d*.png (frontend_sbs.py OUT --script dolls)")
    bg = np.median(np.stack(ours), axis=0)
    res = {}
    for name, (x0, y0, x1, y1) in RECTS.items():
        sl = (slice(y0, y1), slice(x0, x1))
        masks = [np.abs(u[sl] - bg[sl]).sum(2) > MASK_MIN for u in ours]
        worst, frames = 0.0, []
        for o in orig:
            best = None
            for j, u in enumerate(ours):
                d = int(((np.abs(o[sl] - u[sl]).max(2) > TOL) & masks[j]).sum())
                if best is None or d < best[0]:
                    best = (d, j, int(masks[j].sum()))
            frames.append(best[1])
            worst = max(worst, best[0] / max(best[2], 1))
        # dy probe on shot 0 with its best frame
        o, u = orig[0], ours[frames[0]]
        dy = {}
        for s in (-2, -1, 0, 1, 2):
            a = np.roll(u, s, axis=0)
            m = np.abs(a[sl] - bg[sl]).sum(2) > MASK_MIN
            dy[s] = int(((np.abs(o[sl] - a[sl]).max(2) > TOL) & m).sum())
        # shadow probe: signed brightness in the feet band vs the rest outside the figure
        m = dilate(np.abs(u - bg).sum(2) > MASK_MIN)[sl]
        dd = (o - u).sum(2).astype(float)[sl]
        feet = np.nonzero(m.any(1))[0].max()
        band = np.zeros_like(m); band[feet - 6:feet + 4, :] = True; band &= ~m
        ctl = ~m; ctl[feet - 6:feet + 4, :] = False
        res[name] = {"worst_frac": round(worst, 4), "matched_frames": frames, "dy_probe": dy,
                     "feet_band": round(float(dd[band].mean()), 2), "control": round(float(dd[ctl].mean()), 2),
                     "verdict": "EQUAL" if worst <= MAX_FRAC else "DIVERGED"}
    return res


if __name__ == "__main__":
    r = run(sys.argv[1].rstrip("/"))
    print(json.dumps(r, indent=1))
    sys.exit(0 if all(v["verdict"] == "EQUAL" for v in r.values()) else 1)
