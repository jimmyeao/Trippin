// Light-show show: the rig is the subject. A dark arena with 48 moving
// heads (30 on three overhead trusses, 18 on a floor ring), each throwing a
// wide haze cone plus a thin laser core, splashing onto a wet floor.
//  - Shape: cone width follows the slow bass presence (iris opening), the
//    beam spread breathes with it; formations are geometry (curtain, helix,
//    cathedral, fan, crossing sheets, X-weave) that morph in over a beat,
//    a new one every 4 bars. Head count never changes.
//  - Motion: aim points ride the smooth energy clock (clock4) and swing
//    direction on phrase-length sines; the camera orbit swings with the
//    phrase. Nothing integrates raw audio.
//  - Drops: DropDirector tension pulls every beam into one cathedral point
//    over the floor centre and dims the rig; the drop blows the formation
//    outward and flares the cones for a few beats.
// No beat-synced flashes: brightness rides slow presence + the drop impact.

using UnityEngine;

namespace TrippinStage
{
    public class LightstormShow : MonoBehaviour
    {
        public Camera cam;
        public Material beamMat, glowMat, groundMat, hazeMat;

        const int Rows = 3, PerRow = 10, Ring = 18, Heads = Rows * PerRow + Ring;
        const int Formations = 6;
        const float FogDist = 140f;

        readonly Vector3[] _pos = new Vector3[Heads];
        readonly Transform[] _cone = new Transform[Heads], _core = new Transform[Heads], _splash = new Transform[Heads];
        readonly Renderer[] _coneR = new Renderer[Heads], _coreR = new Renderer[Heads], _splashR = new Renderer[Heads];
        readonly Quaternion[] _from = new Quaternion[Heads];
        readonly Quaternion[] _cur = new Quaternion[Heads];
        readonly Material[] _hazeM = new Material[3];
        readonly float[] _hazeBase = { 0.10f, 0.07f, 0.05f };
        Material _groundM;
        MaterialPropertyBlock _mpb;
        int _formation = -1;
        float _formStart;
        float _bassSlow, _calmSlow, _farWas;
        Vector3 _camPos, _camLook;
        bool _camSet;

        void Awake()
        {
            _mpb = new MaterialPropertyBlock();
            var beamMesh = new Mesh { name = "beam" };
            beamMesh.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            beamMesh.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            beamMesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            beamMesh.bounds = new Bounds(Vector3.zero, Vector3.one * 500f); // billboarded in the shader

            // Wet floor.
            var ground = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(ground.GetComponent<Collider>());
            ground.name = "floor";
            ground.transform.SetParent(transform, false);
            ground.transform.localRotation = Quaternion.Euler(90, 0, 0);
            ground.transform.localScale = new Vector3(900, 900, 1);
            _groundM = new Material(groundMat);
            _groundM.SetFloat("_FogDist", FogDist);
            ground.GetComponent<Renderer>().sharedMaterial = _groundM;

            // Heads: three overhead trusses, then a floor ring looking up.
            for (int i = 0; i < Heads; i++)
            {
                if (i < Rows * PerRow)
                {
                    int r = i / PerRow, k = i % PerRow;
                    _pos[i] = new Vector3(Mathf.Lerp(-27f, 27f, k / (PerRow - 1f)), 19f + r * 1.5f, -12f + r * 12f);
                }
                else
                {
                    float a = (i - Rows * PerRow) / (float)Ring * Mathf.PI * 2f;
                    _pos[i] = new Vector3(Mathf.Cos(a) * 24f, 0.4f, Mathf.Sin(a) * 24f);
                }
                _cone[i] = MakeBeam(beamMesh, _pos[i], 0.30f, 0.040f, 6f, 0.55f, 0.9f, 0f, "cone " + i, out _coneR[i]);
                // _Hot 0.2: a 0.6 white-hot centre burnt every core to white (pastel); keep them saturated.
                _core[i] = MakeBeam(beamMesh, _pos[i], 0.025f, 0.0012f, 40f, 0.75f, 0.9f, 0.2f, "core " + i, out _coreR[i]);
                var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Destroy(q.GetComponent<Collider>());
                q.name = "splash " + i;
                q.transform.SetParent(transform, false);
                q.GetComponent<Renderer>().sharedMaterial = new Material(glowMat);
                q.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
                _splash[i] = q.transform;
                _splashR[i] = q.GetComponent<Renderer>();
                _cur[i] = _from[i] = Quaternion.FromToRotation(Vector3.up, Vector3.down);
            }

            // Fog banks between the camera and the rig.
            float[] z = { 18f, 4f, -14f };
            for (int i = 0; i < 3; i++)
            {
                var h = GameObject.CreatePrimitive(PrimitiveType.Quad);
                Destroy(h.GetComponent<Collider>());
                h.name = "haze " + i;
                h.transform.SetParent(transform, false);
                h.transform.localPosition = new Vector3(0, 10f, z[i]);
                h.transform.localScale = new Vector3(120f, 36f, 1f);
                _hazeM[i] = new Material(hazeMat);
                _hazeM[i].SetFloat("_Density", _hazeBase[i]);
                _hazeM[i].SetFloat("_HueOff", i * 0.17f);
                h.GetComponent<Renderer>().sharedMaterial = _hazeM[i];
                h.GetComponent<Renderer>().shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            }
        }

