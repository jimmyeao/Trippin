"""Procedural shadow-dancer choreography, written out as BVH on the CMU skeleton.

    py tools/choreo.py frame --out tools/.mocap_cache/choreo_frame.bvh

Shadow dancing (go-go / club silhouettes behind a screen) is continuous hip
sway, weight on one hip, body rolls and arms making clear shapes, with slow
turns into profile. CMU's mocap has little of that, so this builds it:

- continuous layers: hip sway every beat with the pelvis dropping onto the
  weighted hip, a body roll rippling up the spine, head tilt and knee groove;
- arm shapes keyframed every two beats and eased between (see ARM_POSES);
- body turns and back arches per keyframe;
- legs solved with two-bone IK so the feet stay planted while the hips move.

Every layer is periodic over the routine, so the BVH loops seamlessly. Render
it with `mocap_dancer.py --whole`.
"""
import argparse
import math
from pathlib import Path

import numpy as np
from scipy.spatial.transform import Rotation

import mocap_dancer as md

TOOLS = Path(__file__).resolve().parent
SKELETON = TOOLS / ".mocap_cache" / "05_02.bvh"   # any CMU take: we borrow its hierarchy
FPS = 60
REF_BPM = 124.0

# Arm shapes for the LEFT arm (the right is mirrored). Angles in degrees:
# elev: 0 = out to the side at shoulder height, +90 = straight up, -80 = down by the hip
# fwd:  swing toward the front (+) or back (-)
# bend: elbow bend, curling the forearm up/in toward the head when the arm is raised
# bend_fwd: elbow bend toward the front of the body
ARM_POSES = {
    "down":   dict(elev=-72, fwd=8,   bend=10,  bend_fwd=10),
    "hip":    dict(elev=-35, fwd=0,   bend=-110, bend_fwd=0),    # hand on hip, elbow out
    "side":   dict(elev=5,   fwd=15,  bend=15,  bend_fwd=10),
    "diag":   dict(elev=45,  fwd=15,  bend=10,  bend_fwd=5),
    "up":     dict(elev=100, fwd=8,   bend=25,  bend_fwd=0),
    "crown":  dict(elev=78,  fwd=10,  bend=55,  bend_fwd=0),     # diamond: elbows out, hands meet overhead
    "over":   dict(elev=110, fwd=12,  bend=40,  bend_fwd=10),    # arm arcs over the head
    "hair":   dict(elev=120, fwd=-5,  bend=110, bend_fwd=0),     # hand behind the head
    "front":  dict(elev=10,  fwd=70,  bend=35,  bend_fwd=20),
    "chest":  dict(elev=-15, fwd=70,  bend=20,  bend_fwd=95),    # hand across the chest
}

# Routines: one keyframe every two beats (the list is cyclic).
# (left arm, right arm, turn degrees, back arch degrees, weighted hip: +1 left / -1 right)
ROUTINES = {
    "frame": [("crown", "crown", 0, 4, 1), ("hair", "hip", 15, 6, -1),
              ("crown", "crown", 0, 8, 1), ("hip", "hair", -15, 6, -1)],
    "snake": [("side", "side", 0, 2, 1), ("diag", "down", 10, 4, -1),
              ("side", "side", 0, 2, 1), ("down", "diag", -10, 4, -1)],
    "profile": [("crown", "crown", 55, 10, 1), ("up", "hip", 70, 14, 1),
                ("crown", "crown", 55, 10, -1), ("hair", "hip", 40, 8, -1)],
    "hips": [("hip", "hip", 0, 3, 1), ("hip", "up", 20, 6, -1),
             ("hip", "hip", 0, 3, 1), ("up", "hip", -20, 6, -1)],
    "diva": [("hair", "chest", 25, 8, 1), ("hair", "hip", 35, 10, -1),
             ("crown", "crown", 10, 6, 1), ("hair", "front", 25, 8, -1)],
    "spin": [("crown", "crown", 0, 6, 1), ("up", "up", 90, 8, -1),
             ("crown", "crown", 180, 6, 1), ("up", "up", 270, 8, -1)],
    "wave": [("crown", "crown", 0, 4, 1), ("diag", "side", 0, 6, -1),
             ("crown", "crown", 0, 4, 1), ("side", "diag", 0, 6, -1)],
    "tease": [("hair", "hip", 70, 14, 1), ("hair", "hip", 80, 18, 1),
              ("up", "hip", 70, 14, -1), ("crown", "crown", 60, 10, -1)],
}


