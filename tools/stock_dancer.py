"""Turn a stock silhouette dance clip into a seamlessly looping Trippin dancer.

    py tools/stock_dancer.py clip.mp4 --name stock_heels --matte dark
    py tools/stock_dancer.py clip.mp4 --name stock_hat --matte green

For footage that is already a silhouette on a plain background (plenty of
free stock clips are): the dancer is separated by brightness or colour, so
the edges are exact. Real dance doesn't loop, so the tool searches the clip
for the stretch whose end pose best matches its start (while she's actually
moving), crossfades that seam, crops to the dancer and writes dancers/<name>/.

Loop lengths are chosen to be a whole number of bars at 124 BPM (8, 12 or
16 beats), so at typical EDM tempos the dance plays at its natural speed.
Trippin retimes it to the live tempo.

Mattes:
  light   bright dancer on a dark background (white silhouette on black)
  dark    dark dancer on a bright background (black silhouette on white)
  green   coloured (green) dancer on a dark background
  bg      subject darker or a different colour than a seamless-paper
          backdrop (silhouette on red/blue/green paper, whatever the luma)
"""
import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
FPS = 30
REF_BPM = 124.0
SCOUT_HEIGHT = 270         # first pass: find where the dancer is

# imageio-ffmpeg bundles an ffmpeg binary (but no ffprobe) if none is on PATH.
FFMPEG = shutil.which("ffmpeg")
if not FFMPEG:
    try:
        import imageio_ffmpeg
        FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()
    except ImportError:
        FFMPEG = "ffmpeg"


def video_size(video):
    try:
        probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                                "stream=width,height", "-of", "csv=p=0", str(video)],
                               capture_output=True, text=True, check=True).stdout.strip().split(",")
        return int(probe[0]), int(probe[1])
    except (FileNotFoundError, subprocess.CalledProcessError):
        # No ffprobe: `ffmpeg -i` prints "Stream #... Video: ... WxH" on stderr.
        err = subprocess.run([FFMPEG, "-i", str(video)], capture_output=True, text=True).stderr
        m = re.search(r"Video:.*? (\d{2,5})x(\d{2,5})", err)
        if not m:
            raise SystemExit("can't read video size; install ffprobe or imageio-ffmpeg")
        return int(m.group(1)), int(m.group(2))


def read_frames(video, rgb, height, crop=None):
    """All frames as uint8 arrays (N, H, W[, 3]) at FPS. `crop` = (x, y, w, h)
    in source pixels is cut out at full resolution before scaling to `height`."""
    w0, h0 = video_size(video)
    vf = f"fps={FPS}"
    if crop:
        x, y, cw, chh = crop
        vf += f",crop={cw}:{chh}:{x}:{y}"
        w0, h0 = cw, chh
    h = min(height, h0)
    w = max(2, int(round(w0 * h / h0 / 2)) * 2)
    vf += f",scale={w}:{h}"
    ch = 3 if rgb else 1
    raw = subprocess.run([FFMPEG, "-v", "error", "-i", str(video), "-vf", vf,
                          "-pix_fmt", "rgb24" if rgb else "gray", "-f", "rawvideo", "-"],
                         capture_output=True, check=True).stdout
    frames = np.frombuffer(raw, np.uint8)
    n = len(frames) // (w * h * ch)
    return frames[: n * w * h * ch].reshape((n, h, w, ch) if rgb else (n, h, w))


def matte(frames, kind, threshold, softness):
    f = frames.astype(np.float32) / 255.0
    if kind == "green":
        # How much greener than it is red/blue: the dancer, not the dark background.
        v = f[..., 1] - np.maximum(f[..., 0], f[..., 2]) * 0.5
    elif kind == "bg":
        # Distance from the backdrop colour (the frame's median): the subject is
        # whatever isn't seamless paper. Works for saturated backdrops where a
        # pure luma matte would key the background itself (deep red/blue paper).
        bg = np.median(f.reshape(len(f), -1, 3), axis=1)[:, None, None, :]
        v = np.sqrt(((f - bg) ** 2).sum(-1) / 3.0)
    elif kind == "dark":
        v = 1.0 - f
    else:
        v = f
    lo, hi = threshold - softness, threshold + softness
    return np.clip((v - lo) / (hi - lo), 0.0, 1.0)


