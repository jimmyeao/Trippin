"""The giant android for unity_colossus, from one of Alice's crowd people.

Alice's /agent/crowd makes textured people (Hunyuan3D); one with a strong
pose becomes the colossus. Unity re-skins it as ceramic and chrome armour
with glowing seams, poses it with a skeleton, and gives it a sculpted face
(a separate head mesh, AndroidHead.shader). This prepares the body:

- keeps the largest connected piece (drops stray bits like a ground plate);
- stands it on y=0 at unit height, centred (Hunyuan faces +z);
- Taubin-smooths it hard, so cloth folds melt into sleek android forms;
- finds the head (walking up the centre line) and cuts it away: Unity puts
  the sculpted head on the neck;
- estimates a skeleton from slices of the mesh (tuned for a standing,
  arms-raised pose like member_05): pelvis, waist, neck, shoulders,
  elbows, fists, hips, knees, ankles;
- weights each vertex to its two nearest bones, restricted by region (arm
  bones only reach the arms, leg bones only the legs);
- writes Resources/Android/android.bytes ("TCRS": the crowd mesh format plus
  four bone indices and weights per vertex) and android_rig.json (joints
  and the head ellipsoid).

    python tools/android_mesh.py crowd.zip member_05 [--smooth 80]
"""

import argparse
import io
import json
import struct
import sys
import zipfile
from pathlib import Path

import numpy as np
import trimesh

OUT = Path(__file__).resolve().parent.parent / "unity/TrippinStage/Assets/Trippin/Resources/Android"

# Bone order shared with ColossusShow.cs.
BONES = ["pelvis", "chest", "neck", "upper_l", "fore_l", "upper_r", "fore_r",
         "thigh_l", "shin_l", "thigh_r", "shin_r"]


def load(src, name):
    p = Path(src)
    if p.is_dir():
        data = (p / f"{name}.glb").read_bytes()
    else:
        with zipfile.ZipFile(p) as z:
            data = z.read(next(n for n in z.namelist() if n.endswith(f"{name}.glb")))
    return trimesh.load(io.BytesIO(data), file_type="glb", force="mesh")


def find_head(v):
    # Walk up the centre line from the shoulders until the head ends: the
    # raised fists reach the centre band higher up, so a plain max() lands
    # on them.
    axis = np.abs(v[:, 0]) < 0.035
    top = 0.75
    while (axis & (v[:, 1] > top) & (v[:, 1] < top + 0.012)).any():
        top += 0.004
    near_top = axis & (v[:, 1] > top - 0.02) & (v[:, 1] < top + 0.012)
    c = np.array([0.0, top - 0.068, v[near_top, 2].mean() - 0.01])
    r = np.array([0.056, 0.072, 0.064])
    return c, r


def slice_centroid(v, mask, y0, y1):
    m = mask & (v[:, 1] >= y0) & (v[:, 1] < y1)
    return v[m].mean(0) if m.any() else None


def skeleton(v, head_c):
    """Joints from slices. Sides are by sign of x (trimesh space)."""
    j = {}
    neck_y = head_c[1] - 0.075
    j["pelvis"] = np.array([0.0, 0.47, 0.0])
    j["waist"] = np.array([0.0, 0.57, 0.0])
    j["neck"] = np.array([0.0, neck_y, head_c[2] - 0.01])
    j["head"] = head_c.copy()
    for side, sg in (("r", 1.0), ("l", -1.0)):
        arm = (sg * v[:, 0] > 0.085) & (v[:, 1] > 0.6)
        # Path of the arm: centroids of horizontal slices from shoulder up.
        ys = np.arange(0.66, 1.0, 0.015)
        path = [(y, slice_centroid(v, arm, y, y + 0.015)) for y in ys]
        path = [(y, c) for y, c in path if c is not None]
        sh = path[2][1].copy()
        sh[0] = sg * max(abs(sh[0]), 0.095)
        # Elbow: the slice reaching furthest out.
        el = max(path, key=lambda yc: abs(yc[1][0]))[1].copy()
        fist = path[-1][1].copy()
        fist[1] = v[arm, 1].max()
        j["shoulder_" + side] = sh
        j["elbow_" + side] = el
        j["fist_" + side] = fist
        leg = (sg * v[:, 0] > 0.0) & (v[:, 1] < 0.5)
        hip = slice_centroid(v, leg, 0.44, 0.5)
        hip[0] = sg * 0.05
        # Knee: the leg's narrowest-in point between 0.18 and 0.32.
        kys = np.arange(0.18, 0.32, 0.02)
        knees = [(y, slice_centroid(v, leg, y, y + 0.02)) for y in kys]
        knee = min((kc for kc in knees if kc[1] is not None), key=lambda kc: abs(kc[1][0]))[1]
        ankle = slice_centroid(v, leg, 0.04, 0.08)
        j["hip_" + side] = hip
        j["knee_" + side] = knee
        j["ankle_" + side] = ankle
    return j


def seg_dist(p, a, b):
    ab = b - a
    t = np.clip(((p - a) @ ab) / max(ab @ ab, 1e-9), 0, 1)
    return np.linalg.norm(p - (a + t[:, None] * ab), axis=1)


