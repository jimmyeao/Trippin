"""Mirror a dancer clip horizontally: py tools/mirror_clip.py <name> [<out_name>]

Mirroring is real variety for silhouettes: asymmetric routines (a lean, a
hat tilt, an arm shape) read as a different move when flipped. Loops stay
seamless — the loop is just played the other way round.
"""
import json
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
src = ROOT / "dancers" / sys.argv[1]
name = sys.argv[2] if len(sys.argv) > 2 else f"{sys.argv[1]}_mir"
dst = ROOT / "dancers" / name

meta = json.loads((src / "clip.json").read_text())
(dst / "frames").mkdir(parents=True, exist_ok=True)
for i in range(meta["frames"]):
    f = Image.open(src / "frames" / f"{i:04d}.png")
    f.transpose(Image.FLIP_LEFT_RIGHT).save(dst / "frames" / f"{i:04d}.png")
meta["name"] = name
meta["source"] = (meta.get("source", "") + " (mirrored)").strip()
(dst / "clip.json").write_text(json.dumps(meta, indent=2) + "\n")
print(f"{name}: {meta['frames']} mirrored frames")
