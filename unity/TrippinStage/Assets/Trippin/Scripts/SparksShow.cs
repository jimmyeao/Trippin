// unity_sparks: the HQ version of the 2D sparks scene: a dark steel
// foundry where grinders on a gantry throw showers of sparks (GPU streaks)
// that arc down, bounce and skitter across the floor, with smoke drifting
// up through them, lit by the grinders.
//  - Shape: the bass feeds the spray (more sparks, flung harder); the mids
//    open the cones; each kick bursts the grinders in a pattern that walks
//    along the gantry through the bar; the drop opens every grinder at
//    once. Breakdowns leave slow drips of molten metal.
//  - Motion: the sprays swing round with the phrase, alternate grinders
//    the other way; the camera drifts along the floor.
// Controls: sparks, spray force, cone, bounce, smoke.
using UnityEngine;

namespace TrippinStage
{
    public sealed class SparksShow : KitShow
    {
        public Material sparkMat, smokeMat, backMat;
        public ComputeShader particles;

        [TrippinParam("Sparks", 0.2f, 2.5f)] public float amount = 1f;
        [TrippinParam("Spray force", 0.5f, 2f)] public float force = 1f;
        [TrippinParam("Cone", 0.2f, 2f)] public float cone = 1f;
        [TrippinParam("Bounce", 0f, 0.7f)] public float bounce = 0.35f;
        [TrippinParam("Smoke", 0f, 3f)] public float smoke = 1f;

        public override bool UsesCamera => true;

        const int N = 5;
        const float Z = 9f;
        ParticleField _sparks, _smoke;
        Material _back;
        readonly float[] _burst = new float[N], _glow = new float[N];
        readonly Vector4[] _flash = new Vector4[4], _flashPos = new Vector4[4];
        float _kickPrev, _feed, _open, _acc, _drip;

        protected override void Build()
        {
            Env(90f, 0f);
            floorMat.SetFloat("_FarFade", 42f);     // the foundry's own floor beyond
            _sparks = new ParticleField(particles, sparkMat, 131072, false);
            _sparks.FloorY = 0.02f;
            _smoke = new ParticleField(particles, smokeMat, 4096, false);
            _smoke.Gravity = new Vector3(0.3f, 0.6f, 0f);
            _back = new Material(backMat);
            _back.SetFloat("_URange", 1f);
            _back.SetFloat("_EdgeFade", 0.15f);
            _back.SetFloat("_Haze", 0.15f);
            HqBackdrops.Make(transform, _back, null, new Vector3(0f, -14f, 60f), 150f, 150f * 768f / 1344f);
        }

        void OnDestroy() { _sparks?.Release(); _smoke?.Release(); }

        static Vector3 Src(int i) => new Vector3(Mathf.Lerp(-11f, 11f, i / (N - 1f)), 5.2f + 0.8f * Mathf.Sin(i * 2.1f), Z + (i % 2) * 2.5f);

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            _feed = Eased.Follow(_feed, rx.bassFast, 16f, 3f, dt);
            _open = Eased.Follow(_open, rx.midFast, 12f, 3f, dt);
            if (kick)
            {
                // The bursts walk along the gantry through the bar: ends, inner pair, centre, all.
                int beat = Mathf.FloorToInt(Mathf.Repeat(rx.beatS, 4f));
                for (int i = 0; i < N; i++)
                {
                    int fromEdge = Mathf.Min(i, N - 1 - i);
                    if (beat == 3 || fromEdge == beat) _burst[i] = 1f;
                }
            }
            if (rx.dropped) for (int i = 0; i < N; i++) _burst[i] = 1.6f;
            _sparks.Bounce = bounce;

            float gain = rx.Gain(0.8f, 1.25f);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            int f = 0;
            _acc += dt * 60f;
            bool emit = _acc >= 1f;
            if (emit) _acc -= Mathf.Floor(_acc);
            for (int i = 0; i < N; i++)
            {
                _burst[i] = Mathf.Max(0f, _burst[i] - dt * 2.5f);
                // Each grinder idles at a trickle; the bass and its kick burst open it up.
                float on = calm ? 0.12f : 0.25f + 0.75f * _feed;
                float k = Mathf.Clamp01(on * 0.6f + _burst[i]);
                _glow[i] = Eased.Follow(_glow[i], k, 20f, 4f, dt);
                var p = Src(i);
                // The spray swings round with the phrase, alternate grinders the other way.
                float yaw = (i % 2 == 0 ? 1f : -1f) * swing * 0.9f + (i - (N - 1) * 0.5f) * 0.18f;
                var dir = new Vector3(Mathf.Sin(yaw), 0.12f, -Mathf.Cos(yaw)).normalized;
                if (emit && !calm)
                {
                    int count = Mathf.RoundToInt(amount * (4f + 40f * k));
                    float sp = force * (7f + 9f * k);
                    float c = cone * (0.12f + 0.25f * _open);
                    _sparks.Burst(p, count, ParticleField.Shape.Cone, sp, 0.022f, 1.5f,
                                  new Color(2.6f, 1.25f, 0.35f), 0.25f, 1f, default, -dir, 0.45f, 1, 0.05f, c);
                }
                if (smoke > 0f && Random.value < 0.03f * smoke * (0.3f + k))
                    _smoke.Burst(p + Vector3.down * 0.5f, 2, ParticleField.Shape.Sphere, 0.6f, 1.6f, 4.5f, Color.white, 0.5f, 0.2f, new Vector3(0f, 0.6f, 0f), default, 0.6f);
                // The grinder's hot spot: a short-lived flare at the wheel.
                if (emit && _glow[i] > 0.05f)
                    _sparks.Burst(p, 1, ParticleField.Shape.Sphere, 0f, 0.25f + 0.45f * _glow[i], 0.034f,
                                  new Color(3f, 1.6f, 0.6f) * (0.3f + 0.9f * _glow[i]), 0f, 0f);
                if (f < 4 && _glow[i] > 0.25f)
                {
                    _flash[f] = new Vector4(1.6f, 0.75f, 0.25f, 0f) * _glow[i] * 0.45f;
                    _flashPos[f] = new Vector4(p.x, p.y, p.z, 10f);
                    f++;
                }
            }
            for (; f < 4; f++) { _flash[f] = Vector4.zero; _flashPos[f] = new Vector4(0, -99, 0, 1); }

            // Breakdowns: slow drips of molten metal from the gantry.
            if (calm)
            {
                _drip += dt * 2.5f;
                if (_drip >= 1f)
                {
                    _drip -= 1f;
                    var p = Src(Random.Range(0, N)) + new Vector3(Random.Range(-1f, 1f), 0f, 0f);
                    _sparks.Burst(p, 1, ParticleField.Shape.Sphere, 0.2f, 0.06f, 2.5f, new Color(3f, 1.1f, 0.25f), 0.1f, 1f, default, default, 0.5f, 6, 0.12f);
                }
            }

            _sparks.Tick(dt, gain, 0.05f, 0.6f);
            _smoke.Mat.SetVectorArray("_Flash", _flash);
            _smoke.Mat.SetVectorArray("_FlashPos", _flashPos);
            _smoke.Tick(dt, 0.12f * smoke * gain);
            _back.SetFloat("_Gain", 0.75f * gain);
            _back.SetFloat("_Lit", 0.6f + 0.8f * _feed);

            float drift = Mathf.Sin(rx.phrase * 0.25f);
            rig.Move(cam, new Vector3(drift * 7f, 2.6f, -12f), new Vector3(drift * 3f, 2.4f, Z), dt, 0.5f);
        }
    }
}
