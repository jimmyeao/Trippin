using UnityEngine;
using UnityEngine.Rendering;

namespace TrippinStage
{
    public class TidalCathedralShow : MonoBehaviour
    {
        public Camera cam;
        public Material sailMat, seaMat, skyMat, glowMat, coreMat, frameMat, hazeMat, beamMat;

        const int Pairs = 4;
        readonly Material[] _sails = new Material[Pairs * 2];
        readonly Transform[] _beams = new Transform[Pairs * 2];
        readonly MeshRenderer[] _beamRenderers = new MeshRenderer[Pairs * 2];
        Material _sea, _sky, _glow, _core, _frame, _haze, _beam;
        MaterialPropertyBlock _beamProperties;
        Mesh _sailMesh, _seaMesh, _frameMesh, _beamMesh;
        float _bass, _mids, _highs, _energy, _current, _kick;

        void Awake()
        {
            _sailMesh = Grid(36, 120);
            _seaMesh = Grid(112, 112);
            _frameMesh = Arch(96, 10);
            _frame = new Material(frameMat);
            _beamMesh = new Mesh { name = "laser shaft" };
            _beamMesh.vertices = new[] { Vector3.zero, Vector3.right, Vector3.up, Vector3.right + Vector3.up };
            _beamMesh.uv = new[] { Vector2.zero, Vector2.right, Vector2.up, Vector2.one };
            _beamMesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            _beamMesh.bounds = new Bounds(Vector3.zero, Vector3.one * 240f);
            _beam = new Material(beamMat);
            _beam.SetFloat("_Width", 0.09f);
            _beam.SetFloat("_Spread", 0.004f);
            _beam.SetFloat("_Core", 18f);
            _beam.SetFloat("_Smoke", 0.78f);
            _beam.SetFloat("_Fade", 0.84f);
            _beam.SetFloat("_Hot", 0.08f);
            _beamProperties = new MaterialPropertyBlock();
            for (int row = 0; row < Pairs; row++)
                for (int rib = 0; rib < 2; rib++)
                {
                    var vault = new GameObject("fixed glass vault " + row + " " + rib);
                    vault.transform.SetParent(transform, false);
                    vault.transform.localPosition = new Vector3(0, -0.7f, row * 24f + rib * 3.5f - 2f);
                    vault.AddComponent<MeshFilter>().sharedMesh = _frameMesh;
                    vault.AddComponent<MeshRenderer>().sharedMaterial = _frame;
                    if (rib == 0)
                        for (int bank = 0; bank < 2; bank++)
                        {
                            var foot = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
                            Destroy(foot.GetComponent<Collider>());
                            foot.name = "vault foundation";
                            foot.transform.SetParent(transform, false);
                            foot.transform.localPosition = new Vector3(bank == 0 ? -18f : 18f, -0.65f, row * 24f);
                            foot.transform.localScale = new Vector3(1.1f, 0.18f, 1.1f);
                            foot.GetComponent<Renderer>().sharedMaterial = _frame;
                        }
                }
            for (int row = 0; row < Pairs; row++)
                for (int bank = 0; bank < 2; bank++)
                {
                    int i = row * 2 + bank;
                    float side = bank == 0 ? -1f : 1f;
                    float x = side * (12f + (row % 2) * 2.5f);
                    float z = row * 24f + (bank == 0 ? -1.5f : 1.5f);
                    _sails[i] = new Material(sailMat);
                    _sails[i].SetFloat("_Height", 28f + row * 2.5f + bank * 3f);
                    _sails[i].SetFloat("_Width", 13f + row % 3 * 1.6f);
                    _sails[i].SetFloat("_Side", side);
                    _sails[i].SetFloat("_Phase", row * 1.72f + bank * 0.85f);
                    _sails[i].SetFloat("_Hue", 0.31f + row * 0.08f + bank * 0.14f);
                    var sail = new GameObject("vault sail " + i);
                    sail.transform.SetParent(transform, false);
                    sail.transform.localPosition = new Vector3(x, -0.7f, z);
                    sail.AddComponent<MeshFilter>().sharedMesh = _sailMesh;
                    var renderer = sail.AddComponent<MeshRenderer>();
                    renderer.sharedMaterial = _sails[i];
                    renderer.shadowCastingMode = ShadowCastingMode.Off;
                }
            for (int row = 0; row < Pairs; row++)
                for (int bank = 0; bank < 2; bank++)
                {
                    int i = row * 2 + bank;
                    float side = bank == 0 ? -1f : 1f;
                    var fixture = new GameObject("laser fixture " + i);
                    fixture.transform.SetParent(transform, false);
                    fixture.transform.localPosition = new Vector3(side * 17.5f, -0.3f, row * 24f + 2f);
                    fixture.transform.localScale = new Vector3(1f, 42f, 1f);
                    fixture.AddComponent<MeshFilter>().sharedMesh = _beamMesh;
                    var renderer = fixture.AddComponent<MeshRenderer>();
                    renderer.sharedMaterial = _beam;
                    renderer.shadowCastingMode = ShadowCastingMode.Off;
                    _beams[i] = fixture.transform;
                    _beamRenderers[i] = renderer;
                }

            var sea = new GameObject("dark sea");
            sea.transform.SetParent(transform, false);
            sea.transform.localPosition = new Vector3(0, -0.75f, -45f);
            sea.transform.localScale = new Vector3(340, 1, 340);
            sea.AddComponent<MeshFilter>().sharedMesh = _seaMesh;
            _sea = new Material(seaMat);
            sea.AddComponent<MeshRenderer>().sharedMaterial = _sea;

            var mist = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(mist.GetComponent<Collider>());
            mist.name = "water mist";
            mist.transform.SetParent(transform, false);
            mist.transform.localPosition = new Vector3(0, -0.25f, 46f);
            mist.transform.localRotation = Quaternion.Euler(90, 0, 0);
            mist.transform.localScale = new Vector3(100, 145, 1);
            _haze = new Material(hazeMat);
            mist.GetComponent<Renderer>().sharedMaterial = _haze;

            var sky = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            Destroy(sky.GetComponent<Collider>());
            sky.name = "twilight sky";
            sky.transform.SetParent(transform, false);
            sky.transform.localScale = Vector3.one * 520f;
            _sky = new Material(skyMat);
            sky.GetComponent<Renderer>().sharedMaterial = _sky;

            var glow = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(glow.GetComponent<Collider>());
            glow.name = "far light";
            glow.transform.SetParent(transform, false);
            glow.transform.localPosition = new Vector3(0, 27f, 106f);
            glow.transform.localScale = new Vector3(115f, 95f, 1f);
            _glow = new Material(glowMat);
            _glow.SetFloat("_Hue", 0.76f);
            glow.GetComponent<Renderer>().sharedMaterial = _glow;

            var core = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(core.GetComponent<Collider>());
            core.name = "tidal eye";
            core.transform.SetParent(transform, false);
            core.transform.localPosition = new Vector3(0, 27f, 98f);
            core.transform.localScale = Vector3.one * 68f;
            _core = new Material(coreMat);
            core.GetComponent<Renderer>().sharedMaterial = _core;
        }

