// unity_orbit_foundry — a suspended forge in deep space. A molten metal core
// is held inside three precessing gimbal rings while four weld pods on a
// fixed outer gantry fire beams at its surface. Bass swells the core's mass
// and roughens it, mids drive ring precession, highs steer the weld points
// and scatter sparks, kicks compress the core and surge the beams. The whole
// machine cools to a dark solid in breakdowns. The gantry and floor never
// move — all reaction is on the core, the rings and the beams.
using UnityEngine;
using UnityEngine.Rendering;

namespace TrippinStage
{
    public class OrbitFoundryShow : MonoBehaviour
    {
        public Camera cam;
        public Material coreMat, ringMat, floorMat, skyMat, beamMat, pointsMat;

        const int Sparks = 1200;
        const int Pods = 4;
        const int Rings = 3;

        float _bass, _mids, _highs, _energy, _kick;
        readonly System.Random _rng = new System.Random(90417);

        Material _core, _floor, _sky, _beam;
        MaterialPropertyBlock _beamProps;
        Mesh _beamMesh, _coreMesh, _ringMesh;
        readonly Transform[] _ringT = new Transform[Rings];
        readonly Material[] _ringM = new Material[Rings];
        readonly float[] _ringPhase = new float[Rings];
        readonly Transform[] _pods = new Transform[Pods];
        readonly Transform[] _beams = new Transform[Pods];
        readonly MeshRenderer[] _beamR = new MeshRenderer[Pods];
        readonly Vector3[] _impact = new Vector3[Pods];
        readonly Vector3[] _impactVel = new Vector3[Pods];

        ComputeBuffer _sparkBuf;
        Vector4[] _sparkD;
        Vector3[] _sparkV;
        float[] _sparkAge;
        Material _points;
        RenderParams _sparkRp;
        Transform _coreT;

