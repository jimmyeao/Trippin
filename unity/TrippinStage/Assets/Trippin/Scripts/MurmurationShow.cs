// unity_murmuration: a starling murmuration of 1500 points of light
// wheeling through the dark, the flock folding and stretching as one.
//  - Shape: the flock's volume breathes with the slow bass; every kick
//    pulls it in tight and lets it go, and the fold travels across it as a
//    wave (each bird answers a moment after its neighbour); the highs make
//    the birds sparkle.
//  - Motion: the flock flies a slow figure through the sky on the energy
//    clock, its folds drift on noise, and the way it wheels turns round
//    with each phrase.
//  - Breakdowns: the flock spreads out and drifts; the drop pulls it in.
using UnityEngine;

namespace TrippinStage
{
    public sealed class MurmurationShow : KitShow
    {
        const int N = 1500;
        GlowPool _birds;
        Vector3[] _dir;
        float[] _rad;
        float _pull, _kickPrev, _waveX = 99f;

        protected override void Build()
        {
            _birds = new GlowPool(transform, glowMat, N, "bird", false);
            _dir = new Vector3[N];
            _rad = new float[N];
            for (int i = 0; i < N; i++)
            {
                // An ellipsoid of birds, denser at the core.
                float u = Kit.H(i, 1) * 2f - 1f, a = Kit.H(i, 2) * Mathf.PI * 2f;
                float s = Mathf.Sqrt(1f - u * u);
                _dir[i] = new Vector3(s * Mathf.Cos(a) * 1.8f, u * 0.7f, s * Mathf.Sin(a));
                _rad[i] = Mathf.Pow(Kit.H(i, 3), 0.6f);
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            float gain = rx.Gain(0.8f, 2.0f);
            Quaternion face = cam.transform.rotation;
            if (rx.kick > 0.55f && _kickPrev <= 0.55f) { _pull = 1f; _waveX = -14f; }
            _kickPrev = rx.kick;
            _pull *= Mathf.Exp(-3.2f * dt);
            _waveX += 40f * dt;
            float t = rx.clk * 0.04f;
            // The flight path: a slow figure eight, wheeling round with the phrase.
            float wheel = 0.6f * Mathf.Sin(rx.phrase * 0.5f);
            Vector3 c = new Vector3(Mathf.Sin(t) * 4f, 4f + Mathf.Sin(t * 1.7f) * 1.5f, Mathf.Sin(t * 2f) * 2.5f);
            Quaternion turn = Quaternion.Euler(0f, wheel * 40f + t * 18f, Mathf.Sin(t * 0.8f) * 12f);
            float vol = (9f + 4f * rx.bassSlow) * (1f + 0.35f * rx.calm) * (1f - 0.25f * rx.tension + 0.3f * rx.impact);
            for (int i = 0; i < N; i++)
            {
                Vector3 d = _dir[i] * _rad[i];
                // The fold: noise bends the flock, slowly.
                float nx = Mathf.PerlinNoise(d.x * 0.6f + t * 0.7f, d.z * 0.6f) - 0.5f;
                float ny = Mathf.PerlinNoise(d.z * 0.6f + 13f, d.y * 0.6f + t * 0.5f) - 0.5f;
                d += new Vector3(nx * 0.9f, ny * 0.8f, (nx - ny) * 0.6f);
                // The kick: pulled in tight, the fold rolling across the flock.
                float wv = (d.x * vol - _waveX) / 5f;
                float wave = Mathf.Exp(-wv * wv);
                float r = vol * (1f - 0.35f * _pull - 0.18f * wave);
                Vector3 p = c + turn * (d * r);
                float size = (0.32f + 0.22f * Kit.H(i, 4)) * (1f + 0.8f * rx.highFast * Kit.H(i, 5));
                float hue = 0.1f + 0.35f * _rad[i] + 0.1f * Mathf.Sin(t + d.x);
                _birds.Set(i, p, size, hue, gain * 1.8f * (0.7f + 0.6f * wave) * (0.5f + 0.5f * Kit.H(i, 6)), face);
            }
            rig.Orbit(cam, rx, 22f, 3f + 3f * Mathf.Sin(rx.phrase * 0.5f), 5.6f, dt, 1f, 0.12f);
        }
    }
}
