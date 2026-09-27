"""Clean baked-in background artifacts out of an existing dancer clip.

    py tools/clean_masks.py stock_rim                     # subtract static bg
    py tools/clean_masks.py stock_neon --bg-weight 0 --open 9 --thresh 0.4
    py tools/clean_masks.py stock_purple --bg-weight 0 --floor 0 --thresh 0.88

Some stock captures leave set dressing in the matte (window grids, wall
texture, a static light-spill halo). Two mechanisms, combinable:

  --bg-weight (default 1.0): subtract the per-pixel temporal MEDIAN — anything
      static (grids, spill, texture) vanishes, only what moves survives.
      Kills dancers that linger in one spot, so pass 0 for those.
  --open N: morphological opening with an NxN kernel before blob-keeping —
      thin structures (grids, bars) erode away and can't regrow.
  --preblur N: blur the mask N px before thresholding — a solid dancer keeps
      her alpha while spongy set-dressing texture averages itself away.
  --cut-wide F: zero any bottom-edge-connected region whose rows span more
      than F of the frame width (stage ledges fused to her feet) — the cut is
      feathered so her legs read as behind it.
  --solid A: pixels inside the kept blob above 0.25 are raised to at least A —
      turns the body uniformly solid so residual texture doesn't show through.

The largest connected component is kept per frame and interior holes filled.

    py tools/contact_sheet.py <clip> out.png   # eyeball before/after
"""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter
from scipy.ndimage import (binary_dilation, binary_fill_holes, binary_opening,
                           label as cc_label)

ROOT = Path(__file__).resolve().parent.parent