        void Start()
        {
            _coreMesh = Sphere(96, 64);
            _ringMesh = Torus(64, 16);
            _beamMesh = new Mesh { name = "weld shaft" };
            _beamMesh.vertices = new[] { Vector3.zero, Vector3.right, Vector3.up, Vector3.right + Vector3.up };
            _beamMesh.uv = new[] { Vector2.zero, Vector2.right, Vector2.up, Vector2.one };
            _beamMesh.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            _beamMesh.bounds = new Bounds(Vector3.zero, Vector3.one * 240f);
            _beam = new Material(beamMat);
            _beam.SetFloat("_Width", 0.07f);
            _beam.SetFloat("_Spread", 0.006f);
            _beam.SetFloat("_Core", 22f);
            _beam.SetFloat("_Smoke", 0.55f);
            _beam.SetFloat("_Fade", 0.9f);
            _beam.SetFloat("_Hot", 0.5f);
            _beamProps = new MaterialPropertyBlock();

            _core = new Material(coreMat);
            var core = new GameObject("molten core");
            core.transform.SetParent(transform, false);
            core.transform.localPosition = new Vector3(0, 2f, 0);
            core.transform.localScale = Vector3.one * 2.6f;
            core.AddComponent<MeshFilter>().sharedMesh = _coreMesh;
            core.AddComponent<MeshRenderer>().sharedMaterial = _core;
            _core.SetVector("_Morph", Vector4.one); // fixed sphere — no shape morphs
            _coreT = core.transform;

            // Gantry: the fixed outer housing ring + weld pods. It never
            // moves — the precession lives on the three inner rings.
            var gantry = new GameObject("gantry");
            gantry.transform.SetParent(transform, false);
            gantry.transform.localPosition = new Vector3(0, 2f, 0);
            gantry.transform.localScale = Vector3.one * 13f; // uniform: a round tube, not a ribbon
            gantry.transform.localRotation = Quaternion.Euler(6f, 0, 4f);
            gantry.AddComponent<MeshFilter>().sharedMesh = _ringMesh;
            var gantryMat = new Material(ringMat);
            gantryMat.SetFloat("_Tint", 0.62f);
            gantryMat.SetFloat("_Glow", 0.15f);
            gantryMat.SetFloat("_Segs", 10f);
            gantry.AddComponent<MeshRenderer>().sharedMaterial = gantryMat;

            float[] rr = { 5.6f, 7.6f, 9.7f };
            for (int i = 0; i < Rings; i++)
            {
                var r = new GameObject("gimbal ring " + i);
                r.transform.SetParent(transform, false);
                r.transform.localPosition = new Vector3(0, 2f, 0);
                r.transform.localScale = Vector3.one * rr[i];
                r.AddComponent<MeshFilter>().sharedMesh = _ringMesh;
                _ringM[i] = new Material(ringMat);
                _ringM[i].SetFloat("_Tint", 0.52f + i * 0.13f);
                _ringM[i].SetFloat("_Segs", 6f + i * 2f);
                r.AddComponent<MeshRenderer>().sharedMaterial = _ringM[i];
                _ringT[i] = r.transform;
            }

            for (int i = 0; i < Pods; i++)
            {
                float a = i * Mathf.PI * 0.5f + Mathf.PI * 0.25f;
                var pod = new GameObject("weld pod " + i);
                pod.transform.SetParent(transform, false);
                // Weld pods mounted on the gantry ring, angled in at the core.
                pod.transform.localPosition = new Vector3(Mathf.Cos(a) * 12.7f, 2f + (i % 2 == 0 ? 0.6f : -0.6f), Mathf.Sin(a) * 12.7f);
                pod.transform.localScale = new Vector3(0.8f, 0.8f, 3.2f);
                var podGo = GameObject.CreatePrimitive(PrimitiveType.Cube);
                Destroy(podGo.GetComponent<Collider>());
                podGo.transform.SetParent(pod.transform, false);
                var podMat = new Material(ringMat);
                podMat.SetFloat("_Tint", 0.05f);
                podMat.SetFloat("_Glow", 0.3f);
                podMat.SetFloat("_Segs", 2f);
                podGo.GetComponent<Renderer>().sharedMaterial = podMat;
                _pods[i] = pod.transform;

                var beam = new GameObject("weld beam " + i);
                beam.transform.SetParent(transform, false);
                beam.AddComponent<MeshFilter>().sharedMesh = _beamMesh;
                _beamR[i] = beam.AddComponent<MeshRenderer>();
                _beamR[i].sharedMaterial = _beam;
                _beams[i] = beam.transform;
                _impact[i] = Random.onUnitSphere * 2.4f;
                _impactVel[i] = Random.onUnitSphere;
            }

            var floor = new GameObject("foundry floor");
            floor.transform.SetParent(transform, false);
            floor.transform.localPosition = new Vector3(0, -13f, 0);
            floor.transform.localScale = new Vector3(150, 1, 150);
            floor.AddComponent<MeshFilter>().sharedMesh = Grid(2);
            _floor = new Material(floorMat);
            floor.AddComponent<MeshRenderer>().sharedMaterial = _floor;

            var sky = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            Destroy(sky.GetComponent<Collider>());
            sky.name = "deep sky";
            sky.transform.SetParent(transform, false);
            sky.transform.localScale = Vector3.one * 560f;
            _sky = new Material(skyMat);
            _sky.SetFloat("_Aurora", 0.4f);
            sky.GetComponent<Renderer>().sharedMaterial = _sky;

            _sparkBuf = new ComputeBuffer(Sparks, 16);
            _sparkD = new Vector4[Sparks];
            _sparkV = new Vector3[Sparks];
            _sparkAge = new float[Sparks];
            for (int i = 0; i < Sparks; i++) _sparkAge[i] = 99f;
            _points = new Material(pointsMat);
            _points.SetBuffer("_Pos", _sparkBuf);
            _points.SetFloat("_Size", 0.11f);
            _points.SetFloat("_Gain", 1.5f);
            _points.SetFloat("_Spark", 0f); // sparks stream; they don't flash
            _sparkRp = new RenderParams(_points) { worldBounds = new Bounds(Vector3.zero, Vector3.one * 900f) };
        }

