// Builds a festival main stage in code at startup and runs it from Trippin's
// show-state feed (TrippinLink.State):
//  - static architecture (deck, truss, frames) — never changes shape;
//  - LED walls whose content crossfades to a new pattern every 8 bars;
//  - lasers that switch formation every 4 bars, morphing in over one beat,
//    thin cores made patchy by haze (Beam.shader);
//  - swinging light shafts, haze layers, an instanced crowd bouncing on kicks;
//  - pyro flames and CO2 jets on drops (drums returning after a breakdown);
//  - a camera that drifts on phrase-length swings, smoothed (no jerks).
// Everything renders to a 1920x1080 RenderTexture that KlakSpout sends to
// Trippin as "Trippin Stage"; the window shows a preview of it.

using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    public class StageDirector : MonoBehaviour
    {
        [Header("Assigned by StageBuilder")]
        public Camera cam;
        public RenderTexture output;
        public Material beamMat, ledMat, structMat, crowdMat, hazeMat, addMat;
        public Material confettiMat, phoneMat, sunMat;

        const int LaserCount = 20;
        const int ShaftCount = 8;
        const int Formations = 6;

        readonly List<Transform> _lasers = new List<Transform>();
        readonly List<Renderer> _laserR = new List<Renderer>();
        readonly List<Quaternion> _laserFrom = new List<Quaternion>();
        readonly List<Transform> _shafts = new List<Transform>();
        readonly List<Renderer> _shaftR = new List<Renderer>();
        readonly List<Material> _walls = new List<Material>();
        readonly List<ParticleSystem> _pyro = new List<ParticleSystem>();
        readonly List<ParticleSystem> _co2 = new List<ParticleSystem>();
        MaterialPropertyBlock _mpb;
        Mesh _beamMesh, _personMesh;
        Matrix4x4[] _crowd;
        RenderParams _crowdRp;

        int _formation = -1, _prevFormation;
        float _ftSum, _ftLast;
        int _ftN;
        float _formStart;
        int _lastBar8 = -1;
        int _lastBar = -1;
        float[] _wallBlendStart;
        float _calmLong; // how long the drums have been out
        bool _drumsWas = true;
        Vector3 _camPos;
        Vector3 _camLook;

        void Start()
        {
            Application.targetFrameRate = 60;
            QualitySettings.vSyncCount = 0;
            Application.runInBackground = true;
            _mpb = new MaterialPropertyBlock();
            _beamMesh = MakeBeamMesh();
            _personMesh = MakePersonMesh();
            BuildArchitecture();
            BuildWalls();
            BuildLasers();
            BuildShafts();
            BuildHaze();
            BuildCrowd();
            BuildPyro();
            BuildFireworks();
            BuildScale();
            BuildSun();
            BuildGodRays();
            BuildKinetic();
            BuildConfetti();
            BuildPhones();
            StageRecorder.TryStart(gameObject, output);
            _camPos = new Vector3(0, 4.5f, -34);
            _camLook = new Vector3(0, 10, 12);
        }

        // ---------------------------------------------------------------- build

        static Mesh MakeBeamMesh()
        {
            var m = new Mesh { name = "beam" };
            m.vertices = new[] { new Vector3(0, 0, 0), new Vector3(1, 0, 0), new Vector3(0, 1, 0), new Vector3(1, 1, 0) };
            m.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            m.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            m.bounds = new Bounds(Vector3.zero, Vector3.one * 400f); // billboarded in the shader
            return m;
        }

        static Mesh MakePersonMesh()
        {
            var m = new Mesh { name = "person" };
            m.vertices = new[] { new Vector3(-0.5f, 0, 0), new Vector3(0.5f, 0, 0), new Vector3(-0.5f, 1, 0), new Vector3(0.5f, 1, 0) };
            m.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            m.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            m.RecalculateBounds();
            return m;
        }

        GameObject Prim(PrimitiveType t, Vector3 pos, Vector3 scale, Material mat, Vector3 euler = default, string name = null)
        {
            var g = GameObject.CreatePrimitive(t);
            Destroy(g.GetComponent<Collider>());
            g.name = name ?? t.ToString();
            g.transform.SetParent(transform, false);
            g.transform.localPosition = pos;
            g.transform.localEulerAngles = euler;
            g.transform.localScale = scale;
            var r = g.GetComponent<Renderer>();
            r.sharedMaterial = mat;
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r.receiveShadows = false;
            return g;
        }

        void BuildArchitecture()
        {
            var floor = new Material(structMat);
            floor.SetFloat("_Floor", 1);
            floor.SetFloat("_StripGain", 0);
            Prim(PrimitiveType.Quad, new Vector3(0, 0, 0), new Vector3(400, 400, 1), floor, new Vector3(90, 0, 0), "floor");
            // Deck, front lip strip, truss, wall frame, tower spines.
            Prim(PrimitiveType.Cube, new Vector3(0, 1.1f, 6), new Vector3(46, 2.2f, 14), structMat, default, "deck");
            Prim(PrimitiveType.Cube, new Vector3(0, 22.5f, 9), new Vector3(54, 0.7f, 0.7f), structMat, default, "truss");
            Prim(PrimitiveType.Cube, new Vector3(-24, 11.5f, 9), new Vector3(0.7f, 22.5f, 0.7f), structMat, default, "truss L");
            Prim(PrimitiveType.Cube, new Vector3(24, 11.5f, 9), new Vector3(0.7f, 22.5f, 0.7f), structMat, default, "truss R");
            Prim(PrimitiveType.Cube, new Vector3(0, 16.3f, 12.4f), new Vector3(27.5f, 0.6f, 0.6f), structMat, default, "frame top");
            Prim(PrimitiveType.Cube, new Vector3(0, 2.7f, 12.4f), new Vector3(27.5f, 0.6f, 0.6f), structMat, default, "frame bottom");
            Prim(PrimitiveType.Cube, new Vector3(-13.6f, 9.5f, 12.4f), new Vector3(0.6f, 14, 0.6f), structMat, default, "frame L");
            Prim(PrimitiveType.Cube, new Vector3(13.6f, 9.5f, 12.4f), new Vector3(0.6f, 14, 0.6f), structMat, default, "frame R");
            // A dark backdrop so the sky isn't empty grey.
            var back = new Material(structMat);
            back.SetFloat("_StripGain", 0);
            Prim(PrimitiveType.Quad, new Vector3(0, 30, 40), new Vector3(300, 120, 1), back, default, "backdrop");
        }

        Material Wall(Vector3 pos, Vector3 size, Vector3 euler, float cols, float rows, float hue, string name)
        {
            var m = new Material(ledMat);
            m.SetFloat("_Cols", cols);
            m.SetFloat("_Rows", rows);
            m.SetFloat("_HueOff", hue);
            m.SetFloat("_PatA", _walls.Count % 5);
            m.SetFloat("_PatB", (_walls.Count + 2) % 5);
            Prim(PrimitiveType.Quad, pos, size, m, euler, name);
            _walls.Add(m);
            return m;
        }

        // Scale: a tall rear arch with LED-edged ribs, side wings stepping
        // out, and towers of speaker stacks — the static set the light plays on.
        void BuildScale()
        {
            var steel = structMat;
            // Rear arch: ribs on a semicircle behind the crown.
            for (int i = 0; i <= 12; i++)
            {
                float a = Mathf.Lerp(0, 180, i / 12f) * Mathf.Deg2Rad;
                var p = new Vector3(Mathf.Cos(a) * 30f, 4f + Mathf.Sin(a) * 26f, 18f);
                Prim(PrimitiveType.Cube, p, new Vector3(1.2f, 7f, 1.2f), steel, new Vector3(0, 0, Mathf.Rad2Deg * a - 90f), "arch rib");
            }
            // Side wings: stepped blocks with LED faces.
            for (int s = -1; s <= 1; s += 2)
            {
                for (int k = 0; k < 3; k++)
                {
                    float x = s * (27f + k * 6f);
                    float h = 14f - k * 3.5f;
                    Prim(PrimitiveType.Cube, new Vector3(x, h * 0.5f, 6f + k * 2f), new Vector3(5.5f, h, 4f), steel, default, "wing block");
                    Wall(new Vector3(x, h * 0.5f + 1f, 3.9f + k * 2f), new Vector3(4.8f, h - 3f, 1), Vector3.zero, 32, (int)((h - 3f) * 6f), 0.25f + k * 0.08f, "wing led");
                }
                // Speaker stacks flanking the deck.
                Prim(PrimitiveType.Cube, new Vector3(s * 24.5f, 4f, 0.5f), new Vector3(2.6f, 8f, 2.6f), steel, default, "speaker stack");
            }
        }

        void BuildWalls()
        {
            Wall(new Vector3(0, 9.5f, 12.5f), new Vector3(27, 13.5f, 1), Vector3.zero, 192, 96, 0f, "main wall");
            Wall(new Vector3(-20, 11, 10), new Vector3(6.5f, 19, 1), new Vector3(0, -22, 0), 40, 116, 0.15f, "tower L");
            Wall(new Vector3(20, 11, 10), new Vector3(6.5f, 19, 1), new Vector3(0, 22, 0), 40, 116, 0.15f, "tower R");
            Wall(new Vector3(-13, 3.6f, -0.2f), new Vector3(16, 2.6f, 1), Vector3.zero, 120, 18, 0.3f, "wing L");
            Wall(new Vector3(13, 3.6f, -0.2f), new Vector3(16, 2.6f, 1), Vector3.zero, 120, 18, 0.3f, "wing R");
            // Crown: LED rays fanning above the main wall — the stage's logo shape.
            for (int i = 0; i < 9; i++)
            {
                float a = Mathf.Lerp(-64, 64, i / 8f);
                var dir = Quaternion.Euler(0, 0, a) * Vector3.up;
                var pos = new Vector3(0, 16.6f, 12.6f) + dir * 6.5f;
                Wall(pos, new Vector3(0.9f, 9, 1), new Vector3(0, 0, a), 6, 72, 0.5f + i * 0.02f, "crown " + i);
            }
            _wallBlendStart = new float[_walls.Count];
        }

        Transform Beam(Vector3 pos, float width, float spread, float core, float smoke, float fade, float hot, string name, List<Renderer> rs)
        {
            var g = new GameObject(name);
            g.transform.SetParent(transform, false);
            g.transform.localPosition = pos;
            g.transform.localScale = new Vector3(1, 90, 1);
            var mf = g.AddComponent<MeshFilter>();
            mf.sharedMesh = _beamMesh;
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
            rs.Add(mr);
            return g.transform;
        }

        void BuildLasers()
        {
            for (int i = 0; i < LaserCount; i++)
            {
                Vector3 pos = i < 12
                    ? new Vector3(Mathf.Lerp(-22, 22, i / 11f), 22f, 8.6f)
                    : new Vector3(Mathf.Lerp(-19, 19, (i - 12) / 7f), 2.4f, -0.4f);
                var t = Beam(pos, 0.035f, 0.0015f, 40, 0.75f, 0.6f, 0.6f, "laser " + i, _laserR);
                t.localRotation = Quaternion.identity;
                _lasers.Add(t);
                _laserFrom.Add(t.localRotation);
                Head(pos, i < 12 ? 1.1f : 0.8f);
            }
        }

        // A lamp-head glow at each beam source — real rigs have bright
        // fixtures where the beams start.
        readonly List<Renderer> _heads = new List<Renderer>();
        void Head(Vector3 pos, float size)
        {
            var g = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(g.GetComponent<Collider>());
            g.name = "fixture";
            g.transform.SetParent(transform, false);
            g.transform.localPosition = pos + new Vector3(0, 0, -0.6f);
            g.transform.localScale = Vector3.one * size;
            var r = g.GetComponent<Renderer>();
            r.sharedMaterial = addMat;
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            _heads.Add(r);
            // Quads have no vertex colour: tint through the mesh colours.
            var mf = g.GetComponent<MeshFilter>();
            var mesh = Instantiate(mf.sharedMesh);
            mesh.colors = new[] { Color.white, Color.white, Color.white, Color.white };
            mf.sharedMesh = mesh;
        }

        void BuildShafts()
        {
            for (int i = 0; i < ShaftCount; i++)
            {
                var pos = new Vector3(Mathf.Lerp(-20, 20, i / (ShaftCount - 1f)), 22f, 9.4f);
                _shafts.Add(Beam(pos, 0.2f, 0.05f, 30f, 0.65f, 0.35f, 0f, "shaft " + i, _shaftR));
                Head(pos, 1.6f);
            }
        }

        void BuildHaze()
        {
            float[] z = { 14, 6, -4, -16 };
            float[] d = { 0.16f, 0.12f, 0.09f, 0.06f };
            for (int i = 0; i < z.Length; i++)
            {
                var m = new Material(hazeMat);
                m.SetFloat("_Density", d[i]);
                m.SetFloat("_HueOff", i * 0.13f);
                Prim(PrimitiveType.Quad, new Vector3(0, 18, z[i]), new Vector3(140, 44, 1), m, default, "haze " + i);
            }
        }

        void BuildCrowd()
        {
            var list = new List<Matrix4x4>();
            var rnd = new System.Random(7);
            for (int i = 0; i < 700; i++)
            {
                float zz = Mathf.Lerp(-27f, -9f, (float)rnd.NextDouble());
                // Wider toward the back, a gap for the camera's centre line.
                float half = Mathf.Lerp(34f, 26f, Mathf.InverseLerp(-27f, -9f, zz));
                float xx = ((float)rnd.NextDouble() * 2f - 1f) * half;
                float h = 1.65f + (float)rnd.NextDouble() * 0.3f;
                float w = h * (0.42f + (float)rnd.NextDouble() * 0.08f);
                list.Add(Matrix4x4.TRS(new Vector3(xx, 0, zz), Quaternion.identity, new Vector3(w, h, 1)));
            }
            // Back to front isn't needed (cutout, opaque) — but sort so the
            // nearest overlap the stage cleanly.
            list.Sort((a, b) => b.m23.CompareTo(a.m23));
            _crowd = list.ToArray();
            crowdMat.enableInstancing = true;
            _crowdRp = new RenderParams(crowdMat) { shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
        }

        ParticleSystem Jet(Vector3 pos, Vector3 euler, bool co2)
        {
            var g = new GameObject(co2 ? "co2" : "pyro");
            g.transform.SetParent(transform, false);
            g.transform.localPosition = pos;
            g.transform.localEulerAngles = euler;
            var ps = g.AddComponent<ParticleSystem>();
            ps.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            var main = ps.main;
            main.playOnAwake = false;
            main.loop = false;
            main.duration = 1f;
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.maxParticles = 800;
            main.startLifetime = co2 ? new ParticleSystem.MinMaxCurve(1.2f, 2.0f) : new ParticleSystem.MinMaxCurve(0.7f, 1.2f);
            main.startSpeed = co2 ? new ParticleSystem.MinMaxCurve(16f, 22f) : new ParticleSystem.MinMaxCurve(10f, 15f);
            main.startSize = co2 ? new ParticleSystem.MinMaxCurve(0.25f, 0.5f) : new ParticleSystem.MinMaxCurve(1.0f, 1.8f);
            main.gravityModifier = co2 ? 0.25f : -0.15f;
            var em = ps.emission;
            em.rateOverTime = 0;
            var shape = ps.shape;
            shape.shapeType = ParticleSystemShapeType.Cone;
            shape.angle = co2 ? 4f : 7f;
            shape.radius = 0.15f;
            var col = ps.colorOverLifetime;
            col.enabled = true;
            var grad = new Gradient();
            if (co2)
                grad.SetKeys(
                    new[] { new GradientColorKey(new Color(0.97f, 0.98f, 1f), 0), new GradientColorKey(new Color(0.82f, 0.86f, 0.95f), 1) },
                    new[] { new GradientAlphaKey(0.0f, 0), new GradientAlphaKey(0.09f, 0.06f), new GradientAlphaKey(0.05f, 0.6f), new GradientAlphaKey(0, 1) });
            else
                grad.SetKeys(
                    new[] { new GradientColorKey(new Color(1f, 0.6f, 0.2f), 0), new GradientColorKey(new Color(0.95f, 0.28f, 0.03f), 0.3f), new GradientColorKey(new Color(0.25f, 0.03f, 0.0f), 1) },
                    new[] { new GradientAlphaKey(0.22f, 0), new GradientAlphaKey(0.14f, 0.45f), new GradientAlphaKey(0, 1) });
            col.color = grad;
            var size = ps.sizeOverLifetime;
            size.enabled = true;
            size.size = new ParticleSystem.MinMaxCurve(1f, AnimationCurve.Linear(0, 0.6f, 1, co2 ? 4f : 2f));
            var noise = ps.noise;
            noise.enabled = true;
            noise.strength = co2 ? 1.5f : 2.5f;
            noise.frequency = 0.6f;
            var r = g.GetComponent<ParticleSystemRenderer>();
            r.sharedMaterial = addMat;
            // CO2 streaks along its velocity so it reads as a jet, not a puff.
            // Both streak along their velocity: flames read as columns, CO2
            // as jets — not puffs.
            // CO2 streaks along its velocity (a jet, not a puff); flames stay
            // soft rising fireballs.
            r.renderMode = co2 ? ParticleSystemRenderMode.Stretch : ParticleSystemRenderMode.Billboard;
            r.velocityScale = 0.25f;
            r.lengthScale = 2f;
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            return ps;
        }

        // Fireworks: a shell rises over the stage and bursts into a sphere of
        // trailing sparks (sub-emitter), on every drop.
        readonly List<ParticleSystem> _fireworks = new List<ParticleSystem>();
        void BuildFireworks()
        {
            for (int i = 0; i < 5; i++)
            {
                var g = new GameObject("firework " + i);
                g.transform.SetParent(transform, false);
                g.transform.localPosition = new Vector3(Mathf.Lerp(-30, 30, i / 4f), 4, 18);
                g.transform.localEulerAngles = new Vector3(-90, 0, 0);
                var shell = g.AddComponent<ParticleSystem>();
                shell.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
                var m = shell.main;
                m.playOnAwake = false; m.loop = false;
                m.startLifetime = new ParticleSystem.MinMaxCurve(1.6f, 1.9f);
                m.startSpeed = new ParticleSystem.MinMaxCurve(17f, 21f);
                m.startSize = 0.5f;
                m.gravityModifier = 1.0f;
                m.simulationSpace = ParticleSystemSimulationSpace.World;
                m.startColor = new Color(1f, 0.85f, 0.6f, 0.8f);
                var em = shell.emission; em.rateOverTime = 0;
                var sh = shell.shape; sh.shapeType = ParticleSystemShapeType.Cone; sh.angle = 8; sh.radius = 0.1f;
                var sr = g.GetComponent<ParticleSystemRenderer>();
                sr.sharedMaterial = addMat;
                sr.renderMode = ParticleSystemRenderMode.Stretch;
                sr.velocityScale = 0.05f;

                var bg = new GameObject("burst");
                bg.transform.SetParent(g.transform, false);
                var burst = bg.AddComponent<ParticleSystem>();
                burst.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
                var bm = burst.main;
                bm.playOnAwake = false; bm.loop = false;
                bm.startLifetime = new ParticleSystem.MinMaxCurve(1.4f, 2.2f);
                bm.startSpeed = new ParticleSystem.MinMaxCurve(9f, 13f);
                bm.startSize = new ParticleSystem.MinMaxCurve(0.25f, 0.45f);
                bm.gravityModifier = 0.35f;
                bm.simulationSpace = ParticleSystemSimulationSpace.World;
                bm.maxParticles = 2000;
                var bem = burst.emission; bem.rateOverTime = 0;
                bem.SetBursts(new[] { new ParticleSystem.Burst(0f, 220) });
                var bsh = burst.shape; bsh.shapeType = ParticleSystemShapeType.Sphere; bsh.radius = 0.2f;
                var bcol = burst.colorOverLifetime; bcol.enabled = true;
                var gr = new Gradient();
                Color hue = FestiveColors[i % FestiveColors.Length];
                gr.SetKeys(new[] { new GradientColorKey(Color.white, 0), new GradientColorKey(hue, 0.15f), new GradientColorKey(hue * 0.6f, 1) },
                           new[] { new GradientAlphaKey(1, 0), new GradientAlphaKey(0.8f, 0.6f), new GradientAlphaKey(0, 1) });
                bcol.color = gr;
                var drag = burst.limitVelocityOverLifetime; drag.enabled = true; drag.drag = 1.2f;
                var tr = burst.trails; tr.enabled = true; tr.lifetime = 0.25f; tr.dieWithParticles = true;
                tr.widthOverTrail = new ParticleSystem.MinMaxCurve(0.6f);
                var br = bg.GetComponent<ParticleSystemRenderer>();
                br.sharedMaterial = addMat;
                br.trailMaterial = addMat;
                var sub = shell.subEmitters; sub.enabled = true;
                sub.AddSubEmitter(burst, ParticleSystemSubEmitterType.Death, ParticleSystemSubEmitterProperties.InheritNothing);
                _fireworks.Add(shell);
            }
        }

        void BuildPyro()
        {
            for (int i = 0; i < 10; i++)
                _pyro.Add(Jet(new Vector3(Mathf.Lerp(-21, 21, i / 9f), 2.3f, -0.6f), new Vector3(-90, 0, 0), false));
            for (int i = 0; i < 6; i++)
            {
                float x = Mathf.Lerp(-17, 17, i / 5f);
                _co2.Add(Jet(new Vector3(x, 2.3f, 0.2f), new Vector3(-75, x * 0.6f, 0), true));
            }
        }

        // ------------------------------------------------------ set pieces

        Material _sun;
        float _sunGlow;
        readonly List<Transform> _rays = new List<Transform>();
        readonly List<Renderer> _rayR = new List<Renderer>();
        readonly List<Transform> _tiles = new List<Transform>();
        readonly List<Vector3> _tileHome = new List<Vector3>();
        readonly List<ParticleSystem> _confetti = new List<ParticleSystem>();
        Matrix4x4[] _phones;
        RenderParams _phoneRp;
        Mesh _dotMesh;

        // A huge sun disc + ring behind the set that blooms on drops.
        void BuildSun()
        {
            _sun = new Material(sunMat);
            Prim(PrimitiveType.Quad, new Vector3(0, 19, 30), new Vector3(70, 70, 1), _sun, default, "sun");
        }

        // Wide backlight beams from behind the wall, sweeping through the
        // haze toward the crowd — strongest in breakdowns.
        void BuildGodRays()
        {
            for (int i = 0; i < 6; i++)
            {
                var pos = new Vector3(Mathf.Lerp(-16, 16, i / 5f), 20f, 22f);
                _rays.Add(Beam(pos, 1.2f, 0.12f, 6f, 0.8f, 0.3f, 0f, "god ray " + i, _rayR));
            }
        }

        // Kinetic rig: 6x8 LED tiles hanging over the deck, rippling up and
        // down in smooth waves on the energy clock (lights, not architecture).
        void BuildKinetic()
        {
            for (int r = 0; r < 6; r++)
                for (int c = 0; c < 8; c++)
                {
                    var home = new Vector3(Mathf.Lerp(-11, 11, c / 7f), 18.5f - r * 0.2f, Mathf.Lerp(1.5f, 10f, r / 5f));
                    Wall(home, new Vector3(2.2f, 0.9f, 1), new Vector3(-70, 0, 0), 12, 5, 0.4f + r * 0.04f, "kinetic");
                    _tiles.Add(transform.GetChild(transform.childCount - 1));
                    _tileHome.Add(home);
                }
        }

        // Confetti cannons on the deck front, palette-coloured flakes that
        // tumble down over the crowd.
        void BuildConfetti()
        {
            for (int i = 0; i < 4; i++)
            {
                var g = new GameObject("confetti " + i);
                g.transform.SetParent(transform, false);
                float x = Mathf.Lerp(-15, 15, i / 3f);
                g.transform.localPosition = new Vector3(x, 2.4f, -1f);
                g.transform.localEulerAngles = new Vector3(-60, x * 1.2f, 0);
                var ps = g.AddComponent<ParticleSystem>();
                ps.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
                var m = ps.main;
                m.playOnAwake = false; m.loop = false;
                m.startLifetime = new ParticleSystem.MinMaxCurve(5f, 8f);
                m.startSpeed = new ParticleSystem.MinMaxCurve(18f, 28f);
                m.startSize3D = true;
                m.startSizeX = new ParticleSystem.MinMaxCurve(0.18f, 0.28f);
                m.startSizeY = new ParticleSystem.MinMaxCurve(0.1f, 0.16f);
                m.startSizeZ = 1f;
                m.startRotation3D = true;
                m.gravityModifier = 0.18f;
                m.maxParticles = 2500;
                m.simulationSpace = ParticleSystemSimulationSpace.World;
                m.startColor = new ParticleSystem.MinMaxGradient(Festive()) { mode = ParticleSystemGradientMode.RandomColor };
                var em = ps.emission; em.rateOverTime = 0;
                var sh = ps.shape; sh.shapeType = ParticleSystemShapeType.Cone; sh.angle = 22; sh.radius = 0.3f;
                var col = ps.colorOverLifetime; col.enabled = true;
                var gr = new Gradient();
                gr.SetKeys(new[] { new GradientColorKey(Color.white, 0), new GradientColorKey(Color.white, 1) },
                           new[] { new GradientAlphaKey(1, 0), new GradientAlphaKey(1, 0.85f), new GradientAlphaKey(0, 1) });
                col.color = gr;
                // Air drag: a fast burst that slows and flutters down.
                var lim = ps.limitVelocityOverLifetime; lim.enabled = true; lim.drag = 1.6f;
                var rot = ps.rotationOverLifetime; rot.enabled = true; rot.separateAxes = true;
                rot.x = new ParticleSystem.MinMaxCurve(-6f, 6f);
                rot.y = new ParticleSystem.MinMaxCurve(-6f, 6f);
                rot.z = new ParticleSystem.MinMaxCurve(-4f, 4f);
                var noise = ps.noise; noise.enabled = true; noise.strength = 0.8f; noise.frequency = 0.3f;
                var r = g.GetComponent<ParticleSystemRenderer>();
                r.sharedMaterial = confettiMat;
                r.renderMode = ParticleSystemRenderMode.Mesh;
                r.mesh = MakeFlake();
                r.alignment = ParticleSystemRenderSpace.World;
                _confetti.Add(ps);
            }
        }

        // Fixed festive colours — the palette isn't known yet at build time.
        static readonly Color[] FestiveColors =
        {
            new Color(1f, 0.8f, 0.2f), new Color(1f, 0.2f, 0.6f), new Color(0.2f, 0.9f, 1f),
            new Color(1f, 1f, 1f), new Color(1f, 0.45f, 0.1f), new Color(0.6f, 0.3f, 1f),
        };

        static Gradient Festive()
        {
            var g = new Gradient();
            var keys = new GradientColorKey[FestiveColors.Length];
            for (int i = 0; i < keys.Length; i++) keys[i] = new GradientColorKey(FestiveColors[i], i / (keys.Length - 1f));
            g.SetKeys(keys, new[] { new GradientAlphaKey(1, 0), new GradientAlphaKey(1, 1) });
            g.mode = GradientMode.Fixed;
            return g;
        }

        static Mesh MakeFlake()
        {
            var m = new Mesh { name = "flake" };
            m.vertices = new[] { new Vector3(-0.5f, -0.5f, 0), new Vector3(0.5f, -0.5f, 0), new Vector3(-0.5f, 0.5f, 0), new Vector3(0.5f, 0.5f, 0) };
            m.uv = new[] { new Vector2(0, 0), new Vector2(1, 0), new Vector2(0, 1), new Vector2(1, 1) };
            m.triangles = new[] { 0, 2, 1, 1, 2, 3 };
            m.RecalculateNormals();
            return m;
        }

        // Phone lights held up over the crowd (visible in breakdowns).
        void BuildPhones()
        {
            _dotMesh = MakePersonMesh(); // a unit quad works for a dot
            var list = new List<Matrix4x4>();
            for (int i = 0; i < _crowd.Length; i++)
            {
                var c = _crowd[i];
                var p = new Vector3(c.m03, c.m13 + c.m11 * 1.18f, c.m23);
                list.Add(Matrix4x4.TRS(p, Quaternion.identity, Vector3.one * 0.35f));
            }
            _phones = list.ToArray();
            phoneMat.enableInstancing = true;
            _phoneRp = new RenderParams(phoneMat) { shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off };
        }

        void UpdateSetPieces(ShowState s, float beat, float dt)
        {
            // Sun: blooms on a drop, settles over ~8 bars, faint otherwise.
            float barsPerSec = s.bpm / 60f / 4f;
            _sunGlow = Mathf.Max(0f, _sunGlow - dt * barsPerSec / 8f);
            _sun.SetFloat("_Glow", Mathf.SmoothStep(0, 1, _sunGlow) * (1f - 0.5f * s.calm) + 0.15f * s.intensity);

            // God rays: slow sweeps, up in breakdowns, down when lasers rule.
            float phrase = beat / 64f * Mathf.PI * 2f;
            for (int i = 0; i < _rays.Count; i++)
            {
                float u = i / (_rays.Count - 1f);
                var d = Quaternion.Euler(-8f + Mathf.Sin(phrase + u * 2f) * 6f, (u - 0.5f) * 40f + Mathf.Sin(phrase * 0.5f + u * 3f) * 18f, 0) * Vector3.back;
                _rays[i].localRotation = Quaternion.FromToRotation(Vector3.up, d.normalized);
                _rays[i].localScale = new Vector3(1, 70, 1);
                _mpb.Clear();
                _mpb.SetColor("_Color", TrippinLink.Palette(0.6f + u * 0.15f));
                _mpb.SetFloat("_Intensity", 0.025f + 0.07f * s.calm);
                _rayR[i].SetPropertyBlock(_mpb);
            }

            // Kinetic tiles: a travelling wave (energy clock), bigger when loud.
            float amp = Mathf.Lerp(1.6f, 0.6f, s.calm);
            float clk = s.clock4 != null && s.clock4.Length > 1 ? s.clock4[1] : beat;
            for (int i = 0; i < _tiles.Count && i < _tileHome.Count; i++)
            {
                var h = _tileHome[i];
                float w = Mathf.Sin(clk * 0.35f + h.x * 0.25f + h.z * 0.4f);
                _tiles[i].localPosition = h + new Vector3(0, w * amp, 0);
            }

            Graphics.RenderMeshInstanced(_phoneRp, _dotMesh, 0, _phones);
        }

        // ---------------------------------------------------------------- run

        void Update()
        {
            var s = TrippinLink.State;
            float beat = s.beat;
            int bar = Mathf.FloorToInt(beat / 4f);
            float dt = Time.deltaTime;

            // Drops: drums back after at least ~4 s out.
            if (!s.drums) _calmLong += dt;
            if (s.drums && !_drumsWas && _calmLong > 4f) Drop();
            if (s.drums) _calmLong = 0f;
            _drumsWas = s.drums;
            // Peaks: a pair of flames on every 8th bar's downbeat.
            if (bar != _lastBar)
            {
                _lastBar = bar;
                if (s.drums && s.intensity > 0.75f && bar % 8 == 0)
                {
                    int k = (bar / 8) % 5;
                    Fire(_pyro[k], 40);
                    Fire(_pyro[9 - k], 40);
                }
            }

            UpdateLasers(s, beat, bar);
            UpdateSetPieces(s, beat, dt);
            UpdateShafts(s, beat);
            UpdateWalls(s, beat, bar);
            UpdateCamera(s, beat, dt);
            _ftSum += Time.unscaledDeltaTime;
            _ftN++;
            if (Time.unscaledTime - _ftLast > 5f)
            {
                Debug.Log($"[Stage] {1000f * _ftSum / _ftN:F2} ms/frame avg over {_ftN} frames, link {(TrippinLink.Live ? "live" : "synthetic")}");
                _ftSum = 0; _ftN = 0; _ftLast = Time.unscaledTime;
            }
            // Fixture glows flicker with the high hits.
            float hh = s.hits4 != null && s.hits4.Length > 3 ? s.hits4[3] : 0f;
            _mpb.Clear();
            for (int i = 0; i < _heads.Count; i++)
            {
                _mpb.SetFloat("_Gain", (0.6f + 1.4f * hh) * (1f - 0.6f * s.calm));
                _heads[i].SetPropertyBlock(_mpb);
            }
            Graphics.RenderMeshInstanced(_crowdRp, _personMesh, 0, _crowd);
        }

        void Drop()
        {
            _sunGlow = 1f;
            foreach (var c in _confetti) c.Emit(450);
            foreach (var p in _pyro) Fire(p, 110);
            foreach (var c in _co2) Fire(c, 260);
            foreach (var f in _fireworks) f.Emit(1);
        }

        static void Fire(ParticleSystem ps, int n)
        {
            ps.Emit(n);
        }

        // Laser formations — a new one every 4 bars, morphing in over a beat.
        Quaternion Aim(int i, int f, ShowState s)
        {
            bool top = i < 12;
            float u = top ? i / 11f : (i - 12) / 7f; // 0..1 across the rig
            float side = u * 2f - 1f;
            float flow = s.flow;
            float phrase = Mathf.Sin(s.beat / 32f * Mathf.PI * 2f);
            Vector3 d;
            switch (f)
            {
                case 0: // fan over the crowd, sweeping slowly
                    d = Quaternion.Euler(top ? -9 : 11, side * 52 + phrase * 14, 0) * Vector3.back;
                    break;
                case 1: // cross: each side aims across the stage
                    d = Quaternion.Euler(top ? -14 + Mathf.Sin(flow * 0.3f) * 6 : 14, -side * 38, 0) * Vector3.back;
                    break;
                case 2: // tunnel: converge on a ring that rotates around the camera
                {
                    float a = u * Mathf.PI * 2f + flow * 0.25f;
                    var target = new Vector3(Mathf.Cos(a) * 9f, 7f + Mathf.Sin(a) * 5f, -45f);
                    d = (target - _lasers[i].localPosition).normalized;
                    break;
                }
                case 3: // wave rolling across the rig
                    d = Quaternion.Euler((top ? -8 : 10) + Mathf.Sin(flow * 0.9f + u * 6f) * 12f, side * 40, 0) * Vector3.back;
                    break;
                case 4: // searchlights into the sky
                    d = Quaternion.Euler(60 + Mathf.Sin(flow * 0.4f + u * 3f) * 18f, side * 30 + phrase * 10, 0) * Vector3.back;
                    break;
                default: // lattice: alternating pitches make a grid
                    d = Quaternion.Euler((i % 2 == 0 ? -6 : 12) + phrase * 6, side * 46 * (i % 2 == 0 ? 1 : -1), 0) * Vector3.back;
                    break;
            }
            // Beam runs along local +Y.
            return Quaternion.FromToRotation(Vector3.up, d.normalized);
        }

        void UpdateLasers(ShowState s, float beat, int bar)
        {
            int f = (bar / 4) % Formations;
            if (f != _formation)
            {
                for (int i = 0; i < _lasers.Count; i++) _laserFrom[i] = _lasers[i].localRotation;
                _prevFormation = _formation;
                _formation = f;
                _formStart = beat;
            }
            float m = Mathf.SmoothStep(0, 1, Mathf.Clamp01(beat - _formStart));
            // Lasers are a drums thing: in breakdowns only every third one
            // stays, dim and slow.
            float on = Mathf.Lerp(1f, 0f, s.calm);
            for (int i = 0; i < _lasers.Count; i++)
            {
                _lasers[i].localRotation = Quaternion.Slerp(_laserFrom[i], Aim(i, f, s), m);
                bool keep = i % 3 == 0;
                float vis = Mathf.Max(on, keep ? 0.35f : 0f);
                Color c = TrippinLink.Palette(0.12f * f + i * 0.015f);
                float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b), 1e-3f);
                c = new Color(c.r / mx, c.g / mx, c.b / mx); // fully saturated
                float hit = s.hits4 != null && s.hits4.Length > 2 ? s.hits4[2] : 0f;
                _mpb.SetColor("_Color", c);
                _mpb.SetFloat("_Intensity", vis * (2.2f + 1.6f * hit) * (0.6f + 0.6f * s.intensity));
                _laserR[i].SetPropertyBlock(_mpb);
            }
        }

        void UpdateShafts(ShowState s, float beat)
        {
            for (int i = 0; i < _shafts.Count; i++)
            {
                float u = i / (_shafts.Count - 1f);
                float yaw = Mathf.Sin(beat / 32f * Mathf.PI * 2f + u * 2.5f) * 28f + (u - 0.5f) * 30f;
                float pitch = 62f + Mathf.Sin(beat / 16f * Mathf.PI * 2f + u) * 10f;
                var d = Quaternion.Euler(pitch, yaw, 0) * Vector3.back;
                // Shafts point down-forward from the truss.
                d.y = -Mathf.Abs(d.y);
                _shafts[i].localRotation = Quaternion.FromToRotation(Vector3.up, d.normalized);
                _shafts[i].localScale = new Vector3(1, 45, 1);
                Color c = TrippinLink.Palette(0.55f + u * 0.25f);
                _mpb.SetColor("_Color", c);
                float lvl = s.lvl4 != null && s.lvl4.Length > 1 ? s.lvl4[1] : 0.5f;
                _mpb.SetFloat("_Intensity", 0.05f + 0.08f * lvl + 0.05f * s.calm);
                _shaftR[i].SetPropertyBlock(_mpb);
            }
        }

        void UpdateWalls(ShowState s, float beat, int bar)
        {
            int bar8 = bar / 8;
            if (_wallBlendStart.Length != _walls.Count) System.Array.Resize(ref _wallBlendStart, _walls.Count);
            for (int i = 0; i < _walls.Count; i++)
            {
                var m = _walls[i];
                if (bar8 != _lastBar8)
                {
                    // New pattern every 8 bars, crossfaded over a beat.
                    m.SetFloat("_PatA", m.GetFloat("_PatB"));
                    int next = (int)((bar8 * 7 + i * 3 + (i > 4 ? 2 : 0)) % 5);
                    if (next == (int)m.GetFloat("_PatA")) next = (next + 1) % 5;
                    m.SetFloat("_PatB", next);
                    _wallBlendStart[i] = beat;
                }
                m.SetFloat("_Blend", Mathf.Clamp01(beat - _wallBlendStart[i]));
                m.SetFloat("_Bright", Mathf.Lerp(2.8f, 1.1f, s.calm));
            }
            _lastBar8 = bar8;
        }

        // Broadcast-style shots, a new one every 8 bars (calm sections favour
        // the slow wide ones), each drifting on phrase-length sines; the
        // camera eases between them over a couple of seconds — no cuts.
        static readonly Vector3[] ShotPos =
        {
            new Vector3(0, 4.2f, -34), // wide from the crowd
            new Vector3(-9, 1.9f, -16), // in the crowd, low, looking up
            new Vector3(14, 9, -8), // side, elevated
            new Vector3(0, 30, -48), // drone, high and wide
            new Vector3(5, 3.5f, -6), // close to the deck
        };
        static readonly Vector3[] ShotLook =
        {
            new Vector3(0, 10.5f, 12),
            new Vector3(2, 14, 10),
            new Vector3(-3, 9, 10),
            new Vector3(0, 8, 8),
            new Vector3(-2, 12, 12),
        };

        void UpdateCamera(ShowState s, float beat, float dt)
        {
            float phrase = beat / 64f * Mathf.PI * 2f;
            int bar8 = Mathf.FloorToInt(beat / 32f);
            int shot = s.calm > 0.5f ? (bar8 % 2 == 0 ? 0 : 3) : (int)((bar8 * 3 + 1) % ShotPos.Length);
            var want = ShotPos[shot] + new Vector3(Mathf.Sin(phrase) * 4f, Mathf.Sin(phrase * 0.5f) * 1.2f, 3f * s.intensity);
            var look = ShotLook[shot] + new Vector3(Mathf.Sin(phrase + 0.6f) * 2f, Mathf.Sin(phrase * 0.7f) * 1f, 0);
            float k = 1f - Mathf.Exp(-dt * 0.9f);
            _camPos = Vector3.Lerp(_camPos, want, k);
            _camLook = Vector3.Lerp(_camLook, look, k);
            cam.transform.position = _camPos;
            cam.transform.LookAt(_camLook);
        }

        void OnGUI()
        {
            if (output != null && Event.current.type == EventType.Repaint)
                GUI.DrawTexture(new Rect(0, 0, Screen.width, Screen.height), output, ScaleMode.ScaleToFit, false);
        }
    }
}