def clean(clip: Path, args):
    meta = json.loads((clip / "clip.json").read_text())
    n = meta["frames"]
    stack = np.stack([np.asarray(Image.open(clip / "frames" / f"{i:04d}.png"), np.float32) / 255.0
                      for i in range(n)])

    m = stack
    if args.bg_weight > 0:
        resid = np.clip(m - args.bg_weight * np.median(m, axis=0), 0.0, 1.0)
        scale = np.percentile(resid, 98.0)
        if scale < 0.05:
            raise SystemExit(f"{clip.name}: residual is empty — lower --bg-weight")
        m = np.clip(resid / scale, 0.0, 1.0)
    if args.floor > 0:
        m = np.clip((m - args.floor) / max(1.0 - args.floor, 1e-3), 0.0, 1.0)

    # Static backdrop = pixels strong in most frames. A blob that's mostly
    # inside it is set dressing, not the dancer — rejected no matter its size.
    # (Disable for clips where the dancer dwells in one spot.)
    static = np.median(stack, axis=0) > 0.9 if args.static_max < 1.0 else None

    kern = np.ones((args.open, args.open)) if args.open > 0 else None
    cleaned = []
    prev_keep = None
    for i, f in enumerate(m):
        b = f > args.thresh
        if args.preblur > 0:
            bb = np.asarray(Image.fromarray((f * 255).astype(np.uint8))
                            .filter(ImageFilter.GaussianBlur(args.preblur)), np.float32) / 255.0
            b = bb > args.thresh
        if kern is not None:
            b = binary_opening(b, structure=kern)
        lab, nb = cc_label(b)
        if nb:
            sizes = np.bincount(lab.ravel()).astype(np.float64)
            sizes[0] = 0
            # Static fraction of each blob.
            if static is not None:
                comp_static = np.bincount(lab[static & (lab > 0)].ravel(), minlength=nb + 1)
                pool = (comp_static / np.maximum(sizes, 1) <= args.static_max) \
                    & (np.arange(nb + 1) > 0)
            else:
                comp_static = np.zeros(nb + 1)
                pool = np.arange(nb + 1) > 0
            if args.max_cover > 0:
                # The dancer never fills the frame — a giant blob is backdrop.
                pool &= sizes <= args.max_cover * b.size
            if not pool.any():
                # Every candidate is static-ish — take the least static one.
                frac = comp_static / np.maximum(sizes, 1)
                frac[0] = np.inf
                best = int(frac.argmin())
            else:
                scores = np.where(pool, sizes, 0.0)
                if prev_keep is not None and prev_keep.shape == b.shape:
                    # Prefer the pooled blob overlapping last frame's keep —
                    # the dancer moves continuously, set dressing doesn't.
                    overlap = np.bincount(lab[prev_keep].ravel(), minlength=nb + 1)
                    overlap[0] = 0
                    overlap[~pool] = 0
                    best = int(overlap.argmax()) if overlap.max() > 0 else int(scores.argmax())
                else:
                    best = int(scores.argmax())
            keep = lab == best
            if args.reattach > 0:
                # Opening severs thin connections (neck, wrists). Regrow: any
                # pre-open component substantially overlapping the kept core
                # is a severed appendage, not set dressing.
                zone = binary_dilation(keep, iterations=args.reattach)
                lab2, n2 = cc_label(b)
                comp_ids = np.unique(lab2[zone])[1:]
                for c in comp_ids:
                    comp = lab2 == c
                    if (comp & zone).sum() / max(comp.sum(), 1) > 0.4:
                        keep |= comp
            if args.cut_wide > 0:
                h2, w2 = keep.shape
                cover = keep.sum(axis=1) / w2
                kill = np.zeros(h2, bool)
                fade = np.ones(h2, np.float32)
                # Long runs of near-full-width rows are structural bands
                # (ledges, ceiling rigs) — a body never spans that for 3+ rows.
                r = 0
                while r < h2:
                    if cover[r] > args.cut_wide:
                        s = r
                        while r < h2 and cover[r] > args.cut_wide:
                            r += 1
                        if r - s >= 3:
                            kill[s:r] = True
                            for k in range(min(14, s)):
                                fade[s - 1 - k] = min(fade[s - 1 - k], 0.25 + 0.75 * k / 14)
                    else:
                        r += 1
                # Thin rails: short runs of mostly-wide rows fused to her.
                r = 0
                while r < h2:
                    if cover[r] > 0.5 and not kill[r]:
                        s = r
                        while r < h2 and cover[r] > 0.5:
                            r += 1
                        if r - s <= 6:
                            kill[s:r] = True
                    else:
                        r += 1
                if kill.any():
                    f = f * fade[:, None]
                    keep = keep & ~kill[:, None]
            # The cut may leave detached slivers behind — re-keep the largest.
            lab, nb = cc_label(keep)
            if nb:
                sizes = np.bincount(lab.ravel())
                sizes[0] = 0
                keep = lab == sizes.argmax()
            holes = binary_fill_holes(keep) & ~keep if args.hole > 0 else None
            if holes is not None:
                keep |= holes
                f = np.maximum(f, holes.astype(np.float32) * args.hole)
            if args.solid > 0:
                f = np.where(keep & (f > 0.25), np.maximum(f, args.solid), f)
            f = np.where(keep, f, 0.0)
            # Never emit a dead frame: if cleaning emptied a frame whose raw
            # mat had real content, keep the raw mat — a faint dancer beats
            # a black flicker mid-routine.
            if (f > 0.125).mean() < 0.01 and (stack[i] > 0.125).mean() > 0.02:
                f = stack[i]
                keep = f > 0.125
            prev_keep = keep
        cleaned.append(f)
        img = Image.fromarray((f * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(args.blur))
        img.save(clip / "frames" / f"{i:04d}.png")
    cleaned = np.stack(cleaned)
    cov = float((cleaned.max(axis=0) > 0.3).mean())
    if args.crop:
        # Dead space left by cut regions: re-crop to the dancer's extent so a
        # cut edge lands at the frame edge (a ledge cut reads as her
        # dissolving at the floor, not floating mid-air).
        ys, xs = np.nonzero(cleaned.max(axis=0) > 0.05)
        if len(ys):
            pad = 6
            y0, y1 = max(ys.min() - pad, 0), min(ys.max() + pad, cleaned.shape[1])
            x0, x1 = max(xs.min() - pad, 0), min(xs.max() + pad, cleaned.shape[2])
            for i in range(n):
                img = Image.open(clip / "frames" / f"{i:04d}.png").crop((x0, y0, x1, y1))
                img.save(clip / "frames" / f"{i:04d}.png")
            meta["width"], meta["height"] = int(x1 - x0), int(y1 - y0)
            (clip / "clip.json").write_text(json.dumps(meta, indent=2) + "\n")
            print(f"{clip.name}: cropped to {x1 - x0}x{y1 - y0}")
    print(f"{clip.name}: cleaned {n} frames (union cover {cov:.2f})")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("clips", nargs="+")
    ap.add_argument("--bg-weight", type=float, default=1.0,
                    help="how much of the temporal median to subtract (0 = off)")
    ap.add_argument("--floor", type=float, default=0.12,
                    help="absolute alpha floor (kills residual haze)")
    ap.add_argument("--open", type=int, default=0, metavar="N",
                    help="morphological opening kernel — removes structures thinner than N px")
    ap.add_argument("--preblur", type=float, default=0.0, metavar="N",
                    help="blur radius before thresholding — averages spongy texture away")
    ap.add_argument("--cut-wide", type=float, default=0.0, metavar="F",
                    help="zero bottom-edge-connected regions wider than this fraction")
    ap.add_argument("--reattach", type=int, default=0, metavar="N",
                    help="regrow pre-open components overlapping the kept core by N px")
    ap.add_argument("--max-cover", type=float, default=0.0, metavar="F",
                    help="reject blobs covering more than this fraction of the frame")
    ap.add_argument("--static-max", type=float, default=0.6, metavar="F",
                    help="reject blobs more than this fraction inside the static "
                         "backdrop (1.0 = off — for dancers who dwell in place)")
    ap.add_argument("--thresh", type=float, default=0.3,
                    help="blob-detection threshold for largest-component keep")
    ap.add_argument("--hole", type=float, default=0.85,
                    help="alpha assigned to filled interior holes (0 = don't fill)")
    ap.add_argument("--solid", type=float, default=0.0,
                    help="raise kept pixels above 0.25 to at least this alpha")
    ap.add_argument("--blur", type=float, default=0.6)
    ap.add_argument("--crop", action="store_true",
                    help="re-crop frames to the cleaned dancer's extent and update clip.json")
    args = ap.parse_args()
    for name in args.clips:
        clean(ROOT / "dancers" / name, args)


if __name__ == "__main__":
    main()