def weights(v, j):
    """Two nearest bones per vertex, region-restricted, 1/d^4 blended."""
    floor = lambda p: np.array([p[0], 0.0, p[2]])
    segs = {
        "pelvis": (np.array([0.0, 0.40, 0.0]), j["waist"]),
        "chest": (j["waist"], j["neck"]),
        "neck": (j["neck"], j["head"]),
        "upper_l": (j["shoulder_l"], j["elbow_l"]),
        "fore_l": (j["elbow_l"], j["fist_l"]),
        "upper_r": (j["shoulder_r"], j["elbow_r"]),
        "fore_r": (j["elbow_r"], j["fist_r"]),
        "thigh_l": (j["hip_l"], j["knee_l"]),
        "shin_l": (j["knee_l"], floor(j["ankle_l"])),
        "thigh_r": (j["hip_r"], j["knee_r"]),
        "shin_r": (j["knee_r"], floor(j["ankle_r"])),
    }
    d = np.stack([seg_dist(v, *segs[b]) for b in BONES], axis=1)
    x, y = v[:, 0], v[:, 1]
    inf = 1e9
    # Regions: arms only where the arms are, legs only below the hips.
    # Above the head (it's cut away) everything is arm, however near the
    # centre the fists come; elsewhere arms start outside the torso.
    head_top = j["head"][1] + 0.07
    arm_zone = (np.abs(x) > 0.07) | (y > head_top)
    for b, i in ((b, i) for i, b in enumerate(BONES)):
        if b.endswith("_l") and b[:4] in ("uppe", "fore"):
            d[~arm_zone | (x > 0) | (y < 0.6), i] = inf
        if b.endswith("_r") and b[:4] in ("uppe", "fore"):
            d[~arm_zone | (x < 0) | (y < 0.6), i] = inf
        if b.startswith(("thigh", "shin")):
            side = -1 if b.endswith("_l") else 1
            d[(y > 0.52) | (side * x < -0.01), i] = inf
        if b == "neck":
            d[(y < j["neck"][1] - 0.04) | (y > head_top) | (np.abs(x) > 0.07), i] = inf
    order = np.argsort(d, axis=1)[:, :2]
    d2 = np.take_along_axis(d, order, axis=1)
    w = 1.0 / (d2 ** 4 + 1e-10)
    w[d2 >= inf] = 0
    w = w / np.maximum(w.sum(1, keepdims=True), 1e-12)
    return order, w


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("source", help="crowd zip from Alice, or an unpacked dir")
    ap.add_argument("member", help="e.g. member_05")
    ap.add_argument("--smooth", type=int, default=80, help="Taubin iterations")
    a = ap.parse_args()
    m = load(a.source, a.member)
    # Textured GLBs split vertices along UV seams: weld by position first,
    # or the body falls apart into hundreds of pieces.
    m = trimesh.Trimesh(m.vertices, m.faces, process=True)
    m.merge_vertices(merge_tex=True, merge_norm=True)
    parts = m.split(only_watertight=False)
    m = max(parts, key=lambda p: len(p.faces))
    # Hunyuan sometimes stands people on a thin plate: trim the bottom slice.
    lo, hi = m.bounds
    keep = m.triangles_center[:, 1] > lo[1] + 0.012 * (hi[1] - lo[1])
    m.update_faces(keep)
    m.remove_unreferenced_vertices()
    lo, hi = m.bounds
    m.apply_translation([-(lo[0] + hi[0]) / 2, -lo[1], -(lo[2] + hi[2]) / 2])
    m.apply_scale(1.0 / (hi[1] - lo[1]))
    trimesh.smoothing.filter_taubin(m, lamb=0.5, nu=0.53, iterations=a.smooth)
    head_c, head_r = find_head(m.vertices)
    # Cut the head away above the neck; the sculpted head replaces it.
    rel = (m.vertices - head_c) / (head_r * 1.15)
    in_head = (np.linalg.norm(rel, axis=1) < 1.0) & (m.vertices[:, 1] > head_c[1] - 0.06)
    m.update_faces(~in_head[m.faces].all(axis=1))
    m.remove_unreferenced_vertices()
    j = skeleton(m.vertices, head_c)
    bones, w = weights(m.vertices, j)
    if len(m.vertices) > 65535:
        print("too many vertices for u16 indices")
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    v = m.vertices.astype(np.float32)
    n = m.vertex_normals.astype(np.float32)
    bi = np.zeros((len(v), 4), np.uint8)
    bw = np.zeros((len(v), 4), np.uint8)
    bi[:, :2] = bones
    bw[:, :2] = np.round(w * 255).astype(np.uint8)
    rec = np.zeros(len(v), dtype=[("p", "<f4", 3), ("n", "<f4", 3), ("i", "u1", 4), ("w", "u1", 4)])
    rec["p"], rec["n"], rec["i"], rec["w"] = v, n, bi, bw
    idx = m.faces.astype("<u2").ravel()
    with open(OUT / "android.bytes", "wb") as fh:
        fh.write(b"TCRS" + struct.pack("<II", len(v), len(idx)))
        fh.write(rec.tobytes())
        fh.write(idx.tobytes())
    rig = {"bones": BONES, "joints": {k: np.round(p, 5).tolist() for k, p in j.items()},
           "head_radii": head_r.tolist()}
    (OUT / "android_rig.json").write_text(json.dumps(rig, indent=1))
    print(f"{a.member}: kept {len(m.faces)} tris, {len(v)} verts")
    for k in ("shoulder_l", "elbow_l", "fist_l", "hip_l", "knee_l", "ankle_l", "neck", "head"):
        print(f"  {k:10s} {np.round(j[k], 3).tolist()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
