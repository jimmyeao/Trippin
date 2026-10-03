// Light-pillar field: 15x15 vertical columns of light standing on a wet
// floor, a skyline of beams whose heights ripple as a landscape. Only the
// Beam shader, so nothing new to compile. The pillars are abstract lights
// (not architecture), so their heights may change; their positions never do.
//  - Shape: heights are a radial wave out from the centre plus a cross
//    wave; the wave depth follows the slow bass presence.
//  - Motion: both waves travel on the smooth energy clock and reverse on
//    phrase-length sines (the radial wave breathes in and out); the camera
//    orbit swings with the phrase.
//  - Drops (DropDirector): tension sinks the field to a low dim carpet;
//    the drop sends a tall ring front racing outward from the centre
//    (a gaussian that travels at fixed speed, so it is smooth by
//    construction) and flares the field through an eased impact.
// No beat flashes. Count is fixed (225); intensities start low.

using UnityEngine;

namespace TrippinStage
{
    public class PillarsShow : MonoBehaviour
    {
        public Camera cam;
        public Material beamMat, groundMat, hazeMat;

        const int Grid = 15, N = Grid * Grid;
        const float Spacing = 4.2f, FogDist = 130f;

        readonly Transform[] _pil = new Transform[N];
        readonly Renderer[] _pilR = new Renderer[N];
        readonly Vector2[] _pos = new Vector2[N];
        readonly Material[] _hazeM = new Material[2];
        readonly float[] _hazeBase = { 0.07f, 0.05f };
        MaterialPropertyBlock _mpb;
        float _bassSlow, _calmSlow, _impS, _dropAge = 99f, _farWas;
        Vector3 _camPos, _camLook;
        bool _camSet;

