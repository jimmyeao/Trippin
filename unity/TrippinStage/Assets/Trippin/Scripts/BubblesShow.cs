// unity_bubbles: the HQ version of the 2D bubble room: soap bubbles drifting
// up through a dark studio (HqBubble.shader: ray-traced impostors with real
// thin-film interference over reflections of softboxes and neon), soft
// coloured lights glowing far behind them.
//  - Shape: the bass makes every bubble wobble (squash and stretch); each
//    kick pops one, its film tearing away in a ring of droplets (GPU
//    sparks), and a new one floats up in its place; the drop pops a handful.
//  - Motion: the bubbles rise and swirl on a slow current that runs on the
//    eased energy and swings round with the phrase; the camera drifts.
// Controls: bubbles, size, wobble, pops, film colour, current.
using UnityEngine;

namespace TrippinStage
{
    public sealed class BubblesShow : KitShow
    {
        public Material bubbleMat, bgMat, sparkMat;
        public ComputeShader particles;

        [TrippinParam("Bubbles", 8f, 64f)] public float count = 40f;
        [TrippinParam("Size", 0.5f, 2f)] public float size = 1f;
        [TrippinParam("Wobble", 0f, 2.5f)] public float wobble = 1f;
        [TrippinParam("Pops", 0f, 1f)] public float pops = 1f;
        [TrippinParam("Film colour", 0f, 1.5f)] public float film = 1f;
        [TrippinParam("Current", 0f, 2.5f)] public float current = 1f;

        public override bool UsesCamera => true;

        const int Max = 64;
        Material _m, _bg;
        RenderParams _rp;
        ParticleField _drops;
        readonly Vector3[] _p = new Vector3[Max], _v = new Vector3[Max];
        readonly float[] _r = new float[Max], _pop = new float[Max], _seed = new float[Max];
        readonly Vector4[] _b = new Vector4[Max], _bs = new Vector4[Max], _glow = new Vector4[4];
        readonly ClkAcc _clk = new ClkAcc();
        float _kickPrev, _wob, _t;
        int _spawned;

        protected override void Build()
        {
            _bg = Kit.Fullscreen(transform, bgMat, "studio");
            _m = new Material(bubbleMat);
            _rp = new RenderParams(_m) { worldBounds = new Bounds(new Vector3(0f, 5f, 8f), Vector3.one * 60f) };
            _drops = new ParticleField(particles, sparkMat, 16384, false);
            _drops.Gravity = new Vector3(0f, -6f, 0f);
            for (int i = 0; i < Max; i++) Spawn(i, true);
        }

        void OnDestroy() { _drops?.Release(); }

        void Spawn(int i, bool anywhere)
        {
            _spawned++;
            float h = Kit.H(_spawned, 1), z = Kit.H(_spawned, 2);
            float depth = Mathf.Lerp(3f, 22f, z * z);
            _p[i] = new Vector3((h - 0.5f) * (6f + depth * 1.1f), anywhere ? Kit.H(_spawned, 3) * 11f - 1f : -2.5f - Kit.H(_spawned, 4) * 2f, depth);
            _r[i] = Mathf.Lerp(0.3f, 1.4f, Kit.H(_spawned, 5) * Kit.H(_spawned, 5));
            _v[i] = Vector3.zero;
            _pop[i] = 0f;
            _seed[i] = Kit.H(_spawned, 6);
        }

        void Pop(int i)
        {
            if (_pop[i] > 0f) return;
            _pop[i] = 0.001f;
            float r = _r[i] * size;
            _drops.Burst(_p[i], 140, ParticleField.Shape.Sphere, 2.2f + r, 0.018f, 0.7f, new Color(1.2f, 1.25f, 1.4f), 1.4f, 1f, _v[i]);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float c = _clk.Step(rx.clk, 1f);
            float rate = Mathf.Clamp((c - _t) / dt, 0f, 6f);
            _t = c;
            _wob = Eased.Follow(_wob, rx.bassFast, 16f, 3.5f, dt);
            int n = Mathf.Clamp(Mathf.RoundToInt(count), 1, Max);

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            // A kick pops the bubble nearest the middle of the view (the one you'd notice).
            if (kick && Random.value < pops)
            {
                int best = -1; float bestD = 1e9f;
                for (int i = 0; i < n; i++)
                {
                    if (_pop[i] > 0f) continue;
                    var vp = cam.WorldToViewportPoint(_p[i]);
                    if (vp.z < 1f || vp.x < 0.1f || vp.x > 0.9f || vp.y < 0.1f || vp.y > 0.9f) continue;
                    float d = (vp.x - 0.5f) * (vp.x - 0.5f) + (vp.y - 0.5f) * (vp.y - 0.5f) + Random.value * 0.05f;
                    if (d < bestD) { bestD = d; best = i; }
                }
                if (best >= 0) Pop(best);
            }
            if (rx.dropped && pops > 0f) for (int k = 0; k < 6; k++) Pop(Random.Range(0, n));

            // The current: rising, swirling, swinging round with the phrase.
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            float speed = current * (0.35f + 0.25f * rate);
            for (int i = 0; i < Max; i++)
            {
                var p = _p[i];
                var want = new Vector3(Mathf.Sin(p.y * 0.35f + c * 0.13f + _seed[i] * 6f) * 0.8f + swing * 0.6f,
                                       0.55f + 0.25f * _seed[i],
                                       Mathf.Cos(p.x * 0.3f + c * 0.11f) * 0.3f) * speed;
                _v[i] = Vector3.Lerp(_v[i], want, 1f - Mathf.Exp(-dt * 1.5f));
                _p[i] += _v[i] * dt;
                if (_pop[i] > 0f) _pop[i] += dt * 5f;
                float W = 6f + _p[i].z * 0.8f;
                if (_p[i].y > 12f || _pop[i] >= 1.5f || Mathf.Abs(_p[i].x) > W) Spawn(i, false);
                bool on = i < n;
                _b[i] = new Vector4(_p[i].x, _p[i].y, _p[i].z, on ? _r[i] * size : 0f);
                _bs[i] = new Vector4(wobble * (0.03f + 0.16f * _wob) * (0.6f + 0.8f * _seed[i]), _seed[i] * 40f, _seed[i], Mathf.Min(_pop[i], 1f));
            }

            float gain = rx.Gain(0.85f, 1.2f);
            _m.SetVectorArray("_B", _b);
            _m.SetVectorArray("_BS", _bs);
            _m.SetFloat("_T", c * 0.6f);
            _m.SetFloat("_Film", film);
            _m.SetFloat("_Gain", gain);
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, Max * 6);
            _drops.Tick(dt, gain * 0.8f, 0.03f, 0f);

            for (int k = 0; k < 4; k++)
            {
                float a = c * 0.02f + k * 1.7f;
                _glow[k] = new Vector4(Mathf.Sin(a) * 1.1f, Mathf.Cos(a * 1.3f) * 0.5f, 0.5f + 0.2f * Mathf.Sin(a * 2f + k), Mathf.Repeat(k * 0.27f + rx.beatS / 512f, 1f));
            }
            _bg.SetVectorArray("_Glow", _glow);
            _bg.SetFloat("_Gain", 0.8f + 0.4f * rx.lum);

            var camPos = new Vector3(Mathf.Sin(rx.phrase * 0.25f) * 1.2f, 4f, -4f);
            rig.Move(cam, camPos, camPos + new Vector3(0f, 0.6f, 12f), dt, 0.5f);
        }
    }
}
