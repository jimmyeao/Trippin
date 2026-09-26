"""Contact sheet of a dancer clip, for eyeballing: py tools/contact_sheet.py <clip> <out.png> [count]"""
import json
import sys
from pathlib import Path

from PIL import Image

clip = Path(__file__).resolve().parent.parent / "dancers" / sys.argv[1]
count = int(sys.argv[3]) if len(sys.argv) > 3 else 8
meta = json.loads((clip / "clip.json").read_text())
frames = [Image.open(clip / "frames" / f"{i * meta['frames'] // count:04d}.png") for i in range(count)]
w, h = frames[0].size
sheet = Image.new("L", (w * count, h), 255)
for k, f in enumerate(frames):
    # Black silhouettes on white read like the real thing.
    sheet.paste(f.point(lambda v: 255 - v), (k * w, 0))
sheet.thumbnail((1600, 400))
sheet.save(sys.argv[2])
