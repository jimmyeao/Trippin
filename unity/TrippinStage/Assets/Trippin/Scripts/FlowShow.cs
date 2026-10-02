// Screen-content show: ~262k GPU particles flowing into big shapes (sphere,
// torus, double helix, galaxy, gyroscope rings), a new shape every 4 bars
// morphing over two beats. Turbulence rides the smoothed mids/highs, the
// outward push the smoothed bass (never raw audio into the integrator); a
// drop scatters the cloud and lets it re-form. The camera orbits slowly,
// swinging direction with the phrase.

using UnityEngine;

namespace TrippinStage
{
    public class FlowShow : MonoBehaviour
    {
        public Camera cam;
        public ComputeShader flow;
        public Material pointsMat;

        const int Count = 1 << 18;
        const int Shapes = 5;

        ComputeBuffer _pos, _vel;
        Material _mat;
        RenderParams _rp;
        int _kernel;
        int _shapeA, _shapeB = 1, _lastBar4 = -1;
        float _morphStart, _bass, _turb, _scatter, _calmLong, _orbit;
        bool _drumsWas = true;

        void Awake()
        {
            _pos = new ComputeBuffer(Count, 16);
            _vel = new ComputeBuffer(Count, 16);
            var init = new Vector4[Count];
            var rnd = new System.Random(5);
            for (int i = 0; i < Count; i++)
                init[i] = new Vector4((float)rnd.NextDouble() * 30 - 15, (float)rnd.NextDouble() * 30 - 15, (float)rnd.NextDouble() * 30 - 15, 0);
            _pos.SetData(init);
            _vel.SetData(new Vector4[Count]);
            _kernel = flow.FindKernel("Step");
            _mat = new Material(pointsMat);
            _mat.SetBuffer("_Pos", _pos);
            _rp = new RenderParams(_mat) { worldBounds = new Bounds(Vector3.zero, Vector3.one * 500f) };
        }

        void OnDestroy()
        {
            _pos?.Release();
            _vel?.Release();
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Mathf.Min(Time.deltaTime, 1f / 30f);
            int bar4 = Mathf.FloorToInt(s.beat / 16f);
            if (bar4 != _lastBar4)
            {
                // New shape every 4 bars: the current blend becomes A.
                _shapeA = _shapeB;
                _shapeB = (_shapeB + 1 + (bar4 % 2)) % Shapes;
                if (_shapeB == _shapeA) _shapeB = (_shapeA + 1) % Shapes;
                _morphStart = s.beat;
                _lastBar4 = bar4;
            }
            float morph = Mathf.SmoothStep(0, 1, Mathf.Clamp01((s.beat - _morphStart) / 2f));

            float k = 1f - Mathf.Exp(-dt / 0.2f);
            float lb = s.lvl4 != null && s.lvl4.Length > 3 ? s.lvl4[0] : 0f;
            float lm = s.lvl4 != null && s.lvl4.Length > 3 ? 0.5f * (s.lvl4[1] + s.lvl4[2]) : 0f;
            _bass += (lb - _bass) * k;
            _turb += ((0.6f + 3.2f * lm) * (1f - 0.6f * s.calm) - _turb) * k;
            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f) _scatter = 1f;
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;
            _scatter = Mathf.Max(0f, _scatter - dt * 0.6f);

            flow.SetBuffer(_kernel, "_Pos", _pos);
            flow.SetBuffer(_kernel, "_Vel", _vel);
            flow.SetInt("_Count", Count);
            flow.SetFloat("_Dt", dt);
            flow.SetFloat("_T", s.flow * 0.15f);
            flow.SetFloat("_Morph", morph);
            flow.SetFloat("_ShapeA", _shapeA);
            flow.SetFloat("_ShapeB", _shapeB);
            flow.SetFloat("_Turb", _turb * (1f + 6f * _scatter));
            flow.SetFloat("_Stiff", 5f * (1f - 0.85f * _scatter));
            flow.SetFloat("_Push", 3f * _bass * (1f - s.calm) + 25f * _scatter);
            flow.SetFloat("_Scale", 1f + 0.12f * _bass);
            flow.Dispatch(_kernel, Count / 256, 1, 1);

            _mat.SetFloat("_Gain", 0.45f + 0.25f * s.intensity);
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, Count * 6);

            // Orbit: speed on the flow clock, direction swinging per phrase.
            float phrase = s.beat / 64f * Mathf.PI * 2f;
            // Direction eases through zero as the phrase sine turns — no snap.
            _orbit += dt * (0.08f + 0.12f * s.intensity) * Mathf.Sin(phrase * 0.5f);
            var p = new Vector3(Mathf.Sin(_orbit) * 34f, 6f + Mathf.Sin(phrase) * 9f, Mathf.Cos(_orbit) * 34f);
            cam.transform.position = Vector3.Lerp(cam.transform.position, p, 1f - Mathf.Exp(-dt * 1.5f));
            cam.transform.LookAt(Vector3.zero);
        }
    }
}
