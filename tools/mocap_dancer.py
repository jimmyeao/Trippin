"""Render real motion capture as a curvy female silhouette clip for Trippin.

    py tools/mocap_dancer.py 60_03.bvh --name salsa --view three-quarter

Takes a BVH dance take (the CMU mocap library, in cgspeed's BVH conversion,
works as-is), finds a seamless loop at the dance's own tempo, and renders it
as a smooth silhouette with an hourglass body and simulated long hair. It
writes dancers/<name>/ (clip.json + PNG masks), which Trippin retimes to the
live BPM.

Motion data credit (CMU): "The data used in this project was obtained from
mocap.cs.cmu.edu. The database was created with funding from NSF EIA-0196217."
"""
import argparse
import json
import math
import shutil
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter
from scipy import ndimage
from scipy.interpolate import PchipInterpolator

ROOT = Path(__file__).resolve().parent.parent
MAX_FRAMES = 240


# --- BVH ---------------------------------------------------------------------

class Joint:
    def __init__(self, name, parent):
        self.name, self.parent = name, parent
        self.offset = np.zeros(3)
        self.channels = []
        self.index = 0


def parse_hierarchy(tokens):
    """Joints (in file order, end sites included) from BVH tokens up to MOTION."""
    pos = 0
    joints, stack = [], []
    motion_start = tokens.index("MOTION")
    while pos < motion_start:
        tok = tokens[pos]
        if tok in ("ROOT", "JOINT"):
            joints.append(Joint(tokens[pos + 1], stack[-1] if stack else -1))
            pos += 2
        elif tok == "End":
            joints.append(Joint(joints[stack[-1]].name + "_end", stack[-1]))
            pos += 2
        elif tok == "{":
            stack.append(len(joints) - 1)
            pos += 1
        elif tok == "}":
            stack.pop()
            pos += 1
        elif tok == "OFFSET":
            joints[stack[-1]].offset = np.array([float(t) for t in tokens[pos + 1:pos + 4]])
            pos += 4
        elif tok == "CHANNELS":
            n = int(tokens[pos + 1])
            joints[stack[-1]].channels = tokens[pos + 2:pos + 2 + n]
            pos += 2 + n
        else:
            pos += 1
    return joints


def load_bvh(path):
    """Returns (joint names, world positions (F, J, 3), frame time)."""
    tokens = Path(path).read_text().split()
    joints = parse_hierarchy(tokens)
    motion_start = tokens.index("MOTION")
    frames = int(tokens[motion_start + 2])
    frame_time = float(tokens[motion_start + 5])
    data = np.array(tokens[motion_start + 6:], dtype=np.float64).reshape(frames, -1)

    col = 0
    for j in joints:
        j.index = col
        col += len(j.channels)

    F, J = frames, len(joints)
    world_r = np.zeros((J, F, 3, 3))
    world_p = np.zeros((J, F, 3))
    for ji, j in enumerate(joints):
        rot = np.broadcast_to(np.eye(3), (F, 3, 3)).copy()
        trans = np.broadcast_to(j.offset, (F, 3)).copy()
        for k, ch in enumerate(j.channels):
            v = data[:, j.index + k]
            if ch.endswith("position"):
                trans[:, "XYZ".index(ch[0])] = v + j.offset["XYZ".index(ch[0])]
            else:
                rot = rot @ axis_rot(ch[0], np.radians(v))
        if j.parent < 0:
            world_r[ji], world_p[ji] = rot, trans
        else:
            pr = world_r[j.parent]
            world_p[ji] = world_p[j.parent] + np.einsum("fij,fj->fi", pr, trans)
            world_r[ji] = pr @ rot
    return [j.name for j in joints], world_p.transpose(1, 0, 2), frame_time


def axis_rot(axis, a):
    c, s = np.cos(a), np.sin(a)
    m = np.zeros((len(a), 3, 3))
    i = "XYZ".index(axis)
    j, k = [(1, 2), (2, 0), (0, 1)][i]
    m[:, i, i] = 1
    m[:, j, j], m[:, j, k], m[:, k, j], m[:, k, k] = c, -s, s, c
    return m


# --- Loop selection ----------------------------------------------------------