# Beats per keyframe for the arms, turns and arches (the dance itself), and
# how much continuous hip sway the routine has (0 = only the weight shifts ..
# 1 = grooving throughout). The hips run slower than the arms: weight shifts
# take WEIGHT_SLOWDOWN times as long, and the figure-8 sway takes 4 beats.
KEY_BEATS = {"tease": 2, "profile": 2, "diva": 2, "frame": 2, "spin": 2,
             "snake": 2, "wave": 2, "hips": 2}
WEIGHT_SLOWDOWN = 2
GROOVE = {"tease": 0.15, "profile": 0.2, "diva": 0.3, "frame": 0.3, "spin": 0.2,
          "snake": 0.35, "wave": 0.35, "hips": 0.6}

# Overlapping action: each part of the body plays the choreography this many
# beats behind the hips, so movement ripples up the body and out along the arms.
LAG = {"hips": 0.0, "spine": 0.15, "neck": 0.25, "head": 0.35,
       "upper_arm": 0.25, "forearm": 0.45, "hand": 0.6}


def routine_beats(routine):
    # The slower weight track sets the loop length (the arm sequence plays twice).
    return len(ROUTINES[routine]) * KEY_BEATS.get(routine, 2) * WEIGHT_SLOWDOWN


def catmull(p0, p1, p2, p3, t):
    """Catmull-Rom spline between p1 and p2: smooth velocity through every key."""
    t2, t3 = t * t, t * t * t
    return 0.5 * (2 * p1 + (p2 - p0) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 + (3 * p1 - p0 - 3 * p2 + p3) * t3)


class Track:
    """Cyclic spline through one value per keyframe, sampled at any beat."""

    def __init__(self, values, key_beats, wrap_add=0.0):
        self.v = [float(x) for x in values]
        self.key_beats = key_beats
        self.wrap_add = wrap_add   # added per full cycle (for turns that spin round)

    def at(self, beat):
        n = len(self.v)
        pos = beat / self.key_beats
        i = math.floor(pos)
        u = pos - i
        # Linger slightly near each pose, but never stop: speed is 0.85..1.15.
        u = u - 0.15 / TWO_PI * math.sin(TWO_PI * u)

        def val(k):
            cycles, j = divmod(k, n)
            return self.v[j] + cycles * self.wrap_add

        return catmull(val(i - 1), val(i), val(i + 1), val(i + 2), u)


TWO_PI = 2 * math.pi


def unwrap(angles):
    """Turn keys unwrapped to be continuous; returns (values, amount per cycle)."""
    out = [angles[0]]
    for a in angles[1:] + angles[:1]:
        d = (a - out[-1] + 180) % 360 - 180
        out.append(out[-1] + d)
    return out[:-1], out[-1] - out[0]


def lerp(a, b, w):
    return a + (b - a) * w


def rx(d):
    return Rotation.from_euler("x", d, degrees=True).as_matrix()


def ry(d):
    return Rotation.from_euler("y", d, degrees=True).as_matrix()


def rz(d):
    return Rotation.from_euler("z", d, degrees=True).as_matrix()


class Skeleton:
    def __init__(self, path):
        tokens = Path(path).read_text().split()
        self.joints = md.parse_hierarchy(tokens)
        self.names = [j.name for j in self.joints]
        self.index = {n: i for i, n in enumerate(self.names)}
        text = Path(path).read_text()
        self.header = text[:text.index("MOTION")]
        self.offset = {j.name: j.offset for j in self.joints}

    def fk(self, root_pos, local):
        """World rotations and positions from local rotations (dict name -> 3x3)."""
        wr, wp = {}, {}
        for j in self.joints:
            r = local.get(j.name, np.eye(3))
            if j.parent < 0:
                wr[j.name], wp[j.name] = r, root_pos
            else:
                pn = self.names[j.parent]
                wp[j.name] = wp[pn] + wr[pn] @ j.offset
                wr[j.name] = wr[pn] @ r
        return wr, wp


def basis(hinge, direction):
    y = direction / np.linalg.norm(direction)
    x = hinge - y * np.dot(hinge, y)
    x /= np.linalg.norm(x)
    return np.stack([x, y, np.cross(x, y)], 1)


