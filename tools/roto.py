"""Rotoscope a dance video into a beat-loopable silhouette clip for Trippin.

    py tools/roto.py dance.mp4 --name vogue --start 12.4 --beats 8 --bpm 124
    py tools/roto.py shadow.mp4 --name shadow --start 3 --end 10.7 --beats 16 --matte luma

Trim the clip so --start lands on a downbeat and it spans a whole number of
beats (--beats). Give either --end or --bpm, the source track's tempo. Trippin
then stretches the loop to the live tempo, switching to half or double time
when the gap is large.

Mattes:
  ai     person segmentation with rembg (u2net_human_seg). Works on any footage.
         Needs `py -m pip install rembg`; the model (~170 MB) downloads on first use.
  luma   bright subject on a dark background (or the reverse with --invert).
  chroma green/blue screen: --key 00ff00.

Output: dancers/<name>/frames/0000.png... (8-bit masks, cropped to the
dancer) plus clip.json. ffmpeg must be on PATH.
"""
import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
MAX_FRAMES = 240  # GPU texture-array limit is 256 layers


def extract(video, start, duration, fps, tmp):
    subprocess.run(
        ["ffmpeg", "-v", "error", "-ss", str(start), "-t", str(duration), "-i", str(video),
         "-vf", f"fps={fps}", str(tmp / "%05d.png")],
        check=True,
    )
    return sorted(tmp.glob("*.png"))


def matte_luma(img, args):
    g = np.asarray(img.convert("L"), dtype=np.float32) / 255.0
    m = g if not args.invert else 1.0 - g
    lo, hi = args.threshold - args.softness, args.threshold + args.softness
    return np.clip((m - lo) / max(hi - lo, 1e-3), 0, 1)


def matte_chroma(img, args):
    rgb = np.asarray(img.convert("RGB"), dtype=np.float32) / 255.0
    key = np.array([int(args.key[i:i + 2], 16) / 255.0 for i in (0, 2, 4)], dtype=np.float32)
    dist = np.linalg.norm(rgb - key, axis=-1)
    lo, hi = args.threshold - args.softness, args.threshold + args.softness
    return np.clip((dist - lo) / max(hi - lo, 1e-3), 0, 1)


def make_ai_matte():
    try:
        from rembg import new_session, remove
    except ImportError:
        sys.exit("--matte ai needs rembg:  py -m pip install rembg")
    session = new_session("u2net_human_seg")

    def matte(img, _args):
        out = remove(img, session=session, only_mask=True)
        return np.asarray(out.convert("L"), dtype=np.float32) / 255.0

    return matte


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("video", type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--start", type=float, default=0.0, help="seconds; should be a downbeat")
    ap.add_argument("--end", type=float, help="seconds (or give --bpm)")
    ap.add_argument("--beats", type=int, required=True, help="beats the loop spans, e.g. 8 = 2 bars")
    ap.add_argument("--bpm", type=float, help="tempo of the source, used when --end is omitted")
    ap.add_argument("--matte", choices=["ai", "luma", "chroma"], default="ai")
    ap.add_argument("--invert", action="store_true", help="luma: dark subject on light background")
    ap.add_argument("--key", default="00ff00", help="chroma key colour, hex")
    ap.add_argument("--threshold", type=float, default=0.5)
    ap.add_argument("--softness", type=float, default=0.08)
    ap.add_argument("--height", type=int, default=512, help="output mask height in px")
    ap.add_argument("--fps", type=float, default=24.0)
    ap.add_argument("--mirror", action="store_true", help="flip horizontally")
    args = ap.parse_args()

    if args.end is None:
        if args.bpm is None:
            sys.exit("give --end or --bpm")
        args.end = args.start + args.beats * 60.0 / args.bpm
    duration = args.end - args.start
    fps = min(args.fps, MAX_FRAMES / duration)

    matte = {"luma": matte_luma, "chroma": matte_chroma}.get(args.matte) or make_ai_matte()
    with tempfile.TemporaryDirectory() as t:
        paths = extract(args.video, args.start, duration, fps, Path(t))
        if not paths:
            sys.exit("ffmpeg produced no frames; check --start/--end")
        masks = []
        for i, p in enumerate(paths):
            img = Image.open(p)
            if args.mirror:
                img = img.transpose(Image.FLIP_LEFT_RIGHT)
            masks.append(matte(img, args))
            print(f"\rmatting {i + 1}/{len(paths)}", end="", flush=True)
        print()

    # Crop every frame to the union bounding box so the dancer fills the texture
    # and stays anchored (feet don't jump around between frames).
    union = np.max(np.stack(masks), axis=0) > 0.2
    ys, xs = np.nonzero(union)
    if len(xs) == 0:
        sys.exit("no subject found; try another --matte / --threshold / --invert")
    pad = int(0.03 * union.shape[0])
    x0, x1 = max(xs.min() - pad, 0), min(xs.max() + pad, union.shape[1])
    y0, y1 = max(ys.min() - pad, 0), min(ys.max() + pad, union.shape[0])
    out_h = args.height
    out_w = max(8, round(out_h * (x1 - x0) / (y1 - y0) / 4) * 4)

    out = ROOT / "dancers" / args.name
    if out.exists():
        shutil.rmtree(out)
    (out / "frames").mkdir(parents=True)
    for i, m in enumerate(masks):
        img = Image.fromarray((m[y0:y1, x0:x1] * 255).astype(np.uint8))
        img = img.resize((out_w, out_h), Image.LANCZOS).filter(ImageFilter.GaussianBlur(0.6))
        img.save(out / "frames" / f"{i:04d}.png")
    meta = {"name": args.name, "fps": round(len(masks) / duration, 3), "frames": len(masks),
            "beats": args.beats, "width": out_w, "height": out_h}
    (out / "clip.json").write_text(json.dumps(meta, indent=2))
    print(f"wrote {len(masks)} frames ({out_w}x{out_h}) to {out}")


if __name__ == "__main__":
    main()
