// unity_polyhedra: three nested wireframe solids (an icosahedron, an
// octahedron and a cube) tumbling inside each other.
//  - Shape: every vertex is pushed out along its own direction by the eased
//    level of a spectrum band, so the solids' corners literally rise with the
//    music; the whole stack also swells with the bass.
//  - Motion: each solid turns about its own axis on the smooth energy clock,
//    swung back and forth by phrase-length sines.
//  - Luminance: an edge glows with the energy of the two corners it joins.
//  - Drops: a build draws the stack tight and dim; the drop blows it open.
using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    public sealed class PolyhedraShow : KitShow
    {
        const int Solids = 3;
        readonly Vector3[][] _v = new Vector3[Solids][];
        readonly int[][] _e = new int[Solids][];   // pairs of vertex indices
        BeamPool _beams;
        int _count;
        static readonly float[] Size = { 8f, 5.2f, 3.2f };
        static readonly Vector3[] Axis = { new Vector3(0.3f, 1f, 0.2f), new Vector3(1f, 0.4f, 0f), new Vector3(0f, 0.6f, 1f) };

        static int[] EdgesOf(Vector3[] v)
        {
            float min = float.MaxValue;
            for (int i = 0; i < v.Length; i++)
                for (int j = i + 1; j < v.Length; j++) min = Mathf.Min(min, (v[i] - v[j]).magnitude);
            var list = new List<int>();
            for (int i = 0; i < v.Length; i++)
                for (int j = i + 1; j < v.Length; j++)
                    if ((v[i] - v[j]).magnitude < min * 1.01f) { list.Add(i); list.Add(j); }
            return list.ToArray();
        }

        protected override void Build()
        {
            Env();
            float t = (1f + Mathf.Sqrt(5f)) / 2f;
            var ico = new List<Vector3>();
            for (int a = -1; a <= 1; a += 2)
                for (int b = -1; b <= 1; b += 2)
                {
                    ico.Add(new Vector3(0, a, b * t)); ico.Add(new Vector3(a, b * t, 0)); ico.Add(new Vector3(b * t, 0, a));
                }
            var oct = new List<Vector3> { Vector3.right, -Vector3.right, Vector3.up, -Vector3.up, Vector3.forward, -Vector3.forward };
            var cube = new List<Vector3>();
            for (int x = -1; x <= 1; x += 2)
                for (int y = -1; y <= 1; y += 2)
                    for (int z = -1; z <= 1; z += 2) cube.Add(new Vector3(x, y, z));
            var sets = new[] { ico, oct, cube };
            for (int k = 0; k < Solids; k++)
            {
                _v[k] = new Vector3[sets[k].Count];
                for (int i = 0; i < _v[k].Length; i++) _v[k][i] = sets[k][i].normalized;
                _e[k] = EdgesOf(_v[k]);
                _count += _e[k].Length / 2;
            }
            _beams = new BeamPool(transform, beamMat, _count, "polyhedra", true, 0.05f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            float tight = (1f - 0.45f * rx.tension) * (1f + 0.6f * rx.impact);
            var centre = new Vector3(0f, 10f, 0f);
            int n = 0;
            for (int k = 0; k < Solids; k++)
            {
                float ang = (rx.clk * 0.05f + 0.9f * Mathf.Sin(rx.phrase + k * 1.7f)) * (1f + k * 0.4f) * (k % 2 == 0 ? 1f : -1f);
                Quaternion q = Quaternion.AngleAxis(ang * Mathf.Rad2Deg, Axis[k].normalized);
                float size = Size[k] * (1f + 0.2f * rx.bassFast + 0.08f * rx.kick) * tight;
                var verts = _v[k];
                int[] e = _e[k];
                for (int i = 0; i < e.Length; i += 2)
                {
                    int a = e[i], b = e[i + 1];
                    float la = rx.Spec(((a * 7 + k * 3) % 32) / 31f), lb = rx.Spec(((b * 7 + k * 3) % 32) / 31f);
                    Vector3 pa = q * (verts[a] * (size * (1f + 0.45f * la)));
                    Vector3 pb = q * (verts[b] * (size * (1f + 0.45f * lb)));
                    _beams.Set(n++, centre + pa, centre + pb, Kit.Hue(0.1f + 0.3f * k + 0.2f * (la + lb)), gain * (0.45f + 1.0f * 0.5f * (la + lb)));
                }
            }
            rig.Orbit(cam, rx, 30f, 10f, 10f, dt);
        }
    }
}