def smooth(x, n):
    """Moving average along axis 0 with edge padding."""
    if n <= 1:
        return x
    pad = np.concatenate([np.repeat(x[:1], n // 2, 0), x, np.repeat(x[-1:], n - n // 2 - 1, 0)])
    kernel = np.ones(n) / n
    return np.apply_along_axis(lambda c: np.convolve(c, kernel, "valid"), 0, pad)


def movement_tempo(P, J, dt):
    """Dominant pulse of the movement (seconds per beat, confidence)."""
    feet = P[:, [J["LeftFoot"], J["RightFoot"]]]
    hips_v = np.diff(P[:, J["Hips"], 1]) / dt
    feet_s = np.linalg.norm(np.diff(feet, axis=0), axis=-1).sum(-1) / dt
    sig = np.abs(hips_v) + 0.5 * feet_s
    sig = sig - smooth(sig, int(2.0 / dt))
    lags = np.arange(int(60 / 150 / dt), int(60 / 70 / dt))
    ac = np.array([np.dot(sig[l:], sig[:-l]) / (len(sig) - l) for l in lags])
    zero = np.dot(sig, sig) / len(sig)
    bpm = 60 / (lags * dt)
    score = ac * np.exp(-0.5 * (np.log2(bpm / 124) / 0.5) ** 2)  # EDM-centred prior
    i = int(np.argmax(score))
    return lags[i] * dt, float(ac[i] / max(zero, 1e-9))


def find_loop(P, J, dt, beat, beats, min_s, max_s, skip_s, H):
    """Best (start, end) frame pair whose poses match, near `beats` * `beat` long.

    Pose match alone favours moments where the dancer holds still, so the cost
    divides by how much she moves during the loop."""
    keys = [J[n] for n in ("LeftHand", "RightHand", "LeftFoot", "RightFoot", "LeftLeg",
                           "RightLeg", "LeftForeArm", "RightForeArm", "Head")]
    rel = P[:, keys] - P[:, J["Hips"]][:, None]
    rel[..., 1] += P[:, J["Hips"], 1][:, None] - P[:, J["Hips"], 1].mean()  # keep bounce
    feat = rel.reshape(len(P), -1)
    vel = np.gradient(feat, axis=0) * 8
    f2 = np.concatenate([feat, vel], 1)
    limbs = [J[n] for n in ("LeftHand", "RightHand", "LeftFoot", "RightFoot", "Head")]
    speed = np.linalg.norm(np.diff(P[:, limbs], axis=0), axis=-1).mean(-1) / dt / H
    cum = np.concatenate([[0.0], np.cumsum(speed)])
    if beat is not None:
        lengths = [int(round(beat * beats / dt))]
        lengths = [l for l in range(int(lengths[0] * 0.97), int(lengths[0] * 1.03) + 1)]
    else:
        lengths = list(range(int(min_s / dt), int(max_s / dt), 2))
    best = (math.inf, 0, 0)
    start0 = int(skip_s / dt)
    for L in lengths:
        if start0 + L >= len(P):
            continue
        a = f2[start0:len(P) - L:3]
        b = f2[start0 + L::3][:len(a)]
        starts = start0 + np.arange(len(a)) * 3
        motion = (cum[np.minimum(starts + L, len(speed))] - cum[starts]) / L
        cost = np.linalg.norm(a - b, axis=1) / (motion + 0.08)
        i = int(np.argmin(cost))
        if cost[i] < best[0]:
            best = (cost[i], start0 + i * 3, start0 + i * 3 + L)
    return best[1], best[2]


# --- Body model --------------------------------------------------------------

# Torso cross-sections from crotch (t=0) to neck base (t=1): half-width, half-depth,
# forward offset, all in units of body height.
# Interpolated with a smooth monotone spline, so the waist-to-hip line curves
# rather than kinking; the widest hip sits low, flowing into the thighs.
TORSO = np.array([
    # t     width  depth  fwd
    [0.00, 0.080, 0.062, -0.006],
    [0.08, 0.094, 0.074, -0.014],   # hips / glutes
    [0.18, 0.092, 0.071, -0.012],
    [0.30, 0.081, 0.062, -0.006],
    [0.44, 0.068, 0.054, 0.000],    # waist
    [0.56, 0.071, 0.057, 0.006],
    [0.66, 0.080, 0.078, 0.030],    # bust
    [0.76, 0.084, 0.064, 0.016],
    [0.88, 0.090, 0.050, 0.000],
    [1.00, 0.068, 0.043, 0.000],
])

# Straighter profile for male figures: no bust, narrower hips, broader chest.
TORSO_MALE = np.array([
    [0.00, 0.086, 0.068, -0.008],
    [0.10, 0.089, 0.071, -0.006],
    [0.22, 0.084, 0.065, -0.002],
    [0.38, 0.077, 0.057, 0.002],
    [0.52, 0.076, 0.054, 0.002],
    [0.66, 0.083, 0.060, 0.005],
    [0.80, 0.089, 0.067, 0.002],
    [0.92, 0.087, 0.052, 0.000],
    [1.00, 0.070, 0.046, 0.000],
])

# Limb segments: (from, to, radius at start, radius at end), radii in body heights.
# Names starting with "_" are derived points built per frame in `body_points`;
# {S} is replaced by Left/Right and {s} by L/R.
LIMB_TEMPLATE = [
    ("{S}UpLeg", "_{s}thigh", 0.042, 0.047),        # flush with the hip line
    ("_{s}thigh", "{S}Leg", 0.047, 0.029),          # tapering to the knee
    ("{S}Leg", "_{s}calf", 0.029, 0.033),
    ("_{s}calf", "{S}Foot", 0.033, 0.014),          # slim ankle
    ("{S}Foot", "_{s}heel", 0.015, 0.013),
    ("_{s}heel", "{S}ToeBase", 0.013, 0.013),
    ("{S}ToeBase", "_{s}toe", 0.012, 0.004),        # pointed toe
    ("Neck", "_{s}sh", 0.030, 0.027),               # collarbone
    ("_{s}sh", "{S}ForeArm", 0.026, 0.018),
    ("{S}ForeArm", "{S}Hand", 0.018, 0.011),        # narrow wrist
    ("{S}Hand", "_{s}palm", 0.012, 0.016),
    ("_{s}palm", "_{s}finger", 0.016, 0.006),       # tapering fingers
    ("{S}Hand", "_{s}thumb", 0.010, 0.005),
]
LIMBS = [(a.format(S=S, s=sd), b.format(S=S, s=sd), ra, rb)
         for S, sd in (("Left", "L"), ("Right", "R")) for a, b, ra, rb in LIMB_TEMPLATE]
LIMBS.append(("Neck", "Head", 0.028, 0.025))


def body_points(q, H):
    """Add the derived points LIMBS refers to (thigh/calf midpoints, heels,
    toe tips, palm and finger tips, narrowed shoulders) to joint dict `q`."""
    for S, s in (("Left", "L"), ("Right", "R")):
        q[f"_{s}thigh"] = q[f"{S}UpLeg"] * 0.6 + q[f"{S}Leg"] * 0.4
        q[f"_{s}calf"] = q[f"{S}Leg"] * 0.7 + q[f"{S}Foot"] * 0.3
        foot = q[f"{S}ToeBase"] - q[f"{S}Foot"]
        foot_dir = foot / max(np.linalg.norm(foot), 1e-9)
        shin = q[f"{S}Foot"] - q[f"{S}Leg"]
        shin_dir = shin / max(np.linalg.norm(shin), 1e-9)
        q[f"_{s}heel"] = q[f"{S}Foot"] - foot_dir * 0.022 * H + shin_dir * 0.018 * H
        q[f"_{s}toe"] = q[f"{S}ToeBase"] + foot_dir * 0.035 * H
        # Mocap skeletons have broad shoulders; pull them in for a slimmer frame.
        q[f"_{s}sh"] = q["Neck"] + (q[f"{S}Arm"] - q["Neck"]) * 0.72
        hand = q[f"{S}FingerBase"] - q[f"{S}Hand"]
        if np.linalg.norm(hand) < 1e-3:
            # Some skeletons put the finger base on the wrist; follow the forearm.
            hand = q[f"{S}HandIndex1"] - q[f"{S}Hand"]
        if np.linalg.norm(hand) < 1e-3:
            hand = q[f"{S}Hand"] - q[f"{S}ForeArm"]
        hand_dir = hand / max(np.linalg.norm(hand), 1e-9)
        q[f"_{s}palm"] = q[f"{S}Hand"] + hand_dir * 0.030 * H
        q[f"_{s}finger"] = q[f"{S}Hand"] + hand_dir * 0.085 * H
        thumb = q[f"{s.upper()}Thumb"] - q[f"{S}Hand"]
        q[f"_{s}thumb"] = q[f"{S}Hand"] + thumb / max(np.linalg.norm(thumb), 1e-9) * 0.045 * H
    return q


def normalize(v):
    return v / np.maximum(np.linalg.norm(v, axis=-1, keepdims=True), 1e-9)


def torso_frames(P, J, H):
    """Per frame: section centres (S,3), right axes, forward axes, up axes, t."""
    crotch = (P[:, J["LeftUpLeg"]] + P[:, J["RightUpLeg"]]) / 2
    spine = [J[n] for n in ("Hips", "LowerBack", "Spine", "Spine1", "Neck")]
    up0 = normalize(P[:, J["Spine1"]] - P[:, J["Hips"]])
    line = np.stack([crotch - up0 * 0.03 * H] + [P[:, j] for j in spine], 1)  # (F, 6, 3)
    seg = np.linalg.norm(np.diff(line, axis=1), axis=-1)
    cum = np.concatenate([np.zeros((len(P), 1)), np.cumsum(seg, 1)], 1)
    ts = np.linspace(0, 1, 44)
    S = len(ts)
    centres = np.zeros((len(P), S, 3))
    for f in range(len(P)):
        u = cum[f] / cum[f, -1]
        for k in range(3):
            centres[f, :, k] = np.interp(ts, u, line[f, :, k])
    ups = normalize(np.gradient(centres, axis=1))
    hip_axis = normalize(P[:, J["RightUpLeg"]] - P[:, J["LeftUpLeg"]])
    sh_axis = normalize(P[:, J["RightArm"]] - P[:, J["LeftArm"]])
    w = np.clip((ts - 0.2) / 0.6, 0, 1)[None, :, None]
    right = hip_axis[:, None] * (1 - w) + sh_axis[:, None] * w
    right = normalize(right - (right * ups).sum(-1, keepdims=True) * ups)
    fwd = np.cross(ups, right)
    return centres, right, fwd, ups, ts


class Hair:
    """Verlet strands hanging from the back of the head."""

    def __init__(self, H, dt, n=9, strands=(-0.034, -0.017, 0.0, 0.017, 0.034)):
        self.H, self.dt, self.n = H, dt, n
        self.offsets = strands
        self.seg = 0.036 * H
        self.g = np.array([0, -9.8 * H / 1.7, 0])
        self.x = None

    def step(self, anchor, right, up, fwd, colliders):
        # Roots sit on an arc over the crown, following the skull's curve.
        roots = []
        for o in self.offsets:
            phi = o / max(self.offsets) * math.radians(60)
            roots.append(anchor + (right * math.sin(phi) + up * (math.cos(phi) - 1)) * 0.05 * self.H
                         - fwd * 0.03 * self.H)
        if self.x is None:
            self.x = np.array([[r - up * self.seg * i for i in range(self.n)] for r in roots])
            self.px = self.x.copy()
        v = (self.x - self.px) * 0.96
        self.px = self.x.copy()
        self.x = self.x + v + self.g * self.dt * self.dt
        for _ in range(6):
            for s, r in enumerate(roots):
                self.x[s, 0] = r
            d = self.x[:, 1:] - self.x[:, :-1]
            L = np.linalg.norm(d, axis=-1, keepdims=True)
            corr = d * (1 - self.seg / np.maximum(L, 1e-9)) * 0.5
            self.x[:, 1:] -= corr
            self.x[:, :-1] += corr
            for c, rad in colliders:
                # The first nodes start inside the head, so only the hanging
                # part collides.
                d = self.x[:, 2:] - c
                dist = np.linalg.norm(d, axis=-1, keepdims=True)
                push = np.maximum(rad - dist, 0)
                self.x[:, 2:] += d / np.maximum(dist, 1e-9) * push
        for s, r in enumerate(roots):
            self.x[s, 0] = r
        return self.x.copy()


# --- Rendering ---------------------------------------------------------------

def convex_hull(pts):
    """Monotone-chain convex hull of 2D points."""
    pts = sorted(map(tuple, pts))

    def half(points):
        h = []
        for p in points:
            while len(h) >= 2 and (h[-1][0] - h[-2][0]) * (p[1] - h[-2][1]) - (h[-1][1] - h[-2][1]) * (p[0] - h[-2][0]) <= 0:
                h.pop()
            h.append(p)
        return h

    lower, upper = half(pts), half(reversed(pts))
    return np.array(lower[:-1] + upper[:-1])


def capsule(draw, a, b, ra, rb):
    d = b - a
    L = np.hypot(*d)
    if L < 1e-6:
        return
    n = np.array([-d[1], d[0]]) / L
    quad = [a + n * ra, b + n * rb, b - n * rb, a - n * ra]
    draw.polygon([tuple(p) for p in quad], fill=255)
    for c, r in ((a, ra), (b, rb)):
        draw.ellipse([c[0] - r, c[1] - r, c[0] + r, c[1] + r], fill=255)


def guitar_items(q, proj, H):
    """An electric guitar silhouetted from the hands: the neck runs along the
    fret-hand axis past a headstock, the body sits behind the strum hand,
    tilted like a real instrument slung low."""
    aL, aR = proj(q["LeftHand"]), proj(q["RightHand"])
    n = aL - aR
    if np.linalg.norm(n) < 1e-6:
        return []
    n = n / np.linalg.norm(n)
    head = aL + n * 0.14 * H
    t = math.radians(-10)
    u = np.array([n[0] * math.cos(t) - n[1] * math.sin(t),
                  n[0] * math.sin(t) + n[1] * math.cos(t)])
    v = np.array([-u[1], u[0]])
    th = np.linspace(0, 2 * np.pi, 30, endpoint=False)
    # Two bouts unioned by the render pass: a wider lower bout behind the
    # strum hand and a narrower upper bout where the neck joins — the
    # classic offset waist of a solid-body guitar.
    lower = aR - n * 0.095 * H + v * 0.012 * H
    upper = aR - n * 0.045 * H + v * 0.004 * H
    body = np.concatenate([
        lower + np.outer(np.cos(th) * 0.105 * H, u) + np.outer(np.sin(th) * 0.088 * H, v),
        upper + np.outer(np.cos(th) * 0.085 * H, u) + np.outer(np.sin(th) * 0.072 * H, v),
    ])
    c = aR - n * 0.05 * H
    return [("poly", convex_hull(body)),
            ("cap", c + n * 0.03 * H, head, 0.014 * H, 0.012 * H),
            ("cap", head, head + n * 0.05 * H, 0.012 * H, 0.024 * H)]


def render(frames2d, size, blur):
    """frames2d: list of shape lists -> smooth-unioned mask image."""
    w, h = size
    img = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(img)
    for kind, *args in frames2d:
        if kind == "poly":
            d.polygon([tuple(p) for p in args[0]], fill=255)
        else:
            capsule(d, *args)
    # Blur + threshold = smooth union: joins between shapes become organic curves.
    img = img.filter(ImageFilter.GaussianBlur(blur))
    mask = np.asarray(img) > 118
    # Fill pinholes where shapes nearly touch, but keep real gaps (a hand on
    # the hip leaves a window that belongs in the silhouette).
    holes, n = ndimage.label(ndimage.binary_fill_holes(mask) & ~mask)
    if n:
        sizes = ndimage.sum_labels(np.ones_like(holes), holes, index=np.arange(1, n + 1))
        small = np.isin(holes, np.nonzero(sizes < (blur * 3) ** 2)[0] + 1)
        mask |= small
    img = Image.fromarray((mask * 255).astype(np.uint8))
    return img.filter(ImageFilter.GaussianBlur(blur * 0.35))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("bvh", type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--view", default="front", help="front | three-quarter | side | <degrees>")
    ap.add_argument("--beats", type=int, default=8, help="beats per loop when a tempo is found")
    ap.add_argument("--skip", type=float, default=1.0, help="seconds to ignore at the start (T-pose)")
    ap.add_argument("--height", type=int, default=512)
    ap.add_argument("--fps", type=float, default=30.0)
    ap.add_argument("--hair", type=float, default=1.0, help="hair length multiplier (0 = none)")
    ap.add_argument("--male", action="store_true", help="straight male torso profile")
    ap.add_argument("--guitar", action="store_true",
                    help="draw an electric guitar from the two hand positions")
    ap.add_argument("--energy", type=float, help="override the measured energy (0 calm .. 1 driving)")
    ap.add_argument("--whole", action="store_true",
                    help="the take already loops (e.g. from choreo.py): use all of it as --beats beats")
    args = ap.parse_args()

    names, P, dt = load_bvh(args.bvh)
    J = {n: i for i, n in enumerate(names)}
    P = P[1:]  # frame 0 is the added T-pose
    H = float(np.median(P[:, J["Head_end"], 1] - np.minimum(P[:, J["LeftToeBase"], 1], P[:, J["RightToeBase"], 1])))

    beats = args.beats
    s = e = 0
    tempo_ok = True
    if args.whole:
        s, e = 0, len(P)
    else:
        beat, conf = movement_tempo(P, J, dt)
        tempo_ok = conf > 0.15
        print(f"movement tempo: {60 / beat:.1f} BPM (confidence {conf:.2f}{'' if tempo_ok else ', ignored'})")
    if not args.whole and tempo_ok:
        # Short takes may not fit the requested phrase; try half of it.
        for beats in (args.beats, args.beats // 2):
            s, e = find_loop(P, J, dt, beat, beats, 0, 0, args.skip, H)
            if e > s:
                break
    if not args.whole and e <= s:
        tempo_ok = False
        s, e = find_loop(P, J, dt, None, 0, 3.5, 8.0, args.skip, H)
        if e <= s:
            s, e = find_loop(P, J, dt, None, 0, 2.0, 8.0, args.skip, H)
        if e <= s:
            raise SystemExit("take is too short to find a loop")
    L = e - s
    if not tempo_ok:
        beats = max(4, int(round(L * dt * 124 / 60 / 4)) * 4)
    print(f"loop: {s * dt:.2f}s - {e * dt:.2f}s ({L * dt:.2f}s, {beats} beats)")

    # Remove floor travel (keep local sway), then blend the loop seam in pose space.
    root = P[:, J["Hips"]].copy()
    drift = smooth(root, int(1.5 / dt))
    drift[:, 1] = 0
    if args.whole:
        drift[:] = 0     # choreographed loops stay in place; keep every hip sway
    P = P - drift[:, None]
    K = min(int(0.4 / dt), s, L // 3)
    Q = P[s:e].copy()
    for i in range(K):
        w = (i + 1) / (K + 1)
        w = w * w * (3 - 2 * w)
        Q[L - K + i] = (1 - w) * P[e - K + i] + w * P[s - K + i]

    centres, right, fwd, ups, ts = torso_frames(Q, J, H)
    torso = TORSO_MALE if args.male else TORSO
    prof = np.stack([PchipInterpolator(torso[:, 0], torso[:, c])(ts) for c in (1, 2, 3)], 1) * H

    # Camera: look at the dancer's average facing, rotated by --view.
    f_avg = fwd[:, 5].mean(0)
    f_avg[1] = 0
    if np.linalg.norm(f_avg) < 0.2:      # she turns right round: face the start instead
        f_avg = fwd[0, 5].copy()
        f_avg[1] = 0
    f_avg = f_avg / np.linalg.norm(f_avg)
    angle = {"front": 0.0, "three-quarter": 35.0, "side": 90.0}.get(args.view)
    angle = math.radians(float(args.view) if angle is None else angle)
    c, sn = math.cos(angle), math.sin(angle)
    cam_f = np.array([f_avg[0] * c + f_avg[2] * sn, 0, -f_avg[0] * sn + f_avg[2] * c])
    world_up = np.array([0.0, 1.0, 0.0])
    cam_r = np.cross(world_up, cam_f)

    def proj(p):
        return np.stack([p @ cam_r, -(p @ world_up)], -1)

    # Hair: simulate two passes over the loop and keep the second so it loops too.
    hair = Hair(H * max(args.hair, 0.01), dt) if args.hair > 0 else None
    hair_frames = []
    head_up = normalize(Q[:, J["Head_end"]] - Q[:, J["Head"]])
    if hair:
        for pass_ in range(2):
            for i in range(L):
                head_c = Q[i, J["Head"]] + head_up[i] * 0.048 * H
                colliders = [(head_c, 0.075 * H), (Q[i, J["Neck"]] - fwd[i, -1] * 0.01 * H, 0.05 * H),
                             (centres[i, 34] - fwd[i, 34] * 0.02 * H, 0.08 * H),
                             (centres[i, 26] - fwd[i, 26] * 0.02 * H, 0.075 * H)]
                x = hair.step(head_c + head_up[i] * 0.04 * H, right[i, -1], head_up[i], fwd[i, -1], colliders)
                if pass_ == 1:
                    hair_frames.append(x)

    # Build 2D shapes per output frame.
    n_out = min(MAX_FRAMES, max(8, int(round(L * dt * args.fps))))
    picks = [int(i * L / n_out) for i in range(n_out)]
    theta = np.linspace(0, 2 * np.pi, 28, endpoint=False)
    shapes = []
    for i in picks:
        items = []
        # Loft the torso: each section is a horizontal ellipse, which a level
        # camera sees edge-on, so fill the hull between neighbouring sections.
        rings = []
        for k in range(len(ts)):
            a, b, o = prof[k]
            pts = (centres[i, k] + np.outer(np.cos(theta) * a, right[i, k])
                   + np.outer(o + np.sin(theta) * b, fwd[i, k]))
            rings.append(proj(pts))
        for k in range(len(rings) - 1):
            items.append(("poly", convex_hull(np.concatenate([rings[k], rings[k + 1]]))))
        q = body_points({n: Q[i, J[n]] for n in names}, H)
        for a_, b_, ra, rb in LIMBS:
            items.append(("cap", proj(q[a_]), proj(q[b_]), ra * H, rb * H))
        if args.guitar:
            items += guitar_items(q, proj, H)
        head_c = Q[i, J["Head"]] + head_up[i] * 0.048 * H
        hp = [head_c + np.cos(t) * 0.050 * H * right[i, -1] + np.sin(t) * 0.066 * H * head_up[i] for t in theta]
        items.append(("poly", proj(np.array(hp))))
        if hair:
            # Hair volume over the crown and back of the head.
            hc = head_c - fwd[i, -1] * 0.012 * H + head_up[i] * 0.006 * H
            hv = [hc + np.cos(t) * 0.058 * H * right[i, -1] + np.sin(t) * 0.07 * H * head_up[i] for t in theta]
            items.append(("poly", proj(np.array(hv))))
        if hair:
            x = hair_frames[i]
            for strand in x:
                for k in range(len(strand) - 1):
                    r0 = 0.022 * H * (1 - 0.6 * k / len(strand))
                    r1 = 0.022 * H * (1 - 0.6 * (k + 1) / len(strand))
                    items.append(("cap", proj(strand[k]), proj(strand[k + 1]), r0, r1))
        shapes.append(items)

    # Frame everything with one fixed camera box over the loop.
    allpts = []
    for items in shapes:
        for it in items:
            allpts.append(it[1] if it[0] == "poly" else np.array([it[1], it[2]]))
    allpts = np.concatenate(allpts)
    pad = 0.06 * H
    x0, y0 = allpts.min(0) - pad
    x1, y1 = allpts.max(0) + pad
    ss = 2
    out_h = args.height
    scale = out_h * ss / (y1 - y0)
    out_w = max(8, int(round((x1 - x0) / (y1 - y0) * out_h / 4)) * 4)
    size = (out_w * ss, out_h * ss)

    def to_px(p):
        return (p - np.array([x0, y0])) * scale

    out = ROOT / "dancers" / args.name
    if out.exists():
        shutil.rmtree(out)
    (out / "frames").mkdir(parents=True)
    for fi, items in enumerate(shapes):
        px = []
        for it in items:
            if it[0] == "poly":
                px.append(("poly", to_px(it[1])))
            else:
                px.append(("cap", to_px(it[1]), to_px(it[2]), it[3] * scale, it[4] * scale))
        img = render(px, size, blur=float(0.008 * H * scale))
        img.resize((out_w, out_h), Image.LANCZOS).save(out / "frames" / f"{fi:04d}.png")
    # Energy 0..1 from how fast the limbs move (for picking clips to suit the track).
    limbs = [J[n] for n in ("LeftHand", "RightHand", "LeftFoot", "RightFoot", "Head", "Hips")]
    speed = np.linalg.norm(np.diff(Q[:, limbs], axis=0), axis=-1).mean() / dt / H
    energy = float(np.clip((speed - 0.1) / 1.0, 0, 1)) if args.energy is None else args.energy
    print(f"energy: {energy:.2f} (mean limb speed {speed:.2f} body-heights/s)")
    meta = {"name": args.name, "energy": round(energy, 2), "fps": round(n_out / (L * dt), 3), "frames": n_out, "beats": beats,
            "width": out_w, "height": out_h,
            "source": f"{args.bvh.name} (CMU Graphics Lab Motion Capture Database, mocap.cs.cmu.edu)"}
    (out / "clip.json").write_text(json.dumps(meta, indent=2))
    print(f"wrote {n_out} frames ({out_w}x{out_h}) to {out}")


if __name__ == "__main__":
    main()