        void Update()
        {
            float dt = Time.deltaTime;
            var s = TrippinLink.State;
            float low = s.lvl4 != null && s.lvl4.Length > 0 ? s.lvl4[0] : 0f;
            float mid = s.lvl4 != null && s.lvl4.Length > 2 ? (s.lvl4[1] + s.lvl4[2]) * 0.5f : 0f;
            float high = s.lvl4 != null && s.lvl4.Length > 3 ? s.lvl4[3] : 0f;
            // Attack fast, release slowly — the forge must answer transients.
            _bass += (low - _bass) * (1f - Mathf.Exp(-dt / (low > _bass ? 0.07f : 0.4f)));
            _mids += (mid - _mids) * (1f - Mathf.Exp(-dt / (mid > _mids ? 0.07f : 0.45f)));
            _highs += (high - _highs) * (1f - Mathf.Exp(-dt / (high > _highs ? 0.05f : 0.3f)));
            _energy += (s.energy - _energy) * (1f - Mathf.Exp(-dt / (s.energy > _energy ? 0.12f : 0.6f)));
            _kick += (s.kick - _kick) * (1f - Mathf.Exp(-dt / (s.kick > _kick ? 0.04f : 0.18f)));
            float heat = (0.45f + _energy * 0.6f + _kick * 0.2f) * (1f - s.calm * 0.6f);

            // The core stays a sphere — it slowly tumbles like a mirror ball
            // while bass/kick breathe its radius in the shader.
            _coreT.rotation = Quaternion.Euler(s.flow * 1.7f, s.flow * 2.6f, s.flow * 1.1f);
            _core.SetFloat("_Bass", _bass);
            _core.SetFloat("_Kick", _kick);
            _core.SetFloat("_Heat", heat);
            _core.SetFloat("_Amp", (0.03f + _bass * 0.09f) * (1f - s.calm * 0.55f));
            _core.SetFloat("_NoiseT", s.flow * 0.35f);

            // Gimbals: each ring keeps a fixed tilt plane and precesses its
            // axis slowly — a gyroscope reads clean; tumbling reads tangled.
            float spin = 0.06f + _mids * 0.45f;
            float[] tilts = { 18f, 62f, 78f };
            for (int i = 0; i < Rings; i++)
            {
                _ringPhase[i] += dt * spin * (i == 1 ? -0.85f : 0.5f + i * 0.4f);
                float ph = _ringPhase[i] * Mathf.Rad2Deg;
                _ringT[i].rotation = Quaternion.AngleAxis(ph, Vector3.up)
                                   * Quaternion.AngleAxis(tilts[i], Vector3.right)
                                   * Quaternion.AngleAxis(ph * 0.6f, Vector3.forward);
                _ringM[i].SetFloat("_Scroll", s.flow * (0.3f + i * 0.2f));
                _ringM[i].SetFloat("_Glow", 0.2f + _energy * 0.5f + _kick * 0.4f);
            }

            // Weld points wander the core surface on the flow clock, jitter
            // with highs; kicks pull every beam toward a common strike zone.
            Vector3 strike = new Vector3(Mathf.Sin(s.flow * 0.4f), Mathf.Cos(s.flow * 0.27f), Mathf.Sin(s.flow * 0.31f)) * 2.4f;
            for (int i = 0; i < Pods; i++)
            {
                Vector3 want = new Vector3(Mathf.Sin(s.flow * 0.5f + i * 1.9f), Mathf.Sin(s.flow * 0.34f + i * 2.7f), Mathf.Cos(s.flow * 0.43f + i * 1.3f));
                want = Vector3.Lerp(want.normalized * 2.4f, strike, _kick * 0.55f);
                want += Random.insideUnitSphere * _highs * 0.55f;
                _impact[i] = Vector3.Lerp(_impact[i], want, 1f - Mathf.Exp(-dt / 0.12f));
                Vector3 wp = _impact[i] + new Vector3(0, 2f, 0);
                Vector3 start = _pods[i].position + _pods[i].forward * 1.2f;
                _pods[i].LookAt(wp);
                Vector3 d = wp - start;
                _beams[i].position = start;
                _beams[i].up = d.normalized;
                _beams[i].localScale = new Vector3(1, d.magnitude, 1);
                _beamProps.SetColor("_Color", Color.Lerp(TrippinLink.Palette(0.6f + i * 0.06f), new Color(0.7f, 0.82f, 1f), 0.55f));
                _beamProps.SetFloat("_Intensity", (0.4f + _energy * 0.6f + _kick * 0.4f) * (1f - s.calm * 0.6f));
                _beamR[i].SetPropertyBlock(_beamProps);
            }

            // Sparks shed from the weld points; highs feed the emission rate.
            float rate = (8f + _highs * 60f + _energy * 25f) * (1f - s.calm * 0.8f);
            SpawnSparks(rate * dt);
            for (int i = 0; i < Sparks; i++)
            {
                if (_sparkAge[i] > 2.2f) continue;
                _sparkAge[i] += dt;
                _sparkV[i] += Vector3.down * dt * 1.6f;
                _sparkV[i] *= 1f - dt * 0.5f;
                _sparkD[i] += new Vector4(_sparkV[i].x * dt, _sparkV[i].y * dt, _sparkV[i].z * dt, 0f);
                _sparkD[i].w = _sparkAge[i] < 2.2f ? _sparkV[i].magnitude : 0f;
            }
            _sparkBuf.SetData(_sparkD);
            Graphics.RenderPrimitives(_sparkRp, MeshTopology.Triangles, Sparks * 6);

            _floor.SetFloat("_Glow", 0.3f + _energy * 0.45f + _kick * 0.35f);
            _floor.SetFloat("_Drift", s.flow * 0.1f);
            _sky.SetFloat("_Drift", s.flow * 0.06f);

            // Camera: a slow orbit round the machine, kick pushes in.
            float ang = s.flow * 0.035f + Mathf.Sin(s.flow * 0.008f) * 0.8f;
            float rad = 21f + Mathf.Sin(s.flow * 0.021f) * 3f - _kick * 1.6f;
            cam.transform.position = new Vector3(Mathf.Sin(ang) * rad, 6.5f + Mathf.Sin(s.flow * 0.017f) * 3.5f, Mathf.Cos(ang) * rad);
            cam.transform.LookAt(new Vector3(0, 2f, 0));
        }