        void OnEnable()
        {
            if (cam == null) return;
            cam.fieldOfView = 77f;
            cam.transform.position = new Vector3(0, 7.5f, -36f);
            cam.transform.LookAt(new Vector3(0, 11f, 42f));
        }

        void OnDisable()
        {
            if (cam != null) cam.fieldOfView = 52f;
        }

        void OnDestroy()
        {
            foreach (var m in _sails) if (m != null) Destroy(m);
            if (_sea != null) Destroy(_sea);
            if (_sky != null) Destroy(_sky);
            if (_glow != null) Destroy(_glow);
            if (_core != null) Destroy(_core);
            if (_frame != null) Destroy(_frame);
            if (_haze != null) Destroy(_haze);
            if (_beam != null) Destroy(_beam);
            if (_beamMesh != null) Destroy(_beamMesh);
            if (_frameMesh != null) Destroy(_frameMesh);
            if (_seaMesh != null) Destroy(_seaMesh);
            if (_sailMesh != null) Destroy(_sailMesh);
        }

        void Update()
        {
            var s = TrippinLink.State;
            float dt = Mathf.Min(Time.deltaTime, 1f / 30f);
            float low = s.lvl4 != null && s.lvl4.Length > 0 ? s.lvl4[0] : 0f;
            float mid = s.lvl4 != null && s.lvl4.Length > 2 ? (s.lvl4[1] + s.lvl4[2]) * 0.5f : 0f;
            float high = s.lvl4 != null && s.lvl4.Length > 3 ? s.lvl4[3] : 0f;
            // Attack fast, release slowly: the shapes must answer transients.
            _bass += (low - _bass) * (1f - Mathf.Exp(-dt / (low > _bass ? 0.07f : 0.4f)));
            _mids += (mid - _mids) * (1f - Mathf.Exp(-dt / (mid > _mids ? 0.07f : 0.45f)));
            _highs += (high - _highs) * (1f - Mathf.Exp(-dt / (high > _highs ? 0.05f : 0.3f)));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / (s.energy > _energy ? 0.12f : 0.6f)));
            _kick += (s.kick - _kick) * (1f - Mathf.Exp(-dt / (s.kick > _kick ? 0.04f : 0.18f)));
            float phrase = s.beat * Mathf.PI / 32f;
            _current += dt * (0.3f + _energy * 0.95f) * Mathf.Sin(phrase * 0.5f + 0.7f);
            for (int i = 0; i < _sails.Length; i++)
            {
                float swing = Mathf.Sin(phrase * 0.5f + i * 0.26f);
                var m = _sails[i];
                m.SetFloat("_Bass", _bass);
                m.SetFloat("_Mids", _mids);
                m.SetFloat("_Highs", _highs);
                m.SetFloat("_Current", _current);
                m.SetFloat("_Swing", swing);
                m.SetFloat("_Kick", _kick);
            }
            _sea.SetFloat("_Current", _current);
            _sea.SetFloat("_Energy", _energy);
            _sea.SetFloat("_Kick", _kick);
            _sky.SetFloat("_Drift", _current);
            _glow.SetFloat("_Glow", 0.25f + 0.25f * s.intensity);
            _core.SetFloat("_Current", _current);
            _core.SetFloat("_Pulse", _kick);

