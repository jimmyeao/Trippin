// unity_fireworks: the HQ version of the 2D fireworks scene. A harbour at
// night: a city skyline across the water, real 3D shells bursting over it,
// mirrored in the water, with bokeh depth of field.
//  - In time: a kick launches a rocket timed to burst on the beat two beats
//    later (its climb is solved for that), so the bursts land on the beat.
//  - Shape: the bass sets the shell's size; the phrase picks the shell type
//    (peony, ring, willow, double); the highs make the dying stars crackle.
//  - The drop fires a finale of shells at once; breakdowns keep only slow
//    golden willows on the bar, and the focus softens so they bloom into
//    bokeh discs, sharpening again when the drums come back.
// Controls: shell size, star count, trails, crackle, focus blur, colour.
using System.Collections.Generic;
using UnityEngine;

namespace TrippinStage
{
    public sealed class FireworksShow : KitShow
    {
        public ComputeShader particles;
        public Material sparkMat, smokeMat, backdropMat, skyMat;

        [TrippinParam("Shell size", 0.5f, 2f)] public float shellSize = 1f;
        [TrippinParam("Stars", 0.3f, 2f)] public float stars = 1f;
        [TrippinParam("Trails", 0f, 3f)] public float trails = 1f;
        [TrippinParam("Crackle", 0f, 2f)] public float crackle = 1f;
        [TrippinParam("Smoke", 0f, 3f)] public float smoke = 1f;
        [TrippinParam("City lights", 0f, 2f)] public float city = 1f;
        [TrippinParam("Focus blur", 0f, 2f)] public float blur = 1f;
        [TrippinParam("Colour", 0f, 1f)] public float hue = 0f;

        const float BurstZ = 190f, WaterY = 0f;
        ParticleField _field, _smoke;
        // The last few bursts, lighting the smoke: colour, position, time.
        readonly Color[] _flashCol = new Color[4];
        readonly Vector4[] _flashPos = new Vector4[4], _flashV = new Vector4[4];
        readonly float[] _flashT = { -99f, -99f, -99f, -99f };
        int _flashNext;
        Material _back, _backMir, _skyM, _groundM;

        struct Rocket { public Vector3 pos, vel; public float t, T, g; public int kind; public Color col; public float size; }
        readonly List<Rocket> _rockets = new List<Rocket>();
        float _kickPrev, _lastLaunch = -9f, _soft, _beatPrev = -1f;
        int _shells;

        protected override void Build()
        {
            _field = new ParticleField(particles, sparkMat, 262144, true, WaterY);
            _smoke = new ParticleField(particles, smokeMat, 8192, false);
            _smoke.Gravity = new Vector3(0.6f, -9.8f, 0f);   // a light breeze carries it
            BuildCity();
        }

        void OnDestroy() { _field?.Release(); _smoke?.Release(); }

        void BuildCity()
        {
            // The skyline: a keyed night photo (Alice) on the far shore,
            // mirror-tiled to fill the width, and its reflection in the bay.
            _back = new Material(backdropMat);
            _back.SetFloat("_URange", 1.6f);
            _backMir = new Material(backdropMat);
            _backMir.SetFloat("_URange", 1.6f);
            _backMir.SetFloat("_Mirror", 1f);
            HqBackdrops.Make(transform, _back, _backMir, new Vector3(0f, 0f, 780f), 1700f, 1700f / 1.6f * 444f / 1536f);

            var sky = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            Destroy(sky.GetComponent<Collider>());
            sky.name = "sky";
            sky.transform.SetParent(transform, false);
            sky.transform.localScale = Vector3.one * 1700f;   // inside the 900 m far plane
            _skyM = new Material(skyMat);
            sky.GetComponent<Renderer>().sharedMaterial = _skyM;
            var water = GameObject.CreatePrimitive(PrimitiveType.Quad);
            Destroy(water.GetComponent<Collider>());
            water.name = "water";
            water.transform.SetParent(transform, false);
            water.transform.localPosition = new Vector3(0, WaterY + 0.02f, 0);
            water.transform.localRotation = Quaternion.Euler(90, 0, 0);
            water.transform.localScale = new Vector3(1700, 1700, 1);
            _groundM = new Material(groundMat);
            water.GetComponent<Renderer>().sharedMaterial = _groundM;
        }

        Color Star(float t)
        {
            // Bright, saturated firework colours from the palette.
            Color c = Kit.Hue(hue + t);
            Color.RGBToHSV(c, out float h, out float s, out float v);
            return Color.HSVToRGB(h, Mathf.Max(s, 0.75f), 1f) * 2.2f;
        }

