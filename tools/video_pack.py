"""Pack video clips for Trippin's `video_*` scenes (see src/video.rs).

    python tools/video_pack.py clip.mp4 [more.mp4 ...] --out videos/ [--quality 80] [--xfade 0.75]

Each input becomes `<stem>.tvid`: frames scaled and centre-cropped to
1280x720, JPEG-encoded, with an index. The loop seam is hidden by a
crossfade: the last `--xfade` seconds dissolve into the first ones, and those
leading frames are dropped, so the last frame flows straight into frame 0.
Needs ffmpeg (on PATH, or `pip install imageio-ffmpeg`) and Pillow.
"""
import argparse, io, os, shutil, struct, subprocess, sys

import numpy as np
from PIL import Image

W, H = 1280, 720


def ffmpeg_exe():
    exe = shutil.which('ffmpeg')
    if exe:
        return exe
    import imageio_ffmpeg
    return imageio_ffmpeg.get_ffmpeg_exe()


def probe_fps(ff, path):
    out = subprocess.run([ff, '-hide_banner', '-i', path], capture_output=True, text=True).stderr
    for tok in out.split(','):
        tok = tok.strip()
        if tok.endswith(' fps'):
            return float(tok[:-4])
    raise SystemExit(f'{path}: no fps in ffmpeg output')


def frames(ff, path):
    vf = f'scale={W}:{H}:force_original_aspect_ratio=increase,crop={W}:{H}'
    p = subprocess.Popen([ff, '-v', 'error', '-i', path, '-vf', vf, '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-'],
                         stdout=subprocess.PIPE)
    n = W * H * 3
    while True:
        b = p.stdout.read(n)
        if len(b) < n:
            break
        yield np.frombuffer(b, np.uint8).reshape(H, W, 3)
    p.wait()


def pack(ff, src, out_dir, quality, xfade):
    fps = probe_fps(ff, src)
    fr = [f.astype(np.float32) for f in frames(ff, src)]
    k = min(int(round(xfade * fps)), len(fr) // 3)
    if k > 0:
        # Output = frames[k:], whose last k frames dissolve into frames[0:k]:
        # it ends on frames[k-1] and restarts on frames[k], seamlessly.
        n = len(fr)
        for j in range(k):
            w = (j + 1) / (k + 1)
            w = w * w * (3 - 2 * w)
            fr[n - k + j] = fr[n - k + j] * (1 - w) + fr[j] * w
        fr = fr[k:]
    jpgs = []
    for f in fr:
        b = io.BytesIO()
        Image.fromarray(np.clip(f + 0.5, 0, 255).astype(np.uint8)).save(b, 'JPEG', quality=quality, optimize=True, subsampling=0 if quality >= 90 else 2)
        jpgs.append(b.getvalue())
    name = os.path.splitext(os.path.basename(src))[0]
    dst = os.path.join(out_dir, name + '.tvid')
    off = 24 + 8 * len(jpgs)
    with open(dst, 'wb') as o:
        o.write(b'TVID' + struct.pack('<IIIIf', 1, W, H, len(jpgs), fps))
        for j in jpgs:
            o.write(struct.pack('<II', off, len(j)))
            off += len(j)
        for j in jpgs:
            o.write(j)
    print(f'{dst}: {len(jpgs)} frames at {fps:g} fps ({len(jpgs) / fps:.1f} s loop), {os.path.getsize(dst) >> 20} MB')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('inputs', nargs='+')
    ap.add_argument('--out', default='videos')
    ap.add_argument('--quality', type=int, default=80)
    ap.add_argument('--xfade', type=float, default=0.75, help='loop crossfade, seconds')
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    ff = ffmpeg_exe()
    for src in a.inputs:
        pack(ff, src, a.out, a.quality, a.xfade)


if __name__ == '__main__':
    sys.exit(main())
