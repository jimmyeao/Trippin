// unity_neon_portals: the HQ version of the 2D neon portals scene: a
// corridor of real neon-tube archways (TubeRibbon: a hot white core in a
// coloured glow) standing on a wet floor that mirrors them, receding into
// haze, flown through slowly.
//  - Shape: every 8 bars the arches morph (circle, hexagon, triangle,
//    square) over a beat; each arch breathes with its band of the spectrum;
//    each kick sends a ripple through the arches' size from the far end to
//    the camera (a shape pulse, no flash: it's a flight).
//  - Motion: the flight rides the smooth energy clock; the polygons turn,
//    alternate arches the other way, swinging back with the phrase.
// Controls: speed, arch size, breathing, ripple, spin, second tube, floor glow.
using UnityEngine;

namespace TrippinStage
{
    public sealed class NeonPortalsShow : KitShow
    {
        [TrippinParam("Speed", 0f, 3f)] public float speed = 1f;
        [TrippinParam("Arch size", 0.6f, 1.6f)] public float archSize = 1f;
        [TrippinParam("Breathing", 0f, 2.5f)] public float breathe = 1f;
        [TrippinParam("Ripple", 0f, 2f)] public float ripple = 1f;
        [TrippinParam("Spin", 0f, 3f)] public float spin = 1f;
        [TrippinParam("Second tube", 0f, 1.5f)] public float second = 1f;
        [TrippinParam("Floor glow", 0f, 2f)] public float floorGlow = 1f;

        const int Arches = 12, Pts = 160;
        const float Gap = 9f, R0 = 4.2f;
        static readonly int[] Sides = { 0, 6, 3, 4 };     // 0 = circle
        TubeRibbon[] _main = new TubeRibbon[Arches], _inner = new TubeRibbon[Arches];
        GlowPool _pools;
        readonly ClkAcc _fly = new ClkAcc(), _turn = new ClkAcc();
        float _kickPrev, _morph = 1f;
        int _shape, _prevShape, _block = int.MinValue;
        readonly float[] _wave = { 99f, 99f, 99f };
        int _waves;

        protected override void Build()
        {
            Env(110f, 0.11f);
            // The light each arch throws on the wet floor below it.
            _pools = new GlowPool(transform, glowMat, Arches, "floor light", false);
            for (int i = 0; i < Arches; i++)
            {
                _main[i] = new TubeRibbon(transform, ribbonMat, Pts + 1, 8, "arch " + i, true);
                _main[i].Mat.SetFloat("_PulseAmt", 0f);
                _main[i].Mat.SetFloat("_Core", 7f);
                _main[i].Mat.SetFloat("_White", 0.55f);
                _inner[i] = new TubeRibbon(transform, ribbonMat, Pts + 1, 6, "arch inner " + i, true);
                _inner[i].Mat.SetFloat("_PulseAmt", 0f);
                _inner[i].Mat.SetFloat("_Core", 9f);
            }
        }

        // The arch's radius at angle a for a polygon of n sides (0 = circle).
        static float Shape(float a, int n)
        {
            if (n == 0) return 1f;
            float seg = Mathf.PI * 2f / n;
            float t = Mathf.Repeat(a, seg) - seg * 0.5f;
            return Mathf.Cos(Mathf.PI / n) / Mathf.Cos(t);
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            float travel = _fly.Step(rx.clk, speed) * 3.2f;
            float shift = Mathf.Repeat(travel, Gap);
            int first = Mathf.FloorToInt(travel / Gap);
            float turn = _turn.Step(rx.clk, spin) * 0.03f + 0.4f * spin * Mathf.Sin(rx.phrase * 0.5f);

            // A new shape every 8 bars, morphing in over a beat.
            int block = Mathf.FloorToInt(rx.beatS / 32f);
            if (block != _block)
            {
                if (_block != int.MinValue) { _prevShape = _shape; _morph = 0f; }
                _block = block;
                _shape = ((block % Sides.Length) + Sides.Length) % Sides.Length;
                if (_morph >= 1f) _prevShape = _shape;
            }
            _morph = Mathf.Min(1f, _morph + dt * Mathf.Clamp(s.bpm, 60f, 200f) / 60f);
            float m = Mathf.SmoothStep(0f, 1f, _morph);

            bool kick = rx.kick > 0.55f && _kickPrev <= 0.55f && rx.calm < 0.6f;
            _kickPrev = rx.kick;
            if (kick) _wave[_waves++ % 3] = 0f;
            for (int w = 0; w < 3; w++) _wave[w] += dt * 1.6f;

            float gain = rx.Gain(0.7f, 1.3f);
            for (int i = 0; i < Arches; i++)
            {
                float z = i * Gap - shift + 3f;
                int id = first + i;
                // Fade in from the haze and out as it passes the camera.
                float vis = Mathf.Clamp01((z - 1f) / 5f) * Mathf.Clamp01((Arches * Gap - z) / 25f);
                float band = rx.Spec(Mathf.Repeat(id * 0.137f, 1f) * 0.85f);
                // The kick ripple: travels from the far arches to the near ones.
                float rip = 0f;
                for (int w = 0; w < 3; w++)
                {
                    float d = (1f - z / (Arches * Gap) - _wave[w]) / 0.12f;
                    rip += Mathf.Exp(-d * d) * Mathf.Exp(-_wave[w] * 1.2f);
                }
                float r = R0 * archSize * (1f + 0.12f * breathe * band + 0.18f * ripple * rip);
                float rot = turn * (id % 2 == 0 ? 1f : -1f);
                float hue = Mathf.Repeat(id * 0.09f, 1f);
                var a = _main[i];
                var b = _inner[i];
                var centre = new Vector3(0f, r * 0.98f, z);
                for (int k = 0; k <= Pts; k++)
                {
                    float ang = k / (float)Pts * Mathf.PI * 2f;
                    float rr = Mathf.Lerp(Shape(ang - rot, Sides[_prevShape]), Shape(ang - rot, Sides[_shape]), m);
                    var dir = new Vector3(Mathf.Cos(ang), Mathf.Sin(ang), 0f);
                    a.P[k] = centre + dir * (r * rr);
                    a.R[k] = 0.13f;
                    a.K[k] = vis * (0.9f + 0.6f * band);
                    a.Hu[k] = 0f;
                    b.P[k] = centre + dir * (r * rr * 0.9f);
                    b.R[k] = 0.07f;
                    b.K[k] = vis * second * (0.7f + 0.5f * band);
                    b.Hu[k] = 0f;
                }
                // Stand it on the floor: lift so its lowest point clears it.
                float low = float.MaxValue;
                for (int k = 0; k <= Pts; k++) low = Mathf.Min(low, a.P[k].y);
                var lift = new Vector3(0f, 0.18f - low, 0f);
                for (int k = 0; k <= Pts; k++) { a.P[k] += lift; b.P[k] += lift; }
                a.Mat.SetFloat("_Hue", hue);
                b.Mat.SetFloat("_Hue", hue + 0.45f);
                a.Apply(gain * 1.4f);
                b.Apply(gain * 1.2f);
                _pools.Set(i, new Vector3(0f, 0.05f, z), r * 2.6f, hue, vis * floorGlow * gain * 0.5f * (0.8f + 0.6f * band), Quaternion.Euler(90f, 0f, 0f));
            }

            var pos = new Vector3(Mathf.Sin(rx.phrase * 0.5f) * 0.8f, 2.6f, -2f);
            rig.Move(cam, pos, pos + new Vector3(0f, 0.8f, 20f), dt, 1f);

        }
    }
}
