// Bass-drop show: a big circular wire membrane (a sub-bass plate) floating
// just over a reflective floor, with a ring of light shafts rising round it.
// The plate is an abstract display: it is allowed to change shape.
//  - Shape: the plate's ripple depth follows the slow bass presence; each
//    kick launches a ring pulse that travels out across the plate (fixed
//    speed, eased in over ~0.15 s so there is no pop); the shaft heights
//    follow the slow bass and a travelling wave round the ring (count fixed).
//  - Motion: the ripple travels on the smooth energy clock and reverses
//    direction on a phrase-length sine; the camera swings with the phrase.
//  - Drops (DropDirector): tension gathers the plate (calmer, dimmer, the
//    shafts creep up); the drop sends three big staggered rings across the
//    plate and flares the shafts, through an eased impact (0.25 s attack).
// No beat flashes: pulses are shape travelling over geometry; brightness
// rides slow presence. Intensities start low (additive lines wash out).

using UnityEngine;

namespace TrippinStage
{
    public class BasscoreShow : MonoBehaviour
    {
        public Camera cam;
        public Material membraneMat, beamMat, groundMat, hazeMat;

        const int Rings = 72, Segs = 192, Shafts = 24, Slots = 4;
        const float Radius = 16f, PlateY = 2.6f, FogDist = 150f;

        Material _plateM, _mirM;
        readonly Transform[] _shaft = new Transform[Shafts];
        readonly Renderer[] _shaftR = new Renderer[Shafts];
        readonly Material[] _hazeM = new Material[2];
        readonly float[] _hazeBase = { 0.08f, 0.05f };
        MaterialPropertyBlock _mpb;

        // Ring pulses: radius 0..1, age in s (negative = waiting), strength multiplier.
        readonly float[] _r = new float[Slots], _age = new float[Slots], _str = new float[Slots];
        readonly bool[] _live = new bool[Slots];
        float _lastKick = -10f;

        float _bassSlow, _calmSlow, _impS, _farWas;
        Vector3 _camPos, _camLook;
        bool _camSet;

