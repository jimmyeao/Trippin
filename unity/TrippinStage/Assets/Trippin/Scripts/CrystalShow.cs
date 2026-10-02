// Screen-content show: a slow flight through a spiralling tunnel of faceted
// chrome crystals with a light at the far end.
//  - kicks punch the crystals through an attack/decay envelope, delayed
//    by distance so each kick is a wave running away down the tunnel;
//  - travel speed integrates the smoothed energy (surges with the track,
//    slows in breakdowns); the radius breathes with the mids;
//  - spin speed follows the smoothed highs; the spiral swings direction
//    on phrase-length sines;
//  - a drop blooms the tunnel radius outward, easing back over ~4 bars.

using UnityEngine;

namespace TrippinStage
{
    public class CrystalShow : MonoBehaviour
    {
        public Camera cam;
        public Material chromeMat, sunMat;

        const int Count = 600;
        const float Length = 240f;

        Mesh _mesh;
        Matrix4x4[] _m = new Matrix4x4[Count];
        float[] _rad, _sz, _spin, _h;
        RenderParams _rp;
        Material _sun;
        float _bass, _bloom, _calmLong;
        float _kick, _mid, _high, _energy, _travel, _spinPhase;
        readonly float[] _kickHist = new float[96];
        int _kh;
        bool _drumsWas = true;

        void Awake()
        {
            _mesh = Bipyramid(6, 0.5f, 1.6f);
            var rnd = new System.Random(3);
            _rad = new float[Count]; _sz = new float[Count]; _spin = new float[Count]; _h = new float[Count];
            for (int i = 0; i < Count; i++)
            {
                _h[i] = (float)rnd.NextDouble();
                _rad[i] = 7f + (float)rnd.NextDouble() * 9f;
                _sz[i] = 0.6f + (float)rnd.NextDouble() * 1.2f;
                _spin[i] = ((float)rnd.NextDouble() - 0.5f) * 2f;
            }
            var inst = new Material(chromeMat) { enableInstancing = true };
            _rp = new RenderParams(inst) { shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off,
                                           worldBounds = new Bounds(Vector3.zero, Vector3.one * 1000f) };
            // The light at the end of the tunnel.
            _sun = new Material(sunMat);
            var g = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(g.GetComponent<Collider>());
            g.name = "tunnel light";
            g.transform.SetParent(transform, false);
            g.transform.localPosition = new Vector3(0, 0, 230);
            g.transform.localScale = Vector3.one * 120f;
            g.GetComponent<Renderer>().sharedMaterial = _sun;
        }

