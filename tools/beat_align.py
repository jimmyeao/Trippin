"""Beat-align a dancer clip's loop: put the sharpest accent on frame 0 and
rewrite clip.json's `beats` to the footage's own onset tempo.

    py tools/beat_align.py dancers/stock_ruby
    py tools/beat_align.py            # every clip under dancers/

The runtime stretches a clip's loop to span `beats` live beats, so if the
footage's dancer actually hits K accents during the loop, setting
beats = K maps every one of her hits onto a live beat. Rotating the loop so
its strongest accent sits at frame 0 lands a pose-snap right on the
downbeat — and moves the seam to the fastest-motion point, where a little
mismatch is invisible anyway (the new seam gets a short crossfade too).
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
FPS = 30.0
# Plausible footage tempo: an onset every 0.3–2 s of source frames.
MIN_P, MAX_P = int(0.3 * FPS), int(2.0 * FPS)


def load_frames(clip: Path) -> np.ndarray:
    files = sorted((clip / "frames").glob("*.png"))
    return np.stack(
        [np.asarray(Image.open(f).convert("L"), np.float32) / 255.0 for f in files]
    )


def motion(frames: np.ndarray) -> np.ndarray:
    """E[i] = mean |mask[i+1] - mask[i]|, lightly smoothed."""
    e = np.abs(frames[1:] - frames[:-1]).mean(axis=(1, 2))
    k = np.ones(3) / 3.0
    return np.convolve(np.concatenate([e, e[:1]]), k, mode="same")[: len(e)]


def onset_period(e: np.ndarray) -> float | None:
    """Dominant onset period in frames via autocorrelation, or None if flat."""
    x = e - e.mean()
    if x.std() < 1e-4:
        return None
    ac = np.correlate(x, x, "full")[len(x) - 1 :]
    ac = ac / (ac[0] + 1e-9)
    lo, hi = MIN_P, min(MAX_P, len(ac) // 2)
    if hi <= lo:
        return None
    lag = lo + int(np.argmax(ac[lo:hi]))
    # Sub-sample refine with the parabola through the peak.
    if 0 < lag < len(ac) - 1:
        d = ac[lag - 1] - 2 * ac[lag] + ac[lag + 1]
        if abs(d) > 1e-9:
            lag += 0.5 * (ac[lag - 1] - ac[lag + 1]) / d
    return float(lag) if ac[min(int(round(lag)), len(ac) - 1)] > 0.15 else None


def crossfade(seq: np.ndarray, w: int = 8) -> None:
    """Blend the tail into the head so the rotated seam stays seamless."""
    w = min(w, len(seq) // 4)
    for i in range(w):
        a = (i + 1) / (w + 1)
        j = len(seq) - w + i
        seq[j] = seq[j] * (1 - a) + seq[i % w] * a


def align(clip: Path, dry: bool = False) -> None:
    meta = json.loads((clip / "clip.json").read_text())
    n = int(meta["frames"])
    frames = load_frames(clip)
    e = motion(frames)
    # Accent = arrival pose after the sharpest motion; keep it off the very
    # ends so a little room survives for the seam crossfade.
    k = int(np.argmax(e[: n - 1]))
    rot = np.concatenate([frames[k + 1 :], frames[: k + 1]])
    crossfade(rot)
    period = onset_period(e)
    beats = meta.get("beats", 8)
    if period:
        beats = int(round(n / period))
        beats = max(4, min(24, beats))
    print(
        f"{clip.name}: accent at frame {k} ({e[k]:.3f}), "
        f"onset period {period and round(period, 1)} f -> beats {meta.get('beats')} -> {beats}"
    )
    if dry:
        return
    meta["beats"] = beats
    (clip / "clip.json").write_text(json.dumps(meta, indent=2) + "\n")
    for i, f in enumerate(sorted((clip / "frames").glob("*.png"))):
        Image.fromarray((rot[i] * 255).astype(np.uint8), "L").save(f)


def main() -> None:
    dry = "--dry" in sys.argv
    args = [a for a in sys.argv[1:] if a != "--dry"]
    clips = [Path(a) for a in args] or sorted(
        p for p in (ROOT / "dancers").iterdir() if (p / "clip.json").exists()
    )
    for c in clips:
        align(c, dry)


if __name__ == "__main__":
    main()
