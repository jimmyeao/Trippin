"""Turn any dance clip into a looping Trippin dancer using an AI matte.

    py tools/ai_dancer.py clip.mp4 --name vogue [--start 0] [--len 12]

Where `stock_dancer.py` needs silhouette footage (a bright/dark plain
backdrop), this rotoscopes the person with rembg (u2net_human_seg), then
reuses stock_dancer's loop search + seam crossfade + union crop, so the
result is a normal dancers/<name>/ clip that loops seamlessly.

    py -m pip install rembg onnxruntime   # model (~170 MB) downloads on first use

The scan window is the first --len seconds after --start; the loop finder
picks the stretch inside it whose end pose best matches its start.
"""
import argparse
import json
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
import stock_dancer as sd

ROOT = sd.ROOT
FPS = sd.FPS
MAX_FRAMES = 240  # GPU texture-array limit is 256 layers


def ai_matte(video, start, seconds, work_h=448, model="u2net_human_seg"):
    """Person mask per frame (N,H,W float 0..1) over a scan window."""
    from rembg import new_session, remove

    session = new_session(model)
    w0, h0 = sd.video_size(video)
    w = max(2, int(round(w0 * work_h / h0 / 2)) * 2)
    raw = __import__("subprocess").run(
        [sd.FFMPEG, "-v", "error", "-ss", str(start), "-t", str(seconds), "-i", str(video),
         "-vf", f"fps={FPS},scale={w}:{work_h}", "-pix_fmt", "rgb24", "-f", "rawvideo", "-"],
        capture_output=True, check=True).stdout
    frames = np.frombuffer(raw, np.uint8)
    n = len(frames) // (w * work_h * 3)
    frames = frames[: n * w * work_h * 3].reshape((n, work_h, w, 3))
    out = np.empty((n, work_h, w), np.float32)
    for i, f in enumerate(frames):
        m = remove(f, session=session, only_mask=True)  # uint8 HxW
        out[i] = np.asarray(m, dtype=np.float32) / 255.0
        if i % 30 == 0:
            print(f"\rmatting {i}/{n}", end="", flush=True)
    print(f"\rmatting {n}/{n}")
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("video", type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--start", type=float, default=0.0, help="seconds into the clip to start scanning")
    ap.add_argument("--len", dest="seconds", type=float, default=14.0, help="scan window length")
    ap.add_argument("--beats", type=int, nargs="+", default=[16, 12, 8])
    ap.add_argument("--energy", type=float)
    ap.add_argument("--source", default="", help="credit / origin, stored in clip.json")
    ap.add_argument("--model", default="u2net_human_seg",
                    help="rembg session model — try isnet-general-use for flowing clothes")
    args = ap.parse_args()

    masks = ai_matte(args.video, args.start, args.seconds, model=args.model)

    # Largest person per frame — kill stray second people / debris, but keep
    # parts of the same subject the matte split off (a skirt mid-spin, a
    # flicked arm): anything within ~50px of the main body counts as her.
    from scipy.ndimage import label as cc_label, binary_closing, binary_dilation
    clean = np.empty_like(masks)
    for i, m in enumerate(masks):
        b = binary_closing(m > 0.5, structure=np.ones((7, 7)), iterations=2)
        lab, n = cc_label(b)
        if n == 0:
            continue
        sizes = np.bincount(lab.ravel()); sizes[0] = 0
        near = binary_dilation(lab == sizes.argmax(), iterations=12)
        keep = np.isin(lab, np.unique(lab[near & (lab > 0)]))
        clean[i] = np.where(keep, m, 0.0)
    masks = clean

    # Frames where the subject is cropped by the *source* footage — a loop
    # baked from these shows a hard cut-off at the frame edge, so pick a
    # --start/--len window where this count is ~0.
    edge_hit = lambda m: (m[0, :] > 0.5).any() or (m[-1, :] > 0.5).any() \
        or (m[:, 0] > 0.5).any() or (m[:, -1] > 0.5).any()
    clipped = sum(int(edge_hit(m)) for m in masks if m.max() > 0.5)
    print(f"{clipped}/{len(masks)} scan frames have the subject touching the source edge")

    blend = 8
    lengths = sorted({int(round(b * 60 / sd.REF_BPM * FPS * k))
                      for b in args.beats for k in np.linspace(0.96, 1.04, 9)})
    lengths = [L for L in lengths if L + blend + 2 < len(masks) and L <= MAX_FRAMES]
    if not lengths:
        raise SystemExit("scan window too short for any loop length — raise --len")
    s, L, cost, motion = sd.find_loop(masks, lengths, blend)
    beats = min(args.beats, key=lambda b: abs(b * 60 / sd.REF_BPM * FPS - L))
    print(f"loop: {s / FPS:.2f}s + {L / FPS:.2f}s ({beats} beats), seam cost {cost:.3f}")

    loop = masks[s:s + L].copy()
    loop_edge = sum(int(edge_hit(m)) for m in loop)
    if loop_edge:
        print(f"WARNING: {loop_edge}/{L} loop frames touch the source edge — "
              f"dancer will be visibly cut off; pick a different --start/--len")
    for i in range(blend):
        w = (i + 1) / (blend + 1)
        w = w * w * (3 - 2 * w)
        loop[L - blend + i] = (1 - w) * masks[s + L - blend + i] + w * masks[s - blend + i]

    union = loop.max(axis=0) > 0.3
    ys, xs = np.nonzero(union)
    if len(xs) == 0:
        raise SystemExit("no subject found")
    pad = int(0.04 * union.shape[0])
    x0, x1 = max(xs.min() - pad, 0), min(xs.max() + pad, union.shape[1])
    y0, y1 = max(ys.min() - pad, 0), min(ys.max() + pad, union.shape[0])
    out_h, out_w = 512, max(8, int(round((x1 - x0) / (y1 - y0) * 512 / 4)) * 4)

    out = ROOT / "dancers" / args.name
    if out.exists():
        shutil.rmtree(out)
    (out / "frames").mkdir(parents=True)
    for i, m in enumerate(loop):
        img = Image.fromarray((m[y0:y1, x0:x1] * 255).astype(np.uint8))
        img = img.resize((out_w, out_h), Image.LANCZOS).filter(ImageFilter.GaussianBlur(0.5))
        img.save(out / "frames" / f"{i:04d}.png")

    coverage = float(loop.mean())
    activity = float(motion[s:s + L].mean()) / max(coverage, 1e-3)
    energy = args.energy if args.energy is not None else float(np.clip((activity - 0.05) / 0.25, 0, 1))
    meta = {"name": args.name, "fps": FPS, "frames": L, "beats": beats,
            "width": out_w, "height": out_h, "energy": round(energy, 2), "source": args.source}
    (out / "clip.json").write_text(json.dumps(meta, indent=2) + "\n")
    print(f"wrote {L} frames ({out_w}x{out_h}), energy {energy:.2f} -> {out}")


if __name__ == "__main__":
    main()
