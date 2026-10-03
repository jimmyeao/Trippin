// Laser-cage show: 96 thin laser segments strung between emitters on a
// ceiling truss and the floor, so the beams draw GEOMETRY - a twisting
// hyperboloid, a tent, a bending curtain, a double cone, a rippling forest,
// a braid - instead of fanning out from a rig. A wireframe made of light,
// reflected in a wet floor. Segment count never changes.
//  - Shape: the formation changes every 4 bars, morphing in over a beat by
//    lerping endpoints. Inside a formation the twist/waist follows the slow
//    bass presence (the hourglass pinches and relaxes), ripple amplitude too.
//  - Motion: the cage turns on the smooth energy clock plus a phrase-length
//    swing that reverses direction; the camera swings with the phrase.
//  - Drops (DropDirector): tension draws the whole cage into a tight dim
//    spindle; the drop blows it open and flares it.
// No beat flashes: brightness rides slow presence and the drop impact.
// Intensities follow the M2-tuned lightstorm numbers (additive beams blow
// out fast: _Hot low so cores stay saturated).

using UnityEngine;

namespace TrippinStage
{
    public class PrismShow : MonoBehaviour
    {
        public Camera cam;
        public Material beamMat, groundMat, hazeMat;

        const int N = 96;
        const int Formations = 6;
        const float FogDist = 140f;
        const float H = 17f, Floor = 0.4f;

        readonly Transform[] _seg = new Transform[N], _mir = new Transform[N];
        readonly Renderer[] _segR = new Renderer[N], _mirR = new Renderer[N];
        readonly Vector3[] _fromA = new Vector3[N], _fromB = new Vector3[N];
        readonly Vector3[] _curA = new Vector3[N], _curB = new Vector3[N];
        readonly Material[] _hazeM = new Material[3];
        readonly float[] _hazeBase = { 0.10f, 0.07f, 0.05f };
        MaterialPropertyBlock _mpb;
        int _formation = -1;
        float _formStart;
        float _bassSlow, _calmSlow, _farWas;
        float _bassFast, _kick, _midEase; // eased fast vocabulary: shape reacts within ~70 ms, never steps
        Vector3 _camPos, _camLook;
        bool _camSet, _snap = true;

        void Awake()
        {
            _mpb = new MaterialPropertyBlock();
            var mesh = new Mesh { name = "beam" };
            mesh.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            mesh.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            mesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 500f); // billboarded in the shader

            var ground = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(ground.GetComponent<Collider>());
            ground.name = "floor";
            ground.transform.SetParent(transform, false);
            ground.transform.localRotation = Quaternion.Euler(90, 0, 0);
            ground.transform.localScale = new Vector3(900, 900, 1);
            var gm = new Material(groundMat);
            gm.SetFloat("_FogDist", FogDist);
            ground.GetComponent<Renderer>().sharedMaterial = gm;

            for (int i = 0; i < N; i++)
            {
                _seg[i] = MakeBeam(mesh, "seg " + i, out _segR[i]);
                _mir[i] = MakeBeam(mesh, "reflection " + i, out _mirR[i]);
            }

            float[] z = { 16f, 2f, -14f };
            for (int i = 0; i < 3; i++)
            {
                var h = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Destroy(h.GetComponent<Collider>());
                h.name = "haze " + i;
                h.transform.SetParent(transform, false);
                h.transform.localPosition = new Vector3(0, 9f, z[i]);
                h.transform.localScale = new Vector3(110f, 30f, 1f);
                _hazeM[i] = new Material(hazeMat);
                _hazeM[i].SetFloat("_Density", _hazeBase[i]);
                _hazeM[i].SetFloat("_HueOff", i * 0.17f);
                h.GetComponent<Renderer>().sharedMaterial = _hazeM[i];
                h.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
        }

        Transform MakeBeam(Mesh mesh, string name, out Renderer r)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            var m = new Material(beamMat);
            m.SetFloat("_Width", 0.035f);
            m.SetFloat("_Spread", 0f);
            m.SetFloat("_Core", 40f);
            m.SetFloat("_Smoke", 0.6f);
            m.SetFloat("_Fade", 0.98f);
            m.SetFloat("_Hot", 0.2f);
            mr.sharedMaterial = m;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r = mr;
            return g.transform;
        }