        void Awake()
        {
            _mpb = new MaterialPropertyBlock();

            // Polar grid: uv.x = angle, uv.y = radius, displaced in the shader.
            var verts = new Vector3[(Rings + 1) * (Segs + 1)];
            var uvs = new Vector2[verts.Length];
            var tris = new int[Rings * Segs * 6];
            for (int y = 0; y <= Rings; y++)
                for (int x = 0; x <= Segs; x++)
                {
                    int k = y * (Segs + 1) + x;
                    uvs[k] = new Vector2(x / (float)Segs, y / (float)Rings);
                    verts[k] = new Vector3(uvs[k].x, uvs[k].y, 0f);
                }
            int t = 0;
            for (int y = 0; y < Rings; y++)
                for (int x = 0; x < Segs; x++)
                {
                    int a = y * (Segs + 1) + x, b = a + 1, c = a + Segs + 1, d = c + 1;
                    tris[t++] = a; tris[t++] = c; tris[t++] = b;
                    tris[t++] = b; tris[t++] = c; tris[t++] = d;
                }
            var disc = new Mesh { name = "plate", indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            disc.vertices = verts;
            disc.uv = uvs;
            disc.triangles = tris;
            disc.bounds = new Bounds(Vector3.zero, Vector3.one * 200f); // displaced in the shader

            _plateM = new Material(membraneMat);
            _plateM.SetFloat("_Radius", Radius);
            _mirM = new Material(membraneMat);
            _mirM.SetFloat("_Radius", Radius);
            MakePart(disc, "plate", _plateM, new Vector3(0, PlateY, 0), Vector3.one);
            MakePart(disc, "reflection", _mirM, new Vector3(0, -PlateY, 0), new Vector3(1, -1, 1));

            var ground = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(ground.GetComponent<Collider>());
            ground.name = "floor";
            ground.transform.SetParent(transform, false);
            ground.transform.localRotation = Quaternion.Euler(90, 0, 0);
            ground.transform.localScale = new Vector3(1000, 1000, 1);
            var gm = new Material(groundMat);
            gm.SetFloat("_FogDist", FogDist);
            ground.GetComponent<Renderer>().sharedMaterial = gm;

            var beamMesh = new Mesh { name = "beam" };
            beamMesh.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            beamMesh.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            beamMesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            beamMesh.bounds = new Bounds(Vector3.zero, Vector3.one * 400f);
            for (int i = 0; i < Shafts; i++)
            {
                float a = i / (float)Shafts * Mathf.PI * 2f;
                var g = new GameObject("shaft " + i);
                g.transform.SetParent(transform, false);
                g.transform.localPosition = new Vector3(Mathf.Cos(a) * (Radius + 2.5f), PlateY, Mathf.Sin(a) * (Radius + 2.5f));
                g.AddComponent<MeshFilter>().sharedMesh = beamMesh;
                var mr = g.AddComponent<MeshRenderer>();
                var m = new Material(beamMat);
                m.SetFloat("_Width", 0.14f);
                m.SetFloat("_Spread", 0.008f);
                m.SetFloat("_Core", 14f);
                m.SetFloat("_Smoke", 0.6f);
                m.SetFloat("_Fade", 0.45f);
                m.SetFloat("_Hot", 0.2f);
                mr.sharedMaterial = m;
                mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
                g.transform.localRotation = Quaternion.identity; // beams run along local +Y: straight up
                _shaft[i] = g.transform;
                _shaftR[i] = mr;
            }

            float[] z = { 12f, -14f };
            for (int i = 0; i < 2; i++)
            {
                var h = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Destroy(h.GetComponent<Collider>());
                h.name = "haze " + i;
                h.transform.SetParent(transform, false);
                h.transform.localPosition = new Vector3(0, 8f, z[i]);
                h.transform.localScale = new Vector3(110f, 28f, 1f);
                _hazeM[i] = new Material(hazeMat);
                _hazeM[i].SetFloat("_Density", _hazeBase[i]);
                _hazeM[i].SetFloat("_HueOff", 0.05f + i * 0.2f);
                h.GetComponent<Renderer>().sharedMaterial = _hazeM[i];
                h.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
        }

        void MakePart(Mesh mesh, string name, Material m, Vector3 pos, Vector3 scale)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.transform.localPosition = pos;
            g.transform.localScale = scale;
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            mr.sharedMaterial = m;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
        }

        void OnEnable()
        {
            if (cam != null) { _farWas = cam.farClipPlane; cam.farClipPlane = 800f; }
            _camSet = false;
            for (int i = 0; i < Slots; i++) _live[i] = false;
            _impS = 0f;
        }

        void OnDisable()
        {
            if (cam != null && _farWas > 0f) cam.farClipPlane = _farWas;
        }

        void Launch(float strength, float delay)
        {
            for (int i = 0; i < Slots; i++)
                if (!_live[i])
                {
                    _live[i] = true;
                    _r[i] = 0.02f;
                    _age[i] = -delay;
                    _str[i] = strength;
                    return;
                }
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Time.deltaTime;
            float beat = s.beat;
            DropDirector.Tick(s, dt);
            float clk = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            float pres0 = s.pres4 != null && s.pres4.Length > 0 ? s.pres4[0] : 0.4f;
            float hit0 = s.hits4 != null && s.hits4.Length > 0 ? s.hits4[0] : 0f;
            float k1 = 1f - Mathf.Exp(-dt * 1.2f);
            _bassSlow += (pres0 - _bassSlow) * k1;
            _calmSlow += (s.calm - _calmSlow) * k1;

            float tn = Mathf.SmoothStep(0f, 1f, DropDirector.Tension);
            // Eased impact (the raw value steps in one frame and would pop).
            float impTarget = DropDirector.Impact;
            _impS = impTarget > _impS ? _impS + (impTarget - _impS) * (1f - Mathf.Exp(-12f * dt)) : impTarget;
            float phrase = beat / 64f * Mathf.PI * 2f;

            // Kicks launch ring pulses (shape travelling over the plate).
            if (hit0 > 0.55f && Time.time - _lastKick > 0.35f && s.drums)
            {
                _lastKick = Time.time;
                Launch(0.8f + 0.5f * Mathf.Clamp01(_bassSlow), 0f);
            }
            if (DropDirector.Dropped)
            {
                Launch(2.2f, 0f);
                Launch(2.0f, 0.15f);
                Launch(1.8f, 0.3f);
            }
            var rr = new Vector4(-1f, -1f, -1f, -1f);
            var ra = Vector4.zero;
            float speed = 0.22f + 0.1f * Mathf.Clamp01(s.intensity);
            for (int i = 0; i < Slots; i++)
            {
                if (!_live[i]) continue;
                _age[i] += dt;
                if (_age[i] > 0f) _r[i] += speed * dt;
                if (_r[i] > 1.05f) { _live[i] = false; continue; }
                float env = Mathf.SmoothStep(0f, 1f, Mathf.Clamp01(_age[i] / 0.15f)) * Mathf.Pow(Mathf.Clamp01(1f - _r[i]), 1.5f);
                rr[i] = _r[i];
                ra[i] = 1.8f * _str[i] * env;
            }

            float level = Mathf.Lerp(0.8f, 1f, _calmSlow) * (1f - 0.35f * tn) * (1f + 0.8f * _impS);
            float amp = (0.5f + 1.9f * Mathf.Clamp01(_bassSlow)) * (1f - 0.4f * tn);
            float phase = clk * 0.12f + 1.5f * Mathf.Sin(phrase); // direction reverses with the phrase
            float hue = 0.02f + 0.05f * Mathf.Sin(phrase * 0.5f);
            _mpb.Clear();
            ApplyPlate(_plateM, amp, phase, rr, ra, hue, 0.45f * level);
            ApplyPlate(_mirM, amp, phase, rr, ra, hue, 0.45f * level * 0.3f);

            // Shafts: heights ride slow bass and a wave round the ring; fixed count.
            for (int i = 0; i < Shafts; i++)
            {
                float a = i / (float)Shafts * Mathf.PI * 2f;
                float wave = 0.6f + 0.4f * Mathf.Sin(a * 3f - clk * 0.1f);
                float h = 6f + (22f * Mathf.Clamp01(_bassSlow) + 14f * tn + 18f * _impS) * wave;
                _shaft[i].localScale = new Vector3(1f, h, 1f);
                Color c = TrippinLink.Palette(hue + 0.1f + i / (float)Shafts * 0.4f);
                float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b), 1e-3f);
                c = new Color(c.r / mx, c.g / mx, c.b / mx);
                _mpb.Clear();
                _mpb.SetColor("_Color", c);
                _mpb.SetFloat("_Intensity", 0.5f * level * (0.6f + 0.6f * s.intensity));
                _shaftR[i].SetPropertyBlock(_mpb);
            }