        Transform MakeBeam(Mesh mesh, Vector3 pos, float width, float spread, float core, float smoke, float fade, float hot, string name, out Renderer r)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.transform.localPosition = pos;
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

        void OnEnable()
        {
            if (cam != null) { _farWas = cam.farClipPlane; cam.farClipPlane = 800f; }
            _camSet = false;
        }

        void OnDisable()
        {
            if (cam != null && _farWas > 0f) cam.farClipPlane = _farWas;
        }

        // Where head i aims in formation f: a world point (floor splash is
        // wherever the ray meets y=0). `clk` is the smooth energy clock in
        // beats, `ph` a phrase-length sine for direction swings.
        Vector3 Target(int i, int f, float clk, float ph, float beat)
        {
            bool ring = i >= Rows * PerRow;
            int row = ring ? 0 : i / PerRow;
            float u = ring ? (i - Rows * PerRow) / (float)Ring : (i % PerRow) / (PerRow - 1f);
            float side = u * 2f - 1f;
            float a = u * Mathf.PI * 2f;
            switch (f)
            {
                case 0: // curtain: every head hits one line that sweeps over the floor
                {
                    float z = ph * 16f + (ring ? 0f : (row - 1) * 3f);
                    return ring ? new Vector3(side * 14f, 18f, z * 0.4f) : new Vector3(side * 22f, 0f, z);
                }
                case 1: // helix: rings on the floor, ring heads spiral up to a cone
                {
                    float w = clk * 0.18f + u * Mathf.PI * 2f + row * 2.1f;
                    return ring ? new Vector3(Mathf.Cos(w) * 5f, 24f, Mathf.Sin(w) * 5f)
                                : new Vector3(Mathf.Cos(w) * 11f, 0f, Mathf.Sin(w) * 11f);
                }
                case 2: // fan: each head waves about straight down, phase across the rig
                {
                    float wv = Mathf.Sin(clk * 0.22f + u * 5f + row * 1.3f);
                    return ring ? new Vector3(Mathf.Cos(a) * 24f * (1f - 0.6f * (0.5f + 0.5f * wv)), 22f, Mathf.Sin(a) * 24f * (1f - 0.6f * (0.5f + 0.5f * wv)))
                                : new Vector3(side * 26f + wv * 14f, 0f, (row - 1) * 12f + Mathf.Cos(clk * 0.17f + u * 3f) * 8f);
                }
                case 3: // cathedral: ring converges overhead, trusses draw an X on the floor
                {
                    float d = ph * 6f;
                    return ring ? new Vector3(d, 17f, 0f)
                                : new Vector3(side * 20f * (row % 2 == 0 ? 1f : -1f), 0f, (row - 1) * 14f + side * 10f);
                }
                case 4: // crossing sheets: rows scan in opposite directions
                {
                    float dir = row % 2 == 0 ? 1f : -1f;
                    float sw = Mathf.Sin(clk * 0.13f + u * 0.7f) * 20f * dir;
                    return ring ? new Vector3(Mathf.Cos(a + clk * 0.05f) * 14f, 20f, Mathf.Sin(a + clk * 0.05f) * 14f)
                                : new Vector3(sw, 0f, (row - 1) * 16f + side * 4f);
                }
                default: // X-weave: heads aim at the mirrored head's spot
                {
                    float m = ring ? Mathf.Cos(a) * -22f : -side * 22f;
                    float sz = ring ? Mathf.Sin(a) * -22f : (row - 1) * 12f + ph * 6f;
                    return ring ? new Vector3(m, 14f, sz) : new Vector3(m, 0f, sz);
                }
            }
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Time.deltaTime;
            float beat = s.beat;
            DropDirector.Tick(s, dt);
            int bar = Mathf.FloorToInt(beat / 4f);
            int f = (bar / 4) % Formations;
            if (f != _formation)
            {
                for (int i = 0; i < Heads; i++) _from[i] = _cur[i];
                _formation = f;
                _formStart = beat;
            }
            float m = Mathf.SmoothStep(0, 1, Mathf.Clamp01(beat - _formStart));

            float clk = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            float ph = Mathf.Sin(beat / 32f * Mathf.PI * 2f);
            float pres0 = s.pres4 != null && s.pres4.Length > 0 ? s.pres4[0] : 0.4f;
            float k = 1f - Mathf.Exp(-dt * 1.2f);
            _bassSlow += (pres0 - _bassSlow) * k;
            _calmSlow += (s.calm - _calmSlow) * k;

            float tn = Mathf.SmoothStep(0f, 1f, DropDirector.Tension);
            float imp = DropDirector.Impact;
            var focus = new Vector3(0f, 16f, 0f);
            float burst = 1f + 0.9f * imp; // the drop blows the formation outward

            // Breakdowns keep every third head, dim and slow; tension wakes them up.
            float on = Mathf.Lerp(1f, 0.15f, _calmSlow);
            float drama = (1f - 0.5f * tn) * (1f + 1.6f * imp);
            float iris = 0.65f + 0.9f * _bassSlow + 0.5f * imp; // cone opening, slow bass

            float hueBase = 0.1f * f + 0.03f * Mathf.Sin(beat / 64f * Mathf.PI * 2f);
            for (int i = 0; i < Heads; i++)
            {
                var tgt = Target(i, f, clk, ph, beat);
                tgt = new Vector3(tgt.x * burst, tgt.y, tgt.z * burst);
                tgt = Vector3.Lerp(tgt, focus, tn * 0.55f); // 0.8 stacked all 48 beams in one point: white-out
                var dirv = tgt - _pos[i];
                var d = dirv.normalized;
                var want = Quaternion.FromToRotation(Vector3.up, d);
                _cur[i] = Quaternion.Slerp(_from[i], want, m);
                d = _cur[i] * Vector3.up;

                // Length: stop at the floor when the beam meets it.
                float len = 90f;
                bool hit = d.y < -0.02f;
                if (hit) len = Mathf.Min(len, -_pos[i].y / d.y);
                _cone[i].localRotation = _core[i].localRotation = _cur[i];
                _cone[i].localScale = _core[i].localScale = new Vector3(1f, len, 1f);

                bool keep = i % 3 == 0;
                float vis = Mathf.Max(on, keep ? 0.35f : 0f, tn * 0.6f);
                Color c = TrippinLink.Palette(hueBase + (i < Rows * PerRow ? (i / PerRow) * 0.09f : 0.3f) + (i % PerRow) * 0.012f);
                float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b), 1e-3f);
                c = new Color(c.r / mx, c.g / mx, c.b / mx);

