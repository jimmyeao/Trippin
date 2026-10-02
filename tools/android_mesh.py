"""The giant android for unity_colossus, from one of Alice's crowd people.

Alice's /agent/crowd makes textured people (Hunyuan3D); one with a strong
pose becomes the colossus. Unity re-skins it as chrome and ceramic with
glowing panel seams; this prepares the shape:

- keeps the largest connected piece (drops stray bits like a ground plate);
- stands it on y=0 at unit height, centred (Hunyuan faces +z);
- Taubin-smooths it hard, so cloth folds melt into sleek android forms;
- turns the head into a smooth helmet (an ellipsoid blended in), which also
  hides Hunyuan's distorted faces;
- writes Assets/Trippin/Resources/Android/android.bytes in the crowd mesh
  format (see crowd_meshes.py), which ColossusShow loads.

    python tools/android_mesh.py crowd.zip member_05 [--smooth 25]
"""

import argparse
import io
import sys
import zipfile
from pathlib import Path

import numpy as np
import trimesh

from crowd_meshes import write

OUT = Path(__file__).resolve().parent.parent / "unity/TrippinStage/Assets/Trippin/Resources/Android"


def load(src, name):
    p = Path(src)
    if p.is_dir():
        data = (p / f"{name}.glb").read_bytes()
    else:
        with zipfile.ZipFile(p) as z:
            data = z.read(next(n for n in z.namelist() if n.endswith(f"{name}.glb")))
    return trimesh.load(io.BytesIO(data), file_type="glb", force="mesh")


def helmet(m):
    v = m.vertices
    # Walk up the centre line from the shoulders until the head ends: the
    # raised fists reach the centre band higher up, so a plain max() lands
    # on them.
    axis = np.abs(v[:, 0]) < 0.035
    top = 0.75
    while ((axis & (v[:, 1] > top) & (v[:, 1] < top + 0.012)).any()):
        top += 0.004
    near_top = axis & (v[:, 1] > top - 0.02) & (v[:, 1] < top + 0.012)
    c = np.array([0.0, top - 0.068, v[near_top, 2].mean() - 0.01])
    r = np.array([0.056, 0.072, 0.064])
    d = v - c
    dist = np.linalg.norm(d, axis=1)
    target = c + d / np.maximum(np.linalg.norm(d / r, axis=1), 1e-6)[:, None]
    w = 1.0 - np.clip((dist - 0.075) / 0.025, 0, 1)
    w *= np.clip((v[:, 1] - (c[1] - 0.075)) / 0.03, 0, 1)
    w = w * w * (3 - 2 * w)
    m.vertices = v + (target - v) * w[:, None]
    return c


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
    c = helmet(m)
    trimesh.smoothing.filter_taubin(m, lamb=0.5, nu=0.53, iterations=4)
    m.visual = trimesh.visual.ColorVisuals(m, vertex_colors=np.full((len(m.vertices), 4), 255, np.uint8))
    if len(m.vertices) > 65535:
        print("too many vertices for u16 indices")
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    write(m, OUT / "android.bytes")
    print(f"{a.member}: {len(parts)} pieces -> kept {len(m.faces)} tris, {len(m.vertices)} verts; helmet at {np.round(c, 3).tolist()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
