// unity_vortex: the HQ version of the 2D vortex: a tornado of light, a
// few hundred thousand sparks (HqVortex.shader, no stored state) spiralling
// up a funnel that flares toward the sky, debris skimming round its base,
// the glow it throws pooling on the wet ground, which mirrors it, under a
// storm on the horizon.
//  - Shape: the bass breathes the funnel's width; the spectrum bulges it
//    along its height (bass at the base, highs at the top); each kick sends
//    a ring bulging up the funnel; the drop throws it wide and sends two.
//  - Motion: the swirl rides the smooth energy clock (the core spins faster
//    than the rim, so it shears into filaments); the funnel's axis bends and
//    swings with the phrase; the camera circles slowly.
// Controls: particles, spin, width, twist, kick rings, sway, spark size.
using UnityEngine;

namespace TrippinStage
{
    public sealed class VortexShow : KitShow
    {
        public Material vortexMat, skyMat;

        [TrippinParam("Particles", 0.25f, 1f)] public float amount = 1f;
        [TrippinParam("Spin", 0f, 3f)] public float spin = 1f;
        [TrippinParam("Width", 0.5f, 1.8f)] public float width = 1f;
        [TrippinParam("Twist", 0f, 18f)] public float twist = 9f;
        [TrippinParam("Kick rings", 0f, 2f)] public float rings = 1f;
        [TrippinParam("Sway", 0f, 2.5f)] public float sway = 1f;
        [TrippinParam("Spark size", 0.4f, 2.5f)] public float sparkSize = 1f;

        public override bool UsesCamera => true;

        const int Max = 420000;
        const float H = 21f;
        Material _m;
        RenderParams _rp, _mirRp;
        Material _sky;
        GlowPool _pool;
        readonly ClkAcc _spin = new ClkAcc(), _rise = new ClkAcc();
        readonly Vector4[] _pulse = new Vector4[4];
        float _clkPrev, _kickPrev, _breath, _wide, _swayPh;
        int _np;

        protected override void Build()
        {
            Env(150f, 0.06f);
            _m = new Material(vortexMat);
            _rp = new RenderParams(_m) { worldBounds = new Bounds(Vector3.up * 12f, Vector3.one * 80f) };
            var mir = new MaterialPropertyBlock();
            mir.SetFloat("_Mirror", 1f);
            _mirRp = new RenderParams(_m) { worldBounds = new Bounds(Vector3.down * 12f, Vector3.one * 80f), matProps = mir };
            // The storm on the horizon, behind the tornado.
            _sky = new Material(skyMat);
            _sky.SetFloat("_URange", 1f);
            _sky.SetFloat("_EdgeFade", 0.2f);
            _sky.SetFloat("_Haze", 0f);
            HqBackdrops.Make(transform, _sky, null, new Vector3(0f, -12f, 150f), 640f, 420f * 768f / 1344f);
            _pool = new GlowPool(transform, glowMat, 2, "vortex light", false);
            for (int i = 0; i < 4; i++) _pulse[i] = new Vector4(2f, 0f, 0f, 0f);
        }

        void Ring(float amp)
        {
            _pulse[_np++ % 4] = new Vector4(0f, amp, 0f, 0f);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float clk = _spin.Step(rx.clk, spin) * 0.9f;
            float rate = (clk - _clkPrev) / dt;
            _clkPrev = clk;
            float rise = _rise.Step(rx.clk, 1f) * 0.035f;

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick) Ring(0.22f * rings);
            if (rx.dropped) { Ring(0.5f * rings); _wide = 1f; }
            // Kick rings climb the funnel in about two beats, fading as they go.
            float climb = Mathf.Clamp(s.bpm, 60f, 200f) / 60f / 2.2f;
            for (int i = 0; i < 4; i++)
            {
                _pulse[i].x += dt * climb;
                _pulse[i].y *= Mathf.Exp(-dt * 0.9f);
            }
            _wide = Mathf.Max(0f, _wide - dt * 0.35f);

            _breath = Eased.Follow(_breath, rx.bassFast, 14f, 3.5f, dt);
            _swayPh += dt * (0.25f + 0.25f * rx.lum);
            float w = width * (0.85f + 0.3f * _breath + 0.4f * Mathf.SmoothStep(0f, 1f, _wide)) * (calm ? 0.85f : 1f);

            float gain = rx.Gain(0.75f, 1.3f);
            _m.SetFloat("_Clk", clk);
            _m.SetFloat("_ClkRate", rate);
            _m.SetFloat("_Rise", rise);
            _m.SetFloat("_Width", w);
            _m.SetFloat("_Height", H);
            _m.SetFloat("_Twist", twist);
            _m.SetFloat("_Spec", 1f);
            _m.SetFloat("_Size", sparkSize);
            _m.SetFloat("_Hue", Mathf.Repeat(rx.beatS / 256f, 1f));
            _m.SetVector("_Sway", new Vector4(_swayPh + Mathf.Sin(rx.phrase * 0.5f) * 1.2f, sway * (calm ? 0.16f : 0.1f), 0f, 0f));
            _m.SetVectorArray("_Pulse", _pulse);
            int n = Mathf.RoundToInt(Max * amount);
            _m.SetFloat("_Gain", gain * 0.09f * Mathf.Sqrt(Max / (float)n));
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, n * 6);
            Graphics.RenderPrimitives(_mirRp, MeshTopology.Triangles, n * 6);
            _sky.SetFloat("_Gain", 1.1f * (0.85f + 0.25f * rx.lum));
            _sky.SetFloat("_Lit", 0.5f + _breath);

            _pool.Set(0, new Vector3(0f, 0.04f, 0f), 18f * w, Mathf.Repeat(rx.beatS / 256f, 1f), gain * 0.35f, Quaternion.Euler(90f, 0f, 0f));
            _pool.Set(1, new Vector3(0f, 0.05f, 0f), 6f * w, Mathf.Repeat(rx.beatS / 256f + 0.1f, 1f), gain * 0.5f * (0.7f + 0.6f * _breath), Quaternion.Euler(90f, 0f, 0f));

            rig.Orbit(cam, rx, 40f, 3.5f, 11.5f, dt, 0.42f, 0.15f);
        }
    }
}
