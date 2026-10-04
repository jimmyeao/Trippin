// Shared toolkit for the "kit" shows (KitShow subclasses): one place for the
// audio vocabulary every show reacts to, and the beam / glow / haze / camera
// helpers they are built from, so each show is just geometry and mappings.
//
// Reactivity rules (see AGENTS.md): the slow pres4/clock4 signals move too
// slowly on real music to read as reactive, so every show also reads eased
// fast signals (Rx.*Fast, Rx.kick, Rx.lum) and maps them onto SHAPE and
// onto LUMINANCE. They are eased (fast attack, slower release), so shape and
// brightness follow the music within ~70 ms but never step or flash. They
// never go into a motion integrator: travel and rotation use the energy
// clocks (Rx.clk), direction swings use phrase-length sines (Rx.phrase).

using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    /// One show's view of the music, ticked once a frame by KitShow.
    public sealed class Rx
    {
        public float beat, clk, clkBass, clkHigh, phrase;
        /// Trippin's tracked beat count, continuous: where the tempo tracker jumps (a re-lock or a
        /// downbeat-phase correction), the output carries on smoothly and eases onto the new grid
        /// over ~0.4 s instead of stepping. Use this (never `clk`) for anything that has to land on
        /// the beat: dance moves, pumps, scratches, a lap that returns every N bars.
        public float beatS;
        float _bPrev = -1f, _bRate = 2f, _bOff;
        public float bassSlow, midSlow, highSlow, calm;
        public float bassFast, midFast, mhFast, highFast, kick, lum;
        public float tension, impact, intensity;
        public bool dropped;
        public readonly float[] spec = new float[32]; // eased spectrum, 0..1 per bin
        readonly Vector4[] _specVec = new Vector4[8];
        static readonly int IdLvl = Shader.PropertyToID("_RxLvl"), IdMisc = Shader.PropertyToID("_RxMisc"),
            IdClk = Shader.PropertyToID("_RxClk"), IdSpec = Shader.PropertyToID("_RxSpec");

        public void Reset()
        {
            impact = 0f;
            _bPrev = -1f; _bOff = 0f;
            for (int i = 0; i < spec.Length; i++) spec[i] = 0f;
        }

        public void Tick(ShowState s, float dt)
        {
            DropDirector.Tick(s, dt);
            beat = s.beat;
            if (_bPrev < 0f || dt <= 1e-4f) { _bOff = 0f; }
            else
            {
                float delta = s.beat - _bPrev;
                if (Mathf.Abs(delta - _bRate * dt) > 0.15f)      // the tracker jumped: keep the output continuous
                    _bOff = (_bPrev + _bOff) + _bRate * dt - s.beat;
                else
                    _bRate += (Mathf.Clamp(delta / dt, 0.5f, 5f) - _bRate) * 0.1f;
                _bOff *= Mathf.Exp(-2.5f * dt);
            }
            _bPrev = s.beat;
            beatS = s.beat + _bOff;
            clk = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            clkBass = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            clkHigh = s.clock4 != null && s.clock4.Length > 3 ? s.clock4[3] : beat;
            phrase = beat / 64f * Mathf.PI * 2f;
            intensity = s.intensity;

            float k = 1f - Mathf.Exp(-dt * 1.2f);
            bassSlow += (Pres(s, 0) - bassSlow) * k;
            midSlow += (Pres(s, 1) - midSlow) * k;
            highSlow += (Pres(s, 3) - highSlow) * k;
            calm += (s.calm - calm) * k;

            bassFast = Eased.Follow(bassFast, Eased.Lvl(s, 0), 14f, 3f, dt);
            midFast = Eased.Follow(midFast, Eased.Lvl(s, 1), 8f, 3f, dt);
            mhFast = Eased.Follow(mhFast, Eased.Lvl(s, 2), 8f, 3f, dt);
            highFast = Eased.Follow(highFast, Eased.Lvl(s, 3), 8f, 3f, dt);
            kick = Eased.Follow(kick, Eased.Hit(s, 0), 20f, 4f, dt);
            // Overall loudness, weighted to where most of the energy lives:
            // drives luminance, so quiet passages dim and loud ones bloom.
            float loud = 0.45f * Eased.Lvl(s, 0) + 0.3f * Eased.Lvl(s, 1) + 0.15f * Eased.Lvl(s, 2) + 0.1f * Eased.Lvl(s, 3);
            lum = Eased.Follow(lum, Mathf.Clamp01(loud * 1.4f), 6f, 1.5f, dt);

            if (s.spectrum != null)
                for (int i = 0; i < spec.Length && i < s.spectrum.Length; i++)
                    spec[i] = Eased.Follow(spec[i], Mathf.Clamp01(s.spectrum[i]), 16f, 5f, dt);

            tension = Mathf.SmoothStep(0f, 1f, DropDirector.Tension);
            // DropDirector.Impact steps 0 -> 1 in one frame: give it a ~0.25 s attack.
            float imp = DropDirector.Impact;
            impact = imp > impact ? impact + (imp - impact) * (1f - Mathf.Exp(-12f * dt)) : imp;
            dropped = DropDirector.Dropped;
            PushGlobals();
        }

        /// The eased signals as shader globals, so a fullscreen / surface shader
        /// reads the same smoothed vocabulary the C# shows do (TrippinCommon.hlsl:
        /// _RxLvl bass/mid/mh/high, _RxMisc kick/lum/tension/impact, _RxClk
        /// clk/clkHigh/phrase/calm, _RxSpec[8] = 32 spectrum bins).
        void PushGlobals()
        {
            Shader.SetGlobalVector(IdLvl, new Vector4(bassFast, midFast, mhFast, highFast));
            Shader.SetGlobalVector(IdMisc, new Vector4(kick, lum, tension, impact));
            Shader.SetGlobalVector(IdClk, new Vector4(clk, clkHigh, phrase, calm));
            for (int i = 0; i < 8; i++)
                _specVec[i] = new Vector4(spec[i * 4], spec[i * 4 + 1], spec[i * 4 + 2], spec[i * 4 + 3]);
            Shader.SetGlobalVectorArray(IdSpec, _specVec);
        }

        /// Energy clock `i` (0 mix, 1 bass, 2 mid, 3 high), in beats; the beat if missing.
        public static float Clock(ShowState s, int i) =>
            s.clock4 != null && s.clock4.Length > i ? s.clock4[i] : s.beat;

        /// Eased band level 0..3 (bass, mid, mid-high, high).
        public float Band(int i)
        {
            switch (i & 3)
            {
                case 0: return bassFast;
                case 1: return midFast;
                case 2: return mhFast;
                default: return highFast;
            }
        }

        static float Pres(ShowState s, int band) =>
            s.pres4 != null && s.pres4.Length > band ? s.pres4[band] : 0.4f;

        /// Eased spectrum at x in 0..1 (interpolated between bins).
        public float Spec(float x)
        {
            float f = Mathf.Clamp01(x) * (spec.Length - 1);
            int i = Mathf.Min((int)f, spec.Length - 2);
            return Mathf.Lerp(spec[i], spec[i + 1], f - i);
        }

        /// Luminance gain shared by the kit shows: follows the music's
        /// loudness, dips through a build (the held breath) and flares on the
        /// drop. `lo`/`hi` bound it for quiet and loud passages.
        public float Gain(float lo = 0.6f, float hi = 1.5f)
        {
            return Mathf.Lerp(lo, hi, lum) * (1f - 0.35f * tension) * (1f + 1.2f * impact);
        }
    }

    public static class Kit
    {
        /// Deterministic hash of an integer seed -> 0..1 (stable per index, no Random).
        public static float H(int n, int salt = 0)
        {
            uint x = (uint)(n * 73856093) ^ (uint)(salt * 19349663 + 83492791);
            x ^= x >> 16; x *= 0x7feb352dU; x ^= x >> 15; x *= 0x846ca68bU; x ^= x >> 16;
            return (x & 0xffffffu) / 16777216f;
        }

        /// The global palette at t, pushed to a fully saturated colour.
        public static Color Hue(float t)
        {
            Color c = TrippinLink.Palette(t);
            float mx = Mathf.Max(Mathf.Max(c.r, c.g), Mathf.Max(c.b, 1e-3f));
            return new Color(c.r / mx, c.g / mx, c.b / mx, 1f);
        }

        public static Mesh BeamMesh()
        {
            var m = new Mesh { name = "beam" };
            m.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            m.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            m.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            m.bounds = new Bounds(Vector3.zero, Vector3.one * 600f); // billboarded in the shader
            return m;
        }

        /// A (nu+1) x (nv+1) vertex grid with uv in 0..1 and position (u, v, 0);
        /// the vertex shaders re-interpret uv, so the bounds are huge.
        public static Mesh GridMesh(int nu, int nv, string name)
        {
            var verts = new Vector3[(nu + 1) * (nv + 1)];
            var uvs = new Vector2[verts.Length];
            var tris = new int[nu * nv * 6];
            for (int y = 0; y <= nv; y++)
                for (int x = 0; x <= nu; x++)
                {
                    int k = y * (nu + 1) + x;
                    uvs[k] = new Vector2(x / (float)nu, y / (float)nv);
                    verts[k] = new Vector3(uvs[k].x, uvs[k].y, 0f);
                }
            int t = 0;
            for (int y = 0; y < nv; y++)
                for (int x = 0; x < nu; x++)
                {
                    int a = y * (nu + 1) + x, b = a + 1, c = a + nu + 1, d = c + 1;
                    tris[t++] = a; tris[t++] = c; tris[t++] = b;
                    tris[t++] = b; tris[t++] = c; tris[t++] = d;
                }
            var mesh = new Mesh { name = name, indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            mesh.vertices = verts;
            mesh.uv = uvs;
            mesh.triangles = tris;
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 1200f);
            return mesh;
        }

        /// A full-screen pass: a quad whose vertex shader writes clip space straight
        /// from uv (so it covers the view whatever the camera does). Returns the
        /// material instance. The shader should use ZTest Always and draw first.
        public static Material Fullscreen(Transform parent, Material mat, string name)
        {
            var m = new Material(mat);
            Part(parent, name, GridMesh(1, 1, name), m, Vector3.zero, Vector3.one);
            return m;
        }

        public static GameObject Part(Transform parent, string name, Mesh mesh, Material mat, Vector3 pos, Vector3 scale)
        {
            var g = new GameObject(name);
            g.transform.SetParent(parent, false);
            g.transform.localPosition = pos;
            g.transform.localScale = scale;
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            mr.sharedMaterial = mat;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            return g;
        }

        /// A big floor quad with the wet-asphalt Ground material (fogs to the palette).
        public static void Floor(Transform parent, Material groundMat, float fog)
        {
            var g = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Object.Destroy(g.GetComponent<Collider>());
            g.name = "floor";
            g.transform.SetParent(parent, false);
            g.transform.localRotation = Quaternion.Euler(90, 0, 0);
            g.transform.localScale = new Vector3(1200, 1200, 1);
            var m = new Material(groundMat);
            m.SetFloat("_FogDist", fog);
            g.GetComponent<Renderer>().sharedMaterial = m;
        }

        /// Quads at `pos` (centre) with the given sizes, shared hue-offset per layer.
        public static Material[] HazeLayers(Transform parent, Material hazeMat, Vector3[] pos, Vector2[] size, float[] hue)
        {
            var mats = new Material[pos.Length];
            for (int i = 0; i < pos.Length; i++)
            {
                var h = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Object.Destroy(h.GetComponent<Collider>());
                h.name = "haze " + i;
                h.transform.SetParent(parent, false);
                h.transform.localPosition = pos[i];
                h.transform.localScale = new Vector3(size[i].x, size[i].y, 1f);
                mats[i] = new Material(hazeMat);
                mats[i].SetFloat("_HueOff", hue[i]);
                h.GetComponent<Renderer>().sharedMaterial = mats[i];
                h.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
            return mats;
        }
    }

    /// A fixed set of thin laser segments (the Beam shader). Count never
    /// changes; segments you don't set stay hidden. With `mirror`, a dimmer
    /// reflection is drawn under the floor (y -> -y).
    public sealed class BeamPool
    {
        public readonly int Count;
        public float MirrorGain = 0.3f;
        readonly Transform[] _t, _m;
        readonly Renderer[] _r, _mr;
        readonly bool _mirror;
        readonly MaterialPropertyBlock _mpb = new MaterialPropertyBlock();

        public BeamPool(Transform parent, Material beamMat, int n, string name, bool mirror,
            float width = 0.035f, float spread = 0f, float core = 40f, float smoke = 0.55f, float fade = 0.98f, float hot = 0.2f)
        {
            Count = n;
            _mirror = mirror;
            _t = new Transform[n]; _r = new Renderer[n];
            if (mirror) { _m = new Transform[n]; _mr = new Renderer[n]; }
            var mesh = Kit.BeamMesh();
            for (int i = 0; i < n; i++)
            {
                _t[i] = Make(parent, beamMat, mesh, name + " " + i, width, spread, core, smoke, fade, hot, out _r[i]);
                if (mirror) _m[i] = Make(parent, beamMat, mesh, name + " reflection " + i, width, spread, core, smoke, fade, hot, out _mr[i]);
            }
        }

        static Transform Make(Transform parent, Material beamMat, Mesh mesh, string name,
            float width, float spread, float core, float smoke, float fade, float hot, out Renderer r)
        {
            var g = new GameObject(name);
            g.transform.SetParent(parent, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            var m = new Material(beamMat);
            m.SetFloat("_Width", width);
            m.SetFloat("_Spread", spread);
            m.SetFloat("_Core", core);
            m.SetFloat("_Smoke", smoke);
            m.SetFloat("_Fade", fade);
            m.SetFloat("_Hot", hot);
            mr.sharedMaterial = m;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r = mr;
            return g.transform;
        }

        static void Place(Transform t, Vector3 a, Vector3 b)
        {
            Vector3 d = b - a;
            float len = d.magnitude;
            t.localPosition = a;
            t.localRotation = Quaternion.FromToRotation(Vector3.up, len > 1e-4f ? d / len : Vector3.up);
            t.localScale = new Vector3(1f, Mathf.Max(len, 1e-3f), 1f);
        }

        /// Place segment i from a to b with colour c and intensity (<= 0 hides it).
        /// `width` > 0 overrides the beam width for this segment.
        public void Set(int i, Vector3 a, Vector3 b, Color c, float intensity, float width = -1f)
        {
            bool on = intensity > 0.002f;
            _r[i].enabled = on;
            if (_mirror) _mr[i].enabled = on;
            if (!on) return;
            Place(_t[i], a, b);
            _mpb.Clear();
            _mpb.SetColor("_Color", c);
            _mpb.SetFloat("_Intensity", intensity);
            if (width > 0f) _mpb.SetFloat("_Width", width);
            _r[i].SetPropertyBlock(_mpb);
            if (_mirror)
            {
                Place(_m[i], new Vector3(a.x, -a.y, a.z), new Vector3(b.x, -b.y, b.z));
                _mpb.SetFloat("_Intensity", intensity * MirrorGain);
                _mr[i].SetPropertyBlock(_mpb);
            }
        }

        public void Hide(int i)
        {
            _r[i].enabled = false;
            if (_mirror) _mr[i].enabled = false;
        }
    }

    /// A glowing tube along a polyline (the Tube shader): one mesh and one draw
    /// call however many points it has, rebuilt each frame. Fill P / R / K / Hu
    /// (position, radius, intensity, hue offset per point), then Apply(). For a
    /// closed loop repeat the first point as the last. With `mirror`, a dimmer
    /// reflection is drawn under the floor (y -> -y).
    public sealed class TubeRibbon
    {
        public readonly int Points, Sides;
        public readonly Vector3[] P;
        public readonly float[] R, K, Hu;
        public readonly Material Mat;
        public float MirrorGain = 0.3f;
        readonly Mesh _mesh;
        readonly Vector3[] _v, _n;
        readonly Vector2[] _uv0, _uv1;
        readonly Renderer _r, _mr;
        readonly MaterialPropertyBlock _mpb = new MaterialPropertyBlock();

        public TubeRibbon(Transform parent, Material tube, int points, int sides, string name, bool mirror = false)
        {
            Points = points; Sides = sides;
            P = new Vector3[points]; R = new float[points]; K = new float[points]; Hu = new float[points];
            int ring = sides + 1;
            _v = new Vector3[points * ring]; _n = new Vector3[points * ring];
            _uv0 = new Vector2[points * ring]; _uv1 = new Vector2[points * ring];
            var tris = new int[(points - 1) * sides * 6];
            int t = 0;
            for (int i = 0; i < points - 1; i++)
                for (int j = 0; j < sides; j++)
                {
                    int a = i * ring + j, b = a + 1, c = a + ring, d = c + 1;
                    tris[t++] = a; tris[t++] = c; tris[t++] = b;
                    tris[t++] = b; tris[t++] = c; tris[t++] = d;
                }
            for (int i = 0; i < points; i++)
                for (int j = 0; j < ring; j++)
                    _uv0[i * ring + j] = new Vector2(i / (float)(points - 1), j / (float)sides);
            _mesh = new Mesh { name = name, indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            _mesh.MarkDynamic();
            _mesh.vertices = _v;
            _mesh.triangles = tris;
            _mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 1200f);
            Mat = new Material(tube);
            _r = Make(parent, name, Vector3.one);
            if (mirror) _mr = Make(parent, name + " reflection", new Vector3(1f, -1f, 1f));
        }

        Renderer Make(Transform parent, string name, Vector3 scale)
        {
            var g = new GameObject(name);
            g.transform.SetParent(parent, false);
            g.transform.localScale = scale;
            g.AddComponent<MeshFilter>().sharedMesh = _mesh;
            var mr = g.AddComponent<MeshRenderer>();
            mr.sharedMaterial = Mat;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            return mr;
        }

        public void Show(bool on) { _r.enabled = on; if (_mr != null) _mr.enabled = on; }

        /// Rebuild the mesh from P/R/K/Hu and set the luminance gain.
        public void Apply(float gain)
        {
            int ring = Sides + 1;
            Vector3 nrm = Vector3.zero;
            for (int i = 0; i < Points; i++)
            {
                Vector3 a = P[Mathf.Max(i - 1, 0)], b = P[Mathf.Min(i + 1, Points - 1)];
                Vector3 tg = (b - a).normalized;
                if (tg.sqrMagnitude < 1e-8f) tg = Vector3.up;
                if (i == 0 || nrm.sqrMagnitude < 1e-8f)
                {
                    Vector3 axis = Mathf.Abs(tg.y) < 0.9f ? Vector3.up : Vector3.right;
                    nrm = Vector3.Cross(tg, axis).normalized;
                }
                else
                {
                    nrm = (nrm - tg * Vector3.Dot(nrm, tg)).normalized;
                    if (nrm.sqrMagnitude < 1e-8f) nrm = Vector3.Cross(tg, Vector3.up).normalized;
                }
                Vector3 bin = Vector3.Cross(tg, nrm);
                float r = Mathf.Max(R[i], 1e-3f);
                for (int j = 0; j < ring; j++)
                {
                    float ang = j / (float)Sides * Mathf.PI * 2f;
                    Vector3 dir = nrm * Mathf.Cos(ang) + bin * Mathf.Sin(ang);
                    int k = i * ring + j;
                    _v[k] = P[i] + dir * r;
                    _n[k] = dir;
                    _uv1[k] = new Vector2(K[i], Hu[i]);
                }
            }
            _mesh.vertices = _v;
            _mesh.normals = _n;
            _mesh.uv = _uv0;
            _mesh.uv2 = _uv1;
            _mpb.Clear();
            _mpb.SetFloat("_Gain", gain);
            _r.SetPropertyBlock(_mpb);
            if (_mr != null)
            {
                _mpb.SetFloat("_Gain", gain * MirrorGain);
                _mr.SetPropertyBlock(_mpb);
            }
        }
    }

    /// A fixed set of soft radial glows (the Backglow shader) that face the camera.
    public sealed class GlowPool
    {
        public readonly int Count;
        public float MirrorGain = 0.3f;
        readonly Transform[] _t, _m;
        readonly Renderer[] _r, _mr;
        readonly bool _mirror;
        readonly MaterialPropertyBlock _mpb = new MaterialPropertyBlock();

        public GlowPool(Transform parent, Material glowMat, int n, string name, bool mirror)
        {
            Count = n;
            _mirror = mirror;
            _t = new Transform[n]; _r = new Renderer[n];
            if (mirror) { _m = new Transform[n]; _mr = new Renderer[n]; }
            for (int i = 0; i < n; i++)
            {
                _t[i] = Make(parent, glowMat, name + " " + i, out _r[i]);
                if (mirror) _m[i] = Make(parent, glowMat, name + " reflection " + i, out _mr[i]);
            }
        }

        static Transform Make(Transform parent, Material glowMat, string name, out Renderer r)
        {
            var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Object.Destroy(q.GetComponent<Collider>());
            q.name = name;
            q.transform.SetParent(parent, false);
            var rend = q.GetComponent<Renderer>();
            rend.sharedMaterial = new Material(glowMat);
            rend.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r = rend;
            return q.transform;
        }

        /// Glow i at `pos`, `size` metres across, palette index `hue`, strength
        /// `glow` (<= 0 hides it), facing the camera rotation `face`.
        public void Set(int i, Vector3 pos, float size, float hue, float glow, Quaternion face)
        {
            bool on = glow > 0.002f && size > 0.001f;
            _r[i].enabled = on;
            if (_mirror) _mr[i].enabled = on;
            if (!on) return;
            _t[i].localPosition = pos;
            _t[i].rotation = face;
            _t[i].localScale = new Vector3(size, size, 1f);
            _mpb.Clear();
            _mpb.SetFloat("_Glow", glow);
            _mpb.SetFloat("_Hue", hue);
            _r[i].SetPropertyBlock(_mpb);
            if (_mirror)
            {
                _m[i].localPosition = new Vector3(pos.x, -pos.y, pos.z);
                _m[i].rotation = face;
                _m[i].localScale = new Vector3(size, size, 1f);
                _mpb.SetFloat("_Glow", glow * MirrorGain);
                _mr[i].SetPropertyBlock(_mpb);
            }
        }
    }

    /// Haze layers whose density breathes with a build and a drop.
    public sealed class HazeSet
    {
        readonly Material[] _m;
        readonly float[] _base;

        public HazeSet(Transform parent, Material hazeMat, Vector3[] pos, Vector2[] size, float[] hue, float[] baseDensity)
        {
            _m = Kit.HazeLayers(parent, hazeMat, pos, size, hue);
            _base = baseDensity;
            Apply(1f);
        }

        public void Apply(float mul)
        {
            for (int i = 0; i < _m.Length; i++) _m[i].SetFloat("_Density", _base[i] * mul);
        }
    }

    /// Eases a camera towards a target pose; snaps on enable.
    public sealed class CamRig
    {
        Vector3 _pos, _look;
        bool _set;

        public void Snap() { _set = false; }

        public void Move(Camera cam, Vector3 want, Vector3 look, float dt, float rate = 0.8f)
        {
            if (!_set) { _pos = want; _look = look; _set = true; }
            float k = 1f - Mathf.Exp(-dt * rate);
            _pos = Vector3.Lerp(_pos, want, k);
            _look = Vector3.Lerp(_look, look, k);
            cam.transform.position = _pos;
            cam.transform.LookAt(_look);
        }

        /// A slow orbit round the origin that swings direction with the phrase
        /// and creeps in during a build.
        public void Orbit(Camera cam, Rx rx, float radius, float height, float lookY, float dt, float swing = 1f, float push = 0.2f)
        {
            float sw = Mathf.Sin(rx.beat / 128f * Mathf.PI * 2f) * swing;
            float r = Mathf.Lerp(radius, radius * (1f - push), rx.tension) + Mathf.Sin(rx.phrase) * radius * 0.06f;
            var want = new Vector3(Mathf.Sin(sw) * r, height + Mathf.Sin(rx.beat / 48f * Mathf.PI * 2f) * height * 0.12f, -Mathf.Cos(sw) * r);
            Move(cam, want, new Vector3(0f, lookY + 2f * rx.tension, 0f), dt);
        }
    }

    /// Base for the kit shows: registers uniformly in StageBuilder, owns the
    /// shared signals, camera rig and (optionally) floor + haze.
    public abstract class KitShow : MonoBehaviour
    {
        public Camera cam;
        public Material beamMat, glowMat, groundMat, hazeMat, surfaceMat, tubeMat, orbMat, structMat, membraneMat;
        public Material ribbonMat, deepMat, corridorMat, scopeMat, landMat, horizonMat, robotMat, screenMat, crowdMeshMat;

        protected readonly Rx rx = new Rx();
        protected readonly CamRig rig = new CamRig();
        protected HazeSet haze;
        float _farWas;

        protected abstract void Build();
        protected abstract void Frame(ShowState s, float dt);

        void Awake() { Build(); }

        void OnEnable()
        {
            if (cam != null) { _farWas = cam.farClipPlane; cam.farClipPlane = 900f; }
            rig.Snap();
            rx.Reset();
        }

        void OnDisable()
        {
            if (cam != null && _farWas > 0f) cam.farClipPlane = _farWas;
        }

        void Update()
        {
            float dt = Time.deltaTime;
            rx.Tick(TrippinLink.State, dt);
            Frame(TrippinLink.State, dt);
            if (haze != null) haze.Apply(1f + 0.8f * rx.tension + 0.8f * rx.impact);
        }

        /// Floor (fogged to the palette) plus two haze layers: the common stage.
        protected void Env(float fog = 140f, float hazeBase = 0.07f, bool floor = true)
        {
            if (floor) Kit.Floor(transform, groundMat, fog);
            haze = new HazeSet(transform, hazeMat,
                new[] { new Vector3(0, 10f, 14f), new Vector3(0, 10f, -14f) },
                new[] { new Vector2(120f, 36f), new Vector2(120f, 36f) },
                new[] { 0.15f, 0.4f },
                new[] { hazeBase, hazeBase * 0.7f });
        }
    }
}
