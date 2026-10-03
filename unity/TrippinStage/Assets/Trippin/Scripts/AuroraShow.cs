// Calm-sweep show, the one made for breakdowns: five slow aurora curtains
// folding over a still reflective floor, with six searchlights sweeping the
// sky from the horizon. Reflections are mirrored copies under the floor.
//  - Shape: the curtains' fold amount follows the slow bass presence (they
//    ripple deeper as the low end swells) and their height follows the mids;
//    count never changes.
//  - Motion: the folds travel on the smooth energy clock; each curtain also
//    swings its travel direction on a phrase-length sine, and the
//    searchlights sweep back and forth, reversing with the phrase.
//  - Calm: brighter and slower in a breakdown (this is its show), subdued
//    but alive when the drums play.
//  - Drops (DropDirector): tension gathers the curtains (folds flatten,
//    they rise a little, the rig dims); the drop flares them and fans the
//    searchlights open.
// No beat flashes: everything rides slow presence, the clocks and the drop.
// Brightness numbers start low on purpose: additive layers wash out fast.

using UnityEngine;

namespace TrippinStage
{
    public class AuroraShow : MonoBehaviour
    {
        public Camera cam;
        public Material auroraMat, beamMat, groundMat, hazeMat;

        const int Curtains = 5, Lights = 6, GridU = 160, GridV = 24;
        const float FogDist = 160f;

        readonly Renderer[] _cur = new Renderer[Curtains], _curMir = new Renderer[Curtains];
        readonly Transform[] _light = new Transform[Lights], _lightMir = new Transform[Lights];
        readonly Renderer[] _lightR = new Renderer[Lights], _lightMirR = new Renderer[Lights];
        readonly Vector3[] _lightPos = new Vector3[Lights];
        readonly Material[] _hazeM = new Material[2];
        readonly float[] _hazeBase = { 0.08f, 0.05f };
        MaterialPropertyBlock _mpb;
        float _bassSlow, _midSlow, _calmSlow, _farWas;
        float _impS; // DropDirector.Impact with a short attack (see Update)
        Vector3 _camPos, _camLook;
        bool _camSet;

