// unity_chrome: the HQ version of the 2D metaballs. Liquid chrome blobs
// (HqChrome.shader, ray traced) melting together over a black mirror floor
// in a studio of softboxes, each blob tinted by the palette like anodised
// metal.
//  - Shape: each blob swells with its own band of the spectrum (eased); a
//    kick throws one blob out of the mass, and it springs back and melts in;
//    the drop blows the cluster apart, a breakdown gathers it into one slow
//    drop of mercury.
//  - Motion: the blobs orbit on the smooth energy clock, the swirl reversing
//    with the phrase; the camera circles slowly, focused on the cluster.
// Controls: blobs, melt, spread, swirl, shine, thin film, colour tint, room
// (a softbox studio or a neon showroom in the reflections).
using UnityEngine;

namespace TrippinStage
{
    public sealed class ChromeShow : KitShow
    {
        public Material chromeMat;

        [TrippinParam("Blobs", 3f, 10f)] public float blobs = 8f;
        [TrippinParam("Melt", 0.3f, 4f)] public float melt = 1.1f;
        [TrippinParam("Spread", 0.5f, 2f)] public float spread = 1f;
        [TrippinParam("Swirl", 0f, 3f)] public float swirl = 1f;
        [TrippinParam("Shine", 0.3f, 2.5f)] public float shine = 1f;
        [TrippinParam("Thin film", 0f, 1.5f)] public float film = 0.5f;
        [TrippinParam("Colour tint", 0f, 1f)] public float tint = 0.6f;
        [TrippinParam("Room", 0f, 1f)] public float room = 1f;

        static readonly Vector3 Centre = new Vector3(0f, 7f, 0f);
        Material _m;
        readonly Vector4[] _b = new Vector4[10], _c = new Vector4[10];
        readonly Vector3[] _kickOff = new Vector3[10], _kickVel = new Vector3[10];
        readonly ClkAcc _orbit = new ClkAcc();
        float _kickPrev, _gather, _burst;
        int _kicks;

        protected override void Build()
        {
            _m = Kit.Fullscreen(transform, chromeMat, "chrome");
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            int n = Mathf.Clamp(Mathf.RoundToInt(blobs), 3, 10);
            float swing = Mathf.Sin(rx.phrase * 0.5f);
            float t = _orbit.Step(rx.clk, swirl) * 0.08f;

            // A breakdown gathers the cluster into one drop; the drop scatters it (eased).
            _gather = Eased.Follow(_gather, rx.calm, 0.5f, 0.8f, dt);
            _burst = Eased.Follow(_burst, rx.impact, 8f, 1.2f, dt);
            float reach = (6.5f + 2.4f * rx.bassSlow) * spread * (1f - 0.6f * _gather) * (1f + 0.8f * _burst);

            // A kick throws the next blob out; a spring pulls it home.
            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick)
            {
                int k = _kicks++ % n;
                var dir = new Vector3(Kit.H(_kicks, 1) - 0.5f, Kit.H(_kicks, 2) * 0.6f, Kit.H(_kicks, 3) - 0.5f).normalized;
                _kickVel[k] += dir * (9f + 8f * rx.bassFast) * spread;
            }

            for (int i = 0; i < 10; i++)
            {
                // Spring (critically-ish damped): out on the kick, home within a beat or so.
                _kickVel[i] += (-_kickOff[i] * 38f - _kickVel[i] * 7f) * dt;
                _kickOff[i] += _kickVel[i] * dt;
                if (i >= n) { _b[i] = Vector4.zero; continue; }
                float ph = i * 2.399963f;
                float a = t * (0.7f + 0.25f * (i % 3)) * (i % 2 == 0 ? 1f : -1f) + ph + 0.6f * swing;
                float el = Mathf.Sin(t * 0.9f + ph * 1.7f) * 0.55f;
                var p = Centre + new Vector3(Mathf.Cos(a) * Mathf.Cos(el), Mathf.Sin(el) * 0.8f, Mathf.Sin(a) * Mathf.Cos(el)) * reach * (0.45f + 0.55f * Kit.H(i, 7));
                p += _kickOff[i];
                float band = rx.Spec((i + 0.5f) / n * 0.85f);
                float r = (1.1f + 0.9f * Kit.H(i, 8)) * (1f + 0.45f * band + 0.2f * rx.bassFast) * (1f + 0.5f * _gather);
                _b[i] = new Vector4(p.x, Mathf.Max(p.y, r * 0.6f), p.z, r);
                Color c = Kit.Hue(i / (float)n * 0.8f + 0.05f * Mathf.Sin(t * 0.3f));
                _c[i] = new Vector4(c.r, c.g, c.b, 1f);
            }
            _m.SetVectorArray("_Blobs", _b);
            _m.SetVectorArray("_BlobCol", _c);
            _m.SetInt("_BlobCount", n);
            _m.SetFloat("_Smooth", melt * (1f + 0.6f * _gather));
            _m.SetFloat("_EnvGain", 1.4f * shine);
            _m.SetFloat("_EnvYaw", t * 0.05f);
            _m.SetFloat("_Film", film);
            _m.SetFloat("_Tint", tint);
            _m.SetFloat("_EnvMix", room);
            _m.SetFloat("_Gain", rx.Gain(0.85f, 1.15f));

            rig.Orbit(cam, rx, 17f, 8f, Centre.y, dt, 0.8f, 0.15f);
            // (No depth of field: URP's bokeh softened the blobs themselves.)
        }
    }
}