        void Awake()
        {
            _mpb = new MaterialPropertyBlock();
            var mesh = new Mesh { name = "beam" };
            mesh.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            mesh.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            mesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            mesh.bounds = new Bounds(Vector3.zero, Vector3.one * 400f); // billboarded in the shader

            var ground = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(ground.GetComponent<Collider>());
            ground.name = "floor";
            ground.transform.SetParent(transform, false);
            ground.transform.localRotation = Quaternion.Euler(90, 0, 0);
            ground.transform.localScale = new Vector3(1000, 1000, 1);
            var gm = new Material(groundMat);
            gm.SetFloat("_FogDist", FogDist);
            ground.GetComponent<Renderer>().sharedMaterial = gm;

            for (int i = 0; i < N; i++)
            {
                int gx = i % Grid, gz = i / Grid;
                _pos[i] = new Vector2((gx - (Grid - 1) / 2f) * Spacing, (gz - (Grid - 1) / 2f) * Spacing);
                var g = new GameObject("pillar " + i);
                g.transform.SetParent(transform, false);
                g.transform.localPosition = new Vector3(_pos[i].x, 0f, _pos[i].y);
                g.AddComponent<MeshFilter>().sharedMesh = mesh;
                var mr = g.AddComponent<MeshRenderer>();
                var m = new Material(beamMat);
                m.SetFloat("_Width", 0.10f);
                m.SetFloat("_Spread", 0f);
                m.SetFloat("_Core", 14f);
                m.SetFloat("_Smoke", 0.4f);
                m.SetFloat("_Fade", 0.75f);
                m.SetFloat("_Hot", 0.15f);
                mr.sharedMaterial = m;
                mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
                g.transform.localRotation = Quaternion.identity; // beams run along local +Y: straight up
                g.transform.localScale = new Vector3(1f, 2f, 1f);
                _pil[i] = g.transform;
                _pilR[i] = mr;
            }

            float[] z = { 10f, -16f };
            for (int i = 0; i < 2; i++)
            {
                var h = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Destroy(h.GetComponent<Collider>());
                h.name = "haze " + i;
                h.transform.SetParent(transform, false);
                h.transform.localPosition = new Vector3(0, 9f, z[i]);
                h.transform.localScale = new Vector3(120f, 30f, 1f);
                _hazeM[i] = new Material(hazeMat);
                _hazeM[i].SetFloat("_Density", _hazeBase[i]);
                _hazeM[i].SetFloat("_HueOff", 0.3f + i * 0.2f);
                h.GetComponent<Renderer>().sharedMaterial = _hazeM[i];
                h.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
        }

        void OnEnable()
        {
            if (cam != null) { _farWas = cam.farClipPlane; cam.farClipPlane = 800f; }
            _camSet = false;
            _dropAge = 99f;
            _impS = 0f;
        }

        void OnDisable()
        {
            if (cam != null && _farWas > 0f) cam.farClipPlane = _farWas;
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Time.deltaTime;
            float beat = s.beat;
            DropDirector.Tick(s, dt);
            float clk = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            float pres0 = s.pres4 != null && s.pres4.Length > 0 ? s.pres4[0] : 0.4f;
            float k1 = 1f - Mathf.Exp(-dt * 1.0f);
            _bassSlow += (pres0 - _bassSlow) * k1;
            _calmSlow += (s.calm - _calmSlow) * k1;

            float tn = Mathf.SmoothStep(0f, 1f, DropDirector.Tension);
            float impTarget = DropDirector.Impact;
            _impS = impTarget > _impS ? _impS + (impTarget - _impS) * (1f - Mathf.Exp(-12f * dt)) : impTarget;
            if (DropDirector.Dropped) _dropAge = 0f;
            _dropAge += dt;
            float front = _dropAge * 36f - 10f; // the ring front, metres from the centre (starts off-centre so the middle pillar grows in)
            float phrase = beat / 64f * Mathf.PI * 2f;

            float level = Mathf.Lerp(0.75f, 1f, _calmSlow) * (1f - 0.4f * tn) * (1f + 0.7f * _impS);
            float waveDepth = 3f + 11f * Mathf.Clamp01(_bassSlow);
            float p1 = clk * 0.09f + 1.6f * Mathf.Sin(phrase);       // radial wave, breathes in and out
            float p2 = clk * 0.06f - 1.2f * Mathf.Sin(phrase * 0.5f); // cross wave, opposite swing
            float sink = 1f - 0.75f * tn;                             // a build sinks the field to a carpet
            float hue = 0.62f + 0.05f * Mathf.Sin(phrase * 0.5f);

            for (int i = 0; i < N; i++)
            {
                var p = _pos[i];
                float d = p.magnitude;
                float radial = 0.5f + 0.5f * Mathf.Sin(d * 0.34f - p1 * 2f);
                float cross = 0.5f + 0.5f * Mathf.Sin(p.x * 0.19f + p.y * 0.13f + p2 * 2f);
                float h = 1.5f + waveDepth * (0.65f * radial + 0.35f * cross);
                h *= sink;
                float fd = (d - front) / 4f;
                h += 16f * Mathf.Exp(-fd * fd) * Mathf.Clamp01(1f - _dropAge / 3f); // drop front: travels, then fades out
                _pil[i].localScale = new Vector3(1f, Mathf.Max(h, 0.3f), 1f);

                Color c = TrippinLink.Palette(hue + d * 0.012f + radial * 0.08f);
                float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b), 1e-3f);
                c = new Color(c.r / mx, c.g / mx, c.b / mx);
                _mpb.Clear();
                _mpb.SetColor("_Color", c);
                // Fade pillars the orbiting camera passes close to: up close one filled
                // the lens as a wide blurred slab (M2 render).
                var cp = cam.transform.position;
                var wp = _pil[i].position;
                float near = Mathf.SmoothStep(0f, 1f, Mathf.InverseLerp(5f, 14f, new Vector2(wp.x - cp.x, wp.z - cp.z).magnitude));
                _mpb.SetFloat("_Intensity", 0.5f * level * near * (0.6f + 0.6f * s.intensity));
                _pilR[i].SetPropertyBlock(_mpb);
            }

            for (int i = 0; i < _hazeM.Length; i++)
                _hazeM[i].SetFloat("_Density", _hazeBase[i] * (1f + 0.8f * tn + 0.8f * _impS));

            UpdateCamera(beat, dt, tn);
        }

        // Orbits the field low and wide, swinging with the phrase, creeping in
        // during a build.
        void UpdateCamera(float beat, float dt, float tn)
        {
            float swing = Mathf.Sin(beat / 128f * Mathf.PI * 2f) * 0.9f;
            float r = Mathf.Lerp(46f, 38f, tn) + Mathf.Sin(beat / 64f * Mathf.PI * 2f) * 3f;
            var want = new Vector3(Mathf.Sin(swing) * r, 6.5f + Mathf.Sin(beat / 48f * Mathf.PI * 2f) * 1.2f, -Mathf.Cos(swing) * r);
            var look = new Vector3(0f, 5f, 0f);
            if (!_camSet) { _camPos = want; _camLook = look; _camSet = true; }
            float k = 1f - Mathf.Exp(-dt * 0.8f);
            _camPos = Vector3.Lerp(_camPos, want, k);
            _camLook = Vector3.Lerp(_camLook, look, k);
            cam.transform.position = _camPos;
            cam.transform.LookAt(_camLook);
        }
    }
}
