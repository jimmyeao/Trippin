// unity_confetti: the HQ version of the 2D confetti scene. A dark stage in
// haze, light shafts from the rig, and real confetti: tens of thousands of
// paper and foil pieces (GPU, HqConfetti.shader) tumbling and fluttering
// down through the beams, the foil flashing as it turns, the nearest pieces
// melting into bokeh.
//  - Shape: cannons on the floor fire bursts on the bar line every 4 bars
//    (their force follows the bass); the drop fires all of them at once.
//  - Motion: a steady fall from the rig that thickens with the energy; the
//    wind carrying it swings across with the phrase; the camera drifts
//    round through it.
//  - The foil's glints flare with the highs; breakdowns keep only a slow
//    drift of paper.
// Controls: amount, cannon power, piece size, foil, glints, wind, focus blur.
using UnityEngine;

namespace TrippinStage
{
    public sealed class ConfettiShow : KitShow
    {
        public ComputeShader particles;
        public Material confettiMat;

        [TrippinParam("Amount", 0.2f, 3f)] public float amount = 1f;
        [TrippinParam("Cannon power", 0.3f, 2f)] public float power = 1f;
        [TrippinParam("Piece size", 0.5f, 2f)] public float pieceSize = 1f;
        [TrippinParam("Foil", 0f, 1f)] public float foil = 0.4f;
        [TrippinParam("Glints", 0f, 3f)] public float glints = 1f;
        [TrippinParam("Wind", 0f, 3f)] public float wind = 1f;
        [TrippinParam("Focus blur", 0f, 2f)] public float blur = 1f;

        ParticleField _field;
        BeamPool _shafts;
        float _drizzle, _beatPrev = -1f, _wind;
        int _bursts;
        readonly Vector4[] _ld = new Vector4[3], _lc = new Vector4[3];
        static readonly Vector3[] Cannons = { new Vector3(-7f, 0.3f, 3f), new Vector3(7f, 0.3f, 3f), new Vector3(-4f, 0.3f, -4f), new Vector3(4f, 0.3f, -4f) };

        protected override void Build()
        {
            Env(120f, 0.07f);
            _field = new ParticleField(particles, confettiMat, 131072, false);
            _shafts = new BeamPool(transform, beamMat, 6, "shaft", false, 0.6f, 0.03f, 6f, 0.5f, 0.35f, 0.15f);
        }

        void OnDestroy() => _field?.Release();

        Color Piece(int k)
        {
            Color c = Kit.Hue(Kit.H(k, 3) * 0.9f);
            Color.RGBToHSV(c, out float h, out float s, out float v);
            return Color.HSVToRGB(h, Mathf.Max(s, 0.7f), 0.95f);
        }

        // A blast streams out over a third of a second (all at once it left a
        // solid cone of paper), fanning wide as it climbs.
        struct Blast { public Vector3 at; public float force, left; }
        readonly System.Collections.Generic.List<Blast> _blasts = new System.Collections.Generic.List<Blast>();

        void Cannon(Vector3 at, float force) => _blasts.Add(new Blast { at = at, force = force, left = 0.35f });

        void Fire(float dt)
        {
            for (int i = _blasts.Count - 1; i >= 0; i--)
            {
                var b = _blasts[i];
                float share = Mathf.Min(dt, b.left) / 0.35f;
                b.left -= dt;
                _bursts++;
                _field.Burst(b.at, Mathf.RoundToInt(5600 * amount * share), ParticleField.Shape.Cone, 22f * b.force * power, 0.075f * pieceSize, 11f,
                             Piece(_bursts), 1.5f, 0.45f, new Vector3(-b.at.x * 0.25f, 0f, -b.at.z * 0.2f), Vector3.down, 0.6f, 0, 0f, 0.6f);
                if (b.left <= 0f) _blasts.RemoveAt(i); else _blasts[i] = b;
            }
        }

        protected override void Frame(ShowState s, float dt)
        {
            dt = Mathf.Clamp(dt, 1e-4f, 1f / 20f);
            bool calm = rx.calm > 0.5f;

            // The wind swings across with the phrase (eased, never a jolt).
            _wind = Eased.Follow(_wind, Mathf.Sin(rx.phrase * 0.5f) * 2.2f * wind, 0.6f, 0.6f, dt);
            _field.Gravity = new Vector3(_wind, -4.5f, 0.4f * _wind);

            // A steady fall from the rig, thicker as the energy rises.
            _drizzle += dt * amount * (calm ? 220f : 450f + 900f * rx.lum);
            while (_drizzle >= 40f)
            {
                _drizzle -= 40f;
                _bursts++;
                // Scattered over the whole rig, not dropped in clumps.
                _field.Burst(new Vector3(0f, 17f, 0f), 40, ParticleField.Shape.Box, 0.6f, 0.075f * pieceSize, 12f, Piece(_bursts), 2.2f, 0.3f,
                             Vector3.down * 0.5f, new Vector3(13f, 1.5f, 10f), 0.8f);
            }
            // Cannons: two on the bar line every 4 bars, all four on the drop.
            float bars4 = Mathf.Floor(rx.beatS / 16f);
            if (!calm && _beatPrev >= 0f && Mathf.Floor(_beatPrev / 16f) != bars4)
            {
                int pair = (int)bars4 & 1;
                float force = 0.85f + 0.5f * rx.bassFast;
                Cannon(Cannons[pair * 2], force);
                Cannon(Cannons[pair * 2 + 1], force);
            }
            _beatPrev = rx.beatS;
            if (rx.dropped) foreach (var c in Cannons) Cannon(c, 1.35f);
            Fire(dt);

            // Light shafts from the rig, sweeping slowly; they light the confetti.
            float gain = rx.Gain(0.7f, 1.3f);
            for (int i = 0; i < 6; i++)
            {
                float a = i / 6f * Mathf.PI * 2f + 0.3f * Mathf.Sin(rx.phrase * 0.5f + i);
                var top = new Vector3(Mathf.Cos(a) * 9f, 22f, Mathf.Sin(a) * 7f);
                var foot = new Vector3(Mathf.Cos(a + 0.6f * Mathf.Sin(rx.phrase * 0.25f + i)) * 4f, 0f, Mathf.Sin(a) * 3f);
                _shafts.Set(i, top, foot, Kit.Hue(0.15f * i), gain * (0.12f + 0.25f * rx.Spec(i / 5f)));
                if (i < 3)
                {
                    var d = (top - foot).normalized;
                    _ld[i] = new Vector4(d.x, d.y, d.z, 0f);
                    Color lc = Color.Lerp(Color.white, Kit.Hue(0.15f * i), 0.5f) * (0.8f * gain);
                    _lc[i] = new Vector4(lc.r, lc.g, lc.b, 0f);
                }
            }
            _field.Mat.SetVectorArray("_LightDir", _ld);
            _field.Mat.SetVectorArray("_LightCol", _lc);
            _field.Mat.SetVector("_Ambient", new Vector4(0.05f, 0.045f, 0.06f, 0f));
            _field.Mat.SetFloat("_Foil", foil);
            _field.Mat.SetFloat("_Glint", glints * (0.5f + 1.5f * rx.highFast));
            _field.Tick(dt, gain);

            rig.Orbit(cam, rx, 11f, 3.5f, 5.5f, dt, 0.7f, 0.12f);
            float dist = (new Vector3(0f, 5.5f, 0f) - cam.transform.position).magnitude;
            HqPost.Focus(dist * 0.75f, Mathf.Lerp(8f, 1.4f, Mathf.Clamp01(blur * 0.5f)), 135f);
        }
    }
}
