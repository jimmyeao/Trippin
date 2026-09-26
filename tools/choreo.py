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


def smooth(x):
    return x * x * (3 - 2 * x)


def keyframe_blend(keys, beat, beats):
    """(prev key, next key, eased weight) for a cyclic list, one key per 2 beats."""
    pos = (beat % beats) / 2.0
    i = int(math.floor(pos)) % len(keys)
    return keys[i], keys[(i + 1) % len(keys)], smooth(pos - math.floor(pos))


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


def solve_leg(sk, local, root_pos, side, foot_target, facing, point_toe):
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
    local[f"{S}Foot"] = w_shin.T @ (facing @ rx(-35 if point_toe else 0))


def arm_rotations(pose, side, wave):
    """Upper arm and forearm local rotations for one arm (side +1 left, -1 right)."""
    elev = pose["elev"] + wave
    upper = ry(-pose["fwd"] * side) @ rz(elev * side)
    fore = rz(pose["bend"] * side) @ ry(-pose["bend_fwd"] * side)
    return upper, fore


def pose_at(sk, routine, beat, beats, rest_hips):
    keys = ROUTINES[routine]
    k0, k1, w = keyframe_blend(keys, beat, beats)
    turn = lerp(k0[2], k1[2] if abs(k1[2] - k0[2]) <= 180 else k1[2] - 360, w)
    arch = lerp(k0[3], k1[3], w)
    weight = lerp(k0[4], k1[4], w)                 # +1 on the left hip, -1 on the right

    ph = math.pi * beat
    sway = math.sin(ph)                             # hips cross side to side every beat
    groove = 0.5 + 0.5 * math.cos(2 * ph)           # dip on each beat
    roll = 2 * math.pi * beat / 4                   # body roll, one per bar

    local = {}
    facing = ry(turn)
    # Pelvis: sway plus the weighted hip pushed out and dropped on the other side.
    hip_shift = 1.7 * sway + 0.9 * weight
    tilt = 13 * sway + 7 * weight
    local["Hips"] = facing @ rz(tilt) @ rx(-4 * math.sin(roll))
    root = rest_hips + facing @ np.array([hip_shift, -0.45 - 0.3 * groove, 0.0])

    # Spine: counter-tilt the chest, ripple a body roll upward, arch the back.
    for i, name in enumerate(("LowerBack", "Spine", "Spine1")):
        wave = 5 * math.sin(roll - 0.8 * (i + 1))
        # The ribcage swings back over the feet: an S-curve through the body.
        local[name] = rz(-tilt * 0.6) @ rx(wave + arch / 3)
    local["Neck"] = rz(-4 * sway) @ rx(-arch * 0.3)
    local["Head"] = rz(-6 * sway) @ rx(4 * math.sin(roll - 3.0))

    # Arms: eased between keyframe shapes, with a slow snake undulation.
    for side, S, idx in ((1, "Left", 0), (-1, "Right", 1)):
        a = ARM_POSES[k0[idx]]
        b = ARM_POSES[k1[idx]]
        pose = {k: lerp(a[k], b[k], w) for k in a}
        wave = 8 * math.sin(roll + (0 if side > 0 else math.pi))
        upper, fore = arm_rotations(pose, side, wave)
        local[f"{S}Shoulder"] = rz(max(pose["elev"], 0) * 0.12 * side)
        local[f"{S}Arm"] = upper
        local[f"{S}ForeArm"] = fore
        local[f"{S}Hand"] = rz(-12 * side * math.sin(roll + side))

    # Feet planted under the hips, the free leg's foot drawn in with toe pointed.
    ground = rest_hips[1] + sk.offset["LHipJoint"][1] + sk.offset["LeftUpLeg"][1] \
        + sk.offset["LeftLeg"][1] + sk.offset["LeftFoot"][1]
    for side in (1, -1):
        free = (weight * side) < 0          # the leg on the unweighted side relaxes
        amount = min(abs(weight), 1.0) if free else 0.0
        x = side * (2.0 - 1.1 * amount)
        z = 0.9 * amount
        target = rest_hips + facing @ np.array([x, 0.0, z])
        target[1] = ground + 0.9 * amount   # heel lifts on the free foot
        solve_leg(sk, local, root, side, target, facing, amount > 0.5)
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
    ap.add_argument("--beats", type=int, default=8)
    ap.add_argument("--pose", help="static test pose LEFT,RIGHT (routine name is then ignored)")
    args = ap.parse_args()
    if args.pose:
        l, r = args.pose.split(",")
        ROUTINES[args.routine] = [(l, r, 20, 6, 1)] * 2

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
