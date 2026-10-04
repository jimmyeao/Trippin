// unity_coaster: the neon roller coaster of shaders/scenes/neon_coaster.wgsl,
// rebuilt for the Unity engine — the raymarched city/track becomes real
// geometry, so it holds 60 fps where the shader version cannot. The track is
// the same closed Fourier figure-eight with the same physical timing (the
// WARP table): the car crawls the chain lift and flies the drops, banking
// into the turns, first person, on the energy clock.
//  - Motion: the ride itself — lap position comes from the smooth energy
//    clock (clock4.x), never raw audio; the camera banks with the track and
//    the FOV widens with speed.
//  - Shape: rail chase lights run forward on the beat; window blocks and
//    billboards follow the spectrum; a colour ring crosses the city on kicks.
//  - Luminance: rails, facades and trim follow the eased loudness; kick
//    pulses calm down in breakdowns (as the wgsl original).
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace TrippinStage
{
    public sealed class CoasterShow : KitShow
    {
        public Material coasterCityMat, deckMat, noseMat, skyMat;

        const float LAP_BEATS = 96f;
        const float S = 40f;            // city block size
        const float TAUf = Mathf.PI * 2f;
        const int N = 720;              // track samples

        // ---- GENERATED track tables (tools/coaster_track.py; same numbers as
        // shaders/scenes/neon_coaster.wgsl): (k, cos coef, sin coef) ----------
        static readonly Vector3[] TX = { V(1, 0, 330), V(3, 0, 45) };
        static readonly Vector3[] TY = { V(1, -20, 55), V(2, 22, 0), V(3, 0, 14), V(5, 8, 0) };
        static readonly Vector3[] TZ = { V(2, 0, 190), V(1, 60, 0), V(3, 25, 0) };
        const float TY0 = 95f;
        static readonly Vector3[] WARP = {
            V(1, -0.07860f, -0.06091f), V(2, 0.02667f, 0.02272f), V(3, -0.00478f, 0.00788f),
            V(4, -0.00161f, 0.00986f), V(5, 0.00213f, -0.00053f), V(6, 0.00019f, 0.00074f),
            V(7, -0.00161f, -0.00056f), V(8, 0.00084f, -0.00086f) };
        const float WARP0 = 0.05715f;
        static readonly Vector3[] BANK = {
            V(1, -0.06884f, -0.24405f), V(2, -0.07326f, -0.09030f), V(3, 0.04695f, -0.15037f),
            V(4, -0.06289f, -0.00165f), V(5, 0.03201f, 0.20547f), V(6, 0.09718f, 0.04219f),
            V(7, -0.02412f, 0.00083f), V(8, -0.00690f, -0.00192f) };
        const float BANK0 = 0.02306f;
        static Vector3 V(float k, float c, float s) => new Vector3(k, c, s);

        // Per-sample track frame.
        Vector3[] _pos; Vector3[] _rgt, _up;

        TubeRibbon _railL, _railR;
        Mesh _cube;
        Matrix4x4[] _towers, _towersMir;
        Material _cityM, _cityMirM, _deckM, _noseM, _skyM, _groundM;
        RenderParams _cityRp, _cityMirRp;
        float _wave = 999f, _waveAmp, _lit;
        readonly float[] _hc = new float[9];
        readonly float[] _hs = new float[9];

        // ---- Track math (verbatim port of the WGSL) --------------------------

        void Harm(float th)
        {
            float c1 = Mathf.Cos(th), s1 = Mathf.Sin(th);
            _hc[0] = 1f; _hs[0] = 0f; _hc[1] = c1; _hs[1] = s1;
            for (int k = 2; k < 9; k++)
            {
                _hc[k] = _hc[k - 1] * c1 - _hs[k - 1] * s1;
                _hs[k] = _hs[k - 1] * c1 + _hc[k - 1] * s1;
            }
        }

        static float Eval(Vector3[] t, float[] c, float[] s)
        {
            float p = 0f;
            for (int i = 0; i < t.Length; i++) { int k = (int)t[i].x; p += t[i].y * c[k] + t[i].z * s[k]; }
            return p;
        }

        static float EvalD(Vector3[] t, float[] c, float[] s)
        {
            float d = 0f;
            for (int i = 0; i < t.Length; i++) { int k = (int)t[i].x; d += k * (-t[i].y * s[k] + t[i].z * c[k]); }
            return d;
        }

        Vector3 Track(float th) { Harm(th); return new Vector3(Eval(TX, _hc, _hs), TY0 + Eval(TY, _hc, _hs), Eval(TZ, _hc, _hs)); }
        Vector3 TrackD(float th) { Harm(th); return new Vector3(EvalD(TX, _hc, _hs), EvalD(TY, _hc, _hs), EvalD(TZ, _hc, _hs)); }
        float Bank(float th) { Harm(th); return BANK0 + Eval(BANK, _hc, _hs); }

        // Lap fraction -> track parameter (physical timing), and d(th)/d(phi).
        Vector2 Warp(float phi)
        {
            float th = TAUf * (phi - WARP0);
            float dp = 1f / TAUf;
            for (int n = 0; n < 6; n++)
            {
                float p = th / TAUf + WARP0;
                dp = 1f / TAUf;
                for (int i = 0; i < 8; i++)
                {
                    float k = WARP[i].x;
                    p += WARP[i].y * Mathf.Cos(k * th) + WARP[i].z * Mathf.Sin(k * th);
                    dp += k * (-WARP[i].y * Mathf.Sin(k * th) + WARP[i].z * Mathf.Cos(k * th));
                }
                float e = Mathf.Repeat(p - phi + 0.5f, 1f) - 0.5f;
                th -= Mathf.Clamp(e / dp, -0.8f, 0.8f);
            }
            return new Vector2(th, 1f / dp);
        }

        // Banked frame at a track point: T tangent, out right/up.
        void FrameAt(Vector3 t, float b, out Vector3 r, out Vector3 u)
        {
            Vector3 r0 = Vector3.Cross(Vector3.up, t).normalized;
            Vector3 u0 = Vector3.Cross(t, r0);
            float cb = Mathf.Cos(b), sb = Mathf.Sin(b);
            r = r0 * cb - u0 * sb;
            u = u0 * cb + r0 * sb;
        }

        // ---- Build -----------------------------------------------------------

        protected override void Build()
        {
            _pos = new Vector3[N + 1]; _rgt = new Vector3[N + 1]; _up = new Vector3[N + 1];
            float trackLen = 0f;
            Vector3 prev = Track(0f);
            for (int i = 0; i <= N; i++)
            {
                float th = i / (float)N * TAUf;
                Vector3 tp = Track(th);
                Vector3 t = TrackD(th).normalized;
                FrameAt(t, Bank(th), out _rgt[i], out _up[i]);
                _pos[i] = tp;
                if (i > 0) trackLen += Vector3.Distance(prev, tp);
                prev = tp;
            }

            BuildRails(trackLen);
            BuildDeck(trackLen);
            BuildPylons();
            BuildCity();

            // Smog dome, wet street, the car's nose.
            var sky = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            Destroy(sky.GetComponent<Collider>());
            sky.name = "sky";
            sky.transform.SetParent(transform, false);
            sky.transform.localScale = Vector3.one * 2600f;
            _skyM = new Material(skyMat);
            sky.GetComponent<Renderer>().sharedMaterial = _skyM;
            _skyM.SetFloat("_Glow", 0.4f);

            var ground = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(ground.GetComponent<Collider>());
            ground.name = "wet street";
            ground.transform.SetParent(transform, false);
            ground.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            ground.transform.localScale = new Vector3(2400f, 2400f, 1f);
            _groundM = new Material(groundMat);
            _groundM.SetFloat("_FogDist", 500f);
            ground.GetComponent<Renderer>().sharedMaterial = _groundM;

            _noseM = Kit.Fullscreen(transform, noseMat, "car nose");
        }

        void BuildRails(float trackLen)
        {
            _railL = new TubeRibbon(transform, tubeMat, N + 1, 6, "rail L");
            _railR = new TubeRibbon(transform, tubeMat, N + 1, 6, "rail R");
            for (int i = 0; i <= N; i++)
            {
                _railL.P[i] = _pos[i] - _rgt[i] * 0.75f + _up[i] * 0.1f;
                _railR.P[i] = _pos[i] + _rgt[i] * 0.75f + _up[i] * 0.1f;
                _railL.R[i] = _railR.R[i] = 0.14f;
                _railL.Hu[i] = _railR.Hu[i] = i * 0.5f / N;   // subtle hue drift along the lap
            }
            _railL.Mat.SetFloat("_Hue", 0.55f);
            _railR.Mat.SetFloat("_Hue", 0.55f);
            _railL.Mat.SetFloat("_PulseAmt", 0f);           // chase is per-point, not the built-in packets
            _railR.Mat.SetFloat("_PulseAmt", 0f);
            _railL.Apply(0.5f);                             // populate the mesh before the first Frame
            _railR.Apply(0.5f);
        }

        void BuildDeck(float trackLen)
        {
            var mesh = new Mesh { name = "deck", indexFormat = IndexFormat.UInt32 };
            var verts = new Vector3[(N + 1) * 2];
            var uvs = new Vector2[(N + 1) * 2];
            var tris = new int[N * 6];
            for (int i = 0; i <= N; i++)
            {
                verts[i * 2] = _pos[i] - _rgt[i] * 0.95f + _up[i] * 0.05f;
                verts[i * 2 + 1] = _pos[i] + _rgt[i] * 0.95f + _up[i] * 0.05f;
                uvs[i * 2] = new Vector2(i / (float)N, 0f);
                uvs[i * 2 + 1] = new Vector2(i / (float)N, 1f);
                if (i < N)
                {
                    int a = i * 2, b = a + 1, c = a + 2, d = a + 3, t = i * 6;
                    tris[t] = a; tris[t + 1] = c; tris[t + 2] = b;
                    tris[t + 3] = b; tris[t + 4] = c; tris[t + 5] = d;
                }
            }
            mesh.vertices = verts; mesh.uv = uvs; mesh.triangles = tris;
            mesh.RecalculateNormals();
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 2600f);
            _deckM = new Material(deckMat);
            _deckM.SetFloat("_Len", trackLen);
            Kit.Part(transform, "deck", mesh, _deckM, Vector3.zero, Vector3.one);
        }

        void BuildPylons()
        {
            var prim = GameObject.CreatePrimitive(PrimitiveType.Cube);
            _cube = prim.GetComponent<MeshFilter>().sharedMesh;
            Destroy(prim);
            var m = new Material(structMat);
            int n = 0;
            for (int i = 0; i < N; i += 8)
            {
                if (_pos[i].y < 14f) continue;
                var g = new GameObject("pylon " + n++);
                g.transform.SetParent(transform, false);
                g.transform.localPosition = new Vector3(_pos[i].x, _pos[i].y * 0.5f, _pos[i].z);
                g.transform.localScale = new Vector3(0.9f, _pos[i].y, 0.9f);
                g.AddComponent<MeshFilter>().sharedMesh = _cube;
                var mr = g.AddComponent<MeshRenderer>();
                mr.sharedMaterial = m;
                mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
        }

        // City blocks on the 40 m grid, towers cleared wherever the track
        // passes (sampled live — follows the tables if they're regenerated).
        void BuildCity()
        {
            var cleared = new HashSet<long>();
            for (int i = 0; i < 4000; i++)
            {
                Vector3 p = Track(i / 4000f * TAUf);
                int cx = Mathf.FloorToInt(p.x / S), cz = Mathf.FloorToInt(p.z / S);
                for (int dx = -1; dx <= 1; dx++)
                    for (int dz = -1; dz <= 1; dz++)
                    {
                        Vector2 ctr = new Vector2((cx + dx + 0.5f) * S, (cz + dz + 0.5f) * S);
                        if (Vector2.Distance(ctr, new Vector2(p.x, p.z)) < 45f)
                            cleared.Add(((long)(cx + dx) << 20) | (uint)(cz + dz));
                    }
            }
            var list = new List<Matrix4x4>();
            for (int gx = -13; gx <= 13; gx++)
                for (int gz = -8; gz <= 8; gz++)
                {
                    if (cleared.Contains(((long)gx << 20) | (uint)gz)) continue;
                    float h = Kit.H(gx * 977 + gz, 1);
                    float h2 = Kit.H(gx * 977 + gz, 5);
                    if (Kit.H(gx * 977 + gz, 9) < 0.06f) continue;   // a few empty lots
                    float hgt = 40f + h * h * 160f + Mathf.Pow(Kit.H(gx * 977 + gz, 7), 6f) * 180f;
                    float half = S * (0.28f + 0.1f * h2);
                    var c = new Vector3((gx + 0.5f) * S, hgt * 0.5f, (gz + 0.5f) * S);
                    list.Add(Matrix4x4.TRS(c, Quaternion.identity, new Vector3(half * 2f, hgt, half * 2f)));
                }
            _towers = list.ToArray();
            Debug.Log($"[Coaster] city: {_towers.Length} towers, {cleared.Count} cells cleared for the track");
            var flip = Matrix4x4.Scale(new Vector3(1f, -1f, 1f));
            _towersMir = new Matrix4x4[_towers.Length];
            for (int i = 0; i < _towers.Length; i++) _towersMir[i] = flip * _towers[i];
            _cityM = new Material(coasterCityMat) { enableInstancing = true };
            _cityMirM = new Material(coasterCityMat) { enableInstancing = true };
            _cityMirM.SetFloat("_Mirror", 1f);
            var big = new Bounds(Vector3.zero, Vector3.one * 4000f);
            _cityRp = new RenderParams(_cityM) { worldBounds = big, shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
            _cityMirRp = new RenderParams(_cityMirM) { worldBounds = big, shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
        }

        // ---- Frame -----------------------------------------------------------

        protected override void Frame(ShowState s, float dt)
        {
            cam.farClipPlane = 3000f;
            float drive = 0.5f + 0.8f * s.intensity;
            float phi = Mathf.Repeat(s.clock4 != null && s.clock4.Length > 0 ? s.clock4[0] : s.beat, LAP_BEATS) / LAP_BEATS + 0.0137f;
            phi = Mathf.Repeat(phi, 1f);
            Vector2 w = Warp(phi);
            float th = w.x;

            Vector3 cpos = Track(th);
            Vector3 t = TrackD(th).normalized;
            FrameAt(t, Bank(th), out Vector3 r, out Vector3 u);
            Vector3 cd = TrackD(th);
            float speed = cd.magnitude * w.y;                       // metres per lap-fraction
            float spdN = Mathf.Clamp01((speed / LAP_BEATS / 0.5f - 20f) / 50f);

            Vector3 ro = cpos + u * 1.5f;
            // Look a touch ahead along the track (smooths the view in turns).
            Vector3 ta = Track(th + 0.025f), td = TrackD(th + 0.025f).normalized;
            FrameAt(td, Bank(th + 0.025f), out Vector3 ra, out Vector3 ua);
            Vector3 ahead = (ta + ua * 1.5f - ro).normalized;
            Vector3 fwd = Vector3.Lerp(t, ahead, 0.7f).normalized;
            Vector3 upv = Vector3.Cross(fwd, Vector3.Cross(u, fwd).normalized).normalized;

            cam.transform.position = ro;
            cam.transform.rotation = Quaternion.LookRotation(fwd, upv);
            cam.fieldOfView = Mathf.Lerp(47f, 62f, spdN);

            // Rail chase lights running forward on the beat (calm-aware).
            float bp = Mathf.Lerp(Mathf.Exp(-s.beat_phase * 6f), 0.25f, s.calm);
            float flowPh = s.flow * TAUf;
            for (int i = 0; i <= N; i++)
            {
                float thi = i / (float)N * TAUf;
                float chase = Mathf.Pow(0.5f + 0.5f * Mathf.Cos(thi * 260f - flowPh), 8f);
                float k = (0.5f + 1.8f * chase * (0.4f + rx.Spec(0.1f))) * (0.6f + 0.9f * bp) * drive;
                _railL.K[i] = _railR.K[i] = k;
            }
            _railL.Apply(drive);
            _railR.Apply(drive);
            _deckM.SetFloat("_Gain", drive);
            _noseM.SetFloat("_Gain", drive);
            _skyM.SetFloat("_Glow", 0.3f + 0.6f * s.intensity);

            // City: kick ring over the rooftops, more windows as it builds.
            if (rx.kick > 0.8f && _wave > 60f) { _wave = 0f; _waveAmp = Mathf.Clamp01(rx.kick + 0.3f); }
            _wave += dt * 140f;
            _waveAmp *= Mathf.Exp(-dt / 1.2f);
            _lit = Mathf.Lerp(_lit, rx.lum, 1f - Mathf.Exp(-dt * 4f));
            foreach (var m in new[] { _cityM, _cityMirM })
            {
                m.SetFloat("_Wave", _wave);
                m.SetFloat("_WaveAmp", _waveAmp);
                m.SetFloat("_Lit", _lit);
                m.SetFloat("_FogDist", 420f);
                m.SetFloat("_Block", S);
            }

            for (int i = 0; i < _towers.Length; i += 500)
            {
                int n = Mathf.Min(500, _towers.Length - i);
                Graphics.RenderMeshInstanced(_cityRp, _cube, 0, _towers, n, i);
                Graphics.RenderMeshInstanced(_cityMirRp, _cube, 0, _towersMir, n, i);
            }
        }
    }
}
