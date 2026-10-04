"""Fade a dancer clip's floor region into a soft elliptical contact shadow.

    py tools/shadow_fade.py band_guitar            # defaults suit a standing figure
    py tools/shadow_fade.py band_keys --ankle 0.90 --rx 1.6

Matting keeps near-opaque floor reflections fused to the feet — they can't
be separated by alpha threshold or component selection. Cutting them flat
reads as amputated feet; fading everything below the ankle into a soft pool
reads as standing on a glossy stage instead.

How it works per frame: the subject's y-extent sets the ankle line (bottom
`--ankle` fraction is the foot zone). Below it, an ellipse is fitted around
the feet's column extent; pixels inside keep their alpha, and outside it
the alpha decays exponentially, so the leftover reflection reads as a
contact shadow rather than a spill.
"""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent


def fade(clip: Path, args):
    meta = json.loads((clip / "clip.json").read_text())
    n = meta["frames"]
    for i in range(n):
        f = np.asarray(Image.open(clip / "frames" / f"{i:04d}.png"), np.float32) / 255.0
        b = f > 0.3
        if not b.any():
            continue
        ys = np.where(b.any(axis=1))[0]
        y0, y1 = ys.min(), ys.max()
        ankle = y0 + (y1 - y0) * args.ankle
        # Foot zone: subject pixels below the ankle, used to centre the pool.
        fz = np.where(b[int(ankle):])[1]
        cx = float(np.median(fz))
        fx = np.percentile(fz, [5, 95])
        rx = max((fx[1] - fx[0]) / 2 * args.rx, 6.0)
        ry = max((y1 - ankle) * args.ry + 6, 6.0)
        cy = ankle + (y1 - ankle) * 0.4
        yy, xx = np.mgrid[0:f.shape[0], 0:f.shape[1]]
        d2 = ((xx - cx) / rx) ** 2 + ((yy - cy) / ry) ** 2
        keep = np.exp(-np.maximum(d2 - 1.0, 0.0) * args.falloff)
        keep[yy < ankle] = 1.0
        out = f * keep
        Image.fromarray((out * 255).astype(np.uint8)).save(clip / "frames" / f"{i:04d}.png")
    print(f"{clip.name}: faded floor region on {n} frames")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("clips", nargs="+")
    ap.add_argument("--ankle", type=float, default=0.86,
                    help="fraction of subject height where the foot zone starts")
    ap.add_argument("--rx", type=float, default=1.5,
                    help="pool half-width as a multiple of the feet's half-span")
    ap.add_argument("--ry", type=float, default=1.2,
                    help="pool half-height as a multiple of the below-ankle span")
    ap.add_argument("--falloff", type=float, default=2.5,
                    help="exponential decay outside the pool (bigger = tighter edge)")
    args = ap.parse_args()
    for name in args.clips:
        fade(ROOT / "dancers" / name, args)


if __name__ == "__main__":
    main()
