"""Build Trippin's built-in dancer library.

    py tools/build_dancer_library.py               # real dancers from stock footage
    py tools/build_dancer_library.py --procedural  # also the choreographed shadow routines
    py tools/build_dancer_library.py --modern      # also the CMU modern-dance mocap takes

The default library is real dancers: free stock clips that are already
silhouettes on a plain background (Pixabay Content License: free to use and
modify, no attribution required, no standalone redistribution of the
unaltered clip). stock_dancer.py mattes each one, finds a seamless loop and
crops it. Sources are listed in STOCK and credited in dancers/CREDITS.md.

The procedural routines (choreo.py on a CMU skeleton) and the modern-dance
mocap clips are opt-in. Motion data credit for those: "The data used in this
project was obtained from mocap.cs.cmu.edu. The database was created with
funding from NSF EIA-0196217." BVH conversion by B. Hahne (cgspeed.com),
mirrored at github.com/una-dinosauria/cmu-mocap.
"""
import subprocess
import sys
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import choreo  # noqa: E402  (routine lengths)

TOOLS = Path(__file__).resolve().parent
ROOT = TOOLS.parent
CACHE = TOOLS / ".mocap_cache"
STOCK_CACHE = TOOLS / ".stock_cache"
BASE = "https://raw.githubusercontent.com/una-dinosauria/cmu-mocap/master/data"
UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36"

# Real dancers: (clip name, matte, 1080p file, Pixabay page). Energy is measured.
STOCK = [
    ("stock_free", "light", "https://cdn.pixabay.com/video/2022/02/23/108612-680697681_small.mp4",
     "https://pixabay.com/videos/dancer-dancing-woman-dancing-happy-108612/"),
    ("stock_heels", "dark", "https://cdn.pixabay.com/video/2023/01/24/147813-792666132_small.mp4",
     "https://pixabay.com/videos/girl-dancer-leisure-silhouette-147813/"),
    ("stock_skirt", "light", "https://cdn.pixabay.com/video/2021/05/26/75329-555531891_small.mp4",
     "https://pixabay.com/videos/dancer-girl-dancing-movement-75329/"),
    ("stock_dress", "light", "https://cdn.pixabay.com/video/2021/05/28/75547-556034400_small.mp4",
     "https://pixabay.com/videos/dancer-performance-dance-dancing-75547/"),
    ("stock_club", "light", "https://cdn.pixabay.com/video/2021/05/28/75551-556034415_small.mp4",
     "https://pixabay.com/videos/girl-dancer-party-dancing-75551/"),
    ("stock_hat", "green", "https://cdn.pixabay.com/video/2021/08/04/83833-584851771_small.mp4",
     "https://pixabay.com/videos/girl-dancer-dance-silhouette-joy-83833/"),
]

# Choreographed routines (choreo.py), opt-in with --procedural, with the
# energy auto-pilot uses to match them to the track (breakdown .. drop).
SHADOW = {"tease": 0.1, "profile": 0.25, "diva": 0.35, "frame": 0.5,
          "snake": 0.55, "wave": 0.65, "hips": 0.85}

# Real mocap, opt-in with --modern: (CMU take, clip name, camera view).
LIBRARY = [
    # Modern dance (CMU subject 05), from graceful to explosive.
    ("05_02", "arms_pirouette", "three-quarter"),
    ("05_10", "glissade_arms", "three-quarter"),
    ("05_09", "glissade", "three-quarter"),
    ("05_04", "arabesque_backbend", "three-quarter"),
    ("05_03", "arabesque_turn", "three-quarter"),
    ("05_11", "pirouette_steps", "three-quarter"),
    ("05_13", "jetes_pirouette", "three-quarter"),
    ("05_07", "jete_arabesque", "three-quarter"),
    ("05_08", "rond_de_jambe_leap", "three-quarter"),
    ("05_16", "jete_en_tournant", "three-quarter"),
    ("05_19", "leap_backbend", "three-quarter"),
    # Slow lean with sweeping arms (subject 49), for deep breakdowns.
    ("49_14", "slow_lean", "three-quarter"),
]


def fetch(take):
    CACHE.mkdir(exist_ok=True)
    path = CACHE / f"{take}.bvh"
    if not path.exists():
        subject = take.split("_")[0].zfill(3)
        url = f"{BASE}/{subject}/{take}.bvh"
        print(f"downloading {url}")
        urllib.request.urlretrieve(url, path)
    return path


def fetch_stock(url):
    STOCK_CACHE.mkdir(exist_ok=True)
    path = STOCK_CACHE / url.rsplit("/", 1)[1]
    if not path.exists():
        print(f"downloading {url}")
        req = urllib.request.Request(url, headers={"User-Agent": UA, "Referer": "https://pixabay.com/"})
        with urllib.request.urlopen(req) as r:
            path.write_bytes(r.read())
    return path


def write_credits():
    lines = ["# Dancer credits", "",
             "The built-in dancers are silhouettes derived from free stock footage",
             "on Pixabay (Pixabay Content License), matted, looped and cropped by",
             "`tools/stock_dancer.py`.", ""]
    lines += [f"- `{name}`: {page}" for name, _, _, page in STOCK]
    (ROOT / "dancers" / "CREDITS.md").write_text("\n".join(lines) + "\n")


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--procedural", action="store_true", help="also build the choreographed shadow routines")
    ap.add_argument("--modern", action="store_true", help="also render the CMU modern-dance takes")
    args = ap.parse_args()
    for name, matte, url, page in STOCK:
        print(f"== {name}")
        subprocess.run([sys.executable, str(TOOLS / "stock_dancer.py"), str(fetch_stock(url)), "--name", name,
                        "--matte", matte, "--source", f"Pixabay: {page}"], check=True)
    write_credits()
    if args.procedural:
        fetch("05_02")  # choreo.py borrows this take's skeleton
        for routine, energy in SHADOW.items():
            print(f"== shadow_{routine}")
            bvh = CACHE / f"choreo_{routine}.bvh"
            subprocess.run([sys.executable, str(TOOLS / "choreo.py"), routine, "--out", str(bvh)], check=True)
            subprocess.run([sys.executable, str(TOOLS / "mocap_dancer.py"), str(bvh), "--name", f"shadow_{routine}",
                            "--view", "front", "--whole", "--beats", str(choreo.routine_beats(routine)),
                            "--energy", str(energy)], check=True)
    if args.modern:
        for take, name, view in LIBRARY:
            print(f"== {name} ({take}, {view})")
            subprocess.run([sys.executable, str(TOOLS / "mocap_dancer.py"), str(fetch(take)),
                            "--name", name, "--view", view], check=True)


if __name__ == "__main__":
    main()
