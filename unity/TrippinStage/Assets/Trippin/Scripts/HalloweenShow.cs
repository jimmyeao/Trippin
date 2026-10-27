// unity_halloween: the HQ version of the 2D halloween scene: a haunted
// mansion on a hill under a full moon (an Alice backdrop), a flock of bats
// wheeling round the moon (HqBat.shader), will-o'-the-wisps drifting among
// the graves trailing green fire (GPU sparks), and banks of ground mist
// rolling through (HqFog.shader) lit by the wisps.
//  - Shape: the bass loosens the flock and lifts the mist; each kick
//    scatters the bats outward before they regroup; the drop blows them
//    wide; the mids widen the wisps' loops.
//  - Motion: the flock circles and drifts on the eased energy clock, the
//    mist rolls with the wind, the wisps wander; all swing with the phrase.
// Controls: bats, flock size, scatter, wisps, mist, wing speed.
using UnityEngine;

namespace TrippinStage
{
    public sealed class HalloweenShow : KitShow
    {
        public Material batMat, fogMat, sparkMat, backMat;
        public ComputeShader particles;

        [TrippinParam("Bats", 0f, 1f)] public float bats = 1f;
        [TrippinParam("Flock size", 0.5f, 2f)] public float flock = 1f;
        [TrippinParam("Scatter", 0f, 2f)] public float scatter = 1f;
        [TrippinParam("Wisps", 0f, 2f)] public float wisps = 1f;
        [TrippinParam("Mist", 0f, 2f)] public float mist = 1f;
        [TrippinParam("Wing speed", 0.5f, 2f)] public float wing = 1f;

        public override bool UsesCamera => true;

        const int MaxBats = 80, Wisps = 6;
        Material _bat, _fog, _back;
        RenderParams _rp;
        ParticleField _trail;
        readonly ClkAcc _clk = new ClkAcc();
        readonly Vector4[] _wisp = new Vector4[Wisps];
        float _kickPrev, _spread, _scat, _lift, _loop, _drift;

        protected override void Build()
        {
            Env(120f, 0f, false);
            _back = new Material(backMat);
            _back.SetFloat("_URange", 1f);
            _back.SetFloat("_EdgeFade", 0.05f);
            _back.SetFloat("_Haze", 0f);
            HqBackdrops.Make(transform, _back, null, new Vector3(0f, -37f, 75f), 152f, 152f * 768f / 1344f);
            _fog = Kit.Fullscreen(transform, fogMat, "mist");
            _fog.SetTexture("_Vol", HqNoise.Volume);
            _bat = new Material(batMat);
            _rp = new RenderParams(_bat) { worldBounds = new Bounds(new Vector3(0f, 10f, 30f), Vector3.one * 80f) };
            _trail = new ParticleField(particles, sparkMat, 16384, false);
            _trail.Gravity = new Vector3(0f, 0.6f, 0f);
        }

        void OnDestroy() { _trail?.Release(); }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float t = _clk.Step(rx.clk, 1f) * 0.55f;
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick) _scat = Mathf.Max(_scat, 0.55f * scatter);
            if (rx.dropped) _scat = 1.6f * scatter;
            _scat = Eased.Follow(_scat, 0f, 0f, 1.6f, dt);
            _spread = Eased.Follow(_spread, rx.bassFast, 10f, 2f, dt);
            _lift = Eased.Follow(_lift, rx.bassFast, 6f, 1.2f, dt);
            _loop = Eased.Follow(_loop, rx.midFast, 8f, 2f, dt);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            _drift += dt * (0.25f + 0.3f * rx.lum) * (0.6f + swing * 0.5f);

            // The flock wheels round the moon.
            float a = t * 0.18f;
            var centre = new Vector3(Mathf.Sin(a) * 9f + swing * 3f, 12.5f + Mathf.Sin(a * 1.3f) * 2.5f, 34f + Mathf.Cos(a) * 6f);
            _bat.SetVector("_Flock", new Vector4(centre.x, centre.y, centre.z, flock * (5f + 5f * _spread)));
            _bat.SetVector("_Scatter", new Vector4(_scat, 0f, 0f, 0f));
            _bat.SetFloat("_T", t);
            _bat.SetFloat("_Flap", wing * 3.2f / 0.55f);
            _bat.SetFloat("_Size", 2.4f);
            int n = Mathf.RoundToInt(MaxBats * bats);
            if (n > 0) Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, n * 6);

            // Will-o'-the-wisps: each leaves a trail of green fire as it wanders.
            float gain = rx.Gain(0.85f, 1.2f);
            for (int k = 0; k < Wisps; k++)
            {
                float w = t * (0.35f + 0.07f * k) + k * 1.9f;
                float r = 1.5f + 2f * _loop;
                var p = new Vector3((k - 2.5f) * 2.4f + Mathf.Sin(w) * r * 0.7f, 0.15f + 0.3f * Mathf.Sin(w * 2.3f + k), 5.5f + (k % 3) * 1.6f + Mathf.Cos(w * 0.8f) * r * 0.3f);
                if (wisps > 0f)
                {
                    float glow = wisps * (0.7f + 0.6f * rx.highFast);
                    _trail.Burst(p, 3, ParticleField.Shape.Sphere, 0.15f, 0.07f, 1.1f, new Color(0.15f, 1.5f, 0.9f) * glow, 1.5f, 1f, default, default, 0.5f);
                    _trail.Burst(p, 1, ParticleField.Shape.Sphere, 0f, 0.22f, 0.05f, new Color(0.3f, 2.2f, 1.3f) * glow, 0f, 0f);
                }
                var vp = cam.WorldToViewportPoint(p);
                _wisp[k] = new Vector4(vp.x, vp.y, vp.z > 0f ? 0.25f * wisps : 0f, 0f);
            }
            _trail.Tick(dt, gain, 0.02f, 0.3f);

            var tint = Kit.Hue(0.78f);
            _fog.SetFloat("_Top", (0.22f + 0.16f * _lift) * Mathf.Clamp(mist, 0.3f, 1.5f));
            _fog.SetFloat("_Gain", 0.35f * Mathf.Min(mist, 1f) * gain);
            _fog.SetFloat("_T", t);
            _fog.SetFloat("_Drift", _drift);
            _fog.SetVector("_Tint", new Vector4(tint.r * 0.5f, tint.g * 0.4f, tint.b * 0.6f, 1f));
            _fog.SetVectorArray("_Wisp", _wisp);
            _back.SetFloat("_Gain", 1.05f * (0.9f + 0.15f * rx.lum));
            _back.SetFloat("_Lit", 0.9f);

            var camPos = new Vector3(swing * 0.8f, 1.6f, -2f);
            rig.Move(cam, camPos, camPos + new Vector3(0f, 0.7f, 20f), dt, 0.4f);
        }
    }
}