        void Awake()
        {
            _mpb = new MaterialPropertyBlock();

            // One ribbon grid shared by every curtain; the vertex shader folds it.
            var verts = new Vector3[(GridU + 1) * (GridV + 1)];
            var uvs = new Vector2[verts.Length];
            var tris = new int[GridU * GridV * 6];
            for (int y = 0; y <= GridV; y++)
                for (int x = 0; x <= GridU; x++)
                {
                    int k = y * (GridU + 1) + x;
                    uvs[k] = new Vector2(x / (float)GridU, y / (float)GridV);
                    verts[k] = new Vector3(uvs[k].x, uvs[k].y, 0f);
                }
            int t = 0;
            for (int y = 0; y < GridV; y++)
                for (int x = 0; x < GridU; x++)
                {
                    int a = y * (GridU + 1) + x, b = a + 1, c = a + GridU + 1, d = c + 1;
                    tris[t++] = a; tris[t++] = c; tris[t++] = b;
                    tris[t++] = b; tris[t++] = c; tris[t++] = d;
                }
            var ribbon = new Mesh { name = "ribbon", indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            ribbon.vertices = verts;
            ribbon.uv = uvs;
            ribbon.triangles = tris;
            ribbon.bounds = new Bounds(Vector3.zero, Vector3.one * 600f); // displaced in the shader

            var ground = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(ground.GetComponent<Collider>());
            ground.name = "floor";
            ground.transform.SetParent(transform, false);
            ground.transform.localRotation = Quaternion.Euler(90, 0, 0);
            ground.transform.localScale = new Vector3(1200, 1200, 1);
            var gm = new Material(groundMat);
            gm.SetFloat("_FogDist", FogDist);
            ground.GetComponent<Renderer>().sharedMaterial = gm;

            for (int i = 0; i < Curtains; i++)
            {
                _cur[i] = MakeRibbon(ribbon, "curtain " + i, 0f);
                _curMir[i] = MakeRibbon(ribbon, "reflection " + i, 1f);
            }

            var beamMesh = new Mesh { name = "beam" };
            beamMesh.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            beamMesh.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            beamMesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            beamMesh.bounds = new Bounds(Vector3.zero, Vector3.one * 500f);
            for (int i = 0; i < Lights; i++)
            {
                float a = (i + 0.5f) / Lights * Mathf.PI * 2f + 0.4f;
                _lightPos[i] = new Vector3(Mathf.Cos(a) * 48f, 0.4f, Mathf.Sin(a) * 36f + 10f);
                _light[i] = MakeBeam(beamMesh, "searchlight " + i, out _lightR[i]);
                _lightMir[i] = MakeBeam(beamMesh, "searchlight reflection " + i, out _lightMirR[i]);
            }

            float[] z = { 6f, -22f };
            for (int i = 0; i < 2; i++)
            {
                var h = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Destroy(h.GetComponent<Collider>());
                h.name = "haze " + i;
                h.transform.SetParent(transform, false);
                h.transform.localPosition = new Vector3(0, 10f, z[i]);
                h.transform.localScale = new Vector3(140f, 34f, 1f);
                _hazeM[i] = new Material(hazeMat);
                _hazeM[i].SetFloat("_Density", _hazeBase[i]);
                _hazeM[i].SetFloat("_HueOff", 0.5f + i * 0.15f);
                h.GetComponent<Renderer>().sharedMaterial = _hazeM[i];
                h.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
        }

        Renderer MakeRibbon(Mesh mesh, string name, float mirror)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            var m = new Material(auroraMat);
            m.SetFloat("_Mirror", mirror);
            mr.sharedMaterial = m;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            return mr;
        }

        Transform MakeBeam(Mesh mesh, string name, out Renderer r)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = g.AddComponent<MeshRenderer>();
            var m = new Material(beamMat);
            m.SetFloat("_Width", 0.05f);
            m.SetFloat("_Spread", 0.004f);
            m.SetFloat("_Core", 25f);
            m.SetFloat("_Smoke", 0.7f);
            m.SetFloat("_Fade", 0.55f);
            m.SetFloat("_Hot", 0.2f);
            mr.sharedMaterial = m;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r = mr;
            return g.transform;
        }