def solve_leg(sk, local, root_pos, side, foot_target, facing, toe_point):
    """Two-bone IK: set UpLeg/Leg/Foot local rotations so the foot reaches its target."""
    S = "Left" if side > 0 else "Right"
    hip_name = "LHipJoint" if side > 0 else "RHipJoint"
    wr, wp = sk.fk(root_pos, local)
    H = wp[f"{S}UpLeg"]
    l1 = np.linalg.norm(sk.offset[f"{S}Leg"])
    l2 = np.linalg.norm(sk.offset[f"{S}Foot"])
    to_f = foot_target - H
    d = min(np.linalg.norm(to_f), (l1 + l2) * 0.999)
    dirv = to_f / np.linalg.norm(to_f)
    a = (l1 * l1 - l2 * l2 + d * d) / (2 * d)
    h = math.sqrt(max(l1 * l1 - a * a, 0.0))
    # Knees bend forward and a little outward.
    pole = facing @ np.array([0.08 * side, 0.0, 1.0])
    perp = pole - dirv * np.dot(pole, dirv)
    perp /= np.linalg.norm(perp)
    K = H + dirv * a + perp * h
    F = H + dirv * d
    v1, v2 = K - H, F - K
    hinge = np.cross(v1, v2)
    if np.linalg.norm(hinge) < 1e-6:
        hinge = facing @ np.array([1.0, 0.0, 0.0])
    rest_thigh = sk.offset[f"{S}Leg"]
    rest_shin = sk.offset[f"{S}Foot"]
    x_axis = np.array([1.0, 0.0, 0.0])
    w_thigh = basis(hinge, v1) @ basis(x_axis, rest_thigh).T
    w_shin = basis(hinge, v2) @ basis(x_axis, rest_shin).T
    parent = wr[hip_name]
    local[f"{S}UpLeg"] = parent.T @ w_thigh
    local[f"{S}Leg"] = w_thigh.T @ w_shin
    local[f"{S}Foot"] = w_shin.T @ (facing @ rx(-35 * toe_point))


def arm_rotations(pose, side, wave):
    """Upper arm and forearm local rotations for one arm (side +1 left, -1 right)."""
    elev = pose["elev"] + wave
    upper = ry(-pose["fwd"] * side) @ rz(elev * side)
    fore = rz(pose["bend"] * side) @ ry(-pose["bend_fwd"] * side)
    return upper, fore


