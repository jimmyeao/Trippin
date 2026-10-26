// unity_fireflies: the HQ version of the 2D firefly forest: thousands of
// fireflies (HqFirefly.shader, no stored state) drifting through a misty
// moonlit forest (an Alice backdrop), each blurred by its true distance into
// a bokeh disc by a long lens wide open, the focus racking between near and
// far through the track.
//  - Shape: the bass draws the swarm together round three drifting centres
//    (a calm passage lets it spread out and float); the mids lift it.
//  - Motion: each kick sends a flash wave rippling out through the swarm
//    from one of the centres (synchronous fireflies), the drop three at
//    once; the wander and blinks run on the smooth energy clock; the centres
//    drift and the camera eases sideways with the phrase.
// Controls: fireflies, gather, flash waves, focus pull, size, colour.
using UnityEngine;

namespace TrippinStage
{
    public sealed class FirefliesShow : KitShow
    {
        public Material flyMat, backMat;

        [TrippinParam("Fireflies", 0.2f, 1f)] public float amount = 1f;
        [TrippinParam("Gather", 0f, 2f)] public float gather = 1f;
        [TrippinParam("Flash waves", 0f, 2f)] public float waves = 1f;
        [TrippinParam("Focus pull", 0f, 1f)] public float pull = 1f;
        [TrippinParam("Size", 0.5f, 2.5f)] public float size = 1f;
        [TrippinParam("Colour", 0f, 1f)] public float palMix = 0f;

        public override bool UsesCamera => true;

        const int Max = 1200;
        Material _m, _back;
        RenderParams _rp;
        readonly ClkAcc _clk = new ClkAcc();
        readonly Vector4[] _swarm = new Vector4[3], _wave = new Vector4[4];
        float _kickPrev, _breath, _lift, _focus = 10f;
        int _nw;

        protected override void Build()
        {
            Env(120f, 0f, false);
            _m = new Material(flyMat);
            _rp = new RenderParams(_m) { worldBounds = new Bounds(new Vector3(0f, 4f, 30f), Vector3.one * 120f) };
            _back = new Material(backMat);
            _back.SetFloat("_URange", 1f);
            _back.SetFloat("_EdgeFade", 0.1f);
            _back.SetFloat("_Haze", 0.1f);
            HqBackdrops.Make(transform, _back, null, new Vector3(0f, -40f, 75f), 200f, 200f * 768f / 1344f);
            for (int i = 0; i < 4; i++) _wave[i] = new Vector4(0f, -99f, 0f, 99f);
        }

        void Wave(int centre, float strength)
        {
            var c = _swarm[centre];
            // A weaker wave starts already a little faded.
            _wave[_nw++ % 4] = new Vector4(c.x, c.y, c.z, 2f * (1f - strength));
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;
            float t = _clk.Step(rx.clk, 1f) * 0.9f;
            _breath = Eased.Follow(_breath, calm ? 0f : rx.bassFast, 14f, 3f, dt);
            _lift = Eased.Follow(_lift, rx.midFast, 10f, 2.5f, dt);

            // Three swarm centres drifting through the clearing.
            for (int k = 0; k < 3; k++)
            {
                float a = t * 0.05f + k * 2.1f;
                _swarm[k] = new Vector4(Mathf.Sin(a * 1.3f) * 7f + (k - 1) * 6f, 1.6f + Mathf.Sin(a * 0.9f + k) * 0.8f, 14f + Mathf.Cos(a) * 7f + k * 4f, 0f);
            }

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && !calm;
            _kickPrev = rx.kick;
            if (kick && waves > 0f) Wave(Mathf.FloorToInt(Mathf.Repeat(rx.beatS, 3f)), Mathf.Min(1f, waves));
            if (rx.dropped && waves > 0f) { Wave(0, 1f); Wave(1, 1f); Wave(2, 1f); }
            for (int i = 0; i < 4; i++) _wave[i].w += dt * (1.2f + 0.4f * waves);

            float gain = rx.Gain(0.8f, 1.25f);
            int n = Mathf.RoundToInt(Max * amount);
            _m.SetFloat("_T", t);
            _m.SetFloat("_Pull", Mathf.Clamp01(gather * (0.08f + 0.5f * _breath)));
            _m.SetFloat("_Lift", 1.6f * _lift);
            _m.SetFloat("_Size", size);
            _m.SetFloat("_PalMix", palMix);
            _m.SetFloat("_Gain", gain);
            _m.SetVectorArray("_Swarm", _swarm);
            _m.SetVectorArray("_Wave", _wave);
            Graphics.RenderPrimitives(_rp, MeshTopology.Triangles, n * 6);
            _back.SetFloat("_Gain", 1.5f * (0.85f + 0.2f * rx.lum));
            _back.SetFloat("_Lit", 0.4f);

            // Focus racks between the near swarm and the far one every 8 bars;
            // a breakdown lets it all melt into bokeh.
            int block = Mathf.FloorToInt(rx.beatS / 32f);
            float want = ((block % 2) + 2) % 2 == 0 ? 6f : 18f;
            want = Mathf.Lerp(11f, want, pull);
            if (calm) want = Mathf.Lerp(11f, 2.2f, pull);
            _focus = Mathf.Lerp(_focus, want, 1f - Mathf.Exp(-dt * 1.2f));
            HqPost.Focus(_focus, 2f, 135f);

            var camPos = new Vector3(Mathf.Sin(rx.phrase * 0.5f) * 1.5f, 1.5f, -2f);
            rig.Move(cam, camPos, camPos + new Vector3(0f, 0.9f, 20f), dt, 0.5f);
        }
    }
}