        void OnEnable()
        {
            if (cam != null) { _farWas = cam.farClipPlane; cam.farClipPlane = 900f; }
            _camSet = false;
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
            float pres1 = s.pres4 != null && s.pres4.Length > 1 ? s.pres4[1] : 0.4f;
            float k1 = 1f - Mathf.Exp(-dt * 0.9f);
            _bassSlow += (pres0 - _bassSlow) * k1;
            _midSlow += (pres1 - _midSlow) * k1;
            _calmSlow += (s.calm - _calmSlow) * k1;

            float tn = Mathf.SmoothStep(0f, 1f, DropDirector.Tension);
            // Impact jumps 0 -> 1 in one frame; fed straight into fold depth (x1.8) and
            // brightness it popped the whole sky in a single frame (M2 render: frame
            // change 33 vs a 0.34 median). A ~0.25 s attack keeps the flare but eases it in.
            float impT = DropDirector.Impact;
            _impS = impT > _impS ? Mathf.Lerp(_impS, impT, 1f - Mathf.Exp(-dt * 12f)) : impT;
            float imp = _impS;
            float phrase = beat / 64f * Mathf.PI * 2f;

            // Brighter in a breakdown, subdued while the drums play. A breakdown is also
            // when Tension builds, so a 0.35 tension dim cancelled the calm boost (only
            // ~15% brighter than the drums on the M2): weaker dim, wider calm range.
            float level = Mathf.Lerp(0.45f, 1f, _calmSlow) * (1f - 0.15f * tn) * (1f + 0.7f * imp);
            float hueBase = 0.45f + 0.06f * Mathf.Sin(phrase * 0.5f);
            float fold = (0.45f + 0.9f * Mathf.Clamp01(_bassSlow)) * (1f - 0.5f * tn) * (1f + 0.8f * imp);
            for (int i = 0; i < Curtains; i++)
            {
                float u = i / (Curtains - 1f);
                float swing = Mathf.Sin(phrase + i * 1.3f) * 2.2f; // travel direction reverses with the phrase
                float phase = clk * 0.05f + swing;
                float height = (15f + 8f * Mathf.Clamp01(_midSlow) + 4f * u + 5f * tn);
                _mpb.Clear();
                _mpb.SetFloat("_Hue", hueBase + u * 0.18f);
                _mpb.SetFloat("_Intensity", 0.55f * level);
                _mpb.SetFloat("_Phase", phase);
                _mpb.SetFloat("_Fold", fold);
                _mpb.SetFloat("_Height", height);
                _mpb.SetFloat("_Base", 5f + i * 1.2f);
                _mpb.SetFloat("_Z", -18f + i * 16f);
                _mpb.SetFloat("_Seed", i * 0.37f);
                _mpb.SetFloat("_Mirror", 0f);
                _cur[i].SetPropertyBlock(_mpb);
                _mpb.SetFloat("_Intensity", 0.55f * level * 0.3f);
                _mpb.SetFloat("_Mirror", 1f);
                _curMir[i].SetPropertyBlock(_mpb);
            }

            // Searchlights: slow sweeps from the horizon, reversing with the phrase.
            for (int i = 0; i < Lights; i++)
            {
                var toCentre = new Vector3(-_lightPos[i].x, 0f, -_lightPos[i].z + 10f).normalized;
                float sweep = Mathf.Sin(phrase * 0.5f + i * 1.05f) * 38f;
                float elev = (58f + 14f * Mathf.Sin(clk * 0.045f + i * 0.9f) + 14f * imp) * Mathf.Deg2Rad;
                var horiz = Quaternion.Euler(0f, sweep * (1f + 0.6f * imp), 0f) * toCentre;
                var d = (horiz * Mathf.Cos(elev) + Vector3.up * Mathf.Sin(elev)).normalized;
                var rot = Quaternion.FromToRotation(Vector3.up, d);
                _light[i].localPosition = _lightPos[i];
                _light[i].localRotation = rot;
                _light[i].localScale = new Vector3(1f, 110f, 1f);
                var pm = new Vector3(_lightPos[i].x, -_lightPos[i].y, _lightPos[i].z);
                var dm = new Vector3(d.x, -d.y, d.z);
                _lightMir[i].localPosition = pm;
                _lightMir[i].localRotation = Quaternion.FromToRotation(Vector3.up, dm);
                _lightMir[i].localScale = new Vector3(1f, 110f, 1f);

                Color c = TrippinLink.Palette(hueBase + 0.2f + i * 0.07f);
                float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b), 1e-3f);
                c = new Color(c.r / mx, c.g / mx, c.b / mx);
                float inten = 0.8f * level * (0.6f + 0.6f * s.intensity);
                _mpb.Clear();
                _mpb.SetColor("_Color", c);
                _mpb.SetFloat("_Intensity", inten);
                _lightR[i].SetPropertyBlock(_mpb);
                _mpb.SetFloat("_Intensity", inten * 0.3f);
                _lightMirR[i].SetPropertyBlock(_mpb);
            }

            for (int i = 0; i < _hazeM.Length; i++)
                _hazeM[i].SetFloat("_Density", _hazeBase[i] * (1f + 0.8f * tn + 0.8f * imp));

            UpdateCamera(beat, dt, tn);
        }

        // Low over the water, drifting sideways with the phrase and creeping
        // in on a build.
        void UpdateCamera(float beat, float dt, float tn)
        {
            float drift = Mathf.Sin(beat / 128f * Mathf.PI * 2f) * 12f;
            var want = new Vector3(drift, 2.6f + Mathf.Sin(beat / 48f * Mathf.PI * 2f) * 0.8f, Mathf.Lerp(-48f, -38f, tn));
            var look = new Vector3(drift * 0.3f, 13f + 2f * tn, 8f);
            if (!_camSet) { _camPos = want; _camLook = look; _camSet = true; }
            float k = 1f - Mathf.Exp(-dt * 0.7f);
            _camPos = Vector3.Lerp(_camPos, want, k);
            _camLook = Vector3.Lerp(_camLook, look, k);
            cam.transform.position = _camPos;
            cam.transform.LookAt(_camLook);
        }
    }
}