        void OnEnable()
        {
            if (cam != null) { _farWas = cam.farClipPlane; cam.farClipPlane = 800f; }
            _camSet = false;
            _snap = true; // start in the current formation, not morphing from a stale pose
        }

        void OnDisable()
        {
            if (cam != null && _farWas > 0f) cam.farClipPlane = _farWas;
        }

        static Vector3 Ring(float r, float ang, float y) => new Vector3(Mathf.Cos(ang) * r, y, Mathf.Sin(ang) * r);

        // Endpoints of segment k in formation f (unrotated, unscaled-by-drop).
        // `tw` is the bass-driven twist, `pinch` a 0..1 bass amount, `clk` the
        // smooth energy clock (beats), `ph` a phrase-length sine.
        void Ends(int k, int f, float tw, float pinch, float clk, float ph, out Vector3 a, out Vector3 b)
        {
            float u = k / (float)N;
            float th = u * Mathf.PI * 2f;
            const float R = 15f;
            switch (f)
            {
                case 0: // hyperboloid: the twist makes the waist
                    a = Ring(R, th, H);
                    b = Ring(R, th + tw, Floor);
                    break;
                case 1: // tent: everything funnels to a small ring on the floor
                    a = Ring(R, th, H);
                    b = Ring(2f + 3f * pinch, th + tw * 0.5f, Floor);
                    break;
                case 2: // curtain: parallel beams that bend and ripple
                {
                    float x = (u * 2f - 1f) * 18f;
                    a = new Vector3(x, H, -12f);
                    b = new Vector3(x + ph * 10f + Mathf.Sin(clk * 0.2f + u * 8f) * (2f + 3f * pinch), Floor, 12f);
                    break;
                }
                case 3: // double cone: opposite points, so the waist pinches shut
                    a = Ring(14f, th, H);
                    b = Ring(14f, th + Mathf.PI + 0.35f * Mathf.Sin(clk * 0.1f), Floor);
                    break;
                case 4: // forest: a grid of beams swaying in waves
                {
                    int gx = k % 12, gz = k / 12;
                    float x = (gx / 11f * 2f - 1f) * 18f, z = (gz / 7f * 2f - 1f) * 12f;
                    float amp = 5f * (0.5f + pinch);
                    a = new Vector3(x, H, z);
                    b = new Vector3(x + Mathf.Sin(clk * 0.2f + z * 0.35f) * amp, Floor, z + Mathf.Cos(clk * 0.2f + x * 0.3f) * amp);
                    break;
                }
                default: // braid: the twist grows round the ring
                    a = Ring(R, th, H);
                    b = Ring(R * 0.9f, th + Mathf.PI * 2f * 3f * u + clk * 0.05f, Floor);
                    break;
            }
        }