                _mpb.Clear();
                _mpb.SetColor("_Color", c);
                // Haze cones were ~1.8 m wide at the floor (x3 for the glow quad) and added up
                // across 48 heads into a flat wash; a third of the width keeps dark gaps.
                _mpb.SetFloat("_Width", 0.12f * iris);
                _mpb.SetFloat("_Spread", 0.012f + 0.008f * iris);
                _mpb.SetFloat("_Fade", hit ? 0.95f : 0.5f);
                _mpb.SetFloat("_Intensity", vis * drama * 0.16f * (0.55f + 0.45f * s.intensity));
                _coneR[i].SetPropertyBlock(_mpb);
                _mpb.Clear();
                _mpb.SetColor("_Color", c);
                _mpb.SetFloat("_Fade", hit ? 0.95f : 0.55f);
                _mpb.SetFloat("_Intensity", vis * drama * (0.9f + 0.5f * _bassSlow) * (0.6f + 0.6f * s.intensity));
                _coreR[i].SetPropertyBlock(_mpb);

                // Floor splash where the beam lands, stretched along the beam.
                bool show = hit && vis > 0.02f;
                _splashR[i].enabled = show;
                if (show)
                {
                    var p = _pos[i] + d * len;
                    float sz = 2.2f + len * 0.07f * iris;
                    float el = 1f / Mathf.Max(0.35f, -d.y);
                    _splash[i].localPosition = new Vector3(p.x, 0.04f, p.z);
                    _splash[i].localRotation = Quaternion.Euler(90f, Mathf.Atan2(d.x, d.z) * Mathf.Rad2Deg, 0f);
                    _splash[i].localScale = new Vector3(sz, sz * el, 1f);
                    _mpb.Clear();
                    _mpb.SetFloat("_Glow", 0.55f * vis * drama);
                    _mpb.SetFloat("_Hue", hueBase + (i / PerRow) * 0.09f + (i % PerRow) * 0.012f);
                    _splashR[i].SetPropertyBlock(_mpb);
                }
            }

            // Fog thickens through a build and puffs out on the drop.
            for (int i = 0; i < _hazeM.Length; i++)
                _hazeM[i].SetFloat("_Density", _hazeBase[i] * (1f + 0.8f * tn + 0.8f * imp));

            UpdateCamera(s, beat, dt, tn);
        }

        // A slow orbit round the rig that swings direction with the phrase,
        // creeping in during a build and springing back after the drop.
        void UpdateCamera(ShowState s, float beat, float dt, float tn)
        {
            float swing = Mathf.Sin(beat / 128f * Mathf.PI * 2f) * 1.0f;
            float r = Mathf.Lerp(40f, 30f, tn) + Mathf.Sin(beat / 64f * Mathf.PI * 2f) * 4f;
            var want = new Vector3(Mathf.Sin(swing) * r, 3.2f + 2.5f * (1f - tn) + Mathf.Sin(beat / 48f * Mathf.PI * 2f) * 1.2f, -Mathf.Cos(swing) * r);
            var look = new Vector3(0f, 10f + 3f * tn, 0f);
            if (!_camSet) { _camPos = want; _camLook = look; _camSet = true; }
            float k = 1f - Mathf.Exp(-dt * 0.8f);
            _camPos = Vector3.Lerp(_camPos, want, k);
            _camLook = Vector3.Lerp(_camLook, look, k);
            cam.transform.position = _camPos;
            cam.transform.LookAt(_camLook);
        }
    }
}
