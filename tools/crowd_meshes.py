"""Crowd members for the Unity stage, from Alice's /agent/crowd batches.

Alice (the owner's generation server) turns photos into textured GLB people
with Hunyuan3D (see its external agent API doc). This prepares them for
instancing hundreds of times at 60 fps on an M2:

- drops broken reconstructions (people come out ~2 units tall and narrow;
  a flat card means the white backdrop got meshed, a squat blob means a
  failed pose);
- bakes the texture into vertex colours (no texture or UVs needed in Unity);
- decimates to ~2500 triangles (meshes arrive at 28-40k);
- stands each one on y=0 at unit height, centred (Hunyuan faces +z, the stage);
- writes Assets/Trippin/Resources/Crowd/<name>.bytes, which StageDirector
  loads at runtime.

Format (little-endian): b"TCRW", u32 vertex count, u32 index count,
then per vertex float3 position, float3 normal, u8x4 RGBA; then u16 indices.

    pip install trimesh fast-simplification scipy networkx pillow
    python tools/crowd_meshes.py crowd.zip [more.zip|dir ...] [--faces 2500]
"""

import argparse
import io
import struct
import sys
import zipfile
from pathlib import Path

import numpy as np
import trimesh
import fast_simplification
from scipy.spatial import cKDTree

OUT = Path(__file__).resolve().parent.parent / "unity/TrippinStage/Assets/Trippin/Resources/Crowd"


def members(src):
    """Yield (name, GLB bytes) from a crowd zip or an unpacked directory."""
    p = Path(src)
    if p.is_dir():
        for f in sorted(p.glob("*.glb")):
            if not f.stem.endswith("_shape"):
                yield f.stem, f.read_bytes()
    else:
        with zipfile.ZipFile(p) as z:
            for n in sorted(z.namelist()):
                if n.endswith(".glb") and not n.endswith("_shape.glb"):
                    yield Path(n).stem, z.read(n)


def prepare(glb, faces):
    m = trimesh.load(io.BytesIO(glb), file_type="glb", force="mesh")
    e = m.extents
    # Height is the longest axis (y) for a standing person: reject the rest.
    if e[1] < 0.9 * e.max() or not (1.8 < e[1] / e[0] < 5.0) or e[2] < 0.12 * e[1]:
        return None, f"rejected (extents {np.round(e, 2).tolist()})"
    col = np.asarray(m.visual.to_color().vertex_colors, dtype=np.uint8)

    v, f = fast_simplification.simplify(
        m.vertices.astype(np.float32), m.faces.astype(np.int32),
        target_reduction=max(0.0, 1.0 - faces / len(m.faces)))
    # Colour each kept vertex from the nearest original one.
    _, nearest = cKDTree(m.vertices).query(v)
    out = trimesh.Trimesh(v, f, vertex_colors=col[nearest], process=True)

    # Feet on y=0, unit height, centred on the body's footprint.
    lo, hi = out.bounds
    out.apply_translation([-(lo[0] + hi[0]) / 2, -lo[1], -(lo[2] + hi[2]) / 2])
    out.apply_scale(1.0 / (hi[1] - lo[1]))

    # Hunyuan3D's people already face +z (the stage). A nose/toe-lean
    # heuristic got half of a batch backwards, so trust the generator.
    return out, f"{len(m.faces)} -> {len(out.faces)} tris"


def write(mesh, path):
    v = mesh.vertices.astype(np.float32)
    n = mesh.vertex_normals.astype(np.float32)
    c = np.asarray(mesh.visual.vertex_colors, dtype=np.uint8)[:, :4]
    idx = mesh.faces.astype(np.uint16).ravel()
    rec = np.zeros(len(v), dtype=[("p", "<f4", 3), ("n", "<f4", 3), ("c", "u1", 4)])
    rec["p"], rec["n"], rec["c"] = v, n, c
    with open(path, "wb") as fh:
        fh.write(b"TCRW" + struct.pack("<II", len(v), len(idx)))
        fh.write(rec.tobytes())
        fh.write(idx.astype("<u2").tobytes())


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("sources", nargs="+", help="crowd zips from Alice, or unpacked dirs")
    ap.add_argument("--faces", type=int, default=2500)
    ap.add_argument("--prefix", default="", help="name prefix, so batches don't collide")
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    kept = 0
    for src in a.sources:
        tag = a.prefix or Path(src).stem
        for name, glb in members(src):
            mesh, msg = prepare(glb, a.faces)
            print(f"{tag}/{name}: {msg}")
            if mesh is not None:
                if len(mesh.vertices) > 65535:
                    print("  too many vertices for u16 indices, skipped")
                    continue
                write(mesh, OUT / f"{tag}_{name}.bytes")
                kept += 1
    print(f"{kept} crowd members in {OUT}")
    return 0 if kept else 1


if __name__ == "__main__":
    sys.exit(main())