        static void Place(Transform t, Vector3 a, Vector3 b)
        {
            var d = b - a;
            float len = d.magnitude;
            t.localPosition = a;
            t.localRotation = Quaternion.FromToRotation(Vector3.up, len > 1e-4f ? d / len : Vector3.up);
            t.localScale = new Vector3(1f, Mathf.Max(len, 1e-3f), 1f);
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Time.deltaTime;
            float beat = s.beat;
            DropDirector.Tick(s, dt);
            int bar = Mathf.FloorToInt(beat / 4f);
            int f = (bar / 4) % Formations;
            float clk = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            float ph = Mathf.Sin(beat / 32f * Mathf.PI * 2f);
            float pres0 = s.pres4 != null && s.pres4.Length > 0 ? s.pres4[0] : 0.4f;
            float k1 = 1f - Mathf.Exp(-dt * 1.2f);
            _bassSlow += (pres0 - _bassSlow) * k1;
            _calmSlow += (s.calm - _calmSlow) * k1;

            float tn = Mathf.SmoothStep(0f, 1f, DropDirector.Tension);
            float imp = DropDirector.Impact;
            _bassFast = Eased.Follow(_bassFast, Eased.Lvl(s, 0), 14f, 3f, dt);
            _kick = Eased.Follow(_kick, Eased.Hit(s, 0), 20f, 4f, dt);
            _midEase = Eased.Follow(_midEase, Eased.Lvl(s, 1), 4f, 4f, dt);
            float pinch = Mathf.Clamp01(_bassSlow + 0.4f * _midEase);
            // Twist breathes with the slow bass; the drop unwinds a little extra.
            float tw = 1.1f + 0.9f * pinch + 0.5f * _bassFast + 0.35f * ph + 0.8f * imp;
            // 0.35 rather than 0.5: with the camera creeping in, a 0.5 shrink read as ~0.72
            // on screen and the build's contraction was barely visible (M2 render).
            float scale = (1f - 0.65f * tn) * (1f + 0.6f * imp) * (1f + 0.18f * _kick); // the cage breathes out on each kick

            if (_snap)
            {
                for (int i = 0; i < N; i++) Ends(i, f, tw, pinch, clk, ph, out _curA[i], out _curB[i]);
                for (int i = 0; i < N; i++) { _fromA[i] = _curA[i]; _fromB[i] = _curB[i]; }
                _formation = f;
                _formStart = beat - 1f; // morph already done
                _snap = false;
            }
            else if (f != _formation)
            {
                for (int i = 0; i < N; i++) { _fromA[i] = _curA[i]; _fromB[i] = _curB[i]; }
                _formation = f;
                _formStart = beat;
            }
            float m = Mathf.SmoothStep(0, 1, Mathf.Clamp01(beat - _formStart));

            // Cage turn: smooth energy clock plus a phrase swing that reverses.
            float yaw = (clk * 0.012f + 1.1f * Mathf.Sin(beat / 64f * Mathf.PI * 2f)) * Mathf.Rad2Deg;
            var rot = Quaternion.Euler(0f, yaw, 0f);

            float calmDim = Mathf.Lerp(1f, 0.3f, _calmSlow);
            float drama = (1f - 0.5f * tn) * (1f + 1.6f * imp);
            float hueBase = 0.1f * f + 0.03f * Mathf.Sin(beat / 64f * Mathf.PI * 2f);
            for (int i = 0; i < N; i++)
            {
                Ends(i, f, tw, pinch, clk, ph, out var ta, out var tb);
                _curA[i] = Vector3.Lerp(_fromA[i], ta, m);
                _curB[i] = Vector3.Lerp(_fromB[i], tb, m);
                var a = rot * new Vector3(_curA[i].x * scale, _curA[i].y, _curA[i].z * scale);
                var b = rot * new Vector3(_curB[i].x * scale, _curB[i].y, _curB[i].z * scale);
                Place(_seg[i], a, b);
                Place(_mir[i], new Vector3(a.x, -a.y, a.z), new Vector3(b.x, -b.y, b.z));

                Color c = TrippinLink.Palette(hueBase + i / (float)N * 0.55f);
                float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b), 1e-3f);
                c = new Color(c.r / mx, c.g / mx, c.b / mx); // saturated
                float inten = calmDim * drama * (0.7f + 0.4f * _bassSlow) * (0.6f + 0.6f * s.intensity);
                _mpb.Clear();
                _mpb.SetColor("_Color", c);
                _mpb.SetFloat("_Intensity", inten);
                _segR[i].SetPropertyBlock(_mpb);
                _mpb.SetFloat("_Intensity", inten * 0.3f);
                _mirR[i].SetPropertyBlock(_mpb);
            }

            for (int i = 0; i < _hazeM.Length; i++)
                _hazeM[i].SetFloat("_Density", _hazeBase[i] * (1f + 0.8f * tn + 0.8f * imp));

            UpdateCamera(beat, dt, tn);
        }

        // Outside the cage, low, swinging round with the phrase; creeps in on
        // a build and springs back after the drop.
        void UpdateCamera(float beat, float dt, float tn)
        {
            float swing = Mathf.Sin(beat / 128f * Mathf.PI * 2f) * 1.0f;
            float r = Mathf.Lerp(36f, 31f, tn) + Mathf.Sin(beat / 64f * Mathf.PI * 2f) * 3f; // gentle creep, so the cage's contraction reads
            var want = new Vector3(Mathf.Sin(swing) * r, 4.5f + Mathf.Sin(beat / 48f * Mathf.PI * 2f) * 1.2f, -Mathf.Cos(swing) * r);
            var look = new Vector3(0f, 8f + 2f * tn, 0f);
            if (!_camSet) { _camPos = want; _camLook = look; _camSet = true; }
            float k = 1f - Mathf.Exp(-dt * 0.8f);
            _camPos = Vector3.Lerp(_camPos, want, k);
            _camLook = Vector3.Lerp(_camLook, look, k);
            cam.transform.position = _camPos;
            cam.transform.LookAt(_camLook);
        }
    }
}