        // A faceted crystal: n-sided bipyramid, flat-shaded (split vertices).
        static Mesh Bipyramid(int n, float r, float h)
        {
            var v = new System.Collections.Generic.List<Vector3>();
            var t = new System.Collections.Generic.List<int>();
            var top = new Vector3(0, h, 0); var bot = new Vector3(0, -h * 0.6f, 0);
            for (int i = 0; i < n; i++)
            {
                float a0 = i * Mathf.PI * 2f / n, a1 = (i + 1) * Mathf.PI * 2f / n;
                var p0 = new Vector3(Mathf.Cos(a0) * r, 0, Mathf.Sin(a0) * r);
                var p1 = new Vector3(Mathf.Cos(a1) * r, 0, Mathf.Sin(a1) * r);
                int k = v.Count; v.Add(top); v.Add(p1); v.Add(p0); t.Add(k); t.Add(k + 1); t.Add(k + 2);
                k = v.Count; v.Add(bot); v.Add(p0); v.Add(p1); t.Add(k); t.Add(k + 1); t.Add(k + 2);
            }
            var m = new Mesh { name = "crystal" };
            m.SetVertices(v); m.SetTriangles(t, 0);
            m.RecalculateNormals();
            m.RecalculateBounds();
            return m;
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Time.deltaTime;
            float lvl = s.lvl4 != null && s.lvl4.Length > 0 ? s.lvl4[0] : 0f;
            _bass += (lvl - _bass) * (1f - Mathf.Exp(-dt / 0.18f));
            // Kick envelope: fast attack (40 ms), 220 ms release — a punch,
            // never a one-frame jump.
            float kt = Mathf.Max(s.kick, s.hits4 != null && s.hits4.Length > 0 ? s.hits4[0] : 0f) * (1f - s.calm);
            _kick = kt > _kick ? Mathf.Lerp(_kick, kt, 1f - Mathf.Exp(-dt / 0.04f)) : _kick * Mathf.Exp(-dt / 0.22f);
            _kh = (_kh + 1) % _kickHist.Length;
            _kickHist[_kh] = _kick;
            float lm = s.lvl4 != null && s.lvl4.Length > 1 ? s.lvl4[1] : 0f;
            float hh = s.hits4 != null && s.hits4.Length > 3 ? Mathf.Max(s.hits4[2], s.hits4[3]) : 0f;
            _mid += (lm - _mid) * (1f - Mathf.Exp(-dt / 0.25f));
            _high += (hh - _high) * (1f - Mathf.Exp(-dt / 0.3f));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / 0.6f));
            // Speed and spin integrate smoothed values (no raw audio in the
            // integrator — motion stays smooth).
            _travel += dt * s.bpm / 60f * (1.2f + 7f * _energy * (1f - 0.6f * s.calm));
            _spinPhase += dt * (0.3f + 3.5f * _high);
            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f) _bloom = 1f;
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;
            _bloom = Mathf.Max(0f, _bloom - dt * s.bpm / 60f / 16f);
            float burst = 1f + 0.9f * Mathf.SmoothStep(0, 1, _bloom);

            float travel = _travel;
            float phrase = s.beat / 64f * Mathf.PI * 2f;
            float twist = 0.012f + 0.01f * Mathf.Sin(phrase * 0.5f);
            float swing = Mathf.Sin(phrase) * 1.4f + s.flow * 0.04f;
            float swell = 1f + 0.3f * _bass * (1f - s.calm);
            float breathe = 1f + 0.35f * (_mid - 0.35f);
            for (int i = 0; i < Count; i++)
            {
                float z0 = _h[i] * Length;
                float z = Mathf.Repeat(z0 - travel, Length) - 6f;
                float a = i * 2.39996f + z * twist + swing;
                float r = _rad[i] * burst * breathe;
                // The kick wave: nearer crystals get it first.
                int delay = (int)(Mathf.Clamp01(z / 140f) * 70f);
                float punch = _kickHist[(_kh - delay + _kickHist.Length) % _kickHist.Length];
                var p = new Vector3(Mathf.Cos(a) * r, Mathf.Sin(a) * r, z);
                var rot = Quaternion.Euler(_spinPhase * 60f * _spin[i], a * Mathf.Rad2Deg, _spinPhase * 35f * _spin[i] + 90f);
                // Grow in from the far fog so recycled crystals don't pop.
                float fade = Mathf.SmoothStep(0, 1, Mathf.InverseLerp(Length - 6f, Length - 40f, z));
                _m[i] = Matrix4x4.TRS(p, rot, Vector3.one * (_sz[i] * swell * (1f + 0.6f * punch) * fade));
            }
            Graphics.RenderMeshInstanced(_rp, _mesh, 0, _m);

            _sun.SetFloat("_Glow", 0.35f + 0.5f * Mathf.SmoothStep(0, 1, _bloom) + 0.2f * s.intensity);
            // Camera: centred, banking gently with the phrase.
            cam.transform.position = new Vector3(Mathf.Sin(phrase * 0.5f) * 1.5f, Mathf.Cos(phrase * 0.7f) * 1.0f, -8f);
            cam.transform.rotation = Quaternion.Euler(0, Mathf.Sin(phrase * 0.5f) * 4f, Mathf.Sin(phrase) * 7f);
        }
    }
}