def measure_beat_seconds(masks):
    """The footage's own tempo: dominant period of the mask-motion series, in
    seconds. Musicians (and dancers) hit on a period; choosing the loop length
    in whole periods keeps those hits on the grid Trippin retimes to. Returns
    None when there's no clear periodic motion."""
    small = np.stack([np.asarray(Image.fromarray((m * 255).astype(np.uint8)).resize((64, 36), Image.BILINEAR),
                                 np.float32) / 255.0 for m in masks])
    motion = np.abs(small[1:] - small[:-1]).mean(axis=(1, 2))
    motion = motion - motion.mean()
    var = float(motion.var())
    if var < 1e-8:
        return None
    lo, hi = int(0.18 * FPS), int(0.9 * FPS)          # 66-330 hits/min
    ac = np.array([float((motion[: len(motion) - l] * motion[l:]).mean())
                   for l in range(hi + 1)]) / var
    lag = lo + int(np.argmax(ac[lo:]))
    # Sub-harmonic unwrap: if the hits are really twice as fast (8ths/16ths),
    # half lags correlate nearly as well — take the shortest strong one.
    while lag >= 2 * lo and ac[lag // 2] > 0.8 * ac[lag]:
        lag //= 2
    if ac[lag] < 0.15:
        return None
    # Sub-frame refine around the peak.
    if 0 < lag < hi:
        y0, y1, y2 = ac[lag - 1], ac[lag], ac[lag + 1]
        lag = lag + 0.5 * (y0 - y2) / max(y0 - 2 * y1 + y2, 1e-9)
    return lag / FPS


def find_loop(masks, lengths, blend):
    """Best (start, length): end pose matches start pose, the dancer is moving
    during the loop, and there are `blend` frames before the start to
    crossfade the seam with."""
    n = len(masks)
    small = np.stack([np.asarray(Image.fromarray((m * 255).astype(np.uint8)).resize((64, 36), Image.BILINEAR),
                                 np.float32) / 255.0 for m in masks])
    vel = np.zeros_like(small)
    vel[1:] = small[1:] - small[:-1]
    motion = np.abs(vel).mean(axis=(1, 2))
    cum = np.concatenate([[0.0], np.cumsum(motion)])
    typical = float(np.median(motion[1:]))
    best = (np.inf, 0, 0)
    for L in lengths:
        for s in range(blend, n - L - 1):
            activity = (cum[s + L] - cum[s]) / L
            if activity < 0.6 * typical:      # she's barely moving here: not a dance loop
                continue
            pose = np.abs(small[s] - small[s + L]).mean()
            move = np.abs(vel[s + 1] - vel[s + L + 1]).mean()
            cost = (pose + 2.0 * move) / (activity + 0.25 * typical)
            if cost < best[0]:
                best = (cost, s, L)
    if best[0] == np.inf:
        raise SystemExit("no active stretch long enough for a loop")
    return best[1], best[2], best[0], motion


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("video", type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--matte", choices=["light", "dark", "green", "bg"], default="light")
    ap.add_argument("--threshold", type=float, default=0.5)
    ap.add_argument("--softness", type=float, default=0.12)
    ap.add_argument("--height", type=int, default=512)
    ap.add_argument("--beats", type=int, nargs="+", default=[16, 12, 8],
                    help="loop lengths to consider, in beats at --bpm")
    ap.add_argument("--bpm", default=str(REF_BPM),
                    help="tempo the loop beats are counted at: a number, or 'auto' "
                         "to measure the footage's own playing tempo (musicians — "
                         "their hits then land on the live grid)")
    ap.add_argument("--fill", action="store_true",
                    help="close gaps and fill holes in the matte (for glow/outline footage)")
    ap.add_argument("--energy", type=float, help="0 calm .. 1 driving (default: measured)")
    ap.add_argument("--source", default="", help="credit / origin, stored in clip.json")
    args = ap.parse_args()

    rgb = args.matte in ("green", "bg")
    # Pass 1 (low res): where does the dancer go over the whole clip?
    scout = matte(read_frames(args.video, rgb, SCOUT_HEIGHT), args.matte, args.threshold, args.softness)
    ref_bpm = REF_BPM
    if args.bpm == "auto":
        beat_sec = measure_beat_seconds(scout)
        if beat_sec is not None:
            ref_bpm = 60.0 / beat_sec
            # Hits can come twice per beat: snap into a sensible tempo band.
            while ref_bpm > 190.0:
                ref_bpm /= 2.0
            while ref_bpm < 66.0:
                ref_bpm *= 2.0
            print(f"measured footage tempo: {ref_bpm:.1f} BPM")
        else:
            print(f"no clear tempo in the footage; keeping {REF_BPM:.0f} BPM")
    else:
        ref_bpm = float(args.bpm)
    ys, xs = np.nonzero(scout.max(axis=0) > 0.3)
    if len(xs) == 0:
        raise SystemExit("no dancer found; try another --matte or --threshold")
    w0, h0 = video_size(args.video)
    k = h0 / scout.shape[1]
    pad = 0.06 * (ys.max() - ys.min())
    x0 = max(int((xs.min() - pad) * k) // 2 * 2, 0)
    y0 = max(int((ys.min() - pad) * k) // 2 * 2, 0)
    x1 = min(int((xs.max() + pad) * k), w0)
    y1 = min(int((ys.max() + pad) * k), h0)
    del scout
    # Pass 2 (full detail): just the dancer's region, at about output size.
    frames = read_frames(args.video, rgb, int(args.height * 1.25), crop=(x0, y0, (x1 - x0) // 2 * 2, (y1 - y0) // 2 * 2))
    masks = matte(frames, args.matte, args.threshold, args.softness)
    if args.fill:
        # Outline/glow footage mattes hollow: close the rim, fill the body.
        from scipy.ndimage import binary_closing, binary_fill_holes
        solid = np.empty_like(masks)
        for i, m in enumerate(masks):
            b = binary_closing(m > 0.5, structure=np.ones((9, 9)), iterations=3)
            solid[i] = np.where(b, np.maximum(m, 0.6), m)
            solid[i] = np.where(binary_fill_holes(solid[i] > 0.4), np.maximum(solid[i], 0.6), solid[i])
        masks = solid

    # Keep only the largest blob per frame: backdrop debris, poles or a second
    # figure at the frame edge otherwise render as stray objects next to her.
    # Close first so a limb split by a matte gap still joins the body.
    from scipy.ndimage import label as cc_label
    clean = np.empty_like(masks)
    for i, m in enumerate(masks):
        b = binary_closing(m > 0.5, structure=np.ones((7, 7)), iterations=2) if args.fill else m > 0.5
        lab, n = cc_label(b)
        if n == 0:
            continue
        sizes = np.bincount(lab.ravel())
        sizes[0] = 0
        clean[i] = np.where(lab == sizes.argmax(), m, 0.0)
    masks = clean
    del frames
    print(f"{len(masks)} frames at {FPS} fps ({len(masks) / FPS:.1f}s), dancer region {x1 - x0}x{y1 - y0}px")

    blend = 8
    # Lengths within 4% of a whole number of beats at the reference tempo.
    lengths = sorted({int(round(b * 60 / ref_bpm * FPS * k))
                      for b in args.beats for k in np.linspace(0.96, 1.04, 9)})
    lengths = [L for L in lengths if L + blend + 2 < len(masks) and L <= 240]
    if not lengths:
        raise SystemExit("clip too short for any loop length")
    s, L, cost, motion = find_loop(masks, lengths, blend)
    beats = min(args.beats, key=lambda b: abs(b * 60 / ref_bpm * FPS - L))
    print(f"loop: {s / FPS:.2f}s + {L / FPS:.2f}s ({beats} beats at {ref_bpm:.0f} BPM), seam cost {cost:.3f}")

    # Crossfade the seam: the last frames blend into the frames just before the start.
    loop = masks[s:s + L].copy()
    for i in range(blend):
        w = (i + 1) / (blend + 1)
        w = w * w * (3 - 2 * w)
        loop[L - blend + i] = (1 - w) * masks[s + L - blend + i] + w * masks[s - blend + i]

    # Crop to everywhere the dancer goes during the loop, so she stays anchored.
    union = loop.max(axis=0) > 0.3
    ys, xs = np.nonzero(union)
    pad = int(0.04 * union.shape[0])
    x0, x1 = max(xs.min() - pad, 0), min(xs.max() + pad, union.shape[1])
    y0, y1 = max(ys.min() - pad, 0), min(ys.max() + pad, union.shape[0])
    out_h = args.height
    out_w = max(8, int(round((x1 - x0) / (y1 - y0) * out_h / 4)) * 4)

    out = ROOT / "dancers" / args.name
    if out.exists():
        shutil.rmtree(out)
    (out / "frames").mkdir(parents=True)
    for i, m in enumerate(loop):
        img = Image.fromarray((m[y0:y1, x0:x1] * 255).astype(np.uint8))
        img = img.resize((out_w, out_h), Image.LANCZOS).filter(ImageFilter.GaussianBlur(0.5))
        img.save(out / "frames" / f"{i:04d}.png")

    # Movement as a share of the dancer's own size, so small and large clips compare.
    coverage = float(loop.mean())
    activity = float(motion[s:s + L].mean()) / max(coverage, 1e-3)
    energy = args.energy if args.energy is not None else float(np.clip((activity - 0.05) / 0.25, 0, 1))
    print(f"activity {activity:.3f} (movement per frame relative to body area)")
    meta = {"name": args.name, "fps": FPS, "frames": L, "beats": beats, "width": out_w, "height": out_h,
            "energy": round(energy, 2), "source": args.source}
    (out / "clip.json").write_text(json.dumps(meta, indent=2))
    print(f"wrote {L} frames ({out_w}x{out_h}), energy {energy:.2f} -> {out}")


if __name__ == "__main__":
    main()