            for (int i = 0; i < _hazeM.Length; i++)
                _hazeM[i].SetFloat("_Density", _hazeBase[i] * (1f + 0.8f * tn + 0.8f * _impS));

            UpdateCamera(beat, dt, tn);
        }

        static void ApplyPlate(Material m, float amp, float phase, Vector4 rings, Vector4 ringAmp, float hue, float inten)
        {
            m.SetFloat("_Amp", amp);
            m.SetFloat("_Phase", phase);
            m.SetVector("_Rings", rings);
            m.SetVector("_RingAmp", ringAmp);
            m.SetFloat("_Hue", hue);
            m.SetFloat("_Intensity", inten);
        }

        // Raised, looking down across the plate; orbit swings with the phrase,
        // creeping in during a build.
        void UpdateCamera(float beat, float dt, float tn)
        {
            float swing = Mathf.Sin(beat / 128f * Mathf.PI * 2f) * 1.0f;
            float r = Mathf.Lerp(34f, 28f, tn) + Mathf.Sin(beat / 64f * Mathf.PI * 2f) * 3f;
            var want = new Vector3(Mathf.Sin(swing) * r, 7.5f + Mathf.Sin(beat / 48f * Mathf.PI * 2f) * 1.2f, -Mathf.Cos(swing) * r);
            var look = new Vector3(0f, 6f + 2f * tn, 0f);
            if (!_camSet) { _camPos = want; _camLook = look; _camSet = true; }
            float k = 1f - Mathf.Exp(-dt * 0.8f);
            _camPos = Vector3.Lerp(_camPos, want, k);
            _camLook = Vector3.Lerp(_camLook, look, k);
            cam.transform.position = _camPos;
            cam.transform.LookAt(_camLook);
        }
    }
}