        void SpawnSparks(float budget)
        {
            for (int i = 0; i < Sparks && budget > 0; i++)
            {
                if (_sparkAge[i] <= 2.2f) continue;
                budget -= 1f;
                int p = _rng.Next(Pods);
                Vector3 n = _impact[p].normalized;
                _sparkD[i] = new Vector4(_impact[p].x, _impact[p].y + 2f, _impact[p].z, 0f);
                _sparkV[i] = n * (2f + (float)_rng.NextDouble() * 3.5f)
                           + Random.insideUnitSphere * 2.2f;
                _sparkAge[i] = 0f;
            }
        }

        static Mesh Sphere(int nu, int nv)
        {
            var verts = new Vector3[(nu + 1) * (nv + 1)];
            var uvs = new Vector2[verts.Length];
            for (int v = 0; v <= nv; v++)
            {
                float t = v / (float)nv * Mathf.PI;
                for (int u = 0; u <= nu; u++)
                {
                    float a = u / (float)nu * Mathf.PI * 2f;
                    int i = v * (nu + 1) + u;
                    verts[i] = new Vector3(Mathf.Sin(t) * Mathf.Cos(a), Mathf.Cos(t), Mathf.Sin(t) * Mathf.Sin(a));
                    uvs[i] = new Vector2(u / (float)nu, v / (float)nv);
                }
            }
            var tris = new int[nu * nv * 6];
            for (int v = 0, t = 0; v < nv; v++)
                for (int u = 0; u < nu; u++)
                {
                    int a = v * (nu + 1) + u;
                    tris[t] = a; tris[t + 1] = a + nu + 1; tris[t + 2] = a + 1;
                    tris[t + 3] = a + 1; tris[t + 4] = a + nu + 2; tris[t + 5] = a + nu + 1;
                    t += 6;
                }
            var m = new Mesh { name = "forge core" };
            m.vertices = verts; m.uv = uvs; m.triangles = tris;
            m.bounds = new Bounds(Vector3.zero, Vector3.one * 12f);
            return m;
        }

        static Mesh Torus(int nu, int nv)
        {
            var verts = new Vector3[(nu + 1) * (nv + 1)];
            var uvs = new Vector2[verts.Length];
            for (int u = 0; u <= nu; u++)
            {
                float a = u / (float)nu * Mathf.PI * 2f;
                for (int v = 0; v <= nv; v++)
                {
                    float t = v / (float)nv * Mathf.PI * 2f;
                    int i = u * (nv + 1) + v;
                    Vector3 c = new Vector3(Mathf.Cos(a), 0, Mathf.Sin(a));
                    verts[i] = c * (1f + 0.06f * Mathf.Cos(t)) + new Vector3(0, 0.06f * Mathf.Sin(t), 0);
                    uvs[i] = new Vector2(u / (float)nu, v / (float)nv);
                }
            }
            var tris = new int[nu * nv * 6];
            for (int u = 0, t = 0; u < nu; u++)
                for (int v = 0; v < nv; v++)
                {
                    int a = u * (nv + 1) + v;
                    tris[t] = a; tris[t + 1] = a + nv + 1; tris[t + 2] = a + 1;
                    tris[t + 3] = a + 1; tris[t + 4] = a + nv + 2; tris[t + 5] = a + nv + 1;
                    t += 6;
                }
            var m = new Mesh { name = "gimbal torus" };
            m.vertices = verts; m.uv = uvs; m.triangles = tris;
            m.RecalculateNormals();
            m.bounds = new Bounds(Vector3.zero, Vector3.one * 6f);
            return m;
        }

        static Mesh Grid(int n)
        {
            var verts = new Vector3[(n + 1) * (n + 1)];
            var uvs = new Vector2[verts.Length];
            for (int z = 0; z <= n; z++)
                for (int x = 0; x <= n; x++)
                {
                    int i = z * (n + 1) + x;
                    verts[i] = new Vector3(x / (float)n - 0.5f, 0, z / (float)n - 0.5f);
                    uvs[i] = new Vector2(x / (float)n, z / (float)n);
                }
            var tris = new int[n * n * 6];
            for (int z = 0, t = 0; z < n; z++)
                for (int x = 0; x < n; x++)
                {
                    int a = z * (n + 1) + x;
                    tris[t] = a; tris[t + 1] = a + n + 1; tris[t + 2] = a + 1;
                    tris[t + 3] = a + 1; tris[t + 4] = a + n + 2; tris[t + 5] = a + n + 1;
                    t += 6;
                }
            var m = new Mesh { name = "floor" };
            m.vertices = verts; m.uv = uvs; m.triangles = tris;
            m.normals = new Vector3[verts.Length];
            for (int i = 0; i < verts.Length; i++) m.normals[i] = Vector3.up;
            return m;
        }

        void OnDestroy()
        {
            _sparkBuf?.Release();
        }
    }
}