            float clk1 = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : s.flow;
            float clk2 = s.clock4 != null && s.clock4.Length > 2 ? s.clock4[2] : s.flow * 0.7f;
            int block = Mathf.FloorToInt(s.beat / 16f);
            int formation = ((block % 4) + 4) % 4;
            float morph = Mathf.SmoothStep(0f, 1f, Mathf.Clamp01(s.beat - block * 16f));
            var focus = new Vector3(0f, 26f, 92f);
            for (int i = 0; i < _beams.Length; i++)
            {
                int row = i / 2;
                float side = i % 2 == 0 ? -1f : 1f;
                float scan = Mathf.Sin(clk1 * Mathf.PI + i * 1.37f) * (2.5f + 6.5f * _energy)
                           + Mathf.Sin(s.beat * Mathf.PI + i * 0.9f) * (1.2f + 2.2f * _kick);
                float lift = Mathf.Sin(clk2 * Mathf.PI * 0.5f + i * 0.83f) * (1.5f + 4f * _energy);
                var end = Vector3.Lerp(BeamTarget((formation + 3) % 4, row, side, scan, lift),
                                       BeamTarget(formation, row, side, scan, lift), morph);
                // Kicks pull the rig part-way toward the window — a dip, not a flash.
                end = Vector3.Lerp(end, focus, _kick * 0.3f);
                var delta = end - _beams[i].localPosition;
                _beams[i].localRotation = Quaternion.FromToRotation(Vector3.up, delta.normalized);
                _beams[i].localScale = new Vector3(1f, delta.magnitude, 1f);
                _beamProperties.Clear();
                _beamProperties.SetColor("_Color", Color.Lerp(TrippinLink.Palette(0.12f + i * 0.09f), new Color(0.06f, 0.65f, 0.85f), 0.42f));
                _beamProperties.SetFloat("_Intensity", (0.2f + 0.45f * _energy + 0.3f * _kick) * (1f - 0.6f * s.calm));
                _beamRenderers[i].SetPropertyBlock(_beamProperties);
            }

