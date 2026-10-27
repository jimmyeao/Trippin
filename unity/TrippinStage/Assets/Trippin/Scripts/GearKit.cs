// Mesh builders and a draw helper for the DJ gear shows (unity_decks): rounded
// boxes (bevelled chassis, buttons, fader caps), lathed parts (platters,
// records, knobs, feet, spindles) and swept tubes (the tonearm). Built once in
// code at real proportions (1 unit = 10 cm), drawn every frame with
// Graphics.RenderMesh so moving parts are just matrices.
using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    public static class GearMesh
    {
        /// A box `size` with edges rounded by `r` (smooth normals), centred on the
        /// origin. uv is planar over the top (x, z), 0..1 across the box.
        public static Mesh RoundedBox(Vector3 size, float r, int seg = 4, string name = "rbox")
        {
            var h = size * 0.5f;
            r = Mathf.Min(r, Mathf.Min(h.x, Mathf.Min(h.y, h.z)) * 0.99f);
            var verts = new List<Vector3>(); var norms = new List<Vector3>(); var uvs = new List<Vector2>(); var tris = new List<int>();
            float[] Axis(float half)
            {
                var a = new List<float>();
                for (int i = 0; i <= seg; i++) a.Add(-half + r * (1f - Mathf.Cos(i / (float)seg * Mathf.PI * 0.5f)));
                for (int i = seg; i >= 0; i--) a.Add(half - r * (1f - Mathf.Cos(i / (float)seg * Mathf.PI * 0.5f)));
                return a.ToArray();
            }
            var ax = Axis(h.x); var ay = Axis(h.y); var az = Axis(h.z);
            var inner = h - Vector3.one * r;
            void Face(Vector3 n, Vector3 u, Vector3 v, float[] au, float[] av)
            {
                int b = verts.Count;
                for (int j = 0; j < av.Length; j++)
                    for (int i = 0; i < au.Length; i++)
                    {
                        var p = n * Vector3.Dot(h, Abs(n)) + u * au[i] + v * av[j];
                        var c = new Vector3(Mathf.Clamp(p.x, -inner.x, inner.x), Mathf.Clamp(p.y, -inner.y, inner.y), Mathf.Clamp(p.z, -inner.z, inner.z));
                        var d = p - c;
                        var nn = d.sqrMagnitude > 1e-12f ? d.normalized : n;
                        var q = c + nn * r;
                        verts.Add(q); norms.Add(nn);
                        uvs.Add(new Vector2(q.x / size.x + 0.5f, q.z / size.z + 0.5f));
                    }
                int w = au.Length;
                for (int j = 0; j < av.Length - 1; j++)
                    for (int i = 0; i < w - 1; i++)
                        Quad(verts, tris, b + j * w + i, b + j * w + i + 1, b + (j + 1) * w + i + 1, b + (j + 1) * w + i, n);
            }
            Face(Vector3.up, Vector3.right, Vector3.forward, ax, az);
            Face(Vector3.down, Vector3.right, Vector3.forward, ax, az);
            Face(Vector3.right, Vector3.forward, Vector3.up, az, ay);
            Face(Vector3.left, Vector3.forward, Vector3.up, az, ay);
            Face(Vector3.forward, Vector3.right, Vector3.up, ax, ay);
            Face(Vector3.back, Vector3.right, Vector3.up, ax, ay);
            return Make(name, verts, norms, uvs, tris);
        }

        static Vector3 Abs(Vector3 v) => new Vector3(Mathf.Abs(v.x), Mathf.Abs(v.y), Mathf.Abs(v.z));

        /// Adds a quad, wound so its face normal agrees with `nref`.
        static void Quad(List<Vector3> v, List<int> t, int a, int b, int c, int d, Vector3 nref)
        {
            var fn = Vector3.Cross(v[c] - v[a], v[d] - v[b]);
            if (fn.sqrMagnitude < 1e-16f) fn = Vector3.Cross(v[b] - v[a], v[c] - v[a]);
            if (Vector3.Dot(fn, nref) >= 0f) { t.Add(a); t.Add(b); t.Add(c); t.Add(a); t.Add(c); t.Add(d); }
            else { t.Add(a); t.Add(c); t.Add(b); t.Add(a); t.Add(d); t.Add(c); }
        }

        static Mesh Make(string name, List<Vector3> v, List<Vector3> n, List<Vector2> uv, List<int> t)
        {
            var m = new Mesh { name = name };
            if (v.Count > 65000) m.indexFormat = UnityEngine.Rendering.IndexFormat.UInt32;
            m.SetVertices(v); m.SetNormals(n); m.SetUVs(0, uv); m.SetTriangles(t, 0);
            m.RecalculateBounds();
            return m;
        }

        /// Revolves a profile of (radius, y) points round the y axis. Repeat a
        /// point to make a crisp edge there. uv: planar over x/z scaled by
        /// `uvR` (0..1 across 2*uvR), or with `band` u = angle, v = (y - y0)/(y1 - y0).
        public static Mesh Lathe(Vector2[] prof, int segs = 64, string name = "lathe", float uvR = 0f, bool band = false)
        {
            if (uvR <= 0f) foreach (var p in prof) uvR = Mathf.Max(uvR, p.x);
            float y0 = float.MaxValue, y1 = float.MinValue;
            foreach (var p in prof) { y0 = Mathf.Min(y0, p.y); y1 = Mathf.Max(y1, p.y); }
            var verts = new List<Vector3>(); var norms = new List<Vector3>(); var uvs = new List<Vector2>(); var tris = new List<int>();
            int n = prof.Length;
            var n2 = new Vector2[n];
            for (int i = 0; i < n; i++)
            {
                Vector2 t;
                bool dupNext = i < n - 1 && (prof[i + 1] - prof[i]).sqrMagnitude < 1e-12f;
                bool dupPrev = i > 0 && (prof[i] - prof[i - 1]).sqrMagnitude < 1e-12f;
                if (dupNext) t = prof[i] - prof[Mathf.Max(i - 1, 0)];
                else if (dupPrev) t = prof[Mathf.Min(i + 1, n - 1)] - prof[i];
                else t = prof[Mathf.Min(i + 1, n - 1)] - prof[Mathf.Max(i - 1, 0)];
                t.Normalize();
                n2[i] = new Vector2(-t.y, t.x);
            }
            for (int i = 0; i < n; i++)
                for (int j = 0; j <= segs; j++)
                {
                    float a = j / (float)segs * Mathf.PI * 2f;
                    float c = Mathf.Cos(a), s = Mathf.Sin(a);
                    var p = new Vector3(prof[i].x * c, prof[i].y, prof[i].x * s);
                    verts.Add(p);
                    norms.Add(new Vector3(n2[i].x * c, n2[i].y, n2[i].x * s).normalized);
                    uvs.Add(band ? new Vector2(j / (float)segs, (prof[i].y - y0) / Mathf.Max(y1 - y0, 1e-6f))
                                 : new Vector2(p.x / (2f * uvR) + 0.5f, p.z / (2f * uvR) + 0.5f));
                }
            int w = segs + 1;
            for (int i = 0; i < n - 1; i++)
            {
                if ((prof[i + 1] - prof[i]).sqrMagnitude < 1e-12f) continue;
                for (int j = 0; j < segs; j++)
                {
                    int a = i * w + j, b = i * w + j + 1, c = (i + 1) * w + j + 1, d = (i + 1) * w + j;
                    var nref = norms[a] + norms[b] + norms[c] + norms[d];
                    if (nref.sqrMagnitude < 1e-8f) nref = (verts[a] + verts[c]) * 0.5f;
                    Quad(verts, tris, a, b, c, d, nref);
                }
            }
            return Make(name, verts, norms, uvs, tris);
        }

        /// A tube swept along `pts` with radius `rad` per point, capped.
        public static Mesh Tube(Vector3[] pts, float[] rad, int sides = 14, string name = "tube")
        {
            var verts = new List<Vector3>(); var norms = new List<Vector3>(); var uvs = new List<Vector2>(); var tris = new List<int>();
            int n = pts.Length;
            var t0 = (pts[1] - pts[0]).normalized;
            var nrm = Vector3.Cross(t0, Mathf.Abs(t0.y) < 0.9f ? Vector3.up : Vector3.right).normalized;
            for (int i = 0; i < n; i++)
            {
                var t = (pts[Mathf.Min(i + 1, n - 1)] - pts[Mathf.Max(i - 1, 0)]).normalized;
                nrm = (nrm - t * Vector3.Dot(nrm, t)).normalized;       // parallel transport
                var bin = Vector3.Cross(t, nrm);
                for (int j = 0; j <= sides; j++)
                {
                    float a = j / (float)sides * Mathf.PI * 2f;
                    var d = nrm * Mathf.Cos(a) + bin * Mathf.Sin(a);
                    verts.Add(pts[i] + d * rad[i]); norms.Add(d); uvs.Add(new Vector2(j / (float)sides, i / (n - 1f)));
                }
            }
            int w = sides + 1;
            for (int i = 0; i < n - 1; i++)
                for (int j = 0; j < sides; j++)
                {
                    int a = i * w + j, b = i * w + j + 1, c = (i + 1) * w + j + 1, d = (i + 1) * w + j;
                    Quad(verts, tris, a, b, c, d, norms[a] + norms[c]);
                }
            // End caps.
            for (int e = 0; e < 2; e++)
            {
                int i = e == 0 ? 0 : n - 1;
                var t = e == 0 ? (pts[0] - pts[1]).normalized : (pts[n - 1] - pts[n - 2]).normalized;
                int c0 = verts.Count;
                verts.Add(pts[i]); norms.Add(t); uvs.Add(Vector2.zero);
                for (int j = 0; j <= sides; j++) { verts.Add(verts[i * w + j]); norms.Add(t); uvs.Add(Vector2.zero); }
                for (int j = 0; j < sides; j++)
                {
                    var fn = Vector3.Cross(verts[c0 + 1 + j] - verts[c0], verts[c0 + 2 + j] - verts[c0]);
                    if (Vector3.Dot(fn, t) >= 0f) { tris.Add(c0); tris.Add(c0 + 1 + j); tris.Add(c0 + 2 + j); }
                    else { tris.Add(c0); tris.Add(c0 + 2 + j); tris.Add(c0 + 1 + j); }
                }
            }
            return Make(name, verts, norms, uvs, tris);
        }

        /// A flat quad on the xz plane facing up, `w` x `d`, uv 0..1 (v along +z).
        public static Mesh Quad(float w, float d, string name = "quad")
        {
            var m = new Mesh { name = name };
            m.vertices = new[] { new Vector3(-w / 2, 0, -d / 2), new Vector3(w / 2, 0, -d / 2), new Vector3(-w / 2, 0, d / 2), new Vector3(w / 2, 0, d / 2) };
            m.normals = new[] { Vector3.up, Vector3.up, Vector3.up, Vector3.up };
            m.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            m.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            m.RecalculateBounds();
            return m;
        }

        /// A knob cap: a slightly domed top, a knurled skirt flaring at the base.
        public static Mesh Knob(float r, float h, string name = "knob")
        {
            return Lathe(new[]
            {
                new Vector2(0f, h), new Vector2(r * 0.55f, h - r * 0.04f), new Vector2(r * 0.82f, h - r * 0.08f),
                new Vector2(r * 0.88f, h - r * 0.14f), new Vector2(r * 0.88f, h - r * 0.14f),
                new Vector2(r * 0.9f, h * 0.35f), new Vector2(r * 0.9f, h * 0.35f),
                new Vector2(r, h * 0.3f), new Vector2(r, 0f), new Vector2(r, 0f), new Vector2(0f, 0f),
            }, 40, name);
        }
    }

    /// One draw: a mesh, its material, its own property values.
    public sealed class GearDraw
    {
        // A fresh block per draw each frame (a pool), so no draw sees another's values.
        static readonly List<MaterialPropertyBlock> _pool = new List<MaterialPropertyBlock>();
        static int _used;
        public static void BeginFrame() { _used = 0; }
        public static MaterialPropertyBlock Props
        {
            get
            {
                if (_used == _pool.Count) _pool.Add(new MaterialPropertyBlock());
                var b = _pool[_used++];
                b.Clear();
                return b;
            }
        }

        public static void Draw(Mesh mesh, Material mat, Matrix4x4 m, MaterialPropertyBlock props = null)
        {
            var rp = new RenderParams(mat) { matProps = props, shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
            Graphics.RenderMesh(rp, mesh, 0, m);
        }
    }
}
