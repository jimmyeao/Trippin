"""Animate a still silhouette image into a beat-locked dancer clip.

    py tools/puppet_dancer.py jazz_ref.png --name band_guitar

Video models keep collapsing "playing guitar" into one hunched pose, so
this goes the other way: take a single *good* upright silhouette still and
puppet-warp it. Soft elliptical regions pick out parts (head, strum
forearm, fret hand), each part is rotated a few degrees around a joint
pivot, and the whole figure gets a slow sway + per-beat knee bounce. Every
motion is periodic over the loop, so it wraps seamlessly and strums land
exactly on beats — no accent needed.

The part fields are coordinates in the source image (use a viewer).
"""
import argparse
import json
import math
import shutil
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

ROOT = Path(__file__).resolve().parent.parent
TWO_PI = 2 * math.pi


def soft_ellipse(shape, cx, cy, rx, ry, deg):
    """0..1 weight mask: 1 inside the ellipse, feathering out over ~15%."""
    yy, xx = np.mgrid[0:shape[0], 0:shape[1]]
    t = math.radians(deg)
    dx, dy = xx - cx, yy - cy
    u = (dx * math.cos(t) + dy * math.sin(t)) / rx
    v = (-dx * math.sin(t) + dy * math.cos(t)) / ry
    d = np.sqrt(u * u + v * v)
    return np.clip((1.15 - d) / 0.15, 0, 1)


def warp_rot(mask, px, py, ang):
    """Rotate `mask` `ang` degrees about (px, py) — output-space inverse map."""
    a = math.radians(-ang)
    A = np.array([[math.cos(a), -math.sin(a)], [math.sin(a), math.cos(a)]])
    b = np.array([px, py], dtype=float) - A @ np.array([px, py], dtype=float)
    return ndimage.affine_transform(mask, A, b, order=1, mode="constant")


def warp_shift(mask, dx, dy):
    return ndimage.affine_transform(mask, np.eye(2), np.array([dx, dy], dtype=float),
                                    order=1, mode="constant")


# Part motion, all in beats. `wave` returns the rotation in degrees at `beat`.
def nod(beat, amp):
    """A bob that lands ON the beat and decays within it — reads as grooving
    to the music, not a headbang."""
    return amp * math.exp(-4.0 * (beat % 1.0)) - amp * 0.4 * math.sin(TWO_PI * beat / 2)


def strum(beat, amp):
    """Eighth-note strokes, swelling through the phrase."""
    return amp * math.sin(TWO_PI * beat * 2) * (0.8 + 0.2 * math.cos(TWO_PI * beat / 8))


def wiggle(beat, amp):
    return amp * math.sin(TWO_PI * beat * 2 + 1.2)


MOTIONS = {"nod": nod, "strum": strum, "wiggle": wiggle}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--beats", type=int, default=8)
    ap.add_argument("--fps", type=float, default=30.0)
    ap.add_argument("--height", type=int, default=512)
    ap.add_argument("--energy", type=float, default=0.5)
    ap.add_argument("--source", default="procedural: puppet-warped silhouette still")
    ap.add_argument("--parts", type=Path, help="JSON rig override (default: built-in guitarist rig)")
    args = ap.parse_args()

    rig = json.loads(args.parts.read_text()) if args.parts else {
        # Rig for the 1072x1920 front-facing jazz guitarist still:
        # head nods, right (image-left) forearm strums at the elbow, the
        # fret hand wiggles a touch on the neck.
        "parts": [
            {"name": "head",  "ell": [480, 200, 100, 145, 0],   "pivot": [480, 340], "motion": "nod",    "amp": 2.6},
            {"name": "strum", "ell": [245, 940, 95, 215, 18],   "pivot": [190, 760], "motion": "strum",  "amp": 9.0},
            {"name": "fret",  "ell": [710, 640, 75, 75, 0],     "pivot": [640, 660], "motion": "wiggle", "amp": 2.5},
        ],
        "sway": {"pivot": [480, 1846], "amp": 1.0, "bob": 4.0},
    }

    src = np.asarray(Image.open(args.image).convert("L"), np.float32) / 255.0
    mask = (src < 0.5).astype(np.float32)          # black figure on white
    mask = ndimage.binary_fill_holes(mask > 0.5).astype(np.float32)  # solid silhouette

    H, W = mask.shape
    weights = []
    for p in rig["parts"]:
        weights.append(soft_ellipse(mask.shape, *p["ell"]))
    w_moving = np.clip(np.sum(weights, axis=0), 0, 1)
    w_static = 1.0 - w_moving

    n = int(round(args.beats * args.fps * 60.0 / 124.0))
    sway = rig["sway"]
    frames = []
    for f in range(n):
        beat = f / n * args.beats
        out = w_static * mask
        for p, w in zip(rig["parts"], weights):
            ang = MOTIONS[p["motion"]](beat, p["amp"])
            out = out + w * warp_rot(mask, *p["pivot"], ang)
        rock = sway["amp"] * math.sin(TWO_PI * beat / 4 - math.pi / 2)
        bob = -sway["bob"] * (0.5 - 0.5 * math.cos(TWO_PI * beat))
        out = warp_rot(out, *sway["pivot"], rock)
        out = warp_shift(out, bob, 0)
        frames.append(np.clip(out, 0, 1))

    # Union crop across the loop, like stock_dancer does.
    union = np.stack(frames).max(axis=0) > 0.05
    ys, xs = np.where(union)
    pad = int(0.02 * H)
    y0, y1 = max(ys.min() - pad, 0), min(ys.max() + pad, H)
    x0, x1 = max(xs.min() - pad, 0), min(xs.max() + pad, W)
    scale = args.height / (y1 - y0)
    out_w = int(round((x1 - x0) * scale / 4)) * 4

    out = ROOT / "dancers" / args.name
    if out.exists():
        shutil.rmtree(out)
    (out / "frames").mkdir(parents=True)
    for f, m in enumerate(frames):
        img = Image.fromarray((m * 255).astype(np.uint8)).crop((x0, y0, x1, y1))
        img = img.resize((out_w, args.height), Image.LANCZOS)
        img.save(out / "frames" / f"{f:04d}.png")
    meta = {"name": args.name, "fps": args.fps, "frames": n, "beats": args.beats,
            "width": out_w, "height": args.height, "energy": args.energy,
            "source": args.source}
    (out / "clip.json").write_text(json.dumps(meta, indent=2) + "\n")
    print(f"{args.name}: {n} frames ({out_w}x{args.height}), {args.beats} beats")


if __name__ == "__main__":
    main()
