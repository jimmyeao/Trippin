// unity_lattice: a 5 x 5 x 5 lattice of glowing nodes joined by thin lasers,
// rippling as if a wave were passing through it.
//  - Shape: nodes are displaced by travelling waves whose depth follows the
//    eased bass, and pushed outward by shell: the spectrum from the core
//    (lows) to the corners (highs), so the lattice breathes as the mix does.
//  - Motion: the waves travel on the smooth energy clock; the lattice turns
//    slowly and swings direction with the phrase.
//  - Luminance: nodes and edges glow with the level of their shell.
//  - Drops: a build compresses it dim; the drop bursts it open.
using UnityEngine;

namespace TrippinStage
{
    public sealed class LatticeShow : KitShow
    {
        const int N = 5, Nodes = N * N * N;
        const float Pitch = 3.8f;
        BeamPool _beams;
        GlowPool _glows;
        readonly Vector3[] _base = new Vector3[Nodes], _pos = new Vector3[Nodes];
        readonly float[] _shell = new float[Nodes];

        static int Idx(int x, int y, int z) => (x * N + y) * N + z;

        protected override void Build()
        {
            Env();
            _beams = new BeamPool(transform, beamMat, 3 * (N - 1) * N * N, "lattice", false, 0.04f);
            _glows = new GlowPool(transform, glowMat, Nodes, "node", false);
            for (int x = 0; x < N; x++)
                for (int y = 0; y < N; y++)
                    for (int z = 0; z < N; z++)
                    {
                        var p = new Vector3(x - 2f, y - 2f, z - 2f) * Pitch;
                        _base[Idx(x, y, z)] = p;
                        _shell[Idx(x, y, z)] = Mathf.Clamp01(p.magnitude / (2f * Pitch * 1.732f));
                    }
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain();
            float amp = (0.5f + 1.1f * rx.bassFast) * (1f - 0.4f * rx.tension) * (1f + 0.9f * rx.impact);
            float spin = rx.clk * 0.02f + 0.6f * Mathf.Sin(rx.phrase);
            Quaternion q = Quaternion.Euler(18f, spin * Mathf.Rad2Deg, 0f);
            var centre = new Vector3(0f, 11f, 0f);
            for (int i = 0; i < Nodes; i++)
            {
                Vector3 b = _base[i];
                float lvl = rx.Spec(_shell[i]);
                Vector3 off = new Vector3(
                    Mathf.Sin(b.y * 0.7f + rx.clk * 0.25f),
                    Mathf.Sin(b.z * 0.7f + rx.clk * 0.21f + 1.3f),
                    Mathf.Sin(b.x * 0.7f + rx.clk * 0.17f + 2.1f)) * amp;
                Vector3 radial = b.normalized * (1.2f * lvl + 0.6f * rx.kick);
                _pos[i] = centre + q * (b + off + radial);
            }
            int n = 0;
            for (int x = 0; x < N; x++)
                for (int y = 0; y < N; y++)
                    for (int z = 0; z < N; z++)
                    {
                        int a = Idx(x, y, z);
                        float la = rx.Spec(_shell[a]);
                        if (x + 1 < N) n = Edge(n, a, Idx(x + 1, y, z), gain, la);
                        if (y + 1 < N) n = Edge(n, a, Idx(x, y + 1, z), gain, la);
                        if (z + 1 < N) n = Edge(n, a, Idx(x, y, z + 1), gain, la);
                    }
            for (int i = 0; i < Nodes; i++)
            {
                float lvl = rx.Spec(_shell[i]);
                _glows.Set(i, _pos[i], 0.9f + 1.4f * lvl + 0.5f * rx.kick, 0.1f + _shell[i] * 0.6f, gain * (0.45f + 0.9f * lvl), cam.transform.rotation);
            }
            rig.Orbit(cam, rx, 36f, 12f, 11f, dt);
        }

        int Edge(int n, int a, int b, float gain, float la)
        {
            float lb = rx.Spec(_shell[b]);
            _beams.Set(n, _pos[a], _pos[b], Kit.Hue(0.15f + 0.5f * _shell[a]), gain * (0.3f + 0.8f * 0.5f * (la + lb)));
            return n + 1;
        }
    }
}