        static readonly Color Gold = new Color(2.4f, 1.35f, 0.45f);

        void Launch(float beatLen, int kind, float side, float power)
        {
            // Burst two beats from now: climb height H in T, under its own gravity.
            float T = 2f * beatLen;
            float H = (90f + 60f * Kit.H(_shells, 3)) * Mathf.Lerp(0.85f, 1.15f, power);
            float g = 2f * H / (T * T);
            var start = new Vector3(side, WaterY + 0.5f, BurstZ + (Kit.H(_shells, 4) - 0.5f) * 60f);
            var drift = new Vector3((Kit.H(_shells, 5) - 0.5f) * 6f, 0f, 0f);
            Color col = kind == 2 ? Gold : Star(_shells * 0.17f + Kit.H(_shells, 6) * 0.2f);
            _rockets.Add(new Rocket { pos = start, vel = new Vector3(drift.x, g * T, 0f), t = 0f, T = T, g = g, kind = kind, col = col, size = power });
            _shells++;
        }

        void Burst(Rocket r)
        {
            float size = shellSize * (0.8f + 0.5f * r.size) * 2.2f;
            int n = Mathf.RoundToInt(300f * stars * (0.7f + 0.5f * r.size));
            int gh = Mathf.RoundToInt(14f * Mathf.Clamp01(trails));   // trail ghosts per star
            float tl = 0.55f * trails;
            switch (r.kind)
            {
                case 0: // peony: an even sphere of stars
                    _field.Burst(r.pos, n, ParticleField.Shape.Shell, 30f * size, 0.9f, 2.4f, r.col, 1.4f, 0.5f, r.vel * 0.1f, default, 0.08f, gh, tl);
                    break;
                case 1: // ring, tilted
                    var ax = new Vector3(Kit.H(_shells, 8) - 0.5f, 1f, Kit.H(_shells, 9) - 0.5f).normalized;
                    _field.Burst(r.pos, n / 2, ParticleField.Shape.Ring, 34f * size, 1.0f, 2.2f, r.col, 1.3f, 0.45f, default, ax, 0.04f, gh, tl);
                    _field.Burst(r.pos, n / 6, ParticleField.Shape.Sphere, 12f * size, 0.7f, 1.6f, Star(_shells * 0.17f + 0.5f), 1.6f, 0.5f, default, default, 0.15f, gh / 2, tl * 0.5f);
                    break;
                case 2: // willow: slow gold stars that droop and hang
                    _field.Burst(r.pos, n, ParticleField.Shape.Shell, 22f * size, 0.8f, 4.2f, r.col, 1.1f, 1.0f, default, default, 0.15f, gh + 4, tl * 2.2f);
                    break;
                default: // double: an inner and an outer shell in two colours
                    _field.Burst(r.pos, n * 2 / 3, ParticleField.Shape.Shell, 34f * size, 0.9f, 2.4f, r.col, 1.4f, 0.5f, default, default, 0.05f, gh, tl);
                    _field.Burst(r.pos, n / 2, ParticleField.Shape.Shell, 17f * size, 0.8f, 2.0f, Star(_shells * 0.17f + 0.45f), 1.6f, 0.5f, default, default, 0.05f, gh, tl);
                    break;
            }
            // The flash of the bursting charge, and the smoke it leaves hanging.
            _field.Burst(r.pos, 16, ParticleField.Shape.Sphere, 2f, 1.4f * size, 0.1f, new Color(3f, 2.6f, 2f), 3f, 0f);
            _smoke.Burst(r.pos, 40, ParticleField.Shape.Sphere, 6f * size, 5f * size, 11f, Color.white, 0.5f, -0.02f, default, default, 0.7f);
            _flashCol[_flashNext] = r.col;
            _flashPos[_flashNext] = new Vector4(r.pos.x, r.pos.y, r.pos.z, 45f * size);
            _flashT[_flashNext] = Time.time;
            _flashNext = (_flashNext + 1) & 3;
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            float beatLen = 60f / Mathf.Clamp(s.bpm, 60f, 200f);
            float power = Mathf.Clamp01(0.35f + 0.8f * rx.bassFast);
            int kindNow = Mathf.FloorToInt(rx.beatS / 32f) & 3;
            bool calm = rx.calm > 0.5f;

            // Rockets launch on the beat grid (every other beat, every beat
            // while the bass drives) and burst on the beat two beats later;
            // a kick off the grid adds one. Real music's kicks come and go,
            // so the grid keeps the sky busy whenever the drums play.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f;
            _kickPrev = rx.kick;
            bool onBeat = _beatPrev >= 0f && Mathf.Floor(rx.beatS) != Mathf.Floor(_beatPrev);
            int beatN = Mathf.FloorToInt(rx.beatS);
            bool grid = onBeat && ((beatN & 1) == 0 || rx.bassFast > 0.55f);
            if (!calm && (grid || kick) && Time.time - _lastLaunch > beatLen * 0.6f && _rockets.Count < 10)
            {
                _lastLaunch = Time.time;
                float side = (Kit.H(_shells, 1) - 0.5f) * 240f;
                Launch(beatLen, kindNow == 2 ? 0 : kindNow, side, power);
            }
            // Breakdowns: a slow willow on each bar.
            float bar = Mathf.Floor(rx.beatS / 4f);
            if (calm && _beatPrev >= 0f && Mathf.Floor(_beatPrev / 4f) != bar)
                Launch(beatLen, 2, (Kit.H(_shells, 1) - 0.5f) * 160f, 0.6f);
            _beatPrev = rx.beatS;
            // The drop: a finale fan of shells.
            if (rx.dropped)
                for (int i = 0; i < 6; i++)
                    Launch(beatLen, i % 4, (i - 2.5f) * 55f, 1f);

            // Rockets climb, shedding sparks, and burst at the top.
            for (int i = _rockets.Count - 1; i >= 0; i--)
            {
                var r = _rockets[i];
                r.t += dt;
                r.vel.y -= r.g * dt;
                r.pos += r.vel * dt;
                _field.Burst(r.pos, 6, ParticleField.Shape.Cone, 4f, 0.45f, 0.5f, new Color(2.2f, 1.4f, 0.6f), 2.5f, 0.3f, r.vel * 0.2f, r.vel.normalized, 0.6f);
                if (r.t >= r.T) { Burst(r); _rockets.RemoveAt(i); }
                else _rockets[i] = r;
            }

            float gain = rx.Gain(0.8f, 1.3f);
            _field.Tick(dt, gain, 0.025f * trails, crackle * (0.3f + 1.2f * rx.highFast));
            // The smoke glows with the bursts inside it: bright while the
            // stars burn, a faint afterglow as they die.
            for (int i = 0; i < 4; i++)
            {
                float age = Time.time - _flashT[i];
                float k = 0.9f * Mathf.Exp(-age * 2.5f) + 0.12f * Mathf.Exp(-age * 0.5f);
                Color c = _flashCol[i] * k;
                _flashV[i] = new Vector4(c.r, c.g, c.b, 0f);
            }
            _smoke.Mat.SetVectorArray("_Flash", _flashV);
            _smoke.Mat.SetVectorArray("_FlashPos", _flashPos);
            _smoke.Tick(dt, 0.18f * smoke * gain);

            // The city's lights answer the energy (and glow with the bursts
            // over them); the buildings never move.
            float flash = 0f;
            for (int i = 0; i < 4; i++) flash = Mathf.Max(flash, Mathf.Exp(-(Time.time - _flashT[i]) * 3f));
            foreach (var m in new[] { _back, _backMir })
            {
                m.SetFloat("_Lit", city * (0.6f + 0.6f * rx.lum));
                m.SetFloat("_Gain", 0.75f + 0.25f * flash);
                m.SetFloat("_Haze", 0.12f);
                m.SetColor("_HazeCol", Kit.Hue(hue + 0.6f) * 0.08f);
            }
            _groundM.SetFloat("_FogDist", 700f);
            _skyM.SetFloat("_Glow", 0.2f + 0.4f * rx.lum);

            // Camera: low on the near shore, drifting slowly along it.
            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 30f, 4f + 2f * Mathf.Sin(rx.phrase * 0.3f), -10f),
                new Vector3(sway * 12f, 78f, BurstZ), dt, 0.4f);
            // Focus on the bursts; a breakdown softens it into bokeh.
            _soft = Eased.Follow(_soft, calm ? 1f : 0f, 0.6f, 1.2f, dt);
            float dist = (new Vector3(0f, 120f, BurstZ) - cam.transform.position).magnitude;
            HqPost.Focus(Mathf.Lerp(dist, dist * 0.18f, _soft * blur), Mathf.Lerp(5.6f, 1.2f, _soft * Mathf.Clamp01(blur)), 200f);
        }
    }
}