            float move = s.flow * 0.055f;
            var target = new Vector3(Mathf.Sin(phrase * 0.5f) * 1.8f, 7.5f + Mathf.Sin(move * 0.6f) * 1.1f,
                                     -12f - 24f * Mathf.Cos(move) - _kick * 2.5f);
            cam.transform.position = Vector3.Lerp(cam.transform.position, target, 1f - Mathf.Exp(-dt * 1.4f));
            cam.transform.LookAt(new Vector3(Mathf.Sin(phrase * 0.5f), 11f, cam.transform.position.z + 80f));
        }

        static Vector3 BeamTarget(int formation, int row, float side, float sweep, float lift)
        {
            float z = row * 24f;
            switch (formation)
            {
                case 0: return new Vector3(-side * (4f + row * 1.5f) + sweep, 31f + row + lift, z + 22f);
                case 1: return new Vector3(side * (9f + row) + sweep * 1.8f, 42f + lift, z + 30f);
                case 2: return new Vector3(side * row * 1.2f + sweep * 0.45f, 25f + row * 2f + lift, 92f);
                default: return new Vector3(side * (2f + row) + sweep, 37f + lift, z + 12f);
            }
        }

        static Mesh Arch(int steps, int sides)
        {
            var vertices = new Vector3[(steps + 1) * (sides + 1)];
            var normals = new Vector3[vertices.Length];
            var uv = new Vector2[vertices.Length];
            var tris = new int[steps * sides * 6];
            Vector3 Point(float t)
            {
                float x = t * 2f - 1f;
                return new Vector3(x * 18f, 35f * Mathf.Pow(1f - Mathf.Abs(x), 0.62f), 0);
            }
            for (int i = 0; i <= steps; i++)
            {
                float t = i / (float)steps;
                var p = Point(t);
                var tangent = (Point(Mathf.Min(1f, t + 0.002f)) - Point(Mathf.Max(0f, t - 0.002f))).normalized;
                var cross = new Vector3(-tangent.y, tangent.x, 0);
                for (int j = 0; j <= sides; j++)
                {
                    float a = j / (float)sides * Mathf.PI * 2f;
                    var n = cross * Mathf.Cos(a) + Vector3.forward * Mathf.Sin(a);
                    int k = i * (sides + 1) + j;
                    vertices[k] = p + n * 0.28f;
                    normals[k] = n;
                    uv[k] = new Vector2(t, j / (float)sides);
                }
            }
            for (int i = 0; i < steps; i++)
                for (int j = 0; j < sides; j++)
                {
                    int a = i * (sides + 1) + j;
                    int t = (i * sides + j) * 6;
                    tris[t] = a; tris[t + 1] = a + 1; tris[t + 2] = a + sides + 1;
                    tris[t + 3] = a + 1; tris[t + 4] = a + sides + 2; tris[t + 5] = a + sides + 1;
                }
            var mesh = new Mesh { indexFormat = IndexFormat.UInt32 };
            mesh.vertices = vertices;
            mesh.normals = normals;
            mesh.uv = uv;
            mesh.triangles = tris;
            mesh.bounds = new Bounds(new Vector3(0, 17, 0), new Vector3(40, 72, 4));
            return mesh;
        }

        static Mesh Grid(int nx, int ny)
        {
            var vertices = new Vector3[(nx + 1) * (ny + 1)];
            var uv = new Vector2[vertices.Length];
            var tris = new int[nx * ny * 6];
            for (int y = 0; y <= ny; y++)
                for (int x = 0; x <= nx; x++)
                {
                    int i = y * (nx + 1) + x;
                    vertices[i] = new Vector3(x / (float)nx - 0.5f, 0, y / (float)ny);
                    uv[i] = new Vector2(x / (float)nx, y / (float)ny);
                }
            for (int y = 0; y < ny; y++)
                for (int x = 0; x < nx; x++)
                {
                    int a = y * (nx + 1) + x;
                    int t = (y * nx + x) * 6;
                    tris[t] = a; tris[t + 1] = a + nx + 1; tris[t + 2] = a + 1;
                    tris[t + 3] = a + 1; tris[t + 4] = a + nx + 1; tris[t + 5] = a + nx + 2;
                }
            var mesh = new Mesh { indexFormat = IndexFormat.UInt32 };
            mesh.vertices = vertices;
            mesh.uv = uv;
            mesh.triangles = tris;
            mesh.bounds = new Bounds(new Vector3(0, 10, 0), new Vector3(80, 100, 110));
            return mesh;
        }
    }
}
