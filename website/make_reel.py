"""Renders the landing-page hero reel from real Trippin scenes.

    python make_reel.py            # render frames (needs target/verify build) + encode
    python make_reel.py --encode   # re-encode from already rendered frames

The cut list accelerates (4 beats -> 2 -> 1 -> half-beat) at the snapper's
synthetic 126 BPM, then lands on a long hold. Output: site/img/reel.mp4 (+ poster).
"""
import subprocess, sys, pathlib, shutil, math

ROOT = pathlib.Path(__file__).resolve().parent.parent
EXE = ROOT / "target/verify/release/trippin.exe"
FFMPEG = r"C:\Users\jimmy\AppData\Roaming\Python\Python313\site-packages\imageio_ffmpeg\binaries\ffmpeg-win-x86_64-v7.1.exe"
WORK = ROOT / "website/reel_work"          # git-ignored scratch
OUT = ROOT / "website/site/img"
W, H, FPS, BPM = 1280, 720, 30, 126.0
BEAT = 60.0 / BPM

# (scene, beats, start time in the scene's own timeline)
CUTS = [
    ("stage_rig", 4, 12), ("neon_coaster", 4, 20),
    ("rooftop_city", 2, 14), ("glass_monoliths", 2, 18), ("chrome_bloom", 2, 16), ("laser_show", 2, 22),
    ("storm_front", 1, 20), ("deep_blue", 1, 16), ("megastructure", 1, 20), ("infinity_room", 1, 14),
    ("neon_alley", 1, 18), ("gyro_core", 1, 14), ("salt_flats", 1, 22), ("fire_mandala", 1, 16),
    ("cathedral", .5, 18), ("kifs_cathedral", .5, 14), ("warehouse_haze", .5, 20), ("crystal_cave", .5, 16),
    ("orbit_night", .5, 20), ("chrome_ferro", .5, 18), ("laser_cavern", .5, 22), ("spiral_galaxy", .5, 12),
    ("stage_rig", 4, 30),
]


def frame_plan():
    """[(scene, start_t, n_frames)] with cumulative rounding so cuts stay on the beat."""
    plan, acc_beats, done = [], 0.0, 0
    for scene, beats, t0 in CUTS:
        acc_beats += beats
        end = round(acc_beats * BEAT * FPS)
        plan.append((scene, t0, end - done))
        done = end
    return plan


def render(plan):
    shutil.rmtree(WORK, ignore_errors=True)
    # one process per distinct (scene, t0) so a scene reused later restarts cleanly
    for i, (scene, t0, n) in enumerate(plan):
        times = ",".join(f"{t0 + k / FPS:.4f}" for k in range(n))
        out = WORK / f"{i:02d}"
        cmd = [str(EXE), "--snap", scene, "--snap-size", f"{W}x{H}", "--snap-at", times,
               "--snap-bench", "0", "--snap-out", str(out)]
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
        if r.returncode:
            sys.exit(f"{scene} failed:\n{r.stdout}\n{r.stderr}")
        print(f"{i:02d} {scene:16s} {n:3d} frames")


def encode(plan):
    seq = WORK / "seq"
    shutil.rmtree(seq, ignore_errors=True)
    seq.mkdir(parents=True)
    k = 0
    for i, (scene, _, n) in enumerate(plan):
        d = WORK / f"{i:02d}"
        for j in range(n):
            src = d / (f"{scene}_{j}.png" if n > 1 else f"{scene}.png")
            shutil.copy(src, seq / f"f{k:05d}.png")
            k += 1
    OUT.mkdir(parents=True, exist_ok=True)
    common = ["-y", "-loglevel", "error", "-framerate", str(FPS), "-i", str(seq / "f%05d.png")]
    # web video: H.264, no audio, fast start. CRF 30 capped at 2.8 Mbit keeps neon gradients clean.
    subprocess.run([FFMPEG, *common, "-an", "-c:v", "libx264", "-crf", "30", "-preset", "veryslow", "-maxrate", "2800k", "-bufsize", "5600k",
                    "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(OUT / "reel.mp4")], check=True)
    poster = seq / f"f{int(BEAT * FPS * 1):05d}.png"
    subprocess.run([FFMPEG, "-y", "-loglevel", "error", "-i", str(poster), "-q:v", "4",
                    str(OUT / "reel-poster.jpg")], check=True)
    import json
    t, cuts = 0.0, []
    for scene, n_frames in [(sc, n) for sc, _, n in plan]:
        cuts.append([round(t, 3), scene]); t += n_frames / FPS
    (OUT / "reel.json").write_text(json.dumps({"duration": round(t, 3), "cuts": cuts}))
    print(f"{k} frames = {k / FPS:.1f}s ->", OUT / "reel.mp4")


if __name__ == "__main__":
    plan = frame_plan()
    if "--encode" not in sys.argv:
        render(plan)
    encode(plan)
