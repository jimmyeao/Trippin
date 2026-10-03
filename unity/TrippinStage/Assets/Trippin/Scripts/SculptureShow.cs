// Screen-content show: one big solid sculpture alone in a void, lit like a
// product shot. Its own thing on the screen: no stage, no crowd.
//  - Shape: a new form every 8 bars, melting in over two beats (pebble,
//    urchin, twisted spire, lobed flower, dimpled disc); it swells with
//    the bass; each kick sends a ripple from top to bottom; the twist
//    swings with the phrase.
//  - Motion: rotation speed integrates the smoothed energy and reverses
//    on phrase-length sines; the camera orbits slowly round it.
//  - Light: studio reflections slide over the surface as it turns; an
//    iridescent sheen drifts on the flow clock; a soft glow rides the
//    kick ripple.

using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    public class SculptureShow : MonoBehaviour
    {
        public Camera cam;
        public Material sculptMat;

        const int Forms = 5;

        const float FloorY = -3.0f;

        Transform _obj, _mirror;
        Material _mat, _mirMat;
        int _formA, _formB = 1, _lastBar8 = -1;
        float _morphStart, _bass, _energy, _kick, _rot, _orbit, _wave = 2f, _waveAmp, _lastKickT = -1f, _lineT;

        void Awake()
        {
            var go = new GameObject("sculpture");
            go.transform.SetParent(transform, false);
            go.AddComponent<MeshFilter>().sharedMesh = IcoSphere(6);
            _mat = new Material(sculptMat);
            var mr = go.AddComponent<MeshRenderer>();
            mr.sharedMaterial = _mat;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            _obj = go.transform;
            _obj.localScale = Vector3.one * 2.2f;
            // A mirrored copy below a glossy floor: the product-shot reflection.
            var mg = new GameObject("reflection");
            mg.transform.SetParent(transform, false);
            mg.AddComponent<MeshFilter>().sharedMesh = go.GetComponent<MeshFilter>().sharedMesh;
            _mirMat = new Material(sculptMat);
            _mirMat.SetFloat("_Mirror", 1f);
            var mmr = mg.AddComponent<MeshRenderer>();
            mmr.sharedMaterial = _mirMat;
            mmr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            _mirror = mg.transform;
            _mirror.localPosition = new Vector3(0, 2f * FloorY, 0);
            _mirror.localScale = new Vector3(2.2f, -2.2f, 2.2f);
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Mathf.Min(Time.deltaTime, 1f / 30f);
            int bar8 = Mathf.FloorToInt(s.beat / 32f);
            if (bar8 != _lastBar8)
            {
                _formA = _formB;
                _formB = (_formB + 1 + (bar8 % 2) * 2) % Forms;
                if (_formB == _formA) _formB = (_formA + 1) % Forms;
                _morphStart = s.beat;
                _lastBar8 = bar8;
            }
            float morph = Mathf.SmoothStep(0, 1, Mathf.Clamp01((s.beat - _morphStart) / 2f));

            float lb = s.lvl4 != null && s.lvl4.Length > 0 ? s.lvl4[0] : 0f;
            _bass += (lb - _bass) * (1f - Mathf.Exp(-dt / 0.2f));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / 0.8f));
            // Kick onsets launch a ripple from the top pole (-> -1 at the base).
            float kt = Mathf.Max(s.kick, s.hits4 != null && s.hits4.Length > 0 ? s.hits4[0] : 0f) * (1f - s.calm);
            if (kt > 0.45f && kt > _kick + 0.25f && Time.time - _lastKickT > 0.15f)
            {
                _wave = 1.15f;
                _waveAmp = Mathf.Clamp01(kt);
                _lastKickT = Time.time;
            }
            _kick = kt;
            _wave -= dt * 3.2f;
            _waveAmp *= Mathf.Exp(-dt / 0.5f);

            float phrase = s.beat / 64f * Mathf.PI * 2f;
            _rot += dt * (0.12f + 0.9f * _energy) * (1f - 0.5f * s.calm) * Mathf.Sin(phrase * 0.5f + 0.4f);
            _lineT += dt * (0.25f + 0.9f * _energy);

            foreach (var m in new[] { _mat, _mirMat })
            {
                m.SetFloat("_FormA", _formA);
                m.SetFloat("_FormB", _formB);
                m.SetFloat("_Morph", morph);
                m.SetFloat("_Swell", 0.1f * _bass * (1f - 0.5f * s.calm));
                m.SetFloat("_Twist", 0.9f * Mathf.Sin(phrase * 0.5f));
                m.SetFloat("_WaveFront", _wave);
                m.SetFloat("_WaveAmp", _waveAmp);
                m.SetFloat("_LineT", _lineT);
                m.SetFloat("_FloorY", FloorY);
            }
            _obj.localRotation = Quaternion.Euler(0, _rot * Mathf.Rad2Deg, 0);
            _mirror.localRotation = _obj.localRotation;

            // Camera: a slow orbit, rising and falling with the phrase.
            _orbit += dt * (0.05f + 0.06f * s.intensity);
            float dist = 10f + 1.2f * s.calm;
            var p = new Vector3(Mathf.Sin(_orbit) * dist, 1.2f + 1.6f * Mathf.Sin(phrase * 0.5f + 1f), Mathf.Cos(_orbit) * dist);
            cam.transform.position = Vector3.Lerp(cam.transform.position, p, 1f - Mathf.Exp(-dt * 1.5f));
            cam.transform.LookAt(new Vector3(0, -0.6f, 0));
        }

        // Unit icosphere, `n` subdivisions (6 -> 40962 vertices).
        internal static Mesh IcoSphere(int n)
        {
            float t = (1f + Mathf.Sqrt(5f)) / 2f;
            var v = new List<Vector3>
            {
                new Vector3(-1, t, 0), new Vector3(1, t, 0), new Vector3(-1, -t, 0), new Vector3(1, -t, 0),
                new Vector3(0, -1, t), new Vector3(0, 1, t), new Vector3(0, -1, -t), new Vector3(0, 1, -t),
                new Vector3(t, 0, -1), new Vector3(t, 0, 1), new Vector3(-t, 0, -1), new Vector3(-t, 0, 1),
            };
            for (int i = 0; i < v.Count; i++) v[i] = v[i].normalized;
            var f = new List<int>
            {
                0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7, 1, 8,
                3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9, 8, 1,
            };
            for (int s = 0; s < n; s++)
            {
                var cache = new Dictionary<long, int>();
                int Mid(int a, int b)
                {
                    long key = a < b ? ((long)a << 32) | (uint)b : ((long)b << 32) | (uint)a;
                    if (cache.TryGetValue(key, out int m)) return m;
                    v.Add(((v[a] + v[b]) * 0.5f).normalized);
                    cache[key] = v.Count - 1;
                    return v.Count - 1;
                }
                var nf = new List<int>(f.Count * 4);
                for (int i = 0; i < f.Count; i += 3)
                {
                    int a = f[i], b = f[i + 1], c = f[i + 2];
                    int ab = Mid(a, b), bc = Mid(b, c), ca = Mid(c, a);
                    nf.AddRange(new[] { a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca });
                }
                f = nf;
            }
            var m = new Mesh { name = "icosphere", indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            m.SetVertices(v);
            m.SetTriangles(f, 0);
            m.bounds = new Bounds(Vector3.zero, Vector3.one * 6f);
            return m;
        }
    }
}
