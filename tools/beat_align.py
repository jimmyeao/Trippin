"""Beat-align a dancer clip by timing, not pixels: find the loop's sharpest
motion accent and write its position into clip.json as `accent` (a 0..1
loop phase). At runtime the shader clock subtracts it, so the accent lands
exactly on the downbeat — while the clip's own seamless seam stays where
the generator put it. No frames are modified.

    py tools/beat_align.py dancers/stock_ruby
    py tools/beat_align.py            # every clip under dancers/
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent


def accent_phase(clip: Path) -> float | None:
    meta = json.loads((clip / "clip.json").read_text())
    n = int(meta["frames"])
    files = sorted((clip / "frames").glob("*.png"))[:n]
    frames = np.stack(
        [np.asarray(Image.open(f).convert("L"), np.float32) / 255.0 for f in files]
    )
    # Motion between consecutive frames, including the wrap -> frame 0 hop.
    e = np.abs(frames[1:] - frames[:-1]).mean(axis=(1, 2))
    wrap = np.abs(frames[0] - frames[-1]).mean()
    e = np.concatenate([e, [wrap]])
    # Lightly smooth, then take the sharpest accent. The accent pose is the
    # frame AFTER the fast change, so phase = (k+1)/n.
    e = np.convolve(np.concatenate([e, e[:2]]), np.ones(3) / 3.0, "same")[: len(e)]
    k = int(np.argmax(e))
    return (k + 1) / n, e[k]


def main() -> None:
    dry = "--dry" in sys.argv
    args = [a for a in sys.argv[1:] if a != "--dry"]
    clips = [Path(a) for a in args] or sorted(
        p for p in (ROOT / "dancers").iterdir() if (p / "clip.json").exists()
    )
    for clip in clips:
        (phase, strength) = accent_phase(clip)
        meta = json.loads((clip / "clip.json").read_text())
        print(f"{clip.name}: accent {phase:.2f} of loop (motion {strength:.3f})")
        if not dry:
            meta["accent"] = round(phase, 4)
            (clip / "clip.json").write_text(json.dumps(meta, indent=2) + "\n")


if __name__ == "__main__":
    main()
