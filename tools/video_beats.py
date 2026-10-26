"""Find the beats in a packed video clip, for tempo-matched playback.

    python tools/video_beats.py packs/club-nights/videos/club_lasers.tvid [...] [--plot]

Footage has its own rhythm: dancers bounce, a speaker cone pumps, lights
flash. This finds it, so Trippin can steer the play head to land each of
those hits on a beat of the music (video.rs, beat-matched playback) and the
clip's speed follows the track's tempo.

Per frame (on a 96x54 grey copy) it measures motion (mean absolute frame
difference) and light onsets (rise in the brightest 5% of pixels). Each
signal is detrended, its autocorrelation gives the clip's period, and the
hits are the strongest frames one period apart along the loop (periods on
the video model's 8-frame chunk grid are ignored): light onsets
for light-led clips, the turn of a movement (motion minima between surges)
for dance. Writes `<clip>.beats.json` next to the clip:

    {"fps": 24, "frames": 271, "period": 11.6, "hits": [3.0, 14.7, ...],
     "strength": 0.62, "source": "motion"}

`hits` are frame positions (fractional) around the loop. A clip with no
clear rhythm (strength < 0.25) gets no file and keeps drive-led playback.
"""
import io, json, os, struct, sys

import numpy as np
from PIL import Image


def frames(path):
    data = open(path, 'rb').read()
    assert data[:4] == b'TVID'
    ver, w, h, n = struct.unpack('<4I', data[4:20])
    fps = struct.unpack('<f', data[20:24])[0]
    idx = [struct.unpack('<2I', data[24 + 8 * i:32 + 8 * i]) for i in range(n)]
    out = []
    for off, ln in idx:
        im = Image.open(io.BytesIO(data[off:off + ln])).convert('L').resize((96, 54), Image.BILINEAR)
        out.append(np.asarray(im, dtype=np.float32) / 255.0)
    return np.stack(out), fps


def detrend(x, win):
    k = np.ones(win) / win
    pad = np.concatenate([x[-win:], x, x[:win]])
    base = np.convolve(pad, k, 'same')[win:-win]
    return x - base


def period_of(x, fps):
    """Best period (frames) between 0.3 s and 1.5 s, with its normalised
    autocorrelation, on a circular (looping) signal."""
    x = x - x.mean()
    n = len(x)
    if np.allclose(x, 0):
        return None, 0.0
    ac = np.array([np.dot(x, np.roll(x, -l)) for l in range(n)]) / np.dot(x, x)
    lo, hi = max(2, int(0.3 * fps)), min(n // 3, int(1.5 * fps))
    if hi <= lo + 2:
        return None, 0.0
    # The video model builds clips in 8-frame chunks, which leaves a faint
    # rhythm at multiples of 8 frames in every clip: never a real beat.
    cand = ac.copy()
    for k in range(8, n, 8):
        cand[max(0, k - 1):k + 2] = -1.0
    l = lo + int(np.argmax(cand[lo:hi]))
    # Parabolic refine.
    a, b, c = ac[l - 1], ac[l], ac[l + 1]
    d = a - 2 * b + c
    off = 0.5 * (a - c) / d if abs(d) > 1e-9 else 0.0
    return l + max(-0.5, min(0.5, off)), float(b)


def hits_along(sig, period):
    """One hit per period around the loop: the comb phase with the most
    signal, then each hit nudged to the local maximum within a quarter
    period (footage tempo wanders)."""
    n = len(sig)
    best, phase = -1e9, 0.0
    for p in np.linspace(0, period, 48, endpoint=False):
        pos = np.arange(p, n, period)
        v = np.interp(pos, np.arange(n), sig, period=n).sum() / len(pos)
        if v > best:
            best, phase = v, p
    hits = []
    r = period / 4
    for c in np.arange(phase, n, period):
        lo, hi = int(np.floor(c - r)), int(np.ceil(c + r))
        ii = np.arange(lo, hi + 1)
        vals = sig[ii % n]
        k = int(np.argmax(vals))
        # Sub-frame peak.
        if 0 < k < len(ii) - 1:
            a, b, cc = vals[k - 1], vals[k], vals[k + 1]
            d = a - 2 * b + cc
            off = 0.5 * (a - cc) / d if abs(d) > 1e-9 else 0.0
        else:
            off = 0.0
        hits.append(float((ii[k] + max(-0.5, min(0.5, off))) % n))
    hits = sorted(set(round(h, 2) for h in hits))
    # Drop near-duplicates (two maxima picked across the seam).
    out = []
    for h in hits:
        if not out or h - out[-1] > period * 0.5:
            out.append(h)
    if len(out) > 1 and (out[0] + n) - out[-1] < period * 0.5:
        out.pop()
    return out


def analyse(path):
    f, fps = frames(path)
    n = len(f)
    motion = np.array([np.abs(f[i] - f[i - 1]).mean() for i in range(n)])
    thr = np.quantile(f, 0.95)
    light = np.array([np.clip(f[i] - f[i - 1], 0, None)[f[i] > thr].sum() for i in range(n)])
    # The model's frame interpolation flickers the highlights on a 2- and
    # 4-frame cycle (every clip has it, and it puts fake periods on every
    # even frame count); a circular 4-frame box removes that cycle exactly.
    box = lambda s: (s + np.roll(s, 1) + np.roll(s, 2) + np.roll(s, 3)) / 4.0
    motion, light = box(motion), box(light)
    win = max(3, int(fps * 1.0))
    cands = []
    for name, sig in (('light', detrend(light, win)), ('motion', -detrend(motion, win))):
        # Motion: the hit is the turn of a move, a dip between surges.
        p, s = period_of(sig, fps)
        if p:
            cands.append((s, name, p, sig))
    if not cands:
        return None
    s, name, p, sig = max(cands, key=lambda c: c[0])
    return {
        'fps': fps,
        'frames': n,
        'period': round(float(p), 3),
        'hits': hits_along(sig, p),
        'strength': round(s, 3),
        'source': name,
    }


def main():
    for path in [a for a in sys.argv[1:] if not a.startswith('--')]:
        r = analyse(path)
        out = os.path.splitext(path)[0] + '.beats.json'
        if not r or r['strength'] < 0.25:
            print(f"{os.path.basename(path)}: no clear rhythm ({r and r['strength']})")
            if os.path.exists(out):
                os.remove(out)
            continue
        json.dump(r, open(out, 'w'), indent=1)
        print(f"{os.path.basename(path)}: {r['source']} period {r['period']:.1f} frames "
              f"({60 * r['fps'] / r['period']:.0f} 'bpm'), {len(r['hits'])} hits, strength {r['strength']}")


if __name__ == '__main__':
    main()
