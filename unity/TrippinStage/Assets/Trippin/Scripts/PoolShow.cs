// unity_pool: the HQ version of the 2D caustics scene: a swimming pool at
// night, ray traced (HqPool.shader): Alice-tiled mosaic under moving water,
// real caustics dancing on the floor, underwater lights in the palette.
//  - Shape: the bass raises the swell (eased); every kick drops a ripple
//    ring into the water at a new spot, the drop sends several at once;
//    each underwater light swells with its own band.
//  - Motion: the waves travel faster with the energy; the camera drifts
//    along the pool's edge, turning back with the phrase.
// Controls: swell, caustics, depth, ripples, lights, glints, camera height.
using UnityEngine;

namespace TrippinStage
{
    public sealed class PoolShow : KitShow
    {
        public Material poolMat;

        [TrippinParam("Swell", 0.2f, 3f)] public float swell = 1f;
        [TrippinParam("Caustics", 0f, 2.5f)] public float caustics = 1f;
        [TrippinParam("Depth", 1f, 4f)] public float depth = 2.2f;
        [TrippinParam("Ripples", 0f, 2f)] public float ripples = 1f;
        [TrippinParam("Lights", 0f, 2.5f)] public float lights = 1f;
        [TrippinParam("Glints", 0f, 3f)] public float glints = 1f;
        [TrippinParam("Camera height", 4f, 16f)] public float height = 8f;

        Material _m;
        float _t, _kickPrev, _swell;
        int _drops;
        readonly Vector4[] _rip = new Vector4[6], _lp = new Vector4[4], _lc = new Vector4[4];

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, poolMat, "pool");
            for (int i = 0; i < 6; i++) _rip[i] = new Vector4(0f, 0f, 99f, 0f);
        }

        void Ripple(float amp)
        {
            // Into the slot whose ring has faded most.
            int best = 0;
            for (int i = 1; i < 6; i++) if (_rip[i].w < _rip[best].w) best = i;
            _drops++;
            var at = new Vector2((Kit.H(_drops, 1) - 0.5f) * 9f, (Kit.H(_drops, 2) - 0.5f) * 20f);
            _rip[best] = new Vector4(at.x, at.y, 0f, amp * ripples);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            _t += dt * (0.7f + 0.7f * rx.lum);
            _swell = Eased.Follow(_swell, (0.6f + 0.6f * rx.bassSlow + 0.8f * rx.bassFast) * (1f - 0.5f * rx.calm), 3f, 1f, dt);

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick) Ripple(0.6f + 0.6f * rx.bassFast);
            if (rx.dropped) for (int k = 0; k < 4; k++) Ripple(1.3f);
            for (int i = 0; i < 6; i++)
            {
                var r = _rip[i];
                r.z += dt * 3.2f;
                r.w *= Mathf.Exp(-dt * 0.7f);
                _rip[i] = r;
            }

            float gain = rx.Gain(0.8f, 1.25f);
            for (int i = 0; i < 4; i++)
            {
                float side = i < 2 ? -1f : 1f;
                _lp[i] = new Vector4(side * 6.2f, -1.1f, (i % 2 == 0 ? -1f : 1f) * 6f, 0f);
                Color c = Kit.Hue(0.12f + 0.22f * i) * (1.6f * lights * (0.55f + 0.7f * rx.Band(i)));
                _lc[i] = new Vector4(c.r, c.g, c.b, 0f);
            }
            _m.SetVectorArray("_Rip", _rip);
            _m.SetVectorArray("_LightPos", _lp);
            _m.SetVectorArray("_LightCol", _lc);
            _m.SetFloat("_T", _t);
            _m.SetFloat("_Waves", swell * _swell);
            _m.SetFloat("_Caustic", caustics);
            _m.SetFloat("_Depth", depth);
            _m.SetFloat("_Glint", glints * (0.5f + 1.2f * rx.highFast));
            _m.SetFloat("_Gain", gain);

            float sway = Mathf.Sin(rx.phrase * 0.5f);
            rig.Move(cam, new Vector3(sway * 5f, height + 3f, -11f + 3f * Mathf.Sin(rx.phrase * 0.3f)), new Vector3(sway * 1.5f, -2f, 3f), dt, 0.6f);
        }
    }
}
