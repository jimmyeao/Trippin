"""Generate a still via the Alice agent API (Flux), for puppet_dancer rigs.

    py tools/alice_image.py "full-body silhouette of a ..." out.png [--wxh 1072x1920]

Submit → poll → save. The job endpoint returns the image as base64 JSON.
Reads API_KEY from ../alice.env (repo root; gitignored, owner/machine-only).
"""
import argparse
import base64
import json
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOST = "https://alice.deviousweb.com/api"
UA = {"User-Agent": "trippin-tools/1.0"}

KEY = dict(
    l.split("=", 1) for l in (ROOT / "alice.env").read_text().splitlines() if "=" in l
)["API_KEY"].strip()

ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument("prompt")
ap.add_argument("out", type=Path)
ap.add_argument("--wxh", default="1072x1920", help="widthxheight, default portrait 1072x1920")
args = ap.parse_args()
w, h = (int(v) for v in args.wxh.split("x"))

req = urllib.request.Request(
    HOST + "/agent/image",
    data=json.dumps({"prompt": args.prompt, "imageModel": "flux2",
                     "width": w, "height": h}).encode(),
    headers={"X-API-Key": KEY, "Content-Type": "application/json", **UA},
    method="POST")
job = json.loads(urllib.request.urlopen(req, timeout=60).read())
url = HOST + job["status_url"]
print("job:", job["job_id"], flush=True)

while True:
    time.sleep(4)
    r = urllib.request.urlopen(
        urllib.request.Request(url, headers={"X-API-Key": KEY, **UA}), timeout=60)
    j = json.loads(r.read())
    if j.get("status") == "processing":
        continue
    if "image_base64" not in j:
        raise SystemExit(f"job failed: {j}")
    break

args.out.write_bytes(base64.b64decode(j["image_base64"]))
print("saved", args.out, args.out.stat().st_size, "bytes")
