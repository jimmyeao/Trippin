"""Procedural guitarist loop, written out as BVH on the CMU skeleton.

    py tools/guitar_choreo.py --out tools/.mocap_cache/guitar.bvh
    py tools/mocap_dancer.py tools/.mocap_cache/guitar.bvh --name band_guitar \
        --whole --beats 8 --hair 0 --male --guitar

A *regular* rhythm guitarist: feet planted, a gentle figure-8 groove, the
head nodding once per beat, the left hand shifting chords every two beats,
and the right forearm strumming eighth-notes. No headbanging — the nod is
a small decaying pulse, not a whip. Render it with mocap_dancer.py's
--guitar flag, which draws the instrument from the two hand positions.

Every layer is periodic over the routine, so the BVH loops seamlessly.
"""
import argparse
import math
from pathlib import Path

import numpy as np

import mocap_dancer as md
from choreo import (FPS, SKELETON, Skeleton, Track, arm_rotations, rx, ry, rz,
                    solve_leg, to_channels)

TWO_PI = 2 * math.pi
BEATS = 8

# Fret-hand positions, one per two beats (cyclic): the hand slides along the
# neck for chord changes. Angles follow choreo.ARM_POSES (elev/fwd/bend/bend_fwd).
FRET_KEYS = [
    dict(elev=16, fwd=36, bend=-64, bend_fwd=14),   # open position
    dict(elev=21, fwd=40, bend=-58, bend_fwd=18),   # up the neck
    dict(elev=13, fwd=33, bend=-70, bend_fwd=11),   # back down
    dict(elev=19, fwd=42, bend=-56, bend_fwd=20),   # stretch
]


def guitar_pose(sk, beat, rest_hips):
    """(root_pos, local rotations) for a right-handed rhythm guitarist.

    The player faces 28 degrees to camera-left; the left arm carries the neck
    up-left, the right hand works over the body near the bridge."""
    facing = ry(28)

    # Body groove: a slow figure-8, plus a knee bounce on every beat.
    fig_x = 0.55 * math.sin(math.pi * beat / 4)      # side-to-side over 8 beats
    fig_z = 0.18 * math.sin(math.pi * beat / 2)
    tilt = 6.0 * fig_x
    dip = 0.12 + 0.20 * (0.5 + 0.5 * math.cos(TWO_PI * beat))
    local = {}
    local["Hips"] = facing @ rz(tilt) @ ry(5 * fig_z)
    root = rest_hips + facing @ np.array([fig_x, -0.42 - dip, fig_z])

    # Spine leans a touch toward the instrument, counter-swaying the hips.
    for i, name in enumerate(("LowerBack", "Spine", "Spine1")):
        k = (i + 1) / 3
        local[name] = rz(-4.5 * fig_x * k) @ rx(7 + 2 * math.sin(math.pi * beat / 4 - k))

    # Head: a small nod that lands ON each beat and decays within it — a bob,
    # not a headbang — plus a slight lean toward the guitar.
    frac = beat % 1.0
    nod = 6.5 * math.exp(-4.5 * frac) + 2.5 * math.exp(-4.5 * ((frac + 0.5) % 1.0))
    local["Neck"] = rz(-2.5 * fig_x) @ rx(nod * 0.45)
    local["Head"] = rz(5 - 2 * fig_x) @ rx(nod + 1.5 * math.sin(math.pi * beat / 4 - 1.0))

    # Left arm: fretting. Chord-shift keys are tracked over 2-beat intervals.
    lp = {k: Track([p[k] for p in FRET_KEYS], 2).at(beat - 0.2)
          for k in ("elev", "fwd", "bend", "bend_fwd")}
    upper, fore = arm_rotations(lp, 1, 0)
    local["LeftShoulder"] = rz(2)
    local["LeftArm"] = upper
    local["LeftForeArm"] = fore
    # The fretting wrist stays bent, fingers wrapped over the neck.
    local["LeftHand"] = rz(18) @ ry(-30)

    # Right arm: strumming eighth-notes. The elbow swings the forearm in an
    # arc that lifts the hand past the guitar body's silhouette edge — a
    # stylised exaggeration so the strum reads in a flat silhouette — while
    # the whole arm dips a touch with each downstroke. The strum swell
    # follows the 8-beat phrase so accenting breathes with the music.
    phrase = 0.75 + 0.25 * math.cos(TWO_PI * beat / BEATS)
    strum = 16 * math.sin(TWO_PI * beat * 2) * phrase
    dip = 4 * max(0.0, math.sin(TWO_PI * beat * 2)) * phrase
    rp = dict(elev=-30 - dip, fwd=54, bend=-46 + strum, bend_fwd=38 + strum * 0.5)
    upper, fore = arm_rotations(rp, -1, 0)
    local["RightShoulder"] = rz(-2)
    local["RightArm"] = upper
    local["RightForeArm"] = fore
    local["RightHand"] = rz(-strum * 0.9) @ ry(strum * 0.3)

    # Feet planted hip-width; knees soak up the bounce. The lead foot's heel
    # taps lightly on off-beats — a small lift, not a step.
    ground = rest_hips[1] + sk.offset["LHipJoint"][1] + sk.offset["LeftUpLeg"][1] \
        + sk.offset["LeftLeg"][1] + sk.offset["LeftFoot"][1]
    weight = 0.4 * fig_x
    for side in (1, -1):
        ramp = float(np.clip((-weight * side + 0.15) / 1.3, 0.0, 1.0))
        amount = ramp * ramp * (3 - 2 * ramp)
        tap = 0.35 * max(0.0, math.sin(TWO_PI * beat)) if side > 0 else 0.0
        x = side * (2.3 - 0.4 * amount)
        z = 0.5 * amount
        target = rest_hips + facing @ np.array([x, 0.0, z])
        target[1] = ground + 0.5 * amount + tap
        solve_leg(sk, local, root, side, target, facing, amount)
    return root, local


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--beats", type=int, default=BEATS)
    args = ap.parse_args()

    sk = Skeleton(SKELETON)
    rest_hips = np.array([0.0, 17.0, 0.0])
    n = int(round(args.beats * 60 / 124.0 * FPS))
    rows = [to_channels(sk, rest_hips, {})]   # frame 0: rest pose (loader skips it)
    for f in range(n):
        beat = f / n * args.beats
        root, local = guitar_pose(sk, beat, rest_hips)
        rows.append(to_channels(sk, root, local))
    body = "\n".join(" ".join(f"{v:.4f}" for v in r) for r in rows)
    args.out.write_text(f"{sk.header}MOTION\nFrames: {len(rows)}\nFrame Time: {1 / FPS:.6f}\n{body}\n")
    print(f"wrote {n} frames ({args.beats} beats at 124 BPM) to {args.out}")


if __name__ == "__main__":
    main()