def pose_at(sk, routine, beat, beats, rest_hips):
    keys = ROUTINES[routine]
    kb = KEY_BEATS.get(routine, 2)
    turns, spin = unwrap([k[2] for k in keys])
    turn_t = Track(turns, kb, spin)
    arch_t = Track([k[3] for k in keys], kb)
    weight_t = Track([k[4] for k in keys], kb * WEIGHT_SLOWDOWN)
    arm_t = {
        (side, param): Track([ARM_POSES[k[idx]][param] for k in keys], kb)
        for side, idx in ((1, 0), (-1, 1)) for param in ("elev", "fwd", "bend", "bend_fwd")
    }
    groove = GROOVE.get(routine, 0.3)

    def part(name):
        return beat - LAG[name]

    # Hips lead: the weight shifts plus a soft figure-8 (side to side over two
    # beats, forward and back twice as fast), scaled by the routine's groove.
    b = part("hips")
    weight = weight_t.at(b)
    turn = turn_t.at(b)
    fig_x = math.sin(math.pi * b / 2) * groove
    fig_z = math.sin(math.pi * b) * groove * 0.35
    facing = ry(turn)
    hip_shift = 1.0 * weight + 1.1 * fig_x
    tilt = 8 * weight + 8 * fig_x
    # Knees soften as the weight passes through the middle.
    dip = 0.35 * (1.0 - min(abs(weight), 1.0)) + 0.1 * groove * (0.5 + 0.5 * math.cos(math.pi * b))
    local = {}
    local["Hips"] = facing @ rz(tilt) @ ry(6 * fig_z)
    root = rest_hips + facing @ np.array([hip_shift, -0.45 - dip, 0.4 * fig_z])

    # Spine follows the hips, counter-tilting into an S-curve, with a slow roll.
    bs = part("spine")
    w_s = weight_t.at(bs)
    sway_s = math.sin(math.pi * bs / 2) * groove
    arch = arch_t.at(bs)
    roll = TWO_PI * bs / 8
    for i, name in enumerate(("LowerBack", "Spine", "Spine1")):
        wave = 3 * math.sin(roll - 0.8 * (i + 1))
        local[name] = rz(-(8 * w_s + 8 * sway_s) * 0.6) @ rx(wave + arch / 3)
    bn, bh = part("neck"), part("head")
    local["Neck"] = rz(-3 * weight_t.at(bn) - 4 * math.sin(math.pi * bn / 2) * groove) @ rx(-arch_t.at(bn) * 0.3)
    local["Head"] = rz(-4 * weight_t.at(bh) - 5 * math.sin(math.pi * bh / 2) * groove)         @ rx(3 * math.sin(TWO_PI * bh / 8 - 3.0))

    # Arms: the upper arm leads, the forearm and hand trail behind it, which
    # turns every shape change into a flowing, snake-like gesture.
    for side, S in ((1, "Left"), (-1, "Right")):
        bu, bf, bw = part("upper_arm"), part("forearm"), part("hand")
        upper_pose = {k: arm_t[(side, k)].at(bu) for k in ("elev", "fwd")}
        fore_pose = {k: arm_t[(side, k)].at(bf) for k in ("bend", "bend_fwd")}
        pose = {**upper_pose, **fore_pose}
        undulate = 4 * math.sin(TWO_PI * bu / 8 + (0 if side > 0 else math.pi))
        upper, fore = arm_rotations(pose, side, undulate)
        local[f"{S}Shoulder"] = rz(max(pose["elev"], 0) * 0.12 * side)
        local[f"{S}Arm"] = upper
        local[f"{S}ForeArm"] = fore
        local[f"{S}Hand"] = rz(-10 * side * math.sin(TWO_PI * bw / 8 + side))

    # Feet planted under the hips, the free leg's foot drawn in with toe pointed.
    ground = rest_hips[1] + sk.offset["LHipJoint"][1] + sk.offset["LeftUpLeg"][1]         + sk.offset["LeftLeg"][1] + sk.offset["LeftFoot"][1]
    for side in (1, -1):
        # The unweighted side relaxes. A smooth ramp (not a clip at zero) so the
        # free foot eases into and out of its step instead of kicking off.
        ramp = float(np.clip((-weight * side + 0.3) / 1.3, 0.0, 1.0))
        amount = ramp * ramp * (3 - 2 * ramp)
        x = side * (2.0 - 1.1 * amount)
        z = 0.9 * amount
        target = rest_hips + facing @ np.array([x, 0.0, z])
        target[1] = ground + 0.9 * amount   # heel lifts on the free foot
        solve_leg(sk, local, root, side, target, facing, amount)
    return root, local


def to_channels(sk, root, local):
    vals = []
    for j in sk.joints:
        if not j.channels:
            continue
        r = local.get(j.name, np.eye(3))
        z, y, x = Rotation.from_matrix(r).as_euler("ZYX", degrees=True)
        rot = {"Zrotation": z, "Yrotation": y, "Xrotation": x}
        for ch in j.channels:
            vals.append(root["XYZ".index(ch[0])] if ch.endswith("position") else rot[ch])
    return vals


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("routine", choices=sorted(ROUTINES))
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--beats", type=int, help="loop length (default: the routine's own length)")
    ap.add_argument("--pose", help="static test pose LEFT,RIGHT (routine name is then ignored)")
    args = ap.parse_args()
    if args.pose:
        l, r = args.pose.split(",")
        ROUTINES[args.routine] = [(l, r, 20, 6, 1)] * 2
    args.beats = args.beats or routine_beats(args.routine)

    sk = Skeleton(SKELETON)
    rest_hips = np.array([0.0, 17.0, 0.0])
    n = int(round(args.beats * 60 / REF_BPM * FPS))
    rows = [to_channels(sk, rest_hips, {})]   # frame 0: rest pose (the loader skips it)
    for f in range(n):
        beat = f / n * args.beats
        root, local = pose_at(sk, args.routine, beat, args.beats, rest_hips)
        rows.append(to_channels(sk, root, local))
    body = "\n".join(" ".join(f"{v:.4f}" for v in r) for r in rows)
    args.out.write_text(f"{sk.header}MOTION\nFrames: {len(rows)}\nFrame Time: {1 / FPS:.6f}\n{body}\n")
    print(f"wrote {n} frames ({args.beats} beats at {REF_BPM:.0f} BPM) to {args.out}")


if __name__ == "__main__":
    main()
