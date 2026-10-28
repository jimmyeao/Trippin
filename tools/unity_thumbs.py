"""Render the stills the panel shows for the Unity shows (shaders/thumbs).

Each unity_* stub in shaders/scenes is recorded for a few seconds by a
player build replaying a real track's feed, and one frame becomes
shaders/thumbs/<show>.png (256x144). Re-run it when a show is added or its
look changes, before the release that ships it.

    python tools/unity_thumbs.py --player unity/TrippinStage/BuildHq/TrippinStage.exe \
        --feed feed.jsonl [--from 70] [--at 8] [shows...]

The feed comes from `trippin --dump-feed track.flac feed.jsonl`; a track with
drums at --from (seconds) gives livelier stills than the synthetic groove.
"""
import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
W, H = 256, 144


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--player", required=True)
    ap.add_argument("--feed", required=True)
    ap.add_argument("--from", dest="start", type=float, default=70.0)
    ap.add_argument("--at", type=float, default=8.0, help="seconds into the recording to take the still")
    ap.add_argument("shows", nargs="*")
    a = ap.parse_args()
    a.player = str(pathlib.Path(a.player).resolve())
    shows = a.shows or sorted(p.stem for p in (ROOT / "shaders/scenes").glob("unity_*.wgsl"))
    out = ROOT / "shaders/thumbs"
    out.mkdir(exist_ok=True)
    failed = []
    for show in shows:
        tmp = pathlib.Path(tempfile.mkdtemp(prefix="uthumb_"))
        try:
            subprocess.run(
                [a.player, "-show", show, "-replayFeed", a.feed, "-replayFrom", str(a.start),
                 "-record", str(tmp / "rec"), "-recordSeconds", str(a.at + 1.0),
                 "-logFile", str(tmp / "log.txt")],
                timeout=300, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            frames = sorted((tmp / "rec").glob("f*.jpg"))
            if not frames:
                failed.append(show)
                continue
            pick = frames[min(len(frames) - 1, int(len(frames) * a.at / (a.at + 1.0)))]
            Image.open(pick).convert("RGB").resize((W, H), Image.LANCZOS).save(out / f"{show}.png", optimize=True)
            print(show, pick.name)
        except subprocess.TimeoutExpired:
            failed.append(show)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    stale = [p.name for p in out.glob("*.png") if not (ROOT / "shaders/scenes" / (p.stem + ".wgsl")).exists()]
    for name in stale:
        (out / name).unlink()
    if stale:
        print("removed stills of removed shows:", ", ".join(stale))
    if failed:
        print("no frames for:", ", ".join(failed), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
